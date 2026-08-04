//! Presentación de métricas y resumen de un fichero de rastreo.
//!
//! Hay dos audiencias y dos salidas. La normal habla con quien audita un sitio: recuentos,
//! tiempo total y hallazgos. La de `--bench` habla con quien desarrolla el motor: métricas de
//! rendimiento y la comprobación del motor. Mezclarlas fue un error
//! señalado en revisión: un consultor leyendo «el proyecto se detiene» en su auditoría no sabe
//! si le hablan a él.
//!
//! **Toda la salida dirigida a quien audita pasa por el catálogo de cadenas**
//! (`crawlforge_cli::i18n`): el idioma lo deciden `--lang` y `CRAWLFORGE_LANG`, con el inglés
//! como origen. Lo que se queda en inglés a propósito son [`print_metrics`] y [`print_gate`]:
//! salida de desarrollo detrás de `--bench`, con umbrales de una fase interna — el mismo idioma
//! que los logs, y no es una pantalla de producto.

use anyhow::Result;
use crawlforge_cli::i18n::{self, msg};
use crawlforge_core::engine::{CrawlMetrics, CrawlOutcome, TruncationReason};
use crawlforge_rules::Lang;
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

/// Mínimos que el motor debe dar en una máquina de trabajo.
///
/// No son una promesa de rendimiento ni un objetivo de producto: son el suelo por debajo del cual
/// algo va mal —una máquina saturada, un disco lento, una regresión— y conviene mirarlo antes de
/// fiarse de los tiempos. Se revisaron el 2026-07-28, cuando se descubrió que los anteriores
/// medían la velocidad del servidor rastreado y no la del motor.
const THRESHOLD_EFFICIENCY: f64 = 0.85;
const THRESHOLD_RSS_MB: f64 = 200.0;
const THRESHOLD_ELEMENTS_PER_SEC: f64 = 40_000.0;
/// Suelo de páginas por segundo. Fijado desde el caso de uso —auditar un `dist/` de hasta
/// 5.000 páginas en menos de 20 s, que es lo que hace que la comprobación se quede puesta en
/// un pipeline de integración continua— y no desde lo que da el motor.
const THRESHOLD_PAGES_PER_SEC: f64 = 300.0;

/// Resumen breve de un rastreo: lo que le importa a quien audita, sin métricas de motor.
pub fn print_brief(outcome: &CrawlOutcome) {
    let lang = i18n::current_lang();
    let m = &outcome.metrics;
    let mut linea = msg::crawl_finished(
        lang,
        i18n::group_thousands(lang, m.urls_fetched),
        i18n::decimal1(lang, m.elapsed.as_secs_f64()),
    );
    if m.urls_errored > 0 {
        linea.push_str(&msg::crawl_failed_suffix(
            lang,
            i18n::group_thousands(lang, m.urls_errored),
        ));
    }
    println!("{linea}");
    print_truncation(outcome, lang);
}

/// Como [`print_brief`], para una reanudación: las métricas del motor cubren solo el tramo
/// reanudado, así que la primera línea dice «más URLs» y no un total que no es.
pub fn print_brief_resumed(outcome: &CrawlOutcome) {
    let lang = i18n::current_lang();
    let m = &outcome.metrics;
    let mut linea = msg::resume_finished(
        lang,
        i18n::group_thousands(lang, m.urls_fetched),
        i18n::decimal1(lang, m.elapsed.as_secs_f64()),
    );
    if m.urls_errored > 0 {
        linea.push_str(&msg::crawl_failed_suffix(
            lang,
            i18n::group_thousands(lang, m.urls_errored),
        ));
    }
    println!("{linea}");
    print_truncation(outcome, lang);
}

fn print_truncation(outcome: &CrawlOutcome, lang: Lang) {
    for linea in truncation_lines(outcome, lang) {
        match linea {
            Some(texto) => println!("  {texto}"),
            None => println!(),
        }
    }
}

/// Las líneas de [`print_truncation`], separadas para poder afirmarlas en tests: `None` es
/// una línea en blanco, `Some` va con la sangría del resumen.
fn truncation_lines(outcome: &CrawlOutcome, lang: Lang) -> Vec<Option<String>> {
    let mut out = Vec::new();
    match outcome.truncated {
        // `ListMode` no es un corte: el rastreo auditó su lista entera. Decir «truncado»
        // aquí sería mentir; lo que hay que decir es que las reglas de grafo completo no
        // se evalúan sobre un conjunto que no es el sitio.
        Some(TruncationReason::ListMode) => {
            out.push(None);
            out.push(Some(msg::crawl_list_mode_note(lang)));
        }
        Some(reason) => {
            out.push(None);
            // El motivo (`max_urls`, `max_duration`) es un identificador de configuración y
            // no se traduce: es literalmente el nombre del límite que lo causó.
            out.push(Some(msg::crawl_truncated(lang, reason.as_str())));
        }
        None => {}
    }
    // La comprobación de externas se dice también cuando fue bien: es tiempo del rastreo
    // que el cierre atribuía en silencio a las páginas del sitio, y en un WordPress con
    // cientos de externas lentas era «el rastreo se muere» sin ninguna pista (revisión §4).
    if outcome.metrics.externals_checked > 0 {
        out.push(Some(msg::external_checked_note(
            lang,
            i18n::group_thousands(lang, outcome.metrics.externals_checked),
        )));
    }
    // Alcanzar `max_external` **no** es un truncado del rastreo (no toca `crawl_meta.truncated`
    // ni apaga ninguna regla), pero sí deja enlaces sin comprobar y hay que decir cuántos.
    if outcome.metrics.externals_unchecked > 0 {
        out.push(None);
        out.push(Some(msg::external_unchecked(
            lang,
            i18n::group_thousands(lang, outcome.metrics.externals_unchecked),
        )));
    }
    // Y el tope del registro, que es el otro y deja aún menos rastro: estas externas no tienen
    // fila en el fichero, así que si no se dicen aquí no se sabrán nunca.
    if outcome.metrics.externals_unregistered > 0 {
        out.push(None);
        out.push(Some(msg::external_unregistered(
            lang,
            i18n::group_thousands(lang, outcome.metrics.externals_unregistered),
        )));
    }
    out
}

/// Métricas de motor. **En inglés a propósito**: es salida de desarrollo detrás de `--bench`,
/// no una pantalla de producto (ver la cabecera del módulo).
pub fn print_metrics(outcome: &CrawlOutcome) {
    let m = &outcome.metrics;
    println!("── Metrics ──────────────────────────────────");
    println!("  URLs crawled       {:>12}", m.urls_fetched);
    println!("  URLs discovered    {:>12}", m.urls_discovered);
    println!("  URLs failed        {:>12}", m.urls_errored);
    println!("  URLs excluded      {:>12}", m.urls_excluded);
    println!("  Findings           {:>12}", m.issues_found);
    println!("  Elements written   {:>12}", m.elements_written);
    println!("  Downloaded         {:>12}", human_bytes(m.bytes_downloaded));
    println!("  Total time         {:>12.2} s", m.elapsed.as_secs_f64());
    println!("  Throughput         {:>12.1} URL/s", m.urls_per_second());
    println!("  Pages parsed       {:>12}", m.pages_parsed);
    println!("  Elements/s         {:>12.0}", m.elements_per_second());
    println!("  Pages/s            {:>12.0}", m.pages_per_second());
    println!("  RAM (peak)         {:>12.1} MB", m.peak_rss_mb());

    // El aviso de truncado sí es para quien audita, aunque aparezca en la salida de `--bench`.
    print_truncation(outcome, i18n::current_lang());
}

/// Comprueba que el motor rinde como debe en esta máquina. **En inglés a propósito**, como
/// [`print_metrics`]: son métricas de ingeniería, no el informe de la auditoría.
///
/// Responde a «¿puedo fiarme de estos tiempos?», que es lo que hace falta antes de comparar dos
/// ejecuciones o de meter la herramienta en un pipeline. Un FAIL casi nunca es el motor: es una
/// máquina ocupada, un disco lento o un sitio que responde despacio.
pub fn print_gate(outcome: &CrawlOutcome, filesystem_mode: bool) {
    let m = &outcome.metrics;

    println!();
    println!("── Engine check ─────────────────────────────");

    if filesystem_mode {
        let paginas_s = m.pages_per_second();
        print_check(
            "Elements/s",
            m.elements_per_second() >= THRESHOLD_ELEMENTS_PER_SEC,
            &format!(
                "{:.0} (threshold ≥{THRESHOLD_ELEMENTS_PER_SEC:.0})",
                m.elements_per_second()
            ),
        );
        print_check(
            "Pages/s",
            paginas_s >= THRESHOLD_PAGES_PER_SEC,
            &format!("{paginas_s:.0} (threshold ≥{THRESHOLD_PAGES_PER_SEC:.0})"),
        );
    } else {
        let eficiencia = m.parallelism_efficiency();
        // Por encima de 1,0 la métrica no aplica: hubo trabajo que no pasó por la red.
        if eficiencia > 1.05 {
            println!(
                "  {:<18} n/a  {eficiencia:.2} — not applicable: part of the work bypassed the network",
                "Efficiency"
            );
        } else {
            print_check(
                "Efficiency",
                eficiencia >= THRESHOLD_EFFICIENCY,
                &format!(
                    "{eficiencia:.2} of the theoretical floor (threshold ≥{THRESHOLD_EFFICIENCY:.2})"
                ),
            );
        }
        println!(
            "  {:<18}     loop {:.1} s, floor {:.1} s at mean concurrency {:.1}",
            "",
            m.crawl_loop.as_secs_f64(),
            m.theoretical_floor().as_secs_f64(),
            m.effective_concurrency
        );
        println!(
            "  {:<18}     sitemaps and final pass: {:.1} s ({:.0}% of total)",
            "",
            m.setup_and_teardown.as_secs_f64(),
            100.0 * m.setup_and_teardown.as_secs_f64() / m.elapsed.as_secs_f64().max(0.001)
        );
    }

    print_check(
        "Memory",
        m.peak_rss_mb() < THRESHOLD_RSS_MB,
        &format!("{:.1} MB (threshold <{THRESHOLD_RSS_MB:.0})", m.peak_rss_mb()),
    );

}

