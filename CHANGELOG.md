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

## [0.22.0] — 2026-11-03

The engine does not change: same rules, same crawl file, same output. What changes is how it is
named and where you can get it. Minor bump because names people may already use change: the
package is now `crawlforge` (it was `crawlforge-cli`), and the release files are
`crawlforge-<target>` and `crawlforge-installer.{sh,ps1}` (they were `crawlforge-cli-…`), so a
link to `releases/latest/download/crawlforge-cli-installer.sh`, published with 0.21.2, stops
working from this release on.

### Added

- **On crates.io.** `cargo install crawlforge` installs the command line with Rust 1.88 or newer.
  The engine and the rule catalog are published too, as `crawlforge-core` and `crawlforge-rules`,
  always at the same version as the tool; their API is not stable while the version is 0.x. Each
  tagged release publishes the three from the release workflow with crates.io Trusted Publishing.

### Changed

- **The command-line package is now `crawlforge`**, not `crawlforge-cli`, so the release assets
  carry the name of the command: `crawlforge-installer.sh`, `crawlforge-installer.ps1` and
  `crawlforge-<target>.tar.gz`/`.zip` replace the `crawlforge-cli-*` names of 0.21.2. Update any
  script that downloads them by name. The Homebrew formula was already `crawlforge` and keeps its
  name. Building from source is `cargo build --release -p crawlforge`; the binary is still
  `crawlforge`.
- **The translations moved to `crates/crawlforge-rules/locales/<lang>/`** (from `locales/` at the
  root of the repository), with the same two files per language. A published crate contains only
  its own folder, and the CLI's texts were embedded from outside it. They are still compiled into
  the binary, and the completeness tests are the same.

## [0.21.2] — 2026-11-03

The first release with prebuilt binaries. The engine does not change: same rules, same crawl
file, same output. Everything below was meant for 0.21.1; see that entry.

### Added

- **Binaries for every release**, built by the release workflow (generated with `dist`): macOS
  (Apple Silicon and Intel), Linux (x86_64 and ARM64) and Windows (x86_64), with checksums.
- **Install without Rust**: `brew install dcarrero/tap/crawlforge` on macOS and Linux, and
  one-line installers for a shell (`crawlforge-cli-installer.sh`) and PowerShell
  (`crawlforge-cli-installer.ps1`) attached to each release. The macOS binaries are not notarized
  yet: Homebrew installs them without a warning, but one downloaded by hand needs the quarantine
  attribute removed before Gatekeeper lets it run.
- **Continuous checks on GitHub**: `cargo-deny` with `deny.toml` on every pull request, every push
  to `main` and weekly, and a performance A/B on every pull request that touches code (base and
  branch built on the same runner, six alternating rounds, a table in the job summary).

### Fixed

- **A test compared the published JSON Schema byte for byte**, so a Windows checkout with CRLF
  line endings failed it. It now compares the content.
- **A timing test could fail on a slow runner.** The sitemap-discovery test separated batch and
  continuous refill with a 1.5 s threshold that a loaded macOS runner exceeded (2.1 s, twice). Its
  slow sitemap now takes 1.5 s instead of 0.6 s, so the two behaviours sit ≥ 4.5 s and ~1.5 s apart
  and the threshold is 3.5 s.
- **The manual** had a table split by a blank line and still said 59 of 60 rules were implemented.

## [0.21.1] — 2026-11-03

Tagged, never released. The release workflow failed on every target because the `dist` build
profile was missing from `Cargo.toml` (`profile 'dist' is not defined`), so no binaries, installers
or Homebrew formula were published. 0.21.2 adds the profile, which inherits `release` unchanged, and
ships what this version was meant to.

## [0.21.0] — 2026-11-03

A new rule, a gentler crawler and a dependency audit. Minor bump: the catalog reports something it
did not, and the crawl behaves differently under rate limiting. Files from 0.20.0 and earlier open,
migrate and resume as before; as with migration 011, an earlier binary refuses a file written by
this one (migration 012 adds `links.fragment` and `pages.fragment_ids`). The minimum Rust version
to build from source goes from 1.85 to 1.88. The catalog has 72 rules.

### Added

- **`LINK-FRAGMENT-BROKEN`** (low, free). An internal link to `/page/#section` where nothing in
  that page is called `section`: no element with that `id`, no `<a name>`. The link opens the page
  at the top instead of the section it promised. Matching follows the HTML standard: the fragment
  as written, then percent-decoded, exact and case-sensitive. Same-page links count. Never judged:
  `#`, `#top`, text fragments (`#:~:text=…`), hash routes of JavaScript apps (`#/…`, `#!…`), CDN
  infrastructure (`/cdn-cgi/l/email-protection#…`), and targets that are not internal HTML pages
  answering 200. One finding per target page and fragment, with the number of links and source
  pages and a sample, grouped by fragment so a template's broken skip link reads as one defect.
  On its first real crawl it found exactly that: the «skip to content» link of a WordPress theme
  pointing to `#content` on 396 of 400 pages whose article template has no such id. `low` because
  an id added by JavaScript is not in the served HTML, and that jump works for a visitor.

- **`links.fragment` and `pages.fragment_ids`** (migration 012), plus a partial index on the links
  that have a fragment. The URL graph stays fragment-free. Ids are stored as sorted 32-bit hashes:
  36 KB for a Wikipedia article with 9,187 ids, between 12 and 480 bytes per page on average on
  three WordPress sites. NULL in files from earlier versions, where the rule stays silent.
  The ids are found by scanning the bytes for `id=` instead of a `[id]` selector, which cost 9-16 %
  of the parse of a long Wikipedia article; the scan adds about 0.5-0.7 ms there (+7 %) and 2-4 %
  on WordPress pages. It does not see `ID="x"` in capitals or `id = "x"` with spaces.
- **`crawlforge config schema`** prints the JSON Schema (draft 2020-12) of the `--config` YAML:
  every key with its type, default and description, and the rule IDs and severities `rules:`
  accepts, taken from the catalog. It is also published as `docs/crawl-config.schema.json`, and
  `docs/crawl-config.example.yaml` now starts with a `# yaml-language-server: $schema=…` line, so
  an editor with a YAML language server completes keys and underlines typos while the file is
  written. Built by hand, without a new dependency; tests fail if it drifts from `JobConfig`, from
  the engine defaults or from the example. `tools/gen-artefactos-web.sh` regenerates it and
  `tools/verificar.sh` fails while it is stale.
