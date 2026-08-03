//! Una auditoría de un `dist/` lee el sitemap que el generador dejó dentro.
//!
//! Hasta el 2026-07-30 no lo hacía: `CrawlJob::filesystem` dejaba `discover_sitemaps` en `false`.
//! La consecuencia no se veía leyendo el código —ningún test fallaba— pero era grave: sin leer el
//! sitemap, `urls.in_sitemap` valía 0 en todas las filas, así que la vista `v_orphans` no podía
//! devolver nada y **la detección de páginas huérfanas estaba muerta justo en el modo que
//! diferencia el producto**. Cuatro reglas `INDEX-*` dependían de ello.
//!
//! Estos tests existen para que no se vuelva a apagar sin que nadie se entere.

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;

struct Dist {
    path: std::path::PathBuf,
}

impl Dist {
    fn nuevo(nombre: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("crawlforge-sm-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear dist");
        Self { path }
    }

    fn escribe(&self, relativa: &str, contenido: &str) {
        let completa = self.path.join(relativa);
        if let Some(padre) = completa.parent() {
            std::fs::create_dir_all(padre).expect("crear subdirectorio");
        }
        std::fs::write(completa, contenido).expect("escribir fichero");
    }

    fn pagina(&self, ruta: &str, titulo: &str, cuerpo: &str) {
        let fichero =
            if ruta == "/" { "index.html".to_string() } else { format!("{}index.html", &ruta[1..]) };
        self.escribe(
            &fichero,
            &format!(
                "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\">\
                 <title>{titulo}</title><link rel=\"canonical\" href=\"https://fixture.local{ruta}\">\
                 </head><body><main><h1>{titulo}</h1>{cuerpo}</main></body></html>"
            ),
        );
    }

    fn store(&self) -> std::path::PathBuf {
        self.path.join("crawl.sqlite")
    }
}

impl Drop for Dist {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn abrir(path: &std::path::Path) -> Connection {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el rastreo")
}

/// `dist/` con tres páginas, de las que solo una está enlazada. Las tres en el sitemap.
fn dist_con_huerfana(nombre: &str) -> Dist {
    let d = Dist::nuevo(nombre);
    d.pagina("/", "Portada", "<a href=\"/enlazada/\">Enlazada</a>");
    d.pagina("/enlazada/", "Enlazada", "<p>Tiene un enlace entrante.</p>");
    d.pagina("/huerfana/", "Huerfana", "<p>No lo tiene.</p>");
    d.escribe(
        "sitemap.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://fixture.local/</loc></url>
  <url><loc>https://fixture.local/enlazada/</loc></url>
  <url><loc>https://fixture.local/huerfana/</loc></url>
</urlset>"#,
    );
    d
}

#[tokio::test]
async fn una_auditoria_de_dist_marca_las_urls_que_estan_en_el_sitemap() {
    let d = dist_con_huerfana("in-sitemap");
    let job = CrawlJob::filesystem(&d.path, "https://fixture.local/");
    assert!(job.discover_sitemaps, "el modo filesystem tiene que leer sitemaps");

    let outcome = engine::run(job, &d.store()).await.expect("auditar");
    let conn = abrir(&outcome.store_path);

    let en_sitemap: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls WHERE in_sitemap = 1", [], |r| r.get(0))
        .expect("contar");
    assert_eq!(en_sitemap, 3, "las tres URLs del sitemap deben quedar marcadas");
}

#[tokio::test]
async fn la_vista_de_huerfanas_encuentra_la_pagina_que_nadie_enlaza() {
    let d = dist_con_huerfana("orphans");
    let job = CrawlJob::filesystem(&d.path, "https://fixture.local/");
    let outcome = engine::run(job, &d.store()).await.expect("auditar");
    let conn = abrir(&outcome.store_path);

    let mut stmt = conn.prepare("SELECT url FROM v_orphans").expect("consultar v_orphans");
    let huerfanas: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .expect("leer")
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        huerfanas,
        vec!["https://fixture.local/huerfana/".to_string()],
        "solo la página sin enlaces entrantes es huérfana; ni la portada ni la enlazada lo son"
    );
}

#[tokio::test]
async fn un_dist_sin_sitemap_se_audita_igual() {
    // La otra mitad del cambio: leer sitemaps no puede convertirse en un requisito. Un `dist/`
    // sin sitemap.xml —o en pleno desarrollo, antes de generarlo— se audita como siempre.
    let d = Dist::nuevo("sin-sitemap");
    d.pagina("/", "Portada", "<a href=\"/otra/\">Otra</a>");
    d.pagina("/otra/", "Otra", "<p>Contenido.</p>");

    let job = CrawlJob::filesystem(&d.path, "https://fixture.local/");
    let outcome = engine::run(job, &d.store()).await.expect("auditar");
    let conn = abrir(&outcome.store_path);

    let paginas: i64 =
        conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0)).expect("contar");
    assert_eq!(paginas, 2);

    let en_sitemap: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls WHERE in_sitemap = 1", [], |r| r.get(0))
        .expect("contar");
    assert_eq!(en_sitemap, 0, "sin sitemap no hay nada marcado, y no es un error");
}

