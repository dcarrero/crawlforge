# 01 — Arquitectura

## 1. Vista general

```
┌─────────────────────┐   ┌─────────────────────┐
│  apps/macos         │   │  apps/windows       │
│  SwiftUI + GRDB     │   │  WinUI 3 + C#       │
└──────┬───────┬──────┘   └──────┬───────┬──────┘
       │       │                 │       │
   FFI │       │ lectura     FFI │       │ lectura
 (~10 fn)      │ SQLite    (~10 fn)      │ SQLite
       │       │  R/O            │       │  R/O
┌──────▼───────┴─────────────────▼───────┴──────┐
│  crawlforge-ffi                                │
│  UniFFI (Swift)  ·  extern "C" (C#)            │
└──────┬─────────────────────────────────────────┘
       │
┌──────▼─────────────────────────────────────────┐
│  crawlforge-core                                │
│  planificador · fetch · parseo · almacén        │
│  ┌──────────────┐ ┌──────────────┐ ┌─────────┐ │
│  │ -rules       │ │ -adapters    │ │ -hub    │ │
│  └──────────────┘ └──────────────┘ └────┬────┘ │
└─────────────────────────────────────────┼──────┘
       │                                  │ sqlx
       ▼ escribe                          ▼
  ┌──────────┐                   ┌──────────────────┐
  │ crawl_N  │                   │ Postgres/MariaDB │
  │ .sqlite  │                   │  (agregados)     │
  └──────────┘                   └──────────────────┘
       ▲
       │ mismo core
┌──────┴──────────┐
│ crawlforge-cli  │  → CI, cron, uso interno
└─────────────────┘
```

## 2. SQLite como frontera FFI

**Este es el concepto central de la arquitectura.** Los datos no cruzan el puente FFI.

El core escribe el resultado del rastreo en un fichero SQLite. La UI abre **ese mismo fichero** en
solo lectura y lanza sus propias consultas. Por el FFI solo viajan órdenes de control y un
`ProgressSnapshot` pequeño.

Consecuencias, todas buenas:

- La superficie FFI queda en ~10 funciones y un callback. Escribirla a mano en C para Windows es
  media jornada de trabajo, no una dependencia de riesgo.
- Cada UI ordena, filtra y pagina con SQL nativo, aprovechando índices. Nada de reimplementar
  ordenación en Swift o C#.
- El core es testeable en Rust puro sin ninguna UI.
- La CLI usa exactamente el mismo almacén: los ficheros son intercambiables entre CLI y app.
- Un rastreo es **un fichero**. Se comprime y se envía a un cliente. Ventaja de UX enorme.

**Reglas:**
- La UI abre la conexión con `mode=ro` y `immutable=false` (el core puede estar escribiendo en WAL).
- Durante un rastreo activo, la UI refresca por polling cada 500 ms sobre la vista de progreso,
  no por notificación de fila.
- La UI **nunca** escribe en el fichero de rastreo. Preferencias y estado de vista van en su propio
  almacén (`UserDefaults` / `ApplicationData`).

## 3. Crates

### `crawlforge-core`
El motor. No conoce UI ni FFI. Expone:

```rust
pub struct Engine { /* pool tokio, config, almacén */ }

impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self>;
    pub fn start_crawl(&self, job: CrawlJob) -> Result<CrawlId>;
    pub fn pause(&self, id: CrawlId) -> Result<()>;
    pub fn resume(&self, id: CrawlId) -> Result<()>;
    pub fn cancel(&self, id: CrawlId) -> Result<()>;
    pub fn progress(&self, id: CrawlId) -> Result<Progress>;
    pub fn store_path(&self, id: CrawlId) -> Result<PathBuf>;
}
```

Submódulos: `frontier` (cola y planificación), `fetch` (HTTP + sistema de ficheros), `parse`
(extracción con `lol_html`), `store` (SQLite), `normalize` (canonicalización de URL), `robots`.

### `crawlforge-rules`
Crate aparte deliberadamente: las reglas son el producto y evolucionan a otro ritmo que el motor.

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;              // "SEO-TITLE-MISSING"
    fn severity(&self) -> Severity;
    fn category(&self) -> Category;
    fn min_tier(&self) -> Tier;                // Free | Pro | Agency
    fn evaluate(&self, ctx: &RuleContext) -> Vec<Issue>;
}
```

Dos modos de evaluación:
- **Por página** (`PageRule`): se evalúa durante el rastreo, en streaming. Barato.
- **Sobre el conjunto** (`SiteRule`): requiere el rastreo completo (duplicados, huérfanas,
  profundidad). Se ejecuta en una pasada final con SQL sobre el almacén.

### `crawlforge-adapters`
`trait SiteAdapter` y sus implementaciones. Ver `05-ADAPTADORES.md`.

### `crawlforge-hub`
**Único crate que usa `sqlx`.** Sincroniza agregados a Postgres o MariaDB. Opcional, nivel Pro.
Aislado para que una compilación sin la feature `hub` no arrastre las dependencias.

### `crawlforge-ffi`
Dos módulos hermanos sobre la misma lógica:
- `swift`: `#[uniffi::export]`, se compila a XCFramework.
- `c`: `#[no_mangle] extern "C"`, cabecera generada con `cbindgen`, se compila a `cdylib`.

**Ninguna función FFI es `async`.** El core gestiona sus propios hilos; el progreso se comunica por
callback registrado o por polling.

### `crawlforge-cli`
Interfaz de línea de comandos con `clap`. Es a la vez herramienta interna, producto del nivel Agency
y banco de pruebas.

## 3.bis Decisión sobre `spider-rs` — cerrada

**`spider` se usa como referencia, no como dependencia.** Escribimos nuestro propio planificador.
Decidido tras evaluarlo con el motor delante. **No se reabre.**