- **The crawl summary says when the server pushed back.** When the adaptive throttle halved a
  host's concurrency, the summary of `crawl` and `resume` now adds one line: "The server pushed
  back (429, 503, dropped connections or timeouts), so the crawl slowed down: its concurrency was
  halved 3 times. Next time, try a lower --concurrency." Until now it went only to the log, and a
  crawl that took three times as long looked like a slow engine. It is not stored in the crawl
  file, so `report` does not repeat it.

### Changed

- **Dropped connections and timeouts now count as overload** in the adaptive throttle. Until
  now only `429` and `503` fed the streak that halves a host's concurrency, but a saturated
  server often does not get as far as answering: it resets or refuses the connection, or does
  not answer in time. A `connection` or `timeout` failure (after the retries) now adds to the
  same streak. DNS and TLS failures do not, and neither add to it nor break it: they would fail
  the same with the server idle. `CrawlMetrics::throttle_reductions` says how many times a host
  was halved.
- **`Retry-After` on the site's own pages is obeyed.** A `429` or `503` that carries it (in
  seconds or as an HTTP date, in any of the three formats RFC 9110 requires) is retried after at
  least that long instead of the fixed 1/2/4 s backoff, and the whole host is paused meanwhile,
  not just that URL. The wait is capped at two minutes (`fetch::MAX_RETRY_AFTER`): above that the
  page is not retried at all, so a `429` is left pending for `resume` exactly as one that
  outlives the retries (`truncated_reason = 'rate_limited'`), and a `503` is recorded as such.
  `robots.txt` and sitemap downloads honour the same wait; external status probes are
  unchanged.
- **The rate-limit note no longer says "after the retries".** A page whose `Retry-After` asks for
  more than two minutes is not retried at all, so the note of `crawl` and `report` now says the
  pages "kept answering 429 Too Many Requests, or asked to wait more than two minutes", in all ten
  languages.
- **Exclusion reasons are shown translated.** The "Excluded" table of `report` and the crawl-state
  line of `inspect` printed the stored identifier (`external_check_off`, `no_network`,
  `pattern`…); they now read "external check off", "folder audit, no network", "URL pattern", in
  all ten languages. The crawl file, `stats` and the CSV/XLSX exports keep the identifier.

### Fixed

- **`crawlforge rules | head` no longer panics.** A command whose output went to a pipe that
  closed early (`head`, `less` quit early) died with "failed printing to stdout: Broken pipe" and
  exit status 101. Commands that never open a network connection now end quietly, like any Unix
  filter. `crawl`, `list` and `resume` keep the previous behaviour on purpose: a server closing a
  connection mid-write must stay a per-URL error, never a signal that ends the crawl.

### Security

- **Sitemaps are parsed with `quick-xml` 0.42** (was 0.37), past RUSTSEC-2026-0194 (quadratic
  duplicate-attribute check) and RUSTSEC-2026-0195 (unbounded namespace allocation in `NsReader`).
  A sitemap comes from a site that is not ours. The parser never used `NsReader` nor read
  attributes, so neither path was reachable, and two new tests keep it so: a tag with 100,000
  duplicate attributes and a file with 100,000 namespace declarations both parse in milliseconds.
  What a sitemap yields is unchanged, including what a broken one yields: a `<loc>` with a bare
  `&`, an unknown entity or a byte that is not UTF-8 is dropped and the rest of the file is still
  read — the new parser would have stopped there, and losing every URL after the first `/café` of
  an ISO-8859-1 sitemap is not an acceptable price for an upgrade. Two edge cases now read better:
  a comment inside a `<loc>` no longer splits it in two, and a `<loc>` cut off by the end of a
  truncated file is no longer taken as a URL.
- **XLSX export uses `rust_xlsxwriter` 0.102** (was 0.79), which brings `zip` 9 (was 2.4) and with
  it the fixes for RUSTSEC-2026-0367, -0368, -0369 and -0429. Those are about extracting archives,
  which CrawlForge never does, but the advisories were in the tree. The exported workbook is
  identical cell for cell, and the peak memory of a 1-million-row export fell from 1.23 GB to
  20 MB.
- **TLS stack updated**: `rustls` 0.23.46 (RUSTSEC-2026-0285, -0346, -0348, -0349),
  `rustls-webpki` 0.103.16 (RUSTSEC-2026-0382) and `webpki-roots` 1.0.11 (RUSTSEC-2026-0381,
  distrusted emSign intermediates). Also `tokio` 1.54.1 (unsound `AsyncFd::new`,
  RUSTSEC-2026-0386), and `quinn-proto` 0.11.19 and `chacha20` 0.10.2 (yanked 0.10.1) in the lock
  file, although neither is compiled into the binary.
- **`indicatif` 0.18** (was 0.17) drops the unmaintained `number_prefix` (RUSTSEC-2025-0119).
  The progress line looks the same.
- **A `deny.toml`** states the dependency policy: crates.io only, the licenses the tree actually
  needs, no wildcard versions, advisories and unmaintained crates fail `cargo deny check`.
- **The crates declare `Apache-2.0`**, as the `LICENSE` file always said; `Cargo.toml` still said
  `UNLICENSED` from before the repository was opened.
- **Minimum Rust is now 1.88** (was 1.85), the version `zip` 9 requires.

## [0.20.0] — 2026-11-03

Five new rules, all `free`, per-site rule configuration, and the whole output in ten languages.
Minor bump when released: the catalog reports things it did not, and there are new flags. No
schema change: the og:image travels as one more `links` row (`element = 'meta'`), a value the
column never constrained, so older crawl files open as before and older binaries read the new
ones. The catalog has 71 rules.

### Added

- **`SOCIAL-OG-IMAGE-BROKEN`** (medium, site). The page's first `og:image` returns an error —any
  4xx/5xx on your own host, 404/410 or a dead domain on someone else's—, directly or at the end
  of up to ten redirects. The card shared on WhatsApp, LinkedIn or Slack comes out without an
  image, and nothing in the HTML shows it. One finding per image URL, with how many pages declare
  it and one of them. It was planned as `pro`; it ships as `free`, like the broken-image rules.
  To have a status to read, the parser records the first `og:image` as a link and the crawl
  requests it like an `<img>` (or probes it, when it is on another host). That URL counts against
  `--max-urls`, and it is kept out of the page rules: an `http://` og:image is not mixed content.
  `twitter:image` is not checked.
- **`HREFLANG-LANG-MISMATCH`** (medium, page). The hreflang entry pointing at the page itself
  names another language than its `<html lang>` —a Spanish page announced as `en`, the
  translation plugin that copied a set without swapping the codes—. Only the language subtag is
  compared (`es-ES` and `es` agree), withdrawn aliases (`iw`/`he`, `nb`/`no`…) count as the same
  language, and it stays quiet when `lang` is missing, the page has no self-reference or the code
  is malformed: other rules own those.
