# 03 — Crawl engine

> Versión en español: [`es/03-MOTOR-CRAWL.md`](es/03-MOTOR-CRAWL.md)

## 1. The three modes

| Mode | Source | Use |
|---|---|---|
| `http` | Crawl from a seed URL | Normal mode |
| `filesystem` | Local directory (`dist/`, `public/`, `_site/`) | Pre-deploy audit. **Differentiator** |
| `list` | Pasted or imported list of URLs | Audit a specific set. In high demand, near-zero cost |

All three flow into the same parsing, rules and storage pipeline. Only the source of bytes changes:

```rust
trait Fetcher: Send + Sync {
    async fn fetch(&self, target: &Target) -> Result<FetchedDoc>;
}
// HttpFetcher · FilesystemFetcher · WebviewFetcher (JS rendering, Pro)
```

**The `filesystem` mode is what Screaming Frog cannot do.** Paths are resolved the way the server
would (`/about` → `about/index.html` or `about.html`), links are rewritten against `--base`, and
thousands of pages are crawled in seconds with no network.

**The `list` mode audits exactly the requested set, and the file knows it.** Each page's links are
a property of that page and all of them are recorded —the target gets its row in `urls` with
`crawl_state='skipped'`—, but **nothing outside the list is downloaded**: not link targets, not
redirect targets, not the URLs the sitemaps declare (those are recorded with `in_sitemap=1`; the
cross-check is information, not an expansion of the crawl). External URLs are checked the same way
as in `http` mode (§9). And since the pages linking to a given page are never downloaded, the link
graph is incomplete **by definition**: every list-mode crawl sets `crawl_meta.truncated=1` with
`truncated_reason='list_mode'`, which turns off the `REQUIERE_GRAFO_COMPLETO` rules and keeps
`diff` from asserting absences over it. It is not a cutoff —the crawl did exactly what it was asked
to do— so the CLI reports it with its own wording, never with the "truncated crawl" warning.

## 2. Lifecycle

```
seeds (URL | sitemap | directory | list)
   → normalize
   → seen already? (in-memory url_hash index)
   → allowed? (robots, include/exclude patterns, depth, tier limit)
   → enqueue in frontier (priority by depth)
   → fetch (with a free slot in the host's concurrency limit)
   → parse (lol_html, streaming)
   → extract links → normalize again
   → evaluate PageRules
   → send batch to the writer thread
   → [queue drained] final pass: SiteRules + aggregate metrics + FTS + VACUUM
```

## 3. URL normalization

The most common and most expensive mistake: crawling the same page fifty times with different
querystrings. Rules, in this order:

1. Lowercase scheme and host. **Never the path** (it may be case-sensitive).
2. Remove the default port (`:80` on http, `:443` on https).
3. Resolve `.` and `..`.
4. Decode unnecessary percent-encoding; re-encode consistently.
5. Remove the fragment (`#...`) unless it starts with `#!` (legacy hashbang).
6. **Sort query parameters alphabetically.**
7. Remove parameters on the configurable strip list. By default: `utm_*`, `gclid`, `fbclid`,
   `msclkid`, `mc_cid`, `mc_eid`, `_ga`, `ref`, `si`.
8. Normalize the trailing slash according to what the server answers on the host's first
   resolution, not according to an assumption.
9. IDN to Punycode.

Always store **both**: the original URL as it appears in the HTML (for reports) and the normalized
one (for deduplication).

## 4. robots.txt and sitemaps

- One `robots.txt` per host, cached for the whole crawl.
- `Disallow` is respected for the configured user-agent, with a fallback to `*`.
- `Crawl-delay` is respected and **overrides** the user's concurrency setting for that host.
- Blocked URLs are recorded with `crawl_state='excluded'`, `exclusion_reason='robots'`. **They are
  not hidden**: knowing what is blocked is a finding in itself.
- An "ignore robots.txt" mode is available only after explicit confirmation and with a warning
  that it must only be used on sites you own.
