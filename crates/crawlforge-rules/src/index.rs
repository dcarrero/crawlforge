//! `INDEX` — indexabilidad y rastreo. `docs/04-CATALOGO-REGLAS.md §2`.
//!
//! Es la categoría que responde a «¿por qué esta página no está en Google?», la consulta más
//! frecuente que hace un SEO. Todo lo demás del catálogo —títulos, imágenes, hreflang— solo
//! importa si la respuesta de esta sección es «sí está».
//!
//! # Reglas de §2 escritas pero **no registradas**
//!
//! El registro obliga a tener un fixture que dispare la regla al rastrearlo de verdad, y el
//! banco de fixtures (`crawlforge-core/tests/fixtures_de_reglas.rs`) rastrea siempre en modo
//! `filesystem`. Ese modo no descubre sitemaps: [`CrawlJob::filesystem`] deja
//! `discover_sitemaps = false` y `engine::run_with` solo lee sitemaps si ese campo está a `true`,
//! así que **`urls.in_sitemap` vale 0 en todas las filas de un fixture**. Cuatro reglas de esta
//! sección dependen de ese dato:
//!
//! - [`BlockedInSitemap`], [`NoindexInSitemap`], [`OrphanPage`] y [`SitemapMissing`] llevan su
//!   consulta escrita y probada contra el esquema real, pero no están en [`site_rules`] porque
//!   ningún fixture puede producir su hallazgo. Registrarlas rompería el banco de fixtures.
//! - [`RobotsBlocked`] es peor: el catálogo la declara de alcance `page` y en un rastreo normal
//!   el motor nunca entrega una página con `PageContext::blocked_by_robots = true` — cuando
//!   `robots.txt` prohíbe una URL, `engine::process_url` devuelve `Excluded(Robots)` **antes** de
//!   descargarla, así que no hay `PageContext` que evaluar. El dato sí queda en el almacén
//!   (`crawl_state='excluded'`, `exclusion_reason='robots'`), pero leerlo la convierte en una
//!   `SiteRule` y el alcance del catálogo es normativo.
//!
//!   La excepción es `--ignore-robots`: ahí la página bloqueada **sí** se descarga y desde el
//!   2026-08-04 llega marcada, de modo que `evaluate_indexability` le pone
//!   `IndexabilityReason::Robots`. Aun así la regla sigue sin registrarse, y por el motivo del
//!   párrafo anterior: ningún fixture puede producir el hallazgo, porque hace falta un
//!   `robots.txt` servido por HTTP más el flag. Registrarla rompería el banco de fixtures. Lo que
//!   el usuario de `--ignore-robots` sí ve hoy es el motivo de no indexabilidad en la fila de la
//!   página, que es donde iba a buscarlo.
//!
//! Tres reglas de §2 no están ni escritas, porque el dato que necesitan no existe en ninguna
//! parte del esquema:
//!
//! - `INDEX-ROBOTS-TXT-MISSING` y `INDEX-ROBOTS-TXT-BLOCKS-ALL` necesitan el estado de
//!   `/robots.txt` (si respondió, con qué código y con qué contenido). El motor lo descarga en
//!   `engine::load_host_rules` y lo guarda en un `RobotsCache` **en memoria**: no hay tabla ni
//!   columna donde acabe.
//! - `INDEX-SITEMAP-ERROR` necesita, por cada sitemap descargado, si el XML era válido, cuántas
//!   URLs declaraba y cuántos bytes pesaba. `engine::collect_sitemap` descarta todo eso.
//!
//! [`CrawlJob::filesystem`]: https://docs.rs/crawlforge-core

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::{Connection, OptionalExtension};

pub static INDEX_ROBOTS_TXT_MISSING: RuleMeta = RuleMeta {
    id: "INDEX-ROBOTS-TXT-MISSING",
    severity: Severity::Medium,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Sin robots.txt",
    name_en: "Missing robots.txt",
    desc_es: "El sitio no sirve /robots.txt. No impide que se le indexe —a falta de fichero se \
              rastrea todo— pero se pierde el sitio donde se anuncia el sitemap y donde se \
              excluyen las zonas que no aportan nada al buscador, como los resultados de \
              búsqueda interna o las páginas de carrito.",
    desc_en: "The site does not serve /robots.txt. It does not prevent indexing —with no file, \
              everything is crawlable— but it gives up the place where the sitemap is announced \
              and where you exclude the areas that add nothing for a search engine, such as \
              internal search results or cart pages.",
    references: &[],
};

pub static INDEX_ROBOTS_TXT_BLOCKS_ALL: RuleMeta = RuleMeta {
    id: "INDEX-ROBOTS-TXT-BLOCKS-ALL",
    severity: Severity::Critical,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "robots.txt bloquea el sitio entero",
    name_en: "robots.txt blocks the whole site",
    desc_es: "El robots.txt prohíbe rastrear la raíz del sitio, así que ninguna página puede \
              leerse ni indexarse. Es la forma más rápida y silenciosa de desaparecer de Google, \
              y casi siempre es el mismo accidente: el fichero del entorno de pruebas, que lleva \
              Disallow: /, subido a producción en un despliegue.",
    desc_en: "The robots.txt forbids crawling the site root, so no page can be read or indexed. \
              It is the fastest and quietest way to disappear from Google, and it is nearly \
              always the same accident: the staging file, which carries Disallow: /, shipped to \
              production in a deploy.",
    references: &[],
};

pub static INDEX_SITEMAP_ERROR: RuleMeta = RuleMeta {
    id: "INDEX-SITEMAP-ERROR",
    severity: Severity::High,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Sitemap con errores",
    name_en: "Sitemap with errors",
    desc_es: "Un sitemap no responde, tiene el XML mal formado o se pasa de los límites del \
              protocolo (50.000 URLs o 50 MB). El buscador deja de leerlo donde encuentra el \
              error, así que todo lo que venga detrás no llega a descubrirse por esa vía y nadie \
              avisa: el sitemap sigue ahí, aparentemente correcto.",
    desc_en: "A sitemap does not respond, has malformed XML, or exceeds the protocol limits \
              (50,000 URLs or 50 MB). Search engines stop reading at the error, so everything \
              after it is never discovered through that route, and nothing warns you: the \
              sitemap is still there, seemingly fine.",
    references: &[],
};

/// Límites del protocolo de sitemaps: 50.000 URLs y 50 MB sin comprimir.
pub const SITEMAP_MAX_URLS: i64 = 50_000;
pub const SITEMAP_MAX_BYTES: i64 = 50 * 1024 * 1024;

/// Profundidad de clic máxima admitida antes de avisar. `04-CATALOGO-REGLAS.md §2`: «> 4».
pub const MAX_CLICK_DEPTH: i64 = 4;

/// Cuántos enlaces de ejemplo se guardan en el detalle de un hallazgo de enlazado.
///
/// Un menú con cincuenta enlaces internos en `nofollow` no debe producir un `detail_json` de
/// cincuenta entradas repetido en cada página del sitio: con unos pocos ejemplos el usuario ya
/// sabe dónde mirar.
const MAX_EJEMPLOS: usize = 10;

// ---------------------------------------------------------------- Metadatos

// La severidad bajó de `critical` a `medium` el 2026-08-01, con datos de un rastreo real: 848
// hallazgos `critical` en un sitio de 1.500 páginas, el 55%, y todos eran archivos `/tag/`,
// paginaciones y `/author/` con el `follow, noindex` que pone a propósito el plugin SEO. Un
// informe cuya mitad es «crítico» y deliberado deja de leerse. Los casos donde un noindex sí es
// una emergencia conservan su severidad por otra vía: la contradicción con el sitemap es
// `INDEX-NOINDEX-IN-SITEMAP` (`critical`), y un noindex en la portada se eleva a `critical` en
// la propia evaluación, porque ahí no hay lectura benigna posible.
pub static INDEX_NOINDEX: RuleMeta = RuleMeta {
    id: "INDEX-NOINDEX",
    severity: Severity::Medium,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Con noindex",
    name_en: "Noindex",
    desc_es: "La página pide que no se indexe, por su meta robots o por la cabecera \
              X-Robots-Tag. Google la rastrea y la descarta: no aparecerá en resultados por \
              ninguna consulta. En archivos, etiquetas y páginas de sistema suele ser una \
              decisión deliberada del plugin SEO, y por eso el aviso es moderado; se eleva a \
              crítico si afecta a la portada, y el conflicto con el sitemap tiene su propia \
              regla.",
    desc_en: "The page asks not to be indexed, either through its meta robots tag or the \
              X-Robots-Tag header. Google crawls it and drops it: it will not show up for any \
              query. On archives, tags and utility pages it is usually a deliberate choice of \
              the SEO plugin, which is why the warning is moderate; it escalates to critical on \
              the home page, and the conflict with the sitemap has its own rule.",
    references: &[],
};

pub static INDEX_ROBOTS_BLOCKED: RuleMeta = RuleMeta {
    id: "INDEX-ROBOTS-BLOCKED",
    severity: Severity::Critical,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Bloqueada por robots.txt",
    name_en: "Blocked by robots.txt",
    desc_es: "El sitio enlaza esta URL desde sus propias páginas y a la vez la prohíbe en \
              robots.txt. Google no puede leerla, así que no sabe qué contiene ni sigue sus \
              enlaces: el enlace interno no lleva a ninguna parte y la autoridad que le pasa se \
              pierde. Es distinto de un noindex, que sí permite leer la página.",
    desc_en: "The site links to this URL from its own pages and at the same time forbids it in \
              robots.txt. Google cannot read it, so it does not know what it contains nor does \
              it follow its links: the internal link leads nowhere and the authority it passes \
              is lost. This is not the same as noindex, which still allows reading the page.",
    references: &[],
};

pub static INDEX_BLOCKED_IN_SITEMAP: RuleMeta = RuleMeta {
    id: "INDEX-BLOCKED-IN-SITEMAP",
    severity: Severity::Critical,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "En el sitemap y bloqueada",
    name_en: "Blocked but in sitemap",
    desc_es: "El sitemap presenta la URL como contenido que se quiere indexar y robots.txt \
              prohíbe rastrearla. Las dos instrucciones se contradicen, Search Console lo \
              reporta como error de cobertura y la URL se queda fuera del índice. Casi siempre \
              es un Disallow escrito para otra cosa que atrapó de paso a una sección publicada.",
    desc_en: "The sitemap presents the URL as content you want indexed while robots.txt forbids \
              crawling it. The two instructions contradict each other, Search Console flags it \
              as a coverage error and the URL stays out of the index. It is almost always a \
              Disallow written for something else that caught a published section along the way.",
    references: &[],
};

pub static INDEX_NOINDEX_IN_SITEMAP: RuleMeta = RuleMeta {
    id: "INDEX-NOINDEX-IN-SITEMAP",
    severity: Severity::Critical,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "En el sitemap y con noindex",
    name_en: "Noindex but in sitemap",
    desc_es: "La URL está en el sitemap, que es la lista de lo que el sitio quiere ver \
              indexado, y su propia cabecera o meta robots dice lo contrario. Una de las dos \
              cosas está mal: o sobra del sitemap, o el noindex es un resto que nadie ha \
              retirado. Mientras conviven, el sitio se contradice ante el buscador.",
    desc_en: "The URL is in the sitemap, which is the list of what the site wants indexed, and \
              its own header or meta robots says the opposite. One of the two is wrong: either \
              it does not belong in the sitemap, or the noindex is a leftover nobody removed. \
              While both coexist, the site contradicts itself in front of the search engine.",
    references: &[],
};

pub static INDEX_NOFOLLOW_INTERNAL: RuleMeta = RuleMeta {
    id: "INDEX-NOFOLLOW-INTERNAL",
    severity: Severity::Medium,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Enlace interno con nofollow",
    name_en: "Nofollow internal link",
    desc_es: "Esta página enlaza a otra del mismo sitio con rel=nofollow. El enlace no \
              transmite autoridad ni sirve para descubrir el destino, así que dentro de un \
              mismo dominio rara vez tiene sentido: el «sculpting» de PageRank dejó de \
              funcionar en 2009. Suele venir de un plugin o de una plantilla que lo pone en \
              todos los enlaces sin distinguir internos de externos.",
    desc_en: "This page links to another page on the same site with rel=nofollow. The link \
              passes no authority and does not help discover the target, so within one domain \
              it rarely makes sense: PageRank sculpting stopped working in 2009. It usually \
              comes from a plugin or template that adds it to every link without telling \
              internal from external.",
    references: &[],
};

