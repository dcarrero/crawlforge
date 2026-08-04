//! `INDEX` — indexability and crawling. `docs/04-CATALOGO-REGLAS.md §2`.
//!
//! This is the category that answers "why is this page not on Google?", the most frequent
//! question an SEO asks. Everything else in the catalogue —titles, images, hreflang— only
//! matters if this section's answer is "it is".
//!
//! # Every §2 rule is registered, and how they got there is the useful part
//!
//! This header used to be an inventory of rules that were written but not registered. It is
//! not one any more —[`site_rules`] carries all eleven, [`page_rules`] the other two— and the
//! history is worth keeping, because it is the pattern this category keeps repeating: **a rule
//! stays unregistered while the engine cannot produce the datum it needs, and the fix belongs
//! in the engine, not in the rule.**
//!
//! What unblocked them:
//!
//! - [`BlockedInSitemap`], [`NoindexInSitemap`], [`OrphanPage`] and [`SitemapMissing`] all read
//!   `urls.in_sitemap`, which was 0 in every fixture because `filesystem` mode did not discover
//!   sitemaps. It does since 2026-07-30, and the four went in the same day.
//! - `INDEX-ROBOTS-TXT-MISSING`, `INDEX-ROBOTS-TXT-BLOCKS-ALL` and `INDEX-SITEMAP-ERROR` needed
//!   the state of `/robots.txt` and of each sitemap, which the engine downloaded, used and threw
//!   away. Migration 004 gave them the `robots_txt` and `sitemaps` tables.
//! - [`RobotsBlocked`] was catalogued `page`-scoped and could never be one: when `robots.txt`
//!   forbids a URL, `engine::process_url` returns `Excluded(Robots)` **before** downloading it,
//!   so there is no `PageContext` to evaluate. The datum lives in the store
//!   (`crawl_state='excluded'`, `exclusion_reason='robots'`), and reading it is a site-wide
//!   query. The catalogue was corrected to `Scope::Site`; the rule was not bent to fit it.
//!
//!   There is one case where a blocked page does get downloaded, and it is `--ignore-robots`.
//!   Since 2026-08-04 it arrives marked, so `evaluate_indexability` gives it
//!   `IndexabilityReason::Robots` — and since 2026-08-04 [`RobotsBlocked`] and
//!   [`BlockedInSitemap`] read **both** stores: the excluded rows of a normal crawl and the
//!   crawled-but-marked pages of an `--ignore-robots` one. Before that, the flag meant to see
//!   more silenced both rules completely, because with everything crawled no row was ever
//!   `excluded` and `pages.indexability_reason = 'robots'` had no reader.
//!
//! # The two that no fixture proves, and it is on purpose
//!
//! `INDEX-ROBOTS-TXT-MISSING` and `INDEX-SITEMAP-MISSING` are restricted to `http` mode, so the
//! fixture bank lists them in `SIN_FIXTURE_EN_FILESYSTEM` with the reason spelled out: in a
//! `dist/` the hosting serves `robots.txt`, not the generator, and the site is not published
//! yet, so warning about either on every build would be noise in a CI pipeline. Their fixtures
//! exist and are ready for the day they are served over HTTP.
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

/// Sitemap protocol limits: 50,000 URLs and 50 MB uncompressed.
pub const SITEMAP_MAX_URLS: i64 = 50_000;
pub const SITEMAP_MAX_BYTES: i64 = 50 * 1024 * 1024;

/// Maximum click depth allowed before warning. `04-CATALOGO-REGLAS.md §2`: "> 4".
pub const MAX_CLICK_DEPTH: i64 = 4;

/// How many example links are kept in the detail of a linking finding.
///
/// A menu with fifty internal `nofollow` links must not produce a fifty-entry `detail_json`
/// repeated on every page of the site: with a few examples the user already knows where to
/// look.
const MAX_EJEMPLOS: usize = 10;

// ---------------------------------------------------------------- Metadata

// Severity went down from `critical` to `medium` on 2026-08-01, with data from a real crawl:
// 848 `critical` findings on a site of 1,500 pages, 55% of the total, and every one was a
// `/tag/` archive, a pagination page or an `/author/` page with the deliberate `follow,
// noindex` the SEO plugin sets. A report where half the rows are "critical" and deliberate
// stops being read. The cases where a noindex is a genuine emergency keep their severity
// through other routes: the contradiction with the sitemap is `INDEX-NOINDEX-IN-SITEMAP`
// (`critical`), and a noindex on the home page is escalated to `critical` in the evaluation
// itself, because there is no benign reading there.
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

// ---------------------------------------------------------------- Page rules

/// Does this directive carry a `noindex`?
///
/// The value is a comma-separated list and may carry a bot prefix
/// (`googlebot: noindex`). Searching for the bare substring would give a false positive on
/// `max-image-preview` or on any word containing "index".
///
/// It duplicates `crawlforge_core::job::has_noindex` on purpose: this crate does not know the
/// core and the dependency points the other way. The two implementations have to agree, so the
/// edge cases are covered by the same tests on both sides.
fn declares_noindex(directive: Option<&str>) -> bool {
    directive.is_some_and(|d| {
        d.to_ascii_lowercase()
            .split(',')
            .map(|token| token.trim().rsplit(':').next().unwrap_or("").trim())
            .any(|token| token == "noindex" || token == "none")
    })
}

/// The page asks not to be indexed.
///
/// **It does not filter by `is_indexable`**, unlike almost every page rule: a `noindex` is
/// precisely what makes `is_indexable` false, so filtering on it would leave the rule dead.
///
/// A `200` is required, though. A `noindex` on a 404 or on a redirect is noise: the root cause
/// of that URL being out of the index is the status code, and there is an `HTTP` rule for that.
/// It is the same order of precedence `evaluate_indexability` applies in the core.
pub struct Noindex;

impl PageRule for Noindex {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_NOINDEX
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if ctx.status != 200 {
            return Vec::new();
        }
        // Order matters for the detail: if both sources declare it, the meta is named, which
        // is the one the user can change in their template.
        let fuente = [("meta_robots", ctx.meta_robots), ("x_robots_tag", ctx.x_robots_tag)]
            .into_iter()
            .find(|(_, valor)| declares_noindex(*valor));

