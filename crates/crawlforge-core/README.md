# crawlforge-core

The crawl engine of [CrawlForge](https://github.com/dcarrero/crawlforge), a technical SEO auditor
for sites you own.

It crawls over HTTP, a built directory (`dist/`) or an exact list of URLs, extracts what matters
for search from each page, evaluates the rules of
[`crawlforge-rules`](https://crates.io/crates/crawlforge-rules) and writes everything to a single
SQLite file through one writer thread. It knows nothing about any interface: the command line and
any other front end read that file.

Most people want the command line instead: `cargo install crawlforge`. This crate is published so
the CLI can be built from crates.io; its API follows the CrawlForge version and is not stable while
that is 0.x.

The architecture is described in
[`docs/01-ARQUITECTURA.md`](https://github.com/dcarrero/crawlforge/blob/main/docs/01-ARQUITECTURA.md)
and the store schema in
[`docs/02-MODELO-DATOS.md`](https://github.com/dcarrero/crawlforge/blob/main/docs/02-MODELO-DATOS.md).

Licensed under Apache-2.0.
