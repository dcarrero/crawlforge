//! Patrones de inclusión y exclusión de URLs. Ver `docs/03-MOTOR-CRAWL.md §9`.
//!
//! # Por qué expresiones regulares y no globs
//!
//! Quien escribe estos patrones es un consultor SEO, y su referencia es Screaming Frog, que
//! usa expresiones regulares en include/exclude desde siempre: los patrones que la gente ya
//! tiene apuntados (`\?replytocom=`, `/wp-admin/`, `/page/\d+/`) deben funcionar tal cual.
//! Además, la coincidencia es **sin anclar**: se busca el patrón en cualquier parte de la URL
//! completa, así que una cadena literal como `/carrito/` funciona como un «contiene» sin saber
//! nada de regex. Un glob habría obligado a inventar una sintaxis propia que nadie más usa.
//!
//! El coste está acotado por diseño: el crate `regex` no tiene *backtracking* —un patrón
//! patológico no puede degenerar en tiempo exponencial— y cada patrón se compila **una sola
//! vez** por rastreo, no por URL. El motor además solo evalúa el filtro una vez por URL única
//! (el índice de vistas del frontier corta antes las repeticiones), así que en el caso denso
//! de 6,15 M de enlaces el filtro se evalúa ~50.000 veces, no seis millones.
//!
//! # Semántica
//!
//! - `exclude` **gana** sobre `include`: si una URL casa con los dos, se excluye. Es la
//!   convención de Screaming Frog y la única que permite «todo el blog menos los borradores»
//!   (`--include '/blog/' --exclude '/blog/borradores/'`).
//! - Un `include` no vacío restringe: solo se rastrean las URLs que casen con **alguno** de
//!   sus patrones. Vacío, no restringe nada.
//! - La URL excluida **queda registrada** con `exclusion_reason='pattern'`, no desaparece:
//!   el informe debe poder decir «esto no se rastreó porque tú lo excluiste».

use crate::error::CoreError;
use crate::job::CrawlLimits;
use regex::Regex;

/// El filtro de URLs de un rastreo, con sus patrones ya compilados.
///
/// Se construye una vez al arrancar el rastreo —donde un patrón inválido es un error
/// inmediato, antes de tocar el disco— y después solo se consulta.
#[derive(Debug, Default)]
pub struct UrlFilter {
    include: Vec<Regex>,
    exclude: Vec<Regex>,
}

impl UrlFilter {
    /// Compila los patrones de unos límites de rastreo.
    ///
    /// Es la validación que ve el usuario: un patrón inválido devuelve un error que nombra el
    /// patrón y explica el fallo, **antes** de que el rastreo empiece.
    pub fn from_limits(limits: &CrawlLimits) -> Result<Self, CoreError> {
        Self::compile(&limits.include_patterns, &limits.exclude_patterns)
    }

    /// Compila listas de patrones sueltas. `from_limits` es la vía normal.
    pub fn compile(include: &[String], exclude: &[String]) -> Result<Self, CoreError> {
        Ok(Self {
            include: compile_list(include, "include")?,
            exclude: compile_list(exclude, "exclude")?,
        })
    }

    /// ¿No hay ningún patrón configurado?
    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty()
    }

    /// ¿Se puede rastrear esta URL?
    ///
    /// Recibe la URL completa ya normalizada (`https://host/ruta?query`): los patrones pueden
    /// mirar el host, la ruta o la query sin sintaxis especial.
    pub fn allows(&self, url: &str) -> bool {
        // El exclude gana: es lo que hace posible «todo el blog menos los borradores».
        if self.exclude.iter().any(|re| re.is_match(url)) {
            return false;
        }
        self.include.is_empty() || self.include.iter().any(|re| re.is_match(url))
    }
}

fn compile_list(patterns: &[String], kind: &'static str) -> Result<Vec<Regex>, CoreError> {
    patterns
        .iter()
        .map(|p| {
            Regex::new(p).map_err(|e| CoreError::InvalidPattern {
                kind,
                pattern: p.clone(),
                message: e.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(include: &[&str], exclude: &[&str]) -> UrlFilter {
        UrlFilter::compile(
            &include.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &exclude.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
        .expect("patrones de test válidos")
    }

    #[test]
    fn sin_patrones_todo_pasa() {
        let f = filter(&[], &[]);
        assert!(f.is_empty());
        assert!(f.allows("https://ejemplo.es/cualquier-cosa"));
    }

    #[test]
    fn una_cadena_literal_funciona_como_contiene() {
        // El caso del consultor que no sabe regex: pega la ruta tal cual.
        let f = filter(&[], &["/wp-admin/"]);
        assert!(!f.allows("https://ejemplo.es/wp-admin/options.php"));
        assert!(f.allows("https://ejemplo.es/blog/"));
    }

    #[test]
    fn el_include_restringe_de_verdad() {
        let f = filter(&["/blog/"], &[]);
        assert!(f.allows("https://ejemplo.es/blog/post-1"));
        assert!(!f.allows("https://ejemplo.es/tienda/producto"));
        assert!(!f.allows("https://ejemplo.es/"), "la raíz tampoco casa con /blog/");
    }

    #[test]
    fn el_exclude_gana_al_include() {
        // «Todo el blog menos los borradores»: la única semántica en que esto se puede decir.
        let f = filter(&["/blog/"], &["/blog/borradores/"]);
        assert!(f.allows("https://ejemplo.es/blog/post-1"));
        assert!(!f.allows("https://ejemplo.es/blog/borradores/wip"));
    }

    #[test]
    fn varios_patrones_son_una_disyuncion() {
        let f = filter(&["/blog/", "/docs/"], &[]);
        assert!(f.allows("https://ejemplo.es/blog/a"));
        assert!(f.allows("https://ejemplo.es/docs/b"));
        assert!(!f.allows("https://ejemplo.es/tienda/c"));
    }

    #[test]
    fn los_patrones_ven_la_query() {
        // El clásico de WordPress: los permalinks de comentario duplican cada entrada.
        let f = filter(&[], &[r"\?replytocom="]);
        assert!(!f.allows("https://ejemplo.es/post/?replytocom=5"));
        assert!(f.allows("https://ejemplo.es/post/"));
    }

    #[test]
    fn un_regex_de_verdad_tambien_funciona() {
        let f = filter(&[], &[r"/page/\d+/$"]);
        assert!(!f.allows("https://ejemplo.es/blog/page/2/"));
        assert!(f.allows("https://ejemplo.es/blog/pagenotfound/"));
    }

    #[test]
    fn un_patron_invalido_es_un_error_que_nombra_al_culpable() {
        let err = UrlFilter::compile(&[], &["[".to_string()])
            .expect_err("un corchete sin cerrar no compila");
        let msg = err.to_string();
        assert!(msg.contains("exclude"), "dice en qué lista está: {msg}");
        assert!(msg.contains('['), "dice qué patrón es: {msg}");
    }

    #[test]
    fn un_include_invalido_se_nombra_como_include() {
        let err = UrlFilter::compile(&["(".to_string()], &[])
            .expect_err("un paréntesis sin cerrar no compila");
        assert!(err.to_string().contains("include"), "{err}");
    }
}
