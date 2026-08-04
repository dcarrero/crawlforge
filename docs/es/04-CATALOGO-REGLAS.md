# 04 — Catálogo de reglas

> Versión en inglés: [`../04-CATALOGO-REGLAS.md`](../04-CATALOGO-REGLAS.md) — **la inglesa es la que manda.** Esta
> traducción puede ir por detrás; si las dos discrepan, la buena es la otra.

Las reglas **son** el producto. El motor es infraestructura; esto es lo que el usuario compra.

## 1. Convenciones

**ID:** `CATEGORIA-SUJETO-CONDICION`, en inglés, mayúsculas, estable para siempre.
Un ID publicado nunca cambia de significado: si la lógica cambia sustancialmente, se crea un ID
nuevo y el viejo se deprecia. Los diffs históricos dependen de esa estabilidad.

**Severidades:**

| Nivel | Significado |
|---|---|
| `critical` | Impide la indexación o rompe el sitio |
| `high` | Daño claro y medible al posicionamiento |
| `medium` | Buena práctica incumplida, impacto moderado |
| `low` | Mejora menor |
| `info` | Dato informativo, no es un problema |

**Alcance:** `page` (evaluada en streaming durante el rastreo) o `site` (requiere el rastreo
completo, se evalúa en la pasada final con SQL).

**Nivel:** regla disponible en `free`, `pro` o `agency`.

**Cada regla requiere:** un fixture HTML en `crates/crawlforge-rules/fixtures/` y un test. Sin
excepción.

---

## 2. Indexabilidad y rastreo (`INDEX`)

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `INDEX-NOINDEX` | medium | page | free | `meta robots` o `X-Robots-Tag` contiene `noindex`. **`critical` solo en la portada** |
| `INDEX-ROBOTS-BLOCKED` | critical | site | free | URL bloqueada por robots.txt pero enlazada internamente |
| `INDEX-BLOCKED-IN-SITEMAP` | critical | site | free | URL en sitemap pero bloqueada por robots.txt |
| `INDEX-NOINDEX-IN-SITEMAP` | critical | site | free | URL en sitemap con `noindex` |
| `INDEX-ROBOTS-TXT-MISSING` | medium | site | free | No existe `/robots.txt` (404) |
| `INDEX-ROBOTS-TXT-BLOCKS-ALL` | critical | site | free | `Disallow: /` para `*` |
| `INDEX-NOFOLLOW-INTERNAL` | medium | page | free | Enlace interno con `rel=nofollow` |
| `INDEX-SITEMAP-MISSING` | high | site | free | No se encuentra ningún sitemap. **Solo en modo `http`**: en una auditoría de un `dist/` el sitio aún no se ha publicado, y avisarlo en cada compilación sería ruido en el pipeline de CI |
| `INDEX-SITEMAP-ERROR` | high | site | free | Sitemap con XML inválido o >50.000 URLs / >50 MB |
| `INDEX-ORPHAN-PAGE` | high | site | free | En sitemap o en adaptador, pero sin ningún enlace interno entrante |
| `INDEX-DEEP-PAGE` | medium | site | free | Profundidad de clic > 4, **contando solo lo alcanzable** |
| `INDEX-SECTION-DISCONNECTED` | high | site | free | Conjunto de páginas con enlaces entrantes al que no se llega desde la portada ni desde una raíz de idioma. **Un hallazgo por sección, no por página** |
| `INDEX-NO-INTERNAL-LINKS-IN` | high | site | free | Página indexable con 0 enlaces internos entrantes |

**`INDEX-NOINDEX` bajó de `critical` a `medium` el 2026-08-01.** «Esta página lleva noindex» es una
directiva, no un defecto: en un WordPress real eran 848 páginas —el 55% del sitio— y todas eran
`/tag/`, paginaciones y `/author/` que el plugin SEO excluye a propósito. Un `critical` que acierta
el 100% de las veces y aporta cero es peor que no estar: enseña a saltarse la columna de severidad.
Los casos donde sí es una emergencia se detectan **por señales estructurales, no por listas de
patrones**: la contradicción con el sitemap ya es `INDEX-NOINDEX-IN-SITEMAP` (`critical`), y el
`noindex` en la portada —el accidente clásico de un despliegue desde *staging*— se escala aquí.

