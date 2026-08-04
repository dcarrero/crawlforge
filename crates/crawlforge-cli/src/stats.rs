//! Registro estadístico de rastreos.
//!
//! Cada ejecución añade una línea a un fichero JSONL append-only. Sirve para dos cosas que la
//! el desarrollo del motor necesita y una sesión suelta de terminal no da:
//!
//! - **Comparar con Screaming Frog.** El campo `tool` permite anotar a mano una ejecución de SF
//!   sobre el mismo sujeto y con la misma configuración, que es lo que exige
//!   Una comparación con parámetros distintos no vale nada, así que
//!   la configuración se guarda en el mismo registro que las métricas.
//! - **Detectar regresiones.** Los objetivos de rendimiento se convierten
//!   en tests de regresión (`docs/01-ARQUITECTURA.md §8`), y para eso hace falta la serie
//!   histórica, no la última medición.
//!
//! El formato es JSONL y no CSV a propósito: el desglose por regla y por código de estado es de
//! ancho variable, y en CSV obligaría a una columna por regla o a un campo escapado.
//!
//! **La salida de este módulo se queda en inglés a propósito** y no pasa por el catálogo de
//! `crawlforge_cli::i18n`: `stats` es un comando oculto (`#[command(hide = true)]`) de
//! desarrollo del motor, su audiencia es quien compara contra otra herramienta, y
//! sus textos citan umbrales de una fase interna («the project stops»). Es el mismo criterio
//! que `report::print_metrics` y `report::print_gate`: la herramienta habla el idioma del
//! usuario; el banco de pruebas del motor, el de los logs.

use anyhow::Result;
use crawlforge_core::engine::CrawlOutcome;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::Path;

/// Un registro de rastreo, tal como se guarda en el histórico.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BenchRecord {
    /// Marca de tiempo UTC, en formato ISO 8601.
    pub timestamp: String,
    /// Qué herramienta produjo el registro: `crawlforge` o, anotado a mano, `screamingfrog`.
    pub tool: String,
    pub tool_version: String,
    /// Sitio o directorio rastreado.
    pub target: String,
    pub mode: String,

    // Configuración: sin ella la comparación no significa nada.
    pub concurrency: u8,
    pub max_urls: Option<u64>,
    pub max_depth: Option<u32>,
    pub respect_robots: bool,
    pub sitemaps: bool,

    // Rendimiento.
    pub elapsed_secs: f64,
    pub urls_fetched: u64,
    pub urls_discovered: u64,
    pub urls_errored: u64,
    pub urls_excluded: u64,
    pub urls_per_second: f64,
    /// Ritmo contando solo páginas HTML parseadas. Es la cifra honesta cuando el sitio tiene
    /// muchos recursos: comprobar el estado de una imagen es trabajo, pero no es parsear.
    pub html_pages_per_second: f64,
    pub bytes_downloaded: u64,
    pub peak_rss_mb: f64,

    // Resultados.
    pub pages_html: i64,
    pub pages_indexable: i64,
    pub links: i64,
    pub images: i64,
    pub truncated: Option<String>,
    /// Recuento por familia de código de estado (`2xx`, `3xx`, `4xx`, `5xx`).
    pub status_groups: BTreeMap<String, i64>,
    /// Recuento por motivo de no indexabilidad.
    pub indexability_reasons: BTreeMap<String, i64>,
    /// Recuento por regla.
    pub issues_by_rule: BTreeMap<String, i64>,
    /// Recuento por motivo de exclusión.
    pub exclusions: BTreeMap<String, i64>,

    /// Nota libre: versión de Screaming Frog, condiciones de red, lo que haga falta recordar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Configuración con la que se lanzó un rastreo.
///
/// Va agrupada y no suelta porque es inseparable de las métricas: una comparación con
/// otra herramienta hecha con parámetros distintos no vale nada.
pub struct BenchConfig<'a> {
    pub target: &'a str,
    pub mode: &'a str,
    pub concurrency: u8,
    pub max_urls: Option<u64>,
    pub max_depth: Option<u32>,
    pub respect_robots: bool,
    pub sitemaps: bool,
    pub note: Option<String>,
}

