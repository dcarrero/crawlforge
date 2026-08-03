//! Definición de un rastreo: modo, semillas, límites y presupuesto.
//! Ver `docs/03-MOTOR-CRAWL.md §1 y §9`.

use crate::normalize::NormalizePolicy;
use std::path::PathBuf;
use std::time::Duration;

/// De dónde salen las URLs de un rastreo.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase", tag = "mode")]
pub enum CrawlMode {
    /// Rastreo normal, desde una URL semilla.
    Http { seed: String },
    /// Auditoría de un directorio ya construido. **El diferenciador.**
    Filesystem { root: PathBuf, base: String },
    /// Un conjunto concreto de URLs, pegado o importado.
    List { urls: Vec<String> },
}

impl CrawlMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http { .. } => "http",
            Self::Filesystem { .. } => "filesystem",
            Self::List { .. } => "list",
        }
    }
}

/// Credencial de autenticación básica HTTP para auditar un *staging* protegido.
///
/// # Por qué existe este tipo y no un `usuario:contraseña` en la URL
///
/// La revisión 2026-08-01 §1.6 retiró el *userinfo* de las URLs porque la contraseña acababa en
/// `crawl_meta`, en cada fila de `urls`, en los exports y en el nombre del fichero — todo lo que
/// viaja en el entregable. Pero auditar un pre-producción protegido con Basic Auth es trabajo
/// normal de un consultor SEO, así que la función vuelve por aquí: una credencial **fuera de la
/// URL**, que el fetcher convierte en cabecera `Authorization` y que está **acotada al host de
/// la semilla** — jamás se manda a otro host, ni siquiera con `follow_external`.
///
/// Dos propiedades no negociables, cada una con su test:
///
/// - **No se serializa.** El campo que la transporta lleva `#[serde(skip)]` y este tipo no
///   deriva `Serialize` a propósito: si alguien intentara guardarlo en `config_json` — que se
///   escribe dentro del fichero de rastreo, que se comparte — no compilaría. Consecuencia
///   deliberada: una reanudación no hereda la credencial del fichero; quien reanuda la vuelve a
///   dar en su sesión, igual que pasa con `ignore_robots`.
/// - **Su `Debug` no enseña la contraseña.** `CrawlJob` deriva `Debug` y cualquier traza o
///   mensaje de error podría volcarlo; el secreto no puede depender de que nadie logee nunca.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpBasicAuth {
    pub username: String,
    pub password: String,
}

impl HttpBasicAuth {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self { username: username.into(), password: password.into() }
    }
}

impl std::fmt::Debug for HttpBasicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // El usuario sí se enseña: identifica la credencial sin comprometerla.
        f.debug_struct("HttpBasicAuth")
            .field("username", &self.username)
            .field("password", &"***")
            .finish()
    }
}

/// Presupuesto de rastreo. Ver `03-MOTOR-CRAWL.md §9`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrawlLimits {
    /// Tope de URLs. En nivel Free lo impone `EntitlementSource`, **no la UI**.
    pub max_urls: Option<u64>,
    pub max_depth: Option<u32>,
    #[serde(with = "duration_opt_secs")]
    pub max_duration: Option<Duration>,
    pub max_size_per_url: u64,
    /// Regex sin anclar sobre la URL completa: si no está vacío, solo se rastrea lo que case
    /// con alguno. La semilla de un rastreo HTTP se rastrea siempre. Semántica: `pattern.rs`.
    pub include_patterns: Vec<String>,
    /// Regex sin anclar sobre la URL completa: lo que case no se rastrea, pero queda
    /// registrado con `exclusion_reason='pattern'`. Gana sobre `include_patterns`.
    pub exclude_patterns: Vec<String>,
    /// Por defecto las externas solo se comprueban de estado, no se rastrean.
    pub follow_external: bool,
    pub respect_nofollow: bool,
    /// 1..=20. Por defecto 5.
    pub concurrency_per_host: u8,
    pub user_agent: String,
    /// Ignorar `robots.txt`. Solo tras confirmación explícita y en sitios propios.
    pub ignore_robots: bool,
    /// Autenticación básica HTTP, acotada al host de la semilla. Ver [`HttpBasicAuth`].
    ///
    /// El `skip` es la línea de defensa contra la fuga de la revisión 2026-08-01 §1.6:
    /// `CrawlJob` entero se guarda como `config_json` dentro del fichero de rastreo, y ese
    /// fichero se comparte. La credencial vive solo en la memoria del proceso que rastrea.
    #[serde(skip)]
    pub http_basic_auth: Option<HttpBasicAuth>,
}