- **`HREFLANG-SELF-ONLY`** (low, page). Every hreflang entry points at the page itself, so the
  annotation does nothing. Either the page has no translation and the tags are unnecessary, or
  its alternates went missing. It never fires together with `HREFLANG-NO-SELF` (its exact
  complement) or `HREFLANG-NOT-RECIPROCAL`.
- **`LINK-ANCHOR-EMPTY`** (medium, page), first rule of the new `LINK` category. Internal links
  with no text, no image and no `aria-label`/`aria-labelledby`/`title`: an icon drawn with CSS, an
  empty overlay link. Links that wrap an image are left to `ASSET-IMG-EMPTY-ALT-LINK` and
  `ASSET-IMG-NO-ALT`, so the same `<img>` is never reported twice. One finding per page, with
  the count.
- **`LINK-ANCHOR-GENERIC`** (low, page). Internal links whose whole text is a stock phrase —«read
  more», «click here», «leer más», «aquí», «weiterlesen», «lire la suite», «leia mais», «czytaj
  więcej», «続きを読む»…—. The phrases live in one list per language (English, Spanish, German,
  French, Italian, Dutch, Turkish, Portuguese, Polish, Japanese), chosen by the page's
  `<html lang>`; words that can be a name elsewhere («Go», «Plus», «Link», «Veja», «Tu», «詳細»)
  only count when the page declares that language. Matching folds case, accents and full-width
  Latin letters (`ＲＥＡＤ ＭＯＲＥ`). A link whose text is its target's own tag slug is left
  out, percent-decoded, so `/tag/wi%C4%99cej/` named «Więcej» is a tag and not a stock phrase.
  One finding per page, grouped by the phrases found, so an archive template's «Leer más» reads
  as one cause.
- **Eight new languages: German, French, Italian, Dutch, Turkish, Brazilian Portuguese, Japanese
  and Polish.** `--lang de|fr|it|nl|tr|pt|ja|pl`, or `CRAWLFORGE_LANG`, translates every command's
  output and the rule catalog, with each language's thousands separator and plural forms.
  `rules.json` and `rules --format json` now carry `name` and `description` in all ten languages.
  Help texts, argument errors, exports (XLSX/CSV) and identifiers (rule IDs, `--fail-on` tokens,
  categories, stored values) stay in English.
- **Per-site rule configuration, applied when the crawl file is read.** A `rules:` block in
  the `--config` YAML disables rules (`disable: [META-DESC-TOO-LONG]`) and changes severities
  (`severity: {HTTP-404-EXTERNAL: low}`). The crawl still evaluates every rule and stores every
  finding with the catalog's severity; `crawl`, `list` and `audit` save the block inside the
  crawl file (in `crawl_meta.config_json`, no schema change), and `report`, `inspect`,
  `export`, `diff` and `portfolio` apply it. Changing the criteria never requires a new crawl.
- **`--config <YAML>` on `report`, `inspect`, `export`, `diff` and `portfolio`.** Its
  `rules:` block replaces the stored one for that run; a file without `rules:` shows every
  finding. An unknown rule ID or severity is an error that names it.
