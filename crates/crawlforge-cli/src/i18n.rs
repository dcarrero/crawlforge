//! El catálogo de cadenas de la CLI, con inglés y español.
//!
//! # Por qué así y no de otra forma
//!
//! **El inglés es el idioma de origen y el español una traducción** (`CONVENTIONS.md §4`), y habrá
//! más idiomas. El diseño que hay que evitar es el de los campos `name_es`/`name_en` de
//! `RuleMeta`: un par de campos por idioma no escala al tercero. Aquí cada mensaje es **una
//! entrada con una columna por idioma**, generada por el macro [`messages!`]:
//!
//! - **Añadir un idioma es añadir una columna**: una variante en [`Lang`], una rama por entrada.
//!   Ningún `println!` se toca.
//! - **Una cadena sin traducir no compila.** El macro expande a un `match` exhaustivo sobre
//!   `Lang`: si `Lang` gana una variante y una entrada no tiene su texto, el compilador señala
//!   la entrada exacta. No hay hueco que descubrir en producción.
//! - **Un placeholder desusado no compila.** Los argumentos se pasan como argumentos con nombre
//!   de `format!`, y `format!` rechaza un argumento con nombre que el texto no usa. Las dos
//!   columnas quedan obligadas a usar los mismos datos.
//! - **Sin dependencias.** `fluent` y compañía resuelven pluralización y géneros de cuarenta
//!   idiomas; para dos idiomas y unos cientos de cadenas, una tabla en Rust da lo mismo con
//!   coste cero y errores en compilación en vez de en ejecución.
//!
//! # Qué NO pasa por aquí
//!
//! IDs de regla, URLs, rutas de fichero, códigos de estado, valores de columna de la base de
//! datos (`max_urls`, `noindex`, `critical` como token de `--fail-on`) y los nombres de los
//! comandos: son identificadores, no prosa. Tampoco los textos del catálogo de reglas, que
//! viven en `crawlforge-rules` para que la CLI y las apps digan lo mismo.
//!
//! # Resolución del idioma
//!
//! `--lang` gana; sin él, la variable de entorno `CRAWLFORGE_LANG`; sin ninguna, inglés.
//! Un `--lang` desconocido es un error (una errata debe doler); un `CRAWLFORGE_LANG`
//! desconocido se ignora y se cae al inglés, porque una variable de entorno rota no debe
//! inutilizar todos los comandos de la máquina.

use anyhow::{bail, Result};
use crawlforge_rules::{Lang, Severity};
use std::sync::OnceLock;

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
            bail!("language not recognised: {flag}. Available: en and es");
        };
        return Ok(lang);
    }
    Ok(env.and_then(Lang::parse).unwrap_or_default())
}

// ───────────────────────────────────────────────────────── Formato de números

/// Separador de millares según el idioma: `3,816` en inglés, `3.816` en español.
pub fn group_thousands(lang: Lang, n: u64) -> String {
    let separator = match lang {
        Lang::En => ',',
        Lang::Es => '.',
    };
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(separator);
        }
        out.push(c);
    }
    out
}

/// [`group_thousands`] para los recuentos `i64` que devuelve SQLite. Nunca son negativos;
/// si alguno lo fuera, se muestra sin agrupar antes que mentir.
pub fn count(lang: Lang, n: i64) -> String {
    u64::try_from(n).map_or_else(|_| n.to_string(), |v| group_thousands(lang, v))
}

/// Un número con un decimal, con la coma decimal del español: `12.3` / `12,3`.
///
/// Va a la vez que [`group_thousands`]: un `3.816` de millares junto a un `1.5` de decimal
/// sería ambiguo, así que o se localizan los dos o ninguno.
pub fn decimal1(lang: Lang, v: f64) -> String {
    let text = format!("{v:.1}");
    match lang {
        Lang::En => text,
        Lang::Es => text.replace('.', ","),
    }
}

// ──────────────────────────────────────────────────────── Cabeceras y etiquetas

/// Ancho de las cabeceras de sección de la terminal (`── Título ────…`).
const SECTION_WIDTH: usize = 45;

/// Una cabecera de sección con el ancho de siempre, rellene lo que rellene el título.
/// Cuenta caracteres y no bytes: «Comparación» mide 11, no 12.
pub fn section(title: &str) -> String {
    let used = 3 + title.chars().count() + 1;
    format!("── {title} {}", "─".repeat(SECTION_WIDTH.saturating_sub(used)))
}

/// Etiqueta de severidad con inicial mayúscula, para informes.
pub fn severity_label(lang: Lang, s: Severity) -> &'static str {
    match (s, lang) {
        (Severity::Critical, Lang::Es) => "Crítico",
        (Severity::High, Lang::Es) => "Alto",
        (Severity::Medium, Lang::Es) => "Medio",
        (Severity::Low, Lang::Es) => "Bajo",
        (Severity::Info, Lang::Es) => "Informativo",
        (Severity::Critical, Lang::En) => "Critical",
        (Severity::High, Lang::En) => "High",
        (Severity::Medium, Lang::En) => "Medium",
        (Severity::Low, Lang::En) => "Low",
        (Severity::Info, Lang::En) => "Info",
    }
}

