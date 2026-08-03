//! Compatibilidad hacia atrás: un fichero de rastreo escrito por una versión anterior del
//! esquema **se abre con el código actual, migra sin perder datos y sus vistas siguen
//! funcionando**.
//!
//! Es el test que pide el compromiso de compatibilidad de `docs/CONVENTIONS.md §4` y que protege el compromiso de `docs/CONVENTIONS.md`:
//! «un rastreo de hace un año debe seguir abriéndose». No es teórico: el export a XLSX murió
//! con «no such column: truncated» al abrir `fixtures/crawl-500k.sqlite`, que es un fichero de
//! esquema v1 real. Los tests de `store.rs` solo migraban desde cero, así que ningún salto
//! v(N-1)→vN estaba cubierto.
//!
//! Cómo se construye el fichero antiguo: replicando lo que hacía el binario de la versión N —
//! ejecutar las migraciones publicadas 1..=N y estampar sus filas en `schema_version` — y
//! sembrando datos representativos con el SQL que aquel esquema admitía. Las migraciones
//! publicadas no se editan nunca (regla del proyecto), así que este replay es fiel por
//! construcción.
//!
//! Se cubren todos los saltos (v1→v5, v2→v5, …) y no solo el más largo. Hoy es redundante —las
//! migraciones se aplican en secuencia, así que los saltos cortos son sufijos estrictos del
//! largo— pero el bucle cuesta milisegundos y deja cada estado de partida escrito.
//!
//! Ese día llegó: la 005 exige fila en `pages` para que una URL sea huérfana, y la siembra de
//! aquí no la tenía. El estado que sembraba —una URL del sitemap, con 200, sin fila en `pages`—
//! es justo el que el motor produce para una imagen, y era el falso positivo que la 005 arregla.
//! Ahora se siembran los dos casos: la página huérfana de verdad y la imagen que no lo es.

use crawlforge_core::{store, SCHEMA_VERSION};
use rusqlite::Connection;

/// Las mismas migraciones publicadas que aplica el core, replicadas aquí porque la lista de
/// `store.rs` es privada. Si esto se desincroniza, el assert de `MIGRATIONS.len()` de abajo
/// falla en rojo diciendo qué falta.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/001_initial.sql")),
    (2, include_str!("../migrations/002_truncated.sql")),
    (3, include_str!("../migrations/003_orphans_exclude_seed.sql")),
    (4, include_str!("../migrations/004_robots_y_sitemaps.sql")),
    (5, include_str!("../migrations/005_orphans_solo_paginas.sql")),
    (6, include_str!("../migrations/006_indice_html_hash.sql")),
    (7, include_str!("../migrations/007_indice_images_src.sql")),
    (8, include_str!("../migrations/008_indice_unico_resources.sql")),
];

struct Dir {
    path: std::path::PathBuf,
}

impl Dir {
    fn new(nombre: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("crawlforge-mig-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear directorio temporal");
        Self { path }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).expect("contar")
}

/// Construye un fichero tal como lo dejó el binario que escribía el esquema `version`.
fn fichero_en_version(dir: &Dir, version: i64) -> std::path::PathBuf {
    let path = dir.path.join(format!("crawl-v{version}.sqlite"));
    let conn = Connection::open(&path).expect("crear el fichero");
    conn.execute_batch(
        "CREATE TABLE schema_version (version INTEGER NOT NULL, applied_at TEXT NOT NULL);",
    )
    .expect("crear schema_version");
    for (v, sql) in MIGRATIONS.iter().take(version as usize) {
        conn.execute_batch(sql).expect("aplicar la migración publicada");
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
            rusqlite::params![v],
        )
        .expect("estampar la versión");
    }
    sembrar_datos_v1(&conn);
    path
}

