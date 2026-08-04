# 02 — Data model

> Versión en español: [`es/02-MODELO-DATOS.md`](es/02-MODELO-DATOS.md)

## 1. Principles

1. **One crawl = one SQLite file.** Portable, compressible, sendable to a client.
2. The core writes; the UI and the CLI read. See `01-ARQUITECTURA.md §2`.
3. URLs are stored once in `urls` and referenced by `INTEGER` everywhere else. With 500k URLs of
   80 characters, duplicating them in `links` would cost gigabytes.
4. Numbered, forward-only migrations. A year-old file must still open.

## 2. Connection configuration

There are **two** sets, and the difference between them is deliberate. Both live in `store.rs`
(`WRITER_PRAGMAS` and `READER_PRAGMAS`) so that Swift and C# do not have to work them out and end
up diverging per platform.

The writer:

```sql
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous  = NORMAL;   -- enough: a crawl can be repeated
PRAGMA foreign_keys = ON;
PRAGMA temp_store   = MEMORY;
PRAGMA cache_size   = -64000;   -- 64 MB
PRAGMA mmap_size    = 0;        -- see below
```

The interface's read-only connection:

```sql
PRAGMA query_only   = 1;
PRAGMA busy_timeout = 5000;
PRAGMA temp_store   = MEMORY;
PRAGMA cache_size   = -64000;
PRAGMA mmap_size    = 268435456;
```

**Memory mapping pays off for the reader and not for the writer.** The interface pages and sorts
the same table constantly and does nothing but read; the engine writes in bulk from a single
thread, where the mapping buys nothing and costs resident memory — the metric this product is
argued on. This document used to prescribe a single set with the 256 MB mapping for everything,
and following it would put mmap on the writer, which is exactly what the code avoids.

`busy_timeout` is not optional either: a reader on the same file is the architecture (`CONVENTIONS.md
§2.2`), not a corner case, and without the timeout any brush between locks was an immediate
`database is locked` even when the reader was about to let go.

When the crawl finishes: `PRAGMA optimize;` and the WAL checkpoint that takes the file out of WAL
mode, so that «one crawl = one portable file» survives copying the `.sqlite` on its own.

**`VACUUM` was measured and rejected**, and this document used to recommend it. On a crawl of
50,000 URLs and 6.15 million links it shrank the file by **2%** (380 MB → 372 MB) and pushed peak
memory from 168 MB to 246 MB. The 30-40% figure shows up when there is fragmentation from deletes
and updates; a crawl file is written once and incrementally, so there is almost none. Paying 78 MB
of peak for 2% of disk is a bad trade. Whoever wants it has it in `store::compact`.

## 3. Schema

### 3.1 Metadata

```sql
CREATE TABLE schema_version (
    version     INTEGER NOT NULL,
    applied_at  TEXT    NOT NULL
);

CREATE TABLE crawl_meta (
    id              TEXT PRIMARY KEY,        -- uuid v7 (time-sortable)
    project_id      TEXT NOT NULL,
    project_name    TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    mode            TEXT NOT NULL,           -- 'http' | 'filesystem' | 'list'
    source_path     TEXT,                    -- filesystem mode only
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    status          TEXT NOT NULL,           -- 'running'|'paused'|'done'|'cancelled'|'failed'
    config_json     TEXT NOT NULL,           -- the full serialized CrawlJob
    core_version    TEXT NOT NULL,
    rules_version   TEXT NOT NULL,
    adapter         TEXT,                    -- 'wordpress' | 'astro' | NULL
    tier_at_runtime TEXT NOT NULL,           -- rules applied according to tier
    truncated       INTEGER NOT NULL DEFAULT 0,  -- migration 002
    truncated_reason TEXT                    -- 'max_urls'|'max_depth'|'max_duration'|'list_mode'
);
```

`config_json` and `rules_version` are essential so that a diff between two crawls can tell whether
the difference comes from the site or from a configuration/rules change.

`truncated` says **the crawled set is not the whole site**, and it is load-bearing: the engine uses
it to silence the rules in `crawlforge_rules::REQUIERE_GRAFO_COMPLETO`, and `diff` uses it to refuse
to claim that a URL disappeared. `list_mode` is set on every crawl in list mode — not because
anything was cut short, but because a list only ever sees the URLs it was given.

### 3.2 URLs

