//! Almacen SQLite. Un rastreo = un fichero. El core escribe, la UI lee en solo lectura.
//! Ver `docs/02-MODELO-DATOS.md`.

use crate::{error::Result, CoreError, SCHEMA_VERSION};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Migraciones numeradas y hacia adelante. Nunca se edita una ya publicada:
/// un fichero de rastreo antiguo debe seguir abriendose.
///
/// El tercer campo es **si un rastreo interrumpido puede reanudarse a través de ella**. Ver
/// [`first_blocking_resume`]: la respuesta no es la misma para todas.
const MIGRATIONS: &[(i64, &str, ResumeSafety)] = &[
    (1, include_str!("../migrations/001_initial.sql"), ResumeSafety::Safe),
    (2, include_str!("../migrations/002_truncated.sql"), ResumeSafety::Safe),
    (3, include_str!("../migrations/003_orphans_exclude_seed.sql"), ResumeSafety::Safe),
    (4, include_str!("../migrations/004_robots_y_sitemaps.sql"), ResumeSafety::Safe),
    (5, include_str!("../migrations/005_orphans_solo_paginas.sql"), ResumeSafety::Safe),
    (6, include_str!("../migrations/006_indice_html_hash.sql"), ResumeSafety::Safe),
    (7, include_str!("../migrations/007_indice_images_src.sql"), ResumeSafety::Safe),
    // `Safe` con matiz: el índice no cambia el significado de ninguna fila ya escrita, y el
    // motor repone las filas de `resources` de la mitad antigua al reanudar (ver
    // `engine::resend_existing_rows`), así que el fichero reanudado queda completo.
    (8, include_str!("../migrations/008_indice_unico_resources.sql"), ResumeSafety::Safe),
    // Redefinir una vista no toca ni una fila: la mitad rastreada antes y la de después dicen
    // lo mismo, y la vista nueva las lee igual.
    (9, include_str!("../migrations/009_broken_links_sigue_redirecciones.sql"), ResumeSafety::Safe),
];

/// ¿Puede un rastreo a medias cruzar esta migración y seguir reanudándose?
///
/// **`Safe` no significa «no rompe nada»** —eso lo son todas, o no se publicarían— sino que las
/// filas ya escritas y las que quedan por escribir siguen queriendo decir lo mismo. Añadir un
/// índice, crear una tabla vacía, redefinir una vista o añadir una columna con defecto lo son:
/// la mitad rastreada antes y la mitad de después son comparables.
///
/// `Blocking` es para el día que una migración cambie **qué se escribe**: renombrar el
/// significado de una columna, cambiar cómo se calcula un hash, alterar la normalización de
/// URLs. Ahí sí, la primera mitad del rastreo diría una cosa y la segunda otra, y es mejor
/// rechazar y pedir un rastreo nuevo que entregar un fichero mestizo.
///
/// Hoy las nueve son `Safe`. La distinción existe porque **hasta el 2026-08-02 no había ninguna**:
/// `resume` exigía versión exacta, así que la migración 006 —que solo crea un índice— convirtió
/// un rastreo de dieciocho horas en irrecuperable, y el error decía «vuelve a rastrearlo».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeSafety {
    Safe,
    Blocking,
}

/// La primera migración pendiente que impide reanudar un fichero en la versión `from`.
///
/// `None` significa que se puede reanudar: o está al día, o solo le faltan migraciones seguras.
/// Un fichero **más nuevo** que este binario nunca se puede reanudar —no hay marcha atrás— y eso
/// lo comprueba quien llama, con [`SCHEMA_VERSION`].
pub fn first_blocking_resume(from: i64) -> Option<i64> {
    first_blocking_in(MIGRATIONS, from)
}

/// La lógica, separada de la lista real para poder probarla con una migración `Blocking`: hoy no
/// hay ninguna, y un test que solo recorre seis `Safe` no demuestra que el rechazo funcione.
fn first_blocking_in(migrations: &[(i64, &str, ResumeSafety)], from: i64) -> Option<i64> {
    migrations
        .iter()
        .find(|(v, _, safety)| *v > from && *safety == ResumeSafety::Blocking)
        .map(|(v, _, _)| *v)
}

