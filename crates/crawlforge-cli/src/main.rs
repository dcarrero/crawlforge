//! CLI de CrawlForge: herramienta interna, producto del nivel Agency y banco de pruebas de la
//! Línea de comandos: rastrear, auditar, comparar, exportar.
//!
//! # El idioma de la CLI
//!
//! **Todo lo que imprime esta CLI está en inglés por defecto.** No es una preferencia estética:
//! la plantilla de `clap` («Usage», «Options») y sus errores de parseo («invalid value…») son
//! ingleses y no se pueden localizar, así que el inglés es el único idioma en el que la CLI
//! puede ser coherente de arriba abajo — y además es el idioma de origen del producto
//! (`CONVENTIONS.md §4`: el español es una traducción, no la fuente). El español está disponible
//! donde existe un canal real de localización: `rules --lang es` y `report --lang es`, cuyos
//! textos salen del catálogo de reglas o de tablas de cadenas por idioma, nunca de literales
//! sueltos. Ampliar `--lang` al resto de la salida es trabajo pendiente, no de
//! este fichero.

mod export;
mod report;
mod rules;
mod stats;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use crawlforge_cli::i18n::msg;
use crawlforge_cli::{audit_report, diff, xlsx};
use crawlforge_core::engine::{self, CrawlPhase, CrawlProgress};
use crawlforge_core::entitlement::{DevSource, EntitlementSource};
use crawlforge_core::job::{CrawlJob, CrawlMode, JobConfig};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Histórico de benchmarks por defecto cuando se pide `--bench` sin `--stats`.
const DEFAULT_STATS_FILE: &str = "benchmarks/resultados.jsonl";

