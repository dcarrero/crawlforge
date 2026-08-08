#!/usr/bin/env bash
#
# Genera un fichero de rastreo sintético contra el esquema congelado (migración 001).
#
# Para qué sirve: cualquier cliente que lea el fichero de rastreo —una interfaz gráfica, un
# cuaderno de análisis, un script— puede desarrollarse contra este fixture sin rastrear nada.
# Y sirve de banco: medio millón de filas es donde se ve si una tabla se desplaza con soltura.
#
# Uso:
#   tools/gen-fixture.sh                      # 500.000 URLs → fixtures/crawl-500k.sqlite
#   tools/gen-fixture.sh 50000 pequeno.sqlite # tamaño y destino a medida
#
# El .sqlite resultante NO se versiona (ver .gitignore): se regenera con este script.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
N_URLS="${1:-500000}"
OUT="${2:-$ROOT/fixtures/crawl-500k.sqlite}"
MIGRATION="$ROOT/crates/crawlforge-core/migrations/001_initial.sql"

command -v sqlite3 >/dev/null || { echo "Falta sqlite3 en el PATH." >&2; exit 1; }
[ -f "$MIGRATION" ] || { echo "No encuentro la migración: $MIGRATION" >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT" "$OUT-wal" "$OUT-shm"

# Proporciones aproximadas a un rastreo real de blog WordPress grande.
N_PAGES=$(( N_URLS * 80 / 100 ))    # el 80% de las URLs son HTML con página asociada
N_LINKS=$(( N_URLS * 3 ))           # ~3 enlaces salientes registrados por URL
N_IMAGES=$(( N_URLS * 60 / 100 ))
N_ISSUES=$(( N_URLS * 35 / 100 ))

# Ningún enlace apunta por encima de este ID. El 1% superior queda sin enlaces entrantes
# para que `v_orphans` devuelva filas: el contenido huérfano es uno de los hallazgos que
# la UI tiene que saber pintar, y sin datos no se puede desarrollar esa vista.
N_LINKABLE=$(( N_URLS * 99 / 100 ))

echo "Generando $OUT"
echo "  urls=$N_URLS pages=$N_PAGES links=$N_LINKS images=$N_IMAGES issues=$N_ISSUES"

sqlite3 "$OUT" > /dev/null <<SQL
PRAGMA journal_mode = OFF;
PRAGMA synchronous  = OFF;
PRAGMA temp_store   = MEMORY;
PRAGMA cache_size   = -256000;

.read '$MIGRATION'

CREATE TABLE schema_version (version INTEGER NOT NULL, applied_at TEXT NOT NULL);
INSERT INTO schema_version VALUES (1, datetime('now'));

-- Contador auxiliar: más rápido que un CTE recursivo repetido en cada INSERT.
CREATE TEMP TABLE seq (i INTEGER PRIMARY KEY);
INSERT INTO seq (i)
WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM c WHERE i < $N_URLS)
SELECT i FROM c;

BEGIN;

INSERT INTO crawl_meta VALUES (
    '0192f4a0-0000-7000-8000-000000000001',
    'proj-fixture', 'Fixture sintético', 'https://ejemplo-fixture.es',
    'http', NULL, datetime('now', '-2 hours'), datetime('now'), 'done',
    json_object('max_depth', 12, 'concurrency', 8, 'respect_robots', 1),
    '0.0.1-fixture', '0.0.1-fixture', NULL, 'pro'
);

-- URLs. Distribución de códigos de estado deliberadamente sucia: sin 4xx, 5xx ni
-- redirecciones el fixture no ejercita las vistas ni los filtros de la tabla.
INSERT INTO urls (
    id, url, url_hash, scheme, host, path, query, depth, discovered_from,
    is_internal, in_sitemap, crawl_state, exclusion_reason, status_code,
    redirect_to, redirect_chain_len, content_type, content_length,
    response_time_ms, fetched_at, error_kind, error_message)
