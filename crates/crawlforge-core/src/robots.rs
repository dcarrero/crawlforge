//! `robots.txt`: análisis, caché por host y decisión de exclusión.
//! Ver `docs/03-MOTOR-CRAWL.md §4`.
//!
//! Dos principios que no son negociables:
//!
//! - `Crawl-delay` **anula** la concurrencia configurada por el usuario para ese host.
//!   Un crawler que tumba el servidor del cliente es un crawler inservible.
//! - Una URL bloqueada **no se oculta**: se registra con `crawl_state='excluded'` y
//!   `exclusion_reason='robots'`. Saber qué está bloqueado es un hallazgo en sí mismo.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use texting_robots::Robot;
use tokio::sync::RwLock;
use url::Url;

/// Tope del `Crawl-delay` que se obedece.
///
/// El valor lo pone el sitio rastreado, así que es entrada hostil. `texting_robots` solo valida
/// que sea `>= 0.0`, lo que **acepta `inf`**, y `Duration::from_secs_f32(inf)` hace panic: un
/// `robots.txt` con `Crawl-delay: inf` tumbaba el worker, y como el `JoinSet` del motor se traga
/// el panic, el rastreo terminaba «bien» con cero URLs y sin decir por qué. Eso es peor que caer.
///
/// El tope existe además por un segundo motivo, sin panic de por medio: el retardo se aplica por
/// URL y `max_duration` solo se comprueba al terminar cada una, así que un `Crawl-delay: 86400`
/// deja el rastreo colgado un día por página sin forma de enterarse. Treinta segundos es más de
/// lo que declara cualquier sitio real y sigue siendo obedecer.
pub const MAX_CRAWL_DELAY: Duration = Duration::from_secs(30);

/// Convierte el `Crawl-delay` declarado en una espera que se puede obedecer sin caerse.
fn sane_crawl_delay(declarado: f32) -> Option<Duration> {
    match Duration::try_from_secs_f32(declarado) {
        Ok(d) if d > MAX_CRAWL_DELAY => {
            tracing::warn!(
                declarado,
                tope_s = MAX_CRAWL_DELAY.as_secs(),
                "Crawl-delay recortado al tope"
            );
            Some(MAX_CRAWL_DELAY)
        }
        Ok(d) => Some(d),
        // `inf`, `NaN` o un valor que no cabe en `Duration`. No es un retardo, es un intento de
        // romper el rastreo: se ignora y se sigue con la concurrencia configurada.
        Err(_) => {
            tracing::warn!(declarado, "Crawl-delay no representable: se ignora");
            None
        }
    }
}

/// Lo que el motor necesita saber de un host tras leer su `robots.txt`.
pub struct HostRules {
    robot: Option<Robot>,
    /// `Crawl-delay` declarado, si lo hay.
    pub crawl_delay: Option<Duration>,
    /// Sitemaps anunciados con `Sitemap:`.
    pub sitemaps: Vec<String>,
    /// Código con el que respondió `/robots.txt`. `None` si no se llegó a pedir.
    ///
    /// Se guarda para poder distinguir en el informe «este sitio no tiene robots.txt» de «no
    /// pudimos leerlo», que son cosas distintas: la primera es un aviso menor y la segunda no
    /// permite afirmar nada.
    pub status_code: Option<u16>,
    /// El fichero tal cual, para explicar el hallazgo y para comparar dos rastreos.
    pub content: Option<String>,
}

impl HostRules {
    /// Analiza un `robots.txt`.
    ///
    /// Si el fichero está corrupto o no se puede interpretar, se permite todo. Es la
    /// interpretación conservadora correcta: un `robots.txt` ilegible no es una prohibición,
    /// y bloquear el rastreo entero de un sitio propio por un fichero mal formado sería peor
    /// que rastrearlo.
    pub fn parse(body: &[u8], user_agent: &str) -> Self {
        match Robot::new(user_agent, body) {
            Ok(robot) => Self {
                crawl_delay: robot.delay.and_then(sane_crawl_delay),
                sitemaps: robot.sitemaps.clone(),
                robot: Some(robot),
                status_code: Some(200),
                content: Some(String::from_utf8_lossy(body).into_owned()),
            },
            Err(e) => {
                tracing::warn!(error = %e, "robots.txt ilegible: se permite todo en este host");
                let mut rules = Self::allow_all();
                rules.status_code = Some(200);
                rules.content = Some(String::from_utf8_lossy(body).into_owned());
                rules
            }
        }
    }

