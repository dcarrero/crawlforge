//! `META` — títulos y meta descripciones. `docs/04-CATALOGO-REGLAS.md §4`.
//!
//! Este módulo es la plantilla del resto: [`MetaTitleMissing`] es el ejemplo de regla de página
//! y [`MetaTitleDuplicate`] el de regla de conjunto.
//!
//! Los umbrales de «demasiado largo» son de **ancho estimado en píxeles**, no de número de
//! caracteres, porque es así como Google trunca. La tabla de anchos y el error que se le atribuye
//! están en [`arial_advance_per_mille`].

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

// ---------------------------------------------------------------- Ancho en píxeles
//
// Google no corta el título ni la descripción por número de caracteres: los corta cuando no
// caben en el ancho disponible del resultado. Contar caracteres da avisos falsos en los dos
// sentidos —un título de sesenta íes cabe de sobra y uno de cuarenta y cinco emes no— y en
// español el error es mayor: las palabras son más largas, las mayúsculas y las tildes están más
// presentes, y «ÁRBOL» ocupa un 50 % más que «árbol» con los mismos cinco caracteres.

/// Tamaño con el que Google renderiza el título del resultado en escritorio.
const TITLE_FONT_PX: f64 = 20.0;
/// Tamaño con el que Google renderiza el fragmento de descripción en escritorio.
const DESC_FONT_PX: f64 = 14.0;

/// Ancho máximo del título antes de que se corte, en píxeles. `docs/04-CATALOGO-REGLAS.md §4`.
pub const TITLE_MAX_WIDTH_PX: f64 = 580.0;
/// Ancho máximo de la descripción antes de que se corte, en píxeles.
pub const DESC_MAX_WIDTH_PX: f64 = 990.0;

/// Por debajo de esto el título desaprovecha el espacio del resultado. En caracteres a propósito:
/// el aviso es «te falta texto que escribir», y eso se cuenta, no se mide.
pub const TITLE_MIN_CHARS: usize = 30;
/// Por debajo de esto la descripción desaprovecha el espacio del fragmento.
pub const DESC_MIN_CHARS: usize = 70;

/// Ancho de avance de un carácter en Arial Regular, en milésimas de em.
///
/// **De dónde sale la tabla:** son las anchuras de avance de Arial Regular, expresadas en la
/// unidad de los ficheros AFM (milésimas de em, la misma escala que la tabla `hmtx` de un
/// TrueType con 1000 unidades por em). Arial se diseñó como sustituto métricamente compatible de
/// Helvetica, así que en todo el rango ASCII comparte avances con la Helvetica de las catorce
/// fuentes base de PostScript, cuyas métricas son públicas. Los caracteres se agrupan en clases
/// porque dentro de cada clase el avance es literalmente el mismo valor: `A`, `B`, `E`, `K`, `P`,
/// `S`, `V`, `X` e `Y` miden 667 los nueve.
///
/// **Qué error tiene:**
///
/// - Sobre prosa latina normal se queda por debajo del 2 % frente a lo que mide un navegador. Lo
///   que se ignora es el *kerning* —Arial acerca pares como `AV` o `To`— y el redondeo a
///   subpíxel del rasterizador, y los dos restan ancho, así que la estimación se equivoca por
///   arriba: avisa un poco antes de tiempo, nunca un poco tarde.
/// - Las vocales acentuadas y la `ñ` miden lo que su letra base, lo cual es exacto en Arial.
/// - Fuera de Latin-1 —CJK, emoji, símbolos matemáticos— todo cae en el valor por defecto y el
///   error es grande. Se acepta: un título en japonés no lo renderiza Google con Arial, y la
///   regla no pretende ser un motor de composición.
///
/// El core tiene una función equivalente para rellenar las columnas `pages.title_px` y
/// `pages.meta_desc_px`. Está duplicada porque la dependencia va del core a las reglas y no al
/// revés; [`estimated_width_px`] es `pub` para que la consolidación futura pueda ir en esa
/// dirección y no en la contraria.
fn arial_advance_per_mille(c: char) -> u64 {
    match c {
        // Clase estrecha: lo único que baja de un cuarto de em.
        'i' | 'j' | 'l' | 'í' | 'ì' | 'î' | 'ï' => 222,
        // La `'` tipográfica es aún más estrecha (191), pero no merece una clase propia.
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
        // Minúsculas restantes, dígitos y el resto de la puntuación media. Es también el valor
        // que reciben los caracteres que la tabla no conoce.
        _ => 556,
    }
}

