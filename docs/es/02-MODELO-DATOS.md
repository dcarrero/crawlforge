# 02 — Modelo de datos

> Versión en inglés: [`../02-MODELO-DATOS.md`](../02-MODELO-DATOS.md) — **la inglesa es la que manda.** Esta
> traducción puede ir por detrás; si las dos discrepan, la buena es la otra.

## 1. Principios

1. **Un rastreo = un fichero SQLite.** Portable, comprimible, enviable a un cliente.
2. El core escribe; la UI y la CLI leen. Ver `01-ARQUITECTURA.md §2`.
3. Las URLs se almacenan una sola vez en `urls` y se referencian por `INTEGER` en todas partes.
   Con 500k URLs de 80 caracteres, duplicarlas en `links` costaría gigabytes.
4. Migraciones numeradas y hacia adelante. Un fichero de hace un año debe seguir abriéndose.

## 2. Configuración de la conexión

Son **dos** juegos, y la diferencia entre ellos es deliberada. Los dos viven en `store.rs`
(`WRITER_PRAGMAS` y `READER_PRAGMAS`) para que Swift y C# no tengan que deducirlos ni acaben
divergiendo entre plataformas.

El escritor:

```sql
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous  = NORMAL;   -- suficiente: un rastreo se puede repetir
PRAGMA foreign_keys = ON;
PRAGMA temp_store   = MEMORY;
PRAGMA cache_size   = -64000;   -- 64 MB
PRAGMA mmap_size    = 0;        -- ver abajo
```

La conexión de solo lectura de la interfaz:

```sql
PRAGMA query_only   = 1;
PRAGMA busy_timeout = 5000;
PRAGMA temp_store   = MEMORY;
PRAGMA cache_size   = -64000;
PRAGMA mmap_size    = 268435456;
```

**El mapeo en memoria compensa al lector y no al escritor.** La interfaz pagina y ordena la misma
tabla constantemente y no hace más que leer; el motor escribe en masa desde un solo hilo, donde el
mapeo no aporta nada y cuesta memoria residente, que es la métrica sobre la que se argumenta este
producto. Este documento prescribía un solo juego con el mapeo de 256 MB para todo, y seguirlo
pone mmap en el escritor, que es justo lo que el código evita.

El `busy_timeout` tampoco es opcional: un lector sobre el mismo fichero **es la arquitectura**
(`CONVENTIONS.md §2.2`), no un caso raro, y sin él cualquier roce de bloqueos era un `database is
locked` inmediato incluso cuando el lector iba a soltarlo décimas de segundo después.

Al finalizar el rastreo: `PRAGMA optimize;` y el checkpoint de WAL que saca al fichero del modo
WAL, para que «un rastreo = un fichero portable» sobreviva a copiar el `.sqlite` a solas.

**El `VACUUM` se midió y se descartó**, y este documento lo recomendaba. Sobre un rastreo de 50.000
URLs y 6,15 millones de enlaces redujo el fichero un **2%** (380 MB → 372 MB) y disparó el pico de
memoria de 168 MB a 246 MB. El 30-40% aparece cuando hay fragmentación por borrados y
actualizaciones; un fichero de rastreo se escribe una vez y de forma incremental, así que casi no
la hay. Pagar 78 MB de pico por un 2% de disco es mal negocio. Quien lo quiera, lo tiene en
`store::compact`.

## 3. Esquema

### 3.1 Metadatos

```sql
CREATE TABLE schema_version (
    version     INTEGER NOT NULL,
    applied_at  TEXT    NOT NULL
);

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
    tier_at_runtime TEXT NOT NULL,           -- reglas aplicadas según nivel
    truncated       INTEGER NOT NULL DEFAULT 0,  -- migración 002
    truncated_reason TEXT                    -- 'max_urls'|'max_depth'|'max_duration'|'list_mode'
);
```

`truncated` dice que **el conjunto rastreado no es el sitio entero**, y no es decorativo: el motor
lo usa para callar las reglas de `crawlforge_rules::REQUIERE_GRAFO_COMPLETO`, y `diff` para negarse
a afirmar que una URL ha desaparecido. `list_mode` se marca en todo rastreo en modo lista — no
porque nada se haya cortado, sino porque una lista solo ve las URLs que le diste.

`config_json` y `rules_version` son imprescindibles para que un diff entre dos rastreos sepa si la
diferencia viene del sitio o de un cambio de configuración/reglas.

### 3.2 URLs

```sql
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
```