/// PRAGMAs de la conexión **escritora**. Ver `docs/02-MODELO-DATOS.md §2`.
///
/// Difieren de los del documento en un punto, y es deliberado: **el escritor no usa `mmap`**.
///
/// El mapeo en memoria acelera lecturas, y esta conexión solo escribe. Medido sobre un rastreo
/// de 10.000 páginas con 1,32 millones de enlaces: `mmap_size = 256 MB` costaba **70 MB de RSS
/// adicionales y cero milisegundos** (17,94 s frente a 17,86 s). Con el criterio de memoria de
/// memoria en 200 MB, regalar 70 MB por nada no es aceptable.
///
/// La conexión de solo lectura de la UI sí debe usarlo: ver [`READER_PRAGMAS`].
///
/// **`busy_timeout` no es opcional.** El fichero de rastreo se lee mientras se escribe —la CLI
/// con `report` en otra terminal hoy, una interfaz por diseño (`CONVENTIONS.md §2.2`)— y sin
/// el timeout cualquier roce de bloqueos era un `database is locked` inmediato, incluso cuando
/// el lector iba a soltar el fichero décimas de segundo después. Con 5 s se aguanta cualquier
/// lector transitorio; a un lector permanente lo trata [`finalize`] como degradación, no error.
const WRITER_PRAGMAS: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA busy_timeout = 5000;
    PRAGMA synchronous  = NORMAL;
    PRAGMA foreign_keys = ON;
    PRAGMA temp_store   = MEMORY;
    PRAGMA cache_size   = -64000;
    PRAGMA mmap_size    = 0;
";

/// PRAGMAs recomendados para la conexión de **solo lectura** de la UI.
///
/// Aquí el mapeo sí compensa: la tabla se pagina y se ordena constantemente, y son todo
/// lecturas. `immutable=false` porque el core puede estar escribiendo en WAL mientras tanto.
///
/// No los aplica el core —la UI abre su propia conexión— pero viven aquí para que Swift y C#
/// no tengan que deducirlos ni acaben divergiendo entre plataformas.
pub const READER_PRAGMAS: &str = "
    PRAGMA query_only   = 1;
    PRAGMA busy_timeout = 5000;
    PRAGMA temp_store   = MEMORY;
    PRAGMA cache_size   = -64000;
    PRAGMA mmap_size    = 268435456;
";

/// Abre (o crea) un fichero de rastreo y aplica las migraciones pendientes.
pub fn open_writer(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(WRITER_PRAGMAS)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Reabre un almacén que **tiene que existir ya**.
///
/// La diferencia con [`open_writer`] no es cosmética. La pasada final reabre el fichero que acaba
/// de escribir el hilo escritor, y `Connection::open` lo crea si no está: si algo lo borró
/// mientras tanto —otro rastreo del mismo sitio, que usa el mismo nombre por defecto— la pasada
/// final se ejecutaba contra una base nueva y vacía, y el rastreo terminaba anunciando «31 URLs,
/// 217 hallazgos» con un fichero de cero filas. Reproducido.
///
/// Un fichero que desaparece a mitad es un error, y hay que decirlo.
pub fn reopen_writer(path: &std::path::Path) -> Result<Connection> {
    use rusqlite::OpenFlags;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| {
        CoreError::Store(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(14),
            Some(format!(
                "el fichero de rastreo {} ha desaparecido durante el rastreo: {e}. \
                 ¿Hay otro rastreo escribiendo sobre el mismo fichero?",
                path.display()
            )),
        ))
    })?;
    conn.execute_batch(WRITER_PRAGMAS)?;
    Ok(conn)
}