- Every output that applies a configuration says so ("2 rules disabled, 1 severity
  changed"): the terminal summary, the Markdown/HTML report, the URL card, the diff warnings,
  the portfolio warnings, the XLSX `Summary` sheet and, for CSV, the terminal.

### Changed

- The `og:image` is now a URL of the crawl: on an internal host it is requested like an image and
  appears in `urls`, `links` (as `element = 'meta'`, `rel = 'og:image'`) and the exports. Pages
  keep their `internal_links_out` figure: the og:image is not counted as a link.
- **`diff` evaluates both crawls with one configuration**: `--config` if given, otherwise the
  one stored in the later crawl. Disabling a rule between two crawls therefore does not show its
  findings as resolved, and `--fail-on` judges the overridden severity. When the two files store
  different configurations, the diff warns, and the `rules` block is not listed among the
  crawl configuration changes (it does not change what was crawled).
- **`portfolio` reads each site with its own stored configuration** and names the sites that
  have one, so a disabled rule does not read as a site without that problem.
- Exports leave out the findings of disabled rules and write the overridden severity.
- **Texts moved to Fluent files, ready for ten languages.** The CLI messages and the rule names
  and descriptions now live in `locales/<lang>/cli.ftl` and `locales/<lang>/rules.ftl`, embedded
  in the binary. `--lang` and `CRAWLFORGE_LANG` accept `en`, `es`, `de`, `fr`, `it`, `nl`, `tr`,
  `pt`, `ja` and `pl` (with or without a region: `pt-BR`); a language not translated yet shows
  English, numbers included. Plurals go through CLDR plural rules, so a translation can have the forms its language needs (Polish has
  four). Terminal tables align by display width, so Japanese text does not break the columns.
  `rules --format json` and `rules.json` keep their shape; a language joins `name` and
  `description` once its translation is complete. `CONTRIBUTING.md` explains how to add or
  improve a language.
- **Labels that were still English in every language are translated**: the URLs row of the
  Markdown/HTML report, the severity headers of the portfolio's at-a-glance table, the severity
  in the rules table and on a rule card, and the non-indexability reason in `report` and
  `inspect` (`canonicalised` reads «canonicalizada» with `--lang es`). The stored values do not
  change, and exports and `stats` keep writing them as they are.
- **Columns take the width of the longest label of the language in use**, never less than the
  English width: the `inspect` card, its region column, the `diff` header and warnings, the rules
  table, the report summary, the portfolio table and every severity column. No translation has
  to shorten a word to fit, and English output does not move.

### Fixed

- **English plurals**: «1 URLs», «1 new finding(s)», «1 change(s)», «1 external links were not
  checked» and the rate-limit notes now agree in number. Spanish gets the same.
- **Counts of 1,000 or more** in some messages reached the text ungrouped and took the default
  plural form: the `--fail-on` result, the template-issue headline, the character count on the
  `inspect` card, the base-mismatch warning, the portfolio counts and date-spread warning, the
  deep-pages summary and the rules summary. The `diff` header counts now agree in number too.
- **The truncation warning of `diff`** is two complete sentences, with and without the reason,
  so each language puts the reason where its grammar wants it.
- **Japanese rule cards wrap properly.** `rules --detail` broke Japanese after every ASCII word
  and could leave a line with only «。»; lines now fill up, and break with the usual rules
  (no closing punctuation at the start of a line, no opening bracket at the end).
- English rule descriptions use “…” quotation marks, and `ASSET-FORM-BROKEN` lost a stray
  dash-and-colon.

## [0.19.0] — 2026-10-28

A new rule. Minor bump: the catalog reports something it did not. No schema change; the rule
reads what the parser sees while the page streams. The catalog has 66 rules.

### Added

- **`META-OUTSIDE-HEAD`** (high). The canonical, meta robots, hreflang, title or meta
  description ends up outside the `<head>`, so Google ignores it although it is still in the
  source. An element that does not belong in the head —a tracking `<img>`, a `<div>`, an
  `<iframe>`, stray text— makes the HTML parser close the head right there and open the body;
  Google documents the same behaviour ("Google ignores any elements that appear after the
  invalid element"). A PHP warning printed before the document has the same effect on every tag
  of the page, and is caught too. The finding says what closed the head, with the attribute that
  identifies it (`<img src="https://…">`), because that is the fix. It also fires on pages we
  see as `noindex`: the `noindex` itself may be one of the lost tags.

  The parser follows the HTML standard's rules for the head, not a tag list: `<noscript>`
  content is skipped as a browser with scripting does, so the Meta Pixel's
  `<noscript><img></noscript>` is not a finding, and a `<link>` between `</head>` and `<body>`
  goes back to the head. Checked against parse5, a full WHATWG parser, on 26 hand-written cases
  and the home pages of 81 real sites: they agree on all but one, a stray `</br>` in the head, which the
  standard turns into `<br>` and this parser does not see. None of the 81 real sites had the
  defect. It adds 3-7 % to the parse of a real home page, about 0.03 ms.

## [0.18.0] — 2026-10-28

A new rule, and a new column in the crawl file to feed it. Minor bump: the catalog reports
something it did not, and files from 0.17.0 and earlier open, migrate and resume as before. The
catalog has 65 rules.

### Added

- **`DUP-CONTENT-NEAR`** (medium). Two or more indexable pages whose visible text is at least
  80 % alike while their HTML differs. `DUP-CONTENT-EXACT` only catches byte-for-byte copies, and
  on a CMS those are rare: a nonce, a date or a "recent posts" widget is enough to change the
  hash. The rule compares a 128-byte b-bit MinHash of each page's text (word 3-shingles, without
  `nav`, `footer` or `aside`), finds candidates by locality-sensitive hashing instead of comparing
  every pair, and groups pages transitively under one `group_key`. Left out: texts under 50
  shingles, which `CONTENT-THIN` already covers; groups where every page has the same HTML, which
  are `DUP-CONTENT-EXACT`; and pages that declare each other as hreflang alternates, which are
  translations. That last exclusion came from crawling a real site, where the Spanish and English
  lists of 222 World Cup stadiums were 96 % alike because proper names do not translate.

  80 % and not Screaming Frog's 90 %, because 90 % found nothing real. On the same site, one
  stadium published under two slugs measured between 72 % and 85 % (exact Jaccard, checked by
  hand): hundred-word pages that repeat a name the two versions spell differently, where one
  changed word changes three shingles. Pages built from one template with different data
  measured 62 %, and distinct posts on a WordPress blog 25 % at most.

  The planned version of this rule was a 64-bit SimHash in the Pro tier. SimHash was dropped
  because one 64-bit fingerprint is too noisy for a threshold like this: at the distance that
  catches 90 % alike pages, a 200,000-page crawl already has dozens of unrelated pairs that fall
  under it by chance. And the rule is free, because the free tier limits scale, not findings.

- **`pages.text_minhash`** (migration 011), the signature the rule reads. 128 bytes per page;
  NULL for short texts and for pages crawled by earlier versions, so the rule finds nothing in
  old files and a resumed crawl only compares its newer half. Computed while the HTML streams,
  in constant memory. It adds about 0.11 ms of parsing to a 2,700-word page.

### Fixed

- **`pages.text_hash` was documented as a SimHash for near-duplicates.** It is an exact hash of
  the body text, filled only when the Pro full-text index is on, and no rule reads it. The data
  model now says so; the column is unchanged.

## [0.17.0] — 2026-10-12

A page the site rate-limited is no longer reported as broken. Minor bump: what the rules report
changes, and the crawl file gains a `truncated_reason`.

### Fixed

- **A 429 was stored as the page's status, and read as a broken page.** When a site rate-limits
  the crawler —Cloudflare does it to bots it doesn't know— pages that still answered
  `429 Too Many Requests` after the retries were written with that status, and every rule that
  takes an internal 4xx as "broken" believed it. Crawling colorvivo.com on 2026-10-12, with
  Cloudflare limiting the crawl, 88 pages that open fine came out as critical
  `HTTP-404-INTERNAL`. In August the same site, crawled at the same concurrency, answered 200.

  Those pages are now left `pending`: not processed, not counted and not reported. The crawl is
  marked incomplete with `truncated_reason = 'rate_limited'`, so the rules that need the whole
  site stay quiet, and the summary and the report say how many pages were left and how to get
  them. A 429 that clears within the retries is processed as always.

### Changed

- **`resume` accepts a finished crawl that the site rate-limited**, and fetches only its pending
  pages. Any other finished crawl is still run again, not resumed.

## [0.16.0] — 2026-10-12

A new rule, and a fix to how title and description widths are measured. Both change what the
catalog reports, so it is a minor bump. The catalog has 64 rules.

### Added

- **`HTTP-EXTERNAL-UNVERIFIABLE`** (low). An external `<a>` link whose host refused the status
  check with a 401, 403 or 429: usually an anti-bot wall, a paywall or a rate limit, and the page
  almost always opens in a browser. `HTTP-404-EXTERNAL` keeps those codes out because they don't
  prove the link is broken, and that stays as it was; but they don't prove it works either, and
  until now they appeared nowhere. The finding is on the external URL, one per URL, `<a>` links
  only. 407, 451 and 400/405 stay out of both rules.

### Fixed

- **The em dash and the en dash were measured too narrow.** The Arial width table behind
  `META-TITLE-TOO-LONG`, `META-TITLE-TOO-SHORT`, the description rules and the stored
  `title_px`/`meta_desc_px` gave the em dash `—` the default 556 thousandths of an em instead of
  1000, and the en dash `–` the hyphen's 333 instead of 556. A `Title — Site` title came out 9 px
  short, so titles with a brand suffix could pass the length check by a few pixels. Found while
  preparing copy changes for this project's own website.

## [0.15.0] — 2026-10-12

The portfolio panel counts sites, not files. It is a minor bump because what the panel reports
changes, and so does what fits under a tier's site cap.

### Changed

- **A `list` that mixed sites counts as each of its sites.** `crawlforge portfolio` counted one
  site per crawl file, so a list with fifty domains in one file passed as a single site: in the
  header, in "fires on N of M sites" and against the tier's cap (`max_portfolio_sites`, 10 in
  Pro). The file is still one row of the glance table, labelled with all its sites as in 0.14.0,
  but the counts add up its sites. The findings of a multi-site file are split by the site of
  their URL, so a rule that fires on two of its three sites counts two; findings on external
  URLs don't make their host a site, and site-level findings, which carry no URL, count once.
  On an incomplete crawl, a full-graph rule is inconclusive on each site where it did not fire.

