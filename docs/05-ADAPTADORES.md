# 05 — Platform adapters

> Versión en español: [`es/05-ADAPTADORES.md`](es/05-ADAPTADORES.md)

## 1. Why they exist

A generic crawler sees what a browser sees. An adapter **crosses the crawl with the platform's
source of truth**, and that comparison produces findings impossible to obtain any other way:

> WordPress says there are 1,240 published posts. The crawl reached 1,187.
> **53 published posts are not linked from anywhere.**

No generalist tool will ever do this: it takes knowing the CMS from the inside.
It is the product's deepest moat.

## 2. The trait

```rust
#[async_trait]
pub trait SiteAdapter: Send + Sync {
    fn id(&self) -> &'static str;

    /// Runs before the crawl: contributes seeds and context.
    async fn discover(&self, ctx: &AdapterContext) -> Result<Discovery>;

    /// Enriches a page during the crawl (streaming, must be cheap).
    fn enrich_page(&self, page: &mut PageData, doc: &FetchedDoc);

    /// Runs when the crawl ends: crosses entities against what was crawled.
    async fn reconcile(&self, store: &Store) -> Result<Vec<Issue>>;

    fn rules(&self) -> Vec<Box<dyn Rule>>;
}

pub struct Discovery {
    pub seeds: Vec<Url>,
    pub entities: Vec<AdapterEntity>,   // → adapter_entities table
    pub hints: PlatformHints,           // version, SEO plugins, etc.
}
```

**Automatic detection:** before crawling, one request is made to the root and heuristics are
applied (`Link: <.../wp-json/>` header, `/wp-content/` in the assets, the `generator` meta,
`_astro/` present in the paths). If a platform is detected, enabling the adapter is offered. It is
never enabled without the user's confirmation.

---

## 3. WordPress adapter

### 3.1 Data sources, in order of preference

| Source | Requires | Provides |
|---|---|---|
| Public REST API (`/wp-json/wp/v2/`) | Nothing | Posts, pages, taxonomies, media |
| Authenticated REST API | Application Password | Drafts, private posts, SEO metadata |
| Yoast / RankMath sitemap | Nothing | The SEO plugin's view of what should be indexed |
| SSH + WP-CLI (`russh`) | Credentials | Direct `wp_postmeta`, plugins, options |

**Sandbox rule:** SSH **always** through `russh`, in-process. Never launch `/usr/bin/ssh`, not
even in the direct build, to keep a single code path. See `docs/CONVENTIONS.md §2`.

### 3.2 What `discover()` collects

```
GET /wp-json/wp/v2/posts?per_page=100&_fields=id,link,status,date,modified,title
GET /wp-json/wp/v2/pages?...
GET /wp-json/wp/v2/categories?per_page=100&_fields=id,link,count,name
GET /wp-json/wp/v2/tags?...
GET /wp-json/wp/v2/media?...           (to detect attachment pages)
GET /wp-json/                          (exposed plugins, version, namespaces)
```

Paginate with the `X-WP-TotalPages` header. Apply a rate limit of its own: the WordPress REST API
is far more fragile than the cached frontend. **At most 2 concurrent requests to `/wp-json/`.**

SEO plugin detection: the `yoast/v1` or `rankmath/v1` namespace exists, or the characteristic HTML
comments appear in the `<head>`.

Technical inventory without authentication: plugin and theme versions deduced from the `?ver=`
parameters of the enqueued assets. It is public information, and surprisingly useful.

### 3.3 Characteristic findings

The rules live in `04-CATALOGO-REGLAS.md §10`. The ones that deliver the most value in practice:

1. **Orphan posts** — published but with no incoming internal link. Obtained from
   `adapter_entities` where `url_id IS NULL`, or `url_id` present but with no rows in `links`.
2. **Indexable attachment pages** — WordPress creates a page for every uploaded image. If they are
   indexable, that is junk content at scale. Detection: `media` entities with a crawlable URL that
   returns 200 and HTML.