    /// Host sin `robots.txt` (404) o inalcanzable: todo permitido.
    pub fn allow_all() -> Self {
        Self {
            robot: None,
            crawl_delay: None,
            sitemaps: Vec::new(),
            status_code: None,
            content: None,
        }
    }

    /// Como [`Self::allow_all`], pero dejando constancia de qué respondió el servidor.
    pub fn absent(status_code: Option<u16>) -> Self {
        Self { status_code, ..Self::allow_all() }
    }

    /// ¿Este `robots.txt` prohíbe rastrear el sitio entero a nuestro user-agent?
    ///
    /// Se evalúa con el parser y no buscando la cadena `Disallow: /`: esa línea puede estar bajo
    /// otro `User-agent` y no aplicarnos, y darla por buena sería el falso positivo más caro del
    /// catálogo —decirle a alguien que su sitio está bloqueado cuando no lo está—.
    pub fn blocks_all(&self, base: &Url) -> bool {
        self.robot.is_some() && !self.allows(base)
    }

    /// ¿Permite el `robots.txt` de este host rastrear esta URL?
    pub fn allows(&self, url: &Url) -> bool {
        match &self.robot {
            Some(r) => r.allowed(url.as_str()),
            None => true,
        }
    }
}

/// Caché de `robots.txt`: un fichero por host, durante todo el rastreo.
///
/// La caché guarda también los fallos: si un host no tiene `robots.txt`, no se vuelve a pedir
/// en cada URL. Con 50.000 URLs de un mismo dominio eso es una petición, no cincuenta mil.
#[derive(Default)]
pub struct RobotsCache {
    hosts: RwLock<HashMap<String, Arc<HostRules>>>,
}

