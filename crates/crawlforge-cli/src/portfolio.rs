//! `crawlforge portfolio` — the aggregate panel across many crawl files.
//!
//! A single audit is a snapshot; whoever runs a hundred sites needs to know **what changed and
//! what fails everywhere at once** (`CONVENTIONS.md §1`). This module answers three questions
//! over a set of crawl files:
//!
//! 1. **What changed** — for every site with its `.prev.sqlite` next to it, the aggregate of
//!    what [`crate::diff`] already knows how to compute, ordered by impact: new `critical` and
//!    `high` findings first, everything else after. The comparison itself is
//!    [`crate::diff::compare`] — reimplementing it here would give two commands that disagree.
//! 2. **What fails across the portfolio** — on how many sites each rule fires. When a rule
//!    fires on 9 of 12 sites it is almost never content: it is a shared template or plugin,
//!    one fix that serves nine sites.
//! 3. **One line per site** — URLs, indexables, findings by severity, crawl date.
//!
//! # The three honesty traps, and how they are handled
//!
//! - **A rule that does not appear is not a rule that does not fail.** A truncated or
//!   list-mode crawl does not evaluate the rules in
//!   `crawlforge_rules::REQUIERE_GRAFO_COMPLETO`, so for those rules such a site is counted as
//!   **inconclusive**, never as "does not fire here": "9 of 12 sites (2 inconclusive)".
//! - **Two sites crawled with different catalogs are not comparable without saying so.** If
//!   `rules_version` (or `core_version`) differs across the files, the warning goes at the
//!   top of the panel, not in a footnote: a rule can be missing on a site because it did not
//!   exist when that site was crawled.
//! - **Crawls from very different dates are not a snapshot.** The header always states the
//!   date range, and a spread beyond [`MAX_DATE_SPREAD_DAYS`] adds a warning.
//!
//! And the standing rule for a many-file command: a file that cannot be opened, is not a
//! crawl, or has a newer schema than this build **is reported and skipped** — one bad file
//! must not take down the whole panel.
//!
//! # Cost discipline
//!
//! Every file is opened read-only, queried with aggregates, and closed. Nothing loads a crawl
//! into memory (`CONVENTIONS.md §5.1`), and no query is quadratic on the number of sites: the
//! per-rule matrix is a pass over per-site aggregate maps of at most catalog size.

use crate::i18n::{self, msg};
use anyhow::{bail, Context, Result};
use crawlforge_core::entitlement::{EntitlementSource, Feature};
use rusqlite::{Connection, OpenFlags};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Beyond this many days between the oldest and the newest crawl, the panel warns that it is
/// not a snapshot of the portfolio. A week is the natural cadence of the weekly review this
/// panel exists for; wider than that, "what changed" means a different period on each site.
pub const MAX_DATE_SPREAD_DAYS: i64 = 7;

/// Example URLs listed per new critical/high finding group. Same reasoning as the diff:
/// three make the pattern recognisable and the full list lives one command away.
const MAX_EXAMPLES: usize = 3;

/// Severity identifiers from worst to best — the same ranking the diff uses.
const SEVERITIES: [&str; 5] = ["critical", "high", "medium", "low", "info"];

fn severity_rank(severity: &str) -> usize {
    SEVERITIES.iter().position(|s| *s == severity).unwrap_or(SEVERITIES.len())
}

// ─────────────────────────────────────────────────────────────────── Data model

/// One site of the portfolio: the aggregates of one crawl file.
#[derive(Debug, Clone)]
pub struct SiteSummary {
    pub path: PathBuf,
    /// What names the site in the panel: its `base_url`.
    pub label: String,
    pub started_at: String,
    pub status: String,
    pub truncated: bool,
    pub truncated_reason: Option<String>,
    pub rules_version: String,
    pub core_version: String,
    /// Internal URLs the crawl actually resolved (same universe as the diff: `pending` rows
    /// are queue leftovers, not site size).
    pub urls_total: i64,
    pub indexable: i64,
    /// Findings by severity, indexed like [`SEVERITIES`].
    pub sev_counts: [i64; 5],
    /// Which rules fired on this site: `rule_id → (worst observed severity, findings)`.
    pub fired: BTreeMap<String, (String, i64)>,
    /// The `.prev.sqlite` next to this file, if it exists — the "before" of the comparison.
    pub prev: Option<PathBuf>,
}

impl SiteSummary {
    /// A crawl from which absences cannot be asserted: truncated (including list mode) or
    /// never finished. For the full-graph rules this site is inconclusive, not clean.
    pub fn incomplete(&self) -> bool {
        self.truncated || self.status != "done"
    }
}

/// A path that was given (or found) and did not make it into the panel, with the reason
/// already in the user's language.
#[derive(Debug, Clone)]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: String,
}

/// A portfolio-level warning. All of them print at the top of the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioWarning {
    /// Trap 3.2: the files were crawled with different rule catalogs.
    MixedRulesVersions(Vec<String>),
    MixedCoreVersions(Vec<String>),
    /// Trap 3.3: the crawls span too many days to be one snapshot.
    DateSpread { oldest: String, newest: String, days: i64 },
}

impl PortfolioWarning {
    pub fn message(&self, lang: crawlforge_rules::Lang) -> String {
        match self {
            Self::MixedRulesVersions(v) => msg::warn_portfolio_rules_versions(lang, v.join(", ")),
            Self::MixedCoreVersions(v) => msg::warn_portfolio_core_versions(lang, v.join(", ")),
            Self::DateSpread { oldest, newest, days } => {
                msg::warn_portfolio_date_spread(lang, days, oldest, newest)
            }
        }
    }
}

/// One row of the "failing across the portfolio" table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSpread {
    pub rule_id: String,
    /// Worst severity observed for the rule across the portfolio.
    pub severity: String,
    /// Sites where the rule fired.
    pub fired: usize,
    /// Sites where the rule **could not be evaluated**: it needs the complete link graph and
    /// that site's crawl is incomplete. Counting these as "does not fire" would be lying.
    pub inconclusive: usize,
    /// Total findings across the portfolio, for scale.
    pub findings: i64,
}

/// A new critical/high finding group of one compared site.
#[derive(Debug, Clone)]
pub struct NewFindingGroup {
    pub severity: String,
    pub rule_id: String,
    pub count: i64,
    pub examples: Vec<String>,
}

/// What changed on one site since its previous crawl.
#[derive(Debug, Clone)]
pub struct SiteChanges {
    pub label: String,
    pub path: PathBuf,
    pub prev_path: PathBuf,
    /// Whether the diff can assert what it says (no side truncated or unfinished).
    pub conclusive: bool,
    pub new_critical_high: Vec<NewFindingGroup>,
    /// New findings below `high`.
    pub new_other: i64,
    pub resolved: i64,
    pub urls_added: i64,
    pub urls_removed: i64,
    pub status_worse: i64,
    pub indexability_lost: i64,
}