```sql
CREATE TABLE urls (
    id                  INTEGER PRIMARY KEY,
    url                 TEXT    NOT NULL UNIQUE,
    url_hash            INTEGER NOT NULL,      -- xxh3 of the normalized URL
    scheme              TEXT    NOT NULL,
    host                TEXT    NOT NULL,
    path                TEXT    NOT NULL,
    query               TEXT,
    depth               INTEGER,               -- clicks from the root; NULL if not reached
    discovered_from     INTEGER REFERENCES urls(id),
    is_internal         INTEGER NOT NULL,      -- 0/1
    in_sitemap          INTEGER NOT NULL DEFAULT 0,
    crawl_state         TEXT    NOT NULL,      -- 'pending'|'done'|'error'|'excluded'|'skipped'
    exclusion_reason    TEXT,                  -- 'robots'|'nofollow'|'depth'|'pattern'|'limit'
    status_code         INTEGER,
    redirect_to         INTEGER REFERENCES urls(id),
    redirect_chain_len  INTEGER DEFAULT 0,
    content_type        TEXT,
    content_length      INTEGER,
    response_time_ms    INTEGER,
    fetched_at          TEXT,
    error_kind          TEXT,                  -- 'dns'|'tls'|'timeout'|'connection'|'toolarge'
    error_message       TEXT
);

CREATE INDEX idx_urls_hash        ON urls(url_hash);
CREATE INDEX idx_urls_state       ON urls(crawl_state);
CREATE INDEX idx_urls_status      ON urls(status_code) WHERE status_code IS NOT NULL;
CREATE INDEX idx_urls_host        ON urls(host);
CREATE INDEX idx_urls_depth       ON urls(depth);
CREATE INDEX idx_urls_internal    ON urls(is_internal, crawl_state);
```

**About `url_hash`:** comparing URLs is the most frequent operation of the crawl (have we visited
it already?). An indexed `INTEGER` is much faster than `TEXT UNIQUE`. Keep both: the hash for the
hot path, the unique text as an integrity guarantee.

### 3.3 HTML pages

```sql
CREATE TABLE pages (
    url_id              INTEGER PRIMARY KEY REFERENCES urls(id),
    title               TEXT,
    title_len           INTEGER,
    title_px            INTEGER,      -- estimated width in pixels (Google truncates by pixels)
    meta_description    TEXT,
    meta_desc_len       INTEGER,
    meta_desc_px        INTEGER,
    h1                  TEXT,         -- first h1
    h1_count            INTEGER,
    h2_count            INTEGER,
    heading_json        TEXT,         -- full hierarchy, to detect level skips
    canonical           TEXT,
    canonical_is_self   INTEGER,
    meta_robots         TEXT,
    x_robots_tag        TEXT,
    is_indexable        INTEGER NOT NULL,
    indexability_reason TEXT,         -- 'noindex'|'canonicalised'|'robots'|'redirect'|'4xx'|'5xx'
    lang                TEXT,
    hreflang_json       TEXT,
    word_count          INTEGER,
    text_hash           INTEGER,      -- simhash of the text, for near-duplicates
    html_hash           INTEGER,      -- exact xxh3
    content_ratio       REAL,         -- text / html
    viewport            TEXT,
    og_json             TEXT,
    twitter_json        TEXT,
    schema_types        TEXT,         -- CSV of @type values detected in JSON-LD
    amp_url             TEXT,
    internal_links_out  INTEGER,
    internal_links_in   INTEGER,      -- filled in during the final pass
    crawl_depth_source  TEXT          -- 'link'|'sitemap'|'list'|'adapter'
);

CREATE INDEX idx_pages_title_len  ON pages(title_len);
CREATE INDEX idx_pages_indexable  ON pages(is_indexable);
CREATE INDEX idx_pages_text_hash  ON pages(text_hash);
CREATE INDEX idx_pages_links_in   ON pages(internal_links_in);
```

**`title_px` and `meta_desc_px`:** Google truncates by pixel width, not by character count.
Computing it with the Arial 20px/14px metrics gives a much more useful warning than "over 60
characters".

### 3.4 Links

```sql
CREATE TABLE links (
    id           INTEGER PRIMARY KEY,
    from_url_id  INTEGER NOT NULL REFERENCES urls(id),
    to_url_id    INTEGER NOT NULL REFERENCES urls(id),
    anchor       TEXT,
    rel          TEXT,
    is_nofollow  INTEGER NOT NULL DEFAULT 0,
    element      TEXT NOT NULL,     -- 'a'|'link'|'img'|'script'|'iframe'|'form'
    region       TEXT,              -- 'nav'|'main'|'footer'|'aside'|'unknown'
    position     INTEGER            -- order of appearance in the document
);

CREATE INDEX idx_links_from ON links(from_url_id);
CREATE INDEX idx_links_to   ON links(to_url_id);
```

