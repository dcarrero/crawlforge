//! La comprobación de estado de las URLs externas (`CrawlLimits::check_external`).
//!
//! El defecto que cierra este fichero: `HTTP-404-EXTERNAL` estaba escrita, con fixture y test,
//! y en un rastreo real no saltaba jamás — las externas quedaban registradas **sin estado**, y
//! sin estado no hay 404 que reportar. El comentario de `CrawlLimits` («por defecto las
//! externas solo se comprueban») prometía una comprobación que no existía.
//!
//! Qué es la comprobación, y qué no es:
//! - **Solo estado.** `HEAD` con `GET` de respaldo ante 405/501; no se parsea, no se extraen
//!   enlaces, no se crea fila en `pages`. `follow_external` (rastrear el sitio ajeno entero)
//!   es otra cosa y se queda como estaba.
//! - **Una vez por URL única** — la cola deduplica por hash —, sin contar contra `max_urls`,
//!   con una sola petición en vuelo por host ajeno y sin pedir su `robots.txt`.
//! - **El tope `max_external` no trunca el rastreo**: deja externas sin comprobar y lo dice
//!   (`externals_unchecked`), pero no toca `crawl_meta.truncated` — ese campo apaga las
//!   reglas de `REQUIERE_GRAFO_COMPLETO`, y apagarlas por enlaces ajenos sería un silencio.
//!
//! Dos hosts sin salir de la máquina: `127.0.0.1` es el sitio auditado y `localhost` el
//! ajeno, como enseña `crawl_delay.rs`.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::job::CrawlJob;
use rusqlite::Connection;
use std::time::Duration;
use support::servidor::{Respuesta, ServidorDePruebas};

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-externas-{}-{nombre}", std::process::id()));
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

/// El `status_code` (posiblemente NULL) de la fila de una URL externa, por su ruta.
fn estado_externa(conn: &Connection, ruta: &str) -> Option<Option<i64>> {
    conn.query_row(
        "SELECT status_code FROM urls WHERE is_internal = 0 AND path = ?1",
        [ruta],
        |row| row.get(0),
    )
    .ok()
}

/// Una página del sitio auditado cuyo cuerpo enlaza lo que se le pase.
fn pagina_con(cuerpo: &str) -> Respuesta {
    Respuesta::pagina("Inicio", cuerpo)
}

#[tokio::test]
async fn un_enlace_externo_roto_recibe_su_estado_y_dispara_la_regla() {
    // El ajeno: una ruta que existe y una que no. La que no responde 404, como cualquier
    // servidor real.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[(
        "/guia",
        Respuesta::pagina("Guía", "<p>sigo aquí</p>"),
    )])
    .await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        pagina_con(&format!(
            "<a href=\"{viva}\">la guía</a> <a href=\"{rota}\">se mudó</a>",
            viva = ajeno.url_como_otro_host("/guia"),
            rota = ajeno.url_como_otro_host("/se-mudo"),
        )),
    )])
    .await;

    let tmp = Temporal::new("rota");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_checked, 2, "las dos externas se comprobaron");
    assert_eq!(outcome.metrics.externals_unchecked, 0);

    let conn = abrir(&tmp.store());
    assert_eq!(
        estado_externa(&conn, "/guia"),
        Some(Some(200)),
        "la externa viva queda con su 200"
    );
    assert_eq!(
        estado_externa(&conn, "/se-mudo"),
        Some(Some(404)),
        "la externa rota queda con su 404: sin esto no hay nada que reportar"
    );

    // Y con el estado en la fila, HTTP-404-EXTERNAL por fin puede dispararse en un rastreo.
    let hallazgos: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM issues WHERE rule_id = 'HTTP-404-EXTERNAL'",
            [],
            |r| r.get(0),
        )
        .expect("contar hallazgos");
    assert_eq!(hallazgos, 1, "un enlace externo roto, un hallazgo");

    // Solo estado: la sonda fue HEAD y el sitio ajeno no aporta páginas al fichero.
    assert_eq!(ajeno.metodos("/se-mudo"), vec!["HEAD"]);
    let paginas_ajenas: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pages p JOIN urls u ON u.id = p.url_id
             WHERE u.is_internal = 0",
            [],
            |r| r.get(0),
        )
        .expect("contar páginas externas");
    assert_eq!(paginas_ajenas, 0, "no se parsea nada del sitio ajeno");
}

