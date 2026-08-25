-- 009 · `v_broken_links` sigue las redirecciones hasta donde acaban.
--
-- La vista original solo miraba el destino inmediato del enlace: si una página enlaza
-- `/go/producto` y esa URL responde 301 hacia una tienda ajena que devuelve 404, el enlace no
-- aparecía por ninguna parte. El 301 no es un estado roto y el 404 no lo apunta ningún enlace,
-- así que la fila caía entre las dos condiciones. Es el patrón donde más se pudren los enlaces,
-- porque el destino es de otro y nadie avisa cuando cae; desde la 0.9.0 el motor registra y
-- comprueba ese destino, y hasta ahora ese trabajo no se veía aquí.
--
-- La cadena se recorre en SQL para que la vista siga siendo autónoma: quien abra el fichero con
-- `sqlite3` y escriba `SELECT * FROM v_broken_links` tiene la respuesta sin pasar por la
-- herramienta. Las reglas la recorren en Rust (`load_redirects`/`walk`), y las dos formas
-- conviven porque resuelven preguntas distintas.
--
-- El tope de diez saltos es lo que corta un bucle: `A → B → A` generaría filas sin fin, y sin el
-- tope la vista colgaría el proceso. Diez es holgado —`HTTP-REDIRECT-CHAIN` ya avisa a partir de
-- dos— y hace de red de seguridad, no de criterio.
--
-- Dos columnas nuevas para no perder información al seguir la cadena: `via` es la URL que se
-- enlazó de verdad, la que hay que reescribir, y `hops` cuántos saltos hicieron falta. En una
-- fila directa las dos van a NULL, que es la diferencia entre «esto está roto» y «esto lleva a
-- algo roto».
DROP VIEW IF EXISTS v_broken_links;

CREATE VIEW v_broken_links AS
WITH RECURSIVE resolved(start_id, end_id, hops) AS (
    SELECT id, redirect_to, 1
      FROM urls
     WHERE status_code >= 300 AND status_code < 400 AND redirect_to IS NOT NULL
    UNION ALL
    SELECT r.start_id, u.redirect_to, r.hops + 1
      FROM resolved r
      JOIN urls u ON u.id = r.end_id
     WHERE u.status_code >= 300 AND u.status_code < 400
       AND u.redirect_to IS NOT NULL
       AND r.hops < 10
)
SELECT l.from_url_id,
       uf.url         AS from_url,
       ut.url         AS to_url,
       ut.status_code AS status_code,
       l.anchor       AS anchor,
       NULL           AS via,
       NULL           AS hops
  FROM links l
  JOIN urls uf ON uf.id = l.from_url_id
  JOIN urls ut ON ut.id = l.to_url_id
 WHERE ut.status_code >= 400

UNION ALL

SELECT l.from_url_id,
       uf.url          AS from_url,
       fin.url         AS to_url,
       fin.status_code AS status_code,
       l.anchor        AS anchor,
       ut.url          AS via,
       r.hops          AS hops
  FROM links l
  JOIN urls uf     ON uf.id = l.from_url_id
  JOIN urls ut     ON ut.id = l.to_url_id
  JOIN resolved r  ON r.start_id = ut.id
  JOIN urls fin    ON fin.id = r.end_id
 WHERE fin.status_code >= 400;
