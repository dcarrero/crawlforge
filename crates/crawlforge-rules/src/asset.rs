//! `ASSET` — images and resources. `docs/04-CATALOGO-REGLAS.md §7`.
//!
//! The module splits in two for a reason that is not cosmetic:
//!
//! - What is written in the HTML —a missing `alt` attribute, an `alt=""` that leaves a link
//!   without an accessible name— can be decided with the page in front of you: those are
//!   [`PageRule`]s, evaluated in streaming, and they cost not a single query.
//! - What requires **having requested the resource** —its status code and its actual size— is
//!   only known once the crawl is done: those are [`SiteRule`]s with SQL over the store.
//!
//! The catalog classifies `ASSET-IMG-HEAVY` as a page rule, and it cannot be one: the weight of
//! an image does not appear in any HTML attribute; you have to download it and count the bytes
//! that arrived. The engine already does —every `<img src>` is a crawl URL, with its
//! `urls.content_length`— so the number exists, but in the store, not in the `PageContext`.
//! See [`AssetImgHeavy`].

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::Connection;

/// From here on an image is "heavy": 200 KiB.
///
/// It is the catalog threshold (§7). Measured over the bytes the server returned, which is what
/// the visitor pays for, not over the dimensions declared in the HTML.
pub const HEAVY_IMAGE_MAX_BYTES: i64 = 200 * 1024;

/// From here on a script is "heavy": 250 KiB **as delivered**.
///
/// Measured over `resources.size_bytes`, the bytes the server actually sent, so a bundle served
/// with compression is judged by what the visitor downloads and not by what it weighs on disk.
/// A script this size is a parse-and-execute cost on the main thread of every page that loads
/// it, which is where a bad INP comes from on a mid-range phone.
///
/// The threshold is deliberately above the usual framework runtime —React plus its DOM sits
/// around 45 KB compressed— so what it catches is a bundle nobody has split, not a site that
/// chose a framework.
pub const HEAVY_SCRIPT_MAX_BYTES: i64 = 250 * 1024;

/// From here on a stylesheet is "heavy": 100 KiB as delivered.
///
/// CSS is render-blocking: the browser paints nothing until it has it. That is why the bar is
/// lower than for a script — the same bytes hurt more, and a sheet this size is almost always a
/// whole framework shipped to use a tenth of it.
pub const HEAVY_STYLESHEET_MAX_BYTES: i64 = 100 * 1024;

/// How many URLs are kept in the `detail_json` of a page finding.
///
/// A gallery can bring two hundred images without `alt`. The count stays complete; the list is
/// cut, because the store is not the place to keep two hundred strings per page, and with ten
/// examples the user already knows which template to fix.
const SAMPLE_LIMIT: usize = 10;

pub static ASSET_IMG_NO_ALT: RuleMeta = RuleMeta {
    id: "ASSET-IMG-NO-ALT",
    severity: Severity::High,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Imagen sin atributo alt",
    name_en: "Image without alt attribute",
    desc_es: "Hay imágenes sin atributo `alt`. El lector de pantalla no tiene nada que leer y \
              acaba deletreando el nombre del fichero, y Google pierde el único texto que \
              describe la imagen: es lo que la posiciona en la búsqueda de imágenes y lo que se \
              muestra cuando la foto no carga. Un `alt=\"\"` vacío sí es válido, pero solo para \
              imágenes decorativas.",
    desc_en: "Some images have no `alt` attribute. A screen reader has nothing to read and ends \
              up spelling out the file name, and Google loses the only text describing the \
              image: it is what ranks it in image search and what shows up when the picture \
              fails to load. An empty `alt=\"\"` is valid, but only for decorative images.",
    references: &[],
};

pub static ASSET_IMG_EMPTY_ALT_LINK: RuleMeta = RuleMeta {
    id: "ASSET-IMG-EMPTY-ALT-LINK",
    severity: Severity::High,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Enlace con una sola imagen de alt vacío",
    name_en: "Link with only an empty-alt image",
    desc_es: "Un enlace cuyo único contenido es una imagen con `alt=\"\"` no tiene nombre \
              accesible: quien navega con lector de pantalla oye «enlace» y nada más, y el \
              buscador no recibe ninguna señal de a dónde lleva. El caso típico es el logotipo \
              de la cabecera, que suele ser el enlace más repetido del sitio. Aquí el `alt` no \
              es decorativo: es el texto del enlace.",
    desc_en: "A link whose only content is an image with `alt=\"\"` has no accessible name: \
              someone using a screen reader hears «link» and nothing else, and the search engine \
              gets no signal about where it leads. The typical case is the header logo, usually \
              the most repeated link on the site. Here the `alt` is not decorative: it is the \
              link text.",
    references: &[],
};

pub static ASSET_IMG_BROKEN: RuleMeta = RuleMeta {
    id: "ASSET-IMG-BROKEN",
    severity: Severity::High,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Imagen que no carga",
    name_en: "Broken image",
    desc_es: "Una imagen a la que apunta el sitio devuelve 4xx o 5xx. El hueco se ve en la \
              página, la imagen no existe para la búsqueda de imágenes y cada visita gasta una \
              petición en un error. Casi siempre es una migración que no se llevó la carpeta de \
              subidas, o una ruta escrita a mano.",
    desc_en: "An image the site points to returns 4xx or 5xx. The gap shows on the page, the \
              image does not exist for image search, and every visit spends a request on an \
              error. It is almost always a migration that left the uploads folder behind, or a \
              hand-written path.",
    references: &[],
};

pub static ASSET_IMG_HEAVY: RuleMeta = RuleMeta {
    id: "ASSET-IMG-HEAVY",
    severity: Severity::Medium,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Imagen demasiado pesada",
    name_en: "Oversized image",
    desc_es: "La imagen supera los 200 KB. Es la causa más frecuente de un LCP malo en móvil: \
              retrasa la pintura del elemento principal y se come el ancho de banda de una \
              conexión lenta. Suele bastar con exportarla al tamaño en que se muestra de verdad \
              y servirla en WebP o AVIF.",
    desc_en: "The image is over 200 KB. It is the most common cause of a poor mobile LCP: it \
              delays painting the main element and eats the bandwidth of a slow connection. \
              Exporting it at the size it is actually displayed and serving WebP or AVIF is \
              usually enough.",
    references: &[],
};

pub static ASSET_BROKEN: RuleMeta = RuleMeta {
    id: "ASSET-BROKEN",
    severity: Severity::High,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Hoja de estilo o script que no carga",
    name_en: "Broken CSS or JS",
    desc_es: "Un CSS o un JS del sitio devuelve 4xx o 5xx. Google renderiza la página para \
              indexarla, así que una hoja de estilo ausente puede hacer que la vea sin maquetar \
              —y por tanto no apta para móvil— y un script ausente puede dejar vacío el \
              contenido que se pinta al hidratar. Es típico de un despliegue con los hashes de \
              fichero desincronizados.",
    desc_en: "A CSS or JS file on the site returns 4xx or 5xx. Google renders the page to index \
              it, so a missing stylesheet can make it see an unstyled —hence not \
              mobile-friendly— page, and a missing script can leave client-rendered content \
              empty. It is typical of a deploy with mismatched file hashes.",
    references: &[],
};

