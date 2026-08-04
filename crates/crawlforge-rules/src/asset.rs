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

/// Image returning 4xx or 5xx.
///
/// The finding is recorded **on the image's URL**, not on every page that loads it, with the
/// count of affected pages: the missing file is one and gets fixed once. Same criterion as
/// `HTTP-404-INTERNAL`, whose family it belongs to.
pub struct AssetImgBroken;

impl SiteRule for AssetImgBroken {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // `images.src_url_id` points at the image's `urls` row, which the engine requests like
        // any other URL: that is where its `status_code` comes from. The `COUNT(DISTINCT ...)`
        // is what turns "a file is missing" into "a file is missing and 40 pages load it".
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, u.url, u.status_code, COUNT(DISTINCT i.page_url_id) AS pages
             FROM urls u
             JOIN images i ON i.src_url_id = u.id
             WHERE u.status_code >= 400
             GROUP BY u.id",
        )?;

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

/// Stylesheet or script returning 4xx or 5xx.
///
/// The parser only records `<link rel="stylesheet">` as `element = 'link'` —the canonical, the
/// `amphtml` and the `hreflang` are not resources and go to their own columns— so the CSS/JS
/// distinction is read off the element itself, without looking at the file extension.
pub struct AssetBroken;

impl SiteRule for AssetBroken {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // Also grouped by `element` so two different uses of the same URL do not mix: a file
        // served both as a stylesheet and as a script is rare, but if it happens those are two
        // findings with two causes.
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, u.url, u.status_code,
                    CASE l.element WHEN 'script' THEN 'js' ELSE 'css' END AS kind,
                    COUNT(DISTINCT l.from_url_id) AS pages
             FROM urls u
             JOIN links l ON l.to_url_id = u.id
             WHERE u.status_code >= 400 AND l.element IN ('link', 'script')
             GROUP BY u.id, l.element",
        )?;

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

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![Box::new(AssetImgNoAlt), Box::new(AssetImgEmptyAltLink)]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    vec![Box::new(AssetImgBroken), Box::new(AssetImgHeavy), Box::new(AssetBroken)]
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
}