## [0.14.0] — 2026-10-12

A `list` crawl that mixes sites is named after all of them. It is a minor bump because the report's
title changes, and something may be reading it.

### Fixed

- **The report of a multi-site list was titled after its first line.** Since 0.11.0 a list can
  audit several sites, but `report --format md|html` still said `Audit of https://first-site/`,
  as if the others had not been audited. It now says `Audit of 2 sites: first-site, second-site`,
  naming up to three and counting the rest (`… and 9 more`). The portfolio panel labelled the
  file the same way and now uses the same list. The sites are read from the file itself, the
  listed URLs being the only internal rows at depth 0, so lists crawled with 0.11.0 or later get
  the new title without being crawled again. A list from before 0.11.0 marked the other sites
  external, and keeps the title of the one site it audited.

## [0.13.0] — 2026-10-12

External URLs that are recorded without a status check now say why. It is a minor bump because
the crawl file gains two `exclusion_reason` values and the report changes what it says about them.

### Fixed

- **The report of a folder audit blamed the wrong thing.** `crawlforge audit ./dist` makes no
  network requests, so external links are recorded without a status. The report counted them as
  links left over by the `max_external` cap and advised raising it with `--max-external`, a flag
  `audit` does not accept. It now says that auditing a folder makes no network requests and that
  checking them means crawling the published site. Older `audit` files, which carry no reason,
  get the right message too: they are recognised by their mode. Found by auditing this project's
  own website, which reported 314.

### Changed

- **`exclusion_reason` is filled in for external URLs that were never checked.**
  `external_check_off` with `--no-external-check`, and `no_network` in a folder audit. Until now
  both were left empty, which made the row identical to a probe that never went out: anyone
  querying the file could not tell "not asked for" from "cut short". `pattern`, `nofollow` and
  `local_network` already worked this way.
- `inspect` shows such a URL as "external URL — status not checked (reason)", not as excluded from
  the crawl: it was recorded, and what is missing is its status.



The file `crawlforge diff --out` saves can be opened again. Until now the tool wrote it and did not
know how to read it: `report` refused it, politely, and the only way in was a SQLite client. It is a
minor bump because `diff` gains a form and two flags.

### Added

- **`crawlforge diff <file>` opens a saved diff.** Give it the file written by `--out` and nothing
  else, and the summary comes back exactly as it was printed that day, in whichever language you
  ask for now, with a line saying when and by which version it was generated. A saved diff is only
  read, so `--out` and `--fail-on` are refused there with a message that says to run them on the
  crawls.
- **`--rule <ID>` and `--change <type>` list every row**, without the three-example cut-off of the
  summary — the same idea as `report --rule`. They work on a saved diff and on a live comparison.
  `--rule` lists the new and resolved findings of one rule; `--change` takes one of the twelve
  change types (`url_removed`, `status_changed`, `title_changed`…), and a typo lists them all. Title,
  meta description and canonical changes show the full text before and after; a changed robots.txt
  shows the lines that went and the lines that came, not both files.
- When a summary list is cut short, the summary now ends by saying how to see the rest.

### Changed

- **Warnings are stored structured in the diff file.** Each entry of `diff_meta.warnings_json`
  keeps its English `message` and `breaks_conclusion`, and adds a `kind` with its data, so a
  reopened diff can say them in Spanish. Files written by 0.11.0 or earlier still open: their
  warnings come back as the English text they saved, and an inconclusive diff stays inconclusive.
  The diff file format version does not change — the keys are additive.
- Passing a single crawl file to `diff` explains that comparing needs two, and if its
  `.prev.sqlite` is next to it, prints the exact command.
- `report`, `export` and `inspect` given a diff file now say how to open it.

### Fixed

- The sitemap lines of the diff summary stopped at six with no sign that there were more. On a real
  20,000-URL crawl there were 1,177. The summary now says "… and 1,171 more".

## [0.11.0] — 2026-09-24

A list that mixes sites now audits all of them. It is a minor bump because what a crawl reports
changes: rules fire on pages that used to be treated as somebody else's, and stop firing on what
used to be probed as external.

### Changed

- **Every site in a `list` file is internal.** Until now "internal" was decided by the first URL in
  the file, so a list carrying several blogs audited the first and treated the rest as foreign.
  Their pages were fetched but recorded with `is_internal = 0`, their broken pages came out as
  `HTTP-404-EXTERNAL`, and every page of theirs that was linked but not listed got a status probe —
  121 of them in a twelve-URL list of two real sites. Two of those probes produced false
  `ASSET-BROKEN` findings on Cloudflare's `/cdn-cgi/` scripts, which the engine deliberately ignores
  on a site of your own.

  Internal now means *one of the audited sites*, compared by host and port as since 0.10.0. A page
  of any of them that is linked but not in the list is recorded and not fetched, as it always was
  for the first one.

- **A link between two sites of the list is not internal to its page.** Both sites are audited, but
  a link from one to the other leaves the page's site: it does not count in `internal_links_out`,
  and a `nofollow` on it does not fire `INDEX-NOFOLLOW-INTERNAL`. With one site nothing changes —
  `http` and `filesystem` crawls behave exactly as before.

Still tied to the first URL, on purpose: the `CRAWLFORGE_AUTH` credential (choosing which of several
hosts gets it would be guessing), and which site's sitemaps are read when the core runs a list with
sitemaps on (the CLI turns them off in list mode).

## [0.10.0] — 2026-08-26

Four new rules and one change to what counts as your own site. Both halves are minor bumps by this
project's own rule: rules start firing, and a rule that starts firing can break someone's CI gate.

### Added

- **`ASSET-JS-HEAVY` and `ASSET-CSS-HEAVY`.** The `resources` table has been filled since 0.8.0 and
  got a sheet in 0.9.0, and until now nothing judged it: the number was there and no rule said
  whether it was too much. A script over 250 KB as delivered, or a stylesheet over 100 KB, is now a
  `medium` finding. The stylesheet bar is lower on purpose — CSS is render-blocking, so the same
  bytes cost more — and the script bar sits above a normal framework runtime, so what it catches is
  a bundle nobody split rather than a site that chose React.

  **Only what the site serves itself.** A heavy script on someone else's CDN is not something the
  owner can split, and its size arrives from a `HEAD` probe rather than a downloaded body, so
  asserting on it would mean judging a number of a different quality.

