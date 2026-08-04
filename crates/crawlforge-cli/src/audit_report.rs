//! Informe de un rastreo en Markdown o HTML.
//!
//! No es el resumen de terminal de [`crate::report`], que sirve para mirar lo que acaba de pasar.
//! Esto es lo que se **pega en un ticket** o se adjunta al resumen de un pipeline: alguien que no
//! ha lanzado el rastreo tiene que entenderlo sin más contexto.
//!
//! De ahí las tres decisiones que lo gobiernan:
//!
//! - **Ordenado por severidad, no por frecuencia.** Un `INDEX-NOINDEX` en una página vale más que
//!   ochocientos títulos largos, y quien abra el informe tiene que ver eso primero.
//! - **Cada regla se explica.** El nombre y la descripción salen del catálogo, no de aquí, así que
//!   dicen lo mismo que la app y en el idioma que se pida. Un ticket que solo dice
//!   «CANON-TO-NOINDEX ×14» obliga al que lo lee a buscar qué significa.
//! - **Con examples de URL concretos.** Sin ellos, un hallazgo no es accionable: hay que poder
//!   pinchar y verlo.
//!
//! El HTML lleva su estilo dentro. Un informe que depende de una hoja externa deja de verse en
//! cuanto se adjunta a un correo o se sube a un artefacto de CI.

use crate::i18n::{self, msg};
use anyhow::{bail, Result};
use crawlforge_rules::{catalog, Lang, RuleMeta, Severity};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

/// Formato del informe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Self::Markdown),
            "html" => Some(Self::Html),
            _ => None,
        }
    }
}

/// Cuántas URLs de ejemplo se listan por regla.
///
/// Con tres se entiende el patrón y el informe sigue cabiendo en un ticket. El recuento total va
/// siempre al lado, así que no se pierde la magnitud.
const EXAMPLES_PER_RULE: usize = 3;

/// Un grupo de hallazgos de la misma regla.
struct Group {
    rule_id: String,
    severity: Severity,
    total: i64,
    examples: Vec<String>,
    /// Cuántos grupos de plantilla tiene la regla: hallazgos con el mismo `group_key` en
    /// tantas páginas que la causa es una y se arregla una vez. El criterio es
    /// `crawlforge_rules::is_template_group`, el mismo que usa el resumen de terminal.
    template_count: usize,
    /// Páginas **distintas** cubiertas por esos grupos. Distintas porque los grupos pueden
    /// solaparse: cada banner del pie forma su grupo y todos viven en las mismas páginas.
    template_pages: i64,
    /// Hallazgos fuera de los grupos de plantilla. Se dicen «hallazgos» y no «páginas» a
    /// propósito: la misma página puede estar dentro del grupo (el logo de la plantilla) y
    /// traer además un hallazgo propio (su imagen destacada), así que no son páginas nuevas.
    template_rest: i64,
    /// Cuota del sitio de una regla dominante (`crawlforge_rules::is_pervasive`), redondeada.
    /// `None` cuando el recuento se lee de un vistazo y el porcentaje no aporta nada.
    pervasive_pct: Option<i64>,
    /// La forma del problema de profundidad, solo para `INDEX-DEEP-PAGE` en ficheros con
    /// `click_depth` en el detalle.
    deep: Option<crawlforge_rules::DeepPageShape>,
}

impl Group {
    /// El titular del grupo: el recuento crudo, o «N problema(s) de plantilla (M páginas)»
    /// cuando la regla es ruido de plantilla, o —para una regla dominante sin causa común
    /// hashable— el recuento con su cuota del sitio. Un hallazgo que aparece en el 90% de las
    /// páginas es un problema que se arregla una vez, y el informe tiene que leerse así.
    fn headline(&self, lang: Lang) -> String {
        if self.template_count == 0 {
            if let Some(pct) = self.pervasive_pct {
                if let Some(f) = self.deep {
                    return msg::deep_pages_summary(
                        lang,
                        i18n::count(lang, f.pages),
                        f.max_click_depth,
                        pct,
                        f.typical_min,
                        f.typical_max,
                        f.deepest,
                    );
                }
                return format!(
                    "{} ({})",
                    i18n::count(lang, self.total),
                    msg::pervasive_note(lang, pct)
                );
            }
            return i18n::count(lang, self.total);
        }
        let mut texto = if self.template_count == 1 {
            msg::one_template_issue(lang, i18n::count(lang, self.template_pages))
        } else {
            msg::n_template_issues(lang, self.template_count, i18n::count(lang, self.template_pages))
        };
        if self.template_rest > 0 {
            texto.push_str(&msg::plus_more_findings(lang, i18n::count(lang, self.template_rest)));
        }
        texto
    }
}

/// Genera el informe de un fichero de rastreo.
pub fn render(store: &Path, format: &str, lang: Lang, out: Option<&Path>) -> Result<String> {
    use std::io::IsTerminal;
    render_impl(store, format, lang, out, std::io::stdout().is_terminal())
}

/// La lógica de [`render`], con el terminal como parámetro para poder probar los dos casos.
fn render_impl(
    store: &Path,
    format: &str,
    lang: Lang,
    out: Option<&Path>,
    stdout_is_tty: bool,
) -> Result<String> {
    // Estos dos errores se quedan en inglés a propósito: son errores de parseo de flags, el
    // terreno de clap, y en el caso de `--lang` ni siquiera hay un idioma válido en el que
    // responder. Es la misma decisión que `i18n::resolve_lang`.
    let Some(format) = Format::parse(format) else {
        // La lista completa de `--format`, incluido el valor por defecto: omitir `terminal`
        // hacía parecer que el modo con el que arranca todo el mundo no existía.
        bail!(
            "format not recognised: {format}. Available: terminal (the default), \
             md and html"
        );
    };
    // Antes de nada: que el fichero sea de verdad un rastreo. Sin esto, un fichero equivocado
    // acababa en «no such table: urls», que no le dice nada a un consultor SEO.
    crate::store_check::ensure_crawl_store(store)?;

    // Un informe HTML en el terminal son decenas de líneas de código que nadie puede leer, así
    // que sin `--out` y con la salida conectada a la pantalla se corta aquí, antes de generarlo.
    // Con la salida redirigida (`> informe.html`) no hay pantalla que llenar y se imprime, que
    // es lo que espera quien lo usa en un pipeline.
    if format == Format::Html && out.is_none() && stdout_is_tty {
        bail!(
            "an HTML report is code for the browser, not something readable here.\n\
             Save it and open it:  crawlforge report <crawl.sqlite> --format html --out report.html\n\
             Redirecting also works: … --format html > report.html"
        );
    }

    let conn = Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    let texto = match format {
        Format::Markdown => markdown(&conn, lang, store)?,
        Format::Html => html(&conn, lang, store)?,
    };

    if let Some(path) = out {
        std::fs::write(path, &texto)?;
    }
    Ok(texto)
}

