//! El hueco del modo `list`: los enlaces salientes se registran, las externas se comprueban
//! y el core sabe que el grafo está incompleto por construcción.
//!
//! Tres promesas, y la tercera es la que protege de un falso positivo sistemático:
//!
//! 1. **Los enlaces de una página son una propiedad de esa página.** Auditar exactamente el
//!    conjunto pedido incluye saber a dónde apunta. El destino se registra —sin él la
//!    `LinkRow` se descarta en silencio contra el índice hash→id— pero **no se pide**:
//!    descargarlo sería rastrear más allá de la lista, que es justo lo que este modo promete
//!    no hacer.
//! 2. **Las externas se comprueban igual que en modo `http`** (`check_external`): pedir una
//!    URL ajena para saber si resuelve no es rastrear el sitio del usuario, y es lo que hace
//!    Screaming Frog en su modo lista.
//! 3. **Un rastreo en modo lista tiene el grafo incompleto siempre**: ninguna página tiene a
//!    sus enlazadores, porque no se descargan. Sin `TruncationReason::ListMode`, las cuatro
//!    reglas de `REQUIERE_GRAFO_COMPLETO` afirmarían sobre un grafo que es todo agujeros —
//!    con sitemaps encendidos, la lista entera salía como huérfana (`v_orphans`: interna,
//!    en el sitemap, sin enlaces entrantes y con fila en `pages`). Que la CLI apague los
//!    sitemaps a mano era lo único que lo impedía: casualidad, no diseño.
//!
//! Dos hosts sin salir de la máquina: `127.0.0.1` es el sitio auditado y `localhost` el
//! ajeno, como enseñan `externas.rs` y `crawl_delay.rs`.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::engine::TruncationReason;
use crawlforge_core::job::{CrawlJob, CrawlMode};
use rusqlite::Connection;
use support::servidor::{Respuesta, ServidorDePruebas};

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-modo-lista-{}-{nombre}", std::process::id()));
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

/// Un trabajo en modo `list`, con los defectos del core (sitemaps encendidos incluidos,
/// que es como llega desde la FFI o las apps: la línea de la CLI que los apaga no existe ahí).
fn trabajo_de_lista(urls: Vec<String>) -> CrawlJob {
    let first = urls.first().cloned().unwrap_or_default();
    let mut job = CrawlJob::http(first);
    job.mode = CrawlMode::List { urls };
    job
}

/// `(crawl_state, status_code, is_internal)` de la fila de una URL, por su ruta.
fn fila(conn: &Connection, ruta: &str) -> Option<(String, Option<i64>, i64)> {
    conn.query_row(
        "SELECT crawl_state, status_code, is_internal FROM urls WHERE path = ?1",
        [ruta],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .ok()
}

fn contar(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).expect("consulta de recuento")
}

// ─── Parte 1: los enlaces salientes se registran, sin ampliar el rastreo ─────────────

#[tokio::test]
async fn los_enlaces_salientes_se_registran_sin_pedir_sus_destinos() {
    // /a y /b están en la lista; /fuera es interna pero no está. El grafo debe tener las dos
    // aristas de /a —a /b y a /fuera—, y /fuera debe quedar registrada sin que se pida.
    let servidor = ServidorDePruebas::arrancar(&[
        (
            "/a",
            Respuesta::pagina(
                "A",
                "<a href=\"/b\">b</a> <a href=\"/fuera\">no está en la lista</a>",
            ),
        ),
        ("/b", Respuesta::pagina("B", "<p>sin enlaces</p>")),
        ("/fuera", Respuesta::pagina("Fuera", "<p>nadie debe pedirme</p>")),
    ])
    .await;

    let tmp = Temporal::new("enlaces");
    let mut job = trabajo_de_lista(vec![servidor.url("/a"), servidor.url("/b")]);
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 2, "se rastrea la lista y nada más");
    assert_eq!(servidor.peticiones("/fuera"), 0, "el destino fuera de la lista no se pide");

    let conn = abrir(&tmp.store());
    let enlaces = contar(
        &conn,
        "SELECT COUNT(*) FROM links l
         JOIN urls f ON f.id = l.from_url_id
         WHERE f.path = '/a'",
    );
    assert_eq!(
        enlaces, 2,
        "las dos aristas de /a sobreviven: sin fila de destino, el escritor las descarta \
         en silencio"
    );

    // El destino fuera de la lista queda registrado como «no rastreado», no como pendiente:
    // `pending` es exactamente lo que una reanudación relee, y releerlo rastrearía más allá
    // de la lista.
    let (estado, status, interna) = fila(&conn, "/fuera").expect("la fila de /fuera existe");
    assert_eq!(estado, "skipped", "registrada sin rastrear");
    assert_eq!(status, None, "sin estado: nadie la pidió");
    assert_eq!(interna, 1);
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM urls WHERE crawl_state = 'pending'"),
        0,
        "en modo lista no queda nada pendiente: una reanudación no debe tener qué releer"
    );

    // Guarda del banco de comparación contra otras herramientas: sus consultas filtran por
    // `status_code = 200` y HTML, y las filas nuevas —destinos `skipped`, sin estado— no
    // pueden colarse en ese filtro. Si esto falla, la comparación campo a campo con
    // Screaming Frog deja de medir lo que cree medir.
    assert_eq!(
        contar(
            &conn,
            "SELECT COUNT(*) FROM urls WHERE is_internal = 1 AND status_code = 200
             AND content_type LIKE 'text/html%'"
        ),
        2,
        "el banco de comparación tiene que seguir viendo exactamente la lista"
    );
}