/// Ancho estimado de un texto renderizado en Arial al tamaño dado, en píxeles.
pub fn estimated_width_px(text: &str, font_size_px: f64) -> f64 {
    let por_millar: u64 = text.chars().map(arial_advance_per_mille).sum();
    por_millar as f64 * font_size_px / 1000.0
}

/// Ancho estimado de un título en el resultado de búsqueda (Arial 20 px).
pub fn title_width_px(title: &str) -> f64 {
    estimated_width_px(title.trim(), TITLE_FONT_PX)
}

/// Ancho estimado de una meta descripción en el fragmento (Arial 14 px).
pub fn description_width_px(description: &str) -> f64 {
    estimated_width_px(description.trim(), DESC_FONT_PX)
}

/// Página indexable sin `<title>`.
///
/// Solo se aplica a páginas indexables: avisar del título de una página con `noindex` es ruido.
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

/// La ruta base de una URL de paginación, si la ruta lo es.
///
/// Reconoce exactamente el sufijo `/page/<n>` o `/pagina/<n>`, con o sin barra final: son las
/// dos formas vistas en rastreos reales (el permalink por defecto de WordPress en inglés y su
/// traducción). **A propósito no es una lista larga de patrones**: cada patrón nuevo es una
/// oportunidad de degradar un duplicado real, así que solo entra lo que un rastreo haya
/// demostrado. `/category/x/page/2/` → `Some("/category/x")`; `/category/x/` → `None`.
fn pagination_base(path: &str) -> Option<&str> {
    let sin_barra = path.trim_end_matches('/');
    let (resto, numero) = sin_barra.rsplit_once('/')?;
    if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (base, segmento) = resto.rsplit_once('/')?;
    (segmento == "page" || segmento == "pagina").then_some(base)
}

