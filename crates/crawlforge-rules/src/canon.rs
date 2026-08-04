//! `CANON` and `DUP` — canonicals and duplicate content. `docs/04-CATALOGO-REGLAS.md §5`.
//!
//! # The `JOIN` behind the site-scope rules
//!
//! The five `site`-scope rules in this section need to join the source page with the `urls` row
//! its canonical points at. `pages.canonical` stores the **already-normalised absolute URL**
//! (`engine::finish_page` resolves it against the page URL), and `urls` is indexed by
//! `url_hash`, an `xxh3_64` computed in Rust (`crawlforge_core::engine::url_hash`). SQLite has
//! no such function, so **the hash `JOIN` cannot be written in pure SQL**: it would take
//! registering a `create_scalar_function` on the connection, and a rule must not modify the
//! connection the engine lends it.
//!
//! The way out is joining by text: `JOIN urls tgt ON tgt.url = p.canonical`. `urls.url` is
//! `UNIQUE`, so the `JOIN` uses its index and is exactly as selective as the hash one. Both
//! strings come out of the same normaliser, so they match byte for byte when they point at the
//! same URL.

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::Connection;

pub static CANON_MISSING: RuleMeta = RuleMeta {
    id: "CANON-MISSING",
    severity: Severity::Medium,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin canonical",
    name_en: "Missing canonical",
    desc_es: "La página indexable no declara rel=canonical. No es grave por sí solo, porque \
              Google infiere el canonical, pero sin él cualquier parámetro de URL —una campaña, \
              un filtro, un orden— puede acabar indexado como una página distinta.",
    desc_en: "The indexable page declares no rel=canonical. Not serious on its own, since Google \
              infers the canonical, but without it any URL parameter — a campaign, a filter, a \
              sort order — can end up indexed as a separate page.",
    references: &[],
};

pub static CANON_MULTIPLE: RuleMeta = RuleMeta {
    id: "CANON-MULTIPLE",
    severity: Severity::High,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Varios canonical",
    name_en: "Multiple canonicals",
    desc_es: "La página declara más de un link rel=canonical. Google no elige entre ellos: los \
              ignora todos, así que el efecto es el de no tener ninguno. Casi siempre es la \
              plantilla y un plugin de SEO emitiendo la etiqueta cada uno por su cuenta.",
    desc_en: "The page declares more than one link rel=canonical. Google does not pick one: it \
              ignores them all, so the effect is having none at all. It is almost always the \
              theme and an SEO plugin each emitting the tag on their own.",
    references: &[],
};

pub static CANON_RELATIVE: RuleMeta = RuleMeta {
    id: "CANON-RELATIVE",
    severity: Severity::Medium,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Canonical relativo",
    name_en: "Relative canonical",
    desc_es: "El canonical se declara como referencia relativa en vez de con URL absoluta. \
              Funciona mientras la página se sirva en un solo sitio, pero si el HTML se \
              reproduce bajo otro host —un entorno de pruebas, un proxy, un scraper— el \
              canonical se resuelve contra ese host y deja de señalar al original.",
    desc_en: "The canonical is declared as a relative reference instead of an absolute URL. It \
              works while the page is served from one place, but if the HTML is reproduced under \
              another host — a staging environment, a proxy, a scraper — the canonical resolves \
              against that host and stops pointing at the original.",
    references: &[],
};

pub static CANON_TO_4XX: RuleMeta = RuleMeta {
    id: "CANON-TO-4XX",
    severity: Severity::Critical,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Canonical a una URL con error",
    name_en: "Canonical to an error URL",
    desc_es: "El canonical apunta a una URL que responde con error. La página se está declarando \
              duplicada de algo que no existe, así que ni ella ni el destino pueden indexarse: \
              el contenido desaparece de los resultados por completo.",
    desc_en: "The canonical points to a URL that answers with an error. The page declares itself \
              a duplicate of something that does not exist, so neither it nor the target can be \
              indexed: the content disappears from results entirely.",
    references: &[],
};

pub static CANON_TO_REDIRECT: RuleMeta = RuleMeta {
    id: "CANON-TO-REDIRECT",
    severity: Severity::High,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Canonical a una redirección",
    name_en: "Canonical to a redirect",
    desc_es: "El canonical apunta a una URL que redirige a otra. Google tiene que decidir entre \
              la señal del canonical y la de la redirección, y suele quedarse con el destino \
              final, con lo que la etiqueta no sirve para nada. Apúntala directamente a la URL \
              que responde 200.",
    desc_en: "The canonical points to a URL that redirects elsewhere. Google has to choose \
              between the canonical signal and the redirect one, and usually keeps the final \
              destination, which makes the tag pointless. Point it straight at the URL that \
              answers 200.",
    references: &[],
};