#[tokio::test]
async fn muchos_enlaces_a_la_misma_externa_son_una_peticion() {
    // Mil anclas al mismo destino repartidas en dos páginas: la cola deduplica por hash de
    // URL, así que al host ajeno le llega **una** petición.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[(
        "/unica",
        Respuesta::pagina("Única", "<p>hola</p>"),
    )])
    .await;
    let destino = ajeno.url_como_otro_host("/unica");
    let anclas: String =
        (0..500).map(|i| format!("<a href=\"{destino}\">enlace {i}</a> ")).collect();
    let propio = ServidorDePruebas::arrancar(&[
        ("/", pagina_con(&format!("{anclas}<a href=\"/otra\">otra</a>"))),
        ("/otra", pagina_con(&anclas)),
    ])
    .await;

    let tmp = Temporal::new("dedup");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_checked, 1, "mil enlaces, una comprobación");
    assert_eq!(ajeno.peticiones("/unica"), 1, "y una sola petición al servidor ajeno");
}

#[tokio::test]
async fn las_externas_no_cuentan_contra_max_urls() {
    // Dos páginas propias y cuatro externas, con presupuesto de sobra para las páginas: si
    // las externas consumieran presupuesto, `urls_fetched` las incluiría y el rastreo se
    // marcaría truncado. El tope de URLs es del sitio del usuario, no de sus enlaces.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[
        ("/a", Respuesta::pagina("A", "<p>a</p>")),
        ("/b", Respuesta::pagina("B", "<p>b</p>")),
        ("/c", Respuesta::pagina("C", "<p>c</p>")),
        ("/d", Respuesta::pagina("D", "<p>d</p>")),
    ])
    .await;
    let enlaces = |rutas: &[&str]| -> String {
        rutas
            .iter()
            .map(|r| format!("<a href=\"{}\">{r}</a> ", ajeno.url_como_otro_host(r)))
            .collect()
    };
    let propio = ServidorDePruebas::arrancar(&[
        ("/", pagina_con(&format!("{}<a href=\"/dos\">dos</a>", enlaces(&["/a", "/b"])))),
        ("/dos", pagina_con(&enlaces(&["/c", "/d"]))),
    ])
    .await;

    let tmp = Temporal::new("presupuesto");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;
    // Presupuesto 3 para un sitio de 2 páginas: si las externas contaran, la tercera
    // comprobación agotaría el presupuesto, el rastreo se marcaría truncado y las externas
    // restantes se cancelarían con el corte.
    job.limits.max_urls = Some(3);

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 2, "el presupuesto lo gastan las páginas propias");
    assert_eq!(
        outcome.metrics.externals_checked, 4,
        "las cuatro externas se comprueban sin gastar presupuesto"
    );
    assert!(outcome.truncated.is_none(), "comprobar externas no puede truncar el rastreo");
}

