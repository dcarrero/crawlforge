//! Orquestación del rastreo: el ciclo de vida de `docs/03-MOTOR-CRAWL.md §2`.
//!
//! ```text
//! semillas → normalizar → ¿vista ya? → ¿permitida? → encolar
//!          → fetch → parsear → extraer enlaces → evaluar reglas de página
//!          → lote al hilo escritor
//!          → [cola agotada] pasada final: reglas de conjunto, enlaces entrantes, VACUUM
//! ```

use crate::error::Result;
use crate::fetch::{FetchedDoc, Fetcher, FilesystemFetcher, HttpFetcher};
use crate::frontier::{DiscoverySource, ExclusionReason, Frontier, QueuedUrl};
use crate::job::{evaluate_indexability, CrawlJob, CrawlMode, IndexabilityInput};
use crate::normalize::{self, NormalizePolicy, NormalizedUrl};
use crate::parse::{self, ParsedPage};
use crate::robots::{HostRules, RobotsCache};
use crate::store;
use crate::writer::{
    self, CrawlResult, CrawlState, ImageRow, IssueRow, LinkRow, PageRow, ResourceRow, UrlRow,
};
use crawlforge_rules::{ImageView, LinkView, PageContext, PageRule};
use rusqlite::Connection;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use url::Url;

/// Métricas del rastreo. Sin números no hay decisión.
#[derive(Debug, Clone, Default)]
pub struct CrawlMetrics {
    pub urls_discovered: u64,
    pub urls_fetched: u64,
    pub urls_errored: u64,
    pub urls_excluded: u64,
    pub issues_found: u64,
    pub bytes_downloaded: u64,
    pub elapsed: Duration,
    /// Pico de memoria residente del proceso, en bytes.
    pub peak_rss_bytes: u64,

    /// Suma de los tiempos de respuesta de todas las peticiones.
    ///
    /// Es lo que permite separar «el crawler es lento» de «el servidor es lento», que resultó
    /// ser la diferencia entre un defecto real y un criterio mal formulado.
    pub total_response_time: Duration,
    /// Concurrencia media realmente empleada. Puede ser menor que la configurada si el freno
    /// adaptativo la redujo, o si la cola se quedó sin trabajo.
    pub effective_concurrency: f64,
    /// Elementos escritos: páginas, enlaces e imágenes.
    ///
    /// El trabajo de un rastreo no se mide en URLs: una página con doce enlaces y otra con
    /// ciento veintitrés cuestan lo mismo de descargar y siete veces distinto de procesar.
    pub elements_written: u64,
    /// Documentos HTML efectivamente parseados.
    ///
    /// No coincide con `urls_fetched`: un rastreo pide también las imágenes, hojas de estilo
    /// y scripts internos —URLs que se descargan para conocer su estado y su peso, quedan en
    /// la tabla `resources`, y **no** se parsean como páginas ni aportan enlaces—.
    pub pages_parsed: u64,
    /// URLs externas cuyo estado se comprobó (`CrawlLimits::check_external`): una sonda de
    /// solo cabeceras por URL única. No cuentan en [`Self::urls_fetched`] ni contra
    /// `max_urls`: el presupuesto del rastreo es del sitio del usuario.
    pub externals_checked: u64,
    /// URLs externas descubiertas por encima de `max_external`: quedaron registradas sin
    /// estado. El resumen tiene que decir cuántas — un tope que trunca en silencio hace que
    /// el informe parezca completo cuando no lo está.
    pub externals_unchecked: u64,
    /// URLs externas descubiertas por encima de `max_external_urls`: **ni siquiera se
    /// registraron**. Es un aviso distinto del de arriba —aquellas están en el fichero sin
    /// estado, estas no están— y el resumen tiene que decirlo por el mismo motivo.
    pub externals_unregistered: u64,
    /// Duración del bucle de rastreo, sin el descubrimiento de sitemaps ni la pasada final.
    ///
    /// La eficiencia de paralelismo se calcula sobre esto y no sobre [`Self::elapsed`]: el suelo
    /// teórico solo contempla las peticiones de páginas, así que compararlo con un tiempo que
    /// además incluye leer sitemaps y ejecutar las reglas de conjunto mide dos cosas distintas
    /// y penaliza al motor por trabajo que no es esperar a la red.
    pub crawl_loop: Duration,
    /// Tiempo de preparación y cierre: sitemaps, enlaces entrantes, reglas de conjunto.
    pub setup_and_teardown: Duration,
}

impl CrawlMetrics {
    /// URLs por segundo.
    ///
    /// Útil para informar, **no para juzgar el motor**: en un rastreo HTTP la manda la latencia
    /// del servidor. Ver [`Self::parallelism_efficiency`].
    pub fn urls_per_second(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs <= 0.0 {
            return 0.0;
        }
        self.urls_fetched as f64 / secs
    }

    /// Tiempo mínimo en el que el rastreo podría haberse hecho, dada la latencia observada y
    /// la concurrencia empleada.
    ///
    /// Es el suelo físico del **bucle de rastreo**: ni el mejor crawler del mundo baja de aquí
    /// sin abrir más conexiones.
    pub fn theoretical_floor(&self) -> Duration {
        if self.effective_concurrency <= 0.0 {
            return self.total_response_time;
        }
        self.total_response_time.div_f64(self.effective_concurrency)
    }

    /// Qué fracción del techo teórico aprovecha el motor, de 0 a 1.
    ///
    /// **Esta es la métrica que juzga al crawler**, y no las URL/s. Un valor bajo significa
    /// paralelismo desperdiciado: esperas innecesarias, trabajo serializado o un pool que no se
    /// rellena. Es exactamente lo que delataba el bucle por tandas, que daba 0,18 mientras las
    /// URL/s parecían «normales para ese servidor».
    ///
    /// Puede superar 1,0 cuando parte del trabajo no pasa por la red (respuestas cacheadas,
    /// modo `filesystem`); en ese caso la métrica no aplica y conviene mirar
    /// [`Self::elements_per_second`].
    pub fn parallelism_efficiency(&self) -> f64 {
        let real = self.crawl_loop.as_secs_f64();
        let floor = self.theoretical_floor().as_secs_f64();
        if real <= 0.0 || floor <= 0.0 {
            return 0.0;
        }
        floor / real
    }

    /// Documentos HTML parseados por segundo.
    ///
    /// Acompaña a [`Self::elements_per_second`] para que no se pueda aprobar la puerta
    /// procesando muchos enlaces y pocas páginas.
    pub fn pages_per_second(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs <= 0.0 {
            return 0.0;
        }
        self.pages_parsed as f64 / secs
    }

    /// Elementos escritos por segundo: páginas, enlaces e imágenes.
    ///
    /// Es la medida honesta del trabajo del motor cuando no hay red de por medio. Las URL/s del
    /// modo `filesystem` varían siete veces según cuántos enlaces tenga cada página; los
    /// elementos por segundo se mantienen estables porque miden lo que de verdad se hace.
    pub fn elements_per_second(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs <= 0.0 {
            return 0.0;
        }
        self.elements_written as f64 / secs
    }

    pub fn peak_rss_mb(&self) -> f64 {
        self.peak_rss_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// Fase en la que está un rastreo. Existe para que la UI pueda decir «esto no se ha colgado,
/// está leyendo sitemaps», que es distinto de enseñar un contador de URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrawlPhase {
    /// Descubrimiento de sitemaps: puede tardar y no produce URLs rastreadas todavía.
    Sitemaps,
    /// El bucle de rastreo propiamente dicho.
    Crawl,
    /// Pasada final: enlaces entrantes, reglas de conjunto, cierre del fichero.
    Finalize,
}

/// Instantánea del avance de un rastreo, tal como se entrega al observador.
#[derive(Debug, Clone)]
pub struct CrawlProgress {
    pub phase: CrawlPhase,
    pub urls_fetched: u64,
    pub urls_discovered: u64,
    /// URLs **del sitio** pendientes: las encoladas más las que están en vuelo ahora mismo.
    ///
    /// Las sondas de enlaces externos no cuentan aquí, y antes sí: iban sumadas sin etiqueta,
    /// así que un rastreo que había terminado su sitio y solo esperaba a hosts ajenos enseñaba
    /// miles de «queued» que no eran suyos, y al vaciarse el resto la cifra se quedaba quieta
    /// sin explicación. Van aparte, en `externals_pending`.
    pub urls_queued: u64,
    /// Sondas de estado de enlaces externos por resolver: en cola más en vuelo.
    ///
    /// Es el dato que separa «esto se ha colgado» de «esto está esperando a servidores de
    /// otros». Un host ajeno lento se paga entero —una petición en vuelo por host, por
    /// educación— y esa espera es la última parte de muchos rastreos.
    pub externals_pending: u64,
    pub urls_errored: u64,
    pub issues_found: u64,
    /// Tiempo transcurrido **en la fase actual**. Para la fase de rastreo, dividir
    /// `urls_fetched` entre esto da las URL/s reales del bucle, sin contar los sitemaps.
    pub elapsed: Duration,
    /// En [`CrawlPhase::Finalize`], qué se está calculando ahora mismo. `None` en el resto.
    ///
    /// La pasada final era la única fase muda, y puede durar más que el propio rastreo: sobre
    /// un sitio de 487.621 URLs se midieron **más de ocho horas** en una sola regla de conjunto
    /// (2026-08-02, antes del índice de la migración 006). Sin esto, el usuario ve una barra
    /// terminada, un proceso al 100% de CPU y ninguna palabra, y acaba matándolo — que además
    /// es cómo se pierde el volcado del WAL.
    pub step: Option<FinalizeStep>,
}

/// Un paso de la pasada final: su nombre y por dónde va la cuenta.
#[derive(Debug, Clone)]
pub struct FinalizeStep {
    /// El ID de la regla de conjunto, o el nombre del paso previo.
    pub name: &'static str,
    /// Empezando en 1. Junto con `total`, da «3 de 15».
    pub index: u32,
    pub total: u32,
}

/// Observador de progreso. Es una función y no un trait de UI a propósito: el core no conoce
/// terminales ni barras; solo entrega números y quien escucha decide qué pintar.
pub type ProgressCallback = Arc<dyn Fn(&CrawlProgress) + Send + Sync>;

/// Cada cuánto se emite progreso como máximo.
///
/// Se muestrea por tiempo y no por URL: el modo `filesystem` procesa decenas de miles de
/// elementos por segundo, y formatear una cadena por URL costaría más que el propio parseo.
/// A este ritmo el observador recibe ~7 instantáneas por segundo, que es más de lo que un
/// terminal necesita y menos de lo que el bucle nota.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

/// Emisor de progreso: muestrea por tiempo y no hace nada si nadie escucha.
struct ProgressEmitter {
    callback: Option<ProgressCallback>,
    last_emit: Instant,
    phase_started: Instant,
    phase: CrawlPhase,
    step: Option<FinalizeStep>,
}

impl ProgressEmitter {
    fn new(callback: Option<ProgressCallback>) -> Self {
        let now = Instant::now();
        Self { callback, last_emit: now, phase_started: now, phase: CrawlPhase::Crawl, step: None }
    }

    /// Cambia de fase y lo anuncia inmediatamente, sin esperar al intervalo.
    fn enter_phase(&mut self, phase: CrawlPhase, metrics: &CrawlMetrics, queued: u64) {
        self.phase = phase;
        self.phase_started = Instant::now();
        self.emit(metrics, queued, 0);
    }

    /// Emite si ha pasado el intervalo. Es la llamada del camino caliente: cuando no hay
    /// observador cuesta una comparación, y cuando lo hay, una lectura de reloj.
    fn tick(&mut self, metrics: &CrawlMetrics, queued: u64, externals: u64) {
        if self.callback.is_none() || self.last_emit.elapsed() < PROGRESS_INTERVAL {
            return;
        }
        self.emit(metrics, queued, externals);
    }

    /// Anuncia un paso de la pasada final **sin esperar al intervalo**: son pasos largos y de
    /// número conocido, así que cada uno merece su línea en cuanto empieza.
    fn enter_step(&mut self, name: &'static str, index: u32, total: u32, metrics: &CrawlMetrics) {
        if self.callback.is_none() {
            return;
        }
        self.step = Some(FinalizeStep { name, index, total });
        self.emit(metrics, 0, 0);
    }

    fn emit(&mut self, metrics: &CrawlMetrics, queued: u64, externals: u64) {
        let Some(cb) = &self.callback else { return };
        self.last_emit = Instant::now();
        cb(&CrawlProgress {
            phase: self.phase,
            urls_fetched: metrics.urls_fetched,
            urls_discovered: metrics.urls_discovered,
            urls_queued: queued,
            externals_pending: externals,
            urls_errored: metrics.urls_errored,
            issues_found: metrics.issues_found,
            elapsed: self.phase_started.elapsed(),
            step: self.step.clone(),
        });
    }
}

/// Por qué el conjunto rastreado no es el sitio entero.
///
/// Tres motivos son cortes de verdad —un límite que el rastreo alcanzó— y el cuarto es
/// estructural: [`Self::ListMode`] no significa «se cortó», significa que un rastreo en modo
/// lista solo ve las URLs que se le dieron, así que ninguna página tiene a sus enlazadores y
/// el grafo de enlaces está incompleto **por definición**. Comparten tipo y columna
/// (`crawl_meta.truncated`) porque comparten la única consecuencia que importa al motor: las
/// reglas de `REQUIERE_GRAFO_COMPLETO` no pueden afirmar lo que afirman, y `diff` no puede
/// afirmar ausencias. Quien lo enseñe al usuario debe distinguirlos: decir «tu rastreo se
/// cortó» de una lista que se auditó entera sería mentir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationReason {
    MaxUrls,
    MaxDepth,
    MaxDuration,
    /// Modo lista: el grafo está incompleto por construcción, no por un corte.
    ListMode,
}

impl TruncationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxUrls => "max_urls",
            Self::MaxDepth => "max_depth",
            Self::MaxDuration => "max_duration",
            Self::ListMode => "list_mode",
        }
    }
}

/// Resultado de un rastreo completo.
#[derive(Debug)]
pub struct CrawlOutcome {
    pub crawl_id: String,
    pub store_path: std::path::PathBuf,
    pub metrics: CrawlMetrics,
    pub truncated: Option<TruncationReason>,
    /// El rastreo se cortó a petición ([`CancelSignal`]) antes de terminar. El fichero queda
    /// con `crawl_meta.status = 'paused'`, todo lo rastreado a salvo y las URLs no visitadas
    /// como `pending`: es exactamente el estado que [`resume`] relee. Si el corte llegó
    /// durante la pasada final, sus hallazgos de conjunto quedaron a medias y la reanudación
    /// los recalcula desde cero.
    pub interrupted: bool,
    /// El cierre no pudo sacar el fichero del modo WAL: otra conexión —un visor, un `report`,
    /// la UI— lo mantenía abierto. El rastreo está completo y es funcional, pero los ficheros
    /// `-wal`/`-shm` de al lado forman parte de él: copiar el `.sqlite` suelto puede perder
    /// datos, y quien enseña este resultado tiene que decirlo. Ver `store::FinalizeOutcome`.
    pub wal_kept: bool,
}

/// Señal de cancelación cooperativa.
///
/// Quien lanza el rastreo conserva el `Sender` y emite `true` para pedir el corte (un Ctrl+C
/// en la CLI, el botón de parar en una UI). El motor la consulta en la cabecera del bucle y
/// dentro de las esperas en vuelo, igual que hace con el presupuesto de tiempo: el corte no
/// espera a que termine un `Crawl-delay`. Es un `watch` de tokio y no un callback porque tiene
/// que poder despertar a un `select!`.
pub type CancelSignal = tokio::sync::watch::Receiver<bool>;

/// Muestreador de memoria residente.
///
/// `sysinfo` devuelve el uso **actual**, no el máximo histórico, así que una sola lectura al
/// terminar no mide el pico: mide lo que quedaba después del `VACUUM`. El criterio de la puerta
/// El criterio de memoria habla del máximo durante el rastreo, así que hay que muestrear
/// mientras corre y quedarse con el mayor valor visto.
///
/// El `System` se reutiliza entre muestras: construirlo cuesta bastante más que la lectura.
struct RssSampler {
    system: sysinfo::System,
    pid: sysinfo::Pid,
    peak: u64,
    /// Cuándo se tomó la última muestra de verdad: la puerta de [`Self::sample_if_due`].
    last_sample: Instant,
    /// Muestras reales tomadas. Existe para que un test pueda afirmar que el muestreo del
    /// camino caliente es por tiempo y no por iteración.
    samples_taken: u64,
}

/// Cada cuánto se muestrea la memoria residente como máximo en el camino caliente.
///
/// Muestrear son syscalls de `sysinfo` (5-20 µs), y el bucle de rastreo une **una tarea por
/// iteración**: muestrear ahí era pagar ese peaje por URL — decenas de miles de veces por
/// segundo en modo `filesystem`. Por tiempo, como ya hace el progreso con
/// [`PROGRESS_INTERVAL`], son 4 muestras por segundo: el pico de RSS lo dominan el almacén y
/// los búferes del proceso, que no aparecen y desaparecen en 250 ms, así que no se pierde.
const RSS_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

impl RssSampler {
    fn new() -> Self {
        Self {
            system: sysinfo::System::new(),
            pid: sysinfo::Pid::from_u32(std::process::id()),
            peak: 0,
            last_sample: Instant::now(),
            samples_taken: 0,
        }
    }

    /// Toma una muestra y devuelve el máximo visto hasta ahora.
    fn sample(&mut self) -> u64 {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
        self.last_sample = Instant::now();
        self.samples_taken += 1;
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::new().with_memory(),
        );
        if let Some(current) = self.system.process(self.pid).map(|p| p.memory()) {
            self.peak = self.peak.max(current);
        }
        self.peak
    }

    /// Como [`Self::sample`], pero solo si ha pasado [`RSS_SAMPLE_INTERVAL`]: la llamada del
    /// camino caliente. Cuando no toca, devuelve el pico ya conocido sin tocar el sistema.
    fn sample_if_due(&mut self) -> u64 {
        if self.last_sample.elapsed() < RSS_SAMPLE_INTERVAL {
            return self.peak;
        }
        self.sample()
    }
}

/// Media corrida de la concurrencia observada en el bucle.
///
/// Antes era un `Vec<f64>` con una muestra por iteración para promediar al final: la única
/// estructura del bucle que crecía linealmente con el rastreo sin necesitarlo — 8 bytes por
/// URL son 80 MB a 10 millones de URLs. Una suma corrida y un contador dan exactamente el
/// mismo promedio (los sumandos son enteros pequeños: la suma en `f64` es exacta hasta 2^53)
/// ocupando 16 bytes constantes.
struct ConcurrencyMeter {
    sum: f64,
    samples: u64,
}

impl ConcurrencyMeter {
    fn new() -> Self {
        Self { sum: 0.0, samples: 0 }
    }

    fn record(&mut self, in_flight: usize) {
        self.sum += in_flight as f64;
        self.samples += 1;
    }

    /// El promedio observado, o `fallback` si el bucle no llegó a medir nada.
    fn average(&self, fallback: f64) -> f64 {
        if self.samples == 0 {
            fallback
        } else {
            self.sum / self.samples as f64
        }
    }
}

/// Ejecuta un rastreo completo y devuelve el fichero resultante.
pub async fn run(job: CrawlJob, store_path: &Path) -> Result<CrawlOutcome> {
    run_observed(job, store_path, None).await
}

/// Como [`run`], pero entregando instantáneas de avance a un observador.
///
/// El observador se llama desde el bucle del motor, muestreado por tiempo
/// ([`PROGRESS_INTERVAL`]): debe volver rápido y no bloquear. Es la vía prevista para
/// cualquier UI —terminal, Swift o C#—; el core no sabe qué hay al otro lado.
pub async fn run_observed(
    job: CrawlJob,
    store_path: &Path,
    progress: Option<ProgressCallback>,
) -> Result<CrawlOutcome> {
    run_cancellable(job, store_path, progress, None).await
}

/// Como [`run_observed`], y además interrumpible con una [`CancelSignal`].
///
/// Al recibir la señal, el motor deja de despachar, cancela lo que esté en vuelo —cuyas URLs
/// ya están escritas como `pending`—, vacía el hilo escritor y marca el fichero como
/// `status = 'paused'`. Todo lo rastreado queda a salvo y [`resume`] continúa desde ahí.
pub async fn run_cancellable(
    job: CrawlJob,
    store_path: &Path,
    progress: Option<ProgressCallback>,
    cancel: Option<CancelSignal>,
) -> Result<CrawlOutcome> {
    dispatch(job, store_path, RunControls { progress, cancel, resume: None, lookup: None }).await
}

/// Reanuda un rastreo interrumpido a partir de su fichero.
///
/// Es la otra mitad del mecanismo que `03-MOTOR-CRAWL.md §7` define: las URLs descubiertas y
/// no visitadas se escriben como `crawl_state='pending'` **antes** de rastrearlas, así que
/// reanudar es releer esa tabla. Se recargan las pendientes en el frontier —con su `depth`
/// guardado, para conservar el orden BFS—, se marcan como vistas las ya resueltas y el bucle
/// continúa donde se quedó.
///
/// **La configuración que manda es la del rastreo original**, guardada íntegra en
/// `crawl_meta.config_json`. No se aceptan flags nuevos: mezclar dos configuraciones en un
/// mismo fichero haría que ni el resultado ni el propio `config_json` fueran de fiar, y la
/// promesa de esta función es que reanudar da lo mismo que no haber parado.
///
/// No se puede reanudar: un rastreo terminado (`status='done'` — se rastrea de nuevo, no se
/// reanuda), un fichero de otra versión de esquema (la mitad vieja y la mitad nueva del
/// rastreo no serían comparables) ni uno cuya configuración guardada no se pueda leer. En los
/// tres casos el error es [`crate::CoreError::NotResumable`] con el motivo.
pub async fn resume(store_path: &Path) -> Result<CrawlOutcome> {
    resume_cancellable(store_path, None, None).await
}

/// Como [`resume`], con observador de progreso y señal de cancelación: una reanudación
/// también puede interrumpirse, y volver a reanudarse después.
pub async fn resume_cancellable(
    store_path: &Path,
    progress: Option<ProgressCallback>,
    cancel: Option<CancelSignal>,
) -> Result<CrawlOutcome> {
    resume_with_auth(store_path, progress, cancel, None).await
}

/// Como [`resume_cancellable`], reponiendo además la credencial de autenticación básica.
///
/// Existe porque la credencial **no está en el fichero**: `config_json` la omite a propósito
/// (ver [`crate::job::HttpBasicAuth`]), así que un rastreo interrumpido de un staging
/// protegido solo puede continuar si quien reanuda la vuelve a dar en su sesión — el mismo
/// principio que `ignore_robots`, que tampoco se hereda de un fichero. Se acota, como
/// siempre, al host de la semilla guardada; para un rastreo sin credencial es exactamente
/// [`resume_cancellable`].
pub async fn resume_with_auth(
    store_path: &Path,
    progress: Option<ProgressCallback>,
    cancel: Option<CancelSignal>,
    auth: Option<crate::job::HttpBasicAuth>,
) -> Result<CrawlOutcome> {
    resume_inner(store_path, progress, cancel, auth, None).await
}

/// Como [`resume_with_auth`], con resolutor de nombres inyectado. **Solo para tests**: ver
/// [`run_http_with_lookup`].
#[doc(hidden)]
pub async fn resume_with_lookup(
    store_path: &Path,
    lookup: Arc<dyn crate::dns::Lookup>,
) -> Result<CrawlOutcome> {
    resume_inner(store_path, None, None, None, Some(lookup)).await
}

async fn resume_inner(
    store_path: &Path,
    progress: Option<ProgressCallback>,
    cancel: Option<CancelSignal>,
    auth: Option<crate::job::HttpBasicAuth>,
    lookup: Option<Arc<dyn crate::dns::Lookup>>,
) -> Result<CrawlOutcome> {
    let (mut job, setup) = read_resume_plan(store_path)?;
    job.limits.http_basic_auth = auth;
    dispatch(job, store_path, RunControls { progress, cancel, resume: Some(setup), lookup }).await
}

/// Lo que acompaña a un rastreo además del trabajo: observador, cancelación y, si es una
/// reanudación, el estado recargado del fichero.
#[derive(Default)]
struct RunControls {
    progress: Option<ProgressCallback>,
    cancel: Option<CancelSignal>,
    resume: Option<ResumeSetup>,
    /// Resolutor de nombres alternativo. **Solo lo pone un test**, por el mismo motivo que
    /// [`run_http_with_lookup`]: sin él, «un nombre público que responde 127.0.0.1» no se puede
    /// montar contra el servidor de pruebas, y depender de que un dominio comodín real siga
    /// existiendo convertiría el test en una prueba de internet. El perímetro no se relaja.
    lookup: Option<Arc<dyn crate::dns::Lookup>>,
}

/// Construye el fetcher y las semillas de cada modo y lanza el bucle común.
async fn dispatch(
    job: CrawlJob,
    store_path: &Path,
    mut controls: RunControls,
) -> Result<CrawlOutcome> {
    // La exclusiva de escritura del fichero, antes de tocarlo y durante todo el rastreo,
    // pasada final incluida. Es lo que impide que dos procesos escriban el mismo fichero
    // —duplicando `links` y pisándose el cierre— y lo que hace que `resume` distinga un
    // rastreo muerto (`kill -9`: el sistema soltó el cerrojo) de uno vivo (el cerrojo
    // sigue tomado). Ver `store::StoreLock`.
    let _lock = store::StoreLock::acquire(store_path)?;
    // El trabajo se **mueve** a `run_with`, no se clona: `CrawlJob` arrastra el `CrawlMode`,
    // y en modo `list` eso es la lista completa de URLs — clonarla aquí costaba una copia
    // entera del conjunto antes de rastrear nada. Las semillas se construyen prestando del
    // modo y el préstamo termina antes del movimiento.
    match &job.mode {
        CrawlMode::Http { seed } => {
            let seeds = vec![normalize::normalize(seed, &job.normalize_policy())?];
            let seed_host = seeds[0].normalized.host_str();
            let fetcher =
                http_fetcher_for(&job, seed_host, network_screen(&seeds), controls.lookup.take())?;
            run_with(job, fetcher, seeds, store_path, controls).await
        }
        CrawlMode::Filesystem { root, base } => {
            let base = Url::parse(base)?;
            let fetcher = FilesystemFetcher::new(root.clone(), base.clone());
            // Se siembra con todo lo publicado, no solo con el índice: así se ven también los
            // ficheros que existen en `dist/` pero a los que no llega ningún enlace.
            let policy = job.normalize_policy();
            let mut seeds = Vec::new();
            for url in fetcher.discover_html() {
                seeds.push(normalize::normalize(url.as_str(), &policy)?);
            }
            if seeds.is_empty() {
                seeds.push(normalize::normalize(base.as_str(), &policy)?);
            }
            run_with(job, fetcher, seeds, store_path, controls).await
        }
        CrawlMode::List { urls } => {
            let policy = job.normalize_policy();
            let mut seeds = Vec::new();
            for u in urls {
                seeds.push(normalize::normalize(u, &policy)?);
            }
            // En una lista, «el host de la semilla» es el de la primera URL: el mismo criterio
            // con el que `run_with` calcula `seed_host` para decidir qué es interno. El
            // perímetro, en cambio, ya **no** sale de la primera línea: sale de todas.
            let seed_host = seeds.first().and_then(|s| s.normalized.host_str());
            let fetcher =
                http_fetcher_for(&job, seed_host, network_screen(&seeds), controls.lookup.take())?;
            run_with(job, fetcher, seeds, store_path, controls).await
        }
    }
}

