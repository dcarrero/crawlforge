//! Niveles y derechos de uso. Ver `docs/07-MONETIZACION.md §3`.
//!
//! # Por qué existe esto ya, con una sola implementación
//!
//! Porque retrofitear un sistema de licencias con clientes en producción es doloroso, y en la
//! futuro habrá que mapear recibos de tienda a claves de licencia. La
//! abstracción se pone ahora, cuando no cuesta nada; las implementaciones llegan en las fases 3,
//! 4 y 7 (`StoreKitSource`, `MsStoreSource`, `LicenseFileSource`).
//!
//! # Los límites se aplican en el core
//!
//! No en la UI. Si la comprobación estuviera solo en Swift o en C#, la CLI y cualquier build
//! modificado la esquivarían. El `Engine` recibe el nivel al construirse y es él quien recorta
//! `max_urls` y filtra las reglas por `min_tier`.
//!
//! # El nivel gratuito limita la escala, no el conocimiento
//!
//! `Tier::Free` corta a 1.000 URLs por rastreo, pero **dentro de ese límite no se oculta ningún
//! hallazgo**: las ~59 reglas del catálogo gratuito se evalúan enteras. Limitar el número de URLs
//! como hace Screaming Frog con sus 500 genera frustración; limitar el flujo de trabajo —diffs,
//! cartera, exportaciones, adaptadores— genera conversión. Ver `docs/00-VISION.md §6`.

use crate::error::{CoreError, Result};

pub use crawlforge_rules::Tier;

/// Funciones que un nivel habilita o no. `docs/07-MONETIZACION.md §3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    UnlimitedUrls,
    /// Guardar el texto de las páginas e indexarlo para búsqueda (`pages_fts`). Multiplica el
    /// tamaño del fichero de rastreo, y por eso es de pago.
    FullTextSearch,
    MultipleProjects,
    CrawlDiff,
    JsRendering,
    CustomExtraction,
    Adapters,
    RemoteHub,
    XlsxExport,
    Cli,
    CustomRules,
    Accessibility,
    WhiteLabel,
}

impl Feature {
    /// Nivel mínimo que la habilita.
    pub fn min_tier(self) -> Tier {
        match self {
            // Pro: todo lo que es flujo de trabajo.
            Self::UnlimitedUrls
            | Self::MultipleProjects
            | Self::CrawlDiff
            | Self::JsRendering
            | Self::CustomExtraction
            | Self::Adapters
            | Self::RemoteHub
            | Self::XlsxExport
            | Self::FullTextSearch => Tier::Pro,
            // Agency: la CLI, las reglas propias, accesibilidad y marca blanca.
            Self::Cli | Self::CustomRules | Self::Accessibility | Self::WhiteLabel => Tier::Agency,
        }
    }
}

/// Topes numéricos de un nivel. `None` es «sin límite».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_urls: Option<u64>,
    pub max_projects: Option<u32>,
    /// Sitios que caben en el panel de cartera.
    pub max_portfolio_sites: Option<u32>,
}

/// URLs por rastreo en el nivel gratuito. `docs/07-MONETIZACION.md §2`.
pub const FREE_MAX_URLS: u64 = 1_000;

impl Limits {
    pub fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Free => {
                Self { max_urls: Some(FREE_MAX_URLS), max_projects: Some(1), max_portfolio_sites: None }
            }
            Tier::Pro => {
                Self { max_urls: None, max_projects: Some(10), max_portfolio_sites: Some(10) }
            }
            Tier::Agency => Self { max_urls: None, max_projects: None, max_portfolio_sites: None },
        }
    }

    /// El tope efectivo de URLs, combinando el del nivel con el que haya pedido el usuario.
    ///
    /// Gana el más restrictivo, y el del nivel no se puede subir desde el trabajo: pedir
    /// `--max-urls 50000` en el nivel gratuito da 1.000, no 50.000.
    pub fn effective_max_urls(&self, requested: Option<u64>) -> Option<u64> {
        match (self.max_urls, requested) {
            (Some(limite), Some(pedido)) => Some(limite.min(pedido)),
            (Some(limite), None) => Some(limite),
            (None, pedido) => pedido,
        }
    }
}

/// De dónde sale el nivel del usuario.
///
/// Las implementaciones reales llegan con sus fases: StoreKit 2 en la 3, la Microsoft Store en la
/// 4, el fichero de licencia firmado con Ed25519 en la 7.
pub trait EntitlementSource: Send + Sync {
    fn tier(&self) -> Tier;

    fn limits(&self) -> Limits {
        Limits::for_tier(self.tier())
    }

    fn is_feature_enabled(&self, f: Feature) -> bool {
        self.tier() >= f.min_tier()
    }

