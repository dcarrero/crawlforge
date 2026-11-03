# crawlforge-rules

The audit rule catalog of [CrawlForge](https://github.com/dcarrero/crawlforge), a technical SEO
auditor for sites you own.

It holds every rule's metadata (stable ID, severity, category, scope, tier), the page rules that
run on one parsed page and the site rules that run as SQL over a finished crawl store, and the
[Fluent](https://projectfluent.org/) texts of the ten languages the tool speaks, embedded at
compile time under `locales/<lang>/`.

Most people want the command line instead: `cargo install crawlforge`. This crate is published so
that [`crawlforge-core`](https://crates.io/crates/crawlforge-core) and the CLI can be built from
crates.io; its API follows the CrawlForge version and is not stable while that is 0.x.

The rule catalog, with the reasoning behind each rule, is in
[`docs/04-CATALOGO-REGLAS.md`](https://github.com/dcarrero/crawlforge/blob/main/docs/04-CATALOGO-REGLAS.md).
How to add a rule or a language is in
[`CONTRIBUTING.md`](https://github.com/dcarrero/crawlforge/blob/main/CONTRIBUTING.md).

Licensed under Apache-2.0.
