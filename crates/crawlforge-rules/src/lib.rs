//! Catálogo de reglas de auditoría. Ver `docs/04-CATALOGO-REGLAS.md`.
//!
//! Crate aparte a propósito: **las reglas son el producto** y evolucionan a otro ritmo que el
//! motor.
//!
//! Dos modos de evaluación:
//!
//! - [`PageRule`] se evalúa durante el rastreo, en streaming, sobre una página suelta. Barato.
//! - [`SiteRule`] necesita el rastreo completo (duplicados, huérfanas, profundidad) y se
//!   ejecuta en una pasada final con SQL sobre el almacén.
//!
//! # Cómo se añade una regla
//!
//! 1. Declara su [`RuleMeta`] como `pub static` en el módulo de su categoría. El ID es para
//!    siempre: un diff histórico depende de que no cambie de significado.
//! 2. Implementa [`PageRule`] o [`SiteRule`] sobre un struct vacío.
//! 3. Añádela a la función `page_rules()` o `site_rules()` de su módulo.
//! 4. Escribe su fixture en `fixtures/<RULE-ID>.html` y su test en el módulo. **Las dos cosas,
//!    sin excepción.** Un test comprueba que ninguna regla se queda sin fixture, y otro que el
//!    fixture dispara la regla al rastrearlo de verdad.
//!
//! `MetaTitleMissing` y `MetaTitleDuplicate` son los dos ejemplos a copiar: una de página y una
//! de conjunto, con su meta, su evaluación y sus tests.

use rusqlite::Connection;

pub mod asset;
pub mod canon;
pub mod content;
pub mod hreflang;
pub mod http;
pub mod index;
pub mod meta;
pub mod social;

pub use index::{deep_page_shape, DeepPageShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Info => "info",
        }
    }
}

/// Familia de la regla. Se corresponde con el prefijo del ID y con las secciones de
/// `docs/04-CATALOGO-REGLAS.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Indexability,
    Http,
    Meta,
    Canonical,
    Duplicate,
    Content,
    Asset,
    Hreflang,
    Schema,
    Social,
    Links,
    Accessibility,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Indexability => "indexability",
            Self::Http => "http",
            Self::Meta => "meta",
            Self::Canonical => "canonical",
            Self::Duplicate => "duplicate",
            Self::Content => "content",
            Self::Asset => "asset",
            Self::Hreflang => "hreflang",
            Self::Schema => "schema",
            Self::Social => "social",
            Self::Links => "links",
            Self::Accessibility => "accessibility",
        }
    }

    /// Prefijos de ID admitidos para esta categoría. Un test comprueba que el ID de cada regla
    /// empieza por uno de ellos: es la forma de que la categoría y el ID no se contradigan.
    pub fn id_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Indexability => &["INDEX"],
            Self::Http => &["HTTP"],
            Self::Meta => &["META"],
            Self::Canonical => &["CANON"],
            Self::Duplicate => &["DUP"],
            Self::Content => &["CONTENT"],
            Self::Asset => &["ASSET"],
            Self::Hreflang => &["HREFLANG"],
            Self::Schema => &["SCHEMA"],
            Self::Social => &["SOCIAL"],
            Self::Links => &["LINK"],
            Self::Accessibility => &["A11Y"],
        }
    }
}

/// Nivel a partir del cual una regla se aplica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Agency,
}

/// Cuándo se puede decidir la regla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Con la página delante, durante el rastreo.
    Page,
    /// Solo con el rastreo terminado: duplicados, huérfanas, cadenas de redirección.
    Site,
}

/// Referencia normativa de un hallazgo.
///
/// Existe desde ya, vacía en casi todas las reglas, porque el futuro bloque de
/// accesibilidad tiene que citar WCAG 2.1 AA, EN 301 549 y la Directiva UE 2019/882, y añadir
/// el campo entonces obligaría a tocar las ~85 reglas. Ver `docs/04-CATALOGO-REGLAS.md §12`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    pub standard: &'static str,
    pub clause: &'static str,
    pub url: &'static str,
}

/// Todo lo que se sabe de una regla sin evaluarla.
///
/// Está separado de la implementación para que la CLI y las UI puedan listar el catálogo, y para
/// que los textos vivan **en el crate** y no en cada interfaz: si el nombre de una regla estuviera
/// en la app de macOS, Windows y la CLI dirían cosas distintas.
#[derive(Debug, Clone, Copy)]
pub struct RuleMeta {
    /// `CATEGORIA-SUJETO-CONDICION`, en inglés y estable para siempre.
    pub id: &'static str,
    pub severity: Severity,
    pub category: Category,
    pub min_tier: Tier,
    pub scope: Scope,
    /// Nombre corto, para la columna de una tabla.
    pub name_es: &'static str,
    pub name_en: &'static str,
    /// Qué es y por qué importa. Es lo que el usuario lee para decidir si le hace caso.
    pub desc_es: &'static str,
    pub desc_en: &'static str,
    pub references: &'static [Reference],
}