pub static INDEX_SITEMAP_MISSING: RuleMeta = RuleMeta {
    id: "INDEX-SITEMAP-MISSING",
    severity: Severity::High,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Sin sitemap",
    name_en: "No sitemap",
    desc_es: "No se ha encontrado ningún sitemap: ni robots.txt lo anuncia, ni está en las \
              rutas habituales, ni declara ninguna URL. Sin él, el buscador solo llega a lo que \
              esté enlazado y a la velocidad que le permita el enlazado interno; y se pierde el \
              contraste entre lo que el sitio dice publicar y lo que se alcanza rastreando.",
    desc_en: "No sitemap was found: robots.txt does not announce one, it is not at the usual \
              paths, and none declares any URL. Without it the search engine only reaches what \
              is linked, at whatever pace internal linking allows; and you lose the comparison \
              between what the site claims to publish and what a crawl actually reaches.",
    references: &[],
};

pub static INDEX_ORPHAN_PAGE: RuleMeta = RuleMeta {
    id: "INDEX-ORPHAN-PAGE",
    severity: Severity::High,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Página huérfana",
    name_en: "Orphan page",
    desc_es: "El sitio declara esta URL en su sitemap pero ninguna de sus páginas la enlaza. \
              Un visitante no puede llegar a ella navegando y el buscador la ve como contenido \
              sin contexto ni autoridad interna. Es el hallazgo que aparece al cruzar lo \
              declarado con lo alcanzado, y no se puede obtener mirando una página suelta.",
    desc_en: "The site declares this URL in its sitemap but none of its pages links to it. A \
              visitor cannot reach it by browsing and the search engine sees content with no \
              context and no internal authority. It is the finding that comes from comparing \
              what is declared against what is reached, and no single page can reveal it.",
    references: &[],
};

pub static INDEX_DEEP_PAGE: RuleMeta = RuleMeta {
    id: "INDEX-DEEP-PAGE",
    severity: Severity::Medium,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Demasiados clics desde la portada",
    name_en: "Too many clicks from home",
    desc_es: "Hacen falta más de cuatro clics desde la portada para llegar a esta página. \
              Cuanta más distancia, menos autoridad interna recibe y menos a menudo la revisita \
              el buscador; en catálogos y archivos es el síntoma de una paginación sin atajos, \
              donde la página 40 solo se alcanza pasando por las 39 anteriores.",
    desc_en: "It takes more than four clicks from the home page to reach this page. The further \
              away, the less internal authority it gets and the less often the search engine \
              revisits it; in catalogues and archives it is the symptom of pagination with no \
              shortcuts, where page 40 is only reachable through the previous 39.",
    references: &[],
};

pub static INDEX_NO_INTERNAL_LINKS_IN: RuleMeta = RuleMeta {
    id: "INDEX-NO-INTERNAL-LINKS-IN",
    severity: Severity::High,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Sin enlaces internos entrantes",
    name_en: "No inbound internal links",
    desc_es: "Ninguna otra página del sitio enlaza a esta, aunque es indexable y se publica. \
              Sin enlaces entrantes no recibe autoridad interna, el buscador tarda mucho más en \
              revisitarla y sus cambios pasan desapercibidos. Es lo primero que se mira cuando \
              una página nueva no acaba de posicionar.",
    desc_en: "No other page on the site links to this one, even though it is indexable and \
              published. With no inbound links it receives no internal authority, the search \
              engine takes far longer to revisit it and its changes go unnoticed. It is the \
              first thing to check when a new page never quite ranks.",
    references: &[],
};

pub static INDEX_SECTION_DISCONNECTED: RuleMeta = RuleMeta {
    id: "INDEX-SECTION-DISCONNECTED",
    severity: Severity::High,
    category: Category::Indexability,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Sección desconectada del enlazado interno",
    name_en: "Section disconnected from internal linking",
    desc_es: "Hay un grupo de páginas que se enlazan entre sí pero al que no se llega desde la \
              portada siguiendo enlaces normales: el puente que las une al resto del sitio es \
              JavaScript, un formulario o no existe. El buscador solo las descubre por el \
              sitemap y les llega poca autoridad interna. La causa es una —falta un enlace \
              rastreable hacia la sección— y se arregla una vez, no página a página.",
    desc_en: "A group of pages link to each other but cannot be reached from the home page by \
              following regular links: the bridge joining them to the rest of the site is \
              JavaScript, a form, or missing altogether. Search engines only discover them \
              through the sitemap and little internal authority flows to them. The cause is \
              one —a crawlable link into the section is missing— and it is fixed once, not \
              page by page.",
    references: &[],
};

// ---------------------------------------------------------------- Reglas de página

/// ¿Lleva esta directiva un `noindex`?
///
/// El valor es una lista separada por comas y puede llevar prefijo de bot
/// (`googlebot: noindex`). Buscar la subcadena a secas daría un falso positivo con
/// `max-image-preview` o con cualquier palabra que contenga «index».
///
/// Duplica `crawlforge_core::job::has_noindex` a propósito: este crate no conoce al core y la
/// dirección de la dependencia es la contraria. Las dos implementaciones tienen que coincidir,
/// así que los casos límite están cubiertos con los mismos tests en los dos lados.
fn declares_noindex(directive: Option<&str>) -> bool {
    directive.is_some_and(|d| {
        d.to_ascii_lowercase()
            .split(',')
            .map(|token| token.trim().rsplit(':').next().unwrap_or("").trim())
            .any(|token| token == "noindex" || token == "none")
    })
}

/// La página pide no ser indexada.
///
/// **No se filtra por `is_indexable`**, al contrario que casi todas las reglas de página: un
/// `noindex` es precisamente lo que hace que `is_indexable` valga `false`, así que filtrar por
/// ahí dejaría la regla muerta.
///
/// Sí se exige un `200`. Un `noindex` en un 404 o en una redirección es ruido: la causa raíz de
/// que esa URL no esté en el índice es el código de estado, y hay una regla `HTTP` para eso.
/// Es el mismo orden de prioridad que aplica `evaluate_indexability` en el core.
pub struct Noindex;

impl PageRule for Noindex {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_NOINDEX
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if ctx.status != 200 {
            return Vec::new();
        }
        // El orden importa para el detalle: si las dos fuentes lo declaran, se nombra la meta,
        // que es la que el usuario puede cambiar en su plantilla.
        let fuente = [("meta_robots", ctx.meta_robots), ("x_robots_tag", ctx.x_robots_tag)]
            .into_iter()
            .find(|(_, valor)| declares_noindex(*valor));

        match fuente {
            Some((nombre, valor)) => {
                // Un noindex en la raíz del host no tiene lectura benigna: no es un archivo ni
                // una página de sistema, es el sitio pidiendo desaparecer de Google. Es el único
                // caso en que esta regla mantiene el `critical` que tenía para todo.
                let en_portada = is_host_root(ctx.url);
                let mut issue = Issue::new(&INDEX_NOINDEX).with_detail(serde_json::json!({
                    "source": nombre,
                    "value": valor.unwrap_or_default(),
                    "home_page": en_portada,
                }));
                if en_portada {
                    issue = issue.with_severity(Severity::Critical);
                }
                vec![issue]
            }
            None => Vec::new(),
        }
    }
}

/// ¿La URL es la raíz de su host (`https://ejemplo.es/`, con o sin barra, con o sin query)?
fn is_host_root(url: &str) -> bool {
    let path = &url[origin(url).len()..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    path.is_empty() || path == "/"
}

/// `scheme://host` de una URL absoluta, sin barra final. Si no parece absoluta, la devuelve
/// entera: quien la use para recortar obtiene una ruta vacía y quien la use para concatenar no
/// inventa un host.
fn origin(url: &str) -> &str {
    let Some(esquema) = url.find("://") else {
        return url;
    };
    let resto = &url[esquema + 3..];
    match resto.find('/') {
        Some(barra) => &url[..esquema + 3 + barra],
        None => url,
    }
}

/// URL prohibida en `robots.txt` a la que el propio sitio enlaza.
///
/// **Es de alcance `site`, no `page` como decía el catálogo (corregido el 2026-07-30).** No es un
/// matiz de implementación: el motor devuelve `Excluded(Robots)` *antes* de descargar la URL, que
/// es lo que significa respetar `robots.txt`, así que nunca existe un `PageContext` sobre el que
/// evaluarla. La alternativa —descargar las bloqueadas que estén enlazadas, como hace Screaming
/// Frog— cambia el comportamiento del rastreador y no se hace de tapadillo para que encaje una
/// regla.
///
/// El dato sí queda en el almacén: `crawl_state='excluded'` con `exclusion_reason='robots'`, y
/// una fila en `links` que apunta a ella. Eso es exactamente el hallazgo: el sitio gasta enlaces
/// internos en una URL que él mismo ha prohibido rastrear.
pub struct RobotsBlocked;

impl SiteRule for RobotsBlocked {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_ROBOTS_BLOCKED
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // La infraestructura del CDN queda fuera, igual que en `INDEX-NOFOLLOW-INTERNAL`:
        // Cloudflare inyecta los enlaces a `/cdn-cgi/` y los prohíbe él mismo en el robots.txt
        // que gestiona, así que «el sitio enlaza una URL que él mismo bloquea» es literalmente
        // cierto y completamente inaccionable. En un rastreo real eran los tres únicos
        // `critical` del informe. El filtro de página (`LinkView::is_infrastructure`) no llega
        // aquí porque esta regla es de sitio y lee `urls` con SQL.
        //
        // `INDEX-BLOCKED-IN-SITEMAP` no lleva este filtro a propósito: si una URL de
        // infraestructura aparece en el sitemap es porque el dueño del sitio la declaró, y
        // quitarla del sitemap sí está en su mano.
        let sql = format!(
            "SELECT u.url_hash, u.url, COUNT(DISTINCT l.from_url_id) AS inlinks
             FROM urls u
             JOIN links l ON l.to_url_id = u.id
             WHERE u.is_internal = 1
               AND u.crawl_state = 'excluded'
               AND u.exclusion_reason = 'robots'
               AND {}
             GROUP BY u.id",
            crate::sql_not_infrastructure("u.path")
        );
        let mut stmt = conn.prepare(&sql)?;
        let filas = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;

        let mut out = Vec::new();
        for fila in filas {
            let (hash, url, inlinks) = fila?;
            out.push((
                Some(hash),
                Issue::new(&INDEX_ROBOTS_BLOCKED)
                    .with_detail(serde_json::json!({ "url": url, "linked_from": inlinks })),
            ));
        }
        Ok(out)
    }
}

/// Enlace a otra página del mismo sitio con `rel=nofollow`.
///
/// Emite **un hallazgo por página, no uno por enlace**. Un `nofollow` en el menú aparece en
/// todas las páginas del sitio: con un hallazgo por enlace, un sitio de 10.000 páginas con tres
/// enlaces así en su plantilla generaría 30.000 filas en `issues` que dicen lo mismo. El
/// `detail_json` lleva la cuenta y hasta [`MAX_EJEMPLOS`] destinos para poder localizarlos.
pub struct NofollowInternal;

impl PageRule for NofollowInternal {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_NOFOLLOW_INTERNAL
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // El `is_success` corta la plantilla de error: sin él, cada 404 del sitio repetía el
        // nofollow del pie del tema como si fuera un hallazgo de esa URL. Ver
        // `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        // Solo enlaces de navegación: un `rel` en un `<img>` o en un `<script>` no existe, y
        // `is_resource` es lo que distingue lo que el usuario pulsa de lo que la página carga.
        //
        // La infraestructura del CDN queda fuera: Cloudflare reescribe los `mailto:` como
        // `/cdn-cgi/l/email-protection#…` con `rel=nofollow`, y eso llenaba el informe de un
        // aviso que nadie puso ni puede quitar —39 de 40 páginas en un sitio real—.
        let mut destinos: Vec<&str> = Vec::new();
        let mut causas: Vec<(&str, &str)> = Vec::new();
        for link in ctx.links {
            if link.is_internal && link.is_nofollow && !link.is_resource && !link.is_infrastructure
            {
                // El mismo destino enlazado dos veces es un solo defecto.
                if !destinos.contains(&link.href) {
                    destinos.push(link.href);
                }
                let causa = (link.href, link.anchor.unwrap_or("").trim());
                if !causas.contains(&causa) {
                    causas.push(causa);
                }
            }
        }
        if destinos.is_empty() {
            return Vec::new();
        }

