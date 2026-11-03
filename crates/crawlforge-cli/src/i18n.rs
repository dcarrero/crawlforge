//! Los mensajes de la CLI: qué funciones hay, con qué argumentos, y cómo se resuelve el idioma.
//! Los textos no están aquí: viven en `locales/<idioma>/cli.ftl`, en Fluent.
//!
//! # Por qué así
//!
//! **El inglés es el idioma de origen y los demás, traducciones** (`CONVENTIONS.md §4`). Con dos
//! idiomas, una tabla en Rust con una columna por idioma bastaba y se descartó Fluent. Con diez
//! deja de bastar: el polaco tiene cuatro formas de plural (`one`, `few`, `many`, `other`) y el
//! japonés una, y eso no se escribe con `if n == 1`. Ahora cada mensaje es una **declaración**
//! —nombre y argumentos— en el macro [`messages!`], y su texto, una entrada de `cli.ftl` con el
//! mismo nombre. Los motivos de Fluent y del cargador están en `crawlforge_rules::l10n`.
//!
//! - **Una llamada no cambia.** `msg::crawl_finished(lang, urls, secs)` sigue siendo una función
//!   con sus argumentos con nombre, así que un `println!` no sabe de Fluent.
//! - **Lo que antes no compilaba ahora no pasa los tests.** Que cada declaración tenga su texto
//!   en inglés con exactamente esos argumentos, que el inglés no tenga textos sin declarar y que
//!   cada traducción tenga todo lo del inglés con las mismas variables lo comprueban los tests de
//!   este módulo, que `tools/verificar.sh` ejecuta. Fallan listando todos los problemas a la vez.
//! - **Los plurales se resuelven en el `.ftl`.** Un argumento que llega como recuento formateado
//!   por [`count`] o [`group_thousands`] (o un entero pequeño) entra en Fluent como número, y los
//!   selectores `[one]`/`[few]`/`*[other]` eligen la forma según las reglas CLDR del idioma.
//! - **Un idioma sin traducir habla inglés**, números incluidos: ver [`effective`].
//!
//! # Qué NO pasa por aquí
//!
//! IDs de regla, URLs, rutas de fichero, códigos de estado, valores de columna de la base de
//! datos (`max_urls`, `noindex`, `critical` como token de `--fail-on`) y los nombres de los
//! comandos: son identificadores, no prosa. Tampoco los textos del catálogo de reglas, que
//! viven en `locales/<idioma>/rules.ftl` y se leen con `RuleMeta::name`/`description`.
//!
//! # Resolución del idioma
//!
//! `--lang` gana; sin él, la variable de entorno `CRAWLFORGE_LANG`; sin ninguna, inglés.
//! Un `--lang` desconocido es un error (una errata debe doler); un `CRAWLFORGE_LANG`
//! desconocido se ignora y se cae al inglés, porque una variable de entorno rota no debe
//! inutilizar todos los comandos de la máquina.

use anyhow::{bail, Result};
use crawlforge_rules::l10n::{self, Catalog};
use crawlforge_rules::{Lang, Severity};
use fluent_bundle::FluentArgs;
use std::fmt::Display;
use std::sync::OnceLock;
use unicode_width::UnicodeWidthStr;

// ─────────────────────────────────────────────────────── Resolución del idioma

/// El idioma fijado por `--lang` para todo el proceso. Es un `OnceLock` y no un parámetro en
/// cada firma porque el idioma de una CLI es genuinamente global al proceso, y porque así los
/// puntos de entrada que `main.rs` ya llama (`print_brief`, `print_report`…) no cambian de
/// firma: `main.rs` fija el idioma una vez y todo lo demás lo consulta.
static LANG_OVERRIDE: OnceLock<Lang> = OnceLock::new();

/// Fija el idioma del proceso (normalmente desde `--lang`). Solo la primera llamada cuenta.
pub fn set_lang(lang: Lang) {
    let _ = LANG_OVERRIDE.set(lang);
}

/// El idioma vigente: lo fijado por [`set_lang`], o `CRAWLFORGE_LANG`, o inglés.
pub fn current_lang() -> Lang {
    if let Some(lang) = LANG_OVERRIDE.get() {
        return *lang;
    }
    resolve_pure(None, std::env::var("CRAWLFORGE_LANG").ok().as_deref()).unwrap_or_default()
}

/// Resuelve el idioma con la precedencia completa: flag > `CRAWLFORGE_LANG` > inglés.
///
/// Pensada para `main.rs`: `let lang = i18n::resolve_lang(flag.as_deref())?;
/// i18n::set_lang(lang);` y a partir de ahí toda la salida obedece.
pub fn resolve_lang(flag: Option<&str>) -> Result<Lang> {
    resolve_pure(flag, std::env::var("CRAWLFORGE_LANG").ok().as_deref())
}

/// La parte pura de la resolución, separada para poder probar la precedencia sin tocar el
/// entorno del proceso (que es global y haría carreras entre tests).
fn resolve_pure(flag: Option<&str>, env: Option<&str>) -> Result<Lang> {
    if let Some(flag) = flag {
        // Una errata en el flag es un error, no un silencio en otro idioma. El mensaje va en
        // inglés a propósito: si el idioma pedido no se entiende, no hay idioma en el que
        // responder que no sea el de origen.
        let Some(lang) = Lang::parse(flag) else {
            bail!("language not recognised: {flag}. Available: {}", Lang::codes());
        };
        return Ok(lang);
    }
    Ok(env.and_then(Lang::parse).unwrap_or_default())
}

// ─────────────────────────────────────────────────────────── Los textos

/// `locales/<idioma>/cli.ftl`, incrustado en el binario.
pub static CLI_TEXTS: Catalog = Catalog::new(crawlforge_rules::locale_sources!("cli.ftl"));

/// El idioma en el que de verdad se habla: el pedido si `cli.ftl` tiene su traducción, inglés si
/// todavía no. Los números siguen a los textos: un mensaje inglés con millares alemanes
/// (`3.816 URLs`) sería ambiguo, así que mientras un idioma no esté traducido, tampoco sus
/// separadores.
pub fn effective(lang: Lang) -> Lang {
    if CLI_TEXTS.is_translated(lang) {
        lang
    } else {
        Lang::En
    }
}

/// Formatea un mensaje de `cli.ftl`. Lo llaman las funciones que genera [`messages!`].
///
/// Cada argumento entra como texto, tal como lo imprime su `Display`, salvo cuando es un
/// recuento tal como lo escribe [`count`]: entonces entra como número para que los plurales
/// funcionen, y Fluent lo vuelve a escribir igual (`crawlforge_rules::l10n::arg_value`).
pub fn format(lang: Lang, key: &str, args: &[(&str, &dyn Display)]) -> String {
    let lang = effective(lang);
    let fluent_args = (!args.is_empty()).then(|| {
        let mut fa = FluentArgs::with_capacity(args.len());
        for (name, value) in args {
            fa.set(*name, l10n::arg_value(lang, value.to_string()));
        }
        fa
    });
    // Sin texto ni en inglés solo se llega con un `cli.ftl` roto, que los tests no dejan pasar;
    // aun así, la clave dice algo más que una cadena vacía.
    CLI_TEXTS.format(lang, key, None, fluent_args.as_ref()).unwrap_or_else(|| key.to_string())
}

// ───────────────────────────────────────────────────────── Formato de números

/// Separador de millares según el idioma: `3,816` en inglés, `3.816` en español.
pub fn group_thousands(lang: Lang, n: u64) -> String {
    effective(lang).format_integer(n)
}

