//! Extracción de datos de una página con `lol_html`. Ver `docs/03-MOTOR-CRAWL.md §5`.
//!
//! `lol_html` procesa en streaming mediante manejadores por selector, sin construir el DOM.
//! Es la razón de rendimiento del proyecto: 5-10x más rápido que `scraper` en páginas grandes.
//!
//! Todo se extrae en **una sola pasada**. El orden de aparición importa (primer `h1`, jerarquía
//! de encabezados, posición del enlace), así que el estado vive en un [`PageAccumulator`]
//! mutable a lo largo del recorrido.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// De qué elemento salió un enlace. Se corresponde con `links.element`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkElement {
    A,
    Link,
    Img,
    Script,
    Iframe,
    Form,
}

impl LinkElement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::Link => "link",
            Self::Img => "img",
            Self::Script => "script",
            Self::Iframe => "iframe",
            Self::Form => "form",
        }
    }
}

/// Zona semántica de la que cuelga un enlace. Se corresponde con `links.region`.
///
/// Distingue enlaces de plantilla de enlaces de contenido, que es la diferencia que importa
/// al analizar enlazado interno: 200 enlaces de menú repetidos en todas las páginas no dicen
/// lo mismo que un enlace dentro de un artículo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Region {
    Nav,
    Main,
    Footer,
    Aside,
    #[default]
    Unknown,
}

impl Region {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nav => "nav",
            Self::Main => "main",
            Self::Footer => "footer",
            Self::Aside => "aside",
            Self::Unknown => "unknown",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "nav" => Some(Self::Nav),
            "main" => Some(Self::Main),
            "footer" => Some(Self::Footer),
            "aside" => Some(Self::Aside),
            _ => None,
        }
    }
}

/// Un enlace tal como aparece en el HTML, todavía sin normalizar ni resolver.
#[derive(Debug, Clone)]
pub struct RawLink {
    pub href: String,
    pub anchor: Option<String>,
    pub rel: Option<String>,
    pub is_nofollow: bool,
    pub element: LinkElement,
    pub region: Region,
    pub position: u32,
}

/// Una imagen con sus atributos de accesibilidad y rendimiento.
#[derive(Debug, Clone)]
pub struct RawImage {
    pub src: String,
    pub alt: Option<String>,
    pub title: Option<String>,
    pub width_attr: Option<i64>,
    pub height_attr: Option<i64>,
    pub loading: Option<String>,
    pub in_srcset: bool,
    /// Índice en `links` del `<a>` que envuelve a esta imagen, si hay alguno.
    ///
    /// Lo necesita `ASSET-IMG-EMPTY-ALT-LINK`: una imagen sin `alt` dentro de un enlace sin
    /// texto deja el enlace sin nombre accesible, y no se puede saber en el momento de leer
    /// el `<img>` porque el texto del enlace llega después. Con el índice, la regla lo
    /// resuelve al final mirando el enlace.
    pub anchor_index: Option<usize>,
}

/// Un encabezado, con su nivel y su texto. En orden de aparición.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

/// Todo lo que una página aporta al almacén.
#[derive(Debug, Default)]
pub struct ParsedPage {
    pub title: Option<String>,
    /// Cuántas etiquetas `<title>` trae la página, excluidas las de dentro de un `<svg>`.
    /// Más de una es un error de plantilla y Google se queda con la primera.
    pub title_count: u32,
    pub meta_description: Option<String>,
    pub meta_robots: Option<String>,
    pub viewport: Option<String>,
    /// Contenido de `<meta http-equiv="refresh">`. Es una redirección que los buscadores
    /// tratan peor que un 301, y a veces esconde un secuestro de contenido.
    pub meta_refresh: Option<String>,
    pub canonical: Option<String>,
    /// Cuántos `<link rel="canonical">` trae la página. Con más de uno, Google los ignora
    /// todos, así que el efecto es el de no tener ninguno.
    pub canonical_count: u32,
    pub amp_url: Option<String>,
    pub lang: Option<String>,
    pub hreflang: Vec<(String, String)>,
    pub headings: Vec<Heading>,
    pub links: Vec<RawLink>,
    pub images: Vec<RawImage>,
    pub stylesheets: Vec<String>,
    pub scripts: Vec<String>,
    pub og: Vec<(String, String)>,
    pub twitter: Vec<(String, String)>,
    pub schema_types: Vec<String>,
    pub word_count: u32,
    /// Texto visible. Solo se acumula si se pidió (nivel Pro, para FTS5).
    pub body_text: Option<String>,
    /// Longitud del HTML en bytes, para `content_ratio`.
    pub html_bytes: u64,
    /// Longitud del texto visible en bytes, para `content_ratio`.
    pub text_bytes: u64,
}

impl ParsedPage {
    /// Primer `h1` de la página, que es el que cuenta a efectos de SEO.
    pub fn h1(&self) -> Option<&str> {
        self.headings.iter().find(|h| h.level == 1).map(|h| h.text.as_str())
    }

    pub fn h1_count(&self) -> u32 {
        self.headings.iter().filter(|h| h.level == 1).count() as u32
    }

    pub fn h2_count(&self) -> u32 {
        self.headings.iter().filter(|h| h.level == 2).count() as u32
    }

    /// Texto / HTML. Un valor muy bajo delata plantilla pesada con poco contenido.
    pub fn content_ratio(&self) -> f64 {
        if self.html_bytes == 0 {
            return 0.0;
        }
        self.text_bytes as f64 / self.html_bytes as f64
    }

    /// Enlaces internos salientes, dado el host semilla.
    pub fn heading_json(&self) -> String {
        serde_json::to_string(&self.headings).unwrap_or_else(|_| "[]".to_string())
    }
}

/// Estado mutable durante el recorrido en streaming.
#[derive(Default)]
struct PageAccumulator {
    page: ParsedPage,
    /// Pila de regiones semánticas abiertas. La región de un enlace es la última abierta.
    region_stack: Vec<Region>,
    /// Profundidad dentro de elementos cuyo texto no cuenta como contenido.
    non_content_depth: u32,
    /// Encabezado que se está leyendo, si hay alguno abierto.
    current_heading: Option<(u8, String)>,
    /// `true` mientras se recorre un `<title>`.
    in_title: bool,
    /// Profundidad de `<svg>` abiertos. Ver el manejador de `<title>`.
    svg_depth: u32,
    /// `true` mientras se recorre un `<script type="application/ld+json">`.
    in_ld_json: bool,
    ld_json_buffer: String,
    /// `@type` ya vistos en la página, para deduplicar entre bloques JSON-LD en O(1).
    schema_seen: HashSet<String>,
    /// Texto del enlace que se está leyendo: (índice en `links`, texto acumulado).
    current_anchor: Option<(usize, String)>,
    link_position: u32,
    collect_body_text: bool,
}

impl PageAccumulator {
    fn current_region(&self) -> Region {
        self.region_stack.last().copied().unwrap_or_default()
    }
}

/// Entidades HTML con nombre más frecuentes, incluidas las que importan en español.
///
/// No es la tabla completa del estándar —son más de dos mil— sino las que aparecen de verdad en
/// títulos y encabezados. Las numéricas se resuelven aparte y cubren el resto.
const NAMED_ENTITIES: &[(&str, char)] = &[
    ("amp", '&'), ("lt", '<'), ("gt", '>'), ("quot", '"'), ("apos", '\''),
    ("nbsp", '\u{00a0}'), ("ndash", '–'), ("mdash", '—'), ("hellip", '…'),
    ("laquo", '«'), ("raquo", '»'), ("lsquo", '\u{2018}'), ("rsquo", '\u{2019}'),
    ("ldquo", '\u{201c}'), ("rdquo", '\u{201d}'), ("bull", '•'), ("middot", '·'),
    ("aacute", 'á'), ("eacute", 'é'), ("iacute", 'í'), ("oacute", 'ó'), ("uacute", 'ú'),
    ("Aacute", 'Á'), ("Eacute", 'É'), ("Iacute", 'Í'), ("Oacute", 'Ó'), ("Uacute", 'Ú'),
    ("ntilde", 'ñ'), ("Ntilde", 'Ñ'), ("uuml", 'ü'), ("Uuml", 'Ü'),
    ("iquest", '¿'), ("iexcl", '¡'), ("deg", '°'), ("euro", '€'), ("copy", '©'),
    ("reg", '®'), ("trade", '™'), ("times", '×'), ("divide", '÷'), ("shy", '\u{00ad}'),
];

