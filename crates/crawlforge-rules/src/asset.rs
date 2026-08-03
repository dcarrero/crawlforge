//! `ASSET` — imágenes y recursos. `docs/04-CATALOGO-REGLAS.md §7`.
//!
//! El módulo se parte en dos por una razón que no es estética:
//!
//! - Lo que está escrito en el HTML —falta el atributo `alt`, un `alt=""` que deja un enlace sin
//!   nombre accesible— se decide con la página delante: son [`PageRule`], se evalúan en
//!   streaming y no cuestan ni una consulta.
//! - Lo que exige **haber pedido el recurso** —su código de estado y su tamaño real— solo se
//!   sabe con el rastreo terminado: son [`SiteRule`] con SQL sobre el almacén.
//!
//! El catálogo clasifica `ASSET-IMG-HEAVY` como regla de página, y no puede serlo: el peso de una
//! imagen no aparece en ningún atributo del HTML, hay que descargarla y contar los bytes que
//! llegaron. El motor ya lo hace —cada `<img src>` es una URL del rastreo, con su
//! `urls.content_length`— así que el dato existe, pero en el almacén y no en el `PageContext`.
//! Ver [`AssetImgHeavy`].

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::Connection;

/// A partir de aquí una imagen es «pesada»: 200 KiB.
///
/// Es el umbral del catálogo (§7). Se mide sobre los bytes que devolvió el servidor, que es lo
/// que paga el visitante, y no sobre las dimensiones declaradas en el HTML.
pub const HEAVY_IMAGE_MAX_BYTES: i64 = 200 * 1024;

/// Cuántas URLs se guardan en el `detail_json` de un hallazgo de página.
///
/// Una galería puede traer doscientas imágenes sin `alt`. El recuento va completo; la lista se
/// corta, porque el almacén no es el sitio donde guardar doscientas cadenas por página y con diez
/// ejemplos el usuario ya sabe qué plantilla arreglar.
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

// ---------------------------------------------------------------- Reglas de página

/// Imágenes sin atributo `alt`.
///
/// **`None` y `Some("")` no son lo mismo.** Un `alt` ausente es un descuido; un `alt=""` es una
/// decisión deliberada de imagen decorativa y es HTML válido, así que aquí no cuenta. El único
/// caso en que un `alt=""` sí es un defecto lo cubre [`AssetImgEmptyAltLink`].
///
/// Un hallazgo por página y no por imagen: la causa está casi siempre en la plantilla o en el
/// editor de contenidos, y treinta filas de la misma galería no dicen más que una con el recuento.
pub struct AssetImgNoAlt;

impl PageRule for AssetImgNoAlt {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_NO_ALT
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // Sí se exige un 2xx: sin él, la plantilla de error del tema se auditaba una vez por
        // cada URL rota del sitio. Ver `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        // No se exige que la página sea indexable: el `alt` es el texto alternativo de la imagen
        // para quien la visita, y a una página con `noindex` se llega igual.
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

/// Imagen con `alt=""` dentro de un enlace que no tiene ningún otro texto.
///
/// El enlace se queda sin nombre accesible: el `alt` vacío declara «esta imagen no aporta
/// información», y si es lo único que hay dentro del `<a>`, tampoco la aporta el enlace.
///
/// Es la otra mitad de [`AssetImgNoAlt`], y la razón por la que [`crate::ImageView::alt`]
/// distingue `None` de `Some("")`: la misma marca es correcta fuera de un enlace e incorrecta
/// dentro.
///
/// Emite **un hallazgo por imagen distinta, no por página ni por enlace**, con la URL de la
/// imagen como `group_key`. Es la granularidad de la causa: el mismo logo repetido veinte veces
/// en la página es un defecto (una fila, con `occurrences`), y el logo en 18.089 páginas es un
/// grupo que el informe colapsa a un solo problema de plantilla. La alternativa de una fila por
/// página con el **conjunto** de imágenes como clave se probó contra un rastreo real y agrupaba
/// mal: el logo de la plantilla está en todas las páginas, pero la mitad de ellas añade su
/// imagen destacada propia, el conjunto cambia, y la misma causa se repartía en 171 grupos de
/// una página.
pub struct AssetImgEmptyAltLink;

impl PageRule for AssetImgEmptyAltLink {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_EMPTY_ALT_LINK
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // El 2xx corta la plantilla de error, donde esta regla era la más ruidosa del catálogo:
        // el logo de la cabecera del tema aparecía como hallazgo `high` en cada 404 del sitio.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        let sin_nombre: Vec<&str> = ctx
            .images
            .iter()
            // `alt` presente y vacío. Un `alt` ausente dentro de un enlace también lo deja sin
            // nombre, pero de eso ya avisa `ASSET-IMG-NO-ALT`: dos hallazgos sobre el mismo
            // `<img>` serían ruido.
            .filter(|img| img.alt.is_some_and(|alt| alt.trim().is_empty()))
            // `anchor_text` es `None` cuando la imagen no cuelga de ningún enlace, y `Some("")`
            // cuando el enlace no tiene más texto que la imagen. Solo el segundo es un defecto.
            .filter(|img| img.anchor_text.is_some_and(|texto| texto.trim().is_empty()))
            .map(|img| img.src)
            .collect();
        if sin_nombre.is_empty() {
            return Vec::new();
        }