/// Un rastreo HTTP con el resolutor de nombres inyectado. **Existe para los tests.**
///
/// No forma parte de la API del producto y por eso está oculta de la documentación. Los tests
/// de integración la necesitan porque el caso que hay que demostrar —«un nombre público que
/// responde `127.0.0.1`»— solo se puede montar contra el servidor de pruebas si el motor
/// pregunta a otro resolutor, y depender de que `localtest.me` siga existiendo el año que viene
/// convertiría el test en una prueba de internet.
///
/// El perímetro es el mismo que el de [`run`]: se calcula de las semillas, no se relaja.
#[doc(hidden)]
pub async fn run_http_with_lookup(
    job: CrawlJob,
    store_path: &Path,
    lookup: Arc<dyn crate::dns::Lookup>,
) -> Result<CrawlOutcome> {
    run_http_with_lookup_cancellable(job, store_path, lookup, None).await
}

/// Como [`run_http_with_lookup`], con señal de cancelación. **Existe para los tests.**
///
/// La necesita el que comprueba que las sondas cortadas se reencolan al reanudar: un corte por
/// `max_duration` cierra el fichero como `done` —terminó, aunque truncado— y un rastreo
/// terminado no se reanuda. Solo una cancelación deja el fichero reanudable.
#[doc(hidden)]
pub async fn run_http_with_lookup_cancellable(
    job: CrawlJob,
    store_path: &Path,
    lookup: Arc<dyn crate::dns::Lookup>,
    cancel: Option<CancelSignal>,
) -> Result<CrawlOutcome> {
    if !matches!(job.mode, CrawlMode::Http { .. }) {
        return Err(crate::CoreError::Http("run_http_with_lookup only takes http mode".into()));
    }
    dispatch(
        job,
        store_path,
        RunControls { cancel, lookup: Some(lookup), ..RunControls::default() },
    )
    .await
}

/// El perímetro de red de un rastreo: los hosts que el usuario nombró al lanzarlo.
///
/// Se calcula de las semillas y no de la primera de ellas. En modo lista `seeds` es el fichero
/// del usuario en orden de fichero, y un fichero de lista muchas veces llega de fuera: con el
/// criterio anterior, su primera línea decidía el perímetro de todas las demás.
///
/// `run_with` lo vuelve a calcular de las mismas semillas en vez de recibirlo: es una función
/// pura de un dato que ambos ya tienen, y pasarlo por la firma obligaría a los tres modos a
/// acarrearlo hasta un bucle que no lo necesita para nada más.
fn network_screen(seeds: &[NormalizedUrl]) -> normalize::NetworkScreen {
    normalize::NetworkScreen::for_seeds(seeds.iter().map(|s| &s.normalized))
}

/// El fetcher HTTP de un trabajo, con la credencial acotada al host de la semilla si la hay y
/// el perímetro de red instalado en su resolutor.
///
/// El acotado ocurre aquí, con el host **ya normalizado**: `fetch.rs` no decide a quién
/// pertenece la credencial, solo la aplica al host que se le dio. Sin host —una semilla
/// inverosímil sin autoridad— la credencial no se aplica a nada, que es el lado seguro.
fn http_fetcher_for(
    job: &CrawlJob,
    seed_host: Option<&str>,
    screen: normalize::NetworkScreen,
    lookup: Option<Arc<dyn crate::dns::Lookup>>,
) -> Result<HttpFetcher> {
    let mut fetcher = match lookup {
        Some(l) => HttpFetcher::screened_with(&job.limits.user_agent, screen, l)?,
        None => HttpFetcher::screened(&job.limits.user_agent, screen)?,
    }
    .with_max_body_bytes(job.limits.max_size_per_url);
    if let (Some(auth), Some(host)) = (&job.limits.http_basic_auth, seed_host) {
        fetcher = fetcher.with_basic_auth(host, auth);
    }
    Ok(fetcher)
}

