//! Cola de rastreo. Ver `docs/03-MOTOR-CRAWL.md §2` y `docs/01-ARQUITECTURA.md §5`.
//!
//! Dos responsabilidades: no repetir URLs y servirlas en orden de profundidad.
//!
//! El orden por profundidad (BFS) importa para el producto, no solo para el motor: cuando un
//! rastreo se trunca por el límite del nivel Free, lo que queda dentro son las páginas más
//! cercanas a la portada, que son las que de verdad importan. Un DFS dejaría fuera secciones
//! enteras de primer nivel mientras baja por una rama cualquiera.

use std::collections::{HashSet, VecDeque};
use url::Url;

/// Una URL pendiente de rastrear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedUrl {
    pub url: Url,
    pub depth: u32,
    /// URL desde la que se descubrió. `None` en las semillas.
    pub discovered_from: Option<i64>,
    /// De dónde salió. Se corresponde con `pages.crawl_depth_source`.
    pub source: DiscoverySource,
}

/// Cómo se descubrió una URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySource {
    Link,
    Sitemap,
    List,
    Adapter,
}

impl DiscoverySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Sitemap => "sitemap",
            Self::List => "list",
            Self::Adapter => "adapter",
        }
    }
}

/// Por qué una URL quedó fuera del rastreo. Se corresponde con `urls.exclusion_reason`.
///
/// Las exclusiones **se registran, no se ocultan**: saber qué quedó fuera y por qué es un
/// hallazgo en sí mismo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionReason {
    Robots,
    Nofollow,
    Depth,
    Pattern,
    Limit,
}

impl ExclusionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Robots => "robots",
            Self::Nofollow => "nofollow",
            Self::Depth => "depth",
            Self::Pattern => "pattern",
            Self::Limit => "limit",
        }
    }
}

/// Umbral a partir del cual la cola desborda a SQLite (`01-ARQUITECTURA.md §5`).
pub const SPILL_THRESHOLD: usize = 100_000;

/// Techo de niveles de profundidad con cola propia.
///
/// La profundidad llega del fichero al reanudar, y el fichero es entrada no confiable («un
/// rastreo = un fichero portable» que se comparte): con un `depth = 4·10⁹` manipulado,
/// `enqueue` reservaba un vector de ~4.000 millones de colas (~160 GB, unos 40 bytes por
/// `VecDeque`) antes de encolar la primera URL. Por encima del techo, todas las URLs
/// comparten el último nivel: su `depth` no se altera —se guarda y se informa tal cual— y
/// solo pierden la separación fina por niveles, que a esas profundidades coincide con el
/// orden de descubrimiento de todos modos. Mil niveles cubren con holgura cualquier cadena
/// real de paginación y acotan la reserva a ~40 KB.
pub const MAX_DEPTH_LEVELS: usize = 1_000;

/// Prefijos de ruta que son infraestructura del CDN o del alojamiento, no contenido del sitio.
///
/// **Descubierto ejecutando.** Un rastreo de un sitio real dio `INDEX-NOFOLLOW-INTERNAL` en 39 de
/// 40 páginas, y los 78 enlaces culpables eran todos `/cdn-cgi/l/email-protection#…`: la
/// reescritura con la que Cloudflare oculta las direcciones de correo del HTML, que lleva
/// `rel=nofollow` de serie. Técnicamente son URLs internas con nofollow, así que la regla estaba
/// en lo cierto y el hallazgo era inútil: no hay nada que arreglar, y 39 avisos falsos en 40
/// páginas hacen que el usuario deje de leer el informe.
///
/// `/cdn-cgi/` es un prefijo reservado por Cloudflare: ningún sitio publica contenido ahí. La
/// URL se registra como excluida —queda el rastro, no desaparece— y no se pide ni cuenta como
/// enlace del sitio.
///
/// Esta lista es **independiente a propósito** de `CrawlLimits::exclude_patterns` (que ya se
/// aplica; ver `pattern.rs`): si fuera su valor por defecto, un usuario que configure sus
/// propios patrones lo sustituiría y desactivaría esta protección sin querer.
pub const INFRASTRUCTURE_PATH_PREFIXES: &[&str] = &["/cdn-cgi/"];

/// ¿La ruta es infraestructura del CDN en vez de contenido del sitio?
pub fn is_infrastructure_path(path: &str) -> bool {
    INFRASTRUCTURE_PATH_PREFIXES.iter().any(|prefix| path.starts_with(prefix))
}