impl RuleMeta {
    pub fn name(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Es => self.name_es,
            Lang::En => self.name_en,
        }
    }

    pub fn description(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Es => self.desc_es,
            Lang::En => self.desc_en,
        }
    }
}

/// Idiomas del catálogo.
///
/// **El inglés es el idioma de origen y el español una traducción**, no al revés: es el orden en
/// que se publica el producto y el que decide qué texto manda cuando los dos discrepan.
///
/// Los dos existen desde el día uno, no como retrofit. Cuando entre un tercer idioma, los campos
/// `name_*` y `desc_*` de [`RuleMeta`] dejarán de servir —no se puede añadir un par de campos por
/// idioma a cincuenta reglas— y habrá que mover los textos a un catálogo aparte indexado por
/// idioma. La API de [`RuleMeta::name`] y [`RuleMeta::description`] ya está pensada para que ese
/// cambio no se note fuera de este crate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    En,
    Es,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "es" | "es-es" | "spanish" | "español" => Some(Self::Es),
            "en" | "en-us" | "en-gb" | "english" => Some(Self::En),
            _ => None,
        }
    }
}

pub const RULES_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Un hallazgo. Se corresponde con una fila de `issues`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub rule_id: &'static str,
    pub severity: Severity,
    pub category: Category,
    pub detail_json: Option<String>,
    /// Agrupa hallazgos equivalentes: el hash del título repetido, por ejemplo. Permite que la
    /// UI diga «este título está en 14 páginas» en vez de listar 14 hallazgos sueltos.
    pub group_key: Option<String>,
}

impl Issue {
    /// Un hallazgo de esta regla, sin detalle. Copia severidad y categoría de la meta para que
    /// no puedan desincronizarse.
    pub fn new(meta: &'static RuleMeta) -> Self {
        Self {
            rule_id: meta.id,
            severity: meta.severity,
            category: meta.category,
            detail_json: None,
            group_key: None,
        }
    }

    /// Detalle en JSON. Es lo que la UI usa para explicar el hallazgo concreto: el título que se
    /// repite, los milisegundos que tardó, la URL de destino.
    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail_json = Some(detail.to_string());
        self
    }

    /// Ajusta la severidad de **este hallazgo**, apartándola de la que declara la regla.
    ///
    /// La severidad de [`RuleMeta`] es la del caso general; hay hallazgos donde el mismo dato
    /// pesa distinto y la regla lo sabe con certeza —un `noindex` en la portada no es un
    /// `noindex` en una etiqueta; un título repetido dentro de una serie paginada no es el mismo
    /// defecto que dos artículos compitiendo—. Este método existe para esos casos y solo para
    /// ellos: el ajuste tiene que estar razonado en la regla que lo hace, y el `detail_json`
    /// tiene que decir por qué, porque una severidad que cambia sin explicación en el informe es
    /// peor que una constante equivocada.
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_group(mut self, key: impl Into<String>) -> Self {
        self.group_key = Some(key.into());
        self
    }
}

/// Una imagen de la página, tal como la ve una regla.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImageView<'a> {
    pub src: &'a str,
    /// `None` es «sin atributo `alt`»; `Some("")` es un `alt=""` deliberado de imagen
    /// decorativa. Son cosas distintas y hay una regla para cada una.
    pub alt: Option<&'a str>,
    pub width_attr: Option<i64>,
    pub height_attr: Option<i64>,
    /// Texto del `<a>` que envuelve la imagen. `None` si no va dentro de un enlace.
    pub anchor_text: Option<&'a str>,
}

impl ImageView<'_> {
    pub fn in_anchor(&self) -> bool {
        self.anchor_text.is_some()
    }
}