/// Datos representativos de un rastreo real, escritos con el SQL que admite el esquema v1
/// (el mínimo común de todas las versiones): la semilla, una página buena, un 404 enlazado,
/// una huérfana del sitemap, una redirección, imágenes, hallazgos y una entrada de búsqueda.
fn sembrar_datos_v1(conn: &Connection) {
    conn.execute_batch(
        r#"
        INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                finished_at, status, config_json, core_version, rules_version,
                                tier_at_runtime)
        VALUES ('c1', 'p1', 'Ejemplo', 'https://ejemplo.es/', 'http',
                '2025-07-30 10:00:00', '2025-07-30 10:05:00', 'done', '{}',
                '0.1.0', '0.1.0', 'pro');

        INSERT INTO urls (id, url, url_hash, scheme, host, path, depth, is_internal,
                          in_sitemap, crawl_state, status_code)
        VALUES
            (1, 'https://ejemplo.es/',          11, 'https', 'ejemplo.es', '/',          0, 1, 1, 'done', 200),
            (2, 'https://ejemplo.es/guia/',     12, 'https', 'ejemplo.es', '/guia/',     1, 1, 1, 'done', 200),
            (3, 'https://ejemplo.es/rota',      13, 'https', 'ejemplo.es', '/rota',      1, 1, 0, 'done', 404),
            (4, 'https://ejemplo.es/huerfana/', 14, 'https', 'ejemplo.es', '/huerfana/', NULL, 1, 1, 'done', 200),
            (5, 'https://ejemplo.es/vieja',     15, 'https', 'ejemplo.es', '/vieja',     1, 1, 0, 'done', 301),
            -- En el sitemap y sin ningún `<a>` que la enlace: es el caso de WordPress que la
            -- migración 005 dejó de reportar como página huérfana.
            (6, 'https://ejemplo.es/foto.webp', 16, 'https', 'ejemplo.es', '/foto.webp', 1, 1, 1, 'done', 200);

        UPDATE urls SET redirect_to = 2, redirect_chain_len = 1 WHERE id = 5;

        INSERT INTO pages (url_id, title, title_len, meta_description, h1, h1_count, h2_count,
                           heading_json, is_indexable, word_count, internal_links_out)
        VALUES
            (1, 'Inicio', 6, 'Portada del sitio', 'Bienvenido', 1, 0, '[]', 1, 120, 2),
            (2, 'Guía de diseño', 14, NULL, 'Guía', 1, 3, '[]', 1, 800, 0),
            (4, 'Huérfana', 8, NULL, 'Huérfana', 1, 0, '[]', 1, 300, 0);

        INSERT INTO links (from_url_id, to_url_id, anchor, is_nofollow, element, region, position)
        VALUES
            (1, 2, 'la guía', 0, 'a', 'main', 0),
            (1, 3, 'enlace roto', 0, 'a', 'main', 1);

        INSERT INTO images (page_url_id, src_url_id, alt, alt_present, in_srcset)
        VALUES (1, 6, NULL, 0, 0);

        INSERT INTO issues (url_id, rule_id, severity, category)
        VALUES (3, 'HTTP-404-INTERNAL', 'critical', 'http'),
               (NULL, 'SITE-NO-SITEMAP', 'medium', 'site');

        INSERT INTO extractions (url_id, name, value, occurrence)
        VALUES (2, 'precio', '19,90', 0);

        INSERT INTO adapter_entities (adapter, entity_type, external_id, url_id, data_json)
        VALUES ('wordpress', 'post', '42', NULL, '{}');

        -- Un fichero Pro antiguo llegaba con su índice de búsqueda poblado.
        INSERT INTO pages_fts (rowid, url, title, meta_description, body_text)
        VALUES (2, 'https://ejemplo.es/guia/', 'Guía de diseño', NULL,
                'El diseño de páginas rápidas empieza por medir.');
        "#,
    )
    .expect("sembrar los datos");
}