- Sitemaps: discover via `robots.txt` (`Sitemap:`), via `/sitemap.xml` and via
  `/sitemap_index.xml`. Support nested indexes, `.gz`, and image and news sitemaps. Mark
  `in_sitemap = 1`. The sitemap ↔ links cross-check is what produces the orphan findings.

## 5. Parsing with `lol_html`

`lol_html` processes in streaming through per-selector handlers, without building the DOM. It is
the project's performance reason: 5-10x faster than `scraper` on large pages.

Handlers needed in a single pass:

```
title, meta[name=description], meta[name=robots], meta[name=viewport]
link[rel=canonical], link[rel=alternate][hreflang], link[rel=amphtml]
html[lang]
h1..h6
a[href], img[src], img[srcset], script[src], link[rel=stylesheet], iframe[src]
meta[property^=og:], meta[name^=twitter:]
script[type="application/ld+json"]
nav, main, footer, aside          → to infer the links' `region`
```

**Watch the state:** order of appearance matters (first `h1`, heading hierarchy, link position).
Keep a mutable `struct PageAccumulator` across the pass.

Body text: accumulate only if the tier allows it (`word_count` always, full text for FTS only on
Pro). Exclude the content of `<script>`, `<style>`, `<nav>`, `<footer>` from the word count.

## 6. Indexability

The central rule of the whole product. A page is indexable if **all** of these hold:

1. Status code 200.
2. `Content-Type` is HTML.
3. No `noindex` in `meta robots` or in the `X-Robots-Tag` header.
4. Not blocked by `robots.txt`.
5. The canonical points to itself or is absent.
6. It is not the origin of a redirect.

The reason is always stored in `indexability_reason`. The question "why is this page not on
Google?" is answered with that column, and it is the most frequent query an SEO makes.

## 7. Retries and resilience

```
connection timeout: 10 s
total timeout per request: 30 s
retries: 3, exponential backoff with jitter (1s, 2s, 4s ±50%)
retry on: 429, 500, 502, 503, 504, timeout, connection error
do not retry on: 4xx except 429, TLS error, DNS error
response size limit: 10 MB (configurable). Exceeding it → error_kind='toolarge'
```

After three consecutive overload responses from the same host —**429 or 503**— automatically halve
its concurrency and warn in the UI. A crawler that takes down the client's server is a useless
crawler. The 503 counts because a saturated Varnish or Cloudflare answers 503 rather than 429 and
the effect on the server is the same: telling them apart would be faithful to the letter of this
document and unfaithful to its reason. Recovery is deliberately slower than the braking — halved
at once, raised one point at a time after a run of good responses.

An interrupted crawl (app closed, network drop) must be resumable: the pending queue lives in
`urls` with `crawl_state='pending'`, so resuming is re-reading that table.

Done in `engine::resume` (CLI: `crawlforge resume <file>`). Settled semantics:

- **The configuration that rules is the original crawl's** (`crawl_meta.config_json`), and no new
  flags are accepted: resuming has to give the same result as never having stopped. With a
  different configuration you crawl again, you do not resume.
- The `pending` URLs return to the frontier with their stored `depth`: the BFS order survives the
  interruption.
- A cooperative stop (Ctrl+C in the CLI, `CancelSignal` in the engine) drains the writer thread
  and leaves `status='paused'`; an abrupt one (kill, crash) leaves `status='running'`. Both
  resume. The final pass does not run on interruption: whoever finishes runs it.
- **Not resumable**: a finished crawl (`status='done'`), a file from another schema version, or
  one whose stored configuration cannot be read.

## 8. JavaScript rendering (Pro, planned)

Two implementations behind the same interface:

| Build | Engine | Fidelity |
|---|---|---|
| Store | WKWebView (macOS) / **WebView2 = Chromium** (Windows) | Windows: high. macOS: WebKit, enough for 95% of cases |
| Direct (Agency) | `chromiumoxide` → CDP against the installed Chrome | High, with fine-grained request interception |