/// Un enlace o recurso de la página.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkView<'a> {
    /// El `href` **tal como venía en el HTML**: puede ser relativo, absoluto o sin esquema.
    ///
    /// No se resuelve a absoluto a propósito. Resolverlo obligaría a construir una cadena nueva
    /// por enlace, y en un sitio muy enlazado eso son millones de asignaciones que ninguna regla
    /// necesita: `is_internal` ya viene resuelto, y las reglas que miran el esquema —contenido
    /// mixto— quieren precisamente saber si el HTML dice `http://` de forma explícita, porque un
    /// enlace relativo hereda el de la página y nunca es contenido mixto.
    pub href: &'a str,
    pub anchor: Option<&'a str>,
    pub is_nofollow: bool,
    pub is_internal: bool,
    /// `true` para `<img>`, `<script src>` y `<link rel=stylesheet>`: son recursos que la página
    /// carga, no enlaces que el usuario sigue.
    pub is_resource: bool,
    /// `true` si el destino es infraestructura del CDN y no contenido del sitio.
    ///
    /// Hoy significa `/cdn-cgi/`, el prefijo reservado de Cloudflare. Lo que lo hizo necesario:
    /// Cloudflare reescribe las direcciones de correo del HTML como
    /// `/cdn-cgi/l/email-protection#…` con `rel=nofollow`, y eso hacía que
    /// `INDEX-NOFOLLOW-INTERNAL` avisara en 39 de 40 páginas de un sitio real por algo que nadie
    /// puso ahí ni puede quitar. Una regla que habla de los enlaces del sitio debe ignorarlos.
    pub is_infrastructure: bool,
}

/// Lo que una regla de página necesita saber para decidir.
///
/// Es deliberadamente plano y prestado: se construye una vez por página durante el rastreo y
/// no debe obligar a copiar cadenas. Implementa [`Default`] para que un test solo tenga que
/// escribir los campos que le importan; para el caso normal, [`PageContext::indexable_html`].
#[derive(Debug, Clone, Default)]
pub struct PageContext<'a> {
    pub url: &'a str,
    pub status: u16,
    pub is_html: bool,
    pub is_internal: bool,
    pub is_https: bool,
    /// La URL está bloqueada por `robots.txt` pero se llegó a ella por un enlace interno.
    pub blocked_by_robots: bool,
    pub content_type: Option<&'a str>,
    /// Time to first byte. `None` en el modo `filesystem`, donde no significa nada.
    pub ttfb_ms: Option<u32>,
    pub html_bytes: u64,
    pub title: Option<&'a str>,
    pub title_count: u32,
    pub meta_description: Option<&'a str>,
    pub meta_robots: Option<&'a str>,
    pub x_robots_tag: Option<&'a str>,
    pub meta_refresh: Option<&'a str>,
    pub viewport: Option<&'a str>,
    pub lang: Option<&'a str>,
    pub h1: Option<&'a str>,
    pub h1_count: u32,
    /// Niveles de los encabezados en el orden en que aparecen. `[1, 2, 4]` es un salto.
    pub heading_levels: &'a [u8],
    /// Texto de cada encabezado, en el mismo orden que [`Self::heading_levels`].
    ///
    /// Existe porque el diagnóstico de un salto de encabezados **es su texto**: el `detail_json`
    /// de `CONTENT-HEADING-SKIP` decía `{"from":1,"to":4}` en 16.764 páginas de un rastreo real
    /// y hubo que abrir el HTML a mano para descubrir que el culpable era un único
    /// `<h4>` de la firma del autor. Los tests pueden dejarlo vacío: la regla trata la ausencia
    /// de texto como «no se sabe», nunca como error.
    pub heading_texts: &'a [&'a str],
    /// Canonical resuelto a absoluto.
    pub canonical: Option<&'a str>,
    /// Canonical tal como venía en el HTML, para distinguir el relativo del absoluto.
    pub canonical_raw: Option<&'a str>,
    pub canonical_count: u32,
    pub is_indexable: bool,
    pub word_count: u32,
    pub images: &'a [ImageView<'a>],
    pub links: &'a [LinkView<'a>],
    /// `(código, href)` de cada `link rel=alternate hreflang`, con el href **tal como venía en
    /// el HTML**: igual que en [`LinkView::href`], puede ser relativo. Quien compare destinos
    /// tiene que resolverlo contra [`Self::url`].
    pub hreflang: &'a [(&'a str, &'a str)],
    /// Claves Open Graph presentes: `og:title`, `og:image`…
    pub og_keys: &'a [&'a str],
}