        match fuente {
            Some((nombre, valor)) => {
                // A noindex on the host root has no benign reading: it is not an archive or a
                // utility page, it is the site asking to disappear from Google. It is the only
                // case where this rule keeps the `critical` it used to have across the board.
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

/// Is the URL the root of its host (`https://ejemplo.es/`, with or without slash, with or
/// without query)?
fn is_host_root(url: &str) -> bool {
    let path = &url[origin(url).len()..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    path.is_empty() || path == "/"
}

/// `scheme://host` of an absolute URL, without trailing slash. If it does not look absolute,
/// it is returned whole: a caller trimming with it gets an empty path and a caller
/// concatenating with it does not invent a host.
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

/// URL forbidden in `robots.txt` that the site itself links to.
///
/// **It is `site`-scoped, not `page`-scoped as the catalogue used to say (corrected
/// 2026-07-30).** This is not an implementation nuance: the engine returns `Excluded(Robots)`
/// *before* downloading the URL, which is what honouring `robots.txt` means, so a
/// `PageContext` to evaluate it on never exists. The alternative —downloading the blocked URLs
/// that are linked, as Screaming Frog does— changes the crawler's behaviour, and that is not
/// something to do on the sly just so a rule fits.
///
/// The datum does end up in the store, through one of two doors, and the rule reads both:
///
/// - **Normal crawl**: the URL is never downloaded, `crawl_state='excluded'` with
///   `exclusion_reason='robots'`, and a row in `links` pointing at it.
/// - **`--ignore-robots`**: everything gets crawled, so no row is ever `excluded` — the mark
///   moves to `pages.indexability_reason='robots'` on the downloaded page's own row.
///
/// Reading only the first silenced the rule exactly when the user asked to see more: a site
/// that links a forbidden URL was a `critical` in a normal crawl and **zero findings** with
/// the flag on. The finding is the same through either door: the site spends internal links
/// on a URL it has itself forbidden from being crawled.
pub struct RobotsBlocked;

impl SiteRule for RobotsBlocked {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_ROBOTS_BLOCKED
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // CDN infrastructure stays out, as in `INDEX-NOFOLLOW-INTERNAL`: Cloudflare injects
        // the links to `/cdn-cgi/` and forbids them itself in the robots.txt it manages, so
        // "the site links a URL it blocks itself" is literally true and completely
        // unactionable. In a real crawl they were the only three `critical` findings in the
        // report. The page filter (`LinkView::is_infrastructure`) does not reach here because
        // this rule is site-scoped and reads `urls` with SQL.
        //
        // `INDEX-BLOCKED-IN-SITEMAP` deliberately does not carry this filter: if an
        // infrastructure URL shows up in the sitemap it is because the site owner declared it,
        // and taking it out of the sitemap is within their power.
        //
        // The `GROUP BY u.id` also guarantees one finding per URL if a row ever satisfied
        // both robots conditions at once.
        let sql = format!(
            "SELECT u.url_hash, u.url, COUNT(DISTINCT l.from_url_id) AS inlinks
             FROM urls u
             JOIN links l ON l.to_url_id = u.id
             LEFT JOIN pages p ON p.url_id = u.id
             WHERE u.is_internal = 1
               AND ((u.crawl_state = 'excluded' AND u.exclusion_reason = 'robots')
                 OR p.indexability_reason = 'robots')
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

/// Link to another page of the same site with `rel=nofollow`.
///
/// Emits **one finding per page, not one per link**. A `nofollow` in the menu shows up on
/// every page of the site: with one finding per link, a 10,000-page site with three such links
/// in its template would generate 30,000 rows in `issues` that all say the same thing. The
/// `detail_json` carries the count and up to [`MAX_EJEMPLOS`] targets so they can be located.
pub struct NofollowInternal;

impl PageRule for NofollowInternal {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_NOFOLLOW_INTERNAL
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // `is_success` cuts off the error template: without it, every 404 on the site repeated
        // the nofollow of the theme's footer as if it were a finding of that URL. See
        // `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        // Navigation links only: a `rel` on an `<img>` or a `<script>` does not exist, and
        // `is_resource` is what separates what the user clicks from what the page loads.
        //
        // CDN infrastructure stays out: Cloudflare rewrites `mailto:` links as
        // `/cdn-cgi/l/email-protection#…` with `rel=nofollow`, and that filled the report with
        // a warning nobody wrote and nobody can remove —39 out of 40 pages on a real site—.
        let mut destinos: Vec<&str> = Vec::new();
        let mut causas: Vec<(&str, &str)> = Vec::new();
        for link in ctx.links {
            if link.is_internal && link.is_nofollow && !link.is_resource && !link.is_infrastructure
            {
                // The same target linked twice is a single defect.
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

        // The `group_key` identifies **the cause, not the page**: the set of offending links,
        // each one by its target and its anchor. The "friendly sites" block in the footer is
        // the same set across the 18,089 pages of a real crawl, so they all share the key; a
        // page that additionally adds its own nofollow in its content is a different set and
        // stays out of the group, which is exactly what should happen. The anchor is part of
        // the key because two template links to the same target with different anchors —the
        // logo and the footer link— are two different places to touch in the theme.
        //
        // It is hashed because the key is a set of URLs of arbitrary length, not a readable
        // value; the readable targets already go in `examples`. Same criterion as the
        // `title:{hash}` of META-TITLE-DUPLICATE.
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

// ---------------------------------------------------------------- Site-wide rules

/// SQL that recognizes the crawl's home page among the rows of `urls`.
///
/// `crawl_meta.base_url` stores whatever the user typed when launching the crawl, with or
/// without trailing slash, while the normalized URL always carries it. It is the same
/// comparison migration 003 makes so the home page does not come out as an orphan, and for the
/// same reason: a false positive in the first row of the report keeps the rest from being
/// read.
const ES_LA_PORTADA: &str = "u.url IN (SELECT base_url FROM crawl_meta)
      OR u.url IN (SELECT RTRIM(base_url, '/') FROM crawl_meta)
      OR u.url IN (SELECT RTRIM(base_url, '/') || '/' FROM crawl_meta)";

/// Collects the `url_hash`es of a query and turns them into findings of a rule.
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

/// The crawl mode, as it was left in `crawl_meta`.
fn crawl_mode(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT mode FROM crawl_meta LIMIT 1", [], |r| r.get(0)).optional()
}

/// URL in the sitemap and at the same time forbidden in `robots.txt`.
///
/// Registered since 2026-07-30. Before that it could not be: `urls.in_sitemap` was 0 in every
/// fixture because `filesystem` mode did not discover sitemaps. See the module header.
///
/// Like [`RobotsBlocked`], it reads the mark through both doors: the `excluded` row of a
/// normal crawl and the `pages.indexability_reason='robots'` of an `--ignore-robots` one —
/// the contradiction between sitemap and robots.txt is the same whichever way the crawler
/// honoured it. The `GROUP BY` keeps it at one finding per URL.
pub struct BlockedInSitemap;

impl SiteRule for BlockedInSitemap {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_BLOCKED_IN_SITEMAP
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        hallazgos_por_url(
            conn,
            "SELECT u.url_hash FROM urls u
             LEFT JOIN pages p ON p.url_id = u.id
             WHERE u.in_sitemap = 1
               AND ((u.crawl_state = 'excluded' AND u.exclusion_reason = 'robots')
                 OR p.indexability_reason = 'robots')
             GROUP BY u.id",
            &[],
            &INDEX_BLOCKED_IN_SITEMAP,
        )
    }
}

/// URL in the sitemap that also asks not to be indexed.
///
/// Registered since 2026-07-30, for the same reason as [`BlockedInSitemap`].
///
/// It relies on `pages.indexability_reason` and not on a `LIKE '%noindex%'` over
/// `meta_robots`: the engine already resolved the meta, the header and their bot prefixes
/// there, and repeating that logic in SQL would be a second implementation drifting away from
/// the first.
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

/// No sitemap was found in the whole crawl.
///
/// Registered, and **deliberately limited to `http` mode**, which is why no filesystem fixture
/// triggers it: the fixture bank lists it in `SIN_FIXTURE_EN_FILESYSTEM` with that reason. In an
/// audit of a `dist/` the site is not published yet, so "no sitemap" on every build would be
/// noise in a CI pipeline — the same class of false positive migration 003 fixed.
///
/// The missing piece to do this properly is a record of the sitemaps consulted: which URL,
/// what status it answered with and how many URLs it declared. With that, this rule and
/// `INDEX-SITEMAP-ERROR` can be implemented without heuristics.
pub struct SitemapMissing;

impl SiteRule for SitemapMissing {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_SITEMAP_MISSING
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        if crawl_mode(conn)?.as_deref() != Some("http") {
            return Ok(Vec::new());
        }
        // `config_json` is the full serialized `CrawlJob`: if the user turned sitemap
        // discovery off, not having found any is not a finding about the site.
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

/// URL declared in the sitemap that no internal link reaches.
///
/// Registered since 2026-07-30, when `filesystem` mode started discovering sitemaps and
/// `urls.in_sitemap` stopped being 0 in every fixture. See the module header.
///
/// It uses `v_orphans`, which already resolves the cross-check and excludes the home page
/// (migration 003). The other half of the catalogue's condition —"or in an adapter"— will
/// arrive with the adapters: until then nothing populates `adapter_entities`, so adding it now
/// would be code that never runs.
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

/// The `home(id)` CTE every click traversal starts from: the home page plus the language
/// alternates the home page declares via `hreflang` (see [`hreflang_seed_ids`]).
///
/// The `seed_ids` are interpolated rather than bound as parameters because they are `i64`s
/// fresh out of the database itself: there is no user input to escape.
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

/// The `reach(id)` CTE: everything reachable from `home` following `<a>` links, with no depth
/// limit. It is what separates "deep" from "unreachable", which are two different diagnoses
/// with two different rules.
///
/// It keeps the `INDEXED BY` from `shallow`, and for the same reason: without it SQLite builds
/// an automatic index over the whole of `links` in RAM. Since it carries a single column,
/// `UNION` deduplicates by node and the traversal visits each reached node once: it is
/// O(links) over the persistent index. Measured on the real crawls on 2026-08-01, cold with
/// `sqlite3`: a site with 220,491 links, 0.03 s; one with 2,413,074 links, 1.4 s. The plan
/// creates no automatic index (verified with EXPLAIN QUERY PLAN on both).
const REACH_CTE: &str = "reach(id) AS (
                     SELECT id FROM home
                     UNION
                     SELECT l.to_url_id
                     FROM links l INDEXED BY idx_links_from
                     JOIN reach r ON l.from_url_id = r.id
                     WHERE l.element = 'a'
                 )";

/// The `urls.id`s of the home page's `hreflang` targets, to seed the click traversal.
///
/// **Why they exist:** on a real bilingual site, the only bridge from `/es` to `/en` was the
/// `<link rel="alternate" hreflang="en">` in the head —the visible selector was JavaScript—
/// and the traversal that only follows `<a>` reported the 1,987 English pages as "deep" with
/// `depth = 0`. Google does discover and crawl `hreflang` targets, so treating them as entry
/// points equivalent to the home page is faithful to how the site is browsed and to how it is
/// indexed; the section's depth is then measured from its own language home page.
///
/// Only the home page's are read: that is where a multilingual site declares its language
/// roots. Seeding with the `hreflang`s of every page would turn each article's alternate into
/// an entry point and disable the depth measurement altogether.
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
        // A JSON that cannot be read does not abort the rule: with no alternates, the
        // traversal starts from the home page alone, which is the behaviour it always had.
        let Ok(pares) = serde_json::from_str::<Vec<(String, String)>>(&json) else {
            continue;
        };
        for (_codigo, href) in pares {
            let absoluta = if href.starts_with("https://") || href.starts_with("http://") {
                href
            } else if href.starts_with('/') {
                format!("{}{href}", origin(&base))
            } else {
                // A relative `hreflang` without a leading slash is vanishingly rare and
                // ambiguous: better not to seed than to seed wrong.
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

    // The `hreflang` may come with or without trailing slash and so may the normalized URL in
    // the store: both forms are tried, as `ES_LA_PORTADA` does with the home page.
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

/// Breadth-first traversal that leaves in `temp.deep_bfs_depth (id, d)` the **minimum** click
/// depth of every URL reachable from the home page (and the `hreflang` seeds) following `<a>`
/// links.
///
/// It replaces the two recursive CTEs `DeepPage` used to run (`shallow` capped at 4 levels
/// plus the full closure [`REACH_CTE`]): a single traversal yields at once the reachable set,
/// the shallow set **and each page's actual depth**, which is what lets the report say
/// "202,392 pages more than 4 clicks away, the deepest at 48" in one line instead of two
/// hundred thousand. A recursive CTE cannot yield the minimum depth: with `(id, d)` in the
/// recursion column the `UNION` deduplicates pairs and the traversal blows up along paths.
///
/// Measured on the real crawl of 487,621 URLs and 26.6 million links (2026-08-03, same
/// result: 202,392 deep pages): the two previous CTEs, 29.1 s; this traversal, 23.6 s. The
/// cost is O(links) just like the closure, because each node enters the frontier once.
///
/// Two details that are not decorative:
///
/// - `CROSS JOIN` forces the frontier→links join order. With a plain `JOIN`, SQLite chose to
///   scan the whole of `links` per level: 49 levels × 26.6 M rows, more than five minutes
///   without finishing.
/// - `INDEXED BY` keeps the measured lesson of the previous CTE: without it SQLite builds an
///   automatic index over the whole of `links` in RAM (the peak went from 85 to 242 MB). The
///   temp tables of this traversal are sized by what is reached (two integers per URL), the
///   same order of magnitude the closure's `UNION` already materialized.
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
        // The ids are interpolated rather than bound as parameters because they are `i64`s
        // fresh out of the database itself: there is no user input to escape.
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
    // The loop always terminates: each pass adds at least one new node to `deep_bfs_depth`
    // (if it adds none, it breaks), and nodes are finite. Cycles do not repeat because what
    // has been visited is kept out by the `NOT IN`.
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

/// Page only reachable more than [`MAX_CLICK_DEPTH`] clicks away from the home page.
///
/// Depth is computed here with a breadth-first traversal over `links`
/// ([`click_depth_bfs`]), and is **not read from `urls.depth`**. The reason is that
/// `urls.depth` measures the hops the crawl took, not the clicks a visitor makes, and the two
/// part ways as soon as the crawl does not start at the home page: in `filesystem` mode every
/// file in the directory is a seed and `depth` is 0 in every row, so a rule based on that
/// column would never warn and the fixture could not prove it. The traversal, instead, gives
/// the same answer in all three modes.
///
/// Only `<a>` links count: a page pointed at solely by a `<link rel=next>` or a `<script>` is
/// not reached by clicking. The one exception is the home page's `hreflang` targets, which
/// seed the traversal as language roots: see [`hreflang_seed_ids`].
///
/// "Absent from the first four levels" has two possible causes and only one belongs to this
/// rule: the page may be *further away* (deep) or it may be *unreachable* (disconnected, and
/// that is `INDEX-SECTION-DISCONNECTED`). The traversal separates them on its own: what is
/// unreachable has no depth. Discovered on a real crawl where 1,987 pages "more than four
/// clicks away" were in fact infinitely far, and the rule's advice —add pagination shortcuts—
/// fixed nothing.
///
/// Each finding's `detail_json` carries the **actual depth** (`click_depth`). It is what
/// turns two hundred thousand identical rows into data: the report can state the shape of the
/// problem —how many, how far— in one line, the XLSX can be sorted by depth, and
/// `report --rule` can list the most sunken first. The previous decision ("the exact number
/// does not change what needs doing") was true page by page and false in aggregate: 202,392
/// true findings with no shape do not get read. Cost measured in [`click_depth_bfs`]: lower
/// than that of the two CTEs it replaces.
pub struct DeepPage;

impl SiteRule for DeepPage {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_DEEP_PAGE
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // In `list` mode a loose set of URLs is audited and links are not followed: there is
        // no home page to count clicks from, and measuring them would flag every single row.
        if crawl_mode(conn)?.as_deref() == Some("list") {
            return Ok(Vec::new());
        }

        click_depth_bfs(conn, &hreflang_seed_ids(conn)?)?;
        // With no crawled home page the traversal comes out empty and nothing is flagged:
        // there is nowhere to count from. It is the old `EXISTS (SELECT 1 FROM home)`, now
        // implicit.
        let mut stmt = conn.prepare(
            // `CROSS JOIN` from what was reached: two primary-key lookups per reached page,
            // instead of letting the planner scan the whole of `urls`.
            "SELECT u.url_hash, c.d
             FROM temp.deep_bfs_depth c
             CROSS JOIN urls u ON u.id = c.id
             CROSS JOIN pages p ON p.url_id = u.id
             WHERE c.d > ?1
               AND u.is_internal = 1
               AND p.is_indexable = 1
               -- With no inbound links it is not a deep page, it is an orphan, and that has
               -- its own rule. It also protects against flagging the whole site when the
               -- crawl did not reach the home page.
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

/// The shape of an already evaluated crawl's depth problem, read from the `detail_json`s
/// [`DeepPage`] left behind.
///
/// It exists so the report can say **once** what the rows say two hundred thousand times:
/// "202,392 pages more than 4 clicks away (typical depth 5–8, deepest 48)". It lives in this
/// crate and not in the CLI for the same reason as `is_template_group`: the macOS and Windows
/// apps have to summarize exactly like the CLI does, or the same file would tell different
/// stories depending on where it is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepPageShape {
    /// Pages with a depth finding (one row per page).
    pub pages: i64,
    /// The threshold they exceeded, as it was written into the file.
    pub max_click_depth: i64,
    /// Typical band: the interquartile range (P25–P75) of the depths.
    pub typical_min: i64,
    pub typical_max: i64,
    /// The most sunken page of the site.
    pub deepest: i64,
}

/// Reads the shape of the depth problem from the `INDEX-DEEP-PAGE` rows.
///
/// Returns `None` if there are no findings or if the file predates the `click_depth` in the
/// detail (then there is only a count, and the report falls back to the generic percentage
/// rephrasing). It groups by depth in SQL —202,392 rows collapse into ~45 groups— and takes
/// the quartiles from the histogram in memory.
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

    // Quartiles over the cumulative histogram: the depth where the page sitting at 25% and at
    // 75% of the count falls.
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

/// Group of pages linked to each other but unreachable from the home page following `<a>`.
///
/// **One site finding, not one per page.** The cause is one —no crawlable link enters the
/// section— and in the real case that motivated the rule it was 1,987 pages: as individual
/// rows they would have buried the whole report, and every one would say the same thing.
///
/// `internal_links_in > 0` is required: a loose page with no inbound links at all already has
/// its rule (`INDEX-NO-INTERNAL-LINKS-IN`). What characterizes the disconnected section is
/// the opposite: its pages *are* linked, but only among themselves.
///
/// It shares the home page's `hreflang` seeds with [`DeepPage`]: a language section declared
/// via `hreflang` is not disconnected, it is linked by the only mechanism a multilingual site
/// with a JavaScript selector can offer the crawler, and Google discovers it that way. Each
/// rule runs its own traversal; the closure is measured in [`REACH_CTE`] and duplicating it
/// costs tenths of a second in the final pass, not shared state between rules.
pub struct SectionDisconnected;

impl SiteRule for SectionDisconnected {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_SECTION_DISCONNECTED
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // Same reason as in `DeepPage`: in `list` mode links are not followed and everything
        // would look disconnected.
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
            // The first path segment groups the section: `/en/mundial/grupos` counts towards
            // `/en/`. It is what lets the report say "the /en/ section" without listing all
            // 1,987 pages.
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

        // Most populated prefixes first; with three the diagnosis is already told.
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

/// Indexable page that no other page of the site links to.
///
/// It reads `pages.internal_links_in`, the column the final pass fills in and the UI already
/// shows in `v_indexable_pages`. Recomputing it here with a different criterion would make the
/// table and the finding say different things about the same page.
///
/// The home page stays out: it is the entry point and nobody links to it. It is the same
/// exclusion migration 003 had to add to `v_orphans`.
///
/// It overlaps with [`OrphanPage`] on the pages that are also in the sitemap, and the overlap
/// is deliberate. The two say different things: this one is "nothing on the site links here",
/// the other is "you declared it in the sitemap and nothing links here". The second is the
/// worse defect and deserves its own finding; suppressing this one where they meet would hide
/// the plain fact behind the qualified one.
///
/// The original reason on record was different —that `INDEX-ORPHAN-PAGE` was not registered, so
/// subtracting would leave those pages with no finding at all— and it stopped being true on
/// 2026-07-30. The overlap was re-examined then and kept on its own merits.
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

// ---------------------------------------------------------------- Registry

// ---------------------------------------------------------------- robots.txt and sitemaps
//
// The three rules below read the `robots_txt` and `sitemaps` tables, which exist since
// migration 004. Before it the engine downloaded both files, used them and threw them away:
// no trace remained of whether the robots.txt existed or of whether a sitemap had broken XML,
// so these rules could not be written.

/// The site does not serve `/robots.txt`.
///
/// `http` mode only. In an audit of a `dist/` the `robots.txt` is almost always served by the
/// hosting —Cloudflare, nginx, the static-files provider— and not by the generator, so its
/// absence from the directory says nothing about the published site.
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

        // With no row nothing can be asserted: it means it was never requested.
        let Some(estado) = estado else {
            return Ok(Vec::new());
        };
        // A network failure is not an absence either: only a 4xx is.
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

/// The `robots.txt` forbids crawling the site root.
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
            // The content is trimmed: the detail holds what explains the finding, not a whole
            // file that may carry hundreds of lines of third-party rules.
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

/// A sitemap does not respond, cannot be read, or exceeds the protocol limits.
pub struct SitemapError;

impl SiteRule for SitemapError {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_SITEMAP_ERROR
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // A well-known sitemap that does not exist is **not an error**: `/sitemap.xml` and
        // `/sitemap_index.xml` are probed blindly, and one of the two returning 404 is
        // normal. One announced in `robots.txt` or declared by an index failing is: someone
        // points at that one.
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
    // The four that depend on `urls.in_sitemap` were registered on 2026-07-30, when
    // `filesystem` mode started discovering sitemaps: until then `in_sitemap` was 0 in every
    // audit of a `dist/` and none of them could produce a finding.
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

    /// A healthy page to start from. Each test breaks only what it cares about.
    fn ctx<'a>() -> PageContext<'a> {
        PageContext::indexable_html("https://ejemplo.es/a")
    }

    // --- Reading the robots directives ---

    #[test]
    fn recognizes_noindex_within_a_directive_list() {
        assert!(declares_noindex(Some("noindex")));
        assert!(declares_noindex(Some("noindex, follow")));
        assert!(declares_noindex(Some("follow, NOINDEX")), "case-insensitive");
        assert!(declares_noindex(Some("googlebot: noindex")), "with a bot prefix");
        assert!(declares_noindex(Some("none")), "none is equivalent to noindex, nofollow");
    }

    #[test]
    fn does_not_mistake_other_directives_for_noindex() {
        // The same trap the core covers: searching for the bare substring "noindex" fails
        // here.
        assert!(!declares_noindex(Some("index, follow")));
        assert!(!declares_noindex(Some("max-image-preview:large, max-snippet:-1")));
        assert!(!declares_noindex(Some("nofollow")), "nofollow does not prevent indexing");
        assert!(!declares_noindex(Some("")));
        assert!(!declares_noindex(None));
    }

    // --- INDEX-NOINDEX ---

    #[test]
    fn does_not_flag_noindex_on_a_page_with_no_directives() {
        assert!(Noindex.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn flags_the_noindex_from_the_meta_robots_tag() {
        let mut c = ctx();
        c.meta_robots = Some("noindex, follow");
        // A page with a noindex is never indexable: if the rule filtered by `is_indexable` it
        // would never fire.
        c.is_indexable = false;
        let issues = Noindex.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "INDEX-NOINDEX");
        // Medium, not critical: on a real site 55% of the pages carried the SEO plugin's
        // deliberate noindex on /tag/, paginations and /author/, and a report where half the
        // rows are "critical" stops being read. What is genuinely critical is kept through
        // other routes: the home page (next test) and the contradiction with the sitemap (its
        // own rule).
        assert_eq!(issues[0].severity, Severity::Medium);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("meta_robots"), "the detail says where it comes from: {detalle}");
    }

    #[test]
    fn a_noindex_on_the_home_page_is_critical() {
        // A noindex on the host root has no benign reading: it is the site asking to
        // disappear from Google, the classic accident of the staging environment shipped to
        // production.
        for portada in ["https://ejemplo.es/", "https://ejemplo.es", "https://ejemplo.es/?utm=x"]
        {
            let mut c = PageContext::indexable_html(portada);
            c.meta_robots = Some("noindex");
            c.is_indexable = false;
            let issues = Noindex.evaluate(&c);
            assert_eq!(issues.len(), 1, "with url = {portada}");
            assert_eq!(issues[0].severity, Severity::Critical, "with url = {portada}");
            let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
            assert!(detalle.contains("\"home_page\":true"), "{detalle}");
        }
    }

    #[test]
    fn an_inner_path_is_not_mistaken_for_the_home_page() {
        assert!(is_host_root("https://ejemplo.es/"));
        assert!(is_host_root("https://ejemplo.es"));
        assert!(!is_host_root("https://ejemplo.es/tag/rust/"));
        assert!(!is_host_root("https://ejemplo.es/eliminatorias/imprimir"));
    }

    #[test]
    fn flags_the_noindex_from_the_x_robots_tag_header() {
        let mut c = ctx();
        c.x_robots_tag = Some("noindex");
        c.is_indexable = false;
        let issues = Noindex.evaluate(&c);
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("x_robots_tag"), "{detalle}");
    }

    #[test]
    fn a_single_finding_even_when_both_sources_declare_it() {
        let mut c = ctx();
        c.meta_robots = Some("noindex");
        c.x_robots_tag = Some("noindex");
        c.is_indexable = false;
        assert_eq!(Noindex.evaluate(&c).len(), 1);
    }

    #[test]
    fn does_not_flag_noindex_on_an_error_status() {
        // The root cause of a 404 being out of the index is the 404, and it has its HTTP
        // rule.
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            c.meta_robots = Some("noindex");
            c.is_indexable = false;
            assert!(Noindex.evaluate(&c).is_empty(), "should not warn on a {status}");
        }
    }

    #[test]
    fn a_noindex_on_a_pdf_counts_too() {
        // `X-Robots-Tag` is the only way to exclude a PDF, and excluding it has the same
        // effect as on a page: it will not be in the index. That is why the rule does not
        // require HTML.
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
    fn does_not_flag_links_injected_by_the_cdn() {
        // Regression of a real false positive: Cloudflare rewrites email addresses as
        // `/cdn-cgi/l/email-protection#…` with `rel=nofollow`. The rule was warning on 39 out
        // of 40 pages of a site about something the site owner did not write and cannot
        // remove.
        let mut cdn = enlace("/cdn-cgi/l/email-protection#a1b2c3", true, true);
        cdn.is_infrastructure = true;
        let links = [cdn];
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.links = &links;
        assert!(
            NofollowInternal.evaluate(&c).is_empty(),
            "a CDN infrastructure link is not a site link"
        );
    }

    #[test]
    fn does_not_flag_ordinary_internal_links() {
        let mut c = ctx();
        let links = [enlace("https://ejemplo.es/b", true, false)];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn does_not_flag_a_nofollow_pointing_outward() {
        // A nofollow to a foreign domain is a legitimate and very common decision.
        let mut c = ctx();
        let links = [enlace("https://otro.com/x", false, true)];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn flags_an_internal_link_with_nofollow() {
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
    fn several_nofollow_links_produce_one_finding_with_the_count() {
        // A nofollow menu repeats on every page of the site: one finding per link would fill
        // `issues` with rows that say the same thing.
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
        assert!(detalle.contains("\"links\":2"), "the repeated target does not count twice: {detalle}");
    }

    #[test]
    fn the_detail_does_not_grow_without_bound() {
        let mut c = ctx();
        let hrefs: Vec<String> = (0..40).map(|i| format!("https://ejemplo.es/p{i}")).collect();
        let links: Vec<LinkView<'_>> =
            hrefs.iter().map(|h| enlace(h.as_str(), true, true)).collect();
        c.links = &links;
        let issues = NofollowInternal.evaluate(&c);
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"links\":40"), "the count is complete: {detalle}");
        assert_eq!(detalle.matches("https://ejemplo.es/p").count(), MAX_EJEMPLOS);
    }

    #[test]
    fn two_pages_with_the_same_link_block_share_a_group() {
        // The "friendly sites" block in the footer is the same set of links across the 18,089
        // pages of a real crawl: the key identifies the cause, not the page.
        let links = [enlace("https://ejemplo.es/amigos", true, true)];
        let mut a = ctx();
        a.links = &links;
        let mut b = PageContext::indexable_html("https://ejemplo.es/otra");
        b.links = &links;
        let ka = NofollowInternal.evaluate(&a)[0].group_key.clone();
        let kb = NofollowInternal.evaluate(&b)[0].group_key.clone();
        assert!(ka.as_deref().is_some_and(|k| k.starts_with("nofollow:")), "{ka:?}");
        assert_eq!(ka, kb, "the same cause on two pages is a single group");
    }

    #[test]
    fn a_different_target_or_anchor_is_a_different_cause() {
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
        assert_ne!(claves[0], claves[1], "another target is not the same template");
        assert_ne!(claves[0], claves[2], "the same target with another anchor is another link to fix");
    }

    #[test]
    fn link_order_does_not_change_the_group() {
        // The hash is computed over the sorted set: if the DOM shuffles two blocks, the cause
        // is still the same.
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
    fn a_nofollow_resource_is_not_an_internal_link() {
        let mut c = ctx();
        let mut recurso = enlace("https://ejemplo.es/a.css", true, true);
        recurso.is_resource = true;
        let links = [recurso];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn does_not_flag_nofollow_on_something_that_is_not_html() {
        let mut c = ctx();
        c.is_html = false;
        let links = [enlace("https://ejemplo.es/b", true, true)];
        c.links = &links;
        assert!(NofollowInternal.evaluate(&c).is_empty());
    }

    #[test]
    fn the_error_template_nofollow_is_not_audited() {
        // Regression from a real crawl: every 404 on the site repeated the theme footer's
        // nofollow as a finding of the broken URL —26 rows on one site— when the only
        // actionable thing is the 404, which already has its HTTP rule.
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            let links = [enlace("https://ejemplo.es/b", true, true)];
            c.links = &links;
            assert!(
                NofollowInternal.evaluate(&c).is_empty(),
                "should not audit the HTML of a {status}"
            );
        }
    }

    // --- INDEX-ROBOTS-BLOCKED ---
    //
    // It has been a `SiteRule` since 2026-07-30: the engine excludes the URL before
    // downloading it, so there is no `PageContext` to evaluate. The tests go against the
    // store, and the real one is the fixture, which is crawled end to end with its
    // `robots.txt`.

    // --- Site-wide rules ---

    /// An empty crawl file with the **real schema**: every published migration, from the
    /// shared helper in `test_schema.rs`, whose guard test keeps it in sync with the
    /// `migrations/` directory. This module once carried its own list and it stopped at 005.
    fn db() -> Connection {
        crate::test_schema::full_schema()
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
        .expect("insert crawl_meta");
    }

    /// Inserts a successfully crawled URL and its page. Returns its `id`, which matches its
    /// `url_hash` so tests can cross-reference them by eye.
    fn con_pagina(conn: &Connection, id: i64, url: &str, indexable: bool, in_sitemap: bool) -> i64 {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', '/', 1, ?3, 'done', 200)",
            rusqlite::params![id, url, in_sitemap as i64],
        )
        .expect("insert url");
        conn.execute(
            "INSERT INTO pages (url_id, is_indexable, indexability_reason, internal_links_in)
             VALUES (?1, ?2, ?3, 0)",
            rusqlite::params![id, indexable as i64, (!indexable).then_some("noindex")],
        )
        .expect("insert page");
        id
    }

    /// Inserts a URL excluded by `robots.txt`, as the engine leaves it without downloading
    /// it. The `path` goes separately because the infrastructure filter in `RobotsBlocked`
    /// reads that column, not the URL.
    fn con_url_bloqueada_en(conn: &Connection, id: i64, url: &str, path: &str) -> i64 {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, exclusion_reason)
             VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', ?3, 1, 0, 'excluded', 'robots')",
            rusqlite::params![id, url, path],
        )
        .expect("insert blocked url");
        id
    }

    fn con_url_bloqueada(conn: &Connection, id: i64, url: &str) -> i64 {
        con_url_bloqueada_en(conn, id, url, "/privado/")
    }

    #[test]
    fn flags_a_blocked_url_the_site_links_to() {
        let conn = db();
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let bloqueada = con_url_bloqueada(&conn, 2, "https://ejemplo.es/privado/");
        con_enlace(&conn, portada, bloqueada);

        let hallazgos = RobotsBlocked.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, Some(bloqueada), "the finding goes on the blocked URL");
    }

    #[test]
    fn cdn_infrastructure_blocked_by_robots_is_not_a_finding() {
        // Regression of the same false positive already removed from
        // INDEX-NOFOLLOW-INTERNAL, this time in its site-level version: Cloudflare injects
        // the links to /cdn-cgi/ and forbids them itself in the robots.txt it manages. They
        // were the only three `critical` findings of a real crawl and the user cannot fix any
        // of them.
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
            RobotsBlocked.evaluate(&conn).expect("evaluate").is_empty(),
            "CDN infrastructure is not site content"
        );
    }

    #[test]
    fn does_not_flag_a_blocked_url_nobody_links_to() {
        // With no inbound links there is nothing to fix: the site is not spending internal
        // linking on it, and the `Disallow` is doing exactly its job.
        let conn = db();
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_url_bloqueada(&conn, 2, "https://ejemplo.es/privado/");

        assert!(RobotsBlocked.evaluate(&conn).expect("evaluate").is_empty());
    }

    /// Inserts a URL crawled despite `robots.txt`, as `--ignore-robots` leaves it: downloaded
    /// (`crawl_state='done'`), with its page marked `indexability_reason='robots'` by
    /// `evaluate_indexability`. No row is ever `excluded` under that flag.
    fn con_pagina_rastreada_pese_a_robots(conn: &Connection, id: i64, url: &str, path: &str) -> i64 {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', ?3, 1, 0, 'done', 200)",
            rusqlite::params![id, url, path],
        )
        .expect("insert crawled url");
        conn.execute(
            "INSERT INTO pages (url_id, is_indexable, indexability_reason, internal_links_in)
             VALUES (?1, 0, 'robots', 0)",
            rusqlite::params![id],
        )
        .expect("insert page");
        id
    }

    #[test]
    fn flags_a_blocked_url_crawled_with_ignore_robots() {
        // Proves the fix. With `--ignore-robots` everything gets crawled, so no row is ever
        // `excluded` and the mark lives in `pages.indexability_reason` instead. Before the
        // rule read that second door, the flag meant to see more produced **zero** findings
        // about exactly what it exists to show.
        let conn = db();
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let bloqueada = con_pagina_rastreada_pese_a_robots(
            &conn,
            2,
            "https://ejemplo.es/privado/",
            "/privado/",
        );
        con_enlace(&conn, portada, bloqueada);

        let hallazgos = RobotsBlocked.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, Some(bloqueada), "the finding goes on the blocked URL");
    }

    #[test]
    fn an_unlinked_blocked_page_under_ignore_robots_is_not_flagged() {
        // Same criterion as the excluded door: with no inbound links, the Disallow is doing
        // its job and there is nothing to fix.
        let conn = db();
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_pagina_rastreada_pese_a_robots(&conn, 2, "https://ejemplo.es/privado/", "/privado/");

        assert!(RobotsBlocked.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn cdn_infrastructure_is_not_flagged_through_the_ignore_robots_door_either() {
        // The infrastructure filter must hold on both doors, or the Cloudflare false positive
        // would come back the moment someone crawls with the flag.
        let conn = db();
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let cdn = con_pagina_rastreada_pese_a_robots(
            &conn,
            2,
            "https://ejemplo.es/cdn-cgi/l/email-protection",
            "/cdn-cgi/l/email-protection",
        );
        con_enlace(&conn, portada, cdn);

        assert!(RobotsBlocked.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_url_matching_both_robots_doors_is_one_finding() {
        // No real crawl produces both marks on one URL — an excluded row is never downloaded,
        // so it has no page — but the rule must not depend on that invariant to avoid
        // duplicates: the GROUP BY guarantees one finding either way.
        let conn = db();
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let bloqueada = con_url_bloqueada(&conn, 2, "https://ejemplo.es/privado/");
        conn.execute(
            "INSERT INTO pages (url_id, is_indexable, indexability_reason, internal_links_in)
             VALUES (?1, 0, 'robots', 0)",
            rusqlite::params![bloqueada],
        )
        .expect("insert page");
        con_enlace(&conn, portada, bloqueada);

        assert_eq!(RobotsBlocked.evaluate(&conn).expect("evaluate").len(), 1);
    }

    #[test]
    fn flags_a_robots_txt_that_blocks_the_whole_site() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        conn.execute(
            "INSERT INTO robots_txt (host, status_code, content, blocks_all, sitemap_count)
             VALUES ('ejemplo.es', 200, 'User-agent: *\nDisallow: /', 1, 0)",
            [],
        )
        .expect("insert robots");

        let hallazgos = RobotsTxtBlocksAll.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, None, "it is a site finding, not a URL finding");
    }

    #[test]
    fn does_not_flag_a_robots_txt_that_only_blocks_one_area() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        conn.execute(
            "INSERT INTO robots_txt (host, status_code, content, blocks_all, sitemap_count)
             VALUES ('ejemplo.es', 200, 'User-agent: *\nDisallow: /admin/', 0, 0)",
            [],
        )
        .expect("insert robots");

        assert!(RobotsTxtBlocksAll.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn flags_a_missing_robots_txt_only_in_http_mode() {
        for (modo, esperados) in [("http", 1), ("filesystem", 0)] {
            let conn = db();
            con_meta(&conn, modo, "https://ejemplo.es/", true);
            con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
            conn.execute(
                "INSERT INTO robots_txt (host, status_code, blocks_all, sitemap_count)
                 VALUES ('ejemplo.es', 404, 0, 0)",
                [],
            )
            .expect("insert robots");

            let hallazgos = RobotsTxtMissing.evaluate(&conn).expect("evaluate");
            assert_eq!(hallazgos.len(), esperados, "mode {modo}");
        }
    }

    #[test]
    fn a_network_failure_is_not_a_missing_robots_txt() {
        // A null `status_code` means there was no response. Not being able to check it is not
        // the same as checking it does not exist, and asserting the latter would be making
        // things up.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        conn.execute(
            "INSERT INTO robots_txt (host, status_code, blocks_all, sitemap_count)
             VALUES ('ejemplo.es', NULL, 0, 0)",
            [],
        )
        .expect("insert robots");

        assert!(RobotsTxtMissing.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn flags_a_sitemap_with_broken_xml() {
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, parse_error, url_count,
                                   bytes, discovered_from)
             VALUES ('https://ejemplo.es/sitemap.xml', 200, 0, 0, 'XML mal formado', 3, 400,
                     'well_known')",
            [],
        )
        .expect("insert sitemap");

        let hallazgos = SitemapError.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn a_well_known_sitemap_that_does_not_exist_is_not_an_error() {
        // `/sitemap.xml` and `/sitemap_index.xml` are probed blindly: one of the two
        // returning 404 is normal on every site in the world and is not a finding.
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap_index.xml', 404, 0, 0, 0, 0, 'well_known')",
            [],
        )
        .expect("insert sitemap");

        assert!(SitemapError.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn an_announced_sitemap_that_does_not_respond_is_an_error() {
        // The `robots.txt` points at this one: someone declared it, so it should be there.
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap-posts.xml', 404, 0, 0, 0, 0, 'robots')",
            [],
        )
        .expect("insert sitemap");

        let hallazgos = SitemapError.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn flags_a_sitemap_that_exceeds_the_protocol_limits() {
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap.xml', 200, 0, 1, ?1, 1000, 'well_known')",
            [SITEMAP_MAX_URLS + 1],
        )
        .expect("insert sitemap");

        let hallazgos = SitemapError.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn a_correct_sitemap_produces_no_finding() {
        let conn = db();
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://ejemplo.es/sitemap.xml', 200, 0, 1, 120, 4000, 'well_known')",
            [],
        )
        .expect("insert sitemap");

        assert!(SitemapError.evaluate(&conn).expect("evaluate").is_empty());
    }

    fn con_enlace(conn: &Connection, from: i64, to: i64) {
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, is_nofollow, element)
             VALUES (?1, ?2, 0, 'a')",
            rusqlite::params![from, to],
        )
        .expect("insert link");
    }

    /// The same statement `engine::finalize` runs. Replicated so the tests measure the column
    /// the rules actually read, and not one the test filled in by hand.
    fn recalcular_enlaces_entrantes(conn: &Connection) {
        conn.execute(
            "UPDATE pages SET internal_links_in = (
                 SELECT COUNT(DISTINCT l.from_url_id) FROM links l
                 WHERE l.to_url_id = pages.url_id
             )",
            [],
        )
        .expect("recompute");
    }

    /// Chain home → p1 → … → pN. Returns the ids in order.
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
    fn does_not_flag_depth_up_to_the_fourth_click() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluate");
        assert!(hashes(&hallazgos).is_empty(), "four clicks are allowed");
    }

    #[test]
    fn flags_from_the_fifth_click_and_only_the_pages_beyond_it() {
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 2);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluate");
        // The chain is home(0) → p1(1) → … → p6(6): p5 and p6 are flagged.
        assert_eq!(hashes(&hallazgos), vec![ids[5], ids[6]]);
        assert_eq!(hallazgos[0].1.rule_id, "INDEX-DEEP-PAGE");
        assert_eq!(hallazgos[0].1.severity, Severity::Medium);
    }

    #[test]
    fn depth_is_not_read_from_urls_depth() {
        // Criterion regression: in `filesystem` mode every URL is a seed and `depth` is 0 in
        // all of them, so a rule based on that column would never warn. Depth has to come out
        // of the link graph.
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute("UPDATE urls SET depth = 0", []).expect("flatten the depth");
        assert_eq!(hashes(&DeepPage.evaluate(&conn).expect("evaluate")), vec![ids[5]]);
    }

    #[test]
    fn a_shortcut_from_the_home_page_makes_it_no_longer_deep() {
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 1);
        con_enlace(&conn, ids[0], ids[5]);
        recalcular_enlaces_entrantes(&conn);
        assert!(
            hashes(&DeepPage.evaluate(&conn).expect("evaluate")).is_empty(),
            "with a link from the homepage it is one click away"
        );
    }

    #[test]
    fn a_page_with_no_inbound_links_is_not_reported_as_deep() {
        // It is an orphan, and it has its own rule. Reporting both things about the same URL
        // forces the user to decide which of the two findings to read.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_pagina(&conn, 2, "https://ejemplo.es/suelta", true, false);
        recalcular_enlaces_entrantes(&conn);
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluate")).is_empty());
    }

    #[test]
    fn without_the_home_page_in_the_crawl_the_whole_site_is_not_flagged() {
        // If `base_url` is not among the crawled URLs, the traversal starts empty and the
        // whole site would look unreachable. Staying silent is right: there is nowhere to
        // count from.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        let a = con_pagina(&conn, 1, "https://ejemplo.es/a", true, false);
        let b = con_pagina(&conn, 2, "https://ejemplo.es/b", true, false);
        con_enlace(&conn, a, b);
        recalcular_enlaces_entrantes(&conn);
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluate")).is_empty());
    }

    #[test]
    fn the_home_page_without_a_trailing_slash_is_recognized_all_the_same() {
        // `base_url` stores what the user typed; the normalized URL always carries the slash.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es", true);
        let mut previa = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        for n in 1..=(MAX_CLICK_DEPTH + 1) {
            let id = con_pagina(&conn, n + 1, &format!("https://ejemplo.es/p{n}"), true, false);
            con_enlace(&conn, previa, id);
            previa = id;
        }
        recalcular_enlaces_entrantes(&conn);
        assert_eq!(hashes(&DeepPage.evaluate(&conn).expect("evaluate")).len(), 1);
    }

    #[test]
    fn a_deep_non_indexable_page_is_of_no_interest() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute("UPDATE pages SET is_indexable = 0 WHERE url_id = 6", [])
            .expect("mark noindex");
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluate")).is_empty());
    }

    #[test]
    fn click_depth_is_not_measured_in_list_mode() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute("UPDATE crawl_meta SET mode = 'list'", []).expect("switch mode");
        assert!(hashes(&DeepPage.evaluate(&conn).expect("evaluate")).is_empty());
    }

    #[test]
    fn a_link_that_cannot_be_clicked_does_not_shorten_the_distance() {
        // A `<link rel=next>` or a `<script src>` on the home page is not a click.
        let conn = db();
        let ids = cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, is_nofollow, element)
             VALUES (?1, ?2, 0, 'link')",
            rusqlite::params![ids[0], ids[5]],
        )
        .expect("insert link");
        recalcular_enlaces_entrantes(&conn);
        assert_eq!(hashes(&DeepPage.evaluate(&conn).expect("evaluate")), vec![ids[5]]);
    }

    #[test]
    fn the_detail_carries_each_pages_actual_depth() {
        // It is what lets the report state the shape of the problem in one line —"202,392
        // pages more than 4 clicks away, the deepest at 48"— instead of two hundred thousand
        // identical rows, and lets the XLSX sort by depth.
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 2);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 2);
        let detalles: Vec<&str> =
            hallazgos.iter().filter_map(|(_, i)| i.detail_json.as_deref()).collect();
        assert!(detalles[0].contains("\"click_depth\":5"), "{detalles:?}");
        assert!(detalles[1].contains("\"click_depth\":6"), "{detalles:?}");
        // The threshold travels with the datum, so the export explains itself.
        assert!(detalles[0].contains("\"max_click_depth\":4"), "{detalles:?}");
    }

    /// Writes the findings into `issues` the way the engine does, to test the read side.
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
            .expect("insert finding");
        }
    }

    #[test]
    fn the_shape_of_the_problem_is_read_from_the_written_findings() {
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 4);
        let hallazgos = DeepPage.evaluate(&conn).expect("evaluate");
        escribir_hallazgos(&conn, &hallazgos);

        let forma = deep_page_shape(&conn).expect("read").expect("there are findings");
        // The chain leaves pages at 5, 6, 7 and 8 clicks.
        assert_eq!(forma.pages, 4);
        assert_eq!(forma.deepest, 8);
        assert_eq!(forma.max_click_depth, MAX_CLICK_DEPTH);
        assert_eq!(forma.typical_min, 5);
        assert_eq!(forma.typical_max, 7);
    }

    #[test]
    fn an_old_file_without_depths_has_no_shape_to_read() {
        // Crawls from before this change store a bare `{"max_click_depth":4}`: the read
        // returns None and the report falls back to the generic percentage rephrasing,
        // instead of inventing depths.
        let conn = db();
        cadena(&conn, MAX_CLICK_DEPTH + 1);
        conn.execute(
            "INSERT INTO issues (url_id, rule_id, severity, category, detail_json)
             VALUES (6, 'INDEX-DEEP-PAGE', 'medium', 'indexability',
                     '{\"max_click_depth\":4}')",
            [],
        )
        .expect("old-style finding");
        assert_eq!(deep_page_shape(&conn).expect("read"), None);
    }

    #[test]
    fn without_depth_findings_there_is_no_shape() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        assert_eq!(deep_page_shape(&conn).expect("read"), None);
    }

    // --- Disconnected sections and hreflang seeds ---
    //
    // The real case: a bilingual site whose only bridge from /es to /en was the
    // `<link rel="alternate" hreflang>` in the head —the visible selector was JavaScript—.
    // The traversal that only follows `<a>` reported the 1,987 English pages as "deep".

    /// Declares the `hreflang_json` of an already inserted page, as the engine writes it.
    fn con_hreflang(conn: &Connection, id: i64, json: &str) {
        conn.execute(
            "UPDATE pages SET hreflang_json = ?2 WHERE url_id = ?1",
            rusqlite::params![id, json],
        )
        .expect("declare hreflang");
    }

    /// Home page plus an /en ↔ /en/a pair that only links to itself. Returns the ids
    /// `[home, en, en_a]`.
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
    fn an_unreachable_section_is_not_reported_as_deep() {
        // Unreachable is not deep: they are two different diagnoses, and the depth one —"add
        // pagination shortcuts"— does not fix a section with no crawlable bridge.
        let conn = db();
        con_seccion_aislada(&conn);
        assert!(
            hashes(&DeepPage.evaluate(&conn).expect("evaluate")).is_empty(),
            "pages with no path from the homepage are not 'too many clicks'"
        );
    }

    #[test]
    fn a_section_linked_only_to_itself_is_a_single_site_finding() {
        let conn = db();
        con_seccion_aislada(&conn);
        let hallazgos = SectionDisconnected.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "one cause, one finding: not one per page");
        assert_eq!(hallazgos[0].0, None, "it is a site finding, not a URL finding");
        assert_eq!(hallazgos[0].1.rule_id, "INDEX-SECTION-DISCONNECTED");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"pages\":2"), "both pages counted: {detalle}");
        assert!(detalle.contains("https://ejemplo.es/en"), "with examples: {detalle}");
    }

    #[test]
    fn a_section_declared_by_hreflang_from_the_home_page_is_not_disconnected() {
        // The home page's hreflang is the legitimate mechanism a multilingual site uses to
        // declare its language roots, and Google discovers the section through it. The
        // trailing slash is exercised too: the hreflang says `/en` and the normalized URL
        // could carry it.
        let conn = db();
        con_seccion_aislada(&conn);
        con_hreflang(
            &conn,
            1,
            r#"[["es","https://ejemplo.es/"],["en","https://ejemplo.es/en"]]"#,
        );
        assert!(
            SectionDisconnected.evaluate(&conn).expect("evaluate").is_empty(),
            "a language root declared via hreflang is not a disconnected section"
        );
    }

    #[test]
    fn a_language_sections_depth_is_measured_from_its_own_home_page() {
        // With the hreflang seed, /en is one more root: whatever sits more than four clicks
        // away from it is deep, exactly as in the main language.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_hreflang(&conn, 1, r#"[["en","https://ejemplo.es/en"]]"#);
        let mut previa = con_pagina(&conn, 2, "https://ejemplo.es/en", true, false);
        // /en needs an inbound link to be a candidate; its first page provides it.
        let mut ids = vec![previa];
        for n in 1..=(MAX_CLICK_DEPTH + 1) {
            let id = con_pagina(&conn, n + 2, &format!("https://ejemplo.es/en/p{n}"), true, false);
            con_enlace(&conn, previa, id);
            ids.push(id);
            previa = id;
        }
        con_enlace(&conn, previa, ids[0]);
        recalcular_enlaces_entrantes(&conn);

        let hallazgos = hashes(&DeepPage.evaluate(&conn).expect("evaluate"));
        assert_eq!(
            hallazgos,
            vec![ids[(MAX_CLICK_DEPTH + 1) as usize]],
            "only the last page of the English chain is more than four clicks away"
        );
        assert!(
            SectionDisconnected.evaluate(&conn).expect("evaluate").is_empty(),
            "the whole section is reachable via the hreflang seed"
        );
    }

    #[test]
    fn a_page_with_no_inbound_links_is_not_a_disconnected_section() {
        // INDEX-NO-INTERNAL-LINKS-IN already warns about the loose page. What defines the
        // disconnected section is the opposite: its pages are linked, but only among
        // themselves.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_pagina(&conn, 2, "https://ejemplo.es/suelta", true, false);
        recalcular_enlaces_entrantes(&conn);
        assert!(SectionDisconnected.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn disconnected_sections_are_not_sought_in_list_mode() {
        // In `list` mode links are not followed: everything would look disconnected and the
        // finding would be an artifact of the mode, not of the site.
        let conn = db();
        con_seccion_aislada(&conn);
        conn.execute("UPDATE crawl_meta SET mode = 'list'", []).expect("switch mode");
        assert!(SectionDisconnected.evaluate(&conn).expect("evaluate").is_empty());
    }

    // --- INDEX-NO-INTERNAL-LINKS-IN ---

    #[test]
    fn flags_the_page_nobody_links_to() {
        let conn = db();
        con_meta(&conn, "filesystem", "https://ejemplo.es/", false);
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let enlazada = con_pagina(&conn, 2, "https://ejemplo.es/enlazada", true, false);
        let aislada = con_pagina(&conn, 3, "https://ejemplo.es/aislada", true, false);
        con_enlace(&conn, portada, enlazada);
        recalcular_enlaces_entrantes(&conn);

        let hallazgos = NoInternalLinksIn.evaluate(&conn).expect("evaluate");
        assert_eq!(hashes(&hallazgos), vec![aislada], "the homepage and the linked page do not count");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
    }

    #[test]
    fn the_home_page_is_never_reported_as_having_no_inbound_links() {
        // It is the entry point: nobody links to it by definition. Migration 003 had to fix
        // exactly this false positive in `v_orphans`.
        for base in ["https://ejemplo.es/", "https://ejemplo.es"] {
            let conn = db();
            con_meta(&conn, "http", base, true);
            con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
            recalcular_enlaces_entrantes(&conn);
            assert!(
                hashes(&NoInternalLinksIn.evaluate(&conn).expect("evaluate")).is_empty(),
                "with base_url = {base}"
            );
        }
    }

    #[test]
    fn a_non_indexable_page_with_no_inbound_links_is_not_a_finding() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        con_pagina(&conn, 2, "https://ejemplo.es/gracias", false, false);
        recalcular_enlaces_entrantes(&conn);
        assert!(hashes(&NoInternalLinksIn.evaluate(&conn).expect("evaluate")).is_empty());
    }

    // --- The rules whose datum arrived late: the four that read `urls.in_sitemap` and the
    // --- three that read the `robots_txt` and `sitemaps` tables. These tests are what proved
    // --- their SQL valid against the real schema while no fixture could reach them.

    #[test]
    fn flags_a_sitemap_url_blocked_by_robots() {
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
        .expect("insert urls");
        assert_eq!(hashes(&BlockedInSitemap.evaluate(&conn).expect("evaluate")), vec![1]);
    }

    #[test]
    fn flags_a_sitemap_url_blocked_by_robots_under_ignore_robots_too() {
        // Proves the fix, same defect as in `RobotsBlocked`: with `--ignore-robots` the URL
        // gets crawled instead of excluded, the mark moves to `pages.indexability_reason`,
        // and the sitemap-vs-robots contradiction — which is exactly the same either way —
        // went unreported.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (1, 'https://ejemplo.es/privado', 1, 'https', 'ejemplo.es', '/privado', 1, 1,
                     'done', 200)",
            [],
        )
        .expect("insert url");
        conn.execute(
            "INSERT INTO pages (url_id, is_indexable, indexability_reason, internal_links_in)
             VALUES (1, 0, 'robots', 0)",
            [],
        )
        .expect("insert page");

        assert_eq!(hashes(&BlockedInSitemap.evaluate(&conn).expect("evaluate")), vec![1]);
    }

    #[test]
    fn flags_a_sitemap_url_with_noindex() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/gracias", false, true);
        con_pagina(&conn, 2, "https://ejemplo.es/normal", true, true);
        assert_eq!(hashes(&NoindexInSitemap.evaluate(&conn).expect("evaluate")), vec![1]);
    }

    #[test]
    fn flags_the_orphan_page_declared_in_the_sitemap() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        let portada = con_pagina(&conn, 1, "https://ejemplo.es/", true, true);
        let enlazada = con_pagina(&conn, 2, "https://ejemplo.es/enlazada", true, true);
        con_pagina(&conn, 3, "https://ejemplo.es/huerfana", true, true);
        con_enlace(&conn, portada, enlazada);
        recalcular_enlaces_entrantes(&conn);
        assert_eq!(hashes(&OrphanPage.evaluate(&conn).expect("evaluate")), vec![3]);
    }

    #[test]
    fn flags_the_missing_sitemap_at_site_level() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        let hallazgos = SitemapMissing.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, None, "it is a site finding, not a URL finding");
    }

    #[test]
    fn does_not_flag_a_missing_sitemap_when_one_was_found() {
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, true);
        assert!(SitemapMissing.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn does_not_flag_a_missing_sitemap_when_none_was_sought() {
        // Neither when the user turned it off, nor in the modes that do not consult it: not
        // finding something that was never looked for is not a finding.
        let conn = db();
        con_meta(&conn, "http", "https://ejemplo.es/", false);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        assert!(SitemapMissing.evaluate(&conn).expect("evaluate").is_empty());

        let conn = db();
        con_meta(&conn, "filesystem", "https://ejemplo.es/", true);
        con_pagina(&conn, 1, "https://ejemplo.es/", true, false);
        assert!(SitemapMissing.evaluate(&conn).expect("evaluate").is_empty());
    }

    // --- Registry ---

    #[test]
    fn the_registered_rules_are_the_ones_with_a_fixture() {
        // The fixture bank demands that every catalogue rule fire its own fixture when it is
        // crawled, or be declared an exception with its reason. Those that meet neither stay
        // out of the registry on purpose, and this test keeps anyone from adding them without
        // realizing they break the bank.
        //
        // The four sitemap rules entered on 2026-07-30, when `filesystem` mode started
        // reading the `dist/`'s sitemap: until then `urls.in_sitemap` was 0 and none of them
        // could produce a finding. `INDEX-SITEMAP-MISSING` is the group's declared exception
        // —it only applies in `http` mode—; the other three fire with their fixture.
        //
        // `INDEX-ROBOTS-BLOCKED` went in once the catalogue corrected its scope to `site`: it
        // was catalogued `page` and could never be one, because a URL forbidden by `robots.txt`
        // is never downloaded and so has no `PageContext`. See the module header.
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
            "the thirteen rules of §2 are registered: none is left out since migration 004"
        );
    }
}
