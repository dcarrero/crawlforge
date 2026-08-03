//! Sitemaps XML: análisis de índices anidados, sitemaps de URLs y variantes `.gz`.
//! Ver `docs/03-MOTOR-CRAWL.md §4`.
//!
//! El cruce sitemap ↔ enlaces es lo que produce los hallazgos de páginas huérfanas: una URL
//! que el sitio declara en su sitemap pero a la que no llega ningún enlace interno.

use std::io::Read;

/// Rutas donde buscar un sitemap cuando `robots.txt` no anuncia ninguno.
pub const WELL_KNOWN_SITEMAP_PATHS: &[&str] = &["/sitemap.xml", "/sitemap_index.xml"];

/// Resultado de analizar un documento de sitemap.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Sitemap {
    /// URLs de página, de un `<urlset>`.
    pub urls: Vec<String>,
    /// Sitemaps hijos, de un `<sitemapindex>`. Hay que seguirlos.
    pub children: Vec<String>,
    /// Por qué no se pudo leer del todo, si es que pasó.
    ///
    /// El parseo es tolerante a propósito —media lista de URLs es mejor que ninguna— pero esa
    /// tolerancia escondía el problema: un sitemap corrupto se veía igual que uno vacío. La
    /// regla `INDEX-SITEMAP-ERROR` necesita saber que lo hubo, y el usuario también: un índice
    /// que se corta a la mitad deja sin descubrir todo lo que venía detrás.
    pub parse_error: Option<String>,
}

impl Sitemap {
    pub fn is_empty(&self) -> bool {
        self.urls.is_empty() && self.children.is_empty()
    }
}

/// Analiza un sitemap, descomprimiéndolo antes si viene en gzip.
///
/// Distingue `<urlset>` de `<sitemapindex>` por el elemento padre del `<loc>`, no por el
/// nombre del fichero: hay sitios que sirven un índice desde `sitemap.xml` y una lista de
/// URLs desde `sitemap_index.xml`. Fiarse del nombre pierde páginas.
pub fn parse(body: &[u8]) -> Sitemap {
    let decompressed;
    let xml = if is_gzip(body) {
        match decompress(body) {
            Ok(Some(d)) => {
                decompressed = d;
                decompressed.as_slice()
            }
            // Se pasa del tope: no se lee, y el motivo queda como hallazgo. Un sitemap que al
            // descomprimirse ocupa más que el límite del protocolo es un defecto del sitio, y
            // decirlo es más útil —y mucho más barato— que intentar procesarlo.
            Ok(None) => {
                tracing::warn!("sitemap .gz por encima del tope de descompresión");
                return Sitemap {
                    parse_error: Some(format!(
                        "el sitemap comprimido supera el tope de {} MB al descomprimirse",
                        MAX_DECOMPRESSED_BYTES / (1024 * 1024)
                    )),
                    ..Sitemap::default()
                };
            }
            Err(e) => {
                tracing::warn!(error = %e, "sitemap .gz ilegible");
                return Sitemap {
                    parse_error: Some(format!("gzip ilegible: {e}")),
                    ..Sitemap::default()
                };
            }
        }
    } else {
        body
    };

    parse_xml(xml)
}

/// Cabecera mágica de gzip. Algunos servidores sirven `.xml.gz` sin `Content-Encoding`,
/// así que hay que detectarlo por contenido.
fn is_gzip(body: &[u8]) -> bool {
    body.starts_with(&[0x1f, 0x8b])
}

/// Tope de lo que se acepta al descomprimir un sitemap.
///
/// **No es una cifra de estilo, es una defensa.** `read_to_end` sobre un `GzDecoder` no tiene
/// límite, y deflate alcanza ratios de ~1030:1: un fichero de 1 MB servido desde `/sitemap.xml`
/// —detectado por su cabecera mágica, sin necesidad de `Content-Encoding` ni de extensión `.gz`—
/// se convertía en **2,1 GB de memoria residente**, medido. Con el tope real de respuesta de
/// 10 MB, en unos 21 GB: cualquier sitio ajeno tumbaba la aplicación con un fichero pequeño.
///
/// 64 MB deja sitio de sobra a un sitemap legítimo: el protocolo los limita a 50 MB sin
/// comprimir, y ese límite ya es un hallazgo del catálogo (`INDEX-SITEMAP-ERROR`).
pub const MAX_DECOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;

