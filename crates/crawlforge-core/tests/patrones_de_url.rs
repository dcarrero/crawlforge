//! Los patrones de include/exclude, demostrados rastreando de verdad.
//!
//! Lo que estos tests garantizan, en orden de importancia:
//!
//! 1. **Una URL excluida no se rastrea pero queda registrada** con su motivo (`pattern`).
//!    La alternativa —que desaparezca en silencio— es indistinguible de un fallo del motor,
//!    y quien excluye media web por error tiene que poder verlo en el informe.
//! 2. **Un patrón inválido es un error antes de empezar**, no un rastreo a medias ni un
//!    fichero a medio crear.
//! 3. `include` restringe de verdad, `exclude` gana cuando los dos casan, y las semillas y
//!    las URLs de sitemap siguen la misma regla que los enlaces.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;
use support::servidor::{Respuesta, ServidorDePruebas};

/// Directorio temporal que se limpia solo. Mismo patrón que `pipeline.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-pat-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear el directorio temporal");
        Self { path }
    }

    fn store(&self) -> std::path::PathBuf {
        self.path.join("crawl.sqlite")
    }

    fn write(&self, relative: &str, contents: &str) {
        let full = self.path.join(relative);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("crear subdirectorio");
        }
        std::fs::write(full, contents).expect("escribir fichero");
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn abrir(path: &std::path::Path) -> Connection {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el fichero de rastreo")
}

fn contar(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).expect("contar")
}

/// `(crawl_state, exclusion_reason)` de una URL, para afirmar que quedó registrada y por qué.
fn estado_de(conn: &Connection, url: &str) -> Option<(String, Option<String>)> {
    conn.query_row(
        "SELECT crawl_state, exclusion_reason FROM urls WHERE url = ?1",
        [url],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}

/// Un `dist/` con portada, dos posts de blog y una página de tienda.
fn sitio_en_disco(tmp: &Temporal) {
    tmp.write(
        "site/index.html",
        r#"<html lang="es"><head><title>Inicio</title></head><body><main><h1>Inicio</h1>
           <a href="/blog/post-1/">Post 1</a>
           <a href="/blog/post-2/">Post 2</a>
           <a href="/tienda/producto/">Producto</a>
           </main></body></html>"#,
    );
    for (ruta, titulo) in [
        ("site/blog/post-1/index.html", "Post 1"),
        ("site/blog/post-2/index.html", "Post 2"),
        ("site/tienda/producto/index.html", "Producto"),
    ] {
        tmp.write(
            ruta,
            &format!(
                r#"<html lang="es"><head><title>{titulo}</title></head>
                   <body><main><h1>{titulo}</h1><p>Texto.</p></main></body></html>"#
            ),
        );
    }
}

// ---------------------------------------------------------------- exclude

#[tokio::test]
async fn una_url_excluida_no_se_rastrea_pero_queda_registrada() {
    // El mismo sitio, con y sin patrones: la diferencia es exactamente lo excluido.
    let sin = Temporal::new("exclude-sin");
    sitio_en_disco(&sin);
    let job = CrawlJob::filesystem(sin.path.join("site"), "https://ejemplo.es/");
    let outcome = engine::run(job, &sin.store()).await.expect("rastrear sin patrones");
    let conn = abrir(&outcome.store_path);
    assert_eq!(contar(&conn, "SELECT COUNT(*) FROM pages"), 4, "sin patrones se audita todo");
    assert_eq!(outcome.metrics.urls_excluded, 0);

    let con = Temporal::new("exclude-con");
    sitio_en_disco(&con);
    let mut job = CrawlJob::filesystem(con.path.join("site"), "https://ejemplo.es/");
    job.limits.exclude_patterns = vec!["/blog/".to_string()];
    let outcome = engine::run(job, &con.store()).await.expect("rastrear con exclude");
    let conn = abrir(&outcome.store_path);

    // No se rastrean: ni página ni petición.
    assert_eq!(contar(&conn, "SELECT COUNT(*) FROM pages"), 2, "portada y tienda, sin blog");
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM pages p JOIN urls u ON u.id = p.url_id
                       WHERE u.url LIKE '%/blog/%'"),
        0,
        "ninguna página del blog debe haberse parseado"
    );

    // Pero quedan registradas, con su motivo: el informe puede decir «tú lo excluiste».
    for url in ["https://ejemplo.es/blog/post-1/", "https://ejemplo.es/blog/post-2/"] {
        assert_eq!(
            estado_de(&conn, url),
            Some(("excluded".to_string(), Some("pattern".to_string()))),
            "{url} debe constar como excluida por patrón"
        );
    }
    assert_eq!(outcome.metrics.urls_excluded, 2, "las métricas cuadran con las filas");
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM urls
                       WHERE crawl_state = 'excluded' AND exclusion_reason = 'pattern'"),
        2,
        "el bloque de exclusiones del resumen sale de esta consulta"
    );
}

