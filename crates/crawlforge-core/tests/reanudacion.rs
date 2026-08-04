//! Reanudación de un rastreo interrumpido (`docs/03-MOTOR-CRAWL.md §7`).
//!
//! La prueba que importa es la de equivalencia: **reanudar no puede dar un resultado distinto
//! a no haber parado**. Se rastrea el mismo sitio dos veces —una del tirón, otra cortada en
//! mitad con la señal de cancelación y reanudada— y se comparan fila a fila los recuentos que
//! definen una auditoría: estados y profundidades de cada URL, páginas, enlaces con sus dos
//! extremos, imágenes, hallazgos por regla y los `internal_links_in` de la pasada final.
//!
//! El corte se provoca con una página lenta (`/lenta`, 700 ms): la cancelación se dispara en
//! cuanto las páginas rápidas se han pedido, así que cae con seguridad antes de que `/lenta`
//! responda y las páginas que solo se descubren a través de ella (`/s1..12`) quedan sin
//! rastrear. La reanudación tiene que pedir `/lenta`, descubrirlas y terminar el trabajo —
//! incluidos los enlaces de vuelta a URLs que escribió la sesión anterior, que es donde un
//! índice hash→id sin reponer los perdería en silencio.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::engine;
use crawlforge_core::job::CrawlJob;
use crawlforge_core::CoreError;
use rusqlite::Connection;
use std::time::Duration;
use support::servidor::{Respuesta, ServidorDePruebas};

/// Directorio temporal que se limpia solo. Mismo patrón que `pipeline.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("crawlforge-res-{}-{nombre}", std::process::id()));
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

fn abrir(path: &std::path::Path) -> Connection {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("abrir el fichero de rastreo")
}

/// El sitio de prueba. La mitad rápida (`/p*`) cae antes del corte; la mitad honda (`/s*`)
/// solo se descubre a través de `/lenta`, que tarda lo bastante para que el corte llegue
/// primero. Las `/s*` no tienen `h1` y llevan una imagen rota: hallazgos que solo puede
/// producir la sesión reanudada, y que tienen que salir idénticos al rastreo del tirón.
fn sitio() -> Vec<(String, Respuesta)> {
    let mut rutas = Vec::new();
    let enlaces_p: String =
        (1..=12).map(|i| format!("<p><a href=\"/p{i}\">P{i}</a></p>")).collect();
    rutas.push((
        "/".to_string(),
        Respuesta::pagina(
            "Inicio",
            &format!("{enlaces_p}<p><a href=\"/lenta\">Sección lenta</a></p>"),
        ),
    ));
    for i in 1..=12 {
        rutas.push((
            format!("/p{i}"),
            Respuesta::pagina(
                &format!("P{i}"),
                "<p>Contenido de la página rápida.</p><p><a href=\"/\">Inicio</a></p>",
            ),
        ));
    }
    let enlaces_s: String =
        (1..=12).map(|i| format!("<p><a href=\"/s{i}\">S{i}</a></p>")).collect();
    rutas.push((
        "/lenta".to_string(),
        Respuesta::pagina("Lenta", &enlaces_s).con_retardo(Duration::from_millis(700)),
    ));
    for i in 1..=12 {
        // Sin `h1` y con una imagen que da 404: cada una es un hallazgo de página que la
        // sesión reanudada tiene que producir igual que el rastreo completo.
        rutas.push((
            format!("/s{i}"),
            Respuesta::html(format!(
                "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\">\
                 <title>S{i}</title>\
                 <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
                 </head><body><main><p>Página S{i}, sin encabezado.</p>\
                 <img src=\"/logo.png\">\
                 <p><a href=\"/\">Inicio</a></p></main></body></html>"
            )),
        ));
    }
    rutas
}

/// Los recuentos que definen el resultado de una auditoría, listos para comparar entre dos
/// ficheros. Todo va por `path` y no por URL completa: los dos servidores usan puertos
/// distintos y el puerto no es parte del resultado.
#[derive(Debug, PartialEq)]
struct Huella {
    urls: Vec<(String, String, Option<i64>, Option<i64>)>,
    enlaces: Vec<(String, String, i64)>,
    imagenes: Vec<(String, String)>,
    hallazgos: Vec<(String, i64)>,
    paginas: Vec<(String, i64, i64)>,
    status: String,
}

