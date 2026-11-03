//! El freno adaptativo ante conexiones caídas, y el `Retry-After` de las páginas propias.
//!
//! Dos huecos que cierra este fichero, ejecutando cada caso contra el servidor de pruebas:
//!
//! 1. Solo los 429 y 503 contaban como sobrecarga. Un servidor saturado muchas veces no llega a
//!    contestar: corta la conexión o no responde a tiempo, y el freno no se enteraba.
//! 2. El `Retry-After` de un 429 o un 503 propio se ignoraba: el reintento salía al cabo del
//!    backoff de siempre (1, 2, 4 s) aunque el servidor hubiera dicho cuánto esperar, y el resto
//!    de páginas del host se seguía despachando mientras tanto.
//!
//! Las aserciones de tiempo son cotas inferiores holgadas: en una máquina cargada las esperas
//! solo pueden crecer, y así ese crecimiento no vuelve rojo ningún test.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::dns::{Lookup, StaticLookup};
use crawlforge_core::engine;
use crawlforge_core::job::CrawlJob;
use rusqlite::Connection;
use std::sync::Arc;
use std::time::Duration;
use support::servidor::{Respuesta, ServidorDePruebas};

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-freno-{}-{nombre}", std::process::id()));
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

/// `(crawl_state, status_code, error_kind)` de una URL.
fn fila(conn: &Connection, url: &str) -> (String, Option<i64>, Option<String>) {
    conn.query_row(
        "SELECT crawl_state, status_code, error_kind FROM urls WHERE url = ?1",
        [url],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .expect("la fila existe")
}

fn job(servidor: &ServidorDePruebas) -> CrawlJob {
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;
    job
}

/// Lo que pasó entre la primera y la segunda petición a una ruta.
fn hueco_entre_las_dos_primeras(servidor: &ServidorDePruebas, ruta: &str) -> Duration {
    let l = servidor.llegadas(ruta);
    assert!(l.len() >= 2, "{ruta} debería haberse reintentado: {} peticiones", l.len());
    l[1].duration_since(l[0])
}

/// Una fecha HTTP (IMF-fixdate) `segundos` después de ahora. Escrita a mano por la misma razón
/// que el parser del motor: no hay otra fecha HTTP en el proyecto que justifique una dependencia.
fn fecha_http_dentro_de(segundos: u64) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("reloj posterior a 1970")
        .as_secs()
        + segundos;
    let (dias, resto) = (t / 86_400, t % 86_400);
    // Inverso de `days_from_civil` (Howard Hinnant).
    let z = dias + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = yoe + era * 400 + u64::from(mes <= 2);
    const MESES: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const DIAS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    format!(
        "{}, {dia:02} {} {anio} {:02}:{:02}:{:02} GMT",
        DIAS[(dias % 7) as usize],
        MESES[(mes - 1) as usize],
        resto / 3_600,
        resto % 3_600 / 60,
        resto % 60
    )
}

// ─── Retry-After ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn un_429_con_retry_after_en_segundos_espera_lo_pedido_antes_de_reintentar() {
    // Sin el arreglo, el reintento salía al cabo de `backoff_delay(1)`: entre 0,5 y 1,5 s.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", Respuesta::pagina("Inicio", "<a href=\"/frenada\">f</a>")),
        (
            "/frenada",
            Respuesta::pagina("Frenada", "<p>ya</p>")
                .fallando_las_primeras_con(429, 1, &[("Retry-After", "2")]),
        ),
    ])
    .await;
    let tmp = Temporal::new("segundos");
    let outcome = engine::run(job(&servidor), &tmp.store()).await.expect("rastrear");

    let hueco = hueco_entre_las_dos_primeras(&servidor, "/frenada");
    assert!(hueco >= Duration::from_secs(2), "reintentó a los {hueco:?}; pedía 2 s");
    assert_eq!(outcome.truncated, None, "el segundo intento respondió: nada queda pendiente");
    let conn = abrir(&tmp.store());
    assert_eq!(fila(&conn, &servidor.url("/frenada")), ("done".into(), Some(200), None));
}

#[tokio::test]
async fn un_retry_after_con_fecha_http_tambien_se_obedece() {
    // La fecha se trunca a segundos y se fija al montar las rutas: con cuatro segundos de
    // margen, lo pedido al llegar la petición son tres. La cota de 2 s deja holgura.
    let fecha = fecha_http_dentro_de(4);
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", Respuesta::pagina("Inicio", "<a href=\"/frenada\">f</a>")),
        (
            "/frenada",
            Respuesta::pagina("Frenada", "<p>ya</p>")
                .fallando_las_primeras_con(503, 1, &[("Retry-After", &fecha)]),
        ),
    ])
    .await;
    let tmp = Temporal::new("fecha");
    engine::run(job(&servidor), &tmp.store()).await.expect("rastrear");

    let hueco = hueco_entre_las_dos_primeras(&servidor, "/frenada");
    assert!(hueco >= Duration::from_secs(2), "reintentó a los {hueco:?}; pedía hasta {fecha}");
    let conn = abrir(&tmp.store());
    assert_eq!(fila(&conn, &servidor.url("/frenada")), ("done".into(), Some(200), None));
}

