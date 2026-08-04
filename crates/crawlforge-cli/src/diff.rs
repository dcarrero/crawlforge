//! `crawlforge diff` — comparación entre dos rastreos del mismo sitio.
//!
//! Es la función que Screaming Frog no tiene y la razón por la que este producto existe
//! (`CONVENTIONS.md §1`): una auditoría suelta es una foto, y lo que necesita quien lleva una cartera
//! de cien sitios es saber **qué cambió desde el despliegue del martes**.
//!
//! El punto de entrada es [`compare`]; la presentación en terminal es [`print_report`], separada
//! a propósito para que el subcomando decida cuándo imprimir y con qué código de salida termina
//! (ver [`DiffOutcome::should_fail`]).
//!
//! # Las cuatro decisiones de criterio
//!
//! **1. Las filas se emparejan por `urls.url`, no por `id` ni por `url_hash`.** Los `id` son
//! autoincrementales y dependen del orden en que el rastreo descubrió cada URL: entre dos
//! rastreos del mismo sitio no significan nada. `url_hash` es estable —es el xxh3 de la URL
//! normalizada— y es lo que propone `docs/02-MODELO-DATOS.md §5`, pero tiene dos pegas para
//! esto: es un hash de 64 bits, así que admite colisiones que aquí se traducirían en «esta
//! página cambió de título» sobre dos URLs distintas, y depende de que la función de hash no
//! cambie nunca entre versiones del core. El texto de la URL es `UNIQUE` —o sea, indexado y sin
//! colisiones posibles— y es, en palabras del propio documento, «la garantía de integridad».
//! Un diff se hace una vez, no en el camino caliente: la diferencia de velocidad no compensa
//! arriesgar una afirmación falsa.
//!
//! **2. «El mismo hallazgo» es la terna `(rule_id, url, group_key)`.** Con `rule_id` y URL a
//! secas no bastaría: `SOCIAL-OG-MISSING` emite un hallazgo por cada propiedad que falta en la
//! misma página, y `HREFLANG-INVALID-CODE` uno por código mal escrito. Es precisamente
//! `group_key` quien los distingue (`og-missing:og:image`, `hreflang-code:es_es`). En las reglas
//! de duplicados el `group_key` es compartido entre las páginas que comparten título, así que un
//! título duplicado que cambia de texto sale como un hallazgo resuelto y otro nuevo — que es la
//! verdad: el grupo de duplicados ya no es el mismo.
//!
//! **3. Un rastreo truncado no permite afirmar ausencias.** Si el rastreo «después» se cortó
//! —el nivel gratuito corta a 1.000 URLs—, una URL que estaba antes y no está ahora pudo no
//! desaparecer: pudo no llegarse a ella. Lo mismo vale para un hallazgo «resuelto». Por eso,
//! cuando un lado está truncado o no terminó, **se suprimen las categorías que dependen de una
//! ausencia en ese lado** y se cuentan aparte en [`Suppressed`], el resultado se marca como no
//! concluyente y el aviso se imprime lo primero. Lo que sí se conserva es la intersección: las
//! URLs que ambos rastreos visitaron de verdad, con sus cambios de estado, de título y de
//! indexabilidad, que siguen siendo ciertos. Suprimir y avisar es mejor que enseñar
//! novecientas «URLs desaparecidas» que no han desaparecido: un diff que miente es peor que no
//! tener diff.
//!
//! **4. Dos sitios distintos no se comparan.** Si los `base_url` tienen origen distinto
//! (esquema + host), el emparejamiento por texto no casaría ni una URL y el resultado sería
//! «desapareció el sitio entero y apareció otro». Se rechaza con un error explícito en vez de
//! producir ese informe. Comparar producción contra pre-producción es una función legítima y
//! deseable, pero necesita reescribir el host antes de emparejar, y eso es trabajo aparte.
//!
//! # Qué compara y qué no
//!
//! Compara hallazgos, URLs internas, páginas HTML (título, meta description, canonical e
//! indexabilidad), `robots.txt` y sitemaps. **No** compara enlaces, imágenes, recursos ni
//! extracciones: son tablas de millones de filas cuyo diff útil es un agregado, no una lista, y
//! su sitio natural es un panel de cartera. Tampoco compara URLs externas, que solo
//! se piden con `follow_external` y por tanto casi nunca tienen estado que comparar.

use crate::i18n::{self, msg};
use anyhow::{bail, Context, Result};
use crawlforge_rules::{catalog, Lang, Severity};
use rusqlite::{Connection, OpenFlags};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Versión del esquema del fichero de diff. Se guarda en su tabla `schema_version`, igual que
/// hace el core con un fichero de rastreo, para que una interfaz pueda negarse a abrir uno
/// más nuevo de lo que entiende.
pub const DIFF_SCHEMA_VERSION: i64 = 1;

/// Cuántas URLs de ejemplo se listan por grupo en el resumen de terminal. Un informe que imprime
/// cuatrocientas URLs no se lee; el fichero de diff las tiene todas.
const MAX_EXAMPLES: usize = 3;

/// Severidades de mayor a menor. El orden es el rango: `SEVERITIES[0]` es la más grave.
const SEVERITIES: [Severity; 5] = [
    Severity::Critical,
    Severity::High,
    Severity::Medium,
    Severity::Low,
    Severity::Info,
];

/// Cuál de los dos rastreos. `antes` es el de referencia; `después`, el que se juzga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

impl Side {
    /// Cómo se nombra cada lado en la salida, en el idioma pedido.
    fn label(self, lang: Lang) -> String {
        match self {
            Self::Before => msg::side_before(lang),
            Self::After => msg::side_after(lang),
        }
    }
}

/// Un aviso sobre la comparación. Algunos son informativos; otros invalidan la conclusión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// Un rastreo se cortó por un límite. Las ausencias en ese lado no se pueden afirmar.
    Truncated { side: Side, reason: Option<String> },
    /// Un rastreo no terminó (`cancelled`, `failed`, `running`). Mismo efecto que el truncado.
    Unfinished { side: Side, status: String },
    /// El catálogo de reglas cambió entre los dos rastreos.
    RulesVersionChanged { before: String, after: String },
    /// La configuración del rastreo cambió en campos que afectan al alcance.
    ConfigChanged { fields: Vec<String> },
    /// Mismo origen pero distinta URL base: uno arrancó en una subcarpeta.
    ScopeChanged { before: String, after: String },
    /// Modos distintos: comparar un `dist/` con un rastreo HTTP mezcla dos cosas.
    ModeChanged { before: String, after: String },
    /// Una tabla que un fichero antiguo no tiene (migración posterior a él).
    MissingTable { side: Side, table: String },
    /// El rastreo pasado como «después» empezó antes que el «antes»: huele a argumentos del
    /// revés. No es un error —comparar hacia atrás puede ser deliberado— pero sin el aviso,
    /// «Ha empeorado» y «Ha mejorado» se leerían justo al contrario de lo que pasó.
    OrderInverted { before_started: String, after_started: String },
}

impl Warning {
    /// El texto que se le enseña a quien ejecuta el comando, en el idioma pedido.
    ///
    /// Los motivos de truncado (`max_urls`), los estados (`cancelled`), los modos y los campos
    /// de configuración son identificadores y viajan tal cual dentro del texto.
    pub fn message(&self, lang: Lang) -> String {
        match self {
            // `list_mode` no es un corte: un rastreo en modo lista nunca ve más que su
            // lista. La consecuencia para el diff es idéntica —las ausencias no se pueden
            // afirmar— pero decirle al usuario que su rastreo «está truncado» sería mentir.
            Self::Truncated { side, reason } if reason.as_deref() == Some("list_mode") => {
                msg::warn_list_mode(lang, side.label(lang))
            }
            Self::Truncated { side, reason } => msg::warn_truncated(
                lang,
                side.label(lang),
                reason
                    .as_deref()
                    .map(|r| msg::warn_truncated_by(lang, r))
                    .unwrap_or_default(),
            ),
            Self::Unfinished { side, status } => {
                msg::warn_unfinished(lang, side.label(lang), status)
            }
            Self::RulesVersionChanged { before, after } => {
                msg::warn_rules_changed(lang, before, after)
            }
            Self::ConfigChanged { fields } => msg::warn_config_changed(lang, fields.join(", ")),
            Self::ScopeChanged { before, after } => msg::warn_scope_changed(lang, before, after),
            Self::ModeChanged { before, after } => msg::warn_mode_changed(lang, before, after),
            Self::MissingTable { side, table } => {
                msg::warn_missing_table(lang, side.label(lang), table)
            }
            Self::OrderInverted { before_started, after_started } => {
                msg::warn_order_inverted(lang, before_started, after_started)
            }
        }
    }

    /// Si este aviso impide dar el diff por concluyente.
    pub fn breaks_conclusion(&self) -> bool {
        matches!(self, Self::Truncated { .. } | Self::Unfinished { .. })
    }
}

/// Los datos de `crawl_meta` que hacen falta para decidir si dos rastreos son comparables.
#[derive(Debug, Clone)]
pub struct CrawlInfo {
    pub path: String,
    pub crawl_id: String,
    pub project_name: String,
    pub base_url: String,
    pub mode: String,
    pub started_at: String,
    pub status: String,
    pub core_version: String,
    pub rules_version: String,
    pub config_json: String,
    pub truncated: bool,
    pub truncated_reason: Option<String>,
    pub schema_version: i64,
    /// URLs internas que se llegaron a resolver. No cuenta las `pending`, que en un rastreo
    /// truncado son las que quedaron en la cola.
    pub urls_total: i64,
    pub issues_total: i64,
}

impl CrawlInfo {
    /// Un rastreo del que no se pueden afirmar ausencias.
    fn incomplete(&self) -> bool {
        self.truncated || self.status != "done"
    }
}

/// Tipo de cambio.
///
/// Los diez primeros son los de `docs/02-MODELO-DATOS.md §5` literalmente. Los tres últimos
/// amplían aquella lista: `meta_description_changed` porque el documento nombra el título y el
/// canonical y se dejó la meta description, que se toca igual de a menudo; y los dos de
/// `robots.txt` y sitemaps porque sus tablas son de la migración 004, posterior al documento, y
/// el propio `docs/02-MODELO-DATOS.md §3.10` dice que existen justamente para esto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeType {
    UrlAdded,
    UrlRemoved,
    StatusChanged,
    TitleChanged,
    MetaDescriptionChanged,
    CanonicalChanged,
    IndexabilityLost,
    IndexabilityGained,
    IssueAppeared,
    IssueResolved,
    RobotsTxtChanged,
    SitemapChanged,
}

impl ChangeType {
    /// El valor que se escribe en `changes.change_type`. En inglés, como todo identificador
    /// (`CONVENTIONS.md §4`), y estable: hay una UI que va a consultar por él.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UrlAdded => "url_added",
            Self::UrlRemoved => "url_removed",
            Self::StatusChanged => "status_changed",
            Self::TitleChanged => "title_changed",
            Self::MetaDescriptionChanged => "meta_description_changed",
            Self::CanonicalChanged => "canonical_changed",
            Self::IndexabilityLost => "indexability_lost",
            Self::IndexabilityGained => "indexability_gained",
            Self::IssueAppeared => "issue_appeared",
            Self::IssueResolved => "issue_resolved",
            Self::RobotsTxtChanged => "robots_txt_changed",
            Self::SitemapChanged => "sitemap_changed",
        }
    }
}

/// Una fila del diff. Se corresponde una a una con la tabla `changes` del fichero de salida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub change_type: ChangeType,
    /// La URL afectada. `None` en los hallazgos de sitio (los que tienen `issues.url_id` nulo).
    /// En `robots_txt_changed` es el host, que es la clave de esa tabla.
    pub url: Option<String>,
    /// Qué campo cambió. En los cambios de hallazgo, el `rule_id`.
    pub field: Option<String>,
    pub value_before: Option<String>,
    pub value_after: Option<String>,
    /// Solo en los cambios de hallazgo (la severidad de la regla) y en un `robots.txt` que pasa
    /// a bloquearlo todo, que es un `critical` por derecho propio.
    pub severity: Option<String>,
}

impl Change {
    fn new(change_type: ChangeType) -> Self {
        Self { change_type, url: None, field: None, value_before: None, value_after: None, severity: None }
    }

