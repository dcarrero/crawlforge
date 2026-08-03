//! `CANON` y `DUP` — canonical y contenido duplicado. `docs/04-CATALOGO-REGLAS.md §5`.
//!
//! # El `JOIN` de las reglas de conjunto
//!
//! Las cinco reglas de alcance `site` de esta sección necesitan unir la página de origen con la
//! fila de `urls` a la que apunta su canonical. `pages.canonical` guarda la **URL absoluta ya
//! normalizada** (`engine::finish_page` la resuelve contra la URL de la página), y `urls` se
//! indexa por `url_hash`, que es un `xxh3_64` calculado en Rust
//! (`crawlforge_core::engine::url_hash`). SQLite no tiene esa función, así que **el `JOIN` por
//! hash no se puede escribir en SQL puro**: habría que registrar un `create_scalar_function` en
//! la conexión, y una regla no debe modificar la conexión que le presta el motor.
//!
//! La salida es unir por texto: `JOIN urls tgt ON tgt.url = p.canonical`. `urls.url` es `UNIQUE`,
//! por lo que el `JOIN` usa su índice y es igual de selectivo que el del hash. Las dos cadenas
//! salen del mismo normalizador, así que coinciden byte a byte cuando apuntan a la misma URL.

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

// ---------------------------------------------------------------- Utilidades de URL

/// El host de una URL absoluta, sin `userinfo` ni puerto.
///
/// Se hace a mano porque este crate no depende de `url` a propósito: las reglas reciben cadenas
/// ya resueltas por el motor y no deben volver a parsear nada pesado.
fn host_of(url: &str) -> Option<&str> {
    let (_, resto) = url.split_once("://")?;
    let autoridad = resto.split(['/', '?', '#']).next()?;
    // `usuario:clave@host` — el último `@` delimita la autoridad real.
    let sin_userinfo = autoridad.rsplit_once('@').map_or(autoridad, |(_, h)| h);
    // El puerto es lo que va tras el último `:` **y son todos dígitos**. Así un literal IPv6
    // como `[::1]` no se recorta, y `[::1]:8080` sí pierde el puerto.
    let host = match sin_userinfo.rsplit_once(':') {
        Some((h, puerto)) if !puerto.is_empty() && puerto.bytes().all(|b| b.is_ascii_digit()) => h,
        _ => sin_userinfo,
    };
    (!host.is_empty()).then_some(host)
}

/// `www.` inicial fuera. Se compara sin él porque `ejemplo.es` y `www.ejemplo.es` son el mismo
/// sitio: un canonical entre ellos es consolidación de host, no un canonical a otro dominio, y
/// avisarlo como tal sería un falso positivo en el patrón más común que existe.
fn sin_www(host: &str) -> &str {
    if host.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("www.")) {
        &host[4..]
    } else {
        host
    }
}

/// ¿Son el mismo sitio los hosts de estas dos URLs absolutas?
///
/// `None` si alguna de las dos no tiene host reconocible: sin saberlo no se puede afirmar que
/// haya cruce de dominio, y una regla no inventa hallazgos sobre datos que no entiende.
fn same_host(a: &str, b: &str) -> Option<bool> {
    let (ha, hb) = (host_of(a)?, host_of(b)?);
    Some(sin_www(ha).eq_ignore_ascii_case(sin_www(hb)))
}

/// ¿La referencia trae esquema propio (`https:`, `mailto:`) y por tanto es absoluta?
///
/// Por RFC 3986 una referencia es absoluta si y solo si empieza por un esquema. Eso deja
/// `//ejemplo.es/a` —una *network-path reference*— del lado de las relativas, que es lo correcto:
/// depende del esquema de la página que la contiene.
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

// ---------------------------------------------------------------- Reglas de página

/// Página indexable sin `rel=canonical`.
///
/// No es un fallo grave por sí solo —Google infiere el canonical— pero sin él cualquier
/// parámetro de URL puede generar un duplicado. Severidad media, no alta.
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

