# 05 — Adaptadores de plataforma

> Versión en inglés: [`../05-ADAPTADORES.md`](../05-ADAPTADORES.md) — **la inglesa es la que manda.** Esta
> traducción puede ir por detrás; si las dos discrepan, la buena es la otra.

## 1. Por qué existen

Un rastreador genérico ve lo que ve un navegador. Un adaptador **cruza el rastreo con la fuente de
la verdad de la plataforma**, y esa comparación produce hallazgos imposibles de obtener de otro modo:

> WordPress dice que hay 1.240 posts publicados. El rastreo alcanzó 1.187.
> **53 posts publicados no están enlazados desde ningún sitio.**

Ninguna herramienta generalista hará esto: hace falta conocer el CMS por dentro.
Es el foso más profundo del producto.

## 2. El trait

```rust
#[async_trait]
pub trait SiteAdapter: Send + Sync {
    fn id(&self) -> &'static str;

    /// Se ejecuta antes del rastreo: aporta semillas y contexto.
    async fn discover(&self, ctx: &AdapterContext) -> Result<Discovery>;

    /// Enriquece una página durante el rastreo (streaming, debe ser barato).
    fn enrich_page(&self, page: &mut PageData, doc: &FetchedDoc);

    /// Se ejecuta al terminar: cruza entidades con lo rastreado.
    async fn reconcile(&self, store: &Store) -> Result<Vec<Issue>>;

    fn rules(&self) -> Vec<Box<dyn Rule>>;
}

pub struct Discovery {
    pub seeds: Vec<Url>,
    pub entities: Vec<AdapterEntity>,   // → tabla adapter_entities
    pub hints: PlatformHints,           // versión, plugins SEO, etc.
}
```

**Detección automática:** antes de rastrear, se hace una petición a la raíz y se aplican heurísticas
(cabecera `Link: <.../wp-json/>`, `/wp-content/` en los assets, `generator` meta, presencia de
`_astro/` en las rutas). Si se detecta plataforma, se ofrece activar el adaptador. Nunca se activa
sin confirmación del usuario.

---

## 3. Adaptador WordPress

### 3.1 Fuentes de datos, por orden de preferencia

| Fuente | Requiere | Aporta |
|---|---|---|
| REST API pública (`/wp-json/wp/v2/`) | Nada | Posts, páginas, taxonomías, medios |
| REST API autenticada | Application Password | Borradores, privados, metadatos SEO |
| Sitemap de Yoast / RankMath | Nada | Vista del plugin SEO sobre qué debe indexarse |
| SSH + WP-CLI (`russh`) | Credenciales | `wp_postmeta` directo, plugins, opciones |

**Regla de sandbox:** SSH **siempre** con `russh` in-process. Nunca lanzar `/usr/bin/ssh`, ni
siquiera en el build directo, para mantener un solo camino de código. Ver `docs/CONVENTIONS.md §2`.

### 3.2 Qué se recopila en `discover()`

```
GET /wp-json/wp/v2/posts?per_page=100&_fields=id,link,status,date,modified,title
GET /wp-json/wp/v2/pages?...
GET /wp-json/wp/v2/categories?per_page=100&_fields=id,link,count,name
GET /wp-json/wp/v2/tags?...
GET /wp-json/wp/v2/media?...           (para detectar páginas de adjunto)
GET /wp-json/                          (plugins expuestos, versión, namespaces)
```

Paginar con la cabecera `X-WP-TotalPages`. Respetar rate limit propio: la REST API de WordPress es
mucho más frágil que el frontend cacheado. **Máximo 2 peticiones concurrentes a `/wp-json/`.**

Detección del plugin SEO: si existe el namespace `yoast/v1` o `rankmath/v1`, o si aparecen los
comentarios HTML característicos en el `<head>`.

Inventario técnico sin autenticación: versiones de plugin y tema deducidas de los parámetros `?ver=`
de los assets encolados. Es información pública y sorprendentemente útil.

### 3.3 Hallazgos característicos

Las reglas están en `04-CATALOGO-REGLAS.md §10`. Los que más valor aportan en la práctica:

1. **Posts huérfanos** — publicados pero sin enlace interno entrante. Se obtiene con
   `adapter_entities` donde `url_id IS NULL`, o `url_id` presente pero sin filas en `links`.
2. **Páginas de adjunto indexables** — WordPress crea una página por cada imagen subida. Si están
   indexables, es contenido basura a escala. Detección: entidades `media` con URL rastreable que
   devuelve 200 y HTML.
