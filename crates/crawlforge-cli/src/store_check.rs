//! Comprobación previa de que un `.sqlite` es lo que el comando espera.
//!
//! Nace de una revisión de experiencia de uso: pasarle a `report` un fichero que no era de
//! CrawlForge respondía «no such table: urls», que para un consultor SEO no significa nada. Peor
//! aún: el fichero que genera `crawlforge diff --out` producía exactamente ese mismo error al
//! pasarlo a `report` — la herramienta creaba un fichero que ella misma no sabía abrir y no lo
//! explicaba.
//!
//! La regla es una: **todo comando que reciba un `.sqlite` lo identifica antes de trabajar con
//! él**, y si no es lo esperado, dice *qué es* cuando se puede averiguar (un diff tiene su tabla
//! `diff_meta`; un rastreo, su `crawl_meta`) y qué comando produce lo que hacía falta. Un error
//! que solo dice lo que falló obliga a adivinar; uno que dice qué hacer a continuación no.

use crate::i18n::{self, msg};
use anyhow::{bail, Context, Result};
use crawlforge_rules::Lang;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// Qué es el fichero, según las tablas que contiene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreKind {
    /// Un fichero de rastreo: lo generan `crawl`, `audit` y `list`.
    Crawl,
    /// Un fichero de diff: lo genera `diff --out`.
    Diff,
    /// Una base de datos SQLite válida, pero de otro programa.
    Foreign,
    /// Ni siquiera es una base de datos SQLite.
    NotSqlite,
}

/// Identifica un `.sqlite` sin modificarlo. Se abre en solo lectura y solo se mira
/// `sqlite_master`: la comprobación tiene que ser más barata que el trabajo que evita.
pub fn identify(path: &Path) -> Result<StoreKind> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open {}", path.display()))?;

    // Abrir un fichero que no es SQLite no falla: la cabecera se lee en la primera consulta.
    // Es esa consulta la que devuelve `NotADatabase`.
    let has_table = |table: &str| -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
    };

    let is_crawl = match has_table("crawl_meta") {
        Ok(found) => found,
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::NotADatabase =>
        {
            return Ok(StoreKind::NotSqlite);
        }
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };

    if is_crawl && has_table("urls")? {
        return Ok(StoreKind::Crawl);
    }
    if has_table("diff_meta")? {
        return Ok(StoreKind::Diff);
    }
    Ok(StoreKind::Foreign)
}

/// Falla con un mensaje que se entiende si `path` no es un fichero de rastreo.
///
/// Es la comprobación común de `report`, `export` y `diff`. El mensaje dice qué es el fichero
/// cuando se puede averiguar y qué comandos generan lo que se esperaba, porque el usuario
/// objetivo es un consultor SEO, no alguien que sepa qué es una tabla de SQLite. El idioma es
/// el del proceso (`--lang` > `CRAWLFORGE_LANG` > inglés); los nombres de comando dentro del
/// mensaje no se traducen, son los comandos.
pub fn ensure_crawl_store(path: &Path) -> Result<()> {
    ensure_crawl_store_lang(path, i18n::current_lang())
}

