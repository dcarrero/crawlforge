//! La ficha completa de una URL: `crawlforge inspect <fichero> <url>`.
//!
//! Nace de la revisión de UX 2026-08-01 §5.2: «¿quién enlaza a esta página?» es la pregunta
//! que un consultor hace veinte veces por auditoría y la razón número uno para reabrir el
//! panel de *Inlinks* de Screaming Frog. Los datos estaban todos en `links`; faltaba el
//! comando. Los **enlaces entrantes son la sección estrella** y las demás —estado, extracción,
//! hallazgos, redirecciones, salientes e imágenes— evitan abrir el XLSX para mirar una URL.
//!
//! # Las tres decisiones de diseño
//!
//! **Cuántos entrantes se enseñan.** La lista se deduplica por página que enlaza —los 13
//! enlaces del pie de una misma página son una línea con `×13`, no trece— y se cortan a
//! [`DEFAULT_LIMIT`] por defecto, con los enlaces de contenido (`region = 'main'`) primero:
//! son los editoriales, que es lo que el consultor busca entre el ruido de plantilla. El
//! agregado por región da el total sin cortar nada, y la línea de corte dice el comando
//! exacto (`--limit all`) que lista todo, porque cortar sin decir cómo ver el resto es el
//! error del que ya nació `report --rule` (§5.1).
//!
//! **Cómo se identifica la URL.** Exacta primero (índice único, 2 ms); si no, variantes
//! baratas que también son búsquedas exactas: con y sin barra final, `https`/`http`, y una
//! ruta (`/blog/`) resuelta contra la `base_url` del rastreo. Solo si todo eso falla se hace
//! la única pasada cara —un `instr` sobre el índice de URLs, ~3,7 s en frío con 1,29 M de
//! URLs— y es para el mensaje de error: sugerir las más parecidas en vez de decir «no está».
//!
//! **Formato.** `terminal` por defecto y `--format md` para pegar en un ticket, con `--out`,
//! como hace `report`. La salida se traduce (`--lang`), como `report` y a diferencia de la
//! salida de desarrollo de `--bench`.
//!
//! # Coste medido sobre un rastreo real
//!
//! Todas las consultas van por índices existentes (`EXPLAIN QUERY PLAN` sobre
//! `un-diario-completo.sqlite`: 1,29 M URLs, 31,8 M enlaces). El peor caso es la portada de un
//! sitio grande: 1,08 M de filas entrantes obligan a dos pasadas por `idx_links_to` (~8 s
//! cada una en frío, <1 s en caliente). Una página normal responde en milisegundos.

use crate::i18n::{self, msg};
use anyhow::{bail, Context, Result};
use crawlforge_rules::Lang;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::fmt::Write as _;
use std::path::Path;

/// Cuántas líneas se listan por sección (páginas que enlazan, destinos, imágenes) sin
/// `--limit`. Suficiente para reconocer el patrón, poco para no sepultar el resto de la
/// ficha; los totales van en las cabeceras y la lista completa a un `--limit all`.
/// El `default_value = "20"` del flag en `main.rs` debe decir este mismo número.
pub const DEFAULT_LIMIT: u32 = 20;

/// Longitud máxima del texto de ancla en pantalla. Más allá es prosa, no un ancla.
const MAX_ANCHOR_CHARS: usize = 60;

/// Tope de saltos al seguir una cadena de redirecciones: por encima hay un bucle o un
/// fichero fabricado, y en ambos casos lo honesto es cortar.
const MAX_REDIRECT_HOPS: usize = 12;

/// Cuántas URLs parecidas sugiere el error de «no está en este rastreo».
const MAX_SUGGESTIONS: usize = 6;

/// El límite de las listas de la ficha: un número o `all`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListLimit {
    All,
    N(u32),
}

impl ListLimit {
    /// El valor para un `LIMIT ?` de SQLite: `-1` es «sin límite».
    fn sql(self) -> i64 {
        match self {
            ListLimit::All => -1,
            // Se pide una fila de más para saber si hay corte, sin contar toda la tabla.
            ListLimit::N(n) => i64::from(n) + 1,
        }
    }

    fn shown(self) -> usize {
        match self {
            ListLimit::All => usize::MAX,
            ListLimit::N(n) => n as usize,
        }
    }
}

/// `value_parser` de clap para `--limit`. El error habla del contrato, no del tipo.
pub fn parse_limit(s: &str) -> std::result::Result<ListLimit, String> {
    if s.trim().eq_ignore_ascii_case("all") {
        return Ok(ListLimit::All);
    }
    match s.trim().parse::<u32>() {
        Ok(n) if n >= 1 => Ok(ListLimit::N(n)),
        _ => Err(format!("'{s}' is not a limit: use a number of 1 or more, or 'all'")),
    }
}

/// El formato de salida de la ficha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Terminal,
    Md,
}

/// Genera la ficha de `input` leyendo `store` en solo lectura.
///
/// El error de «no encontrada» incluye las URLs más parecidas del fichero: la respuesta a
/// una errata debe acercar, no solo negar.
pub fn render_card(
    store: &Path,
    input: &str,
    limit: ListLimit,
    format: &str,
    lang: Lang,
) -> Result<String> {
    let format = match format.trim().to_ascii_lowercase().as_str() {
        "terminal" => Format::Terminal,
        "md" => Format::Md,
        // El contrato del flag es inglés, como los errores de clap (ver `main.rs` cabecera).
        otro => bail!("format not recognised: {otro}. Available: terminal and md"),
    };
    let conn = Connection::open_with_flags(
        store,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("could not open {}", store.display()))?;

    let Some(row) = resolve_url(&conn, input)? else {
        bail!(not_found_error(&conn, input, lang)?);
    };
    // El comando de «lista completa», con la URL entre comillas: un `?` o un `&` sin
    // comillas se lo comería la shell.
    let full_cmd = format!("crawlforge inspect {} '{}' --limit all", store.display(), row.url);
    build_card(&conn, &row, limit, format, lang, &full_cmd)
}

// ────────────────────────────────────────────────────── Identificación de la URL

/// La fila de `urls` de la URL inspeccionada.
struct UrlRow {
    id: i64,
    url: String,
    status_code: Option<i64>,
    content_type: Option<String>,
    response_time_ms: Option<i64>,
    depth: Option<i64>,
    in_sitemap: bool,
    crawl_state: String,
    exclusion_reason: Option<String>,
    redirect_to: Option<i64>,
    error_kind: Option<String>,
    error_message: Option<String>,
}

