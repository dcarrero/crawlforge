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

/// Caché de `robots.txt`: **una descarga por host** durante todo el rastreo.
///
/// La caché guarda también los fallos: si un host no tiene `robots.txt`, no se vuelve a pedir
/// en cada URL. Con 50.000 URLs de un mismo dominio eso es una petición, no cincuenta mil.
///
/// # La descarga va bajo llave, no solo el resultado
///
/// Guardar el resultado no basta, y la diferencia es la primera ola de cada host. El motor
/// despacha `concurrency` URLs de golpe; las N tareas consultan la caché **antes** de que
/// ninguna haya terminado de descargar, las N fallan el `get` y las N piden el fichero.
/// Medido: **122 peticiones de `robots.txt` para 25 hosts**, 4,9x — exactamente la
/// concurrencia por host. Y esa ráfaga simultánea es justo la que el `Crawl-delay` existe
/// para evitar, con el agravante de que ocurre antes de poder saber que el host lo declara.
///
/// Por eso cada host guarda una celda ([`tokio::sync::OnceCell`]) y no un valor: la primera
/// tarea que llega descarga y las demás **esperan a su resultado** en vez de repetirlo.
#[derive(Default)]
pub struct RobotsCache {
    hosts: RwLock<HashMap<String, Arc<tokio::sync::OnceCell<Arc<HostRules>>>>>,
}

impl RobotsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Devuelve las reglas del host si ya están cargadas. `None` también mientras otra tarea
    /// las está descargando: quien quiera esperar a esa descarga usa [`Self::get_or_fetch`].
    pub async fn get(&self, host: &str) -> Option<Arc<HostRules>> {
        self.hosts.read().await.get(host).and_then(|cell| cell.get().cloned())
    }

    /// Las reglas del host, descargándolas **una sola vez** aunque lleguen N tareas a la vez.
    ///
    /// `fetch` solo se ejecuta si esta llamada es la primera del host; el resto de tareas
    /// esperan a que termine y comparten su resultado.
    pub async fn get_or_fetch<F, Fut>(&self, host: &str, fetch: F) -> Arc<HostRules>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = HostRules>,
    {
        // Camino caliente: cerrojo de lectura y un `Arc` clonado. El de escritura solo se toma
        // la primera vez que se ve un host.
        let known = {
            let guard = self.hosts.read().await;
            guard.get(host).map(Arc::clone)
        };
        let cell = match known {
            Some(cell) => cell,
            None => {
                let mut guard = self.hosts.write().await;
                Arc::clone(guard.entry(host.to_string()).or_default())
            }
        };
        if let Some(rules) = cell.get() {
            return Arc::clone(rules);
        }
        Arc::clone(cell.get_or_init(|| async { Arc::new(fetch().await) }).await)
    }

    /// Guarda las reglas de un host. Si otra tarea se adelantó, gana la primera: el contenido
    /// es el mismo y así no se invalidan las referencias ya repartidas.
    pub async fn insert(&self, host: String, rules: HostRules) -> Arc<HostRules> {
        let cell = {
            let mut guard = self.hosts.write().await;
            Arc::clone(guard.entry(host).or_default())
        };
        Arc::clone(cell.get_or_init(|| async { Arc::new(rules) }).await)
    }

    /// Todo lo cacheado, para volcarlo al almacén al terminar el rastreo.
    ///
    /// El `robots.txt` se descarga una vez por host y vive en este caché; sin este volcado, el
    /// fichero de rastreo no conserva ni rastro de él.
    pub async fn snapshot(&self) -> Vec<(String, Arc<HostRules>)> {
        self.hosts
            .read()
            .await
            .iter()
            .filter_map(|(host, cell)| cell.get().map(|r| (host.clone(), Arc::clone(r))))
            .collect()
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

    #[tokio::test]
    async fn the_first_wave_of_a_host_downloads_its_robots_txt_once() {
        // The measured defect: check-then-fetch with no door. The N tasks of a host's first
        // wave all miss the cache before any of them has finished downloading, so all N
        // download — 122 requests for 25 hosts, exactly the per-host concurrency.
        use std::sync::atomic::{AtomicUsize, Ordering};

        let cache = Arc::new(RobotsCache::new());
        let downloads = Arc::new(AtomicUsize::new(0));
        // The download is slow on purpose: without the door every task gets past the `get`
        // while the first one is still in flight, which is the whole point.
        let fetch = |downloads: Arc<AtomicUsize>| async move {
            downloads.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            HostRules::parse(b"User-agent: *\nDisallow: /privado/", UA)
        };

        let mut wave = tokio::task::JoinSet::new();
        for _ in 0..8 {
            let cache = Arc::clone(&cache);
            let downloads = Arc::clone(&downloads);
            wave.spawn(async move {
                cache.get_or_fetch("ejemplo.es", || fetch(downloads)).await
            });
        }
        while let Some(joined) = wave.join_next().await {
            let rules = joined.expect("the task does not die");
            assert!(!rules.allows(&url("https://ejemplo.es/privado/x")), "everyone gets the rules");
        }

        assert_eq!(
            downloads.load(Ordering::SeqCst),
            1,
            "eight tasks of one host must download its robots.txt once"
        );
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn each_host_has_its_own_door() {
        // The door serialises one host, never the crawl: a slow robots.txt on one host cannot
        // hold up the others.
        let cache = RobotsCache::new();
        for host in ["uno.es", "dos.es", "tres.es"] {
            cache.get_or_fetch(host, || async { HostRules::allow_all() }).await;
        }
        assert_eq!(cache.len().await, 3);
        assert_eq!(cache.snapshot().await.len(), 3);
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
