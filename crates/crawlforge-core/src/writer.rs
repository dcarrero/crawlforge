//! Hilo escritor: el **único** que toca el fichero SQLite.
//! Ver `docs/01-ARQUITECTURA.md §5`.
//!
//! Los workers nunca escriben. Con veinte escritores concurrentes, la contención de WAL cuesta
//! más que el propio rastreo. En su lugar, cada worker manda su resultado por un canal `mpsc`
//! y este hilo los agrupa en transacciones grandes.
//!
//! Lote por defecto: 200 URLs o 2 segundos, lo que llegue antes. El tiempo importa tanto como
//! el tamaño: sin él, un rastreo lento dejaría el fichero sin actualizar durante minutos y la
//! UI, que lee ese mismo fichero, parecería congelada.

use crate::error::Result;
use crate::frontier::{DiscoverySource, ExclusionReason};
use crate::job::IndexabilityReason;
use crate::parse::{LinkElement, Region};
use rusqlite::{params, Connection, Transaction};
use std::collections::HashMap;
use std::time::Duration;

/// Número de URLs que fuerza el volcado de un lote.
pub const BATCH_SIZE: usize = 200;
/// Tiempo máximo que un resultado espera en el lote antes de escribirse.
pub const BATCH_INTERVAL: Duration = Duration::from_secs(2);

/// Estado de rastreo de una URL. Se corresponde con `urls.crawl_state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrawlState {
    Pending,
    Done,
    Error,
    Excluded,
    Skipped,
}

impl CrawlState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Done => "done",
            Self::Error => "error",
            Self::Excluded => "excluded",
            Self::Skipped => "skipped",
        }
    }
}

/// Fila de `urls`.
#[derive(Debug, Clone)]
pub struct UrlRow {
    pub url: String,
    pub url_hash: i64,
    pub scheme: String,
    pub host: String,
    pub path: String,
    pub query: Option<String>,
    pub depth: Option<u32>,
    pub discovered_from: Option<i64>,
    pub is_internal: bool,
    pub in_sitemap: bool,
    pub crawl_state: CrawlState,
    pub exclusion_reason: Option<ExclusionReason>,
    pub status_code: Option<u16>,
    /// Hash de la URL de destino de la redirección; se resuelve a `id` en la pasada final.
    pub redirect_to_hash: Option<i64>,
    pub redirect_chain_len: u32,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub response_time_ms: Option<u32>,
    pub fetched_at: Option<String>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
}

/// Fila de `pages`.
#[derive(Debug, Clone)]
pub struct PageRow {
    pub url_hash: i64,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub h1_count: u32,
    pub h2_count: u32,
    pub heading_json: String,
    pub canonical: Option<String>,
    pub canonical_is_self: Option<bool>,
    pub meta_robots: Option<String>,
    pub x_robots_tag: Option<String>,
    pub is_indexable: bool,
    pub indexability_reason: Option<IndexabilityReason>,
    pub lang: Option<String>,
    pub hreflang_json: Option<String>,
    pub word_count: u32,
    pub text_hash: Option<i64>,
    pub html_hash: Option<i64>,
    pub content_ratio: f64,
    pub viewport: Option<String>,
    pub og_json: Option<String>,
    pub twitter_json: Option<String>,
    pub schema_types: Option<String>,
    pub amp_url: Option<String>,
    pub internal_links_out: u32,
    pub crawl_depth_source: DiscoverySource,
    pub body_text: Option<String>,
}

/// Fila de `links`. Los extremos se guardan por hash y se resuelven a `id` al final.
#[derive(Debug, Clone)]
pub struct LinkRow {
    pub from_hash: i64,
    pub to_hash: i64,
    pub anchor: Option<String>,
    pub rel: Option<String>,
    pub is_nofollow: bool,
    pub element: LinkElement,
    pub region: Region,
    pub position: u32,
}

/// Fila de `images`.
#[derive(Debug, Clone)]
pub struct ImageRow {
    pub page_hash: i64,
    pub src_hash: i64,
    pub alt: Option<String>,
    pub alt_present: bool,
    pub title: Option<String>,
    pub width_attr: Option<i64>,
    pub height_attr: Option<i64>,
    pub loading: Option<String>,
    pub in_srcset: bool,
    pub format: Option<String>,
}

/// Fila de `resources`: **una por URL de recurso**, no por par (página, recurso).
///
/// La arista página↔recurso no existe a propósito (`docs/02-MODELO-DATOS.md §3.5`): en CSS y
/// JS aporta mucho menos que en imágenes —un `bundle.js` pesado se carga en toda la plantilla,
/// no en una página concreta—, así que el fichero ya identifica el problema. Para las imágenes
/// esa arista sí existe y vive en `images`.
#[derive(Debug, Clone)]
pub struct ResourceRow {
    pub url_hash: i64,
    /// 'img'|'css'|'js'|'font'|'video'. Deducido del `content_type` de la respuesta, con la
    /// extensión de la URL como respaldo (ver `engine::resource_kind`).
    pub kind: &'static str,
    pub status_code: Option<u16>,
    pub size_bytes: Option<u64>,
    pub mime: Option<String>,
}

/// Fila de `issues`.
#[derive(Debug, Clone)]
pub struct IssueRow {
    pub url_hash: Option<i64>,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    pub detail_json: Option<String>,
    pub group_key: Option<String>,
}