#[derive(Parser)]
#[command(name = "crawlforge", version, about = "Technical SEO auditor")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Crawl a site over HTTP.
    ///
    /// For a staging site behind HTTP Basic auth, either put the credential in the URL
    /// (https://user:pass@host/) or set CRAWLFORGE_AUTH=user:password. It is sent only to
    /// that host and never stored in the crawl file.
    Crawl {
        /// Seed URL to start from, e.g. https://example.com/.
        url: String,
        /// Output crawl file (SQLite). Defaults to crawl-<site>.sqlite; if it already
        /// exists, the previous crawl is kept next to it as <name>.prev.sqlite.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// YAML file with crawl settings. Command-line flags override it.
        /// See docs/crawl-config.example.yaml.
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Concurrent requests per host (1..=20) [default: 5].
        #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..=20))]
        concurrency: Option<u8>,
        /// Stop after this many URLs.
        #[arg(long)]
        max_urls: Option<u64>,
        /// Maximum depth in clicks from the seed.
        #[arg(long)]
        max_depth: Option<u32>,
        /// Skip sitemap discovery.
        #[arg(long)]
        no_sitemaps: bool,
        /// Ignore robots.txt. Only for sites you own.
        #[arg(long)]
        ignore_robots: bool,
        /// Only crawl URLs matching this regex (repeatable; a plain string matches anywhere
        /// in the URL). The seed URL is always crawled. Overrides the config file.
        #[arg(long, value_name = "REGEX")]
        include: Vec<String>,
        /// Skip URLs matching this regex (repeatable). Wins over --include. Skipped URLs
        /// are recorded as excluded, not hidden. Overrides the config file.
        #[arg(long, value_name = "REGEX")]
        exclude: Vec<String>,
        /// Also export the results as CSV files into this directory.
        #[arg(long, value_name = "DIR")]
        csv: Option<PathBuf>,
        /// Show engine metrics and append them to the benchmark history.
        #[arg(long)]
        bench: bool,
        /// Benchmark history file. Providing it implies recording the run;
        /// with plain `--bench`, benchmarks/resultados.jsonl is used.
        #[arg(long, value_name = "FILE")]
        stats: Option<PathBuf>,
        /// Free-form note stored with the benchmark record.
        #[arg(long)]
        note: Option<String>,
    },
    /// Audit an already built directory (an Astro `dist/` and the like).
    Audit {
        /// Directory with the built site (the folder that would be uploaded to the server).
        dir: PathBuf,
        /// URL where the site will be published, e.g. https://example.com/. Absolute
        /// canonicals in the site are compared against it, so without the real one the
        /// indexability audit is meaningless.
        #[arg(short, long)]
        base: String,
        /// Output crawl file (SQLite). Defaults to crawl-<dir>.sqlite; if it already
        /// exists, the previous crawl is kept next to it as <name>.prev.sqlite.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// YAML file with crawl settings. Command-line flags override it.
        /// See docs/crawl-config.example.yaml.
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Only audit URLs matching this regex (repeatable; a plain string matches anywhere
        /// in the URL). Overrides the config file.
        #[arg(long, value_name = "REGEX")]
        include: Vec<String>,
        /// Skip URLs matching this regex (repeatable). Wins over --include. Skipped URLs
        /// are recorded as excluded, not hidden. Overrides the config file.
        #[arg(long, value_name = "REGEX")]
        exclude: Vec<String>,
        /// Also export the results as CSV files into this directory.
        #[arg(long, value_name = "DIR")]
        csv: Option<PathBuf>,
        /// Show engine metrics and append them to the benchmark history.
        #[arg(long)]
        bench: bool,
        /// Benchmark history file. Providing it implies recording the run;
        /// with plain `--bench`, benchmarks/resultados.jsonl is used.
        #[arg(long, value_name = "FILE")]
        stats: Option<PathBuf>,
        /// Free-form note stored with the benchmark record.
        #[arg(long)]
        note: Option<String>,
    },
    /// Audit an exact list of URLs, one per line.
    ///
    /// This is the mode that makes comparisons with another tool fair: both receive the
    /// exact same set, so any difference comes from parsing and normalisation, not from
    /// where each one decided to start.
    List {
        /// Text file with one URL per line. Empty lines and lines starting with # are skipped.
        file: PathBuf,
        /// Output crawl file (SQLite). Defaults to crawl-<file>.sqlite; if it already
        /// exists, the previous crawl is kept next to it as <name>.prev.sqlite.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Concurrent requests per host (1..=20) [default: 5].
        #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..=20))]
        concurrency: Option<u8>,
        /// Also export the results as CSV files into this directory.
        #[arg(long, value_name = "DIR")]
        csv: Option<PathBuf>,
        /// Show engine metrics and append them to the benchmark history.
        #[arg(long)]
        bench: bool,
        /// Benchmark history file. Providing it implies recording the run;
        /// with plain `--bench`, benchmarks/resultados.jsonl is used.
        #[arg(long, value_name = "FILE")]
        stats: Option<PathBuf>,
        /// Free-form note stored with the benchmark record.
        #[arg(long)]
        note: Option<String>,
    },
    /// Resume an interrupted crawl from its file.
    ///
    /// Continues exactly where the crawl stopped: URLs already crawled are not fetched
    /// again, pending ones are, and the final pass runs at the end. The configuration is
    /// the one saved in the file by the original run — no crawl flags are accepted here,
    /// so resuming gives the same result as never having been interrupted. A finished
    /// crawl (status 'done') cannot be resumed: run the original crawl command again.
    ///
    /// One exception to that equivalence: `--ignore-robots` is not inherited. Permission to
    /// ignore robots.txt is granted by the person running the command, not by a file, so a
    /// resumed crawl always honours robots.txt. Re-run the original crawl if you need it.
    ///
    /// Likewise, HTTP Basic auth credentials are never stored in the crawl file: to resume
    /// a crawl of a protected staging, set CRAWLFORGE_AUTH=user:password again.
    Resume {
        /// The `.sqlite` file of an interrupted `crawl`, `audit` or `list` run.
        store: PathBuf,
        /// Also export the results as CSV files into this directory when it finishes.
        #[arg(long, value_name = "DIR")]
        csv: Option<PathBuf>,
    },
    /// Show the benchmark history and the comparison against Screaming Frog.
    ///
    /// Oculto del help general: es una herramienta de desarrollo del motor, no de auditoría.
    /// El histórico lo alimentan los rastreos ejecutados con `--bench`.
    #[command(hide = true)]
    Stats {
        /// Benchmark history file (JSONL, appended by crawls run with --bench).
        #[arg(default_value = DEFAULT_STATS_FILE)]
        file: PathBuf,
    },
    /// Summarise an existing crawl file.
    Report {
        /// A `.sqlite` file produced by `crawl`, `audit` or `list`.
        store: PathBuf,
        /// `terminal` to read it here, or `md`/`html` for a report you can paste in a ticket.
        #[arg(short, long, default_value = "terminal")]
        format: String,
        /// A rule ID, e.g. HTTP-404-INTERNAL: lists every affected URL, with no cut-off.
        /// The summary shows three examples per rule; this is where the rest are.
        #[arg(short, long)]
        rule: Option<String>,
        /// File to write the report to. Without it, the report is printed.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Report language. English is the original; `es` is available. Falls back to
        /// CRAWLFORGE_LANG, then to English.
        #[arg(long)]
        lang: Option<String>,
    },
    /// Show everything about one URL: who links to it, its status, extraction and findings.
    ///
    /// This is the "who links here?" panel: incoming links come first, deduplicated by
    /// linking page and with content links before template ones (nav, footer). It also
    /// shows outlinks with the status of each target, the redirect chain, the page's
    /// images, and — if the URL is an image — the pages that embed it.
    Inspect {
        /// A `.sqlite` file produced by `crawl`, `audit` or `list`.
        store: PathBuf,
        /// The URL to inspect. A path (`/blog/`) or a bare domain also work; with or
        /// without the trailing slash.
        url: String,
        /// How many rows to list per section (linking pages, link targets, images):
        /// a number, or `all` for the complete lists.
        #[arg(long, default_value = "20", value_parser = crawlforge_cli::inspect::parse_limit)]
        limit: crawlforge_cli::inspect::ListLimit,
        /// `terminal` to read it here, or `md` for a card you can paste in a ticket.
        #[arg(short, long, default_value = "terminal")]
        format: String,
        /// File to write the card to. Without it, the card is printed.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Card language. English is the original; `es` is available. Falls back to
        /// CRAWLFORGE_LANG, then to English.
        #[arg(long)]
        lang: Option<String>,
    },
    /// Export a crawl file to CSV or XLSX.
    Export {
        /// A `.sqlite` file produced by `crawl`, `audit` or `list`.
        store: PathBuf,
        /// Output directory for `csv`, or `.xlsx` file for `xlsx`.
        #[arg(short, long)]
        out: PathBuf,
        /// Output format: csv or xlsx.
        #[arg(short, long, default_value = "csv")]
        format: String,
    },
    /// Compare two crawls and report what changed.
    ///
    /// This is what turns a one-off audit into monitoring: Screaming Frog gives you a
    /// snapshot, this tells you whether the latest deploy made anything worse. With
    /// `--fail-on` it doubles as a CI gate.
    Diff {
        /// The earlier crawl (the reference).
        before: PathBuf,
        /// The later crawl (the one being judged).
        after: PathBuf,
        /// Save the diff as a file, to open it later.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Diff language. English is the original; `es` is available. Falls back to
        /// CRAWLFORGE_LANG, then to English.
        #[arg(long)]
        lang: Option<String>,
        /// Rule IDs or severities that fail the command when they show up as new findings.
        /// A severity means that one or worse: `--fail-on high` also fails on a `critical`.
        #[arg(long, value_delimiter = ',')]
        fail_on: Vec<String>,
    },
    /// List the audit rule catalog, or explain one rule.
    Rules {
        /// A rule ID from a report, e.g. CANON-CHAIN: shows that rule's full record.
        /// Without it, the whole catalog is listed.
        id: Option<String>,
        /// Language for rule names and descriptions. English is the original; `es` is
        /// available. Falls back to CRAWLFORGE_LANG, then to English.
        #[arg(long)]
        lang: Option<String>,
        /// Show a single category: meta, http, canonical, content, asset…
        #[arg(long)]
        category: Option<String>,
        /// Full description of every rule instead of a table.
        #[arg(long)]
        detail: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                // `error` y no `warn`: los avisos del motor —un sitemap que no responde, un
                // `Crawl-delay` recortado, una ruta que se sale del directorio— son útiles para
                // depurar y jerga para quien está auditando su web. Además son redundantes: lo
                // que importa de ellos acaba en el fichero de rastreo y lo reporta una regla.
                // Con `RUST_LOG=debug` se ven todos.
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error")),
        )
        .init();

    let command = Cli::parse().command;

    // El idioma se fija una sola vez y para todo el proceso: `--lang` del subcomando si lo hay,
    // si no `CRAWLFORGE_LANG`, si no inglés. A partir de aquí, cada módulo lo consulta con
    // `i18n::current_lang()` en vez de recibirlo por parámetro — el idioma de una CLI es
    // genuinamente global y pasarlo por seis firmas no lo hace más correcto.
    let lang_flag = match &command {
        Command::Report { lang, .. }
        | Command::Rules { lang, .. }
        | Command::Diff { lang, .. }
        | Command::Inspect { lang, .. } => lang.clone(),
        _ => None,
    };
    let lang = crawlforge_cli::i18n::resolve_lang(lang_flag.as_deref())?;
    crawlforge_cli::i18n::set_lang(lang);

    match command {
        Command::Crawl {
            url,
            out,
            config,
            concurrency,
            max_urls,
            max_depth,
            no_sitemaps,
            ignore_robots,
            include,
            exclude,
            csv,
            bench,
            stats,
            note,
        } => {
            let url = ensure_scheme(&url);
            // La credencial se extrae **antes** de que la URL llegue al trabajo o al nombre
            // del fichero: la URL limpia es la que viaja al entregable (§1.6); la credencial
            // va aparte, acotada al host de la semilla y sin serializar.
            let (url, url_auth) = split_url_credentials(&url);
            let store = out.unwrap_or_else(|| default_store_path(&url));
            let mut job = CrawlJob::http(&url);
            apply_config_file(&mut job, config.as_deref())?;
            apply_crawl_flags(
                &mut job,
                &CrawlFlags { concurrency, max_urls, max_depth, no_sitemaps, ignore_robots },
            );
            apply_pattern_flags(&mut job, include, exclude);
            apply_http_auth(&mut job, &url, url_auth)?;

            run_and_report(job, &store, RunOptions { csv, bench, stats, note, audit_base: None })
                .await
        }

        Command::Audit { dir, base, out, config, include, exclude, csv, bench, stats, note } => {
            anyhow::ensure!(dir.is_dir(), "{} is not a directory", dir.display());
            // En una auditoría de directorio no hay peticiones HTTP: una credencial en la
            // base no sirve para nada y sí acabaría en el entregable (§1.6). Se retira y se
            // dice; extraerla sin decirlo dejaría al usuario creyendo que hizo algo.
            let (base, base_auth) = split_url_credentials(&base);
            if base_auth.is_some() {
                eprintln!(
                    "{}",
                    msg::auth_base_ignored(crawlforge_cli::i18n::current_lang())
                );
            }
            let store = out.unwrap_or_else(|| default_store_path(&dir.display().to_string()));
            let mut job = CrawlJob::filesystem(&dir, &base);
            apply_config_file(&mut job, config.as_deref())?;
            apply_pattern_flags(&mut job, include, exclude);
            run_and_report(
                job,
                &store,
                RunOptions { csv, bench, stats, note, audit_base: Some(base) },
            )
            .await
        }

        Command::List { file, out, concurrency, csv, bench, stats, note } => {
            anyhow::ensure!(file.is_file(), "{} does not exist", file.display());
            let contents = std::fs::read_to_string(&file).context("the URL list could not be read")?;
            // También aquí: la lista entera se guarda en `crawl_meta.config_json`, así que
            // una línea con `usuario:contraseña@` acabaría en el fichero que se comparte
            // (§1.6). A diferencia de `crawl`, aquí la credencial extraída **no se usa**:
            // una lista puede mezclar hosts y elegir a cuál de ellos mandársela sería
            // adivinar. La vía para autenticar una lista es CRAWLFORGE_AUTH, acotada al
            // host de la primera URL, y el aviso lo dice.
            let mut had_credentials = false;
            let urls: Vec<String> = contents
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| {
                    let (clean, auth) = split_url_credentials(l);
                    had_credentials |= auth.is_some();
                    clean
                })
                .collect();
            anyhow::ensure!(!urls.is_empty(), "the list is empty");
            if had_credentials {
                eprintln!(
                    "{}",
                    msg::auth_list_ignored(crawlforge_cli::i18n::current_lang())
                );
            }

            let store = out.unwrap_or_else(|| default_store_path(&file.display().to_string()));
            let first_url = urls.first().cloned().unwrap_or_default();
            let mut job = CrawlJob::http(&first_url);
            job.project_name = file.display().to_string();
            job.mode = crawlforge_core::job::CrawlMode::List { urls };
            job.limits.concurrency_per_host = concurrency.unwrap_or(DEFAULT_CONCURRENCY);
            // En modo lista se audita lo que se pide y nada más: ni sitemaps ni seguir enlaces.
            job.discover_sitemaps = false;
            job.limits.follow_external = false;
            apply_http_auth(&mut job, &first_url, None)?;

            run_and_report(job, &store, RunOptions { csv, bench, stats, note, audit_base: None })
                .await
        }

        Command::Resume { store, csv } => run_resume(&store, csv).await,

        Command::Stats { file } => {
            stats::print_history(&stats::load(&file).context("the benchmark history could not be read")?);
            Ok(())
        }

        Command::Report { store, format, rule, out, .. } => {
            anyhow::ensure!(store.is_file(), "{} does not exist", store.display());
            // Antes de imprimir nada: si el fichero no es un rastreo, decirlo con palabras en vez
            // de dejar que salte el «no such table: urls» a mitad del encabezado.
            crawlforge_cli::store_check::ensure_crawl_store(&store)?;
            // Autocuración oportunista: si un cierre anterior no pudo salir de WAL porque otro
            // programa leía el fichero, el primer comando que vuelva a tocarlo reintegra los
            // `-wal`/`-shm` y lo deja portable. Si sigue ocupado —o el medio es de solo
            // lectura— no pasa nada: se lee igual, en silencio.
            let _ = crawlforge_core::store::try_make_portable(&store);
            if let Some(rule) = rule {
                // La lista completa es texto plano pensado para el terminal o para un pipe;
                // mezclarla con `--format md/html` sería otra pantalla, y no existe todavía.
                anyhow::ensure!(
                    format.trim().eq_ignore_ascii_case("terminal"),
                    "--rule lists plain text: drop --format, and redirect (`> urls.txt`) to save it"
                );
                return report::print_rule_urls(&store, &rule, lang)
                    .context("the rule listing could not be generated");
            }
            if format.trim().eq_ignore_ascii_case("terminal") {
                return report::print_summary(&store, lang).context("the crawl file could not be summarised");
            }
            let texto = audit_report::render(&store, &format, lang, out.as_deref())
                .context("the report could not be generated")?;
            match out {
                Some(path) => println!("Report written to {}", path.display()),
                None => print!("{texto}"),
            }
            Ok(())
        }

        Command::Inspect { store, url, limit, format, out, .. } => {
            anyhow::ensure!(store.is_file(), "{} does not exist", store.display());
            // La misma pareja que `report`: identificar el fichero antes de trabajar con él
            // (que el error hable de ficheros y comandos, no de tablas), y el reintento
            // oportunista de salir de WAL — ver el comentario de `report`.
            crawlforge_cli::store_check::ensure_crawl_store(&store)?;
            let _ = crawlforge_core::store::try_make_portable(&store);
            let card = crawlforge_cli::inspect::render_card(&store, &url, limit, &format, lang)
                .context("the URL card could not be generated")?;
            match out {
                Some(path) => {
                    std::fs::write(&path, &card)
                        .with_context(|| format!("could not write {}", path.display()))?;
                    println!("{}", msg::report_written(lang, path.display()));
                }
                None => print!("{card}"),
            }
            Ok(())
        }

        Command::Export { store, out, format } => {
            anyhow::ensure!(store.is_file(), "{} does not exist", store.display());
            // El mismo reintento oportunista que en `report`: ver el comentario de allí.
            let _ = crawlforge_core::store::try_make_portable(&store);
            match format.trim().to_ascii_lowercase().as_str() {
                "csv" => {
                    let written = export::to_csv(&store, &out).context("the CSV export failed")?;
                    println!("Exported {written} CSV files to {}", out.display());
                }
                "xlsx" => {
                    let hojas = xlsx::to_xlsx(&store, &out).context("the XLSX export failed")?;
                    println!("Exported {hojas} sheets to {}", out.display());
                }
                // Parquet está previsto y aún no se ha hecho. Decirlo así es
                // mejor que un «formato no reconocido» que hace pensar en una errata.
                "parquet" => bail!("Parquet export is not implemented yet"),
                otro => bail!("format not recognised: {otro}. Available: csv and xlsx"),
            }
            Ok(())
        }
        Command::Diff { before, after, out, fail_on, lang: _ } => {
            let outcome = diff::compare(&before, &after, out.as_deref(), &fail_on)
                .context("the crawls could not be compared")?;
            diff::print_report(&outcome);

            // Salida distinta de cero para que un pipeline se entere, pero sin el prefijo de
            // error de `anyhow`: que una regla vigilada aparezca no es un fallo del programa,
            // es su respuesta.
            if outcome.should_fail() {
                std::process::exit(1);
            }
            Ok(())
        }
        Command::Rules { id, category, detail, .. } => match id {
            // El bucle real es «el informe enseña un ID → quiero saber qué significa».
            // Con un ID delante, los filtros de catálogo no pintan nada: la ficha ya es una.
            Some(id) => rules::print_rule(lang, &id),
            None => rules::print_catalog(lang, category.as_deref(), detail),
        },
    }
}