// ---------------------------------------------------------------- include

#[tokio::test]
async fn el_include_restringe_pero_la_semilla_http_se_rastrea_igual() {
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/blog/a", "/blog/b", "/tienda/x"])),
        ("/blog/a", Respuesta::pagina("A", "<p>Texto.</p>")),
        ("/blog/b", Respuesta::pagina("B", "<p>Texto.</p>")),
        ("/tienda/x", Respuesta::pagina("X", "<p>Texto.</p>")),
    ])
    .await;

    let tmp = Temporal::new("include");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;
    job.limits.include_patterns = vec!["/blog/".to_string()];
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let conn = abrir(&outcome.store_path);

    // La semilla no casa con `/blog/` y aun así se rastrea: el rastreo tiene que poder
    // empezar (es lo que hace Screaming Frog con la start URL).
    assert_eq!(
        estado_de(&conn, &servidor.base()).map(|(s, _)| s),
        Some("done".to_string()),
        "la semilla se rastrea aunque no case con el include"
    );
    assert_eq!(estado_de(&conn, &servidor.url("/blog/a")).map(|(s, _)| s),
               Some("done".to_string()));
    assert_eq!(estado_de(&conn, &servidor.url("/blog/b")).map(|(s, _)| s),
               Some("done".to_string()));

    // Lo que no casa queda excluido, registrado y **sin pedirse**.
    assert_eq!(
        estado_de(&conn, &servidor.url("/tienda/x")),
        Some(("excluded".to_string(), Some("pattern".to_string())))
    );
    assert_eq!(servidor.peticiones("/tienda/x"), 0, "una excluida no genera ni una petición");
    assert_eq!(outcome.metrics.urls_excluded, 1);
}

// ---------------------------------------------------------------- include + exclude

#[tokio::test]
async fn cuando_una_url_casa_con_los_dos_gana_el_exclude() {
    // «Todo el blog menos lo privado»: la combinación que da sentido a la precedencia.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/blog/a", "/blog/privado/x", "/tienda/x"])),
        ("/blog/a", Respuesta::pagina("A", "<p>Texto.</p>")),
        ("/blog/privado/x", Respuesta::pagina("Privado", "<p>Texto.</p>")),
        ("/tienda/x", Respuesta::pagina("X", "<p>Texto.</p>")),
    ])
    .await;

    let tmp = Temporal::new("ambos");
    let mut job = CrawlJob::http(servidor.base());
    job.discover_sitemaps = false;
    job.limits.include_patterns = vec!["/blog/".to_string()];
    job.limits.exclude_patterns = vec!["/blog/privado/".to_string()];
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let conn = abrir(&outcome.store_path);

    assert_eq!(estado_de(&conn, &servidor.url("/blog/a")).map(|(s, _)| s),
               Some("done".to_string()), "dentro del include y fuera del exclude: se rastrea");
    assert_eq!(
        estado_de(&conn, &servidor.url("/blog/privado/x")),
        Some(("excluded".to_string(), Some("pattern".to_string()))),
        "casa con los dos: el exclude gana"
    );
    assert_eq!(
        estado_de(&conn, &servidor.url("/tienda/x")),
        Some(("excluded".to_string(), Some("pattern".to_string()))),
        "fuera del include: excluida"
    );
    assert_eq!(servidor.peticiones("/blog/privado/x"), 0);
    assert_eq!(servidor.peticiones("/tienda/x"), 0);
    assert_eq!(outcome.metrics.urls_excluded, 2);
}

// ---------------------------------------------------------------- sitemaps

