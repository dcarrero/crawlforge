//! El perímetro del fichero de rastreo (revisión 2026-08-01, tanda 3).
//!
//! El motor aguanta; lo que no estaba cerrado eran los caminos de **cierre y coexistencia**
//! del fichero: qué pasa cuando alguien más lo lee mientras se cierra (§3.1), qué impide que
//! dos procesos escriban el mismo fichero (§3.3) y si el primer Ctrl-C responde durante la
//! pasada final (§3.6). Los tres ocurrieron de verdad el 2026-08-02, encadenados.
//!
//! Confirmación del rojo: cada test de este fichero falla si se revierte su arreglo —la
//! tolerancia al lector en `store::finalize`, el `StoreLock` de `engine::dispatch` y la
//! consulta de la señal entre reglas de conjunto en `engine::finalize`, respectivamente.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::engine;
use crawlforge_core::job::CrawlJob;
use crawlforge_core::store::StoreLock;
use crawlforge_core::CoreError;
use rusqlite::Connection;
use std::time::Duration;
use support::servidor::{Respuesta, ServidorDePruebas};

/// Directorio temporal que se limpia solo. Mismo patrón que `reanudacion.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("crawlforge-per-{}-{nombre}", std::process::id()));
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

fn status_de(path: &std::path::Path) -> String {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("abrir el fichero de rastreo");
    conn.query_row("SELECT status FROM crawl_meta", [], |r| r.get(0)).expect("leer status")
}

fn hallazgos_por_regla(path: &std::path::Path) -> Vec<(String, i64)> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("abrir el fichero de rastreo");
    let mut stmt = conn
        .prepare("SELECT rule_id, COUNT(*) FROM issues GROUP BY rule_id ORDER BY rule_id")
        .expect("preparar");
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).expect("consultar");
    rows.collect::<rusqlite::Result<Vec<_>>>().expect("recoger")
}

/// Un sitio pequeño con hallazgos previsibles: páginas sin `h1` y con imagen rota, para que
/// la pasada final tenga reglas de conjunto con trabajo real que hacer.
fn sitio_pequeno() -> Vec<(String, Respuesta)> {
    let mut rutas = Vec::new();
    let enlaces: String = (1..=6).map(|i| format!("<p><a href=\"/p{i}\">P{i}</a></p>")).collect();
    rutas.push(("/".to_string(), Respuesta::pagina("Inicio", &enlaces)));
    for i in 1..=6 {
        rutas.push((
            format!("/p{i}"),
            Respuesta::html(format!(
                "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\">\
                 <title>P{i}</title>\
                 <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
                 </head><body><main><p>Página P{i}, sin encabezado.</p>\
                 <img src=\"/no-existe.png\">\
                 <p><a href=\"/\">Inicio</a></p></main></body></html>"
            )),
        ));
    }
    rutas
}

// ─────────────────────────────────────────────────────────────── §3.1: el lector concurrente

#[tokio::test]
async fn un_lector_concurrente_no_hace_fallar_el_cierre_del_rastreo() {
    // El caso que con una interfaz será el normal: la UI (aquí, un visor cualquiera) tiene el
    // `.sqlite` abierto mientras el rastreo termina. Salir de WAL exige ser la única
    // conexión, así que el cierre no puede ser portable — pero el rastreo TIENE que terminar
    // bien: antes fallaba con «database is locked» después de marcar `done`, y el usuario
    // creía haber perdido un rastreo que estaba entero.
    //
    // Rojo sin el arreglo: `store::finalize` propagaba el error y `engine::run` devolvía Err.
    let tmp = Temporal::new("lector");

    // El sitio, servido desde el disco: este test no necesita red.
    let dist = tmp.path.join("dist");
    std::fs::create_dir_all(&dist).expect("crear dist");
    std::fs::write(
        dist.join("index.html"),
        "<!DOCTYPE html><html lang=\"es\"><head><title>Inicio</title></head>\
         <body><h1>Inicio</h1><p><a href=\"/otra.html\">Otra</a></p></body></html>",
    )
    .expect("escribir index");
    std::fs::write(
        dist.join("otra.html"),
        "<!DOCTYPE html><html lang=\"es\"><head><title>Otra</title></head>\
         <body><h1>Otra</h1><p><a href=\"/index.html\">Inicio</a></p></body></html>",
    )
    .expect("escribir otra");

    // El fichero existe de antemano (como cuando se re-rastrea) y un visor lo tiene abierto
    // y ya ha leído: con eso retiene el cerrojo del `-shm` durante todo el rastreo.
    let store = tmp.path.join("crawl.sqlite");
    {
        crawlforge_core::store::open_writer(&store).expect("crear el fichero");
    }
    let visor = Connection::open(&store).expect("el visor abre el fichero");
    let _: i64 = visor
        .query_row("SELECT COUNT(*) FROM crawl_meta", [], |r| r.get(0))
        .expect("el visor lee");

    let job = CrawlJob::filesystem(&dist, "https://ejemplo.es/");
    let outcome = engine::run(job, &store)
        .await
        .expect("con un lector concurrente el rastreo termina bien, no con «database is locked»");

    assert!(!outcome.interrupted);
    assert!(
        outcome.wal_kept,
        "el cierre declara la degradación: el fichero se queda en WAL por el lector"
    );
    assert_eq!(status_de(&store), "done", "el rastreo está terminado y entero");

    // El visor sigue pudiendo leer el resultado completo, y el WAL sigue al lado: es la
    // parte del rastreo que un aviso tiene que decir que viaja con el fichero.
    let urls: i64 =
        visor.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0)).expect("el visor relee");
    assert!(urls >= 2, "las páginas rastreadas están ahí: {urls}");
}

