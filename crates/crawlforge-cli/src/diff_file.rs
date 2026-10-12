//! Leer un fichero de `crawlforge diff --out` y listar las filas de un diff sin recortar.
//!
//! Hasta la 0.11.0 la herramienta generaba ese fichero y no sabía abrirlo: `report` lo
//! rechazaba —con un mensaje claro, pero lo rechazaba— y lo único que quedaba era abrirlo con un
//! cliente de SQLite. Este módulo es la otra mitad.
//!
//! # Las decisiones
//!
//! **Se reabre con el mismo comando que lo creó**, `crawlforge diff <fichero>`, y no con uno
//! nuevo: es lo primero que se teclea al tener el fichero delante, y el resumen que se obtiene
//! es exactamente el de aquel día, con la misma [`diff::print_report`]. Un lector que dibujara
//! su propia versión del resumen acabaría diciendo cosas distintas del original.
//!
//! **El fichero se reconstruye como un [`DiffOutcome`]**, no se consulta aparte. Así el resumen,
//! la cabecera, los avisos y los listados salen del mismo sitio para un diff en vivo y para uno
//! guardado, y [`changes_text`] sirve a los dos. Un fichero de diff tiene una fila por cambio, no
//! por URL rastreada: cabe en memoria sin violar el antipatrón de cargar un rastreo entero.
//!
//! **Lo que el fichero no guarda no se inventa.** `diff_meta` no tiene el estado, el modo, el
//! nombre ni la configuración de cada rastreo, así que esos campos de [`CrawlInfo`] quedan
//! vacíos; el resumen no los usa, y la conclusión sale de la columna `conclusive` a través de
//! los avisos guardados, no de recalcularla. Por la misma razón **`--fail-on` no se acepta al
//! reabrir**: decidir si los hallazgos nuevos se pueden afirmar exige saber si el rastreo
//! «antes» terminó, y el fichero no lo dice. Una puerta que aprueba por falta de datos es peor
//! que no tener puerta.
//!
//! **Los listados (`--rule`, `--change`) van sin corte.** El resumen enseña tres ejemplos por
//! grupo a propósito; quien pide una regla o un tipo concreto quiere la lista para trabajarla,
//! igual que con `report --rule`.