/// La concurrencia cuando ni el flag ni el fichero de configuración dicen otra cosa.
/// Es el mismo valor que `CrawlLimits::default()`; existe para que el help pueda nombrarlo.
const DEFAULT_CONCURRENCY: u8 = 5;

/// Los flags de `crawl` que compiten con el fichero de configuración.
///
/// `Option`/`bool` distinguen «no se dijo» de «se dijo lo de siempre»: solo lo dicho
/// explícitamente pisa al fichero.
struct CrawlFlags {
    concurrency: Option<u8>,
    max_urls: Option<u64>,
    max_depth: Option<u32>,
    no_sitemaps: bool,
    ignore_robots: bool,
}

/// Carga `--config` sobre el trabajo, si se pidió. Los flags se aplican después y ganan.
fn apply_config_file(job: &mut CrawlJob, config: Option<&Path>) -> Result<()> {
    let Some(path) = config else { return Ok(()) };
    let config = load_job_config(path)?;
    config
        .apply_to(job)
        .with_context(|| format!("could not apply the config file {}", path.display()))?;
    Ok(())
}

/// Lee y valida un fichero YAML de configuración de rastreo.
fn load_job_config(path: &Path) -> Result<JobConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read the config file {}", path.display()))?;
    serde_norway::from_str(&text)
        .with_context(|| format!("could not parse the config file {}", path.display()))
}

/// Vuelca sobre el trabajo los flags explícitos de la línea de comandos.
///
/// Se llama **después** de [`apply_config_file`]: el orden es la precedencia.
fn apply_crawl_flags(job: &mut CrawlJob, flags: &CrawlFlags) {
    if let Some(c) = flags.concurrency {
        job.limits.concurrency_per_host = c;
    }
    if let Some(n) = flags.max_urls {
        job.limits.max_urls = Some(n);
    }
    if let Some(d) = flags.max_depth {
        job.limits.max_depth = Some(d);
    }
    if flags.no_sitemaps {
        job.discover_sitemaps = false;
    }
    if flags.ignore_robots {
        job.limits.ignore_robots = true;
    }
}

/// Vuelca los patrones de `--include`/`--exclude` sobre el trabajo.
///
/// Como con el resto de flags, la línea de comandos gana sobre el fichero: una lista no vacía
/// **sustituye** a la del YAML, no se mezcla con ella — mezclar haría imposible quitar desde la
/// terminal un patrón que el fichero trae de más.
fn apply_pattern_flags(job: &mut CrawlJob, include: Vec<String>, exclude: Vec<String>) {
    if !include.is_empty() {
        job.limits.include_patterns = include;
    }
    if !exclude.is_empty() {
        job.limits.exclude_patterns = exclude;
    }
}

/// Lo que acompaña a un rastreo además del trabajo en sí.
struct RunOptions {
    csv: Option<PathBuf>,
    /// Métricas de motor, comprobación del motor y registro en el histórico. Son herramientas de
    /// desarrollo del motor: sin este flag, la salida habla solo de la auditoría.
    bench: bool,
    /// Fichero del histórico. Darlo implica registrar, aunque no se pida `--bench`.
    stats: Option<PathBuf>,
    note: Option<String>,
    /// Solo en `audit`: la base declarada, para avisar si los canonicals la contradicen.
    audit_base: Option<String>,
}

async fn run_and_report(mut job: CrawlJob, store: &Path, opts: RunOptions) -> Result<()> {
    // El nivel manda sobre el trabajo, y se aplica **en el core**: aquí solo se lee de dónde
    // sale. Sin esto, `EntitlementSource` existía y no lo usaba nadie, así que el tope de 1.000
    // URLs del nivel gratuito no se aplicaba en el producto real.
    job.tier = DevSource::from_env().context("invalid CRAWLFORGE_TIER")?.tier();

    // Los patrones se validan **antes de tocar nada**: el motor los rechazaría igual, pero para
    // entonces esta función ya habría apartado el rastreo anterior como `.prev.sqlite` — una
    // errata en un regex no debe costar una rotación de ficheros que no produce nada a cambio.
    crawlforge_core::pattern::UrlFilter::from_limits(&job.limits)
        .context("invalid --include/--exclude pattern")?;

    let target = match &job.mode {
        CrawlMode::Http { seed } => seed.clone(),
        CrawlMode::Filesystem { root, .. } => root.display().to_string(),
        CrawlMode::List { urls } => format!("{} URLs", urls.len()),
    };
    let lang = crawlforge_cli::i18n::current_lang();
    println!("{}", msg::crawling(lang, &target));
    println!("File:     {}", store.display());

    // Re-rastrear no destruye el rastreo anterior: se aparta como `.prev.sqlite`, que es
    // justo el «antes» que `diff` necesita tras un despliegue. Ver [`rotate_previous_store`].
    // Y nunca se aparta un fichero **vivo**: ver [`rotate_unless_live`].
    let previous = rotate_unless_live(store)?;
    if let Some(prev) = &previous {
        println!("Previous: {} (kept, from the last run)", prev.display());
        // Si lo apartado era un rastreo interrumpido, quien quería continuarlo debe saber
        // que aún puede: reanudar y rotar son incompatibles a propósito —`resume` trabaja
        // sobre el fichero, nunca lo aparta— así que este comando es la puerta de vuelta.
        if unfinished_status(prev).is_some() {
            println!();
            println!("{}", msg::hint_previous_unfinished(lang));
            println!("    crawlforge resume {}", shorten(prev).display());
        }
    }
    println!();

    let filesystem_mode = matches!(job.mode, CrawlMode::Filesystem { .. });
    let mode = job.mode.as_str().to_string();
    let concurrency = job.limits.effective_concurrency();
    let max_urls = job.limits.max_urls;
    let max_depth = job.limits.max_depth;
    let respect_robots = !job.limits.ignore_robots;
    let sitemaps = job.discover_sitemaps;

    let (bar, callback) = spawn_progress_bar();

    let (cancel, engine_done) = spawn_cancel_on_ctrl_c();
    let resultado = engine::run_cancellable(job, store, Some(callback), Some(cancel)).await;
    engine_done.store(true, std::sync::atomic::Ordering::SeqCst);
    bar.finish_and_clear();
    let outcome = resultado.context("the crawl failed")?;

    // Si otro programa mantiene el fichero en WAL, hay que decirlo antes que nada: copiar el
    // `.sqlite` suelto perdería lo que quede en el `-wal`.
    if outcome.wal_kept {
        warn_wal_kept(&outcome.store_path);
    }

    // Un Ctrl+C ya no pierde el trabajo: el motor corta limpio, deja el fichero en `paused`
    // y aquí solo queda decir cómo continuarlo. Exit 130, el código convencional de SIGINT.
    if outcome.interrupted {
        print_interrupted(&outcome.store_path);
        std::process::exit(130);
    }

    // Un rastreo en el que ninguna URL respondió no es una auditoría, es un error: hay que
    // decirlo y salir con código distinto de cero, no imprimir un resumen de aspecto válido.
    if let Some(motivo) = report::empty_crawl_error(&outcome.store_path, &outcome.metrics)? {
        bail!(motivo);
    }

    if opts.bench {
        report::print_metrics(&outcome);
        report::print_gate(&outcome, filesystem_mode);
    } else {
        report::print_brief(&outcome);
    }
    report::print_summary(&outcome.store_path, crawlforge_cli::i18n::current_lang())?;

    // El aviso del `--base` va después del resumen: se refiere a lo que el resumen acaba
    // de enseñar (cero indexables, todo `canonicalised`).
    if let Some(base) = &opts.audit_base {
        if let Some(aviso) = report::check_base_mismatch(&outcome.store_path, base)? {
            println!();
            println!("{aviso}");
        }
    }

    if let Some(dir) = opts.csv {
        let written = export::to_csv(&outcome.store_path, &dir)?;
        println!("\nExported {written} CSV files to {}", dir.display());
    }

    // El histórico de benchmarks solo se toca si se pide: cada ejecución creaba
    // `benchmarks/resultados.jsonl` en el directorio del usuario sin preguntar.
    if opts.bench || opts.stats.is_some() {
        let path = opts.stats.unwrap_or_else(|| PathBuf::from(DEFAULT_STATS_FILE));
        let record = stats::collect(
            &outcome,
            stats::BenchConfig {
                target: &target,
                mode: &mode,
                concurrency,
                max_urls,
                max_depth,
                respect_robots,
                sitemaps,
                note: opts.note,
            },
        )?;
        stats::append(&record, &path)?;
        println!("\nBenchmark record appended to {}", path.display());
    }

    // Al final del todo: el resumen dice qué pasa, esto dice qué hacer a continuación.
    print_next_steps(&outcome.store_path, previous.as_deref());

    Ok(())
}