Es un crate maduro y rápido (2M descargas, MIT). Los motivos de no depender de él son cuatro:

1. **Cadencia de publicación.** 11 versiones entre el 23 de junio y el 23 de julio de 2026. Anclar
   el hot path del producto a una API que se mueve así convierte cada actualización en trabajo de
   mantenimiento sobre la pieza más crítica.
2. **Choca con la decisión cerrada #2.** `spider` trae su propio modelo de página y su propia
   gestión de resultados. Nosotros escribimos a SQLite por lotes desde un único hilo escritor, y
   el fichero *es* la frontera con la UI. Adaptar su salida a eso cuesta aproximadamente lo mismo
   que escribir el `frontier`, y nos deja con una capa de traducción que mantener para siempre.
3. **Necesitamos control fino del parseo.** El `PageAccumulator` de una sola pasada con `lol_html`
   (§5 de `03-MOTOR-CRAWL.md`) depende del orden de aparición de los elementos: primer `h1`,
   jerarquía de encabezados, `region` deducida del ancestro semántico, posición del enlace. Eso es
   nuestro, no delegable.
4. **Build de tienda.** Una superficie de dependencias amplia es riesgo directo en firma,
   notarización y sandbox, donde los problemas aparecen tarde y caros.

Lo que sí tomamos de él como referencia al escribir el planificador: su control de concurrencia y
su estrategia de caché.

## 4. Superficie FFI completa

Diez funciones. Si crece más allá de quince, es que estás pasando datos por el puente. Revisa §2.

```
engine_create(config_json: String) -> EngineHandle
engine_destroy(h)

crawl_start(h, job_json: String) -> String        // devuelve crawl_id (uuid)
crawl_pause(h, crawl_id)
crawl_resume(h, crawl_id)
crawl_cancel(h, crawl_id)
crawl_progress(h, crawl_id) -> ProgressSnapshot   // struct plano, ~12 campos
crawl_store_path(h, crawl_id) -> String

crawl_diff(h, path_a, path_b, out_path)           // genera un SQLite de diff
export(h, crawl_id, format, out_path)             // csv | xlsx | parquet | html

set_progress_callback(h, cb)                      // opcional, alternativa al polling
last_error(h) -> String
```

`ProgressSnapshot`: `crawl_id`, `state`, `urls_discovered`, `urls_fetched`, `urls_pending`,
`urls_errored`, `issues_found`, `bytes_downloaded`, `elapsed_ms`, `eta_ms`, `current_rate_per_s`,
`current_url`.

## 5. Concurrencia

- Un runtime `tokio` multi-hilo por `Engine`.
- **Un limitador por host**, no global — el `Throttle` propio de `throttle.rs`, consultado con
  el host de cada URL. (`governor` se descartó: modela un ritmo, no conexiones simultáneas, y no
  trae el freno adaptativo; razonamiento en la cabecera de `throttle.rs`.) Un rastreo de un solo
  dominio va limitado por la
  concurrencia configurada para ese host (por defecto 5, máximo 20); un rastreo de cartera con 20
  dominios puede ir a 100 peticiones en vuelo sin castigar a ningún servidor.
- Respeto obligatorio de `Crawl-delay` de robots.txt cuando existe.
- **Backoff exponencial con jitter** ante 429 y 5xx. Tres reintentos, luego se marca la URL como
  errónea y se continúa. Un fallo nunca aborta el rastreo.
- Escrituras a SQLite **por lotes** desde un único hilo escritor que consume un canal `mpsc`.
  Nunca escribas desde los workers: la contención de WAL con 20 escritores es peor que el propio
  rastreo. Lote por defecto: 200 URLs o 2 segundos, lo que llegue antes.
- La cola (`frontier`) vive en memoria con desbordamiento a SQLite si supera 100.000 URLs pendientes.

## 6. Los dos únicos `#[cfg]` de plataforma

Todo lo demás debe ser idéntico entre tienda y build directo.

```rust
#[cfg(feature = "render_cdp")]      // build directo: chromiumoxide contra Chrome del sistema
#[cfg(feature = "render_webview")]  // tienda: WKWebView / WebView2, in-process

#[cfg(feature = "scheduler_daemon")] // build directo: servicio de fondo
#[cfg(feature = "scheduler_in_app")] // tienda: app abierta o login item
```

Si te encuentras añadiendo un tercer `#[cfg]` de plataforma, párate y consulta: probablemente hay
una solución in-process que mantiene la paridad.

## 7. Manejo de errores

- `crawlforge-core` define `CoreError` con `thiserror`. Variantes explícitas, nunca `Box<dyn Error>`.
- Errores de red por URL **no son errores del rastreo**: se guardan en la fila de la URL y el
  rastreo sigue.
- El FFI nunca hace panic. Todo `Result` se traduce a un código de error + `last_error()`. Un panic
  cruzando el límite FFI es comportamiento indefinido: envuelve el punto de entrada en
  `catch_unwind`.

## 8. Rendimiento — objetivos medibles

Se verifican con el banco de pruebas y se convierten en tests de regresión.

| Métrica | Objetivo | Referencia |
|---|---|---|
| Rastreo HTTP, sitio de 10k URLs | > 150 URL/s | Screaming Frog: ~50-80 |
| Rastreo de sistema de ficheros (`dist/`) | > 2.000 URL/s | No tiene equivalente |
| RAM con 500k URLs rastreadas | < 500 MB | SF necesita configurar el heap de la JVM |
| Apertura de tabla con 500k filas en la UI | < 300 ms | |
| Scroll de la tabla | 60 fps sostenidos | |
| Tamaño del instalador | < 40 MB | |