fn print_check(label: &str, ok: bool, detail: &str) {
    println!("  {:<18} {}  {detail}", label, if ok { "PASS" } else { "FAIL" });
}

/// Abre el resumen de un fichero con lo que el propio fichero dice de su completitud:
/// truncado, modo lista, externas apagadas o sin comprobar. Es la mitad `report` del aviso
/// que el `crawl` ya daba y que se perdía en cuanto el fichero viajaba (revisión §1).
pub fn print_store_notes(store: &Path, lang: Lang) -> Result<()> {
    let conn = Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    let notes = crawlforge_cli::audit_report::store_notes(&conn)?;
    for linea in store_note_lines(&notes, lang) {
        println!("{linea}");
    }
    Ok(())
}

/// Las líneas de [`print_store_notes`], separadas para poder afirmarlas en tests.
///
/// Reutiliza las cadenas del `crawl` a propósito: el aviso del modo lista ya existía allí y
/// era bueno; dos textos distintos para el mismo hecho acabarían diciendo cosas distintas.
fn store_note_lines(
    notes: &crawlforge_cli::audit_report::StoreNotes,
    lang: Lang,
) -> Vec<String> {
    let mut out = Vec::new();
    if notes.truncated {
        match notes.truncated_reason.as_deref() {
            Some("list_mode") => out.push(msg::crawl_list_mode_note(lang)),
            reason => out.push(msg::crawl_truncated(lang, reason.unwrap_or("?"))),
        }
    }
    if let Some(nota) = notes.silenced_rules_note(lang) {
        out.push(nota);
    }
    if let Some(nota) = notes.external_note(lang) {
        out.push(nota);
    }
    out
}

/// Cómo respondieron (o no) las URLs de un rastreo.
///
/// Separa dos cosas que la salida mezclaba bajo «sin respuesta» y que no se parecen en nada:
/// una petición que **falló** (conexión rechazada, DNS, timeout) y una URL descubierta que
/// **nunca se pidió** porque el rastreo se detuvo antes (`crawl_state = 'pending'`). En un
/// rastreo truncado las segundas se contaban como si hubieran fallado, y 82 URLs «sin
/// respuesta» acusaban a un servidor al que no se había preguntado.
struct ResponseBreakdown {
    /// Recuento por familia de código (`2xx`…`5xx`) de URLs que respondieron, con las
    /// internas y las externas **por separado**: un 404 ajeno no es un error del sitio
    /// auditado, y sumarlos hacía que «4xx 14» no cuadrara con ningún otro número del
    /// informe. Cada tupla es `(familia, internas, externas)`.
    groups: Vec<(String, i64, i64)>,
    /// Peticiones hechas que no obtuvieron código de estado (`crawl_state = 'error'`).
    no_response: i64,
    /// URLs descubiertas y nunca pedidas (`crawl_state = 'pending'`).
    never_requested: i64,
    /// Internas `skipped` sin motivo de exclusión ni respuesta: fuera del alcance del
    /// rastreo. Es lo que en modo lista explica «URLs 21» cuando la lista traía 5.
    out_of_scope: i64,
}

fn response_breakdown(conn: &Connection) -> Result<ResponseBreakdown> {
    let mut stmt = conn.prepare(
        "SELECT CASE
                    WHEN status_code < 300 THEN '2xx'
                    WHEN status_code < 400 THEN '3xx'
                    WHEN status_code < 500 THEN '4xx'
                    ELSE '5xx' END AS grupo,
                COUNT(*) FILTER (WHERE is_internal = 1),
                COUNT(*) FILTER (WHERE is_internal = 0)
         FROM urls WHERE status_code IS NOT NULL
         GROUP BY grupo ORDER BY grupo",
    )?;
    let groups: Vec<(String, i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let no_response: i64 = conn.query_row(
        "SELECT COUNT(*) FROM urls WHERE status_code IS NULL AND crawl_state = 'error'",
        [],
        |r| r.get(0),
    )?;
    let never_requested: i64 = conn.query_row(
        "SELECT COUNT(*) FROM urls WHERE crawl_state = 'pending'",
        [],
        |r| r.get(0),
    )?;
    let out_of_scope: i64 = conn.query_row(
        "SELECT COUNT(*) FROM urls
         WHERE is_internal = 1 AND crawl_state = 'skipped'
           AND status_code IS NULL AND exclusion_reason IS NULL AND error_kind IS NULL",
        [],
        |r| r.get(0),
    )?;

    Ok(ResponseBreakdown { groups, no_response, never_requested, out_of_scope })
}

/// Resume el contenido de un fichero de rastreo, en el idioma pedido.
///
/// Lo que se ve son etiquetas del catálogo y números con el millar del idioma; lo que **no** se
/// traduce son los valores que vienen de la base de datos (`noindex`, `robots`, familias `4xx`)
/// y los IDs de regla: son identificadores, y además tokens de `--fail-on` y de filtros.
pub fn print_summary(store: &Path, lang: Lang) -> Result<()> {
    let conn = Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    let n = |v: i64| i18n::count(lang, v);

    println!();
    println!("{}", i18n::section(&msg::results_title(lang)));

    let total: i64 = conn.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))?;
    let internal: i64 =
        conn.query_row("SELECT COUNT(*) FROM urls WHERE is_internal = 1", [], |r| r.get(0))?;
    let indexable: i64 =
        conn.query_row("SELECT COUNT(*) FROM pages WHERE is_indexable = 1", [], |r| r.get(0))?;
    let pages: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?;
    let links: i64 = conn.query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))?;

    println!(
        "  {:<19}{:>12}  ({})",
        msg::label_urls(lang),
        n(total),
        msg::note_internal(lang, n(internal))
    );
    println!(
        "  {:<19}{:>12}  ({})",
        msg::label_html_pages(lang),
        n(pages),
        msg::note_indexable(lang, n(indexable))
    );
    println!("  {:<19}{:>12}", msg::label_links(lang), n(links));

    // Distribución de respuestas: códigos de estado, fallos y lo nunca pedido, por separado.
    let breakdown = response_breakdown(&conn)?;
    if !breakdown.groups.is_empty() || breakdown.no_response > 0 {
        println!();
        println!("  {}", msg::heading_status_codes(lang));
        for (grupo, internas, externas) in &breakdown.groups {
            // Las externas comprobadas se etiquetan aparte: un 404 ajeno no es un error del
            // sitio auditado, y mezclado hacía que este recuento no cuadrara con ningún otro.
            if *externas > 0 {
                println!(
                    "    {grupo:<16} {:>10}  ({})",
                    n(*internas),
                    msg::note_external_status(lang, *externas)
                );
            } else {
                println!("    {grupo:<16} {:>10}", n(*internas));
            }
        }
        if breakdown.no_response > 0 {
            println!(
                "    {:<16} {:>10}  ({})",
                msg::label_no_response(lang),
                n(breakdown.no_response),
                msg::note_request_failed(lang)
            );
        }
    }
    if breakdown.never_requested > 0 {
        // Fuera del cajón de los códigos de estado: a estas URLs nunca se les preguntó.
        println!();
        println!(
            "  {:<19}{:>12}  {}",
            msg::label_never_requested(lang),
            n(breakdown.never_requested),
            msg::note_never_requested(lang)
        );
    }
    if breakdown.out_of_scope > 0 {
        // También sin preguntar, pero por diseño y no por un corte: los enlaces que en modo
        // lista apuntan fuera de la lista. Sin esta línea, «URLs 21» con una lista de 5 es
        // un misterio que el usuario resuelve con SQL.
        println!();
        println!(
            "  {:<19}{:>12}  {}",
            msg::label_out_of_scope(lang),
            n(breakdown.out_of_scope),
            msg::note_out_of_scope(lang)
        );
    }

    // Motivos de no indexabilidad: la consulta más frecuente que hace un SEO.
    let mut stmt = conn.prepare(
        "SELECT indexability_reason, COUNT(*) FROM pages
         WHERE is_indexable = 0 AND indexability_reason IS NOT NULL
         GROUP BY indexability_reason ORDER BY COUNT(*) DESC",
    )?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if !rows.is_empty() {
        println!();
        println!("  {}", msg::heading_non_indexable(lang));
        for (reason, cuantas) in rows {
            println!("    {reason:<16} {:>10}", n(cuantas));
        }
    }

    // Hallazgos. El ID de regla es identificador; la severidad se traduce porque aquí es prosa
    // de columna, no token.
    println!();
    for linea in findings_lines(&conn, store, lang, pages)? {
        println!("{linea}");
    }

    // Excluidas: no se ocultan, saber qué quedó fuera es un hallazgo en sí mismo.
    let mut stmt = conn.prepare(
        "SELECT exclusion_reason, COUNT(*) FROM urls
         WHERE crawl_state = 'excluded' AND exclusion_reason IS NOT NULL
         GROUP BY exclusion_reason",
    )?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if !rows.is_empty() {
        println!();
        println!("  {}", msg::heading_excluded(lang));
        for (reason, cuantas) in rows {
            println!("    {reason:<16} {:>10}", n(cuantas));
        }
    }

    Ok(())
}

/// Un grupo de hallazgos con el mismo `group_key`, candidato a colapso de plantilla.
struct GroupCount {
    rule_id: String,
    severity: String,
    group_key: String,
    n: i64,
}