/// Reanuda un rastreo interrumpido a partir de su fichero.
///
/// # Por qué un subcomando y no un `--resume` de `crawl`
///
/// Todo lo que hace falta está dentro del fichero: la semilla, el modo y la configuración
/// entera (`crawl_meta.config_json`). Un `crawl --resume` obligaría a repetir la URL y abriría
/// la puerta a pasar flags nuevos, que aquí no caben: **la configuración que manda es la del
/// rastreo original**, porque la promesa de la función es que reanudar da el mismo resultado
/// que no haber parado, y un fichero rastreado a medias con una configuración y a medias con
/// otra no se lo cree nadie. Además `crawl` **rota** el fichero anterior a `.prev.sqlite`, y
/// reanudar y rotar son incompatibles: `resume` continúa el fichero en el sitio, sin rotarlo.
async fn run_resume(store: &Path, csv: Option<PathBuf>) -> Result<()> {
    anyhow::ensure!(store.is_file(), "{} does not exist", store.display());
    crawlforge_cli::store_check::ensure_crawl_store(store)?;
    // Un `status = 'running'` significa dos cosas opuestas: «se mató el proceso» y «hay otro
    // rastreo escribiendo ahora mismo». El cerrojo del fichero las distingue sin heurísticas:
    // si otro proceso lo tiene, se rechaza aquí con el mensaje del usuario; si está libre, el
    // proceso anterior murió y reanudar es correcto. La sonda se suelta enseguida: el motor
    // vuelve a tomar la exclusiva de verdad al arrancar, y no puede haber dos a la vez ni
    // dentro del mismo proceso.
    {
        let _probe = crawlforge_core::store::StoreLock::acquire(store).map_err(|e| match e {
            crawlforge_core::CoreError::StoreLocked { .. } => anyhow::anyhow!(msg::error_store_locked(
                crawlforge_cli::i18n::current_lang(),
                shorten(store).display()
            )),
            otro => anyhow::Error::from(otro),
        })?;
    }
    let info = resume_precheck(store)?;
    let lang = crawlforge_cli::i18n::current_lang();

    println!("{}", msg::resuming(lang, &info.base_url));
    println!("File:     {}", store.display());
    println!(
        "{}",
        msg::resume_counts(
            lang,
            i18n_count(lang, info.done),
            i18n_count(lang, info.pending)
        )
    );
    println!("{}", msg::resume_config_note(lang));
    if info.ignoraba_robots {
        eprintln!("{}", msg::resume_robots_not_inherited(lang));
    }
    println!();

    // La credencial de un staging protegido no está en el fichero —`config_json` la omite a
    // propósito— así que reanudarlo exige volver a darla, y la única vía sin flags de rastreo
    // es CRAWLFORGE_AUTH. El motor la acota al host de la semilla guardada.
    let auth = auth_from_env()?;
    if auth.is_some() {
        if let Some(host) =
            url::Url::parse(&info.base_url).ok().and_then(|u| u.host_str().map(String::from))
        {
            eprintln!("{}", msg::auth_env_note(lang, &host));
        }
    }

    let (bar, callback) = spawn_progress_bar();
    let (cancel, engine_done) = spawn_cancel_on_ctrl_c();
    let resultado = engine::resume_with_auth(store, Some(callback), Some(cancel), auth).await;
    engine_done.store(true, std::sync::atomic::Ordering::SeqCst);
    bar.finish_and_clear();
    let outcome = resultado.context("the crawl could not be resumed")?;

    if outcome.wal_kept {
        warn_wal_kept(&outcome.store_path);
    }

    // Una reanudación también se puede interrumpir, y volver a reanudarse después.
    if outcome.interrupted {
        print_interrupted(&outcome.store_path);
        std::process::exit(130);
    }

    // El error de «rastreo vacío» solo aplica si el fichero sigue sin una sola respuesta:
    // una reanudación con cero pendientes es legítima (solo faltaba la pasada final).
    if info.done == 0 {
        if let Some(motivo) = report::empty_crawl_error(&outcome.store_path, &outcome.metrics)? {
            bail!(motivo);
        }
    }

    report::print_brief_resumed(&outcome);
    report::print_summary(&outcome.store_path, lang)?;

    if let Some(dir) = csv {
        let written = export::to_csv(&outcome.store_path, &dir)?;
        println!("\nExported {written} CSV files to {}", dir.display());
    }

    print_next_steps(&outcome.store_path, None);
    Ok(())
}

/// Atajo local: recuento con los millares del idioma.
fn i18n_count(lang: crawlforge_rules::Lang, n: i64) -> String {
    crawlforge_cli::i18n::count(lang, n)
}

/// Lo que se sabe del rastreo interrumpido antes de reanudarlo.
struct ResumeInfo {
    /// El rastreo original llevaba `--ignore-robots`, permiso que al reanudar no se hereda.
    ignoraba_robots: bool,
    base_url: String,
    done: i64,
    pending: i64,
}

/// Valida que el fichero se puede reanudar y lee lo que la pantalla necesita.
///
/// El motor repite estas comprobaciones (`engine::resume` es quien manda); aquí se hacen
/// primero para que el error hable el idioma del usuario y diga qué hacer a continuación,
/// como exige la regla de `store_check.rs`.
fn resume_precheck(store: &Path) -> Result<ResumeInfo> {
    let lang = crawlforge_cli::i18n::current_lang();
    let conn = rusqlite::Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("could not open {}", store.display()))?;

    let version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
        .with_context(|| format!("could not read the schema version of {}", store.display()))?;
    // Un fichero **más nuevo** que este binario no se puede reanudar: no hay marcha atrás.
    // Uno más antiguo, sí, siempre que las migraciones que le faltan no cambien lo que el motor
    // escribe — `store::first_blocking_resume` es quien lo sabe, migración a migración.
    //
    // Hasta el 2026-08-02 esto era `version != SCHEMA_VERSION` a secas, y una migración que solo
    // crea un índice bastó para dejar irrecuperable un rastreo de dieciocho horas, con un error
    // que decía «vuelve a rastrearlo». `report`, `export` y `diff` ya migraban el fichero hacia
    // adelante sin protestar; era `resume` el que se salía del compromiso de que un rastreo
    // antiguo siga abriéndose.
    if version > crawlforge_core::SCHEMA_VERSION {
        bail!(msg::error_resume_schema(
            lang,
            store.display(),
            version,
            crawlforge_core::SCHEMA_VERSION
        ));
    }
    if let Some(bloqueante) = crawlforge_core::store::first_blocking_resume(version) {
        bail!(msg::error_resume_schema_blocking(lang, store.display(), version, bloqueante));
    }

    let (base_url, status, config_json): (String, String, String) = conn
        .query_row("SELECT base_url, status, config_json FROM crawl_meta LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .with_context(|| format!("could not read the crawl metadata of {}", store.display()))?;
    // `base_url` se imprime tal cual en la pantalla de reanudación, y viene de un fichero que
    // pudo fabricar cualquiera: sin filtro, un `.sqlite` manipulado inyecta secuencias de
    // escape en el terminal (revisión 2026-08-01 §1.7d).
    let base_url = audit_report::strip_control_chars(&base_url);

    match status.as_str() {
        "running" | "paused" => {}
        "done" => bail!(msg::error_resume_done(lang, store.display())),
        otro => bail!(msg::error_resume_status(lang, store.display(), otro)),
    }

    // El motor descarta el `ignore_robots` guardado: un permiso que vive dentro de un fichero
    // no es un permiso que nadie haya concedido hoy (revisión 2026-08-01 §1.4). Pero eso cambia
    // el comportamiento respecto al rastreo original, y `resume` promete continuar «como si
    // nunca se hubiera interrumpido». Si el cambio no se dice, la promesa es falsa en silencio.
    let Ok(guardado) = serde_json::from_str::<crawlforge_core::job::CrawlJob>(&config_json) else {
        bail!(msg::error_resume_config(lang, store.display()));
    };
    let ignoraba_robots = guardado.limits.ignore_robots;

    let (done, pending): (i64, i64) = conn.query_row(
        "SELECT COUNT(*) FILTER (WHERE crawl_state = 'done'),
                COUNT(*) FILTER (WHERE crawl_state = 'pending')
         FROM urls",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    Ok(ResumeInfo { base_url, done, pending, ignoraba_robots })
}

/// El estado de un fichero de rastreo, si es un rastreo sin terminar. `None` en cualquier otro
/// caso —terminado, ilegible, de otro programa—: esto alimenta una pista, no una decisión.
fn unfinished_status(path: &Path) -> Option<String> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .ok()?;
    let status: String =
        conn.query_row("SELECT status FROM crawl_meta LIMIT 1", [], |r| r.get(0)).ok()?;
    matches!(status.as_str(), "running" | "paused").then_some(status)
}

/// La barra de progreso y el callback que la alimenta.
///
/// La barra vive en la CLI y el motor solo emite números por callback: el core no conoce a
/// `indicatif` ni a ningún terminal. Se pinta en stderr —stdout queda limpio para un pipe— e
/// `indicatif` la oculta sola cuando stderr no es un terminal.
fn spawn_progress_bar() -> (indicatif::ProgressBar, engine::ProgressCallback) {
    let bar = indicatif::ProgressBar::new_spinner();
    bar.set_style(
        indicatif::ProgressStyle::with_template("{spinner} {msg}")
            .unwrap_or_else(|_| indicatif::ProgressStyle::default_spinner()),
    );
    bar.enable_steady_tick(std::time::Duration::from_millis(120));
    let callback: engine::ProgressCallback = {
        let bar = bar.clone();
        Arc::new(move |p: &CrawlProgress| bar.set_message(progress_message(p)))
    };
    (bar, callback)
}

/// Convierte el primer Ctrl+C en una cancelación limpia del motor.
///
/// El motor corta el bucle —o la pasada final, entre dos reglas—, vacía el hilo escritor y
/// deja el fichero en `paused`: nada de lo rastreado se pierde y `crawlforge resume` continúa
/// desde ahí. El segundo Ctrl+C sale al instante, por si el cierre tarda más de lo que el
/// usuario está dispuesto a esperar.
///
/// Devuelve además la marca de «el motor ya terminó», que quien llama debe encender en cuanto
/// el motor devuelva su resultado. A partir de ahí un Ctrl+C ya no tiene nada que cancelar
/// —el fichero está completo y cerrado— y lo que corre son los exports: instalado el handler,
/// el SIGINT por defecto ya no existe, así que sin esta rama el primer Ctrl+C durante un CSV
/// largo no hacía absolutamente nada (revisión §3.6).
fn spawn_cancel_on_ctrl_c(
) -> (crawlforge_core::engine::CancelSignal, Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let engine_done = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&engine_done);
    let (tx, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        let lang = crawlforge_cli::i18n::current_lang();
        if done.load(Ordering::SeqCst) {
            // El motor ya terminó: no hay rastreo que salvar y sí, quizá, un export a medio
            // escribir. Salir ya es lo que el usuario está pidiendo.
            eprintln!();
            eprintln!("{}", msg::interrupt_after_done(lang));
            std::process::exit(130);
        }
        eprintln!();
        eprintln!("{}", msg::interrupt_flushing(lang));
        let _ = tx.send(true);
        if tokio::signal::ctrl_c().await.is_ok() {
            std::process::exit(130);
        }
    });
    (rx, engine_done)
}

/// El aviso de que el fichero se quedó en modo WAL: sus laterales forman parte del rastreo.
///
/// Va por stderr, como los demás avisos: el dato de auditoría sigue limpio en stdout.
fn warn_wal_kept(store: &Path) {
    let lang = crawlforge_cli::i18n::current_lang();
    eprintln!();
    eprintln!("{}", msg::warn_wal_kept(lang, shorten(store).display()));
}

/// Qué decir cuando un rastreo se interrumpe: dónde quedó y el comando que lo continúa.
fn print_interrupted(store: &Path) {
    let lang = crawlforge_cli::i18n::current_lang();
    let corto = shorten(store);
    println!();
    println!("{}", msg::interrupted_saved(lang, corto.display()));
    println!("    crawlforge resume {}", corto.display());
}

/// La ruta tal como el usuario la escribiría: relativa al directorio actual si cuelga de él.
///
/// Estos comandos son para copiar y pegar, y una ruta absoluta de 120 caracteres los vuelve
/// ilegibles justo cuando su función es que se lean de un vistazo.
fn shorten(path: &Path) -> std::path::PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| path.to_path_buf())
}

