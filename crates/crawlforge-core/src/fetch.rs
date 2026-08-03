//! Obtención de bytes. Ver `docs/03-MOTOR-CRAWL.md §1 y §7`.
//!
//! Los tres modos de rastreo (`http`, `filesystem`, `list`) desembocan en el mismo pipeline de
//! parseo, reglas y almacén. Lo único que cambia es de dónde salen los bytes, y eso es lo que
//! abstrae [`Fetcher`].

use crate::error::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use url::Url;

/// Timeout de establecimiento de conexión.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Timeout total de una petición.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout de una comprobación de estado de URL externa. Más corto que [`REQUEST_TIMEOUT`]
/// a propósito: un host de terceros muerto no puede colgar la auditoría del usuario.
pub const EXTERNAL_CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// Reintentos antes de dar una URL por errónea.
pub const MAX_RETRIES: u32 = 3;
/// Límite de tamaño de una página. Superarlo marca `error_kind='toolarge'`.
///
/// Diez megabytes de HTML es una barbaridad —la regla `HTTP-LARGE-PAGE` avisa a partir de 500 KB—
/// y el tope existe para que un servidor hostil no pueda llenar la memoria: el cuerpo se acumula
/// en RAM mientras se lee.
pub const MAX_BODY_BYTES: u64 = 10 * 1024 * 1024;

/// Límite de tamaño de un documento XML: sitemaps.
///
/// El protocolo de sitemaps permite **50 MB sin comprimir**, así que aplicarles el tope de las
/// páginas dejaba sin leer sitemaps perfectamente legales —del orden de 200.000 URLs— y el sitio
/// se auditaba a ciegas. El tope se elige por el tipo de contenido de la respuesta y no por la
/// URL: un sitemap puede servirse desde cualquier ruta, y fiarse del nombre del fichero es el
/// error que ya se documenta en `sitemap.rs` al distinguir índice de lista.
///
/// El riesgo de memoria sigue acotado: al descomprimir hay otro tope aparte
/// (`sitemap::MAX_DECOMPRESSED_BYTES`), que es donde estaba la bomba de verdad.
pub const MAX_XML_BYTES: u64 = 50 * 1024 * 1024;

/// El tope que aplica a una respuesta, según lo que el servidor dice que es.
fn body_limit_for(content_type: Option<&str>, page_limit: u64) -> u64 {
    let es_xml = content_type.is_some_and(|c| {
        let c = c.to_ascii_lowercase();
        c.contains("xml") || c.contains("gzip") || c.contains("octet-stream")
    });
    if es_xml {
        page_limit.max(MAX_XML_BYTES)
    } else {
        page_limit
    }
}
/// User-Agent por defecto. Identificarse honestamente y de forma verificable (§10).
pub const DEFAULT_USER_AGENT: &str = "CrawlForge/1.0 (+https://crawlforge.app/bot)";

/// Por qué falló la obtención de una URL. Se corresponde con `urls.error_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Dns,
    Tls,
    Timeout,
    Connection,
    TooLarge,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::Tls => "tls",
            Self::Timeout => "timeout",
            Self::Connection => "connection",
            Self::TooLarge => "toolarge",
        }
    }

    /// ¿Merece la pena reintentar este fallo?
    ///
    /// DNS y TLS no: si el nombre no resuelve o el certificado no valida, volver a intentarlo
    /// tres veces solo alarga el rastreo. Timeout y conexión sí: suelen ser transitorios.
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Timeout | Self::Connection)
    }
}

/// Un documento obtenido, con lo que el pipeline necesita para seguir.
#[derive(Debug)]
pub struct FetchedDoc {
    /// URL efectivamente pedida.
    pub url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    /// Cabecera `X-Robots-Tag`, que puede llevar `noindex` igual que la meta.
    pub x_robots_tag: Option<String>,
    /// Destino de la redirección, si el estado es 3xx. **No se sigue automáticamente**: cada
    /// salto es una fila del informe.
    pub location: Option<String>,
    pub body: Vec<u8>,
    pub response_time_ms: u32,
}

impl FetchedDoc {
    /// ¿Es HTML? Solo lo que es HTML se parsea.
    pub fn is_html(&self) -> bool {
        self.content_type
            .as_deref()
            .is_some_and(|c| c.to_ascii_lowercase().contains("text/html")
                || c.to_ascii_lowercase().contains("application/xhtml"))
    }

    pub fn is_redirect(&self) -> bool {
        (300..400).contains(&self.status)
    }

    pub fn content_length(&self) -> u64 {
        self.body.len() as u64
    }
}

/// Fallo al obtener una URL. No aborta el rastreo: se guarda en la fila de la URL.
#[derive(Debug, Clone)]
pub struct FetchFailure {
    pub kind: ErrorKind,
    pub message: String,
}

/// Lo que devuelve una comprobación de estado de una URL externa: **solo cabeceras**.
///
/// No hay cuerpo a propósito: comprobar que un enlace resuelve es lo que hace el navegador
/// cuando el visitante lo pulsa; no se indexa, no se almacena y no se sigue nada del sitio
/// ajeno. `content_length` sale de la cabecera `Content-Length`, no de una descarga.
#[derive(Debug, Clone)]
pub struct StatusProbe {
    pub status: u16,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub response_time_ms: u32,
}