impl Default for CrawlLimits {
    fn default() -> Self {
        Self {
            max_urls: None,
            max_depth: None,
            max_duration: None,
            max_size_per_url: crate::fetch::MAX_BODY_BYTES,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            follow_external: false,
            respect_nofollow: true,
            concurrency_per_host: 5,
            user_agent: crate::fetch::DEFAULT_USER_AGENT.to_string(),
            ignore_robots: false,
            http_basic_auth: None,
        }
    }
}

impl CrawlLimits {
    /// Concurrencia efectiva, acotada al rango admitido.
    ///
    /// El techo de 20 no es negociable por configuración: por encima se castiga al servidor
    /// del cliente sin ganar throughput real.
    pub fn effective_concurrency(&self) -> u8 {
        self.concurrency_per_host.clamp(1, 20)
    }
}

/// Un rastreo completo, tal como se le pide al motor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrawlJob {
    pub project_id: String,
    pub project_name: String,
    pub mode: CrawlMode,
    pub limits: CrawlLimits,
    /// Acumular el texto de cuerpo para FTS5. Solo nivel Pro: multiplica el tamaño del fichero.
    pub collect_body_text: bool,
    /// Descubrir y leer sitemaps. El cruce sitemap ↔ enlaces produce las huérfanas.
    pub discover_sitemaps: bool,
    /// Nivel con el que se ejecuta el rastreo.
    ///
    /// Lo pone quien construye el trabajo, a partir de su `EntitlementSource`, y **el motor lo
    /// hace cumplir**: recorta `limits.max_urls` al tope del nivel y no evalúa reglas por encima
    /// de él. Por defecto `Agency`, que es el nivel de la CLI y del uso interno.
    #[serde(default = "tier_agency")]
    pub tier: crate::entitlement::Tier,
}

fn tier_agency() -> crate::entitlement::Tier {
    crate::entitlement::Tier::Agency
}

impl CrawlJob {
    /// Rastreo HTTP con todo por defecto.
    pub fn http(seed: impl Into<String>) -> Self {
        let seed = seed.into();
        Self {
            project_id: "default".to_string(),
            project_name: seed.clone(),
            mode: CrawlMode::Http { seed },
            limits: CrawlLimits::default(),
            collect_body_text: false,
            discover_sitemaps: true,
            tier: tier_agency(),
        }
    }

    /// Auditoría de un directorio construido.
    pub fn filesystem(root: impl Into<PathBuf>, base: impl Into<String>) -> Self {
        let root = root.into();
        Self {
            project_id: "default".to_string(),
            project_name: root.display().to_string(),
            mode: CrawlMode::Filesystem { root, base: base.into() },
            limits: CrawlLimits::default(),
            collect_body_text: false,
            // Sí, también en `filesystem`. Un `dist/` de Astro trae su `sitemap-index.xml`
            // generado, y sin leerlo `urls.in_sitemap` vale 0 en todas las filas: la vista
            // `v_orphans` no puede devolver nada y la detección de páginas huérfanas —uno de los
            // motivos por los que existe este modo— queda muerta justo donde más valor tiene.
            // Si no hay sitemap, son dos peticiones al árbol de ficheros que devuelven 404.
            discover_sitemaps: true,
            tier: tier_agency(),
        }
    }

    pub fn normalize_policy(&self) -> NormalizePolicy {
        NormalizePolicy::default()
    }
}

