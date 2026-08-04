//! Tests de integración del pipeline completo, sobre un sitio de fixture con casos conocidos.
//!
//! Existen porque **todos los fallos reales salieron ejecutando el pipeline entero,
//! no escribiendo los módulos**. Ninguno lo habría cazado un test unitario: cada pieza hacía bien
//! su trabajo y el defecto estaba en la costura. El patrón para detectarlos es siempre el mismo,
//! y es el que reproduce este fichero: rastrear un sitio cuyo contenido se conoce exactamente y
//! comparar los recuentos con lo que se puso en él.

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;

/// Directorio temporal que se limpia solo.
struct Fixture {
    path: std::path::PathBuf,
}

impl Fixture {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-it-{}-{nombre}", std::process::id()));
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

fn open(path: &std::path::Path) -> Connection {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el fichero de rastreo")
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).expect("contar")
}

/// Sitio con: dos páginas que comparten título, una sin título ni h1 ni canonical, una con
/// noindex, un enlace roto y dos imágenes (una sin alt).
fn sitio_conocido(f: &Fixture) {
    f.write(
        "index.html",
        r#"<!DOCTYPE html><html lang="es"><head>
           <title>Inicio</title><link rel="canonical" href="https://ejemplo.es/">
           </head><body>
           <nav><a href="/about">Sobre</a></nav>
           <main><h1>Bienvenido</h1><p>Uno dos tres cuatro.</p>
             <a href="/blog/post-1">Post 1</a>
             <a href="/blog/post-2">Post 2</a>
             <a href="/no-existe">Roto</a>
             <a href="https://externo.com/x">Externo</a>
             <img src="/foto.webp" alt="Con alt">
             <img src="/sin-alt.png">
           </main>
           <footer><a href="/about">Sobre</a></footer></body></html>"#,
    );
    f.write(
        "about/index.html",
        r#"<html lang="es"><head><title>Repetido</title>
           <link rel="canonical" href="https://ejemplo.es/about/"></head>
           <body><main><h1>Sobre</h1><p>Texto.</p></main></body></html>"#,
    );
    f.write(
        "blog/post-1.html",
        r#"<html lang="es"><head><title>Repetido</title></head>
           <body><main><h1>Post uno</h1><p>Texto.</p></main></body></html>"#,
    );
    f.write(
        "blog/post-2.html",
        r#"<html lang="es"><head><meta name="description" content="sin nada"></head>
           <body><main><p>Sin título, sin h1, sin canonical.</p></main></body></html>"#,
    );
    f.write(
        "noindex.html",
        r#"<html lang="es"><head><title>Oculta</title>
           <meta name="robots" content="noindex, follow"></head>
           <body><main><h1>Oculta</h1></main></body></html>"#,
    );
}

#[tokio::test]
async fn el_rastreo_encuentra_exactamente_lo_que_hay_en_el_sitio() {
    let f = Fixture::new("completo");
    sitio_conocido(&f);

    let job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    let outcome = engine::run(job, &f.store()).await.expect("rastrear");
    let conn = open(&outcome.store_path);

    // Cinco ficheros HTML, cinco páginas. Ni una más: `noindex.html` acaba en la cadena
    // "index.html" y llegó a convertirse en la ruta fantasma `/no`.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM pages"), 5, "una página por fichero HTML");

    // Cuatro indexables: `noindex.html` no lo es.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM pages WHERE is_indexable = 1"), 4);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM pages WHERE indexability_reason = 'noindex'"
        ),
        1
    );

    // `/about` y `/about/` son la misma página: si se cuentan dos veces, el rastreo audita el
    // sitio por duplicado y las reglas de duplicados disparan sobre invenciones del motor.
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM urls WHERE url LIKE '%/about%'"),
        1,
        "las variantes de una misma ruta deben unificarse"
    );

    // Ocho enlaces salen de la portada: 2 a /about (nav y footer), 2 a posts, 1 roto,
    // 1 externo y 2 de imagen.
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM links l JOIN urls u ON u.id = l.from_url_id
             WHERE u.url = 'https://ejemplo.es/'"
        ),
        8,
        "no debe perderse ningún enlace por no encontrar su destino"
    );

    // Las regiones semánticas se distinguen.
    for (region, esperado) in [("nav", 1), ("footer", 1), ("main", 6)] {
        assert_eq!(
            count(
                &conn,
                &format!("SELECT COUNT(*) FROM links WHERE region = '{region}'")
            ),
            esperado,
            "enlaces en la región {region}"
        );
    }

    // Dos imágenes, y `alt=""` ausente se distingue de presente.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM images"), 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM images WHERE alt_present = 1"), 1);

    // El enlace roto se detecta.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM urls WHERE status_code = 404"), 3,
               "la página inexistente y las dos imágenes que no están en el directorio");
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM v_broken_links"), 3);

    // Lo externo se registra pero no se rastrea.
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM urls WHERE is_internal = 0 AND crawl_state = 'skipped'"),
        1
    );
}