/// El bucle de rastreo, común a los tres modos.
async fn run_with<F: Fetcher + 'static>(
    job: CrawlJob,
    fetcher: F,
    seeds: Vec<NormalizedUrl>,
    store_path: &Path,
    controls: RunControls,
) -> Result<CrawlOutcome> {
    let RunControls { progress, mut cancel, resume, lookup: _ } = controls;
    let resuming = resume.is_some();
    let started = Instant::now();
    let policy = job.normalize_policy();
    let crawl_id = match &resume {
        // Una reanudación es el mismo rastreo: mismo id, mismas filas, mismo fichero.
        Some(setup) => setup.crawl_id.clone(),
        None => uuid::Uuid::now_v7().to_string(),
    };

    // Los patrones de include/exclude se compilan **una vez y antes de tocar el disco**: un
    // patrón inválido es un error inmediato, no un fichero de rastreo a medio crear. Después
    // solo se consultan — y solo una vez por URL única, porque el índice de vistas del
    // frontier corta antes las repeticiones. Semántica en `pattern.rs`: el exclude gana.
    let filter = crate::pattern::UrlFilter::from_limits(&job.limits)?;

    // El tope del nivel gana sobre el que pida el trabajo, y no se puede subir desde ahí: en el
    // nivel gratuito, `--max-urls 50000` da 1.000.
    let max_urls =
        crate::entitlement::Limits::for_tier(job.tier).effective_max_urls(job.limits.max_urls);

    // El texto completo es de pago, y el límite se aplica **aquí**, no en la interfaz.
    //
    // `02-MODELO-DATOS.md §3.7` lo dice del índice FTS —«solo se puebla en nivel Pro»— y
    // `job.rs` lo repite de `collect_body_text` («multiplica el tamaño del fichero»). Hasta ahora
    // nadie lo hacía cumplir: un trabajo gratuito podía pedir el cuerpo, guardarlo y, desde que
    // la tabla FTS se puebla de verdad, indexarlo. Es la misma clase de agujero que tenía
    // `max_urls` antes de cablear `EntitlementSource` en la CLI.
    let collect_body_text =
        job.collect_body_text && job.tier >= crate::entitlement::Feature::FullTextSearch.min_tier();
    if job.collect_body_text && !collect_body_text {
        tracing::info!(
            tier = ?job.tier,
            "el texto completo para búsquedas es del nivel Pro: se rastrea sin él"
        );
    }
    let mut job = job;
    job.collect_body_text = collect_body_text;
    // El trabajo queda en un `Arc` porque cada tarea del pool necesita su asidero y el
    // rellenado hacía `job.clone()` **por URL despachada**. `CrawlJob` arrastra el
    // `CrawlMode`, y en modo `list` eso es la lista completa: con n URLs eran n clones de n
    // `String` — O(n²), y justo en el modo que hace justa la comparación con Screaming
    // Frog. Medido con la sonda de asignaciones (`despachar_una_lista_no_clona_la_lista_
    // completa_por_url`, 400 URLs de 1 KB): 214,2 MB asignados con el clon por URL, 44,6 MB
    // con el `Arc` — 4,8x, y la diferencia crece con el cuadrado de la lista (~13 GB de
    // tráfico a 10.000 URLs). Clonar el `Arc` es un incremento atómico.
    let job = Arc::new(job);

    let seed_host = seeds
        .first()
        .and_then(|s| s.normalized.host_str())
        .unwrap_or_default()
        .to_string();

    // El perímetro de red del rastreo. Ver [`network_screen`] y `normalize::NetworkScreen`.
    //
    // Es la primera de las dos líneas —la léxica, sobre el host escrito—; la que decide de
    // verdad es el resolutor del cliente HTTP, que ya lo lleva instalado. Esta sirve para no
    // abrir siquiera la conexión y para poder decir en la fila **por qué** no se sondeó.
    //
    // Es el mismo criterio con el que `ResumeScope` decide qué URLs de un fichero se releen.
    let screen = network_screen(&seeds);

    {
        // `crawl_meta` se escribe antes de arrancar el escritor y con una conexión que se cierra
        // acto seguido: a partir de aquí el fichero es del hilo escritor y de nadie más.
        // En una reanudación la fila ya existe: solo se refleja que vuelve a estar en marcha.
        if resuming {
            let conn = store::reopen_writer(store_path)?;
            conn.execute(
                "UPDATE crawl_meta SET status = 'running' WHERE id = ?1",
                rusqlite::params![crawl_id],
            )?;
        } else {
            let conn = store::open_writer(store_path)?;
            write_crawl_meta(&conn, &crawl_id, &job, &seed_host)?;
        }
    }
    let writer = writer::WriterHandle::spawn(store_path.to_path_buf())?;

    let fetcher = Arc::new(fetcher);
    let robots = Arc::new(RobotsCache::new());
    let throttle = Arc::new(crate::throttle::Throttle::new(job.limits.effective_concurrency()));
    // La puerta que hace cumplir el `Crawl-delay`: una petición en vuelo por host y arranques
    // espaciados. El porqué de que el `Throttle` no baste solo está en su doc (`throttle.rs`).
    let pacer = Arc::new(crate::throttle::CrawlDelayGate::new());
    // En una reanudación el frontier llega precargado: las `pending` en cola con su
    // profundidad guardada —el orden BFS sobrevive al corte— y todo lo demás marcado como
    // visto para no repetirlo. `already_fetched` descuenta del presupuesto lo ya rastreado:
    // sin él, un rastreo con `max_urls` interrumpido y reanudado rastrearía de más.
    let (mut frontier, mut in_sitemap, already_fetched, resumed_probes, rejected_probes) =
        match resume {
            Some(setup) => (
                setup.frontier,
                setup.in_sitemap,
                setup.already_fetched,
                setup.pending_probes,
                setup.rejected_probes,
            ),
            None => (Frontier::new(), std::collections::HashSet::new(), 0, Vec::new(), Vec::new()),
        };
    let mut metrics = CrawlMetrics::default();
    // En modo lista solo se descargan las URLs de la lista, así que ninguna página tiene a
    // sus enlazadores: el grafo de enlaces está incompleto **por definición**, no por un
    // corte. Se marca aquí, en el core y desde el arranque, porque la consecuencia es la
    // misma que la de un corte: las reglas de `REQUIERE_GRAFO_COMPLETO` no pueden afirmar lo
    // que afirman (con sitemaps encendidos reportaban la lista entera como huérfana), y
    // `diff` no puede afirmar que una URL desapareció. Dejarlo en manos de quien construye
    // el trabajo —la CLI apagaba los sitemaps y eso tapaba el caso— era casualidad, no
    // diseño. Un corte real (`max_urls`, `max_duration`) lo sobrescribe: «te faltan URLs de
    // tu propia lista» es más información, y ambos encienden `truncated`.
    let is_list = matches!(job.mode, CrawlMode::List { .. });
    let mut truncated = is_list.then_some(TruncationReason::ListMode);
    let mut interrupted = false;
    let mut emitter = ProgressEmitter::new(progress);

    if resuming {
        // El hilo escritor arranca con su índice hash→id vacío, y `links` e `images` resuelven
        // sus extremos contra ese índice (nunca con un JOIN por fila). Las páginas que se
        // rastreen ahora enlazan a URLs escritas por la sesión anterior: hay que reponerlas.
        let repuestas = resend_existing_rows(store_path, &writer).await?;
        tracing::debug!(filas = repuestas, "reanudación: índice del escritor repuesto");
    }

    // El presupuesto de tiempo, como instante límite. Se comprueba antes de rellenar el pool y
    // cancela las esperas en vuelo —el `sleep` de un `Crawl-delay`, el backoff de los
    // reintentos— vía `tokio::select!`. Antes solo se miraba al terminar una tarea: un
    // `Crawl-delay` de 30 s convertía un presupuesto de 5 s en 30 (medido: 30,03 s), y no había
    // forma de parar antes que matar el proceso.
    let deadline = job.limits.max_duration.map(|d| tokio::time::Instant::now() + d);

    // Semillas. Se escriben como `pending` antes de rastrear nada, no solo se encolan.
    //
    // En modo `filesystem` todas las páginas del directorio son semillas, así que sin esto un
    // enlace de la primera página a la última se pierde: cuando se escribe su lote, la fila de
    // destino todavía no existe y el `JOIN` por hash la descarta en silencio. En un fixture de
    // 10.000 páginas desaparecía la mitad de los enlaces.
    let mut seed_rows: Vec<UrlRow> = Vec::new();
    let seed_source = match job.mode {
        CrawlMode::List { .. } => DiscoverySource::List,
        _ => DiscoverySource::Link,
    };
    // ¿A qué semillas se les aplican los patrones? A las que no escribió el usuario a mano.
    // En modo `http` la semilla es la URL que se tecleó y el rastreo tiene que poder empezar
    // por ella: con `--include '/blog/'` y semilla en la raíz, filtrarla mataría el rastreo
    // antes de descubrir nada. Es también lo que hace Screaming Frog: la URL de arranque se
    // rastrea siempre. En `filesystem` las semillas son todo lo publicado en el directorio y
    // en `list` un conjunto importado: ahí filtrarlas es justo lo que se pide —
    // `audit ./dist --exclude '/borradores/'` no tiene otro sitio donde actuar.
    let seeds_exempt = matches!(job.mode, CrawlMode::Http { .. });
    for seed in &seeds {
        let hash = seed.hash();
        if !seeds_exempt && !filter.allows(seed.normalized.as_str()) {
            if frontier.mark_seen(hash) {
                metrics.urls_excluded += 1;
                let mut row = pending_row(seed, hash, 0, true, false);
                row.crawl_state = CrawlState::Excluded;
                row.exclusion_reason = Some(ExclusionReason::Pattern);
                seed_rows.push(row);
            }
            continue;
        }
        if frontier.enqueue(
            QueuedUrl {
                url: seed.normalized.clone(),
                depth: 0,
                discovered_from: None,
                source: seed_source,
            },
            hash,
        ) {
            metrics.urls_discovered += 1;
            seed_rows.push(pending_row(seed, hash, 0, true, false));
        }
    }

    // Sitemaps: las URLs que declara el sitio pero a las que quizá no llega ningún enlace.
    // Ese cruce es lo que produce el hallazgo de huérfanas. En una reanudación el
    // descubrimiento se repite —el original vivía en memoria y se perdió con el corte—: las
    // URLs ya vistas no se reencolan y los sitemaps quedan por fin registrados en el fichero.
    let mut sitemap_rows: Vec<UrlRow> = Vec::new();
    // Lo que se guarda de los sitemaps en sí, no de las URLs que declaran.
    let mut sitemap_meta: Vec<SitemapRow> = Vec::new();
    if job.discover_sitemaps {
        // Anunciar la fase antes de la primera petición: el descubrimiento puede tardar
        // varios segundos y sin esto la UI parece colgada justo al arrancar.
        emitter.enter_phase(CrawlPhase::Sitemaps, &metrics, 0);
        if let Some(seed) = seeds.first() {
            // El presupuesto del rastreo también manda aquí. No lo hacía: un sitemap que declara
            // un millón de URLs escribía un millón de filas y 397 MB de fichero **antes de
            // rastrear la primera página**, con un trabajo de nivel gratuito cuyo tope son 1.000.
            // Medido: 1,14 GB de RSS. El protocolo permite 50.000 URLs por sitemap y 5.000
            // sitemaps, así que el techo teórico era de 250 millones de filas.
            //
            // Se pasa también al descubrimiento: aquel arreglo acotó las filas escritas, pero
            // el `Vec<Url>` que devuelve `discover_sitemap_urls` seguía acumulando entero
            // —con un índice de 200 hijos de 50 MB, ~250 M de `Url` en memoria antes de
            // rastrear nada— y el corte de abajo llegaba tarde.
            let presupuesto = max_urls.map(|m| m as usize);
            // El presupuesto de tiempo también manda aquí: un sitemap que tarda en responder
            // no debe retener el corte (medido: 20 s de espera con un presupuesto de 1 s).
            // Al cancelar, el descubrimiento suelta su `JoinSet` y con él sus peticiones.
            let (discovered, meta) = tokio::select! {
                r = discover_sitemap_urls(
                    &fetcher,
                    &robots,
                    &throttle,
                    &seed.normalized,
                    &job,
                    job.limits.effective_concurrency() as usize,
                    presupuesto,
                ) => r,
                _ = wait_deadline(deadline) => {
                    truncated.get_or_insert(TruncationReason::MaxDuration);
                    (Vec::new(), Vec::new())
                }
                // Una cancelación durante el descubrimiento corta igual que el plazo: lo que
                // el sitemap no llegó a declarar lo descubrirá la reanudación, que lo repite.
                _ = wait_cancel(&mut cancel) => {
                    interrupted = true;
                    (Vec::new(), Vec::new())
                }
            };
            sitemap_meta = meta;
            for url in discovered {
                if presupuesto.is_some_and(|m| in_sitemap.len() >= m) {
                    tracing::debug!(
                        tope = presupuesto,
                        "sitemap: alcanzado el presupuesto de URLs; se deja de encolar"
                    );
                    truncated.get_or_insert(TruncationReason::MaxUrls);
                    break;
                }
                if let Ok(n) = normalize::normalize(url.as_str(), &policy) {
                    // Las URLs del sitemap pasan por el **mismo filtro que los enlaces**. No lo
                    // hacían, y era la única grieta del comportamiento de red: un sitio ajeno
                    // podía declarar en su sitemap `http://127.0.0.1:8080/panel` y la aplicación
                    // lo pedía, saltándose el `follow_external = false` que sí respetan los
                    // enlaces y las redirecciones. Comprobado con dos servidores locales: por
                    // enlace se rechazaba, por sitemap se pedía.
                    //
                    // Importa por dos motivos más allá del técnico: la promesa por defecto del
                    // producto es que solo se rastrea el sitio auditado, y `CONVENTIONS.md §1` exige
                    // que la app sea defendible como auditor de sitios propios ante la revisión
                    // de Apple.
                    if !normalize::is_crawlable_scheme(&n.normalized) {
                        continue;
                    }
                    let interno = normalize::is_internal(&n.normalized, &seed_host);
                    // `follow_external` amplía el **alcance** del rastreo, no el perímetro de
                    // red: una URL de sitemap que la criba rechaza no se pide ni con él puesto.
                    let seguir_sitemap = interno
                        || (job.limits.follow_external && screen.allows_host(&n.normalized));
                    if !seguir_sitemap {
                        tracing::debug!(
                            url = %n.normalized,
                            "URL de sitemap fuera del sitio auditado: se ignora"
                        );
                        continue;
                    }
                    let hash = n.hash();
                    in_sitemap.insert(hash);
                    // En modo lista el sitemap no amplía el rastreo: declarar una URL no es
                    // pedir que se audite. Se registra —el cruce lista ↔ sitemap es
                    // información, y el flag `in_sitemap` de las URLs de la lista sale del
                    // `insert` de arriba— pero no se encola. Y queda `skipped`, no `pending`:
                    // `pending` es exactamente lo que relee una reanudación, y releerlo
                    // rastrearía más allá de la lista.
                    if is_list {
                        if frontier.mark_seen(hash) {
                            metrics.urls_discovered += 1;
                            let mut row = pending_row(&n, hash, 0, interno, true);
                            row.crawl_state = CrawlState::Skipped;
                            sitemap_rows.push(row);
                        }
                        continue;
                    }
                    // Las URLs del sitemap obedecen los mismos patrones que los enlaces: si
                    // el usuario excluyó `/tag/`, que el sitio lo declare en su sitemap no lo
                    // reactiva. La exclusión queda registrada con `in_sitemap = true`.
                    if !filter.allows(n.normalized.as_str()) {
                        if frontier.mark_seen(hash) {
                            metrics.urls_excluded += 1;
                            let mut row = pending_row(&n, hash, 0, interno, true);
                            row.crawl_state = CrawlState::Excluded;
                            row.exclusion_reason = Some(ExclusionReason::Pattern);
                            sitemap_rows.push(row);
                        }
                        continue;
                    }
                    if frontier.enqueue(
                        QueuedUrl {
                            url: n.normalized.clone(),
                            depth: 0,
                            discovered_from: None,
                            source: DiscoverySource::Sitemap,
                        },
                        hash,
                    ) {
                        metrics.urls_discovered += 1;
                        sitemap_rows.push(pending_row(&n, hash, 0, interno, true));
                    }
                }
            }
        }
    }

    let mut rss = RssSampler::new();
    let mut concurrency = ConcurrencyMeter::new();

    // Las reglas de página se construyen **una vez para todo el rastreo**, no una vez por
    // página: `page_rules_for` monta ~59 `Box<dyn PageRule>` en el heap y las reglas no
    // tienen estado — reconstruirlas por página eran ~59 asignaciones × páginas parseadas
    // para obtener siempre la misma lista. `build_result` las recibe prestadas.
    let page_rules = page_rules_for(&job);

    // Las semillas se mandan antes de empezar: son el destino de muchos enlaces y tienen que
    // existir cuando se escriban. El hilo escritor las trocea en lotes por su cuenta.
    for row in seed_rows.drain(..).chain(sitemap_rows.drain(..)) {
        writer.send(CrawlResult { url: Some(row), ..Default::default() }).await?;
    }
    metrics.peak_rss_bytes = rss.sample();

    let loop_started = Instant::now();
    emitter.enter_phase(CrawlPhase::Crawl, &metrics, frontier.pending() as u64);

    // Pool con reposición continua: en cuanto una petición termina, se lanza la siguiente.
    //
    // La alternativa evidente —despachar tandas de `concurrency` y esperar a que terminen
    // todas— cuesta, por tanda, lo que tarde su petición más lenta. Medido contra un sitio
    // real: 192 de 200 URLs respondían en menos de 200 ms y 8 pasaban de 3 s, y esas 8
    // arrastraban ocho tandas enteras. El rastreo tardaba lo mismo que en secuencial (42,8 s
    // frente a 13,4 s teóricos). Con latencias uniformes el defecto no se ve, que es por lo
    // que el modo `filesystem` iba sobrado y el HTTP no.
    let mut in_flight = tokio::task::JoinSet::new();
    // La comprobación de estado de las externas (`check_external`). Solo actúa cuando las
    // externas no se rastrean enteras —con `follow_external` se piden como cualquier URL, que
    // es más— y cuando la fuente puede hacer red: en modo `filesystem` no hay cliente HTTP y
    // las externas quedan registradas sin estado, como siempre.
    let external_checks =
        job.limits.check_external && !job.limits.follow_external && fetcher.can_check_status();
    let mut externals = ExternalQueue::default();
    // Externas encoladas para comprobar, contra el tope `max_external`. Cuenta encoladas y no
    // comprobadas: así el tope decide en el descubrimiento y las descartadas se cuentan una
    // sola vez en `externals_unchecked`.
    let mut externals_enqueued: u64 = 0;
    // Externas **registradas**, contra `max_external_urls`. Es un tope distinto del de las
    // sondas y hace falta: medido en `--release`, una sola página de 9,3 MB con 350.000 `<a>` a
    // hosts distintos dejaba 350.001 filas en `urls`, 87 MB de fichero y 279 MB de RSS con
    // `max_urls: Some(1000)` y las sondas apagadas. Las externas no cuentan contra `max_urls` a
    // propósito —ese presupuesto es de páginas del usuario— y hasta aquí nada más las acotaba.
    let mut externals_registered: u64 = 0;
    // Sondas despachadas y todavía sin respuesta. Se lleva a mano porque `in_flight` mezcla las
    // dos clases de petición y el progreso necesita distinguirlas: sin este contador, «lo que
    // queda del sitio» incluiría las sondas en vuelo.
    let mut externals_in_flight: u64 = 0;
    // Sondas que quedaron a medias en una sesión anterior: filas externas, sin estado y sin
    // error, que el corte pilló en vuelo o en cola. Sin esto se perdían para siempre — la
    // reanudación solo relee las `pending`, y el frontier ya las tiene por vistas.
    // Las que el perímetro rechazó al releer el plan: se les escribe su motivo.
    //
    // Sin esto la fila se quedaba exactamente igual que la de una sonda cortada a medias —
    // externa, `skipped`, todo nulo—, así que cada reanudación posterior la volvía a leer y a
    // rechazar, y el informe no podía distinguir «no se comprobó porque apunta a tu red» de «no
    // se llegó a comprobar». Se escribe aquí y no al releer el plan porque **nadie escribe en
    // SQLite salvo el hilo escritor**, y allí todavía no existe.
    for url in rejected_probes {
        let Ok(n) = normalize::normalize(url.as_str(), &job.normalize_policy()) else { continue };
        let hash = n.hash();
        let mut row = external_row(&n, hash);
        row.exclusion_reason = Some(ExclusionReason::LocalNetwork);
        metrics.externals_unchecked += 1;
        writer.send(CrawlResult { url: Some(row), ..Default::default() }).await?;
    }
    if external_checks {
        for url in resumed_probes {
            if let Some(host) = url.host_str() {
                throttle.force_serial(host);
            }
            if externals_enqueued >= job.limits.max_external {
                metrics.externals_unchecked += 1;
                continue;
            }
            match externals.push(url) {
                Ok(()) => externals_enqueued += 1,
                Err(_) => metrics.externals_unchecked += 1,
            }
        }
    }
    // Peticiones en vuelo por host: la cuenta contra la que se aplica el límite de cada host.
    let mut in_flight_by_host: HashMap<String, usize> = HashMap::new();
    // Host de cada tarea en vuelo, por identificador: es lo que permite liberar el hueco
    // incluso cuando la tarea muere en pánico y no devuelve su URL.
    let mut host_by_task: HashMap<tokio::task::Id, String> = HashMap::new();

    'crawl: loop {
        // La cancelación y el presupuesto de tiempo se comprueban **antes** de rellenar el
        // pool: lanzar peticiones con el corte ya pedido solo lo retrasa.
        if interrupted || cancel_requested(&cancel) {
            interrupted = true;
            break 'crawl;
        }
        if deadline_reached(deadline) {
            truncated = Some(TruncationReason::MaxDuration);
            break 'crawl;
        }

        // Rellenar el pool respetando el límite **del host de cada URL** —no el del host
        // semilla—, que el freno adaptativo puede haber reducido si ese servidor está dando
        // señales de ir ahogado.
        while in_flight.len() < MAX_TOTAL_IN_FLIGHT {
            if let Some(item) = next_dispatchable(&mut frontier, &throttle, &in_flight_by_host) {
                let host = item.url.host_str().unwrap_or_default().to_string();
                *in_flight_by_host.entry(host.clone()).or_insert(0) += 1;
                let fetcher = Arc::clone(&fetcher);
                let robots = Arc::clone(&robots);
                let throttle = Arc::clone(&throttle);
                let pacer = Arc::clone(&pacer);
                // Un incremento atómico, no una copia del trabajo: ver el `Arc::new` de arriba.
                let job = Arc::clone(&job);
                let task = in_flight.spawn(async move {
                    let outcome =
                        process_url(&*fetcher, &robots, &throttle, &pacer, &job, &item).await;
                    (item, outcome)
                });
                host_by_task.insert(task.id(), host);
                continue;
            }
            // Sin nada interno despachable: una comprobación de externa, si su host tiene
            // hueco. Van detrás de lo interno a propósito — el rastreo del sitio del usuario
            // no espera por el estado de un enlace ajeno.
            let Some(url) = externals.next_dispatchable(&throttle, &in_flight_by_host) else {
                break;
            };
            let host = url.host_str().unwrap_or_default().to_string();
            *in_flight_by_host.entry(host.clone()).or_insert(0) += 1;
            let fetcher = Arc::clone(&fetcher);
            let item = QueuedUrl {
                url,
                depth: 0,
                discovered_from: None,
                source: DiscoverySource::Link,
            };
            externals_in_flight += 1;
            let task = in_flight.spawn(async move {
                // Ni el robots.txt ni el Crawl-delay del host ajeno se consultan, y es una
                // decisión, no un olvido: comprobar que un enlace resuelve es lo que hace el
                // navegador cuando el visitante lo pulsa — no se indexa, no se almacena y no
                // se sigue nada del sitio ajeno. Pedir además su robots.txt casi duplicaría
                // las peticiones a terceros para poder decir menos. La cortesía la ponen la
                // única petición en vuelo por host (`Throttle::force_serial`, fijado al
                // encolar) y el timeout corto de la sonda (`EXTERNAL_CHECK_TIMEOUT`).
                let outcome = UrlOutcome::ExternalChecked(fetcher.check_status(&item.url).await);
                (item, outcome)
            });
            host_by_task.insert(task.id(), host);
        }

        // Sin nada en vuelo y sin nada en la cola, el rastreo ha terminado. No basta con
        // mirar la cola: una petición en vuelo todavía puede descubrir enlaces nuevos. Y sin
        // nada en vuelo no puede quedar nada por despachar: con el límite mínimo de 1 por
        // host, todo lo que quedara en cola —interno o externo— tenía hueco en el rellenado de
        // arriba.
        if in_flight.is_empty() {
            debug_assert!(frontier.is_empty());
            debug_assert!(externals.pending() == 0);
            break;
        }

        // La concurrencia configurada es un techo. Lo que importa para el suelo teórico es
        // cuántas peticiones había de verdad en vuelo, que baja al final del rastreo y cuando
        // el freno adaptativo interviene.
        concurrency.record(in_flight.len());

        // Se espera a **una**, no a todas. Aquí está la diferencia.
        {
            // Las esperas en vuelo —crawl-delay, backoff de reintentos— no retienen el corte:
            // al vencer el plazo se deja de esperar, lo pendiente se cancela al salir del
            // bucle y sus URLs quedan `pending`, que es el estado que relee una reanudación.
            let joined = tokio::select! {
                j = in_flight.join_next_with_id() => j,
                _ = wait_deadline(deadline) => {
                    truncated = Some(TruncationReason::MaxDuration);
                    break 'crawl;
                }
                _ = wait_cancel(&mut cancel) => {
                    interrupted = true;
                    break 'crawl;
                }
            };
            let Some(joined) = joined else {
                continue;
            };
            let (item, outcome) = match joined {
                Ok((task_id, v)) => {
                    release_slot(&mut host_by_task, &mut in_flight_by_host, task_id);
                    v
                }
                // Una tarea que muere no puede desaparecer en silencio. Pasó de verdad: un
                // `robots.txt` con `Crawl-delay: inf` reventaba el worker, este `continue` se
                // tragaba el panic y el rastreo terminaba «bien» con cero URLs, sin nada que
                // explicara por qué. Un resultado vacío que parece correcto es peor que un error.
                Err(e) => {
                    release_slot(&mut host_by_task, &mut in_flight_by_host, e.id());
                    tracing::error!(error = %e, "una tarea de rastreo ha muerto");
                    metrics.urls_errored += 1;
                    continue;
                }
            };
            let hash = url_hash(&item.url);

            // Antes del reparto por brazos y no dentro de uno: `ExternalChecked` lo manejan dos
            // —el general y el del perímetro—, y decrementar solo en el primero dejaba el
            // contador colgado en cuanto una sonda resolvía a una dirección rechazada.
            if matches!(outcome, UrlOutcome::ExternalChecked(_)) {
                externals_in_flight = externals_in_flight.saturating_sub(1);
            }
            match outcome {
                UrlOutcome::Excluded(reason) => {
                    metrics.urls_excluded += 1;
                    writer.send(CrawlResult {
                        url: Some(excluded_row(&item, hash, reason, in_sitemap.contains(&hash))),
                        ..Default::default()
                    }).await?;
                }
                // El perímetro rechazó la dirección al resolverla. No es un fallo del sitio, es
                // una decisión nuestra, y tiene que quedar dicha como tal: una fila de error
                // con `error_kind` acusaría al sitio auditado de algo que no ha hecho.
                UrlOutcome::Failed(failure)
                    if failure.kind == crate::fetch::ErrorKind::Blocked =>
                {
                    metrics.urls_excluded += 1;
                    writer.send(CrawlResult {
                        url: Some(excluded_row(
                            &item,
                            hash,
                            ExclusionReason::LocalNetwork,
                            in_sitemap.contains(&hash),
                        )),
                        ..Default::default()
                    }).await?;
                }
                UrlOutcome::Failed(failure) => {
                    metrics.urls_errored += 1;
                    // Un recurso que no responde sigue siendo un recurso: la fila queda con
                    // el `kind` de su extensión y el estado a NULL, que es la verdad.
                    let resource = resource_row(hash, None, None, None, &item.url);
                    writer.send(CrawlResult {
                        url: Some(failed_row(&item, hash, &failure, in_sitemap.contains(&hash))),
                        resource,
                        ..Default::default()
                    }).await?;
                }
                // Igual que arriba, por el camino de la sonda: el nombre resolvió a una
                // dirección fuera del perímetro. Se cuenta como «registrada sin comprobar» —que
                // es lo que pasó— y **no** como sonda hecha ni como fallo del host ajeno: un
                // rechazo nuestro no puede sumar a su racha de fallos ni salir en el informe
                // como enlace roto.
                UrlOutcome::ExternalChecked(Err(failure))
                    if failure.kind == crate::fetch::ErrorKind::Blocked =>
                {
                    metrics.externals_unchecked += 1;
                    let mut row = external_check_failed_row(&item, hash, &failure);
                    row.error_kind = None;
                    row.error_message = None;
                    row.fetched_at = None;
                    row.exclusion_reason = Some(ExclusionReason::LocalNetwork);
                    tracing::debug!(
                        url = %item.url,
                        "la sonda externa resolvió fuera del perímetro; no se comprobó"
                    );
                    writer.send(CrawlResult { url: Some(row), ..Default::default() }).await?;
                }
                UrlOutcome::ExternalChecked(check) => {
                    // Solo estado: se rellena la fila de `urls` (y la de `resources` si la
                    // sonda clasifica la URL como recurso — un CSS en un CDN). Nada de esto
                    // toca `urls_fetched`: el presupuesto `max_urls` es del sitio del usuario.
                    metrics.externals_checked += 1;
                    if let Some(host) = item.url.host_str() {
                        // El freno adaptativo cuenta también estas respuestas. Su efecto
                        // directo es pequeño —`force_serial` ya dejó el host en una petición
                        // en vuelo— pero es lo que hace que `throttled_hosts()` diga la
                        // verdad, y el commit que trajo las sondas prometía ser «educado con
                        // quien no es nuestro».
                        if let Ok(probe) = &check {
                            throttle.record(host, probe.status);
                        }
                        let outcome = match &check {
                            Ok(probe) => Ok(probe.status),
                            Err(failure) => Err(failure),
                        };
                        if let Some((dropped, reason)) = externals.record(host, outcome) {
                            metrics.externals_unchecked += dropped as u64;
                            let retry_after =
                                check.as_ref().ok().and_then(|p| p.retry_after.clone());
                            tracing::info!(
                                host,
                                motivo = reason.as_str(),
                                sin_comprobar = dropped,
                                retry_after,
                                "se deja de sondear este host ajeno"
                            );
                        }
                    }
                    let (row, resource) = match &check {
                        Ok(probe) => (
                            external_checked_row(&item, hash, probe),
                            resource_row(
                                hash,
                                probe.content_type.as_deref(),
                                Some(probe.status),
                                probe.content_length,
                                &item.url,
                            ),
                        ),
                        Err(failure) => (
                            external_check_failed_row(&item, hash, failure),
                            resource_row(hash, None, None, None, &item.url),
                        ),
                    };
                    writer.send(CrawlResult {
                        url: Some(row),
                        resource,
                        ..Default::default()
                    }).await?;
                }
                UrlOutcome::Fetched(fetched) => {
                let FetchedOutcome { doc, page, blocked_by_robots } = *fetched;
                    if let Some(host) = doc.url.host_str() {
                        if let Some(nuevo) = throttle.record(host, doc.status) {
                            tracing::warn!(
                                host, limite = nuevo,
                                "el servidor da señales de sobrecarga: se reduce la concurrencia"
                            );
                        }
                    }
                    metrics.urls_fetched += 1;
                    metrics.bytes_downloaded += doc.content_length();
                    metrics.total_response_time +=
                        Duration::from_millis(doc.response_time_ms as u64);

                    // Cada enlace se resuelve **una sola vez** y el resultado se comparte con
                    // sus cuatro consumidores: el frontier, el recuento de salientes, las
                    // vistas de las reglas y las filas de `links`. Antes cada uno repetía
                    // `normalize_relative` + `canonicalize` sobre el mismo href —160 ns por
                    // enlace solo de normalizar, ~2,9 s en el caso denso de 6,15 M de
                    // enlaces— y cualquier coste añadido a esa ruta se pagaba por
                    // cuadruplicado: un arreglo de seguridad que metió dos `canonicalize()`
                    // de disco ahí costó 10,7x. El vector vive en la pila y muere con la
                    // página.
                    let resolved_links: Vec<Option<NormalizedUrl>> = page
                        .as_ref()
                        .map(|p| {
                            p.links
                                .iter()
                                .map(|l| resolve_link(&doc.url, &l.href, &policy, &*fetcher))
                                .collect()
                        })
                        .unwrap_or_default();

                    let result = build_result(
                        &item,
                        hash,
                        &doc,
                        page.as_ref(),
                        &resolved_links,
                        blocked_by_robots,
                        in_sitemap.contains(&hash),
                        &seed_host,
                        &policy,
                        &page_rules,
                        &*fetcher,
                    );
                    metrics.issues_found += result.issues.len() as u64;

                    // Enlaces descubiertos: se registran todos; se encolan según el modo.
                    //
                    // En modo `list` no se encola ninguno: se audita exactamente el conjunto
                    // que pidió el usuario, que es también lo que hace Screaming Frog en su
                    // modo lista y la única forma de que una comparación entre ambos sea
                    // justa. Pero los enlaces de una página son una propiedad de esa página,
                    // así que **sí se registran**: el destino recibe su fila de `urls` —sin
                    // ella, la fila de `links` se descarta en silencio contra el índice
                    // hash→id del escritor— con `crawl_state = 'skipped'`, y no se pide. Y a
                    // las externas se les comprueba el estado exactamente igual que en modo
                    // `http`: pedir una URL ajena para saber si resuelve no es rastrear más
                    // allá de la lista.
                    let follow_links = !is_list;
                    if let Some(parsed) = page.as_ref() {
                        let depth = item.depth + 1;
                        // La resolución ya está hecha: `resolved_links` es paralelo a
                        // `parsed.links` y trae la forma publicada, con el `canonicalize` del
                        // modo filesystem aplicado (sin él, `/about` y `/about/` se auditarían
                        // como dos páginas).
                        for (link, resuelto) in parsed.links.iter().zip(&resolved_links) {
                            let Some(n) = resuelto else {
                                continue;
                            };
                            if !normalize::is_crawlable_scheme(&n.normalized) {
                                continue;
                            }
                            let link_hash = n.hash();
                            if frontier.has_seen(link_hash) {
                                continue;
                            }
                            let internal = normalize::is_internal(&n.normalized, &seed_host);
                            // `follow_external` amplía el **alcance** del rastreo; no es un
                            // permiso para llegar a la red del usuario. Sin esta segunda
                            // condición la rama de abajo no se ejecutaba con él puesto y el
                            // perímetro entero quedaba fuera del camino: comprobado, un
                            // `config_json` con `follow_external: true` dentro de un `.sqlite`
                            // compartido hacía que `resume` pidiera un servicio en loopback y
                            // **guardara su título y su h1** en el fichero.
                            let follow_this =
                                job.limits.follow_external && screen.allows_host(&n.normalized);
                            if !internal && !follow_this {
                                // Las externas no se rastrean, pero sí se registran: el
                                // informe necesita saber a dónde apunta el sitio. Y con
                                // `check_external` —el defecto— además se les comprueba el
                                // estado, una vez por URL única: es lo que hace posible
                                // HTTP-404-EXTERNAL. La deduplicación la da el frontier: mil
                                // enlaces a la misma externa entran aquí una sola vez.
                                //
                                // Lo que se decide aquí es **si se sonda**, no si se
                                // registra: la fila se escribe salvo que el registro haya
                                // llegado a su propio tope.
                                frontier.mark_seen(link_hash);
                                if externals_registered >= job.limits.max_external_urls {
                                    metrics.externals_unregistered += 1;
                                    continue;
                                }
                                externals_registered += 1;
                                metrics.urls_discovered += 1;
                                let mut row = external_row(n, link_hash);
                                if external_checks {
                                    let skip = why_not_probe(
                                        &n.normalized,
                                        link.is_nofollow,
                                        &filter,
                                        &job.limits,
                                        externals_enqueued,
                                        &screen,
                                    );
                                    // El host ajeno también decide: puede estar cerrado por
                                    // fallos o haber agotado su propio tope.
                                    let skip = match skip {
                                        Some(reason) => Some(reason),
                                        None => {
                                            if let Some(h) = n.normalized.host_str() {
                                                // Una petición en vuelo por host ajeno, sin
                                                // recuperación: educación, no rendimiento.
                                                throttle.force_serial(h);
                                            }
                                            match externals.push(n.normalized.clone()) {
                                                Ok(()) => {
                                                    externals_enqueued += 1;
                                                    None
                                                }
                                                Err(reason) => Some(reason),
                                            }
                                        }
                                    };
                                    if let Some(reason) = skip {
                                        metrics.externals_unchecked += 1;
                                        row.exclusion_reason = reason.exclusion_reason();
                                        tracing::debug!(
                                            url = %n.normalized,
                                            motivo = reason.as_str(),
                                            "URL externa registrada sin comprobar su estado"
                                        );
                                    }
                                } else if job.limits.follow_external {
                                    // Con `follow_external` no hay sondas, así que llegar aquí
                                    // solo puede significar que el perímetro rechazó la URL.
                                    // La fila lo dice: si no, quedaría idéntica a una externa
                                    // cualquiera y una reanudación la volvería a coger.
                                    metrics.externals_unchecked += 1;
                                    row.exclusion_reason = Some(ExclusionReason::LocalNetwork);
                                    tracing::debug!(
                                        url = %n.normalized,
                                        "URL externa fuera del perímetro: no se rastrea"
                                    );
                                }
                                writer.send(CrawlResult {
                                    url: Some(row),
                                    ..Default::default()
                                }).await?;
                                continue;
                            }
                            // Modo lista: el destino interno se registra y no se pide.
                            // Descargarlo sería rastrear más allá de la lista, que es justo
                            // lo que este modo promete no hacer. `skipped`, no `pending`:
                            // `pending` es lo que relee una reanudación. Y sin pasar por los
                            // patrones ni por `nofollow`: esos deciden qué se rastrea, y en
                            // este modo la lista ya lo decidió.
                            if !follow_links {
                                frontier.mark_seen(link_hash);
                                metrics.urls_discovered += 1;
                                let mut row = pending_row(
                                    n,
                                    link_hash,
                                    depth,
                                    internal,
                                    in_sitemap.contains(&link_hash),
                                );
                                row.crawl_state = CrawlState::Skipped;
                                writer.send(CrawlResult { url: Some(row), ..Default::default() }).await?;
                                continue;
                            }
                            // Los patrones del usuario van antes que `nofollow` y que la
                            // profundidad: cuando una URL casa con un exclude, «tú la
                            // excluiste» es la causa raíz que el informe debe dar, diga lo
                            // que diga el enlace.
                            if !filter.allows(n.normalized.as_str()) {
                                frontier.mark_seen(link_hash);
                                metrics.urls_excluded += 1;
                                let mut row = pending_row(
                                    n,
                                    link_hash,
                                    depth,
                                    internal,
                                    in_sitemap.contains(&link_hash),
                                );
                                row.crawl_state = CrawlState::Excluded;
                                row.exclusion_reason = Some(ExclusionReason::Pattern);
                                writer.send(CrawlResult { url: Some(row), ..Default::default() }).await?;
                                continue;
                            }
                            if job.limits.respect_nofollow && link.is_nofollow {
                                frontier.mark_seen(link_hash);
                                metrics.urls_excluded += 1;
                                let mut row = pending_row(n, link_hash, depth, internal, false);
                                row.crawl_state = CrawlState::Excluded;
                                row.exclusion_reason = Some(ExclusionReason::Nofollow);
                                writer.send(CrawlResult { url: Some(row), ..Default::default() }).await?;
                                continue;
                            }
                            if let Some(max) = job.limits.max_depth {
                                if depth > max {
                                    frontier.mark_seen(link_hash);
                                    metrics.urls_excluded += 1;
                                    truncated.get_or_insert(TruncationReason::MaxDepth);
                                    let mut row = pending_row(n, link_hash, depth, internal, false);
                                    row.crawl_state = CrawlState::Excluded;
                                    row.exclusion_reason = Some(ExclusionReason::Depth);
                                    writer.send(CrawlResult { url: Some(row), ..Default::default() }).await?;
                                    continue;
                                }
                            }
                            if frontier.enqueue(
                                QueuedUrl {
                                    url: n.normalized.clone(),
                                    depth,
                                    discovered_from: None,
                                    source: DiscoverySource::Link,
                                },
                                link_hash,
                            ) {
                                metrics.urls_discovered += 1;
                                writer.send(CrawlResult {
                                    url: Some(pending_row(
                                        n,
                                        link_hash,
                                        depth,
                                        internal,
                                        in_sitemap.contains(&link_hash),
                                    )),
                                    ..Default::default()
                                }).await?;
                            }
                        }
                    }

                    // El destino de una redirección también se rastrea — o, en modo lista,
                    // se registra sin pedirse, como cualquier otro destino.
                    //
                    // Un 3xx no trae HTML, así que no pasa por el bloque de enlaces de arriba y
                    // su destino no se pedía nunca: si ninguna otra página lo enlazaba, no
                    // existía como fila, el hash de `redirect_to` se quedaba sin resolver y las
                    // reglas de cadena, bucle y redirección a 404 no tenían grafo que recorrer.
                    // Funcionaban por suerte —el menú de un sitio real suele enlazar los
                    // eslabones intermedios— y no por diseño.
                    //
                    // La profundidad no aumenta: una redirección no es un clic más para el
                    // visitante, es el mismo clic que acaba en otro sitio.
                    {
                        if let Some(location) = doc.location.as_deref() {
                            if let Ok(mut n) =
                                normalize::normalize_relative(&doc.url, location, &policy)
                            {
                                if let Some(canonical) = fetcher.canonicalize(&n.normalized) {
                                    n.normalized = canonical;
                                }
                                let destino_hash = n.hash();
                                let interno = normalize::is_internal(&n.normalized, &seed_host);
                                // Mismo criterio que en los enlaces: el perímetro manda sobre
                                // `follow_external`. Un `/go/oferta` que redirige a una
                                // dirección de la red del usuario no se sigue.
                                let seguir = interno
                                    || (job.limits.follow_external
                                        && screen.allows_host(&n.normalized));
                                // El destino externo que no se sigue **también deja fila, y
                                // sonda**, con el mismo trato que un enlace externo.
                                //
                                // Sin esta rama, un `/go/producto` que redirige a una tienda
                                // ajena muerta era invisible: se veía el 301 y nada más. El
                                // destino no existía como fila, `redirect_to` se quedaba sin
                                // resolver y ninguna regla podía decir que el otro extremo es
                                // un 404. Es el patrón donde más se pudren los enlaces,
                                // precisamente porque el destino es de otro y nadie avisa
                                // cuando cae.
                                //
                                // `!seguir` ya implica externo: `seguir` es cierto para todo
                                // lo interno. Y va antes del reparto por modo porque no
                                // depende de él: en modo lista tampoco se registraba.
                                if !seguir && frontier.mark_seen(destino_hash) {
                                    if externals_registered >= job.limits.max_external_urls {
                                        metrics.externals_unregistered += 1;
                                    } else {
                                        externals_registered += 1;
                                        metrics.urls_discovered += 1;
                                        let mut row = external_row(&n, destino_hash);
                                        if external_checks {
                                            // Una redirección no tiene `rel`, así que su
                                            // destino nunca llega aquí como `nofollow`.
                                            let skip = why_not_probe(
                                                &n.normalized,
                                                false,
                                                &filter,
                                                &job.limits,
                                                externals_enqueued,
                                                &screen,
                                            );
                                            let skip = match skip {
                                                Some(reason) => Some(reason),
                                                None => {
                                                    if let Some(h) = n.normalized.host_str() {
                                                        throttle.force_serial(h);
                                                    }
                                                    match externals.push(n.normalized.clone()) {
                                                        Ok(()) => {
                                                            externals_enqueued += 1;
                                                            None
                                                        }
                                                        Err(reason) => Some(reason),
                                                    }
                                                }
                                            };
                                            if let Some(reason) = skip {
                                                metrics.externals_unchecked += 1;
                                                row.exclusion_reason = reason.exclusion_reason();
                                                tracing::debug!(
                                                    url = %n.normalized,
                                                    motivo = reason.as_str(),
                                                    "destino externo de una redirección \
                                                     registrado sin comprobar su estado"
                                                );
                                            }
                                        } else if job.limits.follow_external {
                                            // Con `follow_external` puesto, llegar aquí solo
                                            // puede ser el perímetro. La fila lo dice, igual
                                            // que en los enlaces.
                                            metrics.externals_unchecked += 1;
                                            row.exclusion_reason =
                                                Some(ExclusionReason::LocalNetwork);
                                        }
                                        writer.send(CrawlResult {
                                            url: Some(row),
                                            ..Default::default()
                                        }).await?;
                                    }
                                }
                                // En modo lista, el mismo trato que un enlace: la fila del
                                // destino se registra —sin ella `redirect_to` queda sin
                                // resolver y las reglas de cadena no tienen grafo— y no se
                                // pide.
                                if !follow_links {
                                    if seguir && frontier.mark_seen(destino_hash) {
                                        metrics.urls_discovered += 1;
                                        let mut row = pending_row(
                                            &n,
                                            destino_hash,
                                            item.depth,
                                            interno,
                                            in_sitemap.contains(&destino_hash),
                                        );
                                        row.crawl_state = CrawlState::Skipped;
                                        writer.send(CrawlResult {
                                            url: Some(row),
                                            ..Default::default()
                                        }).await?;
                                    }
                                } else if seguir && !filter.allows(n.normalized.as_str()) {
                                    if frontier.mark_seen(destino_hash) {
                                        metrics.urls_excluded += 1;
                                        let mut row = pending_row(
                                            &n,
                                            destino_hash,
                                            item.depth,
                                            interno,
                                            in_sitemap.contains(&destino_hash),
                                        );
                                        row.crawl_state = CrawlState::Excluded;
                                        row.exclusion_reason = Some(ExclusionReason::Pattern);
                                        writer.send(CrawlResult {
                                            url: Some(row),
                                            ..Default::default()
                                        }).await?;
                                    }
                                } else if seguir
                                    && frontier.enqueue(
                                        QueuedUrl {
                                            url: n.normalized.clone(),
                                            depth: item.depth,
                                            discovered_from: None,
                                            source: DiscoverySource::Link,
                                        },
                                        destino_hash,
                                    )
                                {
                                    metrics.urls_discovered += 1;
                                    writer.send(CrawlResult {
                                        url: Some(pending_row(
                                            &n,
                                            destino_hash,
                                            item.depth,
                                            interno,
                                            in_sitemap.contains(&destino_hash),
                                        )),
                                        ..Default::default()
                                    }).await?;
                                }
                            }
                        }
                    }

                    writer.send(result).await?;
                }
            }

            // Presupuesto de rastreo. Al agotarse, el rastreo termina limpiamente y **muestra
            // todo lo encontrado hasta ahí**: se limita la escala, no los resultados. Lo ya
            // rastreado antes de una interrupción cuenta: el presupuesto es del rastreo, no
            // de cada sesión.
            if let Some(max) = max_urls {
                if already_fetched + metrics.urls_fetched >= max {
                    truncated = Some(TruncationReason::MaxUrls);
                    break 'crawl;
                }
            }
            // El presupuesto de tiempo no se comprueba aquí: lo vigilan el `select!` de arriba
            // durante la espera y la cabecera del bucle antes de rellenar el pool.
        }

        // Muestrear son syscalls, así que se hace por tiempo y no por URL: el bucle une una
        // tarea por iteración, y aquí se pagaban 5-20 µs por URL. Ver `RSS_SAMPLE_INTERVAL`.
        metrics.peak_rss_bytes = rss.sample_if_due();
        // Las dos cuentas van separadas: lo que queda del sitio del usuario, y lo que queda de
        // preguntar por enlaces ajenos. Sumarlas era lo que hacía parecer muerto un rastreo que
        // solo esperaba a un host lento de otro.
        let sondas = externals.pending() as u64 + externals_in_flight;
        emitter.tick(
            &metrics,
            (frontier.pending() + in_flight.len()) as u64 - externals_in_flight,
            sondas,
        );
    }

    // Un corte deja sondas de externas sin mandar, y eso tiene que verse en el resumen: sus
    // filas quedan sin estado, y ninguna regla puede decir nada de ellas. En un final normal la
    // cola está vacía y esto suma cero.
    if externals.pending() > 0 {
        tracing::debug!(
            sondas = externals.pending(),
            motivo = NotProbed::Interrupted.as_str(),
            "el corte deja externas sin comprobar"
        );
        metrics.externals_unchecked += externals.pending() as u64;
    }

    // Lo que siga en vuelo tras un corte —presupuesto de tiempo, tope de URLs— se cancela aquí,
    // no al final de la función: sus URLs ya están escritas como `pending`, que es exactamente
    // el estado que una reanudación relee. En un final normal el pool está vacío y esto no hace
    // nada.
    in_flight.shutdown().await;

    // Se cierra el escritor antes de la pasada final: esta necesita la conexión para sí, y hasta
    // que el hilo no termina no hay garantía de que todo esté en disco.
    emitter.enter_phase(CrawlPhase::Finalize, &metrics, 0);
    let escrito = writer.finish().await?;
    tracing::debug!(
        urls = escrito.urls, links = escrito.links, lotes = escrito.batches,
        "hilo escritor cerrado"
    );
    metrics.crawl_loop = loop_started.elapsed();

    let mut conn = store::reopen_writer(store_path)?;

    tracing::debug!(rss_mb = rss.sample() / 1048576, "RSS al terminar el bucle de rastreo");

    // Pasada final.
    // El `robots.txt` vive en el caché en memoria y los sitemaps en la lista de arriba: sin
    // este volcado, el fichero de rastreo no conserva ni rastro de ninguno de los dos.
    let robots_snapshot = robots.snapshot().await;
    let base_url = seeds
        .first()
        .and_then(|s| s.normalized.join("/").ok())
        .unwrap_or_else(fallback_base_url);
    let records = CrawlRecords {
        robots: &robots_snapshot,
        base: &base_url,
        sitemaps: &sitemap_meta,
    };
    let wal_kept;
    if interrupted {
        // Un corte a petición no ejecuta la pasada final: sus reglas de conjunto y
        // `internal_links_in` necesitan el rastreo entero, y los calculará quien lo termine
        // —la reanudación—. Solo se deja constancia y el fichero queda `paused` y portable.
        wal_kept = pause(&mut conn, &crawl_id, records)? == store::FinalizeOutcome::WalKept;
    } else {
        let cierre = finalize(
            &mut conn,
            &crawl_id,
            truncated,
            &mut rss,
            job.tier,
            records,
            resuming,
            &mut emitter,
            &metrics,
            &cancel,
        )?;
        metrics.issues_found += cierre.site_issues;
        wal_kept = cierre.outcome == store::FinalizeOutcome::WalKept;
        // La pasada final también obedece al primer Ctrl-C: si el corte llegó entre dos
        // reglas de conjunto, el fichero quedó `paused` y reanudable, igual que un corte
        // durante el rastreo. Antes la señal solo se miraba en el bucle de rastreo y el
        // usuario acababa con el segundo Ctrl-C, el que sale a lo bruto y deja el WAL sin
        // volcar (así nació el `-wal` de 1 GB del 2026-08-02).
        interrupted = cierre.interrupted;
    }

    metrics.elapsed = started.elapsed();
    metrics.setup_and_teardown = metrics.elapsed.saturating_sub(metrics.crawl_loop);
    metrics.peak_rss_bytes = rss.sample();
    metrics.effective_concurrency =
        concurrency.average(job.limits.effective_concurrency() as f64);
    let (elements, pages) = count_elements(&conn).unwrap_or((0, 0));
    metrics.elements_written = elements;
    metrics.pages_parsed = pages;

    Ok(CrawlOutcome {
        crawl_id,
        store_path: store_path.to_path_buf(),
        metrics,
        // Un corte a petición no es un truncado: el rastreo no terminó, sigue pendiente.
        truncated: if interrupted { None } else { truncated },
        interrupted,
        wal_kept,
    })
}