/// Fuente de bytes. `HttpFetcher`, `FilesystemFetcher` y, en el futuro, un fetcher que renderice JavaScript.
pub trait Fetcher: Send + Sync {
    fn fetch(
        &self,
        url: &Url,
    ) -> impl std::future::Future<Output = Result<std::result::Result<FetchedDoc, FetchFailure>>> + Send;

    /// Forma publicada de una URL, si la fuente puede saberla sin pedirla.
    ///
    /// Solo el modo `filesystem` puede: tiene el árbol de ficheros delante y sabe que
    /// `/about` y `/about/` son la misma página. Por HTTP hay que preguntarle al servidor,
    /// que es lo que dice la regla 8 de la normalización, así que devuelve `None`.
    fn canonicalize(&self, _url: &Url) -> Option<Url> {
        None
    }

    /// ¿La fuente pasa por la red?
    ///
    /// Lo necesitan las reglas que miden latencia: leer un fichero del disco tarda algo, pero no
    /// es un TTFB, y avisar de que un `dist/` local «responde lento» sería un hallazgo inventado.
    fn is_network(&self) -> bool {
        true
    }

    /// ¿Puede esta fuente comprobar el estado de una URL externa?
    ///
    /// Solo el fetcher HTTP puede: el de sistema de ficheros no tiene cliente de red, así que
    /// en modo `filesystem` las externas quedan registradas sin estado, como siempre.
    fn can_check_status(&self) -> bool {
        false
    }

    /// Comprueba el estado de una URL sin descargarla ni parsearla.
    ///
    /// Es la mitad de red de `CrawlLimits::check_external`: `HEAD` con timeout corto
    /// ([`EXTERNAL_CHECK_TIMEOUT`]) y, si el servidor rechaza `HEAD` (405/501), un único
    /// reintento con `GET` del que solo se leen las cabeceras. Sin reintentos de red: un host
    /// ajeno que no responde ya es la respuesta, y el backoff de los reintentos multiplicaría
    /// la espera por cada host muerto que el sitio enlace.
    fn check_status(
        &self,
        _url: &Url,
    ) -> impl std::future::Future<Output = std::result::Result<StatusProbe, FetchFailure>> + Send
    {
        std::future::ready(Err(FetchFailure {
            kind: ErrorKind::Connection,
            message: "status checks are not supported by this fetcher".to_string(),
        }))
    }
}

// ---------------------------------------------------------------- HTTP

/// La credencial ya preparada como cabecera, atada al único host que puede recibirla.
///
/// Se precomputa una vez —el base64 no cambia entre peticiones— y se guarda junto al host
/// **de la semilla**: el fetcher es uno solo para todo el rastreo, incluidas las URLs externas
/// cuando `follow_external` está activo, y mandar un `Authorization` a un host ajeno sería
/// regalar la credencial del staging a cualquier dominio que el sitio enlace.
struct BasicAuthScope {
    /// Host al que —y solo al que— se manda la cabecera. Comparado sin distinguir caja.
    host: String,
    /// `Basic <base64>`, listo para la cabecera. El alfabeto de base64 solo produce
    /// caracteres válidos en un valor de cabecera, así que esto nunca necesita validación.
    header_value: String,
}

/// Obtención por HTTP con reintentos y backoff exponencial con jitter.
pub struct HttpFetcher {
    client: reqwest::Client,
    max_body_bytes: u64,
    max_retries: u32,
    basic_auth: Option<BasicAuthScope>,
}

impl HttpFetcher {
    pub fn new(user_agent: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            // Las redirecciones se siguen a mano: para un auditor SEO cada salto de una
            // cadena es una fila del informe, no un detalle de transporte.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| crate::CoreError::Http(e.to_string()))?;

        Ok(Self {
            client,
            max_body_bytes: MAX_BODY_BYTES,
            max_retries: MAX_RETRIES,
            basic_auth: None,
        })
    }

    pub fn with_max_body_bytes(mut self, bytes: u64) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Autenticación básica para `host` — y para ningún otro.
    ///
    /// Vale para todo lo que este fetcher pida de ese host: páginas, `robots.txt` y sitemaps,
    /// que es justo lo que hace usable un staging protegido — sin autenticar el `robots.txt`,
    /// un 401 ahí haría que el rastreo se comportara de forma rara antes de empezar. Las
    /// redirecciones no pueden sacar la cabecera de aquí: el cliente no las sigue
    /// (`Policy::none`), cada salto vuelve por este mismo método y se reevalúa su host.
    pub fn with_basic_auth(mut self, host: &str, auth: &crate::job::HttpBasicAuth) -> Self {
        let payload = format!("{}:{}", auth.username, auth.password);
        self.basic_auth = Some(BasicAuthScope {
            host: host.to_ascii_lowercase(),
            header_value: format!("Basic {}", base64_standard(payload.as_bytes())),
        });
        self
    }

    /// La cabecera `Authorization` que corresponde a esta URL, si le corresponde alguna.
    ///
    /// Separado de `fetch_once` para poder afirmar con un test unitario que el ámbito es el
    /// nombre de host exacto: un subdominio o cualquier otro dominio no reciben nada. El
    /// puerto no cuenta: el criterio es el mismo con el que `normalize::is_internal` decide
    /// qué es interno, y un staging que sirve en `:443` y `:8443` es el mismo sitio.
    fn auth_header_for(&self, url: &Url) -> Option<&str> {
        let scope = self.basic_auth.as_ref()?;
        let host = url.host_str()?;
        host.eq_ignore_ascii_case(&scope.host).then_some(scope.header_value.as_str())
    }