**`INDEX-SECTION-DISCONNECTED` se separó de `INDEX-DEEP-PAGE` el 2026-08-01.** Inalcanzable no es
profundo. En un sitio Astro bilingüe, las 1.987 páginas `/en/*` salían como «demasiados clics
desde la portada» cuando el problema real era otro: no hay ni un `<a>` de `/es` a `/en` —el único
puente es `<link rel="alternate" hreflang>` y el selector de idioma es JavaScript—, así que el
recorrido nunca llegaba. Ahora el BFS se siembra también con los destinos `hreflang` de la portada,
`DEEP-PAGE` solo mide lo que alcanza, y lo inalcanzable con enlaces entrantes se colapsa en **un**
hallazgo de sitio con su recuento y sus ejemplos. Medido: 2.138 → 236 hallazgos ciertos, más un
hallazgo nuevo y real (66 fichas de jugador genuinamente desconectadas).

**`INDEX-DEEP-PAGE` guarda la profundidad real y el informe la dice una vez (2026-08-03).** En el
rastreo completo de un medio con quince años de archivo (216.349 páginas), la regla dio 202.392
hallazgos **todos ciertos** —el archivo no tiene atajos de paginación— y un informe que abre con
esa cifra no se lee, igual que cuando eran falsos positivos. No hay `group_key` que valga: cada
página es genuinamente distinta. El arreglo tiene dos partes. La regla calcula ahora la
profundidad con un BFS iterativo (mismo coste medido que los dos CTE anteriores, mismo resultado)
y escribe `{"click_depth":N,"max_click_depth":4}` por página; y el informe, cuando una regla
afecta al 40% o más de las páginas (`crawlforge_rules::is_pervasive`, umbral medido sobre seis
rastreos), conserva el recuento y le añade la cuota del sitio — para esta regla, además, la forma:
`202,392 pages deeper than 4 clicks — 94% of the site (typical depth 6–9, deepest 48)`. Nada se
pierde: cada fila sigue en `issues`, el export las lleva todas y `report --rule INDEX-DEEP-PAGE`
las lista ordenadas por profundidad, la más hundida primero.

**Estas tres no se evalúan sobre un rastreo truncado (2026-07-30, ampliado el 2026-08-01).** Su respuesta depende de que el
grafo de enlaces esté completo, y un rastreo cortado —por el tope del nivel gratuito, por
`--max-urls` o por tiempo— deja sin enlaces salientes a todo lo que quedó pendiente. Medido: en un
rastreo de 40 URLs de un blog real, `INDEX-DEEP-PAGE` avisaba en 39 de 40 páginas porque el
recorrido no podía alcanzar ninguna. Están en `crawlforge_rules::REQUIERE_GRAFO_COMPLETO` y el
motor las omite; es preferible no decir nada a decir algo falso.

`INDEX-ORPHAN-PAGE` se sumó a esa lista el 2026-08-01, y también `INDEX-SECTION-DISCONNECTED`:
sobre el grafo truncado de un medio de 176.000 URLs habría dado 202 secciones «desconectadas» que
solo estaban sin visitar.

**`INDEX-ROBOTS-BLOCKED` pasó de `page` a `site` el 2026-07-30.** El motor excluye la URL bloqueada
*antes* de descargarla —que es lo correcto: respetar `robots.txt` significa no pedirla— así que
nunca existe un `PageContext` sobre el que evaluarla. El dato sí está en el almacén
(`crawl_state='excluded'` con `exclusion_reason='robots'`, más su fila en `links`), y ahí es donde
se lee. La alternativa, que es lo que hace Screaming Frog, sería descargar las bloqueadas que estén
enlazadas internamente; eso cambia el comportamiento del rastreador y se descartó: no se toca cómo
rastrea el motor para que encaje el alcance de una regla.

