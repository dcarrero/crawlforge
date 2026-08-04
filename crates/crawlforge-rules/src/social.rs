//! `SOCIAL` — Open Graph cards. `docs/04-CATALOGO-REGLAS.md §9`.
//!
//! Only this rule from the section is `free`-tier: the `SCHEMA-*` rules and
//! `SOCIAL-OG-IMAGE-BROKEN` are paid.
//!
//! Open Graph is not a ranking factor, which is why the severity is `low`. What is lost
//! without it is the click: a link shared on WhatsApp, LinkedIn or Slack with no title, no
//! description and no image is a naked URL nobody opens. For a client that lives off
//! distributing content across 100+ blogs, that is traffic left on the road.

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};

/// The three properties without which a link card cannot be built whole.
///
/// `og:url` and `og:type` are left out on purpose: the networks infer them from the shared
/// URL and from the content, so demanding them would be noise.
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

/// Indexable page missing any of the three basic Open Graph properties.
///
/// One finding per page, with the list of missing ones in the detail: three separate findings
/// for the same incomplete `<head>` tag would be three times the same work for the user. It
/// groups by the missing set, which on a templated site is always the same one, so the UI can
/// say "12,000 pages are missing og:image".
pub struct SocialOgMissing;

impl PageRule for SocialOgMissing {
    fn meta(&self) -> &'static RuleMeta {
        &SOCIAL_OG_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }

        // The keys arrive already lowercased from the parser, but the comparison is
        // case-insensitive in case the `property` attribute were one day read verbatim:
        // `og:Image` is the same property.
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

    /// A healthy page. Each test removes from the card what it cares about.
    fn ctx<'a>(og: &'a [&'a str]) -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.og_keys = og;
        c
    }

    const COMPLETO: &[&str] = &["og:title", "og:description", "og:image"];

    #[test]
    fn does_not_flag_a_complete_card() {
        assert!(SocialOgMissing.evaluate(&ctx(COMPLETO)).is_empty());
    }

    #[test]
    fn does_not_flag_missing_decorative_properties() {
        // `og:url`, `og:type` and `og:site_name` are not required.
        let c = ctx(&["og:title", "og:description", "og:image", "og:url", "og:type"]);
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn flags_a_page_with_no_og_tags_at_all() {
        let issues = SocialOgMissing.evaluate(&ctx(&[]));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "SOCIAL-OG-MISSING");
        assert_eq!(issues[0].severity, Severity::Low, "missing Open Graph is not a serious failure");
        assert_eq!(
            issues[0].group_key.as_deref(),
            Some("og-missing:og:title,og:description,og:image")
        );
    }

    #[test]
    fn a_single_finding_even_when_several_are_missing() {
        // Three warnings for the same incomplete `<head>` would be three times the same work.
        let issues = SocialOgMissing.evaluate(&ctx(&["og:title"]));
        assert_eq!(issues.len(), 1);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("og:description"), "{detalle}");
        assert!(detalle.contains("og:image"), "{detalle}");
        assert!(!detalle.contains("\"missing\":[\"og:title\""), "{detalle}");
    }

    #[test]
    fn flags_when_only_the_image_is_missing() {
        // The most common case: the theme renders title and description and forgets the
        // image.
        let issues = SocialOgMissing.evaluate(&ctx(&["og:title", "og:description"]));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].group_key.as_deref(), Some("og-missing:og:image"));
    }

    #[test]
    fn the_comparison_is_case_insensitive() {
        let c = ctx(&["OG:Title", "og:DESCRIPTION", "og:image"]);
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn does_not_flag_a_non_indexable_page() {
        // Nobody will see the card of a `noindex` page coming from a search engine, and if
        // it is shared by hand it is not an SEO-audit problem.
        let mut c = ctx(&[]);
        c.is_indexable = false;
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn does_not_flag_something_that_is_not_html() {
        let mut c = ctx(&[]);
        c.is_html = false;
        assert!(SocialOgMissing.evaluate(&c).is_empty());
    }
}