#[tokio::test]
async fn el_sitemap_puede_declarar_una_ruta_que_no_existe_en_dist() {
    // El caso que hace valioso leer el sitemap en este modo: el generador declara una URL que
    // luego no produjo fichero. Al publicar, esa URL es un 404 que el buscador ya conoce.
    let d = Dist::nuevo("declara-de-mas");
    d.pagina("/", "Portada", "<p>Sin enlaces.</p>");
    d.escribe(
        "sitemap.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://fixture.local/</loc></url>
  <url><loc>https://fixture.local/no-generada/</loc></url>
</urlset>"#,
    );

    let job = CrawlJob::filesystem(&d.path, "https://fixture.local/");
    let outcome = engine::run(job, &d.store()).await.expect("auditar");
    let conn = abrir(&outcome.store_path);

    let estado: Option<i64> = conn
        .query_row(
            "SELECT status_code FROM urls WHERE url = 'https://fixture.local/no-generada/'",
            [],
            |r| r.get(0),
        )
        .expect("la URL del sitemap tiene que existir como fila");
    assert_eq!(estado, Some(404), "la ruta declarada y no generada se audita como 404");
}

/// El caso de WordPress: el sitemap de imágenes hacía huérfana a media biblioteca de medios.
///
/// `v_orphans` pedía tres cosas —interna, en el sitemap, sin enlaces entrantes— y no pedía la
/// que da nombre a la regla: ser una página. Un `/wp-content/uploads/foto.png` cumple las tres,
/// porque las imágenes se usan con `<img src>` y eso va a la tabla `images`, no a `links`.
///
/// Medido rastreando un medio de comunicación el 2026-08-01: **1.867 de 1.912 hallazgos de
/// `INDEX-ORPHAN-PAGE` eran imágenes**, todas descargadas con 200 y ninguna con fila en `pages`,
/// en severidad `high`, en el CMS más extendido. Lo arregla la migración 005 exigiendo esa fila.
///
/// La que aquí queda fuera del sitemap de imágenes es la que ninguna página usa con esa URL
/// exacta, que es el caso corriente: el sitemap lista el original y las páginas insertan
/// miniaturas. Una imagen que sí aparece en un `<img src>` genera fila en `links` y nunca fue
/// huérfana; por eso está también en el fixture, para que se vea que el arreglo no es esa.
#[tokio::test]
async fn una_imagen_del_sitemap_no_es_una_pagina_huerfana() {
    let d = Dist::nuevo("sitemap-de-imagenes");
    d.pagina("/", "Portada", "<img src=\"/uploads/usada.png\" alt=\"Una foto\">");
    // Basta con que existan y con que el nombre acabe en .png: lo que se prueba es la vista.
    d.escribe("uploads/usada.png", "PNG falso, y da igual: aquí no se decodifica nada");
    d.escribe("uploads/original.png", "PNG falso, y da igual: aquí no se decodifica nada");
    d.escribe(
        "sitemap.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://fixture.local/</loc></url>
  <url><loc>https://fixture.local/uploads/usada.png</loc></url>
  <url><loc>https://fixture.local/uploads/original.png</loc></url>
</urlset>"#,
    );

    let job = CrawlJob::filesystem(&d.path, "https://fixture.local/");
    let outcome = engine::run(job, &d.store()).await.expect("auditar");
    let conn = abrir(&outcome.store_path);

    // La imagen está en el sitemap y nadie la enlaza con un `<a>`: cumplía las tres condiciones
    // viejas. Si vuelve a salir aquí, el falso positivo ha vuelto.
    let mut stmt = conn.prepare("SELECT url FROM v_orphans").expect("consultar v_orphans");
    let huerfanas: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .expect("leer")
        .filter_map(Result::ok)
        .collect();

    assert!(
        huerfanas.is_empty(),
        "una imagen no es una página huérfana, y aquí no hay ninguna otra candidata: {huerfanas:?}"
    );

    // Y que conste que las imágenes se rastrearon: si no, el test pasaría por no haberlas visto.
    let vistas: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE path LIKE '/uploads/%' AND in_sitemap = 1
               AND crawl_state = 'done'",
            [],
            |r| r.get(0),
        )
        .expect("contar");
    assert_eq!(vistas, 2, "las dos imágenes del sitemap tienen que estar rastreadas y marcadas");
}

/// Un rastreo truncado no puede afirmar que nadie enlaza a una página.
///
/// La otra mitad del mismo falso positivo: en ese medio el rastreo se cortó en 20.000 de
/// las 176.000 URLs declaradas, y todo lo que el sitemap anunciaba y no se llegó a visitar salía
/// como huérfano. Con la migración 005 esas URLs ya no tienen fila en `pages`, así que caen solas.
#[tokio::test]
async fn lo_que_el_sitemap_declara_y_el_rastreo_no_alcanzo_no_es_huerfano() {
    let d = Dist::nuevo("truncado");
    d.pagina("/", "Portada", "<p>Sin enlaces.</p>");
    d.escribe(
        "sitemap.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://fixture.local/</loc></url>
  <url><loc>https://fixture.local/no-generada/</loc></url>
</urlset>"#,
    );

    let job = CrawlJob::filesystem(&d.path, "https://fixture.local/");
    let outcome = engine::run(job, &d.store()).await.expect("auditar");
    let conn = abrir(&outcome.store_path);

    let huerfanas: i64 = conn
        .query_row("SELECT COUNT(*) FROM v_orphans", [], |r| r.get(0))
        .expect("contar huérfanas");
    assert_eq!(
        huerfanas, 0,
        "una URL del sitemap que devolvió 404 no es una página huérfana: no es una página"
    );
}