3. **Anemic archives** — categories and tags with `count <= 1`. On blogs with years of history
   they usually number in the hundreds.
4. **Sitemap ↔ content mismatch** — the Yoast sitemap says X, the REST API says Y.
   It almost always points to a misunderstood indexing configuration.
5. **Pagination traps** — `/page/N/` still returning 200 past the real number of
   pages. Generates infinite crawling and wastes crawl budget.

### 3.4 WordPress portfolio mode

Our own use case: 100+ blogs. A project can declare multiple WordPress sites with different
credentials, and the panel aggregates the findings per site and per rule. It is the intersection
of the adapter and the portfolio feature.

---

## 4. Astro / static sites adapter

### 4.1 The `filesystem` mode

The product's cleanest differentiator. Instead of crawling over HTTP, `dist/` is read
directly:

```bash
crawlforge audit ./dist --base https://ejemplo.com \
  --adapter astro \
  --compare-with ./ultimo-crawl-produccion.sqlite \
  --fail-on new-404,canonical-broken,orphan-page
```

**Route resolution.** We must emulate what the server will do. Attempt order for route `/x`:

```
dist/x/index.html   ← Astro with build.format = 'directory' (the default)
dist/x.html         ← build.format = 'file'
dist/x              ← literal file (assets)
dist/404.html       ← fallback
```

Read `dist/_routes.json` or the adapter configuration when present to refine this. Relative links
are resolved against the file's path, not against `--base`; `--base` is only used to rewrite the
absolute URLs in the report.

**Performance:** no network, no rate limit, no robots. Target > 2,000 URL/s. A 5,000-page
site is audited in three seconds, inside the CI pipeline.

### 4.2 Characteristic findings

- **Links that only exist after hydration.** Astro generates islands; a link inside a component
  with `client:only` is not in the static HTML and is therefore invisible to Google. Detected by
  comparing the `dist/` HTML with the rendered HTML (Pro, planned). It is one of the most
  expensive and least diagnosed mistakes on Astro sites.
- **Generated routes absent from the sitemap** — `@astrojs/sitemap` misconfigured, with an
  overly aggressive `filter` or `exclude`.
- **Collection entries with no route** — content in `src/content/` with no generated page.
  Requires reading `src/content/config.ts` or the build manifest.
- **Broken relative links** — detected with absolute certainty with the filesystem right
  there, with no false positives from server redirects.

### 4.3 CI integration

```yaml
# .github/workflows/seo.yml
- run: npm run build
- run: |
    crawlforge audit ./dist \
      --base ${{ vars.SITE_URL }} \
      --adapter astro \
      --baseline .crawlforge/baseline.sqlite \
      --fail-on new-404,canonical-broken,indexability-lost \
      --report-md $GITHUB_STEP_SUMMARY
```

The *baseline* is a SQLite file versioned in the repository or downloaded from the last production
crawl. The command exits with a non-zero code when regressions appear.

This is not just product: it is immediately useful infrastructure for ejemplo.me, another project
and yet another.

### 4.4 Generalization

The same adapter covers Hugo (`public/`), Eleventy (`_site/`), Next.js as static export (`out/`)
and Jekyll (`_site/`). Detection by the presence of marker files. Present it as
"static sites", with Astro as the best-supported case.

---

## 5. Future adapters

Not implemented, but the trait must accommodate them without a refactor:

| Adapter | Source | Interest |
|---|---|---|
| Laravel | Routes from `artisan route:list` | Fits a common PHP stack |
| Shopify | Admin API | High commercial value, e-commerce |
| EAA accessibility | `axe-core` via webview | A second product on the same engine |

The accessibility adapter does not quite fit `SiteAdapter` (it discovers no entities, it only
enriches and evaluates). When the time comes, weigh extracting a sibling `PageAnalyzer` trait
rather than forcing the abstraction. Do not do it before you need it.