pub static ASSET_JS_HEAVY: RuleMeta = RuleMeta {
    id: "ASSET-JS-HEAVY",
    severity: Severity::Medium,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Script demasiado pesado",
    name_en: "Oversized script",
    desc_es: "El script supera los 250 KB tal como se sirve. Cada página que lo carga paga su \
              descarga y, sobre todo, el tiempo de analizarlo y ejecutarlo en el hilo principal, \
              que es de donde sale una mala respuesta al primer toque en un móvil corriente. \
              Suele ser un paquete que nadie ha dividido: mira qué parte hace falta en la \
              primera pantalla y carga el resto aparte.",
    desc_en: "The script is over 250 KB as delivered. Every page that loads it pays for the \
              download and, more to the point, for parsing and executing it on the main thread, \
              which is where poor responsiveness on an ordinary phone comes from. It is usually \
              a bundle nobody has split: work out what is needed for the first screen and load \
              the rest separately.",
    references: &[],
};

pub static ASSET_CSS_HEAVY: RuleMeta = RuleMeta {
    id: "ASSET-CSS-HEAVY",
    severity: Severity::Medium,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Hoja de estilo demasiado pesada",
    name_en: "Oversized stylesheet",
    desc_es: "La hoja de estilo supera los 100 KB tal como se sirve. El navegador no pinta nada \
              hasta tenerla, así que estos bytes cuestan más que los de un script: retrasan la \
              primera pintura entera. El umbral es más bajo por eso. Casi siempre es un \
              framework completo servido para usar una décima parte.",
    desc_en: "The stylesheet is over 100 KB as delivered. The browser paints nothing until it \
              has it, so these bytes cost more than a script's: they delay the whole first \
              paint. That is why the bar is lower. It is nearly always a full framework shipped \
              to use a tenth of it.",
    references: &[],
};

pub static ASSET_IFRAME_BROKEN: RuleMeta = RuleMeta {
    id: "ASSET-IFRAME-BROKEN",
    severity: Severity::High,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Marco embebido que no carga",
    name_en: "Broken embedded frame",
    desc_es: "Un `<iframe>` de la página apunta a una URL propia que devuelve 4xx o 5xx, así que \
              el visitante ve un hueco en blanco donde debería estar el mapa, el vídeo o el \
              formulario. No deja rastro en la página que lo contiene —el HTML sigue siendo \
              válido— y por eso pasa desapercibido durante meses.",
    desc_en: "An `<iframe>` on the page points at a URL of yours returning 4xx or 5xx, so the \
              visitor sees a blank hole where the map, the video or the form should be. It \
              leaves no trace in the containing page —the HTML is still valid— which is why it \
              goes unnoticed for months.",
    references: &[],
};

pub static ASSET_FORM_BROKEN: RuleMeta = RuleMeta {
    id: "ASSET-FORM-BROKEN",
    severity: Severity::Critical,
    category: Category::Asset,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Formulario que envía a una URL rota",
    name_en: "Form posting to a broken URL",
    desc_es: "El `action` de un formulario apunta a una URL propia que devuelve 4xx o 5xx. El \
              formulario se ve bien, se rellena bien y se pierde al enviarlo: es de los pocos \
              defectos que cuestan clientes directamente, y nadie se entera porque quien lo \
              sufre se va sin avisar. Solo se comprueban los formularios que se envían por GET \
              —un buscador, un filtro de catálogo—: comprobar un POST exigiría enviarlo, y esta \
              herramienta no envía formularios.",
    desc_en: "A form's `action` points at a URL of yours returning 4xx or 5xx. The form looks \
              fine, fills in fine and is lost on submit: one of the few defects that costs \
              customers outright, and nobody finds out, because whoever hits it leaves without \
              saying so. Only forms submitted with GET are checked —a search box, a catalogue \
              filter—: checking a POST would mean submitting it, and this tool does not submit \
              forms.",
    references: &[],
};

// ---------------------------------------------------------------- Page rules

/// Images without an `alt` attribute.
///
/// **`None` and `Some("")` are not the same thing.** A missing `alt` is an oversight; an
/// `alt=""` is a deliberate decorative-image decision and valid HTML, so it does not count
/// here. The one case where an `alt=""` is a defect is covered by [`AssetImgEmptyAltLink`].
///
/// One finding per page, not per image: the cause is almost always the template or the content
/// editor, and thirty rows from the same gallery say no more than one row with the count.
pub struct AssetImgNoAlt;

impl PageRule for AssetImgNoAlt {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_NO_ALT
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // A 2xx is required: without it, the theme's error template got audited once per
        // broken URL on the site. See `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        // The page is not required to be indexable: the `alt` is the image's alternative text
        // for whoever visits, and a `noindex` page is reached all the same.
        let sin_alt: Vec<&str> =
            ctx.images.iter().filter(|img| img.alt.is_none()).map(|img| img.src).collect();
        if sin_alt.is_empty() {
            return Vec::new();
        }
        vec![Issue::new(&ASSET_IMG_NO_ALT).with_detail(serde_json::json!({
            "images": sin_alt.len(),
            "sample": sample(&sin_alt),
        }))]
    }
}

/// Image with `alt=""` inside a link that has no other text.
///
/// The link is left without an accessible name: the empty `alt` declares "this image adds no
/// information", and if it is the only thing inside the `<a>`, neither does the link.
///
/// It is the other half of [`AssetImgNoAlt`], and the reason [`crate::ImageView::alt`]
/// distinguishes `None` from `Some("")`: the same markup is correct outside a link and wrong
/// inside one.
///
/// Emits **one finding per distinct image, not per page or per link**, with the image URL as
/// `group_key`. That is the granularity of the cause: the same logo repeated twenty times on
/// the page is one defect (one row, with `occurrences`), and the logo on 18,089 pages is a
/// group the report collapses into a single template problem. The alternative —one row per
/// page with the **set** of images as the key— was tried against a real crawl and grouped
/// badly: the template's logo is on every page, but half of them add their own featured
/// image, the set changes, and the same cause was scattered across 171 single-page groups.
pub struct AssetImgEmptyAltLink;

impl PageRule for AssetImgEmptyAltLink {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_EMPTY_ALT_LINK
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // The 2xx cuts out the error template, where this rule was the noisiest in the catalog:
        // the theme's header logo showed up as a `high` finding on every 404 of the site.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        let sin_nombre: Vec<&str> = ctx
            .images
            .iter()
            // `alt` present and empty. A missing `alt` inside a link also leaves it unnamed,
            // but `ASSET-IMG-NO-ALT` already reports that: two findings on the same `<img>`
            // would be noise.
            .filter(|img| img.alt.is_some_and(|alt| alt.trim().is_empty()))
            // `anchor_text` is `None` when the image does not hang from any link, and
            // `Some("")` when the link has no text beyond the image. Only the second is a
            // defect.
            .filter(|img| img.anchor_text.is_some_and(|texto| texto.trim().is_empty()))
            .map(|img| img.src)
            .collect();
        if sin_nombre.is_empty() {
            return Vec::new();
        }