        // El `group_key` identifica **la causa y no la página**: el conjunto de enlaces
        // ofensivos, cada uno por su destino y su ancla. El bloque de «webs amigas» del pie es
        // el mismo conjunto en las 18.089 páginas de un rastreo real, así que todas comparten
        // clave; una página que además añade un nofollow propio en su contenido es otro conjunto
        // y queda fuera del grupo, que es exactamente lo que debe pasar. El ancla entra en la
        // clave porque dos enlaces de plantilla al mismo destino con anclas distintas —el logo
        // y el enlace del pie— son dos sitios distintos que tocar en el tema.
        //
        // Se hashea porque la clave es un conjunto de URLs de longitud arbitraria, no un valor
        // legible; los destinos legibles ya van en `examples`. Mismo criterio que el
        // `title:{hash}` de META-TITLE-DUPLICATE.
        causas.sort_unstable();
        let mut huella = String::new();
        for (href, ancla) in &causas {
            huella.push_str(href);
            huella.push('\t');
            huella.push_str(ancla);
            huella.push('\n');
        }

        let ejemplos: Vec<&str> = destinos.iter().take(MAX_EJEMPLOS).copied().collect();
        vec![Issue::new(&INDEX_NOFOLLOW_INTERNAL)
            .with_detail(serde_json::json!({
                "links": destinos.len(),
                "examples": ejemplos,
            }))
            .with_group(format!(
                "nofollow:{:016x}",
                xxhash_rust::xxh3::xxh3_64(huella.as_bytes())
            ))]
    }
}

// ---------------------------------------------------------------- Reglas de conjunto

/// SQL que reconoce la portada del rastreo entre las filas de `urls`.
///
/// `crawl_meta.base_url` guarda lo que escribió el usuario al lanzar el rastreo, con barra final
/// o sin ella, mientras la URL normalizada siempre la lleva. Es la misma comparación que hace la
/// migración 003 para que la portada no salga como huérfana, y por el mismo motivo: un falso
/// positivo en la primera fila del informe hace que no se lea el resto.
const ES_LA_PORTADA: &str = "u.url IN (SELECT base_url FROM crawl_meta)
      OR u.url IN (SELECT RTRIM(base_url, '/') FROM crawl_meta)
      OR u.url IN (SELECT RTRIM(base_url, '/') || '/' FROM crawl_meta)";

/// Recoge los `url_hash` de una consulta y los convierte en hallazgos de una regla.
fn hallazgos_por_url(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    meta: &'static RuleMeta,
) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for hash in rows {
        out.push((Some(hash?), Issue::new(meta)));
    }
    Ok(out)
}

/// Modo del rastreo, tal como quedó en `crawl_meta`.
fn crawl_mode(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT mode FROM crawl_meta LIMIT 1", [], |r| r.get(0)).optional()
}

/// URL del sitemap y a la vez prohibida en `robots.txt`.
///
/// **No está registrada**: `urls.in_sitemap` vale 0 en todo fixture porque el modo `filesystem`
/// no descubre sitemaps. Ver la cabecera del módulo.
pub struct BlockedInSitemap;

impl SiteRule for BlockedInSitemap {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_BLOCKED_IN_SITEMAP
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        hallazgos_por_url(
            conn,
            "SELECT u.url_hash FROM urls u
             WHERE u.in_sitemap = 1
               AND u.crawl_state = 'excluded'
               AND u.exclusion_reason = 'robots'",
            &[],
            &INDEX_BLOCKED_IN_SITEMAP,
        )
    }
}

/// URL del sitemap que además pide no ser indexada.
///
/// **No está registrada**, por el mismo motivo que [`BlockedInSitemap`].
///
/// Se apoya en `pages.indexability_reason` y no en un `LIKE '%noindex%'` sobre `meta_robots`:
/// el motor ya resolvió ahí la meta, la cabecera y sus prefijos de bot, y repetir esa lógica en
/// SQL sería una segunda implementación que se desincronizaría con la primera.
pub struct NoindexInSitemap;

impl SiteRule for NoindexInSitemap {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_NOINDEX_IN_SITEMAP
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        hallazgos_por_url(
            conn,
            "SELECT u.url_hash FROM urls u
             JOIN pages p ON p.url_id = u.id
             WHERE u.in_sitemap = 1
               AND p.is_indexable = 0
               AND p.indexability_reason = 'noindex'",
            &[],
            &INDEX_NOINDEX_IN_SITEMAP,
        )
    }
}

/// No se encontró ningún sitemap en todo el rastreo.
///
/// **No está registrada.** La consulta es correcta para el modo `http`, pero no hay forma de
/// que un fixture la dispare: los fixtures se rastrean en modo `filesystem`, que no busca
/// sitemaps, y por eso la regla se limita al modo `http` en vez de avisar en los tres. Sin ese
/// límite, toda auditoría de un `dist/` reportaría «sin sitemap» aunque el `dist/` traiga uno,
/// que es exactamente la clase de falso positivo que corrigió la migración 003.
///
/// El dato que falta para hacerlo bien es un registro de los sitemaps consultados: qué URL, con
/// qué código respondió y cuántas URLs declaraba. Con eso, esta regla y `INDEX-SITEMAP-ERROR`
/// se implementan sin heurística.
pub struct SitemapMissing;

impl SiteRule for SitemapMissing {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_SITEMAP_MISSING
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        if crawl_mode(conn)?.as_deref() != Some("http") {
            return Ok(Vec::new());
        }
        // `config_json` es el `CrawlJob` serializado íntegro: si el usuario desactivó el
        // descubrimiento de sitemaps, no haberlos encontrado no es un hallazgo del sitio.
        let buscados: Option<i64> = conn
            .query_row(
                "SELECT json_extract(config_json, '$.discover_sitemaps') FROM crawl_meta LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if buscados != Some(1) {
            return Ok(Vec::new());
        }

        let declaradas: i64 =
            conn.query_row("SELECT COUNT(*) FROM urls WHERE in_sitemap = 1", [], |r| r.get(0))?;
        if declaradas > 0 {
            return Ok(Vec::new());
        }
        Ok(vec![(None, Issue::new(&INDEX_SITEMAP_MISSING))])
    }
}

/// URL declarada en el sitemap a la que no llega ningún enlace interno.
///
/// **No está registrada**: `urls.in_sitemap` vale 0 en todo fixture. Ver la cabecera del módulo.
///
/// Usa `v_orphans`, que ya resuelve el cruce y excluye la portada (migración 003). La otra mitad
/// de la condición del catálogo —«o en adaptador»— llegará con los adaptadores: hasta entonces nada
/// puebla `adapter_entities`, así que añadirla ahora sería código sin ejecutar.
pub struct OrphanPage;

impl SiteRule for OrphanPage {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_ORPHAN_PAGE
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        hallazgos_por_url(
            conn,
            "SELECT u.url_hash FROM urls u JOIN v_orphans o ON o.id = u.id",
            &[],
            &INDEX_ORPHAN_PAGE,
        )
    }
}

/// El CTE `home(id)` con el que arranca todo recorrido de clics: la portada más las alternativas
/// de idioma que la portada declara por `hreflang` (ver [`hreflang_seed_ids`]).
///
/// Los `seed_ids` se interpolan y no se ligan como parámetros porque son `i64` que acaban de
/// salir de la propia base: no hay entrada del usuario que escapar.
fn home_cte(seed_ids: &[i64]) -> String {
    let extra = if seed_ids.is_empty() {
        String::new()
    } else {
        let lista =
            seed_ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        format!(" UNION SELECT id FROM urls WHERE id IN ({lista})")
    };
    format!("home(id) AS (SELECT u.id FROM urls u WHERE {ES_LA_PORTADA}{extra})")
}

/// El CTE `reach(id)`: todo lo alcanzable desde `home` siguiendo enlaces `<a>`, sin límite de
/// profundidad. Es lo que separa «profunda» de «inalcanzable», que son dos diagnósticos
/// distintos con dos reglas distintas.
///
/// Mantiene el `INDEXED BY` de `shallow` y por el mismo motivo: sin él SQLite construye un
/// índice automático sobre `links` entero en RAM. Al llevar una sola columna, `UNION` deduplica
/// por nodo y el recorrido visita cada nodo alcanzado una vez: es O(enlaces) sobre el índice
/// persistente. Medido sobre los rastreos reales el 2026-08-01, en frío con `sqlite3`:
/// un sitio de 220.491 enlaces, 0,03 s; uno de 2.413.074 enlaces, 1,4 s. El plan no
/// crea ningún índice automático (verificado con EXPLAIN QUERY PLAN en ambos).
const REACH_CTE: &str = "reach(id) AS (
                     SELECT id FROM home
                     UNION
                     SELECT l.to_url_id
                     FROM links l INDEXED BY idx_links_from
                     JOIN reach r ON l.from_url_id = r.id
                     WHERE l.element = 'a'
                 )";

/// Los `urls.id` de los destinos `hreflang` de la portada, para sembrar el recorrido de clics.
///
/// **Por qué existen:** en un sitio bilingüe real, el único puente de `/es` a `/en` era el
/// `<link rel="alternate" hreflang="en">` de la cabecera —el selector visible era JavaScript— y
/// el recorrido que solo sigue `<a>` daba las 1.987 páginas inglesas por «profundas» con
/// `depth = 0`. Google sí descubre y rastrea los destinos `hreflang`, así que tratarlos como
/// puntos de entrada equivalentes a la portada es fiel a cómo se navega y a cómo se indexa; la
/// profundidad de la sección se mide entonces desde su propia portada de idioma.
///
/// Solo se leen los de la portada: es donde un sitio multilingüe declara sus raíces de idioma.
/// Sembrar con los `hreflang` de todas las páginas convertiría cada alternativa de artículo en
/// un punto de entrada y desactivaría la medición de profundidad entera.
fn hreflang_seed_ids(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let sql = format!(
        "SELECT u.url, p.hreflang_json FROM urls u
         JOIN pages p ON p.url_id = u.id
         WHERE ({ES_LA_PORTADA}) AND p.hreflang_json IS NOT NULL"
    );
    let mut stmt = conn.prepare(&sql)?;
    let filas =
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;

    let mut candidatas: Vec<String> = Vec::new();
    for fila in filas {
        let (base, json) = fila?;
        // Un JSON que no se pueda leer no aborta la regla: sin alternativas, el recorrido
        // arranca solo desde la portada, que es el comportamiento de siempre.
        let Ok(pares) = serde_json::from_str::<Vec<(String, String)>>(&json) else {
            continue;
        };
        for (_codigo, href) in pares {
            let absoluta = if href.starts_with("https://") || href.starts_with("http://") {
                href
            } else if href.starts_with('/') {
                format!("{}{href}", origin(&base))
            } else {
                // Un `hreflang` relativo sin barra inicial es rarísimo y ambiguo: mejor no
                // sembrar que sembrar mal.
                continue;
            };
            if !candidatas.contains(&absoluta) {
                candidatas.push(absoluta);
            }
        }
    }
    if candidatas.is_empty() {
        return Ok(Vec::new());
    }

    // El `hreflang` puede venir con o sin barra final y la URL normalizada del almacén también:
    // se prueban las dos formas, como hace `ES_LA_PORTADA` con la portada.
    let mut ids: Vec<i64> = Vec::new();
    let mut stmt =
        conn.prepare("SELECT id FROM urls WHERE is_internal = 1 AND url IN (?1, ?2)")?;
    for candidata in candidatas {
        let sin_barra = candidata.trim_end_matches('/').to_string();
        let con_barra = format!("{sin_barra}/");
        let filas = stmt.query_map(rusqlite::params![sin_barra, con_barra], |r| {
            r.get::<_, i64>(0)
        })?;
        for id in filas {
            ids.push(id?);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// Recorrido en anchura que deja en `temp.deep_bfs_depth (id, d)` la profundidad de clic
/// **mínima** de cada URL alcanzable desde la portada (y las semillas `hreflang`) siguiendo
/// enlaces `<a>`.
///
/// Sustituye a los dos CTE recursivos que usaba `DeepPage` (`shallow` acotado a 4 niveles más
/// el cierre completo [`REACH_CTE`]): un solo recorrido da a la vez el conjunto alcanzable, el
/// conjunto superficial **y la profundidad real de cada página**, que es lo que permite al
/// informe decir «202.392 páginas a más de 4 clics, la más profunda a 48» en una línea en vez
/// de doscientas mil. Un CTE recursivo no puede dar la profundidad mínima: con `(id, d)` en la
/// columna de recursión el `UNION` deduplica pares y el recorrido explota por caminos.
///
/// Medido sobre el rastreo real de 487.621 URLs y 26,6 millones de enlaces (2026-08-03, mismo
/// resultado: 202.392 profundas): los dos CTE anteriores 29,1 s; este recorrido 23,6 s. El
/// coste es O(enlaces) igual que el cierre, porque cada nodo entra en la frontera una vez.
///
/// Dos detalles que no son decorativos:
///
/// - `CROSS JOIN` fuerza el orden de reunión frontera→enlaces. Con `JOIN` a secas, SQLite
///   eligió recorrer `links` entero por nivel: 49 niveles × 26,6 M de filas, más de cinco
///   minutos sin terminar.
/// - `INDEXED BY` conserva la lección medida del CTE anterior: sin él SQLite construye un
///   índice automático sobre `links` entero en RAM (el pico subía de 85 a 242 MB). Las tablas
///   temporales de este recorrido miden lo que lo alcanzado (dos enteros por URL), el mismo
///   orden que ya materializaba el `UNION` del cierre.
fn click_depth_bfs(conn: &Connection, seed_ids: &[i64]) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.deep_bfs_depth;
         DROP TABLE IF EXISTS temp.deep_bfs_frontier;
         DROP TABLE IF EXISTS temp.deep_bfs_next;
         CREATE TEMP TABLE deep_bfs_depth (id INTEGER PRIMARY KEY, d INTEGER NOT NULL);
         CREATE TEMP TABLE deep_bfs_frontier (id INTEGER PRIMARY KEY);
         CREATE TEMP TABLE deep_bfs_next (id INTEGER PRIMARY KEY);",
    )?;

    let raices = format!(
        "INSERT OR IGNORE INTO deep_bfs_depth (id, d)
         SELECT u.id, 0 FROM urls u WHERE {ES_LA_PORTADA}"
    );
    conn.execute(&raices, [])?;
    if !seed_ids.is_empty() {
        // Los ids se interpolan y no se ligan como parámetros porque son `i64` que acaban de
        // salir de la propia base: no hay entrada del usuario que escapar.
        let lista = seed_ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        conn.execute(
            &format!(
                "INSERT OR IGNORE INTO deep_bfs_depth (id, d)
                 SELECT id, 0 FROM urls WHERE id IN ({lista})"
            ),
            [],
        )?;
    }
    conn.execute("INSERT INTO deep_bfs_frontier SELECT id FROM deep_bfs_depth", [])?;

    let mut expand = conn.prepare(
        "INSERT OR IGNORE INTO deep_bfs_next (id)
         SELECT l.to_url_id
         FROM deep_bfs_frontier f
         CROSS JOIN links l INDEXED BY idx_links_from ON l.from_url_id = f.id
         WHERE l.element = 'a'
           AND l.to_url_id NOT IN (SELECT id FROM deep_bfs_depth)",
    )?;
    let mut level: i64 = 0;
    // El bucle termina siempre: cada vuelta añade al menos un nodo nuevo a `deep_bfs_depth`
    // (si no añade ninguno, corta), y los nodos son finitos. Los ciclos no repiten porque lo
    // visitado queda fuera por el `NOT IN`.
    loop {
        level += 1;
        if expand.execute([])? == 0 {
            break;
        }
        conn.execute(
            "INSERT OR IGNORE INTO deep_bfs_depth (id, d) SELECT id, ?1 FROM deep_bfs_next",
            [level],
        )?;
        conn.execute("DELETE FROM deep_bfs_frontier", [])?;
        conn.execute("INSERT INTO deep_bfs_frontier SELECT id FROM deep_bfs_next", [])?;
        conn.execute("DELETE FROM deep_bfs_next", [])?;
    }
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.deep_bfs_frontier;
         DROP TABLE IF EXISTS temp.deep_bfs_next;",
    )?;
    Ok(())
}

