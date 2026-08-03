//! La tabla `resources` se puebla: una fila por URL de recurso, con su `kind`, estado y peso.
//!
//! Existía desde la migración 001 y el escritor nunca había insertado una fila — una tabla
//! que existe y miente, la clase de deuda que conviene no dejar crecer. Estos tests fijan las
//! dos propiedades que la hacen útil y barata:
//!
//! - **El dato es cierto**: cada recurso pedido deja su fila, con el `kind` deducido del
//!   `content_type` de la respuesta y la extensión como respaldo (fuentes servidas como
//!   `application/octet-stream`, el 404 de un CSS que llega como `text/html`).
//! - **El coste es una petición por recurso único**, no una por par (página, recurso): la
//!   cola deduplica por hash de URL, así que un sitio cuyas páginas comparten plantilla paga
//!   los recursos una vez. Es la diferencia entre un coste despreciable y uno inaceptable.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::job::CrawlJob;
use rusqlite::Connection;
use support::servidor::{Respuesta, ServidorDePruebas};

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-recursos-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear el directorio temporal");
        Self { path }
    }

    fn store(&self) -> std::path::PathBuf {
        self.path.join("crawl.sqlite")
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn abrir(store: &std::path::Path) -> Connection {
    Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el fichero de rastreo")
}

/// Una respuesta 200 con el `Content-Type` exacto que se quiera simular.
fn con_tipo(body: &str, content_type: &str) -> Respuesta {
    let mut r = Respuesta::texto(body);
    r.headers[0] = ("Content-Type".to_string(), content_type.to_string());
    r
}

/// La fila de `resources` de una URL, por su ruta: (kind, status_code, size_bytes, mime).
type FilaRecurso = (String, Option<i64>, Option<i64>, Option<String>);

fn fila_de_recurso(conn: &Connection, ruta: &str) -> Option<FilaRecurso> {
    conn.query_row(
        "SELECT r.kind, r.status_code, r.size_bytes, r.mime
         FROM resources r JOIN urls u ON u.id = r.url_id
         WHERE u.path = ?1",
        [ruta],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .ok()
}

#[tokio::test]
async fn un_rastreo_puebla_resources_con_una_fila_por_recurso() {
    let servidor = ServidorDePruebas::arrancar(&[
        (
            "/",
            Respuesta::html(
                r#"<!DOCTYPE html><html><head><title>Inicio</title>
                   <link rel="stylesheet" href="/assets/estilo.css">
                   <script src="/assets/app.js"></script>
                   </head><body><main><h1>Inicio</h1>
                   <img src="/assets/foto.png" alt="una foto">
                   <a href="/assets/fuente.woff2">la fuente</a>
                   </main></body></html>"#,
            ),
        ),
        ("/assets/estilo.css", con_tipo("body{margin:0}", "text/css")),
        ("/assets/app.js", con_tipo("console.log(1)", "application/javascript")),
        ("/assets/foto.png", con_tipo("no-es-un-png-de-verdad", "image/png")),
        // El caso que motiva el respaldo por extensión: una fuente servida como
        // `application/octet-stream`, que es lo que hacen muchos servidores.
        ("/assets/fuente.woff2", con_tipo("woff2woff2", "application/octet-stream")),
    ])
    .await;

    let tmp = Temporal::new("puebla");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert!(outcome.metrics.urls_fetched >= 5, "la página y sus cuatro recursos");

    let conn = abrir(&tmp.store());

    let (kind, status, size, mime) =
        fila_de_recurso(&conn, "/assets/estilo.css").expect("el CSS deja fila en resources");
    assert_eq!(kind, "css");
    assert_eq!(status, Some(200));
    assert_eq!(size, Some("body{margin:0}".len() as i64));
    assert_eq!(mime.as_deref(), Some("text/css"));

    let (kind, ..) = fila_de_recurso(&conn, "/assets/app.js").expect("el JS deja fila");
    assert_eq!(kind, "js");

    let (kind, ..) = fila_de_recurso(&conn, "/assets/foto.png").expect("la imagen deja fila");
    assert_eq!(kind, "img");

    // El content_type no dice «fuente», la extensión sí: el respaldo decide.
    let (kind, ..) = fila_de_recurso(&conn, "/assets/fuente.woff2").expect("la fuente deja fila");
    assert_eq!(kind, "font");

    // Y las páginas HTML no son recursos: ni la portada ni el 404 del robots.txt.
    let recursos: i64 =
        conn.query_row("SELECT COUNT(*) FROM resources", [], |r| r.get(0)).expect("contar");
    assert_eq!(recursos, 4, "una fila por recurso, ninguna por página");
}

#[tokio::test]
async fn los_recursos_compartidos_por_toda_la_plantilla_se_pagan_una_vez() {
    // La propiedad que hace asumible pedir CSS y JS: un sitio de 20.000 páginas que carga
    // los mismos 15 CSS y 30 JS paga 45 peticiones en total, no 45 por página. Aquí, en
    // miniatura: 12 páginas que comparten dos recursos son **una** petición por recurso.
    let paginas = 12;
    let mut rutas: Vec<(String, Respuesta)> = (0..paginas)
        .map(|i| {
            let cuerpo = format!(
                r#"<!DOCTYPE html><html><head><title>P{i}</title>
                   <link rel="stylesheet" href="/tema.css">
                   <script src="/tema.js"></script>
                   </head><body><main><h1>P{i}</h1>{enlaces}</main></body></html>"#,
                enlaces = (0..paginas)
                    .map(|j| format!("<a href=\"/p{j}\">p{j}</a>"))
                    .collect::<String>(),
            );
            (format!("/p{i}"), Respuesta::html(cuerpo))
        })
        .collect();
    rutas.push(("/tema.css".to_string(), con_tipo("body{}", "text/css")));
    rutas.push(("/tema.js".to_string(), con_tipo("1;", "text/javascript")));
    let rutas_ref: Vec<(&str, Respuesta)> =
        rutas.iter().map(|(r, resp)| (r.as_str(), resp.clone())).collect();
    let servidor = ServidorDePruebas::arrancar(&rutas_ref).await;

    let tmp = Temporal::new("dedup");
    let mut job = CrawlJob::http(servidor.url("/p0"));
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(
        outcome.metrics.urls_fetched,
        paginas as u64 + 2,
        "las 12 páginas y los 2 recursos, nada más"
    );

    assert_eq!(servidor.peticiones("/tema.css"), 1, "12 páginas lo cargan; se pide una vez");
    assert_eq!(servidor.peticiones("/tema.js"), 1);

    // Y en la tabla, una fila por recurso: URL de recurso, no par (página, recurso).
    let conn = abrir(&tmp.store());
    let recursos: i64 =
        conn.query_row("SELECT COUNT(*) FROM resources", [], |r| r.get(0)).expect("contar");
    assert_eq!(recursos, 2);
}

#[tokio::test]
async fn un_recurso_roto_queda_en_resources_con_su_estado() {
    // El 404 de una hoja de estilo llega como `text/html` (la página de error del servidor):
    // si mandara el content_type, el recurso roto desaparecería de la tabla justo cuando más
    // falta hace. La extensión es el respaldo que lo retiene.
    let servidor = ServidorDePruebas::arrancar(&[(
        "/",
        Respuesta::html(
            r#"<!DOCTYPE html><html><head><title>Inicio</title>
               <link rel="stylesheet" href="/no-existe.css">
               </head><body><main><h1>Inicio</h1></main></body></html>"#,
        ),
    )])
    .await;

    let tmp = Temporal::new("roto");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;

    crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");

    let conn = abrir(&tmp.store());
    let (kind, status, ..) =
        fila_de_recurso(&conn, "/no-existe.css").expect("el CSS roto deja fila");
    assert_eq!(kind, "css");
    assert_eq!(status, Some(404), "con el estado que delata que está roto");
}