/// Configuración parcial de un rastreo, cargada desde un fichero.
///
/// Es lo que hay detrás de `--config f.yaml`: el fichero describe el
/// sitio, la línea de comandos describe la ejecución, y **los flags ganan sobre el fichero**.
/// El tipo vive en el core, y no en la CLI, porque la configuración es «reutilizable entre CLI
/// y app»; el *formato* (YAML) sí es cosa de cada frontal, así que aquí solo hay `serde`.
///
/// Todos los campos son opcionales: un campo ausente no toca el valor que ya tenga el trabajo.
/// `deny_unknown_fields` convierte una errata (`max_url:`) en un error con el nombre del campo,
/// en vez de una opción ignorada en silencio.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JobConfig {
    /// Nombre del proyecto, para el fichero de rastreo y el hub.
    pub project_name: Option<String>,
    pub max_urls: Option<u64>,
    pub max_depth: Option<u32>,
    /// Duración máxima del rastreo, en segundos.
    pub max_duration_secs: Option<u64>,
    /// Peticiones simultáneas por host (1..=20).
    pub concurrency: Option<u8>,
    pub user_agent: Option<String>,
    pub ignore_robots: Option<bool>,
    pub follow_external: Option<bool>,
    pub respect_nofollow: Option<bool>,
    /// Solo se rastrean las URLs que casen con alguno de estos patrones (regex, sin anclar).
    /// La semilla de un rastreo HTTP se rastrea siempre. Semántica completa: `pattern.rs`.
    pub include_patterns: Option<Vec<String>>,
    /// Las URLs que casen con alguno de estos patrones no se rastrean; quedan registradas
    /// como excluidas. Gana sobre `include_patterns`.
    pub exclude_patterns: Option<Vec<String>>,
    pub discover_sitemaps: Option<bool>,
    /// Acumular el texto de cuerpo para FTS5 (nivel Pro).
    pub collect_body_text: Option<bool>,
}

/// Un valor de [`JobConfig`] fuera de contrato.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum JobConfigError {
    /// El mismo contrato que valida la CLI en `--concurrency`: quien escribe 50 en el YAML debe
    /// oír «no» ahora, no creer que rastreó con 50 mientras el motor recortaba a 20 en silencio.
    #[error("concurrency must be between 1 and 20, got {0}")]
    ConcurrencyOutOfRange(u8),
}

impl JobConfig {
    /// Vuelca sobre `job` los campos presentes. Los ausentes no tocan nada, así que el orden
    /// «primero el fichero, después los flags» produce la precedencia prometida.
    pub fn apply_to(&self, job: &mut CrawlJob) -> Result<(), JobConfigError> {
        if let Some(c) = self.concurrency {
            if !(1..=20).contains(&c) {
                return Err(JobConfigError::ConcurrencyOutOfRange(c));
            }
            job.limits.concurrency_per_host = c;
        }
        if let Some(name) = &self.project_name {
            job.project_name = name.clone();
        }
        if let Some(n) = self.max_urls {
            job.limits.max_urls = Some(n);
        }
        if let Some(d) = self.max_depth {
            job.limits.max_depth = Some(d);
        }
        if let Some(secs) = self.max_duration_secs {
            job.limits.max_duration = Some(Duration::from_secs(secs));
        }
        if let Some(ua) = &self.user_agent {
            job.limits.user_agent = ua.clone();
        }
        if let Some(v) = self.ignore_robots {
            job.limits.ignore_robots = v;
        }
        if let Some(v) = self.follow_external {
            job.limits.follow_external = v;
        }
        if let Some(v) = self.respect_nofollow {
            job.limits.respect_nofollow = v;
        }
        if let Some(p) = &self.include_patterns {
            job.limits.include_patterns = p.clone();
        }
        if let Some(p) = &self.exclude_patterns {
            job.limits.exclude_patterns = p.clone();
        }
        if let Some(v) = self.discover_sitemaps {
            job.discover_sitemaps = v;
        }
        if let Some(v) = self.collect_body_text {
            job.collect_body_text = v;
        }
        Ok(())
    }
}

