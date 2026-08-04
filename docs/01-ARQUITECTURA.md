# 01 — Architecture

> Versión en español: [`es/01-ARQUITECTURA.md`](es/01-ARQUITECTURA.md)

## 1. Overview

```
┌─────────────────────┐   ┌─────────────────────┐
│  apps/macos         │   │  apps/windows       │
│  SwiftUI + GRDB     │   │  WinUI 3 + C#       │
└──────┬───────┬──────┘   └──────┬───────┬──────┘
       │       │                 │       │
   FFI │       │ read        FFI │       │ read
 (~10 fn)      │ SQLite    (~10 fn)      │ SQLite
       │       │  R/O            │       │  R/O
┌──────▼───────┴─────────────────▼───────┴──────┐
│  crawlforge-ffi                                │
│  UniFFI (Swift)  ·  extern "C" (C#)            │
└──────┬─────────────────────────────────────────┘
       │
┌──────▼─────────────────────────────────────────┐
│  crawlforge-core                                │
│  scheduler · fetch · parse · store              │
│  ┌──────────────┐ ┌──────────────┐ ┌─────────┐ │
│  │ -rules       │ │ -adapters    │ │ -hub    │ │
│  └──────────────┘ └──────────────┘ └────┬────┘ │
└─────────────────────────────────────────┼──────┘
       │                                  │ sqlx
       ▼ writes                           ▼
  ┌──────────┐                   ┌──────────────────┐
  │ crawl_N  │                   │ Postgres/MariaDB │
  │ .sqlite  │                   │  (aggregates)    │
  └──────────┘                   └──────────────────┘
       ▲
       │ same core
┌──────┴──────────┐
│ crawlforge-cli  │  → CI, cron, internal use
└─────────────────┘
```

## 2. SQLite as the FFI boundary

**This is the central concept of the architecture.** Data does not cross the FFI bridge.

The core writes the crawl result to a SQLite file. The UI opens **that same file** read-only and
runs its own queries. Only control commands and a small `ProgressSnapshot` travel across the FFI.

Consequences, all of them good:

- The FFI surface stays at ~10 functions and one callback. Writing it by hand in C for Windows is
  half a day of work, not a risky dependency.
- Each UI sorts, filters and paginates with native SQL, riding on indexes. No reimplementing
  sorting in Swift or C#.
- The core is testable in pure Rust without any UI.
- The CLI uses exactly the same store: files are interchangeable between CLI and app.
- A crawl is **one file**. Compress it and send it to a client. Huge UX advantage.

**Rules:**
- The UI opens the connection with `mode=ro` and `immutable=false` (the core may be writing to
  the WAL).
- During an active crawl, the UI refreshes by polling the progress view every 500 ms, not by
  row notification.
- The UI **never** writes to the crawl file. Preferences and view state live in their own store
  (`UserDefaults` / `ApplicationData`).

## 3. Crates

### `crawlforge-core`
The engine. Knows nothing about UI or FFI. Exposes:

```rust
pub struct Engine { /* tokio pool, config, store */ }

impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self>;
    pub fn start_crawl(&self, job: CrawlJob) -> Result<CrawlId>;
    pub fn pause(&self, id: CrawlId) -> Result<()>;
    pub fn resume(&self, id: CrawlId) -> Result<()>;
    pub fn cancel(&self, id: CrawlId) -> Result<()>;
    pub fn progress(&self, id: CrawlId) -> Result<Progress>;
    pub fn store_path(&self, id: CrawlId) -> Result<PathBuf>;
}
```

Submodules: `frontier` (queue and scheduling), `fetch` (HTTP + filesystem), `parse` (extraction
with `lol_html`), `store` (SQLite), `normalize` (URL canonicalization), `robots`.

### `crawlforge-rules`
A separate crate on purpose: the rules are the product and evolve at a different pace than the
engine.

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;              // "SEO-TITLE-MISSING"
    fn severity(&self) -> Severity;
    fn category(&self) -> Category;
    fn min_tier(&self) -> Tier;                // Free | Pro | Agency
    fn evaluate(&self, ctx: &RuleContext) -> Vec<Issue>;
}
```

Two evaluation modes:
- **Per page** (`PageRule`): evaluated during the crawl, streaming. Cheap.
- **Over the whole set** (`SiteRule`): needs the complete crawl (duplicates, orphans, depth).
  Runs in a final pass with SQL over the store.

### `crawlforge-adapters`
`trait SiteAdapter` and its implementations. See `05-ADAPTADORES.md`.

### `crawlforge-hub`
**The only crate that uses `sqlx`.** Syncs aggregates to Postgres or MariaDB. Optional, Pro tier.
Isolated so that a build without the `hub` feature does not drag in its dependencies.

### `crawlforge-ffi`
Two sibling modules over the same logic:
- `swift`: `#[uniffi::export]`, compiled into an XCFramework.
- `c`: `#[no_mangle] extern "C"`, header generated with `cbindgen`, compiled to a `cdylib`.

**No FFI function is `async`.** The core manages its own threads; progress is reported through a
registered callback or by polling.