// ───────────────────────────────────────────────── §3.3: dos escritores sobre el mismo fichero

#[tokio::test]
async fn un_segundo_motor_sobre_el_mismo_fichero_es_rechazado() {
    // Dos rastreos del mismo sitio acaban en el mismo nombre de fichero (es determinista, y
    // con 100 blogs en cron pasará). Dos escritores duplican `links` —no tiene UNIQUE— y el
    // `finalize` del que pierde falla con BUSY. El segundo tiene que ser rechazado al
    // instante, antes de tocar nada.
    //
    // Rojo sin el arreglo: sin el `StoreLock` de `dispatch`, el segundo motor escribía su
    // propio `crawl_meta` en el fichero vivo del primero y terminaba «bien».
    let tmp = Temporal::new("dos-motores");
    let store = tmp.path.join("crawl.sqlite");

    let servidor = ServidorDePruebas::arrancar(&[
        (
            "/",
            Respuesta::pagina("Inicio", "<p><a href=\"/lenta\">Lenta</a></p>"),
        ),
        (
            "/lenta",
            Respuesta::pagina("Lenta", "<p>Tarda.</p>").con_retardo(Duration::from_millis(2_000)),
        ),
    ])
    .await;

    let mut job_a = CrawlJob::http(servidor.base());
    job_a.discover_sitemaps = false;
    let (tx, rx) = tokio::sync::watch::channel(false);
    let motor_a = tokio::spawn({
        let store = store.clone();
        async move { engine::run_cancellable(job_a, &store, None, Some(rx)).await }
    });

    // El primero está de verdad rastreando: ya pidió la portada, así que su cerrojo está
    // tomado desde antes de la primera petición.
    while servidor.peticiones("/") == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let mut job_b = CrawlJob::http(servidor.base());
    job_b.discover_sitemaps = false;
    let err = engine::run(job_b, &store)
        .await
        .expect_err("el segundo motor tiene que ser rechazado, no coexistir");
    assert!(
        matches!(err, CoreError::StoreLocked { .. }),
        "el error dice que hay otro escritor: {err:?}"
    );

    // El primero no se ha enterado de nada: se corta limpio y su fichero queda coherente.
    tx.send(true).expect("el motor A sigue escuchando");
    let outcome = motor_a.await.expect("join").expect("el corte es un cierre limpio");
    assert!(outcome.interrupted);
    let metas: i64 = {
        let conn = Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("abrir");
        conn.query_row("SELECT COUNT(*) FROM crawl_meta", [], |r| r.get(0)).expect("contar")
    };
    assert_eq!(metas, 1, "el fichero es de un solo rastreo: el segundo no llegó a escribir");
}

