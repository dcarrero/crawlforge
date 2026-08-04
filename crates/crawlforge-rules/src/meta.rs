//! `META` — titles and meta descriptions. `docs/04-CATALOGO-REGLAS.md §4`.
//!
//! This module is the template for the rest: [`MetaTitleMissing`] is the example page rule and
//! [`MetaTitleDuplicate`] the example site rule.
//!
//! The "too long" thresholds are **estimated width in pixels**, not character counts, because
//! that is how Google truncates. The advance table and the error attributed to it are in
//! [`arial_advance_per_mille`].

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::Connection;

pub static META_TITLE_MISSING: RuleMeta = RuleMeta {
    id: "META-TITLE-MISSING",
    severity: Severity::Critical,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin título",
    name_en: "Missing title",
    desc_es: "La página es indexable y no tiene <title>, o lo tiene vacío. Es el factor on-page \
              con más peso que se controla directamente, y es el texto en azul del resultado de \
              búsqueda: sin él, Google inventa uno con lo que encuentra en la página.",
    desc_en: "The page is indexable and has no <title>, or an empty one. It is the on-page \
              factor with the most weight that you control directly, and it is the blue text in \
              the search result: without it, Google makes one up from whatever it finds.",
    references: &[],
};

pub static META_TITLE_DUPLICATE: RuleMeta = RuleMeta {
    id: "META-TITLE-DUPLICATE",
    severity: Severity::High,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Título duplicado",
    name_en: "Duplicate title",
    desc_es: "Dos o más páginas indexables comparten el mismo <title>. Google tiene que elegir \
              cuál de ellas mostrar para la misma consulta, y compiten entre sí en vez de \
              sumar. Es el síntoma más común de paginación o de archivos mal configurados.",
    desc_en: "Two or more indexable pages share the same <title>. Google has to pick which one \
              to show for the same query, so they compete with each other instead of adding up. \
              It is the most common symptom of pagination or misconfigured archives.",
    references: &[],
};

pub static META_TITLE_TOO_LONG: RuleMeta = RuleMeta {
    id: "META-TITLE-TOO-LONG",
    severity: Severity::Medium,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Título demasiado ancho",
    name_en: "Title too long",
    desc_es: "El título no cabe en el resultado de búsqueda y Google lo corta con puntos \
              suspensivos, así que la parte final —muchas veces la marca, o la palabra clave que \
              se dejó para el final— no se lee. Se mide en píxeles, no en caracteres: el corte lo \
              decide el ancho renderizado.",
    desc_en: "The title does not fit in the search result and Google truncates it with an \
              ellipsis, so the tail —often the brand, or the keyword left for last— is never \
              read. It is measured in pixels, not characters: the cut is decided by rendered \
              width.",
    references: &[],
};

pub static META_TITLE_TOO_SHORT: RuleMeta = RuleMeta {
    id: "META-TITLE-TOO-SHORT",
    severity: Severity::Low,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Título demasiado corto",
    name_en: "Title too short",
    desc_es: "El título deja sin usar buena parte del espacio que el resultado de búsqueda \
              ofrece. No es un error, es espacio desaprovechado: cabe una variante de la \
              consulta o un motivo para hacer clic, y a menudo indica una plantilla que solo \
              imprime el nombre de la sección.",
    desc_en: "The title leaves most of the space the search result offers unused. It is not an \
              error but wasted space: there is room for a query variant or a reason to click, \
              and it often points to a template that only prints the section name.",
    references: &[],
};

pub static META_TITLE_MULTIPLE: RuleMeta = RuleMeta {
    id: "META-TITLE-MULTIPLE",
    severity: Severity::Medium,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Varias etiquetas de título",
    name_en: "Multiple title tags",
    desc_es: "La página trae más de un <title>. El navegador y Google se quedan con el primero y \
              descartan el resto en silencio, de modo que el título que se escribió con cuidado \
              puede no ser el que se publica. Casi siempre es una plantilla que lo imprime dos \
              veces, o un plugin de SEO que añade el suyo sin quitar el del tema.",
    desc_en: "The page carries more than one <title>. Browsers and Google keep the first and \
              silently discard the rest, so the title that was written with care may not be the \
              one published. It is nearly always a template printing it twice, or an SEO plugin \
              adding its own without removing the theme's.",
    references: &[],
};

pub static META_DESC_MISSING: RuleMeta = RuleMeta {
    id: "META-DESC-MISSING",
    severity: Severity::High,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin meta descripción",
    name_en: "Missing meta description",
    desc_es: "La página indexable no declara descripción, así que Google recorta un fragmento \
              cualquiera del cuerpo para el resultado de búsqueda. No es un factor de \
              posicionamiento, pero sí el texto que decide el clic, y renunciar a escribirlo es \
              renunciar a controlarlo.",
    desc_en: "The indexable page declares no description, so Google clips an arbitrary snippet \
              of the body for the search result. It is not a ranking factor, but it is the text \
              that decides the click, and not writing it means not controlling it.",
    references: &[],
};

pub static META_DESC_DUPLICATE: RuleMeta = RuleMeta {
    id: "META-DESC-DUPLICATE",
    severity: Severity::Medium,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Meta descripción duplicada",
    name_en: "Duplicate meta description",
    desc_es: "Dos o más páginas indexables repiten la misma descripción, con lo que el resultado \
              de búsqueda no distingue entre ellas y ninguna promete nada concreto. Suele venir \
              de una descripción por defecto puesta a nivel de sitio que nadie sobrescribió.",
    desc_en: "Two or more indexable pages repeat the same description, so the search result does \
              not tell them apart and none of them promises anything specific. It usually comes \
              from a site-wide default nobody overrode.",
    references: &[],
};

