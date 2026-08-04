# Conventions and settled decisions

> Versión en español: [`es/CONVENTIONS.md`](es/CONVENTIONS.md)

> This document is the context the code takes as given. Engine comments cite it by section
> —`CONVENTIONS.md §4`— so **sections are never renumbered**: new ones are added at the end.

---

## 1. What CrawlForge is

A technical SEO auditor. It crawls a site you own, extracts what matters for rankings, evaluates a
catalog of rules and stores everything in a SQLite file.

What sets it apart from the usual crawling tools is not speed, but **managing a portfolio of sites
and comparing crawls**. A single audit is a snapshot; whoever runs dozens of projects needs to know
what changed since the last one.

**It is an auditor for sites you own.** It is not a tool for extracting other people's data: the
product only crawls the site it is pointed at, respects `robots.txt` unless the owner explicitly
allows otherwise, and does not chase external links by default. That restriction is by design and
shapes the code, not just the messaging.

## 2. Settled decisions

These are not revisited without explicit instruction.

**2.1 · A crawl's store is a SQLite file, always.** It is not configurable. This is bulk writing
from a single writer: against a remote database it would be 20 to 50 times slower. It also brings
FTS5, WAL and the property that names everything else: *a crawl is a portable file*.

**2.2 · SQLite is also the boundary between the engine and the interface.** The core writes; the
interface reads **the same file, read-only**, while it is being written. A concurrent reader is not
a corner case: **it is the architecture**. Everything that closes or moves the file has to tolerate
it.

**2.3 · A crawl is a portable file.** Whoever copies the `.sqlite` to another machine takes the
whole crawl with them. That is why a clean shutdown takes the file out of WAL mode: if stray `-wal`
files are left behind, copying just the `.sqlite` silently loses data, and that breaks the promise
at its root.

**2.4 · The core knows no interface.** It compiles and is tested on its own. If a UI type is needed
inside the core, the design is wrong.

## 3. Stack

| Layer | Choice | Note |
|---|---|---|
| Async | `tokio` | |
| HTTP | `reqwest` with rustls | rustls avoids OpenSSL's cross-platform build hell |
| HTML parsing | `lol_html` | Streaming. **Not `scraper`**: building the full DOM is 5-10x slower |
| robots.txt | `texting_robots` | |
| Rate limiting | Own `Throttle` | One limiter per host, with an adaptive brake on 429 and 503 |
| Store | `rusqlite` (bundled, WAL, FTS5) | |
| Serialization | `serde` | |
| Export | `rust_xlsxwriter`, `csv` | |
| Errors | `thiserror` in the core, `anyhow` in the CLI | |
| Logs | `tracing` | |

**The stack is closed.** Adding a dependency for something solved in twenty lines is a decision
with a cost: every crate is audit surface, compile time and a version to maintain. That is why the
CLI has its own temporary directory instead of `tempfile`, and why the basic-auth base64 is written
by hand.

## 4. Conventions

**Language: the entire codebase is in English.** Comments, module and type documentation, test
names, `assert!` diagnostic messages, identifiers, database columns, rule IDs and commit messages.
No exceptions.

This changed on 2026-08-04, before opening the repository. Until then, decision comments were
written in Spanish, and the call was made that Spanish in the code is a barrier for anyone who
wants to contribute. **`crawlforge-rules` is already converted; `crawlforge-core` and
`crawlforge-cli` are being converted.** While that lasts, a comment in Spanish is not an exception
but pending work.

Spanish survives in two places, and in both it is product text, not engineering text:

- The `name_es` and `desc_es` fields of the rule catalog, and the `i18n.rs` strings requested with
  `--lang es`. That is what the end user reads.
- The **data** of a test: the string an `assert` compares against, a text that has to exceed
  seventy characters, the accented characters used to measure the width of a typeface. That is not
  prose, it is the test's input, and translating it breaks or weakens the test.

**English is the product's source language and Spanish a translation**, not the other way around.
That is the order in which things are published and what decides which text wins when the two
disagree. The documents in `docs/` live in English and their translation in `docs/es/`, which says
so in its header. Command-line output is all in English for consistency —the argument parser's
template and its errors are not localizable—; Spanish lives where there is real text to translate,
the rule catalog and the reports.

Test names state **a claim, not an API**: `an_empty_alt_is_not_a_missing_alt`, not `test_alt_2`.
That does not change with the language.

**No `unwrap()` or `expect()`** outside tests and `main`.

**Every rule in the catalog needs a fixture and a test. No exception** — the rules are the product:
if one is wrong, the user stops trusting the whole report.

**Per-URL network errors do not abort the crawl.** They are stored on that URL's row and the crawl
moves on.

**SQLite writes go in batches from a single writer thread** that consumes a bounded channel. Never
from the workers. The channel is bounded on purpose: backpressure is the feature, not a side
effect. With an unbounded channel the queue becomes the store, and peak memory went, measured,
from 170 to 387 MB.

**Numbered, forward-only migrations.** A published one is never edited. A year-old crawl must
still open.

**Conventional Commits**: `feat(core):`, `fix(rules):`, `docs:`.

## 4.bis Versioning

Semantic, and with the major at 0 the API is not stable. In practice:

- **Patch** — a fix that changes no behavior anyone could depend on.
- **Minor** — a new rule, a command, a flag, or a fix that changes what a rule reports. **A rule
  starting or stopping to fire is minor**, never patch: someone's continuous-integration gate
  depends on it.
- **Major** — a schema change that earlier binaries cannot open, or removing a command. `1.0.0`
  will mean the schema and the rule IDs are stable, so **that jump is discussed before taking it**.

**Rule IDs never change meaning.** A historical diff between two crawls depends on it.

## 5. Antipatterns

Mistakes this project cannot afford:

1. **Loading the entire crawl result into memory to display it.** Always a paginated query and a
   virtualized table. It is exactly where the tools that load everything into RAM fail, and it is
   this product's reason to exist.
2. **In-memory crawl store.** See the previous point.
3. **Making the crawl database engine configurable.** See §2.1.
4. **Accumulating results in a vector before writing them.** This applies inside the engine and
   also in the final pass: over 500,000 URLs, the duplicate rules produce 971,000 findings, and the
   vector alone is 330 MB. Write in batches as evaluation proceeds.
5. **Prematurely abstracting over several SQL dialects.** `rusqlite` on the hot path and nothing
   else.
6. **A per-row `JOIN` on the write path.** The endpoints of `links` and `images` are resolved
   against an in-memory index; going back to the `JOIN` costs millions of index lookups on a
   medium-sized site.
7. **A rule that claims what it cannot know.** If the crawl is truncated, the rules that depend on
   the full graph stay silent. Saying nothing is preferable to saying something false.

## 6. How it is tested

`cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` before every
change. The performance regression **only asserts in `--release`**, and its floor depends on the
environment: the same version yields about five times more items per second on a development
laptop than on a shared continuous-integration runner.

And what no test replaces: **crawling a real site and checking whether what the tool says is
true**. This project's blunt defects —systematic false positives, missing indexes, incorrect
extraction— all showed up by running, not by reading. At a thousand rows any query plan looks good.