/// Reúne las estadísticas de un rastreo recién terminado.
pub fn collect(outcome: &CrawlOutcome, config: BenchConfig<'_>) -> Result<BenchRecord> {
    let conn = Connection::open_with_flags(
        &outcome.store_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    let count = |sql: &str| -> Result<i64> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };

    let pages_html = count("SELECT COUNT(*) FROM pages")?;
    let pages_indexable = count("SELECT COUNT(*) FROM pages WHERE is_indexable = 1")?;
    let links = count("SELECT COUNT(*) FROM links")?;
    let images = count("SELECT COUNT(*) FROM images")?;

    let m = &outcome.metrics;
    let secs = m.elapsed.as_secs_f64();

    Ok(BenchRecord {
        timestamp: utc_now(&conn)?,
        tool: "crawlforge".to_string(),
        tool_version: crawlforge_core::CORE_VERSION.to_string(),
        target: config.target.to_string(),
        mode: config.mode.to_string(),
        concurrency: config.concurrency,
        max_urls: config.max_urls,
        max_depth: config.max_depth,
        respect_robots: config.respect_robots,
        sitemaps: config.sitemaps,
        elapsed_secs: round2(secs),
        urls_fetched: m.urls_fetched,
        urls_discovered: m.urls_discovered,
        urls_errored: m.urls_errored,
        urls_excluded: m.urls_excluded,
        urls_per_second: round2(m.urls_per_second()),
        html_pages_per_second: round2(if secs > 0.0 { pages_html as f64 / secs } else { 0.0 }),
        bytes_downloaded: m.bytes_downloaded,
        peak_rss_mb: round2(m.peak_rss_mb()),
        pages_html,
        pages_indexable,
        links,
        images,
        truncated: outcome.truncated.map(|t| t.as_str().to_string()),
        status_groups: group_map(
            &conn,
            "SELECT CASE
                        WHEN status_code IS NULL THEN 'none'
                        WHEN status_code < 300 THEN '2xx'
                        WHEN status_code < 400 THEN '3xx'
                        WHEN status_code < 500 THEN '4xx'
                        ELSE '5xx' END, COUNT(*)
             FROM urls WHERE crawl_state != 'skipped' GROUP BY 1",
        )?,
        indexability_reasons: group_map(
            &conn,
            "SELECT indexability_reason, COUNT(*) FROM pages
             WHERE is_indexable = 0 AND indexability_reason IS NOT NULL GROUP BY 1",
        )?,
        issues_by_rule: group_map(&conn, "SELECT rule_id, SUM(n) FROM v_issue_summary GROUP BY 1")?,
        exclusions: group_map(
            &conn,
            "SELECT exclusion_reason, COUNT(*) FROM urls
             WHERE exclusion_reason IS NOT NULL GROUP BY 1",
        )?,
        note: config.note,
    })
}

/// Añade un registro al histórico. Crea el fichero si no existe.
pub fn append(record: &BenchRecord, path: &Path) -> Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

/// Lee el histórico completo.
pub fn load(path: &Path) -> Result<Vec<BenchRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path)?;
    let mut records = Vec::new();
    for (i, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(r) => records.push(r),
            // Una línea corrupta no debe invalidar un histórico de meses.
            Err(e) => tracing::warn!(line = i + 1, error = %e, "unreadable record; skipped"),
        }
    }
    Ok(records)
}