## 3. Códigos de estado y redirecciones (`HTTP`)

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `HTTP-404-INTERNAL` | critical | site | free | Enlace interno a URL que devuelve 404 |
| `HTTP-404-EXTERNAL` | medium | site | free | Enlace externo roto |
| `HTTP-5XX` | critical | page | free | Respuesta 5xx |
| `HTTP-REDIRECT-CHAIN` | high | site | free | Cadena de redirección de 2 o más saltos |
| `HTTP-REDIRECT-LOOP` | critical | site | free | Bucle de redirección |
| `HTTP-TEMP-REDIRECT` | medium | page | free | 302/307 permanente en el tiempo (aparece en 2+ rastreos) |
| `HTTP-REDIRECT-TO-404` | critical | site | free | Redirección que termina en 404 |
| `HTTP-MIXED-CONTENT` | high | page | free | Página HTTPS que carga recursos por HTTP |
| `HTTP-NO-HTTPS` | critical | site | free | El sitio responde por HTTP sin redirigir a HTTPS |
| `HTTP-SLOW-RESPONSE` | medium | page | free | TTFB > 1.000 ms |
| `HTTP-LARGE-PAGE` | medium | page | free | HTML > 500 KB |
| `HTTP-NO-COMPRESSION` | medium | page | pro | Sin `Content-Encoding: gzip/br` en HTML |
| `HTTP-NO-CACHE-HEADERS` | low | page | pro | Recursos estáticos sin `Cache-Control` |
| `HTTP-SOFT-404` | high | site | pro | Devuelve 200 pero el contenido indica error (heurística: pocas palabras + patrón de texto) |

## 4. Títulos y meta descripciones (`META`)

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `META-TITLE-MISSING` | critical | page | free | Sin `<title>` o vacío |
| `META-TITLE-DUPLICATE` | high | site | free | Mismo título en 2+ páginas indexables |
| `META-TITLE-TOO-LONG` | medium | page | free | Ancho estimado > 580 px |
| `META-TITLE-TOO-SHORT` | low | page | free | < 30 caracteres |
| `META-TITLE-MULTIPLE` | medium | page | free | Más de una etiqueta `<title>` |
| `META-DESC-MISSING` | high | page | free | Sin meta description |
| `META-DESC-DUPLICATE` | medium | site | free | Repetida en 2+ páginas indexables |
| `META-DESC-TOO-LONG` | low | page | free | Ancho estimado > 990 px |
| `META-DESC-TOO-SHORT` | low | page | free | < 70 caracteres |
| `META-VIEWPORT-MISSING` | high | page | free | Sin `meta viewport` |
| `META-REFRESH` | high | page | free | Uso de `meta http-equiv=refresh` |

**Nota de implementación:** el ancho en píxeles se calcula con las métricas de Arial 20px (títulos)
y 14px (descripciones), que es como Google trunca. Es un aviso mucho más útil que contar caracteres,
y en español importa más aún porque las palabras son más largas.

## 5. Canonical y contenido duplicado (`CANON`)

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `CANON-MISSING` | medium | page | free | Página indexable sin canonical |
| `CANON-MULTIPLE` | high | page | free | Más de un `link rel=canonical` |
| `CANON-RELATIVE` | medium | page | free | Canonical en URL relativa |
| `CANON-TO-4XX` | critical | site | free | Canonical apunta a URL con error |
| `CANON-TO-REDIRECT` | high | site | free | Canonical apunta a una redirección |
| `CANON-TO-NOINDEX` | critical | site | free | Canonical apunta a página con `noindex` |
| `CANON-CHAIN` | high | site | free | A canoniza a B, y B canoniza a C |
| `CANON-CROSS-DOMAIN` | medium | page | free | Canonical a otro dominio |
| `DUP-CONTENT-EXACT` | high | site | free | Hash de HTML idéntico entre 2+ URLs |
| `DUP-CONTENT-NEAR` | medium | site | pro | Simhash con similitud > 90% |
| `DUP-H1` | low | site | pro | Mismo H1 en 2+ páginas |