    fn url(mut self, url: Option<String>) -> Self {
        // Una cadena vacía viene del `COALESCE` de los hallazgos de sitio: no es una URL.
        self.url = url.filter(|u| !u.is_empty());
        self
    }

    fn field(mut self, field: &str) -> Self {
        self.field = Some(field.to_string());
        self
    }

    fn values(mut self, before: Option<String>, after: Option<String>) -> Self {
        self.value_before = before;
        self.value_after = after;
        self
    }

    fn severity(mut self, severity: &str) -> Self {
        self.severity = Some(severity.to_string());
        self
    }
}

/// Lo que se dejó de contar por venir de un rastreo incompleto. No es ruido: saber que hay
/// «hasta 812 candidatos a URL desaparecida que no se pueden afirmar» es en sí un resultado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suppressed {
    pub urls_added: i64,
    pub urls_removed: i64,
    pub issues_appeared: i64,
    pub issues_resolved: i64,
}

impl Suppressed {
    pub fn any(&self) -> bool {
        self.urls_added + self.urls_removed + self.issues_appeared + self.issues_resolved > 0
    }
}

/// Una regla (o severidad) de `--fail-on` que se cumplió.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailOnHit {
    /// El token tal como lo escribió quien lanzó el comando.
    pub token: String,
    pub rule_id: String,
    pub severity: String,
    pub count: usize,
}

/// El resultado completo de una comparación.
#[derive(Debug, Clone)]
pub struct DiffOutcome {
    pub before: CrawlInfo,
    pub after: CrawlInfo,
    pub warnings: Vec<Warning>,
    pub changes: Vec<Change>,
    /// Hallazgos que estaban antes y siguen estando. No se escriben como filas del diff —serían
    /// decenas de miles y no son un cambio—, pero el recuento contextualiza los otros dos.
    pub issues_persisted: i64,
    /// URLs internas presentes en los dos rastreos: la base sobre la que sí se puede afirmar.
    pub urls_common: i64,
    pub suppressed: Suppressed,
    pub fail_on: Vec<FailOnHit>,
    /// Los tokens de `--fail-on` tal como se pidieron. Hace falta para poder decir «CUMPLE»:
    /// una puerta que no imprime nada cuando pasa no se distingue de una puerta que no se pidió.
    pub fail_on_requested: Vec<String>,
    /// Si `--fail-on` no puede pronunciarse porque los hallazgos nuevos se suprimieron.
    pub fail_on_inconclusive: bool,
    pub out_path: Option<PathBuf>,
}

impl DiffOutcome {
    /// Si se puede afirmar lo que dice. `false` cuando alguno de los dos rastreos está truncado
    /// o no terminó.
    pub fn conclusive(&self) -> bool {
        !self.warnings.iter().any(Warning::breaks_conclusion)
    }

    /// Si el proceso debe terminar con código distinto de cero. Solo lo decide `--fail-on`.
    pub fn should_fail(&self) -> bool {
        !self.fail_on.is_empty()
    }

    pub fn count(&self, change_type: ChangeType) -> usize {
        self.of(change_type).count()
    }

    /// Los cambios de un tipo, en el orden en que se generaron (por URL, alfabético).
    pub fn of(&self, change_type: ChangeType) -> impl Iterator<Item = &Change> {
        self.changes.iter().filter(move |c| c.change_type == change_type)
    }
}

// ─────────────────────────────────────────────────────────────────────── API

/// Compara dos rastreos y devuelve todo lo que cambió.
///
/// - `antes` y `despues` son ficheros `.sqlite` producidos por `crawl`, `audit` o `list`. Se
///   abren en solo lectura; ninguno se modifica.
/// - `out`, si se indica, escribe el diff como fichero SQLite propio (ver [`DIFF_SCHEMA_VERSION`]).
/// - `fail_on` admite IDs de regla (`HTTP-404-INTERNAL`) y severidades (`critical`). Una
///   severidad significa «esa o peor», que es lo que un pipeline quiere de verdad: quien pide
///   fallar ante un `high` no quiere que un `critical` pase. Un token que no existe es un error,
///   no un silencio: una errata en el YAML de CI que nunca falla es peor que no tener la puerta.
///
/// No imprime nada. La presentación es [`print_report`].
pub fn compare(
    antes: &Path,
    despues: &Path,
    out: Option<&Path>,
    fail_on: &[String],
) -> Result<DiffOutcome> {
    // El idioma se resuelve una vez por comparación (`--lang` > `CRAWLFORGE_LANG` > inglés)
    // y gobierna tanto el informe como los errores que ve el usuario.
    let lang = i18n::current_lang();
    anyhow::ensure!(antes.is_file(), "{}", msg::error_missing_file(lang, antes.display()));
    anyhow::ensure!(despues.is_file(), "{}", msg::error_missing_file(lang, despues.display()));

    // Se valida antes de trabajar: si un fichero no es un rastreo —un diff de `diff --out`, una
    // base de otro programa— o el token está mal escrito, mejor saberlo ya y con sus palabras.
    crate::store_check::ensure_crawl_store(antes)?;
    crate::store_check::ensure_crawl_store(despues)?;
    let tokens = parse_fail_on(lang, fail_on)?;

    let conn = open_pair(antes, despues)?;
    let before = read_crawl_info(&conn, "a", antes, lang)?;
    let after = read_crawl_info(&conn, "b", despues, lang)?;

    ensure_same_site(lang, &before, &after)?;
    let warnings = collect_warnings(&conn, &before, &after)?;

    // Una ausencia solo se puede afirmar si el lado donde falta se rastreó entero.
    let suppress_additions = before.incomplete();
    let suppress_removals = after.incomplete();

    let mut changes = Vec::new();
    let mut suppressed = Suppressed::default();

    let urls_common = diff_urls(
        &conn,
        &mut changes,
        &mut suppressed,
        suppress_additions,
        suppress_removals,
    )?;
    diff_pages(&conn, &mut changes)?;
    let issues_persisted = diff_issues(
        &conn,
        &mut changes,
        &mut suppressed,
        suppress_additions,
        suppress_removals,
    )?;
    // Una tabla que uno de los dos ficheros no tiene —porque es de una migración posterior a
    // él— no se compara: el aviso ya lo dice y media comparación diría que todo es nuevo.
    let comparable = |tabla: &str| {
        !warnings.iter().any(|w| matches!(w, Warning::MissingTable { table, .. } if table == tabla))
    };
    if comparable("robots_txt") {
        diff_robots(&conn, &mut changes)?;
    }
    if comparable("sitemaps") {
        diff_sitemaps(&conn, &mut changes)?;
    }

    let fail_on_hits = evaluate_fail_on(&changes, &tokens);

    let mut outcome = DiffOutcome {
        before,
        after,
        warnings,
        changes,
        issues_persisted,
        urls_common,
        suppressed,
        fail_on: fail_on_hits,
        fail_on_requested: tokens.iter().map(FailToken::token).collect(),
        fail_on_inconclusive: !tokens.is_empty() && suppress_additions,
        out_path: None,
    };

    if let Some(path) = out {
        write_diff_file(&conn, &outcome, path)
            .with_context(|| format!("write the diff to {}", path.display()))?;
        outcome.out_path = Some(path.to_path_buf());
    }

    Ok(outcome)
}

// ────────────────────────────────────────────────────────── Apertura y metadatos

/// Abre una conexión en memoria con los dos rastreos adjuntos en solo lectura.
///
/// Es lo que manda `docs/02-MODELO-DATOS.md §5`: «No hay formato propietario: se hace con
/// `ATTACH`». Así el emparejamiento lo resuelve SQLite con sus índices y no hay que cargar dos
/// rastreos enteros en memoria — que es el antipatrón número dos de `CONVENTIONS.md §5`.
fn open_pair(antes: &Path, despues: &Path) -> Result<Connection> {
    let conn = Connection::open_in_memory_with_flags(
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.execute_batch("PRAGMA temp_store = MEMORY;")?;
    conn.execute("ATTACH DATABASE ?1 AS a", [read_only_uri(antes)?])
        .with_context(|| format!("open {} read-only", antes.display()))?;
    conn.execute("ATTACH DATABASE ?1 AS b", [read_only_uri(despues)?])
        .with_context(|| format!("open {} read-only", despues.display()))?;
    Ok(conn)
}

/// URI `file:` en modo solo lectura. `ATTACH` hereda por defecto los permisos de la conexión
/// principal, que es de escritura; el `mode=ro` es lo que garantiza que un diff no pueda tocar un
/// fichero de rastreo.
fn read_only_uri(path: &Path) -> Result<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut encoded = encode_uri_path(&absolute.to_string_lossy());
    if !encoded.starts_with('/') {
        // Windows: `C:/x` tiene que quedar como `file:///C:/x`.
        encoded.insert(0, '/');
    }
    Ok(format!("file://{encoded}?mode=ro"))
}

/// Escapa lo que SQLite interpretaría en una URI. Las barras invertidas de Windows pasan a
/// barras normales, que es lo que exige el formato.
fn encode_uri_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            '%' => out.push_str("%25"),
            '?' => out.push_str("%3F"),
            '#' => out.push_str("%23"),
            ' ' => out.push_str("%20"),
            '\\' => out.push('/'),
            _ => out.push(c),
        }
    }
    out
}

fn has_table(conn: &Connection, schema: &str, table: &str) -> Result<bool> {
    let sql = format!("SELECT COUNT(*) FROM {schema}.sqlite_master WHERE type = 'table' AND name = ?1");
    let n: i64 = conn.query_row(&sql, [table], |r| r.get(0))?;
    Ok(n > 0)
}

fn has_column(conn: &Connection, schema: &str, table: &str, column: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1, ?2) WHERE name = ?3",
        [table, schema, column],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Lee `crawl_meta` tolerando esquemas antiguos: `truncated` es de la migración 002, y un
/// fichero de rastreo de hace un año tiene que seguir abriéndose (`CONVENTIONS.md §4`).
fn read_crawl_info(conn: &Connection, schema: &str, path: &Path, lang: Lang) -> Result<CrawlInfo> {
    let no_es_rastreo = || msg::error_not_a_crawl(lang, path.display());

    let schema_version: i64 = conn
        .query_row(&format!("SELECT COALESCE(MAX(version), 0) FROM {schema}.schema_version"), [], |r| {
            r.get(0)
        })
        .with_context(no_es_rastreo)?;

    let truncated_col = has_column(conn, schema, "crawl_meta", "truncated")?;
    let sql = format!(
        "SELECT id, project_name, base_url, mode, started_at, status, core_version,
                rules_version, config_json, {}, {}
         FROM {schema}.crawl_meta LIMIT 1",
        if truncated_col { "truncated" } else { "0" },
        if truncated_col { "truncated_reason" } else { "NULL" },
    );
    let mut info = conn
        .query_row(&sql, [], |r| {
            Ok(CrawlInfo {
                path: path.display().to_string(),
                crawl_id: r.get(0)?,
                project_name: r.get(1)?,
                base_url: r.get(2)?,
                mode: r.get(3)?,
                started_at: r.get(4)?,
                status: r.get(5)?,
                core_version: r.get(6)?,
                rules_version: r.get(7)?,
                config_json: r.get(8)?,
                truncated: r.get::<_, i64>(9)? != 0,
                truncated_reason: r.get(10)?,
                schema_version,
                urls_total: 0,
                issues_total: 0,
            })
        })
        .with_context(no_es_rastreo)?;

    info.urls_total = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM {schema}.urls WHERE is_internal = 1 AND crawl_state <> 'pending'"
        ),
        [],
        |r| r.get(0),
    )?;
    info.issues_total =
        conn.query_row(&format!("SELECT COUNT(*) FROM {schema}.issues"), [], |r| r.get(0))?;

    Ok(info)
}

/// Esquema + host de una URL base, en minúsculas. Sin `url::Url` a propósito: la CLI no depende
/// de ese crate y aquí solo hace falta el origen.
fn origin(base_url: &str) -> String {
    let (scheme, rest) = base_url.split_once("://").unwrap_or(("", base_url));
    let host = rest.split('/').next().unwrap_or("");
    format!("{}://{}", scheme.to_ascii_lowercase(), host.to_ascii_lowercase())
}