    /// Una petición de solo cabeceras, con el timeout corto de las comprobaciones externas.
    ///
    /// El cuerpo no se lee nunca, ni siquiera en el `GET` de respaldo: la respuesta se suelta
    /// con las cabeceras ya en mano, y `content_length` sale de la cabecera. Descargar el
    /// cuerpo de un host ajeno sería exactamente lo que esta función promete no hacer.
    async fn probe_once(
        &self,
        url: &Url,
        method: reqwest::Method,
    ) -> std::result::Result<StatusProbe, FetchFailure> {
        let started = Instant::now();
        let mut request = self
            .client
            .request(method, url.clone())
            .timeout(EXTERNAL_CHECK_TIMEOUT);
        // La credencial sigue acotada a su host; para una URL externa esto devuelve `None`.
        if let Some(header) = self.auth_header_for(url) {
            request = request.header(reqwest::header::AUTHORIZATION, header);
        }
        let response = request.send().await.map_err(|e| classify_reqwest_error(&e))?;
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        Ok(StatusProbe {
            status: response.status().as_u16(),
            content_type,
            content_length: response.content_length(),
            response_time_ms: started.elapsed().as_millis() as u32,
        })
    }

    /// Una sola petición, sin reintentos.
    async fn fetch_once(
        &self,
        url: &Url,
    ) -> std::result::Result<FetchedDoc, FetchFailure> {
        let started = Instant::now();

        let mut request = self.client.get(url.clone());
        // La credencial se decide **por URL y en cada petición**, no al construir el cliente:
        // con `follow_external` este mismo fetcher pide hosts ajenos, y un `default_header`
        // global les regalaría el `Authorization` del staging.
        if let Some(header) = self.auth_header_for(url) {
            request = request.header(reqwest::header::AUTHORIZATION, header);
        }
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => return Err(classify_reqwest_error(&e)),
        };

        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        };
        let content_type = header("content-type");
        let x_robots_tag = header("x-robots-tag");
        let location = header("location");

        // Un sitemap puede pesar legalmente cinco veces lo que se admite de una página.
        let limite = body_limit_for(content_type.as_deref(), self.max_body_bytes);

        // `Content-Length` permite rechazar un fichero enorme antes de descargarlo. No es
        // fiable (puede faltar o mentir), así que abajo se vuelve a comprobar el tamaño real.
        if let Some(len) = response.content_length() {
            if len > limite {
                return Err(FetchFailure {
                    kind: ErrorKind::TooLarge,
                    message: format!("{len} bytes declarados, máximo {limite}"),
                });
            }
        }

        // El cuerpo se lee **por trozos**, cortando en cuanto se pasa del tope.
        //
        // `response.bytes()` descarga entero y comprueba después, que es tanto como no tener
        // tope: con `Transfer-Encoding: chunked` no hay `Content-Length` que mirar antes, así que
        // un servidor hostil solo tiene que no declararlo. Medido: 40 páginas de 150 MB con el
        // tope por defecto de 10 MB dejaban **2.081 MB de RSS** —y `bytes_downloaded` marcaba 0,
        // así que ni rastro en las métricas—. Es la misma clase de fallo que la bomba de gzip de
        // los sitemaps, por el otro camino.
        let mut body: Vec<u8> = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = futures::StreamExt::next(&mut stream).await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => return Err(classify_reqwest_error(&e)),
            };
            if body.len() as u64 + chunk.len() as u64 > limite {
                return Err(FetchFailure {
                    kind: ErrorKind::TooLarge,
                    message: format!("más de {limite} bytes; la descarga se cortó"),
                });
            }
            body.extend_from_slice(&chunk);
        }

        Ok(FetchedDoc {
            url: url.clone(),
            status,
            content_type,
            x_robots_tag,
            location,
            body,
            response_time_ms: started.elapsed().as_millis() as u32,
        })
    }
}

impl Fetcher for HttpFetcher {
    fn can_check_status(&self) -> bool {
        true
    }

    /// `HEAD` y, si el servidor lo rechaza (405/501 — hay muchos que lo hacen), un único
    /// reintento con `GET` del que solo se leen las cabeceras.
    async fn check_status(
        &self,
        url: &Url,
    ) -> std::result::Result<StatusProbe, FetchFailure> {
        match self.probe_once(url, reqwest::Method::HEAD).await {
            Ok(probe) if matches!(probe.status, 405 | 501) => {
                self.probe_once(url, reqwest::Method::GET).await
            }
            other => other,
        }
    }

    async fn fetch(
        &self,
        url: &Url,
    ) -> Result<std::result::Result<FetchedDoc, FetchFailure>> {
        let mut attempt = 0;

        loop {
            match self.fetch_once(url).await {
                Ok(doc) if should_retry_status(doc.status) && attempt < self.max_retries => {
                    attempt += 1;
                    tokio::time::sleep(backoff_delay(attempt)).await;
                }
                Ok(doc) => return Ok(Ok(doc)),
                Err(failure) if failure.kind.is_retryable() && attempt < self.max_retries => {
                    attempt += 1;
                    tokio::time::sleep(backoff_delay(attempt)).await;
                }
                // Agotados los reintentos, la URL queda marcada como errónea y el rastreo
                // continúa. Un fallo de red nunca aborta un rastreo.
                Err(failure) => return Ok(Err(failure)),
            }
        }
    }
}