        // Una fila por imagen **distinta**. El `group_key` identifica la causa —la URL de la
        // imagen— y no la página: el logo de la cabecera es la misma imagen en las 18.089
        // páginas de un rastreo real, así que todas comparten clave y el informe puede decir
        // «un defecto de plantilla» en vez de contar 18.089 filas. Se hashea porque un `src`
        // inline `data:` puede medir kilobytes; la URL legible va en el detalle, recortada si
        // es `data:` (el base64 no localiza nada y pesaba 45 MB en un rastreo real).
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

/// La forma en que un `src` se guarda en un `detail_json`: tal cual, salvo las URIs `data:`,
/// que se cortan en su coma. El tipo (`data:image/svg+xml;base64,…`) basta para saber qué es;
/// el contenido no localiza nada porque no es una URL que se pueda abrir, y en un rastreo real
/// eran 45 MB de base64 repetido.
fn display_src(src: &str) -> String {
    match src.split_once(',') {
        Some((cabecera, _)) if src.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:")) => {
            format!("{cabecera},…")
        }
        _ => src.to_string(),
    }
}

/// Hasta [`SAMPLE_LIMIT`] URLs **distintas** de una lista, para el `detail_json`.
///
/// Deduplicada: el logo repetido veinte veces en la misma página llenaba la muestra con veinte
/// copias de la misma cadena. Y una URI `data:` se corta en su coma: en un rastreo real la
/// muestra guardaba diez copias del mismo SVG en base64 —peso muerto en 18.089 filas que no
/// ayudaba a localizar nada—. El tipo (`data:image/svg+xml;base64,…`) basta para saber qué es;
/// el contenido no localiza nada porque no es una URL que se pueda abrir.
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

// ---------------------------------------------------------------- Reglas de conjunto

/// Imagen que devuelve 4xx o 5xx.
///
/// El hallazgo se registra **en la URL de la imagen**, no en cada página que la carga, con el
/// recuento de páginas afectadas: el fichero que falta es uno y se arregla una vez. Es el mismo
/// criterio que `HTTP-404-INTERNAL`, con el que forma familia.
pub struct AssetImgBroken;

impl SiteRule for AssetImgBroken {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_IMG_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // `images.src_url_id` apunta a la fila de `urls` de la imagen, que el motor pide como
        // cualquier otra URL: de ahí sale su `status_code`. El `COUNT(DISTINCT ...)` es lo que
        // convierte «falta un fichero» en «falta un fichero y lo cargan 40 páginas».
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

/// Imagen de más de [`HEAVY_IMAGE_MAX_BYTES`].
///
/// **Es de alcance `site` aunque el catálogo la liste como de página**, y no por comodidad: el
/// peso de una imagen no está en el HTML. `width` y `height` declaran cómo se maqueta, no cuántos
/// bytes pesa el fichero; eso solo se sabe tras pedirlo, y el número acaba en
/// `urls.content_length`. Hacerla de página exigiría que el `PageContext` trajera el tamaño de
/// cada imagen, que en el momento de evaluar la página todavía no se ha descargado.
///
/// Se exige `status_code = 200`: el cuerpo de una página de error también tiene tamaño, y decir
/// «esta imagen pesa 60 KB» cuando lo que llegó es un 404 con un HTML bonito sería un hallazgo
/// inventado. De la imagen que no carga ya avisa [`AssetImgBroken`].
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

/// Hoja de estilo o script que devuelve 4xx o 5xx.
///
/// El parser solo registra como `element = 'link'` los `<link rel="stylesheet">` —el canonical,
/// el `amphtml` y los `hreflang` no son recursos y van a sus propias columnas—, así que la
/// distinción entre CSS y JS se lee del propio elemento sin mirar la extensión del fichero.
pub struct AssetBroken;

impl SiteRule for AssetBroken {
    fn meta(&self) -> &'static RuleMeta {
        &ASSET_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // Se agrupa también por `element` para no mezclar dos usos distintos de la misma URL: un
        // fichero servido a la vez como hoja de estilo y como script es raro, pero si pasa son
        // dos hallazgos con dos causas.
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
    /// El `JOIN` de las reglas de imagen tiene que tener índice por donde entrar.
    ///
    /// `ASSET-IMG-HEAVY` y `ASSET-IMG-BROKEN` entran por `images.src_url_id` («¿qué páginas usan
    /// esta imagen?»), y hasta la migración 007 solo existía el índice de la dirección contraria.
    /// El plan era `SCAN i`: un recorrido completo de `images` **por cada URL candidata**.
    ///
    /// No dolía porque la tabla estaba casi vacía en los sitios grandes —los plugins de
    /// *lazy-load* escondían el `src` real en `data-src` y el parser no lo leía—. Al arreglar eso
    /// el 2026-08-02, la tabla de un medio pasó de 0 a 4.409.298 filas en el mismo rastreo y la
    /// pasada final se fue a horas.
    #[test]
    fn las_reglas_de_imagen_tienen_indice_por_donde_hacer_join() {
        let conn = rusqlite::Connection::open_in_memory().expect("abrir en memoria");
        for sql in [
            include_str!("../../crawlforge-core/migrations/001_initial.sql"),
            include_str!("../../crawlforge-core/migrations/007_indice_images_src.sql"),
        ] {
            conn.execute_batch(sql).expect("cargar el esquema");
        }

        let mut stmt = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT u.url_hash, COUNT(DISTINCT i.page_url_id)
                 FROM urls u JOIN images i ON i.src_url_id = u.id
                 WHERE u.status_code = 200 AND u.content_length > 204800
                 GROUP BY u.id",
            )
            .expect("preparar el plan");
        let plan: String = stmt
            .query_map([], |r| r.get::<_, String>(3))
            .expect("leer el plan")
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .join(" | ");