**Sobre `url_hash`:** la comparación de URLs es la operación más frecuente del rastreo (¿ya la
visitamos?). Un `INTEGER` indexado es mucho más rápido que `TEXT UNIQUE`. Mantén ambos: el hash
para el hot path, el texto único como garantía de integridad.

### 3.3 Páginas HTML

```sql
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
```

**`title_px` y `meta_desc_px`:** Google trunca por ancho en píxeles, no por número de caracteres.
Calcularlo con las métricas de Arial 20px/14px da un aviso mucho más útil que "más de 60 caracteres".

### 3.4 Enlaces

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
    position     INTEGER            -- orden de aparición en el documento
);

CREATE INDEX idx_links_from ON links(from_url_id);
CREATE INDEX idx_links_to   ON links(to_url_id);
```

`region` se deduce del ancestro semántico más cercano (`<nav>`, `<main>`, `<footer>`). Permite
distinguir enlaces de plantilla de enlaces de contenido, que es la diferencia que importa en
enlazado interno.

### 3.5 Recursos e imágenes

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

`resources` es **una fila por URL de recurso**, no por par (página, recurso) — fíjate en que no
tiene `page_url_id`, y es a propósito: en CSS y JS la arista página↔recurso aporta mucho menos
que en imágenes (un `bundle.js` de 900 KB se carga en toda la plantilla, no en una entrada
concreta), así que el fichero ya identifica el problema. Para las imágenes esa arista sí existe
y vive en `images`. El `kind` se deduce del `content_type` de la respuesta, con la extensión de
la URL como respaldo cuando el servidor no manda uno útil (`application/octet-stream` es
frecuente en fuentes). La unicidad por `url_id` la garantiza el índice de la migración 008.

### 3.6 Hallazgos — la tabla que es el producto

```sql
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
```

### 3.7 Búsqueda de texto completo

```sql
CREATE VIRTUAL TABLE pages_fts USING fts5(
    url, title, meta_description, body_text,
    content = '',                -- contentless: no duplicamos el texto
    tokenize = 'unicode61 remove_diacritics 2'
);
```

`remove_diacritics 2` es obligatorio para español: permite que "diseño" encuentre "diseno" y
viceversa. **Solo se puebla en nivel Pro** (el texto de cuerpo multiplica el tamaño del fichero).

### 3.8 Extracción personalizada (Pro)

```sql
CREATE TABLE extractions (
    id          INTEGER PRIMARY KEY,
    url_id      INTEGER NOT NULL REFERENCES urls(id),
    name        TEXT    NOT NULL,   -- nombre del extractor definido por el usuario
    value       TEXT,
    occurrence  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_extractions ON extractions(name, url_id);
```

### 3.9 Datos de adaptador

```sql
CREATE TABLE adapter_entities (
    id           INTEGER PRIMARY KEY,
    adapter      TEXT NOT NULL,       -- 'wordpress'|'astro'
    entity_type  TEXT NOT NULL,       -- 'post'|'page'|'term'|'plugin'|'route'|'collection'
    external_id  TEXT,
    url_id       INTEGER REFERENCES urls(id),   -- NULL si no se rastreó → huérfana
    data_json    TEXT NOT NULL
);
CREATE INDEX idx_adapter_entities ON adapter_entities(adapter, entity_type);
```

Que `url_id` sea `NULL` es precisamente el hallazgo valioso: existe en WordPress pero no se alcanzó
rastreando → **contenido huérfano**.

### 3.10 `robots.txt` y sitemaps (migración 004)

Lo que el rastreo consultó, no lo que descubrió. Se añadió el 2026-07-30: el motor descargaba los
dos ficheros, los usaba y los tiraba, así que al terminar no quedaba constancia de si el
`robots.txt` existía ni de si un sitemap tenía el XML roto. Eso bloqueaba tres reglas del catálogo,
y una de ellas —`INDEX-ROBOTS-TXT-BLOCKS-ALL`— avisa del accidente más caro y más silencioso que
hay: el `robots.txt` del entorno de pruebas, con `Disallow: /`, subido a producción.

```sql
CREATE TABLE robots_txt (
    id            INTEGER PRIMARY KEY,
    host          TEXT    NOT NULL UNIQUE,
    status_code   INTEGER,          -- NULL: no se llegó a pedir o no hubo respuesta
    content       TEXT,             -- el fichero tal cual, para explicar el hallazgo y para el diff
    blocks_all    INTEGER NOT NULL DEFAULT 0,   -- evaluado con el parser, no buscando texto
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

Dos detalles que no son evidentes:

- **`blocks_all` se evalúa, no se busca.** Un `Disallow: /` puede estar bajo otro `User-agent` y no
  aplicarnos; darlo por bueno sería el falso positivo más caro del catálogo, decirle a alguien que
  su sitio está bloqueado cuando no lo está.
- **`discovered_from` importa para decidir si algo es un error.** Que `/sitemap_index.xml` dé 404 es
  lo normal: se prueba a ciegas. Que falle uno anunciado en `robots.txt` no lo es, porque alguien lo
  declaró.

Sirven además para comparar: con estas filas, un diff entre dos rastreos puede decir «el robots.txt
cambió» y «el sitemap declara 4.000 URLs menos que la semana pasada».

## 4. Vistas para la UI

Define las agregaciones como vistas para que Swift y C# no dupliquen SQL.

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

-- Definición vigente: migraciones 003 y 005. La de aquí es la original, y le faltaban dos
-- condiciones; se conserva para que se lea qué costó cada una.
--
--   003 — la portada cumple las tres condiciones siempre (está en el sitemap y nadie la enlaza,
--         porque es el punto de entrada), así que salía como huérfana en todos los rastreos.
--   005 — **el `JOIN pages`**. Sin él, «huérfana» no exigía ser una página: las imágenes del
--         sitemap de imágenes de WordPress son internas, están declaradas, y se usan con
--         `<img src>`, que va a `images` y no a `links`. En un medio de comunicación fueron 1.867
--         de 1.912 hallazgos. Exigir la fila de `pages` quita también las URLs que el sitemap
--         declara y el rastreo no llegó a visitar.
CREATE VIEW v_orphans AS
SELECT u.id, u.url FROM urls u
LEFT JOIN links l ON l.to_url_id = u.id
WHERE u.is_internal = 1 AND u.in_sitemap = 1 AND l.id IS NULL;
```

## 5. Diffs entre rastreos

No hay formato propietario: se hace con `ATTACH`.

```sql
ATTACH DATABASE 'crawl_2026-07-01.sqlite' AS a;
ATTACH DATABASE 'crawl_2026-07-08.sqlite' AS b;

-- URLs que aparecen nuevas
SELECT b.url FROM b.urls b LEFT JOIN a.urls a ON a.url_hash = b.url_hash
WHERE a.id IS NULL AND b.is_internal = 1;

-- Cambios de código de estado
SELECT b.url, a.status_code AS antes, b.status_code AS ahora
FROM b.urls b JOIN a.urls a ON a.url_hash = b.url_hash
WHERE a.status_code IS NOT b.status_code;

-- Páginas que perdieron indexabilidad
SELECT u.url, pa.indexability_reason AS antes, pb.indexability_reason AS ahora
FROM b.pages pb
JOIN b.urls u   ON u.id = pb.url_id
JOIN a.urls ua  ON ua.url_hash = u.url_hash
JOIN a.pages pa ON pa.url_id = ua.id
WHERE pa.is_indexable = 1 AND pb.is_indexable = 0;
```

`crawl_diff()` genera un tercer fichero SQLite con una única tabla `changes`:

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

**Requisito:** antes de diffear, comparar `config_json` y `rules_version` de ambos `crawl_meta`.
Si difieren en algo que afecte al alcance, avisar en la UI — si no, se atribuirán al sitio cambios
que en realidad son de configuración.

## 6. Hub remoto (Pro, previsto)

Postgres o MariaDB. **Solo agregados.** Nunca el HTML crudo, ni cada cabecera, ni la tabla `links`.

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
    sample_urls JSONB,           -- máximo 20 URLs de muestra
    PRIMARY KEY (run_id, rule_id)
);
```

**Ventaja futura, que es la razón real de construirlo:** este esquema *es* el SaaS. El día que haya
versión web, ya existen los datos, el esquema y el CLI escribiendo en él desde CI. La app de
escritorio pasa a ser un cliente más.

## 7. Analítica sin servidor: Parquet + DuckDB

Alternativa al hub para quien no quiera montar una base de datos. Cada rastreo exporta a Parquet
particionado, y DuckDB consulta sobre todos ellos sin ningún servidor:

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

Cien sitios por 52 semanas, consultados desde un portátil, cero infraestructura. Es la opción
recomendada por defecto para el nivel Pro; el hub Postgres queda para equipos y para el SaaS.

## 8. Opción intermedia: libSQL / Turso

Si en algún momento se quiere "SQLite pero remoto y sincronizado" sin cambiar de API: `libsql`
es compatible a nivel de driver con réplicas embebidas. Evaluar **solo si** el hub
Postgres resulta demasiado pesado para el caso de uso real. No se implementa antes.