/// Página que solo se alcanza a más de [`MAX_CLICK_DEPTH`] clics de la portada.
///
/// La profundidad se calcula aquí con un recorrido en anchura sobre `links`
/// ([`click_depth_bfs`]), y **no se lee de `urls.depth`**. El motivo es que `urls.depth` mide
/// los saltos que dio el rastreo, no los clics que da un visitante, y las dos cosas se separan
/// en cuanto el rastreo no empieza por la portada: en modo `filesystem` todos los ficheros del
/// directorio son semillas y `depth` vale 0 en todas las filas, así que una regla basada en esa
/// columna no avisaría nunca y el fixture no podría demostrarla. El recorrido, en cambio, da la
/// misma respuesta en los tres modos.
///
/// Solo se cuentan enlaces `<a>`: una página a la que únicamente apunta un `<link rel=next>` o
/// un `<script>` no se alcanza pulsando. La única excepción son los destinos `hreflang` de la
/// portada, que siembran el recorrido como raíces de idioma: ver [`hreflang_seed_ids`].
///
/// «No aparece en los cuatro primeros niveles» tiene dos causas posibles y solo una es de esta
/// regla: la página puede estar *más lejos* (profunda) o puede ser *inalcanzable* (desconectada,
/// y eso es `INDEX-SECTION-DISCONNECTED`). El recorrido las separa solo: lo inalcanzable no
/// tiene profundidad. Descubierto en un rastreo real donde 1.987 páginas «a más de cuatro
/// clics» estaban en realidad a infinitos, y el consejo de la regla —añade atajos de
/// paginación— no arreglaba nada.
///
/// El `detail_json` de cada hallazgo lleva la **profundidad real** (`click_depth`). Es lo que
/// convierte doscientas mil filas idénticas en datos: el informe puede decir la forma del
/// problema —cuántas, hasta dónde— en una línea, el XLSX se puede ordenar por profundidad, y
/// `report --rule` puede listar lo más hundido primero. La decisión anterior («el número exacto
/// no cambia lo que hay que hacer») era cierta página a página y falsa en agregado: 202.392
/// hallazgos verdaderos sin forma no se leen. Coste medido en [`click_depth_bfs`]: menor que el
/// de los dos CTE a los que sustituye.
pub struct DeepPage;

impl SiteRule for DeepPage {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_DEEP_PAGE
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // En modo `list` se audita un conjunto suelto de URLs y no se siguen enlaces: no hay
        // portada desde la que contar clics, y medirlos daría un hallazgo por cada fila.
        if crawl_mode(conn)?.as_deref() == Some("list") {
            return Ok(Vec::new());
        }

        click_depth_bfs(conn, &hreflang_seed_ids(conn)?)?;
        // Sin portada rastreada el recorrido queda vacío y no se marca nada: no hay desde
        // dónde contar. Es el `EXISTS (SELECT 1 FROM home)` de antes, ahora implícito.
        let mut stmt = conn.prepare(
            // `CROSS JOIN` desde lo alcanzado: son dos búsquedas por clave primaria por página
            // alcanzada, en vez de dejar que el planificador recorra `urls` entera.
            "SELECT u.url_hash, c.d
             FROM temp.deep_bfs_depth c
             CROSS JOIN urls u ON u.id = c.id
             CROSS JOIN pages p ON p.url_id = u.id
             WHERE c.d > ?1
               AND u.is_internal = 1
               AND p.is_indexable = 1
               -- Sin enlaces entrantes no es una página profunda, es una huérfana, y tiene su
               -- propia regla. Además protege de marcar el sitio entero cuando el rastreo no
               -- alcanzó la portada.
               AND COALESCE(p.internal_links_in, 0) > 0",
        )?;
        let rows = stmt.query_map([MAX_CLICK_DEPTH], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (hash, depth) = row?;
            out.push((
                Some(hash),
                Issue::new(&INDEX_DEEP_PAGE).with_detail(serde_json::json!({
                    "click_depth": depth,
                    "max_click_depth": MAX_CLICK_DEPTH,
                })),
            ));
        }
        conn.execute_batch("DROP TABLE IF EXISTS temp.deep_bfs_depth;")?;
        Ok(out)
    }
}

/// La forma del problema de profundidad de un rastreo ya evaluado, leída de los `detail_json`
/// que dejó [`DeepPage`].
///
/// Existe para que el informe pueda decir **una vez** lo que las filas dicen doscientas mil:
/// «202.392 páginas a más de 4 clics (profundidad típica 5–8, máxima 48)». Vive en este crate y
/// no en la CLI por la misma razón que `is_template_group`: la app de macOS y la de Windows
/// tienen que resumir exactamente igual que la CLI o el mismo fichero contaría cosas distintas
/// según dónde se abra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepPageShape {
    /// Páginas con hallazgo de profundidad (una fila por página).
    pub pages: i64,
    /// El umbral que superaron, tal como quedó escrito en el fichero.
    pub max_click_depth: i64,
    /// Banda típica: el rango intercuartílico (P25–P75) de las profundidades.
    pub typical_min: i64,
    pub typical_max: i64,
    /// La página más hundida del sitio.
    pub deepest: i64,
}

/// Lee la forma del problema de profundidad de las filas de `INDEX-DEEP-PAGE`.
///
/// Devuelve `None` si no hay hallazgos o si el fichero es anterior al `click_depth` en el
/// detalle (entonces solo hay recuento, y el informe cae a la reformulación genérica por
/// porcentaje). Agrupa por profundidad en SQL —202.392 filas se reducen a ~45 grupos— y saca
/// los cuartiles del histograma en memoria.
pub fn deep_page_shape(conn: &Connection) -> rusqlite::Result<Option<DeepPageShape>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(json_extract(detail_json, '$.click_depth') AS INTEGER) AS d,
                MAX(CAST(json_extract(detail_json, '$.max_click_depth') AS INTEGER)),
                COUNT(*)
         FROM issues
         WHERE rule_id = 'INDEX-DEEP-PAGE'
           AND url_id IS NOT NULL
           AND json_extract(detail_json, '$.click_depth') IS NOT NULL
         GROUP BY d ORDER BY d",
    )?;
    let histograma: Vec<(i64, Option<i64>, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if histograma.is_empty() {
        return Ok(None);
    }

    let pages: i64 = histograma.iter().map(|(_, _, n)| n).sum();
    let deepest = histograma.last().map(|(d, _, _)| *d).unwrap_or(0);
    let max_click_depth = histograma
        .iter()
        .filter_map(|(_, m, _)| *m)
        .max()
        .unwrap_or(MAX_CLICK_DEPTH);

    // Cuartiles sobre el histograma acumulado: la profundidad en la que cae la página que
    // ocupa el 25% y el 75% del recuento.
    let cuartil = |objetivo: i64| -> i64 {
        let mut acumulado = 0;
        for (d, _, n) in &histograma {
            acumulado += n;
            if acumulado * 4 >= objetivo * pages {
                return *d;
            }
        }
        deepest
    };
    Ok(Some(DeepPageShape {
        pages,
        max_click_depth,
        typical_min: cuartil(1),
        typical_max: cuartil(3),
        deepest,
    }))
}

/// Grupo de páginas enlazadas entre sí pero inalcanzable desde la portada siguiendo `<a>`.
///
/// **Un hallazgo de sitio, no uno por página.** La causa es una —no hay ningún enlace rastreable
/// que entre en la sección— y en el caso real que motivó la regla eran 1.987 páginas: como filas
/// individuales habrían enterrado el informe entero, y todas dirían lo mismo.
///
/// Se exige `internal_links_in > 0`: una página suelta sin ningún enlace entrante ya tiene su
/// regla (`INDEX-NO-INTERNAL-LINKS-IN`). Lo que caracteriza a la sección desconectada es lo
/// contrario: sus páginas *sí* se enlazan, pero solo entre ellas.
///
/// Comparte con [`DeepPage`] las semillas `hreflang` de la portada: una sección de idioma
/// declarada por `hreflang` no está desconectada, está enlazada por el único mecanismo que un
/// sitio multilingüe con selector JavaScript puede ofrecer al rastreador, y Google la descubre
/// por ahí. Cada regla ejecuta su propio recorrido; el cierre está medido en [`REACH_CTE`] y
/// duplicarlo cuesta décimas de segundo en la pasada final, no un estado compartido entre
/// reglas.
pub struct SectionDisconnected;

impl SiteRule for SectionDisconnected {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_SECTION_DISCONNECTED
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // Mismo motivo que en `DeepPage`: en modo `list` no se siguen enlaces y todo parecería
        // desconectado.
        if crawl_mode(conn)?.as_deref() == Some("list") {
            return Ok(Vec::new());
        }