/// Imprime el histórico como tabla comparativa.
pub fn print_history(records: &[BenchRecord]) {
    if records.is_empty() {
        println!("No records yet.");
        return;
    }

    println!(
        "{:<20} {:<12} {:<28} {:>7} {:>9} {:>10} {:>9} {:>8}",
        "date", "tool", "target", "conc", "URLs", "URL/s", "HTML/s", "RSS MB"
    );
    println!("{}", "─".repeat(112));

    for r in records {
        println!(
            "{:<20} {:<12} {:<28} {:>7} {:>9} {:>10.1} {:>9.1} {:>8.1}",
            &r.timestamp[..r.timestamp.len().min(19)],
            r.tool,
            truncate(&r.target, 28),
            r.concurrency,
            r.urls_fetched,
            r.urls_per_second,
            r.html_pages_per_second,
            r.peak_rss_mb,
        );
    }

    // Comparación con Screaming Frog sobre los mismos sujetos: el criterio de la puerta.
    //
    // Se emparejan **solo registros con la misma concurrencia**. Es la única forma de que la
    // no es una formalidad: sin ese filtro, la mejor marca nuestra (concurrencia 15) se comparaba
    // contra la suya (5) y salía un 16x que no significaba nada.
    println!();
    let mut comparados = 0;
    for target in unique_targets(records) {
        for concurrency in concurrencies_for(records, &target) {
            let ours = best(records, &target, "crawlforge", concurrency);
            let theirs = best(records, &target, "screamingfrog", concurrency);
            if let (Some(a), Some(b)) = (ours, theirs) {
                if b.urls_per_second > 0.0 {
                    let factor = a.urls_per_second / b.urls_per_second;
                    let veredicto = if factor >= 2.0 {
                        "meets the 2x threshold"
                    } else if factor >= 1.5 {
                        "below 2x but above the stop line"
                    } else {
                        "BELOW 1.5x — the project stops"
                    };
                    let ram = if b.peak_rss_mb > 0.0 && a.peak_rss_mb > 0.0 {
                        format!(", {:.0}x less memory", b.peak_rss_mb / a.peak_rss_mb)
                    } else {
                        String::new()
                    };
                    println!(
                        "{target} (concurrency {concurrency}): {factor:.2}x Screaming Frog{ram} — {veredicto}"
                    );
                    comparados += 1;
                }
            }
        }
    }
    if comparados == 0 {
        println!("No comparable pairs: one record from each tool over the same target at the");
        println!("same concurrency is needed.");
    }
}

/// Concurrencias con las que se ha medido un objetivo.
fn concurrencies_for(records: &[BenchRecord], target: &str) -> Vec<u8> {
    let mut v: Vec<u8> =
        records.iter().filter(|r| r.target == target).map(|r| r.concurrency).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn unique_targets(records: &[BenchRecord]) -> Vec<String> {
    let mut targets: Vec<String> = records.iter().map(|r| r.target.clone()).collect();
    targets.sort();
    targets.dedup();
    targets
}

/// Mejor marca de una herramienta sobre un objetivo **a una concurrencia dada**.
fn best<'a>(
    records: &'a [BenchRecord],
    target: &str,
    tool: &str,
    concurrency: u8,
) -> Option<&'a BenchRecord> {
    records
        .iter()
        .filter(|r| r.target == target && r.tool == tool && r.concurrency == concurrency)
        .max_by(|a, b| a.urls_per_second.total_cmp(&b.urls_per_second))
}

fn group_map(conn: &Connection, sql: &str) -> Result<BTreeMap<String, i64>> {
    let mut stmt = conn.prepare(sql)?;
    let mut map = BTreeMap::new();
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (k, v) = row?;
        map.insert(k, v);
    }
    Ok(map)
}

