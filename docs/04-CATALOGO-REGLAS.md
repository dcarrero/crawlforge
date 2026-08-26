# 04 — Rule catalog

> Versión en español: [`es/04-CATALOGO-REGLAS.md`](es/04-CATALOGO-REGLAS.md)

The rules **are** the product. The engine is infrastructure; this is what the user pays for.

## 1. Conventions

**ID:** `CATEGORY-SUBJECT-CONDITION`, in English, uppercase, stable forever.
A published ID never changes meaning: if the logic changes substantially, a new ID is created and
the old one is deprecated. Historical diffs depend on that stability.

**Severities:**

| Level | Meaning |
|---|---|
| `critical` | Blocks indexing or breaks the site |
| `high` | Clear, measurable damage to rankings |
| `medium` | Good practice not followed, moderate impact |
| `low` | Minor improvement |
| `info` | Informational, not a problem |

**Scope:** `page` (evaluated in streaming during the crawl) or `site` (needs the complete crawl,
evaluated in the final SQL pass).

**Tier:** rule available in `free`, `pro` or `agency`.

**Every rule requires:** an HTML fixture in `crates/crawlforge-rules/fixtures/` and a test. No
exceptions.

---

## 2. Indexability and crawling (`INDEX`)

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `INDEX-NOINDEX` | medium | page | free | `meta robots` or `X-Robots-Tag` contains `noindex`. **`critical` on the home page only** |
| `INDEX-ROBOTS-BLOCKED` | critical | site | free | URL blocked by robots.txt but linked internally |
| `INDEX-BLOCKED-IN-SITEMAP` | critical | site | free | URL in the sitemap but blocked by robots.txt |
| `INDEX-NOINDEX-IN-SITEMAP` | critical | site | free | URL in the sitemap with `noindex` |
| `INDEX-ROBOTS-TXT-MISSING` | medium | site | free | `/robots.txt` does not exist (404) |
| `INDEX-ROBOTS-TXT-BLOCKS-ALL` | critical | site | free | `Disallow: /` for `*` |
| `INDEX-NOFOLLOW-INTERNAL` | medium | page | free | Internal link with `rel=nofollow` |
| `INDEX-SITEMAP-MISSING` | high | site | free | No sitemap found. **`http` mode only**: in a `dist/` audit the site has not been published yet, and flagging it on every build would be noise in the CI pipeline |
| `INDEX-SITEMAP-ERROR` | high | site | free | Sitemap with invalid XML or >50,000 URLs / >50 MB |
| `INDEX-ORPHAN-PAGE` | high | site | free | In the sitemap or in the adapter, but with no incoming internal link at all |
| `INDEX-DEEP-PAGE` | medium | site | free | Click depth > 4, **counting only what is reachable** |
| `INDEX-SECTION-DISCONNECTED` | high | site | free | Set of pages with incoming links that cannot be reached from the home page or from a language root. **One finding per section, not per page** |
| `INDEX-NO-INTERNAL-LINKS-IN` | high | site | free | Indexable page with 0 incoming internal links |

**`INDEX-NOINDEX` dropped from `critical` to `medium` on 2026-08-01.** "This page carries noindex"
is a directive, not a defect: on a real WordPress site that was 848 pages —55% of the site— and
every one was a `/tag/`, a pagination page or an `/author/` archive the SEO plugin excludes on
purpose. A `critical` that is right 100% of the time and contributes nothing is worse than not
existing: it teaches the user to skip the severity column. The cases where it *is* an emergency are
detected **through structural signals, not pattern lists**: the contradiction with the sitemap is
already `INDEX-NOINDEX-IN-SITEMAP` (`critical`), and `noindex` on the home page —the classic
accident of a deploy from staging— is escalated here.

**`INDEX-SECTION-DISCONNECTED` was split out of `INDEX-DEEP-PAGE` on 2026-08-01.** Unreachable is
not deep. On a bilingual Astro site, the 1,987 `/en/*` pages came out as "too many clicks from the
home page" when the real problem was something else: there is not a single `<a>` from `/es` to
`/en` —the only bridge is `<link rel="alternate" hreflang>` and the language switcher is
JavaScript— so the traversal never got there. The BFS is now also seeded with the home page's
`hreflang` targets, `DEEP-PAGE` only measures what it reaches, and anything unreachable that has
incoming links collapses into **one** site-level finding with its count and its examples.
Measured: 2,138 → 236 true findings, plus one new and real finding (66 player pages genuinely
disconnected).

