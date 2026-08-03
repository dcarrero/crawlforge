-- Migración 004 — qué dijeron el `robots.txt` y los sitemaps.
--
-- El motor los descargaba y los usaba, pero no dejaba constancia de ellos: el `robots.txt` vivía
-- solo en el caché en memoria (`RobotsCache`) y de cada sitemap se aprovechaban las URLs y se
-- descartaba todo lo demás. Al terminar el rastreo no quedaba forma de saber si el fichero
-- existía, qué respondió, ni si el XML era válido.
--
-- Eso bloqueaba tres reglas del catálogo —`INDEX-ROBOTS-TXT-MISSING`,
-- `INDEX-ROBOTS-TXT-BLOCKS-ALL` e `INDEX-SITEMAP-ERROR`— que no son un capricho: un
-- `Disallow: /` olvidado en producción tras un despliegue es la forma más rápida y silenciosa de
-- desaparecer de Google, y es exactamente el hallazgo por el que un auditor abre la herramienta.
--
-- Y sirve para comparar: con estas filas, un diff entre dos rastreos puede decir «el robots.txt
-- cambió» y «el sitemap declara 4.000 URLs menos que la semana pasada», que son dos de las
-- alertas más útiles de una cartera de sitios.

-- ---------------------------------------------------------------- robots.txt

CREATE TABLE robots_txt (
    id            INTEGER PRIMARY KEY,
    host          TEXT    NOT NULL UNIQUE,
    -- NULL si no se llegó a pedir (modo `list`) o si la petición falló sin respuesta.
    status_code   INTEGER,
    -- El fichero tal cual. Hace falta para explicar el hallazgo y para comparar dos rastreos:
    -- «alguien añadió Disallow: / el martes» es más útil que «el sitio no se indexa».
    content       TEXT,
    -- Resultado de evaluar el fichero contra nuestro user-agent, no una búsqueda de texto:
    -- `Disallow: /` puede estar bajo otro `User-agent` y no aplicarnos.
    blocks_all    INTEGER NOT NULL DEFAULT 0,
    -- Sitemaps anunciados con `Sitemap:`.
    sitemap_count INTEGER NOT NULL DEFAULT 0,
    fetched_at    TEXT
);

-- ---------------------------------------------------------------- Sitemaps

CREATE TABLE sitemaps (
    id           INTEGER PRIMARY KEY,
    url          TEXT    NOT NULL UNIQUE,
    status_code  INTEGER,
    -- 1 si es un índice (`<sitemapindex>`) en vez de una lista de URLs.
    is_index     INTEGER NOT NULL DEFAULT 0,
    -- 0 si el XML no se pudo interpretar. `parse_error` dice por qué.
    is_valid     INTEGER NOT NULL DEFAULT 1,
    parse_error  TEXT,
    -- URLs declaradas (o sitemaps hijos, si es un índice).
    url_count    INTEGER NOT NULL DEFAULT 0,
    -- Tamaño del fichero descargado. El límite del protocolo es 50 MB sin comprimir.
    bytes        INTEGER NOT NULL DEFAULT 0,
    -- 'robots' | 'well_known' | 'index'
    discovered_from TEXT NOT NULL,
    fetched_at   TEXT
);

CREATE INDEX idx_sitemaps_valid ON sitemaps(is_valid);