impl SiteChanges {
    fn new_critical(&self) -> i64 {
        self.new_critical_high.iter().filter(|g| g.severity == "critical").map(|g| g.count).sum()
    }
    fn new_high(&self) -> i64 {
        self.new_critical_high.iter().filter(|g| g.severity == "high").map(|g| g.count).sum()
    }
    /// Impact key for ordering: new criticals first, then highs, then the rest of the noise.
    fn impact(&self) -> (i64, i64, i64) {
        (
            self.new_critical(),
            self.new_high(),
            self.new_other + self.status_worse + self.indexability_lost,
        )
    }
}

/// The whole panel, computed once and rendered by each format.
#[derive(Debug, Clone)]
pub struct PortfolioOutcome {
    /// Worst site first (by findings per severity).
    pub sites: Vec<SiteSummary>,
    pub skipped: Vec<Skipped>,
    pub warnings: Vec<PortfolioWarning>,
    /// Rules that fire somewhere, most-spread first.
    pub spread: Vec<RuleSpread>,
    /// Sites with a previous crawl, highest impact first.
    pub changes: Vec<SiteChanges>,
    pub oldest: String,
    pub newest: String,
}

// ───────────────────────────────────────────────────────────────── Entry point

/// The whole subcommand: tier gate, build, render, write or print.
pub fn run(
    source: &dyn EntitlementSource,
    paths: &[PathBuf],
    format: &str,
    out: Option<&Path>,
) -> Result<()> {
    use std::io::IsTerminal;
    run_impl(source, paths, format, out, std::io::stdout().is_terminal())
}

/// [`run`] with the terminal as a parameter, to test both cases.
fn run_impl(
    source: &dyn EntitlementSource,
    paths: &[PathBuf],
    format: &str,
    out: Option<&Path>,
    stdout_is_tty: bool,
) -> Result<()> {
    let lang = i18n::current_lang();
    ensure_tier(source, lang)?;

    // Same contract as `report --format`: an unknown format is a flag-parsing error and stays
    // in English, like clap's own errors.
    enum Format {
        Terminal,
        Markdown,
        Html,
    }
    let format = match format.trim().to_ascii_lowercase().as_str() {
        "terminal" => Format::Terminal,
        "md" | "markdown" => Format::Markdown,
        "html" => Format::Html,
        other => bail!("format not recognised: {other}. Available: terminal (the default), md and html"),
    };
    // An HTML panel on the terminal is unreadable code; same guard as `report --format html`.
    if matches!(format, Format::Html) && out.is_none() && stdout_is_tty {
        bail!(
            "an HTML panel is code for the browser, not something readable here.\n\
             Save it and open it:  crawlforge portfolio <paths…> --format html --out panel.html\n\
             Redirecting also works: … --format html > panel.html"
        );
    }

    let outcome = build(paths)?;
    check_site_cap(source, outcome.sites.len(), lang)?;

    match format {
        Format::Terminal => {
            print_report(&outcome);
            if let Some(path) = out {
                // `--out` with the default format writes the Markdown panel: a terminal dump
                // in a file keeps its box-drawing noise and reads worse than the report form.
                std::fs::write(path, markdown(&outcome, lang))
                    .with_context(|| format!("could not write {}", path.display()))?;
                println!();
                println!("{}", msg::markdown_report_written(lang, path.display()));
            }
        }
        Format::Markdown | Format::Html => {
            let texto = match format {
                Format::Markdown => markdown(&outcome, lang),
                _ => crate::audit_report::html_document(
                    lang,
                    &msg::portfolio_html_title(lang),
                    &crate::audit_report::markdown_body_to_html(&markdown(&outcome, lang)),
                ),
            };
            match out {
                Some(path) => {
                    std::fs::write(path, &texto)
                        .with_context(|| format!("could not write {}", path.display()))?;
                    println!("{}", msg::report_written(lang, path.display()));
                }
                None => print!("{texto}"),
            }
        }
    }
    Ok(())
}

/// The feature gate. The portfolio panel is not part of the free tier; the numeric cap in
/// `Limits::max_portfolio_sites` is [`check_site_cap`]'s job once access is granted.
pub fn ensure_tier(source: &dyn EntitlementSource, lang: crawlforge_rules::Lang) -> Result<()> {
    if !source.is_feature_enabled(Feature::Portfolio) {
        bail!(msg::error_portfolio_tier(lang));
    }
    Ok(())
}

/// The numeric cap of the tier, applied to the sites that actually made it into the panel.
/// `None` means "no limit" — the only thing it means (`entitlement.rs`).
pub fn check_site_cap(
    source: &dyn EntitlementSource,
    n_sites: usize,
    lang: crawlforge_rules::Lang,
) -> Result<()> {
    if let Some(max) = source.limits().max_portfolio_sites {
        if n_sites > max as usize {
            bail!(msg::error_portfolio_too_many(lang, n_sites, max));
        }
    }
    Ok(())
}

/// Computes the whole panel from files and directories. Pure of presentation: rendering is
/// [`print_report`] and [`markdown`].
pub fn build(paths: &[PathBuf]) -> Result<PortfolioOutcome> {
    let lang = i18n::current_lang();
    let (candidates, mut skipped) = collect_inputs(paths, lang);

    let mut sites: Vec<SiteSummary> = Vec::new();
    for path in candidates {
        // One bad file must not take down the panel: the reason is kept and shown, and the
        // rest of the portfolio is still computed.
        match read_site(&path, lang) {
            Ok(site) => sites.push(site),
            Err(e) => skipped.push(Skipped { path, reason: format!("{e:#}") }),
        }
    }
    if sites.is_empty() {
        bail!(msg::error_portfolio_no_sites(lang));
    }

    let warnings = collect_warnings(&sites);
    let (oldest, newest) = date_range(&sites);
    let spread = rule_spread(&sites);
    let changes = collect_changes(&sites, &mut skipped, lang);

    // Worst site first: the glance table exists to say which site is worst.
    sites.sort_by(|a, b| b.sev_counts.cmp(&a.sev_counts).then_with(|| a.label.cmp(&b.label)));

    Ok(PortfolioOutcome { sites, skipped, warnings, spread, changes, oldest, newest })
}

// ──────────────────────────────────────────────────────────────────── Inputs

/// Is this file a `.prev.sqlite` — the "before" of the crawl next to it, never an input.
fn is_prev(path: &Path) -> bool {
    path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.ends_with(".prev"))
}

/// The `.prev.sqlite` that `crawl` keeps next to a re-crawled file, if it exists. Mirrors the
/// naming of the rotation in `main.rs` (`crawl-x.sqlite` → `crawl-x.prev.sqlite`).
fn prev_of(path: &Path) -> Option<PathBuf> {
    let (stem, ext) = (
        path.file_stem().and_then(|s| s.to_str())?,
        path.extension().and_then(|e| e.to_str())?,
    );
    let prev = path.with_file_name(format!("{stem}.prev.{ext}"));
    prev.is_file().then_some(prev)
}