- **`ASSET-IFRAME-BROKEN`** (high): an `<iframe>` pointing at a URL of yours that returns 4xx or
  5xx. The visitor sees a blank hole where the map or the video should be, the containing page
  stays valid HTML, and nothing else in the report mentions it.

- **`ASSET-FORM-BROKEN`** (critical): a form whose `action` points at a URL of yours that returns
  4xx or 5xx. The form looks fine, fills in fine, and is lost on submit.

  **GET forms only** — a search box, a catalogue filter. Checking a POST would mean submitting the
  form, which this tool does not do, so the parser does not record those destinations at all: a row
  that cannot be judged is worse than no row. The `<form action>` was in the link-element enum since
  the beginning and the parser never emitted it, which is why the earlier attempt at this rule was
  a false one.

  It is the only `ASSET` rule at `critical`. A form that swallows what was typed costs customers
  outright, and nobody reports it: whoever hits it leaves without saying so.

### Changed

- **The port is now part of "same site".** `is_internal` compared hosts only, so auditing
  `http://localhost:3000` treated a link to `http://localhost:8080` as your own site: its pages
  entered the internal count, its 404s were reported as `HTTP-404-INTERNAL`, and the link graph
  mixed two different applications. In production this is rare; in development it is routine, and
  development is exactly where the mode that audits a `dist/` before publishing gets used.

  `https://example.com` and `https://example.com:443` still mean the same thing — the default port
  for the scheme normalises away. **This is why the bump is minor:** on a site served on a
  non-default port, findings move between the internal and external sides of the report.

  Sitemap discovery derived its own bare host and had to be aligned, or a sitemap of your own site
  served on an explicit port was discarded whole as "outside the audited site". The
  authentication test caught it.

### Performance

- **Migration 010 indexes `links(element)`.** The two new embed rules filter on that column, and
  without an index the plan was a full scan of the links table **for each of them**. Caught by the
  release regression, which fell from 107,702 items per second to 81,169 — a 24% drop that the
  test's own threshold would have let through. With the index it is back to 104,493, and the rest
  is the honest cost of two more queries.

  A first guess blamed the `is_internal` change for building a `String` per link. That was real —
  it is called once per link of every page — and fixing it to compare without allocating bought
  4%, not 24%. The scan was the cause. Both fixes stay, and a plan test now fails if that scan
  ever comes back.

### Documented

- **A list is one site.** In `list` mode, "internal" is decided by the first URL in the file, so a
  list mixing domains audits the first and treats the rest as external. The restriction was
  undeclared; it is now in the manual, in both languages, until it is lifted.

## [0.9.2] — 2026-08-26

### Fixed

- **A crawl waiting on other people's servers no longer looks dead.** External status probes were
  added into `urls_queued` with no label, so a crawl that had finished the user's own site and was
  only waiting on foreign hosts still showed thousands of "queued" that were not its own — and as
  the rest drained, the figure sat still with no explanation. Probes now travel in their own
  `CrawlProgress` field, `externals_pending`, counting both queued and in flight.

  The CLI says it outright. While the site is still being crawled the probes are a note at the
  margin (`… · 1,204 external`); once nothing of the site is left, the line becomes
  `5,865 crawled · checking external links · 1,204 left · 2,834 findings`. That last state is the
  one that read as a hang: it runs at the speed of the slowest server that belongs to someone
  else, one request in flight per host, out of courtesy.

  No crawl data changes, no rule fires differently, and the crawl file is byte-for-byte what it
  was. The only API change is a field added to `CrawlProgress`, a struct nothing outside this
  repository builds today.

  Two tests, because the first one was not enough: one on the emitter, and one on a whole crawl —
  the emitter test kept passing with the wiring reverted, which is exactly the failure it was
  supposed to catch.

## [0.9.1] — 2026-08-26

### Fixed

- **`v_broken_links` now follows redirects to where they end up.** The view only looked at a
  link's immediate destination, so a page linking `/go/product`, which 301s to a shop returning
  404, appeared nowhere: a 301 is not a broken status, and nothing links the 404 directly, so the
  row fell between the two conditions. 0.9.0 started registering and checking that destination;
  this is the half that makes the work visible.

  Two new columns carry what following the chain would otherwise lose. `via` is the URL that was
  actually linked — the one to rewrite — and `hops` how many jumps it took. Both are `NULL` on a
  direct row, which is the difference between "this is broken" and "this leads to something
  broken". The `Broken links` sheet gains the same two columns, and `broken_links.csv` exports
  them.

  The chain is walked in SQL so the view stays self-contained: opening the crawl file with
  `sqlite3` answers the question without going through the tool. A ten-hop cap stops a redirect
  loop from spinning forever; `HTTP-REDIRECT-CHAIN` already reports at two, so the cap is a
  safety net rather than a judgement. Measured on a real crawl of 145,191 links with 486
  redirects, counting the view went from 8 ms to 21 ms.

  **Patch, not minor, on purpose:** no rule reads this view — only the export does — so no rule
  starts or stops firing and no rule ID changes meaning. What changes is that links which were
  broken all along are now listed. If you count rows in `broken_links.csv`, that count can go up.

  Migration 009 redefines the view. Older crawl files open and migrate as usual, and a direct
  broken link keeps reporting exactly as before, with `via` empty. Verified by reverting the
  migration and watching the test fail, plus a test that a redirect loop makes the view answer
  instead of hang.

## [0.9.0] — 2026-08-25

### Added

- **A `Resources` sheet in the XLSX export, and a `resources.csv`.** The table has been in the
  schema since migration 001 and the crawler has been filling it since 0.8.0, but nothing could
  read it: no sheet, no CSV, no report. The case that justified the table — a 900 KB `bundle.js`
  served on every page of a template — could not be answered with any command.

  One row per resource URL, not per (page, resource) pair: that edge only exists for images
  (`docs/02-MODELO-DATOS.md §3.5`), so the sheet answers "what is the heaviest thing I am
  serving", not "on how many pages". Sorted by size, largest first; resources whose server sent
  no `Content-Length` sort last rather than first.

- **The `Redirects` sheet now carries `to_status`**, the status code of the destination. With
  external checks on — the default — that is the column that shows the expensive case at a
  glance: a redirect of yours landing on someone else's 404.

### Fixed