/// [`group_thousands`] para los recuentos `i64` que devuelve SQLite. Nunca son negativos;
/// si alguno lo fuera, se muestra sin agrupar antes que mentir.
pub fn count(lang: Lang, n: i64) -> String {
    u64::try_from(n).map_or_else(|_| n.to_string(), |v| group_thousands(lang, v))
}

/// [`count`] para los recuentos `usize` de Rust (`Vec::len`, contadores en memoria).
pub fn count_usize(lang: Lang, n: usize) -> String {
    group_thousands(lang, n as u64)
}

/// Un número con un decimal, con la coma decimal donde el idioma la usa: `12.3` / `12,3`.
///
/// Va a la vez que [`group_thousands`]: un `3.816` de millares junto a un `1.5` de decimal
/// sería ambiguo, así que o se localizan los dos o ninguno.
pub fn decimal1(lang: Lang, v: f64) -> String {
    let text = format!("{v:.1}");
    match effective(lang).decimal_separator() {
        '.' => text,
        sep => text.replace('.', &sep.to_string()),
    }
}

// ──────────────────────────────────────────────────────── Ancho en la terminal

/// Columnas que ocupa un texto en la terminal: un ideograma japonés ocupa dos, una letra
/// acentuada una. El relleno de `format!` (`{:<9}`) cuenta caracteres, que para el inglés y el
/// español es lo mismo y para el japonés desalinea la tabla.
pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// `{text:<width}` contando columnas de terminal en vez de caracteres.
pub fn pad(text: &str, width: usize) -> String {
    let w = display_width(text);
    let mut out = String::with_capacity(text.len() + width.saturating_sub(w));
    out.push_str(text);
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(w)));
    out
}

/// `{text:>width}` contando columnas de terminal en vez de caracteres.
pub fn pad_left(text: &str, width: usize) -> String {
    let w = display_width(text);
    let mut out = String::with_capacity(text.len() + width.saturating_sub(w));
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(w)));
    out.push_str(text);
    out
}

/// The width of a column of labels: the widest of them, in terminal columns, and never less
/// than `min` (the width the English layout always had, so English output does not move).
///
/// Fixed pads made translators shorten words to fit («k. A.» for «unbekannt»); a column sized
/// from the labels of the language in use lets every language write the natural word.
pub fn column_width<S: AsRef<str>>(labels: impl IntoIterator<Item = S>, min: usize) -> usize {
    labels.into_iter().map(|l| display_width(l.as_ref())).max().unwrap_or(0).max(min)
}

/// The width of a severity column: the longest severity word of the language, nine at least.
/// Computed once per language: it is asked for on every row of long listings.
pub fn severity_width(lang: Lang) -> usize {
    static WIDTHS: OnceLock<[usize; Lang::ALL.len()]> = OnceLock::new();
    WIDTHS.get_or_init(|| {
        Lang::ALL.map(|l| {
            column_width(
                ["critical", "high", "medium", "low", "info"].map(|s| severity_word(l, s)),
                9,
            )
        })
    })[effective(lang).index()]
}

// ──────────────────────────────────────────────────────── Cabeceras y etiquetas

/// Ancho de las cabeceras de sección de la terminal (`── Título ────…`).
const SECTION_WIDTH: usize = 45;

/// Una cabecera de sección con el ancho de siempre, rellene lo que rellene el título.
/// Cuenta columnas y no bytes: «Comparación» mide 11, no 12, y «比較» mide 4.
pub fn section(title: &str) -> String {
    let used = 3 + display_width(title) + 1;
    format!("── {title} {}", "─".repeat(SECTION_WIDTH.saturating_sub(used)))
}

/// Etiqueta de severidad con inicial mayúscula, para informes.
pub fn severity_label(lang: Lang, s: Severity) -> String {
    let key = match s {
        Severity::Critical => "severity_label_critical",
        Severity::High => "severity_label_high",
        Severity::Medium => "severity_label_medium",
        Severity::Low => "severity_label_low",
        Severity::Info => "severity_label_info",
    };
    format(lang, key, &[])
}

/// Severidad en minúsculas para columnas de tabla, desde el valor tal como está en la base de
/// datos. Un valor desconocido se devuelve tal cual: enseñar el dato crudo es mejor que ocultarlo.
pub fn severity_word(lang: Lang, db_value: &str) -> String {
    let key = match db_value {
        "critical" => "severity_word_critical",
        "high" => "severity_word_high",
        "medium" => "severity_word_medium",
        "low" => "severity_word_low",
        "info" => "severity_word_info",
        _ => return db_value.to_string(),
    };
    format(lang, key, &[])
}

/// Por qué una página no es indexable, para pantalla, desde el valor de
/// `pages.indexability_reason`. El valor guardado no cambia —los exports y `stats` lo escriben
/// tal cual, porque se filtran y se cruzan—; lo que se traduce es lo que lee una persona en
/// `report` e `inspect`. Un valor desconocido se enseña tal cual, como en [`severity_word`].
pub fn indexability_reason(lang: Lang, db_value: &str) -> String {
    let key = match db_value {
        "noindex" => "reason_noindex",
        "canonicalised" => "reason_canonicalised",
        "robots" => "reason_robots",
        "redirect" => "reason_redirect",
        "4xx" => "reason_4xx",
        "5xx" => "reason_5xx",
        "not_html" => "reason_not_html",
        _ => return db_value.to_string(),
    };
    format(lang, key, &[])
}

/// Why a URL was left out of the crawl, for display, from the value of `urls.exclusion_reason`
/// (`crawlforge_core::frontier::ExclusionReason::as_str`). Same contract as
/// [`indexability_reason`]: the stored value never changes —exports and `stats` write it as is,
/// because it is filtered and joined on— and an unknown value is shown as is.
pub fn exclusion_reason(lang: Lang, db_value: &str) -> String {
    match exclusion_key(db_value) {
        Some(key) => format(lang, key, &[]),
        None => db_value.to_string(),
    }
}

/// The `cli.ftl` key of a stored exclusion reason, if it is one this binary knows.
fn exclusion_key(db_value: &str) -> Option<&'static str> {
    Some(match db_value {
        "robots" => "exclusion_robots",
        "nofollow" => "exclusion_nofollow",
        "depth" => "exclusion_depth",
        "pattern" => "exclusion_pattern",
        "limit" => "exclusion_limit",
        "local_network" => "exclusion_local_network",
        "external_check_off" => "exclusion_external_check_off",
        "no_network" => "exclusion_no_network",
        _ => return None,
    })
}

/// The keys of [`exclusion_reason`], for the tests.
#[cfg(test)]
const EXCLUSION_KEYS: [&str; 8] = [
    "exclusion_robots",
    "exclusion_nofollow",
    "exclusion_depth",
    "exclusion_pattern",
    "exclusion_limit",
    "exclusion_local_network",
    "exclusion_external_check_off",
    "exclusion_no_network",
];

/// Las claves de [`indexability_reason`], para los tests.
#[cfg(test)]
const REASON_KEYS: [&str; 7] = [
    "reason_noindex",
    "reason_canonicalised",
    "reason_robots",
    "reason_redirect",
    "reason_4xx",
    "reason_5xx",
    "reason_not_html",
];

/// Las claves de `cli.ftl` que no declara [`messages!`] sino las funciones de arriba.
#[cfg(test)]
const SEVERITY_KEYS: [&str; 10] = [
    "severity_label_critical",
    "severity_label_high",
    "severity_label_medium",
    "severity_label_low",
    "severity_label_info",
    "severity_word_critical",
    "severity_word_high",
    "severity_word_medium",
    "severity_word_low",
    "severity_word_info",
];