use crate::audit_report::strip_control_chars;
use crate::diff::{
    self, Change, ChangeType, CrawlInfo, DiffOutcome, SavedDiff, Suppressed, Warning,
    DIFF_SCHEMA_VERSION,
};
use crate::i18n::{self, msg};
use crate::store_check::{self, StoreKind};
use anyhow::{bail, Context, Result};
use crawlforge_rules::Lang;
use rusqlite::{Connection, OpenFlags};
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Abre un fichero de `diff --out` y lo devuelve como si se acabara de calcular.
///
/// Se abre en solo lectura: reabrir un diff no lo modifica. Si `path` no es un fichero de diff,
/// el error dice qué es y qué hacer —en particular, si es un rastreo con su `.prev.sqlite` al
/// lado, da el comando exacto—.
pub fn open(path: &Path) -> Result<DiffOutcome> {
    let lang = i18n::current_lang();
    anyhow::ensure!(path.is_file(), "{}", msg::error_diff_missing(lang, path.display()));
    match store_check::identify(path)? {
        StoreKind::Diff => {}
        StoreKind::Crawl => bail!(needs_two_message(lang, path)),
        StoreKind::Foreign | StoreKind::NotSqlite => {
            bail!(msg::error_diff_unknown_file(lang, path.display()))
        }
    }

    let conn = Connection::open_with_flags(
        diff::read_only_uri(path)?,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open {} read-only", path.display()))?;

    let version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
        .with_context(|| format!("read {}", path.display()))?;
    if version > DIFF_SCHEMA_VERSION {
        bail!(msg::error_diff_too_new(lang, path.display(), version, DIFF_SCHEMA_VERSION));
    }

    read(&conn, path).with_context(|| format!("read the diff in {}", path.display()))
}

/// El error de pasarle un solo rastreo a `diff`, con el comando completo si su rastreo anterior
/// está al lado. Es el caso típico: se teclea `crawlforge diff crawl-x.sqlite` esperando que
/// compare con el anterior.
fn needs_two_message(lang: Lang, path: &Path) -> String {
    let mut m = msg::error_diff_needs_two(lang, path.display());
    if let Some(prev) = prev_of(path).filter(|p| p.is_file()) {
        m.push('\n');
        m.push_str(&msg::hint_diff_prev(lang, prev.display(), path.display()));
    }
    m
}

/// `crawl-x.sqlite` → `crawl-x.prev.sqlite`, la convención con la que `crawl` aparta el
/// rastreo anterior. `None` si el fichero ya es un `.prev`.
fn prev_of(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(".sqlite")?;
    if stem.ends_with(".prev") {
        return None;
    }
    Some(path.with_file_name(format!("{stem}.prev.sqlite")))
}

fn read(conn: &Connection, path: &Path) -> Result<DiffOutcome> {
    let mut outcome = conn.query_row(
        "SELECT generated_at, tool_version, warnings_json, urls_common, issues_persisted,
                suppressed_urls_added, suppressed_urls_removed,
                suppressed_issues_appeared, suppressed_issues_resolved,
                before_path, before_crawl_id, before_base_url, before_started_at,
                before_truncated, before_truncated_reason, before_core_version,
                before_rules_version, before_urls, before_issues,
                after_path, after_crawl_id, after_base_url, after_started_at,
                after_truncated, after_truncated_reason, after_core_version,
                after_rules_version, after_urls, after_issues
         FROM diff_meta WHERE id = 1",
        [],
        |r| {
            // Los campos que `diff_meta` no guarda quedan vacíos: ver la documentación del módulo.
            let crawl = |base: usize| -> rusqlite::Result<CrawlInfo> {
                Ok(CrawlInfo {
                    path: r.get(base)?,
                    crawl_id: r.get(base + 1)?,
                    project_name: String::new(),
                    base_url: r.get(base + 2)?,
                    mode: String::new(),
                    started_at: r.get(base + 3)?,
                    status: String::new(),
                    core_version: r.get(base + 6)?,
                    rules_version: r.get(base + 7)?,
                    config_json: String::new(),
                    truncated: r.get::<_, i64>(base + 4)? != 0,
                    truncated_reason: r.get(base + 5)?,
                    schema_version: 0,
                    urls_total: r.get(base + 8)?,
                    issues_total: r.get(base + 9)?,
                })
            };
            let warnings_json: String = r.get(2)?;
            Ok(DiffOutcome {
                before: crawl(9)?,
                after: crawl(19)?,
                warnings: parse_warnings(&warnings_json),
                changes: Vec::new(),
                issues_persisted: r.get(4)?,
                urls_common: r.get(3)?,
                suppressed: Suppressed {
                    urls_added: r.get(5)?,
                    urls_removed: r.get(6)?,
                    issues_appeared: r.get(7)?,
                    issues_resolved: r.get(8)?,
                },
                fail_on: Vec::new(),
                fail_on_requested: Vec::new(),
                fail_on_inconclusive: false,
                out_path: None,
                saved: Some(SavedDiff {
                    path: path.to_path_buf(),
                    generated_at: r.get(0)?,
                    tool_version: r.get(1)?,
                }),
            })
        },
    )?;

    let mut stmt = conn.prepare(
        "SELECT change_type, url, field, value_before, value_after, severity
         FROM changes ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            Change {
                // Se rellena abajo: el tipo se valida fuera del cierre para poder fallar con
                // un mensaje propio.
                change_type: ChangeType::UrlAdded,
                url: r.get(1)?,
                field: r.get(2)?,
                value_before: r.get(3)?,
                value_after: r.get(4)?,
                severity: r.get(5)?,
            },
        ))
    })?;
    let mut unknown: Vec<String> = Vec::new();
    for row in rows {
        let (tipo, mut change) = row?;
        // Un tipo que esta versión no conoce viene de una más nueva que escribió el mismo
        // formato. No se falla —el resto del diff sigue siendo cierto—, pero tampoco se calla:
        // un aviso dice cuántas filas se quedaron fuera.
        let Some(change_type) = ChangeType::parse(&tipo) else {
            unknown.push(tipo);
            continue;
        };
        change.change_type = change_type;
        outcome.changes.push(change);
    }
    if !unknown.is_empty() {
        let n = unknown.len();
        unknown.sort();
        unknown.dedup();
        let lang = i18n::current_lang();
        outcome.warnings.push(Warning::Stored {
            message: msg::warn_unknown_change_types(lang, i18n::count(lang, n as i64), unknown.join(", ")),
            breaks_conclusion: false,
        });
    }
    Ok(outcome)
}