/// Los grupos por `(rule_id, severity, group_key)` de los hallazgos **de página**.
///
/// Los hallazgos de sitio (`url_id IS NULL`) quedan fuera a propósito: ya son un solo hallazgo
/// agregado —`INDEX-SECTION-DISCONNECTED` cuenta sus páginas dentro de su `detail_json`— y
/// colapsarlos otra vez diría «páginas» de cosas que no lo son (un sitemap roto, por ejemplo).
fn group_counts(conn: &Connection) -> Result<Vec<GroupCount>> {
    let mut stmt = conn.prepare(
        "SELECT rule_id, severity, group_key, COUNT(*) AS n
         FROM issues
         WHERE url_id IS NOT NULL AND group_key IS NOT NULL
         GROUP BY rule_id, severity, group_key
         ORDER BY n DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(GroupCount {
                rule_id: r.get(0)?,
                severity: r.get(1)?,
                group_key: r.get(2)?,
                n: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Hallazgos de una regla fuera de sus grupos de plantilla. Son hallazgos y no páginas: la
/// misma página puede estar dentro de un grupo (el logo de la plantilla) y traer además un
/// hallazgo propio (su imagen destacada).
///
/// Los `group_key` son valores del propio fichero, pero van como parámetros igualmente: el
/// fichero es entrada no confiable y aquí no se interpola nada.
fn findings_outside_groups(
    conn: &Connection,
    rule_id: &str,
    severity: &str,
    template_keys: &[&str],
) -> Result<i64> {
    let placeholders =
        (0..template_keys.len()).map(|i| format!("?{}", i + 3)).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT COUNT(*) FROM issues i
         WHERE i.rule_id = ?1 AND i.severity = ?2
           AND (i.url_id IS NULL OR i.group_key IS NULL
                OR i.group_key NOT IN ({placeholders}))"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&rule_id, &severity];
    for key in template_keys {
        params.push(key);
    }
    let n = conn.query_row(&sql, params.as_slice(), |r| r.get(0))?;
    Ok(n)
}

/// Páginas **distintas** cubiertas por los grupos de plantilla de una regla. Distintas porque
/// los grupos pueden solaparse: cada banner del pie forma su grupo y todos viven en las mismas
/// páginas.
fn pages_in_groups(
    conn: &Connection,
    rule_id: &str,
    severity: &str,
    template_keys: &[&str],
) -> Result<i64> {
    let placeholders =
        (0..template_keys.len()).map(|i| format!("?{}", i + 3)).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT COUNT(DISTINCT i.url_id) FROM issues i
         WHERE i.rule_id = ?1 AND i.severity = ?2 AND i.group_key IN ({placeholders})"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&rule_id, &severity];
    for key in template_keys {
        params.push(key);
    }
    let n = conn.query_row(&sql, params.as_slice(), |r| r.get(0))?;
    Ok(n)
}

/// URLs de ejemplo de un grupo, para poder pinchar sin abrir el XLSX.
fn group_examples(conn: &Connection, rule_id: &str, group_key: &str, limit: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
         WHERE i.rule_id = ?1 AND i.group_key = ?2 ORDER BY u.url LIMIT ?3",
    )?;
    let urls = stmt
        .query_map(rusqlite::params![rule_id, group_key, limit], |r| r.get::<_, String>(0))?
        .filter_map(std::result::Result::ok)
        .map(|u| crawlforge_cli::audit_report::strip_control_chars(&u))
        .collect();
    Ok(urls)
}

/// El bloque de hallazgos del resumen, ya maquetado, una cadena por línea.
///
/// Separado de [`print_summary`] para poder afirmar en un test qué dice exactamente: es la
/// pantalla donde un defecto de plantilla con 18.089 filas tiene que leerse como **un** problema
/// («1 template issue (18,089 pages)») y no como un recuento que tapa todo lo demás. El colapso
/// es de presentación: las filas de `issues` están todas en el fichero, y el criterio de qué es
/// plantilla vive en `crawlforge_rules::is_template_group`, compartido con las apps.
fn findings_lines(
    conn: &Connection,
    store: &Path,
    lang: Lang,
    total_pages: i64,
) -> Result<Vec<String>> {
    let n = |v: i64| i18n::count(lang, v);

    let mut stmt = conn.prepare(
        "SELECT rule_id, severity, n FROM v_issue_summary ORDER BY
             CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2
                           WHEN 'low' THEN 3 ELSE 4 END, n DESC",
    )?;
    let rows: Vec<(String, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    if rows.is_empty() {
        out.push(format!("  {}", msg::no_findings(lang)));
        return Ok(out);
    }

    let groups = group_counts(conn)?;
    let affected = crawlforge_cli::audit_report::affected_pages_by_rule(conn)?;
    // La forma del problema de profundidad se lee una vez; `None` en los ficheros anteriores
    // al `click_depth`, que caen a la reformulación genérica por porcentaje.
    let deep = crawlforge_rules::deep_page_shape(conn)?;
    out.push(format!("  {}", msg::heading_findings(lang)));
    for (rule, severity, total) in rows {
        let template: Vec<&GroupCount> = groups
            .iter()
            .filter(|g| {
                g.rule_id == rule
                    && g.severity == severity
                    && crawlforge_rules::is_template_group(g.n, total_pages)
            })
            .collect();

        if template.is_empty() {
            // El segundo colapso, para lo que la plantilla no cubre: hallazgos masivos ciertos
            // y sin causa común hashable. El recuento se conserva —nada se oculta, también en
            // `critical`— y se le añade la cuota del sitio; para `INDEX-DEEP-PAGE`, además, la
            // forma (banda típica y máxima), que es lo que dice «el archivo no tiene atajos».
            let pages = affected.get(&(rule.clone(), severity.clone())).copied().unwrap_or(0);
            if crawlforge_rules::is_pervasive(pages, total_pages) {
                let pct = crawlforge_cli::audit_report::share_pct(pages, total_pages);
                if rule == "INDEX-DEEP-PAGE" {
                    if let Some(f) = deep {
                        out.push(format!(
                            "    {:<9} {rule:<24} {}",
                            i18n::severity_word(lang, &severity),
                            msg::deep_pages_summary(
                                lang,
                                n(f.pages),
                                f.max_click_depth,
                                pct,
                                f.typical_min,
                                f.typical_max,
                                f.deepest
                            )
                        ));
                        continue;
                    }
                }
                out.push(format!(
                    "    {:<9} {rule:<24} {:>8}  ({})",
                    i18n::severity_word(lang, &severity),
                    n(total),
                    msg::pervasive_note(lang, pct)
                ));
                continue;
            }
            out.push(format!(
                "    {:<9} {rule:<24} {:>8}",
                i18n::severity_word(lang, &severity),
                n(total)
            ));
            continue;
        }

        // El resto son «hallazgos», no «páginas»: la misma página puede estar en el grupo de
        // plantilla (el logo) y traer además un hallazgo propio (su imagen destacada).
        let claves: Vec<&str> = template.iter().map(|g| g.group_key.as_str()).collect();
        let rest = findings_outside_groups(conn, &rule, &severity, &claves)?;
        // Y las páginas colapsadas se cuentan **distintas**: los 13 banners del pie de un
        // rastreo real están cada uno en ~540 páginas, pero son las mismas 567 páginas trece
        // veces, no 6.998.
        let collapsed = pages_in_groups(conn, &rule, &severity, &claves)?;
        let mut texto = if template.len() == 1 {
            msg::one_template_issue(lang, n(collapsed))
        } else {
            msg::n_template_issues(lang, template.len(), n(collapsed))
        };
        if rest > 0 {
            texto.push_str(&msg::plus_more_findings(lang, n(rest)));
        }
        out.push(format!("    {:<9} {rule:<24} {texto}", i18n::severity_word(lang, &severity)));

        // Dos URLs del grupo mayor: con ellas se ve la plantilla sin abrir nada más. `groups`
        // viene ordenado por tamaño, así que el primero es el mayor.
        if let Some(mayor) = template.first() {
            let ejemplos = group_examples(conn, &rule, &mayor.group_key, 2)?;
            if !ejemplos.is_empty() {
                out.push(format!(
                    "              {}",
                    msg::example_urls(lang, ejemplos.join(" · "))
                ));
            }
        }
    }

    // El resumen da el titular; el detalle lo da un comando, y se dice cuál.
    out.push(String::new());
    out.push(format!(
        "  {}",
        msg::hint_full_lists(
            lang,
            format!("crawlforge report {} --rule <RULE-ID>", store.display())
        )
    ));
    Ok(out)
}

/// `crawlforge report <store> --rule <RULE-ID>`: **todas** las URLs afectadas por una regla.
///
/// Es la otra mitad del colapso de plantilla: el informe corta y este comando lista. Sin él, la
/// única vía para ver las URLs que el corte esconde era abrir el XLSX o hacer SQL a mano
/// (revisión de UX §5.1), que es exactamente lo que obligaba a volver a Screaming Frog.
pub fn print_rule_urls(store: &Path, rule: &str, lang: Lang) -> Result<()> {
    let conn = Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    print!("{}", rule_urls_text(&conn, store, rule, lang)?);
    Ok(())
}

/// La lógica de [`print_rule_urls`], separada para poder afirmar la salida en tests.
fn rule_urls_text(conn: &Connection, store: &Path, rule: &str, lang: Lang) -> Result<String> {
    use std::fmt::Write;
    let strip = crawlforge_cli::audit_report::strip_control_chars;

    // El ID se normaliza a mayúsculas: `http-404-internal` es una errata de tecleo, no otra
    // regla. Se acepta si está en el catálogo o si el fichero lo trae (un rastreo hecho por
    // otra versión puede conocer reglas que esta no).
    let rule = rule.trim().to_ascii_uppercase();
    let meta = crawlforge_rules::catalog().into_iter().find(|m| m.id == rule);
    let total: i64 =
        conn.query_row("SELECT COUNT(*) FROM issues WHERE rule_id = ?1", [&rule], |r| r.get(0))?;
    if meta.is_none() && total == 0 {
        anyhow::bail!(msg::error_unknown_rule(lang, &rule));
    }

    let mut s = String::new();
    match meta {
        Some(m) => writeln!(s, "{rule} — {}", m.name(lang))?,
        None => writeln!(s, "{rule}")?,
    }
    if total == 0 {
        writeln!(s, "{}", msg::rule_no_findings(lang, &rule))?;
        return Ok(s);
    }
    // URLs distintas, no filas: una regla puede escribir más de una fila por página (una por
    // imagen en ASSET-IMG-EMPTY-ALT-LINK). Los hallazgos de sitio, sin URL, cuentan aparte.
    let distinct_urls: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT url_id) + SUM(url_id IS NULL) FROM issues WHERE rule_id = ?1",
        [&rule],
        |r| r.get(0),
    )?;
    writeln!(s, "{}", msg::rule_affected_urls(lang, i18n::count(lang, distinct_urls)))?;

    // La lista de profundidad se ordena de lo más hundido a lo menos, con los clics delante:
    // el resumen dice «la más profunda a 48» y la primera pregunta del consultor es cuáles son
    // esas. Solo en los ficheros con `click_depth` en el detalle; los anteriores caen al
    // listado genérico de abajo.
    if rule == "INDEX-DEEP-PAGE" && crawlforge_rules::deep_page_shape(conn)?.is_some() {
        writeln!(s, "{}", msg::rule_deep_sorted(lang))?;
        writeln!(s)?;
        let mut stmt = conn.prepare(
            "SELECT u.url, CAST(json_extract(i.detail_json, '$.click_depth') AS INTEGER) AS d
             FROM issues i JOIN urls u ON u.id = i.url_id
             WHERE i.rule_id = ?1
               AND json_extract(i.detail_json, '$.click_depth') IS NOT NULL
             ORDER BY d DESC, u.url",
        )?;
        let filas = stmt
            .query_map([&rule], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for fila in filas {
            let (url, d) = fila?;
            writeln!(s, "  {d:>4}  {}", strip(&url))?;
        }
        // Un fichero fabricado puede mezclar filas con y sin profundidad: las sin ella se
        // listan igualmente, que ninguna URL se quede sin salir.
        let mut stmt = conn.prepare(
            "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
             WHERE i.rule_id = ?1
               AND json_extract(i.detail_json, '$.click_depth') IS NULL
             ORDER BY u.url",
        )?;
        let filas = stmt.query_map([&rule], |r| r.get::<_, String>(0))?;
        for url in filas {
            writeln!(s, "     ?  {}", strip(&url?))?;
        }
        return Ok(s);
    }

    let total_pages: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?;
    let template: Vec<GroupCount> = group_counts(conn)?
        .into_iter()
        .filter(|g| g.rule_id == rule && crawlforge_rules::is_template_group(g.n, total_pages))
        .collect();

    // Los grupos de plantilla van primero, cada uno con su causa —el `detail_json` de una de
    // sus filas— para que el listado diga qué arreglar y no solo dónde aparece.
    let mut stmt = conn.prepare(
        "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
         WHERE i.rule_id = ?1 AND i.group_key = ?2 ORDER BY u.url",
    )?;
    for g in &template {
        writeln!(s)?;
        writeln!(s, "{}", msg::rule_template_group(lang, i18n::count(lang, g.n)))?;
        let detalle: Option<String> = conn
            .query_row(
                "SELECT detail_json FROM issues
                 WHERE rule_id = ?1 AND group_key = ?2 AND detail_json IS NOT NULL LIMIT 1",
                rusqlite::params![&rule, &g.group_key],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(detalle) = detalle {
            // El detalle se humaniza antes de imprimirse: `missing: og:title, og:image` y no
            // el volcado `{"missing":[…]}` dentro de un informe en prosa (revisión §8).
            let mut causa = strip(&humanize_detail(&detalle));
            if causa.chars().count() > 200 {
                causa = causa.chars().take(200).collect::<String>() + "…";
            }
            writeln!(s, "  {}", msg::rule_group_cause(lang, causa))?;
        }
        let urls = stmt
            .query_map(rusqlite::params![&rule, &g.group_key], |r| r.get::<_, String>(0))?
            .filter_map(std::result::Result::ok);
        for url in urls {
            writeln!(s, "  {}", strip(&url))?;
        }
    }

    // El resto: lo que no pertenece a ningún grupo de plantilla. Sin grupos, es la lista entera.
    let claves_plantilla: Vec<&str> = template.iter().map(|g| g.group_key.as_str()).collect();
    let mut resto: Vec<Option<String>> = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT u.url, i.group_key FROM issues i LEFT JOIN urls u ON u.id = i.url_id
         WHERE i.rule_id = ?1 ORDER BY u.url",
    )?;
    let filas = stmt.query_map([&rule], |r| {
        Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?))
    })?;
    for fila in filas {
        let (url, group_key) = fila?;
        let en_plantilla = url.is_some()
            && group_key.as_deref().is_some_and(|k| claves_plantilla.contains(&k));
        if !en_plantilla {
            resto.push(url);
        }
    }
    // La misma página puede traer dos filas fuera de plantilla (dos imágenes distintas);
    // se lista una vez. Viene ordenado por URL, así que basta el dedup consecutivo. Los
    // hallazgos de sitio (sin URL) no se deduplican: cada fila es un hallazgo distinto.
    resto.dedup_by(|a, b| a == b && a.is_some());
    if !resto.is_empty() {
        if !template.is_empty() {
            writeln!(s)?;
            writeln!(s, "{}", msg::rule_other_pages(lang, i18n::count(lang, resto.len() as i64)))?;
        }
        for url in resto {
            match url {
                Some(url) => writeln!(s, "  {}", strip(&url))?,
                None => writeln!(s, "  {}", msg::site_wide_finding(lang))?,
            }
        }
    }

    // El paso siguiente de las reglas HTTP: la URL listada es el destino roto, pero el
    // arreglo vive en la página que lo enlaza, y ese dato lo da `inspect`. Con el comando
    // listo para copiar, como el resto de cortes de la herramienta (revisión §6).
    if rule.starts_with("HTTP-") {
        let ejemplo: Option<String> = conn
            .query_row(
                "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
                 WHERE i.rule_id = ?1 ORDER BY u.url LIMIT 1",
                [&rule],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(url) = ejemplo {
            writeln!(s)?;
            writeln!(s, "{}", msg::hint_who_links(lang))?;
            writeln!(s, "    crawlforge inspect {} '{}'", store.display(), strip(&url))?;
        }
    }
    Ok(s)
}

/// Un `detail_json` en palabras: `{"missing":["og:title"],"present":[]}` →
/// `missing: og:title`. Las claves son identificadores y no se traducen; lo que se elimina
/// es la sintaxis JSON, que en un informe en prosa es ruido. Si el detalle no es un objeto
/// JSON, se devuelve tal cual: enseñar el dato crudo es mejor que ocultarlo.
fn humanize_detail(raw: &str) -> String {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw)
    else {
        return raw.to_string();
    };
    let partes: Vec<String> = map
        .iter()
        .filter_map(|(k, v)| humanize_json_value(v).map(|texto| format!("{k}: {texto}")))
        .collect();
    if partes.is_empty() {
        raw.to_string()
    } else {
        partes.join(" · ")
    }
}

/// El valor de una clave del detalle, en texto plano. `None` para lo que no aporta nada en
/// un titular: nulos y listas vacías (el `"present":[]` de Open Graph).
fn humanize_json_value(v: &serde_json::Value) -> Option<String> {
    use serde_json::Value;
    match v {
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        Value::Array(a) if a.is_empty() => None,
        Value::Array(a) => Some(
            a.iter()
                .map(|x| humanize_json_value(x).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        // Un objeto anidado se deja en JSON: inventarle una prosa sería mentir mejor.
        Value::Object(_) => Some(v.to_string()),
    }
}

/// Decide si un rastreo terminó sin nada que auditar, y con qué mensaje decírselo al usuario.
///
/// El criterio distingue dos situaciones que se parecen y no son lo mismo:
///
/// - **Algunas URLs fallan.** Normal: es justo lo que la herramienta reporta. No es un error
///   del programa y no toca el código de salida.
/// - **Ninguna URL respondió.** No hay nada que auditar. Terminar con éxito y un resumen de
///   aspecto válido es como un dominio mal escrito se convierte en una «auditoría correcta»:
///   fichero vacío, exit 0 y hasta un hallazgo sobre un sitio que nunca respondió.
///
/// Devuelve `Some(mensaje)` cuando el rastreo no obtuvo ni una respuesta; el llamante debe
/// tratarlo como error y salir con código distinto de cero.
pub fn empty_crawl_error(store: &Path, metrics: &CrawlMetrics) -> Result<Option<String>> {
    empty_crawl_error_lang(store, metrics, i18n::current_lang())
}

/// La lógica de [`empty_crawl_error`], con el idioma explícito para poder probar los dos sin
/// depender del entorno del proceso.
fn empty_crawl_error_lang(
    store: &Path,
    metrics: &CrawlMetrics,
    lang: Lang,
) -> Result<Option<String>> {
    if metrics.urls_fetched > 0 {
        return Ok(None);
    }

    if metrics.urls_errored > 0 {
        let conn = Connection::open_with_flags(
            store,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        if let Some((url, kind)) = first_errored_url(&conn)? {
            return Ok(Some(describe_dead_seed(lang, &url, kind.as_deref())));
        }
    }

    if metrics.urls_excluded > 0 {
        // «Excluidas» no siempre quiere decir `robots.txt`: desde que los patrones de
        // include/exclude se aplican de verdad, un patrón demasiado amplio deja el rastreo a
        // cero, y culpar al `robots.txt` del sitio manda al usuario a mirar donde no es.
        // El motivo está en el almacén, así que se consulta en vez de suponerlo.
        let conn = Connection::open_with_flags(
            store,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        let por_patron: i64 = conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE exclusion_reason = 'pattern'",
            [],
            |r| r.get(0),
        )?;
        let por_robots: i64 = conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE exclusion_reason = 'robots'",
            [],
            |r| r.get(0),
        )?;
        if por_patron > por_robots {
            return Ok(Some(msg::error_all_excluded_by_pattern(lang)));
        }
        return Ok(Some(msg::error_all_blocked_by_robots(lang)));
    }

    Ok(Some(msg::error_no_urls_fetched(lang)))
}

/// La primera URL que falló, en orden de profundidad: con `urls_fetched == 0`, la de menor
/// profundidad es la semilla o lo más cercano a ella que se intentó.
fn first_errored_url(conn: &Connection) -> Result<Option<(String, Option<String>)>> {
    let mut stmt = conn.prepare(
        "SELECT url, error_kind FROM urls
         WHERE crawl_state = 'error'
         ORDER BY depth ASC LIMIT 1",
    )?;
    let row = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .next()
        .transpose()?;
    Ok(row)
}

/// Mensaje de error para una semilla que no respondió, con la causa en palabras y no en jerga.
fn describe_dead_seed(lang: Lang, url: &str, kind: Option<&str>) -> String {
    let causa = match kind {
        Some("dns") => msg::cause_dns(lang),
        Some("tls") => msg::cause_tls(lang),
        Some("timeout") => msg::cause_timeout(lang),
        Some("connection") => msg::cause_connection(lang),
        _ => msg::cause_no_response(lang),
    };
    msg::error_dead_seed(lang, url, causa)
}

/// Comprueba si el `--base` de una auditoría contradice a los canonicals del propio sitio.
///
/// Si la gran mayoría de los canonicals apuntan a un host distinto del de `--base`, lo más
/// probable no es que el sitio canonice a otro dominio: es que el flag está mal, y entonces
/// toda la auditoría de indexabilidad es un artefacto (todas las páginas salen
/// `canonicalised`). Se avisa en vez de fallar porque el caso legítimo existe —un `dist/`
/// que de verdad canoniza a otro dominio— y el aviso lo deja claro sin estorbar.
pub fn check_base_mismatch(store: &Path, base: &str) -> Result<Option<String>> {
    let Some(base_host) = url::Url::parse(base).ok().and_then(|u| u.host_str().map(String::from))
    else {
        return Ok(None);
    };

    let conn = Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    let mut stmt =
        conn.prepare("SELECT canonical FROM pages WHERE canonical IS NOT NULL AND canonical != ''")?;
    let canonicals = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;

    Ok(detect_base_mismatch(
        i18n::current_lang(),
        &base_host,
        base,
        canonicals.iter().map(String::as_str),
    ))
}

/// Umbral del aviso: al menos este porcentaje de los canonicals debe apuntar al mismo host
/// ajeno. Por debajo, lo raro son unas páginas concretas y eso ya lo dice `CANON-CROSS-DOMAIN`.
const BASE_MISMATCH_THRESHOLD_PCT: usize = 80;

/// La parte pura de [`check_base_mismatch`], separada para poder testearla sin fichero.
fn detect_base_mismatch<'a>(
    lang: Lang,
    base_host: &str,
    base: &str,
    canonicals: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let mut total = 0usize;
    let mut origins: HashMap<String, usize> = HashMap::new();
    for c in canonicals {
        let Ok(u) = url::Url::parse(c) else { continue };
        let Some(host) = u.host_str() else { continue };
        total += 1;
        if host != base_host {
            *origins.entry(format!("{}://{host}/", u.scheme())).or_insert(0) += 1;
        }
    }

    // Con una sola página no hay mayoría que valga.
    if total < 2 {
        return None;
    }
    let (origin, cuantos) = origins.into_iter().max_by_key(|(_, n)| *n)?;
    if cuantos * 100 < total * BASE_MISMATCH_THRESHOLD_PCT {
        return None;
    }
    Some(msg::warn_base_mismatch(lang, cuantos, total, origin, base))
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatea_tamanos_legibles() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    // ── El criterio de «la semilla no respondió» ─────────────────────────────

    fn metrics(fetched: u64, errored: u64, excluded: u64) -> CrawlMetrics {
        CrawlMetrics {
            urls_fetched: fetched,
            urls_errored: errored,
            urls_excluded: excluded,
            ..Default::default()
        }
    }

    /// Un fichero con solo la tabla `urls`: es lo único que consulta el criterio.
    fn store_with_errored_seed(dir: &std::path::Path, kind: &str) -> std::path::PathBuf {
        let path = dir.join("caso.sqlite");
        let conn = Connection::open(&path).expect("create the file");
        conn.execute_batch(
            "CREATE TABLE urls (url TEXT, error_kind TEXT, crawl_state TEXT, depth INTEGER);",
        )
        .expect("create the table");
        conn.execute(
            "INSERT INTO urls VALUES ('http://127.0.0.1:9/', ?1, 'error', 0)",
            [kind],
        )
        .expect("insert the seed");
        path
    }

    #[test]
    fn un_rastreo_con_respuestas_no_es_un_fallo_aunque_haya_errores() {
        // URLs con error dentro de un rastreo con respuestas: eso es lo que la herramienta
        // reporta, no un fallo del programa.
        let tmp = std::env::temp_dir();
        let ok = empty_crawl_error_lang(&tmp.join("no-se-abre.sqlite"), &metrics(120, 30, 0), Lang::En)
            .expect("must not fail");
        assert!(ok.is_none(), "with answered URLs there is never an empty-crawl error");
    }

    #[test]
    fn una_semilla_que_no_responde_es_un_error_con_causa() {
        let dir = std::env::temp_dir().join(format!("cf-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create the directory");
        let store = store_with_errored_seed(&dir, "connection");

        let msg = empty_crawl_error_lang(&store, &metrics(0, 1, 0), Lang::En)
            .expect("the query must work")
            .expect("with no responses there must be an error message");
        assert!(msg.contains("http://127.0.0.1:9/"), "it must name the URL: {msg}");
        assert!(msg.contains("connection refused"), "the cause in words: {msg}");

        // Y en español la misma causa, en español.
        let es = empty_crawl_error_lang(&store, &metrics(0, 1, 0), Lang::Es)
            .expect("the query must work")
            .expect("message in Spanish");
        assert!(es.contains("conexión rechazada"), "{es}");
        assert!(es.contains("http://127.0.0.1:9/"), "the URL is not translated: {es}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn las_causas_de_fallo_se_dicen_en_palabras() {
        let url = "https://ejemplo.es/";
        assert!(describe_dead_seed(Lang::En, url, Some("dns")).contains("does not resolve"));
        assert!(describe_dead_seed(Lang::En, url, Some("tls")).contains("certificate"));
        assert!(describe_dead_seed(Lang::En, url, Some("timeout"))
            .contains("did not answer in time"));
        assert!(describe_dead_seed(Lang::En, url, None).contains("no response"));

        assert!(describe_dead_seed(Lang::Es, url, Some("dns")).contains("no existe o no resuelve"));
        assert!(describe_dead_seed(Lang::Es, url, Some("tls")).contains("certificado"));
        assert!(describe_dead_seed(Lang::Es, url, Some("timeout")).contains("no respondió a tiempo"));
        assert!(describe_dead_seed(Lang::Es, url, None).contains("sin respuesta"));
    }

    /// Fichero de rastreo con `n` URLs excluidas por el motivo que se pida.
    fn store_con_exclusiones(nombre: &str, motivo: &str, n: usize) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-excl-{}-{nombre}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).expect("create");
        conn.execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, crawl_state TEXT,
                                exclusion_reason TEXT);",
        )
        .expect("minimal schema");
        for i in 0..n {
            conn.execute(
                "INSERT INTO urls (url, crawl_state, exclusion_reason)
                 VALUES (?1, 'excluded', ?2)",
                rusqlite::params![format!("https://ejemplo.es/{i}"), motivo],
            )
            .expect("insert");
        }
        path
    }

    #[test]
    fn todo_bloqueado_por_robots_tambien_es_un_rastreo_vacio() {
        let path = store_con_exclusiones("robots", "robots", 40);
        let msg = empty_crawl_error_lang(&path, &metrics(0, 0, 40), Lang::En)
            .expect("query")
            .expect("must warn");
        assert!(msg.contains("robots.txt"), "{msg}");

        let es = empty_crawl_error_lang(&path, &metrics(0, 0, 40), Lang::Es)
            .expect("query")
            .expect("must warn");
        assert!(es.contains("robots.txt") && es.contains("--ignore-robots"), "{es}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn si_lo_que_vacio_el_rastreo_fue_un_patron_no_se_culpa_al_robots() {
        // Desde que los patrones de include/exclude se aplican de verdad, un patrón demasiado
        // amplio deja el rastreo a cero. Culpar al `robots.txt` del sitio mandaría al usuario a
        // mirar donde no es: el motivo está en el almacén y se consulta.
        let path = store_con_exclusiones("patron", "pattern", 40);
        let msg = empty_crawl_error_lang(&path, &metrics(0, 0, 40), Lang::Es)
            .expect("query")
            .expect("must warn");
        assert!(msg.contains("patrones"), "{msg}");
        assert!(!msg.contains("robots.txt"), "must not blame robots.txt: {msg}");
        let _ = std::fs::remove_file(&path);
    }

    // ── «Falló» y «nunca se pidió» son cosas distintas ───────────────────────

    /// Un fichero con la tabla `urls` mínima que consulta [`response_breakdown`].
    fn store_with_states(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("estados.sqlite");
        let conn = Connection::open(&path).expect("create the file");
        conn.execute_batch(
            "CREATE TABLE urls (url TEXT, status_code INTEGER, crawl_state TEXT,
                                error_kind TEXT, exclusion_reason TEXT, is_internal INTEGER);",
        )
        .expect("create the table");
        let filas: &[(&str, Option<i64>, &str, i64)] = &[
            ("https://e.es/", Some(200), "done", 1),
            ("https://e.es/a", Some(200), "done", 1),
            ("https://e.es/rota", Some(404), "done", 1),
            // Petición hecha que falló sin código: esto sí es «sin respuesta».
            ("https://e.es/caida", None, "error", 1),
            // Descubiertas y nunca pedidas: el rastreo se detuvo antes.
            ("https://e.es/p1", None, "pending", 1),
            ("https://e.es/p2", None, "pending", 1),
            ("https://e.es/p3", None, "pending", 1),
            // Externas sondeadas: su 404 no es un error del sitio auditado.
            ("https://otro.com/ok", Some(200), "skipped", 0),
            ("https://otro.com/rota", Some(404), "skipped", 0),
        ];
        for (url, status, state, interna) in filas {
            conn.execute(
                "INSERT INTO urls (url, status_code, crawl_state, is_internal)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![url, status, state, interna],
            )
            .expect("insert url");
        }
        path
    }

    #[test]
    fn lo_nunca_pedido_no_se_cuenta_como_fallo() {
        let dir = std::env::temp_dir().join(format!("cf-breakdown-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the directory");
        let store = store_with_states(&dir);
        let conn = Connection::open(&store).expect("open");

        let b = response_breakdown(&conn).expect("break down");

        // Each family splits internal from external: a foreign 404 must not inflate the
        // audited site's error count (review item 2).
        assert_eq!(
            b.groups,
            vec![("2xx".to_string(), 2, 1), ("4xx".to_string(), 1, 1)],
            "internal and external counted apart"
        );
        assert_eq!(b.no_response, 1, "only the request that actually failed");
        assert_eq!(b.never_requested, 3, "pending rows go apart, not as failures");
        assert_eq!(b.out_of_scope, 0, "no skipped internal rows in this crawl");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lo_saltado_sin_motivo_se_cuenta_como_fuera_del_alcance() {
        // List mode records links pointing outside the list as internal 'skipped' rows with
        // no exclusion reason. Without this bucket, "URLs 21" from a 5-URL list was a
        // mystery the user had to solve with SQL (review item 8).
        let dir = std::env::temp_dir().join(format!("cf-breakdown-scope-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the directory");
        let store = store_with_states(&dir);
        let conn = Connection::open(&store).expect("open");
        conn.execute_batch(
            "INSERT INTO urls (url, status_code, crawl_state, is_internal)
             VALUES ('https://e.es/fuera-1', NULL, 'skipped', 1),
                    ('https://e.es/fuera-2', NULL, 'skipped', 1);",
        )
        .expect("insert skipped rows");

        let b = response_breakdown(&conn).expect("break down");
        assert_eq!(b.out_of_scope, 2, "internal skipped rows without a reason are out of scope");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_rastreo_completo_no_ensena_el_cajon_de_no_solicitadas() {
        let dir = std::env::temp_dir().join(format!("cf-breakdown-full-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the directory");
        let path = dir.join("completo.sqlite");
        let conn = Connection::open(&path).expect("create");
        conn.execute_batch(
            "CREATE TABLE urls (url TEXT, status_code INTEGER, crawl_state TEXT,
                                error_kind TEXT, exclusion_reason TEXT, is_internal INTEGER);
             INSERT INTO urls VALUES ('https://e.es/', 200, 'done', NULL, NULL, 1);",
        )
        .expect("create the table");

        let b = response_breakdown(&conn).expect("break down");
        assert_eq!(b.never_requested, 0);
        assert_eq!(b.no_response, 0);
        assert_eq!(b.out_of_scope, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── El colapso de plantilla y la lista completa de una regla ─────────────

    /// Un rastreo con el esquema real: 40 páginas, 35 de ellas con el mismo defecto de
    /// plantilla (mismo `group_key`), 3 con la misma regla por otras causas, y una segunda
    /// regla con solo 2 páginas agrupadas — que NO es una plantilla.
    fn store_con_plantilla(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("plantilla.sqlite");
        let conn = crate::test_schema::crawl_file(&path);
        for i in 0..40 {
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal,
                                   in_sitemap, crawl_state, status_code)
                 VALUES (?1, ?2, ?1, 'https', 'e.es', '/', 1, 0, 'done', 200)",
                rusqlite::params![i + 1, format!("https://e.es/p{i:02}")],
            )
            .expect("url");
            conn.execute("INSERT INTO pages (url_id, is_indexable) VALUES (?1, 1)", [i + 1])
                .expect("page");
        }
        for i in 0..35 {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category, group_key, detail_json)
                 VALUES (?1, 'ASSET-IMG-EMPTY-ALT-LINK', 'high', 'asset', 'img-empty-alt:aaaa',
                         '{\"links\":1,\"sample\":[\"/logo.svg\"]}')",
                [i + 1],
            )
            .expect("template issue");
        }
        for i in 35..38 {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category)
                 VALUES (?1, 'ASSET-IMG-EMPTY-ALT-LINK', 'high', 'asset')",
                [i + 1],
            )
            .expect("loose issue");
        }
        for i in 0..2 {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category, group_key)
                 VALUES (?1, 'CONTENT-HEADING-SKIP', 'low', 'content', 'heading-skip:1>4:x')",
                [i + 1],
            )
            .expect("small issue");
        }
        path
    }

    fn dir_de_prueba(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cf-plantilla-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the directory");
        dir
    }

    #[test]
    fn el_ruido_de_plantilla_se_colapsa_en_el_resumen() {
        let dir = dir_de_prueba("resumen");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");

        let lineas = findings_lines(&conn, &store, Lang::En, 40).expect("summary");
        let texto = lineas.join("\n");

        // El titular: un problema, no 38 filas. Y las 3 páginas fuera del grupo no se ocultan.
        assert!(texto.contains("1 template issue (35 pages)"), "{texto}");
        assert!(texto.contains("+ 3 more findings"), "{texto}");
        assert!(!texto.contains("      38"), "the raw count is no longer the headline: {texto}");
        // Con dos URLs de ejemplo para reconocer la plantilla sin abrir nada más.
        assert!(texto.contains("e.g. https://e.es/p00"), "{texto}");
        // Y el comando que da la lista completa, listo para copiar.
        assert!(texto.contains("--rule <RULE-ID>"), "{texto}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dos_grupos_sobre_las_mismas_paginas_no_duplican_el_recuento() {
        // El caso real: 13 banners del pie, cada uno con su grupo, todos en las mismas ~540
        // páginas. Sumar los grupos decía «6.998 páginas» en un rastreo de 567.
        let dir = dir_de_prueba("solapados");
        let store = store_con_plantilla(&dir);
        {
            let conn = Connection::open(&store).expect("open");
            for grupo in ["img-no-alt:bbbb", "img-no-alt:cccc"] {
                for i in 0..35 {
                    conn.execute(
                        "INSERT INTO issues (url_id, rule_id, severity, category, group_key)
                         VALUES (?1, 'ASSET-IMG-NO-ALT', 'high', 'asset', ?2)",
                        rusqlite::params![i + 1, grupo],
                    )
                    .expect("overlapping issue");
                }
            }
        }
        let conn = Connection::open(&store).expect("open");
        let texto = findings_lines(&conn, &store, Lang::En, 40).expect("summary").join("\n");
        assert!(
            texto.contains("2 template issues (35 pages)"),
            "35 pages with two causes are 35 pages, not 70: {texto}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dos_paginas_con_la_misma_clave_no_son_una_plantilla() {
        let dir = dir_de_prueba("pequeno");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");

        let lineas = findings_lines(&conn, &store, Lang::En, 40).expect("summary");
        let texto = lineas.join("\n");

        // CONTENT-HEADING-SKIP tiene 2 páginas con la misma clave: recuento normal, sin colapso.
        let linea = lineas
            .iter()
            .find(|l| l.contains("CONTENT-HEADING-SKIP"))
            .expect("the small rule is there");
        assert!(!linea.contains("template"), "{texto}");
        assert!(linea.trim_end().ends_with('2'), "{linea:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_colapso_tambien_habla_espanol() {
        let dir = dir_de_prueba("espanol");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");

        let texto = findings_lines(&conn, &store, Lang::Es, 40).expect("summary").join("\n");
        assert!(texto.contains("1 problema de plantilla (35 páginas)"), "{texto}");
        assert!(texto.contains("+ 3 hallazgos más"), "{texto}");
        assert!(texto.contains("p. ej."), "{texto}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── La reformulación de reglas dominantes ────────────────────────────────
    //
    // El caso que el colapso de plantilla no cubre: hallazgos masivos **ciertos y sin causa
    // común hashable**. En el rastreo real que lo motivó, INDEX-DEEP-PAGE daba 202.392
    // hallazgos verdaderos —el archivo del medio no tiene atajos de paginación— y el informe
    // abría con una cifra que nadie lee.

    /// Un rastreo de 40 páginas donde una regla sin `group_key` afecta a 30: dominante.
    fn store_dominante(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("dominante.sqlite");
        let conn = crate::test_schema::crawl_file(&path);
        for i in 0..40 {
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal,
                                   in_sitemap, crawl_state, status_code)
                 VALUES (?1, ?2, ?1, 'https', 'e.es', '/', 1, 0, 'done', 200)",
                rusqlite::params![i + 1, format!("https://e.es/p{i:02}")],
            )
            .expect("url");
            conn.execute("INSERT INTO pages (url_id, is_indexable) VALUES (?1, 1)", [i + 1])
                .expect("page");
        }
        for i in 0..30 {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category)
                 VALUES (?1, 'META-TITLE-TOO-LONG', 'medium', 'meta')",
                [i + 1],
            )
            .expect("dominant issue");
        }
        path
    }

    /// Añade a un fichero 30 hallazgos de profundidad con `click_depth` real: diez a 5 clics,
    /// diez a 6, cinco a 7 y cinco a 12.
    fn con_hallazgos_de_profundidad(store: &std::path::Path) {
        let conn = Connection::open(store).expect("open");
        for i in 0..30 {
            let depth = match i {
                0..=9 => 5,
                10..=19 => 6,
                20..=24 => 7,
                _ => 12,
            };
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category, detail_json)
                 VALUES (?1, 'INDEX-DEEP-PAGE', 'medium', 'indexability', ?2)",
                rusqlite::params![
                    i + 1,
                    format!("{{\"click_depth\":{depth},\"max_click_depth\":4}}")
                ],
            )
            .expect("depth issue");
        }
    }

    #[test]
    fn una_regla_dominante_dice_su_cuota_del_sitio_sin_perder_el_recuento() {
        let dir = dir_de_prueba("dominante");
        let store = store_dominante(&dir);
        let conn = Connection::open(&store).expect("open");

        let texto = findings_lines(&conn, &store, Lang::En, 40).expect("summary").join("\n");
        // El recuento se conserva —nada se oculta— y la cuota lo reformula como propiedad
        // del sitio: 30 títulos largos en 40 páginas no son 30 tareas, son la plantilla.
        assert!(texto.contains("30"), "{texto}");
        assert!(texto.contains("(75% of the site)"), "{texto}");

        let es = findings_lines(&conn, &store, Lang::Es, 40).expect("summary").join("\n");
        assert!(es.contains("(75% del sitio)"), "{es}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_deep_page_masivo_se_dice_una_vez_con_su_forma() {
        // Lo que doscientas mil filas idénticas no dicen y una línea sí: cuántas, qué cuota
        // del sitio, en qué banda viven y hasta dónde llega la más hundida.
        let dir = dir_de_prueba("deep-forma");
        let store = store_dominante(&dir);
        con_hallazgos_de_profundidad(&store);
        let conn = Connection::open(&store).expect("open");

        let texto = findings_lines(&conn, &store, Lang::En, 40).expect("summary").join("\n");
        assert!(
            texto.contains("30 pages deeper than 4 clicks — 75% of the site \
                            (typical depth 5–7, deepest 12)"),
            "{texto}"
        );
        assert!(!texto.contains("INDEX-DEEP-PAGE            "), "the raw count is no longer the headline: {texto}");

        let es = findings_lines(&conn, &store, Lang::Es, 40).expect("summary").join("\n");
        assert!(
            es.contains("30 páginas a más de 4 clics — 75% del sitio \
                         (profundidad típica 5–7, máxima 12)"),
            "{es}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fichero_antiguo_sin_profundidad_cae_a_la_cuota_generica() {
        // Guarda de compatibilidad: los rastreos anteriores guardan `{"max_click_depth":4}` a
        // secas. Sin profundidades no se inventa la forma: recuento y cuota, como cualquier
        // otra regla dominante.
        let dir = dir_de_prueba("deep-antiguo");
        let store = store_dominante(&dir);
        {
            let conn = Connection::open(&store).expect("open");
            for i in 0..30 {
                conn.execute(
                    "INSERT INTO issues (url_id, rule_id, severity, category, detail_json)
                     VALUES (?1, 'INDEX-DEEP-PAGE', 'medium', 'indexability',
                             '{\"max_click_depth\":4}')",
                    [i + 1],
                )
                .expect("old issue");
            }
        }
        let conn = Connection::open(&store).expect("open");
        let texto = findings_lines(&conn, &store, Lang::En, 40).expect("summary").join("\n");
        let linea = texto
            .lines()
            .find(|l| l.contains("INDEX-DEEP-PAGE"))
            .expect("the rule is there");
        assert!(linea.contains("(75% of the site)"), "{linea}");
        assert!(!linea.contains("deepest"), "without data the shape is not made up: {linea}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_regla_critica_dominante_conserva_su_recuento_entero() {
        // La objeción al mecanismo general —«¿y una regla critical que afecta al 90% y sí hay
        // que enumerar?»— se responde así: la reformulación añade la cuota, nunca resta. El
        // recuento sigue en la línea, cada fila sigue en el fichero y `report --rule` la lista.
        let dir = dir_de_prueba("critica-dominante");
        let store = store_dominante(&dir);
        {
            let conn = Connection::open(&store).expect("open");
            for i in 0..36 {
                conn.execute(
                    "INSERT INTO issues (url_id, rule_id, severity, category)
                     VALUES (?1, 'HTTP-404-INTERNAL', 'critical', 'http')",
                    [i + 1],
                )
                .expect("critical issue");
            }
        }
        let conn = Connection::open(&store).expect("open");
        let texto = findings_lines(&conn, &store, Lang::En, 40).expect("summary").join("\n");
        let linea = texto
            .lines()
            .find(|l| l.contains("HTTP-404-INTERNAL"))
            .expect("the rule is there");
        assert!(linea.contains("36"), "the count does not disappear: {linea}");
        assert!(linea.contains("(90% of the site)"), "{linea}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pocas_paginas_afectadas_no_llevan_porcentaje() {
        // Guarda de no-regresión: en un informe normal, un recuento pequeño se queda como
        // estaba. 12 páginas profundas en un sitio de 200 son 12 líneas útiles.
        let dir = dir_de_prueba("pequeno-sin-cuota");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");
        let texto = findings_lines(&conn, &store, Lang::En, 40).expect("summary").join("\n");
        let linea = texto
            .lines()
            .find(|l| l.contains("CONTENT-HEADING-SKIP"))
            .expect("the small rule is there");
        assert!(!linea.contains('%'), "{linea}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_lista_de_deep_page_va_de_lo_mas_hundido_a_lo_menos() {
        // El resumen dice «la más profunda a 12» y la primera pregunta del consultor es
        // cuáles son esas: la lista las trae ordenadas por profundidad, con los clics delante.
        let dir = dir_de_prueba("deep-lista");
        let store = store_dominante(&dir);
        con_hallazgos_de_profundidad(&store);
        let conn = Connection::open(&store).expect("open");

        let texto = rule_urls_text(&conn, &store, "INDEX-DEEP-PAGE", Lang::En).expect("list");
        assert!(texto.contains("30 affected URLs"), "{texto}");
        let urls: Vec<&str> = texto.lines().filter(|l| l.contains("https://")).collect();
        assert_eq!(urls.len(), 30, "all URLs are listed, no cutoff: {texto}");
        assert!(
            urls[0].trim_start().starts_with("12  "),
            "the deepest first, with its clicks up front: {:?}",
            urls[0]
        );
        assert!(urls[29].trim_start().starts_with("5  "), "{:?}", urls[29]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_lista_de_una_regla_trae_todas_las_urls_sin_corte() {
        // La revisión de UX §5.1: el informe corta en tres y no decía dónde estaban las demás.
        let dir = dir_de_prueba("lista");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");

        let texto = rule_urls_text(&conn, &store, "ASSET-IMG-EMPTY-ALT-LINK", Lang::En).expect("list");
        for i in 0..38 {
            assert!(texto.contains(&format!("https://e.es/p{i:02}")), "missing p{i:02}:\n{texto}");
        }
        assert!(texto.contains("38 affected URLs"), "{texto}");
        // El grupo de plantilla se presenta como tal, con su causa, y el resto aparte.
        assert!(texto.contains("Template group — 35 pages"), "{texto}");
        assert!(texto.contains("cause:"), "{texto}");
        assert!(texto.contains("/logo.svg"), "{texto}");
        assert!(texto.contains("Other affected pages — 3"), "{texto}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_lista_acepta_el_id_en_minusculas_y_habla_espanol() {
        let dir = dir_de_prueba("minusculas");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");

        let texto = rule_urls_text(&conn, &store, "asset-img-empty-alt-link", Lang::Es).expect("list");
        assert!(texto.contains("38 URLs afectadas"), "{texto}");
        assert!(texto.contains("Grupo de plantilla — 35 páginas"), "{texto}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_regla_inexistente_es_un_error_que_dice_donde_esta_el_catalogo() {
        let dir = dir_de_prueba("inexistente");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");

        let err = rule_urls_text(&conn, &store, "NO-EXISTE", Lang::En).expect_err("not a rule");
        assert!(err.to_string().contains("crawlforge rules"), "{err}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_regla_sin_hallazgos_no_es_un_error() {
        let dir = dir_de_prueba("sin-hallazgos");
        let store = store_con_plantilla(&dir);
        let conn = Connection::open(&store).expect("open");

        let texto = rule_urls_text(&conn, &store, "HTTP-404-INTERNAL", Lang::En).expect("a catalog rule");
        assert!(texto.contains("No findings for HTTP-404-INTERNAL"), "{texto}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_resumen_del_rastreo_dice_cuantas_externas_se_comprobaron() {
        // Review item 4: the external probe stole crawl seconds without ever being named.
        // The closing summary must say how many externals were checked even when all went
        // fine, not only when the cap cut the list short.
        let outcome = CrawlOutcome {
            crawl_id: "x".into(),
            store_path: std::path::PathBuf::from("c.sqlite"),
            metrics: CrawlMetrics { externals_checked: 214, ..Default::default() },
            truncated: None,
            interrupted: false,
            wal_kept: false,
        };
        let texto: Vec<String> =
            truncation_lines(&outcome, Lang::En).into_iter().flatten().collect();
        assert_eq!(texto, vec!["214 external links checked."]);

        let es: Vec<String> =
            truncation_lines(&outcome, Lang::Es).into_iter().flatten().collect();
        assert_eq!(es, vec!["214 enlaces externos comprobados."]);

        // And with nothing checked, no noise.
        let callado = CrawlOutcome {
            crawl_id: "x".into(),
            store_path: std::path::PathBuf::from("c.sqlite"),
            metrics: CrawlMetrics::default(),
            truncated: None,
            interrupted: false,
            wal_kept: false,
        };
        assert!(truncation_lines(&callado, Lang::En).is_empty());
    }

    // ── El fichero cuenta su propia completitud (`report`, revisión §1) ──────

    /// Un rastreo con el esquema real, con la meta y las externas que se pidan.
    fn store_para_notas(
        nombre: &str,
        truncated: bool,
        reason: Option<&str>,
        check_external: bool,
        externas_sin_comprobar: usize,
    ) -> std::path::PathBuf {
        let dir = dir_de_prueba(nombre);
        let path = dir.join("notas.sqlite");
        let conn = crate::test_schema::crawl_file(&path);
        let mut job = crawlforge_core::job::CrawlJob::http("https://ejemplo.es/");
        job.limits.check_external = check_external;
        let config = serde_json::to_string(&job).expect("serialize the config");
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime, truncated, truncated_reason)
             VALUES ('x','p','P','https://ejemplo.es/','http',datetime('now'),'done',?1,
                     '0','0','free', ?2, ?3)",
            rusqlite::params![config, truncated as i64, reason],
        )
        .expect("insert crawl_meta");
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (1,'https://ejemplo.es/',1,'https','ejemplo.es','/',1,0,'done',200)",
            [],
        )
        .expect("seed url");
        for i in 0..externas_sin_comprobar {
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal,
                                   in_sitemap, crawl_state)
                 VALUES (?1, ?2, ?1, 'https', 'otro.com', '/', 0, 0, 'skipped')",
                rusqlite::params![i as i64 + 10, format!("https://otro.com/{i}")],
            )
            .expect("unchecked external");
        }
        path
    }

    fn notas(path: &std::path::Path, lang: Lang) -> Vec<String> {
        let conn = Connection::open(path).expect("open");
        let notes = crawlforge_cli::audit_report::store_notes(&conn).expect("read the notes");
        store_note_lines(&notes, lang)
    }

    #[test]
    fn el_resumen_de_un_fichero_truncado_lo_dice_y_nombra_las_reglas_calladas() {
        // Review item 1: the truncation warning only existed in the `crawl` output, and the
        // file travels — tomorrow's report, or a colleague's, lied by omission.
        let path = store_para_notas("truncado", true, Some("max_urls"), true, 0);
        let lineas = notas(&path, Lang::En);
        let texto = lineas.join("\n");
        assert!(texto.contains("truncated by max_urls"), "{texto}");
        assert!(
            texto.contains("INDEX-ORPHAN-PAGE"),
            "the silenced rules are named, not alluded to: {texto}"
        );

        let es = notas(&path, Lang::Es).join("\n");
        assert!(es.contains("truncado por max_urls"), "{es}");
        assert!(es.contains("no se evaluaron"), "{es}");
    }

    #[test]
    fn el_resumen_de_un_modo_lista_reutiliza_el_aviso_del_crawl() {
        // The list-mode notice already existed in `crawl` and was good: `report` must say
        // the same thing with the same string, not a second wording of the same fact.
        let path = store_para_notas("notas-lista", true, Some("list_mode"), true, 0);
        let lineas = notas(&path, Lang::En);
        assert_eq!(lineas, vec![msg::crawl_list_mode_note(Lang::En)]);
        assert!(!lineas.join("\n").contains("truncated"), "a list crawl was not cut short");
    }

    #[test]
    fn el_resumen_distingue_externas_apagadas_de_externas_sin_mirar() {
        // With the check off, "no broken external links" must not read as "none exist".
        let apagadas = store_para_notas("ext-off", false, None, false, 3);
        let texto = notas(&apagadas, Lang::En).join("\n");
        assert!(texto.contains("--no-external-check"), "{texto}");
        assert!(texto.contains("does not mean there are none"), "{texto}");

        // With the check on and leftovers, the cap is the cause and the fix is named.
        let tope = store_para_notas("ext-cap", false, None, true, 3);
        let texto = notas(&tope, Lang::En).join("\n");
        assert!(texto.contains("max_external cap"), "{texto}");
        assert!(texto.contains("--max-external"), "{texto}");

        // With the check on inside a truncated crawl, blaming the cap would be a guess.
        let cortado = store_para_notas("ext-cut", true, Some("max_urls"), true, 3);
        let texto = notas(&cortado, Lang::En).join("\n");
        assert!(texto.contains("never checked"), "{texto}");
        assert!(!texto.contains("max_external cap"), "{texto}");
    }

    #[test]
    fn un_rastreo_completo_no_abre_con_ningun_aviso() {
        let path = store_para_notas("completo", false, None, true, 0);
        assert!(notas(&path, Lang::En).is_empty(), "a clean crawl opens clean");
    }

    // ── La causa humanizada y el paso siguiente de `--rule` ──────────────────

    #[test]
    fn la_causa_de_un_grupo_se_lee_en_palabras_y_no_en_json() {
        // Review item 8: `causa: {"missing":["og:title",…]}` inside a Spanish report is a
        // raw dump, not a sentence.
        assert_eq!(
            humanize_detail(r#"{"missing":["og:title","og:image"],"present":[]}"#),
            "missing: og:title, og:image"
        );
        assert_eq!(humanize_detail(r#"{"links":1,"sample":["/logo.svg"]}"#), "links: 1 · sample: /logo.svg");
        // What is not a JSON object is shown as-is: raw data beats hidden data.
        assert_eq!(humanize_detail("texto suelto"), "texto suelto");
    }

    #[test]
    fn una_regla_http_dice_desde_donde_se_arregla() {
        // Review item 6: a broken link is fixed on the page that links to it, and that data
        // lives in `inspect` — the listing must say so, command ready to copy.
        let dir = dir_de_prueba("hint-http");
        let store = store_con_plantilla(&dir);
        {
            let conn = Connection::open(&store).expect("open");
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category)
                 VALUES (1, 'HTTP-404-INTERNAL', 'critical', 'http')",
                [],
            )
            .expect("http issue");
        }
        let conn = Connection::open(&store).expect("open");
        let texto = rule_urls_text(&conn, &store, "HTTP-404-INTERNAL", Lang::En).expect("list");
        assert!(texto.contains("fixed on the page that links to it"), "{texto}");
        assert!(texto.contains("crawlforge inspect"), "{texto}");
        assert!(texto.contains("'https://e.es/p00'"), "quoted, ready to paste: {texto}");

        let es = rule_urls_text(&conn, &store, "HTTP-404-INTERNAL", Lang::Es).expect("list");
        assert!(es.contains("se arregla en la página que lo enlaza"), "{es}");

        // A rule whose fix lives on the page itself does not get the noise.
        let otro = rule_urls_text(&conn, &store, "ASSET-IMG-EMPTY-ALT-LINK", Lang::En).expect("list");
        assert!(!otro.contains("crawlforge inspect"), "{otro}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── La detección del `--base` equivocado ─────────────────────────────────

    #[test]
    fn detecta_el_base_equivocado_cuando_todos_los_canonicals_van_a_otro_host() {
        let canonicals =
            vec!["https://fixture.local/a/", "https://fixture.local/b/", "https://fixture.local/c/"];
        let aviso = detect_base_mismatch(
            Lang::En,
            "localhost",
            "https://localhost/",
            canonicals.into_iter(),
        )
        .expect("must warn");
        assert!(aviso.contains("https://fixture.local/"), "{aviso}");
        assert!(aviso.contains("--base https://fixture.local/"), "proposes the fix: {aviso}");
        assert!(aviso.contains("3 of 3"), "{aviso}");
    }

    #[test]
    fn el_aviso_del_base_tambien_habla_espanol() {
        let canonicals = vec!["https://fixture.local/a/", "https://fixture.local/b/"];
        let aviso = detect_base_mismatch(
            Lang::Es,
            "localhost",
            "https://localhost/",
            canonicals.into_iter(),
        )
        .expect("must warn");
        assert!(aviso.contains("2 de 2"), "{aviso}");
        assert!(aviso.contains("--base https://fixture.local/"), "the command is not translated: {aviso}");
    }

    #[test]
    fn un_base_correcto_no_produce_aviso() {
        let canonicals = vec!["https://ejemplo.es/a/", "https://ejemplo.es/b/"];
        assert!(detect_base_mismatch(
            Lang::En,
            "ejemplo.es",
            "https://ejemplo.es/",
            canonicals.into_iter()
        )
        .is_none());
    }

    #[test]
    fn unos_pocos_canonicals_cruzados_no_disparan_el_aviso() {
        // Un 20% de canonicals a otro dominio es un hallazgo de páginas concretas
        // (CANON-CROSS-DOMAIN), no una señal de flag equivocado.
        let canonicals = vec![
            "https://ejemplo.es/a/",
            "https://ejemplo.es/b/",
            "https://ejemplo.es/c/",
            "https://ejemplo.es/d/",
            "https://otro.com/x/",
        ];
        assert!(detect_base_mismatch(
            Lang::En,
            "ejemplo.es",
            "https://ejemplo.es/",
            canonicals.into_iter()
        )
        .is_none());
    }

    #[test]
    fn una_sola_pagina_no_forma_mayoria() {
        let canonicals = vec!["https://otro.com/x/"];
        assert!(detect_base_mismatch(
            Lang::En,
            "ejemplo.es",
            "https://ejemplo.es/",
            canonicals.into_iter()
        )
        .is_none());
    }
}