impl<'a> PageContext<'a> {
    /// ¿La respuesta sirvió contenido con éxito (2xx)?
    ///
    /// Es la puerta de entrada de las reglas que auditan **el HTML servido** —imágenes, enlaces,
    /// canonicals, hreflang— y que no filtran por `is_indexable`. Sin ella, la plantilla de error
    /// del tema se audita una vez por cada URL rota: en un rastreo real, cada 404 producía tres
    /// hallazgos —el 404, el logo sin nombre accesible y el nofollow del pie— cuando el único
    /// accionable es el 404, que ya tiene su regla `HTTP`. Un 301 con cuerpo HTML tampoco se
    /// audita: ese cuerpo no lo ve nadie, el navegador y Google siguen la redirección.
    ///
    /// Las reglas cuya conclusión **es** el código de estado (`HTTP-5XX`) o que miden el servidor
    /// y no el HTML (`HTTP-SLOW-RESPONSE`: un TTFB lento lo es con cualquier estado) no la usan.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Una página HTML interna, indexable y sana. **Pensado para los tests:** deja que cada uno
    /// escriba solo el defecto que quiere provocar, en vez de veintiocho campos.
    ///
    /// El `word_count` va por encima del umbral de `CONTENT-THIN` a propósito, para que el test
    /// de una regla no dispare otra por descuido.
    pub fn indexable_html(url: &'a str) -> Self {
        Self {
            url,
            status: 200,
            is_html: true,
            is_internal: true,
            is_https: url.starts_with("https://"),
            is_indexable: true,
            word_count: 500,
            html_bytes: 20_000,
            ..Default::default()
        }
    }
}

/// Regla evaluable sobre una sola página, durante el rastreo.
pub trait PageRule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue>;

    fn id(&self) -> &'static str {
        self.meta().id
    }
    fn severity(&self) -> Severity {
        self.meta().severity
    }
    fn category(&self) -> Category {
        self.meta().category
    }
    fn min_tier(&self) -> Tier {
        self.meta().min_tier
    }
}

/// Regla que necesita el rastreo entero. Se ejecuta al final, con SQL sobre el almacén.
pub trait SiteRule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;
    /// Devuelve `(url_hash, issue)`. Un `None` en el hash es un hallazgo de sitio.
    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>>;

    fn id(&self) -> &'static str {
        self.meta().id
    }
    fn severity(&self) -> Severity {
        self.meta().severity
    }
    fn category(&self) -> Category {
        self.meta().category
    }
    fn min_tier(&self) -> Tier {
        self.meta().min_tier
    }
}

/// Todas las reglas de página del catálogo, en orden de categoría.
pub fn page_rules() -> Vec<Box<dyn PageRule>> {
    let mut out = Vec::new();
    out.extend(index::page_rules());
    out.extend(http::page_rules());
    out.extend(meta::page_rules());
    out.extend(canon::page_rules());
    out.extend(content::page_rules());
    out.extend(asset::page_rules());
    out.extend(hreflang::page_rules());
    out.extend(social::page_rules());
    out
}

/// Todas las reglas de conjunto del catálogo, en orden de categoría.
pub fn site_rules() -> Vec<Box<dyn SiteRule>> {
    let mut out = Vec::new();
    out.extend(index::site_rules());
    out.extend(http::site_rules());
    out.extend(meta::site_rules());
    out.extend(canon::site_rules());
    out.extend(content::site_rules());
    out.extend(asset::site_rules());
    out.extend(hreflang::site_rules());
    out.extend(social::site_rules());
    out
}

/// El catálogo completo, para `crawlforge rules` y para la lista de la UI.
///
/// Se deriva del registro en vez de mantener una lista aparte: una regla implementada pero no
/// registrada no aparecería, y una registrada no puede faltar aquí.
pub fn catalog() -> Vec<&'static RuleMeta> {
    let mut out: Vec<&'static RuleMeta> = page_rules().iter().map(|r| r.meta()).collect();
    out.extend(site_rules().iter().map(|r| r.meta()));
    out
}