/// Expands directories into their `*.sqlite` files and filters out what is not an input.
/// Never fails as a whole: what cannot be used is returned with its reason.
fn collect_inputs(
    paths: &[PathBuf],
    lang: crawlforge_rules::Lang,
) -> (Vec<PathBuf>, Vec<Skipped>) {
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    for path in paths {
        if path.is_dir() {
            // A directory is scanned recursively for *.sqlite; its .prev.sqlite files are not
            // inputs (they are found again as the "before" of their pair) and are skipped
            // silently — inside a directory they are expected, not a user mistake.
            walk_sqlite(path, &mut files);
        } else if is_prev(path) {
            // Explicitly given: saying why it is not counted beats silently dropping it.
            skipped
                .push(Skipped { path: path.clone(), reason: msg::portfolio_prev_not_input(lang) });
        } else if path.is_file() {
            files.push(path.clone());
        } else {
            skipped.push(Skipped {
                path: path.clone(),
                reason: msg::error_store_missing(lang, path.display()),
            });
        }
    }
    files.sort();
    files.dedup();
    (files, skipped)
}

/// Recursive `*.sqlite` scan. Read errors on a subdirectory are ignored on purpose: a
/// directory that cannot be listed contributes no files, and the panel says what it found.
fn walk_sqlite(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_sqlite(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("sqlite") && !is_prev(&path) {
            out.push(path);
        }
    }
}

// ─────────────────────────────────────────────────────────────── Reading a site

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
        [table, column],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Reads one crawl file's aggregates. Opens read-only, queries aggregates, closes — the file
/// never loads into memory whole, whatever its size.
fn read_site(path: &Path, lang: crawlforge_rules::Lang) -> Result<SiteSummary> {
    // Identify before working: the error must talk about files and commands, not tables.
    crate::store_check::ensure_crawl_store(path)?;

    let uri = crate::diff::read_only_uri(path)?;
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open {} read-only", path.display()))?;

    // A schema newer than this build cannot be trusted to mean what our queries assume.
    let schema_version: i64 =
        conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))?;
    if schema_version > crawlforge_core::SCHEMA_VERSION {
        bail!(msg::error_portfolio_newer_schema(
            lang,
            schema_version,
            crawlforge_core::SCHEMA_VERSION
        ));
    }

    // `truncated` arrived with migration 002; a year-old crawl must still open.
    let truncated_col = has_column(&conn, "crawl_meta", "truncated")?;
    let sql = format!(
        "SELECT base_url, started_at, status, core_version, rules_version, {}, {}
         FROM crawl_meta LIMIT 1",
        if truncated_col { "truncated" } else { "0" },
        if truncated_col { "truncated_reason" } else { "NULL" },
    );
    // Every string that ends up on a screen is filtered: the file is untrusted input
    // (`audit_report::strip_control_chars`).
    let clean = crate::audit_report::strip_control_chars;
    struct Meta {
        base_url: String,
        started_at: String,
        status: String,
        core_version: String,
        rules_version: String,
        truncated: bool,
        truncated_reason: Option<String>,
    }
    let meta = conn.query_row(&sql, [], |r| {
        Ok(Meta {
            base_url: r.get(0)?,
            started_at: r.get(1)?,
            status: r.get(2)?,
            core_version: r.get(3)?,
            rules_version: r.get(4)?,
            truncated: r.get::<_, i64>(5)? != 0,
            truncated_reason: r.get(6)?,
        })
    })?;

    let urls_total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM urls WHERE is_internal = 1 AND crawl_state <> 'pending'",
        [],
        |r| r.get(0),
    )?;
    let indexable: i64 =
        conn.query_row("SELECT COUNT(*) FROM pages WHERE is_indexable = 1", [], |r| r.get(0))?;

    // One aggregate query per site; the map is at most catalog-sized. This is what keeps the
    // panel linear on the number of sites.
    let mut sev_counts = [0i64; 5];
    let mut fired: BTreeMap<String, (String, i64)> = BTreeMap::new();
    let mut stmt =
        conn.prepare("SELECT rule_id, severity, COUNT(*) FROM issues GROUP BY rule_id, severity")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
    })?;
    for row in rows {
        let (rule_id, severity, count) = row?;
        if let Some(idx) = SEVERITIES.iter().position(|s| *s == severity) {
            sev_counts[idx] += count;
        }
        let entry =
            fired.entry(clean(&rule_id)).or_insert_with(|| (severity.clone(), 0));
        if severity_rank(&severity) < severity_rank(&entry.0) {
            entry.0 = severity.clone();
        }
        entry.1 += count;
    }

    Ok(SiteSummary {
        path: path.to_path_buf(),
        label: clean(&meta.base_url),
        started_at: clean(&meta.started_at),
        status: clean(&meta.status),
        truncated: meta.truncated,
        truncated_reason: meta.truncated_reason.map(|s| clean(&s)),
        rules_version: clean(&meta.rules_version),
        core_version: clean(&meta.core_version),
        urls_total,
        indexable,
        sev_counts,
        fired,
        prev: prev_of(path),
    })
}

// ─────────────────────────────────────────────────────────── Portfolio warnings

fn collect_warnings(sites: &[SiteSummary]) -> Vec<PortfolioWarning> {
    let mut warnings = Vec::new();

    let mut rules_versions: Vec<String> =
        sites.iter().map(|s| s.rules_version.clone()).collect();
    rules_versions.sort();
    rules_versions.dedup();
    if rules_versions.len() > 1 {
        warnings.push(PortfolioWarning::MixedRulesVersions(rules_versions));
    }

    let mut core_versions: Vec<String> = sites.iter().map(|s| s.core_version.clone()).collect();
    core_versions.sort();
    core_versions.dedup();
    if core_versions.len() > 1 {
        warnings.push(PortfolioWarning::MixedCoreVersions(core_versions));
    }

    let (oldest, newest) = date_range(sites);
    if let Some(days) = days_between(&oldest, &newest) {
        if days > MAX_DATE_SPREAD_DAYS {
            warnings.push(PortfolioWarning::DateSpread { oldest, newest, days });
        }
    }
    warnings
}

/// Oldest and newest `started_at`. ISO 8601 from the engine, so text order is time order.
fn date_range(sites: &[SiteSummary]) -> (String, String) {
    let mut dates: Vec<&str> =
        sites.iter().map(|s| s.started_at.as_str()).filter(|s| !s.is_empty()).collect();
    dates.sort_unstable();
    match (dates.first(), dates.last()) {
        (Some(first), Some(last)) => (day_of(first), day_of(last)),
        _ => (String::new(), String::new()),
    }
}

/// The `YYYY-MM-DD` day of an ISO timestamp. The hour is noise at portfolio scale.
fn day_of(iso: &str) -> String {
    iso.chars().take(10).collect()
}

/// Whole days between two `YYYY-MM-DD` days. `None` when either does not parse — a file is
/// untrusted input and a made-up date must not fabricate (or hide) a warning.
fn days_between(oldest: &str, newest: &str) -> Option<i64> {
    Some(civil_day(newest)? - civil_day(oldest)?)
}