/// Aplica las migraciones que falten. Idempotente.
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
             version    INTEGER NOT NULL,
             applied_at TEXT    NOT NULL
         );",
    )?;

    let current: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))?;

    if current > SCHEMA_VERSION {
        return Err(CoreError::SchemaMismatch { found: current, expected: SCHEMA_VERSION });
    }

    for (version, sql, _) in MIGRATIONS {
        if *version > current {
            // Cada migración y su marca de versión van en **una sola transacción**, y con
            // `BEGIN IMMEDIATE` para tomar el bloqueo de escritura desde el principio.
            //
            // Sin esto pasaban dos cosas, las dos reproducidas. La leve: dos rastreos sobre el
            // mismo fichero se pisaban y un tercio fallaba con «table crawl_meta already exists».
            // La grave: si el proceso moría entre crear las tablas y escribir la fila de versión
            // —disco lleno, `kill -9`, la propia carrera— el fichero quedaba **inservible para
            // siempre**: al reabrirlo se intentaba aplicar la 001 otra vez sobre unas tablas que
            // ya existían. Eso contradice de raíz «un rastreo de hace un año debe seguir
            // abriéndose».
            //
            // Con la transacción, o se aplica entera y queda marcada, o no queda nada.
            conn.execute_batch("BEGIN IMMEDIATE")?;
            let aplicada = (|| -> Result<()> {
                conn.execute_batch(sql)?;
                conn.execute(
                    "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
                    rusqlite::params![version],
                )?;
                Ok(())
            })();
            match aplicada {
                Ok(()) => conn.execute_batch("COMMIT")?,
                Err(e) => {
                    // El rollback puede fallar si la conexión está en mal estado; el error que
                    // importa es el de la migración.
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

/// Cierre del rastreo: actualiza las estadísticas del planificador de consultas y deja el
/// fichero **autocontenido**.
///
/// **No compacta.** `02-MODELO-DATOS.md §2` recomienda un `VACUUM` al terminar «puede reducir el
/// fichero un 30-40%», pero medido sobre un rastreo de 50.000 URLs y 6,15 millones de enlaces
/// redujo **un 2%** (380 MB → 372 MB) y disparó el pico de memoria de 168 MB a 246 MB.
///
/// El 30-40% aparece cuando hay fragmentación por borrados y actualizaciones. Un fichero de
/// rastreo se escribe una sola vez y de forma incremental, así que casi no la hay. Pagar 78 MB
/// de pico —en la métrica que decide el argumento del producto— por un 2% de disco es mal negocio.
///
/// Quien lo quiera, lo tiene en [`compact`].
///
/// **Sí sale del modo WAL.** El modo de journal queda grabado en la cabecera del fichero, así
/// que un rastreo terminado en WAL hacía que *cada* lectura posterior —`report`, `export`, la
/// UI— recreara los `.sqlite-wal` y `.sqlite-shm` junto al fichero, y una conexión de solo
/// lectura no puede borrarlos al cerrar. «Un rastreo = un fichero portable» (`CONVENTIONS.md §2.3`)
/// se rompe en cuanto alguien copia el `.sqlite` a otra máquina y le sobran —o, si el WAL
/// llevara páginas sin volcar, le *faltan*— dos ficheros con pinta de error. El checkpoint
/// vuelca el WAL entero y `journal_mode = DELETE` lo elimina y deja el modo grabado; el
/// siguiente rastreo vuelve a WAL en [`open_writer`] vía [`WRITER_PRAGMAS`].
///
/// **Un lector concurrente no es un error.** Salir de WAL exige que ninguna otra conexión
/// tenga el fichero abierto —ni siquiera ociosa: cada conexión a una base en WAL retiene el
/// cerrojo del `-shm` mientras vive—, y ese lector existe legítimamente: el `.sqlite` abierto
/// en un visor, un `report` en otra terminal, una interfaz leyendo mientras el core
/// escribe, que *es* la arquitectura (`CONVENTIONS.md §2.2`). Antes esto se propagaba como error
/// («database is locked») **después** de marcar `done`: el usuario creía haber perdido un
/// rastreo que estaba entero, y `resume` le respondía «ya terminó». Ahora es una degradación
/// declarada: el fichero se queda en WAL —perfectamente funcional— y quien llama decide cómo
/// avisar de que los ficheros auxiliares forman parte del rastreo. [`try_make_portable`] lo
/// reintenta más tarde, cuando el lector ya no esté.
pub fn finalize(conn: &Connection) -> Result<FinalizeOutcome> {
    conn.execute_batch("PRAGMA optimize;")?;
    exit_wal_mode(conn)
}

/// Cómo quedó el fichero al cerrar: portable de verdad, o funcional pero con séquito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizeOutcome {
    /// El WAL se volcó y se eliminó: el `.sqlite` viaja solo.
    Portable,
    /// Otra conexión mantiene el fichero en WAL: los `-wal`/`-shm` de al lado forman parte
    /// del rastreo y copiarlo suelto puede perder datos. Quien llama debe decirlo.
    WalKept,
}

/// Vuelca el WAL y saca el fichero del modo WAL, tolerando al lector concurrente.
///
/// El caso medido que obliga a tolerarlo: matar el proceso durante la pasada final dejó un
/// `.sqlite` de 5,3 GB con un `-wal` de 1,0 GB al lado (2026-08-02). Quien copiaba solo el
/// `.sqlite` se llevaba un rastreo al que le faltaba un giga **sin ningún aviso**.
fn exit_wal_mode(conn: &Connection) -> Result<FinalizeOutcome> {
    // El checkpoint puede quedarse corto si un lector está en mitad de una transacción; no es
    // un error: `journal_mode = DELETE` hace su propio checkpoint al salir de WAL, y si
    // tampoco puede, lo pendiente sigue a salvo en el `-wal`.
    match conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get::<_, i64>(0)) {
        Ok(_) => {}
        Err(e) if is_locked(&e) => {}
        Err(e) => return Err(e.into()),
    }
    // Según la versión de SQLite, el cambio imposible responde con SQLITE_BUSY o devolviendo
    // el modo sin cambiar; las dos formas significan lo mismo: el lector sigue ahí.
    match conn.query_row("PRAGMA journal_mode = DELETE", [], |r| r.get::<_, String>(0)) {
        Ok(mode) if mode.eq_ignore_ascii_case("delete") => Ok(FinalizeOutcome::Portable),
        Ok(_) => Ok(FinalizeOutcome::WalKept),
        Err(e) if is_locked(&e) => Ok(FinalizeOutcome::WalKept),
        Err(e) => Err(e.into()),
    }
}

/// ¿Es este error un «alguien más tiene el fichero», y no un fallo real?
fn is_locked(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _) if matches!(
            f.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        )
    )
}