        let home = home_cte(&hreflang_seed_ids(conn)?);
        let sql = format!(
            "WITH RECURSIVE
                 {home},
                 {REACH_CTE}
             SELECT u.url, u.path FROM urls u
             JOIN pages p ON p.url_id = u.id
             WHERE u.is_internal = 1
               AND p.is_indexable = 1
               AND COALESCE(p.internal_links_in, 0) > 0
               AND u.id NOT IN (SELECT id FROM reach)
               AND EXISTS (SELECT 1 FROM home)
             ORDER BY u.url"
        );

        let mut stmt = conn.prepare(&sql)?;
        let filas = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;

        let mut urls: Vec<String> = Vec::new();
        let mut por_prefijo: std::collections::BTreeMap<String, i64> =
            std::collections::BTreeMap::new();
        for fila in filas {
            let (url, path) = fila?;
            // El primer segmento de la ruta agrupa la sección: `/en/mundial/grupos` cuenta para
            // `/en/`. Es lo que permite al informe decir «la sección /en/» sin listar las 1.987.
            let ruta = path.unwrap_or_default();
            let primer_segmento = ruta
                .split('/')
                .find(|segmento| !segmento.is_empty())
                .map(|segmento| format!("/{segmento}/"))
                .unwrap_or_else(|| "/".to_string());
            *por_prefijo.entry(primer_segmento).or_insert(0) += 1;
            urls.push(url);
        }
        if urls.is_empty() {
            return Ok(Vec::new());
        }

        // Los prefijos más poblados primero; con tres el diagnóstico ya está contado.
        let mut prefijos: Vec<(String, i64)> = por_prefijo.into_iter().collect();
        prefijos.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let secciones: Vec<serde_json::Value> = prefijos
            .into_iter()
            .take(3)
            .map(|(prefijo, n)| serde_json::json!({ "prefix": prefijo, "pages": n }))
            .collect();
        let ejemplos: Vec<&str> =
            urls.iter().take(MAX_EJEMPLOS).map(String::as_str).collect();

        Ok(vec![(
            None,
            Issue::new(&INDEX_SECTION_DISCONNECTED)
                .with_detail(serde_json::json!({
                    "pages": urls.len(),
                    "sections": secciones,
                    "examples": ejemplos,
                }))
                .with_group("section-disconnected"),
        )])
    }
}

/// Página indexable a la que ninguna otra del sitio enlaza.
///
/// Se lee de `pages.internal_links_in`, la columna que la pasada final rellena y que la UI ya
/// muestra en `v_indexable_pages`. Recalcularla aquí con otro criterio haría que la tabla y el
/// hallazgo dijeran cosas distintas de la misma página.
///
/// La portada queda fuera: es el punto de entrada y nadie la enlaza. Es la misma exclusión que
/// la migración 003 tuvo que añadir a `v_orphans`.
///
/// Solapa con [`OrphanPage`] en las páginas que además están en el sitemap. Se ha preferido el
/// solape a la alternativa —descontar aquí las que ya reporta la otra regla— porque
/// `INDEX-ORPHAN-PAGE` no está registrada todavía y descontarlas dejaría a esas páginas sin
/// ningún hallazgo, que es el peor de los dos errores.
pub struct NoInternalLinksIn;

impl SiteRule for NoInternalLinksIn {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_NO_INTERNAL_LINKS_IN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let sql = format!(
            "SELECT u.url_hash FROM urls u
             JOIN pages p ON p.url_id = u.id
             WHERE u.is_internal = 1
               AND p.is_indexable = 1
               AND COALESCE(p.internal_links_in, 0) = 0
               AND NOT ({ES_LA_PORTADA})"
        );
        hallazgos_por_url(conn, &sql, &[], &INDEX_NO_INTERNAL_LINKS_IN)
    }
}

// ---------------------------------------------------------------- Registro

// ---------------------------------------------------------------- robots.txt y sitemaps
//
// Las tres reglas de aquí abajo leen las tablas `robots_txt` y `sitemaps`, que existen desde la
// migración 004. Antes de ella el motor descargaba los dos ficheros, los usaba y los tiraba: no
// quedaba constancia de si el robots.txt existía ni de si un sitemap tenía el XML roto, así que
// estas reglas no se podían escribir.

/// El sitio no sirve `/robots.txt`.
///
/// Solo en modo `http`. En una auditoría de un `dist/` el `robots.txt` lo sirve casi siempre el
/// alojamiento —Cloudflare, nginx, el proveedor de estáticos— y no el generador, así que su
/// ausencia en el directorio no dice nada sobre el sitio publicado.
pub struct RobotsTxtMissing;

impl SiteRule for RobotsTxtMissing {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_ROBOTS_TXT_MISSING
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        if crawl_mode(conn)?.as_deref() != Some("http") {
            return Ok(Vec::new());
        }
        let base_host: Option<String> = conn
            .query_row("SELECT host FROM urls WHERE is_internal = 1 LIMIT 1", [], |r| r.get(0))
            .optional()?;
        let Some(host) = base_host else {
            return Ok(Vec::new());
        };

        let estado: Option<Option<i64>> = conn
            .query_row("SELECT status_code FROM robots_txt WHERE host = ?1", [&host], |r| r.get(0))
            .optional()?;

        // Sin fila no se puede afirmar nada: significa que no se llegó a pedir.
        let Some(estado) = estado else {
            return Ok(Vec::new());
        };
        // Un fallo de red tampoco es una ausencia: solo un 4xx lo es.
        let Some(codigo) = estado else {
            return Ok(Vec::new());
        };
        if !(400..500).contains(&codigo) {
            return Ok(Vec::new());
        }

        Ok(vec![(
            None,
            Issue::new(&INDEX_ROBOTS_TXT_MISSING)
                .with_detail(serde_json::json!({ "host": host, "status_code": codigo })),
        )])
    }
}

/// El `robots.txt` prohíbe rastrear la raíz del sitio.
pub struct RobotsTxtBlocksAll;

impl SiteRule for RobotsTxtBlocksAll {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_ROBOTS_TXT_BLOCKS_ALL
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt =
            conn.prepare("SELECT host, content FROM robots_txt WHERE blocks_all = 1")?;
        let filas = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;

        let mut out = Vec::new();
        for fila in filas {
            let (host, contenido) = fila?;
            // El contenido se recorta: en el detalle cabe lo que explica el hallazgo, no un
            // fichero entero que puede tener cientos de líneas de reglas de terceros.
            let muestra: Option<String> = contenido.map(|c| {
                c.lines().take(20).collect::<Vec<_>>().join("\n")
            });
            out.push((
                None,
                Issue::new(&INDEX_ROBOTS_TXT_BLOCKS_ALL)
                    .with_detail(serde_json::json!({ "host": host, "robots_txt": muestra })),
            ));
        }
        Ok(out)
    }
}

/// Un sitemap no responde, no se puede leer o se pasa de los límites del protocolo.
pub struct SitemapError;

impl SiteRule for SitemapError {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_SITEMAP_ERROR
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // Un sitemap convencional que no existe **no es un error**: se prueban `/sitemap.xml` y
        // `/sitemap_index.xml` a ciegas, y que uno de los dos dé 404 es lo normal. Sí lo es que
        // falle uno anunciado en `robots.txt` o declarado por un índice: a ese le apunta alguien.
        let mut stmt = conn.prepare(
            "SELECT url, status_code, is_valid, parse_error, url_count, bytes, discovered_from
             FROM sitemaps
             WHERE (is_valid = 0 AND discovered_from <> 'well_known')
                OR (is_valid = 0 AND status_code = 200)
                OR url_count > ?1
                OR bytes > ?2",
        )?;
        let filas = stmt.query_map(rusqlite::params![SITEMAP_MAX_URLS, SITEMAP_MAX_BYTES], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;

        let mut out = Vec::new();
        for fila in filas {
            let (url, estado, valido, error, urls, bytes, origen) = fila?;
            let motivo = if urls > SITEMAP_MAX_URLS {
                "too_many_urls"
            } else if bytes > SITEMAP_MAX_BYTES {
                "too_large"
            } else if estado != Some(200) {
                "bad_status"
            } else {
                "invalid_xml"
            };
            out.push((
                None,
                Issue::new(&INDEX_SITEMAP_ERROR)
                    .with_detail(serde_json::json!({
                        "sitemap": url,
                        "reason": motivo,
                        "status_code": estado,
                        "parse_error": error,
                        "url_count": urls,
                        "bytes": bytes,
                        "discovered_from": origen,
                        "valid": valido == 1,
                    }))
                    .with_group(format!("sitemap-error:{motivo}")),
            ));
        }
        Ok(out)
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![Box::new(Noindex), Box::new(NofollowInternal)]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    // Las cuatro que dependen de `urls.in_sitemap` se registraron el 2026-07-30, cuando el modo
    // `filesystem` pasó a descubrir sitemaps: hasta entonces `in_sitemap` valía 0 en toda
    // auditoría de un `dist/` y ninguna podía producir un hallazgo.
    vec![
        Box::new(DeepPage),
        Box::new(SectionDisconnected),
        Box::new(NoInternalLinksIn),
        Box::new(BlockedInSitemap),
        Box::new(NoindexInSitemap),
        Box::new(SitemapMissing),
        Box::new(OrphanPage),
        Box::new(RobotsTxtMissing),
        Box::new(RobotsTxtBlocksAll),
        Box::new(SitemapError),
        Box::new(RobotsBlocked),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LinkView;

    /// Una página sana de la que partir. Cada test rompe solo lo que le interesa.
    fn ctx<'a>() -> PageContext<'a> {
        PageContext::indexable_html("https://ejemplo.es/a")
    }

    // --- Lectura de las directivas robots ---

    #[test]
    fn reconoce_noindex_en_una_lista_de_directivas() {
        assert!(declares_noindex(Some("noindex")));
        assert!(declares_noindex(Some("noindex, follow")));
        assert!(declares_noindex(Some("follow, NOINDEX")), "sin distinguir caja");
        assert!(declares_noindex(Some("googlebot: noindex")), "con prefijo de bot");
        assert!(declares_noindex(Some("none")), "none equivale a noindex, nofollow");
    }

    #[test]
    fn no_confunde_otras_directivas_con_noindex() {
        // La misma trampa que cubre el core: buscar la subcadena "noindex" a secas falla aquí.
        assert!(!declares_noindex(Some("index, follow")));
        assert!(!declares_noindex(Some("max-image-preview:large, max-snippet:-1")));
        assert!(!declares_noindex(Some("nofollow")), "nofollow no impide indexar");
        assert!(!declares_noindex(Some("")));
        assert!(!declares_noindex(None));
    }

    // --- INDEX-NOINDEX ---

    #[test]
    fn no_avisa_de_noindex_en_una_pagina_sin_directivas() {
        assert!(Noindex.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_del_noindex_de_la_meta_robots() {
        let mut c = ctx();
        c.meta_robots = Some("noindex, follow");
        // Una página con noindex nunca es indexable: si la regla filtrara por `is_indexable`
        // no dispararía jamás.
        c.is_indexable = false;
        let issues = Noindex.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "INDEX-NOINDEX");
        // Media, no crítica: en un sitio real el 55% de las páginas llevaban el noindex
        // deliberado del plugin SEO en /tag/, paginaciones y /author/, y un informe cuya mitad
        // es «crítico» deja de leerse. Lo crítico de verdad se conserva por otra vía: la
        // portada (test siguiente) y la contradicción con el sitemap (su propia regla).
        assert_eq!(issues[0].severity, Severity::Medium);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("meta_robots"), "el detalle dice de dónde sale: {detalle}");
    }

    #[test]
    fn el_noindex_en_la_portada_si_es_critico() {
        // Un noindex en la raíz del host no tiene lectura benigna: es el sitio pidiendo
        // desaparecer de Google, el accidente clásico del entorno de pruebas en producción.
        for portada in ["https://ejemplo.es/", "https://ejemplo.es", "https://ejemplo.es/?utm=x"]
        {
            let mut c = PageContext::indexable_html(portada);
            c.meta_robots = Some("noindex");
            c.is_indexable = false;
            let issues = Noindex.evaluate(&c);
            assert_eq!(issues.len(), 1, "con url = {portada}");
            assert_eq!(issues[0].severity, Severity::Critical, "con url = {portada}");
            let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
            assert!(detalle.contains("\"home_page\":true"), "{detalle}");
        }
    }

    #[test]
    fn una_ruta_interior_no_se_confunde_con_la_portada() {
        assert!(is_host_root("https://ejemplo.es/"));
        assert!(is_host_root("https://ejemplo.es"));
        assert!(!is_host_root("https://ejemplo.es/tag/rust/"));
        assert!(!is_host_root("https://ejemplo.es/eliminatorias/imprimir"));
    }

    #[test]
    fn avisa_del_noindex_de_la_cabecera_x_robots_tag() {
        let mut c = ctx();
        c.x_robots_tag = Some("noindex");
        c.is_indexable = false;
        let issues = Noindex.evaluate(&c);
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("x_robots_tag"), "{detalle}");
    }

    #[test]
    fn un_solo_hallazgo_aunque_las_dos_fuentes_lo_declaren() {
        let mut c = ctx();
        c.meta_robots = Some("noindex");
        c.x_robots_tag = Some("noindex");
        c.is_indexable = false;
        assert_eq!(Noindex.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_avisa_de_noindex_sobre_un_codigo_de_error() {
        // La causa raíz de que un 404 no esté en el índice es el 404, y tiene su regla HTTP.
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            c.meta_robots = Some("noindex");
            c.is_indexable = false;
            assert!(Noindex.evaluate(&c).is_empty(), "no debería avisar con un {status}");
        }
    }

    #[test]
    fn el_noindex_de_un_pdf_tambien_cuenta() {
        // `X-Robots-Tag` es la única forma de excluir un PDF, y excluirlo tiene el mismo efecto
        // que en una página: no estará en el índice. Por eso la regla no exige HTML.
        let mut c = ctx();
        c.is_html = false;
        c.is_indexable = false;
        c.content_type = Some("application/pdf");
        c.x_robots_tag = Some("noindex");
        assert_eq!(Noindex.evaluate(&c).len(), 1);
    }

    // --- INDEX-NOFOLLOW-INTERNAL ---

    fn enlace<'a>(href: &'a str, interno: bool, nofollow: bool) -> LinkView<'a> {
        LinkView {
            href,
            anchor: None,
            is_nofollow: nofollow,
            is_internal: interno,
            is_resource: false,
            is_infrastructure: false,
        }
    }

