# Changelog

All notable changes to this project are documented here. Written in English only, on purpose:
the changelog is read by people integrating the tool, and one authoritative version beats two that
can drift apart.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Versioning

While the major version is 0, the API is not stable and minor versions may change it. In practice:

- **Patch** — a fix that changes no behaviour anyone could be relying on.
- **Minor** — a new rule, a new command, a new flag, or a fix that changes what a rule reports.
  A rule that starts or stops firing is a minor bump: someone's CI gate depends on it.
- **Major** — a change to the crawl file schema that older builds cannot open, or removing a
  command. Not taken lightly, and 1.0.0 will mean the schema and the rule IDs are stable.

Rule IDs never change meaning. A historical diff between two crawls depends on it.

## [0.4.0] — 2026-08-04

### Fixed

- **List mode records the links of the pages it audits.** It was dropping every link whose target
  was not itself on the list: the row for the target was never created, so the writer silently
  discarded the edge. A page's outbound links are a property of that page, not an extension of the
  crawl, and auditing exactly the set you were given includes knowing where it points. Internal
  targets outside the list are recorded without being requested — fetching them would be crawling
  past the list, which is the one thing this mode promises not to do. External ones are checked for
  status like anywhere else.
- **List mode no longer crawls past its list when sitemaps are enabled.** It was queueing and
  downloading the URLs a sitemap declared, so a list of three could finish having fetched four. The
  sitemap is still read and its URLs recorded; they are not fetched.
- **A list-mode crawl now declares its own link graph incomplete**, with
  `truncated_reason = 'list_mode'`. By definition it only ever sees the URLs it was given, so no
  page has its linkers. Without this, the rules that need a complete graph fire on a graph that is
  all holes: a three-page list with a sitemap reported two thirds of itself as orphaned. The same
  flag stops `diff` claiming that a URL disappeared when it was simply never in the list. The
  summary says so in plain words — nothing here was cut short, so it does not use the word
  "truncated".

### Changed

- **The whole codebase is moving to English** — comments, module and type documentation, test names
  and `assert!` diagnostics — ahead of the repository opening. `crawlforge-rules` is done; the core
  and the CLI are in progress. Spanish stays where it is product text: the `name_es`/`desc_es`
  fields of the rule catalog, the `--lang es` strings, and the data a test compares against.
- **The documentation is bilingual.** English is the source and lives where it always did; the
  Spanish translation is under `docs/es/` and says in its header that the English one wins when the
  two disagree.
- **Unit tests build their schema from every published migration.** Twelve mount points each kept
  their own hand-written list, and every one of them was different and behind — the worst stopped at
  migration 001, which meant testing the orphan-page rule against the `v_orphans` that migrations
  003 and 005 exist to fix. There is now one list per crate and a test that reads the `migrations/`
  directory and fails if any file is missing from it, so the next migration cannot be forgotten
  quietly.

## [0.3.0] — 2026-08-04

### Added

- **Broken external links are now reported, and the check is on by default.** `HTTP-404-EXTERNAL`
  had been written, fixtured and tested since the beginning, and could never fire: external URLs
  were recorded without ever being requested, so there was no status to report. They are now
  probed for status only — `HEAD`, falling back to `GET` on a 405 or 501, never reading the body.
  Each distinct URL is checked once no matter how many pages link to it, so a thousand links to
  the same URL cost one request.

  The probe is deliberately polite to servers that are not yours: one request in flight per
  external host, its own 10-second timeout so a dead third party cannot stall your audit, and no
  retries. It does not fetch the external host's `robots.txt` — verifying that a link resolves is
  what the visitor's browser does when they click it, and nothing of that site is parsed, stored
  or followed.

  External checks do not count against `--max-urls`: that budget is for your own pages. A separate
  `--max-external` (10,000) bounds the work, and when it is reached the summary says how many links
  went unchecked — a cap that truncates silently makes an incomplete report look complete. Reaching
  it never marks the crawl as truncated, which would suppress the rules that need a complete link
  graph. `--no-external-check` turns the whole thing off.

- **The `resources` table is populated.** It had existed in the schema since the first migration
  and the writer had never inserted a row — a table that exists and lies is worse than no table.
  One row per resource URL, with kind, status, size and mime; `kind` comes from the response
  content type, falling back to the URL extension when the server sends nothing useful, which is
  common for fonts. Migration 008 adds a unique index on `resources(url_id)`, so resuming an
  interrupted crawl updates rows instead of duplicating them. Older crawl files still open.

  There is no page-to-resource edge, on purpose. For images that edge already exists in `images`
  and it matters: a 1.9 MB image used by one post from three years ago is not the same problem as
  the same image in the template header. For CSS and JavaScript it matters much less, because a
  900 KB bundle is loaded by the whole template — the file itself already identifies the problem.