/// Neutraliza los caracteres de control de un valor leído del fichero de rastreo.
///
/// Un rastreo real no puede producirlos —el crate `url` los codifica en porcentaje al
/// normalizar—, pero el fichero es entrada no confiable: «un rastreo = un fichero portable»
/// que se comparte, y un `.sqlite` fabricado puede meter `ESC]0;…` en una URL y reprogramar
/// el terminal de quien imprime el informe (revisión 2026-08-01 §1.7d). Se sustituyen por
/// U+FFFD en vez de borrarse: que se vea que ahí había algo que no debía estar. Caen también
/// `\n` y `\t`: estos valores son de una línea, y un salto embebido fabricaría líneas falsas
/// en el informe.
pub fn strip_control_chars(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { '\u{FFFD}' } else { c }).collect()
}

/// Datos de cabecera del rastreo.
struct Header {
    base_url: String,
    mode: String,
    started_at: String,
    truncated: bool,
    truncated_reason: Option<String>,
}

fn header(conn: &Connection) -> Result<Header> {
    // El filtrado se hace en la lectura, no en la impresión: así ningún camino nuevo
    // —Markdown, HTML, un formato futuro— puede olvidarse de aplicarlo.
    let c = conn.query_row(
        "SELECT base_url, mode, started_at, truncated, truncated_reason FROM crawl_meta LIMIT 1",
        [],
        |r| {
            Ok(Header {
                base_url: strip_control_chars(&r.get::<_, String>(0)?),
                mode: strip_control_chars(&r.get::<_, String>(1)?),
                started_at: strip_control_chars(&r.get::<_, String>(2)?),
                truncated: r.get::<_, i64>(3)? == 1,
                truncated_reason: r
                    .get::<_, Option<String>>(4)?
                    .map(|s| strip_control_chars(&s)),
            })
        },
    )?;
    Ok(c)
}

/// Lo que el propio fichero dice sobre su completitud: si el rastreo se cortó, si es de modo
/// lista, si las externas se comprobaron y cuántas quedaron sin comprobar.
///
/// Es el dato del §1 de la revisión de agosto: la honestidad del producto —«esto no se pudo
/// evaluar»— existía solo en la salida del `crawl`, y el fichero viaja. Vive aquí, en la
/// biblioteca, porque lo leen dos pantallas (el resumen de terminal y el informe MD/HTML) y
/// las dos tienen que contar lo mismo.
pub struct StoreNotes {
    pub truncated: bool,
    /// `max_urls`, `max_depth`, `max_duration` o `list_mode`. Identificadores, no prosa.
    pub truncated_reason: Option<String>,
    /// Lo que la configuración guardada dice de `check_external`. `None` cuando el
    /// `config_json` no se puede leer (un fichero antiguo o fabricado): sin dato no se afirma.
    pub check_external: Option<bool>,
    /// Externas registradas y nunca sondeadas: sin código, sin error de sonda y sin motivo
    /// de exclusión.
    pub externals_unchecked: i64,
}

pub fn store_notes(conn: &Connection) -> Result<StoreNotes> {
    use rusqlite::OptionalExtension;
    let meta: Option<(i64, Option<String>, String)> = conn
        .query_row(
            "SELECT truncated, truncated_reason, config_json FROM crawl_meta LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((truncated, reason, config_json)) = meta else {
        return Ok(StoreNotes {
            truncated: false,
            truncated_reason: None,
            check_external: None,
            externals_unchecked: 0,
        });
    };
    let check_external = serde_json::from_str::<crawlforge_core::job::CrawlJob>(&config_json)
        .ok()
        .map(|job| job.limits.check_external);
    let externals_unchecked: i64 = conn.query_row(
        "SELECT COUNT(*) FROM urls
         WHERE is_internal = 0 AND crawl_state = 'skipped'
           AND status_code IS NULL AND error_kind IS NULL AND exclusion_reason IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(StoreNotes {
        truncated: truncated == 1,
        truncated_reason: reason.map(|s| strip_control_chars(&s)),
        check_external,
        externals_unchecked,
    })
}

impl StoreNotes {
    /// La nota sobre las externas que corresponde a este fichero, si toca alguna.
    ///
    /// Tres casos y solo tres: la comprobación estaba apagada (y «cero externas rotas» no
    /// significa nada); estaba encendida, el rastreo terminó entero y aun así quedaron sin
    /// sondear (el tope `max_external`); o quedaron sin sondear en un rastreo cortado, donde
    /// culpar al tope sería inventar. Sin dato de configuración no se afirma nada.
    pub fn external_note(&self, lang: Lang) -> Option<String> {
        match self.check_external {
            Some(false) => Some(msg::external_check_disabled(lang)),
            Some(true) if self.externals_unchecked > 0 => {
                let n = i18n::count(lang, self.externals_unchecked);
                if self.truncated {
                    Some(msg::external_never_checked(lang, n))
                } else {
                    Some(msg::external_unchecked(lang, n))
                }
            }
            _ => None,
        }
    }

    /// La lista de reglas que un grafo incompleto deja sin evaluar, para los truncados de
    /// verdad (el modo lista ya lo cuenta su propia nota, sin enumerar).
    pub fn silenced_rules_note(&self, lang: Lang) -> Option<String> {
        let is_cut = self.truncated && self.truncated_reason.as_deref() != Some("list_mode");
        is_cut.then(|| {
            msg::rules_not_evaluated(lang, crawlforge_rules::REQUIERE_GRAFO_COMPLETO.join(", "))
        })
    }
}

/// Páginas **distintas** afectadas por cada `(rule_id, severity)`, contando solo filas que
/// son páginas HTML. Es el numerador de `crawlforge_rules::is_pervasive`: los hallazgos pueden
/// repetirse por página (una fila por imagen) y los hay sobre URLs que no son páginas, así que
/// ni `COUNT(*)` ni `COUNT(DISTINCT url_id)` a secas sirven de cuota del sitio.
///
/// Pública porque el resumen de terminal (`report.rs`, en el binario) aplica el mismo criterio:
/// dos pantallas que reformulan las mismas reglas tienen que contarlas igual.
pub fn affected_pages_by_rule(conn: &Connection) -> Result<HashMap<(String, String), i64>> {
    let mut stmt = conn.prepare(
        "SELECT i.rule_id, i.severity, COUNT(DISTINCT i.url_id)
         FROM issues i JOIN pages p ON p.url_id = i.url_id
         GROUP BY i.rule_id, i.severity",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(((r.get::<_, String>(0)?, r.get::<_, String>(1)?), r.get::<_, i64>(2)?))
        })?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    Ok(rows)
}

