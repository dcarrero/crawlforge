-- Migración 002 — marca de rastreo truncado.
--
-- `docs/03-MOTOR-CRAWL.md §9` exige marcar `truncated=true` en `crawl_meta` cuando un rastreo
-- termina por alcanzar el límite del nivel (Free: 1.000 URLs) en vez de por agotar la cola.
-- La columna faltaba en el esquema de `02-MODELO-DATOS.md §3.1`.
--
-- Importa para el producto: al truncar, el rastreo termina limpiamente con `status='done'` y
-- **muestra todos los hallazgos encontrados hasta ahí**. Sin esta columna, la UI no puede
-- distinguir «este sitio tiene 1.000 URLs» de «vimos las primeras 1.000 de un sitio mayor»,
-- y presentaría los recuentos como si fueran completos.

ALTER TABLE crawl_meta ADD COLUMN truncated INTEGER NOT NULL DEFAULT 0;

-- Motivo del truncado, para poder decirle al usuario cuál de sus límites saltó:
-- 'max_urls' | 'max_depth' | 'max_duration' | NULL si no se truncó.
ALTER TABLE crawl_meta ADD COLUMN truncated_reason TEXT;