fn huella(path: &std::path::Path) -> Huella {
    let conn = abrir(path);
    let recoger = |sql: &str, f: &dyn Fn(&rusqlite::Row<'_>) -> rusqlite::Result<_>| {
        let mut stmt = conn.prepare(sql).expect("preparar");
        let rows = stmt.query_map([], f).expect("consultar");
        rows.collect::<rusqlite::Result<Vec<_>>>().expect("recoger")
    };

    let urls = {
        let mut stmt = conn
            .prepare(
                "SELECT path, crawl_state, status_code, depth FROM urls ORDER BY path",
            )
            .expect("preparar urls");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .expect("consultar urls")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("recoger urls")
    };
    let enlaces = recoger(
        "SELECT uf.path, ut.path, COUNT(*) FROM links l
         JOIN urls uf ON uf.id = l.from_url_id
         JOIN urls ut ON ut.id = l.to_url_id
         GROUP BY uf.path, ut.path ORDER BY uf.path, ut.path",
        &|r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)),
    );
    let imagenes = {
        let mut stmt = conn
            .prepare(
                "SELECT up.path, us.path FROM images i
                 JOIN urls up ON up.id = i.page_url_id
                 JOIN urls us ON us.id = i.src_url_id
                 ORDER BY up.path, us.path",
            )
            .expect("preparar imágenes");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("consultar imágenes")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("recoger imágenes")
    };
    let hallazgos = {
        let mut stmt = conn
            .prepare("SELECT rule_id, COUNT(*) FROM issues GROUP BY rule_id ORDER BY rule_id")
            .expect("preparar hallazgos");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("consultar hallazgos")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("recoger hallazgos")
    };
    let paginas = {
        let mut stmt = conn
            .prepare(
                "SELECT u.path, p.internal_links_in, p.is_indexable FROM pages p
                 JOIN urls u ON u.id = p.url_id ORDER BY u.path",
            )
            .expect("preparar páginas");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .expect("consultar páginas")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("recoger páginas")
    };
    let status: String = conn
        .query_row("SELECT status FROM crawl_meta", [], |r| r.get(0))
        .expect("leer status");

    Huella { urls, enlaces, imagenes, hallazgos, paginas, status }
}

async fn rastreo_completo(tmp: &Temporal) -> std::path::PathBuf {
    let servidor = ServidorDePruebas::arrancar_con_puerto(|_| sitio()).await;
    let store = tmp.path.join("completo.sqlite");
    let mut job = CrawlJob::http(servidor.base());
    // Sin descubrimiento de sitemaps: este banco aísla el bucle de rastreo y su reanudación.
    job.discover_sitemaps = false;
    let outcome = engine::run(job, &store).await.expect("rastreo completo");
    assert!(!outcome.interrupted);
    store
}