- **The external destination of a redirect now gets a row, and a probe.** A `/go/product` URL
  that redirects to another shop was recorded as a 301 and nothing else: its destination existed
  in no mode, `redirect_to` was left unresolved, and no rule could say the far end was dead. This
  is the pattern where links rot most, precisely because the destination belongs to someone else
  and nobody tells you when it falls over.

  The destination is now registered and status-checked exactly like an external link — same
  budget (`max_external_urls`), same probe queue, same perimeter, same courtesy of one request in
  flight per foreign host. It is still only a status check: HEAD, no parsing, no page row.

  **This is a minor bump because rules start firing on data that did not exist before.**

- **A foreign host whose probe queue emptied never came back to the round.** `ExternalQueue::push`
  decided whether to put a host back in the dispatch round by asking "is this host new?", but a
  host's entry outlives its queue — it holds the per-host budget and the failure streak — so the
  second time a host was discovered its probe was queued and dispatched by nobody. In `debug` it
  tripped the scheduler's own invariant; in `release` the probe was silently lost and surfaced at
  the end as an external "not checked", with no reason given.

  It needs two URLs of the same host discovered at **different moments**, which is exactly what
  two redirects to the same shop do — so registering redirect destinations is what brought it to
  the surface. Both fixes are covered by tests verified by reverting them and watching them fail.

## [0.8.0] — 2026-08-08

### Fixed

- **In `audit`, a link to another domain is no longer resolved against the audited site.** The
  filesystem mode unifies `/about`, `/about/` and `/about/index.html` because a static server
  serves all three from the same file. That unification looked at the **path alone**, so any
  external URL whose path happened to exist inside `dist/` was canonicalised to the audited site.
  The common case was not exotic: `https://any-domain.com/` has path `/`, which resolves to
  `index.html` — so **every link to another site's home page was recorded as a link to the home
  page of the site being audited**.

  The damage went past the wrong row. Those external links vanished from the graph, so
  `HTTP-404-EXTERNAL` could not check them, and in exchange they inflated the inbound link count
  of the home page, which other rules read as a signal.

  **This is a minor bump because a rule stops firing:** `INDEX-NOFOLLOW-INTERNAL` no longer
  reports share buttons and any other `nofollow` link pointing at another domain's root. If a CI
  gate was counting those, the count changes.

  Found by auditing our own website, not by a test — twelve devlog entries were reported for
  linking to `chatgpt.com/?q=…` and `grok.com/?q=…`. It is the third time this project has decided
  something from the text of a path instead of from the real destination. Regression tests in
  `tests/enlaces_externos_en_dist.rs`, both verified by reverting the fix and watching them fail.

## [0.7.1] — 2026-08-08

### Fixed

- **The User-Agent now points at a URL that exists exactly as written.** It announced
  `https://crawlforge.org/bot` and the site serves `/bot/`. The difference is one character and it
  matters more here than almost anywhere else: that URL is the whole point of identifying
  yourself, and it is the one address this tool asks a stranger to type. Getting there through a
  redirect works until a server is configured not to add one, and then the only promise the
  crawler makes to an administrator reading their logs resolves to a 404.

- **`tools/verificar.sh` no longer dies on a step that does not apply here.** It called a check
  that lives in the private repository, so with `set -e` the verification this project's
  documentation tells you to run failed on a fresh clone — before reaching the performance
  regression, which is the step that matters most. The step is now skipped with a note when the
  check is absent.

## [0.7.0] — 2026-08-04

### Added

- **`crawlforge portfolio <path>…`** — the aggregate panel across many crawl files. A directory
  is scanned for `*.sqlite`; each site's `.prev.sqlite` is picked up automatically as the
  "before" of its pair. Three sections: **what changed** (the per-site diff aggregated and
  ordered by impact, new critical/high findings first), **what fails across the portfolio**
  (on how many sites each rule fires — a rule firing on most sites usually means a shared
  template or plugin), and **one line per site**, worst first. `--lang es`, `--format md|html`
  and `--out` work as in `report`.
- The portfolio counting separates three states per rule and site: fires, does not fire, and
  **could not be evaluated** — a truncated or list-mode crawl never evaluated the full-graph
  rules, and counting it as "clean" would be false ("9 of 12 sites, 2 inconclusive"). Mixed
  rule-catalog versions and a date spread wider than a week are warned about at the top of the
  panel. A file that cannot be opened is set aside with its reason; the panel is still produced.
- `Feature::Portfolio` in the entitlement model, minimum tier Pro. `Limits::max_portfolio_sites`
  now means one thing only: `None` is "no limit" on every tier — access is the feature's job,
  the number is just the cap once access is granted.

## [0.6.2] — 2026-08-04

### Security

- **A local audit is no longer reachable through a public name that answers privately.** A crawl
  every one of whose targets is local may reach that network — deliberate, because whoever started
  it is inside that network already. What that exemption must never cover is `10.0.0.5.nip.io`: a
  name anyone can aim anywhere, pointed at an address the operator never wrote down. Everything
  the exemption exists for — `localhost`, `nas.lan`, a literal `192.168.1.10`, a host named on the
  command line — is a name that is not public, and still goes through.

  Removing the exemption altogether was tried first and put back: in a local audit the operator is
  already inside the network, so what it grants an attacker is far less than it appears, and losing
  it would stop a local audit from checking links to another service of its own network. The narrow
  shape is the one that had no legitimate use.

## [0.6.1] — 2026-08-04

### Fixed

- **An external URL the perimeter rejects while resuming now says so.** Its row was left exactly
  as a probe cut mid-flight leaves one — external, skipped, everything null — so every later
  resume read it and rejected it again, and the report could not tell "not checked because it
  points at your own network" from "never got around to checking it". It now carries
  `local_network` like every other rejection, and counts as unchecked.

## [0.6.0] — 2026-08-04

A second security review took the perimeter that 0.5.0 shipped and broke it, executing every
case. This is what it took to close it properly.

### Security

- **The network perimeter now decides on the address actually dialled, not on the text of the
  host.** The screen in 0.5.0 was lexical, and a lexical screen cannot defend against a name.
  Public wildcard DNS services make that trivial and require the attacker to own nothing:
  `localtest.me` and `lvh.me` resolve to `127.0.0.1`, and `nip.io` and `sslip.io` resolve to
  whatever address you spell into the name — including `169.254.169.254.nip.io`, which walked
  straight past the cloud-metadata exception that 0.5.0 documented as non-negotiable. Verified
  end to end against a service on loopback, with the screen on: `HTTP 200`.

  Name resolution now happens behind a resolver that filters the addresses before any of them
  reaches a socket, and a name is rejected whole if **any** of its addresses is out of bounds —
  keeping the public ones would let DNS pick, which is half of a rebinding attack. The lexical
  screen stays as the first line, because a host written as a literal IP never reaches a
  resolver at all.