/// La lógica de [`ensure_crawl_store`] con el idioma explícito, para poder probar los dos sin
/// depender del entorno del proceso.
fn ensure_crawl_store_lang(path: &Path, lang: Lang) -> Result<()> {
    match identify(path)? {
        StoreKind::Crawl => Ok(()),
        StoreKind::Diff => bail!(msg::error_store_is_diff(lang, path.display())),
        StoreKind::Foreign => bail!(msg::error_store_foreign(lang, path.display())),
        StoreKind::NotSqlite => bail!(msg::error_store_not_sqlite(lang, path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Directorio temporal propio; la CLI no depende de `tempfile` (stack cerrado, `CONVENTIONS.md §3`).
    fn tmpdir(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("crawlforge-check-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// A crawl file with the **real schema**: every published migration, from the shared
    /// helper in `test_schema.rs` — not a similar-looking copy that would pass the test and
    /// fail the command.
    fn crawl_file(path: &Path) -> Connection {
        crate::test_schema::crawl_file(path)
    }

    fn crawl_meta(conn: &Connection, started_at: &str) {
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime, truncated)
             VALUES ('c','p','P','https://ejemplo.es/','http', ?1, 'done','{}','0','0','free',0)",
            [started_at],
        )
        .expect("insert crawl_meta");
    }

    #[test]
    fn un_fichero_de_rastreo_pasa_la_comprobacion() {
        let dir = tmpdir("rastreo");
        let path = dir.join("crawl.sqlite");
        drop(crawl_file(&path));

        assert_eq!(identify(&path).expect("identify"), StoreKind::Crawl);
        ensure_crawl_store(&path).expect("a crawl must pass");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fichero_de_diff_se_identifica_y_el_error_dice_que_es_y_como_se_hace_un_rastreo() {
        // El callejón sin salida de la revisión de UX: `diff --out` produce un fichero que
        // `report` no sabía abrir y el error era «no such table: urls».
        let dir = tmpdir("diff");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        for (path, cuando) in
            [(&antes, "2026-07-01T10:00:00Z"), (&despues, "2026-07-08T10:00:00Z")]
        {
            let conn = crawl_file(path);
            crawl_meta(&conn, cuando);
        }
        let salida = dir.join("miweb-diff.sqlite");
        crate::diff::compare(&antes, &despues, Some(&salida), &[]).expect("generate the real diff");

        assert_eq!(identify(&salida).expect("identify"), StoreKind::Diff);
        let err = ensure_crawl_store_lang(&salida, Lang::En).expect_err("a diff is not a crawl");
        let msg = format!("{err:#}");
        assert!(msg.contains("is a diff file"), "says what it is: {msg}");
        assert!(msg.contains("diff --out"), "and where it came from: {msg}");
        assert!(
            msg.contains("crawl") && msg.contains("audit"),
            "and which commands produce what was needed: {msg}"
        );
        assert!(!msg.contains("no such table"), "no SQLite jargon: {msg}");

        // El mismo callejón, explicado en español y con los comandos intactos.
        let err = ensure_crawl_store_lang(&salida, Lang::Es).expect_err("a diff is not a crawl");
        let msg = format!("{err:#}");
        assert!(msg.contains("es un fichero de diff"), "says what it is: {msg}");
        assert!(msg.contains("diff --out"), "commands are not translated: {msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_base_sqlite_de_otro_programa_no_se_confunde_con_un_rastreo() {
        let dir = tmpdir("ajeno");
        let path = dir.join("ajeno.sqlite");
        {
            let conn = Connection::open(&path).expect("create");
            conn.execute_batch("CREATE TABLE cosas (id INTEGER);").expect("foreign table");
        }

        assert_eq!(identify(&path).expect("identify"), StoreKind::Foreign);
        let err = ensure_crawl_store_lang(&path, Lang::En).expect_err("not a crawl");
        let msg = format!("{err:#}");
        assert!(msg.contains("does not have its tables"), "{msg}");
        assert!(!msg.contains("no such table"), "no SQLite jargon: {msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fichero_que_no_es_sqlite_tambien_tiene_un_mensaje_claro() {
        let dir = tmpdir("texto");
        let path = dir.join("notas.sqlite");
        std::fs::write(&path, "esto es un fichero de texto con extensión mentirosa\n")
            .expect("write");

        assert_eq!(identify(&path).expect("identify"), StoreKind::NotSqlite);
        let err = ensure_crawl_store_lang(&path, Lang::En).expect_err("not SQLite");
        assert!(format!("{err:#}").contains("SQLite database"), "{err:#}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_comprobacion_no_modifica_el_fichero() {
        // Se abre en solo lectura: identificar un fichero jamás debe tocarlo.
        let dir = tmpdir("solo-lectura");
        let path = dir.join("crawl.sqlite");
        drop(crawl_file(&path));
        let antes = std::fs::metadata(&path).and_then(|m| m.modified()).expect("mtime");

        identify(&path).expect("identify");
        ensure_crawl_store(&path).expect("check");

        let despues = std::fs::metadata(&path).and_then(|m| m.modified()).expect("mtime");
        assert_eq!(antes, despues);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