/// Todo lo que una URla rastreada aporta al almacén, en un solo mensaje.
#[derive(Debug, Default)]
pub struct CrawlResult {
    pub url: Option<UrlRow>,
    pub page: Option<PageRow>,
    pub links: Vec<LinkRow>,
    pub images: Vec<ImageRow>,
    pub issues: Vec<IssueRow>,
    /// La fila de `resources` de esta URL, si la respuesta la clasifica como recurso.
    pub resource: Option<ResourceRow>,
}

/// Un lote acumulado, listo para escribirse en una sola transacción.
#[derive(Debug, Default)]
pub struct Batch {
    pub urls: Vec<UrlRow>,
    pub pages: Vec<PageRow>,
    pub links: Vec<LinkRow>,
    pub images: Vec<ImageRow>,
    pub issues: Vec<IssueRow>,
    pub resources: Vec<ResourceRow>,
}

impl Batch {
    pub fn push(&mut self, result: CrawlResult) {
        if let Some(u) = result.url {
            self.urls.push(u);
        }
        if let Some(p) = result.page {
            self.pages.push(p);
        }
        self.links.extend(result.links);
        self.images.extend(result.images);
        self.issues.extend(result.issues);
        if let Some(r) = result.resource {
            self.resources.push(r);
        }
    }

    pub fn len(&self) -> usize {
        self.urls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.urls.is_empty()
            && self.pages.is_empty()
            && self.links.is_empty()
            && self.images.is_empty()
            && self.issues.is_empty()
            && self.resources.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.urls.len() >= BATCH_SIZE
    }

    pub fn take(&mut self) -> Batch {
        std::mem::take(self)
    }
}

/// Índice hash → id de las URLs ya escritas.
///
/// Vive en memoria durante todo el rastreo y es lo que permite escribir `links` e `images` sin
/// un `JOIN` por fila. Cuesta unos 16 bytes por URL —8 MB con 500.000— frente a dos búsquedas
/// por índice en cada enlace: sobre 6,15 millones de enlaces, 12,3 millones de búsquedas.
pub type IdIndex = HashMap<i64, i64>;

/// Escribe un lote completo en una única transacción.
///
/// Los ids de los extremos de `links` e `images` se resuelven contra `index`, que se va poblando
/// con cada URL insertada. La versión anterior los resolvía con `INSERT ... SELECT FROM urls f,
/// urls t WHERE f.url_hash = ? AND t.url_hash = ?`, lo que parecía gratis porque el índice
/// `idx_urls_hash` existe, pero son dos búsquedas por enlace y con millones de enlaces se
/// convierte en el cuello de botella del modo `filesystem`.
pub fn write_batch(conn: &mut Connection, batch: &Batch, index: &mut IdIndex) -> Result<()> {
    let tx = conn.transaction()?;
    insert_urls(&tx, &batch.urls, index)?;
    insert_pages(&tx, &batch.pages, index)?;
    insert_links(&tx, &batch.links, index)?;
    insert_images(&tx, &batch.images, index)?;
    insert_issues(&tx, &batch.issues, index)?;
    insert_resources(&tx, &batch.resources, index)?;
    tx.commit()?;
    Ok(())
}

fn insert_urls(tx: &Transaction<'_>, rows: &[UrlRow], index: &mut IdIndex) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    // `ON CONFLICT DO UPDATE` y no `INSERT OR IGNORE`: una URL se inserta primero como
    // `pending` al descubrirla y se completa al rastrearla. Ignorar el segundo insert
    // dejaría todas las filas sin código de estado.
    let mut stmt = tx.prepare_cached(
        "INSERT INTO urls (
             url, url_hash, scheme, host, path, query, depth, discovered_from,
             is_internal, in_sitemap, crawl_state, exclusion_reason, status_code,
             redirect_chain_len, content_type, content_length, response_time_ms,
             fetched_at, error_kind, error_message)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)
         ON CONFLICT(url) DO UPDATE SET
             depth            = COALESCE(excluded.depth, urls.depth),
             -- `is_internal` tiene que actualizarse, no conservarse. Una URL se inserta al
             -- descubrirla y se completa al rastrearla, y si la primera escritura se equivoca,
             -- sin esta línea el error es permanente: un 404 de otro dominio se quedaba marcado
             -- como interno y salía reportado como HTTP-404-INTERNAL, que es severidad crítica.
             -- Un enlace ajeno roto acusando al sitio del cliente.
             is_internal      = excluded.is_internal,
             in_sitemap       = MAX(urls.in_sitemap, excluded.in_sitemap),
             crawl_state      = excluded.crawl_state,
             exclusion_reason = COALESCE(excluded.exclusion_reason, urls.exclusion_reason),
             status_code      = COALESCE(excluded.status_code, urls.status_code),
             redirect_chain_len = excluded.redirect_chain_len,
             content_type     = COALESCE(excluded.content_type, urls.content_type),
             content_length   = COALESCE(excluded.content_length, urls.content_length),
             response_time_ms = COALESCE(excluded.response_time_ms, urls.response_time_ms),
             fetched_at       = COALESCE(excluded.fetched_at, urls.fetched_at),
             error_kind       = COALESCE(excluded.error_kind, urls.error_kind),
             error_message    = COALESCE(excluded.error_message, urls.error_message)
         RETURNING id",
    )?;

    for r in rows {
        // `RETURNING id` da el id tanto si insertó como si actualizó. `last_insert_rowid()` no
        // sirve aquí: en el caso de conflicto no cambia y devolvería el id de otra fila.
        let id: i64 = stmt.query_row(params![
            r.url,
            r.url_hash,
            r.scheme,
            r.host,
            r.path,
            r.query,
            r.depth,
            r.discovered_from,
            r.is_internal as i64,
            r.in_sitemap as i64,
            r.crawl_state.as_str(),
            r.exclusion_reason.map(|e| e.as_str()),
            r.status_code,
            r.redirect_chain_len,
            r.content_type,
            r.content_length.map(|v| v as i64),
            r.response_time_ms,
            r.fetched_at,
            r.error_kind,
            r.error_message,
        ], |row| row.get(0))?;
        index.insert(r.url_hash, id);
    }

    // `redirect_to` en un segundo paso: el destino puede ser una URL de este mismo lote, así que
    // su id no existía cuando se insertó el origen.
    let mut redirect = tx.prepare_cached("UPDATE urls SET redirect_to = ?2 WHERE id = ?1")?;
    for r in rows {
        let Some(to_hash) = r.redirect_to_hash else { continue };
        // Si el destino todavía no se ha visto, se deja sin resolver: lo hará la pasada final.
        if let (Some(&from_id), Some(&to_id)) =
            (index.get(&r.url_hash), index.get(&to_hash))
        {
            redirect.execute(params![from_id, to_id])?;
        }
    }
    Ok(())
}