## 6. Encabezados y contenido (`CONTENT`)

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `CONTENT-H1-MISSING` | high | page | free | Sin H1 |
| `CONTENT-H1-MULTIPLE` | low | page | free | Más de un H1 |
| `CONTENT-H1-EMPTY` | medium | page | free | H1 vacío o solo con una imagen sin alt |
| `CONTENT-HEADING-SKIP` | low | page | free | Salto de nivel (H2 → H4) |
| `CONTENT-THIN` | high | page | free | Página indexable con < 300 palabras |
| `CONTENT-LOW-RATIO` | medium | page | pro | Ratio texto/HTML < 10% |
| `CONTENT-LANG-MISSING` | medium | page | free | Sin atributo `lang` en `<html>` |
| `CONTENT-LANG-MISMATCH` | medium | page | pro | `lang` declarado no coincide con el idioma detectado |

## 7. Imágenes y recursos (`ASSET`)

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `ASSET-IMG-NO-ALT` | high | page | free | `<img>` sin atributo `alt` |
| `ASSET-IMG-EMPTY-ALT-LINK` | high | page | free | Imagen con `alt=""` dentro de un enlace sin otro texto |
| `ASSET-IMG-BROKEN` | high | site | free | Imagen que devuelve 4xx/5xx |
| `ASSET-IMG-HEAVY` | medium | site | free | Imagen > 200 KB |
| `ASSET-IMG-LEGACY-FORMAT` | low | page | pro | JPEG/PNG sin alternativa WebP/AVIF |
| `ASSET-IMG-NO-DIMENSIONS` | medium | page | pro | Sin `width`/`height` (provoca CLS) |
| `ASSET-BROKEN` | high | site | free | CSS o JS que devuelve 4xx/5xx |

**Corrección de alcance (2026-07-30):** `ASSET-IMG-HEAVY` figuraba como `page` y es `site`. El peso
de una imagen no está en el HTML —`width` y `height` declaran maquetación, no bytes— así que no se
puede decidir con la página delante: hace falta la fila de `urls` del recurso ya descargado. El dato
es `urls.content_length`, y ahí se queda: desde el 2026-08-04 el escritor sí puebla `resources`,
pero con una fila por URL de recurso y no por par (página, recurso), así que la mitad «qué páginas
usan esta imagen» la sigue respondiendo `images` y no ella.

## 8. Internacionalización (`HREFLANG`)

Bloque de alto valor para el cliente (ejemplo.es/ejemplo.me, otro proyecto, otro proyecto multiidioma).

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `HREFLANG-NO-SELF` | high | page | free | Conjunto hreflang sin referencia a sí misma |
| `HREFLANG-NOT-RECIPROCAL` | high | site | free | A apunta a B, B no apunta a A |
| `HREFLANG-INVALID-CODE` | high | page | free | Código de idioma o región no válido según ISO 639-1 / 3166-1 |
| `HREFLANG-TO-4XX` | critical | site | free | hreflang apunta a URL con error |
| `HREFLANG-TO-NOINDEX` | critical | site | pro | hreflang apunta a página no indexable |
| `HREFLANG-CONFLICT-CANONICAL` | high | site | pro | hreflang y canonical se contradicen |
| `HREFLANG-NO-XDEFAULT` | low | site | pro | Conjunto multiidioma sin `x-default` |

## 9. Datos estructurados y social (`SCHEMA`)

| ID | Sev. | Alcance | Nivel | Condición |
|---|---|---|---|---|
| `SCHEMA-INVALID-JSON` | high | page | pro | JSON-LD malformado |
| `SCHEMA-MISSING-REQUIRED` | medium | page | pro | Falta una propiedad obligatoria del tipo declarado |
| `SCHEMA-MISSING-ARTICLE` | low | page | pro | Página tipo artículo sin schema `Article`/`BlogPosting` |
| `SOCIAL-OG-MISSING` | low | page | free | Sin `og:title` / `og:description` / `og:image` |
| `SOCIAL-OG-IMAGE-BROKEN` | medium | site | pro | `og:image` devuelve error |

## 10. WordPress (`WP`) — requiere adaptador, nivel Pro