    #[test]
    fn no_avisa_de_los_enlaces_que_inyecta_el_cdn() {
        // Regresión de un falso positivo real: Cloudflare reescribe las direcciones de correo
        // como `/cdn-cgi/l/email-protection#…` con `rel=nofollow`. La regla avisaba en 39 de 40
        // páginas de un sitio por algo que el dueño del sitio no ha puesto ni puede quitar.
        let mut cdn = enlace("/cdn-cgi/l/email-protection#a1b2c3", true, true);
        cdn.is_infrastructure = true;
        let links = [cdn];
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.links = &links;
        assert!(
            NofollowInternal.evaluate(&c).is_empty(),
            "un enlace de infraestructura del CDN no es un enlace del sitio"
        );
    }

    #[test]
    fn no_avisa_cuando_los_enlaces_internos_son_normales() {
        let mut c = ctx();
        let links = [enlace("https://ejemplo.es/b", true, false)];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn no_avisa_de_un_nofollow_hacia_fuera() {
        // Un nofollow a un dominio ajeno es una decisión legítima y muy común.
        let mut c = ctx();
        let links = [enlace("https://otro.com/x", false, true)];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_de_un_enlace_interno_con_nofollow() {
        let mut c = ctx();
        let links = [enlace("https://ejemplo.es/b", true, true)];
        c.links = &links;
        let issues = NofollowInternal.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "INDEX-NOFOLLOW-INTERNAL");
        assert_eq!(issues[0].severity, Severity::Medium);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("https://ejemplo.es/b"), "{detalle}");
    }