3. **Archivos anémicos** — categorías y etiquetas con `count <= 1`. En blogs con años de historia
   suelen ser cientos.
4. **Incoherencia sitemap ↔ contenido** — el sitemap de Yoast dice X, la REST API dice Y.
   Casi siempre indica configuración de indexación mal entendida.
5. **Trampas de paginación** — `/page/N/` que sigue devolviendo 200 más allá del número real de
   páginas. Genera rastreo infinito y desperdicia presupuesto de rastreo.

### 3.4 Modo cartera de WordPress

El caso de uso propio: 100+ blogs. Un proyecto puede declarar múltiples sitios WordPress con
credenciales distintas, y el panel agrega los hallazgos por sitio y por regla. Es el cruce entre el
adaptador y la funcionalidad de cartera.

---

## 4. Adaptador Astro / sitios estáticos

### 4.1 El modo `filesystem`

Es el diferenciador más limpio del producto. En vez de rastrear por HTTP, se lee `dist/`
directamente:

```bash
crawlforge audit ./dist --base https://ejemplo.com \
  --adapter astro \
  --compare-with ./ultimo-crawl-produccion.sqlite \
  --fail-on new-404,canonical-broken,orphan-page
```

**Resolución de rutas.** Hay que emular lo que hará el servidor. Orden de intento para la ruta `/x`:

```
dist/x/index.html   ← Astro con build.format = 'directory' (por defecto)
dist/x.html         ← build.format = 'file'
dist/x              ← fichero literal (assets)
dist/404.html       ← fallback
```

Leer `dist/_routes.json` o la configuración del adaptador si está presente para afinar. Los enlaces
relativos se resuelven contra la ruta del fichero, no contra `--base`; `--base` solo se usa para
reescribir las URLs absolutas del informe.

**Rendimiento:** sin red, sin rate limit, sin robots. Objetivo > 2.000 URL/s. Un sitio de 5.000
páginas se audita en tres segundos, dentro del pipeline de CI.

### 4.2 Hallazgos característicos

- **Enlaces que solo existen tras hidratar.** Astro genera islas; un enlace dentro de un componente
  con `client:only` no está en el HTML estático y por tanto es invisible para Google. Se detecta
  comparando el HTML de `dist/` con el HTML renderizado (Pro, previsto). Es uno de los errores más
  costosos y menos diagnosticados en sitios Astro.
- **Rutas generadas ausentes del sitemap** — `@astrojs/sitemap` mal configurado, con
  `filter` o `exclude` demasiado agresivos.
- **Entradas de colección sin ruta** — contenido en `src/content/` sin página generada.
  Requiere leer `src/content/config.ts` o el manifiesto de build.
- **Enlaces relativos rotos** — se detectan con certeza absoluta al tener el sistema de ficheros
  delante, sin falsos positivos por redirecciones del servidor.

### 4.3 Integración en CI

```yaml
# .github/workflows/seo.yml
- run: npm run build
- run: |
    crawlforge audit ./dist \
      --base ${{ vars.SITE_URL }} \
      --adapter astro \
      --baseline .crawlforge/baseline.sqlite \
      --fail-on new-404,canonical-broken,indexability-lost \
      --report-md $GITHUB_STEP_SUMMARY
```

El *baseline* es un fichero SQLite versionado en el repositorio o descargado del último rastreo de
producción. El comando devuelve código de salida distinto de cero si aparecen regresiones.

Esto no es solo producto: es infraestructura útil de inmediato para ejemplo.me, otro proyecto y otro proyecto.

### 4.4 Generalización

El mismo adaptador cubre Hugo (`public/`), Eleventy (`_site/`), Next.js en export estático (`out/`)
y Jekyll (`_site/`). Detección por la presencia de ficheros marcadores. Preséntalo como
"sitios estáticos", con Astro como caso mejor soportado.

---

## 5. Adaptadores futuros

No se implementan, pero el trait debe admitirlos sin refactor:

| Adaptador | Fuente | Interés |
|---|---|---|
| Laravel | Rutas de `artisan route:list` | Encaja con un stack PHP habitual |
| Shopify | Admin API | Alto valor comercial, e-commerce |
| Accesibilidad EAA | `axe-core` vía webview | Segundo producto sobre el mismo motor |

El adaptador de accesibilidad no encaja del todo en `SiteAdapter` (no descubre entidades, solo
enriquece y evalúa). Cuando llegue el momento, valora extraer un trait hermano `PageAnalyzer` en
lugar de forzar la abstracción. No lo hagas antes de necesitarlo.