pub static CANON_TO_NOINDEX: RuleMeta = RuleMeta {
    id: "CANON-TO-NOINDEX",
    severity: Severity::Critical,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Canonical a una página con noindex",
    name_en: "Canonical to a noindexed page",
    desc_es: "El canonical apunta a una página marcada con noindex. Las dos señales se \
              contradicen: una dice «indexa aquella» y aquella dice «no me indexes». El \
              resultado habitual es que se pierden las dos URLs.",
    desc_en: "The canonical points to a page marked noindex. The two signals contradict each \
              other: one says «index that one» and that one says «do not index me». The usual \
              outcome is losing both URLs.",
    references: &[],
};

pub static CANON_CHAIN: RuleMeta = RuleMeta {
    id: "CANON-CHAIN",
    severity: Severity::High,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Cadena de canonical",
    name_en: "Canonical chain",
    desc_es: "A declara como canonical a B, y B declara como canonical a C. El canonical no es \
              transitivo para Google: al encontrar una cadena la ignora y decide por su cuenta \
              cuál es la URL principal. Toda la cadena debe apuntar directamente a C.",
    desc_en: "A declares B as its canonical, and B declares C. Canonicals are not transitive for \
              Google: when it finds a chain it ignores it and decides on its own which URL is \
              the main one. Every step should point straight at C.",
    references: &[],
};

pub static CANON_CROSS_DOMAIN: RuleMeta = RuleMeta {
    id: "CANON-CROSS-DOMAIN",
    severity: Severity::Medium,
    category: Category::Canonical,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Canonical a otro dominio",
    name_en: "Cross-domain canonical",
    desc_es: "El canonical apunta a un host distinto del de la página. Es legítimo en \
              sindicación de contenido, pero cuando no es deliberado regala el posicionamiento \
              al otro dominio: la propia página deja de indexarse. Suele venir de una migración \
              a medias o de un entorno de pruebas copiado.",
    desc_en: "The canonical points to a host other than the page's own. That is legitimate for \
              syndicated content, but when it is not deliberate it hands the ranking to the \
              other domain: the page itself stops being indexed. It usually comes from a \
              half-finished migration or a copied staging environment.",
    references: &[],
};

pub static DUP_CONTENT_EXACT: RuleMeta = RuleMeta {
    id: "DUP-CONTENT-EXACT",
    severity: Severity::High,
    category: Category::Duplicate,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Contenido idéntico",
    name_en: "Identical content",
    desc_es: "Dos o más URLs indexables devuelven exactamente el mismo HTML, byte a byte. \
              Compiten entre sí por las mismas consultas y reparten los enlaces entrantes en vez \
              de sumarlos. Se resuelve dejando una indexable y canonizando las demás hacia ella.",
    desc_en: "Two or more indexable URLs return exactly the same HTML, byte for byte. They \
              compete against each other for the same queries and split incoming links instead \
              of adding them up. Fix it by keeping one indexable and canonicalising the rest \
              to it.",
    references: &[],
};

// ---------------------------------------------------------------- URL utilities

/// The host of an absolute URL, without `userinfo` or port.
///
/// Hand-rolled because this crate deliberately does not depend on `url`: rules receive strings
/// the engine has already resolved and must not re-parse anything heavy.
fn host_of(url: &str) -> Option<&str> {
    let (_, resto) = url.split_once("://")?;
    let autoridad = resto.split(['/', '?', '#']).next()?;
    // `user:password@host` — the last `@` delimits the real authority.
    let sin_userinfo = autoridad.rsplit_once('@').map_or(autoridad, |(_, h)| h);
    // The port is whatever follows the last `:` **and is all digits**. That way an IPv6 literal
    // like `[::1]` is not clipped, while `[::1]:8080` does lose its port.
    let host = match sin_userinfo.rsplit_once(':') {
        Some((h, puerto)) if !puerto.is_empty() && puerto.bytes().all(|b| b.is_ascii_digit()) => h,
        _ => sin_userinfo,
    };
    (!host.is_empty()).then_some(host)
}

/// Leading `www.` off. Hosts are compared without it because `ejemplo.es` and `www.ejemplo.es`
/// are the same site: a canonical between them is host consolidation, not a cross-domain
/// canonical, and flagging it as one would be a false positive on the single most common
/// pattern there is.
fn sin_www(host: &str) -> &str {
    if host.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("www.")) {
        &host[4..]
    } else {
        host
    }
}

/// Are the hosts of these two absolute URLs the same site?
///
/// `None` if either has no recognisable host: without knowing it, no cross-domain claim can be
/// made, and a rule does not invent findings out of data it does not understand.
fn same_host(a: &str, b: &str) -> Option<bool> {
    let (ha, hb) = (host_of(a)?, host_of(b)?);
    Some(sin_www(ha).eq_ignore_ascii_case(sin_www(hb)))
}

