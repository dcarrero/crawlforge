//! Un enlace a otro dominio sigue siendo de otro dominio aunque su ruta exista en el `dist/`.
//!
//! El modo `filesystem` unifica `/about`, `/about/` y `/about/index.html` porque un servidor
//! estático las sirve del mismo fichero. Esa unificación se hacía mirando **solo la ruta**, así
//! que cualquier URL ajena cuya ruta existiera dentro del `dist/` acababa canonicalizada al
//! sitio auditado. El caso no era exótico: `https://cualquier-dominio.com/` tiene ruta `/`, que
//! resuelve a `index.html`, de modo que **todo enlace a la portada de otro dominio se registraba
//! como enlace a la portada del sitio auditado**.
//!
//! Lo encontró auditar nuestra propia web el 2026-08-08, no un test: los botones de compartir de
//! las entradas del devlog enlazan a `chatgpt.com/?q=…`, `grok.com/?q=…` y `perplexity.ai/?q=…`,
//! y las doce entradas salieron con `INDEX-NOFOLLOW-INTERNAL`. Ninguno de esos enlaces es
//! interno; lo que comparten es que su ruta es `/`.
//!
//! El daño real iba más allá del falso positivo. Esos enlaces externos **desaparecían del
//! grafo** —nadie comprobaba su estado, así que `HTTP-404-EXTERNAL` no podía verlos— y a cambio
//! inflaban los enlaces entrantes de la portada, que es una señal que otras reglas usan.
//!
//! Confirmación del rojo: los dos tests fallan si se quita la comparación de origen de
//! `FilesystemFetcher::resolve_inner`. Comprobado antes de darlos por buenos.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;

/// Directorio temporal que se limpia solo. Mismo patrón que `patrones_de_url.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("crawlforge-ext-{}-{nombre}", std::process::id()));
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

/// Los destinos de los enlaces salientes de una página, con su marca de interno.
fn destinos(conn: &Connection, desde: &str) -> Vec<(String, i64)> {
    let mut stmt = conn
        .prepare(
            "SELECT d.url, d.is_internal
               FROM links l
               JOIN urls d ON d.id = l.to_url_id
               JOIN urls o ON o.id = l.from_url_id
              WHERE o.url = ?1
              ORDER BY l.position",
        )
        .expect("preparar la consulta de enlaces");
    stmt.query_map([desde], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("consultar enlaces")
        .map(|r| r.expect("leer fila"))
        .collect()
}

/// Cinco formas de enlazar fuera, y las cuatro que fallaban tenían la ruta vacía.
#[tokio::test]
async fn un_enlace_a_la_raiz_de_otro_dominio_no_es_la_portada_propia() {
    let tmp = Temporal::new("raiz");
    tmp.write(
        "site/index.html",
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8">
        <title>Cinco maneras de enlazar fuera de casa</title>
        <meta name="description" content="Las cuatro que fallaban compartían tener la ruta vacía."></head>
        <body><h1>Enlaces</h1><p>
        <a href="https://ejemplo-a.com/?q=1">ruta vacía y query</a>
        <a href="https://ejemplo-b.com?q=1">sin barra y query</a>
        <a href="https://ejemplo-c.com/ruta?q=1">con ruta</a>
        <a href="https://ejemplo-d.com/">solo la barra</a>
        <a href="https://ejemplo-e.com/?utm_source=boletin">el caso de todos los días</a>
        </p></body></html>"#,
    );

    let mut job = CrawlJob::filesystem(tmp.path.join("site"), "https://propio.local/");
    // Sin sonda de externas: aquí se mide a dónde resuelve el enlace, no si el destino responde.
    job.limits.check_external = false;
    engine::run(job, &tmp.store()).await.expect("auditar el dist");

    let conn = abrir(&tmp.store());
    let salientes = destinos(&conn, "https://propio.local/");

    assert_eq!(salientes.len(), 5, "los cinco enlaces tienen que quedar registrados");
    for (url, interno) in &salientes {
        assert_eq!(
            *interno, 0,
            "{url} es de otro dominio y se marcó como interno",
        );
        assert!(
            !url.starts_with("https://propio.local"),
            "{url} se resolvió contra el sitio auditado en vez de contra su propio host",
        );
    }

    // Y el destino concreto, no solo que no sea el propio: una resolución que perdiera el query
    // pasaría la comprobación de arriba y seguiría siendo un enlace distinto del que hay escrito.
    assert!(
        salientes.iter().any(|(u, _)| u == "https://ejemplo-a.com/?q=1"),
        "el query de la URL externa debe conservarse: {salientes:?}",
    );
}

/// La colisión no se limita a `/`: cualquier ruta que exista en el `dist/` valía.
#[tokio::test]
async fn una_ruta_ajena_que_coincide_con_una_del_dist_sigue_siendo_ajena() {
    let tmp = Temporal::new("colision");
    tmp.write(
        "site/index.html",
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8">
        <title>Una ruta que existe en los dos sitios</title>
        <meta name="description" content="El fichero propio no debe capturar la URL ajena."></head>
        <body><h1>Colisión</h1><p>
        <a href="/blog/">el blog de casa</a>
        <a href="https://ajeno.example/blog/">el blog de otro</a>
        </p></body></html>"#,
    );
    // El mismo camino existe dentro del `dist/`, que es lo que hacía que la URL ajena lo capturara.
    tmp.write(
        "site/blog/index.html",
        r#"<!doctype html><html lang="es"><head><meta charset="utf-8">
        <title>El blog de casa, con su propio título</title>
        <meta name="description" content="Existe de verdad, y por eso servía de imán."></head>
        <body><h1>Blog</h1><p>Una entrada.</p></body></html>"#,
    );

    let mut job = CrawlJob::filesystem(tmp.path.join("site"), "https://propio.local/");
    job.limits.check_external = false;
    engine::run(job, &tmp.store()).await.expect("auditar el dist");

    let conn = abrir(&tmp.store());
    let salientes = destinos(&conn, "https://propio.local/");

    assert_eq!(
        salientes,
        vec![
            ("https://propio.local/blog/".to_string(), 1),
            ("https://ajeno.example/blog/".to_string(), 0),
        ],
        "el enlace propio se canonicaliza y el ajeno se queda donde estaba",
    );
}
