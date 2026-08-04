# 03 — Motor de rastreo

> Versión en inglés: [`../03-MOTOR-CRAWL.md`](../03-MOTOR-CRAWL.md) — **la inglesa es la que manda.** Esta
> traducción puede ir por detrás; si las dos discrepan, la buena es la otra.

## 1. Los tres modos

| Modo | Origen | Uso |
|---|---|---|
| `http` | Rastreo desde una URL semilla | Modo normal |
| `filesystem` | Directorio local (`dist/`, `public/`, `_site/`) | Auditoría pre-deploy. **Diferenciador** |
| `list` | Lista de URLs pegada o importada | Auditar un conjunto concreto. Muy demandado, coste casi nulo |

Los tres desembocan en el mismo pipeline de parseo, reglas y almacén. Solo cambia la fuente de bytes:

```rust
trait Fetcher: Send + Sync {
    async fn fetch(&self, target: &Target) -> Result<FetchedDoc>;
}
// HttpFetcher · FilesystemFetcher · WebviewFetcher (render JS, Pro)
```

**El modo `filesystem` es lo que Screaming Frog no puede hacer.** Se resuelven las rutas como lo
haría el servidor (`/about` → `about/index.html` o `about.html`), se reescriben los enlaces contra
`--base`, y se rastrean miles de páginas en segundos sin red.

**El modo `list` audita exactamente el conjunto pedido, y el fichero lo sabe.** Los enlaces de
cada página son una propiedad de esa página y se registran todos —el destino recibe su fila en
`urls` con `crawl_state='skipped'`—, pero **nada fuera de la lista se descarga**: ni destinos de
enlaces, ni destinos de redirección, ni las URLs que declaren los sitemaps (se registran con
`in_sitemap=1`; el cruce es información, no ampliación del rastreo). Las externas se comprueban
igual que en modo `http` (§9). Y como los enlazadores de una página no se descargan nunca, el
grafo de enlaces está incompleto **por definición**: todo rastreo en modo lista marca
`crawl_meta.truncated=1` con `truncated_reason='list_mode'`, lo que apaga las reglas de
`REQUIERE_GRAFO_COMPLETO` y hace que `diff` no afirme ausencias sobre él. No es un corte —el
rastreo hizo exactamente lo que se le pidió— así que la CLI lo cuenta con texto propio, nunca
con el aviso de «rastreo truncado».

## 2. Ciclo de vida

```
semillas (URL | sitemap | directorio | lista)
   → normalizar
   → ¿vista ya? (índice url_hash en memoria)
   → ¿permitida? (robots, patrones de inclusión/exclusión, profundidad, límite de nivel)
   → encolar en frontier (prioridad por profundidad)
   → fetch (con hueco libre en el límite de concurrencia del host)
   → parsear (lol_html, streaming)
   → extraer enlaces → volver a normalizar
   → evaluar PageRules
   → enviar lote al hilo escritor
   → [al agotar la cola] pasada final: SiteRules + métricas agregadas + FTS + VACUUM
```

## 3. Normalización de URL

El error más común y el más caro: rastrear la misma página cincuenta veces con querystrings
distintas. Reglas, en este orden:

1. Minúsculas en esquema y host. **Nunca en la ruta** (puede ser sensible a mayúsculas).
2. Eliminar puerto por defecto (`:80` en http, `:443` en https).
3. Resolver `.` y `..`.
4. Decodificar percent-encoding innecesario; recodificar de forma consistente.
5. Eliminar el fragmento (`#...`) salvo que empiece por `#!` (hashbang legado).
6. **Ordenar los parámetros de query alfabéticamente.**
7. Eliminar parámetros de la lista de descarte configurable. Por defecto: `utm_*`, `gclid`,
   `fbclid`, `msclkid`, `mc_cid`, `mc_eid`, `_ga`, `ref`, `si`.
8. Normalizar barra final según lo que responda el servidor en la primera resolución del host,
   no según una suposición.
9. IDN a Punycode.

Guardar siempre **ambas**: la URL original tal como aparece en el HTML (para los informes) y la
normalizada (para deduplicar).

## 4. robots.txt y sitemaps

- Un `robots.txt` por host, cacheado durante todo el rastreo.
- Se respeta `Disallow` para el user-agent configurado, con fallback a `*`.
- `Crawl-delay` se respeta y **anula** la configuración de concurrencia del usuario para ese host.
- Las URLs bloqueadas se registran con `crawl_state='excluded'`, `exclusion_reason='robots'`.
  **No se ocultan**: saber qué está bloqueado es un hallazgo en sí mismo.
- Modo "ignorar robots.txt" disponible solo tras confirmación explícita y con aviso de que solo
  debe usarse en sitios propios.