- **`--follow-external` no longer bypasses the perimeter.** It was consulted only on the probe
  path, so a crawl with that scope reached the same addresses the probe could not — and worse,
  with a full `GET` whose body was parsed and stored. A resumed crawl no longer inherits it from
  the file either, for the same reason `tier` and `--ignore-robots` are not inherited. The CLI
  has no such flag; both manuals said it did, and no longer do.

- **The perimeter is decided by every target of the crawl, not by the first one.** In list mode
  the first line of the file switched the screen off for all the others — and a line with no host
  at all, such as a stray `mailto:`, switched it off entirely. List files often come from
  elsewhere.

- **A resumed crawl does not grant the local-network exception.** A crawl aimed at a local target
  may reach that network, because whoever started it was already inside it. On a resume that
  reasoning does not hold: the target is declared by the file, and a crawl file is untrusted input
  by design — the product exists so that crawls can be copied and sent. A shared file claiming a
  local target could make the machine that opened it probe its own loopback. What it costs is that
  resuming a local audit leaves its local probes unrepeated and recorded, rather than checked; the
  way to audit it again is to re-run `crawl`, where the target comes from the command line.

### Fixed

- The registration of external URLs is capped. One page with 350,000 outbound links produced
  350,001 rows, an 87 MB file and 279 MB of RSS, with a 1,000-URL limit in force: external URLs do
  not count against that budget, and nothing else counted them either.
- More names screened by the first line: `.lan`, `home.arpa`, `.corp`, `fritz.box`, and the short
  names that resolve inside a cluster or a cloud instance.

## [0.5.0] — 2026-08-04

Everything here comes from a five-front review — security, performance, stability, usability and
rule correctness — of what shipped in 0.2.0 through 0.4.0. Most of it is the external link check
paying for its first day in the open.

### Security

- **The external probe now has a network perimeter.** Until now the only filter before requesting
  a URL from a third party was its scheme, so a site under audit could point the crawler at
  `169.254.169.254`, at the LAN, or at a service on loopback, and the resulting file — the one this
  product is designed to send to a client — carried the status, the response time and an error
  message precise enough to tell a refused connection from a timeout. That is a map of the
  consultant's internal network. Addresses that are not globally routable, and the names
  `localhost`, `*.local` and `*.internal`, are no longer probed.

  The screen is skipped when the audited site is itself local, and that is deliberate: auditing an
  `astro dev` on `localhost` or a client's staging on the office LAN means whoever started the
  crawl is already inside that network. The cloud metadata range is the exception and is screened
  either way, because a crawl of `localhost` from a cloud CI runner is a real thing and that
  address answers with the instance's credentials.

  It is a lexical screen: a name that resolves to a private address still goes through. Closing
  that needs the crawler to own its resolver and check the address it actually dialled, redirects
  included. The code says so where it matters, rather than implying more protection than it gives.

- **`--exclude` and `rel="nofollow"` now stop the probe.** The external branch returned before
  either was evaluated, so neither could prevent a request. The case that matters: spam links in
  a WordPress comment thread carry `rel="ugc nofollow"`, which is precisely the web's way of
  saying do not follow this — and those were the links getting a request from the user's own IP.

- Third-party strings (`mime`, `content_type`, `error_message`) are clipped before being stored.

### Fixed

- **Broken external links no longer include anti-bot walls.** A `HEAD` from a datacenter IP with a
  bot user agent is what Cloudflare, Akamai and DataDome answer with 401, 403 or 429 — measured
  against real hosts, all of which open fine in a browser. Only 404 and 410 now assert that a link
  is broken. A 400 does not either: to a bodyless probe it usually judges the request, not the
  resource. It is the same reasoning that already excluded foreign 5xx, and the code now carries
  it so nobody undoes it in a year.
- **A dead domain is reported.** A probe that fails DNS resolution is not missing data — the domain
  is gone, which is the most common form of link rot and, since expired domains get re-bought, a
  security warning too. Timeouts and connection failures still stay silent, because those are
  missing data.
- **A broken resource no longer produces two findings that contradict each other.** The 404 rules
  did not filter by element, so a missing stylesheet came out as both a critical broken link and a
  high broken asset — and "leaves the visitor on an error page" is false for a stylesheet nobody
  navigates to. They now cover `<a>` only. Broken `<iframe>` and `<form>` targets are left with no
  rule at all for now, which is honest rather than wrong.
- **`--ignore-robots` no longer silences the rules about what robots blocks.** The flag exists to
  see what Google cannot, and it was making two rules report nothing at all, because they looked
  for exclusion rows that the flag prevents from existing.
- **One slow host no longer starves the rest of the crawl.** The dispatcher parked URLs it could
  not send in a buffer, and once that buffer filled it stopped looking at the queue entirely —
  where other hosts were waiting with capacity to spare. Reproduced: 250 URLs of a host with
  `Crawl-delay` alongside 5 of a free host left the free host at zero after twenty seconds. The
  buffer is gone; the frontier now serves the first URL, in BFS order, of a host that has room.
- **A dead external host no longer adds hours of serial waiting.** Three consecutive network
  failures, or a 429, close that host; the probes it still had queued are counted as unchecked
  rather than attempted one ten-second timeout at a time.
- **Probes interrupted by a cut are recovered.** They were left with no status, never re-queued on
  resume, and never re-discovered, so the rule stayed silent about them forever with no counter
  saying so.
- **`resume` validates the rows it re-reads.** It checked the stored configuration but then loaded
  pending URLs without checking host or scheme, so a hand-edited file could point a resumed crawl
  anywhere.
- **`robots.txt` is fetched once per host.** The cache was check-then-fetch, so the first wave of a
  host downloaded it once per concurrent task — measured at 4.9x on a portfolio crawl, and exactly
  the burst that the `Crawl-delay` gate exists to prevent.
- The writer thread no longer outlives a handle dropped without closing it.

### Changed

- **The report tells you what it could not see.** The warnings about a truncated crawl, a list-mode
  crawl, external checks turned off or a cap reached existed only in the output of `crawl`, so a
  file read the next day — or by the colleague it was sent to — lied by omission. `report` now
  opens with them, and names the rules that went unevaluated.
- External URLs are counted separately in the status-code summary, so the totals reconcile.
- `inspect` no longer describes a probed external URL as excluded from the crawl.
- `list` accepts `--config`, `--include`, `--exclude`, `--no-external-check` and `--max-external`.
- `report --rule` on an HTTP rule ends with the command that shows which pages link to each URL,
  because that is where a broken link is fixed.
- An invalid seed is rejected before any file is created.
- URL suggestions on a typo now use edit distance, so a misspelling inside the last path segment
  suggests something instead of nothing.

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