/// Dos o más páginas indexables comparten el mismo `<title>`.
///
/// Necesita el rastreo completo: es el ejemplo canónico de por qué existen las [`SiteRule`].
/// El `group_key` agrupa las páginas que comparten título para que la UI las presente juntas.
///
/// **La serie paginada de un mismo archivo baja a `low`.** El dato es cierto —`/category/x/` y
/// sus `/page/N/` comparten título— pero es lo que WordPress produce de serie en cada archivo
/// paginado, y en un rastreo real esas series eran 38 de los 40 hallazgos `high` de la regla:
/// un aviso alto que sale en todos los WordPress del mundo deja de leerse, y con él los
/// duplicados que sí compiten. La condición es estricta: **todas** las páginas del grupo tienen
/// que reducirse a una misma base al quitar el sufijo de paginación ([`pagination_base`]); si el
/// título lo comparten dos archivos distintos, o un archivo y un artículo, el grupo entero
/// conserva su severidad. El detalle lo declara con `pagination_series`.
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

        // Un grupo es una serie paginada si todas sus rutas colapsan a la misma base al quitar
        // el sufijo `/page/N`, y al menos una lo llevaba. Se calcula por título, no por fila.
        let mut bases: std::collections::HashMap<&str, (Option<&str>, bool)> =
            std::collections::HashMap::new();
        for (_, title, _, path) in &filas {
            let base = pagination_base(path);
            let normalizada = base.unwrap_or_else(|| path.trim_end_matches('/'));
            let entrada = bases.entry(title.as_str()).or_insert((Some(normalizada), false));
            if entrada.0 != Some(normalizada) {
                entrada.0 = None; // dos bases distintas: no es una sola serie
            }
            entrada.1 |= base.is_some(); // alguna página del grupo es una /page/N
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

/// El título no cabe en el resultado de búsqueda.
///
/// El umbral es de ancho, no de longitud: ver [`arial_advance_per_mille`] para el porqué.
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

/// El título deja sin usar el espacio del resultado de búsqueda.
///
/// Aquí sí se cuentan caracteres: el consejo es «escribe más texto», y un aviso en píxeles
/// obligaría a explicarle al usuario cuántas letras le faltan.
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

/// Más de una etiqueta `<title>` en la página.
///
/// El recuento lo hace el parser, que ya descarta el `<title>` de un `<svg>`: ése es el nombre
/// accesible de un icono, no un título de página.
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

/// Página indexable sin meta descripción.
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

/// La descripción no cabe en el fragmento del resultado.
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

/// La descripción desaprovecha el espacio del fragmento.
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

/// Página indexable sin `<meta name="viewport">`.
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

/// La página redirige con `<meta http-equiv="refresh">`.
///
/// El valor de `content` viaja en el detalle porque el retardo cambia el diagnóstico: `0` es una
/// redirección disfrazada y cualquier otro número es además una trampa para el botón de volver.
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

/// Dos o más páginas indexables comparten la misma meta descripción.
///
/// Calcada de [`MetaTitleDuplicate`] a propósito: la misma consulta sobre otra columna. La
/// comparación es exacta, sin normalizar espacios ni mayúsculas, porque dos descripciones que
/// solo difieren en el espaciado son dos descripciones distintas para el índice y hay que poder
/// verlo en el diff.
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

/// El título sobre el que tiene sentido opinar de longitud o de ancho.
///
/// `None` cuando la página no es HTML indexable o cuando no hay título: de la ausencia ya avisa
/// [`MetaTitleMissing`], y añadir «además es corto» sería el mismo defecto contado dos veces.
fn titulo_util<'a>(ctx: &PageContext<'a>) -> Option<&'a str> {
    if !ctx.is_html || !ctx.is_indexable {
        return None;
    }
    ctx.title.map(str::trim).filter(|t| !t.is_empty())
}