**`INDEX-DEEP-PAGE` stores the real depth and the report says it once (2026-08-03).** On the full
crawl of a news site with fifteen years of archive (216,349 pages), the rule produced 202,392
findings — **all of them true** (the archive has no pagination shortcuts) — and a report that opens
with that number does not get read, exactly as when they were false positives. No `group_key` can
help here: every page is genuinely distinct. The fix has two parts. The rule now computes depth
with an iterative BFS (same measured cost as the previous two CTEs, same result)
and writes `{"click_depth":N,"max_click_depth":4}` per page; and the report, when a rule
affects 40% or more of the pages (`crawlforge_rules::is_pervasive`, threshold measured over six
crawls), keeps the count and adds the site share — and for this rule, the shape too:
`202,392 pages deeper than 4 clicks — 94% of the site (typical depth 6–9, deepest 48)`. Nothing is
lost: every row is still in `issues`, the export carries them all, and `report --rule
INDEX-DEEP-PAGE` lists them sorted by depth, deepest first.

**These three are not evaluated on a truncated crawl (2026-07-30, extended 2026-08-01).** Their
answer depends on the link graph being complete, and a cut-off crawl —by the free-tier cap, by
`--max-urls` or by time— leaves everything still pending with no outgoing links. Measured: on a
40-URL crawl of a real blog, `INDEX-DEEP-PAGE` flagged 39 of 40 pages because the traversal could
not reach any of them. They live in `crawlforge_rules::REQUIERE_GRAFO_COMPLETO` and the engine
skips them; saying nothing beats saying something false.

`INDEX-ORPHAN-PAGE` joined that list on 2026-08-01, and so did `INDEX-SECTION-DISCONNECTED`: on
the truncated graph of a 176,000-URL news site it would have reported 202 "disconnected" sections
that were merely unvisited.

**`INDEX-ROBOTS-BLOCKED` moved from `page` to `site` on 2026-07-30.** The engine excludes a blocked
URL *before* downloading it —which is the correct behavior: honoring `robots.txt` means not
requesting it— so a `PageContext` to evaluate it on never exists. The data is in the store
(`crawl_state='excluded'` with `exclusion_reason='robots'`, plus its row in `links`), and that is
where it is read. The alternative, which is what Screaming Frog does, would be to download the
blocked URLs that are linked internally; that changes the crawler's behavior and was rejected: how
the engine crawls is not touched to make a rule's scope fit.

**Since 2026-08-04 the robots mark is read through both of its doors.** With `--ignore-robots`
everything gets crawled, so no row is ever `excluded` and the mark moves to
`pages.indexability_reason='robots'` on the downloaded page's own row. `INDEX-ROBOTS-BLOCKED` and
`INDEX-BLOCKED-IN-SITEMAP` accept either mark: before that, the flag meant to see what Google
cannot silenced both rules completely — a site linking a forbidden URL was a `critical` in a
normal crawl and zero findings with the flag on.

## 3. Status codes and redirects (`HTTP`)

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `HTTP-404-INTERNAL` | critical | site | free | Internal `<a>` link to a URL that returns 4xx |
| `HTTP-404-EXTERNAL` | medium | site | free | External `<a>` link whose URL is gone: 404, 410, or its domain does not resolve |
| `HTTP-5XX` | critical | page | free | 5xx response |
| `HTTP-REDIRECT-CHAIN` | high | site | free | Redirect chain of 2 or more hops |
| `HTTP-REDIRECT-LOOP` | critical | site | free | Redirect loop |
| `HTTP-TEMP-REDIRECT` | medium | page | free | 302/307 that is permanent in practice (appears in 2+ crawls) |
| `HTTP-REDIRECT-TO-404` | critical | site | free | Redirect that ends in a 404 |
| `HTTP-MIXED-CONTENT` | high | page | free | HTTPS page loading resources over HTTP |
| `HTTP-NO-HTTPS` | critical | site | free | Site responds over HTTP without redirecting to HTTPS |
| `HTTP-SLOW-RESPONSE` | medium | page | free | TTFB > 1,000 ms |
| `HTTP-LARGE-PAGE` | medium | page | free | HTML > 500 KB |
| `HTTP-NO-COMPRESSION` | medium | page | pro | No `Content-Encoding: gzip/br` on HTML |
| `HTTP-NO-CACHE-HEADERS` | low | page | pro | Static resources without `Cache-Control` |
| `HTTP-SOFT-404` | high | site | pro | Returns 200 but the content indicates an error (heuristic: few words + text pattern) |