## [0.2.0] — 2026-08-04

Two promises the engine made and did not keep. Both were found by reading the code against its own
documentation, not by a failing test — the test suite was green through all of it.

### Fixed

- **`Crawl-delay` now actually limits the host.** The module documentation said a declared
  `Crawl-delay` overrides the configured concurrency for that host. What the code did was sleep
  inside each worker task, so with a concurrency of 5 all five tasks slept in parallel and the five
  requests left together. The delay spaced batches, not requests — which is exactly the burst the
  directive asks you to avoid. A host that declares a delay is now crawled with **one request in
  flight**, with at least the declared delay between the start of one request and the start of the
  next. Other hosts in the same crawl keep the concurrency you asked for. The wait stays
  cancellable, so `--max-duration` still cuts a long delay short.
- **`--ignore-robots` now reports what is blocked.** The `blocked_by_robots` flag was initialised
  to `false` and never set, leaving its whole path dead: it reaches `evaluate_indexability`, where
  it is the highest-priority root cause of a page not being indexable. Ignoring `robots.txt` is the
  one case where a disallowed page gets crawled at all, and it is precisely when you want to know
  which ones they are — you asked to see what Google does not. Those pages now come back with
  `is_indexable = 0` and `indexability_reason = 'robots'`. Ignoring the file still means ignoring
  it entirely: no exclusion, and no `Crawl-delay` either. If `/robots.txt` cannot be read, nothing
  is marked — an unreadable file is not evidence that nothing is blocked.

## [0.1.0] — 2026-08-03

First public release. Everything below shipped before the repository was opened; it is recorded
because the reasoning is worth more than the diff.

### Added

- **Crawling in three modes.** HTTP, a built directory (`dist/`, before deploying) and an exact
  list of URLs. List mode exists so a comparison against another crawler is fair: both tools get
  the same set, so any difference comes from parsing rather than from where each decided to start.
- **59 audit rules**, each with a fixture and a test. A test fails if any rule ships without one.
- **`diff` between two crawls**, with `--fail-on <severity>` as a CI gate. A crawl is a snapshot;
  the diff is what makes it a routine.
- **`inspect <url>`** — inbound links with anchor text and page region, outbound links with their
  status codes, extracted metadata, findings and the redirect chain. Content links sort before
  template ones.
- **`report --rule <ID>`** — every affected URL, no truncation.
- **Template collapse.** When one defect repeats across pages from a single cause — a footer link,
  a heading in the layout — the report says "1 template issue (18,089 pages)" with examples instead
  of counting it eighteen thousand times. Rows are all still written; only the count changes.
- **Site share on pervasive rules.** A rule affecting 40% or more of pages carries its share, which
  separates a list of pages to fix from one systemic cause.
- **Exports** to CSV and XLSX, and reports in Markdown and HTML.
- **HTTP basic authentication** for protected staging sites, scoped to the seed host and never
  written to the crawl file.
- **Resume** for interrupted crawls, including across schema migrations that do not change what
  the engine writes.

### Fixed

Four defects worth recording, because all four were found by crawling real sites rather than by
reading code, and none would have been caught by a unit test.

- **An image is not an orphan page.** `v_orphans` asked for internal, in the sitemap and no inbound
  links — and not for the thing the rule is named after. WordPress image sitemaps produced 1,867
  false positives out of 1,912. The first fix addressed the view; the cause was in the sitemap
  parser, where `<image:loc>` was being read as `<loc>`.
- **Two missing indexes**, both the same shape: the index nobody reads existed and the one holding
  up a JOIN did not. One left the final pass sitting in a single rule for over eight hours; with
  the index the same query answers in 39 ms.
- **Lazy-loaded images were invisible.** Plugins rewrite `src` as a `data:` placeholder and move
  the real URL to `data-src`. A news site with 18,000 pages of photos reported zero heavy images
  because the images table was empty.
- **An element inside a heading glued the words around it.** `<h1>… WordPress<br />con +25 años`
  returned "WordPresscon". Found by giving another crawler the same 300 URLs: one difference in
  1,800 comparisons, and it was ours.