SELECT
    i,
    'https://' ||
      CASE WHEN i % 50 = 0 THEN 'externo-' || (i % 7) || '.com' ELSE 'ejemplo-fixture.es' END ||
      '/' || CASE i % 6
               WHEN 0 THEN 'blog'     WHEN 1 THEN 'categoria'
               WHEN 2 THEN 'producto' WHEN 3 THEN 'guia'
               WHEN 4 THEN 'tag'      ELSE 'pagina' END ||
      '/' || (i % 400) || '/articulo-numero-' || i,
    (i * 2654435761) % 9223372036854775807,
    'https',
    CASE WHEN i % 50 = 0 THEN 'externo-' || (i % 7) || '.com' ELSE 'ejemplo-fixture.es' END,
    '/' || (i % 400) || '/articulo-numero-' || i,
    CASE WHEN i % 11 = 0 THEN 'utm_source=fixture&pagina=' || (i % 20) ELSE NULL END,
    (i % 12),
    CASE WHEN i > 1 THEN 1 + (i % 1000) ELSE NULL END,
    CASE WHEN i % 50 = 0 THEN 0 ELSE 1 END,
    CASE WHEN i % 3 = 0 THEN 1 ELSE 0 END,
    CASE WHEN i % 97 = 0 THEN 'error' WHEN i % 89 = 0 THEN 'excluded' ELSE 'done' END,
    CASE WHEN i % 89 = 0 THEN 'robots' ELSE NULL END,
    CASE WHEN i % 97 = 0 THEN NULL
         WHEN i % 53 = 0 THEN 404 WHEN i % 211 = 0 THEN 500
         WHEN i % 29 = 0 THEN 301 WHEN i % 137 = 0 THEN 302 ELSE 200 END,
    CASE WHEN i % 29 = 0 THEN 1 + ((i * 7) % $N_URLS) ELSE NULL END,
    CASE WHEN i % 29 = 0 THEN 1 + (i % 4) ELSE 0 END,
    CASE WHEN i % 17 = 0 THEN 'image/webp'
         WHEN i % 23 = 0 THEN 'application/pdf' ELSE 'text/html; charset=utf-8' END,
    12000 + ((i * 37) % 180000),
    35 + ((i * 13) % 1400),
    datetime('now', '-' || (i % 7200) || ' seconds'),
    CASE WHEN i % 97 = 0 THEN
        CASE (i / 97) % 4 WHEN 0 THEN 'timeout' WHEN 1 THEN 'dns'
                          WHEN 2 THEN 'tls' ELSE 'connection' END
    ELSE NULL END,
    CASE WHEN i % 97 = 0 THEN 'fallo sintético del fixture' ELSE NULL END
FROM seq;

-- Páginas. Los títulos se repiten cada 900 filas a propósito: así hay duplicados reales
-- que las reglas de conjunto y la vista de agrupación tienen que detectar.
INSERT INTO pages (
    url_id, title, title_len, title_px, meta_description, meta_desc_len, meta_desc_px,
    h1, h1_count, h2_count, heading_json, canonical, canonical_is_self, meta_robots,
    x_robots_tag, is_indexable, indexability_reason, lang, hreflang_json, word_count,
    text_hash, html_hash, content_ratio, viewport, og_json, twitter_json, schema_types,
    amp_url, internal_links_out, internal_links_in, crawl_depth_source)
