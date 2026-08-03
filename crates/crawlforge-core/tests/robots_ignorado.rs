//! `--ignore-robots` rastrea lo bloqueado, pero **marca** lo que el `robots.txt` prohíbe.
//!
//! El defecto que fija este fichero: `blocked_by_robots` nacía a `false` en
//! `engine::process_url` y nunca cambiaba, así que toda su cañería —`FetchedOutcome` →
//! `PageContext` → `IndexabilityInput`— estaba muerta. Con `ignore_robots`, el `robots.txt`
//! ni siquiera se pedía: el usuario pedía a propósito ver lo que Google no ve, y el informe
//! no podía decirle cuáles de esas páginas estaban bloqueadas.
//!
//! La semántica que se afirma aquí: con `ignore_robots` las reglas del host se cargan
//! igualmente, no para excluir sino para marcar (`indexability_reason = 'robots'`, la causa
//! raíz más prioritaria de `evaluate_indexability`); el `Crawl-delay` **no** se aplica,
//! porque ignorar el fichero es ignorarlo entero; y sin `ignore_robots` nada cambia: lo
//! bloqueado sigue excluido sin pedirse.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::{
    engine,
    job::{CrawlJob, CrawlMode},
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
            .join(format!("crawlforge-igrob-{}-{nombre}", std::process::id()));
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

/// Estado e indexabilidad de una URL, tal como quedaron en el fichero.
fn fila(
    conn: &Connection,
    url: &str,
) -> (String, Option<String>, Option<i64>, Option<String>) {
    conn.query_row(
        "SELECT u.crawl_state, u.exclusion_reason, p.is_indexable, p.indexability_reason
         FROM urls u LEFT JOIN pages p ON p.url_id = u.id
         WHERE u.url = ?1",
        [url],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .expect("la URL debería estar en el fichero")
}

/// El sitio de estos tests: una zona prohibida en robots.txt, enlazada desde la portada.
fn rutas_del_sitio() -> Vec<(&'static str, Respuesta)> {
    vec![
        ("/robots.txt", Respuesta::texto("User-agent: *\nDisallow: /privado/\n")),
        (
            "/",
            Respuesta::pagina(
                "Inicio",
                "<p><a href=\"/privado/secreto\">Secreto</a>\
                 <a href=\"/publico/normal\">Normal</a></p>",
            ),
        ),
        ("/privado/secreto", Respuesta::pagina("Secreto", "<p>Contenido oculto.</p>")),
        ("/publico/normal", Respuesta::pagina("Normal", "<p>Contenido público.</p>")),
    ]
}

#[tokio::test]
async fn ignorar_robots_rastrea_lo_bloqueado_y_lo_marca_como_no_indexable() {
    let servidor = ServidorDePruebas::arrancar(&rutas_del_sitio()).await;

    let tmp = Temporal::new("marca");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.ignore_robots = true;
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 3, "con ignore_robots se rastrea todo");
    assert_eq!(outcome.metrics.urls_excluded, 0, "ignorar robots no excluye nada");

    // El robots.txt se pide igualmente —una vez por host, cacheado—: sin leerlo no se puede
    // decir qué está bloqueado, que es justo lo que el usuario vino a ver.
    assert_eq!(servidor.peticiones("/robots.txt"), 1);

    let conn = abrir(&tmp.store());

    // La página prohibida se rastreó, y el informe dice la verdad: bloqueada por robots.txt,
    // que gana a cualquier otro motivo de no indexabilidad.
    let (estado, exclusion, indexable, motivo) = fila(&conn, &servidor.url("/privado/secreto"));
    assert_eq!(estado, "done", "la página bloqueada se rastrea bajo ignore_robots");
    assert_eq!(exclusion, None);
    assert_eq!(indexable, Some(0), "una página bloqueada no es indexable");
    assert_eq!(motivo.as_deref(), Some("robots"));

    // Y lo que el robots.txt no prohíbe no se mancha.
    let (estado, _, indexable, motivo) = fila(&conn, &servidor.url("/publico/normal"));
    assert_eq!(estado, "done");
    assert_eq!(indexable, Some(1));
    assert_eq!(motivo, None);
}

#[tokio::test]
async fn ignorar_robots_ignora_tambien_el_crawl_delay() {
    // Ignorar el robots.txt es ignorarlo entero: la exclusión y también el ritmo. Si el
    // Crawl-delay se aplicara, tres URLs a 10 s serían más de 20 s de rastreo; la cota de
    // abajo es holgada y con el retardo colado se pasa de largo seguro.
    let servidor = ServidorDePruebas::arrancar(&[
        (
            "/robots.txt",
            Respuesta::texto("User-agent: *\nCrawl-delay: 10\nDisallow: /privado/\n"),
        ),
        ("/a", Respuesta::pagina("A", "<p>a</p>")),
        ("/b", Respuesta::pagina("B", "<p>b</p>")),
        ("/c", Respuesta::pagina("C", "<p>c</p>")),
    ])
    .await;

    let tmp = Temporal::new("sin-delay");
    let mut job = CrawlJob::http(servidor.base());
    job.mode = CrawlMode::List {
        urls: vec![servidor.url("/a"), servidor.url("/b"), servidor.url("/c")],
    };
    job.limits.ignore_robots = true;
    job.discover_sitemaps = false;

    let inicio = Instant::now();
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let transcurrido = inicio.elapsed();

    assert_eq!(outcome.metrics.urls_fetched, 3);
    assert!(
        transcurrido < Duration::from_secs(8),
        "el Crawl-delay se aplicó bajo ignore_robots: {transcurrido:?}"
    );
}

#[tokio::test]
async fn sin_ignorar_robots_lo_bloqueado_sigue_excluido_sin_pedirse() {
    // Guarda de no regresión: el arreglo del marcado no puede cambiar el camino normal, en el
    // que respetar robots.txt significa no descargar la URL.
    let servidor = ServidorDePruebas::arrancar(&rutas_del_sitio()).await;

    let tmp = Temporal::new("respetado");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 2, "la portada y la pública");
    assert_eq!(outcome.metrics.urls_excluded, 1, "la prohibida");
    assert_eq!(servidor.peticiones("/privado/secreto"), 0, "respetar es no pedirla");

    let conn = abrir(&tmp.store());
    let (estado, exclusion, indexable, _) = fila(&conn, &servidor.url("/privado/secreto"));
    assert_eq!(estado, "excluded");
    assert_eq!(exclusion.as_deref(), Some("robots"));
    assert_eq!(indexable, None, "una URL no descargada no tiene fila en pages");
}

#[tokio::test]
async fn un_robots_ilegible_bajo_ignore_robots_no_marca_nada_ni_tumba_el_rastreo() {
    // Si /robots.txt responde 500, no se puede afirmar que nada esté bloqueado: se cae a
    // permitir todo, se rastrea con normalidad y ninguna página queda marcada.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/robots.txt", Respuesta::error(500)),
        ("/", Respuesta::pagina("Inicio", "<p>Sin enlaces.</p>")),
    ])
    .await;

    let tmp = Temporal::new("robots-caido");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.ignore_robots = true;
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 1);

    let conn = abrir(&tmp.store());
    let (estado, _, indexable, motivo) = fila(&conn, &servidor.base());
    assert_eq!(estado, "done");
    assert_eq!(indexable, Some(1), "sin robots legible no se marca nada como bloqueado");
    assert_eq!(motivo, None);
}