/// La descripción sobre la que tiene sentido opinar. Mismo criterio que [`titulo_util`]: de la
/// ausencia avisa [`MetaDescMissing`].
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

    /// Una página sana de la que partir. Cada test rompe solo lo que le interesa.
    fn ctx<'a>() -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.title = Some("Un título correcto y suficientemente descriptivo");
        c.title_count = 1;
        c
    }

    #[test]
    fn no_avisa_cuando_hay_titulo() {
        assert!(MetaTitleMissing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_cuando_falta_el_titulo() {
        let mut c = ctx();
        c.title = None;
        let issues = MetaTitleMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-TITLE-MISSING");
        assert_eq!(issues[0].severity, Severity::Critical);
    }

    #[test]
    fn un_titulo_de_solo_espacios_cuenta_como_ausente() {
        let mut c = ctx();
        c.title = Some("   \n\t ");
        assert_eq!(MetaTitleMissing.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_avisa_en_una_pagina_no_indexable() {
        // Un `noindex` sin título no es un problema: la página no va a aparecer en resultados.
        let mut c = ctx();
        c.title = None;
        c.is_indexable = false;
        assert!(MetaTitleMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn no_avisa_sobre_algo_que_no_es_html() {
        let mut c = ctx();
        c.title = None;
        c.is_html = false;
        assert!(MetaTitleMissing.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ Ancho en píxeles

    /// Igualdad de flotantes con tolerancia. Los valores esperados están calculados a mano desde
    /// la tabla de avances, así que la tolerancia solo cubre el redondeo binario.
    fn casi(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn el_ancho_estimado_reproduce_los_avances_de_arial() {
        // Diez íes son 10 × 222 milésimas de em; a 20 px, 44,4 px.
        casi(title_width_px("iiiiiiiiii"), 44.4);
        // Diez emes son 10 × 833; a 20 px, 166,6 px. Casi cuatro veces más con los mismos diez
        // caracteres: esto es todo el argumento contra contar letras.
        casi(title_width_px("MMMMMMMMMM"), 166.6);
        casi(title_width_px("Hola"), 41.12);
        casi(description_width_px("Hola"), 28.784);
        casi(title_width_px(""), 0.0);
    }

    #[test]
    fn en_espanol_las_mayusculas_y_las_tildes_cambian_el_ancho() {
        // Cinco caracteres los dos, y medio centenar de píxeles de diferencia. En un idioma con
        // tildes y con palabras largas, contar caracteres se equivoca más que en inglés.
        casi(title_width_px("ÁRBOL"), 67.8);
        casi(title_width_px("árbol"), 44.46);
        // La vocal acentuada mide lo que su base, que es exacto en Arial.
        casi(title_width_px("a"), title_width_px("á"));
        casi(title_width_px("o"), title_width_px("ó"));
        // La `í` no: pierde el punto y conserva el avance de la `i`.
        casi(title_width_px("i"), title_width_px("í"));
    }

    #[test]
    fn el_ancho_ignora_los_espacios_de_los_extremos() {
        casi(title_width_px("  Hola  "), title_width_px("Hola"));
    }

    #[test]
    fn un_caracter_desconocido_cae_en_el_valor_por_defecto() {
        // Fuera de Latin-1 la tabla no opina; se documenta que ahí el error es grande.
        casi(title_width_px("漢"), title_width_px("a"));
    }

    // ------------------------------------------------------------ META-TITLE-TOO-LONG

    #[test]
    fn no_avisa_de_un_titulo_que_cabe() {
        assert!(MetaTitleTooLong.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_de_un_titulo_que_no_cabe() {
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
        assert!(detalle.contains("width_px"), "el detalle lleva el ancho medido: {detalle}");
    }

    #[test]
    fn el_umbral_del_titulo_se_mide_justo_en_el_limite() {
        // Una `n` mide 556 milésimas de em: a 20 px son 11,12 px. Cincuenta y dos caben en
        // 578,24 px y cincuenta y tres se van a 589,36 px, con el límite en 580.
        let justo = "n".repeat(52);
        let mut c = ctx();
        c.title = Some(&justo);
        casi(title_width_px(&justo), 578.24);
        assert!(MetaTitleTooLong.evaluate(&c).is_empty(), "578,24 px caben en 580");

        let pasado = "n".repeat(53);
        let mut c = ctx();
        c.title = Some(&pasado);
        casi(title_width_px(&pasado), 589.36);
        assert_eq!(MetaTitleTooLong.evaluate(&c).len(), 1, "589,36 px no caben en 580");
    }

    #[test]
    fn el_ancho_del_titulo_no_se_juzga_si_no_hay_titulo() {
        // De la ausencia avisa META-TITLE-MISSING; contarla dos veces sería ruido.
        let mut c = ctx();
        c.title = None;
        assert!(MetaTitleTooLong.evaluate(&c).is_empty());
        assert!(MetaTitleTooShort.evaluate(&c).is_empty());
    }

    #[test]
    fn el_ancho_del_titulo_no_se_juzga_fuera_de_una_pagina_indexable() {
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
    fn avisa_de_un_titulo_corto() {
        let mut c = ctx();
        c.title = Some("Contacto");
        let issues = MetaTitleTooShort.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-TITLE-TOO-SHORT");
        assert_eq!(issues[0].severity, Severity::Low);
    }

    #[test]
    fn el_umbral_del_titulo_corto_esta_en_treinta_caracteres() {
        let veintinueve = "a".repeat(29);
        let mut c = ctx();
        c.title = Some(&veintinueve);
        assert_eq!(MetaTitleTooShort.evaluate(&c).len(), 1);

        let treinta = "a".repeat(30);
        let mut c = ctx();
        c.title = Some(&treinta);
        assert!(MetaTitleTooShort.evaluate(&c).is_empty(), "treinta caracteres ya está bien");
    }

    #[test]
    fn el_titulo_corto_se_cuenta_en_caracteres_no_en_bytes() {
        // «Añádelo» son siete caracteres y nueve bytes. Contar bytes convertiría cada tilde en
        // texto que no existe, y en español eso pasa en todas las páginas.
        let mut c = ctx();
        c.title = Some("Añádelo");
        let issues = MetaTitleTooShort.evaluate(&c);
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"chars\":7"), "siete caracteres, no nueve: {detalle}");
    }

    #[test]
    fn un_titulo_corto_no_se_juzga_fuera_de_una_pagina_indexable() {
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
    fn no_avisa_con_un_solo_titulo() {
        assert!(MetaTitleMultiple.evaluate(&ctx()).is_empty());
        let mut c = ctx();
        c.title_count = 0;
        assert!(MetaTitleMultiple.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_con_dos_titulos() {
        let mut c = ctx();
        c.title_count = 2;
        let issues = MetaTitleMultiple.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-TITLE-MULTIPLE");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"titles\":2"), "{detalle}");
    }

    #[test]
    fn los_titulos_repetidos_no_se_juzgan_fuera_de_una_pagina_indexable() {
        let mut c = ctx();
        c.title_count = 3;
        c.is_indexable = false;
        assert!(MetaTitleMultiple.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaTitleMultiple.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-DESC-*

    /// Una página sana con descripción, para las reglas de descripción.
    fn ctx_con_descripcion<'a>() -> PageContext<'a> {
        let mut c = ctx();
        c.meta_description = Some(
            "Una descripción de longitud razonable, con más de setenta caracteres y por debajo \
             del ancho que Google recorta.",
        );
        c
    }

    #[test]
    fn no_avisa_cuando_hay_descripcion() {
        let c = ctx_con_descripcion();
        assert!(MetaDescMissing.evaluate(&c).is_empty());
        assert!(MetaDescTooLong.evaluate(&c).is_empty());
        assert!(MetaDescTooShort.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_cuando_falta_la_descripcion() {
        let c = ctx();
        let issues = MetaDescMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-DESC-MISSING");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn una_descripcion_de_solo_espacios_cuenta_como_ausente() {
        let mut c = ctx();
        c.meta_description = Some("  \n ");
        assert_eq!(MetaDescMissing.evaluate(&c).len(), 1);
        // Y no dispara además la de descripción corta: es un solo defecto.
        assert!(MetaDescTooShort.evaluate(&c).is_empty());
    }

    #[test]
    fn la_descripcion_ausente_no_se_juzga_fuera_de_una_pagina_indexable() {
        let mut c = ctx();
        c.is_indexable = false;
        assert!(MetaDescMissing.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaDescMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_de_una_descripcion_que_no_cabe() {
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
    fn el_umbral_de_la_descripcion_se_mide_justo_en_el_limite() {
        // A 14 px una `n` mide 7,784 px: 127 caben en 988,57 px y 128 se van a 996,35 px, con el
        // límite en 990.
        let justo = "n".repeat(127);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&justo);
        casi(description_width_px(&justo), 988.568);
        assert!(MetaDescTooLong.evaluate(&c).is_empty(), "988,57 px caben en 990");

        let pasado = "n".repeat(128);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&pasado);
        casi(description_width_px(&pasado), 996.352);
        assert_eq!(MetaDescTooLong.evaluate(&c).len(), 1, "996,35 px no caben en 990");
    }

    #[test]
    fn avisa_de_una_descripcion_corta() {
        let mut c = ctx_con_descripcion();
        c.meta_description = Some("Página de contacto.");
        let issues = MetaDescTooShort.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-DESC-TOO-SHORT");
    }

    #[test]
    fn el_umbral_de_la_descripcion_corta_esta_en_setenta_caracteres() {
        let sesenta_y_nueve = "a".repeat(69);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&sesenta_y_nueve);
        assert_eq!(MetaDescTooShort.evaluate(&c).len(), 1);

        let setenta = "a".repeat(70);
        let mut c = ctx_con_descripcion();
        c.meta_description = Some(&setenta);
        assert!(MetaDescTooShort.evaluate(&c).is_empty(), "setenta caracteres ya está bien");
    }

    #[test]
    fn el_ancho_de_la_descripcion_no_se_juzga_fuera_de_una_pagina_indexable() {
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
    fn una_descripcion_corta_no_se_juzga_fuera_de_una_pagina_indexable() {
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
    fn no_avisa_cuando_hay_viewport() {
        let mut c = ctx();
        c.viewport = Some("width=device-width, initial-scale=1");
        assert!(MetaViewportMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_cuando_falta_el_viewport() {
        let issues = MetaViewportMissing.evaluate(&ctx());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-VIEWPORT-MISSING");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn un_viewport_vacio_cuenta_como_ausente() {
        // `<meta name="viewport" content="">` no configura nada, y el móvil vuelve a los 980 px.
        let mut c = ctx();
        c.viewport = Some("   ");
        assert_eq!(MetaViewportMissing.evaluate(&c).len(), 1);
    }

    #[test]
    fn el_viewport_no_se_juzga_fuera_de_una_pagina_indexable() {
        let mut c = ctx();
        c.is_indexable = false;
        assert!(MetaViewportMissing.evaluate(&c).is_empty());
        c.is_indexable = true;
        c.is_html = false;
        assert!(MetaViewportMissing.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ META-REFRESH

    #[test]
    fn no_avisa_sin_meta_refresh() {
        assert!(MetaRefresh.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_con_meta_refresh() {
        let mut c = ctx();
        c.meta_refresh = Some("0;url=/destino/");
        let issues = MetaRefresh.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "META-REFRESH");
        assert_eq!(issues[0].severity, Severity::High);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("0;url=/destino/"), "el detalle lleva el content: {detalle}");
    }

    #[test]
    fn un_meta_refresh_sin_destino_tambien_avisa() {
        // `content="30"` recarga la propia página. No es una redirección, pero sí un refresco
        // automático que el usuario no pidió, y la condición del catálogo es el uso de la
        // etiqueta.
        let mut c = ctx();
        c.meta_refresh = Some("30");
        assert_eq!(MetaRefresh.evaluate(&c).len(), 1);
    }

    #[test]
    fn un_meta_refresh_vacio_no_avisa() {
        let mut c = ctx();
        c.meta_refresh = Some("  ");
        assert!(MetaRefresh.evaluate(&c).is_empty());
    }

    #[test]
    fn el_meta_refresh_no_se_juzga_fuera_de_una_pagina_indexable() {
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
    fn la_base_de_paginacion_se_reconoce_y_solo_ella() {
        assert_eq!(pagination_base("/category/seo/page/2/"), Some("/category/seo"));
        assert_eq!(pagination_base("/category/seo/page/2"), Some("/category/seo"));
        assert_eq!(pagination_base("/noticias/pagina/40"), Some("/noticias"));
        assert_eq!(pagination_base("/page/2/"), Some(""), "paginación de la raíz");
        assert_eq!(pagination_base("/category/seo/"), None);
        assert_eq!(pagination_base("/post-con-numero/2019/"), None, "un año no es paginación");
        assert_eq!(pagination_base("/page/dos/"), None);
        assert_eq!(pagination_base("/"), None);
    }

    /// Solo las columnas que la consulta de títulos lee: `(url_hash, title, is_indexable, path)`.
    /// El esquema de verdad lo ejercita el fixture, como en las descripciones.
    fn conexion_con_titulos(filas: &[(i64, Option<&str>, i64, &str)]) -> Connection {
        let conn = match Connection::open_in_memory() {
            Ok(c) => c,
            Err(e) => panic!("abrir sqlite en memoria: {e}"),
        };
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url_hash INTEGER NOT NULL, path TEXT);
             CREATE TABLE pages (
                 url_id INTEGER PRIMARY KEY REFERENCES urls(id),
                 title TEXT,
                 is_indexable INTEGER NOT NULL
             );",
        )
        .expect("crear el esquema mínimo");
        for (i, (hash, title, indexable, path)) in filas.iter().enumerate() {
            let id = i as i64 + 1;
            conn.execute("INSERT INTO urls (id, url_hash, path) VALUES (?1, ?2, ?3)", (id, hash, path))
                .expect("insertar url");
            conn.execute(
                "INSERT INTO pages (url_id, title, is_indexable) VALUES (?1, ?2, ?3)",
                (id, title, indexable),
            )
            .expect("insertar página");
        }
        conn
    }

    #[test]
    fn dos_articulos_con_el_mismo_titulo_siguen_siendo_un_duplicado_alto() {
        // El caso real de un-diario: el mismo artículo publicado dos veces con slug distinto.
        // Compiten de verdad por la misma consulta, y ahí la severidad de la regla es la justa.
        let conn = conexion_con_titulos(&[
            (10, Some("El mismo artículo"), 1, "/articulo/"),
            (20, Some("El mismo artículo"), 1, "/articulo-2/"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("consultar");
        assert_eq!(hallazgos.len(), 2);
        for (_, issue) in &hallazgos {
            assert_eq!(issue.severity, Severity::High);
            let detalle = issue.detail_json.as_deref().unwrap_or_default();
            assert!(detalle.contains("\"pagination_series\":false"), "{detalle}");
        }
    }

    #[test]
    fn la_serie_paginada_de_un_archivo_baja_a_low() {
        // El caso real de un WordPress: /category/x/ y sus /page/N/ comparten título porque es lo
        // que WordPress produce de serie en cada archivo paginado. El dato sigue en el informe
        // —es cierto— pero como `low` y declarado en el detalle: 38 de los 40 `high` de la
        // regla en ese rastreo eran esto.
        let conn = conexion_con_titulos(&[
            (10, Some("Casos de éxito"), 1, "/category/casos-de-exito/"),
            (20, Some("Casos de éxito"), 1, "/category/casos-de-exito/page/2/"),
            (30, Some("Casos de éxito"), 1, "/category/casos-de-exito/page/3/"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("consultar");
        assert_eq!(hallazgos.len(), 3, "la serie se reporta, no se silencia");
        for (_, issue) in &hallazgos {
            assert_eq!(issue.severity, Severity::Low, "una serie paginada no es un duplicado alto");
            let detalle = issue.detail_json.as_deref().unwrap_or_default();
            assert!(detalle.contains("\"pagination_series\":true"), "{detalle}");
        }
    }

    #[test]
    fn la_paginacion_en_espanol_tambien_es_una_serie() {
        let conn = conexion_con_titulos(&[
            (10, Some("Noticias"), 1, "/noticias"),
            (20, Some("Noticias"), 1, "/noticias/pagina/5"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("consultar");
        assert!(hallazgos.iter().all(|(_, i)| i.severity == Severity::Low));
    }

    #[test]
    fn dos_archivos_distintos_con_el_mismo_titulo_no_son_una_serie() {
        // Mismo título en la paginación de dos categorías distintas: eso sí es un duplicado de
        // configuración, no la serie esperable de un solo archivo.
        let conn = conexion_con_titulos(&[
            (10, Some("Archivo"), 1, "/category/a/page/2/"),
            (20, Some("Archivo"), 1, "/category/b/page/2/"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("consultar");
        assert!(hallazgos.iter().all(|(_, i)| i.severity == Severity::High));
    }

    #[test]
    fn una_serie_necesita_al_menos_una_pagina_de_paginacion() {
        // Dos rutas iguales tras normalizar la barra final no bastan: sin ningún /page/N no hay
        // serie, hay dos páginas con el mismo título.
        let conn = conexion_con_titulos(&[
            (10, Some("Duplicado"), 1, "/seccion/"),
            (20, Some("Duplicado"), 1, "/seccion"),
        ]);
        let hallazgos = MetaTitleDuplicate.evaluate(&conn).expect("consultar");
        assert!(hallazgos.iter().all(|(_, i)| i.severity == Severity::High));
    }

    // ------------------------------------------------------------ META-DESC-DUPLICATE

    /// Solo las tres columnas que la consulta lee. El esquema de verdad lo ejercita
    /// `crawlforge-core/tests/fixtures_de_reglas.rs`, que rastrea el fixture: aquí no se puede
    /// cargar la migración porque `crawlforge-rules` no conoce al core.
    fn conexion_con_paginas(filas: &[(i64, Option<&str>, i64)]) -> Connection {
        let conn = match Connection::open_in_memory() {
            Ok(c) => c,
            Err(e) => panic!("abrir sqlite en memoria: {e}"),
        };
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url_hash INTEGER NOT NULL);
             CREATE TABLE pages (
                 url_id INTEGER PRIMARY KEY REFERENCES urls(id),
                 meta_description TEXT,
                 is_indexable INTEGER NOT NULL
             );",
        )
        .expect("crear el esquema mínimo");
        for (i, (hash, desc, indexable)) in filas.iter().enumerate() {
            let id = i as i64 + 1;
            conn.execute("INSERT INTO urls (id, url_hash) VALUES (?1, ?2)", (id, hash))
                .expect("insertar url");
            conn.execute(
                "INSERT INTO pages (url_id, meta_description, is_indexable) VALUES (?1, ?2, ?3)",
                (id, desc, indexable),
            )
            .expect("insertar página");
        }
        conn
    }

    #[test]
    fn no_avisa_con_descripciones_distintas() {
        let conn = conexion_con_paginas(&[(10, Some("Primera"), 1), (20, Some("Segunda"), 1)]);
        let hallazgos = MetaDescDuplicate.evaluate(&conn).expect("consultar");
        assert!(hallazgos.is_empty());
    }

    #[test]
    fn avisa_de_la_descripcion_repetida_en_las_dos_paginas() {
        let conn = conexion_con_paginas(&[
            (10, Some("La misma de siempre"), 1),
            (20, Some("La misma de siempre"), 1),
            (30, Some("Otra"), 1),
        ]);
        let hallazgos = MetaDescDuplicate.evaluate(&conn).expect("consultar");
        assert_eq!(hallazgos.len(), 2, "el hallazgo se anota en cada página implicada");
        let hashes: Vec<Option<i64>> = hallazgos.iter().map(|(h, _)| *h).collect();
        assert!(hashes.contains(&Some(10)) && hashes.contains(&Some(20)));
        // Las dos comparten `group_key`, que es lo que permite a la UI decir «en 2 páginas».
        assert_eq!(hallazgos[0].1.group_key, hallazgos[1].1.group_key);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"pages\":2"), "{detalle}");
    }

    #[test]
    fn las_paginas_no_indexables_no_cuentan_como_duplicados() {
        // Una descripción repetida entre una página con `noindex` y otra sin él no compite en
        // resultados: no hay dos fragmentos iguales que Google pueda mostrar.
        let conn = conexion_con_paginas(&[(10, Some("Repetida"), 1), (20, Some("Repetida"), 0)]);
        assert!(MetaDescDuplicate.evaluate(&conn).expect("consultar").is_empty());
    }

    #[test]
    fn las_descripciones_ausentes_o_vacias_no_son_duplicados() {
        // Tres páginas sin descripción no son «tres descripciones iguales»: de eso avisa
        // META-DESC-MISSING una vez por página.
        let conn =
            conexion_con_paginas(&[(10, None, 1), (20, None, 1), (30, Some("  "), 1), (40, Some("  "), 1)]);
        assert!(MetaDescDuplicate.evaluate(&conn).expect("consultar").is_empty());
    }
}