`region` is inferred from the nearest semantic ancestor (`<nav>`, `<main>`, `<footer>`). It lets
you tell template links from content links, which is the difference that matters in internal
linking.

### 3.5 Resources and images

```sql
CREATE TABLE resources (
    id           INTEGER PRIMARY KEY,
    url_id       INTEGER NOT NULL REFERENCES urls(id),
    kind         TEXT NOT NULL,    -- 'img'|'css'|'js'|'font'|'video'
    status_code  INTEGER,
    size_bytes   INTEGER,
    mime         TEXT
);

CREATE TABLE images (
    id          INTEGER PRIMARY KEY,
    page_url_id INTEGER NOT NULL REFERENCES urls(id),
    src_url_id  INTEGER NOT NULL REFERENCES urls(id),
    alt         TEXT,
    alt_present INTEGER NOT NULL,
    title       TEXT,
    width_attr  INTEGER,
    height_attr INTEGER,
    loading     TEXT,              -- 'lazy'|'eager'|NULL
    in_srcset   INTEGER NOT NULL DEFAULT 0,
    format      TEXT               -- 'jpeg'|'png'|'webp'|'avif'|'svg'|'gif'
);

CREATE INDEX idx_images_page ON images(page_url_id);
CREATE INDEX idx_images_alt  ON images(alt_present);
```

`resources` is **one row per resource URL**, not per (page, resource) pair — note that it has no
`page_url_id`, and that is on purpose: for CSS and JS the page↔resource edge carries much less
information than for images (a 900 KB `bundle.js` is loaded by the whole template, not by one
specific post), so the file already identifies the problem. For images that edge does exist and
lives in `images`. The `kind` is inferred from the response's `content_type`, with the URL's
extension as a fallback when the server does not send a useful one (`application/octet-stream` is
common for fonts). Uniqueness per `url_id` is guaranteed by the index in migration 008.

### 3.6 Findings — the table that is the product

```sql
CREATE TABLE issues (
    id          INTEGER PRIMARY KEY,
    url_id      INTEGER REFERENCES urls(id),   -- NULL = site-level finding
    rule_id     TEXT    NOT NULL,              -- 'SEO-TITLE-MISSING'
    severity    TEXT    NOT NULL,              -- 'critical'|'high'|'medium'|'low'|'info'
    category    TEXT    NOT NULL,
    detail_json TEXT,                          -- rule-specific context
    group_key   TEXT                           -- groups duplicates: hash of the repeated title, etc.
);

CREATE INDEX idx_issues_rule     ON issues(rule_id);
CREATE INDEX idx_issues_url      ON issues(url_id);
CREATE INDEX idx_issues_severity ON issues(severity);
CREATE INDEX idx_issues_group    ON issues(group_key) WHERE group_key IS NOT NULL;
```

### 3.7 Full-text search

```sql
CREATE VIRTUAL TABLE pages_fts USING fts5(
    url, title, meta_description, body_text,
    content = '',                -- contentless: we do not duplicate the text
    tokenize = 'unicode61 remove_diacritics 2'
);
```

`remove_diacritics 2` is mandatory for Spanish: it lets "diseño" match "diseno" and the other way
around. **Only populated at the Pro tier** (body text multiplies the file size).

### 3.8 Custom extraction (Pro)

```sql
CREATE TABLE extractions (
    id          INTEGER PRIMARY KEY,
    url_id      INTEGER NOT NULL REFERENCES urls(id),
    name        TEXT    NOT NULL,   -- name of the user-defined extractor
    value       TEXT,
    occurrence  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_extractions ON extractions(name, url_id);
```

### 3.9 Adapter data

```sql
CREATE TABLE adapter_entities (
    id           INTEGER PRIMARY KEY,
    adapter      TEXT NOT NULL,       -- 'wordpress'|'astro'
    entity_type  TEXT NOT NULL,       -- 'post'|'page'|'term'|'plugin'|'route'|'collection'
    external_id  TEXT,
    url_id       INTEGER REFERENCES urls(id),   -- NULL if it was not crawled → orphan
    data_json    TEXT NOT NULL
);
CREATE INDEX idx_adapter_entities ON adapter_entities(adapter, entity_type);
```