/// Techo global de peticiones en vuelo, sumando todos los hosts.
///
/// El límite que manda es el de cada host ([`crate::throttle::Throttle::limit_for`]); este techo
/// solo impide que un rastreo con `follow_external` o una lista con cientos de dominios abra
/// cientos de conexiones simultáneas. Con un solo host —el caso normal— no llega a tocarse:
/// manda el límite por host. El valor sale de `01-ARQUITECTURA.md §5`: «un rastreo de cartera
/// con 20 dominios puede ir a 100 peticiones en vuelo sin castigar a ningún servidor».
const MAX_TOTAL_IN_FLIGHT: usize = 100;

/// URLs que el planificador mira en la cabeza del frontier antes de rendirse en una llamada.
///
/// Solo se recorre cuando **hay** un host con trabajo en cola y hueco para pedirle algo (la
/// puerta de `Frontier::dequeue_dispatchable`), así que el recorrido no es especulativo: es el
/// precio de encontrar a ese host detrás de lo que haya acumulado uno saturado. Cuatro mil
/// cubren de sobra el caso que lo motivó —cientos de URLs de un host con `Crawl-delay` por
/// delante— y acotan el trabajo de una llamada.
const MAX_FRONTIER_SCAN: usize = 4_096;

/// Sondas de estado que se le mandan como mucho a un mismo host ajeno.
///
/// El tope global (`max_external`, 10.000) no protege del caso real: **un solo host** enlazado
/// desde cientos de páginas. Con una sonda en vuelo por host ajeno, doscientas URLs de un host
/// que tarde un segundo son más de tres minutos pegados al final del rastreo. Doscientas cubren
/// con holgura lo que un sitio real enlaza hacia un mismo dominio; lo que pase de ahí queda
/// registrado sin estado y contado en [`CrawlMetrics::externals_unchecked`].
const MAX_EXTERNAL_PER_HOST: u32 = 200;

/// Fallos de red seguidos de un host ajeno tras los que se deja de sondearlo.
///
/// Un dominio muerto enlazado desde doscientas URLs únicas paga el timeout entero por cada una,
/// y en serie: **~33 minutos** añadidos al final del rastreo (medido: 8 URLs con 400 ms de
/// retardo daban 3,3 s de cola, serialización perfecta). Tres respuestas ausentes seguidas ya
/// dicen lo que hay que saber del host. Un 4xx o un 5xx **no** cuentan: eso es una respuesta,
/// el host está vivo y su estado es justo lo que la sonda venía a averiguar.
const EXTERNAL_HOST_FAILURE_STREAK: u32 = 3;

/// Tope de un texto que controla un servidor ajeno y acaba en el fichero de rastreo.
///
/// `resources.mime` y `urls.error_message` son cadenas que elige quien responde. No hay
/// inyección de por medio —todo va por parámetros—, pero un `Content-Type` de varios KB en cada
/// una de 10.000 externas engorda el fichero que se le entrega al cliente sin aportar nada: el
/// tipo de contenido más largo que existe no llega a cien caracteres.
const MAX_THIRD_PARTY_TEXT: usize = 255;

/// Recorta un texto de terceros a [`MAX_THIRD_PARTY_TEXT`] **caracteres**, no bytes.
///
/// Por caracteres porque la columna es `TEXT` y porque cortar por bytes partiría una secuencia
/// UTF-8 por la mitad. El caso normal no copia ni recorre nada: si la cadena cabe en el tope
/// contada en bytes, cabe contada en caracteres.
fn clip_third_party(mut text: String) -> String {
    if text.len() <= MAX_THIRD_PARTY_TEXT {
        return text;
    }
    if let Some((end, _)) = text.char_indices().nth(MAX_THIRD_PARTY_TEXT) {
        text.truncate(end);
    }
    text
}

/// Espera hasta el instante límite; sin límite, espera para siempre. Para `tokio::select!`.
async fn wait_deadline(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d).await,
        None => std::future::pending().await,
    }
}

fn deadline_reached(deadline: Option<tokio::time::Instant>) -> bool {
    deadline.is_some_and(|d| tokio::time::Instant::now() >= d)
}

/// Espera a que llegue la cancelación; sin señal, espera para siempre. Para `tokio::select!`.
async fn wait_cancel(cancel: &mut Option<CancelSignal>) {
    let Some(rx) = cancel else {
        return std::future::pending().await;
    };
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            // El emisor desapareció sin cancelar: ya no puede haber cancelación nunca.
            return std::future::pending().await;
        }
    }
}

fn cancel_requested(cancel: &Option<CancelSignal>) -> bool {
    cancel.as_ref().is_some_and(|rx| *rx.borrow())
}

/// Cierre de un rastreo interrumpido a petición.
///
/// No es la pasada final: las reglas de conjunto y `internal_links_in` necesitan el rastreo
/// completo y los calculará la reanudación al terminar de verdad. Aquí solo se deja constancia
/// de lo consultado —`robots.txt` y sitemaps, que viven en memoria y se perderían—, se marca
/// `status = 'paused'` (el estado que ya prevé el esquema y que `resume` acepta) y se saca el
/// fichero del modo WAL para que siga siendo «un rastreo = un fichero portable».
///
/// Devuelve cómo quedó el fichero: con un lector concurrente no se puede salir de WAL, y eso
/// **no puede tapar el mensaje de «interrupted, resume with…»** — antes el error lo sustituía
/// y el usuario creía haber perdido el rastreo.
fn pause(
    conn: &mut Connection,
    crawl_id: &str,
    records: CrawlRecords<'_>,
) -> Result<store::FinalizeOutcome> {
    write_robots_and_sitemaps(conn, &records)?;
    conn.execute(
        "UPDATE crawl_meta SET status = 'paused' WHERE id = ?1",
        rusqlite::params![crawl_id],
    )?;
    store::finalize(conn)
}

// ---------------------------------------------------------------- Reanudación

/// El estado recargado de un rastreo interrumpido, listo para continuar.
struct ResumeSetup {
    crawl_id: String,
    /// Frontier con las `pending` encoladas por su `depth` guardado y todo lo demás visto.
    frontier: Frontier,
    /// Hashes con `in_sitemap = 1`, para que las filas que se completen ahora lo conserven.
    in_sitemap: std::collections::HashSet<i64>,
    /// URLs ya rastreadas (`crawl_state = 'done'`): descuentan del presupuesto `max_urls`.
    already_fetched: u64,
    /// Sondas de externas que quedaron sin respuesta al cortar.
    ///
    /// Son las filas que delatan a una sonda a medias: externa, `skipped`, sin `status_code`,
    /// sin `error_kind` y sin motivo de exclusión. Sin reencolarlas se perdían **para
    /// siempre**: `resume` solo relee las `pending`, y como el frontier ya las tiene por
    /// vistas, un enlace nuevo a esa misma URL tampoco las recuperaba. `HTTP-404-EXTERNAL`
    /// callaba sobre ellas y ningún contador lo delataba.
    pending_probes: Vec<Url>,
    /// Las que el perímetro rechaza al releerlas, para poder **escribir su motivo**.
    ///
    /// Viajan hasta el bucle en vez de descartarse aquí porque aquí no hay a quién escribir:
    /// el hilo escritor no existe todavía y **nadie escribe en SQLite salvo él**. Sin esto la
    /// fila se quedaba sin `exclusion_reason`, o sea idéntica a una sonda que se cortó a
    /// medias: cada reanudación posterior la volvía a leer y a rechazar, y el informe no podía
    /// distinguir «no se comprobó porque apunta a tu red» de «no se llegó a comprobar».
    rejected_probes: Vec<Url>,
}

fn not_resumable(store_path: &Path, reason: impl Into<String>) -> crate::CoreError {
    crate::CoreError::NotResumable {
        path: store_path.display().to_string(),
        reason: reason.into(),
    }
}

/// Lee de un fichero interrumpido todo lo necesario para continuarlo, validando antes que se
/// pueda: ni un rastreo terminado, ni un esquema de otra versión, ni una configuración ilegible.
fn read_resume_plan(store_path: &Path) -> Result<(CrawlJob, ResumeSetup)> {
    use rusqlite::OpenFlags;
    let conn = Connection::open_with_flags(
        store_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;

    // Un fichero **más nuevo** que este core no se puede reanudar: hacia atrás no hay migración.
    // Uno más antiguo, sí, siempre que lo que le falte por cruzar no cambie lo que el motor
    // escribe: eso lo decide `store::first_blocking_resume`, migración a migración.
    //
    // Esto era `version != SCHEMA_VERSION` con el argumento de que continuar mezclaría en un
    // fichero la mitad rastreada por un parser viejo y la mitad por el nuevo. El argumento vale
    // para una migración que cambie **qué se escribe**, y no para las seis publicadas hasta hoy
    // —dos vistas, dos tablas, una columna y dos índices—, ninguna de las cuales toca el parser.
    //
    // Medido el 2026-08-02: una migración que solo crea un índice dejó irrecuperable un rastreo
    // de dieciocho horas, y el error decía «vuelve a rastrearlo».
    let version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))
        .map_err(|_| not_resumable(store_path, "it does not look like a crawl file"))?;
    if version > crate::SCHEMA_VERSION {
        return Err(not_resumable(
            store_path,
            format!(
                "it is a schema v{version} crawl and this core writes v{}: \
                 there is no way back. Use a newer build, or run the crawl again",
                crate::SCHEMA_VERSION
            ),
        ));
    }
    if let Some(bloqueante) = crate::store::first_blocking_resume(version) {
        return Err(not_resumable(
            store_path,
            format!(
                "it is a schema v{version} crawl, and migration {bloqueante} changes what the \
                 engine writes: half of it would say one thing and half another. \
                 Run the crawl again",
            ),
        ));
    }

    let (crawl_id, status, config_json, base_url, meta_mode, source_path): (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT id, status, config_json, base_url, mode, source_path
             FROM crawl_meta LIMIT 1",
            [],
            |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            },
        )
        .map_err(|_| not_resumable(store_path, "it has no crawl metadata"))?;

    match status.as_str() {
        // `running` es un corte brusco (kill, cuelgue); `paused` uno limpio (CancelSignal).
        // Los dos dejan las `pending` escritas, que es lo único que la reanudación necesita.
        "running" | "paused" => {}
        "done" => {
            return Err(not_resumable(
                store_path,
                "ya terminó (status='done'); un rastreo terminado se repite, no se reanuda",
            ))
        }
        otro => {
            return Err(not_resumable(store_path, format!("its status is '{otro}'")));
        }
    }

    // La configuración que manda es la del rastreo original, guardada íntegra: reanudar con
    // otra daría un resultado distinto a no haber parado, que es justo lo que no puede pasar.
    let job: CrawlJob = serde_json::from_str(&config_json).map_err(|e| {
        not_resumable(store_path, format!("its saved configuration cannot be read: {e}"))
    })?;

    // …pero el fichero es entrada no confiable —«un rastreo = un fichero portable» que se
    // comparte— y `config_json` no puede ampliar el alcance de lo que este proceso haría por
    // sí mismo. Un fichero fabricado con `mode: filesystem, root: "/"` y
    // `collect_body_text: true` convertía `resume` en un volcado del disco entero del
    // usuario; con `mode: http` e `ignore_robots: true`, en un rastreo dirigido contra un
    // tercero. Dos defensas:
    //
    // 1. El objetivo declarado tiene que ser coherente con los metadatos del propio fichero.
    // 2. `tier`, `ignore_robots` y `follow_external` salen de la sesión en vivo, nunca del
    //    fichero: el `tier` embebido burlaría los límites del nivel, y un `ignore_robots` o un
    //    `follow_external` guardados son permisos que nadie ha vuelto a conceder en esta sesión
    //    — reanudar no tiene flag para pedirlos, así que el valor vivo es el defecto.
    validate_resume_scope(&job, &base_url, &meta_mode, source_path.as_deref(), store_path)?;
    let mut job = job;
    // `DevSource` es hoy la única implementación en vivo (la misma que consulta la CLI al
    // rastrear); cuando lleguen StoreKit y la MS Store, la fuente cambiará, no este punto.
    use crate::entitlement::EntitlementSource as _;
    job.tier = crate::entitlement::DevSource::from_env()?.tier();
    job.limits.ignore_robots = false;
    // `follow_external` no estaba aquí, y era la puerta más ancha de todas: un `.sqlite`
    // compartido con `"follow_external":true` en su `config_json` convertía `resume` en un
    // rastreo completo del host que dijera el fichero. Comprobado con semilla pública: `GET` a
    // un servicio en loopback, `status=200`, y **el título y el h1 del panel interno guardados
    // en el fichero**. La CLI ni siquiera tiene flag para pedirlo.
    job.limits.follow_external = false;
    let job = job;

    let mut frontier = Frontier::new();
    let is_list = matches!(job.mode, CrawlMode::List { .. });

    let mut in_sitemap: std::collections::HashSet<i64> = std::collections::HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT url_hash FROM urls WHERE in_sitemap = 1")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        for hash in rows {
            in_sitemap.insert(hash?);
        }
    }

    // El alcance con el que se releen las filas del fichero.
    //
    // `validate_resume_scope` comprueba el `config_json`, que es lo que pedía la revisión
    // anterior, y justo después las URLs se recargaban con un `SELECT ... WHERE crawl_state =
    // 'pending'` **sin mirar ni host ni esquema**. Un `.sqlite` fabricado —o uno legítimo con
    // una fila inyectada— con una `pending` apuntando a `169.254.169.254` hacía que `resume` la
    // pidiera con un `GET` completo y guardara el cuerpo: peor que la sonda de externas, que al
    // menos no descarga nada. El fichero es entrada no confiable; su lista de URLs también.
    let mut scope = ResumeScope::of(&job, &base_url);
    // **El objetivo de un rastreo nuevo lo declara la línea de comandos; el de una reanudación
    // lo declara el fichero.** Esa es toda la diferencia, y es la que decide aquí.
    //
    // El perímetro deja alcanzar una dirección privada cuando *todos* los objetivos del rastreo
    // son locales: es el `astro dev` y el pre del cliente en la LAN, y quien lanzó ese rastreo
    // ya estaba dentro de esa red. En una reanudación ese razonamiento se cae, porque quien
    // declara el objetivo no es quien lanza el comando: basta un `.sqlite` que diga
    // `base_url = http://localhost:4321/` y traiga una fila externa inyectada para que la
    // máquina que lo reanuda sondee su propio loopback. Comprobado: `HEAD /app/kibana`,
    // `status=200` escrito en el fichero.
    //
    // Y el fichero es entrada no confiable **por diseño**: el producto existe para que un
    // rastreo se copie y se envíe. Es la misma razón por la que aquí se neutralizan `tier`,
    // `ignore_robots` y `follow_external`.
    //
    // Lo que cuesta: reanudar una auditoría local legítima deja sus sondas locales sin repetir,
    // registradas con motivo `local_network` en vez de comprobadas. No se pierde nada en
    // silencio, y quien lo necesite tiene la puerta buena — **relanzar `crawl`**, donde el
    // objetivo vuelve a venir de la línea de comandos y el perímetro se abre con conocimiento
    // de causa.
    if scope.screen.is_local_audit() {
        tracing::warn!(
            base_url,
            "the file declares a local target: on resume its network is not reachable, because \
             that target comes from the file and not from the command line. Re-run `crawl` to \
             audit it again."
        );
        scope.screen = scope.screen.without_local_audit();
    }

    // Las pendientes vuelven a la cola con su profundidad guardada y en su orden de
    // descubrimiento: el frontier sirve por niveles, así que el BFS queda como estaba.
    {
        let mut stmt = conn.prepare(
            "SELECT url, url_hash, depth FROM urls
             WHERE crawl_state = 'pending'
             ORDER BY COALESCE(depth, 0), id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<u32>>(2)?))
        })?;
        for row in rows {
            let (url, hash, depth) = row?;
            let Ok(parsed) = Url::parse(&url) else {
                tracing::warn!(url, "reanudación: URL pendiente ilegible; se omite");
                continue;
            };
            if let Err(motivo) = scope.admits(&parsed) {
                tracing::warn!(
                    url,
                    motivo,
                    "reanudación: URL pendiente fuera del alcance del rastreo; no se pide"
                );
                continue;
            }
            let depth = depth.unwrap_or(0);
            // `urls` no guarda de dónde salió cada pendiente; se reconstruye por contexto.
            // Solo afecta a `pages.crawl_depth_source` de lo que se rastree ahora.
            let source = if is_list {
                DiscoverySource::List
            } else if depth == 0 && in_sitemap.contains(&hash) {
                DiscoverySource::Sitemap
            } else {
                DiscoverySource::Link
            };
            frontier.enqueue(
                QueuedUrl { url: parsed, depth, discovered_from: None, source },
                hash,
            );
        }
    }

    // Todo lo demás —rastreado, con error, excluido, externo— se marca como visto: la promesa
    // de la reanudación es continuar, no repetir.
    {
        let mut stmt =
            conn.prepare("SELECT url_hash FROM urls WHERE crawl_state != 'pending'")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        for hash in rows {
            frontier.mark_seen(hash?);
        }
    }

    let already_fetched: i64 =
        conn.query_row("SELECT COUNT(*) FROM urls WHERE crawl_state = 'done'", [], |r| r.get(0))?;

    // Las sondas que el corte pilló sin respuesta. Se releen solo si este rastreo las hace:
    // con `--no-external-check` o con `follow_external`, esas filas no son sondas a medias.
    let mut pending_probes = Vec::new();
    let mut rejected_probes = Vec::new();
    if job.limits.check_external && !job.limits.follow_external {
        let mut stmt = conn.prepare(
            "SELECT url FROM urls
             WHERE is_internal = 0 AND crawl_state = 'skipped'
               AND status_code IS NULL AND error_kind IS NULL AND exclusion_reason IS NULL",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for row in rows {
            let url = row?;
            let Ok(parsed) = Url::parse(&url) else { continue };
            // El perímetro se vuelve a aplicar aquí, y no vale con que se aplicara al
            // descubrirlas: estas URLs vienen del fichero, no de un enlace recién resuelto.
            //
            // Usaba `is_probeable_host` con guarda, que deja fuera `is_cloud_metadata`, la única
            // criba que el código declara incondicional: un rastreo de `localhost` con un enlace
            // a `169.254.169.254`, cortado y reanudado, mandaba la petición. Ahora es el mismo
            // `NetworkScreen` del rastreo, que la incluye.
            if !normalize::is_crawlable_scheme(&parsed) || !scope.screen.allows_host(&parsed) {
                tracing::warn!(url, "reanudación: sonda externa fuera del perímetro; se descarta");
                rejected_probes.push(parsed);
                continue;
            }
            pending_probes.push(parsed);
        }
    }

    Ok((
        job,
        ResumeSetup {
            crawl_id,
            frontier,
            in_sitemap,
            already_fetched: already_fetched.max(0) as u64,
            pending_probes,
            rejected_probes,
        },
    ))
}

/// Hasta dónde puede llegar una reanudación al releer las URLs de su fichero.
///
/// El criterio es el mismo con el que el motor decide qué es interno mientras rastrea
/// (`normalize::is_internal`), aplicado ahora a lo que el fichero dice que quedó pendiente: los
/// hosts que los metadatos —ya validados contra `config_json`— declaran como objetivo del
/// rastreo, y nada más.
struct ResumeScope {
    /// Hosts admitidos, en minúsculas. Uno en `http` y `filesystem`; los de la lista guardada
    /// en modo `list`, que legítimamente puede mezclar dominios.
    hosts: std::collections::HashSet<String>,
    /// El perímetro de red, construido con esos mismos hosts como objetivos declarados.
    ///
    /// Es el mismo objeto y el mismo criterio que usa el rastreo (`network_screen`): auditar
    /// `http://localhost:4321/` —un `astro dev`— o el pre de un cliente en la red de la oficina
    /// sigue funcionando, porque esos hosts son el objetivo; lo que no pasa es que una URL de
    /// red interna colada en el fichero se pida por ser el fichero quien lo pide.
    screen: normalize::NetworkScreen,
}

impl ResumeScope {
    fn of(job: &CrawlJob, base_url: &str) -> Self {
        let host_of = |raw: &str| {
            Url::parse(raw).ok().and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        };
        let hosts: std::collections::HashSet<String> = match &job.mode {
            // Una lista puede mezclar hosts a propósito, y `validate_resume_scope` solo puede
            // contrastar el primero contra los metadatos: los demás pasan por la criba.
            CrawlMode::List { urls } => urls.iter().filter_map(|u| host_of(u)).collect(),
            _ => host_of(base_url).into_iter().collect(),
        };
        let screen = normalize::NetworkScreen::for_targets(hosts.iter());
        Self { hosts, screen }
    }

    fn admits(&self, url: &Url) -> std::result::Result<(), &'static str> {
        if !normalize::is_crawlable_scheme(url) {
            return Err("no es http ni https");
        }
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        if !self.hosts.contains(&host) {
            return Err("su host no es el del rastreo");
        }
        // Un host del rastreo ya es objetivo declarado, así que aquí solo puede caer lo que se
        // criba **siempre**: el rango de metadatos de nube.
        if !self.screen.allows_host(url) {
            return Err("es una dirección que la auditoría no pide nunca");
        }
        Ok(())
    }
}

