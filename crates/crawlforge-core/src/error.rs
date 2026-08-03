//! Variantes de error explicitas. Nunca `Box<dyn Error>`. Ver `docs/01-ARQUITECTURA.md` SS7.
//!
//! Los mensajes van en ingles: estos errores atraviesan la frontera del core y acaban tal cual
//! en la salida de la CLI, que es toda en ingles a proposito (la plantilla de `clap` no es
//! localizable). La revision de UX de 2026-08-01 (§5.3) cazo un «URL invalida» en castellano
//! en mitad de una salida inglesa; la traduccion al idioma del usuario, cuando exista canal,
//! se hace en el frontal con tablas de cadenas, nunca aqui.

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("store error: {0}")]
    Store(#[from] rusqlite::Error),

    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("incompatible schema: the file is v{found}, this core writes v{expected}")]
    SchemaMismatch { found: i64, expected: i64 },

    #[error("HTTP client error: {0}")]
    Http(String),

    #[error("the writer thread has died: the crawl cannot be saved")]
    WriterGone,

    #[error("invalid configuration: {0}")]
    Config(String),

    /// Un patron de include/exclude que no compila. Nombra al culpable: con varios patrones
    /// en la configuracion, "regex parse error" a secas no dice cual corregir.
    #[error("invalid {kind} pattern {pattern:?}: {message}")]
    InvalidPattern {
        kind: &'static str,
        pattern: String,
        message: String,
    },

    /// El fichero no admite reanudacion: ya termino, es de otra version de esquema o su
    /// configuracion guardada no se puede leer. El motivo dice cual de los tres es.
    #[error("cannot resume {path}: {reason}")]
    NotResumable { path: String, reason: String },

    /// Otro proceso tiene la exclusiva de escritura del fichero de rastreo. Ver
    /// [`crate::store::StoreLock`]: dos escritores duplican `links` y se pisan el cierre.
    #[error(
        "another crawlforge process is writing {path} right now: wait for it to finish, \
         or stop it, before crawling or resuming this file"
    )]
    StoreLocked { path: String },
}

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::CoreError;

    #[test]
    fn los_errores_del_core_hablan_ingles() {
        // Regresión de la revisión 2026-08-01 §5.3: `crawlforge crawl ...` respondía
        // «URL invalida: relative URL without a base» —castellano, y sin tilde— dentro de una
        // salida que es toda en inglés. El mensaje del core es lo que ve el usuario.
        let err = CoreError::from(
            url::Url::parse("sin-esquema").expect_err("una URL relativa no parsea sola"),
        );
        let msg = err.to_string();
        assert!(msg.starts_with("invalid URL:"), "{msg}");
        assert!(!msg.contains("invalida"), "sin castellano en la salida inglesa: {msg}");
    }
}