fn insert_pages(tx: &Transaction<'_>, rows: &[PageRow], index: &IdIndex) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    // Búsqueda de texto completo (`pages_fts`, `docs/02-MODELO-DATOS.md §3.7`).
    //
    // Se indexa aquí, en el mismo lote y la misma transacción que la página, y **solo cuando la
    // fila trae texto de cuerpo** — que es exactamente cuando el trabajo pidió
    // `collect_body_text` (nivel Pro). El invariante queda nítido: FTS poblada ⟺ el rastreo
    // recogió texto. Indexar siempre título y descripción pero el cuerpo solo a veces
    // reproduciría el defecto que motiva esto: una tabla que parece funcionar y a la que le
    // falta lo prometido, sin que quien consulta pueda notarlo.
    //
    // No puede aplazarse a la pasada final: el texto de cuerpo no se guarda en ninguna otra
    // tabla (multiplicaría el fichero), así que o se indexa al pasar por aquí o habría que
    // retenerlo entero en memoria hasta el final, que es el antipatrón nº 2 de `CONVENTIONS.md §5`.
    //
    // `rowid = urls.id`: así el resultado de un MATCH vuelve a la fila real con un JOIN por
    // clave primaria. El `NOT EXISTS` protege de reescrituras: `pages_fts` es contentless
    // (`content = ''`) y no admite DELETE ni REPLACE, así que un segundo INSERT con el mismo
    // rowid no sustituiría la entrada anterior, la *acumularía* (comprobado contra SQLite
    // 3.51). El motor parsea cada página una sola vez; la guarda cubre el mismo caso
    // defensivo que el `INSERT OR REPLACE` de la página, y debe ejecutarse **antes** que él.
    let mut fts = tx.prepare_cached(
        "INSERT INTO pages_fts (rowid, url, title, meta_description, body_text)
         SELECT u.id, u.url, ?2, ?3, ?4 FROM urls u
         WHERE u.id = ?1
           AND NOT EXISTS (SELECT 1 FROM pages WHERE url_id = ?1)",
    )?;
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO pages (
             url_id, title, title_len, title_px, meta_description, meta_desc_len, meta_desc_px,
             h1, h1_count, h2_count, heading_json, canonical, canonical_is_self, meta_robots,
             x_robots_tag, is_indexable, indexability_reason, lang, hreflang_json, word_count,
             text_hash, html_hash, content_ratio, viewport, og_json, twitter_json, schema_types,
             amp_url, internal_links_out, internal_links_in, crawl_depth_source)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,
                 ?21,?22,?23,?24,?25,?26,?27,?28,?29, NULL, ?30)",
    )?;

    for r in rows {
        let title_len = r.title.as_deref().map(|t| t.chars().count() as i64);
        let title_px = r.title.as_deref().map(estimate_pixel_width_title);
        let desc_len = r.meta_description.as_deref().map(|t| t.chars().count() as i64);
        let desc_px = r.meta_description.as_deref().map(estimate_pixel_width_description);

        // Sin id no hay fila de URL a la que colgar la página: se omite en vez de violar la
        // clave foránea y tumbar la transacción entera.
        let Some(&url_id) = index.get(&r.url_hash) else { continue };
        if let Some(body) = r.body_text.as_deref() {
            fts.execute(params![url_id, r.title, r.meta_description, body])?;
        }
        stmt.execute(params![
            url_id,
            r.title,
            title_len,
            title_px,
            r.meta_description,
            desc_len,
            desc_px,
            r.h1,
            r.h1_count,
            r.h2_count,
            r.heading_json,
            r.canonical,
            r.canonical_is_self.map(|b| b as i64),
            r.meta_robots,
            r.x_robots_tag,
            r.is_indexable as i64,
            r.indexability_reason.map(|i| i.as_str()),
            r.lang,
            r.hreflang_json,
            r.word_count,
            r.text_hash,
            r.html_hash,
            r.content_ratio,
            r.viewport,
            r.og_json,
            r.twitter_json,
            r.schema_types,
            r.amp_url,
            r.internal_links_out,
            r.crawl_depth_source.as_str(),
        ])?;
    }
    Ok(())
}