/// Reintenta dejar portable un fichero que se quedó en WAL.
///
/// Es la otra mitad de la degradación de [`finalize`]: si el cierre no pudo salir de WAL por
/// un lector concurrente, alguien tiene que volver a intentarlo cuando el lector ya no esté.
/// La llaman los comandos de lectura de la CLI (`report`, `export`) de forma oportunista:
/// abren en escritura solo para esto, y cualquier impedimento —el lector sigue, un rastreo
/// vivo tiene su conexión abierta, el fichero está en un medio de solo lectura— devuelve
/// [`FinalizeOutcome::WalKept`] o error sin tocar nada. Sobre un fichero ya portable no hace
/// ninguna escritura.
///
/// **No migra ni valida el esquema a propósito**: solo toca el modo de journal, que es igual
/// en todas las versiones, así que puede curarle el WAL incluso a un fichero de un esquema
/// más nuevo que este binario.
pub fn try_make_portable(path: &Path) -> Result<FinalizeOutcome> {
    use rusqlite::OpenFlags;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )?;
    let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Ok(FinalizeOutcome::Portable);
    }
    // Timeout corto: esto es oportunista y corre delante de un `report`; si el fichero está
    // ocupado, mejor responder ya que retener al usuario cinco segundos.
    conn.busy_timeout(std::time::Duration::from_millis(250))?;
    exit_wal_mode(&conn)
}

/// Compacta el fichero. Explícito, nunca automático.
///
/// Se ejecuta con `temp_store = FILE` y no con el `MEMORY` de la sesión: `VACUUM` reconstruye la
/// base entera en una temporal, y mandarla a RAM metía los 380 MB del fichero en memoria. El
/// pico saltaba a 642 MB, cuadruplicando el del rastreo completo.
pub fn compact(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA temp_store = FILE;")?;
    let resultado = conn.execute_batch("VACUUM;");
    // El PRAGMA se restaura pase lo que pase: la conexión puede seguir usándose.
    conn.execute_batch("PRAGMA temp_store = MEMORY;")?;
    resultado?;
    Ok(())
}

// ---------------------------------------------------------------- Cerrojo de escritor