#[tokio::test]
async fn un_retry_after_por_encima_del_tope_deja_la_pagina_pendiente_sin_reintentar() {
    // Un día entero. Obedecerlo colgaría el rastreo; reintentar al cabo de uno, dos y cuatro
    // segundos sería desobedecerlo tres veces. Se pide una vez y queda pendiente, como el 429
    // que sobrevive a los reintentos desde 0.17.0.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", Respuesta::pagina("Inicio", "<a href=\"/frenada\">f</a> <a href=\"/a\">a</a>")),
        ("/a", Respuesta::pagina("A", "<p>a</p>")),
        (
            "/frenada",
            Respuesta::pagina("Frenada", "<p>nunca</p>")
                .fallando_las_primeras_con(429, usize::MAX, &[("Retry-After", "86400")]),
        ),
    ])
    .await;
    let tmp = Temporal::new("tope");
    let outcome = engine::run(job(&servidor), &tmp.store()).await.expect("rastrear");

    assert_eq!(servidor.peticiones("/frenada"), 1, "ni un reintento contra lo que pidió");
    assert_eq!(outcome.truncated, Some(engine::TruncationReason::RateLimited));
    assert_eq!(outcome.metrics.rate_limited, 1);
    let conn = abrir(&tmp.store());
    assert_eq!(fila(&conn, &servidor.url("/frenada")), ("pending".into(), None, None));
    assert_eq!(fila(&conn, &servidor.url("/a")), ("done".into(), Some(200), None));
    assert!(engine::finished_rate_limited(&conn), "reanudable, como cualquier 429 pendiente");
}

#[tokio::test]
async fn mientras_un_host_espera_su_retry_after_no_se_le_despacha_nada_mas() {
    // `/frenada` pide 2 s. `/lenta` tarda medio segundo y descubre dos páginas más del mismo
    // host: sin la pausa, salían en cuanto se descubrían, con el servidor pidiendo calma.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", Respuesta::pagina("Inicio", "<a href=\"/frenada\">f</a> <a href=\"/lenta\">l</a>")),
        (
            "/frenada",
            Respuesta::pagina("Frenada", "<p>ya</p>")
                .fallando_las_primeras_con(429, 1, &[("Retry-After", "2")]),
        ),
        (
            "/lenta",
            Respuesta::pagina("Lenta", "<a href=\"/b1\">1</a> <a href=\"/b2\">2</a>")
                .con_retardo(Duration::from_millis(500)),
        ),
        ("/b1", Respuesta::pagina("B1", "<p>1</p>")),
        ("/b2", Respuesta::pagina("B2", "<p>2</p>")),
    ])
    .await;
    let tmp = Temporal::new("pausa");
    engine::run(job(&servidor), &tmp.store()).await.expect("rastrear");

    let el_429 = servidor.llegadas("/frenada")[0];
    for ruta in ["/b1", "/b2"] {
        let llegada = servidor.llegadas(ruta)[0];
        let tras = llegada.duration_since(el_429);
        assert!(
            tras >= Duration::from_secs(2),
            "{ruta} salió {tras:?} después del 429, dentro de los 2 s que pidió el servidor"
        );
    }
}

// ─── Conexiones caídas ───────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn las_conexiones_cortadas_cuentan_para_el_freno_adaptativo() {
    // Tres páginas que cortan la conexión sin responder: tres señales de sobrecarga seguidas,
    // y la concurrencia del host baja a la mitad. Cada una agota sus tres reintentos con
    // backoff, así que el test tarda lo que ellos (unos siete segundos).
    let servidor = ServidorDePruebas::arrancar(&[
        (
            "/",
            Respuesta::pagina(
                "Inicio",
                "<a href=\"/c1\">1</a> <a href=\"/c2\">2</a> <a href=\"/c3\">3</a>",
            ),
        ),
        ("/c1", Respuesta::html("").cortando_la_conexion()),
        ("/c2", Respuesta::html("").cortando_la_conexion()),
        ("/c3", Respuesta::html("").cortando_la_conexion()),
    ])
    .await;
    let tmp = Temporal::new("cortes");
    let outcome = engine::run(job(&servidor), &tmp.store()).await.expect("rastrear");

    let conn = abrir(&tmp.store());
    for ruta in ["/c1", "/c2", "/c3"] {
        assert_eq!(
            fila(&conn, &servidor.url(ruta)),
            ("error".into(), None, Some("connection".into())),
            "{ruta}"
        );
    }
    assert_eq!(outcome.metrics.throttle_reductions, 1, "5 → 2 tras la tercera conexión cortada");
}

#[tokio::test]
async fn un_nombre_que_no_resuelve_no_cuenta_para_el_freno() {
    // Cuatro páginas de un host que no existe: cuatro fallos de DNS seguidos. Un nombre que no
    // resuelve fallaría igual con el servidor ocioso; frenar por eso sería inventarse una carga.
    // El host se sigue como externo (`follow_external`) para que sus páginas pasen por el mismo
    // camino que las propias.
    let servidor = ServidorDePruebas::arrancar(&[(
        "/",
        Respuesta::pagina(
            "Inicio",
            "<a href=\"http://muerto.invalid/1\">1</a> <a href=\"http://muerto.invalid/2\">2</a> \
             <a href=\"http://muerto.invalid/3\">3</a> <a href=\"http://muerto.invalid/4\">4</a>",
        ),
    )])
    .await;
    let tmp = Temporal::new("dns");
    let mut job = job(&servidor);
    job.limits.follow_external = true;
    // Sin entradas: la semilla es una IP literal y no pregunta; `muerto.invalid` no resuelve.
    let lookup: Arc<dyn Lookup> = Arc::new(StaticLookup::new([]));
    let outcome =
        engine::run_http_with_lookup(job, &tmp.store(), lookup).await.expect("rastrear");

    let conn = abrir(&tmp.store());
    for n in 1..=4 {
        let (_, _, kind) = fila(&conn, &format!("http://muerto.invalid/{n}"));
        assert_eq!(kind.as_deref(), Some("dns"), "la página {n} falló por DNS");
    }
    assert_eq!(outcome.metrics.throttle_reductions, 0, "el DNS no es carga del servidor");
}
