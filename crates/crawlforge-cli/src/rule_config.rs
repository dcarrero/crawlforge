//! Per-site rule configuration, applied when a crawl file is **read**.
//!
//! The crawl file keeps every raw finding with the catalog's severity. A `rules:` block —in
//! the `--config` YAML of `crawl`/`list`/`audit`, which stores it in `crawl_meta.config_json`,
//! or in the `--config` of a read command, which replaces the stored one for that run— hides
//! the disabled rules and rewrites the severity of the overridden ones. Changing a criterion
//! never requires crawling again.
//!
//! # How it is applied: a TEMP view that shadows `issues`
//!
//! SQLite resolves an unqualified table name in the `temp` schema before `main`. So on a
//! read-only connection [`install`] creates `temp.issues` —the real table, filtered and with
//! the severity rewritten— and `temp.v_issue_summary` over it. Every query of `report`,
//! `export`, `inspect` and `portfolio` that says `FROM issues` reads the configured view
//! without being touched, and a query written next year gets it too. The alternative, a
//! filter threaded into a few dozen queries, is a feature that a new query forgets.
//!
//! Two consequences, both deliberate:
//!
//! - The views are only created when there is something to apply. A file without a
//!   configuration reads exactly as before, indexes included: the cost lands only on who
//!   asked for it (a `WHERE severity = …` over a `CASE` cannot use `idx_issues_severity`).
//! - `v_issue_summary` is shadowed too: a view in `main` resolves its own names in `main`,
//!   so the original would keep counting the raw table.
//!
//! A diff attaches its two files under other names, so it cannot use the shadowing; it reads
//! [`issues_select`] for each side instead, **with the same overrides on both**.

use crate::i18n::msg;
use anyhow::{Context, Result};
use crawlforge_core::job::{JobConfig, RuleOverrides};
use crawlforge_rules::Lang;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// What a read command was asked to apply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RulesChoice {
    /// The block stored in each file, if it has one. The default.
    #[default]
    Stored,
    /// The `rules:` block of a `--config` file, for every file the command reads. A file
    /// without the block means "no overrides": that is how to see the raw findings.
    File { path: PathBuf, overrides: RuleOverrides },
}

impl RulesChoice {
    /// The `--config` flag of a read command, loaded and validated. `None` is the default.
    pub fn from_flag(path: Option<&Path>) -> Result<Self> {
        Ok(match path {
            None => Self::Stored,
            Some(path) => Self::File { path: path.to_path_buf(), overrides: load(path)? },
        })
    }
}

/// Where the configuration that was applied came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The crawl file's own `config_json`.
    Stored,
    /// A `--config` file given to the read command.
    File(PathBuf),
}

/// The configuration a read actually applied: what to say about it, and what to filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub overrides: RuleOverrides,
    pub source: Source,
}

impl Applied {
    pub fn none() -> Self {
        Self { overrides: RuleOverrides::default(), source: Source::Stored }
    }

    /// The transparency line: "Rule config (…): N rules disabled, M severities changed".
    /// `None` when nothing is applied — no line about a configuration nobody wrote.
    pub fn note(&self, lang: Lang) -> Option<String> {
        if self.overrides.is_empty() {
            return None;
        }
        let source = match &self.source {
            Source::Stored => msg::rules_config_source_stored(lang),
            Source::File(path) => msg::rules_config_source_file(lang, path.display()),
        };
        Some(msg::rules_config_note(
            lang,
            source,
            self.overrides.disable.len(),
            self.overrides.severity.len(),
        ))
    }
}

/// Reads the `rules:` block of a `--config` YAML. The whole file is parsed with the same
/// type as `crawl --config` —so a typo anywhere is an error and the same site file serves
/// both— but only `rules:` means anything to a read command.
pub fn load(path: &Path) -> Result<RuleOverrides> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read the config file {}", path.display()))?;
    let config: JobConfig = serde_norway::from_str(&text)
        .with_context(|| format!("could not parse the config file {}", path.display()))?;
    config
        .rules
        .unwrap_or_default()
        .validated()
        .with_context(|| format!("could not apply the config file {}", path.display()))
}

/// The block stored in a crawl file (`schema` is `main` or an `ATTACH` alias).
///
/// Lenient on purpose. The IDs were checked by whoever crawled, and a newer binary may have
/// retired one: hiding a rule that no longer fires is harmless, refusing to open the file is
/// not. A `config_json` that does not parse —a fabricated or very old file— reads as no
/// overrides, which is what it would have meant before this existed.
pub fn stored(conn: &Connection, schema: &str) -> Result<RuleOverrides> {
    let json: Option<String> = conn
        .query_row(&format!("SELECT config_json FROM {schema}.crawl_meta LIMIT 1"), [], |r| {
            r.get(0)
        })
        .ok();
    Ok(json
        .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
        .and_then(|v| v.get("rules").cloned())
        .and_then(|r| serde_json::from_value::<RuleOverrides>(r).ok())
        .unwrap_or_default())
}