pub static META_DESC_TOO_LONG: RuleMeta = RuleMeta {
    id: "META-DESC-TOO-LONG",
    severity: Severity::Low,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Meta descripción demasiado ancha",
    name_en: "Meta description too long",
    desc_es: "La descripción no cabe en el resultado de búsqueda y se corta, de modo que la \
              llamada a la acción del final no llega a leerse. Se mide en píxeles, no en \
              caracteres, porque el corte lo decide el ancho renderizado.",
    desc_en: "The description does not fit in the search result and gets truncated, so the call \
              to action at the end is never read. It is measured in pixels, not characters, \
              because the cut is decided by rendered width.",
    references: &[],
};

pub static META_DESC_TOO_SHORT: RuleMeta = RuleMeta {
    id: "META-DESC-TOO-SHORT",
    severity: Severity::Low,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Meta descripción demasiado corta",
    name_en: "Meta description too short",
    desc_es: "La descripción es tan breve que Google tiende a ignorarla y a componer el fragmento \
              con texto del cuerpo. Con tan poco espacio usado no cabe ni el argumento ni la \
              variante de la consulta que justificarían el clic.",
    desc_en: "The description is so brief that Google tends to ignore it and build the snippet \
              from body text instead. With so little space used there is room for neither the \
              argument nor the query variant that would justify the click.",
    references: &[],
};

pub static META_VIEWPORT_MISSING: RuleMeta = RuleMeta {
    id: "META-VIEWPORT-MISSING",
    severity: Severity::High,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin meta viewport",
    name_en: "Missing meta viewport",
    desc_es: "Sin <meta name=viewport> el móvil dibuja la página a 980 px de ancho y la reduce, \
              así que el texto queda ilegible y hay que hacer zoom. La indexación es móvil \
              primero: lo que Google evalúa es esa versión encogida.",
    desc_en: "Without <meta name=viewport> a phone lays the page out at 980 px wide and scales it \
              down, leaving text unreadable and forcing pinch-zoom. Indexing is mobile-first: \
              what Google evaluates is that shrunken version.",
    references: &[],
};

pub static META_REFRESH: RuleMeta = RuleMeta {
    id: "META-REFRESH",
    severity: Severity::High,
    category: Category::Meta,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Redirección por meta refresh",
    name_en: "Meta refresh redirect",
    desc_es: "La página redirige con <meta http-equiv=refresh> en lugar de con un 301. Google lo \
              interpreta con reservas, no traslada la autoridad con la misma fiabilidad y el \
              usuario ve un salto brusco; con un retardo distinto de cero, además, atrapa el \
              botón de volver del navegador.",
    desc_en: "The page redirects with <meta http-equiv=refresh> instead of a 301. Google reads it \
              with reservations, does not pass authority as reliably, and the user sees an abrupt \
              jump; with a non-zero delay it also traps the browser's back button.",
    references: &[],
};

// ---------------------------------------------------------------- Pixel width
//
// Google does not cut the title or the description at a character count: it cuts them when
// they do not fit in the width available to the result. Counting characters gives false
// warnings in both directions —a title of sixty i's fits with room to spare and one of
// forty-five m's does not— and in Spanish the error is bigger: words are longer, capitals and
// accents are more frequent, and "ÁRBOL" takes up 50% more than "árbol" with the same five
// characters.

/// Size Google renders the result title at on desktop.
const TITLE_FONT_PX: f64 = 20.0;
/// Size Google renders the description snippet at on desktop.
const DESC_FONT_PX: f64 = 14.0;

/// Maximum title width before it gets cut, in pixels. `docs/04-CATALOGO-REGLAS.md §4`.
pub const TITLE_MAX_WIDTH_PX: f64 = 580.0;
/// Maximum description width before it gets cut, in pixels.
pub const DESC_MAX_WIDTH_PX: f64 = 990.0;

/// Below this the title wastes the space the result offers. In characters on purpose: the
/// warning is "you are missing text to write", and that is counted, not measured.
pub const TITLE_MIN_CHARS: usize = 30;
/// Below this the description wastes the snippet's space.
pub const DESC_MIN_CHARS: usize = 70;

/// Advance width of a character in Arial Regular, in thousandths of an em.
///
/// **Where the table comes from:** these are the advance widths of Arial Regular, expressed in
/// the unit AFM files use (thousandths of an em, the same scale as the `hmtx` table of a
/// TrueType with 1000 units per em). Arial was designed as a metrically compatible substitute
/// for Helvetica, so across the whole ASCII range it shares advances with the Helvetica of the
/// fourteen PostScript base fonts, whose metrics are public. Characters are grouped in classes
/// because within each class the advance is literally the same value: `A`, `B`, `E`, `K`, `P`,
/// `S`, `V`, `X` and `Y` measure 667, all nine of them.
///
/// **What error it carries:**
///
/// - On normal Latin prose it stays under 2% of what a browser measures. What it ignores is
///   *kerning* —Arial pulls pairs like `AV` or `To` together— and the rasterizer's subpixel
///   rounding, and both subtract width, so the estimate errs upward: it warns a little early,
///   never a little late.
/// - Accented vowels and `ñ` measure the same as their base letter, which is exact in Arial.
/// - Outside Latin-1 —CJK, emoji, mathematical symbols— everything falls into the default value
///   and the error is large. Accepted: Google does not render a Japanese title in Arial, and
///   the rule does not pretend to be a text layout engine.
///
/// The core has an equivalent function to fill the `pages.title_px` and `pages.meta_desc_px`
/// columns. It is duplicated because the dependency goes from the core to the rules and not the
/// other way around; [`estimated_width_px`] is `pub` so a future consolidation can go in that
/// direction and not the opposite one.
fn arial_advance_per_mille(c: char) -> u64 {
    match c {
        // Narrow class: the only things below a quarter of an em.
        'i' | 'j' | 'l' | 'í' | 'ì' | 'î' | 'ï' => 222,
        // The typographic `'` is even narrower (191), but does not deserve a class of its own.
        '\'' | '’' => 191,
        ' ' | '.' | ',' | ':' | ';' | '!' | '|' | '/' | '\\' | '[' | ']' | 'f' | 't' | 'I'
        | 'Í' | 'Ï' => 278,
        '(' | ')' | '{' | '}' | '-' | '–' | '`' | 'r' | '¡' => 333,
        '"' | '“' | '”' | '*' => 355,
        'c' | 'k' | 's' | 'v' | 'x' | 'y' | 'z' | 'J' | 'ç' => 500,
        '+' | '=' | '<' | '>' | '~' => 584,
        'F' | 'T' | 'Z' => 611,
        'A' | 'B' | 'E' | 'K' | 'P' | 'S' | 'V' | 'X' | 'Y' | '&' | 'Á' | 'À' | 'Â' | 'Ä' | 'É'
        | 'È' | 'Ê' | 'Ë' => 667,
        'C' | 'D' | 'H' | 'N' | 'R' | 'U' | 'w' | 'Ç' | 'Ñ' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 722,
        'G' | 'O' | 'Q' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 778,
        'm' | 'M' => 833,
        '%' => 889,
        'W' => 944,
        '@' => 1015,
        // Remaining lowercase, the digits and the rest of the medium punctuation. Also the
        // value that characters unknown to the table receive.
        _ => 556,
    }
}