// ─── Parte 2: las externas se comprueban también en modo lista ───────────────────────

#[tokio::test]
async fn las_externas_se_comprueban_tambien_en_modo_lista() {
    // El mismo contrato que `externas.rs`, en modo lista: una viva y una rota, sonda HEAD,
    // y el 404 externo por fin tiene estado sobre el que disparar.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[(
        "/guia",
        Respuesta::pagina("Guía", "<p>sigo aquí</p>"),
    )])
    .await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/unica",
        Respuesta::pagina(
            "Única",
            &format!(
                "<a href=\"{viva}\">la guía</a> <a href=\"{rota}\">se mudó</a>",
                viva = ajeno.url_como_otro_host("/guia"),
                rota = ajeno.url_como_otro_host("/se-mudo"),
            ),
        ),
    )])
    .await;

    let tmp = Temporal::new("externas");
    let mut job = trabajo_de_lista(vec![propio.url("/unica")]);
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_checked, 2, "las dos externas se comprueban");
    assert_eq!(ajeno.metodos("/se-mudo"), vec!["HEAD"], "solo estado, nunca el cuerpo");

    let conn = abrir(&tmp.store());
    let estado_de = |ruta: &str| fila(&conn, ruta).and_then(|(_, s, _)| s);
    assert_eq!(estado_de("/guia"), Some(200));
    assert_eq!(estado_de("/se-mudo"), Some(404));
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM issues WHERE rule_id = 'HTTP-404-EXTERNAL'"),
        1,
        "un enlace externo roto, un hallazgo — igual que en modo http"
    );

    // Guarda del banco de comparación: una externa sondeada queda con 200 y `text/html`,
    // pero con `is_internal = 0` — el filtro del banco no puede confundirla con una página
    // de la lista.
    assert_eq!(
        contar(
            &conn,
            "SELECT COUNT(*) FROM urls WHERE is_internal = 1 AND status_code = 200
             AND content_type LIKE 'text/html%'"
        ),
        1,
        "el banco de comparación tiene que seguir viendo exactamente la lista"
    );
}

// ─── Parte 3: el grafo del modo lista está incompleto por construcción ───────────────

#[tokio::test]
async fn una_lista_con_sitemaps_no_reporta_la_lista_entera_como_huerfana() {
    // El falso positivo que esta salvaguarda corta. Con sitemaps encendidos —el defecto del
    // core; solo la CLI los apaga a mano— cada URL de la lista cumple las cuatro condiciones
    // de `v_orphans`: interna, en el sitemap, sin enlaces entrantes (sus enlazadores no se
    // descargan nunca) y con fila en `pages`. Sin `TruncationReason::ListMode`, la lista
    // entera menos la primera —`v_orphans` exime a `base_url`— salía como huérfana.
    let servidor = ServidorDePruebas::arrancar_con_puerto(|puerto| {
        let base = format!("http://127.0.0.1:{puerto}");
        vec![
            (
                "/sitemap.xml".to_string(),
                Respuesta::xml(format!(
                    r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                       <url><loc>{base}/p1</loc></url>
                       <url><loc>{base}/p2</loc></url>
                       <url><loc>{base}/p3</loc></url>
                       <url><loc>{base}/no-en-la-lista</loc></url>
                       </urlset>"#
                )),
            ),
            // Páginas reales de un sitio real: ninguna enlaza a las demás de la lista.
            ("/p1".to_string(), Respuesta::pagina("P1", "<p>uno</p>")),
            ("/p2".to_string(), Respuesta::pagina("P2", "<p>dos</p>")),
            ("/p3".to_string(), Respuesta::pagina("P3", "<p>tres</p>")),
            (
                "/no-en-la-lista".to_string(),
                Respuesta::pagina("Extra", "<p>declarada en el sitemap, fuera de la lista</p>"),
            ),
        ]
    })
    .await;

    let tmp = Temporal::new("huerfanas");
    let job = trabajo_de_lista(vec![
        servidor.url("/p1"),
        servidor.url("/p2"),
        servidor.url("/p3"),
    ]);
    assert!(job.discover_sitemaps, "el defecto del core que provoca el caso");

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");

    // El sitemap no amplía la lista: declarar una URL no es pedir que se audite.
    assert_eq!(outcome.metrics.urls_fetched, 3, "se rastrea la lista y nada más");
    assert_eq!(
        servidor.peticiones("/no-en-la-lista"),
        0,
        "una URL del sitemap que no está en la lista no se pide"
    );

    let conn = abrir(&tmp.store());
    // Las tres de la lista están en el sitemap, tienen página y no tienen enlazadores:
    // exactamente el patrón del falso positivo.
    assert_eq!(
        contar(
            &conn,
            "SELECT COUNT(*) FROM urls u JOIN pages p ON p.url_id = u.id
             WHERE u.in_sitemap = 1"
        ),
        3,
        "el cruce con el sitemap sí se registra: es información, no ampliación"
    );
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM issues WHERE rule_id = 'INDEX-ORPHAN-PAGE'"),
        0,
        "sin enlazadores descargados, «huérfana» es una afirmación que el modo lista no \
         puede hacer"
    );
    // Y no solo esa: ninguna de las reglas que exigen el grafo completo puede evaluar.
    assert_eq!(
        contar(
            &conn,
            "SELECT COUNT(*) FROM issues WHERE rule_id IN
             ('INDEX-DEEP-PAGE', 'INDEX-NO-INTERNAL-LINKS-IN', 'INDEX-SECTION-DISCONNECTED')"
        ),
        0,
        "las cuatro reglas de REQUIERE_GRAFO_COMPLETO callan sobre un grafo con agujeros"
    );
}

