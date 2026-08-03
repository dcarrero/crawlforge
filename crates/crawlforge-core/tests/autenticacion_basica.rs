//! Autenticación básica HTTP de extremo a extremo: el caso del *staging* protegido.
//!
//! La revisión 2026-08-01 §1.6 retiró el `usuario:contraseña@` de las URLs porque la contraseña
//! acababa dentro del fichero de rastreo que se entrega al cliente — y de paso rompió la
//! auditoría de pre-producciones protegidas, que es trabajo normal de un consultor SEO. La
//! reposición vive en `CrawlLimits::http_basic_auth`, y estos tests fijan sus tres promesas:
//!
//! 1. **Un servidor que exige `Authorization` se rastrea entero**: páginas, `robots.txt` y
//!    sitemaps. Sin autenticar el robots, un staging protegido devuelve 401 ahí y el rastreo
//!    se comporta de forma rara antes de empezar.
//! 2. **La credencial no viaja a ningún otro host**, ni siquiera cuando `follow_external`
//!    hace que el rastreo pida dominios ajenos.
//! 3. **La contraseña no aparece en ninguna parte del fichero de rastreo.** Es el test que
//!    impide que la fuga original vuelva por otra puerta.

mod support;

use crawlforge_core::engine;
use crawlforge_core::job::{CrawlJob, HttpBasicAuth};
use rusqlite::Connection;
use support::servidor::{Respuesta, ServidorDePruebas};

/// La credencial de todos los tests y su cabecera exacta: `base64("consultor:S3creta")`.
/// El valor está precalculado a mano para que el test no dependa del base64 del propio motor.
const USUARIO: &str = "consultor";
const CONTRASENA: &str = "S3creta";
const CABECERA: &str = "Basic Y29uc3VsdG9yOlMzY3JldGE=";

/// Directorio temporal que se limpia solo. Mismo patrón que `pipeline.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("crawlforge-auth-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear el directorio temporal");
        Self { path }
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

fn estado(conn: &Connection, url: &str) -> Option<i64> {
    conn.query_row("SELECT status_code FROM urls WHERE url = ?1", [url], |r| r.get(0))
        .ok()
        .flatten()
}

/// Una página protegida con enlaces a las rutas que se le pasen.
fn pagina_protegida(titulo: &str, rutas: &[&str]) -> Respuesta {
    let enlaces: String = rutas.iter().map(|r| format!("<p><a href=\"{r}\">{r}</a></p>")).collect();
    Respuesta::pagina(titulo, &enlaces).exigiendo_autorizacion(CABECERA)
}

// ---------------------------------------------------------------- 1. Se rastrea con éxito

#[tokio::test]
async fn un_staging_protegido_se_rastrea_entero_incluidos_robots_y_sitemaps() {
    // Todo el sitio exige la credencial, como un pre-producción real detrás de Basic Auth:
    // también el robots.txt y el sitemap, que es donde un 401 hace el daño más silencioso.
    let servidor = ServidorDePruebas::arrancar_con_puerto(|puerto| {
        vec![
            ("/".to_string(), pagina_protegida("Portada", &["/pagina"])),
            ("/pagina".to_string(), pagina_protegida("Interior", &[])),
            (
                "/robots.txt".to_string(),
                Respuesta::texto("User-agent: *\nAllow: /").exigiendo_autorizacion(CABECERA),
            ),
            (
                "/sitemap.xml".to_string(),
                Respuesta::xml(format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                     <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\
                     <url><loc>http://127.0.0.1:{puerto}/declarada</loc></url></urlset>"
                ))
                .exigiendo_autorizacion(CABECERA),
            ),
            ("/declarada".to_string(), pagina_protegida("Declarada", &[])),
        ]
    })
    .await;

    let tmp = Temporal::new("staging");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.http_basic_auth = Some(HttpBasicAuth::new(USUARIO, CONTRASENA));

    let outcome =
        engine::run(job, &tmp.path.join("crawl.sqlite")).await.expect("rastrear el staging");
    let conn = abrir(&outcome.store_path);

    // Las páginas responden 200: la credencial llegó. Sin ella, todo esto sería 401.
    for ruta in ["/", "/pagina", "/declarada"] {
        assert_eq!(
            estado(&conn, &servidor.url(ruta)),
            Some(200),
            "{ruta} debería haberse rastreado autenticada"
        );
    }

    // El robots.txt y el sitemap también viajaron autenticados — y como el robots respondió
    // 200 con su Allow, el rastreo no quedó marcado como bloqueado.
    for ruta in ["/robots.txt", "/sitemap.xml"] {
        let recibidas = servidor.autorizaciones(ruta);
        assert!(!recibidas.is_empty(), "{ruta} tuvo que pedirse");
        assert!(
            recibidas.iter().all(|a| a.as_deref() == Some(CABECERA)),
            "todas las peticiones a {ruta} deben llevar la credencial: {recibidas:?}"
        );
    }

    // Y el sitemap se leyó de verdad: la URL que declara quedó cruzada en el fichero.
    let in_sitemap: i64 = conn
        .query_row(
            "SELECT in_sitemap FROM urls WHERE url = ?1",
            [servidor.url("/declarada")],
            |r| r.get(0),
        )
        .expect("la URL declarada existe");
    assert_eq!(in_sitemap, 1, "el sitemap protegido tuvo que poder leerse");
}

// ---------------------------------------------------------------- 2. No viaja a otro host