// ─────────────────────────────────────────────────────────────────── El macro

/// Declara los mensajes: genera una función por entrada, `nombre(lang, args…) -> String`, que
/// formatea la entrada `nombre` de `cli.ftl` con esos argumentos como variables `$arg`.
///
/// Genera también `DECLARED`, la lista de nombres y argumentos, que es lo que los tests cruzan
/// con `locales/en/cli.ftl`: una entrada sin texto, un texto sin entrada o una variable que no
/// coincide con los argumentos rompen `cargo test`, no la salida de un usuario.
macro_rules! messages {
    ($(
        $(#[$meta:meta])*
        $name:ident ( $($arg:ident),* );
    )*) => {
        $(
            $(#[$meta])*
            pub fn $name(lang: Lang $(, $arg: impl ::std::fmt::Display)*) -> String {
                super::format(
                    lang,
                    stringify!($name),
                    &[$((stringify!($arg), &$arg as &dyn ::std::fmt::Display)),*],
                )
            }
        )*

        /// Cada mensaje declarado y sus argumentos, para los tests.
        #[cfg(test)]
        pub(super) const DECLARED: &[(&str, &[&str])] = &[$((stringify!($name), &[$(stringify!($arg)),*])),*];
    };
}

/// Todos los mensajes de la CLI, agrupados por pantalla. Sus textos, en `locales/<idioma>/cli.ftl`
/// con el mismo nombre.
///
/// Convención: los textos no llevan sangría ni relleno de columnas —eso es maquetación y vive
/// en el `println!` que los usa—, salvo cuando el mensaje es multilínea y la sangría forma
/// parte del propio texto.
pub mod msg {
    use super::Lang;

    messages! {
        // ── Resumen del rastreo (`report.rs`) ────────────────────────────────────
        crawl_finished(urls, secs);
        crawl_failed_suffix(n);
        crawl_truncated(reason);
        // A 429 is not a page's status: the site asked the crawler to slow down. The pages
        // were left pending, and the useful thing to say is how to get them.
        crawl_rate_limited_note(n, path);
        report_rate_limited_note(n);
        // The adaptive throttle halved a host's concurrency: the server was struggling, and
        // the crawl took longer than it would have. Saying so is what docs/03 §7 promised.
        crawl_throttled_note(n);
        // `list_mode` no es un corte: el rastreo hizo exactamente lo que se le pidió, así que
        // decirle al usuario «tu rastreo se truncó» sería mentir. Lo que sí hay que decirle es
        // la consecuencia de auditar un conjunto y no un sitio.
        crawl_list_mode_note();
        // El tope de externas no trunca el rastreo del sitio —ese es `crawl_truncated`— pero
        // dejar enlaces sin comprobar en silencio haría que el informe pareciera completo sin
        // serlo, así que se dice cuántos quedaron fuera.
        external_unchecked(n);
        // El otro tope de externas: el del registro. Estas no están en el fichero, así que sin
        // este aviso el informe no tendría ni una fila que delatara que faltan.
        external_unregistered(n);
        // La variante de fichero para un rastreo cortado: aquí no se puede afirmar que el
        // tope fuera el motivo — un truncado deja externas sin sondear igualmente.
        external_never_checked(n);
        // Una auditoría de carpeta no tiene red: el motivo no es el tope, y `audit` no acepta
        // --max-external. Lo que sí sirve es rastrear lo publicado.
        external_offline(n);
        // La otra mitad de la honestidad de las externas: cuando la comprobación estaba
        // apagada, «cero enlaces externos rotos» no significa que no los haya.
        external_check_disabled();
        // El resumen del rastreo dice cuántas externas se sondearon aunque todo fuera bien:
        // es el tiempo que el cierre atribuía en silencio al rastreo del sitio.
        external_checked_note(n);
        // Qué reglas calló el truncado, por su ID: sin la lista, «las reglas que necesitan el
        // grafo» obliga a adivinar cuáles son.
        rules_not_evaluated(rules);
        results_title();
        label_urls();
        label_html_pages();
        label_links();
        note_internal(n);
        note_indexable(n);
        heading_status_codes();
        label_no_response();
        note_request_failed();
        label_never_requested();
        note_never_requested();
        // Filas internas `skipped` sin motivo: en modo lista, los enlaces que apuntan fuera
        // de la lista. Explican por qué «URLs 21» cuando se dieron 5.
        label_out_of_scope();
        note_out_of_scope();
        heading_non_indexable();
        heading_findings();
        no_findings();
        heading_excluded();
        // ── Colapso de plantilla ─────────────────────────────────────────────
        //
        // Un defecto de la plantilla aparece en el 90% de las páginas y se arregla una vez:
        // el informe dice «1 problema de plantilla (18.089 páginas)» en vez de un recuento
        // que tapa todo lo demás. Las filas de `issues` no se tocan: esto es presentación.
        // `k` problemas distintos de plantilla, que cubren `pages` páginas.
        template_issues(k, pages);
        // «Hallazgos» y no «páginas»: la misma página puede estar dentro del grupo de
        // plantilla (el logo) y traer además un hallazgo propio (su imagen destacada), así
        // que el resto no son páginas nuevas, son hallazgos fuera del grupo.
        plus_more_findings(n);
        example_urls(urls);
        // ── Reformulación de reglas dominantes ───────────────────────────────
        //
        // El caso que el colapso de plantilla no cubre: hallazgos masivos **ciertos y sin causa
        // común hashable** (202.392 páginas profundas de un archivo sin atajos, 173.654 títulos
        // largos de la misma plantilla). El recuento se conserva —nada se oculta— y se le añade
        // la cuota del sitio, que es lo que lo convierte en un problema de arquitectura y no en
        // una lista de tareas. El criterio es `crawlforge_rules::is_pervasive`.
        pervasive_note(pct);
        // La forma del problema de profundidad, dicha una vez: es la línea que sustituye a
        // «202,392» a secas. Los números llegan ya formateados con el millar del idioma.
        deep_pages_summary(pages, max, pct, low, high, deepest);
        rule_deep_sorted();
        hint_full_lists(cmd);
        // ── Listado de una regla (`report --rule`) ───────────────────────────
        rule_affected_urls(n);
        rule_template_group(n);
        rule_group_cause(detail);
        rule_other_pages(n);
        rule_no_findings(rule);
        // El paso siguiente de un enlace roto: se arregla en la página que lo enlaza, y ese
        // dato lo da `inspect`. El comando lo imprime el llamador, listo para copiar.
        hint_who_links();
        error_unknown_rule(rule);
        error_all_blocked_by_robots();
        error_all_excluded_by_pattern();
        error_no_urls_fetched();
        error_dead_seed(url, cause);
        cause_dns();
        cause_tls();
        cause_timeout();
        cause_connection();
        cause_no_response();
        warn_base_mismatch(n, total, origin, base);

        // ── Ficha de una URL (`inspect.rs`) ──────────────────────────────────────
        //
        // La pregunta que responde es «¿quién enlaza aquí?» (revisión 2026-08-01 §5.2), así que
        // los enlaces entrantes son la estrella; el resto de secciones evitan tener que abrir
        // el XLSX para ver una sola URL. Los valores de la base de datos (códigos, regiones,
        // `nofollow`, motivos de exclusión) no se traducen: son identificadores.
        inspect_error_not_found(url);
        inspect_closest();
        inspect_status_title();
        label_http_status();
        label_content_type();
        label_response_time();
        label_depth();
        note_depth_clicks();
        label_in_sitemap();
        word_yes();
        word_no();
        label_crawl_state();
        state_note_pending();
        state_note_excluded(reason);
        // Una externa con código o con error de sonda **sí se pidió**: decir de ella
        // «excluida del rastreo» era falso. Se comprobó su estado y no se auditó su contenido.
        state_note_external_checked();
        state_note_external_not_checked(reason);
        state_note_external_unchecked();
        // Interna `skipped` sin motivo: fuera del alcance del rastreo (el caso del modo lista).
        state_note_out_of_scope();
        state_note_error(detail);
        // `unknown` no es una región del HTML, es el relleno de «no se supo»: se traduce,
        // a diferencia de `main`/`nav`/`footer`, que son identificadores del documento.
        region_unknown();
        inspect_page_title();
        label_page_title();
        label_meta_description();
        label_h1();
        label_word_count();
        label_canonical();
        canonical_self();
        label_indexable();
        indexable_no(reason);
        chars_suffix(n);
        inspect_redirects_title();
        inlinks_title(n);
        inlinks_by_region(breakdown, nofollow);
        inlinks_none();
        inlinks_sample_heading();
        no_anchor();
        // El corte dice cuántas se enseñan y el comando exacto de la lista completa: cortar
        // sin decir cómo ver el resto es el error del que nació `report --rule` (§5.1).
        inspect_truncated(n, cmd);
        images_title(n);
        image_no_alt();
        image_usage_title();

        // ── Comparación de rastreos (`diff.rs`) ──────────────────────────────────
        diff_title();
        label_before();
        label_after();
        label_site();
        label_common();
        diff_crawl_counts(urls, findings);
        diff_common_note(n);
        warnings_title();
        tag_inconclusive();
        tag_warning();
        side_before();
        side_after();
        // Two complete sentences, not one with a glued «by …» suffix: where the reason goes
        // is the translation's business.
        warn_truncated(side);
        warn_truncated_by(side, reason);
        // La variante de `warn_truncated` para `list_mode`: el rastreo no se cortó — es que
        // un rastreo en modo lista nunca ve más que su lista. La consecuencia para el diff es
        // la misma (las ausencias no se pueden afirmar) y el motivo, distinto.
        warn_list_mode(side);
        warn_unfinished(side, status);
        warn_rules_changed(before, after);
        warn_config_changed(fields);
        warn_scope_changed(before, after);
        warn_mode_changed(before, after);
        warn_missing_table(side, table);
        warn_order_inverted(before_started, after_started);
        worse_title();
        label_new_findings();
        more_examples(n);
        label_pages_lost_index();
        label_status_worse();
        robots_content_changed(host);
        robots_blocks_all();
        nothing_worse();
        better_title();
        label_findings_resolved();
        label_pages_indexable_again();
        label_status_better();
        other_title();
        label_new_urls();
        label_urls_gone();
        label_titles_changed();
        label_meta_changed();
        label_canonicals_changed();
        label_findings_persisted();
        suppressed_title();
        // Multilínea: el punto de llamada imprime cada línea con su sangría.
        suppressed_intro();
        label_candidate_new_urls();
        label_candidate_urls_gone();
        label_candidate_new_findings();
        label_candidate_resolved();
        suppressed_advice();
        gate_title();
        // Multilínea: el punto de llamada imprime cada línea con su sangría.
        gate_inconclusive();
        gate_fail_detail(n, severity, token);
        gate_pass();
        // ── Reabrir un diff guardado y listar sus filas (`diff_file.rs`) ──────
        label_saved();
        diff_saved_note(generated_at, version, path);
        // Multilínea: el punto de llamada imprime cada línea con su sangría.
        hint_diff_full_lists();
        warn_unknown_change_types(n, types);
        label_status_changed();
        diff_no_matching_changes();
        error_diff_missing(path);
        error_diff_needs_two(path);
        hint_diff_prev(prev, path);
        error_diff_unknown_file(path);
        error_diff_too_new(path, version, supported);
        error_diff_reopen_flag(flag);
        error_unknown_change_type(value, available);
        error_rule_with_change(change);
        site_wide_finding();
        site_placeholder();
        error_missing_file(path);
        // El contrato del manual §5: todo error de fichero dice qué comando lo genera.
        error_store_missing(path);
        error_not_a_crawl(path);
        error_different_sites(a, b);
        error_fail_on_unknown(token);

        // ── Panel de cartera (`portfolio.rs`) ────────────────────────────────────
        //
        // Los IDs de regla, las rutas, las fechas y las versiones no se traducen: son datos e
        // identificadores, como en el resto de la CLI. Las cabeceras de severidad de la tabla
        // sí (`th_sev_*`): las lee una persona y ninguna máquina las analiza —el panel no tiene
        // salida JSON ni CSV—, a diferencia de los tokens de `--fail-on`.
        portfolio_title();
        portfolio_range(oldest, newest);
        // Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
        // changed" would silently mean a different period on every site.
        warn_portfolio_date_spread(days, oldest, newest);
        // Trap 3.2: two files crawled with different catalogs are not comparable without
        // saying so — a rule can be "missing" on a site because it did not exist yet.
        warn_portfolio_rules_versions(versions);
        warn_portfolio_core_versions(versions);
        portfolio_skipped_title();
        // A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
        portfolio_prev_not_input();
        error_portfolio_newer_schema(version, supported);
        error_portfolio_no_sites();
        error_portfolio_tier();
        error_portfolio_too_many(n, max);
        portfolio_changes_title();
        portfolio_no_pairs();
        portfolio_new_critical_high();
        portfolio_none_critical_high();
        portfolio_rest_title();
        portfolio_no_changes();
        // The diff of that pair cannot assert absences: one side is incomplete.
        portfolio_pair_inconclusive();
        portfolio_pair_failed(reason);
        portfolio_spread_title();
        portfolio_spread_intro();
        portfolio_spread_none();
        portfolio_glance_title();
        th_site();
        th_crawled();
        th_indexable();
        // The severity columns of the at-a-glance table, abbreviated: five of them share a
        // line with the URL counts. Display only; `--fail-on` keeps its English tokens.
        th_sev_critical();
        th_sev_high();
        th_sev_medium();
        th_sev_low();
        th_sev_info();
        flag_truncated();
        flag_list_mode();
        flag_unfinished(status);
        hint_site_diff();
        portfolio_html_title();

        // ── Per-site rule configuration (`rule_config.rs`) ───────────────────────
        rules_config_source_stored();
        rules_config_source_after();
        rules_config_source_file(path);
        warn_rules_config_differs(source);
        warn_portfolio_rules_config(sites);

        // ── Identificación de ficheros (`store_check.rs`) ────────────────────────
        error_store_is_diff(path);
        error_store_foreign(path);
        error_store_not_sqlite(path);

        // ── Informe Markdown/HTML (`audit_report.rs`) ────────────────────────────
        report_audit_of();
        // Un rastreo `list` que mezcla sitios: titularlo con el primero decía que los demás
        // no se habían auditado.
        report_audit_of_sites(n, sites);
        sites_and_more(sites, n);
        report_mode_line(mode, started);
        report_truncated_note(reason);
        // Para `truncated_reason = 'list_mode'`: el rastreo no se cortó, auditó su lista
        // entera. El informe tiene que decir eso, no «truncado».
        report_list_mode_note();
        th_metric();
        th_value();
        row_pages();
        row_indexable();
        row_errors_4xx_5xx();
        report_more(n);
        // El corte del informe dice dónde están las que no enseña: sin el comando, la única
        // vía para verlas era abrir el XLSX o hacer SQL a mano (revisión de UX §5.1).
        report_more_run(n, cmd);
        // La variante para una regla colapsada: el titular ya dio los números («13 problemas
        // de plantilla (567 páginas)») y repetir aquí un recuento crudo de filas los
        // contradiría.
        report_full_list_run(cmd);
        html_title();

        // ── Catálogo de reglas (`rules.rs`) ──────────────────────────────────────
        th_severity();
        th_category();
        th_scope();
        th_tier();
        th_name();
        lbl_category();
        lbl_scope();
        lbl_tier();
        lbl_reference();
        scope_page();
        scope_site();
        rules_summary(total, free, page, site);
        error_unknown_category(category, list);

        // ── Cadenas de `main.rs`, listas para cablear ────────────────────────────
        //
        // `main.rs` es de otro propietario; estas entradas existen para que el cableado sea
        // sustituir cada literal por su llamada. Los números se pre-formatean con
        // `i18n::group_thousands` / `i18n::decimal1` antes de pasarlos.
        crawling(target);
        // Se valida **antes de tocar el disco**: una semilla imposible no debe costar una
        // rotación de ficheros ni dejar un `.sqlite` de aspecto válido con un `.lock` al lado.
        error_invalid_seed(url);
        file_line(path);
        previous_kept(path);
        report_written(path);
        /// Con `--out` y el formato de terminal, lo que se escribe es Markdown: un volcado del
        /// terminal en un fichero conserva sus cajas y se lee peor. Se dice **qué** se escribió
        /// para que nadie abra el fichero esperando lo que acaba de ver en pantalla.
        markdown_report_written(path);
        exported_csv(n, dir);
        exported_sheets(n, path);
        bench_appended(path);
        next_steps_title();
        next_explain();
        next_spreadsheet();
        next_compare_previous();
        next_compare_later();
        next_reread();
        // ── Autenticación básica HTTP (staging protegido) ────────────────────
        //
        // Los tres avisos van por stderr: hablan de secretos y no deben ensuciar un pipe.
        // Ninguno repite la credencial, como corresponde a un aviso sobre secretos.
        auth_url_note(host);
        auth_env_note(host);
        auth_base_ignored();
        auth_list_ignored();
        progress_sitemaps();
        progress_finalize();
        progress_crawl(fetched, queued, rate, findings);
        progress_failed_suffix(n);

        // ── Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`) ──────
        resuming(target);
        resume_counts(done, pending);
        resume_config_note();
        error_resume_schema_blocking(file, version, blocking);
        resume_robots_not_inherited();
        resume_finished(urls, secs);
        interrupt_flushing();
        // Un Ctrl+C cuando el motor ya terminó —durante un export largo, por ejemplo— no
        // tiene nada que guardar: se sale ya, diciendo que el rastreo no corre peligro.
        interrupt_after_done();
        // La degradación del cierre (§3.1/§3.2 de la revisión): con otro programa leyendo el
        // fichero no se puede salir del modo WAL, y copiar el `.sqlite` suelto perdería lo
        // que quede en el `-wal`. El aviso da el dato y las dos salidas.
        warn_wal_kept(path);
        // El cerrojo del fichero (§3.3): el que llega segundo no espera ni rota nada, se le
        // dice que hay otro rastreo escribiendo y qué hacer.
        error_store_locked(path);
        interrupted_saved(path);
        hint_previous_unfinished();
        error_resume_done(path);
        error_resume_status(path, status);
        error_resume_schema(path, found, expected);
        error_resume_config(path);
    }

    // Los mensajes con recuentos que llegan como enteros: formatean el número con [`count`]
    // (o lo pasan tal cual, si la salida nunca lo agrupó) y dejan el plural al `.ftl`, que es
    // donde cada idioma decide sus formas. Antes eran `if n == 1` escritos a mano por idioma.

    /// Los mensajes que declaran estas funciones y no el macro, para los tests.
    #[cfg(test)]
    pub(super) const HAND_WRITTEN: &[(&str, &[&str])] = &[
        ("outlinks_title", &["total", "internal", "external"]),
        ("note_external_status", &["n"]),
        ("image_usage_line", &["times", "pages"]),
        ("portfolio_sites_count", &["n"]),
        ("rules_config_note", &["source", "disabled", "changed"]),
        ("portfolio_sites_of", &["fired", "total"]),
        ("portfolio_inconclusive_suffix", &["n"]),
        ("portfolio_pairs_line", &["pairs", "total"]),
    ];

    /// La cabecera de enlaces salientes: «Outlinks (8: 7 internal, 1 external)».
    pub fn outlinks_title(lang: Lang, total: i64, internal: i64, external: i64) -> String {
        let n = |v: i64| super::count(lang, v);
        let (total, internal, external) = (n(total), n(internal), n(external));
        super::format(
            lang,
            "outlinks_title",
            &[("total", &total), ("internal", &internal), ("external", &external)],
        )
    }

    /// El sufijo que separa las externas del recuento de códigos de estado: un 404 ajeno no
    /// es un error del sitio auditado y no debe sumarse como si lo fuera.
    pub fn note_external_status(lang: Lang, n: i64) -> String {
        let n = super::count(lang, n);
        super::format(lang, "note_external_status", &[("n", &n)])
    }

    /// «Incrustada N veces en M páginas», sin decir «1 veces» ni «1 pages».
    pub fn image_usage_line(lang: Lang, times: i64, pages: i64) -> String {
        let (times, pages) = (super::count(lang, times), super::count(lang, pages));
        super::format(lang, "image_usage_line", &[("times", &times), ("pages", &pages)])
    }

    /// "{n} sites" with the plural resolved per language.
    pub fn portfolio_sites_count(lang: Lang, n: usize) -> String {
        let n = super::count_usize(lang, n);
        super::format(lang, "portfolio_sites_count", &[("n", &n)])
    }

    /// "Rule configuration (stored in the crawl file): 2 rules disabled, 1 severity changed."
    pub fn rules_config_note(
        lang: Lang,
        source: impl std::fmt::Display,
        disabled: usize,
        changed: usize,
    ) -> String {
        let (disabled, changed) =
            (super::count_usize(lang, disabled), super::count_usize(lang, changed));
        super::format(
            lang,
            "rules_config_note",
            &[("source", &source), ("disabled", &disabled), ("changed", &changed)],
        )
    }

    /// "9 of 12 sites" — the numerator of the spread table.
    pub fn portfolio_sites_of(lang: Lang, fired: usize, total: usize) -> String {
        let (fired, total) = (super::count_usize(lang, fired), super::count_usize(lang, total));
        super::format(lang, "portfolio_sites_of", &[("fired", &fired), ("total", &total)])
    }

    /// " (2 inconclusive)" — sites where the rule **could not be evaluated**: a truncated or
    /// list-mode crawl does not evaluate the full-graph rules, and counting those sites as
    /// "does not fire here" would be lying (trap 3.1). Empty when every site could answer.
    pub fn portfolio_inconclusive_suffix(lang: Lang, n: usize) -> String {
        if n == 0 {
            return String::new();
        }
        let n = super::count_usize(lang, n);
        super::format(lang, "portfolio_inconclusive_suffix", &[("n", &n)])
    }

    /// "3 of 5 sites have a previous crawl (.prev.sqlite) to compare against." The noun
    /// follows the total and the verb follows the pair count: "1 of 5 sites has".
    pub fn portfolio_pairs_line(lang: Lang, pairs: usize, total: usize) -> String {
        let (pairs, total) = (super::count_usize(lang, pairs), super::count_usize(lang, total));
        super::format(lang, "portfolio_pairs_line", &[("pairs", &pairs), ("total", &total)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Precedencia: flag > CRAWLFORGE_LANG > inglés ─────────────────────────

    #[test]
    fn sin_flag_ni_variable_se_responde_en_ingles() {
        assert_eq!(resolve_pure(None, None).expect("resolve"), Lang::En);
    }

    #[test]
    fn la_variable_de_entorno_gobierna_cuando_no_hay_flag() {
        assert_eq!(resolve_pure(None, Some("es")).expect("resolve"), Lang::Es);
        assert_eq!(resolve_pure(None, Some("ES-es")).expect("resolve"), Lang::Es);
        assert_eq!(resolve_pure(None, Some("en")).expect("resolve"), Lang::En);
    }

    #[test]
    fn el_flag_gana_a_la_variable_de_entorno() {
        assert_eq!(resolve_pure(Some("en"), Some("es")).expect("resolve"), Lang::En);
        assert_eq!(resolve_pure(Some("es"), Some("en")).expect("resolve"), Lang::Es);
    }

    #[test]
    fn un_flag_desconocido_es_un_error_y_no_un_silencio() {
        let err = resolve_pure(Some("xx"), None).expect_err("xx does not exist");
        assert!(err.to_string().contains("xx"), "{err}");
        // El error dice qué valores existen, los diez.
        assert!(err.to_string().contains("en, es, de, fr, it, nl, tr, pt, ja, pl"), "{err}");
    }

    #[test]
    fn los_diez_idiomas_se_aceptan_en_el_flag() {
        for (flag, lang) in [("de", Lang::De), ("pt-BR", Lang::Pt), ("ja", Lang::Ja), ("pl", Lang::Pl)] {
            assert_eq!(resolve_pure(Some(flag), None).expect("resolve"), lang);
        }
    }

    #[test]
    fn una_variable_de_entorno_rota_cae_al_ingles_sin_romper_el_comando() {
        // Una CRAWLFORGE_LANG con errata no debe inutilizar todos los comandos de la máquina.
        assert_eq!(resolve_pure(None, Some("klingon")).expect("resolve"), Lang::En);
    }

    // ── Números por idioma ───────────────────────────────────────────────────

    #[test]
    fn los_millares_se_separan_segun_el_idioma() {
        assert_eq!(group_thousands(Lang::En, 3816), "3,816");
        assert_eq!(group_thousands(Lang::Es, 3816), "3.816");
        assert_eq!(group_thousands(Lang::En, 1_234_567), "1,234,567");
        assert_eq!(group_thousands(Lang::Es, 1_234_567), "1.234.567");
        assert_eq!(group_thousands(Lang::Es, 999), "999");
        assert_eq!(group_thousands(Lang::Es, 0), "0");
    }

    #[test]
    fn el_decimal_acompana_al_millar() {
        // `3.816` de millares junto a `1.5` de decimal sería ambiguo: o los dos o ninguno.
        assert_eq!(decimal1(Lang::En, 1.55), "1.6");
        assert_eq!(decimal1(Lang::Es, 1.55), "1,6");
        assert_eq!(decimal1(Lang::Es, 12.0), "12,0");
    }

    // ── Cabeceras y textos ───────────────────────────────────────────────────

    #[test]
    fn las_cabeceras_miden_lo_mismo_en_los_dos_idiomas() {
        for title in [
            (msg::results_title(Lang::En), msg::results_title(Lang::Es)),
            (msg::worse_title(Lang::En), msg::worse_title(Lang::Es)),
            (msg::diff_title(Lang::En), msg::diff_title(Lang::Es)),
        ] {
            let en = section(&title.0);
            let es = section(&title.1);
            assert_eq!(en.chars().count(), es.chars().count(), "{en} / {es}");
        }
    }

    #[test]
    fn las_dos_columnas_dicen_cosas_distintas_donde_deben() {
        // Caza el copia-y-pega de dejar el inglés en `locales/es/cli.ftl`. Solo mensajes con
        // traducción real: los que son idénticos a propósito («robots.txt») no entran.
        for (en, es) in [
            (msg::results_title(Lang::En), msg::results_title(Lang::Es)),
            (msg::worse_title(Lang::En), msg::worse_title(Lang::Es)),
            (msg::no_findings(Lang::En), msg::no_findings(Lang::Es)),
            (msg::error_no_urls_fetched(Lang::En), msg::error_no_urls_fetched(Lang::Es)),
            (msg::gate_pass(Lang::En), msg::gate_pass(Lang::Es)),
            (msg::error_store_is_diff(Lang::En, "x"), msg::error_store_is_diff(Lang::Es, "x")),
            (msg::resume_config_note(Lang::En), msg::resume_config_note(Lang::Es)),
            (msg::resume_robots_not_inherited(Lang::En), msg::resume_robots_not_inherited(Lang::Es)),
            // Un aviso que dice qué pasa y no qué hacer deja al usuario donde estaba.
            (msg::error_resume_done(Lang::En, "x"), msg::error_resume_done(Lang::Es, "x")),
            (msg::interrupted_saved(Lang::En, "x"), msg::interrupted_saved(Lang::Es, "x")),
            (msg::warn_wal_kept(Lang::En, "x"), msg::warn_wal_kept(Lang::Es, "x")),
            (msg::error_store_locked(Lang::En, "x"), msg::error_store_locked(Lang::Es, "x")),
            (msg::interrupt_after_done(Lang::En), msg::interrupt_after_done(Lang::Es)),
        ] {
            assert_ne!(en, es, "the Spanish text must not be the English copied over");
        }

        for (aviso, pista) in [
            (msg::resume_robots_not_inherited(Lang::En), "Re-run the original crawl"),
            (msg::resume_robots_not_inherited(Lang::Es), "Vuelve a lanzar el rastreo"),
        ] {
            assert!(aviso.contains(pista), "the notice must say how to get it back: {aviso}");
        }
    }

    #[test]
    fn los_mensajes_interpolan_sus_argumentos_en_los_dos_idiomas() {
        for lang in [Lang::En, Lang::Es] {
            let texto = msg::error_dead_seed(lang, "https://ejemplo.es/", msg::cause_dns(lang));
            assert!(texto.contains("https://ejemplo.es/"), "{texto}");
            let aviso = msg::warn_base_mismatch(lang, 3, 3, "https://a.es/", "https://b.es/");
            assert!(aviso.contains("--base https://a.es/"), "{aviso}");
        }
    }

    #[test]
    fn los_plurales_de_la_ficha_se_resuelven_en_los_dos_idiomas() {
        // «1 externos» y «embedded 1 times on 1 pages» eran los plurales sin resolver que
        // cazó la revisión. Hoy los resuelven los selectores de `cli.ftl`.
        assert_eq!(
            msg::outlinks_title(Lang::Es, 8, 7, 1),
            "Enlaces salientes (8: 7 internos, 1 externo)"
        );
        assert_eq!(
            msg::outlinks_title(Lang::Es, 3, 1, 2),
            "Enlaces salientes (3: 1 interno, 2 externos)"
        );
        assert_eq!(
            msg::outlinks_title(Lang::En, 3, 2, 1),
            "Outlinks (3: 2 internal, 1 external)"
        );
        assert_eq!(msg::image_usage_line(Lang::En, 1, 1), "embedded 1 time on 1 page");
        assert_eq!(msg::image_usage_line(Lang::En, 3, 2), "embedded 3 times on 2 pages");
        assert_eq!(msg::image_usage_line(Lang::Es, 1, 1), "incrustada 1 vez en 1 página");
        assert_eq!(msg::image_usage_line(Lang::Es, 3, 2), "incrustada 3 veces en 2 páginas");
        assert_eq!(msg::note_external_status(Lang::Es, 1), "+1 externa");
        assert_eq!(msg::note_external_status(Lang::Es, 2), "+2 externas");
        assert_eq!(msg::note_external_status(Lang::En, 1), "+1 external");
    }

    #[test]
    fn the_portfolio_plurals_resolve_in_both_languages() {
        // The portfolio counters carry plural selectors for the same reason as the URL card's:
        // "1 sitios" and "1 sites" are the shortcut the review caught.
        assert_eq!(msg::portfolio_sites_count(Lang::En, 1), "1 site");
        assert_eq!(msg::portfolio_sites_count(Lang::En, 5), "5 sites");
        assert_eq!(msg::portfolio_sites_count(Lang::Es, 1), "1 sitio");
        assert_eq!(msg::portfolio_sites_count(Lang::Es, 5), "5 sitios");
        assert_eq!(msg::portfolio_sites_of(Lang::En, 9, 12), "9 of 12 sites");
        assert_eq!(msg::portfolio_sites_of(Lang::Es, 9, 12), "9 de 12 sitios");
        assert_eq!(msg::portfolio_inconclusive_suffix(Lang::En, 0), "");
        assert_eq!(msg::portfolio_inconclusive_suffix(Lang::En, 2), " (2 inconclusive)");
        assert_eq!(msg::portfolio_inconclusive_suffix(Lang::Es, 1), " (1 no concluyente)");
        assert_eq!(msg::portfolio_inconclusive_suffix(Lang::Es, 2), " (2 no concluyentes)");
        assert!(msg::portfolio_pairs_line(Lang::En, 1, 5).starts_with("1 of 5 sites has"));
        assert!(msg::portfolio_pairs_line(Lang::En, 3, 5).starts_with("3 of 5 sites have"));
        assert!(msg::portfolio_pairs_line(Lang::Es, 1, 5).starts_with("1 de 5 sitios tiene "));
        assert!(msg::portfolio_pairs_line(Lang::Es, 3, 5).starts_with("3 de 5 sitios tienen"));
    }

    #[test]
    fn las_notas_de_completitud_estan_en_los_dos_idiomas() {
        for (en, es) in [
            (msg::external_check_disabled(Lang::En), msg::external_check_disabled(Lang::Es)),
            (msg::external_checked_note(Lang::En, 3), msg::external_checked_note(Lang::Es, 3)),
            (msg::external_never_checked(Lang::En, 3), msg::external_never_checked(Lang::Es, 3)),
            (msg::rules_not_evaluated(Lang::En, "X"), msg::rules_not_evaluated(Lang::Es, "X")),
            (msg::state_note_external_checked(Lang::En), msg::state_note_external_checked(Lang::Es)),
            (msg::error_store_missing(Lang::En, "x"), msg::error_store_missing(Lang::Es, "x")),
            (msg::error_invalid_seed(Lang::En, "x"), msg::error_invalid_seed(Lang::Es, "x")),
            (msg::hint_who_links(Lang::En), msg::hint_who_links(Lang::Es)),
        ] {
            assert_ne!(en, es, "the Spanish text must not be the English copied over");
        }
        // El error de fichero dice qué comando lo genera, como promete el manual §5.
        assert!(msg::error_store_missing(Lang::En, "x").contains("crawlforge crawl"));
        assert!(msg::error_store_missing(Lang::Es, "x").contains("crawlforge crawl"));
    }

    // ── Los ficheros `.ftl` frente al código ─────────────────────────────────
    //
    // Estos tests son lo que antes daba el compilador: una entrada sin texto, un texto sin
    // entrada, una variable que no coincide o una traducción a medias. Listan todos los
    // problemas de una vez, con fichero y clave, para que quien traduce tenga la lista entera.

    /// Todo lo que el código pide a `cli.ftl`: el macro, las funciones con plural, la severidad y
    /// los motivos de no indexabilidad.
    fn declared() -> Vec<(&'static str, Vec<&'static str>)> {
        msg::DECLARED
            .iter()
            .chain(msg::HAND_WRITTEN)
            .map(|(k, a)| (*k, a.to_vec()))
            .chain(
                SEVERITY_KEYS
                    .iter()
                    .chain(&REASON_KEYS)
                    .chain(&EXCLUSION_KEYS)
                    .map(|k| (*k, Vec::new())),
            )
            .collect()
    }

    #[test]
    fn every_declared_message_has_its_english_text_with_the_same_variables() {
        let en = CLI_TEXTS.source(Lang::En);
        let mut problems = Vec::new();
        for (key, args) in declared() {
            let mut args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            args.sort();
            match l10n::message_variables(en, key) {
                None => problems.push(format!("locales/en/cli.ftl: {key}: missing")),
                Some(vars) if vars != args => problems.push(format!(
                    "locales/en/cli.ftl: {key}: variables {vars:?}, the code passes {args:?}"
                )),
                Some(_) => {}
            }
        }
        assert!(problems.is_empty(), "{} problem(s):\n{}", problems.len(), problems.join("\n"));
    }

    #[test]
    fn english_has_no_message_the_code_does_not_declare() {
        // A text nobody calls is one a translator translates for nothing.
        let declared: Vec<&str> = declared().into_iter().map(|(k, _)| k).collect();
        let sobran: Vec<String> = l10n::message_ids(CLI_TEXTS.source(Lang::En))
            .into_iter()
            .filter(|id| !declared.contains(&id.as_str()))
            .collect();
        assert!(sobran.is_empty(), "locales/en/cli.ftl has messages no code uses: {sobran:?}");
        let problems = l10n::check_translation("locales/en/cli.ftl", CLI_TEXTS.source(Lang::En), CLI_TEXTS.source(Lang::En));
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn every_translated_cli_file_is_complete() {
        // A folder whose `cli.ftl` has no messages is a language not translated yet; one with
        // messages must have all of English's, with the same variables. And a language is
        // translated in both files or in neither: half the output in German and half in English
        // is worse than either.
        let en = CLI_TEXTS.source(Lang::En);
        let mut problems = Vec::new();
        for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
            let src = CLI_TEXTS.source(lang);
            if l10n::has_messages(src) {
                problems.extend(l10n::check_translation(&format!("locales/{}/cli.ftl", lang.code()), en, src));
            }
            let cli = CLI_TEXTS.is_translated(lang);
            let rules = crawlforge_rules::RULE_TEXTS.is_translated(lang);
            if cli != rules {
                problems.push(format!(
                    "locales/{}/: cli.ftl is {}translated but rules.ftl is {}: translate both",
                    lang.code(),
                    if cli { "" } else { "not " },
                    if rules { "" } else { "not" },
                ));
            }
        }
        assert!(problems.is_empty(), "{} problem(s):\n{}", problems.len(), problems.join("\n"));
    }

    #[test]
    fn spanish_stays_translated() {
        assert!(CLI_TEXTS.is_translated(Lang::Es));
        assert!(crawlforge_rules::RULE_TEXTS.is_translated(Lang::Es));
    }

    #[test]
    fn an_untranslated_language_speaks_english_numbers_included() {
        // Until its folder is filled, a language selected with --lang shows English, and English
        // numbers: German thousands (`3.816`) inside an English sentence would read as decimals.
        for lang in Lang::ALL.into_iter().filter(|l| !CLI_TEXTS.is_translated(*l)) {
            assert_eq!(effective(lang), Lang::En);
            assert_eq!(group_thousands(lang, 3816), "3,816", "{lang:?}");
            assert_eq!(decimal1(lang, 1.5), "1.5", "{lang:?}");
            assert_eq!(msg::results_title(lang), "Results", "{lang:?}");
            assert_eq!(msg::outlinks_title(lang, 3, 2, 1), "Outlinks (3: 2 internal, 1 external)");
        }
    }

    #[test]
    fn a_message_never_comes_out_empty_or_as_its_key() {
        for lang in Lang::ALL {
            for (key, args) in declared() {
                let values: Vec<String> = args.iter().map(|a| format!("<{a}>")).collect();
                let refs: Vec<(&str, &dyn Display)> =
                    args.iter().zip(&values).map(|(a, v)| (*a, v as &dyn Display)).collect();
                let text = format(lang, key, &refs);
                assert!(!text.trim().is_empty(), "{lang:?} {key}");
                assert_ne!(text, key, "{lang:?} {key}");
                for v in &values {
                    assert!(text.contains(v.as_str()), "{lang:?} {key} lost {v}: {text}");
                }
            }
        }
    }

    #[test]
    fn the_template_line_picks_singular_or_plural() {
        assert_eq!(msg::template_issues(Lang::En, 1, "18,089"), "1 template issue (18,089 pages)");
        assert_eq!(msg::template_issues(Lang::En, 3, "18,089"), "3 template issues (18,089 pages)");
        assert_eq!(msg::template_issues(Lang::Es, 1, "18.089"), "1 problema de plantilla (18.089 páginas)");
        assert_eq!(msg::template_issues(Lang::Es, 2, "5"), "2 problemas de plantilla (5 páginas)");
    }

    #[test]
    fn grouped_counts_still_select_the_plural() {
        // The count reaches Fluent formatted ("1.234") and must come out identical and still
        // pick the plural form.
        assert_eq!(msg::image_usage_line(Lang::Es, 1234, 1), "incrustada 1.234 veces en 1 página");
        assert_eq!(msg::note_external_status(Lang::En, 12_345), "+12,345 external");
    }

    #[test]
    fn english_counts_agree_in_number() {
        // «1 new finding(s)», «1 URLs», «1 external links were»: English had no selectors
        // where the other languages already had theirs.
        let en = Lang::En;
        assert!(msg::gate_fail_detail(en, "1", "high", "high").starts_with("1 new finding ["));
        assert!(msg::gate_fail_detail(en, count(en, 1234), "high", "high")
            .starts_with("1,234 new findings ["));
        assert_eq!(msg::crawl_finished(en, "1", "2"), "Crawl finished: 1 URL in 2 s");
        assert_eq!(msg::resume_finished(en, "1", "2"), "Crawl resumed and completed: 1 more URL in 2 s");
        assert!(msg::external_unchecked(en, "1").starts_with("1 external link was not"));
        assert!(msg::external_offline(en, "1").starts_with("1 external link was not"));
        assert!(msg::external_unregistered(en, "1").starts_with("1 external link was not"));
        assert_eq!(msg::external_never_checked(en, "1"), "1 external link was recorded but never checked.");
        assert_eq!(msg::external_checked_note(en, "1"), "1 external link checked.");
        assert!(msg::crawl_rate_limited_note(en, "1", "x").contains("1 page kept answering"));
        assert!(msg::report_rate_limited_note(en, "1").contains("1 page kept answering"));
        assert!(msg::crawl_throttled_note(en, "1").contains("halved once."));
        assert!(msg::crawl_throttled_note(en, "3").contains("halved 3 times."));
        assert!(msg::warn_unknown_change_types(en, "1", "x").starts_with("1 change of a type"));
        assert_eq!(msg::diff_crawl_counts(en, "1", "1"), "1 URL, 1 finding");
        assert_eq!(msg::chars_suffix(en, "1"), "1 char");
        assert_eq!(msg::diff_crawl_counts(en, count(en, 1234), "0"), "1,234 URLs, 0 findings");
    }

    #[test]
    fn hand_written_counts_reach_fluent_grouped() {
        // A raw `usize` of four digits was not a number for Fluent («1234» is not how German
        // writes it), so it showed ungrouped and fell to the default plural form.
        assert_eq!(msg::portfolio_sites_count(Lang::De, 1234), "1.234 Websites");
        assert_eq!(msg::portfolio_sites_of(Lang::En, 1000, 1001), "1,000 of 1,001 sites");
        assert_eq!(msg::portfolio_inconclusive_suffix(Lang::En, 2000), " (2,000 inconclusive)");
        assert!(msg::portfolio_pairs_line(Lang::En, 1000, 1200).starts_with("1,000 of 1,200 sites have"));
        assert!(msg::rules_config_note(Lang::En, "x", 1000, 0).contains("1,000"));
        // Polish 1002 is `few` («witryny»); as raw text it fell to the default («witryn»).
        assert_eq!(msg::portfolio_sites_count(Lang::Pl, 1002), "1\u{A0}002 witryny");
    }

    #[test]
    fn la_cabecera_y_el_relleno_cuentan_columnas_de_terminal() {
        // Un ideograma ocupa dos columnas: «比較» mide 4 y la cabecera debe acabar donde las demás.
        assert_eq!(display_width("比較"), 4);
        assert_eq!(display_width(&section("比較")), display_width(&section("abcd")));
        assert_eq!(pad("比較", 6), "比較  ");
        assert_eq!(pad("Crítico", 9), "Crítico  ");
        assert_eq!(pad("demasiado largo", 3), "demasiado largo");
        assert_eq!(pad_left("比較", 6), "  比較");
        assert_eq!(pad_left("URLs", 6), format!("{:>6}", "URLs"));
    }

    #[test]
    fn every_exclusion_reason_the_engine_stores_has_a_translation() {
        use crawlforge_core::frontier::ExclusionReason as R;
        // The match is the guard: a new variant does not compile here until someone decides
        // how it reads, instead of reaching the report as a raw identifier.
        let all = [
            R::Robots, R::Nofollow, R::Depth, R::Pattern, R::Limit, R::LocalNetwork,
            R::ExternalCheckOff, R::NoNetwork,
        ];
        for r in all {
            match r {
                R::Robots | R::Nofollow | R::Depth | R::Pattern | R::Limit | R::LocalNetwork
                | R::ExternalCheckOff | R::NoNetwork => {}
            }
        }
        let mut keys: Vec<&str> = all
            .iter()
            .map(|r| exclusion_key(r.as_str()).unwrap_or_else(|| panic!("{} has no key", r.as_str())))
            .collect();
        keys.sort_unstable();
        let mut expected = EXCLUSION_KEYS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "EXCLUSION_KEYS must list exactly the mapped keys");
    }

    #[test]
    fn the_exclusion_reason_is_translated_and_the_unknown_shown_as_is() {
        assert_eq!(exclusion_reason(Lang::Es, "pattern"), "patrón de URL");
        assert_eq!(exclusion_reason(Lang::En, "external_check_off"), "external check off");
        assert_eq!(exclusion_reason(Lang::Ja, "robots"), "robots.txt");
        assert_eq!(exclusion_reason(Lang::Es, "algo_nuevo"), "algo_nuevo");
    }

    #[test]
    fn la_severidad_se_traduce_y_lo_desconocido_se_ensena_tal_cual() {
        assert_eq!(severity_word(Lang::Es, "critical"), "crítico");
        assert_eq!(severity_word(Lang::En, "critical"), "critical");
        assert_eq!(severity_word(Lang::Es, "rarisimo"), "rarisimo");
        assert_eq!(severity_label(Lang::Es, Severity::Info), "Informativo");
        assert_eq!(severity_label(Lang::En, Severity::Critical), "Critical");
    }
}