- Sitemaps: descubrir por `robots.txt` (`Sitemap:`), por `/sitemap.xml` y por `/sitemap_index.xml`.
  Soportar índices anidados, `.gz` y sitemaps de imágenes y noticias. Marcar `in_sitemap = 1`.
  El cruce sitemap ↔ enlaces es lo que produce los hallazgos de huérfanas.

## 5. Parseo con `lol_html`

`lol_html` procesa en streaming mediante manejadores por selector, sin construir el DOM. Es la
razón de rendimiento del proyecto: 5-10x más rápido que `scraper` en páginas grandes.

Manejadores necesarios en una sola pasada:

```
title, meta[name=description], meta[name=robots], meta[name=viewport]
link[rel=canonical], link[rel=alternate][hreflang], link[rel=amphtml]
html[lang]
h1..h6
a[href], img[src], img[srcset], script[src], link[rel=stylesheet], iframe[src]
meta[property^=og:], meta[name^=twitter:]
script[type="application/ld+json"]
nav, main, footer, aside          → para deducir `region` de los enlaces
```

**Cuidado con el estado:** el orden de aparición importa (primer `h1`, jerarquía de encabezados,
posición de enlaces). Mantén un `struct PageAccumulator` mutable a lo largo de la pasada.

Texto de cuerpo: acumular solo si el nivel lo permite (`word_count` siempre, texto completo para
FTS solo en Pro). Excluir el contenido de `<script>`, `<style>`, `<nav>`, `<footer>` del recuento
de palabras.

## 6. Indexabilidad

Regla central de todo el producto. Una página es indexable si **todas** se cumplen:

1. Código de estado 200.
2. `Content-Type` es HTML.
3. No hay `noindex` en `meta robots` ni en cabecera `X-Robots-Tag`.
4. No está bloqueada por `robots.txt`.
5. El canonical apunta a sí misma o está ausente.
6. No es el origen de una redirección.

Se guarda siempre el motivo en `indexability_reason`. La pregunta "¿por qué esta página no está en
Google?" se responde con esa columna, y es la consulta más frecuente que hace un SEO.

## 7. Reintentos y resiliencia

```
timeout de conexión: 10 s
timeout total por petición: 30 s
reintentos: 3, backoff exponencial con jitter (1s, 2s, 4s ±50%)
reintentar en: 429, 500, 502, 503, 504, timeout, error de conexión
no reintentar en: 4xx salvo 429, error TLS, error DNS
límite de tamaño de respuesta: 10 MB (configurable). Superarlo → error_kind='toolarge'
```

Ante tres respuestas de sobrecarga consecutivas del mismo host —**429 o 503**— reducir
automáticamente su concurrencia a la mitad y avisar en la UI. Un crawler que tumba el servidor del
cliente es un crawler inservible. El 503 cuenta porque un Varnish o un Cloudflare saturado responde
503 y no 429, y el efecto sobre el servidor es el mismo: distinguirlos sería fiel a la letra de este
documento e infiel a su motivo. La recuperación es deliberadamente más lenta que la reducción: se
baja a la mitad de golpe y se sube de uno en uno tras una racha de respuestas buenas.

Un rastreo interrumpido (cierre de la app, corte de red) debe poder reanudarse: la cola pendiente
vive en `urls` con `crawl_state='pending'`, así que reanudar es releer esa tabla.

Hecho en `engine::resume` (CLI: `crawlforge resume <fichero>`). Semántica cerrada:

- **La configuración que manda es la del rastreo original** (`crawl_meta.config_json`), y no se
  aceptan flags nuevos: reanudar tiene que dar el mismo resultado que no haber parado. Con otra
  configuración se rastrea de nuevo, no se reanuda.
- Las `pending` vuelven al frontier con su `depth` guardado: el orden BFS sobrevive al corte.
- Un corte cooperativo (Ctrl+C en la CLI, `CancelSignal` en el motor) vacía el hilo escritor y
  deja `status='paused'`; un corte brusco (kill, cuelgue) deja `status='running'`. Los dos se
  reanudan. La pasada final no se ejecuta al interrumpir: la ejecuta quien termina.
- **No se reanuda**: un rastreo terminado (`status='done'`), un fichero de otra versión de
  esquema ni uno cuya configuración guardada no se pueda leer.

## 8. Renderizado JavaScript (Pro, previsto)

Dos implementaciones tras la misma interfaz:

| Build | Motor | Fidelidad |
|---|---|---|
| Tienda | WKWebView (macOS) / **WebView2 = Chromium** (Windows) | Windows: alta. macOS: WebKit, suficiente para el 95% de casos |
| Directo (Agency) | `chromiumoxide` → CDP contra el Chrome instalado | Alta, con interceptación fina de peticiones |

