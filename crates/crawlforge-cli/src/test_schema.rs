//! Test-only helper: crawl files with the complete published schema, built in one place.
//!
//! Until 2026-08-04 every test module carried its own hand-written migration list, and the
//! lists drifted: most stopped at 004, `report.rs` at 001 — a schema whose `v_orphans` still
//! had the bug that migrations 003 and 005 fix, and without the indexes from 006-008. One
//! list, one guard test, no drift.
//!
//! This module is included by both compilation targets (the library via `lib.rs` and the
//! binary via `main.rs`, where `report.rs` lives), so the guard test runs once per target.
//!
//! The one test that must **not** use this helper is
//! `xlsx.rs::un_rastreo_del_esquema_inicial_se_exporta_igual`: it builds a 001-only file on
//! purpose, because exporting an old crawl file is the behavior under test.

use rusqlite::Connection;
use std::path::Path;

/// Every published migration, in order. Keyed by file name so the guard test can compare the
/// list against the `migrations/` directory on disk.
pub(crate) const MIGRATIONS: &[(&str, &str)] = &[
    ("001_initial.sql", include_str!("../../crawlforge-core/migrations/001_initial.sql")),
    ("002_truncated.sql", include_str!("../../crawlforge-core/migrations/002_truncated.sql")),
    (
        "003_orphans_exclude_seed.sql",
        include_str!("../../crawlforge-core/migrations/003_orphans_exclude_seed.sql"),
    ),
    (
        "004_robots_y_sitemaps.sql",
        include_str!("../../crawlforge-core/migrations/004_robots_y_sitemaps.sql"),
    ),
    (
        "005_orphans_solo_paginas.sql",
        include_str!("../../crawlforge-core/migrations/005_orphans_solo_paginas.sql"),
    ),
    (
        "006_indice_html_hash.sql",
        include_str!("../../crawlforge-core/migrations/006_indice_html_hash.sql"),
    ),
    (
        "007_indice_images_src.sql",
        include_str!("../../crawlforge-core/migrations/007_indice_images_src.sql"),
    ),
    (
        "008_indice_unico_resources.sql",
        include_str!("../../crawlforge-core/migrations/008_indice_unico_resources.sql"),
    ),
];

/// Creates `path` as a crawl file with the complete published schema.
///
/// Mirrors what `crawlforge_core::store::migrate` leaves behind: the migrations plus the
/// `schema_version` bookkeeping — that table is not created by any migration, the store
/// creates it itself before applying them, and a file without it is not what any command
/// receives in production.
pub(crate) fn crawl_file(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("create the crawl file");
    apply_full_schema(&conn);
    conn
}

/// Applies **all** the published migrations plus the `schema_version` rows.
pub(crate) fn apply_full_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE schema_version (version INTEGER NOT NULL, applied_at TEXT NOT NULL);",
    )
    .expect("create schema_version");
    for (name, sql) in MIGRATIONS {
        conn.execute_batch(sql)
            .unwrap_or_else(|e| panic!("apply migration {name}: {e}"));
        let version: i64 = name[..3].parse().unwrap_or_else(|e| {
            panic!("migration file name {name} must start with its number: {e}")
        });
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
            [version],
        )
        .expect("record the schema version");
    }
}

/// The guard: every `.sql` file published in `crawlforge-core/migrations/` must be applied by
/// [`apply_full_schema`], in order. Reading the directory at run time is the point — a
/// hand-written list can only be kept honest by something that does not need hand-writing.
#[test]
fn every_published_migration_is_applied_by_the_helper() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("crawlforge-core")
        .join("migrations");
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("read a directory entry").file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".sql"))
        .collect();
    on_disk.sort();

    let applied: Vec<&str> = MIGRATIONS.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        applied, on_disk,
        "test_schema::MIGRATIONS must match crawlforge-core/migrations/ exactly; \
         add the missing migration to the list in test_schema.rs"
    );

    // Cross-check against the core, which the CLI does depend on: if the list is complete,
    // its last migration is the schema version the engine declares.
    assert_eq!(
        MIGRATIONS.len() as i64,
        crawlforge_core::SCHEMA_VERSION,
        "the migration list and crawlforge_core::SCHEMA_VERSION disagree"
    );
}