/// Rechaza comparar dos sitios distintos. Ver la decisión 4 de la documentación del módulo.
fn ensure_same_site(lang: Lang, before: &CrawlInfo, after: &CrawlInfo) -> Result<()> {
    let (a, b) = (origin(&before.base_url), origin(&after.base_url));
    if a != b {
        bail!(msg::error_different_sites(lang, a, b));
    }
    Ok(())
}

fn collect_warnings(conn: &Connection, before: &CrawlInfo, after: &CrawlInfo) -> Result<Vec<Warning>> {
    let mut warnings = Vec::new();

    for (side, info) in [(Side::Before, before), (Side::After, after)] {
        if info.truncated {
            warnings.push(Warning::Truncated { side, reason: info.truncated_reason.clone() });
        } else if info.status != "done" {
            warnings.push(Warning::Unfinished { side, status: info.status.clone() });
        }
    }

    // Los `started_at` son ISO 8601 del propio motor, así que comparan bien como texto.
    if !before.started_at.is_empty()
        && !after.started_at.is_empty()
        && after.started_at < before.started_at
    {
        warnings.push(Warning::OrderInverted {
            before_started: before.started_at.clone(),
            after_started: after.started_at.clone(),
        });
    }

    if before.rules_version != after.rules_version {
        warnings.push(Warning::RulesVersionChanged {
            before: before.rules_version.clone(),
            after: after.rules_version.clone(),
        });
    }
    if before.mode != after.mode {
        warnings
            .push(Warning::ModeChanged { before: before.mode.clone(), after: after.mode.clone() });
    }
    if before.base_url != after.base_url {
        warnings.push(Warning::ScopeChanged {
            before: before.base_url.clone(),
            after: after.base_url.clone(),
        });
    }
    let fields = config_differences(&before.config_json, &after.config_json);
    if !fields.is_empty() {
        warnings.push(Warning::ConfigChanged { fields });
    }

    // Tablas de migraciones posteriores a uno de los dos ficheros.
    for table in ["robots_txt", "sitemaps"] {
        for (side, schema) in [(Side::Before, "a"), (Side::After, "b")] {
            if !has_table(conn, schema, table)? {
                warnings.push(Warning::MissingTable { side, table: table.to_string() });
            }
        }
    }

    Ok(warnings)
}

/// Campos de `config_json` que difieren, en notación con puntos.
///
/// `docs/02-MODELO-DATOS.md §5` lo exige: sin esto se le atribuyen al sitio cambios que en
/// realidad son de configuración —bajar `max_urls` «borra» media web—. El recorrido es genérico
/// para no tener que actualizar esta función cada vez que `CrawlJob` gane un campo.
fn config_differences(before: &str, after: &str) -> Vec<String> {
    let (Ok(a), Ok(b)) = (
        serde_json::from_str::<serde_json::Value>(before),
        serde_json::from_str::<serde_json::Value>(after),
    ) else {
        return if before == after { Vec::new() } else { vec!["config_json".to_string()] };
    };

    let mut fields = Vec::new();
    walk_json_diff("", &a, &b, &mut fields);
    fields.sort();
    fields.dedup();
    fields
}

fn walk_json_diff(prefix: &str, a: &serde_json::Value, b: &serde_json::Value, out: &mut Vec<String>) {
    match (a, b) {
        (serde_json::Value::Object(oa), serde_json::Value::Object(ob)) => {
            let mut keys: Vec<&String> = oa.keys().chain(ob.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let path =
                    if prefix.is_empty() { key.to_string() } else { format!("{prefix}.{key}") };
                let null = serde_json::Value::Null;
                walk_json_diff(
                    &path,
                    oa.get(key).unwrap_or(&null),
                    ob.get(key).unwrap_or(&null),
                    out,
                );
            }
        }
        _ if a != b => {
            out.push(if prefix.is_empty() { "config_json" } else { prefix }.to_string())
        }
        _ => {}
    }
}

// ─────────────────────────────────────────────────────────────────────── URLs

/// Universo de comparación: URLs internas que el rastreo llegó a resolver.
///
/// Las `pending` quedan fuera aposta. En un rastreo truncado son las que se quedaron en la cola:
/// contarlas sería decir que existen 40.000 URLs cuando solo se miraron 1.000. Las `skipped` son
/// externas no pedidas, y ya las excluye `is_internal`.
fn url_universe(alias: &str) -> String {
    format!("{alias}.is_internal = 1 AND {alias}.crawl_state <> 'pending'")
}

/// Devuelve cuántas URLs internas tienen los dos rastreos en común.
fn diff_urls(
    conn: &Connection,
    changes: &mut Vec<Change>,
    suppressed: &mut Suppressed,
    suppress_additions: bool,
    suppress_removals: bool,
) -> Result<i64> {
    let (antes, despues) = (url_universe("ua"), url_universe("ub"));

    // Nuevas: están en «después» y no en «antes».
    let sql = format!(
        "SELECT ub.url, ub.status_code
         FROM b.urls ub
         LEFT JOIN a.urls ua ON ua.url = ub.url AND {antes}
         WHERE {despues} AND ua.id IS NULL
         ORDER BY ub.url"
    );
    let added = collect_url_rows(conn, &sql)?;
    if suppress_additions {
        suppressed.urls_added = added.len() as i64;
    } else {
        for (url, status) in added {
            changes.push(
                Change::new(ChangeType::UrlAdded)
                    .url(Some(url))
                    .field("status_code")
                    .values(None, status.map(|s| s.to_string())),
            );
        }
    }

    // Desaparecidas: estaban en «antes» y no están en «después».
    let sql = format!(
        "SELECT ua.url, ua.status_code
         FROM a.urls ua
         LEFT JOIN b.urls ub ON ub.url = ua.url AND {despues}
         WHERE {antes} AND ub.id IS NULL
         ORDER BY ua.url"
    );
    let removed = collect_url_rows(conn, &sql)?;
    if suppress_removals {
        suppressed.urls_removed = removed.len() as i64;
    } else {
        for (url, status) in removed {
            changes.push(
                Change::new(ChangeType::UrlRemoved)
                    .url(Some(url))
                    .field("status_code")
                    .values(status.map(|s| s.to_string()), None),
            );
        }
    }

    // Cambios de estado. Estos sí valen aunque el rastreo esté truncado: son URLs que ambos
    // rastreos visitaron de verdad.
    let sql = format!(
        "SELECT ub.url, ua.status_code, ub.status_code
         FROM b.urls ub
         JOIN a.urls ua ON ua.url = ub.url
         WHERE {despues} AND {antes}
           AND ua.status_code IS NOT ub.status_code
         ORDER BY ub.url"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<i64>>(2)?))
    })?;
    for row in rows {
        let (url, before, after) = row?;
        changes.push(
            Change::new(ChangeType::StatusChanged)
                .url(Some(url))
                .field("status_code")
                .values(before.map(|s| s.to_string()), after.map(|s| s.to_string())),
        );
    }

    let common: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM b.urls ub JOIN a.urls ua ON ua.url = ub.url
             WHERE {despues} AND {antes}"
        ),
        [],
        |r| r.get(0),
    )?;
    Ok(common)
}

fn collect_url_rows(conn: &Connection, sql: &str) -> Result<Vec<(String, Option<i64>)>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

// ────────────────────────────────────────────────────────────────────── Páginas

/// Una fila de la comparación de páginas. Struct y no tupla de once elementos porque clippy
/// tiene razón sobre las tuplas de once elementos.
struct PageRow {
    url: String,
    title: (Option<String>, Option<String>),
    meta_description: (Option<String>, Option<String>),
    canonical: (Option<String>, Option<String>),
    indexable: (i64, i64),
    reason: (Option<String>, Option<String>),
}

fn diff_pages(conn: &Connection, changes: &mut Vec<Change>) -> Result<()> {
    let sql = "SELECT ub.url,
                      pa.title, pb.title,
                      pa.meta_description, pb.meta_description,
                      pa.canonical, pb.canonical,
                      pa.is_indexable, pb.is_indexable,
                      pa.indexability_reason, pb.indexability_reason
               FROM b.pages pb
               JOIN b.urls ub ON ub.id = pb.url_id
               JOIN a.urls ua ON ua.url = ub.url
               JOIN a.pages pa ON pa.url_id = ua.id
               WHERE pa.title            IS NOT pb.title
                  OR pa.meta_description IS NOT pb.meta_description
                  OR pa.canonical        IS NOT pb.canonical
                  OR pa.is_indexable     IS NOT pb.is_indexable
               ORDER BY ub.url";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok(PageRow {
            url: r.get(0)?,
            title: (r.get(1)?, r.get(2)?),
            meta_description: (r.get(3)?, r.get(4)?),
            canonical: (r.get(5)?, r.get(6)?),
            indexable: (r.get(7)?, r.get(8)?),
            reason: (r.get(9)?, r.get(10)?),
        })
    })?;

    for row in rows {
        let row = row?;
        let url = Some(row.url);

        for (change_type, field, (before, after)) in [
            (ChangeType::TitleChanged, "title", row.title),
            (ChangeType::MetaDescriptionChanged, "meta_description", row.meta_description),
            (ChangeType::CanonicalChanged, "canonical", row.canonical),
        ] {
            if before != after {
                changes.push(
                    Change::new(change_type)
                        .url(url.clone())
                        .field(field)
                        .values(before, after),
                );
            }
        }

        // Solo la transición importa. Que cambie el motivo entre dos páginas que siguen sin
        // indexarse no es lo que arruina un despliegue.
        let (ia, ib) = row.indexable;
        let change_type = match (ia, ib) {
            (1, 0) => Some(ChangeType::IndexabilityLost),
            (0, 1) => Some(ChangeType::IndexabilityGained),
            _ => None,
        };
        if let Some(change_type) = change_type {
            let motivo = |indexable: i64, reason: Option<String>| {
                if indexable == 1 {
                    Some("indexable".to_string())
                } else {
                    Some(reason.unwrap_or_else(|| "no indexable".to_string()))
                }
            };
            changes.push(
                Change::new(change_type)
                    .url(url.clone())
                    .field("indexability_reason")
                    .values(motivo(ia, row.reason.0), motivo(ib, row.reason.1)),
            );
        }
    }
    Ok(())
}

// ───────────────────────────────────────────────────────────────────── Hallazgos

