//! El descubrimiento de sitemaps repone de uno en uno, no por tandas.
//!
//! Es un fallo ya conocido reaparecido en este camino: cuando la
//! ronda se llenaba, se vaciaba **entera** antes de lanzar nada nuevo, así que cada tanda costaba
//! lo que su petición más lenta. Con 1.179 sitemaps (un medio real declara esa cantidad) y
//! concurrencia 8, eso son 148 tandas.
//!
//! El sitio de este test lo reproduce a escala: 24 sitemaps hijos, uno lento (600 ms) por cada
//! tanda de 8 y el resto rápidos (10 ms). Por tandas, cada lento arrastra la suya: ≥ 1,8 s solo
//! de descubrimiento. Con reposición continua los lentos se solapan: ~0,6 s. El umbral de 1,5 s
//! separa los dos mundos con margen por ambos lados, y como el tiempo lo dominan los `sleep` del
//! servidor y no la CPU, la medida vale también en debug.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;
use std::time::{Duration, Instant};
use support::servidor::{Respuesta, ServidorDePruebas};

const HIJOS: usize = 24;
const CONCURRENCIA: u8 = 8;

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-sm-{}-{nombre}", std::process::id()));
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

#[tokio::test]
async fn un_sitemap_lento_no_arrastra_a_su_tanda() {
    let servidor = ServidorDePruebas::arrancar_con_puerto(|puerto| {
        let mut rutas = Vec::new();

        let hijos: String = (0..HIJOS)
            .map(|i| {
                format!("<sitemap><loc>http://127.0.0.1:{puerto}/sm/{i}.xml</loc></sitemap>")
            })
            .collect();
        rutas.push((
            "/sitemap.xml".to_string(),
            Respuesta::xml(format!(
                r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{hijos}</sitemapindex>"#
            )),
        ));

        for i in 0..HIJOS {
            // Un lento por cada tanda de `CONCURRENCIA`; el resto, rápidos.
            let retardo = if i % (CONCURRENCIA as usize) == 0 {
                Duration::from_millis(600)
            } else {
                Duration::from_millis(10)
            };
            rutas.push((
                format!("/sm/{i}.xml"),
                Respuesta::xml(format!(
                    r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>http://127.0.0.1:{puerto}/p/{i}</loc></url></urlset>"#
                ))
                .con_retardo(retardo),
            ));
            rutas.push((
                format!("/p/{i}"),
                Respuesta::pagina(&format!("Página {i}"), "<p>Contenido.</p>"),
            ));
        }
        rutas.push(("/".to_string(), Respuesta::pagina("Inicio", "<p>Hola.</p>")));
        rutas
    })
    .await;

    let tmp = Temporal::new("indice-grande");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.concurrency_per_host = CONCURRENCIA;

    let inicio = Instant::now();
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let transcurrido = inicio.elapsed();
    println!(
        "descubrimiento + rastreo: {transcurrido:?} (setup+teardown {:?})",
        outcome.metrics.setup_and_teardown
    );

    // Lo funcional primero: el índice y los 24 hijos quedan registrados, y las URLs que
    // declaran entran al rastreo marcadas como del sitemap.
    let conn = Connection::open_with_flags(
        tmp.store(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("abrir el fichero de rastreo");
    let hijos_validos: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sitemaps WHERE is_valid = 1 AND is_index = 0",
            [],
            |r| r.get(0),
        )
        .expect("contar sitemaps hijos");
    assert_eq!(hijos_validos, HIJOS as i64, "faltan sitemaps hijos en la tabla");
    let en_sitemap: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls WHERE in_sitemap = 1", [], |r| r.get(0))
        .expect("contar urls de sitemap");
    assert_eq!(en_sitemap, HIJOS as i64, "faltan URLs declaradas por los sitemaps");

    // Y el tiempo: por tandas eran ≥ 1,8 s solo de descubrimiento; continuo, ~0,6 s.
    assert!(
        transcurrido < Duration::from_millis(1_500),
        "el descubrimiento vuelve a ir por tandas: {transcurrido:?}"
    );
}
