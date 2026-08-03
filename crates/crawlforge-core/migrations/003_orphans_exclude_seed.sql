-- Migración 003 — la portada no es una página huérfana.
--
-- `v_orphans`, tal como está definida en `docs/02-MODELO-DATOS.md §4`, devuelve toda URL interna
-- que esté en el sitemap y no reciba ningún enlace interno. La raíz del sitio cumple las dos
-- condiciones siempre: está en el sitemap y nadie la enlaza, porque es el punto de entrada.
--
-- Detectado rastreando el banco de pruebas: la portada salía como huérfana. Es un
-- falso positivo que aparecería en el cien por cien de los rastreos, y encima en uno de los
-- hallazgos que diferencian al producto. Un informe que empieza con un error obvio no se lee.
--
-- Las vistas se pueden redefinir sin tocar datos, así que esta migración es segura sobre un
-- fichero de rastreo antiguo.

DROP VIEW IF EXISTS v_orphans;

CREATE VIEW v_orphans AS
SELECT u.id, u.url
FROM urls u
LEFT JOIN links l ON l.to_url_id = u.id
WHERE u.is_internal = 1
  AND u.in_sitemap = 1
  AND l.id IS NULL
  -- La semilla del rastreo nunca es huérfana.
  AND u.url NOT IN (SELECT base_url FROM crawl_meta)
  -- Ni su variante con y sin barra final: `https://sitio.es` y `https://sitio.es/` son la
  -- misma portada, y cuál de las dos quede en `base_url` depende de cómo la escribió el
  -- usuario al lanzar el rastreo.
  AND u.url NOT IN (SELECT RTRIM(base_url, '/') FROM crawl_meta)
  AND u.url NOT IN (SELECT RTRIM(base_url, '/') || '/' FROM crawl_meta);