fn fetch_url_row(conn: &Connection, id: i64) -> Result<Option<UrlRow>> {
    let row = conn
        .query_row(
            "SELECT id, url, status_code, content_type, response_time_ms, depth, in_sitemap,
                    crawl_state, exclusion_reason, redirect_to, error_kind, error_message
             FROM urls WHERE id = ?1",
            [id],
            |r| {
                Ok(UrlRow {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    status_code: r.get(2)?,
                    content_type: r.get(3)?,
                    response_time_ms: r.get(4)?,
                    depth: r.get(5)?,
                    in_sitemap: r.get::<_, i64>(6)? != 0,
                    crawl_state: r.get(7)?,
                    exclusion_reason: r.get(8)?,
                    redirect_to: r.get(9)?,
                    error_kind: r.get(10)?,
                    error_message: r.get(11)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Resuelve lo tecleado a una fila de `urls`, probando solo búsquedas exactas (baratas).
///
/// Orden: tal cual → variantes de barra final y esquema → una ruta (`/blog/`) unida a la
/// `base_url` del rastreo. La búsqueda difusa no vive aquí: es cara y pertenece al error.
fn resolve_url(conn: &Connection, input: &str) -> Result<Option<UrlRow>> {
    for candidate in candidate_urls(conn, input)? {
        let id: Option<i64> = conn
            .query_row("SELECT id FROM urls WHERE url = ?1", [&candidate], |r| r.get(0))
            .optional()?;
        if let Some(id) = id {
            return fetch_url_row(conn, id);
        }
    }
    Ok(None)
}

/// Las formas exactas en las que lo tecleado puede estar en el fichero, en orden de fe.
fn candidate_urls(conn: &Connection, input: &str) -> Result<Vec<String>> {
    let input = input.trim();
    let mut out: Vec<String> = Vec::new();
    let mut push = |cand: String| {
        if !out.contains(&cand) {
            out.push(cand);
        }
    };

    if input.starts_with('/') {
        // Una ruta se resuelve contra la base del rastreo: convierte el caso «/blog/» en
        // búsquedas exactas por índice en vez de un recorrido de `urls.path`, que no tiene
        // índice y se midió en ~7 s sobre un rastreo real.
        let base: Option<String> = conn
            .query_row("SELECT base_url FROM crawl_meta LIMIT 1", [], |r| r.get(0))
            .optional()?;
        if let Some(base) = base.and_then(|b| url::Url::parse(&b).ok()) {
            if let Ok(joined) = base.join(input) {
                for v in slash_and_scheme_variants(&joined) {
                    push(v);
                }
            }
        }
        return Ok(out);
    }

    let with_scheme = if input.split_once("://").is_some() {
        vec![input.to_string()]
    } else {
        // Sin esquema se prueba `https` y luego `http`, el mismo criterio que `crawl`.
        vec![format!("https://{input}"), format!("http://{input}")]
    };
    for candidate in with_scheme {
        match url::Url::parse(&candidate) {
            Ok(parsed) => {
                for v in slash_and_scheme_variants(&parsed) {
                    push(v);
                }
            }
            // Lo que no parsea se intenta literal: el fichero decide, no nosotros.
            Err(_) => push(candidate),
        }
    }
    Ok(out)
}

/// La URL con y sin barra final, en sus dos esquemas. Todas son búsquedas exactas.
fn slash_and_scheme_variants(parsed: &url::Url) -> Vec<String> {
    let mut out = vec![parsed.to_string()];
    if let Some(toggled) = toggle_trailing_slash(parsed) {
        out.push(toggled);
    }
    let other = if parsed.scheme() == "https" { "http" } else { "https" };
    let mut swapped = parsed.clone();
    if swapped.set_scheme(other).is_ok() {
        out.push(swapped.to_string());
        if let Some(toggled) = toggle_trailing_slash(&swapped) {
            out.push(toggled);
        }
    }
    out
}

/// `/blog` ↔ `/blog/`. La raíz (`/`) no se toca: sin ella la URL no es la misma autoridad.
fn toggle_trailing_slash(parsed: &url::Url) -> Option<String> {
    let path = parsed.path();
    if path == "/" {
        return None;
    }
    let mut toggled = parsed.clone();
    if let Some(stripped) = path.strip_suffix('/') {
        toggled.set_path(stripped);
    } else {
        toggled.set_path(&format!("{path}/"));
    }
    Some(toggled.to_string())
}

/// El error de «no está», con las URLs más parecidas del fichero si las hay.
///
/// La búsqueda es un `instr` sobre el índice de URLs: es la consulta más cara del comando
/// (~3,7 s en frío con 1,29 M de URLs) y por eso vive solo en el camino del error.
fn not_found_error(conn: &Connection, input: &str, lang: Lang) -> Result<String> {
    let mut texto = msg::inspect_error_not_found(lang, sanitize(input));
    let needle = suggestion_needle(input);
    if !needle.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT url FROM urls WHERE instr(url, ?1) > 0 ORDER BY length(url) LIMIT ?2",
        )?;
        let parecidas: Vec<String> = stmt
            .query_map(rusqlite::params![needle, MAX_SUGGESTIONS as i64], |r| {
                r.get::<_, String>(0)
            })?
            .filter_map(std::result::Result::ok)
            .map(|u| sanitize(&u))
            .collect();
        if !parecidas.is_empty() {
            texto.push('\n');
            texto.push_str(&msg::inspect_closest(lang));
            for url in parecidas {
                texto.push_str("\n  ");
                texto.push_str(&url);
            }
        }
    }
    Ok(texto)
}

/// Lo más específico de lo tecleado: el último tramo de la ruta, o el host, o todo.
fn suggestion_needle(input: &str) -> String {
    let sin_esquema = input.trim().split_once("://").map_or(input.trim(), |(_, rest)| rest);
    let sin_query = sin_esquema.split(['?', '#']).next().unwrap_or(sin_esquema);
    sin_query
        .trim_end_matches('/')
        .rsplit('/')
        .find(|seg| !seg.is_empty())
        .unwrap_or(sin_query)
        .to_string()
}

// ──────────────────────────────────────────────────────────── La ficha en sí

/// Construye la ficha entera. Separada de [`render_card`] para poder afirmar la salida en
/// tests sin pasar por la resolución de rutas.
fn build_card(
    conn: &Connection,
    row: &UrlRow,
    limit: ListLimit,
    format: Format,
    lang: Lang,
    full_cmd: &str,
) -> Result<String> {
    let mut s = String::new();
    let mut w = Writer { s: &mut s, format };

    w.title(&sanitize(&row.url))?;
    status_section(&mut w, row, lang)?;
    redirect_section(&mut w, conn, row, lang)?;
    page_section(&mut w, conn, row, lang)?;
    findings_section(&mut w, conn, row, lang)?;
    inlinks_section(&mut w, conn, row, limit, lang, full_cmd)?;
    outlinks_section(&mut w, conn, row, limit, lang, full_cmd)?;
    images_section(&mut w, conn, row, limit, lang, full_cmd)?;
    image_usage_section(&mut w, conn, row, limit, lang, full_cmd)?;
    Ok(s)
}

/// La maquetación mínima que separa el terminal del Markdown: título, sección, par
/// etiqueta-valor y línea de lista. Todo lo demás es idéntico en los dos formatos.
struct Writer<'a> {
    s: &'a mut String,
    format: Format,
}

impl Writer<'_> {
    fn title(&mut self, text: &str) -> Result<()> {
        match self.format {
            Format::Terminal => writeln!(self.s, "{text}")?,
            Format::Md => writeln!(self.s, "# {text}")?,
        }
        Ok(())
    }

    fn section(&mut self, title: &str) -> Result<()> {
        writeln!(self.s)?;
        match self.format {
            Format::Terminal => writeln!(self.s, "{}", i18n::section(title))?,
            Format::Md => writeln!(self.s, "## {title}\n")?,
        }
        Ok(())
    }

    fn kv(&mut self, label: &str, value: &str) -> Result<()> {
        match self.format {
            // 19 y no 18: la etiqueta más larga del catálogo («Tiempo de respuesta») mide 19.
            Format::Terminal => writeln!(self.s, "  {label:<19} {value}")?,
            Format::Md => writeln!(self.s, "- **{label}**: {value}")?,
        }
        Ok(())
    }

    fn item(&mut self, text: &str) -> Result<()> {
        match self.format {
            Format::Terminal => writeln!(self.s, "    {text}")?,
            Format::Md => writeln!(self.s, "- {text}")?,
        }
        Ok(())
    }

    fn note(&mut self, text: &str) -> Result<()> {
        match self.format {
            Format::Terminal => writeln!(self.s, "  {text}")?,
            Format::Md => writeln!(self.s, "{text}")?,
        }
        Ok(())
    }
}

fn status_section(w: &mut Writer<'_>, row: &UrlRow, lang: Lang) -> Result<()> {
    w.section(&msg::inspect_status_title(lang))?;
    if let Some(code) = row.status_code {
        w.kv(&msg::label_http_status(lang), &code.to_string())?;
    }
    if let Some(ct) = &row.content_type {
        w.kv(&msg::label_content_type(lang), &sanitize(ct))?;
    }
    if let Some(ms) = row.response_time_ms {
        w.kv(&msg::label_response_time(lang), &format!("{ms} ms"))?;
    }
    if let Some(depth) = row.depth {
        w.kv(
            &msg::label_depth(lang),
            &format!("{depth} ({})", msg::note_depth_clicks(lang)),
        )?;
    }
    let si_no = if row.in_sitemap { msg::word_yes(lang) } else { msg::word_no(lang) };
    w.kv(&msg::label_in_sitemap(lang), &si_no)?;

    // El estado solo se nombra cuando cuenta algo: 'done' es lo normal y sería ruido.
    match row.crawl_state.as_str() {
        "done" => {}
        "pending" => w.kv(
            &msg::label_crawl_state(lang),
            &format!("pending — {}", msg::state_note_pending(lang)),
        )?,
        "excluded" | "skipped" => {
            let reason = row.exclusion_reason.as_deref().unwrap_or("-");
            w.kv(
                &msg::label_crawl_state(lang),
                &format!("{} — {}", row.crawl_state, msg::state_note_excluded(lang, reason)),
            )?;
        }
        "error" => {
            let detalle = match (&row.error_kind, &row.error_message) {
                (Some(k), Some(m)) => format!("{k}: {}", sanitize(m)),
                (Some(k), None) => k.clone(),
                (None, Some(m)) => sanitize(m),
                (None, None) => "-".to_string(),
            };
            w.kv(
                &msg::label_crawl_state(lang),
                &format!("error — {}", msg::state_note_error(lang, detalle)),
            )?;
        }
        otro => w.kv(&msg::label_crawl_state(lang), &sanitize(otro))?,
    }
    Ok(())
}

fn redirect_section(w: &mut Writer<'_>, conn: &Connection, row: &UrlRow, lang: Lang) -> Result<()> {
    if row.redirect_to.is_none() {
        return Ok(());
    }
    w.section(&msg::inspect_redirects_title(lang))?;

    // La propia URL abre la cadena; cada salto se lee del fichero. Un fichero fabricado
    // puede traer un bucle, así que se registran los visitados y se corta con un tope.
    let mut visitados = vec![row.id];
    let status = row.status_code.map_or_else(|| "—".to_string(), |c| c.to_string());
    w.item(&format!("{status:>4}  {}", sanitize(&row.url)))?;
    let mut siguiente = row.redirect_to;
    while let Some(id) = siguiente {
        if visitados.contains(&id) || visitados.len() >= MAX_REDIRECT_HOPS {
            w.item("   …  (redirect loop)")?;
            break;
        }
        visitados.push(id);
        let Some(hop) = fetch_url_row(conn, id)? else { break };
        let status = match hop.status_code {
            Some(c) => c.to_string(),
            None => hop.crawl_state.clone(),
        };
        w.item(&format!("{status:>4}  {}", sanitize(&hop.url)))?;
        siguiente = hop.redirect_to;
    }
    Ok(())
}

fn page_section(w: &mut Writer<'_>, conn: &Connection, row: &UrlRow, lang: Lang) -> Result<()> {
    struct PageRow {
        title: Option<String>,
        title_len: Option<i64>,
        meta_description: Option<String>,
        meta_desc_len: Option<i64>,
        h1: Option<String>,
        word_count: Option<i64>,
        canonical: Option<String>,
        canonical_is_self: Option<i64>,
        is_indexable: bool,
        indexability_reason: Option<String>,
    }
    let page = conn
        .query_row(
            "SELECT title, title_len, meta_description, meta_desc_len, h1, word_count,
                    canonical, canonical_is_self, is_indexable, indexability_reason
             FROM pages WHERE url_id = ?1",
            [row.id],
            |r| {
                Ok(PageRow {
                    title: r.get(0)?,
                    title_len: r.get(1)?,
                    meta_description: r.get(2)?,
                    meta_desc_len: r.get(3)?,
                    h1: r.get(4)?,
                    word_count: r.get(5)?,
                    canonical: r.get(6)?,
                    canonical_is_self: r.get(7)?,
                    is_indexable: r.get::<_, i64>(8)? != 0,
                    indexability_reason: r.get(9)?,
                })
            },
        )
        .optional()?;
    let Some(page) = page else { return Ok(()) };

    w.section(&msg::inspect_page_title(lang))?;
    let with_len = |texto: &Option<String>, len: Option<i64>, lang: Lang| -> String {
        match texto {
            Some(t) if !t.is_empty() => {
                let t = sanitize(t);
                match len {
                    Some(len) => format!("{t} ({})", msg::chars_suffix(lang, len)),
                    None => t,
                }
            }
            _ => "—".to_string(),
        }
    };
    w.kv(&msg::label_page_title(lang), &with_len(&page.title, page.title_len, lang))?;
    w.kv(
        &msg::label_meta_description(lang),
        &with_len(&page.meta_description, page.meta_desc_len, lang),
    )?;
    w.kv(&msg::label_h1(lang), &with_len(&page.h1, None, lang))?;
    if let Some(n) = page.word_count {
        w.kv(&msg::label_word_count(lang), &i18n::count(lang, n))?;
    }
    let canonical = match (&page.canonical, page.canonical_is_self) {
        (Some(_), Some(1)) => msg::canonical_self(lang),
        (Some(c), _) if !c.is_empty() => sanitize(c),
        _ => "—".to_string(),
    };
    w.kv(&msg::label_canonical(lang), &canonical)?;
    let indexable = if page.is_indexable {
        msg::word_yes(lang)
    } else {
        // El motivo (`noindex`, `canonicalised`…) es un valor de columna y no se traduce.
        msg::indexable_no(lang, page.indexability_reason.as_deref().unwrap_or("-"))
    };
    w.kv(&msg::label_indexable(lang), &indexable)?;
    Ok(())
}

fn findings_section(w: &mut Writer<'_>, conn: &Connection, row: &UrlRow, lang: Lang) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT rule_id, severity, COUNT(*) FROM issues WHERE url_id = ?1
         GROUP BY rule_id, severity
         ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2
                                WHEN 'low' THEN 3 ELSE 4 END, rule_id",
    )?;
    let filas: Vec<(String, String, i64)> = stmt
        .query_map([row.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if filas.is_empty() {
        return Ok(());
    }

    let n = filas.iter().map(|(_, _, c)| c).sum::<i64>();
    w.section(&format!("{} ({})", msg::heading_findings(lang), i18n::count(lang, n)))?;
    let catalogo = crawlforge_rules::catalog();
    for (rule, severity, cuantos) in filas {
        let rule = sanitize(&rule);
        let nombre = catalogo
            .iter()
            .find(|m| m.id == rule)
            .map(|m| m.name(lang))
            .unwrap_or_default();
        let veces = if cuantos > 1 { format!(" ×{cuantos}") } else { String::new() };
        w.item(&format!(
            "{:<9} {rule:<26} {nombre}{veces}",
            i18n::severity_word(lang, &severity)
        ))?;
    }
    Ok(())
}

fn inlinks_section(
    w: &mut Writer<'_>,
    conn: &Connection,
    row: &UrlRow,
    limit: ListLimit,
    lang: Lang,
    full_cmd: &str,
) -> Result<()> {
    // Una pasada por `idx_links_to` da el total, el desglose por región y los nofollow.
    // El recuento de páginas distintas se deja a la lista (que ya deduplica): contarlo
    // aparte costaría otra pasada entera, 8 s medidos en la portada de un sitio grande.
    let mut stmt = conn.prepare(
        "SELECT COALESCE(region, 'unknown') AS r, COUNT(*), COALESCE(SUM(is_nofollow), 0)
         FROM links WHERE to_url_id = ?1 GROUP BY r ORDER BY COUNT(*) DESC",
    )?;
    let regiones: Vec<(String, i64, i64)> = stmt
        .query_map([row.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let total: i64 = regiones.iter().map(|(_, n, _)| n).sum();
    let nofollow: i64 = regiones.iter().map(|(_, _, nf)| nf).sum();

    w.section(&msg::inlinks_title(lang, i18n::count(lang, total)))?;
    if total == 0 {
        w.note(&msg::inlinks_none(lang))?;
        return Ok(());
    }
    let desglose = regiones
        .iter()
        .map(|(r, n, _)| format!("{} {}", sanitize(r), i18n::count(lang, *n)))
        .collect::<Vec<_>>()
        .join(" · ");
    w.note(&msg::inlinks_by_region(lang, desglose, i18n::count(lang, nofollow)))?;
    w.note(&msg::inlinks_sample_heading(lang))?;

    // Una línea por página que enlaza, con su enlace de mejor región: los `MIN()` de SQLite
    // garantizan que las columnas sueltas salen de la fila que ganó el mínimo.
    let mut stmt = conn.prepare(
        "SELECT u.url, l.anchor, l.is_nofollow, COALESCE(l.region, 'unknown'), l.element,
                COUNT(*),
                MIN(CASE COALESCE(l.region, 'unknown')
                        WHEN 'main' THEN 0 WHEN 'aside' THEN 1 WHEN 'unknown' THEN 2
                        WHEN 'nav' THEN 3 ELSE 4 END) AS pri
         FROM links l JOIN urls u ON u.id = l.from_url_id
         WHERE l.to_url_id = ?1
         GROUP BY l.from_url_id
         ORDER BY pri, u.url
         LIMIT ?2",
    )?;
    let filas: Vec<LinkLine> = stmt
        .query_map(rusqlite::params![row.id, limit.sql()], |r| {
            Ok(LinkLine {
                url: r.get(0)?,
                anchor: r.get(1)?,
                is_nofollow: r.get::<_, i64>(2)? != 0,
                region: r.get(3)?,
                element: r.get(4)?,
                count: r.get(5)?,
                status_code: None,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let cortado = print_link_lines(w, &filas, limit, lang, LinkListKind::Inlinks)?;
    if cortado {
        w.note(&msg::inspect_truncated(lang, limit.shown(), full_cmd))?;
    }
    Ok(())
}

/// Una línea de la lista de enlaces, ya deduplicada por el `GROUP BY` de su consulta.
struct LinkLine {
    url: String,
    anchor: Option<String>,
    is_nofollow: bool,
    region: String,
    element: String,
    count: i64,
    /// Solo en salientes: el estado del destino, que es lo que delata un enlace roto.
    status_code: Option<i64>,
}

enum LinkListKind {
    Inlinks,
    Outlinks,
}

/// Pinta hasta `limit` líneas y devuelve si hubo corte (las consultas piden una fila extra).
fn print_link_lines(
    w: &mut Writer<'_>,
    filas: &[LinkLine],
    limit: ListLimit,
    lang: Lang,
    kind: LinkListKind,
) -> Result<bool> {
    let visibles = filas.len().min(limit.shown());
    for linea in &filas[..visibles] {
        let ancla = match &linea.anchor {
            Some(a) if !a.trim().is_empty() => format!("\"{}\"", clean_anchor(a)),
            _ => msg::no_anchor(lang),
        };
        let mut marcas = String::new();
        if linea.is_nofollow {
            marcas.push_str(" [nofollow]");
        }
        if linea.element != "a" {
            let _ = write!(marcas, " [{}]", sanitize(&linea.element));
        }
        if linea.count > 1 {
            let _ = write!(marcas, " ×{}", linea.count);
        }
        let texto = match kind {
            LinkListKind::Inlinks => format!(
                "{:<8} {ancla}{marcas} — {}",
                sanitize(&linea.region),
                sanitize(&linea.url)
            ),
            LinkListKind::Outlinks => {
                let status = linea
                    .status_code
                    .map_or_else(|| "—".to_string(), |c| c.to_string());
                format!("{status:>4}  {} {ancla}{marcas}", sanitize(&linea.url))
            }
        };
        w.item(&texto)?;
    }
    Ok(filas.len() > visibles)
}

fn outlinks_section(
    w: &mut Writer<'_>,
    conn: &Connection,
    row: &UrlRow,
    limit: ListLimit,
    lang: Lang,
    full_cmd: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT u.is_internal, COUNT(*)
         FROM links l JOIN urls u ON u.id = l.to_url_id
         WHERE l.from_url_id = ?1 GROUP BY u.is_internal",
    )?;
    let cuentas: Vec<(i64, i64)> = stmt
        .query_map([row.id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if cuentas.is_empty() {
        return Ok(());
    }
    let internos: i64 = cuentas.iter().filter(|(i, _)| *i != 0).map(|(_, n)| n).sum();
    let externos: i64 = cuentas.iter().filter(|(i, _)| *i == 0).map(|(_, n)| n).sum();

    w.section(&msg::outlinks_title(
        lang,
        i18n::count(lang, internos + externos),
        i18n::count(lang, internos),
        i18n::count(lang, externos),
    ))?;

    // Deduplicado por destino y con los rotos primero: el estado del destino es la columna
    // que convierte esta lista en el triaje de enlaces rotos de la página.
    let mut stmt = conn.prepare(
        "SELECT u.url, l.anchor, l.is_nofollow, COALESCE(l.region, 'unknown'), l.element,
                COUNT(*), u.status_code, MIN(l.position)
         FROM links l JOIN urls u ON u.id = l.to_url_id
         WHERE l.from_url_id = ?1
         GROUP BY l.to_url_id
         ORDER BY CASE WHEN u.status_code >= 400 THEN 0 ELSE 1 END,
                  u.is_internal DESC, MIN(l.position)
         LIMIT ?2",
    )?;
    let filas: Vec<LinkLine> = stmt
        .query_map(rusqlite::params![row.id, limit.sql()], |r| {
            Ok(LinkLine {
                url: r.get(0)?,
                anchor: r.get(1)?,
                is_nofollow: r.get::<_, i64>(2)? != 0,
                region: r.get(3)?,
                element: r.get(4)?,
                count: r.get(5)?,
                status_code: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let cortado = print_link_lines(w, &filas, limit, lang, LinkListKind::Outlinks)?;
    if cortado {
        w.note(&msg::inspect_truncated(lang, limit.shown(), full_cmd))?;
    }
    Ok(())
}

fn images_section(
    w: &mut Writer<'_>,
    conn: &Connection,
    row: &UrlRow,
    limit: ListLimit,
    lang: Lang,
    full_cmd: &str,
) -> Result<()> {
    let total: i64 =
        conn.query_row("SELECT COUNT(*) FROM images WHERE page_url_id = ?1", [row.id], |r| {
            r.get(0)
        })?;
    if total == 0 {
        return Ok(());
    }
    w.section(&msg::images_title(lang, i18n::count(lang, total)))?;
    let mut stmt = conn.prepare(
        "SELECT u.url, i.alt, i.alt_present, COUNT(*)
         FROM images i JOIN urls u ON u.id = i.src_url_id
         WHERE i.page_url_id = ?1
         GROUP BY i.src_url_id
         ORDER BY i.alt_present, u.url
         LIMIT ?2",
    )?;
    let filas: Vec<(String, Option<String>, i64, i64)> = stmt
        .query_map(rusqlite::params![row.id, limit.sql()], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let visibles = filas.len().min(limit.shown());
    for (src, alt, alt_present, cuantas) in &filas[..visibles] {
        let alt = match alt {
            Some(a) if *alt_present != 0 && !a.trim().is_empty() => {
                format!("\"{}\"", clean_anchor(a))
            }
            _ => msg::image_no_alt(lang),
        };
        let veces = if *cuantas > 1 { format!(" ×{cuantas}") } else { String::new() };
        w.item(&format!("{alt}{veces} — {}", sanitize(src)))?;
    }
    if filas.len() > visibles {
        w.note(&msg::inspect_truncated(lang, limit.shown(), full_cmd))?;
    }
    Ok(())
}

/// Si la URL inspeccionada **es** una imagen, quién la usa: la dirección contraria a
/// [`images_section`], y la que responde «¿de dónde sale esta imagen rota/huérfana?».
fn image_usage_section(
    w: &mut Writer<'_>,
    conn: &Connection,
    row: &UrlRow,
    limit: ListLimit,
    lang: Lang,
    full_cmd: &str,
) -> Result<()> {
    let (veces, paginas): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT page_url_id) FROM images WHERE src_url_id = ?1",
        [row.id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if veces == 0 {
        return Ok(());
    }
    w.section(&msg::image_usage_title(lang))?;
    w.note(&msg::image_usage_line(lang, i18n::count(lang, veces), i18n::count(lang, paginas)))?;
    let mut stmt = conn.prepare(
        "SELECT u.url, COUNT(*) FROM images i JOIN urls u ON u.id = i.page_url_id
         WHERE i.src_url_id = ?1 GROUP BY i.page_url_id ORDER BY u.url LIMIT ?2",
    )?;
    let filas: Vec<(String, i64)> = stmt
        .query_map(rusqlite::params![row.id, limit.sql()], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let visibles = filas.len().min(limit.shown());
    for (url, cuantas) in &filas[..visibles] {
        let veces = if *cuantas > 1 { format!(" ×{cuantas}") } else { String::new() };
        w.item(&format!("{}{veces}", sanitize(url)))?;
    }
    if filas.len() > visibles {
        w.note(&msg::inspect_truncated(lang, limit.shown(), full_cmd))?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────── Utilidades

/// Todo lo que viene del fichero pasa por aquí antes del terminal: el fichero es entrada no
/// confiable y una URL fabricada puede traer secuencias de escape (revisión §1.7d).
fn sanitize(s: &str) -> String {
    crate::audit_report::strip_control_chars(s)
}

/// Un ancla lista para pantalla: sin caracteres de control, sin saltos, y recortada — más
/// allá de [`MAX_ANCHOR_CHARS`] ya no es un ancla, es la prosa que la rodeaba.
///
/// El orden importa: primero se colapsa el espacio en blanco —un salto de línea dentro de
/// un ancla es HTML normal, no un ataque— y después se sanea, para que un escape de
/// terminal siga saliendo como U+FFFD y se vea que ahí había algo.
fn clean_anchor(s: &str) -> String {
    let colapsado = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let junto = sanitize(&colapsado);
    if junto.chars().count() > MAX_ANCHOR_CHARS {
        let mut corto: String = junto.chars().take(MAX_ANCHOR_CHARS - 1).collect();
        corto.push('…');
        corto
    } else {
        junto
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpdir(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("crawlforge-inspect-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear el directorio temporal");
        dir
    }

    /// Un rastreo con el esquema real (las migraciones del core) y un sitio pequeño con de
    /// todo: portada, blog con 30+ páginas que le enlazan, una redirección, un externo con
    /// nofollow, imágenes con y sin alt, hallazgos y una URL pendiente.
    fn store_de_prueba(nombre: &str) -> PathBuf {
        let dir = tmpdir(nombre);
        let path = dir.join("crawl.sqlite");
        let conn = crawlforge_core::store::open_writer(&path).expect("crear el rastreo");
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime)
             VALUES ('c','p','P','https://ejemplo.es/','http',datetime('now'),'done','{}',
                     '0','0','free')",
            [],
        )
        .expect("crawl_meta");

        let url = |id: i64, url: &str, interna: i64, estado: &str, code: Option<i64>| {
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal,
                                   in_sitemap, crawl_state, status_code, content_type,
                                   response_time_ms, depth)
                 VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', ?3, ?4, 1, ?5, ?6,
                         'text/html; charset=UTF-8', 120, 1)",
                rusqlite::params![
                    id,
                    url,
                    url::Url::parse(url).map(|u| u.path().to_string()).unwrap_or_default(),
                    interna,
                    estado,
                    code
                ],
            )
            .expect("insertar url");
        };
        url(1, "https://ejemplo.es/", 1, "done", Some(200));
        url(2, "https://ejemplo.es/blog/", 1, "done", Some(200));
        url(3, "https://ejemplo.es/contacto", 1, "done", Some(301));
        url(4, "https://ejemplo.es/contacto/", 1, "done", Some(200));
        url(5, "https://externo.com/", 0, "skipped", None);
        url(6, "https://ejemplo.es/logo.png", 1, "done", Some(200));
        url(7, "https://ejemplo.es/foto.jpg", 1, "done", Some(200));
        url(8, "https://ejemplo.es/pendiente", 1, "pending", None);
        // Las 30 páginas de archivo que enlazan al blog desde el pie: el ruido de plantilla.
        for i in 0..30 {
            url(100 + i, &format!("https://ejemplo.es/archivo/p{i:02}/"), 1, "done", Some(200));
        }
        conn.execute("UPDATE urls SET redirect_to = 4 WHERE id = 3", [])
            .expect("marcar la redirección");

        conn.execute(
            "INSERT INTO pages (url_id, title, title_len, meta_description, meta_desc_len, h1,
                                word_count, canonical, canonical_is_self, is_indexable)
             VALUES (2, 'El blog de Ejemplo', 18, 'Las novedades de Ejemplo.', 25,
                     'Últimas entradas', 500, 'https://ejemplo.es/blog/', 1, 1)",
            [],
        )
        .expect("page del blog");
        conn.execute(
            "INSERT INTO pages (url_id, title, is_indexable, indexability_reason)
             VALUES (1, 'Ejemplo', 0, 'noindex')",
            [],
        )
        .expect("page de la portada");

        let link = |from: i64, to: i64, anchor: &str, region: &str, nofollow: i64| {
            conn.execute(
                "INSERT INTO links (from_url_id, to_url_id, anchor, is_nofollow, element,
                                    region, position)
                 VALUES (?1, ?2, ?3, ?4, 'a', ?5, 1)",
                rusqlite::params![from, to, anchor, nofollow, region],
            )
            .expect("insertar link");
        };
        link(1, 2, "Blog", "nav", 0);
        link(4, 2, "Vuelve al blog", "main", 0);
        link(1, 5, "Amigos", "main", 1);
        link(2, 3, "Contacto", "main", 0);
        link(1, 8, "Próximamente", "main", 0);
        for i in 0..30 {
            link(100 + i, 2, "Blog", "footer", if i == 0 { 1 } else { 0 });
        }
        // La misma página repite el enlace del pie: debe salir una vez, con su ×2.
        link(100, 2, "Blog", "footer", 0);

        conn.execute(
            "INSERT INTO images (page_url_id, src_url_id, alt, alt_present)
             VALUES (2, 6, 'Logotipo de Ejemplo', 1), (2, 7, NULL, 0)",
            [],
        )
        .expect("imágenes");

        conn.execute(
            "INSERT INTO issues (url_id, rule_id, severity, category)
             VALUES (2, 'META-DESC-MISSING', 'high', 'meta'),
                    (2, 'ASSET-IMG-EMPTY-ALT-LINK', 'high', 'asset'),
                    (2, 'ASSET-IMG-EMPTY-ALT-LINK', 'high', 'asset'),
                    (NULL, 'SITEMAP-MISSING', 'medium', 'sitemap')",
            [],
        )
        .expect("hallazgos");
        drop(conn);
        path
    }

    fn ficha(store: &Path, input: &str) -> String {
        render_card(store, input, ListLimit::N(DEFAULT_LIMIT), "terminal", Lang::En)
            .expect("la ficha debe generarse")
    }

    // ── Lo que enseña la ficha ───────────────────────────────────────────────

    #[test]
    fn la_ficha_ensena_estado_pagina_y_hallazgos() {
        let store = store_de_prueba("ficha");
        let s = ficha(&store, "https://ejemplo.es/blog/");

        // Estado.
        assert!(s.contains("200"), "el código HTTP: {s}");
        assert!(s.contains("text/html"), "el tipo de contenido: {s}");
        assert!(s.contains("120 ms"), "el tiempo de respuesta: {s}");
        // Extracción.
        assert!(s.contains("El blog de Ejemplo"), "el título: {s}");
        assert!(s.contains("Las novedades de Ejemplo."), "la meta description: {s}");
        assert!(s.contains("Últimas entradas"), "el h1: {s}");
        assert!(s.contains("500"), "el recuento de palabras: {s}");
        assert!(s.contains("self"), "el canonical a sí misma se dice, no se repite: {s}");
        // Hallazgos de la URL, con severidad; el hallazgo de sitio (url_id NULL) no es suyo.
        assert!(s.contains("META-DESC-MISSING"), "{s}");
        assert!(s.contains("high"), "{s}");
        assert!(s.contains("×2"), "dos filas de la misma regla se agrupan: {s}");
        assert!(!s.contains("SITEMAP-MISSING"), "lo de sitio no se cuelga de una URL: {s}");
    }

    #[test]
    fn una_pagina_no_indexable_dice_su_motivo() {
        let store = store_de_prueba("noindex");
        let s = ficha(&store, "https://ejemplo.es/");
        assert!(s.contains("noindex"), "el motivo es el valor de la columna: {s}");
    }

    #[test]
    fn los_entrantes_dicen_quien_ancla_como_y_desde_donde() {
        let store = store_de_prueba("entrantes");
        let s = ficha(&store, "https://ejemplo.es/blog/");

        // El total cuenta enlaces (33: nav + main + 31 de pie), no páginas.
        assert!(s.contains("Inlinks (33)"), "{s}");
        // El desglose por región y los nofollow, sin cortar nada.
        assert!(s.contains("footer 31"), "{s}");
        assert!(s.contains("nav 1"), "{s}");
        assert!(s.contains("1 nofollow"), "{s}");
        // Quién enlaza, con su ancla y su región.
        assert!(s.contains("\"Vuelve al blog\""), "{s}");
        assert!(s.contains("https://ejemplo.es/contacto/"), "{s}");
        // El nofollow se marca en su línea.
        assert!(s.contains("[nofollow]"), "{s}");
        // La página que repite el enlace sale una vez, con su multiplicador.
        assert!(s.contains("×2"), "{s}");
    }

    #[test]
    fn el_enlace_de_contenido_va_antes_que_el_de_plantilla() {
        let store = store_de_prueba("orden");
        let s = ficha(&store, "https://ejemplo.es/blog/");
        let main = s.find("\"Vuelve al blog\"").expect("el enlace de contenido está");
        let nav = s.find("\"Blog\"").expect("el de plantilla también");
        assert!(main < nav, "el de contenido se lista primero: {s}");
    }

    #[test]
    fn los_entrantes_se_cortan_al_limite_y_dicen_como_verlos_todos() {
        let store = store_de_prueba("corte");
        let s = render_card(&store, "https://ejemplo.es/blog/", ListLimit::N(5), "terminal", Lang::En)
            .expect("la ficha debe generarse");

        // Se cuenta dentro de la sección de entrantes: otras secciones también listan URLs.
        let inicio = s.find("Inlinks").expect("hay sección de entrantes");
        let fin = s.find("Outlinks").expect("hay sección de salientes");
        let seccion = &s[inicio..fin];
        let lineas_con_url = seccion.lines().filter(|l| l.contains("— https://")).count();
        assert_eq!(lineas_con_url, 5, "se listan exactamente las pedidas: {seccion}");
        // El corte dice el comando exacto, con el fichero y la URL entre comillas.
        assert!(s.contains("--limit all"), "{s}");
        assert!(s.contains("crawlforge inspect"), "{s}");
        assert!(s.contains("'https://ejemplo.es/blog/'"), "{s}");
        // Y el total no se pierde por cortar la lista.
        assert!(s.contains("Inlinks (33)"), "{s}");
    }

    #[test]
    fn limit_all_lista_todas_las_paginas_sin_corte() {
        let store = store_de_prueba("todo");
        let s = render_card(&store, "https://ejemplo.es/blog/", ListLimit::All, "terminal", Lang::En)
            .expect("la ficha debe generarse");
        assert!(s.contains("/archivo/p29/"), "la última página del archivo sale: {s}");
        assert!(!s.contains("--limit all"), "sin corte no hay línea de corte: {s}");
    }

    #[test]
    fn una_url_sin_entrantes_lo_dice_con_palabras() {
        let store = store_de_prueba("sin-entrantes");
        let s = ficha(&store, "https://ejemplo.es/logo.png");
        assert!(s.contains("Inlinks (0)"), "{s}");
        assert!(s.contains("No crawled page links to this URL."), "{s}");
    }

    // ── Identificación de la URL ─────────────────────────────────────────────

    #[test]
    fn la_url_se_encuentra_sin_barra_final_sin_esquema_y_por_ruta() {
        let store = store_de_prueba("variantes");
        for entrada in [
            "https://ejemplo.es/blog",      // sin la barra final
            "http://ejemplo.es/blog/",      // esquema equivocado
            "ejemplo.es/blog/",             // sin esquema
            "/blog/",                       // solo la ruta
            "/blog",                        // la ruta sin barra
        ] {
            let s = ficha(&store, entrada);
            assert!(
                s.contains("El blog de Ejemplo"),
                "'{entrada}' debe resolver a la ficha del blog: {s}"
            );
        }
    }

    #[test]
    fn una_url_que_no_esta_sugiere_las_parecidas() {
        let store = store_de_prueba("sugerencias");
        let err = render_card(
            &store,
            "https://ejemplo.es/bolg/contacto",
            ListLimit::N(20),
            "terminal",
            Lang::En,
        )
        .expect_err("una URL que no está es un error");
        let msg = format!("{err:#}");
        assert!(msg.contains("is not in this crawl"), "{msg}");
        assert!(
            msg.contains("https://ejemplo.es/contacto"),
            "sugiere lo más parecido en vez de solo negar: {msg}"
        );
    }

    #[test]
    fn una_entrada_sin_nada_parecido_no_inventa_sugerencias() {
        let store = store_de_prueba("sin-parecidas");
        let err = render_card(&store, "zzz-que-no-existe", ListLimit::N(20), "terminal", Lang::En)
            .expect_err("no está");
        let msg = format!("{err:#}");
        assert!(msg.contains("is not in this crawl"), "{msg}");
        assert!(!msg.contains("closest"), "sin parecidas no hay lista vacía: {msg}");
    }

    // ── Redirecciones, salientes, imágenes y estados ─────────────────────────

    #[test]
    fn la_cadena_de_redirecciones_se_ensena_entera() {
        let store = store_de_prueba("redirect");
        let s = ficha(&store, "https://ejemplo.es/contacto");
        assert!(s.contains("Redirect chain"), "{s}");
        assert!(s.contains("301"), "{s}");
        let p301 = s.find("301").expect("301");
        let destino = s.rfind("https://ejemplo.es/contacto/").expect("el destino sale");
        assert!(p301 < destino, "el salto antes que el destino: {s}");
    }

    #[test]
    fn un_bucle_de_redirecciones_se_corta_y_se_dice() {
        // Un fichero fabricado puede traer a→b→a: sin guarda, la ficha no termina nunca.
        let store = store_de_prueba("bucle");
        {
            let conn = rusqlite::Connection::open(&store).expect("abrir para viciar");
            conn.execute("UPDATE urls SET redirect_to = 3 WHERE id = 4", [])
                .expect("cerrar el bucle");
            conn.execute("UPDATE urls SET status_code = 301 WHERE id = 4", [])
                .expect("que parezca redirección");
        }
        let s = ficha(&store, "https://ejemplo.es/contacto");
        assert!(s.contains("redirect loop"), "el bucle se nombra en vez de colgarse: {s}");
    }

    #[test]
    fn los_salientes_separan_internos_de_externos_y_ensenan_el_estado_del_destino() {
        let store = store_de_prueba("salientes");
        let s = ficha(&store, "https://ejemplo.es/");
        assert!(s.contains("Outlinks (3: 2 internal, 1 external)"), "{s}");
        assert!(s.contains("https://externo.com/"), "{s}");
        // El externo era nofollow y sin rastrear: marca y estado sin código.
        assert!(s.contains("[nofollow]"), "{s}");
        // El interno rastreado enseña el código de su destino.
        assert!(s.contains("200  https://ejemplo.es/blog/"), "{s}");
    }

    #[test]
    fn las_imagenes_de_la_pagina_salen_con_su_alt_y_sin_el() {
        let store = store_de_prueba("imagenes");
        let s = ficha(&store, "https://ejemplo.es/blog/");
        assert!(s.contains("Images (2)"), "{s}");
        assert!(s.contains("\"Logotipo de Ejemplo\""), "{s}");
        assert!(s.contains("(no alt)"), "{s}");
        assert!(s.contains("foto.jpg"), "{s}");
    }

    #[test]
    fn una_imagen_dice_en_que_paginas_se_usa() {
        let store = store_de_prueba("uso-imagen");
        let s = ficha(&store, "https://ejemplo.es/logo.png");
        assert!(s.contains("Used as an image"), "{s}");
        assert!(s.contains("embedded 1 times on 1 pages"), "{s}");
        assert!(s.contains("https://ejemplo.es/blog/"), "quién la usa: {s}");
    }

    #[test]
    fn una_url_pendiente_dice_que_nunca_se_pidio() {
        let store = store_de_prueba("pendiente");
        let s = ficha(&store, "https://ejemplo.es/pendiente");
        assert!(s.contains("pending"), "{s}");
        assert!(s.contains("never fetched"), "{s}");
        // Y sus entrantes se enseñan igual: es justo lo que explica por qué se descubrió.
        assert!(s.contains("\"Próximamente\""), "{s}");
    }

    // ── Formatos e idioma ────────────────────────────────────────────────────

    #[test]
    fn el_formato_md_es_markdown_pegable() {
        let store = store_de_prueba("md");
        let s = render_card(&store, "https://ejemplo.es/blog/", ListLimit::N(20), "md", Lang::En)
            .expect("la ficha en md debe generarse");
        assert!(s.starts_with("# https://ejemplo.es/blog/"), "{s}");
        assert!(s.contains("\n## "), "las secciones son encabezados: {s}");
        assert!(s.contains("- **"), "los pares etiqueta-valor son lista: {s}");
        assert!(!s.contains("──"), "sin cajas de terminal dentro del markdown: {s}");
    }

    #[test]
    fn un_formato_desconocido_es_un_error_que_lista_los_validos() {
        let store = store_de_prueba("formato");
        let err = render_card(&store, "https://ejemplo.es/blog/", ListLimit::N(20), "pdf", Lang::En)
            .expect_err("pdf no existe");
        let msg = err.to_string();
        assert!(msg.contains("terminal") && msg.contains("md"), "{msg}");
    }

    #[test]
    fn la_ficha_en_espanol_esta_traducida_y_los_datos_no() {
        let store = store_de_prueba("espanol");
        let s = render_card(
            &store,
            "https://ejemplo.es/blog/",
            ListLimit::N(5),
            "terminal",
            Lang::Es,
        )
        .expect("la ficha debe generarse");
        assert!(s.contains("Enlaces entrantes (33)"), "{s}");
        assert!(s.contains("Hallazgos"), "{s}");
        assert!(s.contains("lista completa:"), "{s}");
        // Lo que es dato no se traduce: URLs, regiones, IDs de regla.
        assert!(s.contains("footer 31"), "{s}");
        assert!(s.contains("META-DESC-MISSING"), "{s}");
    }

    // ── Guardas de no regresión ──────────────────────────────────────────────

    #[test]
    fn los_caracteres_de_control_no_llegan_al_terminal() {
        // El fichero es entrada no confiable (revisión §1.7d): un ancla fabricada con
        // secuencias de escape no debe poder pintar el terminal de quien inspecciona.
        let store = store_de_prueba("control");
        {
            let conn = rusqlite::Connection::open(&store).expect("abrir para viciar");
            conn.execute(
                "UPDATE links SET anchor = 'ancla' || char(27) || '[31mroja' WHERE anchor = 'Vuelve al blog'",
                [],
            )
            .expect("inyectar el escape");
        }
        let s = ficha(&store, "https://ejemplo.es/blog/");
        assert!(!s.contains('\u{1b}'), "ningún escape sobrevive: {s:?}");
        assert!(s.contains("anclaroja") || s.contains("ancla"), "el texto útil queda: {s}");
    }

    #[test]
    fn la_ficha_no_modifica_el_fichero() {
        let store = store_de_prueba("solo-lectura");
        let antes = std::fs::metadata(&store).and_then(|m| m.modified()).expect("mtime");
        let _ = ficha(&store, "https://ejemplo.es/blog/");
        let despues = std::fs::metadata(&store).and_then(|m| m.modified()).expect("mtime");
        assert_eq!(antes, despues, "inspeccionar jamás escribe");
    }

    // ── El flag --limit ──────────────────────────────────────────────────────

    #[test]
    fn el_limite_acepta_un_numero_o_all_y_rechaza_lo_demas() {
        assert_eq!(parse_limit("20"), Ok(ListLimit::N(20)));
        assert_eq!(parse_limit("all"), Ok(ListLimit::All));
        assert_eq!(parse_limit("ALL"), Ok(ListLimit::All));
        for malo in ["0", "-3", "muchos", ""] {
            let err = parse_limit(malo).expect_err("fuera del contrato");
            assert!(err.contains("all"), "el error dice el contrato: {err}");
        }
    }

    #[test]
    fn las_anclas_largas_se_recortan_y_las_vacias_se_nombran() {
        let larga = "palabra ".repeat(30);
        let corta = clean_anchor(&larga);
        assert!(corta.chars().count() <= MAX_ANCHOR_CHARS, "{corta}");
        assert!(corta.ends_with('…'), "{corta}");
        assert_eq!(clean_anchor("  con \n saltos  "), "con saltos");
    }
}