#[tokio::test]
async fn las_reglas_encuentran_exactamente_los_problemas_sembrados() {
    let f = Fixture::new("reglas");
    sitio_conocido(&f);

    let job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    let outcome = engine::run(job, &f.store()).await.expect("rastrear");
    let conn = open(&outcome.store_path);

    let por_regla = |rule: &str| -> i64 {
        count(&conn, &format!("SELECT COUNT(*) FROM issues WHERE rule_id = '{rule}'"))
    };

    // post-2 no tiene título, h1 ni canonical.
    assert_eq!(por_regla("META-TITLE-MISSING"), 1);
    assert_eq!(por_regla("CONTENT-H1-MISSING"), 1);
    // post-1 y post-2 no tienen canonical; `noindex.html` no cuenta por no ser indexable.
    assert_eq!(por_regla("CANON-MISSING"), 2);
    // about/ y post-1 comparten el título "Repetido".
    assert_eq!(por_regla("META-TITLE-DUPLICATE"), 2);
    // Tres 4xx internos enlazados, repartidos entre dos reglas: `HTTP-404-INTERNAL` es de
    // enlaces (`element = 'a'`) y se queda con la página inexistente; las dos imágenes rotas
    // son de `ASSET-IMG-BROKEN`. Antes salían por las dos a la vez, con severidades que se
    // contradecían —critical y high— para el mismo fichero.
    assert_eq!(por_regla("HTTP-404-INTERNAL"), 1);
    assert_eq!(por_regla("ASSET-IMG-BROKEN"), 2);

    // Las páginas sin título no cuentan como duplicadas entre sí.
    let duplicados_sin_titulo: i64 = count(
        &conn,
        "SELECT COUNT(*) FROM issues i JOIN pages p ON p.url_id = i.url_id
         WHERE i.rule_id = 'META-TITLE-DUPLICATE' AND (p.title IS NULL OR TRIM(p.title) = '')",
    );
    assert_eq!(duplicados_sin_titulo, 0);
}

#[tokio::test]
async fn la_portada_nunca_se_reporta_como_huerfana() {
    let f = Fixture::new("huerfanas");
    sitio_conocido(&f);

    let job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    let outcome = engine::run(job, &f.store()).await.expect("rastrear");
    let conn = open(&outcome.store_path);

    let portada: i64 = count(
        &conn,
        "SELECT COUNT(*) FROM v_orphans WHERE url IN ('https://ejemplo.es/', 'https://ejemplo.es')",
    );
    assert_eq!(portada, 0, "la raíz es el punto de entrada, no una página huérfana");
}

#[tokio::test]
async fn un_rastreo_truncado_conserva_lo_descubierto_y_deja_la_cola_para_reanudar() {
    // Regresión doble. Con el rastreo truncado, las URLs descubiertas y no visitadas no se
    // escribían, así que `links` e `images` perdían en el `JOIN` todas las filas que apuntaban
    // a ellas: en un sitio real desaparecieron las 506 imágenes. Y sin filas `pending` tampoco
    // hay de dónde reanudar, que es como `03-MOTOR-CRAWL.md §7` define la reanudación.
    let f = Fixture::new("truncado");
    sitio_conocido(&f);

    let mut job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    // Concurrencia 1 para que el test sea determinista: con varias peticiones en vuelo, cuáles
    // se completan antes de saltar el límite depende del orden de finalización, y el corte podía
    // caer sobre dos páginas sin enlaces salientes.
    job.limits.concurrency_per_host = 1;
    // Las semillas se sirven en el orden de `discover_html()`, que va ordenado: about/,
    // blog/post-1, blog/post-2, index.html. Con cuatro se garantiza llegar a la portada, que es
    // la única con enlaces.
    job.limits.max_urls = Some(4);

    let outcome = engine::run(job, &f.store()).await.expect("rastrear");
    let conn = open(&outcome.store_path);

    assert!(outcome.truncated.is_some(), "debería marcarse como truncado");

    let truncado: i64 = count(&conn, "SELECT truncated FROM crawl_meta");
    assert_eq!(truncado, 1, "crawl_meta debe reflejarlo");

    assert!(
        count(&conn, "SELECT COUNT(*) FROM urls WHERE crawl_state = 'pending'") > 0,
        "la cola pendiente vive en urls: sin ella no se puede reanudar"
    );

    // Y lo ya descubierto no se pierde por no haberse llegado a visitar.
    assert!(
        count(&conn, "SELECT COUNT(*) FROM links") > 0,
        "los enlaces de las páginas visitadas deben conservarse"
    );
}

#[tokio::test]
async fn un_directorio_vacio_no_hace_caer_el_rastreo() {
    let f = Fixture::new("vacio");
    let job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    let outcome = engine::run(job, &f.store()).await.expect("un sitio vacío es válido");

    let conn = open(&outcome.store_path);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM pages"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM crawl_meta WHERE status = 'done'"), 1);
}

#[tokio::test]
async fn el_fichero_resultante_lo_puede_abrir_la_ui_en_solo_lectura() {
    // Es la decisión cerrada #2: la UI abre este mismo fichero y lanza sus consultas. Si las
    // vistas no existen o el esquema no está a la versión esperada, la app no arranca.
    let f = Fixture::new("lectura");
    sitio_conocido(&f);

    let job = CrawlJob::filesystem(&f.path, "https://ejemplo.es/");
    let outcome = engine::run(job, &f.store()).await.expect("rastrear");
    let conn = open(&outcome.store_path);

    let version: i64 = count(&conn, "SELECT MAX(version) FROM schema_version");
    assert_eq!(version, crawlforge_core::SCHEMA_VERSION);

    for vista in ["v_issue_summary", "v_indexable_pages", "v_broken_links", "v_orphans"] {
        conn.query_row(&format!("SELECT COUNT(*) FROM {vista}"), [], |r| r.get::<_, i64>(0))
            .unwrap_or_else(|e| panic!("la vista {vista} debe ser consultable: {e}"));
    }
}