/// Does the reference carry its own scheme (`https:`, `mailto:`) and is therefore absolute?
///
/// Per RFC 3986 a reference is absolute if and only if it starts with a scheme. That leaves
/// `//ejemplo.es/a` — a *network-path reference* — on the relative side, which is correct: it
/// depends on the scheme of the page that contains it.
fn tiene_esquema(referencia: &str) -> bool {
    let bytes = referencia.as_bytes();
    if bytes.first().is_none_or(|b| !b.is_ascii_alphabetic()) {
        return false;
    }
    for (i, b) in bytes.iter().enumerate() {
        if *b == b':' {
            return i > 0;
        }
        if !(b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.')) {
            return false;
        }
    }
    false
}

// ---------------------------------------------------------------- Page rules

/// Indexable page without `rel=canonical`.
///
/// Not a serious failure on its own — Google infers the canonical — but without it any URL
/// parameter can spawn a duplicate. Medium severity, not high.
pub struct CanonMissing;

impl PageRule for CanonMissing {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        if ctx.canonical.map(|c| !c.trim().is_empty()).unwrap_or(false) {
            return Vec::new();
        }
        vec![Issue::new(&CANON_MISSING)]
    }
}

/// More than one `link rel=canonical` on the same page.
///
/// **Not filtered by `is_indexable`.** If the canonicals point at another URL, the engine marks
/// the page as `canonicalised` and therefore non-indexable; requiring `is_indexable` would
/// silence precisely the worst cases. A 2xx is required, though, as in every rule that audits
/// the served HTML: no search engine processes the canonical in a 404's error template, and
/// without the gate every broken URL would repeat the theme's findings.
/// See `PageContext::is_success`.
pub struct CanonMultiple;

impl PageRule for CanonMultiple {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_MULTIPLE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_success() || ctx.canonical_count <= 1 {
            return Vec::new();
        }
        vec![Issue::new(&CANON_MULTIPLE)
            .with_detail(serde_json::json!({ "count": ctx.canonical_count }))]
    }
}

/// Canonical declared as a relative reference.
///
/// Looks at `canonical_raw` — what the HTML actually carried — and not `canonical`, which the
/// engine has already resolved to absolute. It is the one place in the catalogue where the
/// original form matters more than the resolved one.
pub struct CanonRelative;

impl PageRule for CanonRelative {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_RELATIVE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // The 2xx cuts off the error template: see `CanonMultiple` and `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        let Some(bruto) = ctx.canonical_raw.map(str::trim).filter(|c| !c.is_empty()) else {
            return Vec::new();
        };
        if tiene_esquema(bruto) {
            return Vec::new();
        }
        vec![Issue::new(&CANON_RELATIVE).with_detail(serde_json::json!({
            "canonical_raw": bruto,
            "resolved": ctx.canonical,
        }))]
    }
}

/// Canonical to a host other than the page's own.
///
/// Compares the host of the resolved canonical with the page's own. A `www.` of difference does
/// not count: see [`sin_www`].
pub struct CanonCrossDomain;

impl PageRule for CanonCrossDomain {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_CROSS_DOMAIN
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // The 2xx cuts off the error template: see `CanonMultiple` and `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        let Some(canonical) = ctx.canonical.map(str::trim).filter(|c| !c.is_empty()) else {
            return Vec::new();
        };
        // `None` = one of the two hosts could not be determined: claim nothing.
        if same_host(ctx.url, canonical).unwrap_or(true) {
            return Vec::new();
        }
        vec![Issue::new(&CANON_CROSS_DOMAIN).with_detail(serde_json::json!({
            "canonical": canonical,
            "canonical_host": host_of(canonical),
            "page_host": host_of(ctx.url),
        }))]
    }
}

// ---------------------------------------------------------------- Site rules

/// Common trunk of the "the canonical points at something it shouldn't" rules.
///
/// `tgt.id <> src.id` is how "the canonical designates another URL" is spelled. It is preferred
/// over `pages.canonical_is_self` because it falls out of the `JOIN` itself: if the canonical
/// resolves to the page's own row there is nothing to report, without depending on how the
/// string was compared when it was written. The text `JOIN` is justified in the module header.
const CANONICAL_JOIN: &str = "FROM pages p
     JOIN urls src ON src.id = p.url_id
     JOIN urls tgt ON tgt.url = p.canonical
     ";

/// The canonical points at a URL that answers 4xx or 5xx.
///
/// The ID says `4XX` because that is the normal case, but the catalogue's normative condition is
/// "error URL" and a canonical to a 500 is every bit as fatal, so it covers 400 and up. The
/// actual code goes in the finding's detail.
pub struct CanonToError;

impl SiteRule for CanonToError {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_TO_4XX
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let sql = format!(
            "SELECT src.url_hash, p.canonical, tgt.status_code
             {CANONICAL_JOIN}
             WHERE tgt.id <> src.id AND tgt.status_code >= 400"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, canonical, status) = row?;
            out.push((
                Some(hash),
                Issue::new(&CANON_TO_4XX)
                    .with_detail(serde_json::json!({ "canonical": canonical, "status": status })),
            ));
        }
        Ok(out)
    }
}