/// Por qué una página no es indexable. Se corresponde con `pages.indexability_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexabilityReason {
    Noindex,
    Canonicalised,
    Robots,
    Redirect,
    ClientError,
    ServerError,
    NotHtml,
}

impl IndexabilityReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noindex => "noindex",
            Self::Canonicalised => "canonicalised",
            Self::Robots => "robots",
            Self::Redirect => "redirect",
            Self::ClientError => "4xx",
            Self::ServerError => "5xx",
            Self::NotHtml => "not_html",
        }
    }
}

/// Datos de los que depende la indexabilidad de una página.
pub struct IndexabilityInput<'a> {
    pub status: u16,
    pub is_html: bool,
    pub meta_robots: Option<&'a str>,
    pub x_robots_tag: Option<&'a str>,
    pub blocked_by_robots: bool,
    /// Canonical ya normalizado y resuelto, si lo hay.
    pub canonical: Option<&'a str>,
    /// URL normalizada de la propia página, para compararla con el canonical.
    pub self_url: &'a str,
}

/// Decide si una página es indexable y, si no lo es, por qué.
///
/// **Es la regla central de todo el producto.** «¿Por qué esta página no está en Google?» se
/// responde con el motivo que devuelve esta función, y es la consulta más frecuente que hace
/// un SEO. Ver `docs/03-MOTOR-CRAWL.md §6`.
///
/// El orden de comprobación importa: se devuelve la causa *raíz*. Una página con 404 y además
/// `noindex` no es interesante por el `noindex`.
pub fn evaluate_indexability(input: &IndexabilityInput<'_>) -> (bool, Option<IndexabilityReason>) {
    if input.blocked_by_robots {
        return (false, Some(IndexabilityReason::Robots));
    }
    if (500..600).contains(&input.status) {
        return (false, Some(IndexabilityReason::ServerError));
    }
    if (400..500).contains(&input.status) {
        return (false, Some(IndexabilityReason::ClientError));
    }
    if (300..400).contains(&input.status) {
        return (false, Some(IndexabilityReason::Redirect));
    }
    if input.status != 200 {
        return (false, Some(IndexabilityReason::ClientError));
    }
    if !input.is_html {
        return (false, Some(IndexabilityReason::NotHtml));
    }
    if has_noindex(input.meta_robots) || has_noindex(input.x_robots_tag) {
        return (false, Some(IndexabilityReason::Noindex));
    }
    // Un canonical ausente no descalifica; uno que apunta a otra URL, sí.
    if let Some(canonical) = input.canonical {
        if !canonical.is_empty() && canonical != input.self_url {
            return (false, Some(IndexabilityReason::Canonicalised));
        }
    }
    (true, None)
}

/// ¿Lleva esta directiva un `noindex`?
///
/// El valor es una lista separada por comas y puede llevar prefijo de bot
/// (`googlebot: noindex`). Buscar la subcadena a secas daría un falso positivo con
/// `index` dentro de otra palabra.
fn has_noindex(directive: Option<&str>) -> bool {
    directive.is_some_and(|d| {
        d.to_ascii_lowercase()
            .split(',')
            .map(|token| token.trim().rsplit(':').next().unwrap_or("").trim())
            .any(|token| token == "noindex" || token == "none")
    })
}