/// Los comandos que tienen sentido justo después de un rastreo, con el fichero ya puesto.
///
/// El resumen de terminal enseña recuentos por regla y nada más; la explicación de cada regla
/// y las URLs afectadas están en `report`, y el libro completo en `export`. Quien viene de una
/// interfaz gráfica no tiene por qué saber que esos comandos existen: cada pantalla debe
/// decir cuál es el paso siguiente, listo para copiar y pegar.
fn print_next_steps(store: &Path, previous: Option<&Path>) {
    let store = shorten(store);
    let s = store.display();
    println!();
    let lang = crawlforge_cli::i18n::current_lang();
    println!("{}", crawlforge_cli::i18n::section(&msg::next_steps_title(lang)));
    println!("  {}", msg::next_explain(lang));
    println!("    crawlforge report {s} --format md --out report.md");
    println!("  {}", msg::next_spreadsheet(lang));
    println!("    crawlforge export {s} --format xlsx --out {}", xlsx_name(&s.to_string()));
    match previous {
        Some(prev) => {
            println!("  {}", msg::next_compare_previous(lang));
            println!("    crawlforge diff {} {s}", shorten(prev).display());
        }
        None => {
            println!("  {}", msg::next_compare_later(lang));
            println!("    crawlforge diff {} {s}", backup_store_path(&store).display());
        }
    }
    println!("  {}", msg::next_reread(lang));
    println!("    crawlforge report {s}");
}

/// El nombre de XLSX que acompaña a un fichero de rastreo: `crawl-x.sqlite` → `crawl-x.xlsx`.
fn xlsx_name(store: &str) -> String {
    match store.strip_suffix(".sqlite") {
        Some(stem) => format!("{stem}.xlsx"),
        None => format!("{store}.xlsx"),
    }
}

/// La línea de progreso que se enseña durante el rastreo.
///
/// La fase importa tanto como los números: el descubrimiento de sitemaps puede tardar varios
/// segundos sin rastrear nada, y sin nombrarlo parece un cuelgue.
fn progress_message(p: &CrawlProgress) -> String {
    match p.phase {
        CrawlPhase::Sitemaps => "discovering sitemaps…".to_string(),
        // La pasada final puede durar más que el rastreo entero, así que decir solo «final
        // pass…» durante horas es lo mismo que no decir nada. Con el paso y la cuenta, quien
        // mira sabe que avanza y **cuál** es la regla que está tardando, que es justo el dato
        // que hizo falta para encontrar el índice que faltaba (revisión del 2026-08-02).
        CrawlPhase::Finalize => match &p.step {
            Some(s) if s.total > 0 => {
                format!("final pass · rule {}/{} · {}", s.index, s.total, s.name)
            }
            Some(s) => format!("final pass · {}…", s.name.replace('_', " ")),
            None => "final pass: incoming links and site-wide rules…".to_string(),
        },
        CrawlPhase::Crawl => {
            let secs = p.elapsed.as_secs_f64();
            let rate = if secs > 0.0 { p.urls_fetched as f64 / secs } else { 0.0 };
            let mut msg = format!(
                "{} crawled · {} queued · {} URL/s · {} findings",
                thousands(p.urls_fetched),
                thousands(p.urls_queued),
                format_rate(rate),
                thousands(p.issues_found),
            );
            if p.urls_errored > 0 {
                msg.push_str(&format!(" · {} failed", thousands(p.urls_errored)));
            }
            msg
        }
    }
}

/// Separador de millares con coma, como corresponde al inglés de la salida: `3,816`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// El ritmo con un decimal cuando es lento —a 2 URL/s el decimal es información— y redondeado
/// cuando es rápido, que es cuando el decimal es ruido.
fn format_rate(rate: f64) -> String {
    if rate < 10.0 {
        format!("{rate:.1}")
    } else {
        thousands(rate.round() as u64)
    }
}