#[tokio::test]
async fn reanudar_da_el_mismo_resultado_que_no_haber_parado() {
    let tmp = Temporal::new("equivalencia");
    let store_a = rastreo_completo(&tmp).await;

    // El mismo sitio, cortado en mitad y reanudado.
    let servidor = ServidorDePruebas::arrancar_con_puerto(|_| sitio()).await;
    let store_b = tmp.path.join("interrumpido.sqlite");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;

    let (tx, rx) = tokio::sync::watch::channel(false);
    let rastreo = engine::run_cancellable(job, &store_b, None, Some(rx));
    // El disparador: en cuanto las doce páginas rápidas se han pedido, se cancela. `/lenta`
    // sigue en vuelo (700 ms), así que el corte cae con seguridad en mitad del rastreo.
    let disparo = async {
        loop {
            let rapidas_pedidas = (1..=12).all(|i| servidor.peticiones(&format!("/p{i}")) > 0);
            if rapidas_pedidas {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tx.send(true).expect("el motor sigue escuchando la señal");
    };
    let (outcome, ()) = tokio::join!(rastreo, disparo);
    let outcome = outcome.expect("el corte es un cierre limpio, no un error");
    assert!(outcome.interrupted, "la señal llegó antes de terminar");
    assert!(outcome.truncated.is_none(), "interrumpido no es truncado");

    // El fichero interrumpido: en pausa, con pendientes escritas y sin la sección honda.
    {
        let conn = abrir(&store_b);
        let status: String =
            conn.query_row("SELECT status FROM crawl_meta", [], |r| r.get(0)).expect("status");
        assert_eq!(status, "paused", "un corte limpio queda en pausa, no en running");
        let pendientes: i64 = conn
            .query_row("SELECT COUNT(*) FROM urls WHERE crawl_state = 'pending'", [], |r| {
                r.get(0)
            })
            .expect("contar pendientes");
        assert!(pendientes > 0, "las no visitadas quedaron escritas como pending");
        let hondas: i64 = conn
            .query_row("SELECT COUNT(*) FROM urls WHERE path LIKE '/s%'", [], |r| r.get(0))
            .expect("contar hondas");
        assert_eq!(hondas, 0, "lo que cuelga de /lenta aún no se había descubierto");
    }

    // Reanudar termina el trabajo, con el mismo rastreo (mismo id) y la pasada final hecha.
    let reanudado = engine::resume(&store_b).await.expect("reanudar");
    assert!(!reanudado.interrupted);
    assert_eq!(reanudado.crawl_id, outcome.crawl_id, "es el mismo rastreo, no otro");

    // La prueba que importa: mismos recuentos que el rastreo que nunca paró.
    let a = huella(&store_a);
    let b = huella(&store_b);
    assert_eq!(a.status, "done");
    assert_eq!(b.status, "done");
    assert_eq!(a.urls, b.urls, "estados, códigos y profundidades por URL");
    assert_eq!(a.enlaces, b.enlaces, "el grafo de enlaces, extremo a extremo");
    assert_eq!(a.imagenes, b.imagenes, "las imágenes, con su página y su fichero");
    assert_eq!(a.hallazgos, b.hallazgos, "los hallazgos, regla a regla");
    assert_eq!(a.paginas, b.paginas, "las páginas, con los enlaces entrantes de la pasada final");

    // Y los enlaces de la sesión reanudada hacia URLs de la sesión anterior existen: es el
    // caso que perdería un índice hash→id sin reponer.
    let conn = abrir(&store_b);
    let vueltas: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM links l
             JOIN urls uf ON uf.id = l.from_url_id
             JOIN urls ut ON ut.id = l.to_url_id
             WHERE uf.path LIKE '/s%' AND ut.path = '/'",
            [],
            |r| r.get(0),
        )
        .expect("contar enlaces de vuelta");
    assert_eq!(vueltas, 12, "cada /s* enlaza a la portada escrita por la sesión anterior");
}

#[tokio::test]
async fn un_rastreo_terminado_no_se_reanuda() {
    let tmp = Temporal::new("terminado");
    let store = rastreo_completo(&tmp).await;

    let err = engine::resume(&store).await.expect_err("un rastreo done no se reanuda");
    let CoreError::NotResumable { reason, .. } = err else {
        panic!("el error debe ser NotResumable, no {err:?}");
    };
    assert!(reason.contains("done"), "el motivo nombra el estado: {reason}");
}

#[tokio::test]
async fn un_fichero_de_otro_esquema_no_se_reanuda() {
    let tmp = Temporal::new("esquema");
    let store = tmp.path.join("viejo.sqlite");
    {
        let conn = crawlforge_core::store::open_writer(&store).expect("crear el fichero");
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime)
             VALUES ('x','p','P','https://ejemplo.es/','http',datetime('now'),'running','{}',
                     '0','0','free')",
            [],
        )
        .expect("insertar crawl_meta");
        // Un fichero escrito por un core futuro: la versión no coincide con la nuestra.
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
            rusqlite::params![crawlforge_core::SCHEMA_VERSION + 1],
        )
        .expect("marcar una versión futura");
    }

    let err = engine::resume(&store).await.expect_err("otro esquema no se reanuda");
    assert!(matches!(err, CoreError::NotResumable { .. }), "{err:?}");
    assert!(err.to_string().contains("esquema"), "{err}");
}