        assert!(
            !plan.contains("SCAN i"),
            "las imágenes no se pueden recorrer enteras por cada URL, y el plan dice: {plan}"
        );
        assert!(
            plan.contains("idx_images_src"),
            "el JOIN debe entrar por su índice, y el plan dice: {plan}"
        );
    }

    use super::*;
    use crate::ImageView;

    /// Una imagen con `alt` correcto y fuera de todo enlace: el caso sano.
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
    fn no_avisa_cuando_todas_las_imagenes_tienen_alt() {
        let imgs = [imagen_sana()];
        assert!(AssetImgNoAlt.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn no_avisa_en_una_pagina_sin_imagenes() {
        assert!(AssetImgNoAlt.evaluate(&ctx(&[])).is_empty());
    }

    #[test]
    fn avisa_cuando_falta_el_atributo_alt() {
        let imgs = [ImageView { src: "/img/sin-alt.png", ..Default::default() }];
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "ASSET-IMG-NO-ALT");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn un_alt_vacio_no_es_un_alt_ausente() {
        // La distinción es el motivo de que `ImageView::alt` sea un `Option`: `alt=""` es HTML
        // válido y declara una imagen decorativa. Confundirlos convertiría cada icono
        // decorativo del sitio en un hallazgo falso.
        let imgs = [ImageView { src: "/img/decorativa.svg", alt: Some(""), ..Default::default() }];
        assert!(
            AssetImgNoAlt.evaluate(&ctx(&imgs)).is_empty(),
            "un alt=\"\" deliberado no es una imagen sin alt"
        );
    }

    #[test]
    fn un_alt_de_solo_espacios_tampoco_cuenta_como_ausente() {
        // Es un `alt=" "`, que el HTML admite. Discutible como práctica, pero el atributo está:
        // avisar aquí sería avisar de otra cosa con el ID de esta regla.
        let imgs = [ImageView { src: "/img/x.png", alt: Some("   "), ..Default::default() }];
        assert!(AssetImgNoAlt.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn un_solo_hallazgo_por_pagina_con_el_recuento_completo() {
        let imgs = [
            ImageView { src: "/1.png", ..Default::default() },
            ImageView { src: "/2.png", ..Default::default() },
            imagen_sana(),
        ];
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        assert_eq!(issues.len(), 1, "una galería entera no debe dar una fila por imagen");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"images\":2"), "detalle: {detalle}");
        assert!(detalle.contains("/1.png") && detalle.contains("/2.png"), "detalle: {detalle}");
    }

    #[test]
    fn la_muestra_del_detalle_esta_acotada() {
        // Sin el corte, una galería de 200 imágenes escribiría 200 cadenas en el almacén.
        let srcs: Vec<String> = (0..SAMPLE_LIMIT + 5).map(|i| format!("/img-{i:02}.png")).collect();
        let imgs: Vec<ImageView<'_>> =
            srcs.iter().map(|s| ImageView { src: s.as_str(), ..Default::default() }).collect();
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains(&format!("\"images\":{}", SAMPLE_LIMIT + 5)));
        assert_eq!(detalle.matches("/img-").count(), SAMPLE_LIMIT, "detalle: {detalle}");
    }

    #[test]
    fn la_muestra_no_repite_la_misma_url() {
        // Regresión de un rastreo real: el placeholder del lazy-load repetido en veinte `<img>`
        // llenaba la muestra con diez copias de la misma cadena, que no localizan más que una.
        let imgs: Vec<ImageView<'_>> =
            (0..20).map(|_| ImageView { src: "/logo.png", ..Default::default() }).collect();
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"images\":20"), "el recuento sigue completo: {detalle}");
        assert_eq!(detalle.matches("/logo.png").count(), 1, "detalle: {detalle}");
    }

