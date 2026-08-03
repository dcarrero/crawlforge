# CrawlForge

**Technical SEO auditing for people who run many sites, not one.**

CrawlForge crawls a site you own, extracts what matters for search, evaluates 59 audit rules and
writes everything to a single SQLite file. Then it can tell you what changed since the last crawl —
which is the part that turns a one-off audit into something you can run every week.

```
$ crawlforge diff before.sqlite after.sqlite

── Got better ───────────────────────────────
  + 11  HTTP-404-INTERNAL  resolved
        404 → 301  /googlechrome
        404 → 301  /internetexplorer

── Got worse ────────────────────────────────
  −  1  HTTP-REDIRECT-CHAIN  new
        /googlechrome → article → mobile version
```

Eleven things fixed, one created. That is the report a snapshot cannot give you.

---

## What it does

**Crawls three ways.** Over HTTP, against a built directory (`dist/`, before you deploy), or over an
exact list of URLs.

**Evaluates 59 rules**, each with its own test fixture — indexability, HTTP status, meta, canonical,
content, assets and hreflang. The rules are the product: when one is wrong, you stop trusting the
whole report.

**Compares two crawls** and tells you what was resolved and what appeared. With `--fail-on high`
it doubles as a CI gate that fails the build when a deploy introduces something serious.

**Stores everything in plain SQLite.** No proprietary format. Any question the report does not
answer, SQL does.

## Install

Requires Rust 1.85 or newer.

```bash
git clone https://github.com/dcarrero/crawlforge
cd crawlforge
cargo build --release -p crawlforge-cli
cp target/release/crawlforge ~/.cargo/bin/
```

Runs on macOS, Linux and Windows.

## Five minutes

```bash
# Crawl a site
crawlforge crawl https://example.com/

# See what it found
crawlforge report crawl-example-com.sqlite

# Every URL affected by one rule
crawlforge report crawl-example-com.sqlite --rule CONTENT-H1-MISSING

# Who links to this page?
crawlforge inspect crawl-example-com.sqlite '/pricing/'

# Audit a built folder before deploying — no network, seconds
crawlforge audit ./dist --base https://example.com/

# Hand it to someone who does not use a terminal
crawlforge export crawl-example-com.sqlite --format xlsx --out audit.xlsx
```

Every command ends by telling you what to run next. Full guide in
[`docs/MANUAL.md`](docs/MANUAL.md).

## Measurements, with their conditions

Numbers are worth what their method is worth, so here is the method next to each one.

**487,621 URLs in a single crawl with memory flat.** A news site with fifteen years of archive,
4.4 million images, a 5.3 GB crawl file. Memory follows the pending queue, not the size of the
site: it peaked at 259 MB with 155,000 URLs queued and dropped to 123 MB once the queue drained.

**Zero extraction differences across 1,800 comparisons** against another established crawler. The
same 300 URLs to both tools in list mode — status code, title, meta description, H1, canonical and
indexability. The one difference that did show up was ours, and it is
[in the history](CHANGELOG.md): a `<br>` inside an `<h1>` was gluing the words around it.

**59 rules, each with a fixture and a test.** A test asserts that no rule ships without one.

Anything else you read about speed should carry its conditions too. Comparisons run at different
concurrency, on different machines, against different sites are not comparisons.

## How it is built

```
crates/
  crawlforge-core/      crawler, parser, rule engine, SQLite store
  crawlforge-rules/     the 59 audit rules
  crawlforge-cli/       the binary
  crawlforge-adapters/  WordPress and Astro
  crawlforge-ffi/       C and Swift bindings
```

`crawlforge-core` knows nothing about any interface. It compiles and tests on its own.

Design decisions and conventions are in [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md) — read it
before opening a pull request, it explains the choices the code takes for granted.

## What it does not do

Said up front so you do not go looking:

- **No JavaScript rendering.** A site that builds its content in the browser will look empty.
  Planned.
- **No external broken-link checking.** Outbound links are recorded, not fetched.
- **No graphical interface** yet.
- **No scheduling or portfolio dashboard.** Those live in the paid apps, when they exist.

## Licence and what is paid for

The crawler, the rules, the command line and the adapters are **Apache 2.0**. Use them, fork them,
ship them inside your own tooling.

The native desktop applications and the portfolio sync service are not open source and will be
paid. A tool that asks for your staging credentials should be readable; the interface on top of it
is the product.

## Contributing

Rules are the best place to start, and [`CONTRIBUTING.md`](CONTRIBUTING.md) explains the one
requirement that is not negotiable: **a rule arrives with its fixture, its test and its text in
both English and Spanish.** A rule with no test case is a rule nobody can trust, including you.

---

## Author

Built by **David Carrero Fernández-Baillo** — [carrero.es](https://carrero.es) — at
**Color Vivo Internet, S.L.** — [colorvivo.com](https://colorvivo.com).

An agency that runs a hundred-plus sites is where this tool comes from: it was written to solve a
problem we had, and the measurements in this README are from auditing sites we are responsible for.

Spanish version: [`README-es.md`](README-es.md)