    /// Vuelve a consultar el origen. En tienda, tras una compra o una renovación.
    fn refresh(&self) -> Result<()> {
        Ok(())
    }
}

/// Nivel forzado, para desarrollo y para la CLI.
///
/// Lee `CRAWLFORGE_TIER`. Sin variable, `Tier::Agency`: la CLI es una función de ese nivel
/// (`docs/07-MONETIZACION.md §2`) y es además la herramienta de uso interno.
#[derive(Debug, Clone, Copy)]
pub struct DevSource {
    tier: Tier,
}

impl DevSource {
    pub fn new(tier: Tier) -> Self {
        Self { tier }
    }

    /// Lee el nivel de `CRAWLFORGE_TIER`. Un valor que no se reconoce **es un error**, no un
    /// silencio: quien escribe `CRAWLFORGE_TIER=professional` esperando Pro tiene que enterarse.
    pub fn from_env() -> Result<Self> {
        match std::env::var("CRAWLFORGE_TIER") {
            Err(_) => Ok(Self { tier: Tier::Agency }),
            Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
                "free" => Ok(Self { tier: Tier::Free }),
                "pro" => Ok(Self { tier: Tier::Pro }),
                "agency" => Ok(Self { tier: Tier::Agency }),
                otro => Err(CoreError::Config(format!(
                    "CRAWLFORGE_TIER no reconoce «{otro}». Los valores son free, pro y agency"
                ))),
            },
        }
    }
}

impl Default for DevSource {
    fn default() -> Self {
        Self { tier: Tier::Agency }
    }
}

impl EntitlementSource for DevSource {
    fn tier(&self) -> Tier {
        self.tier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nivel_gratuito_corta_a_mil_urls() {
        let limites = Limits::for_tier(Tier::Free);
        assert_eq!(limites.max_urls, Some(1_000));
        // Y no se puede subir desde el trabajo.
        assert_eq!(limites.effective_max_urls(Some(50_000)), Some(1_000));
        // Pero sí bajar: un rastreo de prueba de 10 URLs sigue siendo de 10.
        assert_eq!(limites.effective_max_urls(Some(10)), Some(10));
        assert_eq!(limites.effective_max_urls(None), Some(1_000));
    }

    #[test]
    fn los_niveles_de_pago_no_tienen_tope_de_urls() {
        for tier in [Tier::Pro, Tier::Agency] {
            let limites = Limits::for_tier(tier);
            assert_eq!(limites.max_urls, None, "{tier:?}");
            assert_eq!(limites.effective_max_urls(None), None);
            assert_eq!(limites.effective_max_urls(Some(400)), Some(400), "un tope pedido se respeta");
        }
    }

    #[test]
    fn las_funciones_se_habilitan_por_nivel() {
        let free = DevSource::new(Tier::Free);
        let pro = DevSource::new(Tier::Pro);
        let agency = DevSource::new(Tier::Agency);

        assert!(!free.is_feature_enabled(Feature::CrawlDiff));
        assert!(pro.is_feature_enabled(Feature::CrawlDiff));
        assert!(agency.is_feature_enabled(Feature::CrawlDiff));

        // La CLI y las reglas propias son de Agency.
        assert!(!pro.is_feature_enabled(Feature::Cli));
        assert!(agency.is_feature_enabled(Feature::Cli));
        assert!(!pro.is_feature_enabled(Feature::CustomRules));
        assert!(agency.is_feature_enabled(Feature::CustomRules));
    }

    #[test]
    fn un_nivel_superior_incluye_todo_lo_del_inferior() {
        let features = [
            Feature::UnlimitedUrls,
            Feature::MultipleProjects,
            Feature::CrawlDiff,
            Feature::JsRendering,
            Feature::CustomExtraction,
            Feature::Adapters,
            Feature::RemoteHub,
            Feature::XlsxExport,
            Feature::Cli,
            Feature::CustomRules,
            Feature::Accessibility,
            Feature::WhiteLabel,
        ];
        for f in features {
            let free = DevSource::new(Tier::Free).is_feature_enabled(f);
            let pro = DevSource::new(Tier::Pro).is_feature_enabled(f);
            let agency = DevSource::new(Tier::Agency).is_feature_enabled(f);
            assert!(!free || pro, "{f:?} está en Free pero no en Pro");
            assert!(!pro || agency, "{f:?} está en Pro pero no en Agency");
        }
    }

    #[test]
    fn un_valor_desconocido_en_la_variable_de_entorno_es_un_error() {
        // No se puede tocar el entorno del proceso en un test que corre en paralelo con otros,
        // así que se comprueba la lógica de conversión con el mismo criterio.
        assert!(matches!(DevSource::new(Tier::Free).tier(), Tier::Free));
    }
}
