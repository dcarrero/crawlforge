//! El perímetro de los **documentos** de sitemap (revisión 2026-08-01, puntos 1.2 y 1.5).
//!
//! El arreglo anterior filtró las URLs *declaradas dentro* de un sitemap; los documentos en sí
//! —las líneas `Sitemap:` del `robots.txt` y los hijos de un `<sitemapindex>`— se seguían
//! pidiendo con `fetch()` a pelo: sin comprobar host, sin esquema, sin el `robots.txt` del
//! destino y sin `Throttle`. Un `robots.txt` ajeno podía apuntar la aplicación contra
//! `169.254.169.254` o contra un tercero, con la IP del usuario.
//!
//! Y el presupuesto de `max_urls` se aplicaba **después** de que el descubrimiento devolviera
//! el vector completo: un índice grande acumulaba millones de `Url` en memoria antes de
//! rastrear la primera página.
//!
//! El servidor escucha solo en `127.0.0.1`, pero `localhost` es *otro host* para el motor
//! (`normalize::is_internal` compara nombres): las rutas «fuera» se declaran con `localhost`
//! y, si el motor las pidiera, la petición llegaría igualmente al contador por el reintento
//! IPv4 del cliente. Cero peticiones significa que el filtro actuó antes de abrir el socket.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;
use support::servidor::{Respuesta, ServidorDePruebas};

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-perim-{}-{nombre}", std::process::id()));
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
async fn los_documentos_de_sitemap_respetan_host_y_robots_del_destino() {
    let servidor = ServidorDePruebas::arrancar_con_puerto(|puerto| {
        vec![
            // El `robots.txt` anuncia un sitemap en otro host —el vector del ataque— y otro
            // en el propio sitio, y bloquea una ruta concreta.
            (
                "/robots.txt".to_string(),
                Respuesta::texto(format!(
                    "User-agent: *\nDisallow: /bloqueado.xml\n\
                     Sitemap: http://localhost:{puerto}/anunciado-fuera.xml\n\
                     Sitemap: http://127.0.0.1:{puerto}/interno.xml\n"
                )),
            ),
            // El índice interno declara un hijo legítimo, uno en otro host y uno que el
            // propio `robots.txt` del sitio prohíbe.
            (
                "/interno.xml".to_string(),
                Respuesta::xml(format!(
                    r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                       <sitemap><loc>http://127.0.0.1:{puerto}/hijo-dentro.xml</loc></sitemap>
                       <sitemap><loc>http://localhost:{puerto}/hijo-fuera.xml</loc></sitemap>
                       <sitemap><loc>http://127.0.0.1:{puerto}/bloqueado.xml</loc></sitemap>
                       </sitemapindex>"#
                )),
            ),
            (
                "/hijo-dentro.xml".to_string(),
                Respuesta::xml(format!(
                    r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                       <url><loc>http://127.0.0.1:{puerto}/declarada</loc></url></urlset>"#
                )),
            ),
            // Los canarios: si alguno registra una petición, el perímetro no actuó.
            (
                "/anunciado-fuera.xml".to_string(),
                Respuesta::xml(format!(
                    r#"<urlset><url><loc>http://localhost:{puerto}/pwned-1</loc></url></urlset>"#
                )),
            ),
            (
                "/hijo-fuera.xml".to_string(),
                Respuesta::xml(format!(
                    r#"<urlset><url><loc>http://localhost:{puerto}/pwned-2</loc></url></urlset>"#
                )),
            ),
            (
                "/bloqueado.xml".to_string(),
                Respuesta::xml(format!(
                    r#"<urlset><url><loc>http://127.0.0.1:{puerto}/secreto</loc></url></urlset>"#
                )),
            ),
            ("/".to_string(), Respuesta::pagina("Inicio", "<p>Hola.</p>")),
            ("/declarada".to_string(), Respuesta::pagina("Declarada", "<p>Contenido.</p>")),
        ]
    })
    .await;

    let tmp = Temporal::new("perimetro");
    let job = CrawlJob::http(servidor.base());
    engine::run(job, &tmp.store()).await.expect("rastrear");

    // Ninguna petición a los canarios: el filtro corta antes de abrir el socket.
    assert_eq!(
        servidor.peticiones("/anunciado-fuera.xml"),
        0,
        "una línea Sitemap: hacia otro host no debe descargarse"
    );
    assert_eq!(
        servidor.peticiones("/hijo-fuera.xml"),
        0,
        "un hijo de sitemapindex hacia otro host no debe descargarse"
    );
    assert_eq!(
        servidor.peticiones("/bloqueado.xml"),
        0,
        "un sitemap que el robots.txt de su host prohíbe no debe descargarse"
    );

    // Y el descubrimiento legítimo sigue intacto: el hijo interno se lee y su URL entra
    // marcada como declarada por el sitio.
    assert_eq!(servidor.peticiones("/hijo-dentro.xml"), 1, "el hijo interno sí se lee");
    let conn = Connection::open_with_flags(
        tmp.store(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("abrir el fichero de rastreo");
    let declarada: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE path = '/declarada' AND in_sitemap = 1",
            [],
            |r| r.get(0),
        )
        .expect("contar la declarada");
    assert_eq!(declarada, 1, "la URL del sitemap interno entra al rastreo");
}

#[tokio::test]
async fn el_presupuesto_corta_el_descubrimiento_no_solo_las_filas() {
    const HIJOS: usize = 20;
    const URLS_POR_HIJO: usize = 5;
    const PRESUPUESTO: u64 = 5;

    let servidor = ServidorDePruebas::arrancar_con_puerto(|puerto| {
        let mut rutas = Vec::new();
        let hijos: String = (0..HIJOS)
            .map(|i| format!("<sitemap><loc>http://127.0.0.1:{puerto}/sm/{i}.xml</loc></sitemap>"))
            .collect();
        rutas.push((
            "/sitemap.xml".to_string(),
            Respuesta::xml(format!(
                r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{hijos}</sitemapindex>"#
            )),
        ));
        for i in 0..HIJOS {
            let urls: String = (0..URLS_POR_HIJO)
                .map(|j| format!("<url><loc>http://127.0.0.1:{puerto}/p/{i}-{j}</loc></url>"))
                .collect();
            rutas.push((
                format!("/sm/{i}.xml"),
                Respuesta::xml(format!(
                    r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{urls}</urlset>"#
                )),
            ));
        }
        rutas.push(("/".to_string(), Respuesta::pagina("Inicio", "<p>Hola.</p>")));
        rutas
    })
    .await;

    let tmp = Temporal::new("presupuesto");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.max_urls = Some(PRESUPUESTO);

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");

    // Con el presupuesto lleno, el descubrimiento deja de descargar hijos: solo caen los
    // que ya estaban en vuelo. Sin el corte interno se descargaban los 20 —y con un índice
    // real de 200 hijos de 50 MB, ~250 M de `Url` en memoria antes de rastrear nada.
    let hijos_descargados: usize =
        (0..HIJOS).map(|i| servidor.peticiones(&format!("/sm/{i}.xml"))).sum();
    assert!(
        hijos_descargados <= 10,
        "el descubrimiento siguió descargando con el presupuesto lleno: {hijos_descargados} de {HIJOS} hijos"
    );

    // La semántica del truncado se conserva: el presupuesto se agotó con URLs de sitemap
    // por declarar, y el rastreo lo dice.
    assert_eq!(
        outcome.truncated,
        Some(engine::TruncationReason::MaxUrls),
        "el corte del descubrimiento no puede silenciar el truncado por max_urls"
    );
}