    #[test]
    fn varios_enlaces_con_nofollow_dan_un_solo_hallazgo_con_la_cuenta() {
        // Un menú con nofollow se repite en todas las páginas: un hallazgo por enlace llenaría
        // `issues` de filas que dicen lo mismo.
        let mut c = ctx();
        let links = [
            enlace("https://ejemplo.es/b", true, true),
            enlace("https://ejemplo.es/c", true, true),
            enlace("https://ejemplo.es/b", true, true),
        ];
        c.links = &links;
        let issues = NofollowInternal.evaluate(&c);
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"links\":2"), "el destino repetido no cuenta dos veces: {detalle}");
    }

    #[test]
    fn el_detalle_no_crece_sin_limite() {
        let mut c = ctx();
        let hrefs: Vec<String> = (0..40).map(|i| format!("https://ejemplo.es/p{i}")).collect();
        let links: Vec<LinkView<'_>> =
            hrefs.iter().map(|h| enlace(h.as_str(), true, true)).collect();
        c.links = &links;
        let issues = NofollowInternal.evaluate(&c);
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"links\":40"), "la cuenta es completa: {detalle}");
        assert_eq!(detalle.matches("https://ejemplo.es/p").count(), MAX_EJEMPLOS);
    }

    #[test]
    fn dos_paginas_con_el_mismo_bloque_de_enlaces_comparten_grupo() {
        // El bloque de «webs amigas» del pie es el mismo conjunto de enlaces en las 18.089
        // páginas de un rastreo real: la clave identifica la causa, no la página.
        let links = [enlace("https://ejemplo.es/amigos", true, true)];
        let mut a = ctx();
        a.links = &links;
        let mut b = PageContext::indexable_html("https://ejemplo.es/otra");
        b.links = &links;
        let ka = NofollowInternal.evaluate(&a)[0].group_key.clone();
        let kb = NofollowInternal.evaluate(&b)[0].group_key.clone();
        assert!(ka.as_deref().is_some_and(|k| k.starts_with("nofollow:")), "{ka:?}");
        assert_eq!(ka, kb, "la misma causa en dos páginas es un solo grupo");
    }

    #[test]
    fn otro_destino_u_otra_ancla_es_otra_causa() {
        let base = [enlace("https://ejemplo.es/amigos", true, true)];
        let otro_destino = [enlace("https://ejemplo.es/patrocinado", true, true)];
        let mut con_ancla = enlace("https://ejemplo.es/amigos", true, true);
        con_ancla.anchor = Some("Webs amigas");
        let otra_ancla = [con_ancla];

        let claves: Vec<Option<String>> = [&base[..], &otro_destino[..], &otra_ancla[..]]
            .into_iter()
            .map(|links| {
                let mut c = ctx();
                c.links = links;
                NofollowInternal.evaluate(&c)[0].group_key.clone()
            })
            .collect();
        assert_ne!(claves[0], claves[1], "otro destino no es la misma plantilla");
        assert_ne!(claves[0], claves[2], "el mismo destino con otra ancla es otro enlace que tocar");
    }

    #[test]
    fn el_orden_de_los_enlaces_no_cambia_el_grupo() {
        // El hash se calcula sobre el conjunto ordenado: si el DOM baraja dos bloques, la causa
        // sigue siendo la misma.
        let ab = [
            enlace("https://ejemplo.es/a", true, true),
            enlace("https://ejemplo.es/b", true, true),
        ];
        let ba = [
            enlace("https://ejemplo.es/b", true, true),
            enlace("https://ejemplo.es/a", true, true),
        ];
        let mut c1 = ctx();
        c1.links = &ab;
        let mut c2 = ctx();
        c2.links = &ba;
        assert_eq!(
            NofollowInternal.evaluate(&c1)[0].group_key,
            NofollowInternal.evaluate(&c2)[0].group_key
        );
    }

    #[test]
    fn un_recurso_con_nofollow_no_es_un_enlace_interno() {
        let mut c = ctx();
        let mut recurso = enlace("https://ejemplo.es/a.css", true, true);
        recurso.is_resource = true;
        let links = [recurso];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn no_avisa_de_nofollow_sobre_algo_que_no_es_html() {
        let mut c = ctx();
        c.is_html = false;
        let links = [enlace("https://ejemplo.es/b", true, true)];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn el_nofollow_de_la_plantilla_de_error_no_se_audita() {
        // Regresión de un rastreo real: cada 404 del sitio repetía el nofollow del pie del tema
        // como hallazgo de la URL rota —26 filas en un sitio—, cuando lo único accionable es el
        // 404, que ya tiene su regla HTTP.
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            let links = [enlace("https://ejemplo.es/b", true, true)];
            c.links = &links;
            assert!(
                NofollowInternal.evaluate(&c).is_empty(),
                "no debería auditar el HTML de un {status}"
            );
        }
    }

    // --- INDEX-ROBOTS-BLOCKED ---
    //
    // Es una `SiteRule` desde el 2026-07-30: el motor excluye la URL antes de descargarla, así
    // que no existe un `PageContext` que evaluar. Los tests van contra el almacén, y el de verdad
    // es el fixture, que se rastrea de extremo a extremo con su `robots.txt`.

    // --- Reglas de conjunto ---

    /// Un fichero de rastreo vacío con el **esquema real**.
    ///
    /// Las migraciones se leen del core en tiempo de compilación en vez de reescribir a mano un
    /// esquema parecido: una columna mal escrita en una copia haría pasar el test y fallar el
    /// rastreo. Es solo para tests; el crate sigue sin depender del core.
    ///
    /// **Al añadir una migración al core hay que añadirla también aquí.** El coste de olvidarlo
    /// es que estos tests midan un esquema viejo, no que dejen de compilar.
    fn db() -> Connection {
        let conn = Connection::open_in_memory().expect("abrir en memoria");
        for sql in [
            include_str!("../../crawlforge-core/migrations/001_initial.sql"),
            include_str!("../../crawlforge-core/migrations/002_truncated.sql"),
            include_str!("../../crawlforge-core/migrations/003_orphans_exclude_seed.sql"),
            include_str!("../../crawlforge-core/migrations/004_robots_y_sitemaps.sql"),
            include_str!("../../crawlforge-core/migrations/005_orphans_solo_paginas.sql"),
        ] {
            conn.execute_batch(sql).expect("aplicar la migración");
        }
        conn
    }

    fn con_meta(conn: &Connection, mode: &str, base_url: &str, sitemaps: bool) {
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime)
             VALUES ('c','p','P', ?1, ?2, datetime('now'), 'done', ?3, '0', '0', 'free')",
            rusqlite::params![
                base_url,
                mode,
                format!("{{\"discover_sitemaps\":{sitemaps}}}")
            ],
        )
        .expect("insertar crawl_meta");
    }

    /// Inserta una URL rastreada con éxito y su página. Devuelve su `id`, que coincide con su
    /// `url_hash` para que los tests puedan cruzarlos a ojo.
    fn con_pagina(conn: &Connection, id: i64, url: &str, indexable: bool, in_sitemap: bool) -> i64 {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', '/', 1, ?3, 'done', 200)",
            rusqlite::params![id, url, in_sitemap as i64],
        )
        .expect("insertar url");
        conn.execute(
            "INSERT INTO pages (url_id, is_indexable, indexability_reason, internal_links_in)
             VALUES (?1, ?2, ?3, 0)",
            rusqlite::params![id, indexable as i64, (!indexable).then_some("noindex")],
        )
        .expect("insertar page");
        id
    }

    /// Inserta una URL excluida por `robots.txt`, como la deja el motor al no descargarla.
    /// El `path` va aparte porque el filtro de infraestructura de `RobotsBlocked` lee esa
    /// columna, no la URL.
    fn con_url_bloqueada_en(conn: &Connection, id: i64, url: &str, path: &str) -> i64 {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, exclusion_reason)
             VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', ?3, 1, 0, 'excluded', 'robots')",
            rusqlite::params![id, url, path],
        )
        .expect("insertar url bloqueada");
        id
    }

    fn con_url_bloqueada(conn: &Connection, id: i64, url: &str) -> i64 {
        con_url_bloqueada_en(conn, id, url, "/privado/")
    }

    #[test]
    fn avisa_de_una_url_bloqueada_a_la_que_el_sitio_enlaza() {
        let conn = db();
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let bloqueada = con_url_bloqueada(&conn, 2, "https://ejemplo.es/privado/");
        con_enlace(&conn, portada, bloqueada);

        let hallazgos = RobotsBlocked.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, Some(bloqueada), "el hallazgo va en la URL bloqueada");
    }

    #[test]
    fn la_infraestructura_del_cdn_bloqueada_por_robots_no_es_un_hallazgo() {
        // Regresión del mismo falso positivo que ya se quitó de INDEX-NOFOLLOW-INTERNAL, esta
        // vez en su versión de sitio: Cloudflare inyecta los enlaces a /cdn-cgi/ y los prohíbe
        // él mismo en el robots.txt que gestiona. Eran los tres únicos `critical` de un rastreo
        // real y el usuario no puede arreglar ninguno.
        let conn = db();
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let cdn = con_url_bloqueada_en(
            &conn,
            2,
            "https://ejemplo.es/cdn-cgi/l/email-protection",
            "/cdn-cgi/l/email-protection",
        );
        con_enlace(&conn, portada, cdn);

        assert!(
            RobotsBlocked.evaluate(&conn).expect("evaluar").is_empty(),
            "la infraestructura del CDN no es contenido del sitio"
        );
    }

    #[test]
    fn no_avisa_de_una_url_bloqueada_que_nadie_enlaza() {
        // Sin enlaces entrantes no hay nada que arreglar: el sitio no está gastando enlazado
        // interno en ella, y el `Disallow` está haciendo justo su trabajo.
        let conn = db();
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_url_bloqueada(&conn, 2, "https://ejemplo.es/privado/");

        assert!(RobotsBlocked.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn avisa_cuando_el_robots_bloquea_el_sitio_entero() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        conn.execute(
            "INSERT INTO robots_txt (host, status_code, content, blocks_all, sitemap_count)
             VALUES ('ejemplo.es', 200, 'User-agent: *\nDisallow: /', 1, 0)",
            [],
        )
        .expect("insertar robots");

        let hallazgos = RobotsTxtBlocksAll.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, None, "es un hallazgo del sitio, no de una URL");
    }

    #[test]
    fn no_avisa_cuando_el_robots_solo_bloquea_una_zona() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        conn.execute(
            "INSERT INTO robots_txt (host, status_code, content, blocks_all, sitemap_count)
             VALUES ('ejemplo.es', 200, 'User-agent: *\nDisallow: /admin/', 0, 0)",
            [],
        )
        .expect("insertar robots");

        assert!(RobotsTxtBlocksAll.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn avisa_de_un_robots_ausente_solo_en_modo_http() {
        for (modo, esperados) in [("http", 1), ("filesystem", 0)] {
            let conn = db();
            con_meta(&conn, modo, "https://ejemplo.es/", true);
            con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
            conn.execute(
                "INSERT INTO robots_txt (host, status_code, blocks_all, sitemap_count)
                 VALUES ('ejemplo.es', 404, 0, 0)",
                [],
            )
            .expect("insertar robots");

            let hallazgos = RobotsTxtMissing.evaluate(&conn).expect("evaluar");
            assert_eq!(hallazgos.len(), esperados, "modo {modo}");
        }
    }

    #[test]
    fn un_fallo_de_red_no_es_un_robots_ausente() {
        // `status_code` nulo significa que no hubo respuesta. No poder comprobarlo no es lo
        // mismo que comprobar que no existe, y afirmar lo segundo sería inventar.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        conn.execute(
            "INSERT INTO robots_txt (host, status_code, blocks_all, sitemap_count)
             VALUES ('ejemplo.es', NULL, 0, 0)",
            [],
        )
        .expect("insertar robots");

        assert!(RobotsTxtMissing.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn avisa_de_un_sitemap_con_el_xml_roto() {
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, parse_error, url_count,
                                   bytes, discovered_from)
             VALUES ('https://ejemplo.es/sitemap.xml', 200, 0, 0, 'XML mal formado', 3, 400,
                     'well_known')",
            [],
        )
        .expect("insertar sitemap");

        let hallazgos = SitemapError.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn un_sitemap_convencional_que_no_existe_no_es_un_error() {
        // Se prueban `/sitemap.xml` y `/sitemap_index.xml` a ciegas: que uno de los dos dé 404
        // es lo normal en todos los sitios del mundo y no es un hallazgo.
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap_index.xml', 404, 0, 0, 0, 0, 'well_known')",
            [],
        )
        .expect("insertar sitemap");

        assert!(SitemapError.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn un_sitemap_anunciado_que_no_responde_si_es_un_error() {
        // A este le apunta el `robots.txt`: alguien lo declaró, así que debería estar.
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap-posts.xml', 404, 0, 0, 0, 0, 'robots')",
            [],
        )
        .expect("insertar sitemap");

        let hallazgos = SitemapError.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn avisa_de_un_sitemap_que_se_pasa_de_los_limites_del_protocolo() {
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap.xml', 200, 0, 1, ?1, 1000, 'well_known')",
            [SITEMAP_MAX_URLS + 1],
        )
        .expect("insertar sitemap");

        let hallazgos = SitemapError.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn un_sitemap_correcto_no_produce_hallazgo() {
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap.xml', 200, 0, 1, 120, 4000, 'well_known')",
            [],
        )
        .expect("insertar sitemap");

        assert!(SitemapError.evaluate(&conn).expect("evaluar").is_empty());
    }

    fn con_enlace(conn: &Connection, from: i64, to: i64) {
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, is_nofollow, element)
             VALUES (?1, ?2, 0, 'a')",
            rusqlite::params![from, to],
        )
        .expect("insertar link");
    }

    /// La misma sentencia que ejecuta `engine::finalize`. Se replica para que los tests midan la
    /// columna que las reglas leen de verdad, y no una que el test haya rellenado a mano.
    fn recalcular_enlaces_entrantes(conn: &Connection) {
        conn.execute(
            "UPDATE pages SET internal_links_in = (
                 SELECT COUNT(DISTINCT l.from_url_id) FROM links l
                 WHERE l.to_url_id = pages.url_id
             )",
            [],
        )
        .expect("recalcular");
    }

    /// Cadena portada → p1 → … → pN. Devuelve los ids en orden.
    fn cadena(conn: &Connection, largo: i64) -> Vec<i64> {
        con_meta(conn, "http", "https://ejemplo.es/", true);
        let mut ids = vec![con_pagina(conn, 1, "https://ejemplo.es/", true, false)];
        for n in 1..=largo {
            let id = con_pagina(conn, n + 1, &format!("https://ejemplo.es/p{n}"), true, false);
            con_enlace(conn, ids[ids.len() - 1], id);
            ids.push(id);
        }
        recalcular_enlaces_entrantes(conn);
        ids
    }

    fn hashes(hallazgos: &[(Option<i64>, Issue)]) -> Vec<i64> {
        hallazgos.iter().filter_map(|(h, _)| *h).collect()
    }

    // --- INDEX-DEEP-PAGE ---

    #[test]
    fn no_avisa_de_profundidad_hasta_el_cuarto_clic() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluar");
        assert!(hashes(&hallazgos).is_empty(), "cuatro clics están permitidos");
    }

    #[test]
    fn avisa_a_partir_del_quinto_clic_y_solo_de_los_que_pasan() {
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 2);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluar");
        // La cadena es portada(0) → p1(1) → … → p6(6): avisan p5 y p6.
        assert_eq!(hashes(&hallazgos), vec![ids[5], ids[6]]);
        assert_eq!(hallazgos[0].1.rule_id, "INDEX-DEEP-PAGE");
        assert_eq!(hallazgos[0].1.severity, Severity::Medium);
    }

    #[test]
    fn la_profundidad_no_se_lee_de_urls_depth() {
        // Regresión de criterio: en modo `filesystem` todas las URLs son semillas y `depth`
        // vale 0 en todas, así que una regla basada en esa columna no avisaría nunca. La
        // profundidad tiene que salir del grafo de enlaces.
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute("UPDATE urls SET depth = 0", []).expect("aplanar la profundidad");
        assert_eq!(hashes(&DeepPage.evaluate(&conn).expect("evaluar")), vec![ids[5]]);
    }

    #[test]
    fn un_atajo_desde_la_portada_deja_de_ser_profunda() {
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 1);
        con_enlace(&conn, ids[0], ids[5]);
        recalcular_enlaces_entrantes(&conn);
        assert!(
            hashes(&DeepPage.evaluate(&conn).expect("evaluar")).is_empty(),
            "con un enlace desde la portada está a un clic"
        );
    }

    #[test]
    fn una_pagina_sin_enlaces_entrantes_no_se_reporta_como_profunda() {
        // Es una huérfana, y tiene su propia regla. Reportar las dos cosas de la misma URL
        // obliga al usuario a decidir cuál de los dos hallazgos leer.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_pagina(&conn, 2, "https://ejemplo.es/suelta", true, false);
        recalcular_enlaces_entrantes(&conn);
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluar")).is_empty());
    }

    #[test]
    fn sin_portada_en_el_rastreo_no_se_marca_el_sitio_entero() {
        // Si `base_url` no está entre las URLs rastreadas, el recorrido arranca vacío y todo
        // el sitio parecería inalcanzable. Callar es lo correcto: no hay desde dónde contar.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        let a = con_pagina(&conn, 1, "https://ejemplo.es/a", true, false);
        let b = con_pagina(&conn, 2, "https://ejemplo.es/b", true, false);
        con_enlace(&conn, a, b);
        recalcular_enlaces_entrantes(&conn);
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluar")).is_empty());
    }

    #[test]
    fn la_portada_sin_barra_final_se_reconoce_igual() {
        // `base_url` guarda lo que escribió el usuario; la URL normalizada siempre lleva barra.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es", true);
        let mut previa = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        for n in 1..=(MAX_CLICK_DEPTH + 1) {
            let id = con_pagina(&conn, n + 1, &format!("https://ejemplo.es/p{n}"), true, false);
            con_enlace(&conn, previa, id);
            previa = id;
        }
        recalcular_enlaces_entrantes(&conn);
        assert_eq!(hashes(&DeepPage.evaluate(&conn).expect("evaluar")).len(), 1);
    }

    #[test]
    fn una_pagina_profunda_no_indexable_no_interesa() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute("UPDATE pages SET is_indexable = 0 WHERE url_id = 6", [])
            .expect("marcar noindex");
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluar")).is_empty());
    }

    #[test]
    fn en_modo_lista_no_se_mide_la_profundidad_de_clic() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute("UPDATE crawl_meta SET mode = 'list'", []).expect("cambiar de modo");
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluar")).is_empty());
    }

    #[test]
    fn un_enlace_que_no_se_pulsa_no_acorta_la_distancia() {
        // Un `<link rel=next>` o un `<script src>` de la portada no es un clic.
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, is_nofollow, element)
             VALUES (?1, ?2, 0, 'link')",
            rusqlite::params![ids[0], ids[5]],
        )
        .expect("insertar link");
        recalcular_enlaces_entrantes(&conn);
        assert_eq!(hashes(&DeepPage.evaluate(&conn).expect("evaluar")), vec![ids[5]]);
    }

    #[test]
    fn el_detalle_lleva_la_profundidad_real_de_cada_pagina() {
        // Es lo que permite al informe decir la forma del problema en una línea —«202.392
        // páginas a más de 4 clics, la más profunda a 48»— en vez de doscientas mil filas
        // idénticas, y al XLSX ordenarse por profundidad.
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 2);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 2);
        let detalles: Vec<&str> =
            hallazgos.iter().filter_map(|(_, i)| i.detail_json.as_deref()).collect();
        assert!(detalles[0].contains("\"click_depth\":5"), "{detalles:?}");
        assert!(detalles[1].contains("\"click_depth\":6"), "{detalles:?}");
        // El umbral acompaña al dato, para que el export se explique solo.
        assert!(detalles[0].contains("\"max_click_depth\":4"), "{detalles:?}");
    }

    /// Escribe en `issues` los hallazgos como lo hace el motor, para probar la lectura.
    fn escribir_hallazgos(conn: &Connection, hallazgos: &[(Option<i64>, Issue)]) {
        for (hash, issue) in hallazgos {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category, detail_json)
                 SELECT id, ?2, ?3, ?4, ?5 FROM urls WHERE url_hash = ?1",
                rusqlite::params![
                    hash,
                    issue.rule_id,
                    issue.severity.as_str(),
                    issue.category.as_str(),
                    issue.detail_json
                ],
            )
            .expect("insertar hallazgo");
        }
    }

    #[test]
    fn la_forma_del_problema_se_lee_de_los_hallazgos_escritos() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 4);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluar");
        escribir_hallazgos(&conn, &hallazgos);

        let forma = deep_page_shape(&conn).expect("leer").expect("hay hallazgos");
        // La cadena deja páginas a 5, 6, 7 y 8 clics.
        assert_eq!(forma.pages, 4);
        assert_eq!(forma.deepest, 8);
        assert_eq!(forma.max_click_depth, MAX_CLICK_DEPTH);
        assert_eq!(forma.typical_min, 5);
        assert_eq!(forma.typical_max, 7);
    }

    #[test]
    fn un_fichero_antiguo_sin_profundidad_no_tiene_forma_que_leer() {
        // Los rastreos anteriores a este cambio guardan `{"max_click_depth":4}` a secas: la
        // lectura devuelve None y el informe cae a la reformulación genérica por porcentaje,
        // en vez de inventar profundidades.
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute(
            "INSERT INTO issues (url_id, rule_id, severity, category, detail_json)
             VALUES (6, 'INDEX-DEEP-PAGE', 'medium', 'indexability',
                     '{\"max_click_depth\":4}')",
            [],
        )
        .expect("hallazgo al estilo antiguo");
        assert_eq!(deep_page_shape(&conn).expect("leer"), None);
    }

    #[test]
    fn sin_hallazgos_de_profundidad_no_hay_forma() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        assert_eq!(deep_page_shape(&conn).expect("leer"), None);
    }

    // --- Secciones desconectadas y semillas hreflang ---
    //
    // El caso real: un sitio bilingüe cuyo único puente de /es a /en era el
    // `<link rel="alternate" hreflang>` de la cabecera —el selector visible era JavaScript—.
    // El recorrido que solo sigue `<a>` daba las 1.987 páginas inglesas por «profundas».

    /// Declara el `hreflang_json` de una página ya insertada, como lo escribe el motor.
    fn con_hreflang(conn: &Connection, id: i64, json: &str) {
        conn.execute(
            "UPDATE pages SET hreflang_json = ?2 WHERE url_id = ?1",
            rusqlite::params![id, json],
        )
        .expect("declarar hreflang");
    }

    /// Portada más una pareja /en ↔ /en/a que solo se enlaza entre sí. Devuelve los ids
    /// `[portada, en, en_a]`.
    fn con_seccion_aislada(conn: &Connection) -> Vec<i64> {
        con_meta(conn, "http", "https://ejemplo.es/", true);
        let portada = con_pagina(conn, 1, "https://ejemplo.es/", true, false);
        let en = con_pagina(conn, 2, "https://ejemplo.es/en", true, false);
        let en_a = con_pagina(conn, 3, "https://ejemplo.es/en/a", true, false);
        con_enlace(conn, en, en_a);
        con_enlace(conn, en_a, en);
        recalcular_enlaces_entrantes(conn);
        vec![portada, en, en_a]
    }

    #[test]
    fn una_seccion_inalcanzable_no_se_reporta_como_profunda() {
        // Inalcanzable no es profundo: son dos diagnósticos distintos, y el de profundidad
        // —«añade atajos de paginación»— no arregla una sección sin puente rastreable.
        let conn = db();
        con_seccion_aislada(&conn);
        assert!(
            hashes(&DeepPage.evaluate(&conn).expect("evaluar")).is_empty(),
            "las páginas sin camino desde la portada no son «demasiados clics»"
        );
    }

    #[test]
    fn una_seccion_enlazada_solo_entre_si_es_un_unico_hallazgo_de_sitio() {
        let conn = db();
        con_seccion_aislada(&conn);
        let hallazgos = SectionDisconnected.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1, "una causa, un hallazgo: no uno por página");
        assert_eq!(hallazgos[0].0, None, "es un hallazgo del sitio, no de una URL");
        assert_eq!(hallazgos[0].1.rule_id, "INDEX-SECTION-DISCONNECTED");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"pages\":2"), "las dos páginas contadas: {detalle}");
        assert!(detalle.contains("https://ejemplo.es/en"), "con ejemplos: {detalle}");
    }

    #[test]
    fn la_seccion_declarada_por_hreflang_desde_la_portada_no_esta_desconectada() {
        // El hreflang de la portada es el mecanismo legítimo con el que un sitio multilingüe
        // declara sus raíces de idioma, y Google descubre la sección por ahí. Se prueba además
        // la barra final: el hreflang dice `/en` y la URL normalizada podría llevarla.
        let conn = db();
        con_seccion_aislada(&conn);
        con_hreflang(
            &conn,
            1,
            r#"[["es","https://ejemplo.es/"],["en","https://ejemplo.es/en"]]"#,
        );
        assert!(
            SectionDisconnected.evaluate(&conn).expect("evaluar").is_empty(),
            "una raíz de idioma declarada por hreflang no es una sección desconectada"
        );
    }

    #[test]
    fn la_profundidad_de_una_seccion_de_idioma_se_mide_desde_su_portada() {
        // Con la semilla hreflang, /en es una raíz más: lo que quede a más de cuatro clics de
        // ella sí es profundo, exactamente igual que en el idioma principal.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_hreflang(&conn, 1, r#"[["en","https://ejemplo.es/en"]]"#);
        let mut previa = con_pagina(&conn, 2, "https://ejemplo.es/en", true, false);
        // /en necesita un enlace entrante para ser candidata; se lo da su primera página.
        let mut ids = vec![previa];
        for n in 1..=(MAX_CLICK_DEPTH + 1) {
            let id = con_pagina(&conn, n + 2, &format!("https://ejemplo.es/en/p{n}"), true, false);
            con_enlace(&conn, previa, id);
            ids.push(id);
            previa = id;
        }
        con_enlace(&conn, previa, ids[0]);
        recalcular_enlaces_entrantes(&conn);

        let hallazgos = hashes(&DeepPage.evaluate(&conn).expect("evaluar"));
        assert_eq!(
            hallazgos,
            vec![ids[(MAX_CLICK_DEPTH + 1) as usize]],
            "solo la última página de la cadena inglesa queda a más de cuatro clics"
        );
        assert!(
            SectionDisconnected.evaluate(&conn).expect("evaluar").is_empty(),
            "la sección entera es alcanzable vía la semilla hreflang"
        );
    }

    #[test]
    fn una_pagina_sin_enlaces_entrantes_no_es_una_seccion_desconectada() {
        // De la página suelta ya avisa INDEX-NO-INTERNAL-LINKS-IN. Lo que define a la sección
        // desconectada es lo contrario: sus páginas sí se enlazan, pero solo entre ellas.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_pagina(&conn, 2, "https://ejemplo.es/suelta", true, false);
        recalcular_enlaces_entrantes(&conn);
        assert!(SectionDisconnected.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn en_modo_lista_no_se_busca_seccion_desconectada() {
        // En modo `list` no se siguen enlaces: todo pareceria desconectado y el hallazgo seria
        // un artefacto del modo, no del sitio.
        let conn = db();
        con_seccion_aislada(&conn);
        conn.execute("UPDATE crawl_meta SET mode = 'list'", []).expect("cambiar de modo");
        assert!(SectionDisconnected.evaluate(&conn).expect("evaluar").is_empty());
    }

    // --- INDEX-NO-INTERNAL-LINKS-IN ---

    #[test]
    fn avisa_de_la_pagina_a_la_que_nadie_enlaza() {
        let conn = db();
        con_meta(&conn, "filesystem", "https://ejemplo.es/", false);
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let enlazada = con_pagina(&conn, 2, "https://ejemplo.es/enlazada", true, false);
        let aislada = con_pagina(&conn, 3, "https://ejemplo.es/aislada", true, false);
        con_enlace(&conn, portada, enlazada);
        recalcular_enlaces_entrantes(&conn);

        let hallazgos = NoInternalLinksIn.evaluate(&conn).expect("evaluar");
        assert_eq!(hashes(&hallazgos), vec![aislada], "la portada y la enlazada no cuentan");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
    }

    #[test]
    fn la_portada_nunca_se_reporta_sin_enlaces_entrantes() {
        // Es el punto de entrada: nadie la enlaza por definición. La migración 003 tuvo que
        // arreglar exactamente este falso positivo en `v_orphans`.
        for base in ["https://ejemplo.es/", "https://ejemplo.es"] {
            let conn = db();
            con_meta(&conn, "http", base, true);
            con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
            recalcular_enlaces_entrantes(&conn);
            assert!(
                hashes(&NoInternalLinksIn.evaluate(&conn).expect("evaluar")).is_empty(),
                "con base_url = {base}"
            );
        }
    }

    #[test]
    fn una_pagina_no_indexable_sin_enlaces_entrantes_no_es_un_hallazgo() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_pagina(&conn, 2, "https://ejemplo.es/gracias", false, false);
        recalcular_enlaces_entrantes(&conn);
        assert!(hashes(&NoInternalLinksIn.evaluate(&conn).expect("evaluar")).is_empty());
    }

    // --- Reglas escritas y sin registrar. El test es lo que demuestra que su SQL es válido
    // --- contra el esquema real, ya que no pueden tener fixture.

    #[test]
    fn avisa_de_una_url_del_sitemap_bloqueada_por_robots() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, exclusion_reason)
             VALUES (1, 'https://ejemplo.es/privado', 1, 'https', 'ejemplo.es', '/privado', 1, 1,
                     'excluded', 'robots'),
                    (2, 'https://ejemplo.es/otra', 2, 'https', 'ejemplo.es', '/otra', 1, 1,
                     'done', NULL)",
            [],
        )
        .expect("insertar urls");
        assert_eq!(hashes(&BlockedInSitemap.evaluate(&conn).expect("evaluar")), vec![1]);
    }

    #[test]
    fn avisa_de_una_url_del_sitemap_con_noindex() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/gracias", false, true);
        con_pagina(&conn, 2, "https://ejemplo.es/normal", true, true);
        assert_eq!(hashes(&NoindexInSitemap.evaluate(&conn).expect("evaluar")), vec![1]);
    }

    #[test]
    fn avisa_de_la_pagina_huerfana_declarada_en_el_sitemap() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, true);
        let enlazada = con_pagina(&conn, 2, "https://ejemplo.es/enlazada", true, true);
        con_pagina(&conn, 3, "https://ejemplo.es/huerfana", true, true);
        con_enlace(&conn, portada, enlazada);
        recalcular_enlaces_entrantes(&conn);
        assert_eq!(hashes(&OrphanPage.evaluate(&conn).expect("evaluar")), vec![3]);
    }

    #[test]
    fn avisa_de_que_no_hay_sitemap_y_lo_hace_a_nivel_de_sitio() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let hallazgos = SitemapMissing.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, None, "es un hallazgo del sitio, no de una URL");
    }

    #[test]
    fn no_avisa_de_falta_de_sitemap_si_se_encontro_uno() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, true);
        assert!(SitemapMissing.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn no_avisa_de_falta_de_sitemap_si_nadie_lo_busco() {
        // Ni cuando el usuario lo desactivó, ni en los modos que no lo consultan: no encontrar
        // algo que no se ha buscado no es un hallazgo.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", false);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        assert!(SitemapMissing.evaluate(&conn).expect("evaluar").is_empty());

        let conn = db();
        con_meta(&conn, "filesystem", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        assert!(SitemapMissing.evaluate(&conn).expect("evaluar").is_empty());
    }

    // --- Registro ---

    #[test]
    fn las_reglas_registradas_son_las_que_tienen_fixture() {
        // El banco de fixtures exige que toda regla del catálogo dispare la suya al rastrearla,
        // o esté declarada como excepción con su motivo. Las que no cumplen ninguna de las dos
        // cosas se quedan fuera del registro a propósito, y este test impide que alguien las
        // añada sin darse cuenta de que rompe el banco.
        //
        // Las cuatro de sitemap entraron el 2026-07-30, cuando el modo `filesystem` pasó a leer
        // el sitemap del `dist/`: hasta entonces `urls.in_sitemap` valía 0 y ninguna podía
        // producir un hallazgo. `INDEX-SITEMAP-MISSING` es la excepción declarada del grupo —solo
        // aplica en modo `http`—; las otras tres disparan con su fixture.
        //
        // Sigue fuera `INDEX-ROBOTS-BLOCKED`: ver la cabecera del módulo.
        let paginas: Vec<&str> = page_rules().iter().map(|r| r.id()).collect();
        let conjunto: Vec<&str> = site_rules().iter().map(|r| r.id()).collect();
        assert_eq!(paginas, vec!["INDEX-NOINDEX", "INDEX-NOFOLLOW-INTERNAL"]);
        assert_eq!(
            conjunto,
            vec![
                "INDEX-DEEP-PAGE",
                "INDEX-SECTION-DISCONNECTED",
                "INDEX-NO-INTERNAL-LINKS-IN",
                "INDEX-BLOCKED-IN-SITEMAP",
                "INDEX-NOINDEX-IN-SITEMAP",
                "INDEX-SITEMAP-MISSING",
                "INDEX-ORPHAN-PAGE",
                "INDEX-ROBOTS-TXT-MISSING",
                "INDEX-ROBOTS-TXT-BLOCKS-ALL",
                "INDEX-SITEMAP-ERROR",
                "INDEX-ROBOTS-BLOCKED",
            ],
            "las trece reglas de §2 están registradas: ninguna queda fuera desde la migración 004"
        );
    }
}