#[tokio::test]
async fn la_credencial_no_viaja_a_ningun_otro_host() {
    // El mismo servidor atiende como `127.0.0.1` (el staging, la semilla) y como `localhost`
    // (para el motor, un dominio externo). Con `follow_external` la URL ajena sí se rastrea:
    // exactamente el escenario en el que una credencial mal acotada se regalaría.
    let servidor = ServidorDePruebas::arrancar_como_otro_host_con_puerto(|puerto| {
        vec![
            (
                "/".to_string(),
                Respuesta::pagina(
                    "Portada",
                    &format!("<p><a href=\"http://localhost:{puerto}/externa\">fuera</a></p>"),
                )
                .exigiendo_autorizacion(CABECERA),
            ),
            ("/externa".to_string(), Respuesta::pagina("Ajena", "<p>Otra web.</p>")),
        ]
    })
    .await;
    let externa = servidor.url_como_otro_host("/externa");

    let tmp = Temporal::new("otro-host");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.http_basic_auth = Some(HttpBasicAuth::new(USUARIO, CONTRASENA));
    job.limits.follow_external = true;
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.path.join("crawl.sqlite")).await.expect("rastrear");
    let conn = abrir(&outcome.store_path);

    // La semilla se rastreó autenticada y la externa se pidió de verdad.
    assert_eq!(estado(&conn, &servidor.base()), Some(200));
    assert_eq!(estado(&conn, &externa), Some(200), "la externa tuvo que pedirse");

    // Y la petición al host ajeno viajó limpia: ni rastro de la credencial.
    let recibidas = servidor.autorizaciones("/externa");
    assert!(!recibidas.is_empty(), "/externa tuvo que pedirse");
    assert!(
        recibidas.iter().all(Option::is_none),
        "la credencial del staging no puede viajar a otro host: {recibidas:?}"
    );
}

// ---------------------------------------------------------------- 3. No queda en el fichero

#[tokio::test]
async fn la_contrasena_no_aparece_en_ninguna_parte_del_fichero_de_rastreo() {
    // Es el test que impide que la fuga de la revisión 2026-08-01 §1.6 vuelva por otra puerta.
    // Se busca el secreto en los **bytes crudos** del fichero terminado: eso cubre
    // `crawl_meta.config_json`, cada fila de `urls`, cualquier columna de texto y hasta las
    // páginas libres de SQLite — si la contraseña se escribió y se borró, también falla, y
    // ese fallo sería igual de real.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", pagina_protegida("Portada", &["/pagina"])),
        ("/pagina", pagina_protegida("Interior", &[])),
    ])
    .await;

    let tmp = Temporal::new("sin-fuga");
    let store = tmp.path.join("crawl.sqlite");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.http_basic_auth = Some(HttpBasicAuth::new(USUARIO, CONTRASENA));
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &store).await.expect("rastrear");
    let conn = abrir(&outcome.store_path);

    // Sanidad: el rastreo autenticó de verdad (si no, este test pasaría por casualidad
    // aunque nadie mandara la credencial a ninguna parte).
    assert_eq!(estado(&conn, &servidor.url("/pagina")), Some(200));

    // La puerta concreta por la que salió la fuga original, mirada con nombre y apellido.
    let config_json: String = conn
        .query_row("SELECT config_json FROM crawl_meta LIMIT 1", [], |r| r.get(0))
        .expect("leer config_json");
    assert!(
        !config_json.contains(CONTRASENA) && !config_json.contains(USUARIO),
        "config_json es parte del entregable y no puede llevar la credencial: {config_json}"
    );
    drop(conn);

    // Y el barrido total: ni la contraseña ni su base64 pueden estar en el fichero.
    let bytes = std::fs::read(&outcome.store_path).expect("leer el fichero de rastreo");
    for secreto in [CONTRASENA.as_bytes(), CABECERA.as_bytes()] {
        assert!(
            !bytes.windows(secreto.len()).any(|v| v == secreto),
            "el secreto no puede aparecer en ningún byte del fichero de rastreo"
        );
    }
}

// ---------------------------------------------------------------- 4. Reanudar la repone

#[tokio::test]
async fn una_reanudacion_recibe_la_credencial_que_el_fichero_no_guarda() {
    // La credencial no se serializa, así que un rastreo interrumpido de un staging protegido
    // no puede continuar solo con lo que hay en el fichero: quien reanuda la vuelve a dar,
    // igual que pasa con `ignore_robots`. `resume_with_auth` es esa vía.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", pagina_protegida("Portada", &["/pagina"])),
        ("/pagina", pagina_protegida("Interior", &[])),
    ])
    .await;

    let tmp = Temporal::new("reanudar");
    let store = tmp.path.join("crawl.sqlite");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.http_basic_auth = Some(HttpBasicAuth::new(USUARIO, CONTRASENA));
    job.discover_sitemaps = false;

    // Se interrumpe antes de despachar nada: la señal ya está emitida al arrancar, así que
    // las semillas quedan escritas como `pending` y el fichero como `paused`.
    let (tx, rx) = tokio::sync::watch::channel(false);
    tx.send(true).expect("emitir la cancelación");
    let cortado = engine::run_cancellable(job, &store, None, Some(rx))
        .await
        .expect("el corte no es un error");
    assert!(cortado.interrupted, "el rastreo tuvo que quedar interrumpido");

    // La reanudación repone la credencial de su sesión y termina el trabajo autenticada.
    let outcome = engine::resume_with_auth(
        &store,
        None,
        None,
        Some(HttpBasicAuth::new(USUARIO, CONTRASENA)),
    )
    .await
    .expect("reanudar con credencial");
    let conn = abrir(&outcome.store_path);
    for ruta in ["/", "/pagina"] {
        assert_eq!(
            estado(&conn, &servidor.url(ruta)),
            Some(200),
            "{ruta} debería haberse rastreado autenticada al reanudar"
        );
    }
}