### `crawlforge-cli`
Command-line interface built on `clap`. It is at once internal tool, Agency-tier product and
test bench.

## 3.bis The `spider-rs` decision — closed

**`spider` is used as a reference, not as a dependency.** We write our own scheduler. Decided
after evaluating it with the engine in front of us. **Not up for reopening.**

It is a mature, fast crate (2M downloads, MIT). The reasons for not depending on it are four:

1. **Release cadence.** 11 releases between June 23 and July 23, 2026. Pinning the product's hot
   path to an API that moves like that turns every update into maintenance work on the most
   critical piece.
2. **It collides with closed decision #2.** `spider` brings its own page model and its own result
   handling. We write to SQLite in batches from a single writer thread, and the file *is* the
   boundary with the UI. Adapting its output to that costs roughly the same as writing the
   `frontier`, and leaves us with a translation layer to maintain forever.
3. **We need fine-grained control of parsing.** The single-pass `PageAccumulator` built on
   `lol_html` (§5 of `03-MOTOR-CRAWL.md`) depends on the order elements appear in: first `h1`,
   heading hierarchy, `region` derived from the semantic ancestor, link position. That is ours,
   not delegable.
4. **Store builds.** A wide dependency surface is direct risk in signing, notarization and
   sandboxing, where problems show up late and expensive.

What we do take from it as a reference when writing the scheduler: its concurrency control and
its cache strategy.

## 4. The complete FFI surface

Ten functions. If it grows past fifteen, you are pushing data across the bridge. Reread §2.

```
engine_create(config_json: String) -> EngineHandle
engine_destroy(h)

crawl_start(h, job_json: String) -> String        // returns crawl_id (uuid)
crawl_pause(h, crawl_id)
crawl_resume(h, crawl_id)
crawl_cancel(h, crawl_id)
crawl_progress(h, crawl_id) -> ProgressSnapshot   // flat struct, ~12 fields
crawl_store_path(h, crawl_id) -> String

crawl_diff(h, path_a, path_b, out_path)           // produces a diff SQLite file
export(h, crawl_id, format, out_path)             // csv | xlsx | parquet | html

set_progress_callback(h, cb)                      // optional, alternative to polling
last_error(h) -> String
```

`ProgressSnapshot`: `crawl_id`, `state`, `urls_discovered`, `urls_fetched`, `urls_pending`,
`urls_errored`, `issues_found`, `bytes_downloaded`, `elapsed_ms`, `eta_ms`, `current_rate_per_s`,
`current_url`.

## 5. Concurrency

- One multi-threaded `tokio` runtime per `Engine`.
- **One limiter per host**, not global — the in-house `Throttle` from `throttle.rs`, looked up
  with each URL's host. (`governor` was rejected: it models a rate, not simultaneous connections,
  and lacks the adaptive brake; reasoning in the header of `throttle.rs`.) A single-domain crawl
  is bounded by the
  concurrency configured for that host (default 5, maximum 20); a portfolio crawl across 20
  domains can run 100 requests in flight without punishing any one server.
- Honoring the robots.txt `Crawl-delay` is mandatory whenever one exists.
- **Exponential backoff with jitter** on 429 and 5xx. Three retries, then the URL is marked as
  errored and the crawl moves on. A failure never aborts the crawl.
- SQLite writes happen **in batches** from a single writer thread consuming an `mpsc` channel.
  Never write from the workers: WAL contention with 20 writers is worse than the crawl itself.
  Default batch: 200 URLs or 2 seconds, whichever comes first.
- The queue (`frontier`) lives in memory, spilling over to SQLite past 100,000 pending URLs.

## 6. The only two platform `#[cfg]`s

Everything else must be identical between store and direct builds.

```rust
#[cfg(feature = "render_cdp")]      // direct build: chromiumoxide against the system Chrome
#[cfg(feature = "render_webview")]  // store: WKWebView / WebView2, in-process

#[cfg(feature = "scheduler_daemon")] // direct build: background service
#[cfg(feature = "scheduler_in_app")] // store: app open, or login item
```

If you find yourself adding a third platform `#[cfg]`, stop and ask: there is probably an
in-process solution that keeps parity.

## 7. Error handling

- `crawlforge-core` defines `CoreError` with `thiserror`. Explicit variants, never
  `Box<dyn Error>`.
- Per-URL network errors **are not crawl errors**: they are stored on the URL's row and the
  crawl continues.
- The FFI never panics. Every `Result` is translated into an error code plus `last_error()`. A
  panic crossing the FFI boundary is undefined behavior: wrap the entry point in
  `catch_unwind`.

## 8. Performance — measurable targets

Verified with the benchmark harness and turned into regression tests.

| Metric | Target | Reference |
|---|---|---|
| HTTP crawl, 10k-URL site | > 150 URL/s | Screaming Frog: ~50-80 |
| Filesystem crawl (`dist/`) | > 2,000 URL/s | Has no equivalent |
| RAM with 500k URLs crawled | < 500 MB | SF requires tuning the JVM heap |
| Opening a 500k-row table in the UI | < 300 ms | |
| Table scrolling | sustained 60 fps | |
| Installer size | < 40 MB | |