    #[test]
    fn una_uri_data_no_se_guarda_entera_en_la_muestra() {
        // 18.089 filas guardaban el mismo SVG en base64 como «muestra»: peso muerto que no
        // localiza nada, porque un `data:` no es una URL que se pueda abrir. El tipo basta.
        let data = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";
        let imgs = [ImageView { src: data, ..Default::default() }];
        let issues = AssetImgNoAlt.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(!detalle.contains("PHN2Zy"), "el base64 no se almacena: {detalle}");
        assert!(
            detalle.contains("data:image/svg+xml;base64,…"),
            "el tipo sí, para saber qué es: {detalle}"
        );
    }

    #[test]
    fn no_avisa_sobre_algo_que_no_es_html() {
        let imgs = [ImageView { src: "/x.png", ..Default::default() }];
        let mut c = ctx(&imgs);
        c.is_html = false;
        assert!(AssetImgNoAlt.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_tambien_en_una_pagina_no_indexable() {
        // El texto alternativo lo necesita quien visita la página, y a esa página se llega
        // aunque lleve `noindex`.
        let imgs = [ImageView { src: "/x.png", ..Default::default() }];
        let mut c = ctx(&imgs);
        c.is_indexable = false;
        assert_eq!(AssetImgNoAlt.evaluate(&c).len(), 1);
    }

    #[test]
    fn la_plantilla_de_error_no_se_audita() {
        // Regresión de un rastreo real: la plantilla del 404 del tema, con su logo de `alt`
        // vacío, producía un hallazgo por cada URL rota del sitio —26 en un rastreo, 12 en
        // otro—. El hallazgo accionable de un 404 es el 404, que ya tiene su regla HTTP.
        let imgs = [
            ImageView { src: "/sin-alt.png", ..Default::default() },
            ImageView { src: "/logo.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() },
        ];
        for status in [301, 404, 410, 500] {
            let mut c = ctx(&imgs);
            c.status = status;
            assert!(
                AssetImgNoAlt.evaluate(&c).is_empty(),
                "ASSET-IMG-NO-ALT no debería auditar el HTML de un {status}"
            );
            assert!(
                AssetImgEmptyAltLink.evaluate(&c).is_empty(),
                "ASSET-IMG-EMPTY-ALT-LINK no debería auditar el HTML de un {status}"
            );
        }
    }

    // --- ASSET-IMG-EMPTY-ALT-LINK ---

    #[test]
    fn avisa_cuando_el_enlace_solo_lleva_una_imagen_con_alt_vacio() {
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
    fn no_avisa_si_el_enlace_tiene_texto_propio() {
        // El nombre accesible lo pone el texto del enlace, así que el `alt=""` es correcto: la
        // imagen es decorativa y repetir el texto en el `alt` sería redundante.
        let imgs = [ImageView {
            src: "/icono.svg",
            alt: Some(""),
            anchor_text: Some("Ver el informe"),
            ..Default::default()
        }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn no_avisa_si_la_imagen_decorativa_no_esta_en_un_enlace() {
        // `anchor_text: None` es «esta imagen no cuelga de ningún <a>». Fuera de un enlace, un
        // `alt=""` es exactamente lo que hay que escribir.
        let imgs = [ImageView { src: "/adorno.svg", alt: Some(""), ..Default::default() }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn no_avisa_si_la_imagen_del_enlace_describe_el_destino() {
        let imgs = [ImageView {
            src: "/logo.svg",
            alt: Some("Portada de CrawlForge"),
            anchor_text: Some(""),
            ..Default::default()
        }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
    }

    #[test]
    fn un_alt_ausente_dentro_de_un_enlace_lo_cuenta_la_otra_regla() {
        // El enlace también se queda sin nombre, pero el defecto que hay que arreglar es el
        // `alt` que falta. Dos hallazgos sobre el mismo `<img>` serían ruido.
        let imgs = [ImageView { src: "/logo.svg", anchor_text: Some(""), ..Default::default() }];
        assert!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).is_empty());
        assert_eq!(AssetImgNoAlt.evaluate(&ctx(&imgs)).len(), 1);
    }

    #[test]
    fn un_enlace_con_solo_espacios_de_texto_sigue_sin_tener_nombre() {
        // `<a href="/"> <img alt=""> </a>`: el texto del enlace es un espacio, que no nombra nada.
        let imgs = [ImageView {
            src: "/logo.svg",
            alt: Some(""),
            anchor_text: Some("  \n "),
            ..Default::default()
        }];
        assert_eq!(AssetImgEmptyAltLink.evaluate(&ctx(&imgs)).len(), 1);
    }

    #[test]
    fn un_hallazgo_por_imagen_distinta_y_no_por_enlace() {
        // Dos imágenes distintas son dos causas: cada una con su fila y su clave. La misma
        // imagen repetida es una causa con recuento: el logo veinte veces no son veinte filas.
        let dos = [
            ImageView { src: "/a.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() },
            ImageView { src: "/b.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() },
        ];
        let issues = AssetImgEmptyAltLink.evaluate(&ctx(&dos));
        assert_eq!(issues.len(), 2, "dos imágenes distintas, dos hallazgos");
        assert_ne!(issues[0].group_key, issues[1].group_key);

        let logo =
            ImageView { src: "/logo.svg", alt: Some(""), anchor_text: Some(""), ..Default::default() };
        let repetida = [logo, logo, logo];
        let issues = AssetImgEmptyAltLink.evaluate(&ctx(&repetida));
        assert_eq!(issues.len(), 1, "la misma imagen tres veces es una causa");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"occurrences\":3"), "detalle: {detalle}");
        assert!(detalle.contains("/logo.svg"), "detalle: {detalle}");
    }

    #[test]
    fn el_detalle_del_enlace_sin_nombre_no_guarda_el_base64() {
        let data = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";
        let imgs = [ImageView { src: data, alt: Some(""), anchor_text: Some(""), ..Default::default() }];
        let issues = AssetImgEmptyAltLink.evaluate(&ctx(&imgs));
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(!detalle.contains("PHN2Zy"), "el base64 no se almacena: {detalle}");
        assert!(detalle.contains("data:image/svg+xml;base64,…"), "{detalle}");
    }

    #[test]
    fn el_mismo_logo_en_dos_paginas_comparte_grupo() {
        // El logo de la cabecera era el 90% de los `high` de un rastreo real: la clave es la
        // imagen, no la página, para que el informe lo cuente como un solo defecto de plantilla.
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
    fn otra_imagen_es_otro_grupo_y_las_repeticiones_no_lo_cambian() {
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

        assert_eq!(k_logo, k_repetido, "el logo dos veces sigue siendo la misma causa");
        assert_ne!(k_logo, k_banner, "otra imagen es otra causa");
    }

    #[test]
    fn el_enlace_sin_nombre_no_se_evalua_fuera_del_html() {
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

    // --- Reglas de conjunto ---
    //
    // El test de verdad de las tres es su fixture, que se rastrea de extremo a extremo en
    // `crawlforge-core/tests/fixtures_de_reglas.rs`: es lo único que demuestra que el motor
    // rellena las columnas que estas consultas leen. Lo que se comprueba aquí es la consulta
    // contra el mínimo de esquema que usa, que es lo que caza un nombre de columna mal escrito
    // o un umbral mal comparado sin esperar a un rastreo completo.

    /// Las columnas de `001_initial.sql` que tocan estas reglas, y solo esas.
    fn conn_minima() -> Connection {
        let conn = Connection::open_in_memory().expect("sqlite en memoria");
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
        .expect("esquema mínimo");
        conn
    }

    /// Inserta una URL. El `url_hash` se deriva del id para poder comprobar a qué fila se pega
    /// el hallazgo.
    fn url(conn: &Connection, id: i64, url: &str, status: Option<i64>, bytes: Option<i64>) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, status_code, content_length)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, url, id * 1000, status, bytes],
        )
        .expect("insertar url");
    }

    #[test]
    fn detecta_la_imagen_que_devuelve_un_error_y_cuenta_las_paginas() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/b", Some(200), Some(2_000));
        url(&conn, 3, "https://ejemplo.es/rota.png", Some(404), Some(0));
        url(&conn, 4, "https://ejemplo.es/bien.png", Some(200), Some(1_000));
        conn.execute_batch(
            "INSERT INTO images (page_url_id, src_url_id) VALUES (1, 3), (2, 3), (1, 4);",
        )
        .expect("insertar imágenes");

        let hallazgos = AssetImgBroken.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1, "un hallazgo por fichero que falta, no por página");
        assert_eq!(hallazgos[0].0, Some(3_000), "se registra en la URL de la imagen");
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"used_by_pages\":2"), "detalle: {detalle}");
    }

    #[test]
    fn detecta_la_imagen_que_pasa_del_umbral_de_peso() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/pesada.jpg", Some(200), Some(HEAVY_IMAGE_MAX_BYTES + 1));
        url(&conn, 3, "https://ejemplo.es/justa.jpg", Some(200), Some(HEAVY_IMAGE_MAX_BYTES));
        url(&conn, 4, "https://ejemplo.es/error.jpg", Some(404), Some(HEAVY_IMAGE_MAX_BYTES * 2));
        conn.execute_batch(
            "INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2), (1, 3), (1, 4);",
        )
        .expect("insertar imágenes");

        let hallazgos = AssetImgHeavy.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1, "el umbral es estricto y el cuerpo de un 404 no se mide");
        assert_eq!(hallazgos[0].0, Some(2_000));
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains(&format!("\"bytes\":{}", HEAVY_IMAGE_MAX_BYTES + 1)));
    }

    #[test]
    fn detecta_el_css_y_el_js_que_no_cargan_y_los_distingue() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/e.css", Some(404), Some(0));
        url(&conn, 3, "https://ejemplo.es/a.js", Some(500), Some(0));
        url(&conn, 4, "https://ejemplo.es/b", Some(404), Some(0));
        conn.execute_batch(
            "INSERT INTO links (from_url_id, to_url_id, element)
             VALUES (1, 2, 'link'), (1, 3, 'script'), (1, 4, 'a'), (1, 2, 'link');",
        )
        .expect("insertar enlaces");

        let hallazgos = AssetBroken.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 2, "un <a> roto no es un recurso: eso es HTTP-404-INTERNAL");
        let detalles: Vec<String> =
            hallazgos.iter().map(|(_, i)| i.detail_json.clone().unwrap_or_default()).collect();
        assert!(detalles.iter().any(|d| d.contains("\"kind\":\"css\"")), "{detalles:?}");
        assert!(detalles.iter().any(|d| d.contains("\"kind\":\"js\"")), "{detalles:?}");
        assert!(
            detalles.iter().all(|d| d.contains("\"used_by_pages\":1")),
            "la misma hoja citada dos veces en una página es una página: {detalles:?}"
        );
    }

    #[test]
    fn un_almacen_sin_defectos_no_produce_hallazgos() {
        let conn = conn_minima();
        url(&conn, 1, "https://ejemplo.es/a", Some(200), Some(2_000));
        url(&conn, 2, "https://ejemplo.es/bien.webp", Some(200), Some(30_000));
        url(&conn, 3, "https://ejemplo.es/e.css", Some(200), Some(4_000));
        conn.execute_batch(
            "INSERT INTO images (page_url_id, src_url_id) VALUES (1, 2);
             INSERT INTO links (from_url_id, to_url_id, element) VALUES (1, 3, 'link');",
        )
        .expect("poblar");

        assert!(AssetImgBroken.evaluate(&conn).expect("evaluar").is_empty());
        assert!(AssetImgHeavy.evaluate(&conn).expect("evaluar").is_empty());
        assert!(AssetBroken.evaluate(&conn).expect("evaluar").is_empty());
    }
}