/// Estimated width of a text rendered in Arial at the given size, in pixels.
pub fn estimated_width_px(text: &str, font_size_px: f64) -> f64 {
    let por_millar: u64 = text.chars().map(arial_advance_per_mille).sum();
    por_millar as f64 * font_size_px / 1000.0
}

/// Estimated width of a title in the search result (Arial 20 px).
pub fn title_width_px(title: &str) -> f64 {
    estimated_width_px(title.trim(), TITLE_FONT_PX)
}

/// Estimated width of a meta description in the snippet (Arial 14 px).
pub fn description_width_px(description: &str) -> f64 {
    estimated_width_px(description.trim(), DESC_FONT_PX)
}

/// Indexable page without a `<title>`.
///
/// It only applies to indexable pages: warning about the title of a `noindex` page is noise.
pub struct MetaTitleMissing;

impl PageRule for MetaTitleMissing {
    fn meta(&self) -> &'static RuleMeta {
        &META_TITLE_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        let vacio = ctx.title.map(|t| t.trim().is_empty()).unwrap_or(true);
        if !vacio {
            return Vec::new();
        }
        vec![Issue::new(&META_TITLE_MISSING)]
    }
}

/// The base path of a pagination URL, if the path is one.
///
/// It recognizes exactly the `/page/<n>` or `/pagina/<n>` suffix, with or without a trailing
/// slash: the two forms seen in real crawls (WordPress's default permalink in English and its
/// Spanish translation). **Deliberately not a long list of patterns**: every new pattern is a
/// chance to downgrade a real duplicate, so only what a crawl has demonstrated gets in.
/// `/category/x/page/2/` → `Some("/category/x")`; `/category/x/` → `None`.
fn pagination_base(path: &str) -> Option<&str> {
    let sin_barra = path.trim_end_matches('/');
    let (resto, numero) = sin_barra.rsplit_once('/')?;
    if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (base, segmento) = resto.rsplit_once('/')?;
    (segmento == "page" || segmento == "pagina").then_some(base)
}

/// Two or more indexable pages share the same `<title>`.
///
/// It needs the full crawl: it is the canonical example of why [`SiteRule`] exists. The
/// `group_key` groups the pages sharing a title so the UI can present them together.
///
/// **The paginated series of a single archive drops to `low`.** The fact is true —`/category/x/`
/// and its `/page/N/` share a title— but it is what WordPress produces out of the box on every
/// paginated archive, and in a real crawl those series were 38 of the rule's 40 `high`
/// findings: a high warning that fires on every WordPress in the world stops being read, and
/// with it the duplicates that do compete. The condition is strict: **all** pages in the group
/// must collapse to the same base once the pagination suffix is removed ([`pagination_base`]);
/// if the title is shared by two different archives, or by an archive and an article, the whole
/// group keeps its severity. The detail declares it with `pagination_series`.
pub struct MetaTitleDuplicate;

impl SiteRule for MetaTitleDuplicate {
    fn meta(&self) -> &'static RuleMeta {
        &META_TITLE_DUPLICATE
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, p.title, COUNT(*) OVER (PARTITION BY p.title) AS n, u.path
             FROM pages p
             JOIN urls u ON u.id = p.url_id
             WHERE p.is_indexable = 1 AND p.title IS NOT NULL AND TRIM(p.title) <> ''
             AND p.title IN (
                 SELECT title FROM pages
                 WHERE is_indexable = 1 AND title IS NOT NULL AND TRIM(title) <> ''
                 GROUP BY title HAVING COUNT(*) > 1
             )",
        )?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;

        let mut filas: Vec<(i64, String, i64, String)> = Vec::new();
        for row in rows {
            let (hash, title, n, path) = row?;
            filas.push((hash, title, n, path.unwrap_or_default()));
        }

        // A group is a paginated series if all its paths collapse to the same base once the
        // `/page/N` suffix is removed, and at least one carried it. Computed per title, not per
        // row.
        let mut bases: std::collections::HashMap<&str, (Option<&str>, bool)> =
            std::collections::HashMap::new();
        for (_, title, _, path) in &filas {
            let base = pagination_base(path);
            let normalizada = base.unwrap_or_else(|| path.trim_end_matches('/'));
            let entrada = bases.entry(title.as_str()).or_insert((Some(normalizada), false));
            if entrada.0 != Some(normalizada) {
                entrada.0 = None; // two distinct bases: not a single series
            }
            entrada.1 |= base.is_some(); // some page in the group is a /page/N
        }

        let mut out = Vec::new();
        for (hash, title, n, _) in &filas {
            let es_serie = bases
                .get(title.as_str())
                .is_some_and(|(base, con_paginacion)| base.is_some() && *con_paginacion);
            let mut issue = Issue::new(&META_TITLE_DUPLICATE)
                .with_detail(serde_json::json!({
                    "title": title,
                    "pages": n,
                    "pagination_series": es_serie,
                }))
                .with_group(format!(
                    "title:{:016x}",
                    xxhash_rust::xxh3::xxh3_64(title.as_bytes())
                ));
            if es_serie {
                issue = issue.with_severity(Severity::Low);
            }
            out.push((Some(*hash), issue));
        }
        Ok(out)
    }
}