/// Inserta en un fichero recién migrado unos metadatos con la configuración dada. Es el
/// molde de los tests de manipulación: un `config_json` que no describe el rastreo que los
/// metadatos dicen continuar.
fn fichero_manipulado(store: &std::path::Path, config_json: &str, base_url: &str, mode: &str) {
    let conn = crawlforge_core::store::open_writer(store).expect("crear el fichero");
    conn.execute(
        "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                 status, config_json, core_version, rules_version,
                                 tier_at_runtime)
         VALUES ('x','p','P',?1,?2,datetime('now'),'paused',?3,'0','0','free')",
        rusqlite::params![base_url, mode, config_json],
    )
    .expect("insertar crawl_meta");
}

#[tokio::test]
async fn una_configuracion_guardada_incoherente_con_los_metadatos_no_se_reanuda() {
    // El fichero es entrada no confiable: se comparte. Reanudar no puede ejecutar sin más lo
    // que diga `config_json`; el objetivo guardado tiene que cuadrar con los metadatos del
    // propio fichero.
    let tmp = Temporal::new("incoherente");

    // Una semilla que apunta a otro sitio: sin la validación, `resume` rastrearía un
    // tercero con la apariencia de continuar un rastreo propio.
    let store = tmp.path.join("otro-sitio.sqlite");
    let job = CrawlJob::http("http://objetivo-ajeno.invalid/");
    let config = serde_json::to_string(&job).expect("serializar el job");
    fichero_manipulado(&store, &config, "http://127.0.0.1:1/", "http");
    let err = engine::resume(&store).await.expect_err("otra semilla no se reanuda");
    assert!(matches!(err, CoreError::NotResumable { .. }), "{err:?}");
    assert!(err.to_string().contains("coherente"), "el motivo dice qué no cuadra: {err}");

    // Un modo que no es el de los metadatos: es la forma del ataque de
    // `mode: filesystem, root: "/"` inyectado en el fichero de un rastreo http, que hacía
    // que `resume` recorriera el disco del usuario. Aquí con una raíz inocua: el punto es
    // que ni se llega a abrir.
    let store = tmp.path.join("otro-modo.sqlite");
    let job = CrawlJob::filesystem(&tmp.path, "http://127.0.0.1:1/");
    let config = serde_json::to_string(&job).expect("serializar el job");
    fichero_manipulado(&store, &config, "http://127.0.0.1:1/", "http");
    let err = engine::resume(&store).await.expect_err("otro modo no se reanuda");
    assert!(matches!(err, CoreError::NotResumable { .. }), "{err:?}");
    assert!(err.to_string().contains("coherente"), "el motivo dice qué no cuadra: {err}");

    // Y en modo filesystem, una raíz distinta de la registrada: mismo ataque sin cambiar
    // el modo. `crawl_meta.source_path` queda NULL aquí, que nunca puede cuadrar.
    let store = tmp.path.join("otra-raiz.sqlite");
    let job = CrawlJob::filesystem(&tmp.path, "http://127.0.0.1:1/");
    let config = serde_json::to_string(&job).expect("serializar el job");
    fichero_manipulado(&store, &config, "http://127.0.0.1:1/", "filesystem");
    let err = engine::resume(&store).await.expect_err("otra raíz no se reanuda");
    assert!(matches!(err, CoreError::NotResumable { .. }), "{err:?}");
}