/// Days since the civil epoch, from Howard Hinnant's `days_from_civil` — the standard clean
/// way to count calendar days without pulling a date crate into a closed stack
/// (`CONVENTIONS.md §3`).
fn civil_day(day: &str) -> Option<i64> {
    let mut parts = day.splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

// ─────────────────────────────────────────────────────────────── Rule spread

/// On how many sites each rule fires, and on how many it could not be evaluated.
///
/// Only rules that fire somewhere get a row: a table with the whole catalog at zero would
/// bury the signal. The inconclusive count is trap 3.1: a full-graph rule on an incomplete
/// crawl was never evaluated, and that site can neither confirm nor deny it.
fn rule_spread(sites: &[SiteSummary]) -> Vec<RuleSpread> {
    let mut map: BTreeMap<String, RuleSpread> = BTreeMap::new();
    for site in sites {
        for (rule_id, (severity, count)) in &site.fired {
            let row = map.entry(rule_id.clone()).or_insert_with(|| RuleSpread {
                rule_id: rule_id.clone(),
                severity: severity.clone(),
                fired: 0,
                inconclusive: 0,
                findings: 0,
            });
            row.fired += 1;
            row.findings += count;
            if severity_rank(severity) < severity_rank(&row.severity) {
                row.severity = severity.clone();
            }
        }
    }
    for row in map.values_mut() {
        if crawlforge_rules::requiere_grafo_completo(&row.rule_id) {
            row.inconclusive = sites
                .iter()
                .filter(|s| s.incomplete() && !s.fired.contains_key(&row.rule_id))
                .count();
        }
    }
    let mut out: Vec<RuleSpread> = map.into_values().collect();
    out.sort_by(|a, b| {
        b.fired
            .cmp(&a.fired)
            .then_with(|| severity_rank(&a.severity).cmp(&severity_rank(&b.severity)))
            .then_with(|| a.rule_id.cmp(&b.rule_id))
    });
    out
}

// ─────────────────────────────────────────────────────────────── What changed

/// Runs the diff of every site that has its `.prev.sqlite` and aggregates it by impact.
/// A pair whose comparison fails is reported and does not take down the panel.
fn collect_changes(
    sites: &[SiteSummary],
    skipped: &mut Vec<Skipped>,
    lang: crawlforge_rules::Lang,
) -> Vec<SiteChanges> {
    let mut changes = Vec::new();
    for site in sites {
        let Some(prev) = &site.prev else { continue };
        match crate::diff::compare(prev, &site.path, None, &[]) {
            Ok(outcome) => changes.push(aggregate_changes(site, prev, &outcome)),
            Err(e) => skipped.push(Skipped {
                path: site.path.clone(),
                reason: msg::portfolio_pair_failed(lang, format!("{e:#}")),
            }),
        }
    }
    changes.sort_by(|a, b| b.impact().cmp(&a.impact()).then_with(|| a.label.cmp(&b.label)));
    changes
}

fn aggregate_changes(
    site: &SiteSummary,
    prev: &Path,
    outcome: &crate::diff::DiffOutcome,
) -> SiteChanges {
    use crate::diff::ChangeType;

    // New critical/high findings, grouped by (severity, rule) with a few example URLs.
    let mut groups: BTreeMap<(usize, String), NewFindingGroup> = BTreeMap::new();
    let mut new_other = 0i64;
    for change in outcome.of(ChangeType::IssueAppeared) {
        let severity = change.severity.clone().unwrap_or_default();
        let rule_id = change.field.clone().unwrap_or_default();
        if severity_rank(&severity) > severity_rank("high") {
            new_other += 1;
            continue;
        }
        let group = groups
            .entry((severity_rank(&severity), rule_id.clone()))
            .or_insert_with(|| NewFindingGroup { severity, rule_id, count: 0, examples: Vec::new() });
        group.count += 1;
        if group.examples.len() < MAX_EXAMPLES {
            if let Some(url) = &change.url {
                group.examples.push(url.clone());
            }
        }
    }

    let status_worse = outcome
        .of(ChangeType::StatusChanged)
        .filter(|c| {
            crate::diff::status_rank(c.value_after.as_deref())
                > crate::diff::status_rank(c.value_before.as_deref())
        })
        .count() as i64;

    SiteChanges {
        label: site.label.clone(),
        path: site.path.clone(),
        prev_path: prev.to_path_buf(),
        conclusive: outcome.conclusive(),
        new_critical_high: groups.into_values().collect(),
        new_other,
        resolved: outcome.count(ChangeType::IssueResolved) as i64,
        urls_added: outcome.count(ChangeType::UrlAdded) as i64,
        urls_removed: outcome.count(ChangeType::UrlRemoved) as i64,
        status_worse,
        indexability_lost: outcome.count(ChangeType::IndexabilityLost) as i64,
    }
}

// ─────────────────────────────────────────────────────────────── Presentation

/// The completeness flag a site carries everywhere it is named. Truncated, list-mode and
/// unfinished crawls must be visibly different from complete ones — it is the same honesty
/// the single-file `report` already applies.
fn completeness_flag(site: &SiteSummary, lang: crawlforge_rules::Lang) -> Option<String> {
    if site.truncated {
        if site.truncated_reason.as_deref() == Some("list_mode") {
            return Some(msg::flag_list_mode(lang));
        }
        return Some(msg::flag_truncated(lang));
    }
    if site.status != "done" {
        return Some(msg::flag_unfinished(lang, &site.status));
    }
    None
}

/// Terminal panel. Warnings first — a conclusion the warnings invalidate must not be read
/// before them — then the three queries in the order of the module doc.
pub fn print_report(outcome: &PortfolioOutcome) {
    let lang = i18n::current_lang();
    let n = |v: i64| i18n::count(lang, v);

    println!("{}", i18n::section(&msg::portfolio_title(lang)));
    println!(
        "  {} · {}",
        msg::portfolio_sites_count(lang, outcome.sites.len()),
        msg::portfolio_range(lang, &outcome.oldest, &outcome.newest)
    );

    if !outcome.warnings.is_empty() {
        println!();
        println!("{}", i18n::section(&msg::warnings_title(lang)));
        for warning in &outcome.warnings {
            println!("  {:<9} {}", msg::tag_warning(lang), warning.message(lang));
        }
    }

    if !outcome.skipped.is_empty() {
        println!();
        println!("{}", i18n::section(&msg::portfolio_skipped_title(lang)));
        for skip in &outcome.skipped {
            println!("  {}", skip.path.display());
            for line in skip.reason.lines() {
                println!("      {line}");
            }
        }
    }

    // 1. What changed.
    println!();
    println!("{}", i18n::section(&msg::portfolio_changes_title(lang)));
    if outcome.changes.is_empty() {
        for line in msg::portfolio_no_pairs(lang).lines() {
            println!("  {line}");
        }
    } else {
        println!(
            "  {}",
            msg::portfolio_pairs_line(lang, outcome.changes.len(), outcome.sites.len())
        );
        println!();
        println!("  {}:", msg::portfolio_new_critical_high(lang));
        let mut any = false;
        for change in &outcome.changes {
            if change.new_critical_high.is_empty() {
                continue;
            }
            any = true;
            println!("    {}", change.label);
            for group in &change.new_critical_high {
                println!(
                    "      {:<9} {:<30} {:>6}",
                    i18n::severity_word(lang, &group.severity),
                    group.rule_id,
                    n(group.count)
                );
                for url in &group.examples {
                    println!("        {url}");
                }
            }
        }
        if !any {
            println!("    {}", msg::portfolio_none_critical_high(lang));
        }
        println!();
        println!("  {}:", msg::portfolio_rest_title(lang));
        for change in &outcome.changes {
            let mut parts: Vec<String> = Vec::new();
            let mut push = |label: String, value: i64| {
                if value > 0 {
                    parts.push(format!("{label} {}", n(value)));
                }
            };
            push(msg::label_new_findings(lang), change.new_other);
            push(msg::label_findings_resolved(lang), change.resolved);
            push(msg::label_status_worse(lang), change.status_worse);
            push(msg::label_pages_lost_index(lang), change.indexability_lost);
            push(msg::label_new_urls(lang), change.urls_added);
            push(msg::label_urls_gone(lang), change.urls_removed);
            let detail = if parts.is_empty() && change.new_critical_high.is_empty() {
                msg::portfolio_no_changes(lang)
            } else {
                parts.join(" · ")
            };
            println!("    {}", change.label);
            if !detail.is_empty() {
                println!("      {detail}");
            }
            if !change.conclusive {
                println!(
                    "      {:<15} {}",
                    msg::tag_inconclusive(lang),
                    msg::portfolio_pair_inconclusive(lang)
                );
            }
        }
        if let Some(change) = outcome.changes.first() {
            println!();
            println!("  {}", msg::hint_site_diff(lang));
            println!(
                "    crawlforge diff {} {}",
                change.prev_path.display(),
                change.path.display()
            );
        }
    }

    // 2. What fails across the portfolio.
    println!();
    println!("{}", i18n::section(&msg::portfolio_spread_title(lang)));
    if outcome.spread.is_empty() {
        println!("  {}", msg::portfolio_spread_none(lang));
    } else {
        for line in msg::portfolio_spread_intro(lang).lines() {
            println!("  {line}");
        }
        println!();
        for row in &outcome.spread {
            println!(
                "  {:<9} {:<30} {}{}",
                i18n::severity_word(lang, &row.severity),
                row.rule_id,
                msg::portfolio_sites_of(lang, row.fired, outcome.sites.len()),
                msg::portfolio_inconclusive_suffix(lang, row.inconclusive),
            );
        }
    }

    // 3. One line per site, worst first.
    println!();
    println!("{}", i18n::section(&msg::portfolio_glance_title(lang)));
    // The severity columns keep their identifier abbreviations in both languages, like the
    // `--fail-on` tokens: they are column keys, not prose.
    println!(
        "  {:>9} {:>7} {:>5} {:>5} {:>5} {:>5} {:>5}  {:<10}  {}",
        msg::label_urls(lang),
        msg::th_indexable(lang),
        "crit",
        "high",
        "med",
        "low",
        "info",
        msg::th_crawled(lang),
        msg::th_site(lang),
    );
    for site in &outcome.sites {
        let flag = completeness_flag(site, lang).map(|f| format!("  {f}")).unwrap_or_default();
        println!(
            "  {:>9} {:>7} {:>5} {:>5} {:>5} {:>5} {:>5}  {:<10}  {}{}",
            n(site.urls_total),
            n(site.indexable),
            n(site.sev_counts[0]),
            n(site.sev_counts[1]),
            n(site.sev_counts[2]),
            n(site.sev_counts[3]),
            n(site.sev_counts[4]),
            day_of(&site.started_at),
            site.label,
            flag,
        );
    }
}

/// The panel as Markdown — the same content as the terminal, in the shape that gets pasted
/// into a ticket or rendered to HTML by `audit_report::markdown_body_to_html`.
pub fn markdown(outcome: &PortfolioOutcome, lang: crawlforge_rules::Lang) -> String {
    let n = |v: i64| i18n::count(lang, v);
    let mut s = String::new();

    s.push_str(&format!("# {}\n\n", msg::portfolio_title(lang)));
    s.push_str(&format!(
        "{} · {}\n\n",
        msg::portfolio_sites_count(lang, outcome.sites.len()),
        msg::portfolio_range(lang, &outcome.oldest, &outcome.newest)
    ));

    for warning in &outcome.warnings {
        s.push_str(&format!("> {}\n\n", warning.message(lang)));
    }
    if !outcome.skipped.is_empty() {
        s.push_str(&format!("## {}\n\n", msg::portfolio_skipped_title(lang)));
        for skip in &outcome.skipped {
            s.push_str(&format!(
                "- `{}` — {}\n",
                skip.path.display(),
                skip.reason.replace('\n', " ")
            ));
        }
        s.push('\n');
    }

    s.push_str(&format!("## {}\n\n", msg::portfolio_changes_title(lang)));
    if outcome.changes.is_empty() {
        s.push_str(&format!("{}\n\n", msg::portfolio_no_pairs(lang).replace('\n', " ")));
    } else {
        s.push_str(&format!(
            "{}\n\n",
            msg::portfolio_pairs_line(lang, outcome.changes.len(), outcome.sites.len())
        ));
        s.push_str(&format!("### {}\n\n", msg::portfolio_new_critical_high(lang)));
        let mut any = false;
        for change in &outcome.changes {
            for group in &change.new_critical_high {
                any = true;
                let examples = if group.examples.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", msg::example_urls(lang, group.examples.join(" · ")))
                };
                s.push_str(&format!(
                    "- {} · **{}** `{}` × {}{examples}\n",
                    change.label,
                    i18n::severity_word(lang, &group.severity),
                    group.rule_id,
                    n(group.count),
                ));
            }
        }
        if !any {
            s.push_str(&format!("{}\n", msg::portfolio_none_critical_high(lang)));
        }
        s.push('\n');
        s.push_str(&format!("### {}\n\n", msg::portfolio_rest_title(lang)));
        for change in &outcome.changes {
            let mut parts: Vec<String> = Vec::new();
            let mut push = |label: String, value: i64| {
                if value > 0 {
                    parts.push(format!("{label} {}", n(value)));
                }
            };
            push(msg::label_new_findings(lang), change.new_other);
            push(msg::label_findings_resolved(lang), change.resolved);
            push(msg::label_status_worse(lang), change.status_worse);
            push(msg::label_pages_lost_index(lang), change.indexability_lost);
            push(msg::label_new_urls(lang), change.urls_added);
            push(msg::label_urls_gone(lang), change.urls_removed);
            let detail = if parts.is_empty() && change.new_critical_high.is_empty() {
                msg::portfolio_no_changes(lang)
            } else {
                parts.join(" · ")
            };
            let inconclusive = if change.conclusive {
                String::new()
            } else {
                format!(
                    " — **{}**: {}",
                    msg::tag_inconclusive(lang),
                    msg::portfolio_pair_inconclusive(lang)
                )
            };
            s.push_str(&format!("- {} — {detail}{inconclusive}\n", change.label));
        }
        s.push('\n');
    }

    s.push_str(&format!("## {}\n\n", msg::portfolio_spread_title(lang)));
    if outcome.spread.is_empty() {
        s.push_str(&format!("{}\n\n", msg::portfolio_spread_none(lang)));
    } else {
        s.push_str(&format!("{}\n\n", msg::portfolio_spread_intro(lang).replace('\n', " ")));
        s.push_str(&format!(
            "| {} | {} | {} |\n|---|---|---|\n",
            msg::th_severity(lang),
            "ID",
            msg::th_site(lang)
        ));
        for row in &outcome.spread {
            s.push_str(&format!(
                "| {} | `{}` | {}{} |\n",
                i18n::severity_word(lang, &row.severity),
                row.rule_id,
                msg::portfolio_sites_of(lang, row.fired, outcome.sites.len()),
                msg::portfolio_inconclusive_suffix(lang, row.inconclusive),
            ));
        }
        s.push('\n');
    }

    s.push_str(&format!("## {}\n\n", msg::portfolio_glance_title(lang)));
    s.push_str(&format!(
        "| {} | {} | {} | crit | high | med | low | info | {} |\n\
         |---|---:|---:|---:|---:|---:|---:|---:|---|\n",
        msg::th_site(lang),
        msg::label_urls(lang),
        msg::th_indexable(lang),
        msg::th_crawled(lang),
    ));
    for site in &outcome.sites {
        let flag = completeness_flag(site, lang).map(|f| format!(" {f}")).unwrap_or_default();
        s.push_str(&format!(
            "| {}{flag} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            site.label,
            n(site.urls_total),
            n(site.indexable),
            n(site.sev_counts[0]),
            n(site.sev_counts[1]),
            n(site.sev_counts[2]),
            n(site.sev_counts[3]),
            n(site.sev_counts[4]),
            day_of(&site.started_at),
        ));
    }
    s
}