/// Más de un `link rel=canonical` en la misma página.
///
/// **No se filtra por `is_indexable`.** Si los canonical apuntan a otra URL, el motor marca la
/// página como `canonicalised` y por tanto no indexable; exigir `is_indexable` silenciaría
/// justo los casos peores. Sí se exige un 2xx, como en todas las reglas que auditan el HTML
/// servido: el canonical de la plantilla de error de un 404 no lo procesa ningún buscador, y
/// sin la puerta cada URL rota repetiría los hallazgos del tema. Ver `PageContext::is_success`.
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

/// Canonical declarado como referencia relativa.
///
/// Se mira `canonical_raw` —lo que venía en el HTML— y no `canonical`, que el motor ya ha
/// resuelto a absoluto. Es el único sitio del catálogo donde la forma original importa más que
/// la resuelta.
pub struct CanonRelative;

impl PageRule for CanonRelative {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_RELATIVE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // El 2xx corta la plantilla de error: ver `CanonMultiple` y `PageContext::is_success`.
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

/// Canonical hacia un host distinto del de la página.
///
/// Compara el host del canonical ya resuelto con el de la propia URL. Un `www.` de diferencia no
/// cuenta: ver [`sin_www`].
pub struct CanonCrossDomain;

impl PageRule for CanonCrossDomain {
    fn meta(&self) -> &'static RuleMeta {
        &CANON_CROSS_DOMAIN
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // El 2xx corta la plantilla de error: ver `CanonMultiple` y `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        let Some(canonical) = ctx.canonical.map(str::trim).filter(|c| !c.is_empty()) else {
            return Vec::new();
        };
        // `None` = no se pudo determinar alguno de los dos hosts: no se afirma nada.
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

// ---------------------------------------------------------------- Reglas de conjunto

/// Tronco común de las reglas «el canonical apunta a algo que no debería».
///
/// `tgt.id <> src.id` es la forma de decir «el canonical señala a otra URL». Se prefiere a
/// `pages.canonical_is_self` porque se deduce del propio `JOIN`: si el canonical resuelve a la
/// fila de la propia página, no hay nada que avisar, sin depender de cómo se comparó la cadena
/// al escribirla. El `JOIN` por texto está justificado en la cabecera del módulo.
const CANONICAL_JOIN: &str = "FROM pages p
     JOIN urls src ON src.id = p.url_id
     JOIN urls tgt ON tgt.url = p.canonical
     ";

/// El canonical apunta a una URL que responde 4xx o 5xx.
///
/// El ID dice `4XX` porque es el caso normal, pero la condición normativa del catálogo es «URL
/// con error» y un canonical hacia un 500 es igual de fatal, así que se cubre de 400 arriba. El
/// código real va en el detalle del hallazgo.
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

/// El canonical apunta a una URL que redirige.
///
/// Dos formas de detectarlo, porque el motor guarda las dos cosas: un `status_code` 3xx, y un
/// `redirect_to` resuelto en la pasada del escritor.
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

/// El canonical apunta a una página marcada con `noindex`.
///
/// Se mira la directiva declarada (`meta robots` o `X-Robots-Tag`) y no `is_indexable`: una
/// página puede ser no indexable por media docena de motivos, y cada uno tiene su propia regla.
/// Aquí el hallazgo es la contradicción entre dos señales explícitas.
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

/// A canoniza a B y B canoniza a C.
///
/// **Qué cuenta como cadena:** que el destino del canonical tenga a su vez un canonical que
/// resuelva a una URL distinta de sí mismo. No se exige que C sea distinta de A, así que el
/// bucle `A → B → A` también se avisa: es el mismo defecto y el mismo arreglo. El hallazgo se
/// registra en A, que es la página que hay que corregir.
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

/// Dos o más URLs indexables devuelven un HTML idéntico byte a byte.
///
/// Se restringe a `is_indexable = 1` igual que `META-TITLE-DUPLICATE`: dos copias donde una
/// canoniza a la otra son el arreglo, no el problema, y avisarlas sería ruido sobre trabajo ya
/// hecho.
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
    // `CanonToRedirect` está implementada y probada contra base en memoria, pero **no
    // registrada**: su defecto no se puede provocar con un árbol de ficheros. Los fixtures se
    // rastrean en modo `filesystem`, donde el fetcher solo devuelve 200 o 404, así que
    // `urls.status_code` nunca cae en el rango 3xx y `urls.redirect_to` nunca se rellena. Es el
    // mismo hueco que tienen las reglas `HTTP-REDIRECT-*`.
    //
    // Para activarla hacen falta tres cosas, y ninguna es de este módulo: añadir
    // `Box::new(CanonToRedirect)` a esta lista, escribir `fixtures/CANON-TO-REDIRECT/` con el
    // caso documentado, y declararla en `SIN_FIXTURE_EN_FILESYSTEM`
    // (`crawlforge-core/tests/fixtures_de_reglas.rs`), que es el inventario de lo que este arnés
    // no cubre. Se quita de ahí cuando el caso se pueda provocar con el servidor HTTP de pruebas.
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
    fn no_avisa_cuando_hay_canonical() {
        assert!(CanonMissing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_cuando_falta_el_canonical() {
        let mut c = ctx();
        c.canonical = None;
        c.canonical_raw = None;
        c.canonical_count = 0;
        let issues = CanonMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Medium, "sin canonical no es crítico");
    }

    #[test]
    fn no_avisa_en_una_pagina_no_indexable() {
        let mut c = ctx();
        c.canonical = None;
        c.is_indexable = false;
        assert!(CanonMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn el_canonical_de_la_plantilla_de_error_no_se_audita() {
        // Las tres reglas que no filtran por `is_indexable` exigen un 2xx: el canonical del HTML
        // de un 404 no lo procesa ningún buscador, y sin la puerta la plantilla de error del
        // tema se auditaba una vez por cada URL rota. Ver `PageContext::is_success`.
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            c.canonical_count = 2;
            assert!(
                CanonMultiple.evaluate(&c).is_empty(),
                "CANON-MULTIPLE no debería auditar el HTML de un {status}"
            );

            let mut c = ctx();
            c.status = status;
            c.canonical_raw = Some("/a");
            assert!(
                CanonRelative.evaluate(&c).is_empty(),
                "CANON-RELATIVE no debería auditar el HTML de un {status}"
            );

            let mut c = ctx();
            c.status = status;
            c.canonical = Some("https://otro.com/a");
            assert!(
                CanonCrossDomain.evaluate(&c).is_empty(),
                "CANON-CROSS-DOMAIN no debería auditar el HTML de un {status}"
            );
        }
    }

    // ------------------------------------------------------------ CANON-MULTIPLE

    #[test]
    fn un_solo_canonical_no_es_multiple() {
        assert!(CanonMultiple.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn ningun_canonical_no_es_multiple() {
        let mut c = ctx();
        c.canonical = None;
        c.canonical_raw = None;
        c.canonical_count = 0;
        assert!(CanonMultiple.evaluate(&c).is_empty());
    }

    #[test]
    fn dos_canonical_disparan_la_regla() {
        let mut c = ctx();
        c.canonical_count = 2;
        let issues = CanonMultiple.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CANON-MULTIPLE");
        assert_eq!(issues[0].severity, Severity::High);
        assert_eq!(issues[0].detail_json.as_deref(), Some(r#"{"count":2}"#));
    }

    #[test]
    fn avisa_de_varios_canonical_aunque_la_pagina_no_sea_indexable() {
        // Si los canonical apuntan a otra URL el motor la marca como `canonicalised`, y ese es
        // precisamente el caso que más importa: filtrar por `is_indexable` lo escondería.
        let mut c = ctx();
        c.canonical_count = 3;
        c.canonical = Some("https://ejemplo.es/otra");
        c.is_indexable = false;
        assert_eq!(CanonMultiple.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_avisa_de_varios_canonical_sobre_algo_que_no_es_html() {
        let mut c = ctx();
        c.canonical_count = 2;
        c.is_html = false;
        assert!(CanonMultiple.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ CANON-RELATIVE

    #[test]
    fn un_canonical_absoluto_no_es_relativo() {
        assert!(CanonRelative.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn un_canonical_con_ruta_absoluta_sigue_siendo_relativo() {
        let mut c = ctx();
        c.canonical_raw = Some("/a");
        let issues = CanonRelative.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CANON-RELATIVE");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn un_canonical_relativo_al_documento_dispara_la_regla() {
        let mut c = ctx();
        c.canonical_raw = Some("../otra/");
        assert_eq!(CanonRelative.evaluate(&c).len(), 1);
    }

    #[test]
    fn un_canonical_sin_esquema_es_relativo_aunque_traiga_host() {
        // `//ejemplo.es/a` es una *network-path reference*: hereda el esquema de la página.
        let mut c = ctx();
        c.canonical_raw = Some("//ejemplo.es/a");
        assert_eq!(CanonRelative.evaluate(&c).len(), 1);
    }

    #[test]
    fn un_canonical_en_http_no_se_considera_relativo() {
        let mut c = ctx();
        c.canonical_raw = Some("http://ejemplo.es/a");
        assert!(CanonRelative.evaluate(&c).is_empty());
    }

    #[test]
    fn sin_canonical_no_hay_nada_que_decir_del_relativo() {
        let mut c = ctx();
        c.canonical = None;
        c.canonical_raw = None;
        assert!(CanonRelative.evaluate(&c).is_empty());
    }

    #[test]
    fn un_canonical_de_solo_espacios_no_cuenta_como_relativo() {
        // Un `href=""` o con espacios es otro defecto: para esta regla no hay referencia.
        let mut c = ctx();
        c.canonical_raw = Some("   ");
        assert!(CanonRelative.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ CANON-CROSS-DOMAIN

    #[test]
    fn un_canonical_al_mismo_host_no_cruza_dominio() {
        assert!(CanonCrossDomain.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn un_canonical_a_otro_dominio_dispara_la_regla() {
        let mut c = ctx();
        c.canonical = Some("https://otrodominio.example/a");
        let issues = CanonCrossDomain.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CANON-CROSS-DOMAIN");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn el_www_no_cuenta_como_otro_dominio() {
        let mut c = ctx();
        c.canonical = Some("https://www.ejemplo.es/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());

        let mut c = PageContext::indexable_html("https://www.ejemplo.es/a");
        c.canonical = Some("https://ejemplo.es/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    #[test]
    fn un_subdominio_si_cuenta_como_otro_dominio() {
        // Para la canonicalización de Google `blog.ejemplo.es` es otro host, no otra sección.
        let mut c = ctx();
        c.canonical = Some("https://blog.ejemplo.es/a");
        assert_eq!(CanonCrossDomain.evaluate(&c).len(), 1);
    }

    #[test]
    fn el_puerto_y_el_esquema_no_hacen_que_cruce_dominio() {
        let mut c = ctx();
        c.canonical = Some("http://ejemplo.es:8080/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    #[test]
    fn el_host_se_compara_sin_distinguir_mayusculas() {
        let mut c = ctx();
        c.canonical = Some("https://EJEMPLO.ES/a");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    #[test]
    fn un_canonical_sin_host_reconocible_no_produce_hallazgo() {
        // El motor entrega el canonical ya resuelto a absoluto; si aun así no se le ve el host,
        // la regla calla en vez de inventarse un cruce de dominio.
        let mut c = ctx();
        c.canonical = Some("mailto:hola@ejemplo.es");
        assert!(CanonCrossDomain.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ Utilidades

    #[test]
    fn el_host_se_extrae_de_una_url_absoluta() {
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
    fn el_esquema_se_detecta_como_lo_dice_la_rfc_3986() {
        assert!(tiene_esquema("https://ejemplo.es/a"));
        assert!(tiene_esquema("HTTP://ejemplo.es/a"));
        assert!(tiene_esquema("mailto:hola@ejemplo.es"));
        assert!(!tiene_esquema("//ejemplo.es/a"));
        assert!(!tiene_esquema("/a"));
        assert!(!tiene_esquema("a/b"));
        assert!(!tiene_esquema(""));
        // Un `:` dentro de la ruta no es un esquema.
        assert!(!tiene_esquema("/a/b:c"));
        assert!(!tiene_esquema("2ejemplo:/a"), "un esquema no empieza por dígito");
    }

    // ------------------------------------------------------------ Reglas de conjunto

    /// Una base en memoria con el esquema real. Se carga la migración publicada en vez de un
    /// `CREATE TABLE` a mano: así un cambio de esquema rompe estos tests en vez de dejarlos
    /// midiendo una tabla que ya no existe.
    fn bd() -> Connection {
        let conn = Connection::open_in_memory().expect("abrir en memoria");
        // **Todas** las migraciones, no solo la 001. Se quedó en la inicial y por eso el índice
        // de la 006 no existía aquí: un test sobre un esquema que ya no es el que se despliega
        // mide otra cosa. Al añadir una migración nueva, añádela también a esta lista.
        for sql in [
            include_str!("../../crawlforge-core/migrations/001_initial.sql"),
            include_str!("../../crawlforge-core/migrations/002_truncated.sql"),
            include_str!("../../crawlforge-core/migrations/003_orphans_exclude_seed.sql"),
            include_str!("../../crawlforge-core/migrations/004_robots_y_sitemaps.sql"),
            include_str!("../../crawlforge-core/migrations/005_orphans_solo_paginas.sql"),
            include_str!("../../crawlforge-core/migrations/006_indice_html_hash.sql"),
            include_str!("../../crawlforge-core/migrations/007_indice_images_src.sql"),
        ] {
            conn.execute_batch(sql).expect("cargar el esquema");
        }
        conn
    }

    /// La consulta de contenido duplicado no puede ordenar la tabla entera.
    ///
    /// Sin el índice de la migración 006, SQLite resuelve sus dos agrupaciones por `html_hash`
    /// con B-trees temporales sobre toda la tabla. **Medido el 2026-08-02 rastreando
    /// un medio de comunicación entero: más de ocho horas en esta única regla**, sobre 216.349
    /// páginas en un fichero de 5,3 GB, cuando el rastreo de 487.621 URLs había terminado en
    /// nueve horas y media.
    ///
    /// Se afirma sobre el plan y no sobre el tiempo porque a la escala que cabe en un test el
    /// reloj no distingue los dos mundos. Lo que distingue es que la agrupación tenga índice
    /// donde apoyarse — y este test es además el que descartó la primera forma del índice, un
    /// parcial sobre `html_hash` que SQLite ni llegaba a usar.
    #[test]
    fn la_deteccion_de_duplicados_tiene_indice_donde_agrupar() {
        let conn = bd();
        let existe: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_pages_html_hash'",
                [],
                |r| r.get(0),
            )
            .expect("consultar sqlite_master");
        assert_eq!(existe, 1, "falta el índice sobre pages(html_hash) de la migración 006");

        // Y que sea utilizable por la subconsulta que agrupa, que es la que costaba las horas.
        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT html_hash FROM pages
                 WHERE is_indexable = 1 AND html_hash IS NOT NULL
                 GROUP BY html_hash HAVING COUNT(*) > 1",
            )
            .expect("preparar el plan");
        let plan: String = stmt
            .query_map([], |r| r.get::<_, String>(3))
            .expect("leer el plan")
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(
            plan.contains("idx_pages_html_hash"),
            "la agrupación por html_hash debe apoyarse en su índice, y el plan dice: {plan}"
        );
    }

    /// Inserta una URL. El `url_hash` se hace igual al `id` para que el test pueda comprobar
    /// sobre qué fila se registró el hallazgo.
    fn url(conn: &Connection, id: i64, u: &str, status: Option<i64>) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', 'fixture.local', '/', 1, 0, 'done', ?3)",
            rusqlite::params![id, u, status],
        )
        .expect("insertar url");
    }

    fn redirige(conn: &Connection, id: i64, hacia: i64) {
        conn.execute("UPDATE urls SET redirect_to = ?2 WHERE id = ?1", [id, hacia])
            .expect("marcar redirección");
    }

    /// Inserta la página de una URL. `canonical` en `None` es «sin etiqueta».
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
             VALUES (?1, 'Una página', ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                url_id,
                canonical,
                canonical.map(|c| c == propia),
                robots,
                indexable as i64,
                url_id * 1000,
            ],
        )
        .expect("insertar página");
    }

    fn ids(hallazgos: &[(Option<i64>, Issue)]) -> Vec<i64> {
        hallazgos.iter().filter_map(|(h, _)| *h).collect()
    }

    #[test]
    fn el_canonical_a_un_404_dispara_la_regla() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/roto", Some(404));
        page(&conn, 1, Some("https://fixture.local/roto"), "https://fixture.local/a", None, false);

        let hallazgos = CanonToError.evaluate(&conn).expect("evaluar");
        assert_eq!(ids(&hallazgos), vec![1], "el hallazgo va en la página de origen");
        assert_eq!(hallazgos[0].1.severity, Severity::Critical);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("404"), "el detalle lleva el código real: {detalle}");
    }

    #[test]
    fn el_canonical_a_un_500_tambien_dispara_la_regla() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/roto", Some(500));
        page(&conn, 1, Some("https://fixture.local/roto"), "https://fixture.local/a", None, false);
        assert_eq!(ids(&CanonToError.evaluate(&conn).expect("evaluar")), vec![1]);
    }

    #[test]
    fn el_canonical_a_un_200_no_dispara_la_regla_de_error() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        assert!(CanonToError.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn un_canonical_a_si_misma_no_dispara_ninguna_regla_de_destino() {
        // Caso límite: la propia página responde 404 y se canoniza a sí misma. No es un
        // canonical roto, es un 404, y de eso avisa otra regla.
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(404));
        page(&conn, 1, Some("https://fixture.local/a"), "https://fixture.local/a", None, false);
        assert!(CanonToError.evaluate(&conn).expect("evaluar").is_empty());
        assert!(CanonChain.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn un_canonical_a_una_url_no_rastreada_no_produce_hallazgo() {
        // Sin fila de destino no hay nada que afirmar: el canonical puede ser correcto y estar
        // fuera del alcance del rastreo.
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        page(&conn, 1, Some("https://otro.example/b"), "https://fixture.local/a", None, false);
        assert!(CanonToError.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn el_canonical_a_una_redireccion_dispara_la_regla_por_el_codigo() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/vieja", Some(301));
        url(&conn, 3, "https://fixture.local/nueva", Some(200));
        redirige(&conn, 2, 3);
        page(&conn, 1, Some("https://fixture.local/vieja"), "https://fixture.local/a", None, false);

        let hallazgos = CanonToRedirect.evaluate(&conn).expect("evaluar");
        assert_eq!(ids(&hallazgos), vec![1]);
        assert_eq!(hallazgos[0].1.severity, Severity::High);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("/nueva"), "el detalle dice a dónde acaba: {detalle}");
    }

    #[test]
    fn el_canonical_a_una_redireccion_dispara_la_regla_por_redirect_to() {
        // Caso límite: el código quedó en 200 pero el motor resolvió el destino. Basta uno.
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/vieja", Some(200));
        url(&conn, 3, "https://fixture.local/nueva", Some(200));
        redirige(&conn, 2, 3);
        page(&conn, 1, Some("https://fixture.local/vieja"), "https://fixture.local/a", None, false);
        assert_eq!(ids(&CanonToRedirect.evaluate(&conn).expect("evaluar")), vec![1]);
    }

    #[test]
    fn el_canonical_a_un_200_sin_redireccion_no_dispara_la_regla() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        assert!(CanonToRedirect.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn el_canonical_a_una_pagina_con_noindex_dispara_la_regla() {
        let conn = bd();
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

        let hallazgos = CanonToNoindex.evaluate(&conn).expect("evaluar");
        assert_eq!(ids(&hallazgos), vec![1]);
        assert_eq!(hallazgos[0].1.severity, Severity::Critical);
    }

    #[test]
    fn el_noindex_del_destino_se_reconoce_en_mayusculas() {
        let conn = bd();
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
        assert_eq!(ids(&CanonToNoindex.evaluate(&conn).expect("evaluar")), vec![1]);
    }

    #[test]
    fn el_canonical_a_una_pagina_indexable_no_dispara_la_regla_de_noindex() {
        let conn = bd();
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
        assert!(CanonToNoindex.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_cadena_de_canonical_dispara_la_regla_en_el_primer_eslabon() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        url(&conn, 3, "https://fixture.local/c", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, Some("https://fixture.local/c"), "https://fixture.local/b", None, false);
        page(&conn, 3, Some("https://fixture.local/c"), "https://fixture.local/c", None, true);

        let hallazgos = CanonChain.evaluate(&conn).expect("evaluar");
        assert_eq!(ids(&hallazgos), vec![1], "solo A está en cadena; B ya apunta al final");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
    }

    #[test]
    fn un_bucle_de_canonical_tambien_es_una_cadena() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, Some("https://fixture.local/a"), "https://fixture.local/b", None, false);
        assert_eq!(ids(&CanonChain.evaluate(&conn).expect("evaluar")).len(), 2);
    }

    #[test]
    fn un_canonical_de_un_solo_salto_no_es_cadena() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, Some("https://fixture.local/b"), "https://fixture.local/b", None, true);
        assert!(CanonChain.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn un_destino_sin_canonical_no_forma_cadena() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, Some("https://fixture.local/b"), "https://fixture.local/a", None, false);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        assert!(CanonChain.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn dos_paginas_con_el_mismo_html_disparan_la_regla_de_duplicado() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, None, "https://fixture.local/a", None, true);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        // `page` deriva el `html_hash` del `url_id`: se igualan a mano.
        conn.execute("UPDATE pages SET html_hash = 42", []).expect("igualar el hash");

        let hallazgos = DupContentExact.evaluate(&conn).expect("evaluar");
        assert_eq!(ids(&hallazgos).len(), 2, "el hallazgo se registra en las dos páginas");
        assert_eq!(hallazgos[0].1.severity, Severity::High);
        assert_eq!(
            hallazgos[0].1.group_key, hallazgos[1].1.group_key,
            "las dos copias comparten group_key para que la UI las presente juntas"
        );
    }

    #[test]
    fn dos_paginas_con_html_distinto_no_son_duplicados() {
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, None, "https://fixture.local/a", None, true);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        assert!(DupContentExact.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_copia_ya_canonizada_no_cuenta_como_duplicado() {
        // Caso límite y motivo del filtro por `is_indexable`: si una copia canoniza a la otra,
        // el problema está resuelto y avisarlo sería ruido sobre trabajo ya hecho.
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/a?utm_x=1", Some(200));
        let a = "https://fixture.local/a";
        page(&conn, 1, Some(a), a, None, true);
        page(&conn, 2, Some(a), "https://fixture.local/a?utm_x=1", None, false);
        conn.execute("UPDATE pages SET html_hash = 42", []).expect("igualar el hash");
        assert!(DupContentExact.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_pagina_sin_html_hash_no_es_duplicado_de_nadie() {
        // Un 404 o un PDF no tienen fila en `pages`, pero un HTML truncado puede quedarse sin
        // hash. `NULL` no agrupa con `NULL`.
        let conn = bd();
        url(&conn, 1, "https://fixture.local/a", Some(200));
        url(&conn, 2, "https://fixture.local/b", Some(200));
        page(&conn, 1, None, "https://fixture.local/a", None, true);
        page(&conn, 2, None, "https://fixture.local/b", None, true);
        conn.execute("UPDATE pages SET html_hash = NULL", []).expect("borrar el hash");
        assert!(DupContentExact.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn las_reglas_de_conjunto_no_avisan_sobre_un_rastreo_vacio() {
        let conn = bd();
        for regla in site_rules() {
            assert!(
                regla.evaluate(&conn).expect("evaluar").is_empty(),
                "{} avisa sobre una base vacía",
                regla.id()
            );
        }
        assert!(CanonToRedirect.evaluate(&conn).expect("evaluar").is_empty());
    }
}