/// The canonical points at a URL that redirects.
///
/// Two ways to detect it, because the engine stores both: a 3xx `status_code`, and a
/// `redirect_to` resolved during the writer's pass.
pub struct CanonToRedirect;

impl SiteRule for CanonToRedirect {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_TO_REDIRECT
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let sql = format!(
            "SELECT src.url_hash, p.canonical, tgt.status_code, dst.url
             {CANONICAL_JOIN}
             LEFT JOIN urls dst ON dst.id = tgt.redirect_to
             WHERE tgt.id <> src.id
               AND ((tgt.status_code >= 300 AND tgt.status_code < 400)
                 OR tgt.redirect_to IS NOT NULL)"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, canonical, status, destino) = row?;
            out.push((
                Some(hash),
                Issue::new(&CANON_TO_REDIRECT).with_detail(serde_json::json!({
                    "canonical": canonical,
                    "status": status,
                    "redirects_to": destino,
                })),
            ));
        }
        Ok(out)
    }
}

/// The canonical points at a page marked `noindex`.
///
/// Looks at the declared directive (`meta robots` or `X-Robots-Tag`), not at `is_indexable`: a
/// page can be non-indexable for half a dozen reasons, and each one has its own rule. The
/// finding here is the contradiction between two explicit signals.
pub struct CanonToNoindex;

impl SiteRule for CanonToNoindex {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_TO_NOINDEX
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let sql = format!(
            "SELECT src.url_hash, p.canonical,
                    COALESCE(tp.meta_robots, '') || ' ' || COALESCE(tp.x_robots_tag, '')
             {CANONICAL_JOIN}
             JOIN pages tp ON tp.url_id = tgt.id
             WHERE tgt.id <> src.id
               AND (LOWER(COALESCE(tp.meta_robots, '')) LIKE '%noindex%'
                 OR LOWER(COALESCE(tp.x_robots_tag, '')) LIKE '%noindex%')"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, canonical, directiva) = row?;
            out.push((
                Some(hash),
                Issue::new(&CANON_TO_NOINDEX).with_detail(serde_json::json!({
                    "canonical": canonical,
                    "directive": directiva.trim(),
                })),
            ));
        }
        Ok(out)
    }
}

/// A canonicalises to B and B canonicalises to C.
///
/// **What counts as a chain:** the canonical's target has, in turn, a canonical that resolves to
/// a URL other than itself. C is not required to differ from A, so the `A → B → A` loop is
/// reported too: same defect, same fix. The finding is recorded on A, which is the page that
/// needs correcting.
pub struct CanonChain;

impl SiteRule for CanonChain {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_CHAIN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let sql = format!(
            "SELECT src.url_hash, p.canonical, tp.canonical
             {CANONICAL_JOIN}
             JOIN pages tp ON tp.url_id = tgt.id
             JOIN urls tgt2 ON tgt2.url = tp.canonical
             WHERE tgt.id <> src.id AND tgt2.id <> tgt.id"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, primero, segundo) = row?;
            out.push((
                Some(hash),
                Issue::new(&CANON_CHAIN).with_detail(serde_json::json!({
                    "canonical": primero,
                    "then": segundo,
                })),
            ));
        }
        Ok(out)
    }
}

/// Two or more indexable URLs return byte-for-byte identical HTML.
///
/// Restricted to `is_indexable = 1`, like `META-TITLE-DUPLICATE`: two copies where one
/// canonicalises to the other are the fix, not the problem, and reporting them would be noise
/// over work already done.
pub struct DupContentExact;