#[tokio::test]
async fn el_tope_de_externas_avisa_pero_no_trunca_el_rastreo() {
    // La trampa que este test fija: `crawl_meta.truncated` significa «el rastreo del sitio
    // del usuario está incompleto» y el motor lo usa para apagar las reglas de
    // `REQUIERE_GRAFO_COMPLETO`. Alcanzar `max_external` no puede encenderlo: apagaría en
    // silencio las reglas que más diferencian al producto por culpa de enlaces ajenos.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[
        ("/a", Respuesta::pagina("A", "<p>a</p>")),
        ("/b", Respuesta::pagina("B", "<p>b</p>")),
        ("/c", Respuesta::pagina("C", "<p>c</p>")),
        ("/d", Respuesta::pagina("D", "<p>d</p>")),
    ])
    .await;
    let enlaces: String = ["/a", "/b", "/c", "/d"]
        .iter()
        .map(|r| format!("<a href=\"{}\">{r}</a> ", ajeno.url_como_otro_host(r)))
        .collect();
    let propio = ServidorDePruebas::arrancar(&[("/", pagina_con(&enlaces))]).await;

    let tmp = Temporal::new("tope");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;
    job.limits.max_external = 2;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_checked, 2, "el tope corta en dos");
    assert_eq!(
        outcome.metrics.externals_unchecked, 2,
        "y las que quedaron fuera se cuentan: un tope que trunca en silencio no vale"
    );
    assert!(
        outcome.truncated.is_none(),
        "alcanzar max_external no es un truncado del rastreo"
    );

    let conn = abrir(&tmp.store());
    let truncated: i64 = conn
        .query_row("SELECT truncated FROM crawl_meta", [], |r| r.get(0))
        .expect("leer crawl_meta");
    assert_eq!(
        truncated, 0,
        "crawl_meta.truncated apagaría las reglas de REQUIERE_GRAFO_COMPLETO"
    );
    let comprobadas: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE is_internal = 0 AND status_code IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .expect("contar comprobadas");
    assert_eq!(comprobadas, 2);
}

#[tokio::test]
async fn una_sola_peticion_en_vuelo_por_host_externo() {
    // Tres externas del mismo host, cada una tardando 400 ms: con `force_serial` los
    // arranques se suceden, no se solapan. Como en `crawl_delay.rs`, se afirma orden y cotas
    // holgadas, nunca duraciones exactas.
    let retardo = Duration::from_millis(400);
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[
        ("/uno", Respuesta::pagina("1", "<p>1</p>").con_retardo(retardo)),
        ("/dos", Respuesta::pagina("2", "<p>2</p>").con_retardo(retardo)),
        ("/tres", Respuesta::pagina("3", "<p>3</p>").con_retardo(retardo)),
    ])
    .await;
    let enlaces: String = ["/uno", "/dos", "/tres"]
        .iter()
        .map(|r| format!("<a href=\"{}\">{r}</a> ", ajeno.url_como_otro_host(r)))
        .collect();
    let propio = ServidorDePruebas::arrancar(&[("/", pagina_con(&enlaces))]).await;

    let tmp = Temporal::new("serial");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_checked, 3);

    let mut llegadas: Vec<std::time::Instant> = ["/uno", "/dos", "/tres"]
        .iter()
        .flat_map(|r| {
            let l = ajeno.llegadas(r);
            assert_eq!(l.len(), 1, "cada externa se pide una vez");
            l
        })
        .collect();
    llegadas.sort();
    for par in llegadas.windows(2) {
        let hueco = par[1].duration_since(par[0]);
        assert!(
            hueco >= Duration::from_millis(300),
            "dos sondas al mismo host ajeno a {hueco:?} la una de la otra: iban en paralelo"
        );
    }
}

#[tokio::test]
async fn si_el_servidor_rechaza_head_se_reintenta_con_get() {
    // Hay muchos servidores que responden 405 (o 501) a HEAD. Sin el respaldo, sus enlaces
    // saldrían como rotos sin estarlo — un falso positivo en la regla de severidad media.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[(
        "/sin-head",
        Respuesta::pagina("Sin HEAD", "<p>pero con GET</p>").rechazando_head(),
    )])
    .await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        pagina_con(&format!(
            "<a href=\"{}\">apunta a un servidor sin HEAD</a>",
            ajeno.url_como_otro_host("/sin-head")
        )),
    )])
    .await;

    let tmp = Temporal::new("head-get");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;

    crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");

    assert_eq!(
        ajeno.metodos("/sin-head"),
        vec!["HEAD", "GET"],
        "primero HEAD; ante el 405, un único GET"
    );
    let conn = abrir(&tmp.store());
    assert_eq!(
        estado_externa(&conn, "/sin-head"),
        Some(Some(200)),
        "el estado que queda es el del GET, no el 405 del rechazo"
    );
}