/// Descomprime con un tope. Devuelve `None` si el fichero se pasa.
fn decompress(body: &[u8]) -> std::io::Result<Option<Vec<u8>>> {
    let mut out = Vec::new();
    // `take` corta la lectura en el tope en vez de crecer sin fin. Se pide un byte de más para
    // poder distinguir «cabe justo» de «se ha cortado».
    let leidos = flate2::read::GzDecoder::new(body)
        .take(MAX_DECOMPRESSED_BYTES + 1)
        .read_to_end(&mut out)?;
    if leidos as u64 > MAX_DECOMPRESSED_BYTES {
        return Ok(None);
    }
    Ok(Some(out))
}

fn parse_xml(xml: &[u8]) -> Sitemap {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut result = Sitemap::default();
    let mut buf = Vec::new();
    // Un `<loc>` significa cosas distintas según su padre: dentro de `<sitemap>` es un
    // sitemap hijo; dentro de `<url>` es una página.
    let mut inside_sitemap_entry = false;
    let mut inside_loc = false;
    // Profundidad dentro de una extensión del protocolo (`<image:image>`, `<video:video>`,
    // `<news:news>`). Sus hijos no declaran páginas, pero `local_name` recorta el prefijo y
    // deja el `<image:loc>` del sitemap de imágenes de WordPress indistinguible de un `<loc>`
    // de página. Así entraron 1.868 imágenes con `in_sitemap=1` en un rastreo real de 20.000
    // URLs —el 9,4% del presupuesto gastado en descargar imágenes como si fueran páginas— y
    // de ahí salió el falso `INDEX-NOINDEX-IN-SITEMAP` sobre URLs que el sitio declara como
    // imágenes, no como contenido a indexar. Es un contador y no un booleano porque las
    // extensiones anidan (`<video:video>` lleva estructura dentro) y un cierre no puede
    // reabrir el estado de página antes de tiempo.
    let mut extension_depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match local_name(e.name().as_ref()) {
                b"sitemap" => inside_sitemap_entry = true,
                b"url" => inside_sitemap_entry = false,
                // `video` y `news` no tienen hoy un hijo que colisione con `loc` tras
                // recortar el prefijo (`content_loc`, `player_loc`), pero se cierran igual:
                // la lección de `image` es que el recorte de prefijos convierte cualquier
                // hijo futuro `*:loc` en una página fantasma.
                b"image" | b"video" | b"news" => extension_depth += 1,
                b"loc" if extension_depth == 0 => inside_loc = true,
                _ => {}
            },
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                b"sitemap" => inside_sitemap_entry = false,
                b"image" | b"video" | b"news" => {
                    extension_depth = extension_depth.saturating_sub(1);
                }
                b"loc" => inside_loc = false,
                _ => {}
            },
            Ok(Event::Text(e)) if inside_loc => {
                if let Ok(text) = e.unescape() {
                    let value = text.trim();
                    if !value.is_empty() {
                        if inside_sitemap_entry {
                            result.children.push(value.to_string());
                        } else {
                            result.urls.push(value.to_string());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            // Un sitemap mal formado da lo que se haya podido leer hasta el error, no nada:
            // media lista de URLs es mejor que ninguna.
            Err(e) => {
                tracing::warn!(error = %e, "sitemap XML mal formado; se usa lo leído hasta aquí");
                result.parse_error = Some(format!("XML mal formado: {e}"));
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    result
}

/// Descarta el prefijo de espacio de nombres (`ns:loc` → `loc`). Los sitemaps reales usan
/// prefijos con bastante libertad.
fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const URLSET: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <url><loc>https://ejemplo.es/</loc><lastmod>2026-07-01</lastmod></url>
          <url><loc>https://ejemplo.es/blog/</loc><priority>0.8</priority></url>
        </urlset>"#;

    const INDEX: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <sitemap><loc>https://ejemplo.es/sitemap-posts.xml</loc></sitemap>
          <sitemap><loc>https://ejemplo.es/sitemap-pages.xml</loc></sitemap>
        </sitemapindex>"#;

    #[test]
    fn lee_las_urls_de_un_urlset() {
        let s = parse(URLSET);
        assert_eq!(s.urls, vec!["https://ejemplo.es/", "https://ejemplo.es/blog/"]);
        assert!(s.children.is_empty());
    }

    #[test]
    fn lee_los_hijos_de_un_indice_sin_confundirlos_con_paginas() {
        let s = parse(INDEX);
        assert!(s.urls.is_empty(), "un índice no aporta páginas directamente");
        assert_eq!(
            s.children,
            vec!["https://ejemplo.es/sitemap-posts.xml", "https://ejemplo.es/sitemap-pages.xml"]
        );
    }

    #[test]
    fn distingue_indice_de_urlset_por_el_elemento_padre_no_por_el_nombre_del_fichero() {
        // Hay sitios que sirven un índice desde `sitemap.xml`. Fiarse del nombre pierde páginas.
        assert!(parse(INDEX).urls.is_empty());
        assert!(parse(URLSET).children.is_empty());
    }

    #[test]
    fn tolera_prefijos_de_espacio_de_nombres() {
        let xml = br#"<?xml version="1.0"?>
            <ns:urlset xmlns:ns="http://www.sitemaps.org/schemas/sitemap/0.9">
              <ns:url><ns:loc>https://ejemplo.es/a</ns:loc></ns:url>
            </ns:urlset>"#;
        assert_eq!(parse(xml).urls, vec!["https://ejemplo.es/a"]);
    }

    #[test]
    fn un_image_loc_no_es_una_pagina_declarada() {
        // El sitemap de imágenes de WordPress, tal cual: cada `<url>` lleva su `<loc>` de
        // página y uno o más `<image:loc>` dentro de `<image:image>`. Tratarlos igual metía
        // las imágenes en el rastreo como páginas declaradas (1.868 en un sitio real, el
        // 9,4% del presupuesto de URLs).
        let xml = br#"<?xml version="1.0"?>
            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
                    xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
              <url>
                <loc>https://ejemplo.es/articulo-1</loc>
                <image:image><image:loc>https://ejemplo.es/foto-1.jpg</image:loc></image:image>
                <image:image>
                  <image:loc>https://ejemplo.es/foto-2.jpg</image:loc>
                  <image:title>Foto 2</image:title>
                </image:image>
              </url>
              <url><loc>https://ejemplo.es/articulo-2</loc></url>
            </urlset>"#;
        let s = parse(xml);
        assert_eq!(
            s.urls,
            vec!["https://ejemplo.es/articulo-1", "https://ejemplo.es/articulo-2"],
            "las imágenes no son páginas declaradas; el <loc> legítimo posterior sí entra"
        );
    }

    #[test]
    fn el_contenido_de_una_extension_de_video_tampoco_declara_paginas() {
        // `<video:video>` no trae hoy un hijo que acabe en `loc` a secas, pero el recorte de
        // prefijos de `local_name` convertiría cualquier `*:loc` futuro en página. Se cierra
        // la extensión entera, igual que `image`.
        let xml = br#"<?xml version="1.0"?>
            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
                    xmlns:video="http://www.google.com/schemas/sitemap-video/1.1">
              <url>
                <loc>https://ejemplo.es/pagina-con-video</loc>
                <video:video>
                  <video:loc>https://ejemplo.es/video.mp4</video:loc>
                  <video:title>Un video</video:title>
                </video:video>
              </url>
            </urlset>"#;
        assert_eq!(parse(xml).urls, vec!["https://ejemplo.es/pagina-con-video"]);
    }

    #[test]
    fn desescapa_las_entidades_xml() {
        let xml = br#"<urlset><url><loc>https://ejemplo.es/a?x=1&amp;y=2</loc></url></urlset>"#;
        assert_eq!(parse(xml).urls, vec!["https://ejemplo.es/a?x=1&y=2"]);
    }

    #[test]
    fn descomprime_un_sitemap_gzip() {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(URLSET).expect("comprimir");
        let gz = enc.finish().expect("cerrar gzip");

        assert!(is_gzip(&gz), "el buffer debería detectarse como gzip");
        assert_eq!(parse(&gz).urls.len(), 2);
    }

    #[test]
    fn un_sitemap_vacio_no_aporta_nada() {
        let s = parse(b"<urlset></urlset>");
        assert!(s.is_empty());
    }

    #[test]
    fn un_xml_mal_formado_devuelve_lo_leido_hasta_el_error() {
        // Media lista es mejor que ninguna: un cierre roto no debe descartar el sitemap entero.
        let xml = br#"<urlset><url><loc>https://ejemplo.es/a</loc></url><url><loc>sin cerrar"#;
        assert!(parse(xml).urls.contains(&"https://ejemplo.es/a".to_string()));
    }

    #[test]
    fn ignora_un_loc_vacio() {
        let xml = br#"<urlset><url><loc></loc></url><url><loc>https://ejemplo.es/a</loc></url></urlset>"#;
        assert_eq!(parse(xml).urls, vec!["https://ejemplo.es/a"]);
    }

    #[test]
    fn un_gzip_corrupto_no_hace_caer_el_rastreo() {
        let corrupto = [0x1f, 0x8b, 0x08, 0x00, 0xff, 0xff, 0xff, 0xff];
        assert!(parse(&corrupto).is_empty());
    }
}
