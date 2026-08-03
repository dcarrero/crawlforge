-- Migración 001 — esquema inicial de un fichero de rastreo.
--
-- Congelado a partir de `docs/02-MODELO-DATOS.md §3` y §4.
-- NUNCA se edita este fichero una vez publicado: un rastreo antiguo debe seguir abriéndose.
-- Todo cambio va en una migración nueva y numerada.

-- ---------------------------------------------------------------- 3.1 Metadatos

CREATE TABLE crawl_meta (
    id              TEXT PRIMARY KEY,        -- uuid v7 (ordenable por tiempo)
    project_id      TEXT NOT NULL,
    project_name    TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    mode            TEXT NOT NULL,           -- 'http' | 'filesystem' | 'list'
    source_path     TEXT,                    -- solo modo filesystem
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    status          TEXT NOT NULL,           -- 'running'|'paused'|'done'|'cancelled'|'failed'
    config_json     TEXT NOT NULL,           -- CrawlJob serializado íntegro
    core_version    TEXT NOT NULL,
    rules_version   TEXT NOT NULL,
    adapter         TEXT,                    -- 'wordpress' | 'astro' | NULL
    tier_at_runtime TEXT NOT NULL            -- reglas aplicadas según nivel
);

-- ---------------------------------------------------------------- 3.2 URLs

CREATE TABLE urls (
    id                  INTEGER PRIMARY KEY,
    url                 TEXT    NOT NULL UNIQUE,
    url_hash            INTEGER NOT NULL,      -- xxh3 de la URL normalizada
    scheme              TEXT    NOT NULL,
    host                TEXT    NOT NULL,
    path                TEXT    NOT NULL,
    query               TEXT,
    depth               INTEGER,               -- clics desde la raíz; NULL si no alcanzada
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

-- ---------------------------------------------------------------- 3.3 Páginas HTML

CREATE TABLE pages (
    url_id              INTEGER PRIMARY KEY REFERENCES urls(id),
    title               TEXT,
    title_len           INTEGER,
    title_px            INTEGER,      -- ancho estimado en píxeles (Google corta por píxeles)
    meta_description    TEXT,
    meta_desc_len       INTEGER,
    meta_desc_px        INTEGER,
    h1                  TEXT,         -- primer h1
    h1_count            INTEGER,
    h2_count            INTEGER,
    heading_json        TEXT,         -- jerarquía completa, para detectar saltos de nivel
    canonical           TEXT,
    canonical_is_self   INTEGER,
    meta_robots         TEXT,
    x_robots_tag        TEXT,
    is_indexable        INTEGER NOT NULL,
    indexability_reason TEXT,         -- 'noindex'|'canonicalised'|'robots'|'redirect'|'4xx'|'5xx'
    lang                TEXT,
    hreflang_json       TEXT,
    word_count          INTEGER,
    text_hash           INTEGER,      -- simhash del texto, para casi-duplicados
    html_hash           INTEGER,      -- xxh3 exacto
    content_ratio       REAL,         -- texto / html
    viewport            TEXT,
    og_json             TEXT,
    twitter_json        TEXT,
    schema_types        TEXT,         -- CSV de @type detectados en JSON-LD
    amp_url             TEXT,
    internal_links_out  INTEGER,
    internal_links_in   INTEGER,      -- se rellena en la pasada final
    crawl_depth_source  TEXT          -- 'link'|'sitemap'|'list'|'adapter'
);

CREATE INDEX idx_pages_title_len  ON pages(title_len);
CREATE INDEX idx_pages_indexable  ON pages(is_indexable);
CREATE INDEX idx_pages_text_hash  ON pages(text_hash);
CREATE INDEX idx_pages_links_in   ON pages(internal_links_in);

-- ---------------------------------------------------------------- 3.4 Enlaces

CREATE TABLE links (
    id           INTEGER PRIMARY KEY,
    from_url_id  INTEGER NOT NULL REFERENCES urls(id),
    to_url_id    INTEGER NOT NULL REFERENCES urls(id),
    anchor       TEXT,
    rel          TEXT,
    is_nofollow  INTEGER NOT NULL DEFAULT 0,
    element      TEXT NOT NULL,     -- 'a'|'link'|'img'|'script'|'iframe'|'form'
    region       TEXT,              -- 'nav'|'main'|'footer'|'aside'|'unknown'
    position     INTEGER            -- orden de aparición en el documento
);

CREATE INDEX idx_links_from ON links(from_url_id);
CREATE INDEX idx_links_to   ON links(to_url_id);

-- ---------------------------------------------------------------- 3.5 Recursos e imágenes

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

-- ---------------------------------------------------------------- 3.6 Hallazgos

CREATE TABLE issues (
    id          INTEGER PRIMARY KEY,
    url_id      INTEGER REFERENCES urls(id),   -- NULL = hallazgo de sitio
    rule_id     TEXT    NOT NULL,              -- 'SEO-TITLE-MISSING'
    severity    TEXT    NOT NULL,              -- 'critical'|'high'|'medium'|'low'|'info'
    category    TEXT    NOT NULL,
    detail_json TEXT,                          -- contexto específico de la regla
    group_key   TEXT                           -- agrupa duplicados: hash del título repetido, etc.
);

CREATE INDEX idx_issues_rule     ON issues(rule_id);
CREATE INDEX idx_issues_url      ON issues(url_id);
CREATE INDEX idx_issues_severity ON issues(severity);
CREATE INDEX idx_issues_group    ON issues(group_key) WHERE group_key IS NOT NULL;

-- ---------------------------------------------------------------- 3.7 Búsqueda de texto completo
-- Solo se puebla en nivel Pro: el texto de cuerpo multiplica el tamaño del fichero.
-- `remove_diacritics 2` es obligatorio para español ("diseño" ↔ "diseno").

CREATE VIRTUAL TABLE pages_fts USING fts5(
    url, title, meta_description, body_text,
    content = '',
    tokenize = 'unicode61 remove_diacritics 2'
);

-- ---------------------------------------------------------------- 3.8 Extracción personalizada

CREATE TABLE extractions (
    id          INTEGER PRIMARY KEY,
    url_id      INTEGER NOT NULL REFERENCES urls(id),
    name        TEXT    NOT NULL,   -- nombre del extractor definido por el usuario
    value       TEXT,
    occurrence  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_extractions ON extractions(name, url_id);

-- ---------------------------------------------------------------- 3.9 Datos de adaptador
-- `url_id` NULL es el hallazgo valioso: existe en el CMS pero no se alcanzó rastreando
-- → contenido huérfano.

CREATE TABLE adapter_entities (
    id           INTEGER PRIMARY KEY,
    adapter      TEXT NOT NULL,       -- 'wordpress'|'astro'
    entity_type  TEXT NOT NULL,       -- 'post'|'page'|'term'|'plugin'|'route'|'collection'
    external_id  TEXT,
    url_id       INTEGER REFERENCES urls(id),
    data_json    TEXT NOT NULL
);

CREATE INDEX idx_adapter_entities ON adapter_entities(adapter, entity_type);

-- ---------------------------------------------------------------- 4. Vistas para la UI
-- Las agregaciones viven aquí para que Swift y C# no dupliquen SQL.

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

CREATE VIEW v_orphans AS
SELECT u.id, u.url FROM urls u
LEFT JOIN links l ON l.to_url_id = u.id
WHERE u.is_internal = 1 AND u.in_sitemap = 1 AND l.id IS NULL;