/// Lo que no puede cambiar al migrar: ni una fila menos, ni una consulta rota.
fn verificar_datos_y_vistas(conn: &Connection) {
    // Los datos siguen ahí, fila a fila.
    assert_eq!(count(conn, "SELECT COUNT(*) FROM crawl_meta"), 1);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM urls"), 6);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM pages"), 3);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM links"), 2);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM images"), 1);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM issues"), 2);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM extractions"), 1);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM adapter_entities"), 1);

    let base: String = conn
        .query_row("SELECT base_url FROM crawl_meta", [], |r| r.get(0))
        .expect("leer base_url");
    assert_eq!(base, "https://ejemplo.es/");

    let destino: i64 = conn
        .query_row("SELECT redirect_to FROM urls WHERE id = 5", [], |r| r.get(0))
        .expect("leer la redirección");
    assert_eq!(destino, 2, "la redirección resuelta sobrevive a la migración");

    // Las columnas nuevas existen y valen su defecto: es justo la consulta con la que murió
    // el export («no such column: truncated»).
    let (truncated, reason): (i64, Option<String>) = conn
        .query_row("SELECT truncated, truncated_reason FROM crawl_meta", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("leer las columnas de la 002");
    assert_eq!(truncated, 0);
    assert_eq!(reason, None);

    // Las tablas de la 004 existen y se consultan, aunque un fichero antiguo no tenga filas.
    assert_eq!(count(conn, "SELECT COUNT(*) FROM robots_txt"), 0);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM sitemaps"), 0);

    // Las vistas de la UI siguen funcionando sobre los datos antiguos.
    assert_eq!(count(conn, "SELECT COUNT(*) FROM v_indexable_pages"), 3);
    assert_eq!(count(conn, "SELECT COUNT(*) FROM v_issue_summary"), 2);

    let (from_url, to_url): (String, String) = conn
        .query_row("SELECT from_url, to_url FROM v_broken_links", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("leer v_broken_links");
    assert_eq!(from_url, "https://ejemplo.es/");
    assert_eq!(to_url, "https://ejemplo.es/rota");

    // Y con la semántica corregida de la 003 y la 005: la semilla no es huérfana, la imagen
    // del sitemap tampoco —no es una página—, y la huérfana de verdad sí.
    let orphans: Vec<String> = {
        let mut stmt = conn.prepare("SELECT url FROM v_orphans").expect("preparar");
        let rows = stmt.query_map([], |r| r.get(0)).expect("consultar");
        rows.collect::<rusqlite::Result<_>>().expect("recoger")
    };
    assert_eq!(orphans, vec!["https://ejemplo.es/huerfana/"]);

    // El índice de búsqueda de un fichero Pro antiguo sigue respondiendo, tildes incluidas.
    let hallada: i64 = conn
        .query_row(
            "SELECT rowid FROM pages_fts WHERE pages_fts MATCH 'diseno'",
            [],
            |r| r.get(0),
        )
        .expect("buscar en el índice antiguo");
    assert_eq!(hallada, 2);
}

#[test]
fn un_fichero_de_cada_version_anterior_se_abre_migra_y_no_pierde_nada() {
    // Si esto falla, la lista de migraciones del test se ha quedado atrás: añade la nueva al
    // array de arriba y decide si su salto necesita datos de siembra propios.
    assert_eq!(
        MIGRATIONS.len() as i64,
        SCHEMA_VERSION,
        "el test replica las migraciones publicadas y le falta una"
    );

    let dir = Dir::new("saltos");
    for version_antigua in 1..SCHEMA_VERSION {
        let path = fichero_en_version(&dir, version_antigua);

        let conn = store::open_writer(&path).expect("abrir el fichero antiguo con el código actual");
        let v: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .expect("leer la versión");
        assert_eq!(v, SCHEMA_VERSION, "v{version_antigua} debe migrar hasta la actual");
        verificar_datos_y_vistas(&conn);
    }
}

#[test]
fn un_fichero_ya_migrado_no_se_vuelve_a_migrar() {
    // Idempotencia de verdad, no solo «no falla»: reabrir un fichero al día no debe añadir ni
    // una fila a `schema_version` ni tocar las marcas de cuándo se aplicó cada migración.
    let dir = Dir::new("idempotencia");
    let path = fichero_en_version(&dir, 1);

    let registro = |conn: &Connection| -> Vec<(i64, String)> {
        let mut stmt = conn
            .prepare("SELECT version, applied_at FROM schema_version ORDER BY version")
            .expect("preparar");
        let rows =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).expect("consultar");
        rows.collect::<rusqlite::Result<_>>().expect("recoger")
    };

    let tras_migrar = {
        let conn = store::open_writer(&path).expect("primera apertura: migra");
        registro(&conn)
    };
    assert_eq!(
        tras_migrar.len() as i64,
        SCHEMA_VERSION,
        "una fila por migración aplicada, ninguna repetida"
    );

    let tras_reabrir = {
        let conn = store::open_writer(&path).expect("segunda apertura: ya está al día");
        registro(&conn)
    };
    assert_eq!(
        tras_migrar, tras_reabrir,
        "reabrir no re-aplica nada: mismas filas, mismas marcas de tiempo"
    );
}