`url_id` being `NULL` is precisely the valuable finding: it exists in WordPress but was never
reached by crawling → **orphan content**.

### 3.10 `robots.txt` and sitemaps (migration 004)

What the crawl consulted, not what it discovered. Added on 2026-07-30: the engine downloaded both
files, used them and threw them away, so when the crawl ended there was no record of whether the
`robots.txt` existed or whether a sitemap had broken XML. That blocked three catalog rules, and one
of them —`INDEX-ROBOTS-TXT-BLOCKS-ALL`— warns about the most expensive and most silent accident
there is: the staging environment's `robots.txt`, with `Disallow: /`, pushed to production.

```sql
CREATE TABLE robots_txt (
    id            INTEGER PRIMARY KEY,
    host          TEXT    NOT NULL UNIQUE,
    status_code   INTEGER,          -- NULL: never requested or no response
    content       TEXT,             -- the file as-is, to explain the finding and for the diff
    blocks_all    INTEGER NOT NULL DEFAULT 0,   -- evaluated with the parser, not by text search
    sitemap_count INTEGER NOT NULL DEFAULT 0,
    fetched_at    TEXT
);

CREATE TABLE sitemaps (
    id           INTEGER PRIMARY KEY,
    url          TEXT    NOT NULL UNIQUE,
    status_code  INTEGER,
    is_index     INTEGER NOT NULL DEFAULT 0,
    is_valid     INTEGER NOT NULL DEFAULT 1,
    parse_error  TEXT,
    url_count    INTEGER NOT NULL DEFAULT 0,
    bytes        INTEGER NOT NULL DEFAULT 0,
    discovered_from TEXT NOT NULL,   -- 'robots' | 'well_known' | 'index'
    fetched_at   TEXT
);
CREATE INDEX idx_sitemaps_valid ON sitemaps(is_valid);
```

Two details that are not obvious:

- **`blocks_all` is evaluated, not searched for.** A `Disallow: /` may sit under another
  `User-agent` and not apply to us; taking it at face value would be the most expensive false
  positive in the catalog, telling someone their site is blocked when it is not.
- **`discovered_from` matters when deciding whether something is an error.** `/sitemap_index.xml`
  returning 404 is normal: it is probed blindly. One announced in `robots.txt` failing is not,
  because someone declared it.

They also serve comparison: with these rows, a diff between two crawls can say "the robots.txt
changed" and "the sitemap declares 4,000 fewer URLs than last week".

## 4. Views for the UI

Define the aggregations as views so Swift and C# do not duplicate SQL.

```sql
CREATE VIEW v_issue_summary AS
SELECT rule_id, severity, category, COUNT(*) AS n
FROM issues GROUP BY rule_id, severity, category;

CREATE VIEW v_indexable_pages AS
SELECT u.id, u.url, u.depth, p.title, p.title_len, p.meta_description,
       p.word_count, p.internal_links_in
FROM urls u JOIN pages p ON p.url_id = u.id
WHERE p.is_indexable = 1;

CREATE VIEW v_broken_links AS
SELECT l.from_url_id, uf.url AS from_url, ut.url AS to_url, ut.status_code, l.anchor
FROM links l
JOIN urls uf ON uf.id = l.from_url_id
JOIN urls ut ON ut.id = l.to_url_id
WHERE ut.status_code >= 400;

-- Current definition: migrations 003 and 005. The one here is the original, and it was missing
-- two conditions; it is kept so you can read what each one cost.
--
--   003 — the front page always meets all three conditions (it is in the sitemap and nobody
--         links to it, because it is the entry point), so it showed up as an orphan in every
--         crawl.
--   005 — **the `JOIN pages`**. Without it, "orphan" did not require being a page: the images in
--         WordPress's image sitemap are internal, are declared, and are used with `<img src>`,
--         which goes to `images` and not to `links`. On a news site that was 1,867 out of 1,912
--         findings. Requiring the `pages` row also removes the URLs the sitemap declares but the
--         crawl never got to visit.
CREATE VIEW v_orphans AS
SELECT u.id, u.url FROM urls u
LEFT JOIN links l ON l.to_url_id = u.id
WHERE u.is_internal = 1 AND u.in_sitemap = 1 AND l.id IS NULL;
```

## 5. Diffs between crawls

No proprietary format: it is done with `ATTACH`.