/// The title does not fit in the search result.
///
/// The threshold is width, not length: see [`arial_advance_per_mille`] for why.
pub struct MetaTitleTooLong;

impl PageRule for MetaTitleTooLong {
    fn meta(&self) -> &'static RuleMeta {
        &META_TITLE_TOO_LONG
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        let Some(titulo) = titulo_util(ctx) else {
            return Vec::new();
        };
        let ancho = title_width_px(titulo);
        if ancho <= TITLE_MAX_WIDTH_PX {
            return Vec::new();
        }
        vec![Issue::new(&META_TITLE_TOO_LONG).with_detail(serde_json::json!({
            "width_px": ancho.round() as i64,
            "limit_px": TITLE_MAX_WIDTH_PX as i64,
            "chars": titulo.chars().count(),
        }))]
    }
}

/// The title leaves the search result's space unused.
///
/// Characters are counted here, deliberately: the advice is "write more text", and a warning in
/// pixels would force explaining to the user how many letters they are short of.
pub struct MetaTitleTooShort;

impl PageRule for MetaTitleTooShort {
    fn meta(&self) -> &'static RuleMeta {
        &META_TITLE_TOO_SHORT
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        let Some(titulo) = titulo_util(ctx) else {
            return Vec::new();
        };
        let caracteres = titulo.chars().count();
        if caracteres >= TITLE_MIN_CHARS {
            return Vec::new();
        }
        vec![Issue::new(&META_TITLE_TOO_SHORT).with_detail(serde_json::json!({
            "chars": caracteres,
            "min_chars": TITLE_MIN_CHARS,
            "width_px": title_width_px(titulo).round() as i64,
        }))]
    }
}

/// More than one `<title>` tag on the page.
///
/// The counting is done by the parser, which already discards the `<title>` of an `<svg>`:
/// that one is the accessible name of an icon, not a page title.
pub struct MetaTitleMultiple;

impl PageRule for MetaTitleMultiple {
    fn meta(&self) -> &'static RuleMeta {
        &META_TITLE_MULTIPLE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable || ctx.title_count <= 1 {
            return Vec::new();
        }
        vec![Issue::new(&META_TITLE_MULTIPLE)
            .with_detail(serde_json::json!({ "titles": ctx.title_count }))]
    }
}

/// Indexable page without a meta description.
pub struct MetaDescMissing;

impl PageRule for MetaDescMissing {
    fn meta(&self) -> &'static RuleMeta {
        &META_DESC_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        let vacia = ctx.meta_description.map(|d| d.trim().is_empty()).unwrap_or(true);
        if !vacia {
            return Vec::new();
        }
        vec![Issue::new(&META_DESC_MISSING)]
    }
}

/// The description does not fit in the result's snippet.
pub struct MetaDescTooLong;

impl PageRule for MetaDescTooLong {
    fn meta(&self) -> &'static RuleMeta {
        &META_DESC_TOO_LONG
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        let Some(descripcion) = descripcion_util(ctx) else {
            return Vec::new();
        };
        let ancho = description_width_px(descripcion);
        if ancho <= DESC_MAX_WIDTH_PX {
            return Vec::new();
        }
        vec![Issue::new(&META_DESC_TOO_LONG).with_detail(serde_json::json!({
            "width_px": ancho.round() as i64,
            "limit_px": DESC_MAX_WIDTH_PX as i64,
            "chars": descripcion.chars().count(),
        }))]
    }
}

/// The description wastes the snippet's space.
pub struct MetaDescTooShort;

impl PageRule for MetaDescTooShort {
    fn meta(&self) -> &'static RuleMeta {
        &META_DESC_TOO_SHORT
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        let Some(descripcion) = descripcion_util(ctx) else {
            return Vec::new();
        };
        let caracteres = descripcion.chars().count();
        if caracteres >= DESC_MIN_CHARS {
            return Vec::new();
        }
        vec![Issue::new(&META_DESC_TOO_SHORT).with_detail(serde_json::json!({
            "chars": caracteres,
            "min_chars": DESC_MIN_CHARS,
        }))]
    }
}

/// Indexable page without `<meta name="viewport">`.
pub struct MetaViewportMissing;

impl PageRule for MetaViewportMissing {
    fn meta(&self) -> &'static RuleMeta {
        &META_VIEWPORT_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        let ausente = ctx.viewport.map(|v| v.trim().is_empty()).unwrap_or(true);
        if !ausente {
            return Vec::new();
        }
        vec![Issue::new(&META_VIEWPORT_MISSING)]
    }
}

/// The page redirects with `<meta http-equiv="refresh">`.
///
/// The `content` value travels in the detail because the delay changes the diagnosis: `0` is a
/// redirect in disguise and any other number is, on top of that, a trap for the back button.
pub struct MetaRefresh;

impl PageRule for MetaRefresh {
    fn meta(&self) -> &'static RuleMeta {
        &META_REFRESH
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        let Some(contenido) = ctx.meta_refresh.map(str::trim).filter(|c| !c.is_empty()) else {
            return Vec::new();
        };
        vec![Issue::new(&META_REFRESH)
            .with_detail(serde_json::json!({ "content": contenido }))]
    }
}

/// Two or more indexable pages share the same meta description.
///
/// Deliberately modeled on [`MetaTitleDuplicate`]: the same query over another column. The
/// comparison is exact, without normalizing whitespace or case, because two descriptions that
/// only differ in spacing are two distinct descriptions to the index and that has to be visible
/// in the diff.
pub struct MetaDescDuplicate;