#[tokio::test]
async fn no_se_pide_el_robots_txt_de_los_hosts_externos() {
    // Comprobar que un enlace resuelve es lo que hace el navegador cuando el visitante lo
    // pulsa; no se indexa ni se sigue nada del sitio ajeno. Pedir además su robots.txt casi
    // duplicaría las peticiones a terceros para poder decir menos.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[(
        "/pagina",
        Respuesta::pagina("Ajena", "<p>x</p>"),
    )])
    .await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        pagina_con(&format!(
            "<a href=\"{}\">ajena</a>",
            ajeno.url_como_otro_host("/pagina")
        )),
    )])
    .await;

    let tmp = Temporal::new("robots");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;

    crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");

    assert_eq!(ajeno.peticiones("/pagina"), 1, "la sonda sí llegó");
    assert_eq!(
        ajeno.peticiones("/robots.txt"),
        0,
        "al host ajeno no se le pide el robots.txt"
    );
    assert!(propio.peticiones("/robots.txt") >= 1, "el del sitio auditado, como siempre");
}

#[tokio::test]
async fn sin_check_external_las_externas_quedan_registradas_sin_estado() {
    // `--no-external-check`: el comportamiento anterior, elegido a propósito. La fila existe
    // —el informe necesita saber a dónde apunta el sitio— pero al host ajeno no le llega nada.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[(
        "/fuera",
        Respuesta::pagina("Fuera", "<p>x</p>"),
    )])
    .await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        pagina_con(&format!(
            "<a href=\"{}\">fuera</a>",
            ajeno.url_como_otro_host("/fuera")
        )),
    )])
    .await;

    let tmp = Temporal::new("apagado");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;
    job.limits.check_external = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_checked, 0);
    assert_eq!(outcome.metrics.externals_unchecked, 0, "apagado no es «por encima del tope»");
    assert_eq!(ajeno.peticiones("/fuera"), 0, "ni una petición al host ajeno");

    let conn = abrir(&tmp.store());
    assert_eq!(
        estado_externa(&conn, "/fuera"),
        Some(None),
        "la externa queda registrada, sin estado"
    );
}

#[tokio::test]
async fn un_recurso_externo_comprobado_deja_su_fila_en_resources() {
    // Los recursos siguen la misma política interna/externa que los enlaces: un CSS en un
    // CDN se comprueba con la sonda y su fila de `resources` queda con el estado y el mime
    // de las cabeceras — sin descargar el fichero.
    let cdn = ServidorDePruebas::arrancar_como_otro_host(&[("/lib.css", {
        let mut r = Respuesta::texto("body{}");
        r.headers[0] = ("Content-Type".to_string(), "text/css".to_string());
        r
    })])
    .await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        Respuesta::html(format!(
            r#"<!DOCTYPE html><html><head><title>Inicio</title>
               <link rel="stylesheet" href="{}">
               </head><body><main><h1>Inicio</h1></main></body></html>"#,
            cdn.url_como_otro_host("/lib.css")
        )),
    )])
    .await;

    let tmp = Temporal::new("cdn");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;

    crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");

    assert_eq!(cdn.metodos("/lib.css"), vec!["HEAD"], "solo cabeceras: nada se descarga");
    let conn = abrir(&tmp.store());
    let (kind, status, mime): (String, i64, String) = conn
        .query_row(
            "SELECT r.kind, r.status_code, r.mime
             FROM resources r JOIN urls u ON u.id = r.url_id
             WHERE u.is_internal = 0 AND u.path = '/lib.css'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("la fila del recurso del CDN");
    assert_eq!(kind, "css");
    assert_eq!(status, 200);
    assert_eq!(mime, "text/css");
}