/// La cola de rastreo.
///
/// El índice de vistas guarda solo el hash `i64` de la URL normalizada, no la URL. Con 500.000
/// URLs de 80 caracteres, un `HashSet<String>` costaría decenas de MB; el de hashes son 4 MB.
/// Es una de las razones de que el objetivo de RAM sea alcanzable.
#[derive(Default)]
pub struct Frontier {
    /// Colas por profundidad: se sirve siempre la más superficial que tenga trabajo.
    levels: Vec<VecDeque<QueuedUrl>>,
    seen: HashSet<i64>,
    pending: usize,
    dispatched: u64,
}

impl Frontier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Encola una URL si no se había visto antes.
    ///
    /// Devuelve `true` si entró. Un `false` no es un error: significa que ya estaba, que es lo
    /// normal en un sitio con menú de navegación.
    pub fn enqueue(&mut self, item: QueuedUrl, url_hash: i64) -> bool {
        if !self.seen.insert(url_hash) {
            return false;
        }
        // El índice del nivel se acota; el `depth` del item, no. Ver [`MAX_DEPTH_LEVELS`].
        let depth = (item.depth as usize).min(MAX_DEPTH_LEVELS);
        if self.levels.len() <= depth {
            self.levels.resize_with(depth + 1, VecDeque::new);
        }
        self.levels[depth].push_back(item);
        self.pending += 1;
        true
    }

    /// Marca una URL como vista sin encolarla.
    ///
    /// Es lo que se hace con las excluidas: quedan registradas en el almacén y no se vuelven a
    /// considerar, pero tampoco se rastrean.
    pub fn mark_seen(&mut self, url_hash: i64) -> bool {
        self.seen.insert(url_hash)
    }

    pub fn has_seen(&self, url_hash: i64) -> bool {
        self.seen.contains(&url_hash)
    }

    /// Saca la siguiente URL: la de menor profundidad disponible.
    pub fn dequeue(&mut self) -> Option<QueuedUrl> {
        for level in &mut self.levels {
            if let Some(item) = level.pop_front() {
                self.pending -= 1;
                self.dispatched += 1;
                return Some(item);
            }
        }
        None
    }

    pub fn pending(&self) -> usize {
        self.pending
    }

    pub fn dispatched(&self) -> u64 {
        self.dispatched
    }

    /// URLs distintas vistas, incluidas las excluidas y las ya rastreadas.
    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending == 0
    }

    /// ¿Conviene ya desbordar a SQLite?
    ///
    /// De momento solo se informa: el desbordamiento a disco no está implementado; los sujetos de
    /// los bancos de prueba (hasta 50k URLs) no llegan al umbral.
    pub fn should_spill(&self) -> bool {
        self.pending > SPILL_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(url: &str, depth: u32) -> QueuedUrl {
        QueuedUrl {
            url: Url::parse(url).expect("URL de test válida"),
            depth,
            discovered_from: None,
            source: DiscoverySource::Link,
        }
    }

    #[test]
    fn encola_y_desencola() {
        let mut f = Frontier::new();
        assert!(f.is_empty());
        assert!(f.enqueue(item("https://ejemplo.es/", 0), 1));
        assert_eq!(f.pending(), 1);

        let got = f.dequeue().expect("debería haber una URL");
        assert_eq!(got.url.as_str(), "https://ejemplo.es/");
        assert!(f.is_empty());
        assert_eq!(f.dispatched(), 1);
    }

    #[test]
    fn no_repite_una_url_ya_vista() {
        let mut f = Frontier::new();
        assert!(f.enqueue(item("https://ejemplo.es/a", 0), 42));
        assert!(!f.enqueue(item("https://ejemplo.es/a", 1), 42), "mismo hash, no debe entrar");
        assert_eq!(f.pending(), 1);
        assert_eq!(f.seen_count(), 1);
    }

    #[test]
    fn sirve_primero_lo_mas_superficial() {
        // Un truncado por límite debe dejar dentro lo cercano a la portada.
        let mut f = Frontier::new();
        f.enqueue(item("https://ejemplo.es/profundo", 5), 1);
        f.enqueue(item("https://ejemplo.es/medio", 2), 2);
        f.enqueue(item("https://ejemplo.es/portada", 0), 3);

        assert_eq!(f.dequeue().expect("1").depth, 0);
        assert_eq!(f.dequeue().expect("2").depth, 2);
        assert_eq!(f.dequeue().expect("3").depth, 5);
    }

    #[test]
    fn dentro_de_un_nivel_respeta_el_orden_de_llegada() {
        let mut f = Frontier::new();
        f.enqueue(item("https://ejemplo.es/a", 1), 1);
        f.enqueue(item("https://ejemplo.es/b", 1), 2);
        assert_eq!(f.dequeue().expect("a").url.path(), "/a");
        assert_eq!(f.dequeue().expect("b").url.path(), "/b");
    }

    #[test]
    fn una_url_encolada_despues_pero_mas_superficial_se_adelanta() {
        let mut f = Frontier::new();
        f.enqueue(item("https://ejemplo.es/hondo", 9), 1);
        assert_eq!(f.pending(), 1);
        f.enqueue(item("https://ejemplo.es/somero", 1), 2);
        assert_eq!(f.dequeue().expect("somero").depth, 1);
    }

    #[test]
    fn marcar_vista_evita_reencolar_sin_ocupar_la_cola() {
        // Es lo que se hace con las excluidas por robots.txt.
        let mut f = Frontier::new();
        assert!(f.mark_seen(7));
        assert!(f.has_seen(7));
        assert!(f.is_empty(), "marcar como vista no encola");
        assert!(!f.enqueue(item("https://ejemplo.es/x", 0), 7));
    }

    #[test]
    fn una_cola_vacia_devuelve_none() {
        let mut f = Frontier::new();
        assert!(f.dequeue().is_none());
        f.enqueue(item("https://ejemplo.es/a", 0), 1);
        f.dequeue();
        assert!(f.dequeue().is_none());
    }

    #[test]
    fn avisa_de_desbordamiento_solo_al_superar_el_umbral() {
        let mut f = Frontier::new();
        for i in 0..10 {
            f.enqueue(item(&format!("https://ejemplo.es/{i}"), 0), i as i64);
        }
        assert!(!f.should_spill());
        assert_eq!(f.pending(), 10);
    }

    #[test]
    fn los_motivos_de_exclusion_coinciden_con_el_esquema() {
        assert_eq!(ExclusionReason::Robots.as_str(), "robots");
        assert_eq!(ExclusionReason::Nofollow.as_str(), "nofollow");
        assert_eq!(ExclusionReason::Depth.as_str(), "depth");
        assert_eq!(ExclusionReason::Pattern.as_str(), "pattern");
        assert_eq!(ExclusionReason::Limit.as_str(), "limit");
    }

    #[test]
    fn los_origenes_de_descubrimiento_coinciden_con_el_esquema() {
        assert_eq!(DiscoverySource::Link.as_str(), "link");
        assert_eq!(DiscoverySource::Sitemap.as_str(), "sitemap");
        assert_eq!(DiscoverySource::List.as_str(), "list");
        assert_eq!(DiscoverySource::Adapter.as_str(), "adapter");
    }

    #[test]
    fn aguanta_un_salto_grande_de_profundidad_sin_reservar_de_mas() {
        let mut f = Frontier::new();
        f.enqueue(item("https://ejemplo.es/x", 50), 1);
        assert_eq!(f.pending(), 1);
        assert_eq!(f.dequeue().expect("x").depth, 50);
    }

    #[test]
    fn una_profundidad_manipulada_no_reserva_un_nivel_por_unidad() {
        // El caso del fichero manipulado en `resume`: sin el techo, esta profundidad
        // reservaba cien millones de colas (~4 GB) para una sola URL; el original del
        // ataque, 4·10⁹, eran ~160 GB.
        let mut f = Frontier::new();
        f.enqueue(item("https://ejemplo.es/manipulada", 100_000_000), 1);
        assert!(
            f.levels.len() <= MAX_DEPTH_LEVELS + 1,
            "los niveles reservados deben estar acotados, hay {}",
            f.levels.len()
        );
        // El valor de `depth` viaja intacto: se acota el índice del nivel, no el dato.
        let got = f.dequeue().expect("la URL sigue en la cola");
        assert_eq!(got.depth, 100_000_000);

        // Y el orden por profundidad sobrevive para todo lo que queda bajo el techo.
        f.enqueue(item("https://ejemplo.es/otra-honda", u32::MAX), 2);
        f.enqueue(item("https://ejemplo.es/somera", 3), 3);
        assert_eq!(f.dequeue().expect("somera").depth, 3);
        assert_eq!(f.dequeue().expect("honda").depth, u32::MAX);
    }
}