/// Base64 estándar con relleno (RFC 4648 §4): lo que exige `Authorization: Basic`.
///
/// Escrito aquí en vez de traer el crate `base64`: son quince líneas para un único consumidor,
/// y añadir una dependencia directa al core por esto no compra nada — el día que haga falta
/// base64 en más sitios, se trae el crate y esto se borra.
fn base64_standard(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = [
            ALPHABET[(n >> 18) as usize & 63],
            ALPHABET[(n >> 12) as usize & 63],
            ALPHABET[(n >> 6) as usize & 63],
            ALPHABET[n as usize & 63],
        ];
        let keep = chunk.len() + 1;
        for (i, c) in chars.iter().enumerate() {
            out.push(if i < keep { char::from(*c) } else { '=' });
        }
    }
    out
}

/// Códigos que merecen reintento: sobrecarga temporal del servidor, no error del cliente.
fn should_retry_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Backoff exponencial con jitter de ±50%: 1s, 2s, 4s.
///
/// El jitter evita que veinte workers que chocaron con el mismo 503 reintenten todos en el
/// mismo instante y vuelvan a tumbar el servidor.
fn backoff_delay(attempt: u32) -> Duration {
    use rand::Rng;
    let base = Duration::from_secs(1 << (attempt.saturating_sub(1)).min(6));
    let jitter = rand::thread_rng().gen_range(0.5..=1.5);
    base.mul_f64(jitter)
}

fn classify_reqwest_error(e: &reqwest::Error) -> FetchFailure {
    let message = e.to_string();
    let lower = message.to_ascii_lowercase();

    let kind = if e.is_timeout() {
        ErrorKind::Timeout
    } else if lower.contains("dns") || lower.contains("name or service") {
        ErrorKind::Dns
    } else if lower.contains("tls") || lower.contains("certificate") || lower.contains("ssl") {
        ErrorKind::Tls
    } else {
        ErrorKind::Connection
    };

    FetchFailure { kind, message }
}

// ---------------------------------------------------------------- Sistema de ficheros

/// Rastreo de un directorio ya construido (`dist/` de Astro, `public/`, `_site/`).
///
/// **Es lo que Screaming Frog no puede hacer.** Sin red, se auditan miles de páginas en
/// segundos, antes de desplegar.
pub struct FilesystemFetcher {
    root: PathBuf,
    /// La raíz ya resuelta a su forma real, para comparar contra ella sin volver a pedirla al
    /// sistema en cada fichero. Es invariante durante todo el rastreo.
    root_canonical: Option<PathBuf>,
    /// Base con la que se reescriben las rutas para que el resto del pipeline vea URLs.
    base: Url,
    /// Mismo tope que en HTTP: un fichero enorme dentro del `dist/` no debe acabar en memoria.
    max_body_bytes: u64,
}