/// Cerrojo de exclusividad sobre un fichero de rastreo: **un proceso escribiendo, y solo uno**.
///
/// SQLite no lo impide por sí mismo —en WAL dos conexiones escritoras se turnan sin protestar—
/// y las consecuencias de dos rastreos sobre el mismo fichero están medidas: `links` no tiene
/// UNIQUE y se duplica, y el `finalize` del que pierde falla con BUSY. Peor aún, con el nombre
/// por defecto determinista y 100 blogs en cron, un segundo `crawl` rotaba el fichero **vivo**
/// del primero a `.prev.sqlite` y el primero acababa escribiendo su pasada final dentro del
/// fichero del segundo.
///
/// # Por qué un cerrojo de fichero y no un latido en `crawl_meta`
///
/// Se evaluaron los dos. El latido (pid + marca de tiempo en la base) es portable, pero exige
/// decidir un umbral de «muerto», y ese umbral no existe: durante la pasada final una sola
/// regla de conjunto puede retener la conexión **horas** en una única sentencia (medido:
/// más de 8 h el 2026-08-02), y mientras dura nadie puede refrescar el latido — SQLite solo
/// admite un escritor a la vez sobre el mismo fichero. Cualquier umbral corto declararía
/// muerto un rastreo vivo (y volveríamos a los dos escritores); uno largo haría esperar al
/// usuario tras un `kill -9` real.
///
/// Un cerrojo del sistema operativo no tiene ese dilema: **lo suelta el sistema en el instante
/// en que el proceso muere**, exactamente la semántica que `resume` necesita («rechaza un
/// `running` vivo, acepta uno muerto») sin heurística de tiempo. Y en vez de `flock` —que en
/// Windows es otra API y en Rust estable de este MSRV (1.85) no existe aún— se usa el propio
/// SQLite: un `BEGIN EXCLUSIVE` sobre un fichero lateral `<store>.lock`. Es la misma primitiva
/// de bloqueo del sistema (fcntl en macOS, LockFileEx en Windows) con el comportamiento ya
/// homogéneo entre plataformas, sin dependencias nuevas, y con el registro de bloqueos por
/// inodo que SQLite mantiene en proceso, que hace que dos motores del mismo proceso también
/// se excluyan (los cerrojos POSIX a secas no lo garantizan). En un sistema de ficheros de
/// red hereda las limitaciones de SQLite ahí — el mismo terreno en el que el fichero de
/// rastreo ya no debería estar.
///
/// El lateral `.lock` no se borra al soltar: eliminarlo abriría la carrera clásica de que dos
/// procesos posteriores bloqueen inodos distintos con el mismo nombre y se crean ambos únicos.
/// Un `.lock` huérfano está desbloqueado, pesa cero y se reutiliza.
#[derive(Debug)]
pub struct StoreLock {
    /// La transacción exclusiva vive lo que viva la conexión; al soltarla (drop) el sistema
    /// libera el cerrojo — también si el proceso muere de golpe.
    _conn: Connection,
}

impl StoreLock {
    /// Toma la exclusiva de escritura del fichero de rastreo, o dice quién no le deja.
    ///
    /// Falla con [`CoreError::StoreLocked`] si otro proceso (u otro motor de este mismo
    /// proceso) la tiene. No espera: el que llega segundo tiene que enterarse ya, no
    /// ponerse a la cola de un rastreo de diez horas.
    pub fn acquire(store_path: &Path) -> Result<Self> {
        let conn = Connection::open(lock_path(store_path))?;
        match conn.execute_batch("BEGIN EXCLUSIVE") {
            Ok(()) => Ok(Self { _conn: conn }),
            Err(e) if is_locked(&e) => Err(CoreError::StoreLocked {
                path: store_path.display().to_string(),
            }),
            Err(e) => Err(e.into()),
        }
    }
}