/// Devuelve cuántos hallazgos siguen presentes en los dos rastreos.
///
/// El emparejamiento se hace sobre tablas temporales indexadas por la terna `(rule_id, url,
/// group_key)`. Sin el índice, con 50.000 hallazgos por lado esto sería un producto cartesiano.
fn diff_issues(
    conn: &Connection,
    changes: &mut Vec<Change>,
    suppressed: &mut Suppressed,
    suppress_additions: bool,
    suppress_removals: bool,
) -> Result<i64> {
    for (schema, temp) in [("a", "issues_a"), ("b", "issues_b")] {
        conn.execute_batch(&format!(
            "CREATE TEMP TABLE {temp} AS
                 SELECT DISTINCT i.rule_id AS rule_id, i.severity AS severity,
                        COALESCE(u.url, '') AS url, COALESCE(i.group_key, '') AS group_key
                 FROM {schema}.issues i
                 LEFT JOIN {schema}.urls u ON u.id = i.url_id;
             CREATE INDEX idx_{temp} ON {temp}(rule_id, url, group_key);"
        ))?;
    }

    // Nuevos: en «después» y no en «antes».
    //
    // Un `group_key` vacío en **uno** de los lados empareja con cualquiera del otro: es el caso
    // del rastreo hecho por una versión que aún no poblaba la clave en esa regla. Sin este
    // comodín, el primer diff tras actualizar reportaría cada hallazgo de plantilla como nuevo
    // *y* resuelto a la vez —18.092 × 2 filas por regla en un rastreo real— por un cambio del
    // programa, no del sitio. Cuando los dos lados traen clave, manda la igualdad estricta: un
    // título duplicado que cambia de grupo sigue siendo un cambio real.
    let appeared = collect_issue_rows(
        conn,
        "SELECT ib.rule_id, ib.severity, ib.url, ib.group_key
         FROM issues_b ib
         LEFT JOIN issues_a ia
           ON ia.rule_id = ib.rule_id AND ia.url = ib.url
          AND (ia.group_key = ib.group_key OR ia.group_key = '' OR ib.group_key = '')
         WHERE ia.rule_id IS NULL
         ORDER BY ib.rule_id, ib.url",
    )?;
    if suppress_additions {
        suppressed.issues_appeared = appeared.len() as i64;
    } else {
        for row in appeared {
            changes.push(
                Change::new(ChangeType::IssueAppeared)
                    .url(Some(row.url))
                    .field(&row.rule_id)
                    .severity(&row.severity)
                    .values(None, non_empty(row.group_key)),
            );
        }
    }

    // Resueltos: estaban en «antes» y no están en «después».
    let resolved = collect_issue_rows(
        conn,
        "SELECT ia.rule_id, ia.severity, ia.url, ia.group_key
         FROM issues_a ia
         LEFT JOIN issues_b ib
           ON ib.rule_id = ia.rule_id AND ib.url = ia.url
          AND (ib.group_key = ia.group_key OR ib.group_key = '' OR ia.group_key = '')
         WHERE ib.rule_id IS NULL
         ORDER BY ia.rule_id, ia.url",
    )?;
    if suppress_removals {
        suppressed.issues_resolved = resolved.len() as i64;
    } else {
        for row in resolved {
            changes.push(
                Change::new(ChangeType::IssueResolved)
                    .url(Some(row.url))
                    .field(&row.rule_id)
                    .severity(&row.severity)
                    .values(non_empty(row.group_key), None),
            );
        }
    }

    let persisted: i64 = conn.query_row(
        "SELECT COUNT(*) FROM issues_b ib
         JOIN issues_a ia
           ON ia.rule_id = ib.rule_id AND ia.url = ib.url
          AND (ia.group_key = ib.group_key OR ia.group_key = '' OR ib.group_key = '')",
        [],
        |r| r.get(0),
    )?;
    Ok(persisted)
}

struct IssueRow {
    rule_id: String,
    severity: String,
    url: String,
    group_key: String,
}

fn collect_issue_rows(conn: &Connection, sql: &str) -> Result<Vec<IssueRow>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(IssueRow {
                rule_id: r.get(0)?,
                severity: r.get(1)?,
                url: r.get(2)?,
                group_key: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

// ──────────────────────────────────────────────────────── robots.txt y sitemaps

/// Un `robots.txt` que cambia entre dos rastreos es una alerta de primer orden: el accidente de
/// subir el `Disallow: /` de pre-producción es la forma más rápida y silenciosa de desaparecer
/// de Google (`docs/02-MODELO-DATOS.md §3.10`).
fn diff_robots(conn: &Connection, changes: &mut Vec<Change>) -> Result<()> {
    let sql = "SELECT h.host, ra.status_code, rb.status_code, ra.content, rb.content,
                      ra.blocks_all, rb.blocks_all
               FROM (SELECT host FROM a.robots_txt UNION SELECT host FROM b.robots_txt) h
               LEFT JOIN a.robots_txt ra ON ra.host = h.host
               LEFT JOIN b.robots_txt rb ON rb.host = h.host
               ORDER BY h.host";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<i64>>(6)?,
        ))
    })?;

    for row in rows {
        let (host, status_a, status_b, content_a, content_b, blocks_a, blocks_b) = row?;
        let host = Some(host);

        if blocks_a != blocks_b {
            let change = Change::new(ChangeType::RobotsTxtChanged)
                .url(host.clone())
                .field("blocks_all")
                .values(blocks_a.map(|v| v.to_string()), blocks_b.map(|v| v.to_string()));
            // Pasar a bloquearlo todo no es un cambio más: es el peor hallazgo del catálogo.
            changes.push(if blocks_b == Some(1) { change.severity("critical") } else { change });
        }
        if status_a != status_b {
            changes.push(
                Change::new(ChangeType::RobotsTxtChanged)
                    .url(host.clone())
                    .field("status_code")
                    .values(status_a.map(|v| v.to_string()), status_b.map(|v| v.to_string())),
            );
        }
        if content_a != content_b {
            changes.push(
                Change::new(ChangeType::RobotsTxtChanged)
                    .url(host)
                    .field("content")
                    .values(content_a, content_b),
            );
        }
    }
    Ok(())
}

fn diff_sitemaps(conn: &Connection, changes: &mut Vec<Change>) -> Result<()> {
    let sql = "SELECT s.url, sa.url, sb.url, sa.status_code, sb.status_code,
                      sa.is_valid, sb.is_valid, sa.url_count, sb.url_count
               FROM (SELECT url FROM a.sitemaps UNION SELECT url FROM b.sitemaps) s
               LEFT JOIN a.sitemaps sa ON sa.url = s.url
               LEFT JOIN b.sitemaps sb ON sb.url = s.url
               ORDER BY s.url";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?.is_some(),
            r.get::<_, Option<String>>(2)?.is_some(),
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<i64>>(8)?,
        ))
    })?;

    for row in rows {
        let (url, in_a, in_b, status_a, status_b, valid_a, valid_b, count_a, count_b) = row?;
        let url = Some(url);
        let si_no = |v: bool| Some(if v { "yes" } else { "no" }.to_string());

        if in_a != in_b {
            changes.push(
                Change::new(ChangeType::SitemapChanged)
                    .url(url.clone())
                    .field("present")
                    .values(si_no(in_a), si_no(in_b)),
            );
            continue; // Un sitemap que aparece o desaparece no tiene «cambios de estado».
        }
        for (field, before, after) in [
            ("status_code", status_a, status_b),
            ("is_valid", valid_a, valid_b),
            ("url_count", count_a, count_b),
        ] {
            if before != after {
                changes.push(
                    Change::new(ChangeType::SitemapChanged)
                        .url(url.clone())
                        .field(field)
                        .values(before.map(|v| v.to_string()), after.map(|v| v.to_string())),
                );
            }
        }
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────── `--fail-on`

/// Un token de `--fail-on` ya validado.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FailToken {
    /// Falla si aparece un hallazgo nuevo de esta regla.
    Rule { token: String, rule_id: String },
    /// Falla si aparece un hallazgo nuevo de esta severidad **o peor**.
    Severity { token: String, rank: usize },
}

impl FailToken {
    fn token(&self) -> String {
        match self {
            Self::Rule { token, .. } | Self::Severity { token, .. } => token.clone(),
        }
    }
}

fn parse_fail_on(lang: Lang, tokens: &[String]) -> Result<Vec<FailToken>> {
    let mut out = Vec::new();
    for token in tokens {
        let limpio = token.trim();
        if limpio.is_empty() {
            continue;
        }
        if let Some(rank) = SEVERITIES.iter().position(|s| s.as_str() == limpio.to_ascii_lowercase())
        {
            out.push(FailToken::Severity { token: limpio.to_string(), rank });
            continue;
        }
        let id = limpio.to_ascii_uppercase();
        match catalog().into_iter().find(|m| m.id == id) {
            Some(meta) => {
                out.push(FailToken::Rule { token: limpio.to_string(), rule_id: meta.id.to_string() })
            }
            None => bail!(msg::error_fail_on_unknown(lang, limpio)),
        }
    }
    Ok(out)
}

fn severity_rank(name: &str) -> usize {
    SEVERITIES.iter().position(|s| s.as_str() == name).unwrap_or(SEVERITIES.len())
}

/// Solo los hallazgos **nuevos** disparan la puerta. Uno que ya estaba antes no lo introdujo
/// este despliegue, y una puerta que falla por deuda vieja se desactiva a la semana.
fn evaluate_fail_on(changes: &[Change], tokens: &[FailToken]) -> Vec<FailOnHit> {
    if tokens.is_empty() {
        return Vec::new();
    }

    // (regla, severidad) → cuántos hallazgos nuevos.
    let mut nuevos: BTreeMap<(String, String), usize> = BTreeMap::new();
    for change in changes.iter().filter(|c| c.change_type == ChangeType::IssueAppeared) {
        let rule_id = change.field.clone().unwrap_or_default();
        let severity = change.severity.clone().unwrap_or_default();
        *nuevos.entry((rule_id, severity)).or_default() += 1;
    }

    let mut hits = Vec::new();
    for token in tokens {
        for ((rule_id, severity), count) in &nuevos {
            let (matches, token_text) = match token {
                FailToken::Rule { token, rule_id: wanted } => (rule_id == wanted, token),
                FailToken::Severity { token, rank } => (severity_rank(severity) <= *rank, token),
            };
            if matches {
                hits.push(FailOnHit {
                    token: token_text.clone(),
                    rule_id: rule_id.clone(),
                    severity: severity.clone(),
                    count: *count,
                });
            }
        }
    }
    hits
}

// ─────────────────────────────────────────────────────────── Fichero de diff

/// Esquema del fichero de diff.
///
/// `docs/02-MODELO-DATOS.md §5` define `changes` y es normativo, así que se respeta columna por
/// columna. Lo que añade este módulo es `diff_meta`: un diff sin saber de qué dos rastreos sale,
/// ni si alguno estaba truncado, es un diff que dentro de un mes nadie puede interpretar — y con
/// el aviso de truncado fuera del fichero, una interfaz presentaría como hechos unos
/// recuentos que no lo son.
const DIFF_SCHEMA: &str = "
CREATE TABLE schema_version (
    version    INTEGER NOT NULL,
    applied_at TEXT    NOT NULL
);

CREATE TABLE diff_meta (
    id                      INTEGER PRIMARY KEY CHECK (id = 1),
    generated_at            TEXT    NOT NULL,
    tool_version            TEXT    NOT NULL,
    -- 0 si alguno de los dos rastreos está truncado o no terminó. Ver `warnings_json`.
    conclusive              INTEGER NOT NULL,
    warnings_json           TEXT    NOT NULL,
    -- Recuentos que no son cambios pero sin los cuales los cambios no se interpretan.
    urls_common             INTEGER NOT NULL,
    issues_persisted        INTEGER NOT NULL,
    -- Candidatos ocultos por venir de un rastreo incompleto.
    suppressed_urls_added   INTEGER NOT NULL,
    suppressed_urls_removed INTEGER NOT NULL,
    suppressed_issues_appeared INTEGER NOT NULL,
    suppressed_issues_resolved INTEGER NOT NULL,

    before_path             TEXT    NOT NULL,
    before_crawl_id         TEXT    NOT NULL,
    before_base_url         TEXT    NOT NULL,
    before_started_at       TEXT    NOT NULL,
    before_truncated        INTEGER NOT NULL,
    before_truncated_reason TEXT,
    before_core_version     TEXT    NOT NULL,
    before_rules_version    TEXT    NOT NULL,
    before_urls             INTEGER NOT NULL,
    before_issues           INTEGER NOT NULL,

    after_path              TEXT    NOT NULL,
    after_crawl_id          TEXT    NOT NULL,
    after_base_url          TEXT    NOT NULL,
    after_started_at        TEXT    NOT NULL,
    after_truncated         INTEGER NOT NULL,
    after_truncated_reason  TEXT,
    after_core_version      TEXT    NOT NULL,
    after_rules_version     TEXT    NOT NULL,
    after_urls              INTEGER NOT NULL,
    after_issues            INTEGER NOT NULL
);

CREATE TABLE changes (
    id           INTEGER PRIMARY KEY,
    change_type  TEXT NOT NULL,
    url          TEXT,
    field        TEXT,
    value_before TEXT,
    value_after  TEXT,
    severity     TEXT
);

CREATE INDEX idx_changes_type ON changes(change_type);
CREATE INDEX idx_changes_url  ON changes(url) WHERE url IS NOT NULL;

CREATE VIEW v_change_summary AS
SELECT change_type, severity, COUNT(*) AS n
FROM changes GROUP BY change_type, severity;
";