Reglas comunes:
- El render es **opt-in por proyecto**, nunca por defecto. Es 20-50x más lento.
- Concurrencia de render limitada a 2-4 instancias, independientemente de la concurrencia HTTP.
- Comparar siempre HTML crudo vs. HTML renderizado y registrar la diferencia: enlaces que solo
  existen tras hidratar, contenido inyectado, canonical modificado por JS. **Esa comparación es un
  hallazgo en sí misma** y es una de las cosas por las que se paga.
- Tiempo de espera: `networkidle` con techo de 15 s.

## 9. Presupuesto de rastreo y límites

```rust
struct CrawlLimits {
    max_urls: Option<u64>,        // Free: 1_000 (forzado por EntitlementSource)
    max_depth: Option<u32>,
    max_duration: Option<Duration>,
    max_size_per_url: u64,
    include_patterns: Vec<String>,   // se compilan en `pattern.rs`, no se guardan compilados
    exclude_patterns: Vec<String>,
    follow_external: bool,        // por defecto: solo comprobar estado, no rastrear
    check_external: bool,         // esa comprobación de estado; activada por defecto
    max_external: u64,            // tope de externas comprobadas; 10_000 por defecto
    respect_nofollow: bool,
    concurrency_per_host: u8,     // 1..=20, por defecto 5
    user_agent: String,
    ignore_robots: bool,          // rastrea lo que robots.txt prohíbe, y lo marca
    http_basic_auth: Option<Credential>,  // #[serde(skip)]: nunca llega a config_json
}
```

**El límite del nivel Free se aplica en el core, no en la UI.** Al alcanzarlo, el rastreo termina
limpiamente con `status='done'`, marca `truncated=true` en `crawl_meta`, y **muestra todos los
hallazgos encontrados hasta ahí**. No se ocultan resultados: se limita la escala.

**La comprobación de externas es solo estado.** Una petición `HEAD` por URL externa única —con
un `GET` de respaldo si el servidor responde 405/501—, con una sola petición en vuelo por host
ajeno y un timeout más corto que el del rastreo: no se parsea, no se extraen enlaces, no se crea
fila en `pages`; solo se rellena el estado de la fila de `urls`, que es lo que necesita
`HTTP-404-EXTERNAL`. No se pide el `robots.txt` del host ajeno: comprobar que un enlace resuelve
es lo que hace el navegador cuando el visitante lo pulsa, y pedirlo casi duplicaría las
peticiones a terceros para poder decir menos. Las externas **no cuentan contra `max_urls`**, y
alcanzar `max_external` **no marca `crawl_meta.truncated`** —ese campo apaga las reglas de
`REQUIERE_GRAFO_COMPLETO`—: deja externas sin comprobar y el resumen dice cuántas.

### 9.bis Patrones de inclusión y exclusión (`pattern.rs`)

Expresiones regulares **sin anclar** sobre la URL completa normalizada, como en Screaming Frog:
una cadena literal (`/wp-admin/`) funciona como un «contiene» y los patrones de siempre
(`\?replytocom=`, `/page/\d+/`) valen tal cual. Se compilan **una vez** por rastreo —un patrón
inválido es un error antes de empezar, no un rastreo a medias— y el crate `regex` no tiene
*backtracking*, así que un patrón patológico no puede degenerar.

Reglas, aplicadas en `engine.rs` donde se decide encolar (enlaces, destinos de redirección,
URLs de sitemap y semillas):

- **`exclude` gana sobre `include`.** Es la convención de Screaming Frog y la única que permite
  «todo el blog menos los borradores».
- **Un `include` no vacío restringe**: solo se rastrea lo que case con alguno de sus patrones.
- **Lo excluido queda registrado**, con `crawl_state='excluded'` y `exclusion_reason='pattern'`:
  el resumen enseña cuántas URLs quedaron fuera por patrón, para que excluir media web por error
  se vea en la primera pantalla.
- **La semilla de un rastreo HTTP se rastrea siempre** (como la start URL de Screaming Frog):
  con `--include '/blog/'` y semilla en la raíz, filtrarla mataría el rastreo antes de descubrir
  nada. En `filesystem` y `list` las semillas son un conjunto descubierto o importado y **sí** se
  filtran — es el único sitio donde `audit --exclude` puede actuar.
- **Las URLs de sitemap siguen la misma regla** que los enlaces; la exclusión se registra con
  `in_sitemap=1`.

## 10. User-Agent

Por defecto, identificarse honestamente y de forma verificable:

```
CrawlForge/1.0 (+https://[dominio]/bot)
```

Permitir simular Googlebot para diagnóstico, con aviso de que solo debe usarse en sitios propios.
Nunca falsear un navegador por defecto: además de ser mala práctica, es exactamente el tipo de
comportamiento que provoca un rechazo en la revisión de la App Store.
