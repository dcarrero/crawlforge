//! `max_duration` se respeta aunque haya esperas largas en vuelo.
//!
//! El defecto que fija este fichero: el presupuesto de tiempo solo se comprobaba **después** de
//! que una tarea terminara, así que una espera larga dentro de la tarea —el `sleep` de un
//! `Crawl-delay`, los reintentos con backoff— retenía el corte. Medido antes del arreglo: un
//! `robots.txt` con `Crawl-delay: 600` (recortado a 30 s por `MAX_CRAWL_DELAY`) y
//! `max_duration = 1 s` terminaban en ~30 s. El usuario pedía un segundo y obtenía treinta,
//! sin más salida que matar el proceso.
//!
//! El arreglo: el deadline se comprueba antes de rellenar el pool y las esperas en vuelo se
//! cancelan con `tokio::select!` contra el plazo. El rastreo queda marcado como truncado con
//! `max_duration`, igual que los otros topes.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::{
    engine::{self, TruncationReason},
    job::CrawlJob,
};
use rusqlite::Connection;
use std::time::{Duration, Instant};
use support::servidor::{Respuesta, ServidorDePruebas};

/// Directorio temporal que se limpia solo. Mismo patrón que `pipeline.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-dur-{}-{nombre}", std::process::id()));
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

fn abrir(path: &std::path::Path) -> Connection {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("abrir el fichero de rastreo")
}

#[tokio::test]
async fn el_deadline_corta_un_crawl_delay_largo() {
    // Crawl-delay: 600 se recorta a 30 s, pero 30 s siguen siendo treinta veces el presupuesto.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/robots.txt", Respuesta::texto("User-agent: *\nCrawl-delay: 600\n")),
        ("/", Respuesta::pagina("Inicio", "<p><a href=\"/otra\">Otra</a></p>")),
        ("/otra", Respuesta::pagina("Otra", "<p>Contenido.</p>")),
    ])
    .await;

    let tmp = Temporal::new("crawl-delay");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.max_duration = Some(Duration::from_secs(1));
    // Sin descubrimiento: este test aísla el bucle de rastreo; el de sitemaps tiene el suyo.
    job.discover_sitemaps = false;

    let inicio = Instant::now();
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let transcurrido = inicio.elapsed();

    // El margen es holgado a propósito (el cierre escribe y pasa reglas de conjunto), pero
    // queda muy por debajo de los ~30 s que tardaba antes del arreglo.
    assert!(
        transcurrido < Duration::from_secs(10),
        "se pidió 1 s de presupuesto y el rastreo tardó {transcurrido:?}"
    );
    assert_eq!(outcome.truncated, Some(TruncationReason::MaxDuration));

    // El fichero cuenta lo mismo que el resultado: truncado y por qué.
    let conn = abrir(&tmp.store());
    let (truncated, reason): (i64, String) = conn
        .query_row("SELECT truncated, truncated_reason FROM crawl_meta", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("leer crawl_meta");
    assert_eq!(truncated, 1);
    assert_eq!(reason, "max_duration");
}

#[tokio::test]
async fn el_deadline_corta_tambien_el_descubrimiento_de_sitemaps() {
    // Un sitemap que tarda 20 s en responder no debe retener un presupuesto de 1 s.
    let servidor = ServidorDePruebas::arrancar(&[
        (
            "/sitemap.xml",
            Respuesta::xml(
                r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>"#,
            )
            .con_retardo(Duration::from_secs(20)),
        ),
        ("/", Respuesta::pagina("Inicio", "<p>Hola.</p>")),
    ])
    .await;

    let tmp = Temporal::new("sitemap-lento");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.max_duration = Some(Duration::from_secs(1));

    let inicio = Instant::now();
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let transcurrido = inicio.elapsed();

    assert!(
        transcurrido < Duration::from_secs(10),
        "el descubrimiento retuvo el corte: {transcurrido:?}"
    );
    assert_eq!(outcome.truncated, Some(TruncationReason::MaxDuration));
}