// ─────────────────────────────────────────────────────────────────────── Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crawlforge_core::entitlement::{DevSource, Tier};
    use crawlforge_rules::Lang;
    use rusqlite::params;

    /// Own temp dir: the CLI has no `tempfile` dependency, the stack is closed
    /// (`CONVENTIONS.md §3`).
    fn tmpdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("crawlforge-portfolio-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// A crawl file with the **real schema** — every published migration, via the shared
    /// helper — not a similar-looking copy that would pass the tests and fail the command.
    fn crawl_file(path: &Path) -> Connection {
        crate::test_schema::crawl_file(path)
    }

    struct Meta<'a> {
        base_url: &'a str,
        started_at: &'a str,
        /// `Some(reason)` marks the crawl truncated by that reason.
        truncated: Option<&'a str>,
        rules_version: &'a str,
        core_version: &'a str,
        status: &'a str,
    }

    impl Default for Meta<'_> {
        fn default() -> Self {
            Self {
                base_url: "https://site.example/",
                started_at: "2026-08-01T10:00:00Z",
                truncated: None,
                rules_version: "0.6.0",
                core_version: "0.6.0",
                status: "done",
            }
        }
    }

    fn meta(conn: &Connection, m: &Meta<'_>) {
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime, truncated, truncated_reason)
             VALUES ('c', 'p', 'P', ?1, 'http', ?2, ?3, '{}', ?4, ?5, 'agency', ?6, ?7)",
            params![
                m.base_url,
                m.started_at,
                m.status,
                m.core_version,
                m.rules_version,
                m.truncated.is_some() as i64,
                m.truncated,
            ],
        )
        .expect("insert crawl_meta");
    }

    fn url(conn: &Connection, id: i64, url: &str, host: &str) -> i64 {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', ?3, '/', 1, 0, 'done', 200)",
            params![id, url, host],
        )
        .expect("insert url");
        id
    }

    fn page(conn: &Connection, url_id: i64, indexable: bool) {
        conn.execute(
            "INSERT INTO pages (url_id, is_indexable, internal_links_in) VALUES (?1, ?2, 0)",
            params![url_id, indexable as i64],
        )
        .expect("insert page");
    }

    fn issue(conn: &Connection, url_id: i64, rule_id: &str, severity: &str) {
        conn.execute(
            "INSERT INTO issues (url_id, rule_id, severity, category, group_key)
             VALUES (?1, ?2, ?3, 'indexability', NULL)",
            params![url_id, rule_id, severity],
        )
        .expect("insert issue");
    }

    /// One site file with a single page and the given findings.
    fn site_file(
        dir: &Path,
        name: &str,
        m: &Meta<'_>,
        issues: &[(&str, &str)],
    ) -> PathBuf {
        let path = dir.join(name);
        let conn = crawl_file(&path);
        meta(&conn, m);
        let host = m.base_url.trim_start_matches("https://").trim_end_matches('/');
        let id = url(&conn, 1, m.base_url, host);
        page(&conn, id, true);
        for (rule, severity) in issues {
            issue(&conn, id, rule, severity);
        }
        path
    }

    fn spread_row<'a>(outcome: &'a PortfolioOutcome, rule: &str) -> &'a RuleSpread {
        outcome
            .spread
            .iter()
            .find(|r| r.rule_id == rule)
            .unwrap_or_else(|| panic!("no spread row for {rule}"))
    }

    // ── Trap 3.1: a rule that does not appear is not a rule that does not fail ──

    #[test]
    fn a_graph_rule_on_a_truncated_site_is_inconclusive_not_clean() {
        let dir = tmpdir("inconclusive");
        // Two complete sites where the orphan rule fires, and a truncated one where it does
        // not appear — because it was never evaluated there, not because it passed.
        let a = site_file(
            &dir,
            "a.sqlite",
            &Meta { base_url: "https://a.example/", ..Meta::default() },
            &[("INDEX-ORPHAN-PAGE", "high"), ("META-TITLE-TOO-LONG", "medium")],
        );
        let b = site_file(
            &dir,
            "b.sqlite",
            &Meta { base_url: "https://b.example/", ..Meta::default() },
            &[("INDEX-ORPHAN-PAGE", "high")],
        );
        let c = site_file(
            &dir,
            "c.sqlite",
            &Meta {
                base_url: "https://c.example/",
                truncated: Some("max_urls"),
                ..Meta::default()
            },
            &[],
        );

        let outcome = build(&[a, b, c]).expect("build the panel");

        let orphan = spread_row(&outcome, "INDEX-ORPHAN-PAGE");
        assert_eq!(orphan.fired, 2);
        assert_eq!(
            orphan.inconclusive, 1,
            "the truncated site never evaluated the full-graph rule: counting it as \
             'does not fire here' would be lying"
        );

        // A rule that does not need the full graph *was* evaluated on the truncated site:
        // its absence there is a real absence, not an unknown.
        let title = spread_row(&outcome, "META-TITLE-TOO-LONG");
        assert_eq!(title.fired, 1);
        assert_eq!(title.inconclusive, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_list_mode_crawl_is_also_inconclusive_for_graph_rules() {
        // `list_mode` is stored as a truncation reason but is not a cut; what it shares with
        // a cut is exactly this: the full-graph rules were not evaluated.
        let dir = tmpdir("list-mode");
        let a = site_file(
            &dir,
            "a.sqlite",
            &Meta { base_url: "https://a.example/", ..Meta::default() },
            &[("INDEX-ORPHAN-PAGE", "high")],
        );
        let b = site_file(
            &dir,
            "b.sqlite",
            &Meta {
                base_url: "https://b.example/",
                truncated: Some("list_mode"),
                ..Meta::default()
            },
            &[],
        );

        let outcome = build(&[a, b]).expect("build the panel");
        let orphan = spread_row(&outcome, "INDEX-ORPHAN-PAGE");
        assert_eq!((orphan.fired, orphan.inconclusive), (1, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Trap 3.2: different catalogs are not comparable without saying so ──────

    #[test]
    fn mixed_rule_catalogs_raise_a_warning_at_the_top() {
        let dir = tmpdir("mixed-rules");
        let a = site_file(
            &dir,
            "a.sqlite",
            &Meta { base_url: "https://a.example/", rules_version: "0.5.0", ..Meta::default() },
            &[("META-TITLE-TOO-LONG", "medium")],
        );
        let b = site_file(
            &dir,
            "b.sqlite",
            &Meta { base_url: "https://b.example/", rules_version: "0.6.0", ..Meta::default() },
            &[],
        );

        let outcome = build(&[a.clone(), b]).expect("build the panel");
        assert!(
            outcome
                .warnings
                .iter()
                .any(|w| matches!(w, PortfolioWarning::MixedRulesVersions(v)
                    if v == &["0.5.0".to_string(), "0.6.0".to_string()])),
            "a rule can be missing on a site because it did not exist when it was crawled: \
             {:?}",
            outcome.warnings
        );

        // Same catalog everywhere: no warning to raise.
        let c = site_file(
            &dir,
            "c.sqlite",
            &Meta { base_url: "https://c.example/", rules_version: "0.5.0", ..Meta::default() },
            &[],
        );
        let outcome = build(&[a, c]).expect("build the panel");
        assert!(
            !outcome.warnings.iter().any(|w| matches!(w, PortfolioWarning::MixedRulesVersions(_))),
            "{:?}",
            outcome.warnings
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Trap 3.3: crawls weeks apart are not a snapshot ────────────────────────

    #[test]
    fn crawls_weeks_apart_raise_a_date_spread_warning() {
        let dir = tmpdir("date-spread");
        let a = site_file(
            &dir,
            "a.sqlite",
            &Meta {
                base_url: "https://a.example/",
                started_at: "2026-07-01T10:00:00Z",
                ..Meta::default()
            },
            &[],
        );
        let b = site_file(
            &dir,
            "b.sqlite",
            &Meta {
                base_url: "https://b.example/",
                started_at: "2026-08-01T09:00:00Z",
                ..Meta::default()
            },
            &[],
        );

        let outcome = build(&[a, b]).expect("build the panel");
        assert_eq!(outcome.oldest, "2026-07-01");
        assert_eq!(outcome.newest, "2026-08-01");
        assert!(
            outcome.warnings.iter().any(
                |w| matches!(w, PortfolioWarning::DateSpread { days, .. } if *days == 31)
            ),
            "31 days apart is not a snapshot of the portfolio: {:?}",
            outcome.warnings
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crawls_within_a_week_do_not_warn_about_dates() {
        let dir = tmpdir("date-close");
        let a = site_file(
            &dir,
            "a.sqlite",
            &Meta {
                base_url: "https://a.example/",
                started_at: "2026-08-01T10:00:00Z",
                ..Meta::default()
            },
            &[],
        );
        let b = site_file(
            &dir,
            "b.sqlite",
            &Meta {
                base_url: "https://b.example/",
                started_at: "2026-08-04T09:00:00Z",
                ..Meta::default()
            },
            &[],
        );
        let outcome = build(&[a, b]).expect("build the panel");
        assert!(
            !outcome.warnings.iter().any(|w| matches!(w, PortfolioWarning::DateSpread { .. })),
            "{:?}",
            outcome.warnings
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── The .prev.sqlite is the "before" of its pair, never an input ───────────

    #[test]
    fn a_prev_file_in_a_directory_is_the_before_of_its_pair_not_a_site() {
        let dir = tmpdir("prev-pair");
        // The pair: the previous crawl had one finding; the current one adds a critical.
        site_file(
            &dir,
            "site.prev.sqlite",
            &Meta { started_at: "2026-07-25T10:00:00Z", ..Meta::default() },
            &[("META-TITLE-TOO-LONG", "medium")],
        );
        site_file(
            &dir,
            "site.sqlite",
            &Meta::default(),
            &[("META-TITLE-TOO-LONG", "medium"), ("HTTP-404-INTERNAL", "critical")],
        );

        let outcome = build(std::slice::from_ref(&dir)).expect("build the panel");

        assert_eq!(outcome.sites.len(), 1, "the .prev.sqlite must not count as a site");
        assert_eq!(outcome.changes.len(), 1, "the pair must be compared");
        let change = &outcome.changes[0];
        assert!(change.conclusive);
        assert_eq!(change.new_critical_high.len(), 1);
        assert_eq!(change.new_critical_high[0].rule_id, "HTTP-404-INTERNAL");
        assert_eq!(change.new_critical_high[0].count, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_explicit_prev_file_is_set_aside_with_its_reason() {
        let dir = tmpdir("prev-explicit");
        let prev = site_file(&dir, "site.prev.sqlite", &Meta::default(), &[]);
        let site = site_file(&dir, "site.sqlite", &Meta::default(), &[]);

        let outcome = build(&[site, prev.clone()]).expect("build the panel");
        assert_eq!(outcome.sites.len(), 1);
        assert!(
            outcome.skipped.iter().any(|s| s.path == prev),
            "an explicitly given .prev.sqlite is set aside and the reason is said"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_truncated_side_makes_the_pair_inconclusive() {
        // The diff already suppresses what an incomplete crawl cannot assert; the panel must
        // carry that flag instead of presenting the pair as a firm comparison.
        let dir = tmpdir("pair-truncated");
        site_file(
            &dir,
            "site.prev.sqlite",
            &Meta { started_at: "2026-07-25T10:00:00Z", ..Meta::default() },
            &[],
        );
        site_file(
            &dir,
            "site.sqlite",
            &Meta { truncated: Some("max_urls"), ..Meta::default() },
            &[],
        );
        let outcome = build(std::slice::from_ref(&dir)).expect("build the panel");
        assert_eq!(outcome.changes.len(), 1);
        assert!(!outcome.changes[0].conclusive);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Ordering: impact first ─────────────────────────────────────────────────

    #[test]
    fn changes_are_ordered_by_impact_new_criticals_first() {
        let dir = tmpdir("impact-order");
        // Site "quiet" gains a medium finding; site "broken" gains a critical one. Whatever
        // the alphabetical order says, "broken" must come first.
        site_file(
            &dir,
            "a-quiet.prev.sqlite",
            &Meta { base_url: "https://quiet.example/", started_at: "2026-07-25T10:00:00Z", ..Meta::default() },
            &[],
        );
        site_file(
            &dir,
            "a-quiet.sqlite",
            &Meta { base_url: "https://quiet.example/", ..Meta::default() },
            &[("META-TITLE-TOO-LONG", "medium")],
        );
        site_file(
            &dir,
            "z-broken.prev.sqlite",
            &Meta { base_url: "https://broken.example/", started_at: "2026-07-25T10:00:00Z", ..Meta::default() },
            &[],
        );
        site_file(
            &dir,
            "z-broken.sqlite",
            &Meta { base_url: "https://broken.example/", ..Meta::default() },
            &[("HTTP-404-INTERNAL", "critical")],
        );

        let outcome = build(std::slice::from_ref(&dir)).expect("build the panel");
        let order: Vec<&str> = outcome.changes.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            order,
            ["https://broken.example/", "https://quiet.example/"],
            "a new critical outranks a new medium, whatever the file names say"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_glance_table_puts_the_worst_site_first() {
        let dir = tmpdir("glance-order");
        let a = site_file(
            &dir,
            "a.sqlite",
            &Meta { base_url: "https://a.example/", ..Meta::default() },
            &[("META-TITLE-TOO-LONG", "medium")],
        );
        let b = site_file(
            &dir,
            "b.sqlite",
            &Meta { base_url: "https://b.example/", ..Meta::default() },
            &[("HTTP-404-INTERNAL", "critical")],
        );
        let outcome = build(&[a, b]).expect("build the panel");
        assert_eq!(outcome.sites[0].label, "https://b.example/");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_spread_is_ordered_by_number_of_sites() {
        let dir = tmpdir("spread-order");
        let mut paths = Vec::new();
        for (i, issues) in [
            &[("META-TITLE-TOO-LONG", "medium"), ("HTTP-404-INTERNAL", "critical")][..],
            &[("META-TITLE-TOO-LONG", "medium")][..],
            &[("META-TITLE-TOO-LONG", "medium")][..],
        ]
        .iter()
        .enumerate()
        {
            let base = format!("https://s{i}.example/");
            paths.push(site_file(
                &dir,
                &format!("s{i}.sqlite"),
                &Meta { base_url: &base, ..Meta::default() },
                issues,
            ));
        }
        let outcome = build(&paths).expect("build the panel");
        assert_eq!(outcome.spread[0].rule_id, "META-TITLE-TOO-LONG", "3 sites beat 1 site");
        assert_eq!(outcome.spread[0].fired, 3);
        assert_eq!(outcome.spread[1].rule_id, "HTTP-404-INTERNAL");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── One bad file must not take down the panel ──────────────────────────────

    #[test]
    fn an_unreadable_file_is_reported_and_the_rest_of_the_panel_survives() {
        let dir = tmpdir("bad-file");
        let good = site_file(&dir, "good.sqlite", &Meta::default(), &[]);
        let bad = dir.join("notes.sqlite");
        std::fs::write(&bad, "a text file with a lying extension\n").expect("write");
        let missing = dir.join("never-crawled.sqlite");

        let outcome =
            build(&[good, bad.clone(), missing.clone()]).expect("the panel must survive");
        assert_eq!(outcome.sites.len(), 1);
        let skipped: Vec<&PathBuf> = outcome.skipped.iter().map(|s| &s.path).collect();
        assert!(skipped.contains(&&bad), "the fake SQLite is set aside: {skipped:?}");
        assert!(skipped.contains(&&missing), "the missing file is set aside: {skipped:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_schema_newer_than_the_binary_is_said_and_skipped() {
        let dir = tmpdir("newer-schema");
        let good = site_file(&dir, "good.sqlite", &Meta::default(), &[]);
        let newer = site_file(&dir, "newer.sqlite", &Meta::default(), &[]);
        {
            let conn = Connection::open(&newer).expect("reopen");
            conn.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
                [crawlforge_core::SCHEMA_VERSION + 1],
            )
            .expect("bump the schema");
        }

        let outcome = build(&[good, newer.clone()]).expect("the panel must survive");
        assert_eq!(outcome.sites.len(), 1);
        let skip = outcome
            .skipped
            .iter()
            .find(|s| s.path == newer)
            .expect("the newer-schema file is set aside");
        assert!(
            skip.reason.contains("newer"),
            "the reason must say the schema is newer: {}",
            skip.reason
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_portfolio_with_no_usable_file_is_an_error() {
        let dir = tmpdir("all-bad");
        let bad = dir.join("notes.sqlite");
        std::fs::write(&bad, "not sqlite\n").expect("write");
        let err = build(&[bad]).expect_err("nothing usable");
        let text = format!("{err:#}");
        assert!(text.contains("crawlforge crawl"), "says which commands produce inputs: {text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── The tier gate ──────────────────────────────────────────────────────────

    #[test]
    fn the_free_tier_has_no_portfolio_panel() {
        let err = ensure_tier(&DevSource::new(Tier::Free), Lang::En)
            .expect_err("the portfolio panel is not part of the free tier");
        assert!(format!("{err:#}").contains("Pro"), "the error names the tier that has it");
        ensure_tier(&DevSource::new(Tier::Pro), Lang::En).expect("Pro has the panel");
        ensure_tier(&DevSource::new(Tier::Agency), Lang::En).expect("Agency has the panel");
    }

    #[test]
    fn the_pro_tier_caps_the_portfolio_at_its_limit() {
        let pro = DevSource::new(Tier::Pro);
        check_site_cap(&pro, 10, Lang::En).expect("10 sites fit in Pro");
        let err = check_site_cap(&pro, 11, Lang::En).expect_err("11 sites do not fit in Pro");
        assert!(format!("{err:#}").contains("11"), "{err:#}");
        // Agency: `None` means "no limit" — the only thing it means now.
        check_site_cap(&DevSource::new(Tier::Agency), 500, Lang::En)
            .expect("Agency has no site cap");
    }

    // ── Rendering carries the honesty marks ────────────────────────────────────

    #[test]
    fn the_markdown_panel_says_fired_of_total_with_the_inconclusive_count() {
        let dir = tmpdir("md-inconclusive");
        let a = site_file(
            &dir,
            "a.sqlite",
            &Meta { base_url: "https://a.example/", ..Meta::default() },
            &[("INDEX-ORPHAN-PAGE", "high")],
        );
        let b = site_file(
            &dir,
            "b.sqlite",
            &Meta {
                base_url: "https://b.example/",
                truncated: Some("max_urls"),
                ..Meta::default()
            },
            &[],
        );
        let outcome = build(&[a, b]).expect("build the panel");

        let md = markdown(&outcome, Lang::En);
        assert!(md.contains("1 of 2 sites (1 inconclusive)"), "{md}");
        assert!(md.contains("(truncated)"), "the glance row carries the truncation: {md}");

        let md_es = markdown(&outcome, Lang::Es);
        assert!(md_es.contains("1 de 2 sitios (1 no concluyente)"), "{md_es}");
        assert!(md_es.contains("(truncado)"), "{md_es}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_html_panel_is_a_complete_document() {
        let dir = tmpdir("html-doc");
        let a = site_file(&dir, "a.sqlite", &Meta::default(), &[]);
        let outcome = build(&[a]).expect("build the panel");
        let html = crate::audit_report::html_document(
            Lang::En,
            "Portfolio panel",
            &crate::audit_report::markdown_body_to_html(&markdown(&outcome, Lang::En)),
        );
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<table>"), "the glance table renders as a table: {html}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── The date arithmetic the spread warning depends on ──────────────────────

    #[test]
    fn civil_days_count_calendar_days() {
        assert_eq!(days_between("2026-07-01", "2026-08-01"), Some(31));
        assert_eq!(days_between("2026-08-01", "2026-08-04"), Some(3));
        assert_eq!(days_between("2024-02-28", "2024-03-01"), Some(2), "2024 is a leap year");
        assert_eq!(days_between("2025-12-31", "2026-01-01"), Some(1));
        assert_eq!(days_between("garbage", "2026-01-01"), None, "a fabricated date asserts nothing");
    }
}