/// What `choice` means for the file attached as `schema`.
pub fn resolve(conn: &Connection, schema: &str, choice: &RulesChoice) -> Result<Applied> {
    Ok(match choice {
        RulesChoice::Stored => Applied { overrides: stored(conn, schema)?, source: Source::Stored },
        RulesChoice::File { path, overrides } => {
            Applied { overrides: overrides.clone(), source: Source::File(path.clone()) }
        }
    })
}

/// Opens a crawl file read-only with `choice` applied. The connection's `issues` and
/// `v_issue_summary` are the configured ones from here on.
pub fn open_read_only(store: &Path, choice: &RulesChoice) -> Result<(Connection, Applied)> {
    let conn = Connection::open_with_flags(
        store,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open the crawl file {}", store.display()))?;
    let applied = apply(&conn, choice)?;
    Ok((conn, applied))
}

/// [`resolve`] and [`install`] on an already open connection to a crawl file.
pub fn apply(conn: &Connection, choice: &RulesChoice) -> Result<Applied> {
    let applied = resolve(conn, "main", choice)?;
    install(conn, &applied.overrides)?;
    Ok(applied)
}

/// Shadows `issues` and `v_issue_summary` with their configured versions. See the module
/// documentation. A no-op without overrides.
pub fn install(conn: &Connection, overrides: &RuleOverrides) -> Result<()> {
    if overrides.is_empty() {
        return Ok(());
    }
    conn.execute_batch(&format!(
        "CREATE TEMP VIEW issues AS {};
         CREATE TEMP VIEW v_issue_summary AS
             SELECT rule_id, severity, category, COUNT(*) AS n
             FROM temp.issues GROUP BY rule_id, severity, category;",
        issues_select("main", overrides)
    ))
    .context("apply the rule configuration")?;
    Ok(())
}

/// `SELECT` of `{schema}.issues` with the overrides applied: the same seven columns, the
/// disabled rules filtered out and the severity rewritten.
pub fn issues_select(schema: &str, overrides: &RuleOverrides) -> String {
    let severity = if overrides.severity.is_empty() {
        "severity".to_string()
    } else {
        let whens: String = overrides
            .severity
            .iter()
            .map(|(id, sev)| format!(" WHEN {} THEN '{}'", sql_literal(id), sev.as_str()))
            .collect();
        format!("CASE rule_id{whens} ELSE severity END")
    };
    let filter = if overrides.disable.is_empty() {
        String::new()
    } else {
        let ids: Vec<String> = overrides.disable.iter().map(|id| sql_literal(id)).collect();
        format!(" WHERE rule_id NOT IN ({})", ids.join(", "))
    };
    format!(
        "SELECT id, url_id, rule_id, {severity} AS severity, category, detail_json, group_key
         FROM {schema}.issues{filter}"
    )
}

/// A string as an SQL literal. The stored IDs come from a file that travels and are not
/// re-validated (see [`stored`]), so they are quoted, never trusted.
fn sql_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crawlforge_rules::Severity;

    /// A crawl file with three findings and, optionally, a stored `rules` block.
    fn crawl_file(dir: &Path, rules_json: Option<&str>) -> PathBuf {
        let path = dir.join("crawl.sqlite");
        let conn = crate::test_schema::crawl_file(&path);
        let config = match rules_json {
            Some(r) => format!("{{\"rules\": {r}}}"),
            None => "{}".to_string(),
        };
        insert_meta(&conn, &config);
        conn.execute_batch(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
                 VALUES (1, 'https://ejemplo.es/', 1, 'https', 'ejemplo.es', '/', 1, 0, 'done', 200);
             INSERT INTO issues (url_id, rule_id, severity, category) VALUES
                 (1, 'META-DESC-TOO-LONG', 'low', 'meta'),
                 (1, 'HTTP-404-EXTERNAL', 'high', 'http'),
                 (1, 'CANON-CHAIN', 'medium', 'canonical');",
        )
        .expect("issues");
        path
    }

    pub(crate) fn insert_meta(conn: &Connection, config_json: &str) {
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime)
             VALUES ('c','p','P', 'https://ejemplo.es/', 'http', '2026-07-01T10:00:00Z',
                     'done', ?1, '0.0.1', '0.0.1', 'agency')",
            [config_json],
        )
        .expect("insert crawl_meta");
    }

    fn tmpdir(name: &str) -> PathBuf {
        crate::diff::tests::tmpdir(name)
    }

    fn rows(conn: &Connection, sql: &str) -> Vec<(String, String)> {
        let mut stmt = conn.prepare(sql).expect("prepare");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .collect::<rusqlite::Result<_>>()
            .expect("rows")
    }

    #[test]
    fn the_stored_block_shadows_issues_and_the_summary_view() {
        let dir = tmpdir("rules-stored");
        let store = crawl_file(
            &dir,
            Some(r#"{"disable": ["META-DESC-TOO-LONG"], "severity": {"HTTP-404-EXTERNAL": "low"}}"#),
        );
        let (conn, applied) = open_read_only(&store, &RulesChoice::Stored).expect("open");
        assert_eq!(applied.source, Source::Stored);
        let sql = "SELECT rule_id, severity FROM issues ORDER BY rule_id";
        assert_eq!(
            rows(&conn, sql),
            [("CANON-CHAIN".into(), "medium".into()), ("HTTP-404-EXTERNAL".into(), "low".into())]
        );
        // The view in `main` resolves its names in `main`: without its own shadow it would
        // keep counting the disabled rule.
        let sql = "SELECT rule_id, severity FROM v_issue_summary ORDER BY rule_id";
        assert_eq!(
            rows(&conn, sql),
            [("CANON-CHAIN".into(), "medium".into()), ("HTTP-404-EXTERNAL".into(), "low".into())]
        );
        // And the file itself still has everything: nothing was written.
        let raw: i64 =
            conn.query_row("SELECT COUNT(*) FROM main.issues", [], |r| r.get(0)).expect("count");
        assert_eq!(raw, 3);
    }

    #[test]
    fn a_file_without_a_block_reads_raw() {
        let dir = tmpdir("rules-none");
        let store = crawl_file(&dir, None);
        let (conn, applied) = open_read_only(&store, &RulesChoice::Stored).expect("open");
        assert!(applied.note(Lang::En).is_none());
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0)).expect("n");
        assert_eq!(n, 3);
    }

    #[test]
    fn a_config_flag_replaces_the_stored_block() {
        let dir = tmpdir("rules-flag");
        let store = crawl_file(&dir, Some(r#"{"disable": ["META-DESC-TOO-LONG"]}"#));
        let yaml = dir.join("site.yaml");
        std::fs::write(&yaml, "rules:\n  severity:\n    canon-chain: critical\n").expect("yaml");
        let choice = RulesChoice::from_flag(Some(&yaml)).expect("valid");
        let (conn, applied) = open_read_only(&store, &choice).expect("open");
        assert_eq!(applied.source, Source::File(yaml.clone()));
        let sql = "SELECT rule_id, severity FROM issues ORDER BY rule_id";
        assert_eq!(
            rows(&conn, sql),
            [
                ("CANON-CHAIN".into(), "critical".into()),
                ("HTTP-404-EXTERNAL".into(), "high".into()),
                ("META-DESC-TOO-LONG".into(), "low".into()),
            ],
            "the stored disable is replaced, not merged"
        );
    }

    #[test]
    fn a_config_file_without_rules_means_raw_findings() {
        let dir = tmpdir("rules-flag-empty");
        let store = crawl_file(&dir, Some(r#"{"disable": ["META-DESC-TOO-LONG"]}"#));
        let yaml = dir.join("site.yaml");
        std::fs::write(&yaml, "max_urls: 10\n").expect("yaml");
        let choice = RulesChoice::from_flag(Some(&yaml)).expect("valid");
        let (conn, _) = open_read_only(&store, &choice).expect("open");
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0)).expect("n");
        assert_eq!(n, 3);
    }

    #[test]
    fn a_typo_in_the_config_flag_is_an_error_naming_it() {
        let dir = tmpdir("rules-flag-typo");
        let yaml = dir.join("site.yaml");
        std::fs::write(&yaml, "rules:\n  disable: [META-DESC-TOO-LONGG]\n").expect("yaml");
        let err = RulesChoice::from_flag(Some(&yaml)).expect_err("unknown ID");
        assert!(format!("{err:#}").contains("META-DESC-TOO-LONGG"), "{err:#}");

        std::fs::write(&yaml, "rules:\n  severity:\n    CANON-CHAIN: hihg\n").expect("yaml");
        let err = RulesChoice::from_flag(Some(&yaml)).expect_err("unknown severity");
        assert!(format!("{err:#}").contains("hihg"), "{err:#}");
    }

    #[test]
    fn a_stored_id_with_a_quote_is_quoted_not_executed() {
        let mut overrides = RuleOverrides::default();
        overrides.disable.push("X') OR 1=1 --".to_string());
        overrides.severity.insert("Y'".to_string(), Severity::Low);
        let sql = issues_select("main", &overrides);
        assert!(sql.contains("'X'') OR 1=1 --'"), "{sql}");
        assert!(sql.contains("WHEN 'Y''' THEN 'low'"), "{sql}");
    }

    #[test]
    fn the_note_counts_disabled_rules_and_changed_severities() {
        let overrides = RuleOverrides {
            disable: vec!["A".into(), "B".into()],
            severity: [("C".to_string(), Severity::Low)].into_iter().collect(),
        };
        let applied = Applied { overrides, source: Source::Stored };
        let en = applied.note(Lang::En).expect("note");
        assert!(en.contains("2 rules disabled") && en.contains("1 severity changed"), "{en}");
        let es = applied.note(Lang::Es).expect("nota");
        assert!(es.contains("2 reglas desactivadas") && es.contains("1 severidad cambiada"), "{es}");
    }
}
