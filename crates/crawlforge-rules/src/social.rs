//! `SOCIAL` — tarjetas de Open Graph. `docs/04-CATALOGO-REGLAS.md §9`.
//!
//! De la sección solo es de nivel `free` esta regla: las `SCHEMA-*` y `SOCIAL-OG-IMAGE-BROKEN`
//! son de pago.
//!
//! Open Graph no es un factor de posicionamiento, y por eso la severidad es `low`. Lo que se
//! pierde sin él es el clic: un enlace compartido en WhatsApp, LinkedIn o Slack sin título, sin
//! descripción y sin imagen es una URL desnuda que nadie abre. En un cliente que vive de
//! distribuir contenido en 100+ blogs, eso es tráfico que se queda por el camino.

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};

/// Las tres propiedades sin las cuales una tarjeta de enlace no se construye entera.
///
/// `og:url` y `og:type` quedan fuera a propósito: las redes las infieren de la URL compartida y
/// del contenido, así que exigirlas sería ruido.
const REQUIRED_OG: [&str; 3] = ["og:title", "og:description", "og:image"];

pub static SOCIAL_OG_MISSING: RuleMeta = RuleMeta {
    id: "SOCIAL-OG-MISSING",
    severity: Severity::Low,
    category: Category::Social,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Open Graph incompleto",
    name_en: "Incomplete Open Graph",
    desc_es: "Falta og:title, og:description u og:image. Cuando alguien comparte la página en \
              WhatsApp, LinkedIn o Slack, la red no puede montar la tarjeta y el enlace aparece \
              como una URL desnuda: no afecta al posicionamiento, pero se pierde el clic.",
    desc_en: "og:title, og:description or og:image is missing. When someone shares the page on \
              WhatsApp, LinkedIn or Slack the network cannot build the preview card and the link \
              shows up as a bare URL: it does not affect ranking, but the click is lost.",
    references: &[],
};

/// Página indexable a la que le falta alguna de las tres propiedades Open Graph básicas.
///
/// Un solo hallazgo por página, con la lista de las que faltan en el detalle: tres hallazgos
/// separados para la misma etiqueta `<head>` incompleta serían tres veces el mismo trabajo para
/// el usuario. Se agrupa por el conjunto que falta, que en un sitio con plantilla es siempre el
/// mismo y así la UI puede decir «a 12.000 páginas les falta og:image».
pub struct SocialOgMissing;

impl PageRule for SocialOgMissing {
    fn meta(&self) -> &'static RuleMeta {
        &SOCIAL_OG_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }

        // Las claves llegan ya en minúsculas del parser, pero la comparación es insensible por
        // si algún día el atributo `property` se leyera tal cual: `og:Image` es la misma
        // propiedad.
        let faltan: Vec<&str> = REQUIRED_OG
            .iter()
            .filter(|requerida| {
                !ctx.og_keys.iter().any(|presente| presente.trim().eq_ignore_ascii_case(requerida))
            })
            .copied()
            .collect();

        if faltan.is_empty() {
            return Vec::new();
        }

        vec![Issue::new(&SOCIAL_OG_MISSING)
            .with_detail(serde_json::json!({
                "missing": faltan,
                "present": ctx.og_keys,
            }))
            .with_group(format!("og-missing:{}", faltan.join(",")))]
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![Box::new(SocialOgMissing)]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una página sana. Cada test le quita de la tarjeta lo que le interesa.
    fn ctx<'a>(og: &'a [&'a str]) -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.og_keys = og;
        c
    }

    const COMPLETO: &[&str] = &["og:title", "og:description", "og:image"];

    #[test]
    fn no_avisa_con_la_tarjeta_completa() {
        assert!(SocialOgMissing.evaluate(&ctx(COMPLETO)).is_empty());
    }

    #[test]
    fn no_avisa_por_las_propiedades_de_adorno() {
        // `og:url`, `og:type` y `og:site_name` no se exigen.
        let c = ctx(&["og:title", "og:description", "og:image", "og:url", "og:type"]);
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_cuando_no_hay_ninguna_etiqueta() {
        let issues = SocialOgMissing.evaluate(&ctx(&[]));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "SOCIAL-OG-MISSING");
        assert_eq!(issues[0].severity, Severity::Low, "sin Open Graph no es un fallo grave");
        assert_eq!(
            issues[0].group_key.as_deref(),
            Some("og-missing:og:title,og:description,og:image")
        );
    }

    #[test]
    fn un_solo_hallazgo_aunque_falten_varias() {
        // Tres avisos por el mismo `<head>` incompleto serían tres veces el mismo trabajo.
        let issues = SocialOgMissing.evaluate(&ctx(&["og:title"]));
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("og:description"), "{detalle}");
        assert!(detalle.contains("og:image"), "{detalle}");
        assert!(!detalle.contains("\"missing\":[\"og:title\""), "{detalle}");
    }

    #[test]
    fn avisa_cuando_solo_falta_la_imagen() {
        // El caso más común: el tema pinta título y descripción y se olvida de la imagen.
        let issues = SocialOgMissing.evaluate(&ctx(&["og:title", "og:description"]));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].group_key.as_deref(), Some("og-missing:og:image"));
    }

    #[test]
    fn la_comparacion_no_distingue_mayusculas() {
        let c = ctx(&["OG:Title", "og:DESCRIPTION", "og:image"]);
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn no_avisa_en_una_pagina_no_indexable() {
        // La tarjeta de una página con `noindex` no la va a ver nadie desde un buscador, y si se
        // comparte a mano no es un problema de auditoría SEO.
        let mut c = ctx(&[]);
        c.is_indexable = false;
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn no_avisa_sobre_algo_que_no_es_html() {
        let mut c = ctx(&[]);
        c.is_html = false;
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }
}