/// El fichero lateral del cerrojo: `crawl-x.sqlite` → `crawl-x.sqlite.lock`.
///
/// Va aparte del `.sqlite` a propósito: bloquear la base misma con `BEGIN EXCLUSIVE`
/// impediría escribir al propio hilo escritor.
fn lock_path(store: &Path) -> PathBuf {
    let mut name = store.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migra_desde_cero_y_es_idempotente() {
        let conn = Connection::open_in_memory().expect("abrir memoria");
        migrate(&conn).expect("primera migracion");
        migrate(&conn).expect("segunda migracion");

        let v: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .expect("leer version");
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn crea_todas_las_tablas_del_modelo() {
        let conn = Connection::open_in_memory().expect("abrir memoria");
        migrate(&conn).expect("migrar");
        for t in [
            "crawl_meta", "urls", "pages", "links", "resources", "images", "issues",
            "extractions", "adapter_entities",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![t],
                    |r| r.get(0),
                )
                .expect("consultar sqlite_master");
            assert_eq!(n, 1, "falta la tabla {t}");
        }
    }

    #[test]
    fn un_rastreo_antiguo_se_reanuda_si_lo_que_le_falta_no_cambia_lo_que_escribe() {
        // El caso real que lo motivó: un rastreo v5 y un binario v6 cuya única migración nueva
        // crea un índice. Antes esto era irrecuperable y el error decía «vuelve a rastrearlo»,
        // sobre dieciocho horas de trabajo.
        assert_eq!(first_blocking_resume(5), None, "un índice no invalida un rastreo a medias");
        assert_eq!(first_blocking_resume(1), None, "las seis publicadas son seguras");
        assert_eq!(first_blocking_resume(SCHEMA_VERSION), None, "al día, nada que cruzar");
    }

    #[test]
    fn una_migracion_que_cambia_lo_que_se_escribe_si_impide_reanudar() {
        // Hoy no hay ninguna `Blocking`, así que el rechazo se prueba con una lista de mentira:
        // si no, este test pasaría por no haber nada que rechazar, que es justo el error que
        // deja un guardián inútil hasta el día que hace falta.
        let inventadas: &[(i64, &str, ResumeSafety)] = &[
            (1, "", ResumeSafety::Safe),
            (2, "", ResumeSafety::Safe),
            (3, "", ResumeSafety::Blocking),
            (4, "", ResumeSafety::Safe),
        ];
        assert_eq!(first_blocking_in(inventadas, 1), Some(3), "hay que cruzar la 3, que bloquea");
        assert_eq!(first_blocking_in(inventadas, 2), Some(3));
        assert_eq!(first_blocking_in(inventadas, 3), None, "ya se cruzó: lo de después es seguro");
        assert_eq!(first_blocking_in(inventadas, 4), None);
    }

    #[test]
    fn la_portada_no_se_reporta_como_pagina_huerfana() {
        // Regresión: la raíz cumple las dos condiciones de `v_orphans` (está en el sitemap y
        // nadie la enlaza, porque es el punto de entrada), así que salía como huérfana en
        // todos los rastreos.
        let conn = Connection::open_in_memory().expect("abrir memoria");
        migrate(&conn).expect("migrar");

        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime)
             VALUES ('x','p','P','https://ejemplo.es/','http',datetime('now'),'done','{}',
                     '0','0','free')",
            [],
        )
        .expect("insertar crawl_meta");

        let add = |id: i64, url: &str| {
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal,
                                   in_sitemap, crawl_state)
                 VALUES (?1, ?2, ?1, 'https', 'ejemplo.es', '/', 1, 1, 'done')",
                rusqlite::params![id, url],
            )
            .expect("insertar url");
            // Desde la migración 005 `v_orphans` exige fila en `pages`: una URL sin ella no se
            // ha parseado como HTML y por tanto no es una página. Omitirla aquí construiría un
            // estado que el motor no produce.
            conn.execute("INSERT INTO pages (url_id, is_indexable) VALUES (?1, 1)", rusqlite::params![id])
                .expect("insertar page");
        };
        add(1, "https://ejemplo.es/");
        add(2, "https://ejemplo.es/de-verdad-huerfana");

        let orphans: Vec<String> = {
            let mut stmt = conn.prepare("SELECT url FROM v_orphans").expect("preparar");
            let rows = stmt.query_map([], |r| r.get(0)).expect("consultar");
            rows.collect::<rusqlite::Result<_>>().expect("recoger")
        };

        assert_eq!(orphans, vec!["https://ejemplo.es/de-verdad-huerfana"], "solo la real");
    }

    #[test]
    fn la_portada_sin_barra_final_tampoco_se_reporta_como_huerfana() {
        // `base_url` guarda lo que escribió el usuario al lanzar el rastreo, que puede llevar
        // barra o no; la URL normalizada siempre la lleva.
        let conn = Connection::open_in_memory().expect("abrir memoria");
        migrate(&conn).expect("migrar");
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     status, config_json, core_version, rules_version,
                                     tier_at_runtime)
             VALUES ('x','p','P','https://ejemplo.es','http',datetime('now'),'done','{}',
                     '0','0','free')",
            [],
        )
        .expect("insertar crawl_meta");
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state)
             VALUES (1, 'https://ejemplo.es/', 1, 'https', 'ejemplo.es', '/', 1, 1, 'done')",
            [],
        )
        .expect("insertar url");
        conn.execute("INSERT INTO pages (url_id, is_indexable) VALUES (1, 1)", [])
            .expect("insertar page");

        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM v_orphans", [], |r| r.get(0))
            .expect("contar");
        assert_eq!(n, 0);
    }

    #[test]
    fn finalize_deja_el_fichero_sin_wal_ni_shm_y_las_lecturas_no_los_recrean() {
        // «Un rastreo = un fichero portable»: quien copia solo el `.sqlite` a otra máquina no
        // debe perder datos ni llevarse dos ficheros auxiliares con pinta de error.
        let dir = std::env::temp_dir().join(format!("crawlforge-wal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear temporal");
        let path = dir.join("crawl.sqlite");
        let wal = dir.join("crawl.sqlite-wal");
        let shm = dir.join("crawl.sqlite-shm");

        {
            let conn = open_writer(&path).expect("abrir el escritor");
            conn.execute(
                "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode,
                                         started_at, status, config_json, core_version,
                                         rules_version, tier_at_runtime)
                 VALUES ('x','p','P','https://ejemplo.es/','http',datetime('now'),'done','{}',
                         '0','0','free')",
                [],
            )
            .expect("escribir algo");
            assert!(wal.exists(), "durante el rastreo el WAL existe: es el modo de escritura");

            let salida = finalize(&conn).expect("finalize");
            assert_eq!(salida, FinalizeOutcome::Portable, "sin lectores, el cierre es limpio");
            assert!(!wal.exists(), "finalize vuelca y elimina el WAL");
            assert!(!shm.exists(), "y su memoria compartida");
        }

        // Una lectura posterior —lo que hacen `report`, `export` y la UI— no los recrea: el
        // modo DELETE quedó grabado en la cabecera del fichero.
        {
            let conn = Connection::open_with_flags(
                &path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .expect("abrir en solo lectura");
            let n: i64 = conn
                .query_row("SELECT COUNT(*) FROM crawl_meta", [], |r| r.get(0))
                .expect("leer");
            assert_eq!(n, 1, "los datos siguen ahí tras el checkpoint");
        }
        assert!(!wal.exists() && !shm.exists(), "leer el fichero no deja residuo");

        // Y el siguiente rastreo vuelve a WAL sin fricción.
        {
            let conn = open_writer(&path).expect("reabrir el escritor");
            let modo: String =
                conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).expect("modo");
            assert_eq!(modo, "wal", "el escritor recupera su modo");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finalize_con_un_lector_abierto_se_degrada_a_wal_en_vez_de_fallar() {
        // El caso *normal* en cuanto haya una interfaz: la UI lee el mismo
        // fichero que el core escribe. Salir de WAL exige ser la única conexión, así que con
        // un lector el cierre no puede ser portable — pero eso es una degradación, no un
        // error: antes esto fallaba con «database is locked» después de marcar `done` y el
        // usuario creía haber perdido un rastreo que estaba entero.
        //
        // Este test falla sin el arreglo: `finalize` propagaba el error del cambio de modo.
        let dir = std::env::temp_dir().join(format!("crawlforge-lector-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear temporal");
        let path = dir.join("crawl.sqlite");

        let conn = open_writer(&path).expect("abrir el escritor");
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode,
                                     started_at, status, config_json, core_version,
                                     rules_version, tier_at_runtime)
             VALUES ('x','p','P','https://ejemplo.es/','http',datetime('now'),'done','{}',
                     '0','0','free')",
            [],
        )
        .expect("escribir algo");

        // El lector: una conexión de solo lectura que ya ha leído (con eso retiene el
        // cerrojo del -shm mientras viva, que es lo que impide salir de WAL).
        let lector = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("abrir el lector");
        let _: i64 = lector
            .query_row("SELECT COUNT(*) FROM crawl_meta", [], |r| r.get(0))
            .expect("leer");

        let salida = finalize(&conn).expect("con un lector, finalize degrada, no falla");
        assert_eq!(salida, FinalizeOutcome::WalKept, "el fichero se queda en WAL");

        // Los datos siguen íntegros y legibles para el propio lector.
        let n: i64 = lector
            .query_row("SELECT COUNT(*) FROM crawl_meta", [], |r| r.get(0))
            .expect("releer");
        assert_eq!(n, 1);

        // Sin el lector, `try_make_portable` termina el trabajo que el cierre no pudo hacer.
        drop(lector);
        drop(conn);
        let salida = try_make_portable(&path).expect("reintentar el cierre");
        assert_eq!(salida, FinalizeOutcome::Portable);
        assert!(!dir.join("crawl.sqlite-wal").exists(), "el WAL quedó volcado y eliminado");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_fichero_ya_portable_no_se_toca_al_reintentar() {
        let dir = std::env::temp_dir().join(format!("crawlforge-portable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear temporal");
        let path = dir.join("crawl.sqlite");
        {
            let conn = open_writer(&path).expect("abrir");
            finalize(&conn).expect("cerrar limpio");
        }
        let salida = try_make_portable(&path).expect("sobre un fichero portable no hay nada que hacer");
        assert_eq!(salida, FinalizeOutcome::Portable);
        assert!(
            !dir.join("crawl.sqlite-wal").exists(),
            "el reintento no debe devolver el fichero al modo WAL"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_cerrojo_del_fichero_es_exclusivo_y_se_libera_al_soltarlo() {
        // La base del §3.3: dos escritores sobre el mismo fichero duplican `links` y se
        // pisan el cierre. El segundo en llegar tiene que enterarse ya, no ponerse a la cola.
        let dir = std::env::temp_dir().join(format!("crawlforge-cerrojo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear temporal");
        let store = dir.join("crawl.sqlite");

        let primero = StoreLock::acquire(&store).expect("el primero toma la exclusiva");
        let err = StoreLock::acquire(&store).expect_err("el segundo tiene que ser rechazado");
        assert!(
            matches!(err, CoreError::StoreLocked { .. }),
            "el error dice que hay otro escritor: {err:?}"
        );

        // Al soltarlo —o al morir el proceso, que es lo mismo para el sistema— la exclusiva
        // queda libre y el siguiente entra sin heurísticas de tiempo.
        drop(primero);
        StoreLock::acquire(&store).expect("libre tras soltar el cerrojo");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rechaza_un_fichero_de_esquema_mas_nuevo() {
        let conn = Connection::open_in_memory().expect("abrir memoria");
        migrate(&conn).expect("migrar");
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
            rusqlite::params![SCHEMA_VERSION + 1],
        )
        .expect("insertar version futura");
        assert!(matches!(migrate(&conn), Err(CoreError::SchemaMismatch { .. })));
    }
}

#[cfg(test)]
mod tests_migracion_atomica {
    use super::*;

    /// Una migración que falla a mitad no debe dejar el fichero a medias.
    #[test]
    fn una_migracion_interrumpida_no_deja_el_fichero_inservible() {
        let dir = std::env::temp_dir().join(format!("crawlforge-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear temporal");
        let path = dir.join("crawl.sqlite");

        {
            // Se simula el estado que dejaba una migración cortada: las tablas de la 001 creadas
            // pero sin la fila de `schema_version`. Antes, ese fichero no se podía volver a abrir
            // nunca; ahora la migración se reintenta desde cero sobre una base limpia porque
            // aquella nunca llegó a confirmarse.
            let conn = Connection::open(&path).expect("abrir");
            conn.execute_batch("CREATE TABLE crawl_meta (id TEXT PRIMARY KEY);")
                .expect("dejar el fichero a medias");
        }

        // El fichero está corrupto de la forma que producía el fallo: el error tiene que ser
        // claro, no un panic, y no debe dejar residuo.
        let resultado = open_writer(&path);
        assert!(resultado.is_err(), "un fichero a medias tiene que dar error, no abrirse a ciegas");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrar_dos_veces_seguidas_sobre_la_misma_conexion_no_falla() {
        let conn = Connection::open_in_memory().expect("abrir");
        migrate(&conn).expect("primera");
        migrate(&conn).expect("segunda: la migración es idempotente");
        let v: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .expect("leer versión");
        assert_eq!(v, SCHEMA_VERSION);
    }
}