        // One row per **distinct** image. The `group_key` identifies the cause —the image
        // URL— not the page: the header logo is the same image on the 18,089 pages of a real
        // crawl, so they all share the key and the report can say "one template defect"
        // instead of counting 18,089 rows. It is hashed because an inline `data:` src can
        // measure kilobytes; the readable URL goes in the detail, trimmed when it is `data:`
        // (the base64 locates nothing and weighed 45 MB in a real crawl).
        let mut distintas: Vec<(&str, u32)> = Vec::new();
        for src in sin_nombre {
            match distintas.iter_mut().find(|(s, _)| *s == src) {
                Some((_, veces)) => *veces += 1,
                None => distintas.push((src, 1)),
            }
        }

        distintas
            .into_iter()
            .map(|(src, veces)| {
                let mut detalle = serde_json::json!({ "src": display_src(src) });
                if veces > 1 {
                    if let Some(obj) = detalle.as_object_mut() {
                        obj.insert("occurrences".into(), serde_json::json!(veces));
                    }
                }
                Issue::new(&ASSET_IMG_EMPTY_ALT_LINK).with_detail(detalle).with_group(format!(
                    "img-empty-alt:{:016x}",
                    xxhash_rust::xxh3::xxh3_64(src.as_bytes())
                ))
            })
            .collect()
    }
}

/// How a `src` is stored in a `detail_json`: as is, except `data:` URIs, which are cut at
/// their comma. The type (`data:image/svg+xml;base64,…`) is enough to know what it is; the
/// content locates nothing because it is not a URL you can open, and in a real crawl it was
/// 45 MB of repeated base64.
fn display_src(src: &str) -> String {
    match src.split_once(',') {
        Some((cabecera, _)) if src.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:")) => {
            format!("{cabecera},…")
        }
        _ => src.to_string(),
    }
}

/// Up to [`SAMPLE_LIMIT`] **distinct** URLs from a list, for the `detail_json`.
///
/// Deduplicated: the logo repeated twenty times on the same page filled the sample with twenty
/// copies of the same string. And a `data:` URI is cut at its comma: in a real crawl the
/// sample stored ten copies of the same base64 SVG —dead weight in 18,089 rows that helped
/// locate nothing—. The type (`data:image/svg+xml;base64,…`) is enough to know what it is; the
/// content locates nothing because it is not a URL you can open.
fn sample(srcs: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for src in srcs {
        let entrada = display_src(src);
        if !out.contains(&entrada) {
            out.push(entrada);
        }
        if out.len() >= SAMPLE_LIMIT {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------- Site rules

/// Image returning an error: any 4xx/5xx of your own, 404/410 from someone else's host.
///
/// The finding is recorded **on the image's URL**, not on every page that loads it, with the
/// count of affected pages: the missing file is one and gets fixed once. Same criterion as
/// `HTTP-404-INTERNAL`, whose family it belongs to.
///
/// **External images only assert on the codes the probe can vouch for** (see
/// [`crate::sql_external_gone`]). The concrete, frequent case that forced the split: a
/// hotlinked foreign image behind Referer-based anti-hotlink protection loads fine on the page
/// — the browser sends the Referer — and answers 403 to our probe, which sends none. That was
/// a `high` finding about an image the visitor sees perfectly. And a foreign 5xx is the same
/// volatility `HTTP-404-EXTERNAL` already excludes with its reasoning written down: it entered
/// through this door instead. Your own server keeps the full `>= 400` range: a 403 or a 500 on
/// your own uploads folder is measured with a real request and is yours to fix.
pub struct AssetImgBroken;

impl SiteRule for AssetImgBroken {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // `images.src_url_id` points at the image's `urls` row, which the engine requests like
        // any other URL: that is where its `status_code` comes from. The `COUNT(DISTINCT ...)`
        // is what turns "a file is missing" into "a file is missing and 40 pages load it".
        let sql = format!(
            "SELECT u.url_hash, u.url, u.status_code, COUNT(DISTINCT i.page_url_id) AS pages
             FROM urls u
             JOIN images i ON i.src_url_id = u.id
             WHERE (u.is_internal = 1 AND u.status_code >= 400)
                OR (u.is_internal = 0 AND {externa_rota})
             GROUP BY u.id",
            externa_rota = crate::sql_external_gone("u.status_code"),
        );
        let mut stmt = conn.prepare(&sql)?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, status, pages) = row?;
            out.push((
                Some(hash),
                Issue::new(&ASSET_IMG_BROKEN).with_detail(serde_json::json!({
                    "url": url,
                    "status_code": status,
                    "used_by_pages": pages,
                })),
            ));
        }
        Ok(out)
    }
}

/// Image over [`HEAVY_IMAGE_MAX_BYTES`].
///
/// **It is `site`-scoped even though the catalog lists it as a page rule**, and not out of
/// convenience: the weight of an image is not in the HTML. `width` and `height` declare how it
/// is laid out, not how many bytes the file weighs; that is only known after requesting it,
/// and the number ends up in `urls.content_length`. Making it a page rule would require the
/// `PageContext` to carry the size of every image, which has not been downloaded yet at the
/// moment the page is evaluated.
///
/// `status_code = 200` is required: the body of an error page has a size too, and saying "this
/// image weighs 60 KB" when what arrived is a 404 with pretty HTML would be a made-up finding.
/// The image that fails to load is already reported by [`AssetImgBroken`].
pub struct AssetImgHeavy;

impl SiteRule for AssetImgHeavy {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_HEAVY
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, u.url, u.content_length, COUNT(DISTINCT i.page_url_id) AS pages
             FROM urls u
             JOIN images i ON i.src_url_id = u.id
             WHERE u.status_code = 200 AND u.content_length > ?1
             GROUP BY u.id",
        )?;

        let rows = stmt.query_map([HEAVY_IMAGE_MAX_BYTES], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, bytes, pages) = row?;
            out.push((
                Some(hash),
                Issue::new(&ASSET_IMG_HEAVY).with_detail(serde_json::json!({
                    "url": url,
                    "bytes": bytes,
                    "limit_bytes": HEAVY_IMAGE_MAX_BYTES,
                    "used_by_pages": pages,
                })),
            ));
        }
        Ok(out)
    }
}

/// Stylesheet or script returning an error: any 4xx/5xx of your own, 404/410 from a CDN.
///
/// The parser only records `<link rel="stylesheet">` as `element = 'link'` —the canonical, the
/// `amphtml` and the `hreflang` are not resources and go to their own columns— so the CSS/JS
/// distinction is read off the element itself, without looking at the file extension.
///
/// External resources carry the same restriction as [`AssetImgBroken`], for the same reason: a
/// CDN's 403 to a bot probe or a foreign 5xx during the crawl says nothing about what the
/// visitor's browser gets, and the volatility `HTTP-404-EXTERNAL` excludes on purpose must not
/// re-enter through this rule.
pub struct AssetBroken;

