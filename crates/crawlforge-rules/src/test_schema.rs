//! Test-only helper: the complete published schema, built in one single place.
//!
//! Until 2026-08-04 every test module carried its own hand-written migration list, and the
//! lists drifted: some stopped at 001 and kept exercising the buggy `v_orphans` that the
//! migrations 003 and 005 fix, others lacked the indexes from 006-008 and could not notice a
//! query losing them. One list, one guard test, no drift.
//!
//! The crate deliberately does **not** depend on `crawlforge-core` — the rules evaluate any
//! SQLite connection — so the migrations are embedded by relative path, exactly like the old
//! per-module lists did. What is new is the guard test at the bottom: it reads the
//! `migrations/` directory at run time and fails when a published `.sql` file is missing here,
//! so forgetting to extend this list is a red test instead of a silently stale schema.

use rusqlite::Connection;

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

/// An in-memory database with the complete published schema — **all** the migrations, always.
///
/// Tests that need less than the full schema on purpose (a hand-written three-column table
/// that documents exactly what a query reads, like `meta.rs::conexion_con_paginas`) should
/// keep building it by hand; this helper replaces the hand-written *migration lists*, not the
/// deliberate minimal schemas.
pub(crate) fn full_schema() -> Connection {
    let conn = Connection::open_in_memory().expect("open an in-memory database");
    for (name, sql) in MIGRATIONS {
        conn.execute_batch(sql)
            .unwrap_or_else(|e| panic!("apply migration {name}: {e}"));
    }
    conn
}

/// The guard: every `.sql` file published in `crawlforge-core/migrations/` must be applied by
/// [`full_schema`], in order. Reading the directory at run time is the point — a hand-written
/// list can only be kept honest by something that does not need hand-writing.
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
}