#[tokio::test]
async fn las_urls_de_sitemap_siguen_la_misma_regla_que_los_enlaces() {
    // Que el sitio declare una URL en su sitemap no reactiva un exclude del usuario.
    let servidor = ServidorDePruebas::arrancar_con_puerto(|puerto| {
        vec![
            (
                "/sitemap.xml".to_string(),
                Respuesta::xml(format!(
                    r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                       <url><loc>http://127.0.0.1:{puerto}/alfa</loc></url>
                       <url><loc>http://127.0.0.1:{puerto}/vetada</loc></url>
                       </urlset>"#
                )),
            ),
            ("/".to_string(), Respuesta::pagina("Inicio", "<p>Sin enlaces.</p>")),
            ("/alfa".to_string(), Respuesta::pagina("Alfa", "<p>Texto.</p>")),
            ("/vetada".to_string(), Respuesta::pagina("Vetada", "<p>Texto.</p>")),
        ]
    })
    .await;

    let tmp = Temporal::new("sitemap");
    let mut job = CrawlJob::http(servidor.base());
    job.limits.exclude_patterns = vec!["/vetada".to_string()];
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let conn = abrir(&outcome.store_path);

    assert_eq!(estado_de(&conn, &servidor.url("/alfa")).map(|(s, _)| s),
               Some("done".to_string()), "lo declarado y no excluido se rastrea");
    assert_eq!(
        estado_de(&conn, &servidor.url("/vetada")),
        Some(("excluded".to_string(), Some("pattern".to_string())))
    );
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM urls
                       WHERE exclusion_reason = 'pattern' AND in_sitemap = 1"),
        1,
        "la exclusión conserva que la URL venía del sitemap"
    );
    assert_eq!(servidor.peticiones("/vetada"), 0, "excluida: ni una petición");
    assert!(outcome.metrics.urls_excluded >= 1);
}

// ---------------------------------------------------------------- semillas de lista

#[tokio::test]
async fn las_semillas_de_una_lista_siguen_la_misma_regla() {
    // En modo lista las semillas no las tecleó nadie una a una en la terminal: son un fichero
    // importado, y el filtro es la única forma de recortarlo sin editarlo.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", Respuesta::pagina("Inicio", "<p>Texto.</p>")),
        ("/fuera", Respuesta::pagina("Fuera", "<p>Texto.</p>")),
    ])
    .await;

    let tmp = Temporal::new("lista");
    let mut job = CrawlJob::http(servidor.base());
    job.mode = crawlforge_core::job::CrawlMode::List {
        urls: vec![servidor.base(), servidor.url("/fuera")],
    };
    job.discover_sitemaps = false;
    job.limits.exclude_patterns = vec!["/fuera".to_string()];
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    let conn = abrir(&outcome.store_path);

    assert_eq!(estado_de(&conn, &servidor.base()).map(|(s, _)| s), Some("done".to_string()));
    assert_eq!(
        estado_de(&conn, &servidor.url("/fuera")),
        Some(("excluded".to_string(), Some("pattern".to_string())))
    );
    assert_eq!(servidor.peticiones("/fuera"), 0);
    assert_eq!(outcome.metrics.urls_excluded, 1);
}

// ---------------------------------------------------------------- validación

#[tokio::test]
async fn un_patron_invalido_es_un_error_claro_antes_de_empezar() {
    let tmp = Temporal::new("invalido");
    sitio_en_disco(&tmp);
    let mut job = CrawlJob::filesystem(tmp.path.join("site"), "https://ejemplo.es/");
    job.limits.exclude_patterns = vec!["[".to_string()];

    let err = engine::run(job, &tmp.store()).await.expect_err("un corchete sin cerrar no vale");
    let msg = err.to_string();
    assert!(msg.contains("exclude"), "el error dice en qué lista está el patrón: {msg}");
    assert!(msg.contains('['), "y qué patrón es: {msg}");
    assert!(
        !tmp.store().exists(),
        "el error llega antes de crear el fichero de rastreo: no queda nada a medias"
    );
}

/// Portada con enlaces a las rutas que se le pasen. Mismo patrón que `reglas_http.rs`.
fn portada(rutas: &[&str]) -> Respuesta {
    let enlaces: String =
        rutas.iter().map(|r| format!("<p><a href=\"{r}\">{r}</a></p>")).collect();
    Respuesta::pagina("Portada", &format!("<p>Portada del sitio de pruebas.</p>{enlaces}"))
}