Ver `05-ADAPTADORES.md`.

| ID | Sev. | Condición |
|---|---|---|
| `WP-ORPHAN-POST` | high | Post publicado en la REST API que no se alcanzó rastreando |
| `WP-ATTACHMENT-INDEXABLE` | high | Páginas de adjunto indexables (clásico generador de contenido basura) |
| `WP-THIN-ARCHIVE` | medium | Archivo de etiqueta o categoría con un solo post |
| `WP-REPLYTOCOM` | medium | URLs con `?replytocom` rastreables |
| `WP-PAGINATION-TRAP` | high | Paginación `/page/N/` que continúa más allá del total real |
| `WP-SITEMAP-MISMATCH` | high | El sitemap de Yoast/RankMath no coincide con el contenido publicado |
| `WP-MISSING-SEO-META` | medium | Post sin meta description en Yoast/RankMath |
| `WP-OUTDATED-PLUGIN` | info | Versión de plugin detectada por `?ver=` que está desactualizada |
| `WP-XMLRPC-OPEN` | low | `/xmlrpc.php` accesible |
| `WP-FEED-DUPLICATE` | low | Feeds indexables duplicando contenido |

## 11. Sitios estáticos / Astro (`STATIC`) — nivel Pro

| ID | Sev. | Condición |
|---|---|---|
| `STATIC-ROUTE-NOT-IN-SITEMAP` | medium | Ruta generada en `dist/` ausente del sitemap |
| `STATIC-SITEMAP-ORPHAN` | high | URL en sitemap sin fichero correspondiente en `dist/` |
| `STATIC-COLLECTION-NO-ROUTE` | medium | Entrada de colección de contenido sin ruta generada |
| `STATIC-HYDRATION-ONLY-LINK` | high | Enlace que solo existe tras hidratar la isla → invisible para el rastreador |
| `STATIC-BROKEN-RELATIVE` | critical | Enlace relativo que no resuelve a ningún fichero de `dist/` |
| `STATIC-ASSET-UNREFERENCED` | info | Fichero en `dist/` al que no apunta nada |

## 12. Accesibilidad (`A11Y`) — previsto, puente con la normativa europea

Reservado. Se poblará inyectando `axe-core` a través del webview de renderizado. Los IDs seguirán
el patrón `A11Y-<regla-axe>` y cada hallazgo citará su referencia en **WCAG 2.1 AA + EN 301 549 +
Directiva UE 2019/882**, con el disclaimer de revisión manual siempre visible, tal como se definió
en el plan de MVP de cumplimiento EAA.

**Todavía no se implementa.** Pero el `trait Rule` debe admitir ya un campo de
referencias normativas para no refactorizar después:

```rust
fn references(&self) -> &[Reference];   // { standard, clause, url }
```

## 13. Reparto por nivel — resumen

| Nivel | Reglas | Criterio |
|---|---|---|
| Free | ~50 | Todo el SEO técnico fundamental. **No se oculta ningún hallazgo dentro del límite de 1.000 URLs** |
| Pro | +25 | Casi-duplicados, schema, soft-404, WordPress, estáticos, hreflang avanzado, render JS |
| Agency | +A11Y y reglas propias | Motor de reglas personalizadas del usuario |

Recuerda el principio: **el Free limita la escala, no el conocimiento.** Un
usuario gratuito con un blog de 400 páginas debe obtener una auditoría completa y quedar impresionado.
Ese es el motor de conversión.

## 14. Motor de reglas personalizadas (Agency, previsto)

Definidas por el usuario en YAML, evaluadas sobre el almacén:

```yaml
- id: CUSTOM-PRICE-BLOCK-MISSING
  severity: high
  scope: page
  when:
    url_matches: "^/producto/"
    css_absent: ".precio"
  message: "Ficha de producto sin bloque de precio"
```

Selectores CSS, expresiones regulares sobre el HTML, condiciones sobre columnas del almacén, y
consultas SQL directas para casos avanzados. Es la respuesta a la "extracción personalizada" de
Screaming Frog, yendo un paso más allá: allí extraes, aquí además evalúas.
