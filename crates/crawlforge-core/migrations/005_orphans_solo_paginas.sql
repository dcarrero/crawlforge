-- Migración 005 — una imagen no es una página huérfana.
--
-- `v_orphans` pedía tres cosas: interna, en el sitemap, y sin enlaces entrantes. No pedía la
-- cuarta, que da nombre a la regla: **ser una página**.
--
-- WordPress publica un sitemap de imágenes. Sus `/wp-content/uploads/…​.png` son internas, están
-- en el sitemap, y ninguna página las enlaza con un `<a>` —se usan con `<img src>`, que va a la
-- tabla `images` y no a `links`. Las tres condiciones se cumplían y salían como huérfanas.
--
-- Medido rastreando un medio de comunicación el 2026-08-01: **1.867 de los 1.912 hallazgos eran
-- imágenes.** Páginas de verdad, 42. Es el tercer falso positivo sistemático del proyecto que
-- aparece rastreando un sitio real y que ningún test unitario vio, y el peor de los tres: sale
-- en severidad `high` en cualquier WordPress.
--
-- El arreglo es exigir fila en `pages`, que solo existe cuando se ha descargado algo y se ha
-- parseado como HTML. Eso quita las imágenes, y de paso quita el otro falso positivo de la misma
-- regla: en un rastreo truncado, las URLs que el sitemap declara y a las que no se llegó tampoco
-- tienen fila en `pages`, así que dejan de contarse como huérfanas cuando lo único que pasa es
-- que aún no se han visitado.
--
-- Lo que la regla sigue detectando es exactamente lo que promete: una página que se descargó,
-- que el sitio declara en su sitemap, y a la que nadie enlaza.
--
-- Las vistas se redefinen sin tocar datos: es segura sobre un fichero de rastreo antiguo.

DROP VIEW IF EXISTS v_orphans;

CREATE VIEW v_orphans AS
SELECT u.id, u.url
FROM urls u
JOIN pages p ON p.url_id = u.id
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