SELECT
    u.id,
    CASE WHEN u.id % 71 = 0 THEN NULL
         ELSE 'Guía de ' || CASE u.id % 6
                              WHEN 0 THEN 'decoración' WHEN 1 THEN 'jardinería'
                              WHEN 2 THEN 'cocina'     WHEN 3 THEN 'reformas'
                              WHEN 4 THEN 'iluminación' ELSE 'diseño' END
              || ' — artículo ' || (u.id % 900) END,
    CASE WHEN u.id % 71 = 0 THEN NULL ELSE 28 + (u.id % 45) END,
    CASE WHEN u.id % 71 = 0 THEN NULL ELSE 210 + (u.id % 400) END,
    CASE WHEN u.id % 43 = 0 THEN NULL
         ELSE 'Todo lo que necesitas saber sobre el tema ' || (u.id % 900)
              || ', explicado paso a paso con ejemplos prácticos.' END,
    CASE WHEN u.id % 43 = 0 THEN NULL ELSE 95 + (u.id % 90) END,
    CASE WHEN u.id % 43 = 0 THEN NULL ELSE 640 + (u.id % 500) END,
    CASE WHEN u.id % 61 = 0 THEN NULL ELSE 'Encabezado principal ' || (u.id % 900) END,
    CASE WHEN u.id % 61 = 0 THEN 0 WHEN u.id % 149 = 0 THEN 2 ELSE 1 END,
    2 + (u.id % 9),
    json_array('h2', 'h3', 'h3', 'h2'),
    'https://ejemplo-fixture.es/' || (u.id % 400) || '/articulo-numero-'
        || CASE WHEN u.id % 31 = 0 THEN (u.id - 1) ELSE u.id END,
    CASE WHEN u.id % 31 = 0 THEN 0 ELSE 1 END,
    CASE WHEN u.id % 37 = 0 THEN 'noindex, follow' ELSE 'index, follow' END,
    NULL,
    CASE WHEN u.id % 37 = 0 OR u.id % 31 = 0 OR u.status_code >= 400
              OR u.status_code IN (301, 302) THEN 0 ELSE 1 END,
    CASE WHEN u.id % 37 = 0 THEN 'noindex'
         WHEN u.id % 31 = 0 THEN 'canonicalised'
         WHEN u.status_code >= 500 THEN '5xx'
         WHEN u.status_code >= 400 THEN '4xx'
         WHEN u.status_code IN (301, 302) THEN 'redirect' ELSE NULL END,
    CASE WHEN u.id % 13 = 0 THEN 'en' ELSE 'es' END,
    NULL,
    180 + ((u.id * 17) % 2400),
    (u.id * 1099511628211) % 9223372036854775807,
    (u.id * 6364136223846793) % 9223372036854775807,
    0.12 + ((u.id % 60) / 100.0),
    'width=device-width, initial-scale=1',
    json_object('title', 'og ' || (u.id % 900), 'type', 'article'),
    json_object('card', 'summary_large_image'),
    CASE u.id % 5 WHEN 0 THEN 'Article,BreadcrumbList'
                  WHEN 1 THEN 'Product,Offer'
                  WHEN 2 THEN 'FAQPage' ELSE 'WebPage' END,
    NULL,
    3 + (u.id % 40),
    CASE WHEN u.id % 173 = 0 THEN 0 ELSE 1 + (u.id % 220) END,
    CASE u.id % 4 WHEN 0 THEN 'sitemap' WHEN 1 THEN 'link'
                  WHEN 2 THEN 'link' ELSE 'adapter' END
FROM urls u
WHERE u.id <= $N_PAGES AND u.content_type LIKE 'text/html%';

-- Enlaces.
INSERT INTO links (id, from_url_id, to_url_id, anchor, rel, is_nofollow, element, region, position)
SELECT
    ROW_NUMBER() OVER (),
    1 + (s.i % $N_URLS),
    1 + ((s.i * 7919) % $N_LINKABLE),
    CASE WHEN s.i % 19 = 0 THEN NULL
         WHEN s.i % 7  = 0 THEN 'leer más'
         ELSE 'ancla descriptiva ' || (s.i % 500) END,
    CASE WHEN s.i % 23 = 0 THEN 'nofollow' ELSE NULL END,
    CASE WHEN s.i % 23 = 0 THEN 1 ELSE 0 END,
    CASE s.i % 8 WHEN 0 THEN 'img' WHEN 1 THEN 'link' WHEN 2 THEN 'script' ELSE 'a' END,
    CASE s.i % 5 WHEN 0 THEN 'nav'    WHEN 1 THEN 'footer'
                 WHEN 2 THEN 'aside'  WHEN 3 THEN 'main' ELSE 'unknown' END,
    s.i % 120
FROM (
    WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM c WHERE i < $N_LINKS)
    SELECT i FROM c
) s;

INSERT INTO images (id, page_url_id, src_url_id, alt, alt_present, title,
                    width_attr, height_attr, loading, in_srcset, format)