impl SiteRule for DupContentExact {
    fn meta(&self) -> &'static RuleMeta {
        &DUP_CONTENT_EXACT
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, p.html_hash, COUNT(*) OVER (PARTITION BY p.html_hash) AS n
             FROM pages p
             JOIN urls u ON u.id = p.url_id
             WHERE p.is_indexable = 1 AND p.html_hash IS NOT NULL
             AND p.html_hash IN (
                 SELECT html_hash FROM pages
                 WHERE is_indexable = 1 AND html_hash IS NOT NULL
                 GROUP BY html_hash HAVING COUNT(*) > 1
             )",
        )?;

        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, html_hash, n) = row?;
            out.push((
                Some(hash),
                Issue::new(&DUP_CONTENT_EXACT)
                    .with_detail(serde_json::json!({ "pages": n }))
                    .with_group(format!("html:{html_hash:016x}")),
            ));
        }
        Ok(out)
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![
        Box::new(CanonMissing),
        Box::new(CanonMultiple),
        Box::new(CanonRelative),
        Box::new(CanonCrossDomain),
    ]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    // `CanonToRedirect`'s defect cannot be provoked from a file tree: fixtures are crawled in
    // `filesystem` mode, where the fetcher only answers 200 or 404, so `urls.status_code` never
    // falls in the 3xx range and `urls.redirect_to` never gets filled in — the same gap the
    // `HTTP-REDIRECT-*` rules have. Its end-to-end proof therefore runs against the local HTTP
    // test server (`crawlforge-core/tests/reglas_http.rs`), and the rule is declared in
    // `DEMOSTRADAS_CONTRA_EL_SERVIDOR` (`crawlforge-core/tests/fixtures_de_reglas.rs`), the
    // inventory of what the fixture harness cannot cover.
    vec![
        Box::new(CanonToError),
        Box::new(CanonToNoindex),
        Box::new(CanonChain),
        Box::new(CanonToRedirect),
        Box::new(DupContentExact),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>() -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.canonical = Some("https://ejemplo.es/a");
        c.canonical_raw = Some("https://ejemplo.es/a");
        c.canonical_count = 1;
        c
    }

    // ------------------------------------------------------------ CANON-MISSING

    #[test]
    fn no_finding_when_a_canonical_is_present() {
        assert!(CanonMissing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_missing_canonical_produces_a_finding() {
        let mut c = ctx();
        c.canonical = None;
        c.canonical_raw = None;
        c.canonical_count = 0;
        let issues = CanonMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Medium, "a missing canonical is not critical");
    }

    #[test]
    fn a_non_indexable_page_produces_no_finding() {
        let mut c = ctx();
        c.canonical = None;
        c.is_indexable = false;
        assert!(CanonMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn the_error_template_canonical_is_not_audited() {
        // The three rules that do not filter by `is_indexable` require a 2xx: no search engine
        // processes the canonical in a 404's HTML, and without the gate the theme's error
        // template got audited once per broken URL. See `PageContext::is_success`.
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            c.canonical_count = 2;
            assert!(
                CanonMultiple.evaluate(&c).is_empty(),
                "CANON-MULTIPLE should not audit the HTML of a {status}"
            );

            let mut c = ctx();
            c.status = status;
            c.canonical_raw = Some("/a");
            assert!(
                CanonRelative.evaluate(&c).is_empty(),
                "CANON-RELATIVE should not audit the HTML of a {status}"
            );

            let mut c = ctx();
            c.status = status;
            c.canonical = Some("https://otro.com/a");
            assert!(
                CanonCrossDomain.evaluate(&c).is_empty(),
                "CANON-CROSS-DOMAIN should not audit the HTML of a {status}"
            );
        }
    }

    // ------------------------------------------------------------ CANON-MULTIPLE

    #[test]
    fn a_single_canonical_is_not_multiple() {
        assert!(CanonMultiple.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn no_canonical_at_all_is_not_multiple() {
        let mut c = ctx();
        c.canonical = None;
        c.canonical_raw = None;
        c.canonical_count = 0;
        assert!(CanonMultiple.evaluate(&c).is_empty());
    }

    #[test]
    fn two_canonicals_trigger_the_rule() {
        let mut c = ctx();
        c.canonical_count = 2;
        let issues = CanonMultiple.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CANON-MULTIPLE");
        assert_eq!(issues[0].severity, Severity::High);
        assert_eq!(issues[0].detail_json.as_deref(), Some(r#"{"count":2}"#));
    }

    #[test]
    fn multiple_canonicals_are_reported_even_on_a_non_indexable_page() {
        // If the canonicals point at another URL the engine marks the page as `canonicalised`,
        // and that is precisely the case that matters most: filtering by `is_indexable` would
        // hide it.
        let mut c = ctx();
        c.canonical_count = 3;
        c.canonical = Some("https://ejemplo.es/otra");
        c.is_indexable = false;
        assert_eq!(CanonMultiple.evaluate(&c).len(), 1);
    }

    #[test]
    fn multiple_canonicals_on_something_that_is_not_html_produce_no_finding() {
        let mut c = ctx();
        c.canonical_count = 2;
        c.is_html = false;
        assert!(CanonMultiple.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ CANON-RELATIVE

    #[test]
    fn an_absolute_canonical_is_not_relative() {
        assert!(CanonRelative.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_root_relative_canonical_is_still_relative() {
        let mut c = ctx();
        c.canonical_raw = Some("/a");
        let issues = CanonRelative.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CANON-RELATIVE");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn a_document_relative_canonical_triggers_the_rule() {
        let mut c = ctx();
        c.canonical_raw = Some("../otra/");
        assert_eq!(CanonRelative.evaluate(&c).len(), 1);
    }

    #[test]
    fn a_schemeless_canonical_is_relative_even_with_a_host() {
        // `//ejemplo.es/a` is a *network-path reference*: it inherits the page's scheme.
        let mut c = ctx();
        c.canonical_raw = Some("//ejemplo.es/a");
        assert_eq!(CanonRelative.evaluate(&c).len(), 1);
    }

    #[test]
    fn an_http_canonical_is_not_considered_relative() {
        let mut c = ctx();
        c.canonical_raw = Some("http://ejemplo.es/a");
        assert!(CanonRelative.evaluate(&c).is_empty());
    }

    #[test]
    fn without_a_canonical_there_is_nothing_to_say_about_relative() {
        let mut c = ctx();
        c.canonical = None;
        c.canonical_raw = None;
        assert!(CanonRelative.evaluate(&c).is_empty());
    }

    #[test]
    fn a_whitespace_only_canonical_does_not_count_as_relative() {
        // An `href=""` or a whitespace-only one is a different defect: for this rule there is
        // no reference at all.
        let mut c = ctx();
        c.canonical_raw = Some("   ");
        assert!(CanonRelative.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ CANON-CROSS-DOMAIN

    #[test]
    fn a_canonical_to_the_same_host_does_not_cross_domains() {
        assert!(CanonCrossDomain.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_canonical_to_another_domain_triggers_the_rule() {
        let mut c = ctx();
        c.canonical = Some("https://otrodominio.example/a");
        let issues = CanonCrossDomain.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CANON-CROSS-DOMAIN");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn www_does_not_count_as_another_domain() {
        let mut c = ctx();
        c.canonical = Some("https://www.ejemplo.es/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());

        let mut c = PageContext::indexable_html("https://www.ejemplo.es/a");
        c.canonical = Some("https://ejemplo.es/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    #[test]
    fn a_subdomain_does_count_as_another_domain() {
        // For Google's canonicalisation `blog.ejemplo.es` is another host, not another section.
        let mut c = ctx();
        c.canonical = Some("https://blog.ejemplo.es/a");
        assert_eq!(CanonCrossDomain.evaluate(&c).len(), 1);
    }

    #[test]
    fn port_and_scheme_do_not_make_it_cross_domain() {
        let mut c = ctx();
        c.canonical = Some("http://ejemplo.es:8080/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    #[test]
    fn the_host_comparison_is_case_insensitive() {
        let mut c = ctx();
        c.canonical = Some("https://EJEMPLO.ES/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    #[test]
    fn a_canonical_with_no_recognisable_host_produces_no_finding() {
        // The engine hands over the canonical already resolved to absolute; if even then no host
        // can be seen, the rule stays quiet instead of inventing a cross-domain finding.
        let mut c = ctx();
        c.canonical = Some("mailto:hola@ejemplo.es");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ Utilities

    #[test]
    fn the_host_is_extracted_from_an_absolute_url() {
        assert_eq!(host_of("https://ejemplo.es/a?b=1#c"), Some("ejemplo.es"));
        assert_eq!(host_of("https://ejemplo.es"), Some("ejemplo.es"));
        assert_eq!(host_of("https://ejemplo.es:8443/a"), Some("ejemplo.es"));
        assert_eq!(host_of("https://user:pass@ejemplo.es/a"), Some("ejemplo.es"));
        assert_eq!(host_of("https://[::1]:8080/a"), Some("[::1]"));
        assert_eq!(host_of("https://[::1]/a"), Some("[::1]"));
        assert_eq!(host_of("/solo/una/ruta"), None);
        assert_eq!(host_of("https:///a"), None);
    }

    #[test]
    fn the_scheme_is_detected_as_rfc_3986_says() {
        assert!(tiene_esquema("https://ejemplo.es/a"));
        assert!(tiene_esquema("HTTP://ejemplo.es/a"));
        assert!(tiene_esquema("mailto:hola@ejemplo.es"));
        assert!(!tiene_esquema("//ejemplo.es/a"));
        assert!(!tiene_esquema("/a"));
        assert!(!tiene_esquema("a/b"));
        assert!(!tiene_esquema(""));
        // A `:` inside the path is not a scheme.
        assert!(!tiene_esquema("/a/b:c"));
        assert!(!tiene_esquema("2ejemplo:/a"), "a scheme does not start with a digit");
    }

    // ------------------------------------------------------------ Site rules

    /// An in-memory database with the real schema. Loads the published migrations instead of a
    /// hand-written `CREATE TABLE`: that way a schema change breaks these tests instead of
    /// leaving them measuring a table that no longer exists.
    fn db() -> Connection {
        // **All** the migrations, always, from the shared helper in `test_schema.rs`. This
        // module once carried its own list, the list once stopped at 001, and that is why the
        // index from 006 did not exist here. Now the helper holds the only list and a guard
        // test keeps it in sync with the `migrations/` directory.
        crate::test_schema::full_schema()
    }

    /// The duplicate-content query must not sort the whole table.
    ///
    /// Without the index from migration 006, SQLite resolves its two groupings by `html_hash`
    /// with temporary B-trees over the entire table. **Measured on 2026-08-02 while crawling a
    /// full news site: over eight hours spent in this single rule**, across 216,349 pages in a
    /// 5.3 GB file, when the whole crawl of 487,621 URLs had finished in nine and a half hours.
    ///
    /// The assertion is about the plan, not the time, because at the scale that fits in a test
    /// the clock cannot tell the two worlds apart. What can is whether the grouping has an index
    /// to lean on — and this test is also the one that rejected the index's first form, a
    /// partial index over `html_hash` that SQLite would not even use.
    #[test]
    fn duplicate_detection_has_an_index_to_group_on() {
        let conn = db();
        let existe: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_pages_html_hash'",
                [],
                |r| r.get(0),
            )
            .expect("query sqlite_master");
        assert_eq!(existe, 1, "the pages(html_hash) index from migration 006 is missing");

        // And it must be usable by the grouping subquery, which is the one that cost the hours.
        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT html_hash FROM pages
                 WHERE is_indexable = 1 AND html_hash IS NOT NULL
                 GROUP BY html_hash HAVING COUNT(*) > 1",
            )
            .expect("prepare the plan");
        let plan: String = stmt
            .query_map([], |r| r.get::<_, String>(3))
            .expect("read the plan")
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(
            plan.contains("idx_pages_html_hash"),
            "the html_hash grouping must lean on its index, and the plan says: {plan}"
        );
    }

    /// Inserts a URL. The `url_hash` is set equal to the `id` so the test can check which row a
    /// finding was recorded against.
    fn url(conn: &Connection, id: i64, u: &str, status: Option<i64>) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', 'fixture.local', '/', 1, 0, 'done', ?3)",
            rusqlite::params![id, u, status],
        )
        .expect("insert url");
    }

    fn redirige(conn: &Connection, id: i64, hacia: i64) {
        conn.execute("UPDATE urls SET redirect_to = ?2 WHERE id = ?1", [id, hacia])
            .expect("mark redirect");
    }

    /// Inserts a URL's page row. `canonical` as `None` means "no tag".
    fn page(
        conn: &Connection,
        url_id: i64,
        canonical: Option<&str>,
        propia: &str,
        robots: Option<&str>,
        indexable: bool,
    ) {
        conn.execute(
            "INSERT INTO pages (url_id, title, canonical, canonical_is_self, meta_robots,
                                is_indexable, html_hash)
             VALUES (?1, 'A page', ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                url_id,
                canonical,
                canonical.map(|c| c == propia),
                robots,
                indexable as i64,
                url_id * 1000,
            ],
        )
        .expect("insert page");
    }

    fn ids(hallazgos: &[(Option<i64>, Issue)]) -> Vec<i64> {
        hallazgos.iter().filter_map(|(h, _)| *h).collect()
    }

    #[test]
    fn a_canonical_to_a_404_triggers_the_rule() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/roto", Some(404));
        page(&conn, 1, Some("https://fixture.local/roto"), "https://fixture.local/a", None, false);

        let hallazgos = CanonToError.evaluate(&conn).expect("evaluate");
        assert_eq!(ids(&hallazgos), vec![1], "the finding goes on the source page");
        assert_eq!(hallazgos[0].1.severity, Severity::Critical);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("404"), "the detail carries the real status code: {detalle}");
    }

    #[test]
    fn a_canonical_to_a_500_also_triggers_the_rule() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/roto", Some(500));
        page(&conn, 1, Some("https://fixture.local/roto"), "https://fixture.local/a", None, false);
        assert_eq!(ids(&CanonToError.evaluate(&conn).expect("evaluate")), vec![1]);
    }

    #[test]
    fn a_canonical_to_a_200_does_not_trigger_the_error_rule() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        assert!(CanonToError.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_self_canonical_triggers_no_target_rule() {
        // Edge case: the page itself answers 404 and canonicalises to itself. That is not a
        // broken canonical, it is a 404, and another rule reports that.
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(404));
        page(&conn, 1, Some("https://fixture.local/a"), "https://fixture.local/a", None, false);
        assert!(CanonToError.evaluate(&conn).expect("evaluate").is_empty());
        assert!(CanonChain.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_canonical_to_an_uncrawled_url_produces_no_finding() {
        // With no target row there is nothing to claim: the canonical may be correct and simply
        // outside the crawl's scope.
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        page(&conn, 1, Some("https://otro.example/b"), "https://fixture.local/a", None, false);
        assert!(CanonToError.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_canonical_to_a_redirect_triggers_the_rule_via_the_status_code() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/vieja", Some(301));
        url(&conn, 3, "https://fixture.local/nueva", Some(200));
        redirige(&conn, 2, 3);
        page(&conn, 1, Some("https://fixture.local/vieja"), "https://fixture.local/a", None, false);

        let hallazgos = CanonToRedirect.evaluate(&conn).expect("evaluate");
        assert_eq!(ids(&hallazgos), vec![1]);
        assert_eq!(hallazgos[0].1.severity, Severity::High);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("/nueva"), "the detail says where it ends up: {detalle}");
    }

    #[test]
    fn a_canonical_to_a_redirect_triggers_the_rule_via_redirect_to() {
        // Edge case: the status stayed 200 but the engine resolved the target. Either one is
        // enough.
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/vieja", Some(200));
        url(&conn, 3, "https://fixture.local/nueva", Some(200));
        redirige(&conn, 2, 3);
        page(&conn, 1, Some("https://fixture.local/vieja"), "https://fixture.local/a", None, false);
        assert_eq!(ids(&CanonToRedirect.evaluate(&conn).expect("evaluate")), vec![1]);
    }

    #[test]
    fn a_canonical_to_a_200_without_a_redirect_does_not_trigger_the_rule() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        assert!(CanonToRedirect.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_canonical_to_a_noindexed_page_triggers_the_rule() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(
            &conn,
            2,
            Some("https://fixture.local/b"),
            "https://fixture.local/b",
            Some("noindex, follow"),
            false,
        );

        let hallazgos = CanonToNoindex.evaluate(&conn).expect("evaluate");
        assert_eq!(ids(&hallazgos), vec![1]);
        assert_eq!(hallazgos[0].1.severity, Severity::Critical);
    }

    #[test]
    fn the_target_noindex_is_recognised_in_uppercase() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(
            &conn,
            2,
            Some("https://fixture.local/b"),
            "https://fixture.local/b",
            Some("NOINDEX"),
            false,
        );
        assert_eq!(ids(&CanonToNoindex.evaluate(&conn).expect("evaluate")), vec![1]);
    }

    #[test]
    fn a_canonical_to_an_indexable_page_does_not_trigger_the_noindex_rule() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(
            &conn,
            2,
            Some("https://fixture.local/b"),
            "https://fixture.local/b",
            Some("index, follow"),
            true,
        );
        assert!(CanonToNoindex.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_canonical_chain_triggers_the_rule_on_the_first_link() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        url(&conn, 3, "https://fixture.local/c", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, Some("https://fixture.local/c"), "https://fixture.local/b", None, false);
        page(&conn, 3, Some("https://fixture.local/c"), "https://fixture.local/c", None, true);

        let hallazgos = CanonChain.evaluate(&conn).expect("evaluate");
        assert_eq!(ids(&hallazgos), vec![1], "only A is in a chain; B already points at the end");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
    }

    #[test]
    fn a_canonical_loop_is_also_a_chain() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, Some("https://fixture.local/a"), "https://fixture.local/b", None, false);
        assert_eq!(ids(&CanonChain.evaluate(&conn).expect("evaluate")).len(), 2);
    }

    #[test]
    fn a_single_hop_canonical_is_not_a_chain() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, Some("https://fixture.local/b"), "https://fixture.local/b", None, true);
        assert!(CanonChain.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_target_without_a_canonical_forms_no_chain() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        assert!(CanonChain.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn two_pages_with_identical_html_trigger_the_duplicate_rule() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, None, "https://fixture.local/a", None, true);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        // `page` derives the `html_hash` from the `url_id`: equalise them by hand.
        conn.execute("UPDATE pages SET html_hash = 42", []).expect("equalise the hashes");

        let hallazgos = DupContentExact.evaluate(&conn).expect("evaluate");
        assert_eq!(ids(&hallazgos).len(), 2, "the finding is recorded on both pages");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
        assert_eq!(
            hallazgos[0].1.group_key, hallazgos[1].1.group_key,
            "both copies share the group_key so the UI can present them together"
        );
    }

    #[test]
    fn two_pages_with_different_html_are_not_duplicates() {
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, None, "https://fixture.local/a", None, true);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        assert!(DupContentExact.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn an_already_canonicalised_copy_does_not_count_as_a_duplicate() {
        // Edge case and the reason for the `is_indexable` filter: if one copy canonicalises to
        // the other, the problem is already solved and reporting it would be noise over work
        // already done.
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/a?utm_x=1", Some(200));
        let a = "https://fixture.local/a";
        page(&conn, 1, Some(a), a, None, true);
        page(&conn, 2, Some(a), "https://fixture.local/a?utm_x=1", None, false);
        conn.execute("UPDATE pages SET html_hash = 42", []).expect("equalise the hashes");
        assert!(DupContentExact.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_page_without_an_html_hash_is_no_ones_duplicate() {
        // A 404 or a PDF has no `pages` row, but a truncated HTML document can end up without a
        // hash. `NULL` does not group with `NULL`.
        let conn = db();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, None, "https://fixture.local/a", None, true);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        conn.execute("UPDATE pages SET html_hash = NULL", []).expect("clear the hashes");
        assert!(DupContentExact.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn site_rules_produce_no_findings_on_an_empty_crawl() {
        let conn = db();
        for regla in site_rules() {
            assert!(
                regla.evaluate(&conn).expect("evaluate").is_empty(),
                "{} reports findings on an empty database",
                regla.id()
            );
        }
        assert!(CanonToRedirect.evaluate(&conn).expect("evaluate").is_empty());
    }
}