impl FilesystemFetcher {
    pub fn new(root: impl Into<PathBuf>, base: Url) -> Self {
        let root: PathBuf = root.into();
        let root_canonical = root.canonicalize().ok();
        Self { root, root_canonical, base, max_body_bytes: MAX_BODY_BYTES }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    /// Traduce una URL a la ruta que serviría un servidor estático.
    ///
    /// Se prueban, en orden: la ruta literal, `ruta/index.html` y `ruta.html`. Es el
    /// comportamiento de Astro, Netlify, Vercel y `nginx` con `try_files`. Sin esto, un
    /// enlace a `/about` no encontraría `about/index.html` y daría un 404 falso.
    /// Como [`Self::resolve`], pero **sin** la comprobación de enlaces simbólicos.
    ///
    /// Se usa solo para saber cómo se publica una ruta —`/about` frente a `/about/`—, que no lee
    /// ni un byte del fichero. La distinción no es cosmética: `Fetcher::canonicalize` se llama
    /// cuatro veces por enlace, así que meter dos `canonicalize()` del sistema de ficheros ahí
    /// costó **10,7x de rendimiento** en el modo `filesystem` y dejó el motor por debajo de su
    /// propia puerta (226 páginas/s frente al suelo de 300). Devolver la forma publicada de una
    /// ruta que luego `fetch` rechazará no filtra ningún byte: la amenaza del symlink es leerlo.
    fn resolve_lexical(&self, url: &Url) -> Option<PathBuf> {
        self.resolve_inner(url, false)
    }

    /// Resuelve una URL a la ruta que se va a **leer**, con la comprobación de symlink incluida.
    pub fn resolve(&self, url: &Url) -> Option<PathBuf> {
        self.resolve_inner(url, true)
    }

    fn resolve_inner(&self, url: &Url, check_symlinks: bool) -> Option<PathBuf> {
        let path = url.path().trim_start_matches('/');
        let decoded = percent_decode(path);
        let candidate = self.root.join(&decoded);

        // Un candidato que se escapa de la raíz es un intento de path traversal: fuera.
        if !self.is_within_root(&candidate) {
            tracing::warn!(path = %decoded, "ruta fuera del directorio raíz; se descarta");
            return None;
        }

        if candidate.is_file() {
            return self.if_really_inside(candidate, check_symlinks);
        }
        let index = candidate.join("index.html");
        if index.is_file() {
            return self.if_really_inside(index, check_symlinks);
        }
        let with_ext = self.root.join(format!("{}.html", decoded.trim_end_matches('/')));
        if with_ext.is_file() && self.is_within_root(&with_ext) {
            return self.if_really_inside(with_ext, check_symlinks);
        }
        None
    }

    /// Segunda comprobación, ya con el fichero en el disco: que **de verdad** esté dentro.
    ///
    /// [`Self::is_within_root`] compara rutas como texto, y eso no ve los enlaces simbólicos: un
    /// `dist/assets -> /etc` pasa el filtro léxico y luego `read` sigue el enlace. Comprobado: un
    /// `dist/` con un symlink hacia fuera dejaba leer ficheros ajenos y su contenido acababa en el
    /// fichero de rastreo.
    ///
    /// No sustituye a la comprobación léxica, la completa: `canonicalize` falla en ficheros que no
    /// existen —que es el caso normal de un 404— así que la primera criba tiene que seguir siendo
    /// sin tocar el disco. Aquí ya sabemos que el fichero existe.
    ///
    /// Importa el doble en macOS: el usuario concede acceso a un directorio concreto con un
    /// *security-scoped bookmark*, y leer fuera de él es exactamente lo que la sandbox promete
    /// impedir. En la CLI y en Windows no hay nada más que lo pare.
    fn if_really_inside(&self, candidate: PathBuf, check_symlinks: bool) -> Option<PathBuf> {
        if !check_symlinks {
            return Some(candidate);
        }
        let (Ok(real), Some(root)) = (candidate.canonicalize(), self.root_canonical.as_ref())
        else {
            // Si no se puede resolver, no se sirve: mejor un 404 que leer algo que no se sabe
            // dónde está.
            return None;
        };
        if real.starts_with(root) {
            return Some(candidate);
        }
        tracing::warn!(
            path = %candidate.display(),
            real = %real.display(),
            "el fichero apunta fuera del directorio auditado (enlace simbólico); se descarta"
        );
        None
    }

    fn is_within_root(&self, candidate: &Path) -> bool {
        // Se compara sobre la forma normalizada léxicamente: `canonicalize` fallaría en
        // ficheros que no existen, que es justo el caso de un 404. La comprobación que sí mira el
        // disco, para los que existen, está en `if_really_inside`.
        let normalized = lexically_normalize(candidate);
        let root = lexically_normalize(&self.root);
        normalized.starts_with(&root)
    }

    /// Recorre el directorio y devuelve la URL de cada documento HTML encontrado.
    ///
    /// Sirve para dos cosas: sembrar el rastreo y, al cruzarlo con lo alcanzado por enlaces,
    /// detectar ficheros que se publican pero a los que no llega ningún enlace.
    pub fn discover_html(&self) -> Vec<Url> {
        let mut found = Vec::new();

        for entry in walkdir::WalkDir::new(&self.root).follow_links(false).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let is_html = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
            if !is_html {
                continue;
            }
            if let Ok(relative) = path.strip_prefix(&self.root) {
                if let Some(url) = self.url_for_relative(relative) {
                    found.push(url);
                }
            }
        }

        found.sort();
        found.dedup();
        found
    }

    /// `about/index.html` se publica como `/about/`, no como `/about/index.html`.
    ///
    /// El sufijo tiene que ser el **segmento** `index.html`, no la cadena: `noindex.html`
    /// también acaba en "index.html" y recortarlo lo convertiría en la ruta fantasma `/no`.
    fn url_for_relative(&self, relative: &Path) -> Option<Url> {
        let as_str = relative.to_str()?.replace('\\', "/");
        let route = if as_str == "index.html" {
            String::new()
        } else if let Some(prefix) = as_str.strip_suffix("/index.html") {
            format!("{prefix}/")
        } else {
            as_str
        };
        self.base.join(&route).ok()
    }

    /// Forma publicada de una URL: la que el sitio servirá de verdad.
    ///
    /// `/about`, `/about/` y `/about/index.html` son la misma página, y el servidor estático
    /// las resuelve al mismo fichero. Sin unificarlas, el rastreo audita cada página dos veces
    /// y las reglas de duplicados disparan falsos positivos que el propio motor ha inventado.
    ///
    /// Esto solo se puede hacer aquí: en modo `http` no se sabe cómo resuelve el servidor
    /// hasta preguntárselo, que es justo lo que dice la regla 8 de la normalización.
    pub fn canonical_url(&self, url: &Url) -> Option<Url> {
        let path = self.resolve_lexical(url)?;
        let relative = path.strip_prefix(&self.root).ok()?;
        self.url_for_relative(relative)
    }
}

impl Fetcher for FilesystemFetcher {
    fn canonicalize(&self, url: &Url) -> Option<Url> {
        self.canonical_url(url)
    }

    fn is_network(&self) -> bool {
        false
    }

    async fn fetch(
        &self,
        url: &Url,
    ) -> Result<std::result::Result<FetchedDoc, FetchFailure>> {
        let started = Instant::now();

        let Some(path) = self.resolve(url) else {
            // Un enlace roto dentro de `dist/` es exactamente el hallazgo que hace útil este
            // modo: se detecta antes de desplegar, no después.
            return Ok(Ok(FetchedDoc {
                url: url.clone(),
                status: 404,
                content_type: None,
                x_robots_tag: None,
                location: None,
                body: Vec::new(),
                response_time_ms: started.elapsed().as_millis() as u32,
            }));
        };

        // El tope de tamaño se aplica también aquí, no solo en HTTP. Un `dist/` puede contener
        // un volcado de varios GB al que apunte un enlace, y leerlo entero en memoria es el mismo
        // problema, aunque el fichero sea del propio usuario.
        let limite = body_limit_for(Some(guess_content_type(&path)), self.max_body_bytes);
        match tokio::fs::metadata(&path).await {
            Ok(meta) if meta.len() > limite => {
                return Ok(Err(FetchFailure {
                    kind: ErrorKind::TooLarge,
                    message: format!("{} bytes, máximo {limite}", meta.len()),
                }))
            }
            _ => {}
        }

        let body = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(e) => {
                return Ok(Err(FetchFailure {
                    kind: ErrorKind::Connection,
                    message: format!("{}: {e}", path.display()),
                }))
            }
        };