impl SiteRule for AssetBroken {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // Also grouped by `element` so two different uses of the same URL do not mix: a file
        // served both as a stylesheet and as a script is rare, but if it happens those are two
        // findings with two causes.
        let sql = format!(
            "SELECT u.url_hash, u.url, u.status_code,
                    CASE l.element WHEN 'script' THEN 'js' ELSE 'css' END AS kind,
                    COUNT(DISTINCT l.from_url_id) AS pages
             FROM urls u
             JOIN links l ON l.to_url_id = u.id
             WHERE ((u.is_internal = 1 AND u.status_code >= 400)
                 OR (u.is_internal = 0 AND {externa_rota}))
               AND l.element IN ('link', 'script')
             GROUP BY u.id, l.element",
            externa_rota = crate::sql_external_gone("u.status_code"),
        );
        let mut stmt = conn.prepare(&sql)?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, status, kind, pages) = row?;
            out.push((
                Some(hash),
                Issue::new(&ASSET_BROKEN).with_detail(serde_json::json!({
                    "url": url,
                    "status_code": status,
                    "kind": kind,
                    "used_by_pages": pages,
                })),
            ));
        }
        Ok(out)
    }
}

/// A script or a stylesheet the site serves, over its weight limit.
///
/// The two rules are one implementation with two sets of numbers: same query, same shape of
/// finding, different `kind` and different threshold. Writing them twice would mean two places
/// to fix the day the query grows a condition.
///
/// **Only what the site serves itself** (`is_internal = 1`). A heavy script from someone else's
/// CDN is not something the owner can split, and its size arrives from a `HEAD` probe rather
/// than from a body actually downloaded, so asserting on it would be judging a number of a
/// different quality. The rule stays where its advice is actionable.
///
/// Reads `resources`, the table the crawler fills with one row per resource URL. That is also
/// why the finding cannot say how many pages carry the weight: the page-to-resource edge only
/// exists for images (`docs/02-MODELO-DATOS.md §3.5`).
struct HeavyResource {
    meta: &'static RuleMeta,
    kind: &'static str,
    max_bytes: i64,
}

impl SiteRule for HeavyResource {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, u.url, r.size_bytes
             FROM resources r
             JOIN urls u ON u.id = r.url_id
             WHERE r.kind = ?1
               AND r.status_code = 200
               AND u.is_internal = 1
               AND r.size_bytes > ?2
             ORDER BY r.size_bytes DESC",
        )?;

        let rows = stmt.query_map(rusqlite::params![self.kind, self.max_bytes], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, bytes) = row?;
            out.push((
                Some(hash),
                Issue::new(self.meta).with_detail(serde_json::json!({
                    "url": url,
                    "bytes": bytes,
                    "limit_bytes": self.max_bytes,
                })),
            ));
        }
        Ok(out)
    }
}

/// An `<iframe>` or a `<form>` pointing at a URL of the site that answers 4xx or 5xx.
///
/// Same shape for both, and the same reason to exist: neither leaves a mark on the page that
/// contains it. The HTML stays valid, nothing looks wrong in the source, and the defect only
/// shows up to whoever tries to use it — a blank frame, or a form that swallows what was typed.
///
/// **Internal destinations only**, like [`AssetBroken`]: a foreign 403 to a bot probe says
/// nothing about what a visitor's browser gets, and that volatility must not enter through this
/// rule. An embedded map from another provider is exactly the case that would flap.
///
/// One finding per broken destination, not per page that embeds it: it is normally a template,
/// and the count plus a couple of examples say more than four hundred identical rows.
struct BrokenEmbed {
    meta: &'static RuleMeta,
    element: &'static str,
}

impl SiteRule for BrokenEmbed {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT ut.url_hash, ut.url, ut.status_code,
                    COUNT(DISTINCT l.from_url_id) AS pages,
                    MIN(uf.url) AS ejemplo
             FROM links l
             JOIN urls ut ON ut.id = l.to_url_id
             JOIN urls uf ON uf.id = l.from_url_id
             WHERE l.element = ?1
               AND ut.is_internal = 1
               AND ut.status_code >= 400
             GROUP BY ut.id
             ORDER BY pages DESC, ut.url",
        )?;

        let rows = stmt.query_map([self.element], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, status, pages, ejemplo) = row?;
            out.push((
                Some(hash),
                Issue::new(self.meta).with_detail(serde_json::json!({
                    "url": url,
                    "status": status,
                    "used_by_pages": pages,
                    "example_page": ejemplo,
                })),
            ));
        }
        Ok(out)
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![Box::new(AssetImgNoAlt), Box::new(AssetImgEmptyAltLink)]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    vec![
        Box::new(AssetImgBroken),
        Box::new(AssetImgHeavy),
        Box::new(AssetBroken),
        Box::new(HeavyResource {
            meta: &ASSET_JS_HEAVY,
            kind: "js",
            max_bytes: HEAVY_SCRIPT_MAX_BYTES,
        }),
        Box::new(HeavyResource {
            meta: &ASSET_CSS_HEAVY,
            kind: "css",
            max_bytes: HEAVY_STYLESHEET_MAX_BYTES,
        }),
        Box::new(BrokenEmbed { meta: &ASSET_IFRAME_BROKEN, element: "iframe" }),
        Box::new(BrokenEmbed { meta: &ASSET_FORM_BROKEN, element: "form" }),
    ]
}

#[cfg(test)]
mod tests {
    /// The image rules' `JOIN` needs an index to enter through.
    ///
    /// `ASSET-IMG-HEAVY` and `ASSET-IMG-BROKEN` enter through `images.src_url_id` ("which pages
    /// use this image?"), and until migration 007 only the index for the opposite direction
    /// existed. The plan was `SCAN i`: a full scan of `images` **for every candidate URL**.
    ///
    /// It did not hurt because the table was nearly empty on large sites —lazy-load plugins
    /// hid the real `src` in `data-src` and the parser did not read it—. When that was fixed
    /// on 2026-08-02, a news site's table went from 0 to 4,409,298 rows in the same crawl and
    /// the final pass stretched to hours.
    /// Las reglas de marco y formulario no pueden recorrer `links` entero.
    ///
    /// Es el mismo defecto que el de abajo, encontrado el mismo día que nacieron: el plan era
    /// `SCAN l` **por cada una de las dos**, y en la medición de regresión eso costó un 24% del
    /// rendimiento —de 107.702 elementos por segundo a 81.169—. La migración 010 puso el índice
    /// y devolvió el número a 104.493. Un test de plan, y no de tiempo, porque el tiempo mide
    /// también la máquina.
    #[test]
    fn the_embed_rules_do_not_scan_the_whole_links_table() {
        let conn = crate::test_schema::full_schema();

        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT ut.url_hash, COUNT(DISTINCT l.from_url_id)
                 FROM links l
                 JOIN urls ut ON ut.id = l.to_url_id
                 JOIN urls uf ON uf.id = l.from_url_id
                 WHERE l.element = 'iframe' AND ut.is_internal = 1 AND ut.status_code >= 400
                 GROUP BY ut.id",
            )
            .expect("prepare the plan");
        let plan: String = stmt
            .query_map([], |r| r.get::<_, String>(3))
            .expect("read the plan")
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .join(" | ");