/// Reglas que no se pueden afirmar sobre un rastreo truncado.
///
/// Su conclusión depende de que el grafo de enlaces esté **completo**. Si el rastreo se cortó
/// —por el tope del nivel gratuito, por `--max-urls` o por tiempo—, las URLs que quedaron
/// pendientes no tienen enlaces salientes registrados, así que el grafo tiene agujeros y las dos
/// preguntas que hacen estas reglas se contestan mal:
///
/// - «¿a cuántos clics está esta página?» — inalcanzable en el grafo parcial no es lo mismo que
///   profunda en el sitio.
/// - «¿nadie enlaza a esta página?» — puede que la enlace una de las que no se rastrearon.
///
/// **Esto se descubrió ejecutando**, no escribiendo: un rastreo de 40 URLs de un blog real dio
/// `INDEX-DEEP-PAGE` en 39 de 40 páginas. Las páginas venían del sitemap y la portada solo
/// enlazaba a una de ellas, así que el recorrido no llegaba a ninguna. En el nivel gratuito, que
/// corta a 1.000 URLs, ese falso positivo habría salido en todos los sitios grandes.
///
/// El motor las descarta cuando `crawl_meta.truncated` no es nulo. Es preferible no decir nada a
/// decir algo falso: un auditor con un 97% de falsos positivos en una regla deja de mirar el
/// informe entero.
/// `INDEX-ORPHAN-PAGE` se sumó el 2026-08-01 por el mismo motivo, encontrado otra vez rastreando:
/// una página descargada a la que nadie enlaza **de entre lo que se llegó a rastrear** no es
/// huérfana, es una página cuyo enlazador quedó fuera del corte. La migración 005 quitó de esa
/// regla el otro falso positivo, el de las imágenes; este solo se puede quitar callando.
/// `INDEX-SECTION-DISCONNECTED` nació dentro de esta lista: «inalcanzable desde la portada» es
/// exactamente la afirmación que un grafo con agujeros no puede sostener.
pub const REQUIERE_GRAFO_COMPLETO: &[&str] = &[
    "INDEX-DEEP-PAGE",
    "INDEX-NO-INTERNAL-LINKS-IN",
    "INDEX-ORPHAN-PAGE",
    "INDEX-SECTION-DISCONNECTED",
];

/// Prefijos de ruta que son infraestructura del CDN, no contenido del sitio.
///
/// **Duplica `crawlforge_core::frontier::INFRASTRUCTURE_PATH_PREFIXES` a propósito**, igual que
/// `index::declares_noindex` duplica `job::has_noindex`: este crate no conoce al core y la
/// dirección de la dependencia es la contraria. Las dos listas tienen que coincidir.
///
/// La necesitan las reglas de **sitio**: el filtro de página ya llega hecho en
/// [`LinkView::is_infrastructure`], pero una regla SQL como `INDEX-ROBOTS-BLOCKED` lee `urls`
/// directamente y tiene que excluir estas rutas ella misma. Lo que lo hizo necesario: Cloudflare
/// inyecta enlaces a `/cdn-cgi/` **y** los bloquea con `Disallow: /cdn-cgi/` en el robots.txt
/// que él mismo gestiona, así que los tres hallazgos `critical` de un rastreo real eran cosas
/// que el dueño del sitio no puso y no puede arreglar.
pub const INFRASTRUCTURE_PATH_PREFIXES: &[&str] = &["/cdn-cgi/"];