fn write_diff_file(source: &Connection, outcome: &DiffOutcome, out: &Path) -> Result<()> {
    if out.exists() {
        std::fs::remove_file(out)?;
    }
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let generated_at: String =
        source.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |r| r.get(0))?;

    let mut conn = Connection::open(out)?;
    conn.execute_batch(DIFF_SCHEMA)?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
        rusqlite::params![DIFF_SCHEMA_VERSION, generated_at],
    )?;

    // El fichero de diff guarda los avisos **en inglés, siempre**: es un dato que la UI de la
    // se leerá meses después, y hornear dentro el idioma del terminal que lo generó haría
    // que el mismo fichero dijera cosas distintas según quién lo hubiera creado. Quien lo
    // presente lo traducirá con su propio catálogo.
    let warnings_json = serde_json::to_string(
        &outcome
            .warnings
            .iter()
            .map(|w| {
                serde_json::json!({
                    "message": w.message(Lang::En),
                    "breaks_conclusion": w.breaks_conclusion(),
                })
            })
            .collect::<Vec<_>>(),
    )?;

    let (b, a) = (&outcome.before, &outcome.after);
    conn.execute(
        "INSERT INTO diff_meta (
             id, generated_at, tool_version, conclusive, warnings_json,
             urls_common, issues_persisted,
             suppressed_urls_added, suppressed_urls_removed,
             suppressed_issues_appeared, suppressed_issues_resolved,
             before_path, before_crawl_id, before_base_url, before_started_at,
             before_truncated, before_truncated_reason, before_core_version,
             before_rules_version, before_urls, before_issues,
             after_path, after_crawl_id, after_base_url, after_started_at,
             after_truncated, after_truncated_reason, after_core_version,
             after_rules_version, after_urls, after_issues)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                 ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                 ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30)",
        rusqlite::params![
            generated_at,
            env!("CARGO_PKG_VERSION"),
            outcome.conclusive() as i64,
            warnings_json,
            outcome.urls_common,
            outcome.issues_persisted,
            outcome.suppressed.urls_added,
            outcome.suppressed.urls_removed,
            outcome.suppressed.issues_appeared,
            outcome.suppressed.issues_resolved,
            b.path,
            b.crawl_id,
            b.base_url,
            b.started_at,
            b.truncated as i64,
            b.truncated_reason,
            b.core_version,
            b.rules_version,
            b.urls_total,
            b.issues_total,
            a.path,
            a.crawl_id,
            a.base_url,
            a.started_at,
            a.truncated as i64,
            a.truncated_reason,
            a.core_version,
            a.rules_version,
            a.urls_total,
            a.issues_total,
        ],
    )?;

    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO changes (change_type, url, field, value_before, value_after, severity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for change in &outcome.changes {
            stmt.execute(rusqlite::params![
                change.change_type.as_str(),
                change.url,
                change.field,
                change.value_before,
                change.value_after,
                change.severity,
            ])?;
        }
    }
    tx.commit()?;
    conn.execute_batch("PRAGMA optimize;")?;
    Ok(())
}

// ───────────────────────────────────────────────────────────────── Presentación

/// Imprime el diff en terminal: primero lo que ha empeorado, que es lo que se mira.
///
/// El idioma sale de [`i18n::current_lang`] (`--lang` > `CRAWLFORGE_LANG` > inglés) y se
/// resuelve una sola vez aquí. Lo que no se traduce: URLs, hosts, IDs de regla, códigos de
/// estado, nombres de campo (`status_code`, `blocks_all`) y las marcas de tiempo — son datos.
pub fn print_report(outcome: &DiffOutcome) {
    let lang = i18n::current_lang();
    print_header(outcome, lang);
    print_warnings(outcome, lang);
    print_worse(outcome, lang);
    print_better(outcome, lang);
    print_other(outcome, lang);
    print_suppressed(outcome, lang);
    print_gate(outcome, lang);
}

/// Una línea de recuento con el número pegado a la columna de la derecha, mida lo que mida la
/// etiqueta en cualquiera de los idiomas. Los anchos fijos por línea del diseño anterior
/// estaban calibrados a mano para el inglés y descuadraban con cualquier otra etiqueta.
/// Cuenta caracteres, no bytes: «Códigos» mide 7, no 8.
fn count_line(indent: usize, label: &str, value: &str) -> String {
    /// Columna (en caracteres) donde termina el número, alineada con el ancho de las cabeceras.
    const NUMBER_COLUMN: usize = 44;
    let pad = NUMBER_COLUMN
        .saturating_sub(indent + label.chars().count() + value.chars().count())
        .max(1);
    format!("{}{label}{}{value}", " ".repeat(indent), " ".repeat(pad))
}

fn print_header(outcome: &DiffOutcome, lang: Lang) {
    println!("{}", i18n::section(&msg::diff_title(lang)));
    for (etiqueta, info) in
        [(msg::label_before(lang), &outcome.before), (msg::label_after(lang), &outcome.after)]
    {
        println!(
            "  {etiqueta:<9} {:<19} {}   {}",
            info.started_at,
            msg::diff_crawl_counts(
                lang,
                format!("{:>7}", i18n::count(lang, info.urls_total)),
                format!("{:>6}", i18n::count(lang, info.issues_total)),
            ),
            info.path
        );
    }
    println!("  {:<9} {}", msg::label_site(lang), outcome.after.base_url);
    println!(
        "  {:<9} {}",
        msg::label_common(lang),
        msg::diff_common_note(lang, i18n::count(lang, outcome.urls_common))
    );
}

fn print_warnings(outcome: &DiffOutcome, lang: Lang) {
    if outcome.warnings.is_empty() {
        return;
    }
    println!();
    println!("{}", i18n::section(&msg::warnings_title(lang)));
    for warning in &outcome.warnings {
        let etiqueta = if warning.breaks_conclusion() {
            msg::tag_inconclusive(lang)
        } else {
            msg::tag_warning(lang)
        };
        println!("  {etiqueta:<15} {}", warning.message(lang));
    }
}

fn print_worse(outcome: &DiffOutcome, lang: Lang) {
    println!();
    println!("{}", i18n::section(&msg::worse_title(lang)));
    let mut algo = false;
    let n = |v: usize| i18n::count(lang, v as i64);

    // 1. Hallazgos nuevos, agrupados por severidad y regla.
    let nuevos: Vec<&Change> = outcome.of(ChangeType::IssueAppeared).collect();
    if !nuevos.is_empty() {
        algo = true;
        println!("{}", count_line(2, &msg::label_new_findings(lang), &n(nuevos.len())));
        for (severity, rule_id, ejemplos) in group_issues(&nuevos, lang) {
            println!(
                "    {:<9} {rule_id:<30} {:>6}",
                i18n::severity_word(lang, &severity),
                n(ejemplos.len())
            );
            for url in ejemplos.iter().take(MAX_EXAMPLES) {
                println!("      {url}");
            }
            if ejemplos.len() > MAX_EXAMPLES {
                println!("      {}", msg::more_examples(lang, n(ejemplos.len() - MAX_EXAMPLES)));
            }
        }
    }

    // 2. Indexabilidad perdida: el accidente clásico de un despliegue.
    let perdidas: Vec<&Change> = outcome.of(ChangeType::IndexabilityLost).collect();
    if !perdidas.is_empty() {
        algo = true;
        println!("{}", count_line(2, &msg::label_pages_lost_index(lang), &n(perdidas.len())));
        for change in perdidas.iter().take(MAX_EXAMPLES) {
            let sitio = msg::site_placeholder(lang);
            println!(
                "    {} → {}   {}",
                change.value_before.as_deref().unwrap_or("?"),
                change.value_after.as_deref().unwrap_or("?"),
                change.url.as_deref().unwrap_or(&sitio)
            );
        }
    }

    // 3. Códigos de estado que empeoraron: un 200 que pasa a 404 es la alerta más cara.
    let empeorados: Vec<&Change> = outcome
        .of(ChangeType::StatusChanged)
        .filter(|c| status_rank(c.value_after.as_deref()) > status_rank(c.value_before.as_deref()))
        .collect();
    if !empeorados.is_empty() {
        algo = true;
        println!("{}", count_line(2, &msg::label_status_worse(lang), &n(empeorados.len())));
        print_status_transitions(&empeorados, lang);
    }

    // 4. robots.txt: cualquier cambio, y en rojo si ahora bloquea el sitio entero.
    let robots: Vec<&Change> = outcome.of(ChangeType::RobotsTxtChanged).collect();
    if !robots.is_empty() {
        algo = true;
        println!("{}", count_line(2, "robots.txt", &n(robots.len())));
        for change in &robots {
            let host = change.url.as_deref().unwrap_or("");
            let field = change.field.as_deref().unwrap_or("");
            if field == "content" {
                println!("    {}", msg::robots_content_changed(lang, host));
            } else {
                println!(
                    "    {host}: {field} {} → {}",
                    change.value_before.as_deref().unwrap_or("—"),
                    change.value_after.as_deref().unwrap_or("—")
                );
            }
            if change.severity.as_deref() == Some("critical") {
                println!("      {}", msg::robots_blocks_all(lang));
            }
        }
    }

    if !algo {
        println!("  {}", msg::nothing_worse(lang));
    }
}

/// Las transiciones `antes → después` de código de estado, con «sin respuesta» en palabras.
fn print_status_transitions(cambios: &[&Change], lang: Lang) {
    for change in cambios.iter().take(MAX_EXAMPLES) {
        let sin_respuesta = msg::label_no_response(lang);
        println!(
            "    {} → {}   {}",
            change.value_before.as_deref().unwrap_or(&sin_respuesta),
            change.value_after.as_deref().unwrap_or(&sin_respuesta),
            change.url.as_deref().unwrap_or("")
        );
    }
}

fn print_better(outcome: &DiffOutcome, lang: Lang) {
    let resueltos = outcome.count(ChangeType::IssueResolved);
    let ganadas = outcome.count(ChangeType::IndexabilityGained);
    let mejorados: Vec<&Change> = outcome
        .of(ChangeType::StatusChanged)
        .filter(|c| status_rank(c.value_after.as_deref()) < status_rank(c.value_before.as_deref()))
        .collect();

    if resueltos == 0 && ganadas == 0 && mejorados.is_empty() {
        return;
    }

    let n = |v: usize| i18n::count(lang, v as i64);
    println!();
    println!("{}", i18n::section(&msg::better_title(lang)));
    if resueltos > 0 {
        println!("{}", count_line(2, &msg::label_findings_resolved(lang), &n(resueltos)));
        let cambios: Vec<&Change> = outcome.of(ChangeType::IssueResolved).collect();
        for (severity, rule_id, ejemplos) in group_issues(&cambios, lang) {
            println!(
                "    {:<9} {rule_id:<30} {:>6}",
                i18n::severity_word(lang, &severity),
                n(ejemplos.len())
            );
        }
    }
    if ganadas > 0 {
        println!("{}", count_line(2, &msg::label_pages_indexable_again(lang), &n(ganadas)));
    }
    if !mejorados.is_empty() {
        println!("{}", count_line(2, &msg::label_status_better(lang), &n(mejorados.len())));
        print_status_transitions(&mejorados, lang);
    }
}

fn print_other(outcome: &DiffOutcome, lang: Lang) {
    let nuevas: Vec<&Change> = outcome.of(ChangeType::UrlAdded).collect();
    let idas: Vec<&Change> = outcome.of(ChangeType::UrlRemoved).collect();
    let titulos = outcome.count(ChangeType::TitleChanged);
    let metas = outcome.count(ChangeType::MetaDescriptionChanged);
    let canonicals = outcome.count(ChangeType::CanonicalChanged);
    let sitemaps: Vec<&Change> = outcome.of(ChangeType::SitemapChanged).collect();

    if nuevas.is_empty()
        && idas.is_empty()
        && titulos == 0
        && metas == 0
        && canonicals == 0
        && sitemaps.is_empty()
        && outcome.issues_persisted == 0
    {
        return;
    }

    let n = |v: usize| i18n::count(lang, v as i64);
    println!();
    println!("{}", i18n::section(&msg::other_title(lang)));
    if !nuevas.is_empty() {
        println!("{}", count_line(2, &msg::label_new_urls(lang), &n(nuevas.len())));
        for change in nuevas.iter().take(MAX_EXAMPLES) {
            println!("    {}", change.url.as_deref().unwrap_or(""));
        }
    }
    if !idas.is_empty() {
        println!("{}", count_line(2, &msg::label_urls_gone(lang), &n(idas.len())));
        for change in idas.iter().take(MAX_EXAMPLES) {
            println!("    {}", change.url.as_deref().unwrap_or(""));
        }
    }
    if titulos > 0 {
        println!("{}", count_line(2, &msg::label_titles_changed(lang), &n(titulos)));
    }
    if metas > 0 {
        println!("{}", count_line(2, &msg::label_meta_changed(lang), &n(metas)));
    }
    if canonicals > 0 {
        println!("{}", count_line(2, &msg::label_canonicals_changed(lang), &n(canonicals)));
    }
    for change in sitemaps.iter().take(MAX_EXAMPLES * 2) {
        // La palabra «sitemap», la URL y el nombre del campo son datos; solo el formato es fijo.
        println!(
            "  sitemap {} {}: {} → {}",
            change.url.as_deref().unwrap_or(""),
            change.field.as_deref().unwrap_or(""),
            change.value_before.as_deref().unwrap_or("—"),
            change.value_after.as_deref().unwrap_or("—")
        );
    }
    if outcome.issues_persisted > 0 {
        println!(
            "{}",
            count_line(
                2,
                &msg::label_findings_persisted(lang),
                &i18n::count(lang, outcome.issues_persisted)
            )
        );
    }
}