#[tokio::test]
async fn reanudar_distingue_un_rastreo_vivo_de_uno_muerto() {
    // `status = 'running'` significa dos cosas opuestas: «se mató el proceso» y «hay otro
    // rastreo escribiendo ahora mismo». El cerrojo del sistema las separa sin umbrales de
    // tiempo: lo suelta el sistema en el instante en que el proceso muere.
    //
    // Rojo sin el arreglo: `resume` aceptaba el `running` vivo y se convertía en el segundo
    // escritor.
    let tmp = Temporal::new("vivo-o-muerto");
    let store = tmp.path.join("crawl.sqlite");

    let servidor = ServidorDePruebas::arrancar(&[
        (
            "/",
            Respuesta::pagina(
                "Inicio",
                "<p><a href=\"/p1\">P1</a></p><p><a href=\"/lenta\">Lenta</a></p>",
            ),
        ),
        ("/p1", Respuesta::pagina("P1", "<p>Contenido.</p>")),
        (
            "/lenta",
            Respuesta::pagina("Lenta", "<p>Tarda.</p>").con_retardo(Duration::from_millis(700)),
        ),
    ])
    .await;

    // Un rastreo interrumpido de verdad, con pendientes escritas…
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;
    let (tx, rx) = tokio::sync::watch::channel(false);
    let rastreo = engine::run_cancellable(job, &store, None, Some(rx));
    let disparo = async {
        while servidor.peticiones("/lenta") == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tx.send(true).expect("el motor sigue escuchando");
    };
    let (outcome, ()) = tokio::join!(rastreo, disparo);
    assert!(outcome.expect("cierre limpio").interrupted);

    // …que aparenta un `kill -9`: el estado se queda en `running`.
    {
        let conn = Connection::open(&store).expect("abrir para simular el kill");
        conn.execute("UPDATE crawl_meta SET status = 'running'", []).expect("marcar running");
    }

    // Vivo: otro proceso (aquí, este test) tiene la exclusiva. Reanudar sería el segundo
    // escritor y se rechaza.
    let exclusiva = StoreLock::acquire(&store).expect("tomar la exclusiva, como el proceso vivo");
    let err = engine::resume(&store)
        .await
        .expect_err("un running con la exclusiva tomada está vivo: no se reanuda");
    assert!(
        matches!(err, CoreError::StoreLocked { .. }),
        "el error dice que hay otro escritor: {err:?}"
    );

    // Muerto: la exclusiva se soltó (el sistema lo hace solo al morir el proceso) y el mismo
    // fichero se reanuda sin esperas ni heurísticas.
    drop(exclusiva);
    let reanudado = engine::resume(&store).await.expect("un running muerto sí se reanuda");
    assert!(!reanudado.interrupted);
    assert_eq!(status_de(&store), "done");
}

// ─────────────────────────────────────────────── §3.6: el primer Ctrl-C durante la pasada final

#[tokio::test]
async fn un_corte_durante_la_pasada_final_deja_el_fichero_reanudable() {
    // La pasada final puede durar más que el propio rastreo y era sorda al primer Ctrl-C: el
    // motor solo consultaba la señal en la cabecera del bucle de rastreo. Hacía falta el
    // segundo, el que mata el proceso — y así nació el `-wal` de 1 GB del 2026-08-02.
    //
    // El corte se dispara desde el observador de progreso, en el paso de la primera regla de
    // conjunto: determinista, porque el emisor anuncia cada regla antes de evaluarla y la
    // señal se consulta entre una y la siguiente.
    //
    // Rojo sin el arreglo: la señal se ignoraba, el rastreo terminaba `done` y
    // `outcome.interrupted` era false.
    let tmp = Temporal::new("corte-final");

    // El control: el mismo sitio rastreado del tirón, para comparar al final.
    let servidor_control = ServidorDePruebas::arrancar_con_puerto(|_| sitio_pequeno()).await;
    let store_control = tmp.path.join("control.sqlite");
    let mut job = CrawlJob::http(servidor_control.base());
    job.discover_sitemaps = false;
    let control = engine::run(job, &store_control).await.expect("rastreo de control");
    assert!(!control.interrupted);

    // El interrumpido: la señal se emite al anunciarse la primera regla de conjunto.
    let servidor = ServidorDePruebas::arrancar_con_puerto(|_| sitio_pequeno()).await;
    let store = tmp.path.join("interrumpido.sqlite");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;

    let (tx, rx) = tokio::sync::watch::channel(false);
    let callback: engine::ProgressCallback = std::sync::Arc::new(move |p: &engine::CrawlProgress| {
        if p.phase == engine::CrawlPhase::Finalize
            && p.step.as_ref().is_some_and(|s| s.total > 0)
        {
            let _ = tx.send(true);
        }
    });
    let outcome = engine::run_cancellable(job, &store, Some(callback), Some(rx))
        .await
        .expect("el corte durante la pasada final es un cierre limpio, no un error");

    assert!(outcome.interrupted, "el primer Ctrl-C responde también en la pasada final");
    assert!(outcome.truncated.is_none(), "interrumpido no es truncado");
    assert_eq!(status_de(&store), "paused", "el fichero queda reanudable, como en pause()");

    // Y la reanudación —con cero pendientes: solo faltaba la pasada final— lo termina con
    // exactamente los mismos hallazgos que el rastreo que nunca se cortó. Si el corte hubiera
    // dejado hallazgos de conjunto a medias sin recalcular, aquí saldrían duplicados.
    let reanudado = engine::resume(&store).await.expect("reanudar la pasada final");
    assert!(!reanudado.interrupted);
    assert_eq!(status_de(&store), "done");
    assert_eq!(
        hallazgos_por_regla(&store),
        hallazgos_por_regla(&store_control),
        "los hallazgos del reanudado son los del rastreo del tirón, regla a regla"
    );
}