Common rules:
- Rendering is **opt-in per project**, never the default. It is 20-50x slower.
- Render concurrency capped at 2-4 instances, regardless of HTTP concurrency.
- Always compare raw HTML vs. rendered HTML and record the difference: links that only exist after
  hydration, injected content, canonical modified by JS. **That comparison is a finding in
  itself** and one of the things people pay for.
- Wait condition: `networkidle` with a 15 s ceiling.

## 9. Crawl budget and limits

```rust
struct CrawlLimits {
    max_urls: Option<u64>,        // Free: 1_000 (enforced by EntitlementSource)
    max_depth: Option<u32>,
    max_duration: Option<Duration>,
    max_size_per_url: u64,
    include_patterns: Vec<String>,   // compiled in `pattern.rs`, not stored compiled
    exclude_patterns: Vec<String>,
    follow_external: bool,        // default: only check status, do not crawl
    check_external: bool,         // that status check; enabled by default
    max_external: u64,            // cap on externals checked; 10_000 by default
    respect_nofollow: bool,
    concurrency_per_host: u8,     // 1..=20, default 5
    user_agent: String,
    ignore_robots: bool,          // crawls what robots.txt forbids, and marks it
    http_basic_auth: Option<Credential>,  // #[serde(skip)]: never reaches config_json
}
```

**The Free tier limit is enforced in the core, not in the UI.** On reaching it, the crawl ends
cleanly with `status='done'`, sets `truncated=true` in `crawl_meta`, and **shows every finding
gathered up to that point**. Results are not hidden: scale is limited.

**The external check is status only.** One `HEAD` request per unique external URL —with a `GET`
fallback if the server answers 405/501—, with a single in-flight request per foreign host and a
shorter timeout than the crawl's: nothing is parsed, no links are extracted, no `pages` row is
created; only the status of the `urls` row is filled in, which is what `HTTP-404-EXTERNAL` needs.
The foreign host's `robots.txt` is not requested: checking that a link resolves is what the browser
does when the visitor clicks it, and requesting it would nearly double the requests to third
parties in order to say less. Externals **do not count against `max_urls`**, and reaching
`max_external` **does not set `crawl_meta.truncated`** —that field turns off the
`REQUIERE_GRAFO_COMPLETO` rules—: it leaves externals unchecked and the summary says how many.

### 9.bis Include and exclude patterns (`pattern.rs`)

**Unanchored** regular expressions over the full normalized URL, as in Screaming Frog: a literal
string (`/wp-admin/`) works as a "contains" and the usual patterns (`\?replytocom=`,
`/page/\d+/`) work as-is. They are compiled **once** per crawl —an invalid pattern is an error
before starting, not a half-finished crawl— and the `regex` crate has no *backtracking*, so a
pathological pattern cannot degenerate.

Rules, applied in `engine.rs` where enqueueing is decided (links, redirect targets, sitemap URLs
and seeds):

- **`exclude` wins over `include`.** It is Screaming Frog's convention and the only one that
  allows "the whole blog except the drafts".
- **A non-empty `include` restricts**: only what matches one of its patterns is crawled.
- **What is excluded gets recorded**, with `crawl_state='excluded'` and
  `exclusion_reason='pattern'`: the summary shows how many URLs were left out by pattern, so that
  excluding half the site by mistake is visible on the first screen.
- **The seed of an HTTP crawl is always crawled** (like Screaming Frog's start URL): with
  `--include '/blog/'` and the seed at the root, filtering it would kill the crawl before
  discovering anything. In `filesystem` and `list` the seeds are a discovered or imported set and
  **are** filtered — it is the only place where `audit --exclude` can act.
- **Sitemap URLs follow the same rule** as links; the exclusion is recorded with `in_sitemap=1`.

## 10. User-Agent

By default, identify honestly and verifiably:

```
CrawlForge/1.0 (+https://[domain]/bot)
```

Allow spoofing Googlebot for diagnostics, with a warning that it must only be used on sites you
own. Never impersonate a browser by default: besides being bad practice, it is exactly the kind of
behavior that gets an App Store review rejected.