#[tokio::test]
async fn el_modo_lista_queda_marcado_como_grafo_incompleto_tambien_sin_sitemaps() {
    // La salvaguarda vive en el core y no depende de los sitemaps: el grafo está incompleto
    // por definición del modo, no por lo que declare el sitio. Es también lo que hace que
    // `diff` no afirme que una URL «desapareció» comparando contra un rastreo en modo lista.
    let servidor =
        ServidorDePruebas::arrancar(&[("/solo", Respuesta::pagina("Sola", "<p>x</p>"))]).await;

    let tmp = Temporal::new("marca");
    let mut job = trabajo_de_lista(vec![servidor.url("/solo")]);
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(
        outcome.truncated,
        Some(TruncationReason::ListMode),
        "el conjunto rastreado no es el sitio entero, y el resultado lo dice"
    );

    let conn = abrir(&tmp.store());
    let (truncated, reason): (i64, Option<String>) = conn
        .query_row("SELECT truncated, truncated_reason FROM crawl_meta", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("leer crawl_meta");
    assert_eq!(truncated, 1);
    assert_eq!(reason.as_deref(), Some("list_mode"));
}

// ─── Guardas de no regresión ─────────────────────────────────────────────────────────

#[tokio::test]
async fn un_corte_real_gana_al_marcado_del_modo_lista() {
    // Guarda: si un rastreo en modo lista se corta de verdad (presupuesto de URLs), el
    // motivo registrado es el corte, no `list_mode` — «te faltan URLs de tu propia lista»
    // es más información que «el grafo está incompleto», y ambos encienden `truncated`.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/a", Respuesta::pagina("A", "<p>a</p>")),
        ("/b", Respuesta::pagina("B", "<p>b</p>")),
        ("/c", Respuesta::pagina("C", "<p>c</p>")),
    ])
    .await;

    let tmp = Temporal::new("corte");
    let mut job = trabajo_de_lista(vec![
        servidor.url("/a"),
        servidor.url("/b"),
        servidor.url("/c"),
    ]);
    job.discover_sitemaps = false;
    job.limits.max_urls = Some(2);

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.truncated, Some(TruncationReason::MaxUrls));
}

#[tokio::test]
async fn el_modo_http_sigue_sin_marcarse_como_lista() {
    // Guarda: la marca es del modo lista. Un rastreo http completo sigue sin truncar, con
    // sus reglas de grafo completo activas.
    let servidor = ServidorDePruebas::arrancar(&[(
        "/",
        Respuesta::pagina("Inicio", "<a href=\"/otra\">otra</a>"),
    ), (
        "/otra",
        Respuesta::pagina("Otra", "<p>fin</p>"),
    )])
    .await;

    let tmp = Temporal::new("http");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;

    let outcome = crawlforge_core::engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.truncated, None, "un rastreo http completo no está truncado");

    let conn = abrir(&tmp.store());
    let truncated: i64 =
        conn.query_row("SELECT truncated FROM crawl_meta", [], |r| r.get(0)).expect("crawl_meta");
    assert_eq!(truncated, 0);
}