#[tokio::test]
async fn reanudar_no_hereda_el_permiso_de_ignorar_robots_del_fichero() {
    // `ignore_robots` es un permiso que se concede al lanzar el rastreo, no una propiedad
    // del fichero: un fichero fabricado con `ignore_robots: true` convertiría `resume` en
    // un rastreo que se salta el robots.txt sin que nadie lo haya pedido en esta sesión.
    // Reanudar no tiene flag para volver a concederlo, así que vale el defecto: respetarlo.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/robots.txt", Respuesta::texto("User-agent: *\nDisallow: /lenta\n")),
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
            Respuesta::pagina("Lenta", "<p>Prohibida por robots.</p>")
                .con_retardo(Duration::from_millis(700)),
        ),
    ])
    .await;

    let tmp = Temporal::new("robots-vivo");
    let store = tmp.path.join("rastreo.sqlite");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;
    // El rastreo original sí tenía el permiso: ignora el robots.txt y pide /lenta.
    job.limits.ignore_robots = true;

    let (tx, rx) = tokio::sync::watch::channel(false);
    let rastreo = engine::run_cancellable(job, &store, None, Some(rx));
    // El corte cae con /lenta en vuelo: su fila queda `pending` y es lo que la reanudación
    // tendrá que decidir si pide.
    let disparo = async {
        while servidor.peticiones("/lenta") == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tx.send(true).expect("el motor sigue escuchando la señal");
    };
    let (outcome, ()) = tokio::join!(rastreo, disparo);
    assert!(outcome.expect("el corte es un cierre limpio").interrupted);

    // La reanudación toma el permiso de la sesión viva —que no lo ha concedido—, no del
    // fichero: /lenta queda excluida por robots, no rastreada.
    engine::resume(&store).await.expect("reanudar");
    let conn = abrir(&store);
    let (estado, motivo): (String, Option<String>) = conn
        .query_row(
            "SELECT crawl_state, exclusion_reason FROM urls WHERE path = '/lenta'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("leer la fila de /lenta");
    assert_eq!(estado, "excluded", "el ignore_robots del fichero no puede reactivarse solo");
    assert_eq!(motivo.as_deref(), Some("robots"));
}

#[tokio::test]
async fn una_auditoria_local_no_repite_sus_sondas_al_reanudar_y_lo_dice() {
    // Dos contratos a la vez, y el segundo llegó después.
    //
    // **Uno.** Al cortar, las sondas en vuelo o en cola se perdían para siempre: su fila queda
    // `skipped` con estado nulo, `resume` solo reencola las `pending`, y como el frontier ya la
    // tiene por vista, un enlace nuevo a esa misma URL tampoco la recuperaba. Nadie lo contaba.
    //
    // **Dos.** El perímetro de red deja alcanzar una dirección privada cuando *todos* los
    // objetivos del rastreo son locales — el `astro dev`, el pre en la LAN. Al reanudar, esa
    // excepción **no se concede**: el objetivo lo declara el fichero, y el fichero es entrada no
    // confiable. Un `.sqlite` que dijera `base_url = http://localhost:4321/` con una fila externa
    // inyectada hacía que la máquina que lo reanuda sondeara su propio loopback (comprobado:
    // `HEAD /app/kibana`, `status=200` escrito en el fichero).
    //
    // Este rastreo es local por los dos lados, así que aquí manda el segundo contrato: lo que se
    // afirma es que la reanudación **no** repite las sondas y que quedan dichas, no calladas.
    // La otra mitad —que una auditoría pública sí las repite— no tiene test de integración, y no
    // por descuido: montarla pediría una externa pública que respondiera en loopback, que es
    // exactamente la combinación que el perímetro existe para impedir.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host_con_puerto(|_| {
        (0..6)
            .map(|i| {
                (
                    format!("/e{i}"),
                    // Cada sonda tarda: con una en vuelo por host ajeno, el corte pilla el
                    // resto en cola con seguridad.
                    Respuesta::pagina("Ajena", "<p>x</p>")
                        .con_retardo(Duration::from_millis(300)),
                )
            })
            .collect()
    })
    .await;
    let enlaces: String = (0..6)
        .map(|i| format!("<a href=\"{}\">{i}</a> ", ajeno.url_como_otro_host(&format!("/e{i}"))))
        .collect();
    let propio =
        ServidorDePruebas::arrancar_con_puerto(|_| {
            vec![("/".to_string(), Respuesta::pagina("Inicio", &enlaces))]
        })
        .await;

    let tmp = Temporal::new("sondas-cortadas");
    let store = tmp.path.join("crawl.sqlite");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;

    let (tx, rx) = tokio::sync::watch::channel(false);
    let rastreo = engine::run_cancellable(job, &store, None, Some(rx));
    // Se corta con la primera sonda ya en vuelo: las demás están en la cola de externas.
    let disparo = async {
        while ajeno.peticiones("/e0") == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tx.send(true).expect("el motor sigue escuchando la señal");
    };
    let (outcome, ()) = tokio::join!(rastreo, disparo);
    let outcome = outcome.expect("el corte es un cierre limpio");
    assert!(outcome.interrupted);
    assert!(
        outcome.metrics.externals_unchecked > 0,
        "el resumen tiene que decir cuántas quedaron sin comprobar, y dijo {}",
        outcome.metrics.externals_unchecked
    );

    let sin_estado_tras_el_corte: i64 = {
        let conn = abrir(&store);
        conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE is_internal = 0 AND status_code IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("contar")
    };
    assert!(sin_estado_tras_el_corte > 0, "hay sondas a medias que recuperar");

    // Reanudar no las repite, porque el objetivo local lo declara el fichero.
    let reanudado = engine::resume(&store).await.expect("reanudar");
    assert_eq!(
        reanudado.metrics.externals_checked, 0,
        "una reanudación no concede la excepción de red local: el objetivo sale del fichero"
    );

    let conn = abrir(&store);
    let sin_estado: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE is_internal = 0 AND status_code IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("contar");
    assert_eq!(
        sin_estado, sin_estado_tras_el_corte,
        "siguen exactamente las mismas sin comprobar: la reanudación no tocó ninguna"
    );
    // Desorden conocido y aceptado: esas filas se quedan sin `exclusion_reason`, porque el
    // rechazo ocurre al releer el plan y ahí no se escribe —**nadie escribe en SQLite salvo el
    // hilo escritor**, y en ese punto todavía no existe—. La consecuencia es que cada
    // reanudación posterior las vuelve a leer y a rechazar: idempotente y sin coste de red, pero
    // indistinguibles de una sonda que se cortó a medias. Escribir el motivo pide llevar la lista
    // hasta el escritor, y eso es un cambio aparte.
}