/// El porcentaje del sitio que cubre una regla dominante, redondeado al entero.
pub fn share_pct(affected: i64, total_pages: i64) -> i64 {
    if total_pages <= 0 {
        return 0;
    }
    ((affected as f64) * 100.0 / (total_pages as f64)).round() as i64
}

/// Recuentos que dan la escala del sitio.
struct Totals {
    urls: i64,
    paginas: i64,
    indexables: i64,
    errores: i64,
}

fn totals(conn: &Connection) -> Result<Totals> {
    Ok(Totals {
        urls: conn.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))?,
        paginas: conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?,
        indexables: conn
            .query_row("SELECT COUNT(*) FROM pages WHERE is_indexable = 1", [], |r| r.get(0))?,
        errores: conn.query_row(
            "SELECT COUNT(*) FROM urls WHERE status_code >= 400",
            [],
            |r| r.get(0),
        )?,
    })
}

/// Hallazgos agrupados por regla, de más grave a menos y, dentro de cada severidad, de más
/// frecuente a menos.
fn groups(conn: &Connection) -> Result<Vec<Group>> {
    // Denominador del criterio de plantilla: las páginas HTML del rastreo.
    let total_pages: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?;
    let affected = affected_pages_by_rule(conn)?;
    // La forma del problema de profundidad se lee una vez; `None` en los ficheros anteriores
    // al `click_depth`, que caen a la reformulación genérica por porcentaje.
    let deep_shape = crawlforge_rules::deep_page_shape(conn)?;

    let mut stmt = conn.prepare(
        "SELECT i.rule_id, i.severity, COUNT(*) AS total
         FROM issues i
         GROUP BY i.rule_id, i.severity
         ORDER BY CASE i.severity
                      WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2
                      WHEN 'low' THEN 3 ELSE 4 END,
                  total DESC",
    )?;
    let filas: Vec<(String, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(std::result::Result::ok)
        .collect();

    // Grupos de plantilla: mismo `group_key` en hallazgos de página. Los de sitio (`url_id`
    // nulo) ya son un hallazgo agregado y no se recolapsan.
    // La severidad entra en la clave de agrupación porque una regla puede repartir sus
    // hallazgos en dos severidades (`Issue::with_severity`), y un grupo de la severidad alta
    // no debe descontarse del recuento de la baja.
    let mut stmt = conn.prepare(
        "SELECT i.rule_id, i.severity, i.group_key, COUNT(*) AS n
         FROM issues i
         WHERE i.url_id IS NOT NULL AND i.group_key IS NOT NULL
         GROUP BY i.rule_id, i.severity, i.group_key
         ORDER BY n DESC",
    )?;
    let plantillas: Vec<(String, String, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(std::result::Result::ok)
        .filter(|(_, _, _, n)| crawlforge_rules::is_template_group(*n, total_pages))
        .collect();

    let mut stmt = conn.prepare(
        "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
         WHERE i.rule_id = ?1 LIMIT ?2",
    )?;
    let mut stmt_grupo = conn.prepare(
        "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
         WHERE i.rule_id = ?1 AND i.group_key = ?2 ORDER BY u.url LIMIT ?3",
    )?;

    let mut out = Vec::new();
    for (rule_id, severity, total) in filas {
        let template: Vec<&(String, String, String, i64)> = plantillas
            .iter()
            .filter(|(r, s, _, _)| *r == rule_id && *s == severity)
            .collect();

        // Con plantilla, los ejemplos salen del grupo mayor: enseñar tres URLs del mismo
        // defecto es lo que permite reconocer la plantilla de un vistazo. Sin ella, los
        // primeros hallazgos de la regla, como siempre.
        let examples: Vec<String> = match template.first() {
            Some((_, _, key, _)) => stmt_grupo
                .query_map(rusqlite::params![rule_id, key, EXAMPLES_PER_RULE as i64], |r| {
                    r.get::<_, String>(0)
                })?
                .filter_map(std::result::Result::ok)
                .map(|u| strip_control_chars(&u))
                .collect(),
            None => stmt
                .query_map(rusqlite::params![rule_id, EXAMPLES_PER_RULE as i64], |r| {
                    r.get::<_, String>(0)
                })?
                .filter_map(std::result::Result::ok)
                // Las URLs vienen del fichero, y el fichero no es de fiar. Ver
                // [`strip_control_chars`].
                .map(|u| strip_control_chars(&u))
                .collect(),
        };
        // Hallazgos fuera de los grupos de plantilla y páginas distintas dentro de ellos:
        // ver los comentarios de los campos.
        let (template_rest, template_pages) = if template.is_empty() {
            (0, 0)
        } else {
            let claves: Vec<&str> = template.iter().map(|(_, _, k, _)| k.as_str()).collect();
            let placeholders =
                (0..claves.len()).map(|i| format!("?{}", i + 3)).collect::<Vec<_>>().join(", ");
            let sql = format!(
                "SELECT COUNT(*) FROM issues i
                 WHERE i.rule_id = ?1 AND i.severity = ?2
                   AND (i.url_id IS NULL OR i.group_key IS NULL
                        OR i.group_key NOT IN ({placeholders}))"
            );
            let sql_dentro = format!(
                "SELECT COUNT(DISTINCT i.url_id) FROM issues i
                 WHERE i.rule_id = ?1 AND i.severity = ?2 AND i.group_key IN ({placeholders})"
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&rule_id, &severity];
            for clave in &claves {
                params.push(clave);
            }
            let fuera: i64 = conn.query_row(&sql, params.as_slice(), |r| r.get(0))?;
            let dentro: i64 = conn.query_row(&sql_dentro, params.as_slice(), |r| r.get(0))?;
            (fuera, dentro)
        };
        // La reformulación de regla dominante solo aplica donde la plantilla no colapsó ya:
        // el titular de plantilla ya dice «una causa, N páginas» y no necesita porcentaje.
        let pages = affected.get(&(rule_id.clone(), severity.clone())).copied().unwrap_or(0);
        let pervasive_pct = (template.is_empty()
            && crawlforge_rules::is_pervasive(pages, total_pages))
        .then(|| share_pct(pages, total_pages));
        out.push(Group {
            rule_id: strip_control_chars(&rule_id),
            severity: severity_from(&severity),
            total,
            examples,
            template_count: template.len(),
            template_pages,
            template_rest,
            pervasive_pct,
            deep: (rule_id == "INDEX-DEEP-PAGE" && pervasive_pct.is_some())
                .then_some(deep_shape)
                .flatten(),
        });
    }
    Ok(out)
}

fn severity_from(s: &str) -> Severity {
    match s {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "medium" => Severity::Medium,
        "low" => Severity::Low,
        _ => Severity::Info,
    }
}

/// Índice del catálogo por ID, para poner nombre y explicación a cada hallazgo.
fn rules_index() -> HashMap<&'static str, &'static RuleMeta> {
    catalog().into_iter().map(|m| (m.id, m)).collect()
}

fn markdown(conn: &Connection, lang: Lang, store: &Path) -> Result<String> {
    let head = header(conn)?;
    let tot = totals(conn)?;
    let found = groups(conn)?;
    let rules = rules_index();
    // Los recuentos llevan el millar del idioma (`3,816` / `3.816`), como el resto de la CLI.
    let n = |v: i64| i18n::count(lang, v);

    let mut s = String::new();
    s.push_str(&format!("# {} {}\n\n", msg::report_audit_of(lang), head.base_url));
    s.push_str(&format!("{}\n\n", msg::report_mode_line(lang, &head.mode, &head.started_at)));

    if head.truncated {
        // Va arriba y en negrita a propósito: quien lea recuentos sin saber que el rastreo se
        // cortó sacará conclusiones falsas sobre el tamaño del sitio.
        let motivo = head.truncated_reason.as_deref().unwrap_or("?");
        // `list_mode` no es un corte: el rastreo auditó su lista entera y el informe tiene
        // que decir eso, no «truncado».
        if motivo == "list_mode" {
            s.push_str(&format!("{}\n\n", msg::report_list_mode_note(lang)));
        } else {
            s.push_str(&format!("{}\n\n", msg::report_truncated_note(lang, motivo)));
        }
    }
    // Las mismas notas de completitud que el resumen de terminal, porque el fichero viaja:
    // el informe de mañana, o el del compañero que recibe el `.sqlite`, no puede mentir por
    // omisión sobre qué reglas callaron ni sobre si las externas se miraron.
    let notes = store_notes(conn)?;
    if let Some(nota) = notes.silenced_rules_note(lang) {
        s.push_str(&format!("> {nota}\n\n"));
    }
    if let Some(nota) = notes.external_note(lang) {
        s.push_str(&format!("> {nota}\n\n"));
    }

    s.push_str(&format!("| {} | {} |\n|---|---:|\n", msg::th_metric(lang), msg::th_value(lang)));
    s.push_str(&format!("| URLs | {} |\n", n(tot.urls)));
    s.push_str(&format!("| {} | {} |\n", msg::row_pages(lang), n(tot.paginas)));
    s.push_str(&format!("| {} | {} |\n", msg::row_indexable(lang), n(tot.indexables)));
    s.push_str(&format!("| {} | {} |\n\n", msg::row_errors_4xx_5xx(lang), n(tot.errores)));

    if found.is_empty() {
        s.push_str(&msg::no_findings(lang));
        s.push('\n');
        return Ok(s);
    }

    s.push_str(&format!("## {}\n\n", msg::heading_findings(lang)));

    let mut current_severity: Option<Severity> = None;
    for g in &found {
        if current_severity != Some(g.severity) {
            s.push_str(&format!("### {}\n\n", i18n::severity_label(lang, g.severity)));
            current_severity = Some(g.severity);
        }
        let meta = rules.get(g.rule_id.as_str());
        let nombre = meta.map(|m| m.name(lang)).unwrap_or(g.rule_id.as_str());
        // El titular colapsa el ruido de plantilla: «1 problema de plantilla (18.089 páginas)»
        // en vez de un recuento que tapa el resto del informe. Ver [`Group::headline`].
        s.push_str(&format!("**{nombre}** — {} · `{}`\n\n", g.headline(lang), g.rule_id));
        if let Some(m) = meta {
            s.push_str(&format!("{}\n\n", m.description(lang)));
        }
        for url in &g.examples {
            s.push_str(&format!("- {url}\n"));
        }
        if g.total > g.examples.len() as i64 {
            // El corte dice dónde está lo que no enseña: el comando que lista todas las URLs
            // de la regla, listo para copiar (revisión de UX §5.1). En una regla colapsada el
            // titular ya dio los números, así que aquí no se repite un recuento crudo de filas
            // que los contradiría.
            let cmd = format!("crawlforge report {} --rule {}", store.display(), g.rule_id);
            if g.template_count > 0 || g.pervasive_pct.is_some() {
                // El titular ya dio los números (plantilla o cuota del sitio): repetir aquí un
                // recuento crudo de filas los contradiría.
                s.push_str(&format!("- {}\n", msg::report_full_list_run(lang, cmd)));
            } else {
                let resto = g.total - g.examples.len() as i64;
                s.push_str(&format!("- {}\n", msg::report_more_run(lang, n(resto), cmd)));
            }
        }
        s.push('\n');
    }
    Ok(s)
}

fn html(conn: &Connection, lang: Lang, store: &Path) -> Result<String> {
    let cuerpo = markdown(conn, lang, store)?;
    Ok(html_document(lang, &msg::html_title(lang), &markdown_body_to_html(&cuerpo)))
}

/// The minimal, deliberate Markdown-to-HTML conversion: the reports use a known subset of
/// Markdown, so no dependency is needed to render it. If the reports grow, this changes.
/// `pub(crate)` because the portfolio panel renders its Markdown the same way.
pub(crate) fn markdown_body_to_html(cuerpo: &str) -> String {
    let mut html = String::new();
    let mut en_tabla = false;
    let mut en_lista = false;
    for linea in cuerpo.lines() {
        let l = linea.trim_end();
        if l.starts_with("|---") {
            continue;
        }
        if l.starts_with('|') {
            if !en_tabla {
                html.push_str("<table>\n");
                en_tabla = true;
            }
            let celdas: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            html.push_str("<tr>");
            for c in celdas {
                html.push_str(&format!("<td>{}</td>", escape(c)));
            }
            html.push_str("</tr>\n");
            continue;
        }
        if en_tabla {
            html.push_str("</table>\n");
            en_tabla = false;
        }
        if let Some(item) = l.strip_prefix("- ") {
            if !en_lista {
                html.push_str("<ul>\n");
                en_lista = true;
            }
            html.push_str(&format!("<li>{}</li>\n", escape(item)));
            continue;
        }
        if en_lista {
            html.push_str("</ul>\n");
            en_lista = false;
        }
        if let Some(t) = l.strip_prefix("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", escape(t)));
        } else if let Some(t) = l.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", escape(t)));
        } else if let Some(t) = l.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", escape(t)));
        } else if let Some(t) = l.strip_prefix("> ") {
            html.push_str(&format!("<blockquote>{}</blockquote>\n", inline(t)));
        } else if !l.is_empty() {
            html.push_str(&format!("<p>{}</p>\n", inline(l)));
        }
    }
    if en_tabla {
        html.push_str("</table>\n");
    }
    if en_lista {
        html.push_str("</ul>\n");
    }
    html
}