/// Severidad en minúsculas para columnas de tabla, desde el valor tal como está en la base de
/// datos. Un valor desconocido se devuelve tal cual: enseñar el dato crudo es mejor que ocultarlo.
pub fn severity_word(lang: Lang, db_value: &str) -> String {
    match (db_value, lang) {
        ("critical", Lang::Es) => "crítico".to_string(),
        ("high", Lang::Es) => "alto".to_string(),
        ("medium", Lang::Es) => "medio".to_string(),
        ("low", Lang::Es) => "bajo".to_string(),
        ("info", Lang::Es) => "info".to_string(),
        _ => db_value.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────── El macro

/// Genera una función por mensaje: `nombre(lang, args…) -> String`.
///
/// Cada entrada declara sus argumentos y una columna por idioma. El `match` que expande es
/// exhaustivo sobre [`Lang`], así que **un idioma nuevo sin traducir no compila**, y los
/// argumentos se pasan con nombre a `format!`, así que **un texto que no use un argumento
/// tampoco compila**. Las garantías que en un sistema de ficheros `.po` serían un test aquí
/// las da el compilador.
macro_rules! messages {
    ($(
        $(#[$meta:meta])*
        $name:ident ( $($arg:ident),* ) {
            en: $en:literal,
            es: $es:literal $(,)?
        }
    )*) => {
        $(
            $(#[$meta])*
            #[allow(clippy::useless_format)]
            pub fn $name(lang: Lang $(, $arg: impl ::std::fmt::Display)*) -> String {
                match lang {
                    Lang::En => format!($en $(, $arg = $arg)*),
                    Lang::Es => format!($es $(, $arg = $arg)*),
                }
            }
        )*
    };
}

/// Todos los mensajes de la CLI, agrupados por pantalla.
///
/// Convención: los textos no llevan sangría ni relleno de columnas —eso es maquetación y vive
/// en el `println!` que los usa—, salvo cuando el mensaje es multilínea y la sangría forma
/// parte del propio texto.
pub mod msg {
    use super::Lang;

    // ── Resumen del rastreo (`report.rs`) ────────────────────────────────────
    messages! {
        crawl_finished(urls, secs) {
            en: "Crawl finished: {urls} URLs in {secs} s",
            es: "Rastreo terminado: {urls} URLs en {secs} s",
        }
        crawl_failed_suffix(n) {
            en: " ({n} failed)",
            es: " ({n} con error)",
        }
        crawl_truncated(reason) {
            en: "Crawl truncated by {reason}. The findings shown are those found up to that point.",
            es: "Rastreo truncado por {reason}. Los hallazgos mostrados son los encontrados hasta ese punto.",
        }
        // `list_mode` no es un corte: el rastreo hizo exactamente lo que se le pidió, así que
        // decirle al usuario «tu rastreo se truncó» sería mentir. Lo que sí hay que decirle es
        // la consecuencia de auditar un conjunto y no un sitio.
        crawl_list_mode_note() {
            en: "List crawl: only the URLs you provided were audited. Rules that need the \
                 site's complete link graph (orphans, incoming links, depth) are not evaluated.",
            es: "Rastreo en modo lista: solo se auditaron las URLs que diste. Las reglas que \
                 necesitan el grafo completo del sitio (huérfanas, enlaces entrantes, \
                 profundidad) no se evalúan.",
        }
        // El tope de externas no trunca el rastreo del sitio —ese es `crawl_truncated`— pero
        // dejar enlaces sin comprobar en silencio haría que el informe pareciera completo sin
        // serlo, así que se dice cuántos quedaron fuera.
        external_unchecked(n) {
            en: "{n} external links were not checked: the max_external cap was reached. Raise it with --max-external.",
            es: "{n} enlaces externos quedaron sin comprobar: se alcanzó el tope max_external. Súbelo con --max-external.",
        }
        // El otro tope de externas: el del registro. Estas no están en el fichero, así que sin
        // este aviso el informe no tendría ni una fila que delatara que faltan.
        external_unregistered(n) {
            en: "{n} external links were not even recorded: the max_external_urls cap was reached. \
                 A single page linking to hundreds of thousands of hosts is what this cap is for.",
            es: "{n} enlaces externos no llegaron ni a registrarse: se alcanzó el tope \
                 max_external_urls. Una sola página que enlace a cientos de miles de hosts es \
                 justo lo que este tope acota.",
        }
        // La variante de fichero para un rastreo cortado: aquí no se puede afirmar que el
        // tope fuera el motivo — un truncado deja externas sin sondear igualmente.
        external_never_checked(n) {
            en: "{n} external links were recorded but never checked.",
            es: "{n} enlaces externos quedaron registrados pero sin comprobar.",
        }
        // La otra mitad de la honestidad de las externas: cuando la comprobación estaba
        // apagada, «cero enlaces externos rotos» no significa que no los haya.
        external_check_disabled() {
            en: "External links were not checked (--no-external-check): the absence of broken \
                 external links in this report does not mean there are none.",
            es: "Los enlaces externos no se comprobaron (--no-external-check): que este informe \
                 no traiga enlaces externos rotos no significa que no los haya.",
        }
        // El resumen del rastreo dice cuántas externas se sondearon aunque todo fuera bien:
        // es el tiempo que el cierre atribuía en silencio al rastreo del sitio.
        external_checked_note(n) {
            en: "{n} external links checked.",
            es: "{n} enlaces externos comprobados.",
        }
        // Qué reglas calló el truncado, por su ID: sin la lista, «las reglas que necesitan el
        // grafo» obliga a adivinar cuáles son.
        rules_not_evaluated(rules) {
            en: "Rules that need the complete link graph were not evaluated: {rules}.",
            es: "Las reglas que necesitan el grafo de enlaces completo no se evaluaron: {rules}.",
        }
        results_title() {
            en: "Results",
            es: "Resultados",
        }
        label_urls() {
            en: "URLs",
            es: "URLs",
        }
        label_html_pages() {
            en: "HTML pages",
            es: "Páginas HTML",
        }
        label_links() {
            en: "Links",
            es: "Enlaces",
        }
        note_internal(n) {
            en: "{n} internal",
            es: "{n} internas",
        }
        note_indexable(n) {
            en: "{n} indexable",
            es: "{n} indexables",
        }
        heading_status_codes() {
            en: "Status codes",
            es: "Códigos de estado",
        }
        label_no_response() {
            en: "no response",
            es: "sin respuesta",
        }
        note_request_failed() {
            en: "the request failed",
            es: "la petición falló",
        }
        label_never_requested() {
            en: "Never requested",
            es: "No solicitadas",
        }
        note_never_requested() {
            en: "(discovered, but the crawl stopped before fetching them)",
            es: "(descubiertas, pero el rastreo se detuvo antes de pedirlas)",
        }
        // Filas internas `skipped` sin motivo: en modo lista, los enlaces que apuntan fuera
        // de la lista. Explican por qué «URLs 21» cuando se dieron 5.
        label_out_of_scope() {
            en: "Out of scope",
            es: "Fuera del alcance",
        }
        note_out_of_scope() {
            en: "(linked from audited pages, but outside this crawl's scope)",
            es: "(enlazadas desde páginas auditadas, pero fuera del alcance de este rastreo)",
        }
        heading_non_indexable() {
            en: "Non-indexable, why",
            es: "No indexables, por qué",
        }
        heading_findings() {
            en: "Findings",
            es: "Hallazgos",
        }
        no_findings() {
            en: "No findings.",
            es: "Sin hallazgos.",
        }
        heading_excluded() {
            en: "Excluded",
            es: "Excluidas",
        }
        // ── Colapso de plantilla ─────────────────────────────────────────────
        //
        // Un defecto de la plantilla aparece en el 90% de las páginas y se arregla una vez:
        // el informe dice «1 problema de plantilla (18.089 páginas)» en vez de un recuento
        // que tapa todo lo demás. Las filas de `issues` no se tocan: esto es presentación.
        one_template_issue(pages) {
            en: "1 template issue ({pages} pages)",
            es: "1 problema de plantilla ({pages} páginas)",
        }
        n_template_issues(k, pages) {
            en: "{k} template issues ({pages} pages)",
            es: "{k} problemas de plantilla ({pages} páginas)",
        }
        // «Hallazgos» y no «páginas»: la misma página puede estar dentro del grupo de
        // plantilla (el logo) y traer además un hallazgo propio (su imagen destacada), así
        // que el resto no son páginas nuevas, son hallazgos fuera del grupo.
        plus_more_findings(n) {
            en: " + {n} more findings",
            es: " + {n} hallazgos más",
        }
        example_urls(urls) {
            en: "e.g. {urls}",
            es: "p. ej. {urls}",
        }
        // ── Reformulación de reglas dominantes ───────────────────────────────
        //
        // El caso que el colapso de plantilla no cubre: hallazgos masivos **ciertos y sin causa
        // común hashable** (202.392 páginas profundas de un archivo sin atajos, 173.654 títulos
        // largos de la misma plantilla). El recuento se conserva —nada se oculta— y se le añade
        // la cuota del sitio, que es lo que lo convierte en un problema de arquitectura y no en
        // una lista de tareas. El criterio es `crawlforge_rules::is_pervasive`.
        pervasive_note(pct) {
            en: "{pct}% of the site",
            es: "{pct}% del sitio",
        }
        // La forma del problema de profundidad, dicha una vez: es la línea que sustituye a
        // «202,392» a secas. Los números llegan ya formateados con el millar del idioma.
        deep_pages_summary(pages, max, pct, low, high, deepest) {
            en: "{pages} pages deeper than {max} clicks — {pct}% of the site \
                 (typical depth {low}–{high}, deepest {deepest})",
            es: "{pages} páginas a más de {max} clics — {pct}% del sitio \
                 (profundidad típica {low}–{high}, máxima {deepest})",
        }
        rule_deep_sorted() {
            en: "Sorted by click depth, deepest first; the first column is clicks from home.",
            es: "Ordenadas por profundidad de clic, la más hundida primero; la primera columna \
                 son los clics desde la portada.",
        }
        hint_full_lists(cmd) {
            en: "Full URL list for a rule: {cmd}",
            es: "Lista completa de URLs de una regla: {cmd}",
        }
        // ── Listado de una regla (`report --rule`) ───────────────────────────
        rule_affected_urls(n) {
            en: "{n} affected URLs",
            es: "{n} URLs afectadas",
        }
        rule_template_group(n) {
            en: "Template group — {n} pages",
            es: "Grupo de plantilla — {n} páginas",
        }
        rule_group_cause(detail) {
            en: "cause: {detail}",
            es: "causa: {detail}",
        }
        rule_other_pages(n) {
            en: "Other affected pages — {n}",
            es: "Otras páginas afectadas — {n}",
        }
        rule_no_findings(rule) {
            en: "No findings for {rule} in this crawl.",
            es: "Sin hallazgos de {rule} en este rastreo.",
        }
        // El paso siguiente de un enlace roto: se arregla en la página que lo enlaza, y ese
        // dato lo da `inspect`. El comando lo imprime el llamador, listo para copiar.
        hint_who_links() {
            en: "A broken link is fixed on the page that links to it. Who links to a URL, \
                 with its anchor text:",
            es: "Un enlace roto se arregla en la página que lo enlaza. Quién enlaza a una \
                 URL, con su texto de ancla:",
        }
        error_unknown_rule(rule) {
            en: "{rule} is not a rule ID. The catalog is listed by `crawlforge rules`.",
            es: "«{rule}» no es un ID de regla. El catálogo lo lista `crawlforge rules`.",
        }
        error_all_blocked_by_robots() {
            en: "robots.txt blocks every URL: there is nothing to audit. \
                 If the site is yours, you can retry with --ignore-robots.",
            es: "robots.txt bloquea todas las URLs: no hay nada que auditar. \
                 Si el sitio es tuyo, puedes reintentarlo con --ignore-robots.",
        }
        error_all_excluded_by_pattern() {
            en: "every URL was skipped by your --exclude/--include patterns: there is \
                 nothing to audit. Check the patterns.",
            es: "los patrones de --exclude/--include han descartado todas las URLs: no hay \
                 nada que auditar. Revisa los patrones.",
        }
        error_no_urls_fetched() {
            en: "the crawl fetched no URLs. Check the target.",
            es: "el rastreo no obtuvo ninguna URL. Revisa el objetivo.",
        }
        error_dead_seed(url, cause) {
            en: "could not connect to {url} ({cause}). Check the URL.",
            es: "no se pudo conectar con {url} ({cause}). Revisa la URL.",
        }
        cause_dns() {
            en: "the domain does not exist or does not resolve",
            es: "el dominio no existe o no resuelve",
        }
        cause_tls() {
            en: "the TLS certificate is not valid",
            es: "el certificado TLS no es válido",
        }
        cause_timeout() {
            en: "the server did not answer in time",
            es: "el servidor no respondió a tiempo",
        }
        cause_connection() {
            en: "connection refused",
            es: "conexión rechazada",
        }
        cause_no_response() {
            en: "no response",
            es: "sin respuesta",
        }
        warn_base_mismatch(n, total, origin, base) {
            en: "Warning: {n} of {total} canonicals point at {origin}, not at the given base ({base}).\n\
                 Indexability results depend on that flag: if the site is published at\n\
                 {origin}, repeat the audit with --base {origin}",
            es: "Aviso: {n} de {total} canonicals apuntan a {origin}, no a la base indicada ({base}).\n\
                 Los resultados de indexabilidad dependen de ese flag: si el sitio se publica en\n\
                 {origin}, repite la auditoría con --base {origin}",
        }
    }

    // ── Ficha de una URL (`inspect.rs`) ──────────────────────────────────────
    //
    // La pregunta que responde es «¿quién enlaza aquí?» (revisión 2026-08-01 §5.2), así que
    // los enlaces entrantes son la estrella; el resto de secciones evitan tener que abrir
    // el XLSX para ver una sola URL. Los valores de la base de datos (códigos, regiones,
    // `nofollow`, motivos de exclusión) no se traducen: son identificadores.
    messages! {
        inspect_error_not_found(url) {
            en: "{url} is not in this crawl.",
            es: "{url} no está en este rastreo.",
        }
        inspect_closest() {
            en: "The closest URLs in the file are:",
            es: "Las URLs más parecidas del fichero son:",
        }
        inspect_status_title() {
            en: "Status",
            es: "Estado",
        }
        label_http_status() {
            en: "HTTP status",
            es: "Código HTTP",
        }
        label_content_type() {
            en: "Content type",
            es: "Tipo de contenido",
        }
        label_response_time() {
            en: "Response time",
            es: "Tiempo de respuesta",
        }
        label_depth() {
            en: "Depth",
            es: "Profundidad",
        }
        note_depth_clicks() {
            en: "clicks from the seed",
            es: "clics desde la semilla",
        }
        label_in_sitemap() {
            en: "In sitemap",
            es: "En el sitemap",
        }
        word_yes() {
            en: "yes",
            es: "sí",
        }
        word_no() {
            en: "no",
            es: "no",
        }
        label_crawl_state() {
            en: "Crawl state",
            es: "Estado de rastreo",
        }
        state_note_pending() {
            en: "discovered, but the crawl never fetched it",
            es: "descubierta, pero el rastreo no llegó a pedirla",
        }
        state_note_excluded(reason) {
            en: "excluded from the crawl ({reason})",
            es: "excluida del rastreo ({reason})",
        }
        // Una externa con código o con error de sonda **sí se pidió**: decir de ella
        // «excluida del rastreo» era falso. Se comprobó su estado y no se auditó su contenido.
        state_note_external_checked() {
            en: "external URL — status checked, content not audited",
            es: "URL externa — estado comprobado, contenido no auditado",
        }
        state_note_external_unchecked() {
            en: "external URL — never requested",
            es: "URL externa — nunca solicitada",
        }
        // Interna `skipped` sin motivo: fuera del alcance del rastreo (el caso del modo lista).
        state_note_out_of_scope() {
            en: "discovered, but outside this crawl's scope",
            es: "descubierta, pero fuera del alcance de este rastreo",
        }
        state_note_error(detail) {
            en: "the request failed ({detail})",
            es: "la petición falló ({detail})",
        }
        // `unknown` no es una región del HTML, es el relleno de «no se supo»: se traduce,
        // a diferencia de `main`/`nav`/`footer`, que son identificadores del documento.
        region_unknown() {
            en: "unknown",
            es: "desconocida",
        }
        inspect_page_title() {
            en: "Page",
            es: "Página",
        }
        label_page_title() {
            en: "Title",
            es: "Título",
        }
        label_meta_description() {
            en: "Meta description",
            es: "Meta description",
        }
        label_h1() {
            en: "H1",
            es: "H1",
        }
        label_word_count() {
            en: "Word count",
            es: "Palabras",
        }
        label_canonical() {
            en: "Canonical",
            es: "Canonical",
        }
        canonical_self() {
            en: "self",
            es: "la propia página",
        }
        label_indexable() {
            en: "Indexable",
            es: "Indexable",
        }
        indexable_no(reason) {
            en: "no — {reason}",
            es: "no — {reason}",
        }
        chars_suffix(n) {
            en: "{n} chars",
            es: "{n} caracteres",
        }
        inspect_redirects_title() {
            en: "Redirect chain",
            es: "Cadena de redirecciones",
        }
        inlinks_title(n) {
            en: "Inlinks ({n})",
            es: "Enlaces entrantes ({n})",
        }
        inlinks_by_region(breakdown, nofollow) {
            en: "By region: {breakdown} · {nofollow} nofollow",
            es: "Por región: {breakdown} · {nofollow} nofollow",
        }
        inlinks_none() {
            en: "No crawled page links to this URL.",
            es: "Ninguna página rastreada enlaza a esta URL.",
        }
        inlinks_sample_heading() {
            en: "Linking pages, content links first:",
            es: "Páginas que enlazan, primero los enlaces de contenido:",
        }
        no_anchor() {
            en: "(no anchor text)",
            es: "(sin texto de ancla)",
        }
        // El corte dice cuántas se enseñan y el comando exacto de la lista completa: cortar
        // sin decir cómo ver el resto es el error del que nació `report --rule` (§5.1).
        inspect_truncated(n, cmd) {
            en: "… showing {n} — full list: {cmd}",
            es: "… se muestran {n} — lista completa: {cmd}",
        }
        images_title(n) {
            en: "Images ({n})",
            es: "Imágenes ({n})",
        }
        image_no_alt() {
            en: "(no alt)",
            es: "(sin alt)",
        }
        image_usage_title() {
            en: "Used as an image",
            es: "Usada como imagen",
        }
    }

    // Los dos mensajes con plural real se escriben a mano: el macro no distingue «1 interno»
    // de «2 internos», y resolverlo con «interno(s)» es el atajo que la revisión cazó.
    // Siguen viviendo aquí, con sus dos columnas, como el resto del catálogo.

    /// La cabecera de enlaces salientes, con el plural bien resuelto en cada idioma.
    pub fn outlinks_title(lang: Lang, total: i64, internal: i64, external: i64) -> String {
        let n = |v: i64| super::count(lang, v);
        match lang {
            Lang::En => format!(
                "Outlinks ({}: {} internal, {} external)",
                n(total),
                n(internal),
                n(external)
            ),
            Lang::Es => format!(
                "Enlaces salientes ({}: {} {}, {} {})",
                n(total),
                n(internal),
                if internal == 1 { "interno" } else { "internos" },
                n(external),
                if external == 1 { "externo" } else { "externos" },
            ),
        }
    }

    /// El sufijo que separa las externas del recuento de códigos de estado: un 404 ajeno no
    /// es un error del sitio auditado y no debe sumarse como si lo fuera. A mano por el
    /// plural español («+1 externa», «+2 externas»); el inglés no flexiona.
    pub fn note_external_status(lang: Lang, n: i64) -> String {
        let count = super::count(lang, n);
        match lang {
            Lang::En => format!("+{count} external"),
            Lang::Es => format!("+{count} {}", if n == 1 { "externa" } else { "externas" }),
        }
    }

    /// «Incrustada N veces en M páginas», sin decir «1 veces» ni «1 pages».
    pub fn image_usage_line(lang: Lang, times: i64, pages: i64) -> String {
        let n = |v: i64| super::count(lang, v);
        match lang {
            Lang::En => format!(
                "embedded {} {} on {} {}",
                n(times),
                if times == 1 { "time" } else { "times" },
                n(pages),
                if pages == 1 { "page" } else { "pages" },
            ),
            Lang::Es => format!(
                "incrustada {} {} en {} {}",
                n(times),
                if times == 1 { "vez" } else { "veces" },
                n(pages),
                if pages == 1 { "página" } else { "páginas" },
            ),
        }
    }

    // ── Comparación de rastreos (`diff.rs`) ──────────────────────────────────
    messages! {
        diff_title() {
            en: "Crawl comparison",
            es: "Comparación de rastreos",
        }
        label_before() {
            en: "Before",
            es: "Antes",
        }
        label_after() {
            en: "After",
            es: "Después",
        }
        label_site() {
            en: "Site",
            es: "Sitio",
        }
        label_common() {
            en: "Common",
            es: "En común",
        }
        diff_crawl_counts(urls, findings) {
            en: "{urls} URLs, {findings} findings",
            es: "{urls} URLs, {findings} hallazgos",
        }
        diff_common_note(n) {
            en: "{n} URLs present in both crawls",
            es: "{n} URLs presentes en los dos rastreos",
        }
        warnings_title() {
            en: "Warnings",
            es: "Avisos",
        }
        tag_inconclusive() {
            en: "INCONCLUSIVE",
            es: "NO CONCLUYENTE",
        }
        tag_warning() {
            en: "WARNING",
            es: "AVISO",
        }
        side_before() {
            en: "before",
            es: "antes",
        }
        side_after() {
            en: "after",
            es: "después",
        }
        warn_truncated(side, reason_suffix) {
            en: "The '{side}' crawl is truncated{reason_suffix}. What disappeared or got resolved \
                 cannot be asserted: the crawl may simply not have reached it.",
            es: "El rastreo «{side}» está truncado{reason_suffix}. Lo que desapareció o se resolvió \
                 no se puede afirmar: puede que el rastreo simplemente no llegara hasta ahí.",
        }
        warn_truncated_by(reason) {
            en: " by {reason}",
            es: " por {reason}",
        }
        // La variante de `warn_truncated` para `list_mode`: el rastreo no se cortó — es que
        // un rastreo en modo lista nunca ve más que su lista. La consecuencia para el diff es
        // la misma (las ausencias no se pueden afirmar) y el motivo, distinto.
        warn_list_mode(side) {
            en: "The '{side}' crawl is a list crawl: it only saw the URLs it was given. What \
                 disappeared or got resolved cannot be asserted.",
            es: "El rastreo «{side}» es de modo lista: solo vio las URLs que se le dieron. Lo \
                 que desapareció o se resolvió no se puede afirmar.",
        }
        warn_unfinished(side, status) {
            en: "The '{side}' crawl did not finish (status '{status}'). Its absences mean nothing.",
            es: "El rastreo «{side}» no terminó (estado «{status}»). Sus ausencias no significan nada.",
        }
        warn_rules_changed(before, after) {
            en: "The rule catalog changed ({before} → {after}). Some of the new or resolved \
                 findings may come from the rules, not from the site.",
            es: "El catálogo de reglas cambió ({before} → {after}). Parte de los hallazgos nuevos \
                 o resueltos puede venir de las reglas, no del sitio.",
        }
        warn_config_changed(fields) {
            en: "The crawl configuration changed in: {fields}. Without keeping that in mind, \
                 scope changes get attributed to the site.",
            es: "La configuración del rastreo cambió en: {fields}. Sin tenerlo en cuenta, los \
                 cambios de alcance se atribuyen al sitio.",
        }
        warn_scope_changed(before, after) {
            en: "The base URLs differ: {before} → {after}. Same site, different starting point.",
            es: "Las URLs base difieren: {before} → {after}. El mismo sitio, distinto punto de partida.",
        }
        warn_mode_changed(before, after) {
            en: "The crawl modes differ: {before} → {after}.",
            es: "Los modos de rastreo difieren: {before} → {after}.",
        }
        warn_missing_table(side, table) {
            en: "The '{side}' crawl predates the `{table}` table: that comparison is skipped.",
            es: "El rastreo «{side}» es anterior a la tabla `{table}`: esa comparación se omite.",
        }
        warn_order_inverted(before_started, after_started) {
            en: "The crawl given as 'after' ({after_started}) started earlier than the one \
                 given as 'before' ({before_started}). If that is not deliberate, swap the two \
                 arguments: 'Got worse' and 'Got better' come out reversed.",
            es: "El rastreo pasado como «después» ({after_started}) empezó antes que el pasado \
                 como «antes» ({before_started}). Si no es deliberado, intercambia los dos \
                 argumentos: «Ha empeorado» y «Ha mejorado» salen del revés.",
        }
        worse_title() {
            en: "Got worse",
            es: "Ha empeorado",
        }
        label_new_findings() {
            en: "New findings",
            es: "Hallazgos nuevos",
        }
        more_examples(n) {
            en: "… and {n} more",
            es: "… y {n} más",
        }
        label_pages_lost_index() {
            en: "Pages no longer indexable",
            es: "Páginas que dejan de ser indexables",
        }
        label_status_worse() {
            en: "Status codes that got worse",
            es: "Códigos de estado que empeoran",
        }
        robots_content_changed(host) {
            en: "{host}: the content changed",
            es: "{host}: el contenido cambió",
        }
        robots_blocks_all() {
            en: "ATTENTION: it now blocks crawling of the entire site.",
            es: "ATENCIÓN: ahora bloquea el rastreo del sitio entero.",
        }
        nothing_worse() {
            en: "Nothing. No new findings, no pages dropped from the index.",
            es: "Nada: ni hallazgos nuevos ni páginas fuera del índice.",
        }
        better_title() {
            en: "Got better",
            es: "Ha mejorado",
        }
        label_findings_resolved() {
            en: "Findings resolved",
            es: "Hallazgos resueltos",
        }
        label_pages_indexable_again() {
            en: "Pages indexable again",
            es: "Páginas indexables de nuevo",
        }
        label_status_better() {
            en: "Status codes that improved",
            es: "Códigos de estado que mejoran",
        }
        other_title() {
            en: "Other changes",
            es: "Otros cambios",
        }
        label_new_urls() {
            en: "New URLs",
            es: "URLs nuevas",
        }
        label_urls_gone() {
            en: "URLs gone",
            es: "URLs desaparecidas",
        }
        label_titles_changed() {
            en: "Titles changed",
            es: "Títulos cambiados",
        }
        label_meta_changed() {
            en: "Meta descriptions changed",
            es: "Meta descriptions cambiadas",
        }
        label_canonicals_changed() {
            en: "Canonicals changed",
            es: "Canonicals cambiados",
        }
        label_findings_persisted() {
            en: "Findings still present",
            es: "Hallazgos que persisten",
        }
        suppressed_title() {
            en: "Cannot be asserted",
            es: "No se puede afirmar",
        }
        // Multilínea: el punto de llamada imprime cada línea con su sangría.
        suppressed_intro() {
            en: "One of the two crawls is incomplete, so these differences could come from\n\
                 what was never crawled rather than from the site. They are not counted as changes:",
            es: "Uno de los dos rastreos está incompleto, así que estas diferencias podrían\n\
                 venir de lo que no se llegó a rastrear y no del sitio. No se cuentan como cambios:",
        }
        label_candidate_new_urls() {
            en: "Candidate new URLs",
            es: "URLs nuevas candidatas",
        }
        label_candidate_urls_gone() {
            en: "Candidate URLs gone",
            es: "URLs desaparecidas candidatas",
        }
        label_candidate_new_findings() {
            en: "Candidate new findings",
            es: "Hallazgos nuevos candidatos",
        }
        label_candidate_resolved() {
            en: "Candidate resolved findings",
            es: "Hallazgos resueltos candidatos",
        }
        suppressed_advice() {
            en: "Re-crawl both sides with the same limit to get a firm diff.",
            es: "Vuelve a rastrear los dos lados con el mismo límite para obtener un diff firme.",
        }
        gate_title() {
            en: "CI gate",
            es: "Puerta de CI",
        }
        // Multilínea: el punto de llamada imprime cada línea con su sangría.
        gate_inconclusive() {
            en: "The 'before' crawl is incomplete: new findings cannot be asserted and\n\
                 --fail-on takes no stance. This is not a pass.",
            es: "El rastreo «antes» está incompleto: los hallazgos nuevos no se pueden afirmar\n\
                 y --fail-on no se pronuncia. Esto no es un aprobado.",
        }
        gate_fail_detail(n, severity, token) {
            en: "{n} new finding(s) [{severity}] — matches --fail-on {token}",
            es: "{n} hallazgo(s) nuevo(s) [{severity}] — coincide con --fail-on {token}",
        }
        gate_pass() {
            en: "PASS   nothing watched shows up as a new finding.",
            es: "PASS   nada de lo vigilado aparece como hallazgo nuevo.",
        }
        site_wide_finding() {
            en: "(site-wide finding)",
            es: "(hallazgo de sitio)",
        }
        site_placeholder() {
            en: "(site)",
            es: "(sitio)",
        }
        error_missing_file(path) {
            en: "{path} does not exist",
            es: "{path} no existe",
        }
        // El contrato del manual §5: todo error de fichero dice qué comando lo genera.
        error_store_missing(path) {
            en: "{path} does not exist. Crawl files are produced by `crawlforge crawl`, \
                 `crawlforge audit` and `crawlforge list`.",
            es: "{path} no existe. Los ficheros de rastreo los generan `crawlforge crawl`, \
                 `crawlforge audit` y `crawlforge list`.",
        }
        error_not_a_crawl(path) {
            en: "{path} does not look like a CrawlForge crawl file",
            es: "{path} no parece un fichero de rastreo de CrawlForge",
        }
        error_different_sites(a, b) {
            en: "the two crawls are of different sites: {a} and {b}.\n\
                 URLs are matched by their text, so the diff would say the whole site disappeared \
                 and another one appeared. Compare each site against itself.",
            es: "los dos rastreos son de sitios distintos: {a} y {b}.\n\
                 Las URLs se emparejan por su texto, así que el diff diría que el sitio entero \
                 desapareció y que apareció otro. Compara cada sitio consigo mismo.",
        }
        error_fail_on_unknown(token) {
            en: "--fail-on: '{token}' is not a rule or a severity.\n\
                 The severities are critical, high, medium, low and info; rule IDs are listed \
                 by `crawlforge rules`.",
            es: "--fail-on: «{token}» no es una regla ni una severidad.\n\
                 Las severidades son critical, high, medium, low e info; los IDs de regla los \
                 lista `crawlforge rules`.",
        }
    }

    // ── Panel de cartera (`portfolio.rs`) ────────────────────────────────────
    //
    // Los IDs de regla, las severidades usadas como token (`crit`, `high`…), las rutas, las
    // fechas y las versiones no se traducen: son datos e identificadores, como en el resto
    // de la CLI.
    messages! {
        portfolio_title() {
            en: "Portfolio panel",
            es: "Panel de cartera",
        }
        portfolio_range(oldest, newest) {
            en: "crawls from {oldest} to {newest}",
            es: "rastreos del {oldest} al {newest}",
        }
        // Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
        // changed" would silently mean a different period on every site.
        warn_portfolio_date_spread(days, oldest, newest) {
            en: "The oldest crawl ({oldest}) and the newest ({newest}) are {days} days apart: \
                 this panel is not a snapshot of the portfolio, and 'what changed' covers a \
                 different period on each site.",
            es: "Entre el rastreo más viejo ({oldest}) y el más nuevo ({newest}) hay {days} \
                 días: este panel no es una foto de la cartera, y «qué cambió» cubre un \
                 periodo distinto en cada sitio.",
        }
        // Trap 3.2: two files crawled with different catalogs are not comparable without
        // saying so — a rule can be "missing" on a site because it did not exist yet.
        warn_portfolio_rules_versions(versions) {
            en: "Not every site was crawled with the same rule catalog ({versions}). A rule \
                 can be missing on a site because it did not exist when that site was crawled.",
            es: "No todos los sitios se rastrearon con el mismo catálogo de reglas \
                 ({versions}). Una regla puede faltar en un sitio porque no existía cuando se \
                 rastreó.",
        }
        warn_portfolio_core_versions(versions) {
            en: "The crawls come from different engine versions ({versions}).",
            es: "Los rastreos vienen de versiones distintas del motor ({versions}).",
        }
        portfolio_skipped_title() {
            en: "Files set aside",
            es: "Ficheros apartados",
        }
        // A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
        portfolio_prev_not_input() {
            en: "it is a .prev.sqlite: the 'before' of the crawl next to it, already used by \
                 the comparison. It does not count as a site.",
            es: "es un .prev.sqlite: el «antes» del rastreo de al lado, que la comparación ya \
                 usa. No cuenta como sitio.",
        }
        error_portfolio_newer_schema(version, supported) {
            en: "its schema (v{version}) is newer than this build understands (v{supported}). \
                 Update crawlforge to open it.",
            es: "su esquema (v{version}) es más nuevo de lo que esta versión entiende \
                 (v{supported}). Actualiza crawlforge para abrirlo.",
        }
        error_portfolio_no_sites() {
            en: "no crawl files usable as a portfolio were found in the given paths. Crawl \
                 files are produced by `crawlforge crawl`, `crawlforge audit` and `crawlforge \
                 list`; a directory is scanned for *.sqlite files.",
            es: "en las rutas indicadas no se encontró ningún fichero de rastreo utilizable \
                 como cartera. Los ficheros de rastreo los generan `crawlforge crawl`, \
                 `crawlforge audit` y `crawlforge list`; un directorio se recorre buscando \
                 ficheros *.sqlite.",
        }
        error_portfolio_tier() {
            en: "the portfolio panel is not part of the free tier: it needs Pro. Single-site \
                 commands (`report`, `diff`, `export`) remain available.",
            es: "el panel de cartera no es del nivel gratuito: necesita Pro. Los comandos de \
                 un solo sitio (`report`, `diff`, `export`) siguen disponibles.",
        }
        error_portfolio_too_many(n, max) {
            en: "the portfolio has {n} sites and this tier caps it at {max}.",
            es: "la cartera tiene {n} sitios y este nivel admite {max}.",
        }
        portfolio_changes_title() {
            en: "What changed",
            es: "Qué cambió",
        }
        portfolio_no_pairs() {
            en: "No site has a previous crawl next to it. Re-crawl into the same output file \
                 and the previous crawl is kept as .prev.sqlite — that is the 'before' this \
                 section compares against.",
            es: "Ningún sitio tiene un rastreo anterior al lado. Repite el rastreo sobre el \
                 mismo fichero de salida y el anterior se conserva como .prev.sqlite: ese es \
                 el «antes» con el que compara esta sección.",
        }
        portfolio_new_critical_high() {
            en: "New critical and high findings",
            es: "Hallazgos nuevos críticos y altos",
        }
        portfolio_none_critical_high() {
            en: "No new critical or high findings on any compared site.",
            es: "Ningún hallazgo nuevo crítico o alto en los sitios comparados.",
        }
        portfolio_rest_title() {
            en: "The rest, site by site",
            es: "El resto, sitio a sitio",
        }
        portfolio_no_changes() {
            en: "no changes",
            es: "sin cambios",
        }
        // The diff of that pair cannot assert absences: one side is incomplete.
        portfolio_pair_inconclusive() {
            en: "one side is incomplete: what disappeared or got resolved cannot be asserted",
            es: "un lado está incompleto: lo que desapareció o se resolvió no se puede afirmar",
        }
        portfolio_pair_failed(reason) {
            en: "its comparison against the previous crawl failed: {reason}",
            es: "su comparación con el rastreo anterior falló: {reason}",
        }
        portfolio_spread_title() {
            en: "Failing across the portfolio",
            es: "Qué falla en toda la cartera",
        }
        portfolio_spread_intro() {
            en: "A rule firing on most sites is rarely content: it is usually a shared \
                 template or plugin — one fix that serves them all.",
            es: "Una regla que salta en la mayoría de los sitios rara vez es contenido: suele \
                 ser una plantilla o un plugin compartido — un arreglo que sirve para todos.",
        }
        portfolio_spread_none() {
            en: "No rule fires on any site of the portfolio.",
            es: "Ninguna regla dispara en ningún sitio de la cartera.",
        }
        portfolio_glance_title() {
            en: "The portfolio at a glance",
            es: "La cartera de un vistazo",
        }
        th_site() {
            en: "site",
            es: "sitio",
        }
        th_crawled() {
            en: "crawled",
            es: "rastreado",
        }
        th_indexable() {
            en: "index.",
            es: "index.",
        }
        flag_truncated() {
            en: "(truncated)",
            es: "(truncado)",
        }
        flag_list_mode() {
            en: "(list crawl)",
            es: "(modo lista)",
        }
        flag_unfinished(status) {
            en: "(did not finish: {status})",
            es: "(sin terminar: {status})",
        }
        hint_site_diff() {
            en: "Full diff of one site:",
            es: "Diff completo de un sitio:",
        }
        portfolio_html_title() {
            en: "Portfolio panel",
            es: "Panel de cartera",
        }
    }

    /// "{n} sites" with the plural resolved per language, like [`outlinks_title`].
    pub fn portfolio_sites_count(lang: Lang, n: usize) -> String {
        match lang {
            Lang::En => format!("{n} {}", if n == 1 { "site" } else { "sites" }),
            Lang::Es => format!("{n} {}", if n == 1 { "sitio" } else { "sitios" }),
        }
    }

    /// "9 of 12 sites" — the numerator of the spread table.
    pub fn portfolio_sites_of(lang: Lang, fired: usize, total: usize) -> String {
        match lang {
            Lang::En => format!("{fired} of {total} {}", if total == 1 { "site" } else { "sites" }),
            Lang::Es => {
                format!("{fired} de {total} {}", if total == 1 { "sitio" } else { "sitios" })
            }
        }
    }

    /// " (2 inconclusive)" — sites where the rule **could not be evaluated**: a truncated or
    /// list-mode crawl does not evaluate the full-graph rules, and counting those sites as
    /// "does not fire here" would be lying (trap 3.1). Empty when every site could answer.
    pub fn portfolio_inconclusive_suffix(lang: Lang, n: usize) -> String {
        if n == 0 {
            return String::new();
        }
        match lang {
            Lang::En => format!(" ({n} inconclusive)"),
            Lang::Es => {
                format!(" ({n} no {})", if n == 1 { "concluyente" } else { "concluyentes" })
            }
        }
    }

    /// "3 of 5 sites have a previous crawl (.prev.sqlite) to compare against." The noun
    /// follows the total and the verb follows the pair count: "1 of 5 sites has".
    pub fn portfolio_pairs_line(lang: Lang, pairs: usize, total: usize) -> String {
        match lang {
            Lang::En => format!(
                "{pairs} of {total} {} {} a previous crawl (.prev.sqlite) to compare against.",
                if total == 1 { "site" } else { "sites" },
                if pairs == 1 { "has" } else { "have" }
            ),
            Lang::Es => format!(
                "{pairs} de {total} {} {} un rastreo anterior (.prev.sqlite) con el que comparar.",
                if total == 1 { "sitio" } else { "sitios" },
                if pairs == 1 { "tiene" } else { "tienen" }
            ),
        }
    }

    // ── Identificación de ficheros (`store_check.rs`) ────────────────────────
    messages! {
        error_store_is_diff(path) {
            en: "{path} is not a crawl file: it is a diff file, the kind `crawlforge diff --out` \
                 saves.\nCrawl files are produced by `crawlforge crawl` and `crawlforge audit`.",
            es: "{path} no es un fichero de rastreo: es un fichero de diff, de los que guarda \
                 `crawlforge diff --out`.\nLos ficheros de rastreo los generan `crawlforge crawl` \
                 y `crawlforge audit`.",
        }
        error_store_foreign(path) {
            en: "{path} is not a CrawlForge crawl file: it does not have its tables. \
                 Is it from another program?\nCrawl files are produced by `crawlforge crawl` \
                 and `crawlforge audit`.",
            es: "{path} no es un fichero de rastreo de CrawlForge: no tiene sus tablas. \
                 ¿Es de otro programa?\nLos ficheros de rastreo los generan `crawlforge crawl` \
                 y `crawlforge audit`.",
        }
        error_store_not_sqlite(path) {
            en: "{path} is not a CrawlForge file: it is not even a SQLite database.\n\
                 Crawl files are produced by `crawlforge crawl` and `crawlforge audit`.",
            es: "{path} no es un fichero de CrawlForge: ni siquiera es una base de datos SQLite.\n\
                 Los ficheros de rastreo los generan `crawlforge crawl` y `crawlforge audit`.",
        }
    }

    // ── Informe Markdown/HTML (`audit_report.rs`) ────────────────────────────
    messages! {
        report_audit_of() {
            en: "Audit of",
            es: "Auditoría de",
        }
        report_mode_line(mode, started) {
            en: "Mode `{mode}` · started {started}",
            es: "Modo `{mode}` · iniciada {started}",
        }
        report_truncated_note(reason) {
            en: "> **Truncated crawl** (`{reason}`). The counts cover what was crawled up to \
                 that point, not the whole site.",
            es: "> **Rastreo truncado** (`{reason}`). Los recuentos son de lo rastreado hasta \
                 ese punto, no del sitio entero.",
        }
        // Para `truncated_reason = 'list_mode'`: el rastreo no se cortó, auditó su lista
        // entera. El informe tiene que decir eso, no «truncado».
        report_list_mode_note() {
            en: "> **List crawl.** Only the listed URLs were audited: the counts describe \
                 that set, not the whole site, and rules that need the site's complete link \
                 graph are not evaluated.",
            es: "> **Rastreo en modo lista.** Solo se auditaron las URLs de la lista: los \
                 recuentos describen ese conjunto, no el sitio entero, y las reglas que \
                 necesitan el grafo completo del sitio no se evalúan.",
        }
        th_metric() {
            en: "Metric",
            es: "Métrica",
        }
        th_value() {
            en: "Value",
            es: "Valor",
        }
        row_pages() {
            en: "Pages",
            es: "Páginas",
        }
        row_indexable() {
            en: "Indexable",
            es: "Indexables",
        }
        row_errors_4xx_5xx() {
            en: "4xx/5xx",
            es: "Errores 4xx/5xx",
        }
        report_more(n) {
            en: "… {n} more",
            es: "… {n} más",
        }
        // El corte del informe dice dónde están las que no enseña: sin el comando, la única
        // vía para verlas era abrir el XLSX o hacer SQL a mano (revisión de UX §5.1).
        report_more_run(n, cmd) {
            en: "… {n} more — run: {cmd}",
            es: "… {n} más — ejecuta: {cmd}",
        }
        // La variante para una regla colapsada: el titular ya dio los números («13 problemas
        // de plantilla (567 páginas)») y repetir aquí un recuento crudo de filas los
        // contradiría.
        report_full_list_run(cmd) {
            en: "… full list: {cmd}",
            es: "… lista completa: {cmd}",
        }
        html_title() {
            en: "SEO audit",
            es: "Auditoría SEO",
        }
    }

    // ── Catálogo de reglas (`rules.rs`) ──────────────────────────────────────
    messages! {
        th_severity() {
            en: "SEVERITY",
            es: "SEVERIDAD",
        }
        th_category() {
            en: "CATEGORY",
            es: "CATEGORÍA",
        }
        th_scope() {
            en: "SCOPE",
            es: "ALCANCE",
        }
        th_tier() {
            en: "TIER",
            es: "NIVEL",
        }
        th_name() {
            en: "NAME",
            es: "NOMBRE",
        }
        lbl_category() {
            en: "category",
            es: "categoría",
        }
        lbl_scope() {
            en: "scope",
            es: "alcance",
        }
        lbl_tier() {
            en: "tier",
            es: "nivel",
        }
        lbl_reference() {
            en: "reference",
            es: "referencia",
        }
        scope_page() {
            en: "page",
            es: "página",
        }
        scope_site() {
            en: "site",
            es: "sitio",
        }
        rules_summary(total, free, page, site) {
            en: "{total} rules: {free} in the free tier, {page} page-level and {site} site-wide.",
            es: "{total} reglas: {free} en el nivel gratuito, {page} de página y {site} de conjunto.",
        }
        error_unknown_category(category, list) {
            en: "no rules in category '{category}'. The categories are: {list}.",
            es: "no hay reglas en la categoría «{category}». Las categorías son: {list}.",
        }
    }

    // ── Cadenas de `main.rs`, listas para cablear ────────────────────────────
    //
    // `main.rs` es de otro propietario; estas entradas existen para que el cableado sea
    // sustituir cada literal por su llamada. Los números se pre-formatean con
    // `i18n::group_thousands` / `i18n::decimal1` antes de pasarlos.
    messages! {
        crawling(target) {
            en: "Crawling {target}",
            es: "Rastreando {target}",
        }
        // Se valida **antes de tocar el disco**: una semilla imposible no debe costar una
        // rotación de ficheros ni dejar un `.sqlite` de aspecto válido con un `.lock` al lado.
        error_invalid_seed(url) {
            en: "{url} is not a crawlable URL: expected something like https://example.com/. \
                 Nothing was created.",
            es: "{url} no es una URL rastreable: se esperaba algo como https://example.com/. \
                 No se ha creado nada.",
        }
        file_line(path) {
            en: "File:     {path}",
            es: "Fichero:  {path}",
        }
        previous_kept(path) {
            en: "Previous: {path} (kept, from the last run)",
            es: "Anterior: {path} (conservado, de la ejecución anterior)",
        }
        report_written(path) {
            en: "Report written to {path}",
            es: "Informe escrito en {path}",
        }
        /// Con `--out` y el formato de terminal, lo que se escribe es Markdown: un volcado del
        /// terminal en un fichero conserva sus cajas y se lee peor. Se dice **qué** se escribió
        /// para que nadie abra el fichero esperando lo que acaba de ver en pantalla.
        markdown_report_written(path) {
            en: "Markdown report written to {path}",
            es: "Informe Markdown escrito en {path}",
        }
        exported_csv(n, dir) {
            en: "Exported {n} CSV files to {dir}",
            es: "Exportados {n} ficheros CSV a {dir}",
        }
        exported_sheets(n, path) {
            en: "Exported {n} sheets to {path}",
            es: "Exportadas {n} hojas a {path}",
        }
        bench_appended(path) {
            en: "Benchmark record appended to {path}",
            es: "Registro de benchmark añadido a {path}",
        }
        next_steps_title() {
            en: "Next steps",
            es: "Pasos siguientes",
        }
        next_explain() {
            en: "Explain every finding, with affected URLs:",
            es: "Explicar cada hallazgo, con sus URLs afectadas:",
        }
        next_spreadsheet() {
            en: "Open the full data as a spreadsheet:",
            es: "Abrir todos los datos como hoja de cálculo:",
        }
        next_compare_previous() {
            en: "Compare against the previous run of this crawl:",
            es: "Comparar con la ejecución anterior de este rastreo:",
        }
        next_compare_later() {
            en: "Re-run this same crawl after your next deploy, then compare the two:",
            es: "Repite este mismo rastreo tras tu próximo despliegue y compara los dos:",
        }
        next_reread() {
            en: "Re-read this summary any time:",
            es: "Releer este resumen en cualquier momento:",
        }
        // ── Autenticación básica HTTP (staging protegido) ────────────────────
        //
        // Los tres avisos van por stderr: hablan de secretos y no deben ensuciar un pipe.
        // Ninguno repite la credencial, como corresponde a un aviso sobre secretos.
        auth_url_note(host) {
            en: "Using HTTP Basic auth from the URL: it will be sent only to {host} and will \
                 not be stored in the crawl file.",
            es: "Se usa la autenticación básica HTTP de la URL: se enviará solo a {host} y no \
                 se guardará en el fichero de rastreo.",
        }
        auth_env_note(host) {
            en: "Using HTTP Basic auth from CRAWLFORGE_AUTH: it will be sent only to {host} \
                 and will not be stored in the crawl file.",
            es: "Se usa la autenticación básica HTTP de CRAWLFORGE_AUTH: se enviará solo a \
                 {host} y no se guardará en el fichero de rastreo.",
        }
        auth_base_ignored() {
            en: "Warning: credentials in --base were removed: a directory audit reads from \
                 disk and makes no HTTP requests.",
            es: "Aviso: las credenciales de --base se han retirado: una auditoría de \
                 directorio lee del disco y no hace peticiones HTTP.",
        }
        auth_list_ignored() {
            en: "Warning: credentials inside URL list entries are ignored and removed. To \
                 authenticate the crawl, set CRAWLFORGE_AUTH=user:password; it applies only \
                 to the host of the first URL.",
            es: "Aviso: las credenciales dentro de las URLs de la lista se ignoran y se \
                 retiran. Para autenticar el rastreo, define \
                 CRAWLFORGE_AUTH=usuario:contraseña; se aplica solo al host de la primera URL.",
        }
        progress_sitemaps() {
            en: "discovering sitemaps…",
            es: "descubriendo sitemaps…",
        }
        progress_finalize() {
            en: "final pass: incoming links and site-wide rules…",
            es: "pasada final: enlaces entrantes y reglas de conjunto…",
        }
        progress_crawl(fetched, queued, rate, findings) {
            en: "{fetched} crawled · {queued} queued · {rate} URL/s · {findings} findings",
            es: "{fetched} rastreadas · {queued} en cola · {rate} URL/s · {findings} hallazgos",
        }
        progress_failed_suffix(n) {
            en: " · {n} failed",
            es: " · {n} con error",
        }
    }

    // ── Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`) ──────
    messages! {
        resuming(target) {
            en: "Resuming crawl of {target}",
            es: "Reanudando el rastreo de {target}",
        }
        resume_counts(done, pending) {
            en: "Picking up where it stopped: {done} URLs already crawled, {pending} still pending.",
            es: "Se retoma donde se quedó: {done} URLs ya rastreadas, {pending} pendientes.",
        }
        resume_config_note() {
            en: "Using the configuration saved in the file: the original run's settings apply.",
            es: "Se usa la configuración guardada en el fichero: mandan los ajustes de la ejecución original.",
        }
        error_resume_schema_blocking(file, version, blocking) {
            en: "{file} is a schema v{version} crawl, and migration {blocking} changes what the \
                 engine writes: half of it would say one thing and half another. Run the crawl \
                 again. You can still open this file with `report`, `export` and `diff`.",
            es: "{file} es un rastreo de esquema v{version}, y la migración {blocking} cambia lo \
                 que el motor escribe: la mitad diría una cosa y la otra mitad otra. Vuelve a \
                 lanzar el rastreo. El fichero se sigue abriendo con `report`, `export` y `diff`.",
        }
        resume_robots_not_inherited() {
            en: "Note: the original crawl ignored robots.txt. This one does not — that permission \
                 is granted by you, not by a file. Re-run the original crawl command if you need it.",
            es: "Aviso: el rastreo original ignoraba robots.txt. Este no lo hace: ese permiso lo \
                 concedes tú, no un fichero. Vuelve a lanzar el rastreo original si lo necesitas.",
        }
        resume_finished(urls, secs) {
            en: "Crawl resumed and completed: {urls} more URLs in {secs} s",
            es: "Rastreo reanudado y completado: {urls} URLs más en {secs} s",
        }
        interrupt_flushing() {
            en: "Interrupted: saving what is already crawled… (Ctrl+C again to quit without waiting)",
            es: "Interrumpido: guardando lo ya rastreado… (Ctrl+C otra vez para salir sin esperar)",
        }
        // Un Ctrl+C cuando el motor ya terminó —durante un export largo, por ejemplo— no
        // tiene nada que guardar: se sale ya, diciendo que el rastreo no corre peligro.
        interrupt_after_done() {
            en: "Interrupted. The crawl file is complete; a half-written export may be left behind.",
            es: "Interrumpido. El fichero de rastreo está completo; puede quedar un export a medio escribir.",
        }
        // La degradación del cierre (§3.1/§3.2 de la revisión): con otro programa leyendo el
        // fichero no se puede salir del modo WAL, y copiar el `.sqlite` suelto perdería lo
        // que quede en el `-wal`. El aviso da el dato y las dos salidas.
        warn_wal_kept(path) {
            en: "Note: another program has {path} open, so it stays in WAL mode. The -wal and \
                 -shm files next to it are part of this crawl: copy the three together, or close \
                 the other program and run `crawlforge report` on it to fold them back in.",
            es: "Aviso: otro programa tiene abierto {path}, así que se queda en modo WAL. Los \
                 ficheros -wal y -shm de al lado forman parte de este rastreo: cópialos los tres \
                 juntos, o cierra el otro programa y ejecuta `crawlforge report` sobre él para \
                 reintegrarlos.",
        }
        // El cerrojo del fichero (§3.3): el que llega segundo no espera ni rota nada, se le
        // dice que hay otro rastreo escribiendo y qué hacer.
        error_store_locked(path) {
            en: "another crawlforge process is writing {path} right now. Wait for it to finish, \
                 or stop it, before crawling or resuming this file.",
            es: "otro proceso de crawlforge está escribiendo {path} ahora mismo. Espera a que \
                 termine, o deténlo, antes de rastrear o reanudar este fichero.",
        }
        interrupted_saved(path) {
            en: "Crawl interrupted. Everything crawled so far is saved in {path}. Continue it with:",
            es: "Rastreo interrumpido. Todo lo rastreado queda guardado en {path}. Continúalo con:",
        }
        hint_previous_unfinished() {
            en: "The previous crawl had not finished. To continue it instead of starting over:",
            es: "El rastreo anterior no había terminado. Para continuarlo en vez de empezar de cero:",
        }
        error_resume_done(path) {
            en: "{path} is a finished crawl: there is nothing to resume.\n\
                 To audit the site again, run the original crawl command; the finished file \
                 will be kept next to it as .prev.sqlite.",
            es: "{path} es un rastreo terminado: no hay nada que reanudar.\n\
                 Para auditar el sitio otra vez, repite el comando de rastreo original; el \
                 fichero terminado se conservará al lado como .prev.sqlite.",
        }
        error_resume_status(path, status) {
            en: "{path} cannot be resumed: its status is '{status}'. Only interrupted crawls \
                 (status 'running' or 'paused') can be resumed.",
            es: "{path} no se puede reanudar: su estado es «{status}». Solo se reanudan los \
                 rastreos interrumpidos (estado «running» o «paused»).",
        }
        error_resume_schema(path, found, expected) {
            en: "{path} uses schema v{found} and this build writes v{expected}: an interrupted \
                 crawl can only be resumed by the version that started it. Run the crawl again.",
            es: "{path} es del esquema v{found} y esta versión escribe v{expected}: un rastreo \
                 interrumpido solo lo reanuda la versión que lo empezó. Repite el rastreo.",
        }
        error_resume_config(path) {
            en: "the crawl configuration stored in {path} cannot be read. Was the file written \
                 by another version? Run the crawl again.",
            es: "la configuración de rastreo guardada en {path} no se puede leer. ¿El fichero \
                 es de otra versión? Repite el rastreo.",
        }
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
        let err = resolve_pure(Some("fr"), None).expect_err("fr does not exist");
        assert!(err.to_string().contains("fr"), "{err}");
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
        // Caza el copia-y-pega de dejar el inglés en la columna española. Solo mensajes con
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
            assert_ne!(en, es, "the Spanish column must not be the English copied over");
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
        // cazó la revisión: las dos cadenas con plural real se escriben a mano.
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
        // The portfolio counters are hand-written for the same reason as the URL card's:
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
            assert_ne!(en, es, "the Spanish column must not be the English copied over");
        }
        // El error de fichero dice qué comando lo genera, como promete el manual §5.
        assert!(msg::error_store_missing(Lang::En, "x").contains("crawlforge crawl"));
        assert!(msg::error_store_missing(Lang::Es, "x").contains("crawlforge crawl"));
    }

    #[test]
    fn la_severidad_se_traduce_y_lo_desconocido_se_ensena_tal_cual() {
        assert_eq!(severity_word(Lang::Es, "critical"), "crítico");
        assert_eq!(severity_word(Lang::En, "critical"), "critical");
        assert_eq!(severity_word(Lang::Es, "rarisimo"), "rarisimo");
    }
}