#[tokio::test]
async fn reanudar_no_pide_una_url_pendiente_que_no_es_del_sitio() {
    // `validate_resume_scope` valida el `config_json`, que es lo que pedía la revisión
    // anterior, pero justo después las URLs se recargaban con un `SELECT ... WHERE crawl_state
    // = 'pending'` **sin comprobar host ni esquema**. Una fila inyectada en un fichero por lo
    // demás legítimo hacía que `resume` la pidiera con un `GET` completo y guardara el cuerpo.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", Respuesta::pagina("Inicio", "<p>x</p>")),
        ("/ajena", Respuesta::pagina("Ajena", "<p>no deberías pedirme</p>")),
    ])
    .await;

    let tmp = Temporal::new("pendiente-ajena");
    let store = tmp.path.join("crawl.sqlite");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;
    let config = serde_json::to_string(&job).expect("serializar el job");
    fichero_manipulado(&store, &config, &servidor.base(), "http");

    // La fila inyectada: el mismo servidor bajo otro nombre de host, que para el motor es otro
    // dominio. Que responda de verdad es lo que hace verificable el «no se pidió».
    {
        let conn = crawlforge_core::store::open_writer(&store).expect("abrir el fichero");
        conn.execute(
            "INSERT INTO urls (url, url_hash, scheme, host, path, depth, is_internal,
                               in_sitemap, crawl_state)
             VALUES (?1, 42, 'http', 'localhost', '/ajena', 0, 1, 0, 'pending')",
            rusqlite::params![servidor.url_como_otro_host("/ajena")],
        )
        .expect("insertar la fila manipulada");
    }

    engine::resume(&store).await.expect("reanudar");
    assert_eq!(
        servidor.peticiones("/ajena"),
        0,
        "la URL pendiente no es del sitio de los metadatos: no se pide"
    );
    // La fila sigue ahí y sigue `pending`: descartarla del plan no es borrarla del fichero.
    let conn = abrir(&store);
    let estado: String = conn
        .query_row("SELECT crawl_state FROM urls WHERE path = '/ajena'", [], |r| r.get(0))
        .expect("la fila manipulada sigue en el fichero");
    assert_eq!(estado, "pending");
}

#[tokio::test]
async fn reanudar_un_fichero_sin_metadatos_dice_por_que() {
    // Un fichero migrado pero sin fila de `crawl_meta`: ni es un rastreo ni se puede continuar.
    let tmp = Temporal::new("sin-meta");
    let store = tmp.path.join("vacio.sqlite");
    {
        crawlforge_core::store::open_writer(&store).expect("crear el fichero");
    }
    let err = engine::resume(&store).await.expect_err("sin metadatos no hay reanudación");
    assert!(matches!(err, CoreError::NotResumable { .. }), "{err:?}");
}
