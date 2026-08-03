//! Búsqueda de texto completo de extremo a extremo: rastrear con `collect_body_text` y
//! **buscar** por `pages_fts`.
//!
//! Existe porque la tabla estuvo vacía desde la migración 001: el esquema la creaba, el parseo
//! recogía el texto y nadie lo escribía. Es el mismo defecto que ya apareció dos veces en este
//! proyecto (`resources`, la tabla de sitemaps): algo que existe, parece funcionar y está vacío,
//! y ningún test lo nota porque ninguno *consulta*. Este fichero consulta.
//!
//! La promesa concreta que se verifica es la del tokenizador de la migración
//! (`unicode61 remove_diacritics 2`, obligatorio para español): «diseño» tiene que encontrarse
//! buscando «diseno», y viceversa.

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;

/// Directorio temporal que se limpia solo.
struct Fixture {
    path: std::path::PathBuf,
}

impl Fixture {
    fn new(nombre: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("crawlforge-fts-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear directorio de fixture");
        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let full = self.path.join(relative);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("crear subdirectorio");
        }
        std::fs::write(full, contents).expect("escribir fichero");
    }

    fn store(&self) -> std::path::PathBuf {
        self.path.join("crawl.sqlite")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Sitio mínimo con acentos repartidos por las tres fuentes indexadas: cuerpo, título y
/// meta description.
fn sitio_con_acentos(f: &Fixture) {
    f.write(
        "index.html",
        r#"<!DOCTYPE html><html lang="es"><head><title>Inicio</title></head>
           <body><main><h1>Portada</h1>
           <p>El diseño de páginas rápidas empieza por medir.</p>
           <a href="/optica/">Óptica</a></main></body></html>"#,
    );
    f.write(
        "optica/index.html",
        r#"<!DOCTYPE html><html lang="es"><head><title>Óptica avanzada</title>
           <meta name="description" content="Guía de fotografía nocturna."></head>
           <body><main><h1>Óptica</h1><p>Lentes y monturas.</p></main></body></html>"#,
    );
}

fn open(path: &std::path::Path) -> Connection {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("abrir el fichero de rastreo")
}

/// URLs cuyo `pages_fts` casa con la consulta, resueltas por el `rowid = urls.id` que la UI
/// usará igual: es la consulta completa, no un recuento.
fn buscar(conn: &Connection, query: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT u.url FROM pages_fts f JOIN urls u ON u.id = f.rowid
             WHERE pages_fts MATCH ?1 ORDER BY u.url",
        )
        .expect("preparar la búsqueda");
    let rows = stmt.query_map([query], |r| r.get(0)).expect("buscar");
    rows.collect::<rusqlite::Result<_>>().expect("recoger resultados")
}

#[tokio::test]
async fn con_collect_body_text_la_busqueda_encuentra_incluso_sin_tildes() {
    let f = Fixture::new("pro");
    sitio_con_acentos(&f);

    let mut job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    job.collect_body_text = true;
    let outcome = engine::run(job, &f.store()).await.expect("rastrear");
    let conn = open(&outcome.store_path);

    // La promesa del tokenizador: la palabra está escrita «diseño» y se busca «diseno».
    assert_eq!(
        buscar(&conn, "diseno"),
        vec!["https://ejemplo.es/"],
        "una palabra con tilde tiene que encontrarse buscada sin ella"
    );
    // Y en la dirección contraria: quien escribe la tilde también encuentra.
    assert_eq!(buscar(&conn, "diseño"), vec!["https://ejemplo.es/"]);

    // Las cuatro columnas que declara la migración están pobladas, no solo el cuerpo.
    assert_eq!(buscar(&conn, "title:optica"), vec!["https://ejemplo.es/optica/"]);
    assert_eq!(
        buscar(&conn, "meta_description:fotografia"),
        vec!["https://ejemplo.es/optica/"]
    );
    assert_eq!(buscar(&conn, "body_text:rapidas"), vec!["https://ejemplo.es/"]);
    assert_eq!(buscar(&conn, "url:optica"), vec!["https://ejemplo.es/optica/"]);

    // Cero resultados sigue significando «no está», no «nadie pobló la tabla».
    assert!(buscar(&conn, "inexistente").is_empty());

    // Todas las páginas quedaron indexadas.
    let indexadas: i64 = conn
        .query_row("SELECT COUNT(*) FROM pages_fts", [], |r| r.get(0))
        .expect("contar");
    let paginas: i64 =
        conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0)).expect("contar");
    assert_eq!(indexadas, paginas, "cada página con texto tiene su entrada en el índice");
}

#[tokio::test]
async fn sin_collect_body_text_la_tabla_queda_vacia_como_documenta_el_modelo() {
    // `docs/02-MODELO-DATOS.md §3.7`: «solo se puebla en nivel Pro». Un rastreo sin
    // `collect_body_text` no debe dejarla poblada a medias con títulos sueltos.
    let f = Fixture::new("free");
    sitio_con_acentos(&f);

    let job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    assert!(!job.collect_body_text, "el valor por defecto es no recoger texto");
    let outcome = engine::run(job, &f.store()).await.expect("rastrear");
    let conn = open(&outcome.store_path);

    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM pages_fts", [], |r| r.get(0))
        .expect("contar");
    assert_eq!(n, 0);
}