impl RobotsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Devuelve las reglas del host si ya están cacheadas.
    pub async fn get(&self, host: &str) -> Option<Arc<HostRules>> {
        self.hosts.read().await.get(host).cloned()
    }

    /// Guarda las reglas de un host. Si otra tarea se adelantó, gana la primera: el contenido
    /// es el mismo y así no se invalidan las referencias ya repartidas.
    pub async fn insert(&self, host: String, rules: HostRules) -> Arc<HostRules> {
        let mut guard = self.hosts.write().await;
        guard.entry(host).or_insert_with(|| Arc::new(rules)).clone()
    }

    /// Todo lo cacheado, para volcarlo al almacén al terminar el rastreo.
    ///
    /// El `robots.txt` se descarga una vez por host y vive en este caché; sin este volcado, el
    /// fichero de rastreo no conserva ni rastro de él.
    pub async fn snapshot(&self) -> Vec<(String, Arc<HostRules>)> {
        self.hosts.read().await.iter().map(|(h, r)| (h.clone(), Arc::clone(r))).collect()
    }

    /// Número de hosts cacheados. Para métricas y tests.
    pub async fn len(&self) -> usize {
        self.hosts.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

/// URL del `robots.txt` de un host, a partir de cualquier URL suya.
pub fn robots_url_for(url: &Url) -> Option<Url> {
    url.join("/robots.txt").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UA: &str = "CrawlForge";

    fn url(s: &str) -> Url {
        Url::parse(s).expect("URL de test válida")
    }

    #[test]
    fn respeta_disallow_del_user_agent_configurado() {
        let rules = HostRules::parse(b"User-agent: CrawlForge\nDisallow: /privado/", UA);
        assert!(!rules.allows(&url("https://ejemplo.es/privado/x")));
        assert!(rules.allows(&url("https://ejemplo.es/publico/x")));
    }

    #[test]
    fn cae_al_comodin_cuando_no_hay_grupo_propio() {
        let rules = HostRules::parse(b"User-agent: *\nDisallow: /admin/", UA);
        assert!(!rules.allows(&url("https://ejemplo.es/admin/panel")));
        assert!(rules.allows(&url("https://ejemplo.es/blog/post")));
    }

    #[test]
    fn el_grupo_propio_tiene_prioridad_sobre_el_comodin() {
        let txt = b"User-agent: *\nDisallow: /\n\nUser-agent: CrawlForge\nDisallow: /solo-esto/";
        let rules = HostRules::parse(txt, UA);
        assert!(rules.allows(&url("https://ejemplo.es/blog")), "el comodín no debe aplicarse");
        assert!(!rules.allows(&url("https://ejemplo.es/solo-esto/x")));
    }

    #[test]
    fn lee_el_crawl_delay() {
        let rules = HostRules::parse(b"User-agent: *\nCrawl-delay: 2", UA);
        assert_eq!(rules.crawl_delay, Some(Duration::from_secs(2)));
    }

    #[test]
    fn sin_crawl_delay_no_inventa_ninguno() {
        let rules = HostRules::parse(b"User-agent: *\nDisallow: /x", UA);
        assert_eq!(rules.crawl_delay, None);
    }

    #[test]
    fn descubre_los_sitemaps_anunciados() {
        let txt = b"Sitemap: https://ejemplo.es/sitemap.xml\n\
                    Sitemap: https://ejemplo.es/sitemap-news.xml\n\
                    User-agent: *\nDisallow:";
        let rules = HostRules::parse(txt, UA);
        assert_eq!(rules.sitemaps.len(), 2);
        assert!(rules.sitemaps.contains(&"https://ejemplo.es/sitemap.xml".to_string()));
    }

    #[test]
    fn un_robots_vacio_permite_todo() {
        let rules = HostRules::parse(b"", UA);
        assert!(rules.allows(&url("https://ejemplo.es/lo-que-sea")));
    }

    #[test]
    fn un_robots_ilegible_permite_todo_en_vez_de_bloquearlo_todo() {
        // Bloquear el sitio entero por un fichero corrupto sería peor que rastrearlo.
        let rules = HostRules::parse(b"\xff\xfe basura binaria \x00\x01", UA);
        assert!(rules.allows(&url("https://ejemplo.es/x")));
    }

    #[test]
    fn allow_all_no_bloquea_nada() {
        let rules = HostRules::allow_all();
        assert!(rules.allows(&url("https://ejemplo.es/privado/x")));
        assert!(rules.sitemaps.is_empty());
    }

    #[test]
    fn construye_la_url_de_robots_desde_cualquier_url_del_host() {
        let got = robots_url_for(&url("https://ejemplo.es/blog/post?x=1")).expect("robots url");
        assert_eq!(got.as_str(), "https://ejemplo.es/robots.txt");
    }

    #[tokio::test]
    async fn la_cache_devuelve_lo_guardado_y_no_repite_hosts() {
        let cache = RobotsCache::new();
        assert!(cache.is_empty().await);
        assert!(cache.get("ejemplo.es").await.is_none());

        let rules = HostRules::parse(b"User-agent: *\nDisallow: /x", UA);
        cache.insert("ejemplo.es".into(), rules).await;

        let got = cache.get("ejemplo.es").await.expect("debería estar cacheado");
        assert!(!got.allows(&url("https://ejemplo.es/x")));
        assert_eq!(cache.len().await, 1);

        // Insertar el mismo host otra vez no lo duplica.
        cache.insert("ejemplo.es".into(), HostRules::allow_all()).await;
        assert_eq!(cache.len().await, 1);
    }
}

#[cfg(test)]
mod tests_crawl_delay {
    use super::*;

    #[test]
    fn un_crawl_delay_infinito_no_tumba_el_rastreo() {
        // Regresión: `texting_robots` acepta `inf` porque solo valida `>= 0.0`, y
        // `Duration::from_secs_f32(inf)` hace panic. El worker moría, el `JoinSet` se tragaba el
        // panic y el rastreo terminaba con cero URLs sin decir por qué.
        let rules = HostRules::parse(b"User-agent: *\nCrawl-delay: inf\n", "crawlforge");
        assert_eq!(rules.crawl_delay, None, "un retardo no representable se ignora");
    }

    #[test]
    fn un_crawl_delay_desmedido_se_recorta_en_vez_de_colgar_el_rastreo() {
        // El retardo se aplica por URL, así que 86.400 s son 24 horas por página.
        let rules = HostRules::parse(b"User-agent: *\nCrawl-delay: 86400\n", "crawlforge");
        assert_eq!(rules.crawl_delay, Some(MAX_CRAWL_DELAY));
    }

    #[test]
    fn un_crawl_delay_normal_se_obedece_tal_cual() {
        // Recortar no es ignorar: lo que declara un sitio real se respeta.
        let rules = HostRules::parse(b"User-agent: *\nCrawl-delay: 2\n", "crawlforge");
        assert_eq!(rules.crawl_delay, Some(Duration::from_secs(2)));
    }

    #[test]
    fn un_crawl_delay_negativo_o_absurdo_no_rompe_nada() {
        for valor in ["-1", "NaN", "1e40", "abc"] {
            let robots = format!("User-agent: *\nCrawl-delay: {valor}\n");
            let rules = HostRules::parse(robots.as_bytes(), "crawlforge");
            assert!(
                rules.crawl_delay.is_none() || rules.crawl_delay <= Some(MAX_CRAWL_DELAY),
                "Crawl-delay {valor} dio {:?}",
                rules.crawl_delay
            );
        }
    }
}