        assert!(
            !plan.contains("SCAN l"),
            "los enlaces no se pueden recorrer enteros para buscar dos elementos: {plan}"
        );
        assert!(
            plan.contains("idx_links_element"),
            "la búsqueda tiene que entrar por su índice, y el plan dice: {plan}"
        );
    }

    #[test]
    fn the_image_rules_join_has_an_index_to_enter_through() {
        let conn = crate::test_schema::full_schema();

        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT u.url_hash, COUNT(DISTINCT i.page_url_id)
                 FROM urls u JOIN images i ON i.src_url_id = u.id
                 WHERE u.status_code = 200 AND u.content_length > 204800
                 GROUP BY u.id",
            )
            .expect("prepare the plan");
        let plan: String = stmt
            .query_map([], |r| r.get::<_, String>(3))
            .expect("read the plan")
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .join(" | ");

        assert!(
            !plan.contains("SCAN i"),
            "the images cannot be scanned in full for every URL, and the plan says: {plan}"
        );
        assert!(
            plan.contains("idx_images_src"),
            "the JOIN must enter through its index, and the plan says: {plan}"
        );
    }

    use super::*;
    use crate::ImageView;

    /// An image with a correct `alt` and outside any link: the healthy case.
    fn imagen_sana() -> ImageView<'static> {
        ImageView {
            src: "/img/foto.webp",
            alt: Some("Una descripción de la foto"),
            width_attr: Some(800),
            height_attr: Some(600),
            anchor_text: None,
        }
    }

    fn ctx<'a>(imagenes: &'a [ImageView<'a>]) -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.images = imagenes;
        c
    }

    // --- ASSET-IMG-NO-ALT ---

    #[test]
    fn does_not_warn_when_every_image_has_alt() {
        let imgs = [imagen_sana()];
        assert!(AssetImgNoAlt.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn does_not_warn_on_a_page_with_no_images() {
        assert!(AssetImgNoAlt.evaluate(&ctx(&[])).is_empty());
    }

    #[test]
    fn warns_when_the_alt_attribute_is_missing() {
        let imgs = [ImageView { src: "/img/sin-alt.png", ..Default::default() }];
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "ASSET-IMG-NO-ALT");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn an_empty_alt_is_not_a_missing_alt() {
        // The distinction is why `ImageView::alt` is an `Option`: `alt=""` is valid HTML and
        // declares a decorative image. Conflating them would turn every decorative icon on the
        // site into a false finding.
        let imgs = [ImageView { src: "/img/decorativa.svg", alt: Some(""), ..Default::default() }];
        assert!(
            AssetImgNoAlt.evaluate(&ctx(&imgs)).is_empty(),
            "a deliberate alt=\"\" is not an image without alt"
        );
    }

    #[test]
    fn a_whitespace_only_alt_does_not_count_as_missing_either() {
        // It is an `alt=" "`, which HTML admits. Debatable as a practice, but the attribute is
        // there: warning here would be warning about something else under this rule's ID.
        let imgs = [ImageView { src: "/img/x.png", alt: Some("   "), ..Default::default() }];
        assert!(AssetImgNoAlt.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn a_single_finding_per_page_with_the_full_count() {
        let imgs = [
            ImageView { src: "/1.png", ..Default::default() },
            ImageView { src: "/2.png", ..Default::default() },
            imagen_sana(),
        ];
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        assert_eq!(issues.len(), 1, "a whole gallery must not yield one row per image");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"images\":2"), "detail: {detalle}");
        assert!(detalle.contains("/1.png") && detalle.contains("/2.png"), "detail: {detalle}");
    }

    #[test]
    fn the_detail_sample_is_bounded() {
        // Without the cut, a 200-image gallery would write 200 strings into the store.
        let srcs: Vec<String> = (0..SAMPLE_LIMIT + 5).map(|i| format!("/img-{i:02}.png")).collect();
        let imgs: Vec<ImageView<'_>> =
            srcs.iter().map(|s| ImageView { src: s.as_str(), ..Default::default() }).collect();
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains(&format!("\"images\":{}", SAMPLE_LIMIT + 5)));
        assert_eq!(detalle.matches("/img-").count(), SAMPLE_LIMIT, "detail: {detalle}");
    }

    #[test]
    fn the_sample_does_not_repeat_the_same_url() {
        // Regression from a real crawl: the lazy-load placeholder repeated across twenty
        // `<img>` filled the sample with ten copies of the same string, which locate no more
        // than one does.
        let imgs: Vec<ImageView<'_>> =
            (0..20).map(|_| ImageView { src: "/logo.png", ..Default::default() }).collect();
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"images\":20"), "the count is still complete: {detalle}");
        assert_eq!(detalle.matches("/logo.png").count(), 1, "detail: {detalle}");
    }

    #[test]
    fn a_data_uri_is_not_stored_whole_in_the_sample() {
        // 18,089 rows stored the same base64 SVG as a "sample": dead weight that locates
        // nothing, because a `data:` is not a URL you can open. The type is enough.
        let data = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";
        let imgs = [ImageView { src: data, ..Default::default() }];
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(!detalle.contains("PHN2Zy"), "the base64 is not stored: {detalle}");
        assert!(
            detalle.contains("data:image/svg+xml;base64,…"),
            "the type is, so we know what it is: {detalle}"
        );
    }

    #[test]
    fn does_not_warn_on_something_that_is_not_html() {
        let imgs = [ImageView { src: "/x.png", ..Default::default() }];
        let mut c = ctx(&imgs);
        c.is_html = false;
        assert!(AssetImgNoAlt.evaluate(&c).is_empty());
    }

    #[test]
    fn warns_on_a_non_indexable_page_too() {
        // The alternative text is needed by whoever visits the page, and that page is reached
        // even if it carries `noindex`.
        let imgs = [ImageView { src: "/x.png", ..Default::default() }];
        let mut c = ctx(&imgs);
        c.is_indexable = false;
        assert_eq!(AssetImgNoAlt.evaluate(&c).len(), 1);
    }

    #[test]
    fn the_error_template_is_not_audited() {
        // Regression from a real crawl: the theme's 404 template, with its empty-alt logo,
        // produced one finding per broken URL on the site —26 in one crawl, 12 in another—.
        // The actionable finding of a 404 is the 404, which already has its HTTP rule.
        let imgs = [
            ImageView { src: "/sin-alt.png", ..Default::default() },
            ImageView { src: "/logo.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() },
        ];
        for status in [301, 404, 410, 500] {
            let mut c = ctx(&imgs);
            c.status = status;
            assert!(
                AssetImgNoAlt.evaluate(&c).is_empty(),
                "ASSET-IMG-NO-ALT should not audit the HTML of a {status}"
            );
            assert!(
                AssetImgEmptyAltLink.evaluate(&c).is_empty(),
                "ASSET-IMG-EMPTY-ALT-LINK should not audit the HTML of a {status}"
            );
        }
    }

    // --- ASSET-IMG-EMPTY-ALT-LINK ---

    #[test]
    fn warns_when_the_link_carries_only_an_empty_alt_image() {
        let imgs = [ImageView {
            src: "/logo.svg",
            alt: Some(""),
            anchor_text: Some(""),
            ..Default::default()
        }];
        let issues = AssetImgEmptyAltLink.evaluate(&ctx(&imgs));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "ASSET-IMG-EMPTY-ALT-LINK");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn does_not_warn_when_the_link_has_its_own_text() {
        // The link's text provides the accessible name, so the `alt=""` is correct: the image
        // is decorative and repeating the text in the `alt` would be redundant.
        let imgs = [ImageView {
            src: "/icono.svg",
            alt: Some(""),
            anchor_text: Some("Ver el informe"),
            ..Default::default()
        }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn does_not_warn_when_the_decorative_image_is_not_inside_a_link() {
        // `anchor_text: None` means "this image hangs from no <a>". Outside a link, an
        // `alt=""` is exactly what should be written.
        let imgs = [ImageView { src: "/adorno.svg", alt: Some(""), ..Default::default() }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn does_not_warn_when_the_link_image_describes_the_destination() {
        let imgs = [ImageView {
            src: "/logo.svg",
            alt: Some("Portada de CrawlForge"),
            anchor_text: Some(""),
            ..Default::default()
        }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn a_missing_alt_inside_a_link_is_counted_by_the_other_rule() {
        // The link is left unnamed too, but the defect to fix is the missing `alt`. Two
        // findings on the same `<img>` would be noise.
        let imgs = [ImageView { src: "/logo.svg", anchor_text: Some(""), ..Default::default() }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
        assert_eq!(AssetImgNoAlt.evaluate(&ctx(&imgs)).len(), 1);
    }

    #[test]
    fn a_link_with_whitespace_only_text_still_has_no_name() {
        // `<a href="/"> <img alt=""> </a>`: the link's text is a space, which names nothing.
        let imgs = [ImageView {
            src: "/logo.svg",
            alt: Some(""),
            anchor_text: Some("  \n "),
            ..Default::default()
        }];
        assert_eq!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).len(), 1);
    }

    #[test]
    fn one_finding_per_distinct_image_not_per_link() {
        // Two distinct images are two causes: each with its row and its key. The same image
        // repeated is one cause with a count: the logo twenty times is not twenty rows.
        let dos = [
            ImageView { src: "/a.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() },
            ImageView { src: "/b.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() },
        ];
        let issues = AssetImgEmptyAltLink.evaluate(&ctx(&dos));
        assert_eq!(issues.len(), 2, "two distinct images, two findings");
        assert_ne!(issues[0].group_key, issues[1].group_key);

        let logo =
            ImageView { src: "/logo.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() };
        let repetida = [logo, logo, logo];
        let issues = AssetImgEmptyAltLink.evaluate(&ctx(&repetida));
        assert_eq!(issues.len(), 1, "the same image three times is one cause");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"occurrences\":3"), "detail: {detalle}");
        assert!(detalle.contains("/logo.svg"), "detail: {detalle}");
    }

    #[test]
    fn the_unnamed_link_detail_does_not_store_the_base64() {
        let data = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";
        let imgs = [ImageView { src: data, alt: Some(""), anchor_text: Some(""), ..Default::default() }];
        let issues = AssetImgEmptyAltLink.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(!detalle.contains("PHN2Zy"), "the base64 is not stored: {detalle}");
        assert!(detalle.contains("data:image/svg+xml;base64,…"), "{detalle}");
    }

    #[test]
    fn the_same_logo_on_two_pages_shares_a_group() {
        // The header logo was 90% of a real crawl's `high` findings: the key is the image, not
        // the page, so the report counts it as a single template defect.
        let imgs =
            [ImageView { src: "/logo.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() }];
        let a = AssetImgEmptyAltLink.evaluate(&ctx(&imgs));
        let mut c2 = PageContext::indexable_html("https://ejemplo.es/otra");
        c2.images = &imgs;
        let b = AssetImgEmptyAltLink.evaluate(&c2);
        assert!(
            a[0].group_key.as_deref().is_some_and(|k| k.starts_with("img-empty-alt:")),
            "{:?}",
            a[0].group_key
        );
        assert_eq!(a[0].group_key, b[0].group_key);
    }

    #[test]
    fn another_image_is_another_group_and_repetitions_do_not_change_it() {
        let logo =
            ImageView { src: "/logo.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() };
        let banner =
            ImageView { src: "/banner.png", alt: Some(""), anchor_text: Some(""), ..Default::default() };

        let solo_logo = [logo];
        let logo_repetido = [logo, logo];
        let solo_banner = [banner];

        let k_logo = AssetImgEmptyAltLink.evaluate(&ctx(&solo_logo))[0].group_key.clone();
        let k_repetido = AssetImgEmptyAltLink.evaluate(&ctx(&logo_repetido))[0].group_key.clone();
        let k_banner = AssetImgEmptyAltLink.evaluate(&ctx(&solo_banner))[0].group_key.clone();

        assert_eq!(k_logo, k_repetido, "the logo twice is still the same cause");
        assert_ne!(k_logo, k_banner, "another image is another cause");
    }

    #[test]
    fn the_unnamed_link_is_not_evaluated_outside_html() {
        let imgs = [ImageView {
            src: "/logo.svg",
            alt: Some(""),
            anchor_text: Some(""),
            ..Default::default()
        }];
        let mut c = ctx(&imgs);
        c.is_html = false;
        assert!(AssetImgEmptyAltLink.evaluate(&c).is_empty());
    }

    // --- Site rules ---
    //
    // The real test of these three is their fixture, crawled end to end in
    // `crawlforge-core/tests/fixtures_de_reglas.rs`: it is the only thing that proves the
    // engine fills the columns these queries read. What is checked here is the query against
    // the minimum schema it uses, which is what catches a misspelled column name or a
    // threshold compared the wrong way without waiting for a full crawl.

    /// The `001_initial.sql` columns these rules touch, and only those.
    fn conn_minima() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        conn.execute_batch(
            "CREATE TABLE urls (
                 id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, url_hash INTEGER NOT NULL,
                 is_internal INTEGER NOT NULL DEFAULT 1, status_code INTEGER,
                 content_length INTEGER
             );
             CREATE TABLE images (
                 id INTEGER PRIMARY KEY, page_url_id INTEGER NOT NULL, src_url_id INTEGER NOT NULL
             );
             CREATE TABLE links (
                 id INTEGER PRIMARY KEY, from_url_id INTEGER NOT NULL, to_url_id INTEGER NOT NULL,
                 element TEXT NOT NULL
             );",
        )
        .expect("minimal schema");
        conn
    }

    /// Inserts a URL. The `url_hash` is derived from the id so the tests can check which row a
    /// finding attaches to.
    fn url(conn: &Connection, id: i64, url: &str, status: Option<i64>, bytes: Option<i64>) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, status_code, content_length)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, url, id * 1000, status, bytes],
        )
        .expect("insert url");
    }

    /// Inserts a URL on someone else's host, as the external status probe leaves it.
    fn url_externa(conn: &Connection, id: i64, url: &str, status: Option<i64>) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, status_code, is_internal)
             VALUES (?1, ?2, ?3, ?4, 0)",
            rusqlite::params![id, url, id * 1000, status],
        )
        .expect("insert external url");
    }

    #[test]
    fn detects_the_image_returning_an_error_and_counts_the_pages() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/b", Some(200), Some(2_000));
        url(&conn, 3, "https://ejemplo.es/rota.png", Some(404), Some(0));
        url(&conn, 4, "https://ejemplo.es/bien.png", Some(200), Some(1_000));
        conn.execute_batch(
            "INSERT INTO images (page_url_id, src_url_id) VALUES (1, 3), (2, 3), (1, 4);",
        )
        .expect("insert images");

        let hallazgos = AssetImgBroken.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "one finding per missing file, not per page");
        assert_eq!(hallazgos[0].0, Some(3_000), "it is recorded on the image URL");
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"used_by_pages\":2"), "detail: {detalle}");
    }

    #[test]
    fn detects_the_image_over_the_weight_threshold() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/pesada.jpg", Some(200), Some(HEAVY_IMAGE_MAX_BYTES + 1));
        url(&conn, 3, "https://ejemplo.es/justa.jpg", Some(200), Some(HEAVY_IMAGE_MAX_BYTES));
        url(&conn, 4, "https://ejemplo.es/error.jpg", Some(404), Some(HEAVY_IMAGE_MAX_BYTES * 2));
        conn.execute_batch(
            "INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2), (1, 3), (1, 4);",
        )
        .expect("insert images");

        let hallazgos = AssetImgHeavy.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "the threshold is strict and the body of a 404 is not measured");
        assert_eq!(hallazgos[0].0, Some(2_000));
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains(&format!("\"bytes\":{}", HEAVY_IMAGE_MAX_BYTES + 1)));
    }

    #[test]
    fn detects_broken_css_and_js_and_tells_them_apart() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/e.css", Some(404), Some(0));
        url(&conn, 3, "https://ejemplo.es/a.js", Some(500), Some(0));
        url(&conn, 4, "https://ejemplo.es/b", Some(404), Some(0));
        conn.execute_batch(
            "INSERT INTO links (from_url_id, to_url_id, element)
             VALUES (1, 2, 'link'), (1, 3, 'script'), (1, 4, 'a'), (1, 2, 'link');",
        )
        .expect("insert links");

        let hallazgos = AssetBroken.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 2, "a broken <a> is not a resource: that is HTTP-404-INTERNAL");
        let detalles: Vec<String> =
            hallazgos.iter().map(|(_, i)| i.detail_json.clone().unwrap_or_default()).collect();
        assert!(detalles.iter().any(|d| d.contains("\"kind\":\"css\"")), "{detalles:?}");
        assert!(detalles.iter().any(|d| d.contains("\"kind\":\"js\"")), "{detalles:?}");
        assert!(
            detalles.iter().all(|d| d.contains("\"used_by_pages\":1")),
            "the same stylesheet cited twice on one page is one page: {detalles:?}"
        );
    }

    #[test]
    fn a_hotlink_wall_403_is_not_a_broken_image() {
        // Proves the fix. A foreign image behind Referer-based anti-hotlink protection loads
        // fine on the page — the browser sends the Referer — and answers 403 to our probe,
        // which sends none. Reporting it was a `high` finding about an image the visitor sees
        // perfectly. Same for the other codes a wall answers a bot with.
        for status in [401, 403, 429] {
            let conn = conn_minima();
            url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
            url_externa(&conn, 2, "https://fotos.example/protegida.jpg", Some(status));
            conn.execute_batch("INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2);")
                .expect("insert image");

            assert!(
                AssetImgBroken.evaluate(&conn).expect("evaluate").is_empty(),
                "a foreign {status} says nothing about what the visitor's browser gets"
            );
        }
    }

    #[test]
    fn a_foreign_5xx_is_not_a_broken_image() {
        // Proves the fix. The volatility HTTP-404-EXTERNAL excludes with its reasoning written
        // down — someone else's 5xx is almost always transient — entered through this rule
        // instead. Now the same criterion holds at both doors.
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url_externa(&conn, 2, "https://fotos.example/caida.jpg", Some(503));
        conn.execute_batch("INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2);")
            .expect("insert image");

        assert!(AssetImgBroken.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_foreign_image_that_is_gone_is_still_broken() {
        // Guard: narrowing the external codes must not silence the true case.
        for status in [404, 410] {
            let conn = conn_minima();
            url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
            url_externa(&conn, 2, "https://fotos.example/borrada.jpg", Some(status));
            conn.execute_batch("INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2);")
                .expect("insert image");

            assert_eq!(
                AssetImgBroken.evaluate(&conn).expect("evaluate").len(),
                1,
                "a foreign {status} is the origin stating the image is gone"
            );
        }
    }

    #[test]
    fn an_own_403_or_5xx_image_is_still_broken() {
        // Guard: the restriction is for hosts we do not control. Your own uploads folder
        // answering 403 or 500 is measured with a real request and is yours to fix.
        for status in [403, 500] {
            let conn = conn_minima();
            url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
            url(&conn, 2, "https://ejemplo.es/uploads/rota.jpg", Some(status), Some(0));
            conn.execute_batch("INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2);")
                .expect("insert image");

            assert_eq!(AssetImgBroken.evaluate(&conn).expect("evaluate").len(), 1, "{status}");
        }
    }

    #[test]
    fn a_cdn_wall_or_hiccup_is_not_a_broken_resource() {
        // Proves the fix for ASSET-BROKEN: same criterion as the images, same reasons.
        for status in [403, 429, 503] {
            let conn = conn_minima();
            url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
            url_externa(&conn, 2, "https://cdn.example/lib.js", Some(status));
            conn.execute_batch(
                "INSERT INTO links (from_url_id, to_url_id, element) VALUES (1, 2, 'script');",
            )
            .expect("insert link");

            assert!(
                AssetBroken.evaluate(&conn).expect("evaluate").is_empty(),
                "a foreign {status} says nothing about what the visitor's browser gets"
            );
        }
    }

    #[test]
    fn a_cdn_resource_that_is_gone_is_still_broken() {
        // Guard: a stylesheet a CDN answers 404 for is gone for every visitor too.
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url_externa(&conn, 2, "https://cdn.example/tema.css", Some(404));
        conn.execute_batch(
            "INSERT INTO links (from_url_id, to_url_id, element) VALUES (1, 2, 'link');",
        )
        .expect("insert link");

        let hallazgos = AssetBroken.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"kind\":\"css\""), "detail: {detalle}");
    }

    #[test]
    fn a_store_with_no_defects_produces_no_findings() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/bien.webp", Some(200), Some(30_000));
        url(&conn, 3, "https://ejemplo.es/e.css", Some(200), Some(4_000));
        conn.execute_batch(
            "INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2);
             INSERT INTO links (from_url_id, to_url_id, element) VALUES (1, 3, 'link');",
        )
        .expect("populate");

        assert!(AssetImgBroken.evaluate(&conn).expect("evaluate").is_empty());
        assert!(AssetImgHeavy.evaluate(&conn).expect("evaluate").is_empty());
        assert!(AssetBroken.evaluate(&conn).expect("evaluate").is_empty());
    }

    // --- ASSET-JS-HEAVY · ASSET-CSS-HEAVY · ASSET-IFRAME-BROKEN · ASSET-FORM-BROKEN ---

    /// Esquema real con una URL y su fila de `resources`, que es lo que leen las reglas de peso.
    fn db_recursos() -> Connection {
        let conn = crate::test_schema::full_schema();
        conn.pragma_update(None, "foreign_keys", false).expect("disable foreign keys");
        conn
    }

    fn recurso(conn: &Connection, id: i64, url: &str, interno: bool, kind: &str, bytes: i64) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', '/', ?3, 0, 'done', 200)",
            rusqlite::params![id, url, interno as i64],
        )
        .expect("insertar url");
        conn.execute(
            "INSERT INTO resources (url_id, kind, status_code, size_bytes, mime)
             VALUES (?1, ?2, 200, ?3, NULL)",
            rusqlite::params![id, kind, bytes],
        )
        .expect("insertar recurso");
    }

    fn regla_js() -> HeavyResource {
        HeavyResource { meta: &ASSET_JS_HEAVY, kind: "js", max_bytes: HEAVY_SCRIPT_MAX_BYTES }
    }

    #[test]
    fn a_script_over_the_limit_is_reported_and_one_under_it_is_not() {
        let conn = db_recursos();
        recurso(&conn, 1, "https://ejemplo.es/js/tienda.js", true, "js", HEAVY_SCRIPT_MAX_BYTES + 1);
        recurso(&conn, 2, "https://ejemplo.es/js/poco.js", true, "js", HEAVY_SCRIPT_MAX_BYTES - 1);

        let hallazgos = regla_js().evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "solo el que se pasa");
        assert_eq!(hallazgos[0].1.rule_id, "ASSET-JS-HEAVY");
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("tienda.js"), "el detalle nombra el fichero: {detalle}");
    }

    #[test]
    fn the_limit_itself_is_not_over_the_limit() {
        // Justo en el umbral no se avisa: `>` y no `>=`, igual que en ASSET-IMG-HEAVY.
        let conn = db_recursos();
        recurso(&conn, 1, "https://ejemplo.es/js/justo.js", true, "js", HEAVY_SCRIPT_MAX_BYTES);
        assert!(regla_js().evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_heavy_script_from_someone_else_is_not_reported() {
        // No es algo que el dueño del sitio pueda dividir, y su tamaño viene de una sonda `HEAD`
        // y no de un cuerpo descargado: afirmar sobre él sería juzgar un número de otra calidad.
        let conn = db_recursos();
        recurso(&conn, 1, "https://cdn-ajeno.com/todo.js", false, "js", HEAVY_SCRIPT_MAX_BYTES * 4);
        assert!(regla_js().evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn each_weight_rule_looks_only_at_its_own_kind() {
        // Una hoja de 200 KB pasa del umbral del CSS y no del de un script. Si las dos reglas
        // compartieran filtro, esa hoja saldría dos veces y una de ellas con el nombre cambiado.
        let conn = db_recursos();
        recurso(&conn, 1, "https://ejemplo.es/css/framework.css", true, "css", 200 * 1024);

        let css = HeavyResource {
            meta: &ASSET_CSS_HEAVY,
            kind: "css",
            max_bytes: HEAVY_STYLESHEET_MAX_BYTES,
        };
        assert_eq!(css.evaluate(&conn).expect("evaluate").len(), 1, "el CSS sí");
        assert!(regla_js().evaluate(&conn).expect("evaluate").is_empty(), "la regla del JS no");
    }

    /// Inserta una página que embebe algo, y el destino con su estado.
    fn embebido(conn: &Connection, element: &str, destino_interno: bool, estado: i64) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (1, 'https://ejemplo.es/pagina', 1, 'https', 'ejemplo.es', '/pagina', 1, 0,
                     'done', 200)",
            [],
        )
        .expect("insertar página");
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (2, 'https://ejemplo.es/destino', 2, 'https', 'ejemplo.es', '/destino', ?1, 0,
                     'done', ?2)",
            rusqlite::params![destino_interno as i64, estado],
        )
        .expect("insertar destino");
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, anchor, rel, is_nofollow, element,
                                region, position)
             VALUES (1, 2, NULL, NULL, 0, ?1, 'main', 0)",
            [element],
        )
        .expect("insertar enlace");
    }

    #[test]
    fn a_broken_iframe_is_reported_with_the_page_that_embeds_it() {
        let conn = db_recursos();
        embebido(&conn, "iframe", true, 404);
        let regla = BrokenEmbed { meta: &ASSET_IFRAME_BROKEN, element: "iframe" };
        let hallazgos = regla.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.rule_id, "ASSET-IFRAME-BROKEN");
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("/pagina"), "dice en qué página está: {detalle}");
    }

    #[test]
    fn an_iframe_pointing_somewhere_alive_is_not_reported() {
        let conn = db_recursos();
        embebido(&conn, "iframe", true, 200);
        let regla = BrokenEmbed { meta: &ASSET_IFRAME_BROKEN, element: "iframe" };
        assert!(regla.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn someone_elses_broken_embed_is_not_ours_to_report() {
        // Un 403 de un proveedor a una sonda de bot no dice nada de lo que ve el visitante, y esa
        // volatilidad no puede entrar por esta regla. Es la misma restricción de ASSET-BROKEN.
        let conn = db_recursos();
        embebido(&conn, "iframe", false, 403);
        let regla = BrokenEmbed { meta: &ASSET_IFRAME_BROKEN, element: "iframe" };
        assert!(regla.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_broken_form_target_is_reported_as_critical() {
        let conn = db_recursos();
        embebido(&conn, "form", true, 404);
        let regla = BrokenEmbed { meta: &ASSET_FORM_BROKEN, element: "form" };
        let hallazgos = regla.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.severity, Severity::Critical, "un formulario perdido cuesta clientes");
    }

    #[test]
    fn the_two_embed_rules_do_not_read_each_others_rows() {
        let conn = db_recursos();
        embebido(&conn, "form", true, 404);
        let iframe = BrokenEmbed { meta: &ASSET_IFRAME_BROKEN, element: "iframe" };
        assert!(
            iframe.evaluate(&conn).expect("evaluate").is_empty(),
            "un formulario roto no es un marco roto"
        );
    }
}