        Ok(Ok(FetchedDoc {
            url: url.clone(),
            status: 200,
            content_type: Some(guess_content_type(&path).to_string()),
            x_robots_tag: None,
            location: None,
            body,
            response_time_ms: started.elapsed().as_millis() as u32,
        }))
    }
}

fn guess_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js" | "mjs") => "text/javascript",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("pdf") => "application/pdf",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Normalización léxica: resuelve `.` y `..` sin tocar el disco, para poder validar rutas
/// que todavía no existen.
fn lexically_normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).expect("URL de test válida")
    }

    // --- Clasificación de respuestas ---

    fn doc(status: u16, content_type: Option<&str>) -> FetchedDoc {
        FetchedDoc {
            url: url("https://ejemplo.es/a"),
            status,
            content_type: content_type.map(|s| s.to_string()),
            x_robots_tag: None,
            location: None,
            body: Vec::new(),
            response_time_ms: 0,
        }
    }

    #[test]
    fn reconoce_html_por_su_content_type() {
        assert!(doc(200, Some("text/html; charset=utf-8")).is_html());
        assert!(doc(200, Some("TEXT/HTML")).is_html(), "sin distinguir caja");
        assert!(doc(200, Some("application/xhtml+xml")).is_html());
        assert!(!doc(200, Some("application/pdf")).is_html());
        assert!(!doc(200, None).is_html(), "sin content-type no se asume HTML");
    }

    #[test]
    fn reconoce_las_redirecciones() {
        for s in [301, 302, 307, 308] {
            assert!(doc(s, None).is_redirect(), "{s} es redirección");
        }
        assert!(!doc(200, None).is_redirect());
        assert!(!doc(404, None).is_redirect());
    }

    // --- Política de reintentos ---

    #[test]
    fn reintenta_solo_los_estados_de_sobrecarga() {
        for s in [429, 500, 502, 503, 504] {
            assert!(should_retry_status(s), "{s} debería reintentarse");
        }
        for s in [200, 301, 400, 403, 404, 410] {
            assert!(!should_retry_status(s), "{s} no debería reintentarse");
        }
    }

    #[test]
    fn no_reintenta_fallos_permanentes_de_dns_ni_tls() {
        // Si el nombre no resuelve o el certificado no valida, insistir solo alarga el rastreo.
        assert!(!ErrorKind::Dns.is_retryable());
        assert!(!ErrorKind::Tls.is_retryable());
        assert!(!ErrorKind::TooLarge.is_retryable());
        assert!(ErrorKind::Timeout.is_retryable());
        assert!(ErrorKind::Connection.is_retryable());
    }

    #[test]
    fn el_backoff_crece_y_lleva_jitter() {
        // Con jitter de ±50%: intento 1 ∈ [0,5s, 1,5s], intento 2 ∈ [1s, 3s], intento 3 ∈ [2s, 6s].
        for (attempt, low, high) in [(1, 0.5, 1.5), (2, 1.0, 3.0), (3, 2.0, 6.0)] {
            for _ in 0..50 {
                let d = backoff_delay(attempt).as_secs_f64();
                assert!(d >= low && d <= high, "intento {attempt} dio {d}s, fuera de [{low}, {high}]");
            }
        }
    }

    #[test]
    fn el_jitter_produce_esperas_distintas() {
        // Si veinte workers chocan con el mismo 503, no deben reintentar todos a la vez.
        let a: Vec<_> = (0..20).map(|_| backoff_delay(2).as_nanos()).collect();
        assert!(a.iter().any(|&x| x != a[0]), "el backoff debería variar entre llamadas");
    }

    #[test]
    fn los_error_kind_se_serializan_como_espera_el_esquema() {
        assert_eq!(ErrorKind::Dns.as_str(), "dns");
        assert_eq!(ErrorKind::TooLarge.as_str(), "toolarge");
    }

    // --- Modo filesystem ---

    fn fixture_dir() -> tempdir::Fixture {
        tempdir::Fixture::new()
    }

    #[test]
    fn resuelve_las_rutas_como_lo_haria_un_servidor_estatico() {
        let f = fixture_dir();
        f.write("index.html", "<h1>Inicio</h1>");
        f.write("about/index.html", "<h1>Sobre</h1>");
        f.write("contacto.html", "<h1>Contacto</h1>");
        f.write("style.css", "body{}");

        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));

        // Directorio con index.html
        assert!(fetcher.resolve(&url("https://ejemplo.es/about")).is_some());
        assert!(fetcher.resolve(&url("https://ejemplo.es/about/")).is_some());
        // Fichero .html sin extensión en la URL
        assert!(fetcher.resolve(&url("https://ejemplo.es/contacto")).is_some());
        // Raíz
        assert!(fetcher.resolve(&url("https://ejemplo.es/")).is_some());
        // Fichero literal
        assert!(fetcher.resolve(&url("https://ejemplo.es/style.css")).is_some());
        // Inexistente
        assert!(fetcher.resolve(&url("https://ejemplo.es/no-existe")).is_none());
    }

    #[test]
    fn rechaza_rutas_que_se_escapan_del_directorio_raiz() {
        let f = fixture_dir();
        f.write("index.html", "<h1>x</h1>");
        let fetcher = FilesystemFetcher::new(f.path().join("sub"), url("https://ejemplo.es/"));
        assert!(fetcher.resolve(&url("https://ejemplo.es/../index.html")).is_none());
    }

    #[test]
    fn decodifica_el_percent_encoding_de_la_ruta() {
        let f = fixture_dir();
        f.write("guía de estilo.html", "<h1>x</h1>");
        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));
        assert!(fetcher.resolve(&url("https://ejemplo.es/gu%C3%ADa%20de%20estilo.html")).is_some());
    }

    #[test]
    fn descubre_los_html_del_directorio_como_rutas_publicadas() {
        let f = fixture_dir();
        f.write("index.html", "x");
        f.write("about/index.html", "x");
        f.write("blog/post-1.html", "x");
        f.write("style.css", "x");

        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));
        let found: Vec<String> = fetcher.discover_html().iter().map(|u| u.path().to_string()).collect();

        assert!(found.contains(&"/".to_string()), "index.html se publica como /");
        assert!(found.contains(&"/about/".to_string()), "about/index.html se publica como /about/");
        assert!(found.contains(&"/blog/post-1.html".to_string()));
        assert!(!found.iter().any(|p| p.ends_with(".css")), "solo HTML");
    }

    #[tokio::test]
    async fn lee_un_fichero_y_le_asigna_content_type() {
        let f = fixture_dir();
        f.write("index.html", "<h1>Hola</h1>");
        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));

        let got = fetcher.fetch(&url("https://ejemplo.es/")).await.expect("sin error de core");
        let doc = got.expect("debería encontrarse");
        assert_eq!(doc.status, 200);
        assert!(doc.is_html());
        assert_eq!(doc.body, b"<h1>Hola</h1>");
    }

    #[tokio::test]
    async fn un_enlace_roto_dentro_de_dist_da_404_y_no_un_error() {
        // Es el hallazgo que hace útil este modo: se detecta antes de desplegar.
        let f = fixture_dir();
        f.write("index.html", "x");
        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));

        let doc = fetcher
            .fetch(&url("https://ejemplo.es/pagina-que-no-existe"))
            .await
            .expect("sin error de core")
            .expect("debería ser un 404, no un fallo");
        assert_eq!(doc.status, 404);
    }

    #[test]
    fn un_fichero_que_acaba_en_index_html_no_se_confunde_con_un_indice() {
        // Regresión: `noindex.html` acaba en la cadena "index.html". Recortarla lo convertía
        // en la ruta fantasma `/no`, y la página real nunca se auditaba.
        let f = fixture_dir();
        f.write("noindex.html", "x");
        f.write("index.html", "x");
        f.write("about/index.html", "x");

        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));
        let rutas: Vec<String> =
            fetcher.discover_html().iter().map(|u| u.path().to_string()).collect();

        assert!(rutas.contains(&"/noindex.html".to_string()), "rutas descubiertas: {rutas:?}");
        assert!(!rutas.contains(&"/no".to_string()), "no debe inventarse /no");
        assert!(rutas.contains(&"/".to_string()));
        assert!(rutas.contains(&"/about/".to_string()));
    }

    #[test]
    fn unifica_las_variantes_de_una_misma_pagina() {
        // Regresión: sin esto el rastreo audita `/about` y `/about/` como dos páginas, y las
        // reglas de duplicados disparan sobre duplicados que inventó el propio motor.
        let f = fixture_dir();
        f.write("about/index.html", "x");
        f.write("blog/post-1.html", "x");
        f.write("index.html", "x");

        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));
        let canonical = |s: &str| {
            fetcher.canonical_url(&url(s)).map(|u| u.path().to_string())
        };

        assert_eq!(canonical("https://ejemplo.es/about").as_deref(), Some("/about/"));
        assert_eq!(canonical("https://ejemplo.es/about/").as_deref(), Some("/about/"));
        assert_eq!(canonical("https://ejemplo.es/about/index.html").as_deref(), Some("/about/"));

        // Con extensión y sin ella, la misma página.
        assert_eq!(
            canonical("https://ejemplo.es/blog/post-1"),
            canonical("https://ejemplo.es/blog/post-1.html")
        );

        assert_eq!(canonical("https://ejemplo.es/").as_deref(), Some("/"));
    }

    #[test]
    fn una_url_que_no_existe_no_tiene_forma_canonica() {
        let f = fixture_dir();
        f.write("index.html", "x");
        let fetcher = FilesystemFetcher::new(f.path(), url("https://ejemplo.es/"));
        assert!(fetcher.canonical_url(&url("https://ejemplo.es/no-existe")).is_none());
    }

    #[test]
    fn por_http_no_se_puede_saber_la_forma_publicada_sin_preguntar() {
        // La regla 8 de la normalización: no suponer, observar. Por HTTP no hay nada que
        // observar hasta hacer la petición.
        let fetcher = HttpFetcher::new(DEFAULT_USER_AGENT).expect("construir");
        assert!(fetcher.canonicalize(&url("https://ejemplo.es/about")).is_none());
    }

    #[test]
    fn adivina_el_content_type_por_extension() {
        assert_eq!(guess_content_type(Path::new("a.html")), "text/html; charset=utf-8");
        assert_eq!(guess_content_type(Path::new("a.WEBP")), "image/webp");
        assert_eq!(guess_content_type(Path::new("a.desconocido")), "application/octet-stream");
    }

    #[test]
    fn normaliza_rutas_lexicamente_sin_tocar_el_disco() {
        assert_eq!(lexically_normalize(Path::new("/a/b/../c")), PathBuf::from("/a/c"));
        assert_eq!(lexically_normalize(Path::new("/a/./b")), PathBuf::from("/a/b"));
    }

    #[test]
    fn el_http_fetcher_se_construye_con_el_user_agent_dado() {
        assert!(HttpFetcher::new(DEFAULT_USER_AGENT).is_ok());
    }

    // --- Autenticación básica acotada al host de la semilla ---

    #[test]
    fn el_base64_produce_los_vectores_del_rfc_4648() {
        for (entrada, esperado) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
            ("aladdin:opensesame", "YWxhZGRpbjpvcGVuc2VzYW1l"),
        ] {
            assert_eq!(base64_standard(entrada.as_bytes()), esperado, "para {entrada:?}");
        }
    }

    #[test]
    fn la_cabecera_de_autenticacion_solo_corresponde_al_host_acotado() {
        // La fuga que este ámbito impide: con `follow_external`, el mismo fetcher pide hosts
        // ajenos, y mandarles el `Authorization` regalaría la credencial del staging a
        // cualquier dominio que el sitio enlace.
        let auth = crate::job::HttpBasicAuth::new("consultor", "S3creta");
        let fetcher = HttpFetcher::new(DEFAULT_USER_AGENT)
            .expect("construir")
            .with_basic_auth("pre.cliente.es", &auth);

        let esperada = "Basic Y29uc3VsdG9yOlMzY3JldGE=";
        assert_eq!(
            fetcher.auth_header_for(&url("https://pre.cliente.es/pagina")),
            Some(esperada)
        );
        // El puerto no cambia el ámbito (mismo criterio que `is_internal`)…
        assert_eq!(
            fetcher.auth_header_for(&url("https://pre.cliente.es:8443/robots.txt")),
            Some(esperada)
        );
        // …ni la caja del host.
        assert_eq!(fetcher.auth_header_for(&url("https://PRE.CLIENTE.ES/")), Some(esperada));

        // A cualquier otro host, nada: ni a un dominio ajeno, ni a un subdominio, ni al padre.
        assert_eq!(fetcher.auth_header_for(&url("https://otrodominio.es/")), None);
        assert_eq!(fetcher.auth_header_for(&url("https://api.pre.cliente.es/")), None);
        assert_eq!(fetcher.auth_header_for(&url("https://cliente.es/")), None);
    }

    #[test]
    fn sin_credencial_configurada_no_hay_cabecera_para_nadie() {
        let fetcher = HttpFetcher::new(DEFAULT_USER_AGENT).expect("construir");
        assert_eq!(fetcher.auth_header_for(&url("https://pre.cliente.es/")), None);
    }

    /// Directorio temporal mínimo, para no añadir una dependencia de test por esto.
    mod tempdir {
        use std::path::{Path, PathBuf};

        pub struct Fixture {
            path: PathBuf,
        }

        impl Fixture {
            pub fn new() -> Self {
                let unique = format!(
                    "crawlforge-test-{}-{:?}",
                    std::process::id(),
                    std::thread::current().id()
                );
                let path = std::env::temp_dir().join(unique);
                let _ = std::fs::remove_dir_all(&path);
                std::fs::create_dir_all(&path).expect("crear directorio temporal");
                Self { path }
            }

            pub fn path(&self) -> &Path {
                &self.path
            }

            pub fn write(&self, relative: &str, contents: &str) {
                let full = self.path.join(relative);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent).expect("crear subdirectorio");
                }
                std::fs::write(full, contents).expect("escribir fichero de test");
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests_limite_por_tipo {
    use super::*;

    #[test]
    fn un_sitemap_puede_pesar_mas_que_una_pagina() {
        // El protocolo de sitemaps permite 50 MB sin comprimir; aplicarles el tope de las
        // páginas dejaba sin leer sitemaps legales de unas 200.000 URLs.
        let pagina = body_limit_for(Some("text/html; charset=utf-8"), MAX_BODY_BYTES);
        assert_eq!(pagina, MAX_BODY_BYTES);

        for tipo in ["application/xml", "text/xml", "application/gzip", "application/octet-stream"] {
            assert_eq!(
                body_limit_for(Some(tipo), MAX_BODY_BYTES),
                MAX_XML_BYTES,
                "{tipo} debería admitir el tope de los sitemaps"
            );
        }
    }

    #[test]
    fn sin_tipo_declarado_manda_el_tope_de_pagina() {
        // Lo conservador: si no se sabe qué es, no se le da el margen ancho.
        assert_eq!(body_limit_for(None, MAX_BODY_BYTES), MAX_BODY_BYTES);
    }

    #[test]
    fn un_tope_configurado_mayor_no_se_recorta() {
        // Quien pide 100 MB por configuración no debe acabar con 50 en un XML.
        let grande = 100 * 1024 * 1024;
        assert_eq!(body_limit_for(Some("application/xml"), grande), grande);
        assert_eq!(body_limit_for(Some("text/html"), grande), grande);
    }
}