/// Rechaza una configuración guardada cuyo objetivo no cuadra con los metadatos del fichero.
///
/// No es una defensa completa —quien fabrica el fichero entero puede fabricar también unos
/// metadatos coherentes— pero corta el ataque barato (inyectar solo `config_json` en un
/// fichero legítimo, o pegarle el de otro rastreo) y garantiza que el error diga la verdad:
/// «este fichero no describe el rastreo que dice continuar». La defensa contra el fichero
/// fabricado del todo es la otra mitad: `tier` e `ignore_robots` vivos, no guardados.
fn validate_resume_scope(
    job: &CrawlJob,
    base_url: &str,
    meta_mode: &str,
    source_path: Option<&str>,
    store_path: &Path,
) -> Result<()> {
    let incoherente = |detalle: String| {
        not_resumable(
            store_path,
            format!("su configuración guardada no es coherente con sus metadatos: {detalle}"),
        )
    };
    if job.mode.as_str() != meta_mode {
        return Err(incoherente(format!(
            "la configuración es de un rastreo '{}' y los metadatos dicen '{meta_mode}'",
            job.mode.as_str()
        )));
    }
    // Se compara el host y no la cadena entera: es lo que define el alcance, y es el mismo
    // criterio (`is_internal`) con el que el motor decide qué rastrea.
    let host_de = |raw: &str| {
        Url::parse(raw)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
    };
    let host_meta = host_de(base_url);
    let mismo_host = |raw: &str| host_meta.is_some() && host_de(raw) == host_meta;
    match &job.mode {
        CrawlMode::Http { seed } => {
            if !mismo_host(seed) {
                return Err(incoherente(format!(
                    "la semilla '{seed}' no es del sitio de los metadatos ('{base_url}')"
                )));
            }
        }
        CrawlMode::Filesystem { root, base } => {
            if !mismo_host(base) {
                return Err(incoherente(format!(
                    "la base '{base}' no es del sitio de los metadatos ('{base_url}')"
                )));
            }
            // `crawl_meta.source_path` guarda la raíz con la que se rastreó; una raíz
            // distinta en `config_json` es el ataque de recorrer otro directorio —el disco
            // entero, con `root: "/"`— con la apariencia de continuar un rastreo legítimo.
            if source_path != Some(root.display().to_string().as_str()) {
                return Err(incoherente(format!(
                    "la raíz '{}' no es la de los metadatos ('{}')",
                    root.display(),
                    source_path.unwrap_or("ninguna")
                )));
            }
        }
        CrawlMode::List { urls } => {
            // Los metadatos de una lista guardan su primera URL como `base_url`; una lista
            // puede mezclar hosts legítimamente, así que es lo único contrastable.
            match urls.first() {
                Some(first) if mismo_host(first) => {}
                Some(first) => {
                    return Err(incoherente(format!(
                        "la lista empieza en '{first}' y los metadatos dicen '{base_url}'"
                    )));
                }
                None if base_url.is_empty() => {}
                None => {
                    return Err(incoherente(format!(
                        "la lista está vacía y los metadatos dicen '{base_url}'"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Reenvía al hilo escritor las filas de `urls` que ya existen en el fichero.
///
/// No es para reescribirlas —el upsert con sus valores actuales las deja como están— sino para
/// poblar el índice hash→id del escritor, que arranca vacío en cada sesión: `links` e `images`
/// resuelven sus extremos contra ese índice en memoria, nunca con un JOIN por fila, y las
/// páginas que se rastreen ahora enlazan a URLs escritas por la sesión anterior. Sin la
/// reposición, cada uno de esos enlaces se descartaría en silencio: la misma clase de fallo
/// que hizo desaparecer las 506 imágenes de un rastreo truncado.
///
/// Se lee por tramos de id con una conexión de solo lectura —el escritor ya está en marcha y
/// WAL admite lectores— para no cargar el rastreo entero en memoria.
async fn resend_existing_rows(store_path: &Path, writer: &writer::WriterHandle) -> Result<u64> {
    use rusqlite::OpenFlags;
    let conn = Connection::open_with_flags(
        store_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    let mut last_id = 0i64;
    let mut total = 0u64;
    loop {
        let mut tramo: Vec<(i64, UrlRow)> = Vec::new();
        {
            let mut stmt = conn.prepare_cached(
                "SELECT id, url, url_hash, scheme, host, path, query, depth, is_internal,
                        in_sitemap, crawl_state, exclusion_reason, status_code,
                        redirect_chain_len, content_type, content_length, response_time_ms,
                        fetched_at, error_kind, error_message
                 FROM urls WHERE id > ?1 ORDER BY id LIMIT 512",
            )?;
            let rows = stmt.query_map([last_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    UrlRow {
                        url: r.get(1)?,
                        url_hash: r.get(2)?,
                        scheme: r.get(3)?,
                        host: r.get(4)?,
                        path: r.get(5)?,
                        query: r.get(6)?,
                        depth: r.get(7)?,
                        discovered_from: None,
                        is_internal: r.get::<_, i64>(8)? != 0,
                        in_sitemap: r.get::<_, i64>(9)? != 0,
                        crawl_state: crawl_state_from_db(&r.get::<_, String>(10)?),
                        exclusion_reason: r
                            .get::<_, Option<String>>(11)?
                            .as_deref()
                            .and_then(exclusion_reason_from_db),
                        status_code: r.get(12)?,
                        // `redirect_to` ya está resuelto a id en la fila; con el hash a None
                        // el segundo paso del escritor no lo toca.
                        redirect_to_hash: None,
                        redirect_chain_len: r.get::<_, Option<u32>>(13)?.unwrap_or(0),
                        content_type: r.get(14)?,
                        content_length: r.get::<_, Option<i64>>(15)?.map(|v| v as u64),
                        response_time_ms: r.get(16)?,
                        fetched_at: r.get(17)?,
                        error_kind: r.get(18)?,
                        error_message: r.get(19)?,
                    },
                ))
            })?;
            for row in rows {
                tramo.push(row?);
            }
        }
        if tramo.is_empty() {
            return Ok(total);
        }
        for (id, row) in tramo {
            last_id = id;
            total += 1;
            // La fila de `resources` se repone junto con la de `urls`. Cubre dos casos: el
            // upsert del escritor la deja idéntica si ya estaba, y la **crea** si el fichero
            // es de un esquema anterior a que el motor poblara la tabla — así un rastreo
            // interrumpido antes de la migración 008 no queda con la tabla a medias.
            let resource = row
                .status_code
                .is_some()
                .then(|| Url::parse(&row.url).ok())
                .flatten()
                .and_then(|u| {
                    resource_row(
                        row.url_hash,
                        row.content_type.as_deref(),
                        row.status_code,
                        row.content_length,
                        &u,
                    )
                });
            writer.send(CrawlResult { url: Some(row), resource, ..Default::default() }).await?;
        }
    }
}

/// `urls.crawl_state` → [`CrawlState`]. Un valor desconocido —de un fichero manipulado— se
/// trata como `skipped`: no se reencola ni se pierde.
fn crawl_state_from_db(s: &str) -> CrawlState {
    match s {
        "pending" => CrawlState::Pending,
        "done" => CrawlState::Done,
        "error" => CrawlState::Error,
        "excluded" => CrawlState::Excluded,
        _ => CrawlState::Skipped,
    }
}

fn exclusion_reason_from_db(s: &str) -> Option<ExclusionReason> {
    match s {
        "robots" => Some(ExclusionReason::Robots),
        "nofollow" => Some(ExclusionReason::Nofollow),
        "depth" => Some(ExclusionReason::Depth),
        "pattern" => Some(ExclusionReason::Pattern),
        "limit" => Some(ExclusionReason::Limit),
        "local_network" => Some(ExclusionReason::LocalNetwork),
        _ => None,
    }
}

/// Saca la siguiente URL despachable: la primera, en orden BFS, de un host que tenga hueco.
///
/// Es lo que hace que el límite de ritmo sea **por host y no global**: antes el pool entero se
/// dimensionaba con el límite del host semilla, así que con `follow_external` todos los hosts
/// compartían el freno del semilla — si un 503 lo reducía a 1, los demás también rastreaban de
/// uno en uno. Ver `CONVENTIONS.md §4`.
///
/// La decisión de a quién le queda hueco vive aquí; la de cómo encontrarlo sin recorrer la cola
/// entera, en [`Frontier::dequeue_dispatchable`], que es donde está el recuento por host.
fn next_dispatchable(
    frontier: &mut Frontier,
    throttle: &crate::throttle::Throttle,
    in_flight_by_host: &HashMap<String, usize>,
) -> Option<QueuedUrl> {
    frontier.dequeue_dispatchable(MAX_FRONTIER_SCAN, |host| {
        let in_flight = in_flight_by_host.get(host).copied().unwrap_or(0);
        in_flight < throttle.limit_for(host) as usize
    })
}

/// Libera el hueco que ocupaba una tarea en el recuento por host.
///
/// Se resuelve por el identificador de la tarea y no por la URL: una tarea que muere en pánico
/// no devuelve su URL, y sin esto su hueco quedaría ocupado para siempre — con el límite del
/// host en 1, ese host no volvería a rastrearse en todo el rastreo.
fn release_slot(
    host_by_task: &mut HashMap<tokio::task::Id, String>,
    in_flight_by_host: &mut HashMap<String, usize>,
    id: tokio::task::Id,
) {
    let Some(host) = host_by_task.remove(&id) else { return };
    if let Some(count) = in_flight_by_host.get_mut(&host) {
        *count = count.saturating_sub(1);
        // Sin esta limpieza, un rastreo con `follow_external` acumularía una entrada por cada
        // host externo visto, para siempre.
        if *count == 0 {
            in_flight_by_host.remove(&host);
        }
    }
}

/// Qué pasó al intentar rastrear una URL.
enum UrlOutcome {
    Excluded(ExclusionReason),
    Failed(crate::fetch::FetchFailure),
    /// Va en un `Box` porque es veinte veces mayor que las otras variantes y se construye
    /// una por URL rastreada: sin indirección, cada exclusión pagaría el tamaño de un
    /// documento completo.
    Fetched(Box<FetchedOutcome>),
    /// El resultado de la comprobación de estado de una URL externa (`check_external`):
    /// solo cabeceras, sin cuerpo, sin parseo y sin enlaces.
    ExternalChecked(std::result::Result<crate::fetch::StatusProbe, crate::fetch::FetchFailure>),
}

/// Por qué una URL externa se registra **sin** comprobar su estado.
///
/// Existe para que ninguna se quede en silencio: la fila se escribe igual —el informe necesita
/// saber a dónde apunta el sitio— y el motivo va al recuento de
/// [`CrawlMetrics::externals_unchecked`] y al log. Es el mismo principio que ya aplicaba
/// `max_external`: un tope que trunca sin decirlo hace que el informe parezca completo cuando
/// no lo está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotProbed {
    /// La URL casa con un `--exclude` (o queda fuera de un `--include`) del usuario.
    Pattern,
    /// El enlace lleva `rel="nofollow"` y `respect_nofollow` está activo.
    Nofollow,
    /// El host no es una dirección de internet: loopback, red privada, `localhost`… Ver
    /// `normalize::is_probeable_host`.
    LocalNetwork,
    /// Tope global de sondas del rastreo (`max_external`).
    Budget,
    /// Tope de sondas de un mismo host ([`MAX_EXTERNAL_PER_HOST`]).
    HostBudget,
    /// El host dejó de responder ([`EXTERNAL_HOST_FAILURE_STREAK`] fallos seguidos).
    HostUnreachable,
    /// El host respondió 429 o 503: pidió que dejáramos de pedirle cosas.
    RateLimited,
    /// El rastreo se cortó —a petición o por presupuesto— con la sonda aún en cola.
    Interrupted,
}

impl NotProbed {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pattern => "pattern",
            Self::Nofollow => "nofollow",
            Self::LocalNetwork => "local_network",
            Self::Budget => "max_external",
            Self::HostBudget => "max_external_per_host",
            Self::HostUnreachable => "host_unreachable",
            Self::RateLimited => "rate_limited",
            Self::Interrupted => "interrupted",
        }
    }

    /// Lo que se deja escrito en `urls.exclusion_reason`, si el motivo es una propiedad de la
    /// URL y no una circunstancia de la red de esta sesión.
    ///
    /// Solo esos tres: son los que **no** deben reintentarse al reanudar —el `--exclude`, el
    /// `nofollow` y la dirección de red interna seguirán siendo los mismos mañana— y son los
    /// que el informe puede explicar en términos del sitio. Los demás dejan la fila como
    /// estaba, sin estado y sin motivo, que es lo que hace que una reanudación vuelva a
    /// intentarlos: son de esta sesión, no del sitio.
    ///
    /// `LocalNetwork` devolvía `None`, y eso dejaba la fila **idéntica** a la de una sonda que
    /// el corte pilló en vuelo: el `SELECT` de la reanudación la volvía a coger y la petición
    /// que la criba había rechazado salía en la sesión siguiente. Reproducido sin tocar el
    /// fichero: rastreo local con un enlace a `169.254.169.254`, Ctrl-C, `resume`, y la
    /// petición sale.
    fn exclusion_reason(self) -> Option<ExclusionReason> {
        match self {
            Self::Pattern => Some(ExclusionReason::Pattern),
            Self::Nofollow => Some(ExclusionReason::Nofollow),
            Self::LocalNetwork => Some(ExclusionReason::LocalNetwork),
            _ => None,
        }
    }
}

/// ¿Hay algún motivo para **no** pedirle su estado a esta URL externa?
///
/// Tres de los cuatro motivos son decisiones que ya estaban tomadas y la sonda se saltaba:
///
/// - **`--exclude` / `--include`.** La rama de externas hacía `continue` antes de evaluar los
///   patrones, así que `--exclude 'dominio\.com'` no impedía la petición: excluía del rastreo
///   algo que nunca se iba a rastrear y dejaba pasar lo único que sí se pedía.
/// - **`rel="nofollow"`.** Lo mismo, y con un caso concreto detrás: los enlaces de spam de los
///   comentarios de un WordPress llevan `rel="ugc nofollow"`. Sin esto se le manda un `HEAD`
///   con nuestro user-agent de bot a cada dominio de spam acumulado en años, y sus 404 salen
///   publicados como hallazgos del sitio auditado.
/// - **El perímetro de red** (`normalize::is_probeable_host`), que no existía. Ver su doc.
/// - Y el tope global de sondas, que ya estaba.
///
/// La fila de `urls` se escribe igual en todos los casos: lo que se salta es la petición.
fn why_not_probe(
    url: &Url,
    is_nofollow: bool,
    filter: &crate::pattern::UrlFilter,
    limits: &crate::job::CrawlLimits,
    enqueued: u64,
    screen: &normalize::NetworkScreen,
) -> Option<NotProbed> {
    if !filter.allows(url.as_str()) {
        return Some(NotProbed::Pattern);
    }
    if limits.respect_nofollow && is_nofollow {
        return Some(NotProbed::Nofollow);
    }
    if !screen.allows_host(url) {
        return Some(NotProbed::LocalNetwork);
    }
    if enqueued >= limits.max_external {
        return Some(NotProbed::Budget);
    }
    None
}

/// Cola de comprobaciones de estado de URLs externas.
///
/// Aparte del frontier a propósito: estas URLs no forman parte del rastreo (no cuentan contra
/// `max_urls`, no se parsean, no aportan enlaces) y su único orden relevante es «una petición
/// en vuelo por host ajeno». Se agrupa por host para que buscar una despachable recorra la
/// ronda de hosts y no las URLs retenidas: con el límite en 1 por `force_serial`, un host con
/// cien URLs pendientes tendría noventa y nueve retenidas en cada rellenado del pool.
///
/// La deduplicación no vive aquí: el índice de vistas del frontier corta las repeticiones
/// antes de llegar a `push`.
///
/// # El estado por host es la mitad del trabajo
///
/// Sondar a terceros tiene dos costes que el tope global no acota: **un host muerto** —cada
/// URL suya paga el timeout entero, en serie— y **un host que se queja**. Por eso cada host
/// lleva su cuenta de sondas, su racha de fallos y su cierre. Al cerrarse, lo que le quedara
/// en cola no desaparece: se cuenta como no comprobado con su motivo.
#[derive(Default)]
struct ExternalQueue {
    by_host: HashMap<String, HostProbes>,
    /// Ronda de hosts con trabajo. Invariante: un host está aquí ⟺ su cola existe y no está
    /// vacía.
    hosts: VecDeque<String>,
    pending: usize,
}

/// Lo que la cola sabe de un host ajeno.
#[derive(Default)]
struct HostProbes {
    queue: VecDeque<Url>,
    /// Sondas aceptadas para este host, contra [`MAX_EXTERNAL_PER_HOST`].
    accepted: u32,
    /// Fallos de red seguidos. Cualquier respuesta —incluido un 404— la reinicia.
    failures: u32,
    /// Cerrado: no se le manda nada más y se dice por qué.
    closed: Option<NotProbed>,
}

impl ExternalQueue {
    /// Encola una sonda, o dice por qué no.
    fn push(&mut self, url: Url) -> std::result::Result<(), NotProbed> {
        let host = url.host_str().unwrap_or_default().to_string();
        let state = self.by_host.entry(host.clone()).or_default();
        if let Some(reason) = state.closed {
            return Err(reason);
        }
        if state.accepted >= MAX_EXTERNAL_PER_HOST {
            return Err(NotProbed::HostBudget);
        }
        // Lo que decide si el host entra en la ronda es **su cola**, no si se le había visto
        // antes: por el invariante de `hosts`, una cola vacía significa que el host no está
        // en la ronda, la haya tenido llena antes o no.
        //
        // Mirar `by_host` en su lugar —«¿es un host nuevo?»— dejaba la sonda muerta en cuanto
        // un host repetía: su entrada sobrevive al vaciado, porque guarda el cupo y la racha
        // de fallos, así que el push no lo devolvía a la ronda y la URL no la despachaba
        // nadie. Salía con dos destinos al mismo host descubiertos en momentos distintos, que
        // es justo lo que hacen dos redirecciones a la misma tienda; en `debug` reventaba el
        // invariante del planificador y en `release` esas sondas acababan contadas como «sin
        // comprobar» al terminar, sin más explicación.
        let fuera_de_la_ronda = state.queue.is_empty();
        state.accepted += 1;
        state.queue.push_back(url);
        if fuera_de_la_ronda {
            self.hosts.push_back(host);
        }
        self.pending += 1;
        Ok(())
    }

    /// La siguiente URL cuyo host tiene hueco, rotando la ronda de hosts para que uno lento
    /// no acapare el turno.
    fn next_dispatchable(
        &mut self,
        throttle: &crate::throttle::Throttle,
        in_flight_by_host: &HashMap<String, usize>,
    ) -> Option<Url> {
        for _ in 0..self.hosts.len() {
            let host = self.hosts.pop_front()?;
            let in_flight = in_flight_by_host.get(&host).copied().unwrap_or(0);
            if in_flight >= throttle.limit_for(&host) as usize {
                self.hosts.push_back(host);
                continue;
            }
            let Some(state) = self.by_host.get_mut(&host) else { continue };
            let url = state.queue.pop_front();
            if !state.queue.is_empty() {
                self.hosts.push_back(host);
            }
            if let Some(url) = url {
                self.pending -= 1;
                return Some(url);
            }
        }
        None
    }

    /// Anota lo que respondió —o dejó de responder— una sonda y devuelve cuántas URLs de ese
    /// host quedan sin comprobar por ello, con su motivo.
    ///
    /// Aquí está el cortocircuito: tres ausencias seguidas cierran el host. Una respuesta,
    /// cualquiera, reinicia la racha; un 429 o un 503 lo cierran de golpe, que es lo que
    /// significa «educado con quien no es nuestro».
    fn record(
        &mut self,
        host: &str,
        outcome: std::result::Result<u16, &crate::fetch::FetchFailure>,
    ) -> Option<(usize, NotProbed)> {
        let reason = {
            let state = self.by_host.get_mut(host)?;
            match outcome {
                Ok(status) if crate::throttle::is_overload(status) => {
                    state.failures = 0;
                    NotProbed::RateLimited
                }
                Ok(_) => {
                    state.failures = 0;
                    return None;
                }
                Err(_) => {
                    state.failures += 1;
                    if state.failures < EXTERNAL_HOST_FAILURE_STREAK {
                        return None;
                    }
                    NotProbed::HostUnreachable
                }
            }
        };
        Some((self.close(host, reason), reason))
    }

    /// Cierra un host y devuelve cuántas sondas suyas se quedan sin mandar.
    fn close(&mut self, host: &str, reason: NotProbed) -> usize {
        let Some(state) = self.by_host.get_mut(host) else { return 0 };
        state.closed = Some(reason);
        let dropped = state.queue.len();
        state.queue.clear();
        self.pending -= dropped;
        self.hosts.retain(|h| h != host);
        dropped
    }

    fn pending(&self) -> usize {
        self.pending
    }
}

struct FetchedOutcome {
    doc: FetchedDoc,
    page: Option<ParsedPage>,
    blocked_by_robots: bool,
}

async fn process_url<F: Fetcher>(
    fetcher: &F,
    robots: &RobotsCache,
    throttle: &crate::throttle::Throttle,
    pacer: &crate::throttle::CrawlDelayGate,
    job: &CrawlJob,
    item: &QueuedUrl,
) -> UrlOutcome {
    let mut blocked_by_robots = false;
    // El permiso del host cuando su robots.txt declara `Crawl-delay`. Se suelta al final de
    // la función, con la petición ya respondida: mientras vive no arranca otra de ese host.
    let mut delay_permit: Option<crate::throttle::CrawlDelayPermit> = None;
    if let Some(host) = item.url.host_str() {
        // El robots.txt se carga siempre, también con `ignore_robots`: ese flag es justo el
        // único camino por el que una página bloqueada llega a rastrearse, y el informe tiene
        // que poder decir cuáles lo están (`IndexabilityReason::Robots` es la causa raíz más
        // prioritaria de `evaluate_indexability`). Si el fichero no se puede recuperar,
        // `load_host_rules` cae a `allow_all` sin tumbar el rastreo, y no se marca nada:
        // no se afirma lo que no se sabe.
        let rules = load_host_rules(fetcher, robots, &item.url, host, job).await;
        if job.limits.ignore_robots {
            // Ignorarlo es ignorarlo entero: ni exclusión ni `Crawl-delay`. Solo se anota
            // qué habría bloqueado, que es lo que el usuario pidió ver.
            blocked_by_robots = !rules.allows(&item.url);
        } else {
            if !rules.allows(&item.url) {
                return UrlOutcome::Excluded(ExclusionReason::Robots);
            }
            // Crawl-delay anula la concurrencia configurada para este host (`robots.rs`):
            // el freno baja su límite a 1 para que el pool deje de despacharlo en paralelo,
            // y la puerta serializa las peticiones que ya estaban despachadas cuando el
            // robots.txt aún no se conocía, espaciando los arranques al menos `delay`.
            // La espera vive dentro de esta tarea a propósito: el `select!` del bucle la
            // corta al vencer `max_duration`, igual que cortaba el `sleep` antiguo.
            if let Some(delay) = rules.crawl_delay {
                throttle.force_serial(host);
                delay_permit = Some(pacer.acquire(host, delay).await);
            }
        }
    }

    let outcome = match fetcher.fetch(&item.url).await {
        Ok(Ok(doc)) => {
            let page = if doc.is_html() && !doc.body.is_empty() {
                Some(parse::parse_html(&doc.body, job.collect_body_text))
            } else {
                None
            };
            UrlOutcome::Fetched(Box::new(FetchedOutcome {
                doc,
                page,
                blocked_by_robots,
            }))
        }
        Ok(Err(failure)) => UrlOutcome::Failed(failure),
        Err(e) => UrlOutcome::Failed(crate::fetch::FetchFailure {
            kind: crate::fetch::ErrorKind::Connection,
            message: e.to_string(),
        }),
    };
    // El permiso cubre la petición entera, no solo su arranque: soltarlo antes del `fetch`
    // permitiría dos en vuelo en cuanto el retardo fuera menor que la latencia del servidor.
    drop(delay_permit);
    outcome
}

/// Carga (y cachea) el `robots.txt` de un host, **una sola vez aunque lleguen N tareas a la
/// vez**.
///
/// La descarga va dentro de la puerta de la caché y no antes: consultar primero y descargar
/// después dejaba que la primera ola de un host —`concurrency` tareas despachadas de golpe—
/// fallara el `get` en bloque y pidiera el fichero N veces. Ver `RobotsCache`.
async fn load_host_rules<F: Fetcher>(
    fetcher: &F,
    cache: &RobotsCache,
    url: &Url,
    host: &str,
    job: &CrawlJob,
) -> Arc<HostRules> {
    cache
        .get_or_fetch(host, || async {
            match crate::robots::robots_url_for(url) {
                Some(robots_url) => match fetcher.fetch(&robots_url).await {
                    Ok(Ok(doc)) if doc.status == 200 => {
                        HostRules::parse(&doc.body, &job.limits.user_agent)
                    }
                    // Un 404 de robots.txt es lo normal, no un problema. Pero se anota qué
                    // respondió: la regla que avisa de su ausencia necesita distinguir un 404
                    // de un fallo de red, y con `allow_all()` a secas las dos cosas eran
                    // indistinguibles.
                    Ok(Ok(doc)) => HostRules::absent(Some(doc.status)),
                    _ => HostRules::absent(None),
                },
                None => HostRules::allow_all(),
            }
        })
        .await
}

/// Máximo de sitemaps que se descargan en un rastreo.
///
/// La protección contra bucles no es este número: es el conjunto `visited`, que impide volver
/// a pedir un sitemap ya leído aunque un índice se apunte a sí mismo. Este tope es solo un
/// cortafuegos ante un sitio absurdo, y por eso puede ser generoso.
///
/// Estaba en 50, que parecía de sobra hasta que un medio real declaró **1.179 sitemaps** de
/// unas 150 URLs cada uno: con el tope antiguo se habría rastreado el 4% del sitio sin avisar,
/// que es peor que fallar.
const MAX_SITEMAPS: usize = 5_000;

/// Descubre sitemaps por `robots.txt` y por las rutas convencionales, y los sigue.
///
/// Se descargan en paralelo y **con reposición continua**, como el bucle principal del rastreo:
/// en cuanto un sitemap responde se lanza el siguiente. La versión anterior, al llenar la
/// concurrencia, vaciaba la ronda **entera** antes de lanzar nada nuevo — el defecto de las
/// tandas, un fallo medido en su día (42,8 s frente a 13,4 s teóricos)
/// reaparecido en este camino: cada tanda costaba lo que su petición más lenta, y con los
/// 1.179 sitemaps que declara un medio real y concurrencia 8 eran 148 tandas. Medido en el
/// banco de pruebas (24 hijos, un lento de 600 ms por tanda): 1,86 s por tandas, ~0,7 s con
/// reposición continua.
///
/// Hacerlo secuencial era aún peor: 10,9 s en un sitio real frente a 0,76 s del rastreo entero.
///
/// # Los documentos de sitemap pasan por el mismo perímetro que cualquier URL
///
/// Las líneas `Sitemap:` del `robots.txt` y los `<loc>` hijos de un `<sitemapindex>` se pedían
/// con `fetcher.fetch()` a pelo: sin comprobar esquema, ni host, ni el `robots.txt` del
/// destino, ni pasar por el `Throttle`. Era la mitad que quedó fuera del arreglo de las URLs
/// *declaradas dentro* de un sitemap (ver el comentario del bucle de `run_with`): un
/// `robots.txt` ajeno con `Sitemap: http://169.254.169.254/latest/meta-data/…` se descargaba
/// desde la máquina del usuario y el resultado quedaba en la tabla `sitemaps` con
/// `status_code` y `bytes` — el fichero de rastreo como oráculo de la red interna, y con un
/// índice de 5.000 hijos apuntando a un tercero, una herramienta de ataque con la IP del
/// usuario. Mismos dos motivos de fondo: la promesa de que solo se rastrea el sitio auditado,
/// y `CONVENTIONS.md §1`.
///
/// El orden de las comprobaciones importa: esquema e interno **antes** que el `robots.txt`
/// del destino, porque consultar el `robots.txt` de un host ajeno ya sería la petición que
/// se quiere evitar.
///
/// `budget` corta la acumulación aquí dentro (ver [`collect_sitemap`]): el tope de `max_urls`
/// aplicado después de devolver el `Vec` completo no protege de nada.
async fn discover_sitemap_urls<F: Fetcher + 'static>(
    fetcher: &Arc<F>,
    cache: &RobotsCache,
    throttle: &crate::throttle::Throttle,
    seed: &Url,
    job: &CrawlJob,
    concurrency: usize,
    budget: Option<usize>,
) -> (Vec<Url>, Vec<SitemapRow>) {
    let Some(host) = seed.host_str() else {
        return (Vec::new(), Vec::new());
    };
    let seed_host = host.to_string();
    let rules = load_host_rules(&**fetcher, cache, seed, host, job).await;

    // Una URL más que el presupuesto: el bucle de `run_with` necesita ver la que desborda
    // para marcar el truncado por `max_urls`, igual que la veía con el vector sin acotar.
    let tope_found = budget.map(|b| b.saturating_add(1));

    // De dónde salió cada sitemap: lo anunciado en `robots.txt` no es lo mismo que lo que se
    // encontró probando las rutas convencionales, y en el informe importa la diferencia. Con la
    // reposición continua ya no hay «primera ronda» que delate a los convencionales, así que se
    // apuntan por su URL.
    let mut pending: VecDeque<String> = rules.sitemaps.clone().into();
    let anunciados: std::collections::HashSet<String> = pending.iter().cloned().collect();
    let mut convencionales: std::collections::HashSet<String> = std::collections::HashSet::new();
    for path in crate::sitemap::WELL_KNOWN_SITEMAP_PATHS {
        if let Ok(u) = seed.join(path) {
            let raw = u.to_string();
            convencionales.insert(raw.clone());
            pending.push_back(raw);
        }
    }

    let mut rows: Vec<SitemapRow> = Vec::new();
    let mut found = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut downloaded = 0usize;

    let mut in_flight = tokio::task::JoinSet::new();
    // Sitemaps ya filtrados cuyo host estaba al límite al despachar; se reintentan antes de
    // sacar nada nuevo, como las `deferred` del bucle principal.
    let mut deferred: VecDeque<(String, Url)> = VecDeque::new();
    let mut in_flight_by_host: HashMap<String, usize> = HashMap::new();
    let mut host_by_task: HashMap<tokio::task::Id, String> = HashMap::new();
    loop {
        // Rellenar hasta la concurrencia configurada: un sitio con cientos de sitemaps no debe
        // convertirse en cientos de peticiones simultáneas. Y si el presupuesto ya está lleno,
        // no se lanza nada más: cada descarga extra solo produciría URLs que se van a tirar.
        while in_flight.len() < concurrency
            && downloaded < MAX_SITEMAPS
            && tope_found.is_none_or(|t| found.len() < t)
        {
            let has_slot = |url: &Url, in_flight_by_host: &HashMap<String, usize>| {
                let h = url.host_str().unwrap_or_default();
                let vuelo = in_flight_by_host.get(h).copied().unwrap_or(0);
                vuelo < throttle.limit_for(h) as usize
            };
            // Primero una retenida cuyo host ya tenga hueco: conservan su orden y ya están
            // filtradas.
            let retenida = deferred
                .iter()
                .position(|(_, u)| has_slot(u, &in_flight_by_host))
                .and_then(|i| deferred.remove(i));
            let (raw, url) = match retenida {
                Some(v) => v,
                None => {
                    let Some(raw) = pending.pop_front() else { break };
                    // La protección contra bucles: un índice ya leído no se vuelve a pedir
                    // aunque otro índice —o él mismo— lo declare.
                    if !visited.insert(raw.clone()) {
                        continue;
                    }
                    let Ok(url) = Url::parse(&raw) else { continue };
                    if !normalize::is_crawlable_scheme(&url) {
                        continue;
                    }
                    if !normalize::is_internal(&url, &seed_host) && !job.limits.follow_external {
                        tracing::debug!(
                            url = %url,
                            "sitemap fuera del sitio auditado: no se descarga"
                        );
                        continue;
                    }
                    // El `robots.txt` del host de destino, como en `process_url`. Llega aquí
                    // solo lo interno (o lo externo pedido explícitamente), así que esta
                    // consulta no abre ningún host nuevo.
                    if !job.limits.ignore_robots {
                        if let Some(h) = url.host_str() {
                            let destino = load_host_rules(&**fetcher, cache, &url, h, job).await;
                            if !destino.allows(&url) {
                                tracing::debug!(
                                    url = %url,
                                    "sitemap bloqueado por el robots.txt de su host: no se descarga"
                                );
                                continue;
                            }
                        }
                    }
                    if !has_slot(&url, &in_flight_by_host) {
                        deferred.push_back((raw, url));
                        continue;
                    }
                    (raw, url)
                }
            };
            downloaded += 1;

            let origen = if anunciados.contains(&raw) {
                "robots"
            } else if convencionales.contains(&raw) {
                "well_known"
            } else {
                "index"
            };
            let host_tarea = url.host_str().unwrap_or_default().to_string();
            *in_flight_by_host.entry(host_tarea.clone()).or_insert(0) += 1;
            // El origen y la URL viajan **dentro de la tarea**, no en un vector paralelo.
            // Emparejarlos fuera por orden es un error: las tareas terminan en el orden en que
            // responden los servidores, no en el que se lanzaron, así que un `pop()` cruzaba las
            // etiquetas —un sitemap anunciado en `robots.txt` salía marcado como `well_known`— y,
            // desde que una fila puede ser un error, cruzaría también la URL a la que se culpa.
            let fetcher = Arc::clone(fetcher);
            let task = in_flight.spawn(async move { (origen, raw, fetcher.fetch(&url).await) });
            host_by_task.insert(task.id(), host_tarea);
        }

        // Se espera a **uno**, no a todos: sus hijos entran en `pending` y el rellenado de
        // arriba los lanza en cuanto hay hueco.
        let Some(joined) = in_flight.join_next_with_id().await else { break };
        let joined = match joined {
            Ok((task_id, v)) => {
                release_slot(&mut host_by_task, &mut in_flight_by_host, task_id);
                Ok(v)
            }
            Err(e) => {
                release_slot(&mut host_by_task, &mut in_flight_by_host, e.id());
                Err(e)
            }
        };
        collect_sitemap(joined, &mut pending, &mut found, &mut rows, tope_found, throttle);
    }

    (found, rows)
}

/// Lo que devuelve una tarea de descubrimiento de sitemaps: su origen y su URL viajan con ella.
type SitemapJoin = std::result::Result<
    (&'static str, String, Result<std::result::Result<FetchedDoc, crate::fetch::FetchFailure>>),
    tokio::task::JoinError,
>;

/// Lo que el rastreo tiene que dejar registrado al cerrar, más allá de las URLs.
///
/// Existe porque `finalize` acabó recibiendo ocho parámetros sueltos, que es la forma que tiene
/// el compilador de avisar de que faltaba un concepto: esto no son argumentos, es «la constancia
/// de lo que se consultó durante el rastreo».
struct CrawlRecords<'a> {
    /// `robots.txt` por host, tal como quedó en el caché.
    robots: &'a [(String, std::sync::Arc<HostRules>)],
    /// Raíz del sitio, para evaluar si el `robots.txt` bloquea el sitio entero.
    base: &'a Url,
    sitemaps: &'a [SitemapRow],
}

/// URL de reserva cuando un rastreo no tiene semillas con host. No debería ocurrir; existe para
/// no propagar un `Option` por toda la pasada final.
fn fallback_base_url() -> Url {
    #[allow(clippy::expect_used)]
    Url::parse("https://localhost/").expect("URL constante válida")
}

/// Deja constancia del `robots.txt` de cada host y de cada sitemap descargado.
///
/// Sin esto, los dos se usaban y se tiraban: al terminar el rastreo no quedaba forma de saber si
/// el `robots.txt` existía ni si un sitemap tenía el XML roto, y tres reglas del catálogo no
/// podían escribirse.
fn write_robots_and_sitemaps(
    conn: &mut Connection,
    records: &CrawlRecords<'_>,
) -> Result<()> {
    let CrawlRecords { robots, base, sitemaps } = records;
    let ahora = now_iso8601();
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO robots_txt (host, status_code, content, blocks_all, sitemap_count, fetched_at)
             VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(host) DO UPDATE SET
                 status_code   = excluded.status_code,
                 content       = excluded.content,
                 blocks_all    = excluded.blocks_all,
                 sitemap_count = excluded.sitemap_count,
                 fetched_at    = excluded.fetched_at",
        )?;
        for (host, rules) in *robots {
            // `blocks_all` se evalúa contra la raíz del host correspondiente, no contra la
            // semilla: en un rastreo con enlaces externos seguidos hay más de un host.
            let raiz = base
                .join("/")
                .ok()
                .and_then(|mut u| u.set_host(Some(host)).ok().map(|_| u))
                .unwrap_or_else(|| (*base).clone());
            stmt.execute(rusqlite::params![
                host,
                rules.status_code,
                rules.content,
                rules.blocks_all(&raiz) as i64,
                rules.sitemaps.len() as i64,
                ahora,
            ])?;
        }

        let mut stmt = tx.prepare(
            "INSERT INTO sitemaps
                 (url, status_code, is_index, is_valid, parse_error, url_count, bytes,
                  discovered_from, fetched_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(url) DO UPDATE SET
                 status_code = excluded.status_code,
                 is_index    = excluded.is_index,
                 is_valid    = excluded.is_valid,
                 parse_error = excluded.parse_error,
                 url_count   = excluded.url_count,
                 bytes       = excluded.bytes",
        )?;
        for sm in *sitemaps {
            stmt.execute(rusqlite::params![
                sm.url,
                sm.status_code,
                sm.is_index as i64,
                sm.is_valid as i64,
                sm.parse_error,
                sm.url_count as i64,
                sm.bytes as i64,
                sm.discovered_from,
                ahora,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Escribe los hallazgos de una regla de conjunto en una transacción propia.
///
/// Una transacción por regla y no una por rastreo: así el `Vec` de esa regla se suelta antes de
/// evaluar la siguiente. Con 971.000 hallazgos en un sitio de 500.000 URLs, acumularlos todos
/// costaba 330 MB de más.
fn write_site_issues(
    conn: &mut Connection,
    issues: &[(Option<i64>, crawlforge_rules::Issue)],
) -> Result<()> {
    if issues.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    {
        let mut with_url = tx.prepare(
            "INSERT INTO issues (url_id, rule_id, severity, category, detail_json, group_key)
             SELECT u.id, ?2, ?3, ?4, ?5, ?6 FROM urls u WHERE u.url_hash = ?1",
        )?;
        let mut site_wide = tx.prepare(
            "INSERT INTO issues (url_id, rule_id, severity, category, detail_json, group_key)
             VALUES (NULL, ?1, ?2, ?3, ?4, ?5)",
        )?;
        for (hash, issue) in issues {
            match hash {
                Some(h) => {
                    with_url.execute(rusqlite::params![
                        h,
                        issue.rule_id,
                        issue.severity.as_str(),
                        issue.category.as_str(),
                        issue.detail_json,
                        issue.group_key
                    ])?;
                }
                None => {
                    site_wide.execute(rusqlite::params![
                        issue.rule_id,
                        issue.severity.as_str(),
                        issue.category.as_str(),
                        issue.detail_json,
                        issue.group_key
                    ])?;
                }
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// Lo que se guarda de cada sitemap descargado. Se corresponde con una fila de `sitemaps`.
#[derive(Debug, Clone)]
pub struct SitemapRow {
    pub url: String,
    pub status_code: Option<u16>,
    pub is_index: bool,
    pub is_valid: bool,
    pub parse_error: Option<String>,
    pub url_count: u32,
    pub bytes: u64,
    pub discovered_from: &'static str,
}

/// Reparte el contenido de un sitemap descargado: los hijos vuelven a la cola, las URLs de
/// página se acumulan — hasta `tope_urls`, que es el presupuesto del rastreo más uno.
fn collect_sitemap(
    joined: SitemapJoin,
    pending: &mut VecDeque<String>,
    found: &mut Vec<Url>,
    rows: &mut Vec<SitemapRow>,
    tope_urls: Option<usize>,
    throttle: &crate::throttle::Throttle,
) {
    // Un sitemap que no se puede leer **deja constancia**. Antes este `else` se tragaba tres
    // casos distintos —la tarea muerta, el error del motor y el fallo de red— y no quedaba ni una
    // fila: un `/sitemap.xml` anunciado en `robots.txt` que no responde costaba dos minutos de
    // timeouts y luego el rastreo terminaba `done`, sin truncar, sin nada que dijera que todo lo
    // que ese sitemap declaraba se quedó sin descubrir. Es el mismo patrón de siempre:
    // rastrear una fracción del sitio sin avisar es peor que fallar.
    // Una tarea muerta no trae ni su origen ni su URL, así que es lo único que no se puede
    // atribuir; se registra en el log y se sigue.
    let (origen, url, resultado) = match joined {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "una tarea de sitemap ha muerto");
            return;
        }
    };
    let doc = match resultado {
        Ok(Ok(doc)) => doc,
        otro => {
            let motivo = match otro {
                Err(e) => format!("engine error: {e}"),
                Ok(Err(f)) => format!("{}: {}", f.kind.as_str(), f.message),
                Ok(Ok(_)) => unreachable!("caso cubierto arriba"),
            };
            tracing::warn!(url, motivo, "sitemap ilegible");
            rows.push(SitemapRow {
                url,
                status_code: None,
                is_index: false,
                is_valid: false,
                parse_error: Some(motivo),
                url_count: 0,
                bytes: 0,
                discovered_from: origen,
            });
            return;
        }
    };

    // El freno adaptativo también cuenta estas respuestas: un host que responde 429 a sus
    // sitemaps está dando la misma señal de sobrecarga que con cualquier otra URL.
    if let Some(h) = doc.url.host_str() {
        throttle.record(h, doc.status);
    }

    // La fila se escribe también cuando el sitemap responde mal: «el sitemap declarado en
    // robots.txt devuelve 404» es justo el hallazgo que hay que poder dar.
    if doc.status != 200 {
        rows.push(SitemapRow {
            url: doc.url.to_string(),
            status_code: Some(doc.status),
            is_index: false,
            is_valid: false,
            parse_error: None,
            url_count: 0,
            bytes: doc.body.len() as u64,
            discovered_from: origen,
        });
        return;
    }

    let parsed = crate::sitemap::parse(&doc.body);
    let is_index = !parsed.children.is_empty();
    rows.push(SitemapRow {
        url: doc.url.to_string(),
        status_code: Some(doc.status),
        is_index,
        is_valid: parsed.parse_error.is_none(),
        parse_error: parsed.parse_error.clone(),
        url_count: (parsed.urls.len() + parsed.children.len()) as u32,
        bytes: doc.body.len() as u64,
        discovered_from: origen,
    });

    // El presupuesto corta la acumulación **aquí**, no después de devolver el vector entero:
    // aplicarlo al escribir las filas acotaba el fichero, pero un `sitemapindex` de 200 hijos
    // de 50 MB seguía dando ~250 M de `Url` en memoria antes de rastrear la primera página.
    // Lleno el presupuesto, tampoco se encolan más hijos: cada uno solo aportaría URLs que se
    // van a tirar.
    let lleno = tope_urls.is_some_and(|t| found.len() >= t);
    if lleno {
        tracing::debug!(
            url = %doc.url,
            "sitemap: presupuesto de URLs lleno; ni se acumulan sus URLs ni se siguen sus hijos"
        );
        return;
    }
    pending.extend(parsed.children);
    for u in parsed.urls {
        if tope_urls.is_some_and(|t| found.len() >= t) {
            break;
        }
        if let Ok(u) = Url::parse(&u) {
            found.push(u);
        }
    }
}

fn url_hash(url: &Url) -> i64 {
    xxhash_rust::xxh3::xxh3_64(url.as_str().as_bytes()) as i64
}

fn base_row(item: &QueuedUrl, hash: i64, in_sitemap: bool) -> UrlRow {
    UrlRow {
        url: item.url.to_string(),
        url_hash: hash,
        scheme: item.url.scheme().to_string(),
        host: item.url.host_str().unwrap_or_default().to_string(),
        path: item.url.path().to_string(),
        query: item.url.query().map(|q| q.to_string()),
        depth: Some(item.depth),
        discovered_from: item.discovered_from,
        is_internal: true,
        in_sitemap,
        crawl_state: CrawlState::Pending,
        exclusion_reason: None,
        status_code: None,
        redirect_to_hash: None,
        redirect_chain_len: 0,
        content_type: None,
        content_length: None,
        response_time_ms: None,
        fetched_at: None,
        error_kind: None,
        error_message: None,
    }
}

fn excluded_row(
    item: &QueuedUrl,
    hash: i64,
    reason: ExclusionReason,
    in_sitemap: bool,
) -> UrlRow {
    let mut row = base_row(item, hash, in_sitemap);
    row.crawl_state = CrawlState::Excluded;
    row.exclusion_reason = Some(reason);
    row
}

fn failed_row(
    item: &QueuedUrl,
    hash: i64,
    failure: &crate::fetch::FetchFailure,
    in_sitemap: bool,
) -> UrlRow {
    let mut row = base_row(item, hash, in_sitemap);
    row.crawl_state = CrawlState::Error;
    row.error_kind = Some(failure.kind.as_str().to_string());
    row.error_message = Some(clip_third_party(failure.message.clone()));
    row.fetched_at = Some(now_iso8601());
    row
}

/// Fila de una URL descubierta pero aún no rastreada.
///
/// Emitirla no es opcional por dos motivos. Uno: `links` e `images` resuelven sus extremos con
/// un `JOIN` contra `urls`, así que un destino ausente hace que la fila se descarte en silencio
/// — con un rastreo truncado se perdían todas las imágenes del sitio. Dos: `03-MOTOR-CRAWL.md
/// §7` define la reanudación como releer las filas con `crawl_state='pending'`, y sin escribirlas
/// no hay nada que releer.
fn pending_row(n: &NormalizedUrl, hash: i64, depth: u32, internal: bool, in_sitemap: bool) -> UrlRow {
    UrlRow {
        url: n.normalized.to_string(),
        url_hash: hash,
        scheme: n.normalized.scheme().to_string(),
        host: n.normalized.host_str().unwrap_or_default().to_string(),
        path: n.normalized.path().to_string(),
        query: n.normalized.query().map(|q| q.to_string()),
        depth: Some(depth),
        discovered_from: None,
        is_internal: internal,
        in_sitemap,
        crawl_state: CrawlState::Pending,
        exclusion_reason: None,
        status_code: None,
        redirect_to_hash: None,
        redirect_chain_len: 0,
        content_type: None,
        content_length: None,
        response_time_ms: None,
        fetched_at: None,
        error_kind: None,
        error_message: None,
    }
}

fn external_row(n: &NormalizedUrl, hash: i64) -> UrlRow {
    UrlRow {
        url: n.normalized.to_string(),
        url_hash: hash,
        scheme: n.normalized.scheme().to_string(),
        host: n.normalized.host_str().unwrap_or_default().to_string(),
        path: n.normalized.path().to_string(),
        query: n.normalized.query().map(|q| q.to_string()),
        depth: None,
        discovered_from: None,
        is_internal: false,
        in_sitemap: false,
        crawl_state: CrawlState::Skipped,
        exclusion_reason: None,
        status_code: None,
        redirect_to_hash: None,
        redirect_chain_len: 0,
        content_type: None,
        content_length: None,
        response_time_ms: None,
        fetched_at: None,
        error_kind: None,
        error_message: None,
    }
}

/// La fila de una URL externa cuyo estado se acaba de comprobar.
///
/// `crawl_state` sigue siendo `skipped`, y no es un descuido: «skipped» significa «no se
/// rastreó», y sigue siendo verdad — solo se comprobó su estado. Marcarla `done` tendría un
/// efecto secundario real: la reanudación descuenta las filas `done` del presupuesto
/// `max_urls`, y las externas no cuentan contra ese presupuesto.
fn external_checked_row(item: &QueuedUrl, hash: i64, probe: &crate::fetch::StatusProbe) -> UrlRow {
    let mut row = base_row(item, hash, false);
    row.depth = None;
    row.is_internal = false;
    row.crawl_state = CrawlState::Skipped;
    row.status_code = Some(probe.status);
    row.content_type = probe.content_type.clone().map(clip_third_party);
    row.content_length = probe.content_length;
    row.response_time_ms = Some(probe.response_time_ms);
    row.fetched_at = Some(now_iso8601());
    row
}

/// Como [`external_checked_row`], para una sonda que falló: queda el motivo y ningún estado.
fn external_check_failed_row(
    item: &QueuedUrl,
    hash: i64,
    failure: &crate::fetch::FetchFailure,
) -> UrlRow {
    let mut row = base_row(item, hash, false);
    row.depth = None;
    row.is_internal = false;
    row.crawl_state = CrawlState::Skipped;
    row.error_kind = Some(failure.kind.as_str().to_string());
    row.error_message = Some(clip_third_party(failure.message.clone()));
    row.fetched_at = Some(now_iso8601());
    row
}

/// La fila de `resources` de una URL ya pedida (o sondeada), si algo la clasifica como recurso.
fn resource_row(
    hash: i64,
    content_type: Option<&str>,
    status: Option<u16>,
    size_bytes: Option<u64>,
    url: &Url,
) -> Option<ResourceRow> {
    resource_kind(content_type, status, url).map(|kind| ResourceRow {
        url_hash: hash,
        kind,
        status_code: status,
        size_bytes,
        // El `Content-Type` lo escribe el servidor de enfrente. Ver `MAX_THIRD_PARTY_TEXT`.
        mime: content_type.map(|s| clip_third_party(s.to_string())),
    })
}

/// Clasifica una respuesta como recurso (`'img'|'css'|'js'|'font'|'video'`), si lo es.
///
/// Manda el `content_type` de la respuesta; la extensión de la URL es el respaldo para cuando
/// el servidor no manda uno útil (`application/octet-stream` es frecuente en fuentes) — y para
/// los errores: el 404 de una hoja de estilo llega como `text/html`, y ese recurso roto es
/// justo lo que la tabla tiene que enseñar. La única excepción del respaldo es una página HTML
/// servida con 2xx: eso es una página, aunque su URL acabe en `.css`.
fn resource_kind(content_type: Option<&str>, status: Option<u16>, url: &Url) -> Option<&'static str> {
    if let Some(kind) = resource_kind_from_mime(content_type) {
        return Some(kind);
    }
    let html_ok = status.is_some_and(|s| (200..300).contains(&s))
        && content_type.is_some_and(|c| {
            let c = c.to_ascii_lowercase();
            c.contains("text/html") || c.contains("application/xhtml")
        });
    if html_ok {
        return None;
    }
    resource_kind_from_extension(url)
}

fn resource_kind_from_mime(content_type: Option<&str>) -> Option<&'static str> {
    let ct = content_type?.to_ascii_lowercase();
    let mime = ct.split(';').next().unwrap_or("").trim().to_string();
    if mime == "text/css" {
        return Some("css");
    }
    // text/javascript, application/javascript, application/x-javascript, *ecmascript…
    if mime.contains("javascript") || mime.contains("ecmascript") {
        return Some("js");
    }
    if mime.starts_with("image/") {
        return Some("img");
    }
    // font/woff2, application/font-woff, application/x-font-ttf, application/vnd.ms-fontobject.
    if mime.starts_with("font/") || mime.contains("font") {
        return Some("font");
    }
    if mime.starts_with("video/") {
        return Some("video");
    }
    None
}

fn resource_kind_from_extension(url: &Url) -> Option<&'static str> {
    let ext = url.path().rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "css" => Some("css"),
        "js" | "mjs" => Some("js"),
        "jpg" | "jpeg" | "png" | "webp" | "avif" | "svg" | "gif" | "ico" => Some("img"),
        "woff" | "woff2" | "ttf" | "otf" | "eot" => Some("font"),
        "mp4" | "webm" | "ogv" | "mov" | "m4v" => Some("video"),
        _ => None,
    }
}

/// Resuelve un href a su forma publicada, la misma que usa el frontier al encolar.
///
/// Si dos consumidores resolvieran distinto, el JOIN por hash no encontraría el destino y el
/// enlace se perdería. Por eso hay una sola función — y su resultado se calcula **una vez por
/// enlace** y se comparte (`resolved_links`), no una vez por consumidor.
fn resolve_link<F: Fetcher>(
    base: &Url,
    href: &str,
    policy: &NormalizePolicy,
    fetcher: &F,
) -> Option<NormalizedUrl> {
    let mut n = normalize::normalize_relative(base, href, policy).ok()?;
    // En modo filesystem, `/about` y `/about/` son la misma página. Unificarlas evita auditar
    // cada una dos veces y que las reglas de duplicados disparen sobre duplicados que inventó
    // el propio motor.
    if let Some(canonical) = fetcher.canonicalize(&n.normalized) {
        n.normalized = canonical;
    }
    Some(n)
}

/// Construye la fila de URL, la de página y sus hallazgos.
///
/// `resolved_links` es el vector paralelo a `parsed.links` con cada href ya resuelto: aquí no
/// se vuelve a normalizar ningún enlace.
///
/// `rules` llega prestado y ya filtrado por nivel: esta función se llama una vez por URL y las
/// reglas se construyen una sola vez, antes del bucle. Evaluar **exactamente lo que se recibe**
/// es parte del contrato — reconstruir el catálogo aquí dentro costaba ~59 cajas en el heap
/// por página, y hay un test que lo vigila.
#[allow(clippy::too_many_arguments)]
fn build_result<F: Fetcher>(
    item: &QueuedUrl,
    hash: i64,
    doc: &FetchedDoc,
    page: Option<&ParsedPage>,
    resolved_links: &[Option<NormalizedUrl>],
    blocked_by_robots: bool,
    in_sitemap: bool,
    seed_host: &str,
    policy: &NormalizePolicy,
    rules: &[Box<dyn PageRule>],
    fetcher: &F,
) -> CrawlResult {
    let mut url_row = base_row(item, hash, in_sitemap);
    url_row.crawl_state = CrawlState::Done;
    url_row.status_code = Some(doc.status);
    url_row.content_type = doc.content_type.clone().map(clip_third_party);
    url_row.content_length = Some(doc.content_length());
    url_row.response_time_ms = Some(doc.response_time_ms);
    url_row.fetched_at = Some(now_iso8601());
    url_row.is_internal = normalize::is_internal(&doc.url, seed_host);

    // Redirección: se guarda el destino y se encola aparte. Cada salto es una fila.
    if doc.is_redirect() {
        if let Some(location) = &doc.location {
            if let Ok(target) = normalize::normalize_relative(&doc.url, location, policy) {
                url_row.redirect_to_hash = Some(target.hash());
                url_row.redirect_chain_len = 1;
            }
        }
    }

    // La fila de `resources`, si la respuesta clasifica la URL como recurso: es lo que puebla
    // la tabla que existía desde la 001 y nadie escribía. Se calcula para cualquier respuesta
    // —también un 404: el recurso roto es justo lo que hay que poder enseñar—.
    let resource = resource_row(
        hash,
        doc.content_type.as_deref(),
        Some(doc.status),
        Some(doc.content_length()),
        &doc.url,
    );

    let Some(parsed) = page else {
        return CrawlResult { url: Some(url_row), resource, ..Default::default() };
    };

    // Canonical resuelto a absoluto, para poder compararlo con la propia URL.
    let canonical_abs = parsed.canonical.as_ref().and_then(|c| {
        normalize::normalize_relative(&doc.url, c, policy)
            .ok()
            .map(|n| n.normalized.to_string())
    });
    let self_url = doc.url.to_string();

    let (is_indexable, reason) = evaluate_indexability(&IndexabilityInput {
        status: doc.status,
        is_html: doc.is_html(),
        meta_robots: parsed.meta_robots.as_deref(),
        x_robots_tag: doc.x_robots_tag.as_deref(),
        blocked_by_robots,
        canonical: canonical_abs.as_deref(),
        self_url: &self_url,
    });

    let internal_links_out = resolved_links
        .iter()
        .flatten()
        .filter(|n| normalize::is_internal(&n.normalized, seed_host))
        .count() as u32;

    let page_row = PageRow {
        url_hash: hash,
        title: parsed.title.clone(),
        meta_description: parsed.meta_description.clone(),
        h1: parsed.h1().map(|s| s.to_string()),
        h1_count: parsed.h1_count(),
        h2_count: parsed.h2_count(),
        heading_json: parsed.heading_json(),
        canonical: canonical_abs.clone(),
        canonical_is_self: canonical_abs.as_ref().map(|c| *c == self_url),
        meta_robots: parsed.meta_robots.clone(),
        x_robots_tag: doc.x_robots_tag.clone(),
        is_indexable,
        indexability_reason: reason,
        lang: parsed.lang.clone(),
        hreflang_json: (!parsed.hreflang.is_empty())
            .then(|| serde_json::to_string(&parsed.hreflang).unwrap_or_default()),
        word_count: parsed.word_count,
        text_hash: parsed
            .body_text
            .as_ref()
            .map(|t| xxhash_rust::xxh3::xxh3_64(t.as_bytes()) as i64),
        html_hash: Some(xxhash_rust::xxh3::xxh3_64(&doc.body) as i64),
        content_ratio: parsed.content_ratio(),
        viewport: parsed.viewport.clone(),
        og_json: (!parsed.og.is_empty())
            .then(|| serde_json::to_string(&parsed.og).unwrap_or_default()),
        twitter_json: (!parsed.twitter.is_empty())
            .then(|| serde_json::to_string(&parsed.twitter).unwrap_or_default()),
        schema_types: (!parsed.schema_types.is_empty()).then(|| parsed.schema_types.join(",")),
        amp_url: parsed.amp_url.clone(),
        internal_links_out,
        crawl_depth_source: item.source,
        body_text: parsed.body_text.clone(),
    };

    // Reglas de página.
    //
    // Las vistas se construyen aquí, prestando las cadenas ya parseadas: las reglas no pueden
    // depender de `ParsedPage` porque `crawlforge-rules` no conoce al core, y no deben obligar a
    // copiar nada. Todo esto vive en la pila y muere al acabar la página.
    let heading_levels: Vec<u8> = parsed.headings.iter().map(|h| h.level).collect();
    let heading_texts: Vec<&str> = parsed.headings.iter().map(|h| h.text.as_str()).collect();

    let images: Vec<ImageView<'_>> = parsed
        .images
        .iter()
        .map(|img| ImageView {
            src: &img.src,
            alt: img.alt.as_deref(),
            width_attr: img.width_attr,
            height_attr: img.height_attr,
            // El texto del enlace contenedor solo se conoce ahora, con el parseo terminado.
            anchor_text: img
                .anchor_index
                .and_then(|i| parsed.links.get(i))
                .map(|l| l.anchor.as_deref().unwrap_or("")),
        })
        .collect();

    let links: Vec<LinkView<'_>> = parsed
        .links
        .iter()
        .zip(resolved_links)
        .map(|(l, resuelto)| LinkView {
            href: &l.href,
            anchor: l.anchor.as_deref(),
            is_nofollow: l.is_nofollow,
            is_internal: resuelto
                .as_ref()
                .is_some_and(|n| normalize::is_internal(&n.normalized, seed_host)),
            is_resource: !matches!(l.element, parse::LinkElement::A),
            is_infrastructure: resuelto
                .as_ref()
                .is_some_and(|n| crate::frontier::is_infrastructure_path(n.normalized.path())),
        })
        .collect();

    let hreflang: Vec<(&str, &str)> =
        parsed.hreflang.iter().map(|(lang, href)| (lang.as_str(), href.as_str())).collect();
    let og_keys: Vec<&str> = parsed.og.iter().map(|(k, _)| k.as_str()).collect();

    let ctx = PageContext {
        url: &self_url,
        status: doc.status,
        is_html: doc.is_html(),
        is_internal: url_row.is_internal,
        is_https: doc.url.scheme() == "https",
        blocked_by_robots,
        content_type: doc.content_type.as_deref(),
        // En el modo `filesystem` no hay red, así que un tiempo de respuesta sería inventado.
        ttfb_ms: fetcher.is_network().then_some(doc.response_time_ms),
        html_bytes: doc.content_length(),
        title: parsed.title.as_deref(),
        title_count: parsed.title_count,
        meta_description: parsed.meta_description.as_deref(),
        meta_robots: parsed.meta_robots.as_deref(),
        x_robots_tag: doc.x_robots_tag.as_deref(),
        meta_refresh: parsed.meta_refresh.as_deref(),
        viewport: parsed.viewport.as_deref(),
        lang: parsed.lang.as_deref(),
        h1: parsed.h1(),
        h1_count: parsed.h1_count(),
        heading_levels: &heading_levels,
        heading_texts: &heading_texts,
        canonical: canonical_abs.as_deref(),
        canonical_raw: parsed.canonical.as_deref(),
        canonical_count: parsed.canonical_count,
        is_indexable,
        word_count: parsed.word_count,
        images: &images,
        links: &links,
        hreflang: &hreflang,
        og_keys: &og_keys,
    };

    let issues: Vec<IssueRow> = rules
        .iter()
        .flat_map(|rule| rule.evaluate(&ctx))
        .map(|i| IssueRow {
            url_hash: Some(hash),
            rule_id: i.rule_id.to_string(),
            severity: i.severity.as_str().to_string(),
            category: i.category.as_str().to_string(),
            detail_json: i.detail_json,
            group_key: i.group_key,
        })
        .collect();

    // Enlaces e imágenes, con los extremos por hash.
    let links: Vec<LinkRow> = parsed
        .links
        .iter()
        .zip(resolved_links)
        .filter_map(|(l, resuelto)| {
            let n = resuelto.as_ref()?;
            if !normalize::is_crawlable_scheme(&n.normalized) {
                return None;
            }
            Some(LinkRow {
                from_hash: hash,
                to_hash: n.hash(),
                anchor: l.anchor.clone(),
                rel: l.rel.clone(),
                is_nofollow: l.is_nofollow,
                element: l.element,
                region: l.region,
                position: l.position,
            })
        })
        .collect();

    let images: Vec<ImageRow> = parsed
        .images
        .iter()
        .filter_map(|img| {
            let n = resolve_link(&doc.url, &img.src, policy, fetcher)?;
            Some(ImageRow {
                page_hash: hash,
                src_hash: n.hash(),
                alt: img.alt.clone(),
                alt_present: img.alt.is_some(),
                title: img.title.clone(),
                width_attr: img.width_attr,
                height_attr: img.height_attr,
                loading: img.loading.clone(),
                in_srcset: img.in_srcset,
                format: image_format(&n.normalized),
            })
        })
        .collect();

    CrawlResult { url: Some(url_row), page: Some(page_row), links, images, issues, resource }
}

/// Las reglas de página que corresponden al nivel del trabajo.
///
/// El filtrado está aquí, en el core, y no en la UI: si estuviera en Swift o en C#, la CLI y
/// cualquier build modificado lo esquivarían.
fn page_rules_for(job: &CrawlJob) -> Vec<Box<dyn PageRule>> {
    crawlforge_rules::page_rules_for_tier(job.tier)
}

fn image_format(url: &Url) -> Option<String> {
    let ext = url.path().rsplit('.').next()?.to_ascii_lowercase();
    matches!(ext.as_str(), "jpeg" | "jpg" | "png" | "webp" | "avif" | "svg" | "gif")
        .then(|| if ext == "jpg" { "jpeg".to_string() } else { ext })
}

/// Marca de tiempo UTC con el formato exacto de `datetime('now')` de SQLite:
/// `YYYY-MM-DD HH:MM:SS`.
///
/// Antes se abría una conexión SQLite en memoria **por llamada** solo para leer la hora, y se
/// llama una vez por URL rastreada. Medido con `bench_now_iso8601` en release: 7,35 µs por
/// llamada frente a 0,19 µs formateando desde el reloj del sistema (39x); en modo `filesystem`,
/// a 4.000 URL/s, eran 4.000 handles de SQLite por segundo. El formato tiene que seguir siendo idéntico al de SQLite: hay columnas
/// ya escritas y consultas de reglas que comparan estas cadenas con las que escribe
/// `datetime('now')` en el propio esquema.
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_epoch_seconds(secs)
}

/// Segundos desde la época UNIX → `YYYY-MM-DD HH:MM:SS` en UTC.
///
/// El calendario es el algoritmo de días civiles de Howard Hinnant (el de `<chrono>` de C++):
/// exacto en todo el calendario gregoriano proléptico, sin tablas ni bucles. Los tests lo
/// comparan contra `datetime(N, 'unixepoch')` de SQLite en fechas difíciles —bisiestos, fin de
/// año, el no-bisiesto de siglo 2100— para que cualquier discrepancia de formato salte aquí y
/// no en una consulta de reglas.
fn format_epoch_seconds(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn write_crawl_meta(
    conn: &Connection,
    crawl_id: &str,
    job: &CrawlJob,
    seed_host: &str,
) -> Result<()> {
    let base_url = match &job.mode {
        CrawlMode::Http { seed } => seed.clone(),
        CrawlMode::Filesystem { base, .. } => base.clone(),
        CrawlMode::List { urls } => urls.first().cloned().unwrap_or_default(),
    };
    let source_path = match &job.mode {
        CrawlMode::Filesystem { root, .. } => Some(root.display().to_string()),
        _ => None,
    };

    conn.execute(
        "INSERT INTO crawl_meta (
             id, project_id, project_name, base_url, mode, source_path, started_at,
             finished_at, status, config_json, core_version, rules_version, adapter,
             tier_at_runtime, truncated, truncated_reason)
         VALUES (?1,?2,?3,?4,?5,?6, datetime('now'), NULL, 'running', ?7,?8,?9, NULL, ?10, 0, NULL)",
        rusqlite::params![
            crawl_id,
            job.project_id,
            job.project_name,
            base_url,
            job.mode.as_str(),
            source_path,
            serde_json::to_string(job).unwrap_or_default(),
            crate::CORE_VERSION,
            crawlforge_rules::RULES_VERSION,
            "free",
        ],
    )?;
    let _ = seed_host;
    Ok(())
}

/// Lo que dejó la pasada final: sus hallazgos, cómo quedó el fichero y si un corte a petición
/// la interrumpió entre dos reglas.
struct FinalizeRun {
    site_issues: u64,
    outcome: store::FinalizeOutcome,
    /// El corte llegó durante la pasada final: el fichero queda `paused` y la reanudación
    /// recalculará las reglas de conjunto desde cero ([`delete_stale_site_issues`]).
    interrupted: bool,
}

/// Pasada final: enlaces entrantes, reglas de conjunto y cierre.
///
/// Las trazas de RSS no son decorativas: el pico de memoria resultó estar aquí y
/// no en el rastreo, y sin desglosar el paso no había forma de verlo.
///
/// La señal de cancelación se consulta **entre reglas**: la pasada final puede durar más que
/// el propio rastreo (5 min 20 s en el rastreo de campo; más de 8 h antes del índice de la 006) y era la
/// única fase sorda al primer Ctrl-C — hacía falta el segundo, el que mata el proceso y deja
/// el WAL sin volcar. Una sentencia en curso no se interrumpe: el corte cae en la costura
/// entre una regla y la siguiente, que es el mismo punto donde se emite el progreso.
#[allow(clippy::too_many_arguments)]
fn finalize(
    conn: &mut Connection,
    crawl_id: &str,
    truncated: Option<TruncationReason>,
    rss: &mut RssSampler,
    tier: crate::entitlement::Tier,
    records: CrawlRecords<'_>,
    resuming: bool,
    emitter: &mut ProgressEmitter,
    metrics: &CrawlMetrics,
    cancel: &Option<CancelSignal>,
) -> Result<FinalizeRun> {
    write_robots_and_sitemaps(conn, &records)?;

    // Una interrupción pudo caer en mitad de la pasada final anterior: algunas reglas de
    // conjunto ya habrían escrito sus hallazgos y reevaluarlas ahora los duplicaría. Se
    // borran y se recalculan desde cero — `internal_links_in` es un UPDATE y no lo necesita.
    if resuming {
        delete_stale_site_issues(conn)?;
    }

    let mut count: u64 = 0;
    let mut cancelled = cancel_requested(cancel);
    if !cancelled {
        // `internal_links_in` no se puede saber hasta tener todos los enlaces.
        emitter.enter_step("internal_links_in", 0, 0, metrics);
        conn.execute(
            "UPDATE pages SET internal_links_in = (
                 SELECT COUNT(DISTINCT l.from_url_id) FROM links l
                 WHERE l.to_url_id = pages.url_id
             )",
            [],
        )?;

        tracing::debug!(rss_mb = rss.sample() / 1048576, "RSS tras internal_links_in");

        // Reglas de conjunto, **una a una y escribiendo según se evalúan**.
        //
        // Acumularlas todas en un `Vec` antes de insertar era el antipatrón §9.2 aplicado a los
        // hallazgos: sobre un rastreo real de 500.000 URLs, las reglas de duplicados producen
        // 971.000 hallazgos con su `detail_json`, y el vector solo son **+330 MB**. Escribiendo
        // por regla, el techo de memoria pasa a ser el de la regla más ruidosa en vez de la suma
        // de todas, y las filas insertadas son exactamente las mismas.
        let reglas = crawlforge_rules::site_rules_for_tier(tier);
        let total = reglas.len() as u32;
        for (i, rule) in reglas.iter().enumerate() {
            if cancel_requested(cancel) {
                cancelled = true;
                break;
            }
            // Un rastreo cortado deja el grafo de enlaces con agujeros, y hay reglas cuya
            // conclusión depende de que esté completo. Ver
            // `crawlforge_rules::REQUIERE_GRAFO_COMPLETO`.
            // Antes de evaluarla, no después: lo que interesa saber es qué está tardando, y eso
            // solo se sabe mientras tarda.
            emitter.enter_step(rule.id(), i as u32 + 1, total, metrics);
            if truncated.is_some() && crawlforge_rules::requiere_grafo_completo(rule.id()) {
                tracing::debug!(
                    rule = rule.id(),
                    "omitida: el rastreo está truncado y la regla necesita el grafo completo"
                );
                continue;
            }
            let issues = rule.evaluate(conn)?;
            count += issues.len() as u64;
            write_site_issues(conn, &issues)?;
        }
    }

    if cancelled {
        // El mismo contrato que `pause()`: nada de `done`, el fichero queda reanudable y las
        // reglas de conjunto a medias las recalcula la reanudación desde cero.
        conn.execute(
            "UPDATE crawl_meta SET status = 'paused' WHERE id = ?1",
            rusqlite::params![crawl_id],
        )?;
    } else {
        conn.execute(
            "UPDATE crawl_meta SET finished_at = datetime('now'), status = 'done',
                 truncated = ?2, truncated_reason = ?3
             WHERE id = ?1",
            rusqlite::params![
                crawl_id,
                truncated.is_some() as i64,
                truncated.map(|t| t.as_str())
            ],
        )?;
    }

    let outcome = store::finalize(conn)?;
    tracing::debug!(rss_mb = rss.sample() / 1048576, "RSS tras la pasada final");
    Ok(FinalizeRun { site_issues: count, outcome, interrupted: cancelled })
}

/// Borra los hallazgos de reglas de conjunto que dejara una pasada final interrumpida.
///
/// Se borra por `rule_id` contra el catálogo completo (el nivel más alto: los IDs de un nivel
/// contienen a los inferiores) y no por `url_id IS NULL`, porque una regla de conjunto también
/// escribe hallazgos con URL. Los hallazgos de reglas de página no se tocan: se escribieron en
/// la misma transacción que su página y solo existen para páginas realmente rastreadas.
fn delete_stale_site_issues(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("DELETE FROM issues WHERE rule_id = ?1")?;
    let mut borrados = 0usize;
    for rule in crawlforge_rules::site_rules_for_tier(crate::entitlement::Tier::Agency) {
        borrados += stmt.execute([rule.id()])?;
    }
    if borrados > 0 {
        tracing::debug!(
            borrados,
            "reanudación: hallazgos de conjunto de una pasada final interrumpida, recalculados"
        );
    }
    Ok(())
}

/// Devuelve (elementos escritos, páginas parseadas).
///
/// Los elementos —páginas, enlaces e imágenes— son la medida del trabajo hecho; las páginas
/// solas impiden aprobar la puerta procesando mucho enlace y poco documento.
fn count_elements(conn: &Connection) -> Result<(u64, u64)> {
    let (elements, pages): (i64, i64) = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM pages)
              + (SELECT COUNT(*) FROM links)
              + (SELECT COUNT(*) FROM images),
                (SELECT COUNT(*) FROM pages)",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((elements as u64, pages as u64))
}

/// Índice hash → id, por si el motor lo necesita en memoria.
pub fn hash_index(conn: &Connection) -> Result<HashMap<i64, i64>> {
    writer::load_hash_index(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_metricas_calculan_urls_por_segundo() {
        let m = CrawlMetrics {
            urls_fetched: 300,
            elapsed: Duration::from_secs(2),
            ..Default::default()
        };
        assert_eq!(m.urls_per_second(), 150.0);
    }

    #[test]
    fn sin_tiempo_transcurrido_no_divide_por_cero() {
        let m = CrawlMetrics { urls_fetched: 10, ..Default::default() };
        assert_eq!(m.urls_per_second(), 0.0);
    }

    #[test]
    fn convierte_el_pico_de_memoria_a_megabytes() {
        let m = CrawlMetrics { peak_rss_bytes: 200 * 1024 * 1024, ..Default::default() };
        assert_eq!(m.peak_rss_mb(), 200.0);
    }

    #[test]
    fn el_muestreador_de_memoria_lee_el_proceso_y_no_baja() {
        let mut rss = RssSampler::new();
        let primera = rss.sample();
        assert!(primera > 0, "el proceso debe reportar memoria residente");

        // El pico es monótono: una lectura menor no debe rebajarlo, o el `VACUUM` final
        // borraría el máximo real del rastreo.
        let segunda = rss.sample();
        assert!(segunda >= primera, "el pico nunca decrece");
    }

    fn encolada(url: &str) -> QueuedUrl {
        QueuedUrl {
            url: Url::parse(url).expect("URL de test válida"),
            depth: 0,
            discovered_from: None,
            source: DiscoverySource::Link,
        }
    }

    #[test]
    fn el_planificador_aplica_el_limite_del_host_de_cada_url() {
        // El defecto original: el pool entero se dimensionaba con el límite del host semilla,
        // así que todos los hosts compartían un solo freno. Aquí, con `saturado.es` lleno,
        // `libre.es` tiene que poder despachar igualmente.
        let mut frontier = Frontier::new();
        frontier.enqueue(encolada("https://saturado.es/1"), 1);
        frontier.enqueue(encolada("https://saturado.es/2"), 2);
        frontier.enqueue(encolada("https://libre.es/1"), 3);

        let throttle = crate::throttle::Throttle::new(1);
        let mut in_flight = HashMap::from([("saturado.es".to_string(), 1_usize)]);

        let item = next_dispatchable(&mut frontier, &throttle, &in_flight)
            .expect("libre.es tiene hueco");
        assert_eq!(item.url.host_str(), Some("libre.es"));
        assert_eq!(frontier.pending(), 2, "las de saturado.es siguen en la cola");

        // Todo ocupado: nada despachable, pero nada perdido.
        in_flight.insert("libre.es".to_string(), 1);
        assert!(next_dispatchable(&mut frontier, &throttle, &in_flight).is_none());
        assert_eq!(frontier.pending(), 2);

        // Al liberarse el host, las suyas salen en su orden de llegada.
        in_flight.remove("saturado.es");
        let item = next_dispatchable(&mut frontier, &throttle, &in_flight)
            .expect("saturado.es ya tiene hueco");
        assert_eq!(item.url.path(), "/1", "conserva el orden BFS");
        assert_eq!(frontier.pending(), 1);
    }

    #[test]
    fn el_freno_adaptativo_de_un_host_no_frena_a_los_demas() {
        let throttle = crate::throttle::Throttle::new(4);
        for _ in 0..crate::throttle::OVERLOAD_STREAK {
            throttle.record("ahogado.es", 503);
        }
        assert_eq!(throttle.limit_for("ahogado.es"), 2, "el freno redujo a la mitad");

        let mut frontier = Frontier::new();
        frontier.enqueue(encolada("https://ahogado.es/1"), 1);
        frontier.enqueue(encolada("https://sano.es/1"), 2);
        // El host frenado va lleno con su límite reducido.
        let in_flight = HashMap::from([("ahogado.es".to_string(), 2_usize)]);

        let item = next_dispatchable(&mut frontier, &throttle, &in_flight)
            .expect("el host sano no hereda el freno del ahogado");
        assert_eq!(item.url.host_str(), Some("sano.es"));
    }

    #[test]
    fn un_host_serializado_no_mata_de_hambre_a_los_demas() {
        // La reproducción del defecto: 250 URLs de un host con `Crawl-delay` —una petición en
        // vuelo— por delante de 5 de un host libre. El planificador anterior retenía las
        // primeras 200 en un búfer aparte y, con el búfer lleno, **dejaba de mirar el
        // frontier**: el host libre se quedaba a cero de cinco indefinidamente. Con el retardo
        // en su tope de 30 s, habría esperado cien minutos.
        //
        // El test antiguo (`tests/crawl_delay.rs`) no lo cazaba porque usaba cinco URLs y
        // nunca llenaba el búfer.
        let mut frontier = Frontier::new();
        for i in 0..250 {
            frontier.enqueue(encolada(&format!("https://lento.es/{i}")), i);
        }
        for i in 0..5 {
            frontier.enqueue(encolada(&format!("https://libre.es/{i}")), 1000 + i);
        }

        let throttle = crate::throttle::Throttle::new(5);
        // El host lento declara Crawl-delay: queda en una petición en vuelo, y la tiene.
        throttle.force_serial("lento.es");
        let in_flight = HashMap::from([("lento.es".to_string(), 1_usize)]);

        // Las cinco del host libre salen sin esperar a que drene el lento.
        for _ in 0..5 {
            let item = next_dispatchable(&mut frontier, &throttle, &in_flight)
                .expect("el host libre tiene hueco y trabajo");
            assert_eq!(item.url.host_str(), Some("libre.es"));
        }
        // Y agotado el host libre, no se inventa nada del saturado.
        assert!(next_dispatchable(&mut frontier, &throttle, &in_flight).is_none());
        assert_eq!(frontier.pending(), 250, "las del host lento siguen enteras en la cola");
    }

    #[tokio::test]
    async fn liberar_el_hueco_limpia_el_recuento_del_host() {
        let mut set = tokio::task::JoinSet::new();
        let id = set.spawn(async {}).id();
        let mut host_by_task = HashMap::from([(id, "ejemplo.es".to_string())]);
        let mut in_flight_by_host = HashMap::from([("ejemplo.es".to_string(), 1_usize)]);

        release_slot(&mut host_by_task, &mut in_flight_by_host, id);
        assert!(host_by_task.is_empty());
        assert!(in_flight_by_host.is_empty(), "las entradas a cero se retiran del mapa");
        set.shutdown().await;
    }

    // ── La decisión de sondar una externa ────────────────────────────────────

    fn limites() -> crate::job::CrawlLimits {
        crate::job::CrawlLimits::default()
    }

    fn sin_filtro() -> crate::pattern::UrlFilter {
        crate::pattern::UrlFilter::from_limits(&limites()).expect("sin patrones")
    }

    /// El perímetro de un rastreo de un sitio público: ningún objetivo local declarado.
    fn criba_publica() -> normalize::NetworkScreen {
        normalize::NetworkScreen::for_targets(["ejemplo.es"])
    }

    fn por_que_no(url: &str, nofollow: bool, filtro: &crate::pattern::UrlFilter) -> Option<NotProbed> {
        why_not_probe(
            &Url::parse(url).expect("URL de test válida"),
            nofollow,
            filtro,
            &limites(),
            0,
            &criba_publica(),
        )
    }

    #[test]
    fn a_normal_external_link_is_probed() {
        assert_eq!(por_que_no("https://ejemplo.es/guia", false, &sin_filtro()), None);
    }

    #[test]
    fn the_probe_stops_at_the_users_own_exclude() {
        // La rama de externas hacía `continue` antes de evaluar los patrones: `--exclude` no
        // impedía la petición, solo excluía del rastreo algo que nunca se iba a rastrear.
        let mut limits = limites();
        limits.exclude_patterns = vec![r"spam\.example".to_string()];
        let filtro = crate::pattern::UrlFilter::from_limits(&limits).expect("patrón válido");
        assert_eq!(
            por_que_no("https://spam.example/promo", false, &filtro),
            Some(NotProbed::Pattern)
        );
        assert_eq!(por_que_no("https://ejemplo.es/guia", false, &filtro), None);
    }

    #[test]
    fn a_nofollow_link_is_registered_but_not_probed() {
        // El caso real: los enlaces de spam de los comentarios de un WordPress llevan
        // `rel="ugc nofollow"`. Sin esto se le manda un HEAD con nuestro user-agent de bot a
        // cada dominio de spam acumulado en años, y sus 404 salen publicados como hallazgos.
        assert_eq!(
            por_que_no("https://casino-spam.example/", true, &sin_filtro()),
            Some(NotProbed::Nofollow)
        );
        // Con `respect_nofollow` apagado, el enlace vuelve a sondearse.
        let mut limits = limites();
        limits.respect_nofollow = false;
        assert_eq!(
            why_not_probe(
                &Url::parse("https://casino-spam.example/").expect("URL válida"),
                true,
                &sin_filtro(),
                &limits,
                0,
                &criba_publica(),
            ),
            None
        );
    }

    #[test]
    fn the_probe_does_not_reach_the_users_own_network() {
        for url in [
            "http://169.254.169.254/latest/meta-data/",
            "http://127.0.0.1:8080/panel",
            "http://10.0.0.5/",
            "http://localhost:5432/",
        ] {
            assert_eq!(
                por_que_no(url, false, &sin_filtro()),
                Some(NotProbed::LocalNetwork),
                "{url} no puede pedirse"
            );
        }
    }

    #[test]
    fn auditing_a_local_site_does_not_screen_the_local_network() {
        // Quien rastrea `http://localhost:4321/` ya está dentro de esa red y lo pidió él: la
        // criba no protegería de nada y rompería el caso de uso.
        let solo_local = normalize::NetworkScreen::for_targets(["localhost"]);
        assert_eq!(
            why_not_probe(
                &Url::parse("http://127.0.0.1:9000/x").expect("URL válida"),
                false,
                &sin_filtro(),
                &limites(),
                0,
                &solo_local,
            ),
            None
        );
    }

    #[test]
    fn one_public_target_takes_the_local_exemption_away_from_the_whole_crawl() {
        // El caso del modo lista: la excepción era un interruptor global que accionaba la
        // primera línea del fichero del usuario. Ahora hace falta que **todos** los objetivos
        // sean locales, así que una lista que mezcle un sitio de internet con uno local no
        // abre la red del usuario a lo que enlace el de internet.
        let mezcla = normalize::NetworkScreen::for_targets(["localhost", "cliente.es"]);
        assert_eq!(
            why_not_probe(
                &Url::parse("http://127.0.0.1:9000/x").expect("URL válida"),
                false,
                &sin_filtro(),
                &limites(),
                0,
                &mezcla,
            ),
            Some(NotProbed::LocalNetwork)
        );
        // El objetivo declarado sí se sigue alcanzando: es lo que el usuario pidió auditar.
        assert_eq!(
            why_not_probe(
                &Url::parse("http://localhost:9000/x").expect("URL válida"),
                false,
                &sin_filtro(),
                &limites(),
                0,
                &mezcla,
            ),
            None
        );
    }

    #[test]
    fn a_line_without_a_host_no_longer_turns_the_screen_off() {
        // Reproducido: con `mailto:contacto@cliente.es` de primera línea de la lista, la criba
        // se apagaba para todas las demás y la víctima en loopback recibía la petición.
        // `mailto:` no aporta host, así que ya no aporta nada.
        let criba = normalize::NetworkScreen::for_seeds(
            [Url::parse("mailto:contacto@cliente.es"), Url::parse("http://cliente.es/p1")]
                .iter()
                .filter_map(|u| u.as_ref().ok()),
        );
        assert!(!criba.is_local_audit(), "una línea sin host no vuelve local el rastreo");
        assert_eq!(
            why_not_probe(
                &Url::parse("http://127.0.0.1:9000/admin").expect("URL válida"),
                false,
                &sin_filtro(),
                &limites(),
                0,
                &criba,
            ),
            Some(NotProbed::LocalNetwork)
        );
    }

    #[test]
    fn the_global_probe_budget_is_still_the_last_word() {
        let limits = limites();
        assert_eq!(
            why_not_probe(
                &Url::parse("https://ejemplo.es/x").expect("URL válida"),
                false,
                &sin_filtro(),
                &limits,
                limits.max_external,
                &criba_publica(),
            ),
            Some(NotProbed::Budget)
        );
    }

    // ── La cola de sondas: topes y cortocircuito por host ────────────────────

    fn externa(url: &str) -> Url {
        Url::parse(url).expect("URL de test válida")
    }

    fn fallo_de_red() -> crate::fetch::FetchFailure {
        crate::fetch::FetchFailure {
            kind: crate::fetch::ErrorKind::Timeout,
            message: "timed out".to_string(),
        }
    }

    #[test]
    fn a_dead_host_stops_receiving_probes_after_three_silences() {
        // Un dominio muerto enlazado desde 200 URLs únicas pagaba el timeout entero por cada
        // una, en serie: ~33 minutos pegados al final del rastreo.
        let mut cola = ExternalQueue::default();
        for i in 0..20 {
            cola.push(externa(&format!("https://muerto.example/{i}"))).expect("cabe");
        }
        let fallo = fallo_de_red();

        assert!(cola.record("muerto.example", Err(&fallo)).is_none(), "una no basta");
        assert!(cola.record("muerto.example", Err(&fallo)).is_none(), "dos tampoco");
        let (sin_comprobar, motivo) =
            cola.record("muerto.example", Err(&fallo)).expect("la tercera cierra el host");
        assert_eq!(motivo, NotProbed::HostUnreachable);
        assert_eq!(sin_comprobar, 20, "lo que quedaba en cola se cuenta, no se olvida");
        assert_eq!(cola.pending(), 0);

        // Y no se le vuelve a encolar nada.
        assert_eq!(
            cola.push(externa("https://muerto.example/otra")),
            Err(NotProbed::HostUnreachable)
        );
    }

    #[test]
    fn an_answer_of_any_kind_resets_the_streak() {
        // Un 404 o un 500 son respuestas: el host está vivo, y su estado es justo lo que la
        // sonda venía a averiguar.
        let mut cola = ExternalQueue::default();
        for i in 0..5 {
            cola.push(externa(&format!("https://vivo.example/{i}"))).expect("cabe");
        }
        let fallo = fallo_de_red();
        cola.record("vivo.example", Err(&fallo));
        cola.record("vivo.example", Err(&fallo));
        assert!(cola.record("vivo.example", Ok(404)).is_none(), "un 404 es una respuesta");
        assert!(cola.record("vivo.example", Err(&fallo)).is_none(), "la racha se reinició");
        assert!(cola.record("vivo.example", Err(&fallo)).is_none());
        assert_eq!(cola.pending(), 5, "el host sigue abierto");
    }

    #[test]
    fn a_host_that_asks_for_a_break_gets_it() {
        // El commit que trajo las sondas prometía ser «educado con quien no es nuestro» y esa
        // parte no estaba: la rama de sondas no miraba el estado de la respuesta.
        let mut cola = ExternalQueue::default();
        for i in 0..8 {
            cola.push(externa(&format!("https://saturado.example/{i}"))).expect("cabe");
        }
        let (sin_comprobar, motivo) =
            cola.record("saturado.example", Ok(429)).expect("un 429 cierra el host");
        assert_eq!(motivo, NotProbed::RateLimited);
        assert_eq!(sin_comprobar, 8);
        assert_eq!(
            cola.push(externa("https://saturado.example/otra")),
            Err(NotProbed::RateLimited)
        );
    }

    #[test]
    fn one_host_cannot_take_the_whole_probe_budget() {
        let mut cola = ExternalQueue::default();
        for i in 0..MAX_EXTERNAL_PER_HOST {
            cola.push(externa(&format!("https://enlazado.example/{i}"))).expect("cabe");
        }
        assert_eq!(
            cola.push(externa("https://enlazado.example/una-mas")),
            Err(NotProbed::HostBudget)
        );
        // Y el tope es de ese host, no del rastreo: otro dominio sigue entrando.
        assert_eq!(cola.push(externa("https://otro.example/1")), Ok(()));
    }

    #[test]
    fn closing_a_host_does_not_touch_the_others() {
        let mut cola = ExternalQueue::default();
        cola.push(externa("https://muerto.example/1")).expect("cabe");
        cola.push(externa("https://vivo.example/1")).expect("cabe");
        let fallo = fallo_de_red();
        for _ in 0..EXTERNAL_HOST_FAILURE_STREAK {
            cola.record("muerto.example", Err(&fallo));
        }
        assert_eq!(cola.pending(), 1, "solo queda la del host vivo");

        let throttle = crate::throttle::Throttle::new(1);
        let en_vuelo = HashMap::new();
        let url = cola.next_dispatchable(&throttle, &en_vuelo).expect("la del host vivo");
        assert_eq!(url.host_str(), Some("vivo.example"));
        assert!(cola.next_dispatchable(&throttle, &en_vuelo).is_none());
    }

    /// El fallo que destapó registrar el destino externo de las redirecciones: un host que
    /// vacía su cola y vuelve a recibir trabajo no regresaba a la ronda, y su sonda no la
    /// despachaba nadie.
    ///
    /// Se ve con dos URLs del mismo host **descubiertas en momentos distintos**, que es lo
    /// normal cuando llegan de dos redirecciones y no de la misma página. En `debug` rompía el
    /// invariante del planificador; en `release`, la sonda se perdía y solo aparecía al final
    /// como una externa «sin comprobar».
    #[test]
    fn a_host_that_empties_its_queue_comes_back_when_it_gets_more_work() {
        let throttle = crate::throttle::Throttle::new(1);
        let en_vuelo = HashMap::new();
        let mut cola = ExternalQueue::default();

        cola.push(externa("https://tienda.example/producto-vivo")).expect("cabe");
        let primera = cola.next_dispatchable(&throttle, &en_vuelo).expect("la primera sale");
        assert_eq!(primera.path(), "/producto-vivo");
        assert_eq!(cola.pending(), 0, "la cola del host queda vacía");

        // Segundo descubrimiento del mismo host, ya con su cola vacía.
        cola.push(externa("https://tienda.example/producto-retirado")).expect("cabe");
        assert_eq!(cola.pending(), 1);
        let segunda = cola
            .next_dispatchable(&throttle, &en_vuelo)
            .expect("y la segunda también, que era el fallo");
        assert_eq!(segunda.path(), "/producto-retirado");
        assert_eq!(cola.pending(), 0, "no queda nada colgado");
    }

    // ── El alcance de una reanudación ────────────────────────────────────────

    #[test]
    fn resume_only_reloads_urls_of_the_crawls_own_site() {
        let job = CrawlJob::http("https://cliente.es/");
        let scope = ResumeScope::of(&job, "https://cliente.es/");
        let admite = |s: &str| scope.admits(&Url::parse(s).expect("URL válida")).is_ok();

        assert!(admite("https://cliente.es/blog/entrada"));
        assert!(!admite("https://otro-dominio.es/"), "otro sitio no es este rastreo");
        assert!(!admite("https://sub.cliente.es/"), "un subdominio es otro sitio (is_internal)");
        // Y el esquema: una fila fabricada con `file:` no es una URL de rastreo.
        assert!(scope.admits(&Url::parse("file:///etc/passwd").expect("URL válida")).is_err());
    }

    #[test]
    fn resume_of_a_public_site_refuses_an_address_of_the_internal_network() {
        // El ataque que cierra: un fichero con metadatos coherentes de un sitio público y una
        // fila `pending` apuntando a la red interna. En modo lista es donde cabe, porque una
        // lista mezcla hosts legítimamente y solo su primera URL se contrasta contra los
        // metadatos.
        let job = CrawlJob {
            mode: CrawlMode::List {
                urls: vec![
                    "https://cliente.es/a".to_string(),
                    "http://169.254.169.254/latest/meta-data/".to_string(),
                ],
            },
            ..CrawlJob::http("https://cliente.es/")
        };
        let scope = ResumeScope::of(&job, "https://cliente.es/a");
        assert!(scope.admits(&Url::parse("https://cliente.es/a").expect("URL válida")).is_ok());
        assert!(scope
            .admits(&Url::parse("http://169.254.169.254/latest/meta-data/").expect("URL válida"))
            .is_err());
    }

    #[test]
    fn resume_of_a_local_site_keeps_working() {
        // Auditar `http://localhost:4321/` —un `astro dev`— es trabajo normal, y quien lanzó
        // ese rastreo ya estaba dentro de esa red. La criba no puede romperlo.
        let job = CrawlJob::http("http://localhost:4321/");
        let scope = ResumeScope::of(&job, "http://localhost:4321/");
        assert!(scope
            .admits(&Url::parse("http://localhost:4321/blog/").expect("URL válida"))
            .is_ok());
    }

    // ── Texto de terceros ────────────────────────────────────────────────────

    #[test]
    fn third_party_text_is_clipped_and_short_text_is_left_alone() {
        assert_eq!(clip_third_party("text/css".to_string()), "text/css");

        let largo = "x".repeat(4096);
        assert_eq!(clip_third_party(largo).chars().count(), MAX_THIRD_PARTY_TEXT);

        // Por caracteres y no por bytes: cortar por bytes partiría la secuencia UTF-8 y la
        // cadena dejaría de ser válida.
        let acentuado = "á".repeat(4096);
        let recortado = clip_third_party(acentuado);
        assert_eq!(recortado.chars().count(), MAX_THIRD_PARTY_TEXT);
        assert!(recortado.is_char_boundary(recortado.len()));
    }

    #[test]
    fn a_huge_content_type_does_not_grow_the_crawl_file() {
        // `resources.mime` y `urls.content_type` los escribe el servidor de enfrente: 10.000
        // externas con un `Content-Type` de varios KB engordan el fichero que se entrega al
        // cliente sin aportar nada.
        let mime = format!("text/css; charset=utf-8; {}", "a".repeat(8192));
        let row = resource_row(
            1,
            Some(&mime),
            Some(200),
            Some(10),
            &Url::parse("https://cdn.example/lib.css").expect("URL válida"),
        )
        .expect("es un recurso");
        assert_eq!(row.kind, "css");
        assert_eq!(
            row.mime.as_deref().map(|m| m.chars().count()),
            Some(MAX_THIRD_PARTY_TEXT)
        );
    }

    #[test]
    fn los_motivos_de_truncado_se_nombran_como_en_el_esquema() {
        assert_eq!(TruncationReason::MaxUrls.as_str(), "max_urls");
        assert_eq!(TruncationReason::MaxDepth.as_str(), "max_depth");
        assert_eq!(TruncationReason::MaxDuration.as_str(), "max_duration");
        assert_eq!(TruncationReason::ListMode.as_str(), "list_mode");
    }

    #[test]
    fn clasifica_los_recursos_por_content_type_con_la_extension_de_respaldo() {
        let u = |s: &str| Url::parse(s).expect("URL de test válida");

        // Manda el content_type de la respuesta, con o sin parámetros.
        assert_eq!(resource_kind(Some("text/css"), Some(200), &u("https://x.es/a")), Some("css"));
        assert_eq!(
            resource_kind(Some("application/javascript; charset=utf-8"), Some(200), &u("https://x.es/a")),
            Some("js")
        );
        assert_eq!(resource_kind(Some("image/webp"), Some(200), &u("https://x.es/a")), Some("img"));
        assert_eq!(resource_kind(Some("font/woff2"), Some(200), &u("https://x.es/a")), Some("font"));
        assert_eq!(
            resource_kind(Some("application/vnd.ms-fontobject"), Some(200), &u("https://x.es/a")),
            Some("font")
        );
        assert_eq!(resource_kind(Some("video/mp4"), Some(200), &u("https://x.es/a")), Some("video"));

        // El respaldo por extensión: fuentes servidas como octet-stream, que es frecuente.
        assert_eq!(
            resource_kind(Some("application/octet-stream"), Some(200), &u("https://x.es/f.woff2")),
            Some("font")
        );
        // Y los errores: el 404 de un CSS llega como text/html y tiene que seguir siendo css.
        assert_eq!(
            resource_kind(Some("text/html"), Some(404), &u("https://x.es/roto.css")),
            Some("css")
        );
        // Un fallo de red no trae ni content_type ni estado: decide la extensión.
        assert_eq!(resource_kind(None, None, &u("https://x.es/app.js")), Some("js"));

        // Una página HTML servida con 2xx es una página, aunque su URL acabe en .css.
        assert_eq!(resource_kind(Some("text/html"), Some(200), &u("https://x.es/raro.css")), None);
        // Y lo que no es recurso, no es recurso.
        assert_eq!(resource_kind(Some("text/html"), Some(200), &u("https://x.es/pagina")), None);
        assert_eq!(resource_kind(Some("application/pdf"), Some(200), &u("https://x.es/doc.pdf")), None);
        assert_eq!(resource_kind(None, None, &u("https://x.es/sin-extension")), None);
    }

    #[test]
    fn deduce_el_formato_de_imagen_de_la_extension() {
        let f = |s: &str| image_format(&Url::parse(s).expect("URL válida"));
        assert_eq!(f("https://ejemplo.es/a.webp").as_deref(), Some("webp"));
        assert_eq!(f("https://ejemplo.es/a.JPG").as_deref(), Some("jpeg"), "jpg normaliza a jpeg");
        assert_eq!(f("https://ejemplo.es/a.txt"), None);
        assert_eq!(f("https://ejemplo.es/sin-extension"), None);
    }

    #[test]
    fn genera_una_marca_de_tiempo_valida() {
        let ts = now_iso8601();
        assert_eq!(ts.len(), 19, "formato 'YYYY-MM-DD HH:MM:SS': {ts}");
    }

    #[test]
    fn la_marca_de_tiempo_coincide_con_la_de_sqlite() {
        // Las columnas ya escritas y las consultas de las reglas comparan estas cadenas con las
        // de `datetime('now')`: el formato tiene que ser idéntico byte a byte. La llamada puede
        // caer justo en una frontera de segundo, así que se reintenta.
        let conn = Connection::open_in_memory().expect("abrir SQLite en memoria");
        for _ in 0..3 {
            let nuestra = now_iso8601();
            let sqlite: String = conn
                .query_row("SELECT datetime('now')", [], |r| r.get(0))
                .expect("leer datetime('now')");
            if nuestra == sqlite || now_iso8601() == sqlite {
                return;
            }
        }
        panic!("la marca de tiempo no coincide con la de SQLite");
    }

    #[test]
    fn el_formato_es_identico_al_de_sqlite_en_fechas_dificiles() {
        let conn = Connection::open_in_memory().expect("abrir SQLite en memoria");
        // Época, 29 de febrero de 2000 (siglo bisiesto), 29 de febrero de 2024, fronteras de
        // año, el problema de 2038, y 2100, que no es bisiesto aunque lo parezca.
        for secs in [
            0_i64,
            951_782_400,   // 2000-02-29 00:00:00
            1_709_164_799, // 2024-02-28 23:59:59
            1_709_164_800, // 2024-02-29 00:00:00
            1_672_531_199, // 2022-12-31 23:59:59
            1_672_531_200, // 2023-01-01 00:00:00
            2_147_483_648, // pasado el desbordamiento de 32 bits
            4_107_542_399, // 2100, año de siglo no bisiesto
        ] {
            let sqlite: String = conn
                .query_row("SELECT datetime(?1, 'unixepoch')", [secs], |r| r.get(0))
                .expect("leer datetime(N, 'unixepoch')");
            assert_eq!(format_epoch_seconds(secs as u64), sqlite, "discrepancia en {secs}");
        }
    }

    #[test]
    #[ignore = "medición manual: cargo test --release -- --ignored bench_now_iso8601"]
    fn bench_now_iso8601() {
        let n = 20_000u32;
        let inicio = Instant::now();
        for _ in 0..n {
            std::hint::black_box(now_iso8601());
        }
        println!("now_iso8601: {:?} por llamada", inicio.elapsed() / n);
    }

    #[test]
    fn el_progreso_se_muestrea_por_tiempo_y_no_por_url() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let emitidos = Arc::new(AtomicUsize::new(0));
        let contador = Arc::clone(&emitidos);
        let cb: ProgressCallback = Arc::new(move |_| {
            contador.fetch_add(1, Ordering::SeqCst);
        });

        let mut emitter = ProgressEmitter::new(Some(cb));
        let metrics = CrawlMetrics::default();

        // El cambio de fase emite siempre, sin esperar al intervalo.
        emitter.enter_phase(CrawlPhase::Crawl, &metrics, 0);
        assert_eq!(emitidos.load(Ordering::SeqCst), 1);

        // Mil ticks seguidos dentro del intervalo no emiten nada: es lo que protege al
        // bucle de `filesystem`, que procesa miles de URLs por segundo.
        for _ in 0..1000 {
            emitter.tick(&metrics, 0, 0);
        }
        assert_eq!(emitidos.load(Ordering::SeqCst), 1);

        // Pasado el intervalo, el siguiente tick sí emite.
        std::thread::sleep(PROGRESS_INTERVAL + Duration::from_millis(10));
        emitter.tick(&metrics, 5, 0);
        assert_eq!(emitidos.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn el_progreso_entrega_la_fase_y_la_cola_de_la_instantanea() {
        let visto: Arc<std::sync::Mutex<Option<CrawlProgress>>> =
            Arc::new(std::sync::Mutex::new(None));
        let destino = Arc::clone(&visto);
        let cb: ProgressCallback = Arc::new(move |p| {
            if let Ok(mut guard) = destino.lock() {
                *guard = Some(p.clone());
            }
        });

        let mut emitter = ProgressEmitter::new(Some(cb));
        let metrics = CrawlMetrics { urls_fetched: 42, issues_found: 7, ..Default::default() };
        emitter.enter_phase(CrawlPhase::Sitemaps, &metrics, 9);

        let guard = visto.lock().unwrap_or_else(|e| e.into_inner());
        let p = guard.as_ref().expect("debe haberse emitido una instantánea");
        assert_eq!(p.phase, CrawlPhase::Sitemaps);
        assert_eq!(p.urls_fetched, 42);
        assert_eq!(p.issues_found, 7);
        assert_eq!(p.urls_queued, 9);
        assert_eq!(p.externals_pending, 0, "el descubrimiento de sitemaps no manda sondas");
    }

    /// Lo que pedía la revisión del 4 de agosto: las sondas de externas dejan de ir sumadas al
    /// contador del sitio, porque un rastreo que ya terminó el suyo y solo espera a hosts
    /// ajenos enseñaba miles de «queued» que no eran suyos.
    #[test]
    fn las_sondas_de_externas_viajan_aparte_de_la_cola_del_sitio() {
        let visto: Arc<std::sync::Mutex<Option<CrawlProgress>>> =
            Arc::new(std::sync::Mutex::new(None));
        let destino = Arc::clone(&visto);
        let cb: ProgressCallback = Arc::new(move |p| {
            if let Ok(mut guard) = destino.lock() {
                *guard = Some(p.clone());
            }
        });

        let mut emitter = ProgressEmitter::new(Some(cb));
        let metrics = CrawlMetrics { urls_fetched: 1_000, ..Default::default() };
        std::thread::sleep(PROGRESS_INTERVAL + Duration::from_millis(10));
        emitter.tick(&metrics, 0, 314);

        let guard = visto.lock().unwrap_or_else(|e| e.into_inner());
        let p = guard.as_ref().expect("debe haberse emitido una instantánea");
        assert_eq!(p.urls_queued, 0, "del sitio no queda nada");
        assert_eq!(p.externals_pending, 314, "y lo que queda son sondas, dicho aparte");
    }

    #[test]
    fn sin_observador_el_emisor_no_hace_nada() {
        let mut emitter = ProgressEmitter::new(None);
        let metrics = CrawlMetrics::default();
        emitter.enter_phase(CrawlPhase::Crawl, &metrics, 0);
        emitter.tick(&metrics, 0, 0);
        // Sin callback no hay nada que observar: basta con que no entre en pánico.
    }

    // ── Regresiones de la revisión 2026-08-01, tanda 4 ───────────────────────

    /// Servidor HTTP mínimo en un hilo aparte: responde 200 con un HTML diminuto a todo.
    ///
    /// Existe porque los tests de asignaciones del despacho necesitan que las URLs de una
    /// lista **se rastreen de verdad** (un puerto cerrado dispara los reintentos con backoff
    /// de 1-4 s por URL y el test tardaría minutos). Sus asignaciones no contaminan la
    /// medición: el contador de la sonda es thread-local y este servidor vive en otro hilo.
    fn servidor_minimo() -> u16 {
        use std::io::{Read, Write};
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("un puerto efímero libre");
        let port = listener.local_addr().expect("dirección local").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let body = "<html><head><title>t</title></head><body>ok</body></html>";
                let respuesta = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                let _ = stream.write_all(respuesta.as_bytes());
            }
        });
        port
    }

    /// Un trabajo en modo `list` sobre el servidor mínimo, sin sitemaps.
    fn trabajo_de_lista(urls: Vec<String>) -> CrawlJob {
        CrawlJob {
            project_id: "test".to_string(),
            project_name: "lista".to_string(),
            mode: CrawlMode::List { urls },
            limits: crate::job::CrawlLimits::default(),
            collect_body_text: false,
            discover_sitemaps: false,
            tier: crate::entitlement::Tier::Agency,
        }
    }

    fn fichero_temporal(nombre: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("crawlforge-{nombre}-{}.sqlite", uuid::Uuid::now_v7()))
    }

    fn limpiar_fichero_de_rastreo(path: &Path) {
        let _ = std::fs::remove_file(path);
        for sufijo in ["-wal", "-shm", ".lock"] {
            let mut lateral = path.as_os_str().to_owned();
            lateral.push(sufijo);
            let _ = std::fs::remove_file(std::path::PathBuf::from(lateral));
        }
    }

    #[test]
    fn despachar_una_lista_no_clona_la_lista_completa_por_url() {
        // El O(n²) de la revisión 2026-08-01 §4.1: el rellenado del pool hacía `job.clone()`
        // por URL despachada, y en modo `list` el `CrawlMode` arrastra la lista completa —
        // n URLs eran n clones de n `String`. Un test de tiempo no lo caza con fiabilidad
        // (la varianza del banco es del 24%); contar asignaciones sí, como en `parse.rs`:
        // el clon por URL asigna al menos n × tamaño de la lista, y el `Arc` no.
        //
        // El runtime es de un solo hilo a propósito: el contador de la sonda es thread-local
        // y así cuenta también lo que asignan las tareas del pool, que es donde va el clon.
        //
        // Las URLs son largas a propósito: el término cuadrático crece con n × longitud y el
        // coste base del rastreo (reqwest, parseo, filas) apenas, así que alargarlas separa
        // los dos mundos sin alargar el test.
        let port = servidor_minimo();
        let relleno = "x".repeat(1000);
        let urls: Vec<String> = (0..400)
            .map(|i| format!("http://127.0.0.1:{port}/{relleno}/pagina-{i:04}"))
            .collect();
        let n = urls.len();
        let lista_bytes: usize = urls.iter().map(String::len).sum();
        let job = trabajo_de_lista(urls);
        let path = fichero_temporal("lista-alloc");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime de test");
        let (resultado, bytes, _) =
            crate::alloc_probe::midiendo_asignaciones(|| rt.block_on(run(job, &path)));
        let outcome = resultado.expect("el rastreo de la lista termina");
        limpiar_fichero_de_rastreo(&path);
        assert_eq!(outcome.metrics.urls_fetched, n as u64, "se rastreó la lista entera");

        // Solo las copias de la lista ya costarían n × lista_bytes (165,6 MB aquí). Medido
        // en tres pasadas, muy estable: 214,2 MB con el clon por URL, 44,6 MB con el `Arc`.
        // El umbral a la mitad del suelo cuadrático (82,8 MB) deja 2,6x de margen por arriba
        // y 1,9x por abajo (números visibles con --nocapture para recalibrar).
        let suelo_cuadratico = (n * lista_bytes) as u64;
        println!(
            "lista de {n} URLs ({lista_bytes} bytes): {bytes} bytes asignados · suelo cuadrático {suelo_cuadratico}"
        );
        assert!(
            bytes < suelo_cuadratico / 2,
            "despachar {n} URLs asignó {bytes} bytes (umbral {}): el pool vuelve a clonar el \
             CrawlJob —y con él la lista entera— por cada URL despachada",
            suelo_cuadratico / 2,
        );
    }

    #[test]
    #[ignore = "medición manual: cargo test --release -p crawlforge-core --lib -- --ignored bench_despacho_modo_lista --nocapture"]
    fn bench_despacho_modo_lista() {
        // El banco sintético de la regresión usa modo `filesystem` y no pasa por el despacho
        // de red, así que el coste del clonado en modo `list` no se ve allí. Este banco lo
        // ejercita de verdad: 10.000 URLs contra el servidor mínimo local.
        let port = servidor_minimo();
        let urls: Vec<String> = (0..10_000)
            .map(|i| {
                format!(
                    "http://127.0.0.1:{port}/blog/una-entrada-con-un-slug-razonablemente-largo-{i:05}/"
                )
            })
            .collect();
        let n = urls.len();
        let job = trabajo_de_lista(urls);
        let path = fichero_temporal("lista-bench");

        let rt = tokio::runtime::Runtime::new().expect("runtime de test");
        let inicio = Instant::now();
        let outcome = rt.block_on(run(job, &path)).expect("el rastreo de la lista termina");
        let transcurrido = inicio.elapsed();
        limpiar_fichero_de_rastreo(&path);
        assert_eq!(outcome.metrics.urls_fetched, n as u64);
        println!(
            "modo lista: {n} URLs en {transcurrido:?} · {:.0} URLs/s",
            n as f64 / transcurrido.as_secs_f64()
        );
    }

    #[test]
    fn la_memoria_se_muestrea_por_tiempo_y_no_por_iteracion() {
        // Muestrear el RSS son syscalls de `sysinfo` (5-20 µs), y el bucle une una tarea por
        // iteración: sin la puerta temporal se pagaba ese peaje por URL. Mismo patrón (y
        // mismo tipo de test) que `el_progreso_se_muestrea_por_tiempo_y_no_por_url`.
        let mut rss = RssSampler::new();
        rss.sample(); // la muestra forzada del arranque, como hace el bucle
        for _ in 0..10_000 {
            let _ = rss.sample_if_due();
        }
        assert!(
            rss.samples_taken <= 2,
            "10.000 iteraciones inmediatas tomaron {} muestras: el muestreo vuelve a ser por \
             iteración y no por tiempo",
            rss.samples_taken,
        );

        // Pasada la ventana sí vuelve a muestrear: acotar la frecuencia no congela el pico.
        std::thread::sleep(RSS_SAMPLE_INTERVAL + Duration::from_millis(10));
        let antes = rss.samples_taken;
        let _ = rss.sample_if_due();
        assert_eq!(rss.samples_taken, antes + 1, "vencido el intervalo, la muestra se toma");
    }

    #[test]
    fn medir_la_concurrencia_no_asigna_memoria_por_iteracion() {
        // El `Vec<f64>` de muestras era la única estructura del bucle que crecía linealmente
        // con el rastreo sin necesitarlo: 8 bytes por URL, 80 MB a 10 millones. La media
        // corrida no puede asignar nada por muestra — y tiene que dar el mismo promedio.
        let mut meter = ConcurrencyMeter::new();
        let ((), bytes, _) = crate::alloc_probe::midiendo_asignaciones(|| {
            for i in 0..1_000_000u32 {
                meter.record(if i % 2 == 0 { 4 } else { 8 });
            }
        });
        assert_eq!(
            bytes, 0,
            "registrar la concurrencia asignó {bytes} bytes: la medida vuelve a crecer con el rastreo",
        );
        assert_eq!(meter.average(0.0), 6.0, "la media corrida da el mismo promedio que el vector");
        assert_eq!(
            ConcurrencyMeter::new().average(5.0),
            5.0,
            "sin muestras manda la concurrencia configurada"
        );
    }

    #[test]
    fn build_result_evalua_las_reglas_que_recibe_y_no_las_reconstruye() {
        // Las reglas se construyen una vez antes del bucle y `build_result` las recibe
        // prestadas: reconstruirlas dentro costaba ~59 cajas en el heap por página. El
        // contrato observable es que evalúa exactamente lo que se le pasa: si volviera a
        // montar el catálogo por su cuenta, una lista vacía también produciría hallazgos.
        let item = encolada("https://ejemplo.es/pagina");
        let doc = FetchedDoc {
            url: Url::parse("https://ejemplo.es/pagina").expect("URL de test válida"),
            status: 200,
            content_type: Some("text/html".to_string()),
            x_robots_tag: None,
            location: None,
            // Sin <title> real, sin h1 y casi sin texto: el catálogo completo encuentra algo.
            body: b"<html><body><p>hola</p></body></html>".to_vec(),
            response_time_ms: 10,
        };
        let page = parse::parse_html(&doc.body, false);
        let policy = NormalizePolicy::default();
        let fetcher = FilesystemFetcher::new(
            std::env::temp_dir(),
            Url::parse("https://ejemplo.es/").expect("base de test válida"),
        );
        let resolved: Vec<Option<NormalizedUrl>> = Vec::new();

        let con_reglas = build_result(
            &item, 1, &doc, Some(&page), &resolved, false, false, "ejemplo.es", &policy,
            &crawlforge_rules::page_rules(), &fetcher,
        );
        assert!(!con_reglas.issues.is_empty(), "el catálogo completo encuentra algo aquí");

        let sin_reglas = build_result(
            &item, 1, &doc, Some(&page), &resolved, false, false, "ejemplo.es", &policy,
            &[], &fetcher,
        );
        assert!(
            sin_reglas.issues.is_empty(),
            "sin reglas no puede haber hallazgos: build_result las está reconstruyendo por su cuenta",
        );
    }
}