mod duration_opt_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(v: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(d) => s.serialize_some(&d.as_secs()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        Ok(Option::<u64>::deserialize(d)?.map(Duration::from_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(status: u16, is_html: bool) -> IndexabilityInput<'a> {
        IndexabilityInput {
            status,
            is_html,
            meta_robots: None,
            x_robots_tag: None,
            blocked_by_robots: false,
            canonical: None,
            self_url: "https://ejemplo.es/a",
        }
    }

    #[test]
    fn una_pagina_200_html_y_limpia_es_indexable() {
        let (ok, reason) = evaluate_indexability(&input(200, true));
        assert!(ok);
        assert!(reason.is_none());
    }

    #[test]
    fn un_canonical_a_si_misma_no_descalifica() {
        let mut i = input(200, true);
        i.canonical = Some("https://ejemplo.es/a");
        assert!(evaluate_indexability(&i).0);
    }

    #[test]
    fn un_canonical_a_otra_url_la_descalifica() {
        let mut i = input(200, true);
        i.canonical = Some("https://ejemplo.es/otra");
        assert_eq!(
            evaluate_indexability(&i),
            (false, Some(IndexabilityReason::Canonicalised))
        );
    }

    #[test]
    fn detecta_noindex_en_meta_robots() {
        let mut i = input(200, true);
        i.meta_robots = Some("noindex, follow");
        assert_eq!(evaluate_indexability(&i), (false, Some(IndexabilityReason::Noindex)));
    }

    #[test]
    fn detecta_noindex_en_la_cabecera_x_robots_tag() {
        let mut i = input(200, true);
        i.x_robots_tag = Some("noindex");
        assert_eq!(evaluate_indexability(&i), (false, Some(IndexabilityReason::Noindex)));
    }

    #[test]
    fn detecta_noindex_con_prefijo_de_bot() {
        let mut i = input(200, true);
        i.x_robots_tag = Some("googlebot: noindex");
        assert_eq!(evaluate_indexability(&i), (false, Some(IndexabilityReason::Noindex)));
    }

    #[test]
    fn none_equivale_a_noindex() {
        let mut i = input(200, true);
        i.meta_robots = Some("none");
        assert_eq!(evaluate_indexability(&i), (false, Some(IndexabilityReason::Noindex)));
    }

    #[test]
    fn index_follow_no_se_confunde_con_noindex() {
        // Buscar la subcadena "noindex" a secas fallaría aquí.
        let mut i = input(200, true);
        i.meta_robots = Some("index, follow");
        assert!(evaluate_indexability(&i).0);
    }

    #[test]
    fn max_image_preview_no_se_confunde_con_noindex() {
        let mut i = input(200, true);
        i.meta_robots = Some("max-image-preview:large, max-snippet:-1");
        assert!(evaluate_indexability(&i).0);
    }

    #[test]
    fn los_codigos_de_error_dan_su_propio_motivo() {
        assert_eq!(
            evaluate_indexability(&input(404, true)),
            (false, Some(IndexabilityReason::ClientError))
        );
        assert_eq!(
            evaluate_indexability(&input(503, true)),
            (false, Some(IndexabilityReason::ServerError))
        );
        assert_eq!(
            evaluate_indexability(&input(301, true)),
            (false, Some(IndexabilityReason::Redirect))
        );
    }

    #[test]
    fn lo_que_no_es_html_no_es_indexable_como_pagina() {
        assert_eq!(
            evaluate_indexability(&input(200, false)),
            (false, Some(IndexabilityReason::NotHtml))
        );
    }

    #[test]
    fn robots_txt_gana_a_cualquier_otro_motivo() {
        // Si está bloqueada, Google ni siquiera ve el resto: esa es la causa raíz.
        let mut i = input(200, true);
        i.blocked_by_robots = true;
        i.meta_robots = Some("noindex");
        assert_eq!(evaluate_indexability(&i), (false, Some(IndexabilityReason::Robots)));
    }

    #[test]
    fn un_404_con_noindex_se_reporta_como_404() {
        let mut i = input(404, true);
        i.meta_robots = Some("noindex");
        assert_eq!(evaluate_indexability(&i), (false, Some(IndexabilityReason::ClientError)));
    }

    #[test]
    fn la_concurrencia_queda_acotada_al_rango_admitido() {
        let l = |n| CrawlLimits { concurrency_per_host: n, ..Default::default() };
        assert_eq!(l(0).effective_concurrency(), 1, "nunca cero");
        assert_eq!(l(5).effective_concurrency(), 5);
        assert_eq!(l(200).effective_concurrency(), 20, "techo de 20");
    }

    #[test]
    fn los_valores_por_defecto_son_los_del_documento() {
        let l = CrawlLimits::default();
        assert_eq!(l.concurrency_per_host, 5);
        assert!(l.respect_nofollow);
        assert!(!l.follow_external, "por defecto las externas solo se comprueban");
        assert!(!l.ignore_robots);
        assert_eq!(l.max_size_per_url, 10 * 1024 * 1024);
    }

    #[test]
    fn el_job_serializa_y_vuelve_igual() {
        // `config_json` se guarda íntegro en `crawl_meta` y un diff lo compara: si no
        // sobrevive el viaje, los diffs mienten.
        let job = CrawlJob::http("https://ejemplo.es");
        let json = serde_json::to_string(&job).expect("serializar");
        let back: CrawlJob = serde_json::from_str(&json).expect("deserializar");
        assert_eq!(back.mode, job.mode);
        assert_eq!(back.limits.concurrency_per_host, job.limits.concurrency_per_host);
    }

    #[test]
    fn la_credencial_no_se_serializa_en_config_json() {
        // El JSON de un `CrawlJob` es lo que se guarda como `crawl_meta.config_json` dentro
        // del fichero de rastreo, que se comparte con el cliente. La revisión 2026-08-01 §1.6
        // empezó exactamente por una contraseña ahí: este test es el que impide que la fuga
        // vuelva por esta puerta.
        let mut job = CrawlJob::http("https://pre.cliente.es/");
        job.limits.http_basic_auth = Some(HttpBasicAuth::new("staging", "S3creta"));

        let json = serde_json::to_string(&job).expect("serializar");
        assert!(!json.contains("S3creta"), "la contraseña no puede viajar en el JSON: {json}");
        assert!(!json.contains("staging"), "ni siquiera el usuario: {json}");
        assert!(!json.contains("http_basic_auth"), "el campo entero se omite: {json}");

        // Y al volver del JSON —una reanudación— la credencial no está: quien reanuda la
        // vuelve a dar en su sesión, como pasa con `ignore_robots`.
        let back: CrawlJob = serde_json::from_str(&json).expect("deserializar");
        assert!(back.limits.http_basic_auth.is_none());
    }

    #[test]
    fn el_debug_de_la_credencial_no_ensena_la_contrasena() {
        // `CrawlJob` deriva `Debug`: cualquier traza podría volcarlo entero.
        let auth = HttpBasicAuth::new("staging", "S3creta");
        let texto = format!("{auth:?}");
        assert!(!texto.contains("S3creta"), "{texto}");
        assert!(texto.contains("staging"), "el usuario sí identifica la credencial: {texto}");
    }

    #[test]
    fn una_duracion_maxima_sobrevive_al_viaje_por_json() {
        let mut job = CrawlJob::http("https://ejemplo.es");
        job.limits.max_duration = Some(Duration::from_secs(90));
        let json = serde_json::to_string(&job).expect("serializar");
        let back: CrawlJob = serde_json::from_str(&json).expect("deserializar");
        assert_eq!(back.limits.max_duration, Some(Duration::from_secs(90)));
    }

    // ── JobConfig: la mitad de `--config` que vive en el core ────────────────

    #[test]
    fn un_config_vacio_no_toca_el_trabajo() {
        let mut job = CrawlJob::http("https://ejemplo.es");
        let original = job.clone();
        JobConfig::default().apply_to(&mut job).expect("vacío siempre es válido");
        assert_eq!(job.limits.concurrency_per_host, original.limits.concurrency_per_host);
        assert_eq!(job.limits.max_urls, original.limits.max_urls);
        assert_eq!(job.discover_sitemaps, original.discover_sitemaps);
    }

    #[test]
    fn los_campos_presentes_se_aplican_y_los_ausentes_no() {
        let mut job = CrawlJob::http("https://ejemplo.es");
        let config = JobConfig {
            max_urls: Some(5000),
            concurrency: Some(8),
            max_duration_secs: Some(90),
            discover_sitemaps: Some(false),
            ..JobConfig::default()
        };
        config.apply_to(&mut job).expect("configuración válida");
        assert_eq!(job.limits.max_urls, Some(5000));
        assert_eq!(job.limits.concurrency_per_host, 8);
        assert_eq!(job.limits.max_duration, Some(Duration::from_secs(90)));
        assert!(!job.discover_sitemaps);
        // Un campo ausente conserva el valor previo del trabajo.
        assert!(!job.limits.ignore_robots);
        assert!(job.limits.respect_nofollow);
    }

    #[test]
    fn la_concurrencia_del_config_respeta_el_mismo_contrato_que_la_cli() {
        for fuera in [0u8, 21, 50] {
            let mut job = CrawlJob::http("https://ejemplo.es");
            let config = JobConfig { concurrency: Some(fuera), ..JobConfig::default() };
            assert_eq!(
                config.apply_to(&mut job),
                Err(JobConfigError::ConcurrencyOutOfRange(fuera)),
                "{fuera} está fuera de 1..=20"
            );
        }
        let mut job = CrawlJob::http("https://ejemplo.es");
        let config = JobConfig { concurrency: Some(20), ..JobConfig::default() };
        config.apply_to(&mut job).expect("20 es el techo y vale");
        assert_eq!(job.limits.concurrency_per_host, 20);
    }

    #[test]
    fn aplicar_el_fichero_y_despues_los_flags_da_la_precedencia_prometida() {
        // El patrón exacto de la CLI: primero el fichero, después los flags explícitos.
        let mut job = CrawlJob::http("https://ejemplo.es");
        let config = JobConfig {
            max_urls: Some(5000),
            concurrency: Some(8),
            ..JobConfig::default()
        };
        config.apply_to(&mut job).expect("configuración válida");
        job.limits.max_urls = Some(100); // el flag `--max-urls 100` gana
        assert_eq!(job.limits.max_urls, Some(100));
        assert_eq!(job.limits.concurrency_per_host, 8, "lo no pisado por flags se conserva");
    }

    #[test]
    fn el_job_config_viaja_por_serde_con_los_nombres_del_documento() {
        // La CLI lo deserializa desde YAML; aquí se comprueba con JSON, que comparte serde,
        // para no acoplar el core a un formato.
        let config: JobConfig = serde_json::from_str(
            "{\"max_urls\": 100, \"concurrency\": 3, \"ignore_robots\": true}",
        )
        .expect("deserializar");
        assert_eq!(config.max_urls, Some(100));
        assert_eq!(config.concurrency, Some(3));
        assert_eq!(config.ignore_robots, Some(true));
    }

    #[test]
    fn los_patrones_del_config_se_aplican_y_su_ausencia_no_borra_nada() {
        let mut job = CrawlJob::http("https://ejemplo.es");
        job.limits.exclude_patterns = vec!["/previo/".to_string()];

        // Sin patrones en el config, los del trabajo se conservan.
        JobConfig::default().apply_to(&mut job).expect("vacío siempre es válido");
        assert_eq!(job.limits.exclude_patterns, vec!["/previo/".to_string()]);

        // Con patrones, se sustituyen: el fichero describe el sitio completo.
        let config = JobConfig {
            include_patterns: Some(vec!["/blog/".to_string()]),
            exclude_patterns: Some(vec!["/wp-admin/".to_string(), r"\?s=".to_string()]),
            ..JobConfig::default()
        };
        config.apply_to(&mut job).expect("configuración válida");
        assert_eq!(job.limits.include_patterns, vec!["/blog/".to_string()]);
        assert_eq!(
            job.limits.exclude_patterns,
            vec!["/wp-admin/".to_string(), r"\?s=".to_string()]
        );
    }

    #[test]
    fn un_campo_desconocido_en_el_config_es_un_error_y_no_un_silencio() {
        let err = serde_json::from_str::<JobConfig>("{\"max_url\": 100}")
            .expect_err("max_url es una errata de max_urls");
        assert!(err.to_string().contains("max_url"), "{err}");
    }

    #[test]
    fn los_modos_se_nombran_como_en_el_esquema() {
        assert_eq!(CrawlJob::http("https://ejemplo.es").mode.as_str(), "http");
        assert_eq!(CrawlJob::filesystem("/tmp/dist", "https://ejemplo.es/").mode.as_str(),
                   "filesystem");
    }
}