/// Añade `https://` cuando el objetivo viene sin esquema.
///
/// Un SEO teclea el dominio pelado cien veces al día y Screaming Frog lo acepta; responder
/// «invalid URL: relative URL without a base» a `crawlforge crawl example.com` era hacerle
/// pagar nuestra implementación (revisión 2026-08-01 §5.3). Solo se completa lo que no declara
/// esquema alguno: un `ftp://x` debe fallar con su propio error, no convertirse en silencio en
/// otra petición. Se asume `https` y no `http` porque es lo que sirve cualquier sitio auditable
/// hoy; si el sitio solo responde por HTTP, escribir `http://` sigue funcionando.
fn ensure_scheme(url: &str) -> String {
    let trimmed = url.trim();
    // «Tiene esquema» exige la forma `esquema://`: en `localhost:8080` el crate `url` tomaría
    // `localhost` como esquema y el rastreo fallaría igual de crípticamente.
    let has_scheme = trimmed.split_once("://").is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    if has_scheme {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// La variable de entorno con la credencial de autenticación básica, en la forma
/// `usuario:contraseña`.
///
/// # Por qué una variable de entorno y no un flag `--auth`
///
/// Un `--auth usuario:contraseña` deja el secreto en el historial del shell y en la lista de
/// procesos (`ps`) mientras dura el rastreo, que puede ser largo. La variable de entorno no
/// aparece en `ps`, se exporta una vez por sesión o llega como secreto de CI, y es la vía de
/// siempre para credenciales en herramientas de terminal (`HTTP_PROXY`, `AWS_SECRET_ACCESS_KEY`,
/// `GH_TOKEN`…). Tampoco va en el YAML de `--config`: ese fichero describe el sitio, se versiona
/// y se comparte, y una contraseña en claro dentro de él sería la fuga de §1.6 con otro nombre.
const AUTH_ENV_VAR: &str = "CRAWLFORGE_AUTH";

/// Separa el `usuario:contraseña@` de una URL escrita por el usuario.
///
/// Revisión 2026-08-01 §1.6: la URL de entrada acaba en `crawl_meta.base_url`, en
/// `config_json` y en el nombre por defecto del fichero, tres sitios que viajan en el
/// entregable. Por eso la credencial se **extrae** —no solo se retira— antes de construir el
/// trabajo: la URL limpia sigue su camino hacia `normalize` y el fichero, y la credencial
/// viaja aparte por `CrawlLimits::http_basic_auth`, que no se serializa. Así el atajo de
/// siempre (`crawl https://user:pass@pre.cliente.es/`) autentica el rastreo sin reabrir la
/// fuga. El *userinfo* llega percent-encodificado (`p%40ss` por `p@ss`): se decodifica aquí,
/// porque el servidor espera la contraseña real, no su forma de URL.
fn split_url_credentials(url: &str) -> (String, Option<crawlforge_core::job::HttpBasicAuth>) {
    let Ok(mut parsed) = url::Url::parse(url) else {
        // Lo que no parsea no lleva una credencial extraíble; el motor dará su propio error.
        return (url.to_string(), None);
    };
    if parsed.username().is_empty() && parsed.password().is_none() {
        return (url.to_string(), None);
    }
    let auth = crawlforge_core::job::HttpBasicAuth::new(
        percent_decode(parsed.username()),
        percent_decode(parsed.password().unwrap_or("")),
    );
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    (parsed.to_string(), Some(auth))
}

/// Decodifica el percent-encoding de un componente de URL. `%3A` → `:`, `%40` → `@`.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Interpreta el `usuario:contraseña` de [`AUTH_ENV_VAR`].
///
/// Un valor sin `:` es un error, no un silencio: rastrear sin autenticar creyendo que se
/// autenticaba acabaría en un informe de 401 que no describe el sitio.
fn parse_auth_spec(spec: &str) -> Result<crawlforge_core::job::HttpBasicAuth> {
    let Some((user, pass)) = spec.split_once(':') else {
        bail!("{AUTH_ENV_VAR} must have the form user:password");
    };
    Ok(crawlforge_core::job::HttpBasicAuth::new(user, pass))
}

/// La credencial de [`AUTH_ENV_VAR`], si está definida. Vacía cuenta como no definida.
fn auth_from_env() -> Result<Option<crawlforge_core::job::HttpBasicAuth>> {
    match std::env::var(AUTH_ENV_VAR) {
        Ok(spec) if spec.is_empty() => Ok(None),
        Ok(spec) => parse_auth_spec(&spec).map(Some),
        Err(_) => Ok(None),
    }
}

/// Aplica al trabajo la credencial que corresponda: la de la URL si la traía, si no la de
/// [`AUTH_ENV_VAR`]. La URL gana porque es lo dicho explícitamente en este comando.
///
/// El aviso por stderr no es cortesía: nombra el host al que —y solo al que— se enviará la
/// credencial, que es el contrato de seguridad completo en una línea, y confirma que no se
/// guarda en el fichero. Sin repetir el secreto, como corresponde a un aviso sobre secretos.
fn apply_http_auth(
    job: &mut CrawlJob,
    clean_url: &str,
    from_url: Option<crawlforge_core::job::HttpBasicAuth>,
) -> Result<()> {
    let Some((auth, origin)) = choose_auth(from_url, auth_from_env()?) else {
        return Ok(());
    };
    // Sin host no hay a qué acotar la credencial, y sin acotar no se manda: el motor dará su
    // propio error sobre la URL.
    if let Some(host) = url::Url::parse(clean_url).ok().and_then(|u| u.host_str().map(String::from))
    {
        let lang = crawlforge_cli::i18n::current_lang();
        eprintln!(
            "{}",
            match origin {
                AuthOrigin::Url => msg::auth_url_note(lang, &host),
                AuthOrigin::Env => msg::auth_env_note(lang, &host),
            }
        );
        job.limits.http_basic_auth = Some(auth);
    }
    Ok(())
}

/// De dónde salió la credencial elegida; decide qué aviso se imprime.
#[derive(Debug, PartialEq, Eq)]
enum AuthOrigin {
    Url,
    Env,
}

/// La precedencia entre las dos vías: **la URL gana**, porque es lo dicho explícitamente en
/// este comando, y la variable de entorno puede llevar exportada toda una sesión.
fn choose_auth(
    from_url: Option<crawlforge_core::job::HttpBasicAuth>,
    from_env: Option<crawlforge_core::job::HttpBasicAuth>,
) -> Option<(crawlforge_core::job::HttpBasicAuth, AuthOrigin)> {
    match (from_url, from_env) {
        (Some(auth), _) => Some((auth, AuthOrigin::Url)),
        (None, Some(auth)) => Some((auth, AuthOrigin::Env)),
        (None, None) => None,
    }
}

/// Nombre de fichero por defecto: legible y ordenable.
fn default_store_path(target: &str) -> PathBuf {
    // Defensa propia además de la del llamador: la contraseña de `https://user:pass@host/`
    // no puede acabar en el nombre del fichero, que se ve en `ls`, en la pestaña de Excel y
    // en cada comando de los «next steps» aunque el contenido ya esté limpio (§1.6). El
    // userinfo solo puede vivir antes de la primera `/`: un `@` en la ruta (`/@usuario`) no
    // es una credencial y se conserva.
    let rest = target.trim_start_matches("https://").trim_start_matches("http://");
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (rest, None),
    };
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let target = match path {
        Some(p) => format!("{authority}/{p}"),
        None => authority.to_string(),
    };

    let slug: String = target
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    PathBuf::from(format!("crawl-{}.sqlite", if slug.is_empty() { "sitio" } else { &slug }))
}

/// Dónde se aparta el rastreo anterior: `crawl-x.sqlite` → `crawl-x.prev.sqlite`.
fn backup_store_path(store: &Path) -> PathBuf {
    match (
        store.file_stem().and_then(|s| s.to_str()),
        store.extension().and_then(|e| e.to_str()),
    ) {
        (Some(stem), Some(ext)) => store.with_file_name(format!("{stem}.prev.{ext}")),
        _ => {
            let mut name = store.as_os_str().to_owned();
            name.push(".prev");
            PathBuf::from(name)
        }
    }
}

/// Sufijos con los que SQLite deja ficheros laterales junto a la base.
const SQLITE_SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Como [`rotate_previous_store`], pero **jamás aparta un fichero que otro rastreo está
/// escribiendo ahora mismo**.
///
/// Es la mitad CLI del §3.3: con el nombre por defecto determinista y 100 blogs en cron, dos
/// `crawl` del mismo sitio coinciden tarde o temprano, y el segundo rotaba el fichero *vivo*
/// del primero a `.prev.sqlite` — el primero seguía escribiendo por descriptor y su pasada
/// final reabría por ruta, dentro del fichero del segundo. La sonda toma el mismo cerrojo que
/// tomará el motor y lo suelta al salir de aquí: si está tomado, hay otro escritor y este
/// comando termina sin tocar nada.
fn rotate_unless_live(store: &Path) -> Result<Option<PathBuf>> {
    let _guard = crawlforge_core::store::StoreLock::acquire(store).map_err(|e| match e {
        crawlforge_core::CoreError::StoreLocked { .. } => anyhow::anyhow!(
            msg::error_store_locked(crawlforge_cli::i18n::current_lang(), shorten(store).display())
        ),
        otro => anyhow::Error::from(otro),
    })?;
    rotate_previous_store(store)
}

/// Aparta el fichero de rastreo existente como `.prev.sqlite` en vez de borrarlo.
///
/// El flujo estrella del producto es `crawl` → despliegue → `crawl` → `diff`, y el nombre por
/// defecto es determinista: borrar sin más destruía el «antes» justo cuando más valía. Apartar
/// en vez de preguntar mantiene cómodo el caso normal —re-rastrear lo mismo para ver cómo va
/// no pide ningún flag— y garantiza que siempre hay exactamente un rastreo anterior con el que
/// comparar. Se conserva **una** copia: la tercera ejecución pisa el `.prev` de la primera.
///
/// Devuelve la ruta del `.prev` si había un rastreo que apartar.
fn rotate_previous_store(store: &Path) -> Result<Option<PathBuf>> {
    if !store.exists() {
        return Ok(None);
    }
    let prev = backup_store_path(store);

    // Primero se limpia el destino, laterales incluidos: un `-wal` huérfano de otra base
    // junto a un fichero recién renombrado es corrupción servida.
    for suffix in std::iter::once("").chain(SQLITE_SIDECARS) {
        let stale = sidecar(&prev, suffix);
        if stale.exists() {
            std::fs::remove_file(&stale)
                .with_context(|| format!("could not remove the old backup {}", stale.display()))?;
        }
    }

    std::fs::rename(store, &prev).with_context(|| {
        format!("could not move the previous crawl {} to {}", store.display(), prev.display())
    })?;
    // Los laterales viajan con su base: un `.prev` sin su `-wal` perdería las últimas
    // transacciones si el rastreo anterior no llegó a cerrarse limpio.
    for suffix in SQLITE_SIDECARS {
        let src = sidecar(store, suffix);
        if src.exists() {
            std::fs::rename(&src, sidecar(&prev, suffix))
                .with_context(|| format!("could not move the sidecar {}", src.display()))?;
        }
    }
    Ok(Some(prev))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nombre_por_defecto_sale_del_objetivo() {
        assert_eq!(
            default_store_path("https://ejemplo.es/blog"),
            PathBuf::from("crawl-ejemplo-es-blog.sqlite")
        );
    }

    #[test]
    fn un_objetivo_sin_caracteres_utiles_no_deja_el_nombre_vacio() {
        assert_eq!(default_store_path("https://"), PathBuf::from("crawl-sitio.sqlite"));
    }

    #[test]
    fn el_nombre_del_fichero_no_contiene_la_contrasena() {
        // Revisión 2026-08-01 §1.6: `crawl https://staging:S3cret@pre.cliente.es/` generaba
        // `crawl-staging-S3cret-pre-cliente-es.sqlite` — la contraseña en el `ls`, en la
        // pestaña de Excel y en cada comando copiable de los «next steps».
        assert_eq!(
            default_store_path("https://staging:S3cret@pre.cliente.es/"),
            PathBuf::from("crawl-pre-cliente-es.sqlite")
        );
        // Un `@` en la ruta no es una credencial: los perfiles `/@usuario` se conservan.
        assert_eq!(
            default_store_path("https://ejemplo.es/@davidcf"),
            PathBuf::from("crawl-ejemplo-es--davidcf.sqlite")
        );
    }

    #[test]
    fn las_credenciales_de_la_semilla_se_extraen_y_la_url_queda_limpia() {
        // La semilla acaba en `crawl_meta.base_url`, en `config_json` y en `project_name`:
        // limpiarla aquí es lo que evita que la contraseña llegue al entregable (§1.6). Y a
        // la vez la credencial se conserva aparte: es lo que autentica el rastreo de un
        // staging protegido, la función que la limpieza había roto.
        let (url, auth) = split_url_credentials("https://staging:S3cret@pre.cliente.es/");
        assert_eq!(url, "https://pre.cliente.es/");
        let auth = auth.expect("la credencial se extrae, no se tira");
        assert_eq!(auth.username, "staging");
        assert_eq!(auth.password, "S3cret");

        // Solo usuario: contraseña vacía, que Basic Auth admite.
        let (url, auth) = split_url_credentials("https://usuario@ejemplo.es/a");
        assert_eq!(url, "https://ejemplo.es/a");
        assert_eq!(auth.expect("hay credencial").password, "");

        // El userinfo viaja percent-encodificado; el servidor espera los caracteres reales.
        let (_, auth) = split_url_credentials("https://user:p%40ss%3Aw@ejemplo.es/");
        assert_eq!(auth.expect("hay credencial").password, "p@ss:w");

        // Sin credenciales, la URL vuelve intacta, sin renormalizar de más.
        assert_eq!(
            split_url_credentials("https://ejemplo.es/a"),
            ("https://ejemplo.es/a".to_string(), None)
        );
        // Lo que no parsea no se toca: el motor dará su propio error, con la entrada original.
        assert_eq!(split_url_credentials("no es una url"), ("no es una url".to_string(), None));
    }

    #[test]
    fn la_credencial_de_la_url_gana_a_la_de_la_variable_de_entorno() {
        use crawlforge_core::job::HttpBasicAuth;
        let de_url = HttpBasicAuth::new("de-url", "a");
        let de_env = HttpBasicAuth::new("de-env", "b");

        let (auth, origen) =
            choose_auth(Some(de_url.clone()), Some(de_env.clone())).expect("hay credencial");
        assert_eq!(auth, de_url, "lo dicho en el comando gana a lo exportado en la sesión");
        assert_eq!(origen, AuthOrigin::Url);

        let (auth, origen) = choose_auth(None, Some(de_env.clone())).expect("hay credencial");
        assert_eq!((auth, origen), (de_env, AuthOrigin::Env));

        assert!(choose_auth(None, None).is_none());
    }

    #[test]
    fn la_variable_de_entorno_de_credenciales_exige_su_forma() {
        // `user:pass` vale; la primera `:` separa, el resto es contraseña (que puede llevar
        // sus propios `:`).
        let auth = parse_auth_spec("staging:S3cret").expect("forma válida");
        assert_eq!((auth.username.as_str(), auth.password.as_str()), ("staging", "S3cret"));
        let auth = parse_auth_spec("user:con:dos:puntos").expect("forma válida");
        assert_eq!(auth.password, "con:dos:puntos");

        // Sin `:` es un error que nombra la variable: rastrear sin autenticar creyendo que
        // se autenticaba daría un informe de 401 que no describe el sitio.
        let err = parse_auth_spec("sin-separador").expect_err("falta el separador");
        assert!(err.to_string().contains("CRAWLFORGE_AUTH"), "{err}");
    }

    #[test]
    fn un_dominio_pelado_recibe_https_y_un_esquema_explicito_se_respeta() {
        // Revisión 2026-08-01 §5.3: un SEO teclea el dominio pelado cien veces al día y
        // Screaming Frog lo acepta; responder «relative URL without a base» es hacerle pagar
        // nuestra implementación.
        assert_eq!(ensure_scheme("example.com"), "https://example.com");
        assert_eq!(ensure_scheme("ejemplo.es/blog"), "https://ejemplo.es/blog");
        // `localhost:8080` parsearía con esquema `localhost`: también se completa.
        assert_eq!(ensure_scheme("localhost:8080"), "https://localhost:8080");
        // Un esquema explícito no se pisa: ni el correcto ni el equivocado, que debe fallar
        // con su propio error en vez de convertirse en otra petición distinta.
        assert_eq!(ensure_scheme("http://ejemplo.es"), "http://ejemplo.es");
        assert_eq!(ensure_scheme("https://ejemplo.es"), "https://ejemplo.es");
        assert_eq!(ensure_scheme("ftp://ejemplo.es"), "ftp://ejemplo.es");
    }

    #[test]
    fn rules_acepta_un_id_posicional() {
        // Revisión 2026-08-01 §5.5: `crawlforge rules CANON-CHAIN` respondía
        // `error: unexpected argument` y obligaba a buscar entre 58 fichas con `--detail`.
        let cli = Cli::try_parse_from(["crawlforge", "rules", "CANON-CHAIN"])
            .expect("el ID posicional debe aceptarse");
        let Command::Rules { id, .. } = cli.command else {
            panic!("el subcomando es rules");
        };
        assert_eq!(id.as_deref(), Some("CANON-CHAIN"));

        // Y sin ID, el catálogo de siempre.
        let cli = Cli::try_parse_from(["crawlforge", "rules"]).expect("sin ID también vale");
        let Command::Rules { id, .. } = cli.command else {
            panic!("el subcomando es rules");
        };
        assert!(id.is_none());
    }

    /// Todo subcomando cuya salida está traducida tiene que aceptar `--lang`.
    ///
    /// `diff` no lo aceptaba: sus 58 mensajes ya estaban localizados por dentro y solo faltaba
    /// exponer el flag, así que `crawlforge diff a b --lang es` moría con «unexpected argument»
    /// mientras `report` y `rules` sí lo admitían. Lo encontró el usuario en su primer día de
    /// pruebas reales, siguiendo un ejemplo del manual que yo había escrito mal.
    ///
    /// Este test se recorre la lista a propósito en vez de comprobar solo `diff`: la próxima vez
    /// que se traduzca un subcomando, aquí se ve si se olvidó el flag.
    #[test]
    fn los_subcomandos_traducidos_aceptan_el_flag_de_idioma() {
        for args in [
            vec!["crawlforge", "report", "c.sqlite", "--lang", "es"],
            vec!["crawlforge", "rules", "--lang", "es"],
            vec!["crawlforge", "diff", "a.sqlite", "b.sqlite", "--lang", "es"],
            vec!["crawlforge", "inspect", "c.sqlite", "https://e.es/", "--lang", "es"],
        ] {
            let comando = args[1];
            assert!(
                Cli::try_parse_from(&args).is_ok(),
                "`{comando}` traduce su salida y debe aceptar --lang"
            );
        }
    }

    #[test]
    fn inspect_toma_fichero_y_url_y_valida_su_limite() {
        // El caso normal: fichero y URL, sin más ceremonia.
        let cli = Cli::try_parse_from(["crawlforge", "inspect", "c.sqlite", "https://e.es/blog/"])
            .expect("inspect con fichero y URL debe aceptarse");
        let Command::Inspect { store, url, limit, format, .. } = cli.command else {
            panic!("el subcomando es inspect");
        };
        assert_eq!(store, PathBuf::from("c.sqlite"));
        assert_eq!(url, "https://e.es/blog/");
        assert_eq!(limit, crawlforge_cli::inspect::ListLimit::N(20), "el corte por defecto");
        assert_eq!(format, "terminal");

        // `--limit` acepta un número o `all`, y rechaza lo demás con el contrato.
        for (valor, esperado) in [
            ("5", crawlforge_cli::inspect::ListLimit::N(5)),
            ("all", crawlforge_cli::inspect::ListLimit::All),
        ] {
            let cli = Cli::try_parse_from([
                "crawlforge", "inspect", "c.sqlite", "https://e.es/", "--limit", valor,
            ])
            .expect("el límite válido debe aceptarse");
            let Command::Inspect { limit, .. } = cli.command else {
                panic!("el subcomando es inspect");
            };
            assert_eq!(limit, esperado);
        }
        let r = Cli::try_parse_from([
            "crawlforge", "inspect", "c.sqlite", "https://e.es/", "--limit", "0",
        ]);
        let Err(err) = r else { panic!("0 filas no es inspeccionar nada") };
        assert!(err.to_string().contains("all"), "el error dice el contrato: {err}");
    }

    #[test]
    fn la_cli_acepta_los_comandos_previstos() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    /// Construye un fichero de rastreo interrumpido con la configuración que se le pase.
    fn store_pausado(nombre: &str, ignore_robots: bool) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("crawlforge-cli-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear temporal");
        let path = dir.join("crawl.sqlite");

        let conn = crawlforge_core::store::open_writer(&path).expect("crear el rastreo");
        let mut job = crawlforge_core::job::CrawlJob::http("https://ejemplo.es/");
        job.limits.ignore_robots = ignore_robots;
        let config = serde_json::to_string(&job).expect("serializar la configuración");
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime)
             VALUES ('x','p','P','https://ejemplo.es/','http',datetime('now'),'paused',?1,
                     '0','0','free')",
            rusqlite::params![config],
        )
        .expect("insertar crawl_meta");
        drop(conn);
        path
    }

    #[test]
    fn un_rastreo_de_esquema_anterior_se_puede_reanudar() {
        // El caso que lo destapó: 18 horas de rastreo, esquema v5, y un binario v6 cuya única
        // migración nueva crea un índice. `resume_precheck` exigía versión exacta y respondía
        // «vuelve a lanzar el rastreo».
        let path = store_pausado("esquema-anterior", false);
        {
            let conn = rusqlite::Connection::open(&path).expect("abrir para envejecer");
            conn.execute("DELETE FROM schema_version WHERE version = ?1",
                         [crawlforge_core::SCHEMA_VERSION])
                .expect("quitar la última migración de la marca");
        }

        let info = resume_precheck(&path);
        assert!(
            info.is_ok(),
            "un rastreo de esquema anterior con migraciones seguras tiene que poder reanudarse: {:?}",
            info.err()
        );
    }

    #[test]
    fn un_rastreo_de_esquema_mas_nuevo_no_se_reanuda() {
        // La otra mitad: hacia atrás no hay migración posible, y reanudar con un binario viejo
        // escribiría filas que el esquema del fichero ya no admite.
        let path = store_pausado("esquema-futuro", false);
        {
            let conn = rusqlite::Connection::open(&path).expect("abrir para adelantar");
            conn.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
                [crawlforge_core::SCHEMA_VERSION + 1],
            )
            .expect("marcar una versión futura");
        }
        assert!(resume_precheck(&path).is_err(), "hacia atrás no se puede migrar");
    }

    #[test]
    fn reanudar_avisa_de_que_no_hereda_el_permiso_de_ignorar_robots() {
        // El motor descarta el `ignore_robots` guardado a propósito, pero `resume` promete
        // continuar «como si nunca se hubiera interrumpido». Cuando esa promesa deja de ser
        // exacta hay que decirlo: el usuario rastreó su propio sitio saltándose el robots y al
        // reanudar obtendría menos URLs sin entender por qué.
        let con = store_pausado("robots-si", true);
        let info = resume_precheck(&con).expect("el precheck debe aceptar un rastreo pausado");
        assert!(info.ignoraba_robots, "el rastreo original sí ignoraba robots.txt");

        let sin = store_pausado("robots-no", false);
        let info = resume_precheck(&sin).expect("el precheck debe aceptar un rastreo pausado");
        assert!(!info.ignoraba_robots, "un rastreo normal no debe disparar el aviso");

        // Que el aviso diga además *qué hacer* lo comprueba `i18n`, que es quien ve `Lang`.
    }

    #[test]
    fn resume_toma_el_fichero_y_no_acepta_flags_de_rastreo() {
        // `resume` no reconfigura: la configuración es la guardada en el fichero. Aceptar
        // aquí `--max-urls` o `--include` prometería algo que la reanudación no puede
        // cumplir sin romper «reanudar da lo mismo que no haber parado».
        let cli = Cli::try_parse_from(["crawlforge", "resume", "crawl-miweb-es.sqlite"])
            .expect("resume con solo el fichero debe aceptarse");
        let Command::Resume { store, csv } = cli.command else {
            panic!("el subcomando es resume");
        };
        assert_eq!(store, PathBuf::from("crawl-miweb-es.sqlite"));
        assert!(csv.is_none());

        for flag in [
            vec!["--max-urls", "100"],
            vec!["--concurrency", "8"],
            vec!["--include", "/blog/"],
        ] {
            let mut args = vec!["crawlforge", "resume", "crawl.sqlite"];
            args.extend(flag.iter());
            assert!(
                Cli::try_parse_from(&args).is_err(),
                "resume no debe aceptar {flag:?}"
            );
        }
    }

    #[test]
    fn audit_exige_la_base() {
        // El valor por defecto `https://localhost/` invalidaba la auditoría en silencio:
        // todos los canonicals absolutos salían «cross-domain» y cero páginas indexables.
        let r = Cli::try_parse_from(["crawlforge", "audit", "./dist"]);
        assert!(r.is_err(), "audit sin --base debe rechazarse");
        assert!(Cli::try_parse_from([
            "crawlforge",
            "audit",
            "./dist",
            "--base",
            "https://ejemplo.es/"
        ])
        .is_ok());
    }

    #[test]
    fn la_concurrencia_fuera_de_rango_se_rechaza_con_el_contrato() {
        // El help promete 1..=20; aceptar 0 o 50 y recortar en silencio hacía creer al
        // usuario que rastreó con lo que pidió. Y con 300 el error hablaba del rango del
        // tipo (`0..=255`), no del contrato.
        for fuera in ["0", "21", "50", "300"] {
            let err = Cli::try_parse_from([
                "crawlforge", "crawl", "https://ejemplo.es", "--concurrency", fuera,
            ])
            .err()
            .unwrap_or_else(|| panic!("{fuera} está fuera de 1..=20 y debe rechazarse"));
            let msg = err.to_string();
            assert!(msg.contains("1..=20"), "el error dice el contrato, no el tipo: {msg}");
            assert!(!msg.contains("0..=255"), "sin el rango del tipo: {msg}");
        }
        for dentro in ["1", "5", "20"] {
            assert!(Cli::try_parse_from([
                "crawlforge", "crawl", "https://ejemplo.es", "--concurrency", dentro,
            ])
            .is_ok());
        }
        // `list` comparte el contrato.
        assert!(Cli::try_parse_from(["crawlforge", "list", "urls.txt", "--concurrency", "50"])
            .is_err());
    }

    #[test]
    fn los_millares_se_separan_con_coma() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1240), "1,240");
        assert_eq!(thousands(3816), "3,816");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn el_ritmo_lento_conserva_el_decimal_y_el_rapido_no() {
        assert_eq!(format_rate(2.34), "2.3");
        assert_eq!(format_rate(14.4), "14");
        assert_eq!(format_rate(45752.0), "45,752");
    }

    #[test]
    fn la_linea_de_progreso_dice_lo_que_pide_la_revision() {
        let p = CrawlProgress {
            phase: CrawlPhase::Crawl,
            urls_fetched: 1240,
            urls_discovered: 5056,
            urls_queued: 3816,
            urls_errored: 0,
            issues_found: 32,
            elapsed: std::time::Duration::from_secs_f64(1240.0 / 14.0),
            step: None,
        };
        assert_eq!(
            progress_message(&p),
            "1,240 crawled · 3,816 queued · 14 URL/s · 32 findings"
        );
    }

    #[test]
    fn las_fases_sin_contador_se_nombran() {
        let mut p = CrawlProgress {
            phase: CrawlPhase::Sitemaps,
            urls_fetched: 0,
            urls_discovered: 0,
            urls_queued: 0,
            urls_errored: 0,
            issues_found: 0,
            elapsed: std::time::Duration::ZERO,
            step: None,
        };
        assert!(progress_message(&p).contains("sitemaps"));
        p.phase = CrawlPhase::Finalize;
        assert!(progress_message(&p).contains("final pass"));
    }

    #[test]
    fn la_pasada_final_dice_por_que_regla_va() {
        // La única fase muda del producto, y la que más puede durar: sobre un sitio de 487.621
        // URLs se midieron más de ocho horas en una sola regla. Sin esto se ve un proceso al
        // 100% de CPU sin una palabra, y lo que hace el usuario es matarlo.
        let p = CrawlProgress {
            phase: CrawlPhase::Finalize,
            urls_fetched: 0,
            urls_discovered: 0,
            urls_queued: 0,
            urls_errored: 0,
            issues_found: 0,
            elapsed: std::time::Duration::ZERO,
            step: Some(crawlforge_core::engine::FinalizeStep {
                name: "DUP-CONTENT-EXACT",
                index: 7,
                total: 29,
            }),
        };
        let linea = progress_message(&p);
        assert!(linea.contains("7/29"), "tiene que decir por dónde va: {linea}");
        assert!(linea.contains("DUP-CONTENT-EXACT"), "y cuál está tardando: {linea}");
    }

    #[test]
    fn los_pasos_previos_de_la_pasada_final_tambien_se_nombran() {
        // `internal_links_in` recorre la tabla de enlaces entera —6 millones en un sitio real—
        // y no es una regla, así que no lleva cuenta.
        let p = CrawlProgress {
            phase: CrawlPhase::Finalize,
            urls_fetched: 0,
            urls_discovered: 0,
            urls_queued: 0,
            urls_errored: 0,
            issues_found: 0,
            elapsed: std::time::Duration::ZERO,
            step: Some(crawlforge_core::engine::FinalizeStep {
                name: "internal_links_in",
                index: 0,
                total: 0,
            }),
        };
        let linea = progress_message(&p);
        assert!(linea.contains("internal links in"), "sin subrayados: {linea}");
        assert!(!linea.contains("0/0"), "un paso sin cuenta no la enseña: {linea}");
    }

    // ── El rastreo anterior se aparta, no se borra ───────────────────────────

    fn tmpdir(nombre: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("crawlforge-main-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear el directorio temporal");
        dir
    }

    #[test]
    fn el_nombre_del_backup_conserva_la_extension() {
        assert_eq!(
            backup_store_path(Path::new("crawl-miweb-es.sqlite")),
            PathBuf::from("crawl-miweb-es.prev.sqlite")
        );
        assert_eq!(
            backup_store_path(Path::new("./salidas/rastreo.sqlite")),
            PathBuf::from("./salidas/rastreo.prev.sqlite")
        );
        assert_eq!(backup_store_path(Path::new("rastreo")), PathBuf::from("rastreo.prev"));
    }

    #[test]
    fn re_rastrear_aparta_el_fichero_anterior_en_vez_de_borrarlo() {
        let dir = tmpdir("rotate");
        let store = dir.join("crawl-miweb-es.sqlite");
        std::fs::write(&store, "el rastreo de antes").expect("crear el fichero");

        let prev = rotate_previous_store(&store)
            .expect("rotar")
            .expect("había un fichero que apartar");

        assert_eq!(prev, dir.join("crawl-miweb-es.prev.sqlite"));
        assert!(!store.exists(), "el nombre original queda libre para el nuevo rastreo");
        assert_eq!(
            std::fs::read_to_string(&prev).expect("leer el backup"),
            "el rastreo de antes",
            "el contenido del «antes» sobrevive intacto"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_fichero_de_un_rastreo_vivo_no_se_rota() {
        // Revisión §3.3: con el nombre por defecto determinista, un segundo `crawl` del mismo
        // sitio rotaba el fichero **vivo** del primero a `.prev.sqlite`, y el primero acababa
        // escribiendo su pasada final dentro del fichero del segundo. Guarda de no-regresión
        // de la mitad CLI; la exclusión de los motores la prueba el core
        // (`tests/perimetro_del_fichero.rs`).
        let dir = tmpdir("rotate-vivo");
        let store = dir.join("crawl.sqlite");
        std::fs::write(&store, "rastreo vivo").expect("crear el fichero");

        let exclusiva = crawlforge_core::store::StoreLock::acquire(&store)
            .expect("simular el rastreo vivo tomando su exclusiva");
        let err = rotate_unless_live(&store).expect_err("con la exclusiva tomada no se rota");
        assert!(err.to_string().contains("crawlforge"), "el error nombra al otro proceso: {err}");
        assert!(store.exists(), "el fichero vivo sigue intacto en su sitio");

        // Libre la exclusiva —el otro rastreo terminó—, la rotación de siempre.
        drop(exclusiva);
        let prev = rotate_unless_live(&store)
            .expect("sin otro escritor se rota")
            .expect("había un fichero que apartar");
        assert!(prev.exists());
        assert!(!store.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sin_fichero_previo_no_hay_nada_que_rotar() {
        let dir = tmpdir("rotate-nada");
        let store = dir.join("crawl-nuevo.sqlite");
        assert!(rotate_previous_store(&store).expect("rotar").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_tercera_ejecucion_pisa_el_backup_de_la_primera() {
        let dir = tmpdir("rotate-dos-veces");
        let store = dir.join("crawl.sqlite");

        std::fs::write(&store, "primera").expect("crear");
        rotate_previous_store(&store).expect("rotar la primera");
        std::fs::write(&store, "segunda").expect("simular el segundo rastreo");
        let prev = rotate_previous_store(&store).expect("rotar la segunda").expect("hay backup");

        assert_eq!(std::fs::read_to_string(&prev).expect("leer"), "segunda");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_ficheros_laterales_de_sqlite_viajan_con_su_base() {
        let dir = tmpdir("rotate-wal");
        let store = dir.join("crawl.sqlite");
        std::fs::write(&store, "base").expect("crear");
        std::fs::write(sidecar(&store, "-wal"), "wal").expect("crear wal");

        rotate_previous_store(&store).expect("rotar");

        let prev = dir.join("crawl.prev.sqlite");
        assert!(prev.exists());
        assert!(sidecar(&prev, "-wal").exists(), "el -wal acompaña a su base");
        assert!(!sidecar(&store, "-wal").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── `--config`: el fichero propone, los flags disponen ───────────────────

    #[test]
    fn el_yaml_se_carga_y_los_flags_ganan() {
        let dir = tmpdir("config");
        let path = dir.join("sitio.yaml");
        std::fs::write(&path, "max_urls: 5000\nconcurrency: 8\nignore_robots: true\n")
            .expect("escribir el YAML");

        let mut job = CrawlJob::http("https://ejemplo.es");
        apply_config_file(&mut job, Some(&path)).expect("cargar la configuración");
        apply_crawl_flags(
            &mut job,
            &CrawlFlags {
                concurrency: None,
                max_urls: Some(100), // el flag pisa al fichero
                max_depth: None,
                no_sitemaps: false,
                ignore_robots: false, // no darlo no des-activa lo que pidió el fichero
            },
        );

        assert_eq!(job.limits.max_urls, Some(100), "el flag gana");
        assert_eq!(job.limits.concurrency_per_host, 8, "lo no pisado viene del fichero");
        assert!(job.limits.ignore_robots, "un bool del fichero no se resetea por omisión");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_errata_en_el_yaml_es_un_error_con_el_nombre_del_campo() {
        let dir = tmpdir("config-errata");
        let path = dir.join("sitio.yaml");
        std::fs::write(&path, "max_url: 100\n").expect("escribir el YAML");

        let err = apply_config_file(&mut CrawlJob::http("https://ejemplo.es"), Some(&path))
            .expect_err("max_url no existe");
        assert!(format!("{err:#}").contains("max_url"), "{err:#}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_concurrencia_imposible_en_el_yaml_se_rechaza_con_el_contrato() {
        let dir = tmpdir("config-concurrencia");
        let path = dir.join("sitio.yaml");
        std::fs::write(&path, "concurrency: 50\n").expect("escribir el YAML");

        let err = apply_config_file(&mut CrawlJob::http("https://ejemplo.es"), Some(&path))
            .expect_err("50 está fuera de 1..=20");
        let msg = format!("{err:#}");
        assert!(msg.contains("between 1 and 20"), "{msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_flags_de_patrones_son_repetibles_y_sustituyen_al_yaml() {
        // Repetibles en la línea de comandos, como en Screaming Frog cada patrón es una línea.
        let cli = Cli::try_parse_from([
            "crawlforge", "crawl", "https://ejemplo.es",
            "--exclude", "/wp-admin/", "--exclude", r"\?replytocom=",
            "--include", "/blog/",
        ])
        .expect("los flags de patrones deben aceptarse");
        let Command::Crawl { include, exclude, .. } = cli.command else {
            panic!("el subcomando es crawl");
        };
        assert_eq!(exclude, vec!["/wp-admin/", r"\?replytocom="]);
        assert_eq!(include, vec!["/blog/"]);

        // Y la precedencia es la de siempre: una lista no vacía del flag pisa a la del fichero.
        let dir = tmpdir("config-patrones");
        let path = dir.join("sitio.yaml");
        std::fs::write(
            &path,
            "exclude_patterns:\n  - \"/del-fichero/\"\ninclude_patterns:\n  - \"/docs/\"\n",
        )
        .expect("escribir el YAML");
        let mut job = CrawlJob::http("https://ejemplo.es");
        apply_config_file(&mut job, Some(&path)).expect("cargar la configuración");
        assert_eq!(job.limits.exclude_patterns, vec!["/del-fichero/"]);
        apply_pattern_flags(&mut job, Vec::new(), vec!["/del-flag/".to_string()]);
        assert_eq!(job.limits.exclude_patterns, vec!["/del-flag/"], "el flag sustituye");
        assert_eq!(job.limits.include_patterns, vec!["/docs/"], "lo no pisado se conserva");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_tambien_acepta_patrones() {
        assert!(Cli::try_parse_from([
            "crawlforge", "audit", "./dist", "--base", "https://ejemplo.es/",
            "--exclude", "/borradores/",
        ])
        .is_ok());
    }

    #[test]
    fn el_ejemplo_de_configuracion_del_repositorio_es_valido() {
        // Si el ejemplo documentado deja de cargar, la documentación miente.
        let ejemplo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/crawl-config.example.yaml");
        let config = load_job_config(&ejemplo).expect("el ejemplo tiene que cargar");
        let mut job = CrawlJob::http("https://ejemplo.es");
        config.apply_to(&mut job).expect("y ser aplicable");
    }

    #[test]
    fn el_xlsx_sugerido_cambia_la_extension() {
        assert_eq!(xlsx_name("crawl-miweb-es.sqlite"), "crawl-miweb-es.xlsx");
        assert_eq!(xlsx_name("rastreo"), "rastreo.xlsx");
    }
}