/// Resuelve las entidades HTML de un texto extraído.
///
/// Hace falta porque el texto que entrega `lol_html` viene tal cual está en el documento. Sin
/// esto, un título con `&amp;` se guarda literalmente como `&amp;`, y eso no es solo feo: cuenta
/// cinco caracteres en lugar de uno, así que falsea `title_len` y `title_px` —lo que decide si
/// Google trunca el título— y hace que dos títulos idénticos escritos con entidades distintas no
/// se detecten como duplicados.
///
/// Detectado comparando la extracción con Screaming Frog sobre la misma lista de URLs.
pub fn decode_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'&' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'&' {
                i += 1;
            }
            out.push_str(&input[start..i]);
            continue;
        }
        // Una entidad sin `;` a menos de 32 caracteres no es una entidad: es un `&` suelto.
        // La búsqueda del `;` se acota a esa misma ventana. Con `input[i..].find(';')` cada
        // `&` recorría todo el resto de la cadena antes de que el filtro descartara el
        // resultado, y una entrada sin `;` era O(n²): medido con `rustc -O`, 800.000 `&`
        // tardaban 13,85 s (×4 al doblar la entrada); acotado son 8,04 ms (×2). Un `<title>`
        // patológico de 5 MB pasaba de ~14 minutos —con un hilo del runtime bloqueado, sin
        // deadline ni Ctrl-C— a milisegundos.
        let ventana = &bytes[i + 1..bytes.len().min(i + 33)];
        let fin = ventana.iter().position(|&b| b == b';').map(|p| i + 1 + p);
        let Some(fin) = fin else {
            out.push('&');
            i += 1;
            continue;
        };

        let cuerpo = &input[i + 1..fin];
        let resuelto = if let Some(num) = cuerpo.strip_prefix('#') {
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => num.parse::<u32>().ok(),
            };
            code.and_then(char::from_u32)
        } else {
            NAMED_ENTITIES.iter().find(|(n, _)| *n == cuerpo).map(|(_, c)| *c)
        };

        match resuelto {
            Some(c) => {
                out.push(c);
                i = fin + 1;
            }
            // Lo que no se reconoce se deja intacto: es preferible mostrar `&loquesea;` que
            // comerse texto del sitio.
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

/// Coacciona una closure al tipo que espera [`lol_html::html_content::Element::on_end_tag`].
///
/// `on_end_tag` no acepta una closure suelta: pide un `Box<dyn FnOnce(..)>` ya coaccionado,
/// y un `Box::new(closure)` a secas se infiere como `Box` del tipo concreto. Este envoltorio
/// existe solo para dar esa coerción con nombre.
fn on_end(
    f: impl FnOnce(&mut lol_html::html_content::EndTag<'_>) -> lol_html::HandlerResult + 'static,
) -> lol_html::EndTagHandler<'static> {
    Box::new(f)
}

/// Elementos cuyo texto no es contenido visible y no debe contar para `word_count`.
/// `nav` y `footer` se excluyen por indicación explícita de `§5`.
const NON_CONTENT_TAGS: &[&str] = &["script", "style", "nav", "footer", "noscript", "template"];

/// Atributos donde el lazy-load esconde el `src` real, por orden de preferencia.
///
/// Los plugins de lazy-load de WordPress sustituyen el `src` por un placeholder `data:`
/// (un SVG o GIF del tamaño de la imagen) y mueven la URL real a un atributo `data-*`.
/// Sin leerlos, la tabla `images` de un rastreo de 20.000 URLs de un medio de comunicación
/// (LiteSpeed Cache) quedó **vacía** y `ASSET-IMG-HEAVY`/`ASSET-IMG-BROKEN` no dispararon
/// nunca — con imágenes destacadas de 419 KB sobre un umbral de 200 KB.
///
/// Quién usa cada atributo, verificado contra el ecosistema real:
/// - `data-src`: LiteSpeed Cache (**comprobado el 2026-08-01 contra un WordPress con LiteSpeed Cache**:
///   45 de 51 `<img>` de la portada llevan `data-lazyloaded="1"` + `data-src`), y además
///   lazysizes, a3 Lazy Load, vanilla-lazyload, Smush y EWWW. Es el más extendido y por
///   eso va primero.
/// - `data-lazy-src`: WP Rocket (rocket-lazyload) y Jetpack Lazy Images.
/// - `data-original`: el jQuery lazyload clásico de tuupola, todavía vivo en temas viejos.
const LAZY_SRC_ATTRS: &[&str] = &["data-src", "data-lazy-src", "data-original"];

/// Lo mismo para `srcset`: LiteSpeed y lazysizes usan `data-srcset`; WP Rocket y
/// Jetpack, `data-lazy-srcset`.
const LAZY_SRCSET_ATTRS: &[&str] = &["data-srcset", "data-lazy-srcset"];

/// ¿Es el valor una URI `data:`? Sirve para detectar el placeholder del lazy-load y
/// para no aceptar como «URL real» un `data-*` que también sea `data:`.
fn is_data_uri(value: &str) -> bool {
    let t = value.trim_start();
    t.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:"))
}

/// Primer candidato de un `srcset`, para usarlo como `src` de último recurso.
///
/// El corte por coma y espacio no es el algoritmo completo del estándar (una URL puede
/// llevar comas sin espacio), pero los `srcset` que genera WordPress — que es de donde
/// viene todo este problema — separan candidatos con `, ` y `sanitize_file_name()` no
/// deja comas en los nombres de fichero, así que en la práctica basta.
fn first_srcset_candidate(srcset: &str) -> Option<&str> {
    srcset.split(',').find_map(|candidate| {
        let url = candidate.split_whitespace().next()?;
        (!is_data_uri(url)).then_some(url)
    })
}

/// Analiza un documento HTML.
///
/// `collect_body_text` solo debe activarse en nivel Pro: el texto de cuerpo multiplica el
/// tamaño del fichero de rastreo, y solo hace falta para poblar FTS5.
pub fn parse_html(html: &[u8], collect_body_text: bool) -> ParsedPage {
    use lol_html::{element, text, HtmlRewriter, Settings};

    let acc = Rc::new(RefCell::new(PageAccumulator {
        collect_body_text,
        ..Default::default()
    }));

    // Los manejadores se ejecutan en el orden en que el documento los va encontrando,
    // que es justo lo que necesita el acumulador.
    let html_len = html.len() as u64;

    {
        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![
                    // --- Regiones semánticas: pila abierta/cerrada ---
                    element!("nav, main, footer, aside", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let tag = el.tag_name().to_ascii_lowercase();
                            if let Some(region) = Region::from_tag(&tag) {
                                acc.borrow_mut().region_stack.push(region);
                                let acc = Rc::clone(&acc);
                                let _ = el.on_end_tag(on_end(move |_| {
                                    acc.borrow_mut().region_stack.pop();
                                    Ok(())
                                }));
                            }
                            Ok(())
                        }
                    }),
                    // --- Elementos cuyo texto no es contenido ---
                    element!(NON_CONTENT_TAGS.join(", "), {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            acc.borrow_mut().non_content_depth += 1;
                            let acc = Rc::clone(&acc);
                            let _ = el.on_end_tag(on_end(move |_| {
                                let mut a = acc.borrow_mut();
                                a.non_content_depth = a.non_content_depth.saturating_sub(1);
                                Ok(())
                            }));
                            Ok(())
                        }
                    }),
                    // --- <svg> ---
                    //
                    // Solo se sigue para saber cuándo un `<title>` no es el de la página: un
                    // icono en línea lleva el suyo. Los `<svg>` pueden anidarse, así que es
                    // una profundidad y no un booleano.
                    element!("svg", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            acc.borrow_mut().svg_depth += 1;
                            let acc = Rc::clone(&acc);
                            let _ = el.on_end_tag(on_end(move |_| {
                                let mut a = acc.borrow_mut();
                                a.svg_depth = a.svg_depth.saturating_sub(1);
                                Ok(())
                            }));
                            Ok(())
                        }
                    }),
                    // --- <title> ---
                    element!("title", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            // El `<title>` de un `<svg>` es el nombre accesible de un icono, no
                            // el título de la página: no cuenta ni para el valor ni para el
                            // recuento de duplicados.
                            if acc.borrow().svg_depth > 0 {
                                return Ok(());
                            }
                            acc.borrow_mut().page.title_count += 1;
                            // El valor es el del primero, que es con el que se queda Google.
                            if acc.borrow().page.title.is_none() {
                                acc.borrow_mut().in_title = true;
                                let acc = Rc::clone(&acc);
                                let _ = el.on_end_tag(on_end(move |_| {
                                    acc.borrow_mut().in_title = false;
                                    Ok(())
                                }));
                            }
                            Ok(())
                        }
                    }),
                    // En los manejadores `text!` se usa `t.as_str()` directamente: copiarlo
                    // a una `String` intermedia era una asignación por cada nodo de texto
                    // del documento —hasta 2.000 por página— tirada al instante.
                    text!("title", {
                        let acc = Rc::clone(&acc);
                        move |t| {
                            let mut a = acc.borrow_mut();
                            if a.in_title {
                                a.page.title.get_or_insert_with(String::new).push_str(t.as_str());
                            }
                            Ok(())
                        }
                    }),
                    // --- <html lang> ---
                    element!("html[lang]", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            acc.borrow_mut().page.lang = el.get_attribute("lang");
                            Ok(())
                        }
                    }),
                    // --- <meta> ---
                    element!("meta", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let name = el.get_attribute("name").unwrap_or_default().to_lowercase();
                            let property =
                                el.get_attribute("property").unwrap_or_default().to_lowercase();
                            let content = el.get_attribute("content");

                            let mut a = acc.borrow_mut();

                            // `http-equiv` es independiente de `name`: una misma etiqueta no lleva
                            // los dos, así que se comprueba antes y se sale.
                            if el
                                .get_attribute("http-equiv")
                                .is_some_and(|v| v.trim().eq_ignore_ascii_case("refresh"))
                            {
                                a.page.meta_refresh = content;
                                return Ok(());
                            }

                            match name.as_str() {
                                "description" => a.page.meta_description = content,
                                "robots" => a.page.meta_robots = content,
                                "viewport" => a.page.viewport = content,
                                n if n.starts_with("twitter:") => {
                                    if let Some(c) = content {
                                        a.page.twitter.push((n.to_string(), c));
                                    }
                                }
                                _ => {
                                    if property.starts_with("og:") {
                                        if let Some(c) = content {
                                            a.page.og.push((property, c));
                                        }
                                    }
                                }
                            }
                            Ok(())
                        }
                    }),
                    // --- <link> ---
                    element!("link", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let rel = el.get_attribute("rel").unwrap_or_default().to_lowercase();
                            let href = match el.get_attribute("href") {
                                Some(h) if !h.trim().is_empty() => h,
                                _ => return Ok(()),
                            };
                            let mut a = acc.borrow_mut();
                            match rel.as_str() {
                                "canonical" => {
                                    a.page.canonical_count += 1;
                                    // Se conserva el primero, que es con el que se queda Google
                                    // cuando hay varios.
                                    if a.page.canonical.is_none() {
                                        a.page.canonical = Some(href);
                                    }
                                }
                                "amphtml" => a.page.amp_url = Some(href),
                                "alternate" => {
                                    if let Some(lang) = el.get_attribute("hreflang") {
                                        a.page.hreflang.push((lang, href));
                                    }
                                }
                                "stylesheet" => {
                                    a.page.stylesheets.push(href.clone());
                                    let region = a.current_region();
                                    let position = a.link_position;
                                    a.link_position += 1;
                                    a.page.links.push(RawLink {
                                        href,
                                        anchor: None,
                                        rel: Some(rel),
                                        is_nofollow: false,
                                        element: LinkElement::Link,
                                        region,
                                        position,
                                    });
                                }
                                _ => {}
                            }
                            Ok(())
                        }
                    }),
                    // --- Encabezados h1..h6 ---
                    element!("h1, h2, h3, h4, h5, h6", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let tag = el.tag_name();
                            let level =
                                tag.as_bytes().get(1).map(|b| b - b'0').unwrap_or(1).clamp(1, 6);
                            acc.borrow_mut().current_heading = Some((level, String::new()));
                            let acc = Rc::clone(&acc);
                            let _ = el.on_end_tag(on_end(move |_| {
                                let mut a = acc.borrow_mut();
                                if let Some((level, text)) = a.current_heading.take() {
                                    let limpio = decode_entities(&text);
                                    a.page.headings.push(Heading {
                                        level,
                                        text: limpio.split_whitespace().collect::<Vec<_>>().join(" "),
                                    });
                                }
                                Ok(())
                            }));
                            Ok(())
                        }
                    }),
                    text!("h1, h2, h3, h4, h5, h6", {
                        let acc = Rc::clone(&acc);
                        move |t| {
                            if let Some((_, text)) = acc.borrow_mut().current_heading.as_mut() {
                                text.push_str(t.as_str());
                                // Un elemento dentro del encabezado —`<br>`, `<span>`, `<em>`—
                                // parte el contenido en varios nodos de texto, y pegarlos sin
                                // nada junta las palabras de los extremos. `<h1>Agencia
                                // Especializada en WordPress<br />con +25 años</h1>` daba
                                // «WordPresscon». Encontrado comparando la extracción contra
                                // Screaming Frog sobre 300 URLs reales: era la **única**
                                // diferencia de 2.100 comparaciones.
                                //
                                // El espacio sobrante no molesta: al cerrar el encabezado se
                                // normaliza con `split_whitespace().join(" ")`.
                                if t.last_in_text_node() {
                                    text.push(' ');
                                }
                            }
                            Ok(())
                        }
                    }),
                    // --- Enlaces <a> ---
                    element!("a[href]", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let href = match el.get_attribute("href") {
                                Some(h) if !h.trim().is_empty() => h,
                                _ => return Ok(()),
                            };
                            let rel = el.get_attribute("rel");
                            let is_nofollow = rel
                                .as_deref()
                                .is_some_and(|r| r.to_lowercase().split_whitespace().any(|t| t == "nofollow"));

                            let mut a = acc.borrow_mut();
                            let region = a.current_region();
                            let position = a.link_position;
                            a.link_position += 1;
                            a.page.links.push(RawLink {
                                href,
                                anchor: None,
                                rel,
                                is_nofollow,
                                element: LinkElement::A,
                                region,
                                position,
                            });
                            let index = a.page.links.len() - 1;
                            a.current_anchor = Some((index, String::new()));
                            drop(a);

                            let acc = Rc::clone(&acc);
                            let _ = el.on_end_tag(on_end(move |_| {
                                let mut a = acc.borrow_mut();
                                if let Some((index, text)) = a.current_anchor.take() {
                                    let limpio = decode_entities(&text);
                                    let anchor =
                                        limpio.split_whitespace().collect::<Vec<_>>().join(" ");
                                    if let Some(link) = a.page.links.get_mut(index) {
                                        link.anchor =
                                            if anchor.is_empty() { None } else { Some(anchor) };
                                    }
                                }
                                Ok(())
                            }));
                            Ok(())
                        }
                    }),
                    text!("a", {
                        let acc = Rc::clone(&acc);
                        move |t| {
                            if let Some((_, text)) = acc.borrow_mut().current_anchor.as_mut() {
                                text.push_str(t.as_str());
                                // Igual que en los encabezados: `<a>Ver <b>más</b> aquí</a>` son
                                // tres nodos de texto y sin separador se pegan.
                                if t.last_in_text_node() {
                                    text.push(' ');
                                }
                            }
                            Ok(())
                        }
                    }),
                    // --- Imágenes ---
                    element!("img", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let mut src = el.get_attribute("src").unwrap_or_default();
                            let mut srcset = el.get_attribute("srcset");
                            // Lazy-load: si el `src` es un placeholder `data:` — o falta,
                            // como en el jQuery lazyload clásico — la URL real viaja en un
                            // atributo `data-*` (ver `LAZY_SRC_ATTRS`). El `alt` sí se
                            // conserva en el tag lazy, así que solo hay que rescatar el src.
                            //
                            // algunos sitios traen un `<noscript>` con los
                            // `<img>` originales, y NO se lee a propósito: (1) es redundante
                            // — todo plugin que emite el fallback deja además la URL en un
                            // `data-*` del propio tag, y en las dos páginas reales medidas
                            // el 2026-08-01 (portada y un artículo, 51 y 26 imágenes lazy)
                            // había 0 `<noscript>` y `data-src` cubría el 100% —, y
                            // (2) leerlo duplicaría cada imagen en los sitios que sí lo
                            // emiten (el tag lazy + su copia noscript), obligando a
                            // deduplicar en el acumulador.
                            if is_data_uri(&src) || src.trim().is_empty() {
                                let lazy_srcset = LAZY_SRCSET_ATTRS
                                    .iter()
                                    .find_map(|a| el.get_attribute(a))
                                    .filter(|v| !v.trim().is_empty());
                                let lazy_src = LAZY_SRC_ATTRS
                                    .iter()
                                    .filter_map(|a| el.get_attribute(a))
                                    // Un `data-src` vacío o que sea otro `data:` no es una
                                    // URL real: se pasa al siguiente candidato.
                                    .find(|v| !v.trim().is_empty() && !is_data_uri(v));
                                if let Some(real) = lazy_src {
                                    src = real;
                                } else if let Some(candidate) =
                                    lazy_srcset.as_deref().and_then(first_srcset_candidate)
                                {
                                    // Último recurso: el primer candidato del srcset lazy.
                                    src = candidate.to_string();
                                }
                                // Sin ningún atributo lazy, el `data:` se queda tal cual:
                                // es una imagen incrustada legítima (un icono SVG en línea)
                                // y la normalización la descartará aguas abajo, como hasta
                                // ahora.
                                if srcset.is_none() {
                                    srcset = lazy_srcset;
                                }
                            }
                            if src.trim().is_empty() && srcset.is_none() {
                                return Ok(());
                            }
                            let parse_dim = |v: Option<String>| -> Option<i64> {
                                v.and_then(|s| s.trim().parse::<i64>().ok())
                            };
                            let mut a = acc.borrow_mut();
                            let anchor_index = a.current_anchor.as_ref().map(|(i, _)| *i);
                            a.page.images.push(RawImage {
                                src: src.clone(),
                                // Un `alt` ausente y un `alt=""` son cosas distintas: el vacío
                                // es una decisión deliberada de imagen decorativa.
                                alt: el.get_attribute("alt"),
                                title: el.get_attribute("title"),
                                width_attr: parse_dim(el.get_attribute("width")),
                                height_attr: parse_dim(el.get_attribute("height")),
                                loading: el.get_attribute("loading"),
                                in_srcset: srcset.is_some(),
                                anchor_index,
                            });
                            if !src.trim().is_empty() {
                                let region = a.current_region();
                                let position = a.link_position;
                                a.link_position += 1;
                                a.page.links.push(RawLink {
                                    href: src,
                                    anchor: None,
                                    rel: None,
                                    is_nofollow: false,
                                    element: LinkElement::Img,
                                    region,
                                    position,
                                });
                            }
                            Ok(())
                        }
                    }),
                    // --- Scripts e iframes ---
                    element!("script[src]", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let src = match el.get_attribute("src") {
                                Some(s) if !s.trim().is_empty() => s,
                                _ => return Ok(()),
                            };
                            let mut a = acc.borrow_mut();
                            a.page.scripts.push(src.clone());
                            let region = a.current_region();
                            let position = a.link_position;
                            a.link_position += 1;
                            a.page.links.push(RawLink {
                                href: src,
                                anchor: None,
                                rel: None,
                                is_nofollow: false,
                                element: LinkElement::Script,
                                region,
                                position,
                            });
                            Ok(())
                        }
                    }),
                    element!("iframe[src]", {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            let src = match el.get_attribute("src") {
                                Some(s) if !s.trim().is_empty() => s,
                                _ => return Ok(()),
                            };
                            let mut a = acc.borrow_mut();
                            let region = a.current_region();
                            let position = a.link_position;
                            a.link_position += 1;
                            a.page.links.push(RawLink {
                                href: src,
                                anchor: None,
                                rel: None,
                                is_nofollow: false,
                                element: LinkElement::Iframe,
                                region,
                                position,
                            });
                            Ok(())
                        }
                    }),
                    // --- JSON-LD: solo interesan los @type ---
                    element!(r#"script[type="application/ld+json"]"#, {
                        let acc = Rc::clone(&acc);
                        move |el| {
                            acc.borrow_mut().in_ld_json = true;
                            let acc = Rc::clone(&acc);
                            let _ = el.on_end_tag(on_end(move |_| {
                                let mut a = acc.borrow_mut();
                                a.in_ld_json = false;
                                let buffer = std::mem::take(&mut a.ld_json_buffer);
                                let a = &mut *a;
                                extract_schema_types(
                                    &buffer,
                                    &mut a.schema_seen,
                                    &mut a.page.schema_types,
                                );
                                Ok(())
                            }));
                            Ok(())
                        }
                    }),
                    text!(r#"script[type="application/ld+json"]"#, {
                        let acc = Rc::clone(&acc);
                        move |t| {
                            let mut a = acc.borrow_mut();
                            if a.in_ld_json {
                                a.ld_json_buffer.push_str(t.as_str());
                            }
                            Ok(())
                        }
                    }),
                    // --- Texto visible: recuento de palabras y, si procede, cuerpo para FTS ---
                    text!("body", {
                        let acc = Rc::clone(&acc);
                        move |t| {
                            let chunk = t.as_str();
                            let mut a = acc.borrow_mut();
                            if a.non_content_depth > 0 {
                                return Ok(());
                            }
                            let words = chunk.split_whitespace().count() as u32;
                            if words == 0 {
                                return Ok(());
                            }
                            a.page.word_count += words;
                            a.page.text_bytes += chunk.trim().len() as u64;
                            if a.collect_body_text {
                                let body = a.page.body_text.get_or_insert_with(String::new);
                                if !body.is_empty() {
                                    body.push(' ');
                                }
                                body.push_str(chunk.trim());
                            }
                            Ok(())
                        }
                    }),
                ],
                ..Settings::new()
            },
            // Aquí solo se extrae, no se reescribe: el documento serializado que emite el
            // rewriter no lo lee nadie. Antes el sink lo copiaba a un `Vec` que se tiraba
            // —hasta 10 MB por documento en vuelo, multiplicado por la concurrencia—.
            |_: &[u8]| {},
        );

        // Un HTML mal formado no debe abortar el rastreo: se queda con lo extraído hasta el
        // punto del error. Una página rota sigue siendo un hallazgo que hay que reportar.
        if let Err(e) = rewriter.write(html) {
            tracing::debug!(error = %e, "HTML mal formado; se usa lo extraído hasta aquí");
        } else if let Err(e) = rewriter.end() {
            tracing::debug!(error = %e, "HTML truncado; se usa lo extraído hasta aquí");
        }
    }

    let mut page = Rc::try_unwrap(acc)
        .map(RefCell::into_inner)
        .unwrap_or_default()
        .page;

    page.html_bytes = html_len;
    page.title = page
        .title
        .map(|t| decode_entities(&t).split_whitespace().collect::<Vec<_>>().join(" "));
    page.meta_description = page.meta_description.map(|d| decode_entities(&d));
    page
}