impl SiteRule for MetaDescDuplicate {
    fn meta(&self) -> &'static RuleMeta {
        &META_DESC_DUPLICATE
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, p.meta_description,
                    COUNT(*) OVER (PARTITION BY p.meta_description) AS n
             FROM pages p
             JOIN urls u ON u.id = p.url_id
             WHERE p.is_indexable = 1 AND p.meta_description IS NOT NULL
             AND TRIM(p.meta_description) <> ''
             AND p.meta_description IN (
                 SELECT meta_description FROM pages
                 WHERE is_indexable = 1 AND meta_description IS NOT NULL
                 AND TRIM(meta_description) <> ''
                 GROUP BY meta_description HAVING COUNT(*) > 1
             )",
        )?;

        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, descripcion, n) = row?;
            out.push((
                Some(hash),
                Issue::new(&META_DESC_DUPLICATE)
                    .with_detail(serde_json::json!({ "description": descripcion, "pages": n }))
                    .with_group(format!(
                        "desc:{:016x}",
                        xxhash_rust::xxh3::xxh3_64(descripcion.as_bytes())
                    )),
            ));
        }
        Ok(out)
    }
}

/// The title worth judging for length or width.
///
/// `None` when the page is not indexable HTML or when there is no title: [`MetaTitleMissing`]
/// already reports the absence, and adding "and it is short, too" would be the same defect
/// counted twice.
fn titulo_util<'a>(ctx: &PageContext<'a>) -> Option<&'a str> {
    if !ctx.is_html || !ctx.is_indexable {
        return None;
    }
    ctx.title.map(str::trim).filter(|t| !t.is_empty())
}