fn insert_links(tx: &Transaction<'_>, rows: &[LinkRow], index: &IdIndex) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT INTO links (from_url_id, to_url_id, anchor, rel, is_nofollow, element, region, position)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
    )?;
    for r in rows {
        // Un enlace cuyo destino aún no se ha escrito se omite. No debería ocurrir —el motor
        // registra toda URL descubierta antes de emitir sus enlaces— pero omitir una fila es
        // preferible a abortar el lote.
        let (Some(&from_id), Some(&to_id)) = (index.get(&r.from_hash), index.get(&r.to_hash))
        else {
            continue;
        };
        stmt.execute(params![
            from_id,
            to_id,
            r.anchor,
            r.rel,
            r.is_nofollow as i64,
            r.element.as_str(),
            r.region.as_str(),
            r.position,
        ])?;
    }
    Ok(())
}

fn insert_images(tx: &Transaction<'_>, rows: &[ImageRow], index: &IdIndex) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut stmt = tx.prepare_cached(
        "INSERT INTO images (page_url_id, src_url_id, alt, alt_present, title,
                             width_attr, height_attr, loading, in_srcset, format)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
    )?;
    for r in rows {
        let (Some(&page_id), Some(&src_id)) = (index.get(&r.page_hash), index.get(&r.src_hash))
        else {
            continue;
        };
        stmt.execute(params![
            page_id,
            src_id,
            r.alt,
            r.alt_present as i64,
            r.title,
            r.width_attr,
            r.height_attr,
            r.loading,
            r.in_srcset as i64,
            r.format,
        ])?;
    }
    Ok(())
}

fn insert_resources(tx: &Transaction<'_>, rows: &[ResourceRow], index: &IdIndex) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    // El `url_id` se resuelve contra el índice en memoria, como `links` e `images`: nunca un
    // JOIN por fila. El upsert existe por la reanudación, que repone estas filas junto con las
    // de `urls` (`engine::resend_existing_rows`): reponer actualiza, no duplica. El índice
    // único que lo hace posible es la migración 008.
    let mut stmt = tx.prepare_cached(
        "INSERT INTO resources (url_id, kind, status_code, size_bytes, mime)
         VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(url_id) DO UPDATE SET
             kind        = excluded.kind,
             status_code = COALESCE(excluded.status_code, resources.status_code),
             size_bytes  = COALESCE(excluded.size_bytes, resources.size_bytes),
             mime        = COALESCE(excluded.mime, resources.mime)",
    )?;
    for r in rows {
        // Sin fila de URL no hay recurso al que colgarse: se omite en vez de violar la clave
        // foránea y tumbar la transacción entera. Mismo criterio que `insert_links`.
        let Some(&url_id) = index.get(&r.url_hash) else { continue };
        stmt.execute(params![
            url_id,
            r.kind,
            r.status_code,
            r.size_bytes.map(|v| v as i64),
            r.mime,
        ])?;
    }
    Ok(())
}

fn insert_issues(tx: &Transaction<'_>, rows: &[IssueRow], index: &IdIndex) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    // Un hallazgo de sitio no tiene URL: `url_id` queda a NULL y el SELECT ... WHERE no
    // serviría, así que ese caso va por su propia sentencia.
    let mut with_url = tx.prepare_cached(
        "INSERT INTO issues (url_id, rule_id, severity, category, detail_json, group_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut site_wide = tx.prepare_cached(
        "INSERT INTO issues (url_id, rule_id, severity, category, detail_json, group_key)
         VALUES (NULL, ?1, ?2, ?3, ?4, ?5)",
    )?;

    for r in rows {
        match r.url_hash.and_then(|h| index.get(&h).copied()) {
            Some(url_id) => {
                with_url.execute(params![
                    url_id,
                    r.rule_id,
                    r.severity,
                    r.category,
                    r.detail_json,
                    r.group_key
                ])?;
            }
            None => {
                site_wide.execute(params![
                    r.rule_id,
                    r.severity,
                    r.category,
                    r.detail_json,
                    r.group_key
                ])?;
            }
        }
    }
    Ok(())
}

/// Ancho aproximado de un título en píxeles, con métricas de Arial 20px.
///
/// Google trunca por ancho en píxeles, no por número de caracteres. Avisar de «más de 60
/// caracteres» es un consejo peor: un título de 60 íes cabe y uno de 45 emes no.
///
/// **Delega en `crawlforge-rules`.** Aquí había una segunda tabla de anchos, más basta, y el
/// resultado era que la columna `pages.title_px` del almacén y el umbral con el que
/// `META-TITLE-TOO-LONG` decide se calculaban por caminos distintos: una fila podía tener 585 px
/// guardados y no salir en el informe, o al contrario. La tabla vive donde está la regla.
pub fn estimate_pixel_width_title(text: &str) -> i64 {
    crawlforge_rules::meta::title_width_px(text).round() as i64
}

/// Ancho aproximado de una meta description, con métricas de Arial 14px.
pub fn estimate_pixel_width_description(text: &str) -> i64 {
    crawlforge_rules::meta::description_width_px(text).round() as i64
}

// ---------------------------------------------------------------- Hilo escritor

/// Mensajes en vuelo permitidos antes de frenar al motor.
///
/// El canal **tiene que estar acotado**. Con uno ilimitado, un rastreo en modo `filesystem`
/// produce resultados mucho más rápido de lo que SQLite los escribe, y la cola se convierte en
/// el almacén: medido con 50.000 páginas y 6,15 millones de enlaces, el pico de memoria subía de
/// 170 MB a 387 MB sin que nada lo justificara. Acotarlo aplica contrapresión: si el escritor no
/// da abasto, el motor espera.
///
/// Cuatro lotes es margen suficiente para que el escritor nunca se quede sin trabajo y poco
/// suficiente para que la cola no sea memoria escondida.
pub const CHANNEL_CAPACITY: usize = BATCH_SIZE * 4;