/// `warnings_json` → avisos. Un JSON ilegible no tumba la lectura, pero tampoco se calla: se
/// convierte en un aviso que rompe la conclusión, porque no se puede saber qué decía.
fn parse_warnings(json: &str) -> Vec<Warning> {
    match serde_json::from_str::<Vec<serde_json::Value>>(json) {
        Ok(values) => values.iter().map(Warning::from_json).collect(),
        Err(_) => vec![Warning::Stored {
            message: "The warnings stored in this diff file could not be read.".to_string(),
            breaks_conclusion: true,
        }],
    }
}

// ──────────────────────────────────────────────────────────────── Listados

/// Interpreta el valor de `--change`. El error enumera los tipos válidos: son doce y nadie se
/// los sabe de memoria.
pub fn parse_change_type(lang: Lang, value: &str) -> Result<ChangeType> {
    ChangeType::parse(value).ok_or_else(|| {
        let available: Vec<&str> = ChangeType::ALL.iter().map(|t| t.as_str()).collect();
        anyhow::anyhow!(msg::error_unknown_change_type(lang, value, available.join(", ")))
    })
}

/// Todas las filas del diff que casan con `rule` y `change`, agrupadas por tipo de cambio y sin
/// corte. Al menos uno de los dos filtros debería venir; sin ninguno, lista el diff entero.
///
/// `rule` filtra hallazgos (`issue_appeared` e `issue_resolved`) por ID de regla, normalizado a
/// mayúsculas como en `report --rule`. Un ID que no está en el catálogo se acepta si el diff lo
/// trae —un rastreo de otra versión puede conocer reglas que esta no—; si no, es un error.
pub fn changes_text(
    outcome: &DiffOutcome,
    rule: Option<&str>,
    change: Option<ChangeType>,
    lang: Lang,
) -> Result<String> {
    let is_issue = |t: ChangeType| matches!(t, ChangeType::IssueAppeared | ChangeType::IssueResolved);
    let rule = rule.map(|r| r.trim().to_ascii_uppercase());

    let mut s = String::new();
    if let Some(rule) = &rule {
        if let Some(t) = change.filter(|t| !is_issue(*t)) {
            bail!(msg::error_rule_with_change(lang, t.as_str()));
        }
        let meta = crawlforge_rules::catalog().into_iter().find(|m| m.id == *rule);
        let in_diff = outcome.changes.iter().any(|c| c.field.as_deref() == Some(rule.as_str()));
        if meta.is_none() && !in_diff {
            bail!(msg::error_unknown_rule(lang, rule));
        }
        match meta {
            Some(m) => writeln!(s, "{rule} — {}", m.name(lang))?,
            None => writeln!(s, "{rule}")?,
        }
    }

    let matches = |c: &Change| {
        change.is_none_or(|t| c.change_type == t)
            && rule.as_deref().is_none_or(|r| is_issue(c.change_type) && c.field.as_deref() == Some(r))
    };

    let mut any = false;
    for tipo in ChangeType::ALL {
        let mut filas: Vec<&Change> = outcome.of(tipo).filter(|c| matches(c)).collect();
        // Los hallazgos, de más grave a menos, como en el resumen; dentro, en el orden de
        // siempre (regla y URL). El resto de tipos ya viene por URL.
        if is_issue(tipo) {
            filas.sort_by_key(|c| diff::severity_rank(c.severity.as_deref().unwrap_or("")));
        }
        if filas.is_empty() {
            continue;
        }
        if any {
            writeln!(s)?;
        }
        any = true;
        writeln!(s, "{} — {}", change_label(tipo, lang), i18n::count(lang, filas.len() as i64))?;
        for c in filas {
            write_row(&mut s, c, rule.is_some(), lang)?;
        }
    }
    if !any {
        writeln!(s, "{}", msg::diff_no_matching_changes(lang))?;
    }
    Ok(s)
}