/// Condición SQL «la columna de ruta no es infraestructura del CDN», derivada de
/// [`INFRASTRUCTURE_PATH_PREFIXES`] para que la lista viva en un solo sitio. Los prefijos son
/// literales del propio crate —nunca entrada del usuario—, por eso pueden interpolarse.
pub fn sql_not_infrastructure(path_column: &str) -> String {
    INFRASTRUCTURE_PATH_PREFIXES
        .iter()
        .map(|prefix| format!("{path_column} NOT LIKE '{prefix}%'"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// ¿La regla necesita un rastreo completo para poder afirmar lo que afirma?
pub fn requiere_grafo_completo(rule_id: &str) -> bool {
    REQUIERE_GRAFO_COMPLETO.contains(&rule_id)
}

/// Páginas a partir de las cuales un grupo de hallazgos con el mismo `group_key` se considera
/// **un defecto de plantilla** y se presenta como un solo hallazgo con recuento.
///
/// El número sale de medir cinco rastreos reales (2026-08-01): los grupos que de verdad eran la
/// plantilla —el enlace del pie de un medio, el `<h5>CONTACTO` del pie de una agencia— tenían
/// 645, 11.799 y 18.085 páginas; los grupos que eran coincidencia tenían como mucho 7. Con 30
/// hay un margen de 4x sobre la mayor coincidencia observada y de 20x bajo la menor plantilla
/// observada: si un rastreo futuro cae en medio, el que está mal es este número, no el criterio.
pub const TEMPLATE_GROUP_MIN_PAGES: i64 = 30;

/// La cláusula de rastreo pequeño: por debajo de [`TEMPLATE_GROUP_MIN_PAGES`] páginas afectadas,
/// un grupo sigue siendo plantilla si cubre al menos este porcentaje de las páginas rastreadas.
/// Un rastreo de prueba de 20 páginas con el defecto del pie en 18 es la misma plantilla que en
/// el sitio completo.
pub const TEMPLATE_GROUP_MIN_SHARE_PCT: i64 = 80;

/// Suelo absoluto de la cláusula porcentual: dos páginas con el mismo `group_key` no son una
/// plantilla, son dos páginas.
pub const TEMPLATE_GROUP_FLOOR_PAGES: i64 = 5;

/// ¿Un grupo de `group_pages` hallazgos con el mismo `group_key`, en un rastreo con
/// `total_pages` páginas HTML, es un defecto de plantilla?
///
/// Vive en este crate y no en la CLI porque es semántica del hallazgo, no maquetación: la app de
/// macOS y la de Windows tienen que colapsar exactamente los mismos grupos que el informe de la
/// CLI, o el mismo fichero contaría cosas distintas según dónde se abra.
///
/// El colapso es **solo de presentación**: cada página afectada conserva su fila en `issues`,
/// porque quien exporta o consulta por SQL necesita saber exactamente qué páginas son.
pub fn is_template_group(group_pages: i64, total_pages: i64) -> bool {
    if group_pages >= TEMPLATE_GROUP_MIN_PAGES {
        return true;
    }
    group_pages >= TEMPLATE_GROUP_FLOOR_PAGES
        && total_pages > 0
        && group_pages * 100 >= total_pages * TEMPLATE_GROUP_MIN_SHARE_PCT
}

/// Porcentaje de páginas rastreadas a partir del cual una regla es **dominante**: el problema
/// es una propiedad del sitio, no una lista de páginas que arreglar una a una.
///
/// Es el segundo colapso de presentación, hermano de [`is_template_group`] y para el caso que
/// aquel no puede cubrir: hallazgos masivos **ciertos y sin causa común hashable**. En el
/// rastreo completo de un medio real (216.349 páginas), `INDEX-DEEP-PAGE` dio 202.392 hallazgos
/// todos verdaderos —cada página es genuinamente distinta, no hay `group_key` que compartan— y
/// un informe que abre con esa cifra no se lee, exactamente igual que cuando eran falsos
/// positivos.
///
/// El número sale de medir seis rastreos reales (2026-08-03): las reglas cuya causa era de
/// verdad sistémica —la arquitectura del archivo, el sufijo de título de la plantilla, la
/// lentitud del servidor, el `noindex` de un plugin— afectaban a entre el 41,6% y el 100% de
/// las páginas; las que eran listas de páginas a arreglar una a una (imágenes pesadas,
/// descripciones largas, contenido escaso) quedaban en el 37,6% o menos. El 40 parte ese hueco:
/// si un rastreo futuro cae en medio, el que está mal es este número, no el criterio.
pub const PERVASIVE_MIN_SHARE_PCT: i64 = 40;

/// Suelo absoluto de [`is_pervasive`]: por debajo de estas páginas afectadas, el recuento se
/// lee de un vistazo y reformularlo como porcentaje no aporta nada (3 de 6 páginas no son «el
/// 50% del sitio», son tres páginas).
pub const PERVASIVE_MIN_PAGES: i64 = 20;

/// ¿Una regla con `affected_pages` páginas afectadas, en un rastreo de `total_pages` páginas
/// HTML, es un problema dominante del sitio?
///
/// Vive en este crate por la misma razón que [`is_template_group`]: es semántica del hallazgo,
/// no maquetación, y las apps tienen que reformular exactamente las mismas reglas que la CLI.
///
/// **El colapso que gobierna es solo de presentación y nunca resta información**: la línea del
/// informe conserva el recuento y le añade el porcentaje; cada página afectada conserva su fila
/// en `issues`, el export la lleva, y `report --rule` la lista. Por eso es seguro aplicarlo a
/// cualquier severidad: una regla `critical` que afecta al 90% del sitio sigue enseñando su
/// recuento entero, solo que además dice que es el 90%.
pub fn is_pervasive(affected_pages: i64, total_pages: i64) -> bool {
    affected_pages >= PERVASIVE_MIN_PAGES
        && total_pages > 0
        && affected_pages * 100 >= total_pages * PERVASIVE_MIN_SHARE_PCT
}

/// Ruta del fixture de una regla, si existe.
///
/// Cada regla tiene su caso de prueba en `fixtures/`, de una de estas dos formas:
///
/// - `fixtures/<RULE-ID>.html` — una página basta para provocar el defecto.
/// - `fixtures/<RULE-ID>/` — hacen falta varias páginas: duplicados, enlaces roto, huérfanas.
///
/// El rastreo de verdad de estos ficheros está en `crawlforge-core/tests/fixtures_de_reglas.rs`:
/// aquí no se puede, porque el parser vive en el core y el core depende de este crate, no al
/// revés.
pub fn fixture_path(rule_id: &str) -> Option<std::path::PathBuf> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let fichero = base.join(format!("{rule_id}.html"));
    if fichero.is_file() {
        return Some(fichero);
    }
    let directorio = base.join(rule_id);
    directorio.is_dir().then_some(directorio)
}

/// Las reglas de página que aplican a un nivel. El límite se aplica **en el core**, no en la UI.
pub fn page_rules_for_tier(tier: Tier) -> Vec<Box<dyn PageRule>> {
    page_rules().into_iter().filter(|r| r.min_tier() <= tier).collect()
}

/// Las reglas de conjunto que aplican a un nivel.
pub fn site_rules_for_tier(tier: Tier) -> Vec<Box<dyn SiteRule>> {
    site_rules().into_iter().filter(|r| r.min_tier() <= tier).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ningun_id_esta_repetido() {
        let mut vistos = HashSet::new();
        for meta in catalog() {
            assert!(vistos.insert(meta.id), "ID duplicado en el catálogo: {}", meta.id);
        }
    }

    #[test]
    fn el_id_concuerda_con_su_categoria() {
        for meta in catalog() {
            let prefijos = meta.category.id_prefixes();
            assert!(
                prefijos.iter().any(|p| meta.id.starts_with(p)),
                "{} está en la categoría {:?}, que espera un ID que empiece por {:?}",
                meta.id,
                meta.category,
                prefijos
            );
        }
    }

    #[test]
    fn toda_regla_tiene_textos_en_los_dos_idiomas() {
        // Las cadenas de UI van siempre por el sistema de localización, y el catálogo es la
        // primera de esas superficies. Una regla sin traducir se vería en inglés en la app en
        // español, que es exactamente lo que el proyecto no hace.
        for meta in catalog() {
            for (lang, nombre, desc) in [
                (Lang::Es, meta.name_es, meta.desc_es),
                (Lang::En, meta.name_en, meta.desc_en),
            ] {
                assert!(!nombre.trim().is_empty(), "{} sin nombre en {:?}", meta.id, lang);
                assert!(!desc.trim().is_empty(), "{} sin descripción en {:?}", meta.id, lang);
                assert!(
                    desc.trim().chars().count() > 20,
                    "la descripción de {} en {:?} no explica nada: {:?}",
                    meta.id,
                    lang,
                    desc
                );
            }
        }
    }

    #[test]
    fn el_alcance_declarado_coincide_con_el_trait_que_implementa() {
        for rule in page_rules() {
            assert_eq!(rule.meta().scope, Scope::Page, "{} es una PageRule", rule.id());
        }
        for rule in site_rules() {
            assert_eq!(rule.meta().scope, Scope::Site, "{} es una SiteRule", rule.id());
        }
    }

    #[test]
    fn el_nivel_free_no_incluye_reglas_de_pago() {
        for rule in page_rules_for_tier(Tier::Free) {
            assert_eq!(rule.min_tier(), Tier::Free, "{}", rule.id());
        }
        for rule in site_rules_for_tier(Tier::Free) {
            assert_eq!(rule.min_tier(), Tier::Free, "{}", rule.id());
        }
    }

    #[test]
    fn el_nivel_pro_incluye_las_reglas_free() {
        assert!(
            page_rules_for_tier(Tier::Pro).len() >= page_rules_for_tier(Tier::Free).len(),
            "Pro tiene que ser un superconjunto de Free"
        );
    }

    #[test]
    fn los_idiomas_se_leen_de_una_cadena() {
        assert_eq!(Lang::parse("es"), Some(Lang::Es));
        assert_eq!(Lang::parse("ES-es"), Some(Lang::Es));
        assert_eq!(Lang::parse("en"), Some(Lang::En));
        assert_eq!(Lang::parse("fr"), None);
    }

    #[test]
    fn ninguna_regla_se_queda_sin_fixture() {
        // «Cada regla del catálogo necesita un fixture HTML y un test. Sin excepción — las
        // reglas son el producto.» Esto es esa frase, ejecutable.
        let sin_fixture: Vec<&str> = catalog()
            .iter()
            .filter(|m| fixture_path(m.id).is_none())
            .map(|m| m.id)
            .collect();
        assert!(
            sin_fixture.is_empty(),
            "estas reglas no tienen fixture en crates/crawlforge-rules/fixtures/: {sin_fixture:?}"
        );
    }

    #[test]
    fn no_hay_fixtures_huerfanos() {
        // Un fixture cuyo nombre no corresponde a ninguna regla es casi siempre un ID mal
        // escrito, y el test anterior no lo vería.
        let ids: HashSet<&str> = catalog().iter().map(|m| m.id).collect();
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let Ok(entradas) = std::fs::read_dir(&base) else {
            return;
        };
        let mut huerfanos = Vec::new();
        for entrada in entradas.flatten() {
            let nombre = entrada.file_name().to_string_lossy().to_string();
            let id = nombre.strip_suffix(".html").unwrap_or(&nombre).to_string();
            if !ids.contains(id.as_str()) {
                huerfanos.push(nombre);
            }
        }
        assert!(huerfanos.is_empty(), "fixtures que no corresponden a ninguna regla: {huerfanos:?}");
    }

    #[test]
    fn un_grupo_de_plantilla_se_reconoce_por_tamano_absoluto() {
        // Los medidos en rastreos reales: 645, 11.799 y 18.085 páginas son plantilla…
        assert!(is_template_group(645, 1_549));
        assert!(is_template_group(11_799, 18_134));
        assert!(is_template_group(18_085, 18_134));
        // …y las coincidencias observadas (≤7 páginas en 18.134) no lo son.
        assert!(!is_template_group(7, 18_134));
        assert!(!is_template_group(2, 18_134));
    }

    #[test]
    fn en_un_rastreo_pequeno_manda_el_porcentaje() {
        // 18 de 20 páginas es el pie de la plantilla aunque no llegue al umbral absoluto.
        assert!(is_template_group(18, 20));
        // 4 de 5 cumple el porcentaje pero no el suelo: dos o cuatro páginas no son plantilla.
        assert!(!is_template_group(4, 5));
        // 10 de 40 no cubre el sitio ni llega al umbral absoluto.
        assert!(!is_template_group(10, 40));
        // Sin páginas no hay porcentaje que valga.
        assert!(!is_template_group(10, 0));
    }

    #[test]
    fn una_regla_dominante_se_reconoce_por_su_cuota_del_sitio() {
        // Los medidos en rastreos reales: las causas sistémicas iban del 41,6% al 100%…
        assert!(is_pervasive(202_392, 216_349)); // INDEX-DEEP-PAGE, el archivo sin atajos
        assert!(is_pervasive(103_028, 216_349)); // HTTP-SLOW-RESPONSE, el servidor
        assert!(is_pervasive(848, 1_549)); // INDEX-NOINDEX, el plugin SEO
        assert!(is_pervasive(645, 1_549)); // CONTENT-HEADING-SKIP, la plantilla
        // …y las listas de páginas a arreglar una a una quedaban en el 37,6% o menos.
        assert!(!is_pervasive(61_479, 216_349)); // META-DESC-TOO-LONG, 28,4%
        assert!(!is_pervasive(213, 567)); // META-DESC-TOO-LONG, 37,6%
        assert!(!is_pervasive(1_384, 3_975)); // CONTENT-THIN, 34,8%
    }

    #[test]
    fn pocas_paginas_no_son_dominantes_por_alto_que_sea_su_porcentaje() {
        // 3 de 6 páginas no son «el 50% del sitio»: son tres páginas, y se leen de un vistazo.
        assert!(!is_pervasive(3, 6));
        assert!(!is_pervasive(19, 20));
        // El suelo exacto sí entra si cubre la cuota.
        assert!(is_pervasive(20, 40));
        // Sin páginas no hay porcentaje que valga.
        assert!(!is_pervasive(20, 0));
    }

    #[test]
    fn el_contexto_de_prueba_no_dispara_ninguna_regla_de_pagina() {
        // Si una página sana provocara hallazgos, todos los tests de todas las reglas estarían
        // midiendo ruido. Este test es el que sostiene a los demás.
        let mut ctx = PageContext::indexable_html("https://ejemplo.es/a");
        ctx.title = Some("Un título suficientemente largo para no avisar");
        ctx.title_count = 1;
        ctx.meta_description = Some(
            "Una descripción de longitud razonable, con más de setenta caracteres para no \
             disparar la regla de descripción corta.",
        );
        ctx.h1 = Some("Un encabezado");
        ctx.h1_count = 1;
        ctx.heading_levels = &[1, 2, 2];
        ctx.canonical = Some("https://ejemplo.es/a");
        ctx.canonical_raw = Some("https://ejemplo.es/a");
        ctx.canonical_count = 1;
        ctx.viewport = Some("width=device-width, initial-scale=1");
        ctx.lang = Some("es");
        ctx.og_keys = &["og:title", "og:description", "og:image"];

        let hallazgos: Vec<&str> =
            page_rules().iter().flat_map(|r| r.evaluate(&ctx)).map(|i| i.rule_id).collect();
        assert!(hallazgos.is_empty(), "una página sana no debe dar hallazgos: {hallazgos:?}");
    }
}