/// Lo que se le manda al hilo escritor.
enum Message {
    Write(Box<CrawlResult>),
    /// Latido periódico. Es lo que permite volcar por tiempo: el escritor bloquea esperando
    /// mensajes, así que necesita que alguien lo despierte para cumplir el «o 2 segundos».
    Tick,
    /// Vaciar lo acumulado y devolver el control. Lo usa la pasada final, que necesita la
    /// conexión para sí.
    Finish,
}

/// Estadísticas de lo escrito, devueltas al cerrar el hilo.
#[derive(Debug, Default, Clone, Copy)]
pub struct WriterStats {
    pub urls: u64,
    pub pages: u64,
    pub links: u64,
    pub images: u64,
    pub issues: u64,
    pub resources: u64,
    pub batches: u64,
}

/// Mango del hilo escritor.
///
/// El motor solo puede hacer dos cosas con él: mandar resultados y cerrarlo. No tiene acceso a la
/// conexión, que es lo que garantiza la regla de `docs/01-ARQUITECTURA.md §5`: **ningún worker
/// escribe en SQLite**.
pub struct WriterHandle {
    sender: tokio::sync::mpsc::Sender<Message>,
    thread: std::thread::JoinHandle<Result<WriterStats>>,
    ticker: tokio::task::JoinHandle<()>,
}