SELECT
    s.i,
    1 + (s.i % $N_PAGES),
    1 + ((s.i * 31) % $N_URLS),
    CASE WHEN s.i % 4 = 0 THEN NULL ELSE 'Fotografía de ' || (s.i % 300) END,
    CASE WHEN s.i % 4 = 0 THEN 0 ELSE 1 END,
    NULL,
    CASE WHEN s.i % 6 = 0 THEN NULL ELSE 800 + (s.i % 800) END,
    CASE WHEN s.i % 6 = 0 THEN NULL ELSE 600 + (s.i % 600) END,
    CASE s.i % 3 WHEN 0 THEN 'lazy' WHEN 1 THEN 'eager' ELSE NULL END,
    CASE WHEN s.i % 5 = 0 THEN 1 ELSE 0 END,
    CASE s.i % 5 WHEN 0 THEN 'webp' WHEN 1 THEN 'avif' WHEN 2 THEN 'jpeg'
                 WHEN 3 THEN 'png'  ELSE 'svg' END
FROM (
    WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM c WHERE i < $N_IMAGES)
    SELECT i FROM c
) s;

-- Hallazgos. Los IDs de regla salen de docs/04-CATALOGO-REGLAS.md.
--
-- Severidad y categoría se derivan del rule_id mediante este catálogo, nunca por separado:
-- una regla tiene UNA severidad y UNA categoría. Si se asignan con módulos independientes,
-- `v_issue_summary` devuelve la misma regla con severidades contradictorias y la UI agrupa mal.
INSERT INTO issues (id, url_id, rule_id, severity, category, detail_json, group_key)
SELECT
    s.i,
    1 + (s.i % $N_URLS),
    r.rule_id,
    r.severity,
    r.category,
    json_object('observado', s.i % 500, 'esperado', 'ver regla'),
    CASE WHEN r.rule_id = 'META-TITLE-DUPLICATE' THEN 'dup-' || (s.i % 900) ELSE NULL END
FROM (
    WITH RECURSIVE c(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM c WHERE i < $N_ISSUES)
    SELECT i FROM c
) s
JOIN (
    SELECT 0 AS k, 'META-TITLE-MISSING'   AS rule_id, 'high'     AS severity, 'meta'          AS category
    UNION ALL SELECT 1, 'META-TITLE-DUPLICATE', 'high',     'meta'
    UNION ALL SELECT 2, 'META-TITLE-TOO-LONG',  'medium',   'meta'
    UNION ALL SELECT 3, 'META-DESC-MISSING',    'medium',   'meta'
    UNION ALL SELECT 4, 'CONTENT-H1-MISSING',   'medium',   'content'
    UNION ALL SELECT 5, 'CONTENT-H1-MULTIPLE',  'low',      'content'
    UNION ALL SELECT 6, 'HTTP-404-INTERNAL',    'critical', 'http'
    UNION ALL SELECT 7, 'CANON-MISSING',        'medium',   'indexability'
    UNION ALL SELECT 8, 'IMG-ALT-MISSING',      'low',      'accessibility'
    UNION ALL SELECT 9, 'LINK-REDIRECT-CHAIN',  'info',     'links'
) r ON r.k = s.i % 10;

COMMIT;

PRAGMA optimize;
VACUUM;
SQL

echo
echo "Listo: $OUT ($(du -h "$OUT" | cut -f1))"
sqlite3 "$OUT" "
  SELECT 'urls',    COUNT(*) FROM urls
  UNION ALL SELECT 'pages',   COUNT(*) FROM pages
  UNION ALL SELECT 'links',   COUNT(*) FROM links
  UNION ALL SELECT 'images',  COUNT(*) FROM images
  UNION ALL SELECT 'issues',  COUNT(*) FROM issues
  UNION ALL SELECT 'v_broken_links',    COUNT(*) FROM v_broken_links
  UNION ALL SELECT 'v_indexable_pages', COUNT(*) FROM v_indexable_pages
  UNION ALL SELECT 'v_orphans',         COUNT(*) FROM v_orphans;"