**What a bot probe can assert about someone else's URL (2026-08-04):** the external check is a
`HEAD` with a bot user-agent, which is exactly what Cloudflare, Akamai and DataDome answer with
401, 403 or 429 while the page opens fine in a browser (verified against medium.com → 403,
wsj.com → 401, ft.com → 403 with the probe's own method and user-agent). So on external targets
only **404 and 410** — plus a domain that does not resolve (`error_kind='dns'`) — assert
"broken"; 401, 403, 407, 429, 451 and request-judging codes like 400/405 stay out, for the same
reason someone else's 5xx was already excluded. The criterion is shared
(`crawlforge_rules::sql_external_gone`) by `HTTP-404-EXTERNAL`, `ASSET-IMG-BROKEN`,
`ASSET-BROKEN`, `CANON-TO-4XX` and `HREFLANG-TO-4XX`; internal URLs, crawled with real requests
against a host the user controls, keep their full error ranges.

**Both 404 rules read `<a>` links only (2026-08-04):** the parser also writes `<img>`, `<script>`,
`<link rel=stylesheet>`, `<iframe>` and `<form>` into `links`, and without the `element = 'a'`
filter a broken stylesheet came out both as `ASSET-BROKEN` (high) and as `HTTP-404-INTERNAL`
(critical) — the same file with two severities, and the critical description ("leaves the visitor
on an error page") is false for a resource nobody navigates to. Broken resources belong to the
`ASSET` rules; a broken `<iframe>`/`<form>` target currently has no rule, which is a known,
smaller gap.

## 4. Titles and meta descriptions (`META`)

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `META-TITLE-MISSING` | critical | page | free | No `<title>`, or empty |
| `META-TITLE-DUPLICATE` | high | site | free | Same title on 2+ indexable pages |
| `META-TITLE-TOO-LONG` | medium | page | free | Estimated width > 580 px |
| `META-TITLE-TOO-SHORT` | low | page | free | < 30 characters |
| `META-TITLE-MULTIPLE` | medium | page | free | More than one `<title>` tag |
| `META-DESC-MISSING` | high | page | free | No meta description |
| `META-DESC-DUPLICATE` | medium | site | free | Repeated on 2+ indexable pages |
| `META-DESC-TOO-LONG` | low | page | free | Estimated width > 990 px |
| `META-DESC-TOO-SHORT` | low | page | free | < 70 characters |
| `META-VIEWPORT-MISSING` | high | page | free | No `meta viewport` |
| `META-REFRESH` | high | page | free | Uses `meta http-equiv=refresh` |

**Implementation note:** the pixel width is computed with Arial metrics at 20px (titles) and 14px
(descriptions), which is how Google truncates. It is a far more useful warning than counting
characters, and it matters even more in Spanish because the words are longer.

## 5. Canonical and duplicate content (`CANON`)

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `CANON-MISSING` | medium | page | free | Indexable page without a canonical |
| `CANON-MULTIPLE` | high | page | free | More than one `link rel=canonical` |
| `CANON-RELATIVE` | medium | page | free | Canonical is a relative URL |
| `CANON-TO-4XX` | critical | site | free | Canonical points to a URL with an error (foreign targets: 404/410 only, see §3 note) |
| `CANON-TO-REDIRECT` | high | site | free | Canonical points to a redirect |
| `CANON-TO-NOINDEX` | critical | site | free | Canonical points to a page with `noindex` |
| `CANON-CHAIN` | high | site | free | A canonicalizes to B, and B canonicalizes to C |
| `CANON-CROSS-DOMAIN` | medium | page | free | Canonical to another domain |
| `DUP-CONTENT-EXACT` | high | site | free | Identical HTML hash across 2+ URLs |
| `DUP-CONTENT-NEAR` | medium | site | pro | Simhash with similarity > 90% |
| `DUP-H1` | low | site | pro | Same H1 on 2+ pages |

## 6. Headings and content (`CONTENT`)

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `CONTENT-H1-MISSING` | high | page | free | No H1 |
| `CONTENT-H1-MULTIPLE` | low | page | free | More than one H1 |
| `CONTENT-H1-EMPTY` | medium | page | free | H1 empty, or containing only an image without alt |
| `CONTENT-HEADING-SKIP` | low | page | free | Level skip (H2 → H4) |
| `CONTENT-THIN` | high | page | free | Indexable page with < 300 words |
| `CONTENT-LOW-RATIO` | medium | page | pro | Text-to-HTML ratio < 10% |
| `CONTENT-LANG-MISSING` | medium | page | free | No `lang` attribute on `<html>` |
| `CONTENT-LANG-MISMATCH` | medium | page | pro | Declared `lang` does not match the detected language |

## 7. Images and assets (`ASSET`)

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `ASSET-IMG-NO-ALT` | high | page | free | `<img>` without an `alt` attribute |
| `ASSET-IMG-EMPTY-ALT-LINK` | high | page | free | Image with `alt=""` inside a link with no other text |
| `ASSET-IMG-BROKEN` | high | site | free | Image that returns 4xx/5xx (own host) or 404/410 (foreign host, see §3 note) |
| `ASSET-IMG-HEAVY` | medium | site | free | Image > 200 KB |
| `ASSET-IMG-LEGACY-FORMAT` | low | page | pro | JPEG/PNG without a WebP/AVIF alternative |
| `ASSET-IMG-NO-DIMENSIONS` | medium | page | pro | No `width`/`height` (causes CLS) |
| `ASSET-BROKEN` | high | site | free | CSS or JS that returns 4xx/5xx (own host) or 404/410 (foreign host, see §3 note) |
| `ASSET-JS-HEAVY` | medium | site | free | Own script > 250 KB as delivered |
| `ASSET-CSS-HEAVY` | medium | site | free | Own stylesheet > 100 KB as delivered |
| `ASSET-IFRAME-BROKEN` | high | site | free | `<iframe src>` pointing at an own URL that returns 4xx/5xx |
| `ASSET-FORM-BROKEN` | critical | site | free | `<form action>` (GET only) pointing at an own URL that returns 4xx/5xx |

**Added in 0.10.0.** The two weight rules read `resources`, the table the crawler has been filling
since 0.8.0 and that nothing could judge until now: the sheet showed the number and no rule said
whether it was too much. They only look at what the site serves itself — a heavy script on someone
else's CDN is not something the owner can split, and its size arrives from a `HEAD` probe rather
than a downloaded body. The stylesheet bar is lower than the script one on purpose: CSS is
render-blocking, so the same bytes cost more.

`ASSET-FORM-BROKEN` covers **only forms submitted with GET** — a search box, a catalogue filter.
Checking a POST would mean submitting the form, which this tool does not do, so the parser does not
even record those destinations: a row that cannot be judged is worse than no row. The severity is
`critical` and it is the only `ASSET` rule that is: a form that loses what was typed costs
customers outright, and nobody reports it, because whoever hits it leaves without saying so.

**Scope correction (2026-07-30):** `ASSET-IMG-HEAVY` was listed as `page` and it is `site`. An
image's weight is not in the HTML —`width` and `height` declare layout, not bytes— so it cannot be
decided with only the page at hand: it needs the `urls` row of the already-downloaded resource. The
data is `urls.content_length`, and it stays there: since 2026-08-04 the writer does populate
`resources`, but with one row per resource URL rather than per (page, resource) pair, so the
«which pages use this image» half of the question is still answered by `images`, not by it.

## 8. Internationalization (`HREFLANG`)

High-value block for the client (ejemplo.es/ejemplo.me, another project, another multilingual
project).

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `HREFLANG-NO-SELF` | high | page | free | hreflang set without a reference to itself |
| `HREFLANG-NOT-RECIPROCAL` | high | site | free | A points to B, B does not point to A |
| `HREFLANG-INVALID-CODE` | high | page | free | Language or region code invalid per ISO 639-1 / 3166-1 |
| `HREFLANG-TO-4XX` | critical | site | free | hreflang points to a URL with a 4xx (foreign targets: 404/410 only, see §3 note) |
| `HREFLANG-TO-NOINDEX` | critical | site | pro | hreflang points to a non-indexable page |
| `HREFLANG-CONFLICT-CANONICAL` | high | site | pro | hreflang and canonical contradict each other |
| `HREFLANG-NO-XDEFAULT` | low | site | pro | Multilingual set without `x-default` |

## 9. Structured data and social (`SCHEMA`)

| ID | Sev. | Scope | Tier | Condition |
|---|---|---|---|---|
| `SCHEMA-INVALID-JSON` | high | page | pro | Malformed JSON-LD |
| `SCHEMA-MISSING-REQUIRED` | medium | page | pro | A required property of the declared type is missing |
| `SCHEMA-MISSING-ARTICLE` | low | page | pro | Article-like page without an `Article`/`BlogPosting` schema |
| `SOCIAL-OG-MISSING` | low | page | free | No `og:title` / `og:description` / `og:image` |
| `SOCIAL-OG-IMAGE-BROKEN` | medium | site | pro | `og:image` returns an error |

## 10. WordPress (`WP`) — requires the adapter, Pro tier

See `05-ADAPTADORES.md`.

| ID | Sev. | Condition |
|---|---|---|
| `WP-ORPHAN-POST` | high | Post published in the REST API that the crawl never reached |
| `WP-ATTACHMENT-INDEXABLE` | high | Indexable attachment pages (the classic junk-content generator) |
| `WP-THIN-ARCHIVE` | medium | Tag or category archive with a single post |
| `WP-REPLYTOCOM` | medium | Crawlable `?replytocom` URLs |
| `WP-PAGINATION-TRAP` | high | `/page/N/` pagination that continues past the real total |
| `WP-SITEMAP-MISMATCH` | high | The Yoast/RankMath sitemap does not match the published content |
| `WP-MISSING-SEO-META` | medium | Post without a meta description in Yoast/RankMath |
| `WP-OUTDATED-PLUGIN` | info | Plugin version detected via `?ver=` that is out of date |
| `WP-XMLRPC-OPEN` | low | `/xmlrpc.php` reachable |
| `WP-FEED-DUPLICATE` | low | Indexable feeds duplicating content |

## 11. Static sites / Astro (`STATIC`) — Pro tier

| ID | Sev. | Condition |
|---|---|---|
| `STATIC-ROUTE-NOT-IN-SITEMAP` | medium | Route generated in `dist/` that is absent from the sitemap |
| `STATIC-SITEMAP-ORPHAN` | high | URL in the sitemap with no corresponding file in `dist/` |
| `STATIC-COLLECTION-NO-ROUTE` | medium | Content-collection entry with no generated route |
| `STATIC-HYDRATION-ONLY-LINK` | high | Link that only exists after the island hydrates → invisible to the crawler |
| `STATIC-BROKEN-RELATIVE` | critical | Relative link that resolves to no file in `dist/` |
| `STATIC-ASSET-UNREFERENCED` | info | File in `dist/` that nothing points to |

## 12. Accessibility (`A11Y`) — planned, bridge to the European regulation

Reserved. It will be populated by injecting `axe-core` through the rendering webview. The IDs will
follow the `A11Y-<axe-rule>` pattern and every finding will cite its reference in **WCAG 2.1 AA +
EN 301 549 + EU Directive 2019/882**, with the manual-review disclaimer always visible, as defined
in the EAA compliance MVP plan.

**Not implemented yet.** But the `trait Rule` must already accept a normative-references field so
there is no refactor later:

```rust
fn references(&self) -> &[Reference];   // { standard, clause, url }
```

## 13. Split by tier — summary

| Tier | Rules | Criterion |
|---|---|---|
| Free | ~50 | All the fundamental technical SEO. **No finding is hidden within the 1,000-URL limit** |
| Pro | +25 | Near-duplicates, schema, soft 404s, WordPress, static sites, advanced hreflang, JS rendering |
| Agency | +A11Y and custom rules | The user's own custom rule engine |

Remember the principle: **Free limits scale, not knowledge.** A free user
with a 400-page blog must get a complete audit and walk away impressed. That is the conversion
engine.

## 14. Custom rule engine (Agency, planned)

Defined by the user in YAML, evaluated over the store:

```yaml
- id: CUSTOM-PRICE-BLOCK-MISSING
  severity: high
  scope: page
  when:
    url_matches: "^/producto/"
    css_absent: ".precio"
  message: "Product page without its price block"
```

CSS selectors, regular expressions over the HTML, conditions on store columns, and direct SQL
queries for advanced cases. It is the answer to Screaming Frog's "custom extraction", taken one
step further: there you extract, here you also evaluate.