/// El título de cada grupo, con las mismas palabras que el resumen.
fn change_label(t: ChangeType, lang: Lang) -> String {
    match t {
        ChangeType::UrlAdded => msg::label_new_urls(lang),
        ChangeType::UrlRemoved => msg::label_urls_gone(lang),
        ChangeType::StatusChanged => msg::label_status_changed(lang),
        ChangeType::TitleChanged => msg::label_titles_changed(lang),
        ChangeType::MetaDescriptionChanged => msg::label_meta_changed(lang),
        ChangeType::CanonicalChanged => msg::label_canonicals_changed(lang),
        ChangeType::IndexabilityLost => msg::label_pages_lost_index(lang),
        ChangeType::IndexabilityGained => msg::label_pages_indexable_again(lang),
        ChangeType::IssueAppeared => msg::label_new_findings(lang),
        ChangeType::IssueResolved => msg::label_findings_resolved(lang),
        // Nombres propios: iguales en todos los idiomas.
        ChangeType::RobotsTxtChanged => "robots.txt".to_string(),
        ChangeType::SitemapChanged => "Sitemaps".to_string(),
    }
}

/// Una fila. Los textos que vienen del sitio —URLs, títulos, descripciones— pasan por
/// [`strip_control_chars`]: un título con secuencias de escape no debe poder pintar la terminal.
fn write_row(s: &mut String, c: &Change, rule_known: bool, lang: Lang) -> std::fmt::Result {
    let clean = |v: &Option<String>| v.as_deref().map(strip_control_chars);
    let url = clean(&c.url);
    let before = clean(&c.value_before);
    let after = clean(&c.value_after);
    let dash = || "—".to_string();
    match c.change_type {
        ChangeType::UrlAdded | ChangeType::UrlRemoved => {
            writeln!(s, "  {}", url.unwrap_or_default())
        }
        ChangeType::StatusChanged => {
            let none = msg::label_no_response(lang);
            writeln!(
                s,
                "  {} → {}   {}",
                before.unwrap_or_else(|| none.clone()),
                after.unwrap_or(none),
                url.unwrap_or_default()
            )
        }
        ChangeType::IndexabilityLost | ChangeType::IndexabilityGained => writeln!(
            s,
            "  {} → {}   {}",
            before.unwrap_or_else(|| "?".to_string()),
            after.unwrap_or_else(|| "?".to_string()),
            url.unwrap_or_else(|| msg::site_placeholder(lang))
        ),
        // El texto entero, antes y después: es lo que se viene a buscar a esta lista.
        ChangeType::TitleChanged
        | ChangeType::MetaDescriptionChanged
        | ChangeType::CanonicalChanged => {
            writeln!(s, "  {}", url.unwrap_or_default())?;
            writeln!(s, "    - {}", before.unwrap_or_else(dash))?;
            writeln!(s, "    + {}", after.unwrap_or_else(dash))
        }
        ChangeType::IssueAppeared | ChangeType::IssueResolved => {
            let severity = i18n::severity_word(lang, c.severity.as_deref().unwrap_or(""));
            let url = url.unwrap_or_else(|| msg::site_wide_finding(lang));
            if rule_known {
                // Con --rule, la regla ya está en el título: repetirla en cada fila es ruido.
                writeln!(s, "  {url}")
            } else {
                writeln!(s, "  {severity:<9} {:<30} {url}", c.field.as_deref().unwrap_or(""))
            }
        }
        // `content` guarda el robots.txt entero de cada lado: se enseñan las líneas que se van
        // y las que llegan, que es lo que se busca, en vez de los dos ficheros completos.
        ChangeType::RobotsTxtChanged if c.field.as_deref() == Some("content") => {
            let host = url.unwrap_or_default();
            writeln!(s, "  {}", msg::robots_content_changed(lang, &host))?;
            // Del texto crudo, no del limpio: limpiar antes de partir convertiría los saltos de
            // línea en `\u{FFFD}`. Cada línea se limpia al escribirla.
            let a = c.value_before.as_deref().unwrap_or_default();
            let b = c.value_after.as_deref().unwrap_or_default();
            for line in a.lines().filter(|l| !b.lines().any(|x| x == *l)) {
                writeln!(s, "    - {}", strip_control_chars(line))?;
            }
            for line in b.lines().filter(|l| !a.lines().any(|x| x == *l)) {
                writeln!(s, "    + {}", strip_control_chars(line))?;
            }
            Ok(())
        }
        ChangeType::RobotsTxtChanged => {
            writeln!(
                s,
                "  {}: {} {} → {}",
                url.unwrap_or_default(),
                c.field.as_deref().unwrap_or(""),
                before.unwrap_or_else(dash),
                after.unwrap_or_else(dash)
            )?;
            if c.severity.as_deref() == Some("critical") {
                writeln!(s, "    {}", msg::robots_blocks_all(lang))?;
            }
            Ok(())
        }
        ChangeType::SitemapChanged => writeln!(
            s,
            "  {} {}: {} → {}",
            url.unwrap_or_default(),
            c.field.as_deref().unwrap_or(""),
            before.unwrap_or_else(dash),
            after.unwrap_or_else(dash)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::tests::{escenario, tmpdir};
    use crate::diff::{compare, Side};

    /// Un diff guardado del escenario base, y el resultado en vivo con el que compararlo.
    fn guardado(nombre: &str, truncado: Option<&str>) -> (PathBuf, PathBuf, DiffOutcome) {
        // Prefijo propio: `tmpdir` es el de los tests de `diff.rs`, que corren en paralelo con
        // estos, y dos tests con el mismo nombre de directorio se pisan los ficheros.
        let dir = tmpdir(&format!("lector-{nombre}"));
        let (antes, despues) = escenario(&dir, truncado);
        let salida = dir.join("cambios.sqlite");
        let vivo = compare(&antes, &despues, Some(&salida), &[]).expect("compare");
        (dir, salida, vivo)
    }

    #[test]
    fn reabrir_un_diff_devuelve_lo_mismo_que_se_calculo() {
        let (dir, salida, vivo) = guardado("reabrir", None);

        let leido = open(&salida).expect("open the saved diff");

        assert_eq!(leido.changes, vivo.changes, "every row, in the same order");
        assert_eq!(leido.warnings, vivo.warnings);
        assert_eq!(leido.urls_common, vivo.urls_common);
        assert_eq!(leido.issues_persisted, vivo.issues_persisted);
        assert_eq!(leido.suppressed, vivo.suppressed);
        for (l, v) in [(&leido.before, &vivo.before), (&leido.after, &vivo.after)] {
            assert_eq!(l.path, v.path);
            assert_eq!(l.base_url, v.base_url);
            assert_eq!(l.started_at, v.started_at);
            assert_eq!((l.urls_total, l.issues_total), (v.urls_total, v.issues_total));
            assert_eq!(l.rules_version, v.rules_version);
        }
        assert!(leido.conclusive());
        let saved = leido.saved.expect("a reopened diff says where it comes from");
        assert_eq!(saved.path, salida);
        assert_eq!(saved.tool_version, env!("CARGO_PKG_VERSION"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_avisos_de_un_diff_reabierto_se_traducen() {
        // El fichero guarda el texto en inglés; lo que permite enseñarlo en español al
        // reabrirlo es el `kind` estructurado que lo acompaña.
        let (dir, salida, _) = guardado("avisos", Some("max_urls"));

        let leido = open(&salida).expect("open");

        assert!(!leido.conclusive(), "a truncated side still makes it inconclusive");
        let truncado = leido
            .warnings
            .iter()
            .find(|w| matches!(w, Warning::Truncated { side: Side::After, .. }))
            .expect("the truncation warning comes back as itself, not as text");
        let es = truncado.message(Lang::Es);
        assert!(es.contains("truncado por max_urls"), "{es}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_diff_de_la_0_11_sin_avisos_estructurados_conserva_los_suyos() {
        // Hasta la 0.11.0 `warnings_json` solo llevaba `message` y `breaks_conclusion`. Un
        // aviso que se perdiera al reabrir podría ser justo el que dice que no es concluyente.
        let (dir, salida, _) = guardado("legado", Some("max_urls"));
        {
            let conn = Connection::open(&salida).expect("open rw");
            conn.execute(
                "UPDATE diff_meta SET warnings_json = ?1",
                [r#"[{"message":"The 'after' crawl is truncated by max_urls.","breaks_conclusion":true}]"#],
            )
            .expect("write the old format");
        }

        let leido = open(&salida).expect("open");

        assert_eq!(
            leido.warnings,
            [Warning::Stored {
                message: "The 'after' crawl is truncated by max_urls.".to_string(),
                breaks_conclusion: true,
            }]
        );
        assert!(!leido.conclusive(), "the stored flag still counts");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cada_aviso_sobrevive_al_viaje_por_json() {
        let avisos = [
            Warning::Truncated { side: Side::Before, reason: Some("max_urls".into()) },
            Warning::Truncated { side: Side::After, reason: None },
            Warning::Unfinished { side: Side::After, status: "cancelled".into() },
            Warning::RulesVersionChanged { before: "0.10.0".into(), after: "0.11.0".into() },
            Warning::ConfigChanged { fields: vec!["limits.max_urls".into(), "include".into()] },
            Warning::ScopeChanged { before: "https://a.es/".into(), after: "https://a.es/b/".into() },
            Warning::ModeChanged { before: "http".into(), after: "filesystem".into() },
            Warning::MissingTable { side: Side::Before, table: "robots_txt".into() },
            Warning::OrderInverted { before_started: "2".into(), after_started: "1".into() },
            Warning::Stored { message: "x".into(), breaks_conclusion: true },
        ];
        for aviso in avisos {
            assert_eq!(Warning::from_json(&aviso.to_json()), aviso);
        }
    }

    #[test]
    fn reabrir_no_modifica_el_fichero() {
        let (dir, salida, _) = guardado("solo-lectura", None);
        let antes = std::fs::read(&salida).expect("read");

        open(&salida).expect("open");

        assert_eq!(std::fs::read(&salida).expect("read"), antes);
        assert!(!dir.join("cambios.sqlite-wal").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_tipo_de_cambio_desconocido_se_salta_y_se_avisa() {
        let (dir, salida, vivo) = guardado("tipo-futuro", None);
        {
            let conn = Connection::open(&salida).expect("open rw");
            conn.execute("INSERT INTO changes (change_type, url) VALUES ('hreflang_changed', 'x')", [])
                .expect("insert");
        }

        let leido = open(&salida).expect("a newer row does not make the file unreadable");

        assert_eq!(leido.changes, vivo.changes, "the known rows are all there");
        let aviso = leido.warnings.last().expect("and a warning says something was left out");
        assert!(aviso.message(Lang::En).contains("hreflang_changed"), "{aviso:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_diff_de_formato_mas_nuevo_se_rechaza_diciendo_que_hacer() {
        let (dir, salida, _) = guardado("formato-nuevo", None);
        {
            let conn = Connection::open(&salida).expect("open rw");
            conn.execute("UPDATE schema_version SET version = 99", []).expect("bump");
        }

        let err = open(&salida).expect_err("a newer format is not guessed at");
        assert!(format!("{err:#}").contains("Update crawlforge"), "{err:#}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_solo_rastreo_da_el_comando_con_su_prev() {
        let dir = tmpdir("lector-un-rastreo");
        let (antes, despues) = escenario(&dir, None);
        let actual = dir.join("crawl-ejemplo-es.sqlite");
        let previo = dir.join("crawl-ejemplo-es.prev.sqlite");
        std::fs::rename(&despues, &actual).expect("rename");
        std::fs::rename(&antes, &previo).expect("rename");

        let err = format!("{:#}", open(&actual).expect_err("one crawl is not a diff"));
        assert!(err.contains("comparing needs two"), "{err}");
        assert!(
            err.contains(&format!("crawlforge diff {} {}", previo.display(), actual.display())),
            "gives the exact command: {err}"
        );

        // Sin `.prev` al lado no hay comando que proponer, y no se inventa.
        std::fs::remove_file(&previo).expect("remove");
        let err = format!("{:#}", open(&actual).expect_err("still not a diff"));
        assert!(!err.contains("next to it"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fichero_que_no_es_de_crawlforge_lo_dice() {
        let dir = tmpdir("lector-ajeno");
        let ajeno = dir.join("ajeno.sqlite");
        Connection::open(&ajeno).expect("create").execute_batch("CREATE TABLE t (x);").expect("t");

        let err = format!("{:#}", open(&ajeno).expect_err("not ours"));
        assert!(err.contains("neither a crawl file nor a diff file"), "{err}");
        let err = format!("{:#}", open(&dir.join("no-existe.sqlite")).expect_err("missing"));
        assert!(err.contains("--out"), "says what produces one: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Listados ────────────────────────────────────────────────────────────

    #[test]
    fn rule_lista_cada_hallazgo_nuevo_y_resuelto_sin_corte() {
        let (dir, _, mut vivo) = guardado("rule", None);
        // Más filas que los tres ejemplos del resumen, para que un corte se notara.
        for i in 0..5 {
            vivo.changes.push(Change {
                change_type: ChangeType::IssueAppeared,
                url: Some(format!("https://ejemplo.es/extra-{i}")),
                field: Some("HTTP-404-INTERNAL".into()),
                value_before: None,
                value_after: None,
                severity: Some("critical".into()),
            });
        }

        let texto = changes_text(&vivo, Some("http-404-internal"), None, Lang::En).expect("list");

        assert!(texto.starts_with("HTTP-404-INTERNAL — "), "normalised ID and its name: {texto}");
        assert!(texto.contains("New findings — 6"), "{texto}");
        for i in 0..5 {
            assert!(texto.contains(&format!("https://ejemplo.es/extra-{i}")), "row {i}: {texto}");
        }
        assert!(!texto.contains("INDEX-NOINDEX-IN-SITEMAP"), "only the asked rule: {texto}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn change_lista_un_tipo_con_el_antes_y_el_despues() {
        let (dir, salida, _) = guardado("change", None);
        let leido = open(&salida).expect("open");

        let estados = changes_text(&leido, None, Some(ChangeType::StatusChanged), Lang::Es)
            .expect("list");
        assert!(estados.contains("Códigos de estado cambiados — 1"), "{estados}");
        assert!(estados.contains("200 → 404   https://ejemplo.es/a"), "{estados}");

        let titulos =
            changes_text(&leido, None, Some(ChangeType::TitleChanged), Lang::En).expect("list");
        assert!(titulos.contains("    - Título viejo\n    + Título nuevo"), "{titulos}");

        let nada = changes_text(&leido, None, Some(ChangeType::RobotsTxtChanged), Lang::En)
            .expect("list");
        assert_eq!(nada, "No change in this diff matches.\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_hallazgos_se_listan_de_mas_grave_a_menos() {
        let (dir, _, mut vivo) = guardado("orden", None);
        let hallazgo = |rule: &str, severity: &str| Change {
            change_type: ChangeType::IssueAppeared,
            url: Some(format!("https://ejemplo.es/{rule}")),
            field: Some(rule.into()),
            value_before: None,
            value_after: None,
            severity: Some(severity.into()),
        };
        // En el orden en que los genera el diff —por regla—, que no es el de gravedad.
        vivo.changes.insert(0, hallazgo("AAA-LOW", "low"));
        vivo.changes.push(hallazgo("ZZZ-CRITICAL", "critical"));

        let texto =
            changes_text(&vivo, None, Some(ChangeType::IssueAppeared), Lang::En).expect("list");
        let pos = |s: &str| texto.find(s).unwrap_or_else(|| panic!("{s} missing: {texto}"));
        assert!(pos("ZZZ-CRITICAL") < pos("AAA-LOW"), "{texto}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_robots_txt_cambiado_lista_solo_las_lineas_que_cambian() {
        let (dir, _, mut vivo) = guardado("robots", None);
        vivo.changes.push(Change {
            change_type: ChangeType::RobotsTxtChanged,
            url: Some("ejemplo.es".into()),
            field: Some("content".into()),
            value_before: Some("User-agent: *\nDisallow: /wp-admin/".into()),
            value_after: Some("User-agent: *\nDisallow: /".into()),
            severity: None,
        });

        let texto = changes_text(&vivo, None, Some(ChangeType::RobotsTxtChanged), Lang::En)
            .expect("list");
        assert!(texto.contains("    - Disallow: /wp-admin/\n    + Disallow: /\n"), "{texto}");
        assert!(!texto.contains("User-agent"), "unchanged lines are not repeated: {texto}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_filtros_mal_puestos_son_errores_y_no_listas_vacias() {
        let (dir, _, vivo) = guardado("filtros", None);

        let err = changes_text(&vivo, Some("NO-EXISTE"), None, Lang::En).expect_err("unknown rule");
        assert!(format!("{err}").contains("crawlforge rules"), "{err}");

        let err = changes_text(&vivo, Some("HTTP-404-INTERNAL"), Some(ChangeType::UrlAdded), Lang::En)
            .expect_err("a rule never matches a URL change");
        assert!(format!("{err}").contains("issue_appeared"), "{err}");

        let err = parse_change_type(Lang::En, "urls_removed").expect_err("typo");
        assert!(format!("{err}").contains("url_removed"), "lists the valid ones: {err}");
        assert_eq!(parse_change_type(Lang::En, "URL-REMOVED").ok(), Some(ChangeType::UrlRemoved));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_prev_de_un_rastreo() {
        assert_eq!(prev_of(Path::new("d/crawl-x.sqlite")), Some(PathBuf::from("d/crawl-x.prev.sqlite")));
        assert_eq!(prev_of(Path::new("crawl-x.prev.sqlite")), None);
        assert_eq!(prev_of(Path::new("crawl-x.db")), None);
    }
}