```sql
ATTACH DATABASE 'crawl_2026-07-01.sqlite' AS a;
ATTACH DATABASE 'crawl_2026-07-08.sqlite' AS b;

-- URLs that are new
SELECT b.url FROM b.urls b LEFT JOIN a.urls a ON a.url_hash = b.url_hash
WHERE a.id IS NULL AND b.is_internal = 1;

-- Status code changes
SELECT b.url, a.status_code AS antes, b.status_code AS ahora
FROM b.urls b JOIN a.urls a ON a.url_hash = b.url_hash
WHERE a.status_code IS NOT b.status_code;

-- Pages that lost indexability
SELECT u.url, pa.indexability_reason AS antes, pb.indexability_reason AS ahora
FROM b.pages pb
JOIN b.urls u   ON u.id = pb.url_id
JOIN a.urls ua  ON ua.url_hash = u.url_hash
JOIN a.pages pa ON pa.url_id = ua.id
WHERE pa.is_indexable = 1 AND pb.is_indexable = 0;
```

`crawl_diff()` generates a third SQLite file with a single `changes` table:

```sql
CREATE TABLE changes (
    id          INTEGER PRIMARY KEY,
    change_type TEXT NOT NULL,   -- 'url_added'|'url_removed'|'status_changed'|
                                 -- 'title_changed'|'canonical_changed'|
                                 -- 'indexability_lost'|'indexability_gained'|
                                 -- 'issue_appeared'|'issue_resolved'
    url         TEXT,
    field       TEXT,
    value_before TEXT,
    value_after  TEXT,
    severity     TEXT
);
```

**Requirement:** before diffing, compare `config_json` and `rules_version` of both `crawl_meta`
rows. If they differ in anything that affects scope, warn in the UI — otherwise changes that are
really configuration changes will be attributed to the site.

## 6. Remote hub (Pro, planned)

Postgres or MariaDB. **Aggregates only.** Never the raw HTML, nor every header, nor the `links`
table.

```sql
CREATE TABLE projects (
    id          UUID PRIMARY KEY,
    account_id  UUID NOT NULL,
    name        TEXT NOT NULL,
    base_url    TEXT NOT NULL,
    adapter     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE crawl_runs (
    id                UUID PRIMARY KEY,
    project_id        UUID NOT NULL REFERENCES projects(id),
    started_at        TIMESTAMPTZ NOT NULL,
    finished_at       TIMESTAMPTZ,
    urls_total        INTEGER,
    urls_indexable    INTEGER,
    urls_4xx          INTEGER,
    urls_5xx          INTEGER,
    urls_redirect     INTEGER,
    avg_response_ms   INTEGER,
    issues_critical   INTEGER,
    issues_high       INTEGER,
    issues_medium     INTEGER,
    issues_low        INTEGER,
    core_version      TEXT,
    origin            TEXT      -- 'macos'|'windows'|'cli'
);

CREATE TABLE run_issues (
    run_id      UUID NOT NULL REFERENCES crawl_runs(id),
    rule_id     TEXT NOT NULL,
    severity    TEXT NOT NULL,
    count       INTEGER NOT NULL,
    sample_urls JSONB,           -- at most 20 sample URLs
    PRIMARY KEY (run_id, rule_id)
);
```

**Future advantage, which is the real reason to build it:** this schema *is* the SaaS. The day a
web version exists, the data, the schema and the CLI writing into it from CI already exist. The
desktop app becomes one more client.

## 7. Serverless analytics: Parquet + DuckDB

An alternative to the hub for whoever does not want to run a database. Each crawl exports to
partitioned Parquet, and DuckDB queries across all of them with no server at all:

```
crawls/
  project=blog-decoracion/date=2026-07-01/urls.parquet
  project=blog-decoracion/date=2026-07-08/urls.parquet
  project=meteo-es/date=2026-07-01/urls.parquet
```

```sql
SELECT project, date, count(*) FILTER (WHERE status_code >= 400) AS errores
FROM read_parquet('crawls/**/*.parquet', hive_partitioning = true)
GROUP BY 1, 2 ORDER BY 2 DESC;
```

A hundred sites over 52 weeks, queried from a laptop, zero infrastructure. It is the recommended
default for the Pro tier; the Postgres hub is left for teams and for the SaaS.

## 8. Middle option: libSQL / Turso

If at some point "SQLite but remote and synchronized" is wanted without changing APIs: `libsql` is
driver-compatible with embedded replicas. Evaluate it **only if** the Postgres hub turns out to be
too heavy for the real use case. It is not built before that.