/// Tope de `@type` distintos por página.
///
/// Los generadores reales (Yoast, Rank Math, un e-commerce con ofertas y reseñas) emiten
/// 10-30 tipos distintos como mucho, así que 64 dobla el peor caso legítimo observado. Sin
/// tope, un JSON-LD hostil de 10 MB con ~600.000 `@type` distintos acababa entero en
/// `pages.schema_types` tras ~1,8·10¹¹ comparaciones del `contains` lineal.
const MAX_SCHEMA_TYPES: usize = 64;

/// Saca los `@type` de un bloque JSON-LD, recorriendo objetos, listas y `@graph`.
///
/// `seen` y `out` viven en el acumulador para que la deduplicación —en O(1) por tipo, no el
/// `Vec::contains` lineal que hacía cuadrático el caso hostil— y el tope de
/// [`MAX_SCHEMA_TYPES`] apliquen al conjunto de bloques de la misma página.
fn extract_schema_types(json: &str, seen: &mut HashSet<String>, out: &mut Vec<String>) {
    fn push(s: &str, seen: &mut HashSet<String>, out: &mut Vec<String>) {
        if out.len() < MAX_SCHEMA_TYPES && !seen.contains(s) {
            seen.insert(s.to_string());
            out.push(s.to_string());
        }
    }

    fn walk(value: &serde_json::Value, seen: &mut HashSet<String>, out: &mut Vec<String>) {
        if out.len() >= MAX_SCHEMA_TYPES {
            return;
        }
        match value {
            serde_json::Value::Object(map) => {
                match map.get("@type") {
                    Some(serde_json::Value::String(s)) => push(s, seen, out),
                    Some(serde_json::Value::Array(items)) => {
                        for i in items {
                            if let Some(s) = i.as_str() {
                                push(s, seen, out);
                            }
                        }
                    }
                    _ => {}
                }
                for (_, v) in map {
                    walk(v, seen, out);
                }
            }
            serde_json::Value::Array(items) => {
                for i in items {
                    walk(i, seen, out);
                }
            }
            _ => {}
        }
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json.trim()) {
        walk(&value, seen, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(html: &str) -> ParsedPage {
        parse_html(html.as_bytes(), false)
    }

    // --- Metadatos ---

    #[test]
    fn extrae_los_metadatos_basicos() {
        let p = parse(
            r#"<html lang="es"><head>
                 <title>  Mi   título  </title>
                 <meta name="description" content="Una descripción">
                 <meta name="robots" content="noindex, follow">
                 <meta name="viewport" content="width=device-width">
                 <link rel="canonical" href="https://ejemplo.es/a">
                 <link rel="amphtml" href="https://ejemplo.es/a/amp">
               </head><body></body></html>"#,
        );
        assert_eq!(p.title.as_deref(), Some("Mi título"), "el título se normaliza en espacios");
        assert_eq!(p.meta_description.as_deref(), Some("Una descripción"));
        assert_eq!(p.meta_robots.as_deref(), Some("noindex, follow"));
        assert_eq!(p.viewport.as_deref(), Some("width=device-width"));
        assert_eq!(p.canonical.as_deref(), Some("https://ejemplo.es/a"));
        assert_eq!(p.amp_url.as_deref(), Some("https://ejemplo.es/a/amp"));
        assert_eq!(p.lang.as_deref(), Some("es"));
    }

    #[test]
    fn una_pagina_sin_metadatos_no_los_inventa() {
        let p = parse("<html><body><p>hola</p></body></html>");
        assert!(p.title.is_none());
        assert!(p.meta_description.is_none());
        assert!(p.canonical.is_none());
        assert!(p.h1().is_none());
    }

    #[test]
    fn se_queda_con_el_primer_title() {
        // Algunos temas inyectan un <title> dentro de un <svg>: no es el de la página.
        let p = parse("<head><title>Real</title></head><body><svg><title>Icono</title></svg></body>");
        assert_eq!(p.title.as_deref(), Some("Real"));
        assert_eq!(p.title_count, 1, "el <title> de un <svg> no cuenta");
    }

    #[test]
    fn cuenta_los_titles_de_la_pagina_y_no_los_de_los_iconos() {
        // META-TITLE-MULTIPLE: dos <title> reales son un error de plantilla.
        let p = parse("<head><title>Uno</title><title>Dos</title></head>");
        assert_eq!(p.title_count, 2);
        assert_eq!(p.title.as_deref(), Some("Uno"), "vale el primero");

        // Un icono en línea antes del título real no debe robarle el valor.
        let p = parse("<body><svg><title>Icono</title></svg><title>Real</title>");
        assert_eq!(p.title_count, 1);
        assert_eq!(p.title.as_deref(), Some("Real"));

        // svg anidados: la profundidad tiene que volver a cero.
        let p = parse("<svg><svg><title>a</title></svg></svg><title>Real</title>");
        assert_eq!(p.title_count, 1);
        assert_eq!(p.title.as_deref(), Some("Real"));
    }

    #[test]
    fn cuenta_los_canonical_y_conserva_el_primero() {
        // CANON-MULTIPLE: con dos canonical Google ignora los dos.
        let p = parse(
            r#"<head>
                 <link rel="canonical" href="https://ejemplo.es/a">
                 <link rel="canonical" href="https://ejemplo.es/b">
               </head>"#,
        );
        assert_eq!(p.canonical_count, 2);
        assert_eq!(p.canonical.as_deref(), Some("https://ejemplo.es/a"));

        let p = parse("<head></head>");
        assert_eq!(p.canonical_count, 0);
    }

    #[test]
    fn lee_meta_refresh_sin_confundirlo_con_otros_http_equiv() {
        let p = parse(r#"<head><meta http-equiv="refresh" content="0;url=/otra"></head>"#);
        assert_eq!(p.meta_refresh.as_deref(), Some("0;url=/otra"));

        // El valor va en mayúsculas variables en la vida real.
        let p = parse(r#"<head><meta http-equiv="REFRESH" content="5"></head>"#);
        assert_eq!(p.meta_refresh.as_deref(), Some("5"));

        // Otros http-equiv no son una redirección.
        let p = parse(r#"<head><meta http-equiv="content-type" content="text/html"></head>"#);
        assert!(p.meta_refresh.is_none());

        // Y no debe tragarse una description que viaje en la misma página.
        let p = parse(
            r#"<head><meta http-equiv="refresh" content="0"><meta name="description" content="D"></head>"#,
        );
        assert_eq!(p.meta_refresh.as_deref(), Some("0"));
        assert_eq!(p.meta_description.as_deref(), Some("D"));
    }

    #[test]
    fn una_imagen_recuerda_el_enlace_que_la_envuelve() {
        // ASSET-IMG-EMPTY-ALT-LINK necesita saber si el enlace contenedor tiene texto, y eso
        // solo se sabe al cerrarlo, después de haber leído la imagen.
        let p = parse(r#"<a href="/x"><img src="/i.jpg" alt=""></a><img src="/suelta.jpg">"#);
        assert_eq!(p.images.len(), 2);

        let idx = p.images[0].anchor_index.expect("la primera imagen va dentro de un enlace");
        assert_eq!(p.links[idx].href, "/x");
        assert_eq!(
            p.links[idx].anchor.as_deref().unwrap_or("").trim(),
            "",
            "el enlace no tiene más texto que la imagen"
        );

        assert!(p.images[1].anchor_index.is_none(), "la segunda imagen está fuera de un enlace");
    }

    #[test]
    fn lee_hreflang() {
        let p = parse(
            r#"<head>
                 <link rel="alternate" hreflang="en" href="https://ejemplo.es/en/a">
                 <link rel="alternate" hreflang="fr" href="https://ejemplo.es/fr/a">
               </head>"#,
        );
        assert_eq!(p.hreflang.len(), 2);
        assert_eq!(p.hreflang[0], ("en".into(), "https://ejemplo.es/en/a".into()));
    }

    #[test]
    fn lee_open_graph_y_twitter() {
        let p = parse(
            r#"<head>
                 <meta property="og:title" content="Título OG">
                 <meta property="og:type" content="article">
                 <meta name="twitter:card" content="summary_large_image">
               </head>"#,
        );
        assert_eq!(p.og.len(), 2);
        assert!(p.og.contains(&("og:title".into(), "Título OG".into())));
        assert_eq!(p.twitter, vec![("twitter:card".to_string(), "summary_large_image".to_string())]);
    }

    // --- Encabezados ---

    #[test]
    fn guarda_los_encabezados_en_orden_con_su_nivel() {
        let p = parse(
            "<body><h1>Uno</h1><h2>Dos</h2><h3>Tres</h3><h2>Otro dos</h2></body>",
        );
        assert_eq!(p.h1(), Some("Uno"));
        assert_eq!(p.h1_count(), 1);
        assert_eq!(p.h2_count(), 2);
        assert_eq!(p.headings.len(), 4);
        assert_eq!(p.headings[2].level, 3);
    }

    #[test]
    fn detecta_h1_multiple() {
        let p = parse("<body><h1>Uno</h1><h1>Dos</h1></body>");
        assert_eq!(p.h1_count(), 2);
        assert_eq!(p.h1(), Some("Uno"), "el primero es el que cuenta");
    }

    #[test]
    fn el_texto_del_encabezado_ignora_el_marcado_interno() {
        let p = parse("<body><h1>Guía de <em>cocina</em> fácil</h1></body>");
        assert_eq!(p.h1(), Some("Guía de cocina fácil"));
    }

    #[test]
    fn la_jerarquia_serializa_a_json() {
        let p = parse("<body><h1>A</h1><h2>B</h2></body>");
        let json = p.heading_json();
        assert!(json.contains("\"level\":1"));
        assert!(json.contains("\"text\":\"A\""));
    }

    // --- Enlaces ---

    #[test]
    fn extrae_enlaces_con_ancla_rel_y_posicion() {
        let p = parse(
            r#"<body>
                 <a href="/uno">Primero</a>
                 <a href="/dos" rel="nofollow">Segundo</a>
               </body>"#,
        );
        let anchors: Vec<_> = p.links.iter().filter(|l| l.element == LinkElement::A).collect();
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].href, "/uno");
        assert_eq!(anchors[0].anchor.as_deref(), Some("Primero"));
        assert!(!anchors[0].is_nofollow);
        assert!(anchors[1].is_nofollow);
        assert!(anchors[0].position < anchors[1].position, "la posición conserva el orden");
    }

    #[test]
    fn nofollow_se_detecta_dentro_de_un_rel_con_varios_valores() {
        let p = parse(r#"<body><a href="/x" rel="noopener NOFOLLOW noreferrer">x</a></body>"#);
        assert!(p.links[0].is_nofollow, "rel debe tratarse como lista y sin distinguir caja");
    }

    #[test]
    fn un_href_vacio_no_genera_enlace() {
        let p = parse(r#"<body><a href="">nada</a><a href="   ">nada</a><a>sin href</a></body>"#);
        assert!(p.links.is_empty());
    }

    #[test]
    fn el_ancla_ignora_el_marcado_interno() {
        let p = parse(r#"<body><a href="/x">Leer <strong>más</strong> aquí</a></body>"#);
        assert_eq!(p.links[0].anchor.as_deref(), Some("Leer más aquí"));
    }

    #[test]
    fn un_enlace_sin_texto_deja_el_ancla_vacia() {
        // Es un hallazgo real de accesibilidad y de SEO, así que debe distinguirse.
        let p = parse(r#"<body><a href="/x"><img src="/i.png"></a></body>"#);
        let anchor_link = p.links.iter().find(|l| l.element == LinkElement::A).expect("enlace");
        assert!(anchor_link.anchor.is_none());
    }

    // --- Regiones ---

    #[test]
    fn deduce_la_region_del_ancestro_semantico() {
        let p = parse(
            r#"<body>
                 <nav><a href="/menu">Menú</a></nav>
                 <main><a href="/contenido">Contenido</a></main>
                 <footer><a href="/legal">Legal</a></footer>
                 <aside><a href="/lateral">Lateral</a></aside>
                 <a href="/suelto">Suelto</a>
               </body>"#,
        );
        let region_of = |href: &str| {
            p.links.iter().find(|l| l.href == href).map(|l| l.region).expect("enlace presente")
        };
        assert_eq!(region_of("/menu"), Region::Nav);
        assert_eq!(region_of("/contenido"), Region::Main);
        assert_eq!(region_of("/legal"), Region::Footer);
        assert_eq!(region_of("/lateral"), Region::Aside);
        assert_eq!(region_of("/suelto"), Region::Unknown);
    }

    #[test]
    fn la_region_se_cierra_al_salir_del_elemento() {
        let p = parse(r#"<body><nav><a href="/a">a</a></nav><a href="/b">b</a></body>"#);
        assert_eq!(p.links.iter().find(|l| l.href == "/a").expect("a").region, Region::Nav);
        assert_eq!(p.links.iter().find(|l| l.href == "/b").expect("b").region, Region::Unknown);
    }

    #[test]
    fn con_regiones_anidadas_gana_la_mas_interna() {
        let p = parse(r#"<body><main><aside><a href="/x">x</a></aside></main></body>"#);
        assert_eq!(p.links[0].region, Region::Aside);
    }

    // --- Imágenes ---

    /// Un elemento dentro de un encabezado no puede juntar las palabras de los extremos.
    ///
    /// Encontrado el 2026-08-03 comparando la extracción contra Screaming Frog sobre las mismas
    /// 300 URLs de un sitio real: de 2.100 comparaciones —siete campos por URL— **esta fue la
    /// única diferencia**. El HTML era `<h1>… en WordPress<br />con +25 años…</h1>` y salía
    /// «WordPresscon».
    #[test]
    fn un_elemento_dentro_de_un_encabezado_no_junta_las_palabras() {
        let doc = parse_html(
            b"<html><body><h1>Agencia en WordPress<br />con 25 anos</h1>\
              <h2>Ver <strong>todo</strong> aqui</h2></body></html>",
            false,
        );
        let h1 = doc.headings.iter().find(|h| h.level == 1).map(|h| h.text.as_str());
        assert_eq!(h1, Some("Agencia en WordPress con 25 anos"));

        let h2 = doc.headings.iter().find(|h| h.level == 2).map(|h| h.text.as_str());
        assert_eq!(h2, Some("Ver todo aqui"), "y el espacio que ya había no se duplica");
    }

    #[test]
    fn un_elemento_dentro_de_un_ancla_tampoco() {
        // Sin espacio a los lados del `<em>`: con espacio, el texto sale bien aunque el
        // arreglo no esté, y el test no probaría nada. La primera versión de este test
        // tenía `Leer <em>mas</em>` y pasaba igual con el fallo puesto.
        let doc = parse_html(
            b"<html><body><a href=\"/x\">Leer<em>mas</em>aqui</a></body></html>",
            false,
        );
        let anchor = doc.links.first().and_then(|l| l.anchor.as_deref());
        assert_eq!(anchor, Some("Leer mas aqui"));
    }

    #[test]
    fn extrae_imagenes_con_sus_atributos() {
        let p = parse(
            r#"<body><img src="/a.webp" alt="Una foto" width="800" height="600" loading="lazy"></body>"#,
        );
        assert_eq!(p.images.len(), 1);
        let img = &p.images[0];
        assert_eq!(img.src, "/a.webp");
        assert_eq!(img.alt.as_deref(), Some("Una foto"));
        assert_eq!(img.width_attr, Some(800));
        assert_eq!(img.height_attr, Some(600));
        assert_eq!(img.loading.as_deref(), Some("lazy"));
        assert!(!img.in_srcset);
    }

    #[test]
    fn distingue_alt_ausente_de_alt_vacio() {
        // `alt=""` es una decisión deliberada (imagen decorativa); no tenerlo es un fallo.
        let p = parse(r#"<body><img src="/a.png"><img src="/b.png" alt=""></body>"#);
        assert!(p.images[0].alt.is_none(), "sin atributo alt");
        assert_eq!(p.images[1].alt.as_deref(), Some(""), "alt vacío explícito");
    }

    #[test]
    fn una_imagen_genera_ademas_un_enlace_para_comprobar_su_estado() {
        let p = parse(r#"<body><img src="/a.png" alt="x"></body>"#);
        assert!(p.links.iter().any(|l| l.element == LinkElement::Img && l.href == "/a.png"));
    }

    #[test]
    fn detecta_srcset() {
        let p = parse(r#"<body><img src="/a.png" srcset="/a-2x.png 2x" alt="x"></body>"#);
        assert!(p.images[0].in_srcset);
    }

    // --- Lazy-load (placeholders `data:` de LiteSpeed, WP Rocket, etc.) ---

    #[test]
    fn litespeed_mueve_el_src_a_data_src_y_se_recupera_la_url_real() {
        // Réplica de HTML real de un WordPress con LiteSpeed Cache (2026-08-01):
        // placeholder SVG en `src`, URL real en `data-src`, y `alt` conservado en el tag.
        let p = parse(
            r#"<body><img data-lazyloaded="1"
                 src="data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDo="
                 data-src="https://ejemplo.es/wp-content/uploads/foto.jpg"
                 alt="Una foto" width="876" height="1536"></body>"#,
        );
        assert_eq!(p.images.len(), 1);
        assert_eq!(p.images[0].src, "https://ejemplo.es/wp-content/uploads/foto.jpg");
        assert_eq!(p.images[0].alt.as_deref(), Some("Una foto"), "el alt viaja en el tag lazy");
        assert!(
            p.links.iter().any(|l| l.element == LinkElement::Img
                && l.href == "https://ejemplo.es/wp-content/uploads/foto.jpg"),
            "el enlace de comprobación de estado debe apuntar a la URL real, no al placeholder"
        );
    }

    #[test]
    fn wp_rocket_usa_data_lazy_src() {
        let p = parse(
            r#"<body><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw="
                 data-lazy-src="/uploads/rocket.png" alt="x"></body>"#,
        );
        assert_eq!(p.images[0].src, "/uploads/rocket.png");
    }

    #[test]
    fn jquery_lazyload_clasico_usa_data_original() {
        let p = parse(
            r#"<body><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw="
                 data-original="/uploads/clasica.jpg" alt="x"></body>"#,
        );
        assert_eq!(p.images[0].src, "/uploads/clasica.jpg");
    }

    #[test]
    fn data_src_gana_a_los_demas_atributos_lazy() {
        // Si un tema mezcla plugins puede haber varios; `data-src` es el más extendido
        // y el que rellenan LiteSpeed y a3 Lazy Load, así que manda.
        let p = parse(
            r#"<body><img src="data:image/gif;base64,R0lGODlhAQ="
                 data-original="/vieja.jpg" data-lazy-src="/rocket.jpg"
                 data-src="/buena.jpg" alt="x"></body>"#,
        );
        assert_eq!(p.images[0].src, "/buena.jpg");
    }

    #[test]
    fn un_atributo_lazy_vacio_no_cuenta_y_se_pasa_al_siguiente() {
        let p = parse(
            r#"<body><img src="data:image/gif;base64,R0lGODlhAQ="
                 data-src="   " data-lazy-src="/real.jpg" alt="x"></body>"#,
        );
        assert_eq!(p.images[0].src, "/real.jpg");
    }

    #[test]
    fn sin_data_src_se_toma_el_primer_candidato_de_data_srcset() {
        let p = parse(
            r#"<body><img src="data:image/gif;base64,R0lGODlhAQ="
                 data-srcset="/w-300.jpg 300w, /w-1024.jpg 1024w" alt="x"></body>"#,
        );
        assert_eq!(p.images[0].src, "/w-300.jpg");
    }

    #[test]
    fn data_srcset_cuenta_como_srcset() {
        // El srcset real también viaja en `data-srcset` con lazy-load; `in_srcset`
        // debe reflejarlo o ASSET-IMG-NO-SRCSET daría un falso positivo por imagen.
        let p = parse(
            r#"<body><img src="data:image/gif;base64,R0lGODlhAQ="
                 data-src="/foto.jpg"
                 data-srcset="/foto-300.jpg 300w, /foto.jpg 876w" alt="x"></body>"#,
        );
        assert!(p.images[0].in_srcset);
    }

    #[test]
    fn un_img_sin_src_pero_con_data_src_no_se_pierde() {
        // El lazy-load antiguo (jQuery lazyload, primeras versiones de lazysizes)
        // omitía el `src` por completo. Hoy ese `<img>` se descartaba entero.
        let p = parse(r#"<body><img data-src="/recuperada.jpg" alt="x"></body>"#);
        assert_eq!(p.images.len(), 1);
        assert_eq!(p.images[0].src, "/recuperada.jpg");
    }

    #[test]
    fn un_data_uri_sin_atributos_lazy_es_una_imagen_incrustada_legitima() {
        // Guarda de no-regresión: un icono SVG en línea sin lazy-load debe seguir
        // pasando tal cual (y descartándose después en la normalización, como hasta ahora).
        let p = parse(r#"<body><img src="data:image/svg+xml;base64,PHN2Zz4=" alt="icono"></body>"#);
        assert_eq!(p.images.len(), 1);
        assert_eq!(p.images[0].src, "data:image/svg+xml;base64,PHN2Zz4=");
    }

    #[test]
    fn un_data_src_que_tambien_es_data_uri_no_sustituye_al_src() {
        // Si el propio `data-src` es otro `data:` no hay URL real que rescatar;
        // se deja el src original y la normalización lo descartará aguas abajo.
        let p = parse(
            r#"<body><img src="data:image/gif;base64,R0lGODlhAQ="
                 data-src="data:image/png;base64,iVBORw0=" alt="x"></body>"#,
        );
        assert_eq!(p.images[0].src, "data:image/gif;base64,R0lGODlhAQ=");
    }

    // --- Recursos ---

    #[test]
    fn recoge_hojas_de_estilo_scripts_e_iframes() {
        let p = parse(
            r#"<head><link rel="stylesheet" href="/e.css"></head>
               <body><script src="/a.js"></script><iframe src="/marco"></iframe></body>"#,
        );
        assert_eq!(p.stylesheets, vec!["/e.css"]);
        assert_eq!(p.scripts, vec!["/a.js"]);
        assert!(p.links.iter().any(|l| l.element == LinkElement::Iframe && l.href == "/marco"));
        assert!(p.links.iter().any(|l| l.element == LinkElement::Link));
    }

    // --- JSON-LD ---

    #[test]
    fn extrae_los_tipos_de_json_ld() {
        let p = parse(
            r#"<head><script type="application/ld+json">
                 {"@context":"https://schema.org","@type":"Article","author":{"@type":"Person"}}
               </script></head>"#,
        );
        assert!(p.schema_types.contains(&"Article".to_string()));
        assert!(p.schema_types.contains(&"Person".to_string()), "debe bajar a los anidados");
    }

    #[test]
    fn extrae_los_tipos_de_un_graph() {
        let p = parse(
            r#"<head><script type="application/ld+json">
                 {"@graph":[{"@type":"WebPage"},{"@type":"BreadcrumbList"}]}
               </script></head>"#,
        );
        assert!(p.schema_types.contains(&"WebPage".to_string()));
        assert!(p.schema_types.contains(&"BreadcrumbList".to_string()));
    }

    #[test]
    fn un_json_ld_invalido_no_rompe_el_parseo() {
        let p = parse(
            r#"<head><script type="application/ld+json">{roto</script></head>
               <body><h1>Sigo aquí</h1></body>"#,
        );
        assert!(p.schema_types.is_empty());
        assert_eq!(p.h1(), Some("Sigo aquí"), "el resto de la página debe parsearse igual");
    }

    // --- Texto ---

    #[test]
    fn cuenta_palabras_excluyendo_lo_que_no_es_contenido() {
        let p = parse(
            r#"<body>
                 <nav>menú uno dos</nav>
                 <script>var x = 1 + 2 + 3;</script>
                 <style>.a { color: red }</style>
                 <footer>pie de pagina</footer>
                 <p>Uno dos tres cuatro cinco</p>
               </body>"#,
        );
        assert_eq!(p.word_count, 5, "solo cuentan las palabras del párrafo");
    }

    #[test]
    fn no_acumula_el_cuerpo_salvo_que_se_pida() {
        let html = "<body><p>Texto de prueba</p></body>";
        assert!(parse_html(html.as_bytes(), false).body_text.is_none());
        let con = parse_html(html.as_bytes(), true);
        assert_eq!(con.body_text.as_deref(), Some("Texto de prueba"));
    }

    #[test]
    fn calcula_el_ratio_de_contenido() {
        let p = parse("<body><p>hola</p></body>");
        assert!(p.content_ratio() > 0.0 && p.content_ratio() <= 1.0);
        assert_eq!(ParsedPage::default().content_ratio(), 0.0, "sin HTML no hay ratio");
    }

    // --- Resiliencia ---

    #[test]
    fn un_html_sin_cerrar_no_pierde_lo_ya_extraido() {
        let p = parse("<html><head><title>Título</title></head><body><h1>Encabezado");
        assert_eq!(p.title.as_deref(), Some("Título"));
    }

    #[test]
    fn un_documento_vacio_no_produce_nada() {
        let p = parse("");
        assert!(p.title.is_none());
        assert!(p.links.is_empty());
        assert_eq!(p.word_count, 0);
    }

    // --- Entidades HTML ---

    #[test]
    fn desescapa_las_entidades_del_titulo() {
        // Regresión: el título se guardaba con `&amp;` literal, lo que además de mostrarse mal
        // contaba cinco caracteres en vez de uno y falseaba title_len y title_px.
        let p = parse("<head><title>De la Torre &amp; Ucelay Abogados</title></head>");
        assert_eq!(p.title.as_deref(), Some("De la Torre & Ucelay Abogados"));
    }

    #[test]
    fn desescapa_entidades_numericas_decimales_y_hexadecimales() {
        assert_eq!(decode_entities("A &#038; B"), "A & B");
        assert_eq!(decode_entities("A &#38; B"), "A & B");
        assert_eq!(decode_entities("A &#x26; B"), "A & B");
        assert_eq!(decode_entities("&#191;qu&#233;?"), "¿qué?");
    }

    #[test]
    fn desescapa_las_entidades_que_importan_en_espanol() {
        assert_eq!(decode_entities("&iquest;Dise&ntilde;o &oacute; arte?"), "¿Diseño ó arte?");
        assert_eq!(decode_entities("Ma&ntilde;ana"), "Mañana");
    }

    #[test]
    fn un_ampersand_suelto_no_se_come_texto() {
        // Un `&` que no abre entidad debe sobrevivir tal cual.
        assert_eq!(decode_entities("Tom & Jerry"), "Tom & Jerry");
        assert_eq!(decode_entities("a & b & c"), "a & b & c");
        assert_eq!(decode_entities("100 % & 200"), "100 % & 200");
    }

    #[test]
    fn una_entidad_desconocida_se_deja_intacta() {
        // Mejor mostrar `&loquesea;` que comerse texto del sitio.
        assert_eq!(decode_entities("x &loquesea; y"), "x &loquesea; y");
    }

    #[test]
    fn un_texto_sin_entidades_no_se_toca() {
        assert_eq!(decode_entities("texto normal sin nada"), "texto normal sin nada");
        assert_eq!(decode_entities(""), "");
    }

    #[test]
    fn la_longitud_del_titulo_cuenta_el_caracter_y_no_la_entidad() {
        // Es el motivo real por el que esto importa: `&amp;` son cinco caracteres.
        let p = parse("<head><title>A &amp; B</title></head>");
        assert_eq!(p.title.as_deref().map(|t| t.chars().count()), Some(5), "«A & B»");
    }

    #[test]
    fn desescapa_tambien_encabezados_anclas_y_descripcion() {
        let p = parse(
            r#"<head><meta name="description" content="Uno &amp; dos"></head>
               <body><h1>Tres &amp; cuatro</h1><a href="/x">Cinco &amp; seis</a></body>"#,
        );
        assert_eq!(p.meta_description.as_deref(), Some("Uno & dos"));
        assert_eq!(p.h1(), Some("Tres & cuatro"));
        assert_eq!(p.links[0].anchor.as_deref(), Some("Cinco & seis"));
    }

    #[test]
    fn el_espacio_no_separable_se_normaliza_como_espacio() {
        // `&nbsp;` se resuelve a U+00A0, que es whitespace, así que colapsa con el resto.
        // Screaming Frog lo conserva; normalizarlo es mejor para comparar duplicados.
        let p = parse("<head><title>Uno&nbsp;&nbsp;dos</title></head>");
        assert_eq!(p.title.as_deref(), Some("Uno dos"));
    }

    #[test]
    fn maneja_html_no_utf8_sin_caerse() {
        let p = parse_html(&[0xff, 0xfe, b'<', b'p', b'>', 0x80, b'<', b'/', b'p', b'>'], false);
        assert_eq!(p.h1_count(), 0);
    }

    // --- Sonda de asignaciones ---
    //
    // Dos de los arreglos de rendimiento (el sink del rewriter y las copias por nodo de
    // texto) eliminan asignaciones que no cambian el resultado: no hay nada observable desde
    // fuera salvo la memoria. La única forma honesta de que sus tests de regresión fallen
    // sin el arreglo es contar las asignaciones. La sonda nació aquí y hoy vive en
    // `crate::alloc_probe`, compartida con los tests de despacho de `engine.rs`.

    use crate::alloc_probe::midiendo_asignaciones;

    // --- Regresiones de rendimiento del parseo ---

    #[test]
    fn una_avalancha_de_ampersands_no_es_cuadratica() {
        // Regresión: `find(';')` recorría todo el resto de la cadena por cada `&`, así que
        // una entrada sin `;` era O(n²) y un `<title>` hostil de 5 MB costaba ~14 minutos
        // con un hilo del runtime bloqueado. Un test de tiempo absoluto es aquí lo honesto
        // porque los dos mundos están a dos órdenes de magnitud, medidos en debug, que es
        // como corre este test: con la ventana acotada, 800.000 `&` tardan ~81 ms; con la
        // búsqueda cuadrática, ~13,5 s. El umbral de 2 s deja ~25x de margen al código
        // arreglado incluso en una máquina lenta, y la cuadrática lo supera ~7x.
        let entrada = "&".repeat(800_000);
        let t = std::time::Instant::now();
        let salida = decode_entities(&entrada);
        let transcurrido = t.elapsed();
        assert_eq!(salida, entrada, "los `&` sueltos sobreviven tal cual");
        assert!(
            transcurrido < std::time::Duration::from_secs(2),
            "decode_entities tardó {transcurrido:?} con 800.000 `&`: la búsqueda del ';' vuelve a ser cuadrática"
        );
    }

    #[test]
    fn la_ventana_de_entidad_termina_exactamente_en_32() {
        // Protege la aritmética de la ventana acotada frente a un descuadre de uno: el `;`
        // a 32 caracteres del `&` aún cierra entidad; a 33, el `&` es un `&` suelto.
        let dentro = format!("&#{}38;", "0".repeat(28)); // `;` en la posición 32
        assert_eq!(decode_entities(&dentro), "&");
        let fuera = format!("&#{}38;", "0".repeat(29)); // `;` en la posición 33
        assert_eq!(decode_entities(&fuera), fuera);
    }

    #[test]
    fn parsear_no_copia_el_documento_al_sink() {
        // Regresión: el sink del rewriter copiaba el documento serializado a un `Vec` que
        // nunca se leía —hasta 10 MB por documento en vuelo, multiplicado por la
        // concurrencia—. Parsear no debe asignar ni de lejos el tamaño del documento:
        // medido, este documento de 4 MB asigna ~29 KB en total; con la copia del sink son
        // ≥4 MB solo del `Vec`, así que el umbral de `html.len()` separa los dos mundos
        // con margen de sobra (~136x).
        let html = format!("<html><body><p>{}</p></body></html>", "palabra ".repeat(500_000));
        let (p, bytes, _) = midiendo_asignaciones(|| parse_html(html.as_bytes(), false));
        assert_eq!(p.word_count, 500_000, "el documento se parseó entero");
        assert!(
            (bytes as usize) < html.len(),
            "parsear un documento de {} bytes asignó {bytes} bytes: el sink vuelve a copiar la salida",
            html.len()
        );
    }

    #[test]
    fn los_nodos_de_texto_no_asignan_una_string_por_chunk() {
        // Regresión: los manejadores `text!` hacían `t.as_str().to_string()` por cada nodo
        // de texto —hasta 2.000 por página— para tirar la copia al instante. Con 10.000
        // párrafos, la versión con copia hace ≥10.000 asignaciones solo de chunks; el
        // parseo entero arreglado queda, medido, en 357. El umbral de 2.000 deja ~5,6x de
        // margen por arriba y ~5x por abajo.
        let html = format!("<html><body>{}</body></html>", "<p>uno dos</p>".repeat(10_000));
        let (p, _, asignaciones) = midiendo_asignaciones(|| parse_html(html.as_bytes(), false));
        assert_eq!(p.word_count, 20_000, "el documento se parseó entero");
        assert!(
            asignaciones < 2_000,
            "parsear 10.000 nodos de texto hizo {asignaciones} asignaciones: los manejadores `text!` vuelven a copiar cada chunk"
        );
    }

    #[test]
    fn los_schema_types_tienen_tope_y_conservan_los_primeros() {
        // Regresión: un JSON-LD hostil con cientos de miles de `@type` distintos acababa
        // entero en `pages.schema_types`, tras un `contains` lineal por cada tipo. El tope
        // corta la columna; la deduplicación pasa por HashSet y deja de ser cuadrática.
        let tipos = (0..200)
            .map(|i| format!(r#"{{"@type":"Tipo{i}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let html = format!(
            r#"<head><script type="application/ld+json">{{"@graph":[{tipos}]}}</script></head>"#
        );
        let p = parse(&html);
        assert_eq!(p.schema_types.len(), MAX_SCHEMA_TYPES, "el tope corta la lista");
        assert_eq!(p.schema_types[0], "Tipo0", "se conservan los primeros en orden");
        assert!(!p.schema_types.contains(&"Tipo199".to_string()));
    }

    #[test]
    fn los_tipos_duplicados_entre_bloques_no_se_repiten() {
        // La deduplicación debe aplicar entre bloques JSON-LD distintos de la misma página,
        // como hacía antes del tope.
        let p = parse(
            r#"<head>
                 <script type="application/ld+json">{"@type":"Article"}</script>
                 <script type="application/ld+json">{"@type":"Article"}</script>
                 <script type="application/ld+json">{"@type":"Person"}</script>
               </head>"#,
        );
        assert_eq!(p.schema_types, vec!["Article".to_string(), "Person".to_string()]);
    }
}
