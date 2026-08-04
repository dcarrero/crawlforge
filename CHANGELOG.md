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