fn print_suppressed(outcome: &DiffOutcome, lang: Lang) {
    if !outcome.suppressed.any() {
        return;
    }
    let s = &outcome.suppressed;
    let n = |v: i64| i18n::count(lang, v);
    println!();
    println!("{}", i18n::section(&msg::suppressed_title(lang)));
    for linea in msg::suppressed_intro(lang).lines() {
        println!("  {linea}");
    }
    if s.urls_added > 0 {
        println!("{}", count_line(4, &msg::label_candidate_new_urls(lang), &n(s.urls_added)));
    }
    if s.urls_removed > 0 {
        println!("{}", count_line(4, &msg::label_candidate_urls_gone(lang), &n(s.urls_removed)));
    }
    if s.issues_appeared > 0 {
        println!(
            "{}",
            count_line(4, &msg::label_candidate_new_findings(lang), &n(s.issues_appeared))
        );
    }
    if s.issues_resolved > 0 {
        println!("{}", count_line(4, &msg::label_candidate_resolved(lang), &n(s.issues_resolved)));
    }
    println!("  {}", msg::suppressed_advice(lang));
}

fn print_gate(outcome: &DiffOutcome, lang: Lang) {
    if outcome.fail_on_requested.is_empty() {
        return;
    }
    println!();
    println!("{}", i18n::section(&msg::gate_title(lang)));
    println!("  --fail-on {}", outcome.fail_on_requested.join(", "));
    if outcome.fail_on_inconclusive {
        for linea in msg::gate_inconclusive(lang).lines() {
            println!("  {linea}");
        }
    }
    for hit in &outcome.fail_on {
        // FAIL y PASS se quedan tal cual en los dos idiomas: son la convención de cualquier
        // salida de CI y lo primero que busca un grep.
        println!(
            "  FAIL   {:<30} {}",
            hit.rule_id,
            msg::gate_fail_detail(lang, hit.count, &hit.severity, &hit.token)
        );
    }
    if outcome.fail_on.is_empty() && !outcome.fail_on_inconclusive {
        println!("  {}", msg::gate_pass(lang));
    }
}

/// Agrupa hallazgos por severidad y regla, de más grave a menos y, dentro, por recuento.
fn group_issues(changes: &[&Change], lang: Lang) -> Vec<(String, String, Vec<String>)> {
    let mut grupos: BTreeMap<(usize, String, String), Vec<String>> = BTreeMap::new();
    for change in changes {
        let severity = change.severity.clone().unwrap_or_default();
        let rule_id = change.field.clone().unwrap_or_default();
        grupos
            .entry((severity_rank(&severity), severity, rule_id))
            .or_default()
            .push(change.url.clone().unwrap_or_else(|| msg::site_wide_finding(lang)));
    }
    let mut out: Vec<(String, String, Vec<String>)> =
        grupos.into_iter().map(|((_, sev, rule), urls)| (sev, rule, urls)).collect();
    out.sort_by(|a, b| {
        severity_rank(&a.0)
            .cmp(&severity_rank(&b.0))
            .then(b.2.len().cmp(&a.2.len()))
            .then(a.1.cmp(&b.1))
    });
    out
}

/// De mejor a peor, para poder decir si un cambio de estado empeoró. «Sin respuesta» es lo peor:
/// una URL que antes contestaba y ahora no, no está mejor.
fn status_rank(code: Option<&str>) -> u8 {
    match code.and_then(|c| c.parse::<i64>().ok()) {
        Some(c) if (200..300).contains(&c) => 0,
        Some(c) if (300..400).contains(&c) => 1,
        Some(c) if (400..500).contains(&c) => 2,
        Some(c) if (500..600).contains(&c) => 3,
        Some(_) => 3,
        None => 4,
    }
}