fn utc_now(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |r| r.get(0))?)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(target: &str, tool: &str, rate: f64) -> BenchRecord {
        BenchRecord {
            timestamp: "2026-07-27T10:00:00Z".into(),
            tool: tool.into(),
            tool_version: "0.0.1".into(),
            target: target.into(),
            mode: "http".into(),
            concurrency: 5,
            max_urls: None,
            max_depth: None,
            respect_robots: true,
            sitemaps: true,
            elapsed_secs: 10.0,
            urls_fetched: 1000,
            urls_discovered: 1000,
            urls_errored: 0,
            urls_excluded: 0,
            urls_per_second: rate,
            html_pages_per_second: rate,
            bytes_downloaded: 1000,
            peak_rss_mb: 50.0,
            pages_html: 900,
            pages_indexable: 800,
            links: 5000,
            images: 300,
            truncated: None,
            status_groups: BTreeMap::new(),
            indexability_reasons: BTreeMap::new(),
            issues_by_rule: BTreeMap::new(),
            exclusions: BTreeMap::new(),
            note: None,
        }
    }

    #[test]
    fn un_registro_sobrevive_al_viaje_por_json() {
        let r = record("https://ejemplo.es", "crawlforge", 200.0);
        let json = serde_json::to_string(&r).expect("serialize");
        let back: BenchRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.target, r.target);
        assert_eq!(back.urls_per_second, r.urls_per_second);
    }

    #[test]
    fn el_historico_se_lee_y_se_escribe() {
        let dir = std::env::temp_dir().join(format!("cf-stats-{}", std::process::id()));
        let path = dir.join("resultados.jsonl");
        let _ = std::fs::remove_dir_all(&dir);

        append(&record("https://a.es", "crawlforge", 100.0), &path).expect("write");
        append(&record("https://b.es", "crawlforge", 200.0), &path).expect("append");

        let loaded = load(&path).expect("read");
        assert_eq!(loaded.len(), 2, "the file is append-only, it is not overwritten");
        assert_eq!(loaded[1].target, "https://b.es");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_linea_corrupta_no_invalida_el_historico() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("cf-stats-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("r.jsonl");

        append(&record("https://a.es", "crawlforge", 100.0), &path).expect("write");
        writeln!(
            std::fs::OpenOptions::new().append(true).open(&path).expect("open"),
            "{{ esto no es json"
        )
        .expect("write garbage");
        append(&record("https://b.es", "crawlforge", 200.0), &path).expect("append");

        let loaded = load(&path).expect("read");
        assert_eq!(loaded.len(), 2, "valid lines are preserved");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_historico_inexistente_no_es_un_error() {
        let path = std::env::temp_dir().join("cf-no-existe-jamas.jsonl");
        assert!(load(&path).expect("read").is_empty());
    }

    #[test]
    fn se_queda_con_la_mejor_marca_de_cada_herramienta() {
        let records = vec![
            record("https://a.es", "crawlforge", 100.0),
            record("https://a.es", "crawlforge", 250.0),
            record("https://a.es", "screamingfrog", 80.0),
        ];
        assert_eq!(
            best(&records, "https://a.es", "crawlforge", 5).expect("ours").urls_per_second,
            250.0
        );
        assert_eq!(
            best(&records, "https://a.es", "screamingfrog", 5).expect("theirs").urls_per_second,
            80.0
        );
        assert!(best(&records, "https://otro.es", "crawlforge", 5).is_none());
        assert!(
            best(&records, "https://a.es", "crawlforge", 15).is_none(),
            "must not return records from another concurrency"
        );
    }

    #[test]
    fn no_compara_marcas_de_concurrencias_distintas() {
        // Sin este filtro, nuestra mejor marca a concurrencia 15 se enfrentaba a la suya a 5 y
        // salía un factor de 16x que no medía nada.
        let mut rapida = record("https://a.es", "crawlforge", 300.0);
        rapida.concurrency = 15;
        let records = vec![
            rapida,
            record("https://a.es", "crawlforge", 120.0),
            record("https://a.es", "screamingfrog", 19.0),
        ];
        let nuestra = best(&records, "https://a.es", "crawlforge", 5).expect("at concurrency 5");
        assert_eq!(nuestra.urls_per_second, 120.0, "the concurrency-15 record must not sneak in");
    }

    #[test]
    fn los_objetivos_se_listan_sin_repetir() {
        let records = vec![
            record("https://a.es", "crawlforge", 1.0),
            record("https://a.es", "screamingfrog", 1.0),
            record("https://b.es", "crawlforge", 1.0),
        ];
        assert_eq!(unique_targets(&records), vec!["https://a.es", "https://b.es"]);
    }

    #[test]
    fn acorta_objetivos_largos_sin_romper_caracteres() {
        assert_eq!(truncate("corto", 10), "corto");
        let largo = truncate("https://un-dominio-muy-largo.es/con/ruta", 12);
        assert_eq!(largo.chars().count(), 12);
        assert!(largo.ends_with('…'));
    }
}