/// Wraps a rendered body in the self-contained HTML document (embedded style, correct `lang`).
/// `pub(crate)` because the portfolio panel produces the same kind of document.
pub(crate) fn html_document(lang: Lang, titulo: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{titulo}</title>\n<style>{ESTILO}</style>\n</head>\n<body>\n{body}</body>\n</html>\n",
        if lang == Lang::Es { "es" } else { "en" }
    )
}

/// Estilo mínimo, dentro del documento: un informe que depende de una hoja externa deja de verse
/// en cuanto se adjunta a un correo o se sube como artefacto de CI.
const ESTILO: &str = "
body{font:16px/1.6 -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;
     max-width:52rem;margin:2rem auto;padding:0 1rem;color:#1a1a1a}
h1{font-size:1.6rem;margin-bottom:.2rem}
h2{font-size:1.3rem;margin-top:2rem;border-bottom:1px solid #e5e5e5;padding-bottom:.3rem}
h3{font-size:1rem;text-transform:uppercase;letter-spacing:.04em;color:#666;margin-top:1.6rem}
table{border-collapse:collapse;margin:1rem 0}
td{border:1px solid #e5e5e5;padding:.35rem .7rem}
tr:first-child td{background:#f6f6f6;font-weight:600}
blockquote{border-left:3px solid #d94f4f;margin:1rem 0;padding:.5rem 1rem;background:#fdf3f3}
ul{padding-left:1.2rem}li{word-break:break-all}
code{background:#f2f2f2;padding:.1rem .3rem;border-radius:3px;font-size:.9em}
@media(prefers-color-scheme:dark){
  body{background:#1a1a1a;color:#e8e8e8}
  h3{color:#999}td{border-color:#333}tr:first-child td{background:#262626}
  blockquote{background:#2a1f1f}code{background:#2a2a2a}}
";

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Escapa y traduce el énfasis y el código en línea que usa el informe.
fn inline(s: &str) -> String {
    let mut out = escape(s);
    while let (Some(a), Some(b)) = (out.find("**"), out.rfind("**")) {
        // Con exactamente tres asteriscos las dos coincidencias se solapan (`a + 2 > b`) y el
        // rebanado de abajo hace panic. Hoy ningún dato del sitio rastreado llega hasta aquí
        // —solo textos del catálogo y campos del propio rastreo—, pero es una mina puesta: el día
        // que alguien meta un `<title>` en un párrafo del informe, un sitio ajeno cuelga el
        // comando.
        if b <= a + 2 {
            break;
        }
        out = format!("{}<strong>{}</strong>{}", &out[..a], &out[a + 2..b], &out[b + 2..]);
    }
    while let (Some(a), Some(b)) = (out.find('`'), out.rfind('`')) {
        if a == b {
            break;
        }
        out = format!("{}<code>{}</code>{}", &out[..a], &out[a + 1..b], &out[b + 1..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crawl file with the real schema —all the migrations, via `test_schema.rs`— and known
    /// findings.
    fn store_de_prueba(truncado: bool) -> (tempfile_min::Dir, std::path::PathBuf) {
        let dir = tempfile_min::Dir::new("informe");
        let path = dir.path().join("crawl.sqlite");
        let conn = crate::test_schema::crawl_file(&path);
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime, truncated, truncated_reason)
             VALUES ('x','p','P','https://ejemplo.es/','http','2026-07-31T10:00:00Z','done','{}',
                     '0','0','agency', ?1, ?2)",
            rusqlite::params![truncado as i64, truncado.then_some("max_urls")],
        )
        .expect("meta");
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (1,'https://ejemplo.es/con acentos & <ñ>',1,'https','ejemplo.es','/',1,0,
                     'done',200)",
            [],
        )
        .expect("url");
        conn.execute(
            "INSERT INTO pages (url_id, is_indexable) VALUES (1, 1)",
            [],
        )
        .expect("page");
        for (rule, sev) in
            [("META-TITLE-MISSING", "critical"), ("CONTENT-THIN", "high"), ("CANON-MISSING", "medium")]
        {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category) VALUES (1, ?1, ?2, 'meta')",
                rusqlite::params![rule, sev],
            )
            .expect("issue");
        }
        drop(conn);
        (dir, path)
    }

    /// Directorio temporal mínimo, para no añadir una dependencia solo por los tests.
    mod tempfile_min {
        use std::sync::atomic::{AtomicU32, Ordering};

        /// Contador para que dos tests en paralelo no compartan directorio. Sin él, el segundo
        /// se encuentra la base de datos del primero a medio migrar.
        static SIGUIENTE: AtomicU32 = AtomicU32::new(0);

        pub struct Dir(std::path::PathBuf);
        impl Dir {
            pub fn new(nombre: &str) -> Self {
                let n = SIGUIENTE.fetch_add(1, Ordering::Relaxed);
                let p = std::env::temp_dir()
                    .join(format!("crawlforge-inf-{}-{nombre}-{n}", std::process::id()));
                let _ = std::fs::remove_dir_all(&p);
                std::fs::create_dir_all(&p).expect("create temp dir");
                Self(p)
            }
            pub fn path(&self) -> &std::path::Path {
                &self.0
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    /// Como [`store_de_prueba`], con 35 de 40 páginas compartiendo un defecto de plantilla.
    fn store_con_plantilla() -> (tempfile_min::Dir, std::path::PathBuf) {
        let dir = tempfile_min::Dir::new("plantilla");
        let path = dir.path().join("crawl.sqlite");
        let conn = crate::test_schema::crawl_file(&path);
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime, truncated)
             VALUES ('x','p','P','https://e.es/','http','2026-08-01T10:00:00Z','done','{}',
                     '0','0','agency', 0)",
            [],
        )
        .expect("meta");
        for i in 0i64..40 {
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal,
                                   in_sitemap, crawl_state, status_code)
                 VALUES (?1, ?2, ?1, 'https', 'e.es', '/', 1, 0, 'done', 200)",
                rusqlite::params![i + 1, format!("https://e.es/p{i:02}")],
            )
            .expect("url");
            conn.execute("INSERT INTO pages (url_id, is_indexable) VALUES (?1, 1)", [i + 1])
                .expect("page");
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category, group_key)
                 VALUES (?1, 'ASSET-IMG-EMPTY-ALT-LINK', 'high', 'asset',
                         CASE WHEN ?1 <= 35 THEN 'img-empty-alt:aaaa' END)",
                [i + 1],
            )
            .expect("issue");
        }
        // Y una regla sin plantilla con más URLs que ejemplos, para el corte con recuento.
        for i in 0i64..5 {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category)
                 VALUES (?1, 'META-DESC-MISSING', 'high', 'meta')",
                [i + 1],
            )
            .expect("issue without group");
        }
        drop(conn);
        (dir, path)
    }

    #[test]
    fn el_informe_colapsa_el_ruido_de_plantilla_en_un_titular() {
        let (_d, path) = store_con_plantilla();
        let md = render(&path, "md", Lang::En, None).expect("report");
        // 35 páginas con la misma causa son **un** problema; las 5 sin agrupar no se ocultan.
        assert!(md.contains("1 template issue (35 pages) + 5 more findings"), "{md}");
        assert!(
            !md.contains("— 40 ·"),
            "the raw count is no longer the headline: {md}"
        );
    }

    #[test]
    fn la_linea_de_corte_dice_el_comando_que_lista_el_resto() {
        // La revisión de UX §5.1: «… 26 more» sin decir dónde están las otras 26. Ahora el
        // corte lleva el comando listo para copiar.
        let (_d, path) = store_con_plantilla();
        let md = render(&path, "md", Lang::En, None).expect("report");
        // En la regla sin plantilla, el corte con recuento y el comando.
        assert!(
            md.contains("… 2 more — run: crawlforge report") && md.contains("--rule META-DESC-MISSING"),
            "{md}"
        );
        // En la colapsada, el titular ya dio los números: solo el comando, sin un recuento
        // crudo de filas que los contradiga.
        assert!(
            md.contains("… full list: crawlforge report") && md.contains("--rule ASSET-IMG-EMPTY-ALT-LINK"),
            "{md}"
        );

        let es = render(&path, "md", Lang::Es, None).expect("report");
        assert!(es.contains("más — ejecuta: crawlforge report"), "{es}");
        assert!(es.contains("… lista completa: crawlforge report"), "{es}");
        assert!(es.contains("1 problema de plantilla (35 páginas)"), "{es}");
    }

    /// Como [`store_con_plantilla`], con una regla dominante **sin** `group_key` (30 de 40
    /// páginas) y, si se pide, hallazgos de profundidad con `click_depth` real.
    fn store_dominante(con_profundidad: bool) -> (tempfile_min::Dir, std::path::PathBuf) {
        let dir = tempfile_min::Dir::new("dominante");
        let path = dir.path().join("crawl.sqlite");
        let conn = crate::test_schema::crawl_file(&path);
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime, truncated)
             VALUES ('x','p','P','https://e.es/','http','2026-08-03T10:00:00Z','done','{}',
                     '0','0','agency', 0)",
            [],
        )
        .expect("meta");
        for i in 0i64..40 {
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
        for i in 0i64..30 {
            conn.execute(
                "INSERT INTO issues (url_id, rule_id, severity, category)
                 VALUES (?1, 'META-TITLE-TOO-LONG', 'medium', 'meta')",
                [i + 1],
            )
            .expect("dominant issue");
            if con_profundidad {
                let depth = if i < 20 { 5 } else { 9 };
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
        drop(conn);
        (dir, path)
    }

    #[test]
    fn el_informe_reformula_una_regla_dominante_con_su_cuota_del_sitio() {
        // El colapso de plantilla no cubre los hallazgos masivos ciertos sin causa común
        // hashable: 30 títulos largos en 40 páginas se leen como propiedad del sitio, no como
        // 30 tareas. El recuento no desaparece.
        let (_d, path) = store_dominante(false);
        let md = render(&path, "md", Lang::En, None).expect("report");
        assert!(md.contains("— 30 (75% of the site) ·"), "{md}");
        // El titular ya dio los números: el corte remite a la lista completa, sin un recuento
        // crudo que los contradiga.
        assert!(md.contains("… full list: crawlforge report"), "{md}");

        let es = render(&path, "md", Lang::Es, None).expect("report");
        assert!(es.contains("— 30 (75% del sitio) ·"), "{es}");
    }

    #[test]
    fn el_informe_dice_la_forma_del_problema_de_profundidad_una_vez() {
        let (_d, path) = store_dominante(true);
        let md = render(&path, "md", Lang::En, None).expect("report");
        // 20 páginas a 5 clics y 10 a 9: banda típica 5–9, la más hundida a 9.
        assert!(
            md.contains("30 pages deeper than 4 clicks — 75% of the site \
                         (typical depth 5–9, deepest 9)"),
            "{md}"
        );
        assert!(
            !md.contains("**Too many clicks from home** — 30 ·"),
            "the raw count is no longer the headline: {md}"
        );
    }

    #[test]
    fn pocas_paginas_por_regla_siguen_contandose_una_a_una() {
        // Guarda de no-regresión: el informe de un rastreo sin ruido de plantilla no cambia.
        let (_d, path) = store_de_prueba(false);
        let md = render(&path, "md", Lang::En, None).expect("report");
        assert!(!md.contains("template issue"), "{md}");
        assert!(md.contains("**Missing title** — 1 ·"), "{md}");
    }

    #[test]
    fn el_informe_ordena_por_severidad_y_no_por_frecuencia() {
        let (_d, path) = store_de_prueba(false);
        let md = render(&path, "md", Lang::Es, None).expect("report");

        let critico = md.find("Crítico").expect("critical section");
        let alto = md.find("Alto").expect("high section");
        let medio = md.find("Medio").expect("medium section");
        assert!(critico < alto && alto < medio, "severities go from most severe to least");
    }

    #[test]
    fn cada_regla_se_explica_con_el_texto_del_catalogo() {
        let (_d, path) = store_de_prueba(false);
        let md = render(&path, "md", Lang::Es, None).expect("report");
        // Un ticket que solo dice «META-TITLE-MISSING ×1» obliga a buscar qué significa.
        assert!(md.contains("META-TITLE-MISSING"));
        assert!(md.contains("Sin título"), "the name comes from the catalog");
        assert!(md.contains("factor on-page"), "and its description too");
    }

    #[test]
    fn el_informe_avisa_de_un_rastreo_truncado_antes_de_dar_recuentos() {
        let (_d, path) = store_de_prueba(true);
        let md = render(&path, "md", Lang::Es, None).expect("report");
        let aviso = md.find("truncado").expect("must warn");
        let metrica = md.find("| URLs |").expect("metrics table");
        assert!(aviso < metrica, "the notice comes before the numbers it qualifies");
    }

    #[test]
    fn un_rastreo_completo_no_lleva_el_aviso() {
        let (_d, path) = store_de_prueba(false);
        let md = render(&path, "md", Lang::Es, None).expect("report");
        assert!(!md.contains("truncado"));
    }

    #[test]
    fn un_rastreo_en_modo_lista_avisa_sin_decir_truncado() {
        // `list_mode` enciende `truncated` para que las reglas de grafo completo callen,
        // pero el rastreo no se cortó: auditó su lista entera. El informe dice eso, en el
        // mismo sitio —antes de los recuentos que condiciona— y sin la palabra «truncado».
        let (_d, path) = store_de_prueba(true);
        {
            let conn = Connection::open(&path).expect("open");
            conn.execute(
                "UPDATE crawl_meta SET mode = 'list', truncated_reason = 'list_mode'",
                [],
            )
            .expect("meta");
        }
        let md = render(&path, "md", Lang::Es, None).expect("report");
        assert!(!md.contains("truncado"), "nothing was cut short: {md:.400}");
        let aviso = md.find("modo lista").expect("must warn about what actually happened");
        let metrica = md.find("| URLs |").expect("metrics table");
        assert!(aviso < metrica, "the notice comes before the numbers it qualifies");
    }

    #[test]
    fn el_informe_truncado_nombra_las_reglas_calladas() {
        // Review item 1: the manual promises the report says what could not be evaluated,
        // and the MD/HTML report is exactly the artifact that travels without its author.
        let (_d, path) = store_de_prueba(true);
        let md = render(&path, "md", Lang::En, None).expect("report");
        assert!(
            md.contains("INDEX-ORPHAN-PAGE"),
            "the silenced rules are named, not alluded to: {md:.600}"
        );
    }

    #[test]
    fn el_informe_dice_si_las_externas_no_se_miraron() {
        let (_d, path) = store_de_prueba(false);
        {
            let mut job = crawlforge_core::job::CrawlJob::http("https://ejemplo.es/");
            job.limits.check_external = false;
            let config = serde_json::to_string(&job).expect("serialize");
            let conn = Connection::open(&path).expect("open");
            conn.execute("UPDATE crawl_meta SET config_json = ?1", [config]).expect("meta");
        }
        let md = render(&path, "md", Lang::En, None).expect("report");
        assert!(md.contains("--no-external-check"), "{md:.600}");
        assert!(md.contains("does not mean there are none"), "{md:.600}");

        let es = render(&path, "md", Lang::Es, None).expect("report");
        assert!(es.contains("no significa que no los haya"), "{es:.600}");
    }

    #[test]
    fn el_html_escapa_lo_que_viene_de_las_paginas() {
        // Las URLs y los títulos de un sitio real llevan lo que les da la gana. Un informe que
        // no escapa produce HTML roto en el mejor caso.
        let (_d, path) = store_de_prueba(false);
        let out = render(&path, "html", Lang::Es, None).expect("report");
        assert!(out.contains("&lt;ñ&gt;"), "angle brackets are escaped: {out:.400}");
        assert!(out.contains("&amp;"), "ampersands too");
        assert!(!out.contains("<ñ>"));
    }

    #[test]
    fn el_html_es_autocontenido() {
        let (_d, path) = store_de_prueba(false);
        let out = render(&path, "html", Lang::En, None).expect("report");
        assert!(out.contains("<style>"), "the style is embedded");
        assert!(!out.contains("<link"), "no external stylesheets: an email or a CI artifact will not load them");
        assert!(out.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn se_escribe_al_fichero_que_se_pida() {
        let (d, path) = store_de_prueba(false);
        let salida = d.path().join("informe.md");
        render(&path, "md", Lang::Es, Some(&salida)).expect("report");
        let contenido = std::fs::read_to_string(&salida).expect("read");
        assert!(contenido.contains("# Auditoría de https://ejemplo.es/"));
    }

    #[test]
    fn los_dos_idiomas_producen_informes_distintos() {
        let (_d, path) = store_de_prueba(false);
        let es = render(&path, "md", Lang::Es, None).expect("es");
        let en = render(&path, "md", Lang::En, None).expect("en");
        assert!(es.contains("Hallazgos") && en.contains("Findings"));
        assert_ne!(es, en);
    }

    #[test]
    fn un_formato_desconocido_es_un_error_y_no_un_silencio() {
        // El idioma ya no se valida aquí: lo resuelve `i18n::resolve_lang` una sola vez al
        // arrancar, y esta función recibe un `Lang` que por construcción es válido.
        let (_d, path) = store_de_prueba(false);
        assert!(render(&path, "pdf", Lang::Es, None).is_err());
        assert!(render(&path, "docx", Lang::En, None).is_err());
    }

    #[test]
    fn el_error_de_formato_lista_todos_los_validos_incluido_el_por_defecto() {
        // La revisión de UX cazó que «Los disponibles son md y html» omitía `terminal`, que es
        // el valor por defecto y sí es válido.
        let (_d, path) = store_de_prueba(false);
        let err = render(&path, "pdf", Lang::Es, None).expect_err("pdf does not exist");
        let msg = err.to_string();
        for formato in ["terminal", "md", "html"] {
            assert!(msg.contains(formato), "must list '{formato}': {msg}");
        }
    }

    #[test]
    fn html_sin_out_y_con_pantalla_delante_no_vuelca_codigo() {
        // Las 79 líneas de HTML crudo de la revisión de UX. Con un terminal delante se corta
        // con instrucciones; el test de abajo cubre el caso del pipeline.
        let (_d, path) = store_de_prueba(false);
        let err =
            render_impl(&path, "html", Lang::Es, None, true).expect_err("must not dump to the screen");
        let msg = err.to_string();
        assert!(msg.contains("--out"), "says how to save it: {msg}");
        assert!(msg.contains(">"), "and that redirecting it still works: {msg}");
    }

    #[test]
    fn html_redirigido_se_imprime_como_siempre() {
        // Quien hace `crawlforge report x.sqlite --format html > informe.html` quiere el HTML
        // por la salida estándar, y lo sigue teniendo.
        let (_d, path) = store_de_prueba(false);
        let html = render_impl(&path, "html", Lang::Es, None, false).expect("with a pipe it prints");
        assert!(html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn html_con_out_funciona_aunque_haya_pantalla() {
        let (d, path) = store_de_prueba(false);
        let salida = d.path().join("informe.html");
        render_impl(&path, "html", Lang::Es, Some(&salida), true).expect("with --out there is no dump");
        assert!(salida.exists());
    }

    #[test]
    fn un_fichero_manipulado_no_inyecta_secuencias_de_control_en_el_informe() {
        // Revisión 2026-08-01 §1.7d: un rastreo real no produce bytes de control —el crate
        // `url` los codifica en porcentaje—, pero un `.sqlite` fabricado sí, y los ficheros
        // de rastreo se comparten. `ESC]0;…BEL` reprograma el título del terminal; `ESC[2J`
        // lo limpia. Nada de eso puede llegar a la pantalla.
        let (_d, path) = store_de_prueba(false);
        {
            let conn = Connection::open(&path).expect("open");
            conn.execute(
                "UPDATE urls SET url = 'https://ejemplo.es/' || char(27) || ']0;pwned' || char(7)",
                [],
            )
            .expect("url with escape");
            conn.execute(
                "UPDATE crawl_meta SET base_url = 'https://ejemplo.es/' || char(27) || '[2J',
                                       truncated = 1,
                                       truncated_reason = 'x' || char(27) || '[31m'",
                [],
            )
            .expect("meta with escape");
        }
        let md = render(&path, "md", Lang::En, None).expect("report");
        assert!(!md.contains('\u{1b}'), "no ESC survives: {md:.300}");
        assert!(!md.contains('\u{7}'), "no BEL survives");
        assert!(md.contains('\u{FFFD}'), "the gap is marked, not silently erased");
    }

    #[test]
    fn un_fichero_que_no_es_un_rastreo_falla_sin_jerga_de_sqlite() {
        let d = tempfile_min::Dir::new("ajeno");
        let path = d.path().join("ajeno.sqlite");
        {
            let conn = Connection::open(&path).expect("create");
            conn.execute_batch("CREATE TABLE cosas (id INTEGER);").expect("foreign table");
        }
        let err = render(&path, "md", Lang::Es, None).expect_err("not a crawl");
        let msg = format!("{err:#}");
        assert!(msg.contains("is not a CrawlForge crawl file"), "{msg}");
        assert!(!msg.contains("no such table"), "no SQLite jargon: {msg}");
    }
}

#[cfg(test)]
mod prueba_manual {
    /// Genera el informe de un rastreo real para mirarlo con los ojos.
    ///
    /// `cargo test -p crawlforge-cli -- --ignored --nocapture ver_informe_real`
    #[test]
    #[ignore = "necesita un fichero de rastreo real; se lanza a mano"]
    fn ver_informe_real() {
        let Ok(store) = std::env::var("CRAWLFORGE_STORE") else {
            eprintln!("define CRAWLFORGE_STORE con la ruta de un .sqlite");
            return;
        };
        let md = super::render(std::path::Path::new(&store), "md", crawlforge_rules::Lang::Es, None).expect("report");
        println!("{md}");
    }
}

#[cfg(test)]
mod tests_inline {
    use super::inline;

    #[test]
    fn tres_asteriscos_seguidos_no_cuelgan_el_informe() {
        // Regresión: las dos coincidencias de `**` se solapaban y el rebanado hacía panic con
        // «byte range starts at 2 but ends at 1».
        for entrada in ["***", "hola *** adios", "****", "a**b*c", "**", "*"] {
            let salida = inline(entrada);
            assert!(!salida.is_empty() || entrada.is_empty(), "{entrada:?} → {salida:?}");
        }
    }

    #[test]
    fn el_enfasis_normal_sigue_funcionando() {
        assert_eq!(inline("**Rastreo truncado**"), "<strong>Rastreo truncado</strong>");
        assert_eq!(inline("con `código` dentro"), "con <code>código</code> dentro");
    }
}