/// The description worth judging. Same criterion as [`titulo_util`]: [`MetaDescMissing`]
/// reports the absence.
fn descripcion_util<'a>(ctx: &PageContext<'a>) -> Option<&'a str> {
    if !ctx.is_html || !ctx.is_indexable {
        return None;
    }
    ctx.meta_description.map(str::trim).filter(|d| !d.is_empty())
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![
        Box::new(MetaTitleMissing),
        Box::new(MetaTitleTooLong),
        Box::new(MetaTitleTooShort),
        Box::new(MetaTitleMultiple),
        Box::new(MetaDescMissing),
        Box::new(MetaDescTooLong),
        Box::new(MetaDescTooShort),
        Box::new(MetaViewportMissing),
        Box::new(MetaRefresh),
    ]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    vec![Box::new(MetaTitleDuplicate), Box::new(MetaDescDuplicate)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A healthy page to start from. Each test breaks only what it cares about.
    fn ctx<'a>() -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.title = Some("Un título correcto y suficientemente descriptivo");
        c.title_count = 1;
        c
    }

    #[test]
    fn a_page_with_a_title_produces_no_finding() {
        assert!(MetaTitleMissing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_missing_title_produces_a_finding() {
        let mut c = ctx();
        c.title = None;
        let issues = MetaTitleMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-TITLE-MISSING");
        assert_eq!(issues[0].severity, Severity::Critical);
    }

    #[test]
    fn a_whitespace_only_title_counts_as_missing() {
        let mut c = ctx();
        c.title = Some("   \n\t ");
        assert_eq!(MetaTitleMissing.evaluate(&c).len(), 1);
    }

    #[test]
    fn a_missing_title_on_a_non_indexable_page_is_not_flagged() {
        // A `noindex` without a title is not a problem: the page is not going to show up in
        // results.
        let mut c = ctx();
        c.title = None;
        c.is_indexable = false;
        assert!(MetaTitleMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn a_missing_title_outside_html_is_not_flagged() {
        let mut c = ctx();
        c.title = None;
        c.is_html = false;
        assert!(MetaTitleMissing.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ Pixel width

    /// Float equality with tolerance. The expected values are computed by hand from the advance
    /// table, so the tolerance only covers binary rounding.
    fn casi(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn the_estimated_width_reproduces_arial_advances() {
        // Ten i's are 10 × 222 thousandths of an em; at 20 px, 44.4 px.
        casi(title_width_px("iiiiiiiiii"), 44.4);
        // Ten m's are 10 × 833; at 20 px, 166.6 px. Almost four times as much with the same ten
        // characters: this is the entire argument against counting letters.
        casi(title_width_px("MMMMMMMMMM"), 166.6);
        casi(title_width_px("Hola"), 41.12);
        casi(description_width_px("Hola"), 28.784);
        casi(title_width_px(""), 0.0);
    }

    #[test]
    fn in_spanish_capitals_and_accents_change_the_width() {
        // Five characters each, and some fifty pixels of difference. In a language with accents
        // and long words, counting characters errs more than in English.
        casi(title_width_px("ÁRBOL"), 67.8);
        casi(title_width_px("árbol"), 44.46);
        // The accented vowel measures the same as its base, which is exact in Arial.
        casi(title_width_px("a"), title_width_px("á"));
        casi(title_width_px("o"), title_width_px("ó"));
        // Not the `í`: it loses the dot and keeps the `i`'s advance.
        casi(title_width_px("i"), title_width_px("í"));
    }

    #[test]
    fn the_width_ignores_surrounding_whitespace() {
        casi(title_width_px("  Hola  "), title_width_px("Hola"));
    }

    #[test]
    fn an_unknown_character_falls_back_to_the_default_advance() {
        // Outside Latin-1 the table has no opinion; it is documented that the error there is
        // large.
        casi(title_width_px("漢"), title_width_px("a"));
    }

    // ------------------------------------------------------------ META-TITLE-TOO-LONG

    #[test]
    fn a_title_that_fits_produces_no_finding() {
        assert!(MetaTitleTooLong.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_title_that_does_not_fit_produces_a_finding() {
        let mut c = ctx();
        c.title = Some(
            "Guía completa de auditoría técnica SEO para sitios WordPress y Astro en 2026, \
             con ejemplos",
        );
        let issues = MetaTitleTooLong.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-TITLE-TOO-LONG");
        assert_eq!(issues[0].severity, Severity::Medium);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("width_px"), "the detail carries the measured width: {detalle}");
    }

    #[test]
    fn the_title_threshold_is_measured_right_at_the_limit() {
        // An `n` measures 556 thousandths of an em: at 20 px that is 11.12 px. Fifty-two fit in
        // 578.24 px and fifty-three go to 589.36 px, with the limit at 580.
        let justo = "n".repeat(52);
        let mut c = ctx();
        c.title = Some(&justo);
        casi(title_width_px(&justo), 578.24);
        assert!(MetaTitleTooLong.evaluate(&c).is_empty(), "578.24 px fit in 580");

        let pasado = "n".repeat(53);
        let mut c = ctx();
        c.title = Some(&pasado);
        casi(title_width_px(&pasado), 589.36);
        assert_eq!(MetaTitleTooLong.evaluate(&c).len(), 1, "589.36 px do not fit in 580");
    }

    #[test]
    fn title_width_is_not_judged_when_there_is_no_title() {
        // META-TITLE-MISSING reports the absence; counting it twice would be noise.
        let mut c = ctx();
        c.title = None;
        assert!(MetaTitleTooLong.evaluate(&c).is_empty());
        assert!(MetaTitleTooShort.evaluate(&c).is_empty());
    }

    #[test]
    fn title_width_is_not_judged_outside_an_indexable_page() {
        let largo = "n".repeat(80);
        let mut c = ctx();
        c.title = Some(&largo);
        c.is_indexable = false;
        assert!(MetaTitleTooLong.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaTitleTooLong.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-TITLE-TOO-SHORT

    #[test]
    fn a_short_title_produces_a_finding() {
        let mut c = ctx();
        c.title = Some("Contacto");
        let issues = MetaTitleTooShort.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-TITLE-TOO-SHORT");
        assert_eq!(issues[0].severity, Severity::Low);
    }

    #[test]
    fn the_short_title_threshold_sits_at_thirty_characters() {
        let veintinueve = "a".repeat(29);
        let mut c = ctx();
        c.title = Some(&veintinueve);
        assert_eq!(MetaTitleTooShort.evaluate(&c).len(), 1);

        let treinta = "a".repeat(30);
        let mut c = ctx();
        c.title = Some(&treinta);
        assert!(MetaTitleTooShort.evaluate(&c).is_empty(), "thirty characters is already fine");
    }

    #[test]
    fn a_short_title_is_counted_in_characters_not_bytes() {
        // "Añádelo" is seven characters and nine bytes. Counting bytes would turn every accent
        // into text that does not exist, and in Spanish that happens on every page.
        let mut c = ctx();
        c.title = Some("Añádelo");
        let issues = MetaTitleTooShort.evaluate(&c);
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"chars\":7"), "seven characters, not nine: {detalle}");
    }

    #[test]
    fn a_short_title_is_not_judged_outside_an_indexable_page() {
        let mut c = ctx();
        c.title = Some("Corto");
        c.is_indexable = false;
        assert!(MetaTitleTooShort.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaTitleTooShort.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-TITLE-MULTIPLE

    #[test]
    fn a_single_title_produces_no_finding() {
        assert!(MetaTitleMultiple.evaluate(&ctx()).is_empty());
        let mut c = ctx();
        c.title_count = 0;
        assert!(MetaTitleMultiple.evaluate(&c).is_empty());
    }

    #[test]
    fn two_title_tags_produce_a_finding() {
        let mut c = ctx();
        c.title_count = 2;
        let issues = MetaTitleMultiple.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-TITLE-MULTIPLE");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"titles\":2"), "{detalle}");
    }

    #[test]
    fn repeated_titles_are_not_judged_outside_an_indexable_page() {
        let mut c = ctx();
        c.title_count = 3;
        c.is_indexable = false;
        assert!(MetaTitleMultiple.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaTitleMultiple.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-DESC-*

    /// A healthy page with a description, for the description rules.
    fn ctx_con_descripcion<'a>() -> PageContext<'a> {
        let mut c = ctx();
        c.meta_description = Some(
            "Una descripción de longitud razonable, con más de setenta caracteres y por debajo \
             del ancho que Google recorta.",
        );
        c
    }

    #[test]
    fn a_page_with_a_description_produces_no_finding() {
        let c = ctx_con_descripcion();
        assert!(MetaDescMissing.evaluate(&c).is_empty());
        assert!(MetaDescTooLong.evaluate(&c).is_empty());
        assert!(MetaDescTooShort.evaluate(&c).is_empty());
    }

    #[test]
    fn a_missing_description_produces_a_finding() {
        let c = ctx();
        let issues = MetaDescMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-DESC-MISSING");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn a_whitespace_only_description_counts_as_missing() {
        let mut c = ctx();
        c.meta_description = Some("  \n ");
        assert_eq!(MetaDescMissing.evaluate(&c).len(), 1);
        // And it does not additionally trigger the short-description rule: it is one defect.
        assert!(MetaDescTooShort.evaluate(&c).is_empty());
    }

    #[test]
    fn a_missing_description_is_not_judged_outside_an_indexable_page() {
        let mut c = ctx();
        c.is_indexable = false;
        assert!(MetaDescMissing.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaDescMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn a_description_that_does_not_fit_produces_a_finding() {
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(
            "Auditoría técnica SEO de escritorio para agencias y equipos que gestionan decenas \
             de sitios a la vez, con rastreo nativo, comparación entre rastreos y exportación a \
             CSV y XLSX sin límite de filas.",
        );
        let issues = MetaDescTooLong.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-DESC-TOO-LONG");
        assert_eq!(issues[0].severity, Severity::Low);
    }

    #[test]
    fn the_description_threshold_is_measured_right_at_the_limit() {
        // At 14 px an `n` measures 7.784 px: 127 fit in 988.57 px and 128 go to 996.35 px, with
        // the limit at 990.
        let justo = "n".repeat(127);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&justo);
        casi(description_width_px(&justo), 988.568);
        assert!(MetaDescTooLong.evaluate(&c).is_empty(), "988.57 px fit in 990");

        let pasado = "n".repeat(128);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&pasado);
        casi(description_width_px(&pasado), 996.352);
        assert_eq!(MetaDescTooLong.evaluate(&c).len(), 1, "996.35 px do not fit in 990");
    }

    #[test]
    fn a_short_description_produces_a_finding() {
        let mut c = ctx_con_descripcion();
        c.meta_description = Some("Página de contacto.");
        let issues = MetaDescTooShort.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-DESC-TOO-SHORT");
    }

    #[test]
    fn the_short_description_threshold_sits_at_seventy_characters() {
        let sesenta_y_nueve = "a".repeat(69);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&sesenta_y_nueve);
        assert_eq!(MetaDescTooShort.evaluate(&c).len(), 1);

        let setenta = "a".repeat(70);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&setenta);
        assert!(MetaDescTooShort.evaluate(&c).is_empty(), "seventy characters is already fine");
    }

    #[test]
    fn description_width_is_not_judged_outside_an_indexable_page() {
        let larga = "n".repeat(200);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&larga);
        c.is_indexable = false;
        assert!(MetaDescTooLong.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaDescTooLong.evaluate(&c).is_empty());
    }

    #[test]
    fn a_short_description_is_not_judged_outside_an_indexable_page() {
        let mut c = ctx_con_descripcion();
        c.meta_description = Some("Corta.");
        c.is_indexable = false;
        assert!(MetaDescTooShort.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaDescTooShort.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-VIEWPORT-MISSING

    #[test]
    fn a_page_with_a_viewport_produces_no_finding() {
        let mut c = ctx();
        c.viewport = Some("width=device-width, initial-scale=1");
        assert!(MetaViewportMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn a_missing_viewport_produces_a_finding() {
        let issues = MetaViewportMissing.evaluate(&ctx());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-VIEWPORT-MISSING");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn an_empty_viewport_counts_as_missing() {
        // `<meta name="viewport" content="">` configures nothing, and the phone goes back to
        // the 980 px layout.
        let mut c = ctx();
        c.viewport = Some("   ");
        assert_eq!(MetaViewportMissing.evaluate(&c).len(), 1);
    }

    #[test]
    fn the_viewport_is_not_judged_outside_an_indexable_page() {
        let mut c = ctx();
        c.is_indexable = false;
        assert!(MetaViewportMissing.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaViewportMissing.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-REFRESH

    #[test]
    fn a_page_without_meta_refresh_produces_no_finding() {
        assert!(MetaRefresh.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_meta_refresh_produces_a_finding() {
        let mut c = ctx();
        c.meta_refresh = Some("0;url=/destino/");
        let issues = MetaRefresh.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-REFRESH");
        assert_eq!(issues[0].severity, Severity::High);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("0;url=/destino/"), "the detail carries the content: {detalle}");
    }

    #[test]
    fn a_meta_refresh_without_a_target_is_also_reported() {
        // `content="30"` reloads the page itself. It is not a redirect, but it is an automatic
        // refresh the user did not ask for, and the catalogue's condition is the use of the
        // tag.
        let mut c = ctx();
        c.meta_refresh = Some("30");
        assert_eq!(MetaRefresh.evaluate(&c).len(), 1);
    }

    #[test]
    fn an_empty_meta_refresh_produces_no_finding() {
        let mut c = ctx();
        c.meta_refresh = Some("  ");
        assert!(MetaRefresh.evaluate(&c).is_empty());
    }

    #[test]
    fn meta_refresh_is_not_judged_outside_an_indexable_page() {
        let mut c = ctx();
        c.meta_refresh = Some("0;url=/destino/");
        c.is_indexable = false;
        assert!(MetaRefresh.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaRefresh.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-TITLE-DUPLICATE

    #[test]
    fn the_pagination_base_is_recognized_and_nothing_else() {
        assert_eq!(pagination_base("/category/seo/page/2/"), Some("/category/seo"));
        assert_eq!(pagination_base("/category/seo/page/2"), Some("/category/seo"));
        assert_eq!(pagination_base("/noticias/pagina/40"), Some("/noticias"));
        assert_eq!(pagination_base("/page/2/"), Some(""), "pagination of the root");
        assert_eq!(pagination_base("/category/seo/"), None);
        assert_eq!(pagination_base("/post-con-numero/2019/"), None, "a year is not pagination");
        assert_eq!(pagination_base("/page/dos/"), None);
        assert_eq!(pagination_base("/"), None);
    }

    /// Only the columns the title query reads: `(url_hash, title, is_indexable, path)`.
    /// The real schema is exercised by the fixture, as with the descriptions.
    fn conexion_con_titulos(filas: &[(i64, Option<&str>, i64, &str)]) -> Connection {
        let conn = match Connection::open_in_memory() {
            Ok(c) => c,
            Err(e) => panic!("open in-memory sqlite: {e}"),
        };
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url_hash INTEGER NOT NULL, path TEXT);
             CREATE TABLE pages (
                 url_id INTEGER PRIMARY KEY REFERENCES urls(id),
                 title TEXT,
                 is_indexable INTEGER NOT NULL
             );",
        )
        .expect("create the minimal schema");
        for (i, (hash, title, indexable, path)) in filas.iter().enumerate() {
            let id = i as i64 + 1;
            conn.execute("INSERT INTO urls (id, url_hash, path) VALUES (?1, ?2, ?3)", (id, hash, path))
                .expect("insert url");
            conn.execute(
                "INSERT INTO pages (url_id, title, is_indexable) VALUES (?1, ?2, ?3)",
                (id, title, indexable),
            )
            .expect("insert page");
        }
        conn
    }

    #[test]
    fn two_articles_with_the_same_title_are_still_a_high_duplicate() {
        // The real case from the field crawl: the same article published twice under another slug.
        // They genuinely compete for the same query, and there the rule's severity is the right
        // one.
        let conn = conexion_con_titulos(&[
            (10, Some("El mismo artículo"), 1, "/articulo/"),
            (20, Some("El mismo artículo"), 1, "/articulo-2/"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("query");
        assert_eq!(hallazgos.len(), 2);
        for (_, issue) in &hallazgos {
            assert_eq!(issue.severity, Severity::High);
            let detalle = issue.detail_json.as_deref().unwrap_or_default();
            assert!(detalle.contains("\"pagination_series\":false"), "{detalle}");
        }
    }

    #[test]
    fn the_paginated_series_of_one_archive_drops_to_low() {
        // The real case from a WordPress: /category/x/ and its /page/N/ share a title because
        // that is what WordPress produces out of the box on every paginated archive. The fact
        // stays in the report —it is true— but as `low` and declared in the detail: 38 of the
        // rule's 40 `high` findings in that crawl were this.
        let conn = conexion_con_titulos(&[
            (10, Some("Casos de éxito"), 1, "/category/casos-de-exito/"),
            (20, Some("Casos de éxito"), 1, "/category/casos-de-exito/page/2/"),
            (30, Some("Casos de éxito"), 1, "/category/casos-de-exito/page/3/"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("query");
        assert_eq!(hallazgos.len(), 3, "the series is reported, not silenced");
        for (_, issue) in &hallazgos {
            assert_eq!(issue.severity, Severity::Low, "a paginated series is not a high-severity duplicate");
            let detalle = issue.detail_json.as_deref().unwrap_or_default();
            assert!(detalle.contains("\"pagination_series\":true"), "{detalle}");
        }
    }

    #[test]
    fn spanish_pagination_is_also_a_series() {
        let conn = conexion_con_titulos(&[
            (10, Some("Noticias"), 1, "/noticias"),
            (20, Some("Noticias"), 1, "/noticias/pagina/5"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("query");
        assert!(hallazgos.iter().all(|(_, i)| i.severity == Severity::Low));
    }

    #[test]
    fn two_different_archives_with_the_same_title_are_not_a_series() {
        // The same title across the pagination of two different categories: that is a real
        // configuration duplicate, not the expected series of a single archive.
        let conn = conexion_con_titulos(&[
            (10, Some("Archivo"), 1, "/category/a/page/2/"),
            (20, Some("Archivo"), 1, "/category/b/page/2/"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("query");
        assert!(hallazgos.iter().all(|(_, i)| i.severity == Severity::High));
    }

    #[test]
    fn a_series_needs_at_least_one_pagination_page() {
        // Two paths equal after normalizing the trailing slash are not enough: without any
        // /page/N there is no series, just two pages with the same title.
        let conn = conexion_con_titulos(&[
            (10, Some("Duplicado"), 1, "/seccion/"),
            (20, Some("Duplicado"), 1, "/seccion"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("query");
        assert!(hallazgos.iter().all(|(_, i)| i.severity == Severity::High));
    }

    // ------------------------------------------------------------ META-DESC-DUPLICATE

    /// Only the three columns the query reads, and that is a choice, not a limitation: the
    /// migration *can* be loaded here —`http.rs` does it in this same crate with a relative
    /// `include_str!`, since it is a path and not a crate dependency— but these tests only
    /// exercise a `SELECT` over three columns and the full schema would add nothing. The real
    /// schema is exercised by `crawlforge-core/tests/fixtures_de_reglas.rs`, which crawls the
    /// fixture end to end.
    fn conexion_con_paginas(filas: &[(i64, Option<&str>, i64)]) -> Connection {
        let conn = match Connection::open_in_memory() {
            Ok(c) => c,
            Err(e) => panic!("open in-memory sqlite: {e}"),
        };
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url_hash INTEGER NOT NULL);
             CREATE TABLE pages (
                 url_id INTEGER PRIMARY KEY REFERENCES urls(id),
                 meta_description TEXT,
                 is_indexable INTEGER NOT NULL
             );",
        )
        .expect("create the minimal schema");
        for (i, (hash, desc, indexable)) in filas.iter().enumerate() {
            let id = i as i64 + 1;
            conn.execute("INSERT INTO urls (id, url_hash) VALUES (?1, ?2)", (id, hash))
                .expect("insert url");
            conn.execute(
                "INSERT INTO pages (url_id, meta_description, is_indexable) VALUES (?1, ?2, ?3)",
                (id, desc, indexable),
            )
            .expect("insert page");
        }
        conn
    }

    #[test]
    fn distinct_descriptions_produce_no_finding() {
        let conn = conexion_con_paginas(&[(10, Some("Primera"), 1), (20, Some("Segunda"), 1)]);
        let hallazgos = MetaDescDuplicate.evaluate(&conn).expect("query");
        assert!(hallazgos.is_empty());
    }

    #[test]
    fn a_repeated_description_is_reported_on_both_pages() {
        let conn = conexion_con_paginas(&[
            (10, Some("La misma de siempre"), 1),
            (20, Some("La misma de siempre"), 1),
            (30, Some("Otra"), 1),
        ]);
        let hallazgos = MetaDescDuplicate.evaluate(&conn).expect("query");
        assert_eq!(hallazgos.len(), 2, "the finding is recorded on every page involved");
        let hashes: Vec<Option<i64>> = hallazgos.iter().map(|(h, _)| *h).collect();
        assert!(hashes.contains(&Some(10)) && hashes.contains(&Some(20)));
        // Both share the `group_key`, which is what lets the UI say "on 2 pages".
        assert_eq!(hallazgos[0].1.group_key, hallazgos[1].1.group_key);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"pages\":2"), "{detalle}");
    }

    #[test]
    fn non_indexable_pages_do_not_count_as_duplicates() {
        // A description repeated between a `noindex` page and one without it does not compete
        // in results: there are no two identical snippets Google could show.
        let conn = conexion_con_paginas(&[(10, Some("Repetida"), 1), (20, Some("Repetida"), 0)]);
        assert!(MetaDescDuplicate.evaluate(&conn).expect("query").is_empty());
    }

    #[test]
    fn missing_or_empty_descriptions_are_not_duplicates() {
        // Three pages without a description are not "three identical descriptions":
        // META-DESC-MISSING reports that, once per page.
        let conn =
            conexion_con_paginas(&[(10, None, 1), (20, None, 1), (30, Some("  "), 1), (40, Some("  "), 1)]);
        assert!(MetaDescDuplicate.evaluate(&conn).expect("query").is_empty());
    }
}