impl WriterHandle {
    /// Arranca el hilo escritor sobre un fichero de rastreo ya migrado.
    ///
    /// Debe llamarse dentro de un runtime de tokio: el latido que fuerza el volcado por tiempo
    /// es una tarea.
    pub fn spawn(path: std::path::PathBuf) -> Result<Self> {
        let (sender, receiver) = tokio::sync::mpsc::channel::<Message>(CHANNEL_CAPACITY);

        // El escritor corre en un hilo del sistema y no en una tarea de tokio: `rusqlite` es
        // síncrono, y una escritura de 200 filas dentro del runtime bloquea a los workers que
        // deberían estar esperando en la red.
        let thread = std::thread::Builder::new()
            .name("crawlforge-writer".into())
            .spawn(move || writer_loop(path, receiver))
            .map_err(crate::CoreError::Io)?;

        let latido = sender.clone();
        let ticker = tokio::spawn(async move {
            let mut intervalo = tokio::time::interval(BATCH_INTERVAL);
            intervalo.tick().await; // el primero es inmediato
            loop {
                intervalo.tick().await;
                if latido.send(Message::Tick).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self { sender, thread, ticker })
    }

    /// Encola un resultado, **esperando si el escritor va por detrás**.
    ///
    /// Ese `await` es la contrapresión: sin ella la cola crece hasta convertirse en el almacén.
    /// Devuelve error solo si el hilo escritor ha muerto, que es irrecuperable: sin escritor no
    /// hay rastreo que guardar.
    pub async fn send(&self, result: CrawlResult) -> Result<()> {
        self.sender
            .send(Message::Write(Box::new(result)))
            .await
            .map_err(|_| crate::CoreError::WriterGone)
    }

    /// Vacía lo pendiente, cierra el hilo y devuelve lo escrito.
    pub async fn finish(self) -> Result<WriterStats> {
        self.ticker.abort();
        // Si el hilo ya murió, el envío falla pero aún queda recoger su error real del join.
        let _ = self.sender.send(Message::Finish).await;
        drop(self.sender);
        // El join es bloqueante: se saca del runtime para no parar a los workers que queden.
        tokio::task::spawn_blocking(move || match self.thread.join() {
            Ok(stats) => stats,
            Err(_) => Err(crate::CoreError::WriterGone),
        })
        .await
        .map_err(|_| crate::CoreError::WriterGone)?
    }
}

/// El bucle del escritor: acumula en lotes y vuelca por tamaño o por tiempo.
fn writer_loop(
    path: std::path::PathBuf,
    mut receiver: tokio::sync::mpsc::Receiver<Message>,
) -> Result<WriterStats> {
    let mut conn = crate::store::open_writer(&path)?;
    let mut index: IdIndex = HashMap::new();
    let mut batch = Batch::default();
    let mut stats = WriterStats::default();

    // `blocking_recv` desde un hilo propio: el escritor no participa del runtime, solo consume.
    while let Some(msg) = receiver.blocking_recv() {
        match msg {
            Message::Write(result) => {
                batch.push(*result);
                if batch.is_full() {
                    flush(&mut conn, &mut batch, &mut index, &mut stats)?;
                }
            }
            // Volcado por tiempo: un rastreo lento no debe dejar el fichero sin actualizar
            // durante minutos, porque la UI lee ese mismo fichero y parecería congelada.
            Message::Tick => flush(&mut conn, &mut batch, &mut index, &mut stats)?,
            Message::Finish => break,
        }
    }

    // Si el canal se cerró sin `Finish` —fallo o cancelación— lo acumulado se guarda igual.
    flush(&mut conn, &mut batch, &mut index, &mut stats)?;
    Ok(stats)
}

fn flush(
    conn: &mut Connection,
    batch: &mut Batch,
    index: &mut IdIndex,
    stats: &mut WriterStats,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    stats.urls += batch.urls.len() as u64;
    stats.pages += batch.pages.len() as u64;
    stats.links += batch.links.len() as u64;
    stats.images += batch.images.len() as u64;
    stats.issues += batch.issues.len() as u64;
    stats.resources += batch.resources.len() as u64;
    stats.batches += 1;

    write_batch(conn, batch, index)?;
    *batch = Batch::default();
    Ok(())
}

/// Mapa hash → id para los casos en que el motor sí necesita el `id` en memoria.
pub fn load_hash_index(conn: &Connection) -> Result<HashMap<i64, i64>> {
    let mut stmt = conn.prepare("SELECT url_hash, id FROM urls")?;
    let mut map = HashMap::new();
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (hash, id) = row?;
        map.insert(hash, id);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().expect("abrir memoria");
        store::migrate(&c).expect("migrar");
        c
    }

    /// Índice vacío, como al empezar un rastreo.
    fn idx() -> IdIndex {
        IdIndex::new()
    }

    fn url_row(url: &str, hash: i64, status: Option<u16>) -> UrlRow {
        let parsed = url::Url::parse(url).expect("URL válida");
        UrlRow {
            url: url.to_string(),
            url_hash: hash,
            scheme: parsed.scheme().to_string(),
            host: parsed.host_str().unwrap_or_default().to_string(),
            path: parsed.path().to_string(),
            query: parsed.query().map(|q| q.to_string()),
            depth: Some(0),
            discovered_from: None,
            is_internal: true,
            in_sitemap: false,
            crawl_state: if status.is_some() { CrawlState::Done } else { CrawlState::Pending },
            exclusion_reason: None,
            status_code: status,
            redirect_to_hash: None,
            redirect_chain_len: 0,
            content_type: Some("text/html".into()),
            content_length: Some(1234),
            response_time_ms: Some(50),
            fetched_at: Some("2026-07-26T12:00:00Z".into()),
            error_kind: None,
            error_message: None,
        }
    }

    fn page_row(hash: i64, title: Option<&str>) -> PageRow {
        PageRow {
            url_hash: hash,
            title: title.map(|s| s.to_string()),
            meta_description: None,
            h1: None,
            h1_count: 0,
            h2_count: 0,
            heading_json: "[]".into(),
            canonical: None,
            canonical_is_self: None,
            meta_robots: None,
            x_robots_tag: None,
            is_indexable: true,
            indexability_reason: None,
            lang: Some("es".into()),
            hreflang_json: None,
            word_count: 100,
            text_hash: None,
            html_hash: None,
            content_ratio: 0.2,
            viewport: None,
            og_json: None,
            twitter_json: None,
            schema_types: None,
            amp_url: None,
            internal_links_out: 0,
            crawl_depth_source: DiscoverySource::Link,
            body_text: None,
        }
    }

    #[test]
    fn escribe_un_lote_completo_en_una_transaccion() {
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            page: Some(page_row(1, Some("Título"))),
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir lote");

        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
            .expect("contar urls");
        assert_eq!(n, 1);
        let title: String = c
            .query_row("SELECT title FROM pages", [], |r| r.get(0))
            .expect("leer título");
        assert_eq!(title, "Título");
    }

    #[test]
    fn una_url_descubierta_y_luego_rastreada_se_completa_en_vez_de_duplicarse() {
        // Es el caso normal: se inserta como `pending` al verla y se completa al rastrearla.
        let mut c = conn();
        let mut idx = idx();

        let mut first = Batch::default();
        first.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, None)),
            ..Default::default()
        });
        write_batch(&mut c, &first, &mut idx).expect("primer lote");

        let mut second = Batch::default();
        second.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(404))),
            ..Default::default()
        });
        write_batch(&mut c, &second, &mut idx).expect("segundo lote");

        let (n, status, state): (i64, i64, String) = c
            .query_row(
                "SELECT COUNT(*), MAX(status_code), MAX(crawl_state) FROM urls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("consultar");
        assert_eq!(n, 1, "no debe duplicarse");
        assert_eq!(status, 404, "el segundo insert completa la fila");
        assert_eq!(state, "done");
    }

    #[test]
    fn resuelve_los_extremos_de_un_enlace_por_hash() {
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            ..Default::default()
        });
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/b", 2, Some(200))),
            links: vec![LinkRow {
                from_hash: 1,
                to_hash: 2,
                anchor: Some("ir a b".into()),
                rel: None,
                is_nofollow: false,
                element: LinkElement::A,
                region: Region::Main,
                position: 0,
            }],
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir");

        let (from_url, to_url, anchor): (String, String, String) = c
            .query_row(
                "SELECT uf.url, ut.url, l.anchor FROM links l
                 JOIN urls uf ON uf.id = l.from_url_id
                 JOIN urls ut ON ut.id = l.to_url_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("leer enlace");
        assert_eq!(from_url, "https://ejemplo.es/a");
        assert_eq!(to_url, "https://ejemplo.es/b");
        assert_eq!(anchor, "ir a b");
    }

    #[test]
    fn resuelve_el_destino_de_una_redireccion_del_mismo_lote() {
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        let mut origin = url_row("https://ejemplo.es/vieja", 1, Some(301));
        origin.redirect_to_hash = Some(2);
        origin.redirect_chain_len = 1;
        batch.push(CrawlResult { url: Some(origin), ..Default::default() });
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/nueva", 2, Some(200))),
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir");

        let destino: String = c
            .query_row(
                "SELECT t.url FROM urls o JOIN urls t ON t.id = o.redirect_to
                 WHERE o.url_hash = 1",
                [],
                |r| r.get(0),
            )
            .expect("leer destino");
        assert_eq!(destino, "https://ejemplo.es/nueva");
    }

    #[test]
    fn guarda_hallazgos_de_url_y_de_sitio() {
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            issues: vec![
                IssueRow {
                    url_hash: Some(1),
                    rule_id: "META-TITLE-MISSING".into(),
                    severity: "high".into(),
                    category: "meta".into(),
                    detail_json: None,
                    group_key: None,
                },
                IssueRow {
                    url_hash: None,
                    rule_id: "SITE-NO-SITEMAP".into(),
                    severity: "medium".into(),
                    category: "site".into(),
                    detail_json: None,
                    group_key: None,
                },
            ],
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir");

        let con_url: i64 = c
            .query_row("SELECT COUNT(*) FROM issues WHERE url_id IS NOT NULL", [], |r| r.get(0))
            .expect("contar");
        let de_sitio: i64 = c
            .query_row("SELECT COUNT(*) FROM issues WHERE url_id IS NULL", [], |r| r.get(0))
            .expect("contar");
        assert_eq!(con_url, 1);
        assert_eq!(de_sitio, 1);
    }

    // --- Búsqueda de texto completo ---

    #[test]
    fn indexa_la_pagina_en_fts_cuando_trae_texto_de_cuerpo() {
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        let mut page = page_row(1, Some("Guía de diseño"));
        page.meta_description = Some("Fotografía nocturna en la montaña".into());
        page.body_text = Some("El diseño de páginas rápidas empieza por el texto.".into());
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/diseno", 1, Some(200))),
            page: Some(page),
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir");

        // La promesa del tokenizador (`remove_diacritics 2`): «diseño» se encuentra sin tilde.
        let url: String = c
            .query_row(
                "SELECT u.url FROM pages_fts f JOIN urls u ON u.id = f.rowid
                 WHERE pages_fts MATCH 'diseno'",
                [],
                |r| r.get(0),
            )
            .expect("buscar sin tilde");
        assert_eq!(url, "https://ejemplo.es/diseno");

        // Y las otras columnas declaradas por la migración también se indexan.
        for consulta in ["title:guia", "meta_description:fotografia", "body_text:rapidas"] {
            let n: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM pages_fts WHERE pages_fts MATCH ?1",
                    rusqlite::params![consulta],
                    |r| r.get(0),
                )
                .expect("buscar por columna");
            assert_eq!(n, 1, "sin resultado para {consulta}");
        }
    }

    #[test]
    fn sin_texto_de_cuerpo_la_fts_queda_vacia() {
        // Es el comportamiento documentado: la FTS solo se puebla cuando el trabajo pide
        // `collect_body_text` (nivel Pro). Un rastreo Free no debe dejarla a medias.
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            page: Some(page_row(1, Some("Título sin cuerpo"))),
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir");

        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM pages_fts", [], |r| r.get(0))
            .expect("contar");
        assert_eq!(n, 0, "sin body_text no se indexa nada, ni siquiera el título");
    }

    #[test]
    fn reescribir_una_pagina_no_acumula_entradas_duplicadas_en_fts() {
        // `pages_fts` es contentless: no admite DELETE, y un segundo INSERT con el mismo rowid
        // se *suma* al primero en vez de sustituirlo. La guarda del `NOT EXISTS` tiene que
        // dejar la primera entrada y descartar la segunda.
        let mut c = conn();
        let mut idx = idx();

        for _ in 0..2 {
            let mut batch = Batch::default();
            let mut page = page_row(1, Some("Repetida"));
            page.body_text = Some("contenido repetido".into());
            batch.push(CrawlResult {
                url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
                page: Some(page),
                ..Default::default()
            });
            write_batch(&mut c, &batch, &mut idx).expect("escribir");
        }

        let n: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM pages_fts WHERE pages_fts MATCH 'repetido'",
                [],
                |r| r.get(0),
            )
            .expect("contar coincidencias");
        assert_eq!(n, 1, "una página, una entrada en el índice");
    }

    #[test]
    fn calcula_longitud_y_ancho_en_pixeles_del_titulo() {
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            page: Some(page_row(1, Some("Título de prueba"))),
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir");

        let (len, px): (i64, i64) = c
            .query_row("SELECT title_len, title_px FROM pages", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("leer");
        assert_eq!(len, 16, "cuenta caracteres, no bytes: la í ocupa dos bytes");
        assert!(px > 0);
    }

    #[test]
    fn el_ancho_en_pixeles_distingue_caracteres_anchos_de_estrechos() {
        // 60 caracteres no dicen nada: lo que trunca Google es el ancho.
        let estrecho = estimate_pixel_width_title(&"i".repeat(40));
        let ancho = estimate_pixel_width_title(&"M".repeat(40));
        assert!(ancho > estrecho * 2, "M debería ser mucho más ancha que i");
    }

    #[test]
    fn un_lote_vacio_no_falla() {
        let mut c = conn();
        let mut idx = idx();
        let batch = Batch::default();
        assert!(batch.is_empty());
        write_batch(&mut c, &batch, &mut idx).expect("un lote vacío es válido");
    }

    #[test]
    fn el_lote_avisa_cuando_esta_lleno() {
        let mut batch = Batch::default();
        assert!(!batch.is_full());
        for i in 0..BATCH_SIZE {
            batch.push(CrawlResult {
                url: Some(url_row(&format!("https://ejemplo.es/{i}"), i as i64, Some(200))),
                ..Default::default()
            });
        }
        assert!(batch.is_full());
        assert_eq!(batch.len(), BATCH_SIZE);

        let taken = batch.take();
        assert_eq!(taken.len(), BATCH_SIZE);
        assert!(batch.is_empty(), "take deja el lote vacío para seguir acumulando");
    }

    #[test]
    fn un_enlace_a_una_url_desconocida_no_se_inserta_ni_rompe_el_lote() {
        // El JOIN por hash no encuentra el destino: la fila se omite en silencio en vez de
        // violar la clave foránea y tumbar la transacción entera.
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            links: vec![LinkRow {
                from_hash: 1,
                to_hash: 999,
                anchor: None,
                rel: None,
                is_nofollow: false,
                element: LinkElement::A,
                region: Region::Unknown,
                position: 0,
            }],
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("no debe fallar");

        let n: i64 = c.query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0)).expect("contar");
        assert_eq!(n, 0);
    }

    #[test]
    fn carga_el_indice_de_hashes() {
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 111, Some(200))),
            ..Default::default()
        });
        write_batch(&mut c, &batch, &mut idx).expect("escribir");

        let index = load_hash_index(&c).expect("cargar índice");
        assert!(index.contains_key(&111));
    }

    // --- Hilo escritor ---

    fn fichero_temporal(nombre: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("cf-writer-{}-{nombre}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[tokio::test]
    async fn el_hilo_escritor_guarda_lo_que_se_le_manda_y_devuelve_lo_escrito() {
        let path = fichero_temporal("basico");
        { store::open_writer(&path).expect("migrar"); }

        let w = WriterHandle::spawn(path.clone()).expect("arrancar escritor");
        w.send(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            page: Some(page_row(1, Some("Título"))),
            ..Default::default()
        })
        .await.expect("enviar");
        let stats = w.finish().await.expect("cerrar");

        assert_eq!(stats.urls, 1);
        assert_eq!(stats.pages, 1);
        assert!(stats.batches >= 1);

        let c = Connection::open(&path).expect("reabrir");
        let n: i64 = c.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0)).expect("contar");
        assert_eq!(n, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn el_escritor_resuelve_los_enlaces_sin_join() {
        // El índice en memoria debe emparejar los extremos aunque lleguen en mensajes distintos.
        let path = fichero_temporal("enlaces");
        { store::open_writer(&path).expect("migrar"); }

        let w = WriterHandle::spawn(path.clone()).expect("arrancar");
        w.send(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            ..Default::default()
        })
        .await.expect("enviar a");
        w.send(CrawlResult {
            url: Some(url_row("https://ejemplo.es/b", 2, Some(200))),
            links: vec![LinkRow {
                from_hash: 1,
                to_hash: 2,
                anchor: Some("ir".into()),
                rel: None,
                is_nofollow: false,
                element: LinkElement::A,
                region: Region::Main,
                position: 0,
            }],
            ..Default::default()
        })
        .await.expect("enviar b");
        let stats = w.finish().await.expect("cerrar");
        assert_eq!(stats.links, 1);

        let c = Connection::open(&path).expect("reabrir");
        let (desde, hacia): (String, String) = c
            .query_row(
                "SELECT f.url, t.url FROM links l
                 JOIN urls f ON f.id = l.from_url_id JOIN urls t ON t.id = l.to_url_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("leer enlace");
        assert_eq!(desde, "https://ejemplo.es/a");
        assert_eq!(hacia, "https://ejemplo.es/b");
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn el_escritor_vuelca_por_tiempo_sin_llegar_al_tamano_de_lote() {
        // Sin el volcado por tiempo, un rastreo lento dejaría el fichero sin actualizar y la UI,
        // que lee ese mismo fichero, parecería congelada.
        let path = fichero_temporal("por-tiempo");
        { store::open_writer(&path).expect("migrar"); }

        let w = WriterHandle::spawn(path.clone()).expect("arrancar");
        w.send(CrawlResult {
            url: Some(url_row("https://ejemplo.es/a", 1, Some(200))),
            ..Default::default()
        })
        .await.expect("enviar");

        // Una sola fila no llena el lote de 200: solo el timeout puede haberla escrito.
        tokio::time::sleep(BATCH_INTERVAL + Duration::from_millis(600)).await;
        let c = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )
        .expect("abrir en lectura");
        let n: i64 = c.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0)).expect("contar");
        assert_eq!(n, 1, "el lote debería haberse volcado por tiempo");

        drop(c);
        w.finish().await.expect("cerrar");
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn cerrar_el_escritor_sin_haber_mandado_nada_no_falla() {
        let path = fichero_temporal("vacio");
        { store::open_writer(&path).expect("migrar"); }
        let w = WriterHandle::spawn(path.clone()).expect("arrancar");
        let stats = w.finish().await.expect("cerrar");
        assert_eq!(stats.urls, 0);
        assert_eq!(stats.batches, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn una_pagina_sin_su_url_se_omite_en_vez_de_romper_el_lote() {
        // El índice no tiene ese hash: la fila se descarta sin violar la clave foránea.
        let mut c = conn();
        let mut idx = idx();
        let mut batch = Batch::default();
        batch.push(CrawlResult { page: Some(page_row(999, Some("Huérfana"))), ..Default::default() });
        write_batch(&mut c, &batch, &mut idx).expect("no debe fallar");
        assert_eq!(
            c.query_row::<i64, _, _>("SELECT COUNT(*) FROM pages", [], |r| r.get(0)).expect("contar"),
            0
        );
    }

    #[test]
    fn los_estados_de_rastreo_coinciden_con_el_esquema() {
        assert_eq!(CrawlState::Pending.as_str(), "pending");
        assert_eq!(CrawlState::Done.as_str(), "done");
        assert_eq!(CrawlState::Error.as_str(), "error");
        assert_eq!(CrawlState::Excluded.as_str(), "excluded");
        assert_eq!(CrawlState::Skipped.as_str(), "skipped");
    }
}