// ─────────────────────────────────────────────────────────────────────── Tests

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// A crawl file with the **real schema**: every published migration, from the shared
    /// helper in `test_schema.rs`, whose guard test keeps it in sync with `migrations/`.
    /// Copying a similar-looking schema by hand would pass the tests and fail the command.
    fn crawl_file(path: &Path) -> Connection {
        crate::test_schema::crawl_file(path)
    }

    struct Meta<'a> {
        base_url: &'a str,
        started_at: &'a str,
        truncated: Option<&'a str>,
        rules_version: &'a str,
        config: &'a str,
    }

    impl Default for Meta<'_> {
        fn default() -> Self {
            Self {
                base_url: "https://ejemplo.es/",
                started_at: "2026-07-01T10:00:00Z",
                truncated: None,
                rules_version: "0.0.1",
                config: "{\"limits\":{\"max_urls\":null}}",
            }
        }
    }

    fn meta(conn: &Connection, m: Meta<'_>) {
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime, truncated, truncated_reason)
             VALUES ('c','p','P', ?1, 'http', ?2, 'done', ?3, '0.0.1', ?4, 'free', ?5, ?6)",
            params![
                m.base_url,
                m.started_at,
                m.config,
                m.rules_version,
                m.truncated.is_some() as i64,
                m.truncated,
            ],
        )
        .expect("insert crawl_meta");
    }

    fn url(conn: &Connection, id: i64, url: &str, status: Option<i64>) -> i64 {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code)
             VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', '/', 1, 0, 'done', ?3)",
            params![id, url, status],
        )
        .expect("insert url");
        id
    }

    struct Page<'a> {
        title: Option<&'a str>,
        meta_description: Option<&'a str>,
        canonical: Option<&'a str>,
        indexable: bool,
        reason: Option<&'a str>,
    }

    impl Default for Page<'_> {
        fn default() -> Self {
            Self {
                title: Some("Título"),
                meta_description: Some("Descripción"),
                canonical: Some("https://ejemplo.es/"),
                indexable: true,
                reason: None,
            }
        }
    }

    fn page(conn: &Connection, url_id: i64, p: Page<'_>) {
        conn.execute(
            "INSERT INTO pages (url_id, title, meta_description, canonical, is_indexable,
                                indexability_reason, internal_links_in)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            params![url_id, p.title, p.meta_description, p.canonical, p.indexable as i64, p.reason],
        )
        .expect("insert page");
    }

    fn issue(conn: &Connection, url_id: Option<i64>, rule_id: &str, severity: &str) {
        conn.execute(
            "INSERT INTO issues (url_id, rule_id, severity, category, group_key)
             VALUES (?1, ?2, ?3, 'indexability', NULL)",
            params![url_id, rule_id, severity],
        )
        .expect("insert issue");
    }

    fn issue_grouped(conn: &Connection, url_id: i64, rule_id: &str, severity: &str, group: &str) {
        conn.execute(
            "INSERT INTO issues (url_id, rule_id, severity, category, group_key)
             VALUES (?1, ?2, ?3, 'social', ?4)",
            params![url_id, rule_id, severity, group],
        )
        .expect("insert grouped issue");
    }

    /// Directorio temporal propio. La CLI no tiene `tempfile` entre sus dependencias y el stack
    /// está cerrado (`CONVENTIONS.md §3`), así que se hace a mano.
    fn tmpdir(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("crawlforge-diff-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// El escenario base: dos rastreos del mismo sitio con diferencias conocidas.
    ///
    /// Antes: `/` (200), `/a` (200, indexable, título viejo), `/vieja` (200).
    /// Después: `/` (200), `/a` (404, ya no indexable, título nuevo), `/nueva` (200).
    fn escenario(dir: &Path, despues_truncado: Option<&str>) -> (PathBuf, PathBuf) {
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");

        {
            let conn = crawl_file(&antes);
            meta(&conn, Meta::default());
            let raiz = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, raiz, Page::default());
            let a = url(&conn, 2, "https://ejemplo.es/a", Some(200));
            page(&conn, a, Page { title: Some("Título viejo"), ..Page::default() });
            let vieja = url(&conn, 3, "https://ejemplo.es/vieja", Some(200));
            page(&conn, vieja, Page::default());
            issue(&conn, Some(a), "INDEX-NOINDEX-IN-SITEMAP", "high");
            issue(&conn, Some(raiz), "META-TITLE-TOO-LONG", "medium");
        }
        {
            let conn = crawl_file(&despues);
            meta(
                &conn,
                Meta {
                    started_at: "2026-07-08T10:00:00Z",
                    truncated: despues_truncado,
                    ..Meta::default()
                },
            );
            let raiz = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, raiz, Page::default());
            let a = url(&conn, 2, "https://ejemplo.es/a", Some(404));
            page(
                &conn,
                a,
                Page {
                    title: Some("Título nuevo"),
                    indexable: false,
                    reason: Some("4xx"),
                    ..Page::default()
                },
            );
            let nueva = url(&conn, 4, "https://ejemplo.es/nueva", Some(200));
            page(&conn, nueva, Page::default());
            issue(&conn, Some(a), "HTTP-404-INTERNAL", "critical");
            issue(&conn, Some(raiz), "META-TITLE-TOO-LONG", "medium");
        }

        (antes, despues)
    }

    fn urls_de(outcome: &DiffOutcome, tipo: ChangeType) -> Vec<String> {
        outcome.of(tipo).filter_map(|c| c.url.clone()).collect()
    }

    #[test]
    fn distingue_hallazgo_nuevo_resuelto_y_persistente() {
        let dir = tmpdir("hallazgos");
        let (antes, despues) = escenario(&dir, None);

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        let nuevos: Vec<&str> =
            outcome.of(ChangeType::IssueAppeared).filter_map(|c| c.field.as_deref()).collect();
        assert_eq!(nuevos, ["HTTP-404-INTERNAL"], "the 404 is the only new finding");

        let resueltos: Vec<&str> =
            outcome.of(ChangeType::IssueResolved).filter_map(|c| c.field.as_deref()).collect();
        assert_eq!(resueltos, ["INDEX-NOINDEX-IN-SITEMAP"]);

        assert_eq!(outcome.issues_persisted, 1, "META-TITLE-TOO-LONG is still there");
        assert!(outcome.conclusive());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_group_key_recien_poblado_no_fabrica_un_diff_de_ruido() {
        // El rastreo «antes» lo hizo una versión que aún no escribía `group_key` en
        // INDEX-NOFOLLOW-INTERNAL; el «después» ya lo escribe. El sitio no cambió: el primer
        // diff tras actualizar no puede reportar 18.092 hallazgos nuevos y 18.092 resueltos por
        // un cambio del programa. Un `group_key` vacío en un lado empareja con cualquiera.
        let dir = tmpdir("groupkey-nuevo");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        {
            let conn = crawl_file(&antes);
            meta(&conn, Meta::default());
            let a = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, a, Page::default());
            issue(&conn, Some(a), "INDEX-NOFOLLOW-INTERNAL", "medium"); // sin group_key
        }
        {
            let conn = crawl_file(&despues);
            meta(&conn, Meta { started_at: "2026-07-08T10:00:00Z", ..Meta::default() });
            let a = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, a, Page::default());
            issue_grouped(&conn, a, "INDEX-NOFOLLOW-INTERNAL", "medium", "nofollow:aaaa");
        }

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");
        assert!(
            outcome.of(ChangeType::IssueAppeared).next().is_none(),
            "the finding is not new: it only gained a group key"
        );
        assert!(outcome.of(ChangeType::IssueResolved).next().is_none());
        assert_eq!(outcome.issues_persisted, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn con_clave_en_los_dos_lados_un_grupo_que_cambia_sigue_siendo_un_cambio() {
        // La guarda del comodín: solo se relaja el emparejamiento cuando un lado no trae clave.
        // Con clave en los dos lados, `title:aaaa` → `title:bbbb` es un cambio real —el título
        // duplicado ahora es otro— y se reporta.
        let dir = tmpdir("groupkey-cambia");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        {
            let conn = crawl_file(&antes);
            meta(&conn, Meta::default());
            let a = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, a, Page::default());
            issue_grouped(&conn, a, "META-TITLE-DUPLICATE", "high", "title:aaaa");
        }
        {
            let conn = crawl_file(&despues);
            meta(&conn, Meta { started_at: "2026-07-08T10:00:00Z", ..Meta::default() });
            let a = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, a, Page::default());
            issue_grouped(&conn, a, "META-TITLE-DUPLICATE", "high", "title:bbbb");
        }

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");
        assert_eq!(outcome.count(ChangeType::IssueAppeared), 1);
        assert_eq!(outcome.count(ChangeType::IssueResolved), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_url_que_pasa_de_200_a_404_se_reporta_como_empeoramiento() {
        let dir = tmpdir("estado");
        let (antes, despues) = escenario(&dir, None);

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        let cambios: Vec<&Change> = outcome.of(ChangeType::StatusChanged).collect();
        assert_eq!(cambios.len(), 1);
        assert_eq!(cambios[0].url.as_deref(), Some("https://ejemplo.es/a"));
        assert_eq!(cambios[0].value_before.as_deref(), Some("200"));
        assert_eq!(cambios[0].value_after.as_deref(), Some("404"));
        assert!(
            status_rank(cambios[0].value_after.as_deref())
                > status_rank(cambios[0].value_before.as_deref()),
            "a 404 is worse than a 200"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detecta_urls_nuevas_y_desaparecidas() {
        let dir = tmpdir("urls");
        let (antes, despues) = escenario(&dir, None);

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        assert_eq!(urls_de(&outcome, ChangeType::UrlAdded), ["https://ejemplo.es/nueva"]);
        assert_eq!(urls_de(&outcome, ChangeType::UrlRemoved), ["https://ejemplo.es/vieja"]);
        assert_eq!(outcome.urls_common, 2, "the root and /a are in both");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_rastreo_truncado_no_inventa_urls_desaparecidas() {
        let dir = tmpdir("truncado");
        let (antes, despues) = escenario(&dir, Some("max_urls"));

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        assert!(
            urls_de(&outcome, ChangeType::UrlRemoved).is_empty(),
            "/vieja is missing because the crawl was cut short, not because it disappeared"
        );
        assert_eq!(outcome.suppressed.urls_removed, 1, "but it is counted as a candidate");
        assert!(
            outcome.of(ChangeType::IssueResolved).next().is_none(),
            "a finding is not resolved by no longer looking at it"
        );
        assert_eq!(outcome.suppressed.issues_resolved, 1);
        assert!(!outcome.conclusive(), "the result cannot be asserted");

        // Lo que sí sigue siendo cierto: la intersección.
        assert_eq!(outcome.count(ChangeType::StatusChanged), 1);
        assert_eq!(urls_de(&outcome, ChangeType::UrlAdded), ["https://ejemplo.es/nueva"]);
        assert_eq!(outcome.count(ChangeType::IssueAppeared), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_antes_truncado_suprime_lo_nuevo_y_deja_la_puerta_sin_pronunciarse() {
        let dir = tmpdir("truncado-antes");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        {
            let conn = crawl_file(&antes);
            meta(&conn, Meta { truncated: Some("max_urls"), ..Meta::default() });
            let raiz = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, raiz, Page::default());
        }
        {
            let conn = crawl_file(&despues);
            meta(&conn, Meta::default());
            let raiz = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, raiz, Page::default());
            let a = url(&conn, 2, "https://ejemplo.es/a", Some(404));
            page(&conn, a, Page { indexable: false, reason: Some("4xx"), ..Page::default() });
            issue(&conn, Some(a), "HTTP-404-INTERNAL", "critical");
        }

        let fail_on = vec!["HTTP-404-INTERNAL".to_string()];
        let outcome = compare(&antes, &despues, None, &fail_on).expect("compare");

        assert_eq!(outcome.count(ChangeType::IssueAppeared), 0);
        assert_eq!(outcome.suppressed.issues_appeared, 1);
        assert!(!outcome.should_fail(), "a build is not failed on data that cannot be asserted");
        assert!(outcome.fail_on_inconclusive, "but it clearly says this is not a pass");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detecta_cambios_de_titulo_y_de_indexabilidad() {
        let dir = tmpdir("paginas");
        let (antes, despues) = escenario(&dir, None);

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        let titulos: Vec<&Change> = outcome.of(ChangeType::TitleChanged).collect();
        assert_eq!(titulos.len(), 1);
        assert_eq!(titulos[0].value_before.as_deref(), Some("Título viejo"));
        assert_eq!(titulos[0].value_after.as_deref(), Some("Título nuevo"));

        let perdidas: Vec<&Change> = outcome.of(ChangeType::IndexabilityLost).collect();
        assert_eq!(perdidas.len(), 1);
        assert_eq!(perdidas[0].url.as_deref(), Some("https://ejemplo.es/a"));
        assert_eq!(perdidas[0].value_before.as_deref(), Some("indexable"));
        assert_eq!(perdidas[0].value_after.as_deref(), Some("4xx"));
        assert_eq!(outcome.count(ChangeType::IndexabilityGained), 0);

        // Nadie tocó la meta description ni el canonical: no deben aparecer.
        assert_eq!(outcome.count(ChangeType::MetaDescriptionChanged), 0);
        assert_eq!(outcome.count(ChangeType::CanonicalChanged), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_mismo_rastreo_consigo_mismo_no_tiene_cambios() {
        let dir = tmpdir("identidad");
        let (antes, _) = escenario(&dir, None);

        let outcome = compare(&antes, &antes, None, &[]).expect("compare");

        assert!(outcome.changes.is_empty(), "{:?}", outcome.changes);
        assert_eq!(outcome.issues_persisted, 2);
        assert!(outcome.conclusive());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_group_key_distingue_dos_hallazgos_de_la_misma_regla_en_la_misma_url() {
        let dir = tmpdir("group-key");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        {
            let conn = crawl_file(&antes);
            meta(&conn, Meta::default());
            let raiz = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, raiz, Page::default());
            issue_grouped(&conn, raiz, "SOCIAL-OG-MISSING", "low", "og-missing:og:image");
        }
        {
            let conn = crawl_file(&despues);
            meta(&conn, Meta::default());
            let raiz = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, raiz, Page::default());
            issue_grouped(&conn, raiz, "SOCIAL-OG-MISSING", "low", "og-missing:og:image");
            issue_grouped(&conn, raiz, "SOCIAL-OG-MISSING", "low", "og-missing:og:title");
        }

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        let nuevos: Vec<&Change> = outcome.of(ChangeType::IssueAppeared).collect();
        assert_eq!(nuevos.len(), 1, "only og:title is missing, not the whole rule again");
        assert_eq!(nuevos[0].value_after.as_deref(), Some("og-missing:og:title"));
        assert_eq!(outcome.issues_persisted, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detecta_un_robots_txt_que_pasa_a_bloquearlo_todo() {
        let dir = tmpdir("robots");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        for (path, contenido, bloquea) in [
            (&antes, "User-agent: *\nAllow: /\n", 0),
            (&despues, "User-agent: *\nDisallow: /\n", 1),
        ] {
            let conn = crawl_file(path);
            meta(&conn, Meta::default());
            let raiz = url(&conn, 1, "https://ejemplo.es/", Some(200));
            page(&conn, raiz, Page::default());
            conn.execute(
                "INSERT INTO robots_txt (host, status_code, content, blocks_all, sitemap_count)
                 VALUES ('ejemplo.es', 200, ?1, ?2, 0)",
                params![contenido, bloquea],
            )
            .expect("insert robots_txt");
        }

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        let cambios: Vec<&Change> = outcome.of(ChangeType::RobotsTxtChanged).collect();
        assert_eq!(cambios.len(), 2, "blocks_all and content: {cambios:?}");
        let bloqueo = cambios
            .iter()
            .find(|c| c.field.as_deref() == Some("blocks_all"))
            .expect("the blocks_all change");
        assert_eq!(bloqueo.severity.as_deref(), Some("critical"));
        assert_eq!(bloqueo.url.as_deref(), Some("ejemplo.es"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detecta_un_sitemap_que_pierde_urls() {
        let dir = tmpdir("sitemaps");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        for (path, url_count, valid) in [(&antes, 4000, 1), (&despues, 12, 0)] {
            let conn = crawl_file(path);
            meta(&conn, Meta::default());
            conn.execute(
                "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                       discovered_from)
                 VALUES ('https://ejemplo.es/sitemap.xml', 200, 0, ?1, ?2, 100, 'robots')",
                params![valid, url_count],
            )
            .expect("insert sitemap");
        }

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        let campos: Vec<&str> =
            outcome.of(ChangeType::SitemapChanged).filter_map(|c| c.field.as_deref()).collect();
        assert!(campos.contains(&"url_count"), "{campos:?}");
        assert!(campos.contains(&"is_valid"), "{campos:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fail_on_dispara_por_regla_y_por_severidad_o_peor() {
        let dir = tmpdir("fail-on");
        let (antes, despues) = escenario(&dir, None);

        let por_regla = vec!["HTTP-404-INTERNAL".to_string()];
        let outcome = compare(&antes, &despues, None, &por_regla).expect("compare");
        assert!(outcome.should_fail());
        assert_eq!(outcome.fail_on[0].rule_id, "HTTP-404-INTERNAL");
        assert_eq!(outcome.fail_on[0].count, 1);

        // «high» significa «high o peor»: un critical nuevo tiene que hacerla saltar.
        let por_severidad = vec!["high".to_string()];
        let outcome = compare(&antes, &despues, None, &por_severidad).expect("compare");
        assert!(outcome.should_fail(), "a critical meets the high threshold");

        // Una regla que no aparece como hallazgo nuevo no falla la build.
        let otra = vec!["INDEX-NOINDEX".to_string()];
        let outcome = compare(&antes, &despues, None, &otra).expect("compare");
        assert!(!outcome.should_fail());
        assert_eq!(
            outcome.fail_on_requested,
            ["INDEX-NOINDEX"],
            "a gate that passes must be able to say it passed, not stay silent"
        );

        // Un hallazgo que ya estaba antes tampoco: la puerta juzga el despliegue, no la deuda.
        let vieja = vec!["META-TITLE-TOO-LONG".to_string()];
        let outcome = compare(&antes, &despues, None, &vieja).expect("compare");
        assert!(!outcome.should_fail());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_token_de_fail_on_desconocido_es_un_error_y_no_un_silencio() {
        let dir = tmpdir("fail-on-typo");
        let (antes, despues) = escenario(&dir, None);

        let err = compare(&antes, &despues, None, &["HTTP-404-INTERNA".to_string()])
            .expect_err("a typo must hurt");
        // Las afirmaciones son sobre lo invariante entre idiomas (el token y el comando que
        // ayuda): `compare` responde en el idioma del proceso y este test no debe fijarlo.
        let msg = err.to_string();
        assert!(msg.contains("HTTP-404-INTERNA"), "names the token: {msg}");
        assert!(msg.contains("crawlforge rules"), "and where to look up the IDs: {msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dos_sitios_distintos_no_se_comparan() {
        let dir = tmpdir("sitios");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        {
            let conn = crawl_file(&antes);
            meta(&conn, Meta::default());
        }
        {
            let conn = crawl_file(&despues);
            meta(&conn, Meta { base_url: "https://otro-sitio.es/", ..Meta::default() });
        }

        let err = compare(&antes, &despues, None, &[]).expect_err("comparing makes no sense");
        // Invariante entre idiomas: los dos orígenes tienen que aparecer en el error.
        let msg = err.to_string();
        assert!(msg.contains("https://ejemplo.es"), "{msg}");
        assert!(msg.contains("https://otro-sitio.es"), "{msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn avisa_si_cambio_el_catalogo_de_reglas_o_la_configuracion() {
        let dir = tmpdir("avisos");
        let antes = dir.join("antes.sqlite");
        let despues = dir.join("despues.sqlite");
        {
            let conn = crawl_file(&antes);
            meta(&conn, Meta::default());
        }
        {
            let conn = crawl_file(&despues);
            meta(
                &conn,
                Meta {
                    rules_version: "0.0.2",
                    config: "{\"limits\":{\"max_urls\":500}}",
                    ..Meta::default()
                },
            );
        }

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        assert!(outcome
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::RulesVersionChanged { .. })));
        let config = outcome
            .warnings
            .iter()
            .find_map(|w| match w {
                Warning::ConfigChanged { fields } => Some(fields.clone()),
                _ => None,
            })
            .expect("the config warning");
        assert_eq!(config, ["limits.max_urls"]);
        // Ninguno de los dos invalida la conclusión: son avisos, no un diff imposible.
        assert!(outcome.conclusive());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_fichero_de_diff_se_escribe_y_se_puede_leer() {
        let dir = tmpdir("fichero");
        let (antes, despues) = escenario(&dir, None);
        let salida = dir.join("diff.sqlite");

        let outcome = compare(&antes, &despues, Some(&salida), &[]).expect("compare");
        assert_eq!(outcome.out_path.as_deref(), Some(salida.as_path()));

        let conn = Connection::open(&salida).expect("open the diff");
        let version: i64 =
            conn.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0)).expect("v");
        assert_eq!(version, DIFF_SCHEMA_VERSION);

        let filas: i64 =
            conn.query_row("SELECT COUNT(*) FROM changes", [], |r| r.get(0)).expect("changes");
        assert_eq!(filas as usize, outcome.changes.len());

        let nuevos: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM changes WHERE change_type = 'issue_appeared'",
                [],
                |r| r.get(0),
            )
            .expect("issue_appeared");
        assert_eq!(nuevos, 1);

        let (conclusive, base_url): (i64, String) = conn
            .query_row("SELECT conclusive, after_base_url FROM diff_meta", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("diff_meta");
        assert_eq!(conclusive, 1);
        assert_eq!(base_url, "https://ejemplo.es/");

        // La vista de resumen es lo que consultará una interfaz.
        let tipos: i64 = conn
            .query_row("SELECT COUNT(*) FROM v_change_summary", [], |r| r.get(0))
            .expect("v_change_summary");
        assert!(tipos > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_fichero_de_diff_guarda_el_aviso_de_truncado() {
        let dir = tmpdir("fichero-truncado");
        let (antes, despues) = escenario(&dir, Some("max_urls"));
        let salida = dir.join("diff.sqlite");

        compare(&antes, &despues, Some(&salida), &[]).expect("compare");

        let conn = Connection::open(&salida).expect("open the diff");
        let (conclusive, avisos, ocultas): (i64, String, i64) = conn
            .query_row(
                "SELECT conclusive, warnings_json, suppressed_urls_removed FROM diff_meta",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("diff_meta");
        assert_eq!(conclusive, 0, "the file must carry the warning, not only the terminal");
        assert!(avisos.contains("truncated"), "{avisos}");
        assert_eq!(ocultas, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_rastreo_en_modo_lista_avisa_sin_decir_truncado() {
        // `truncated_reason = 'list_mode'` arrastra el comportamiento del truncado —las
        // ausencias no se pueden afirmar: el otro rastreo pudo simplemente no llevar esa URL
        // en su lista— pero el mensaje no puede decir «se cortó», porque no se cortó: auditó
        // su lista entera.
        let dir = tmpdir("modo-lista");
        let (antes, despues) = escenario(&dir, Some("list_mode"));

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");

        assert!(
            urls_de(&outcome, ChangeType::UrlRemoved).is_empty(),
            "on a list crawl you cannot assert that a URL disappeared"
        );
        assert!(!outcome.conclusive());

        let aviso = outcome
            .warnings
            .iter()
            .find(|w| matches!(w, Warning::Truncated { .. }))
            .expect("the warning must exist");
        assert!(aviso.breaks_conclusion());
        let en = aviso.message(Lang::En);
        assert!(en.contains("list crawl"), "says what actually happens: {en}");
        assert!(!en.contains("truncated"), "nothing was cut short: {en}");
        let es = aviso.message(Lang::Es);
        assert!(es.contains("modo lista"), "and in Spanish: {es}");
        assert!(!es.contains("truncado"), "{es}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_toca_los_ficheros_de_rastreo() {
        let dir = tmpdir("solo-lectura");
        let (antes, despues) = escenario(&dir, None);
        let antes_mtime = std::fs::metadata(&antes).and_then(|m| m.modified()).expect("mtime");

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");
        assert!(!outcome.changes.is_empty());

        let despues_mtime = std::fs::metadata(&antes).and_then(|m| m.modified()).expect("mtime");
        assert_eq!(antes_mtime, despues_mtime);
        assert!(!dir.join("antes.sqlite-wal").exists(), "a diff leaves no WAL behind");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fichero_que_no_es_un_rastreo_da_un_error_claro() {
        let dir = tmpdir("no-rastreo");
        let falso = dir.join("falso.sqlite");
        {
            let conn = Connection::open(&falso).expect("create");
            conn.execute_batch("CREATE TABLE cosas (id INTEGER);").expect("table");
        }
        let (antes, _) = escenario(&dir, None);

        let err = compare(&antes, &falso, None, &[]).expect_err("not a crawl");
        let msg = format!("{err:#}");
        assert!(msg.contains("is not a CrawlForge crawl file"), "{msg}");
        assert!(!msg.contains("no such table"), "no SQLite jargon: {msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fichero_de_diff_no_se_puede_usar_como_rastreo_y_el_error_lo_dice() {
        // El callejón sin salida de la revisión de UX, versión `diff`: encadenar el fichero de
        // un `diff --out` como entrada de otra comparación.
        let dir = tmpdir("diff-de-diff");
        let (antes, despues) = escenario(&dir, None);
        let salida = dir.join("miweb-diff.sqlite");
        compare(&antes, &despues, Some(&salida), &[]).expect("first diff");

        let err = compare(&antes, &salida, None, &[]).expect_err("a diff is not a crawl");
        let msg = format!("{err:#}");
        assert!(msg.contains("is a diff file"), "says what it is: {msg}");
        assert!(msg.contains("crawl") && msg.contains("audit"), "and what was needed: {msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_despues_anterior_al_antes_avisa_del_orden_invertido() {
        let dir = tmpdir("orden");
        let (antes, despues) = escenario(&dir, None);

        // Los mismos ficheros, pasados del revés: el «después» empezó una semana antes.
        let outcome = compare(&despues, &antes, None, &[]).expect("compare");

        let aviso = outcome
            .warnings
            .iter()
            .find(|w| matches!(w, Warning::OrderInverted { .. }))
            .expect("must warn about the order");
        let msg = aviso.message(Lang::En);
        assert!(msg.contains("swap the two arguments"), "says what to do: {msg}");
        let es = aviso.message(Lang::Es);
        assert!(es.contains("intercambia los dos argumentos"), "and in Spanish: {es}");
        assert!(
            !aviso.breaks_conclusion(),
            "it is a warning, not an error: comparing backwards can be deliberate"
        );
        assert!(outcome.conclusive(), "the diff is still valid, just reversed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_orden_correcto_no_lleva_aviso_de_orden() {
        let dir = tmpdir("orden-bien");
        let (antes, despues) = escenario(&dir, None);

        let outcome = compare(&antes, &despues, None, &[]).expect("compare");
        assert!(
            !outcome.warnings.iter().any(|w| matches!(w, Warning::OrderInverted { .. })),
            "{:?}",
            outcome.warnings
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_resumen_en_terminal_se_imprime_sin_romperse() {
        let dir = tmpdir("impresion");
        let (antes, despues) = escenario(&dir, Some("max_urls"));

        let outcome =
            compare(&antes, &despues, None, &["critical".to_string()]).expect("compare");
        print_report(&outcome);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Comparación contra dos ficheros de rastreo **de verdad**, incluidos los del esquema
    /// antiguo (migración 001, sin `truncated` ni `robots_txt`) y los de medio millón de URLs.
    ///
    /// No corre en la suite —necesita ficheros que no se versionan—, igual que las otras pruebas
    /// manuales de la CLI. Se lanza así:
    ///
    /// ```text
    /// CRAWLFORGE_DIFF_A=antes.sqlite CRAWLFORGE_DIFF_B=despues.sqlite \
    ///   cargo test -p crawlforge-cli --lib -- --ignored --nocapture rastreos_reales
    /// ```
    #[test]
    #[ignore = "necesita dos ficheros de rastreo reales; se lanza a mano"]
    fn diff_de_dos_rastreos_reales() {
        let (Ok(antes), Ok(despues)) =
            (std::env::var("CRAWLFORGE_DIFF_A"), std::env::var("CRAWLFORGE_DIFF_B"))
        else {
            panic!("set CRAWLFORGE_DIFF_A and CRAWLFORGE_DIFF_B to the crawl file paths");
        };

        let salida = std::env::var("CRAWLFORGE_DIFF_OUT").ok().map(PathBuf::from);
        let reloj = std::time::Instant::now();
        let outcome = compare(Path::new(&antes), Path::new(&despues), salida.as_deref(), &[])
            .expect("compare");
        let segundos = reloj.elapsed().as_secs_f64();

        print_report(&outcome);
        println!();
        println!("Comparación resuelta en {segundos:.2} s ({} cambios)", outcome.changes.len());
    }

    // ── Unidades sueltas ────────────────────────────────────────────────────

    #[test]
    fn la_uri_de_solo_lectura_escapa_lo_que_sqlite_interpretaria() {
        assert_eq!(encode_uri_path("/tmp/a b?c#d%e"), "/tmp/a%20b%3Fc%23d%25e");
        assert_eq!(encode_uri_path(r"C:\rastreos\a.sqlite"), "C:/rastreos/a.sqlite");
    }

    #[test]
    fn el_origen_ignora_la_ruta_y_las_mayusculas() {
        assert_eq!(origin("https://Ejemplo.ES/blog/"), "https://ejemplo.es");
        assert_eq!(origin("https://ejemplo.es"), "https://ejemplo.es");
        assert_ne!(origin("http://ejemplo.es/"), origin("https://ejemplo.es/"));
    }

    #[test]
    fn el_rango_de_estado_ordena_de_mejor_a_peor() {
        assert!(status_rank(Some("200")) < status_rank(Some("301")));
        assert!(status_rank(Some("301")) < status_rank(Some("404")));
        assert!(status_rank(Some("404")) < status_rank(Some("500")));
        assert!(status_rank(Some("500")) < status_rank(None), "no response is the worst");
    }

    #[test]
    fn una_severidad_de_fail_on_incluye_las_peores() {
        let tokens = parse_fail_on(Lang::En, &["medium".to_string()]).expect("parse");
        let changes = vec![
            Change::new(ChangeType::IssueAppeared)
                .field("HTTP-404-INTERNAL")
                .severity("critical"),
            Change::new(ChangeType::IssueAppeared).field("META-TITLE-TOO-LONG").severity("low"),
        ];
        let hits = evaluate_fail_on(&changes, &tokens);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule_id, "HTTP-404-INTERNAL");
    }

    #[test]
    fn una_configuracion_identica_no_genera_aviso() {
        assert!(config_differences("{\"a\":1}", "{\"a\":1}").is_empty());
        assert_eq!(config_differences("{\"a\":1}", "{\"a\":2}"), ["a"]);
        assert_eq!(
            config_differences("{\"l\":{\"max\":1}}", "{\"l\":{\"max\":2}}"),
            ["l.max"]
        );
        // Un campo que aparece en un lado y no en el otro también es una diferencia.
        assert_eq!(config_differences("{}", "{\"nuevo\":true}"), ["nuevo"]);
        // Y si no es JSON, se compara en crudo antes que callarse.
        assert_eq!(config_differences("no-json", "otra-cosa"), ["config_json"]);
    }
}
