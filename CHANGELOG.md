# Changelog

All notable changes to this project are documented here. Written in English only, on purpose:
the changelog is read by people integrating the tool, and one authoritative version beats two that
can drift apart.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
