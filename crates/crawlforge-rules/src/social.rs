//! `SOCIAL` — Open Graph cards. `docs/04-CATALOGO-REGLAS.md §9`.
//!
//! Both rules here are `free`-tier; the `SCHEMA-*` rules of the same section are paid.
//! `SOCIAL-OG-IMAGE-BROKEN` was planned as `pro` and shipped as `free` (2026-11): a card
//! that loses its image is the same kind of finding as a broken `<img>`, and those are free.
//!
//! Open Graph is not a ranking factor, which is why the severity of a missing tag is `low`.
//! What is lost without it is the click: a link shared on WhatsApp, LinkedIn or Slack with no
//! title, no description and no image is a naked URL nobody opens. For a client that lives off
//! distributing content across 100+ blogs, that is traffic left on the road.

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::Connection;

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

pub static SOCIAL_OG_IMAGE_BROKEN: RuleMeta = RuleMeta {
    id: "SOCIAL-OG-IMAGE-BROKEN",
    severity: Severity::Medium,
    category: Category::Social,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Imagen Open Graph que no carga",
    name_en: "Broken Open Graph image",
    desc_es: "La imagen declarada en og:image devuelve un error, directamente o al final de sus \
              redirecciones. La etiqueta está y ninguna herramienta que solo lea el HTML lo \
              nota, pero al compartir la página en WhatsApp, LinkedIn o Slack la tarjeta sale \
              sin imagen, o con un hueco. Suele ser una imagen borrada de la biblioteca de \
              medios o una ruta que no sobrevivió a una migración.",
    desc_en: "The image declared in og:image returns an error, directly or at the end of its \
              redirects. The tag is there and no tool that only reads the HTML notices, but when \
              the page is shared on WhatsApp, LinkedIn or Slack the card shows no image, or an \
              empty frame. It is usually an image deleted from the media library or a path that \
              did not survive a migration.",
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

/// The `og:image` returns an error: any 4xx/5xx of your own, 404/410 or a dead domain from
/// someone else's host, either directly or at the end of its redirect chain.
///
/// The parser records the first `og:image` of every page as a `links` row with
/// `element = 'meta'` and `rel = 'og:image'`, so the crawl requests it like an `<img>` when it
/// is internal and probes it when it is not. No fetcher of its own: this rule only reads what
/// that machinery left in `urls`.
///
/// **One finding per image URL, not per page**, with how many pages declare it and one of
/// them: a default card image missing from the theme is one file to restore, not four thousand
/// findings. Recorded on the URL as written in the tag, which is what someone has to change if
/// the file is not coming back.
///
/// **Redirects are followed, up to ten hops**, because that is what the networks' scrapers do:
/// an `og:image` that answers 301 to a 404 is as broken for the card as a direct 404, and a
/// migration that redirected the old uploads folder wholesale produces exactly that. A loop or
/// a chain that does not end within ten hops says nothing here; `HTTP-REDIRECT-LOOP` owns it.
///
/// **External images only assert on what the probe can vouch for** (see
/// [`crate::sql_external_gone`]), plus a domain that does not resolve, the same criterion as
/// `HTTP-404-EXTERNAL`. An image CDN that answers our bot `HEAD` with a 403 may well serve
/// Facebook's crawler, and calling that broken would be a guess.
///
/// When the same file is also an `<img>` on the page, `ASSET-IMG-BROKEN` reports it too. That
/// is on purpose: the two findings are different damage —a hole in the page, a card without an
/// image— and fixing the file closes both.
pub struct SocialOgImageBroken;

impl SiteRule for SocialOgImageBroken {
    fn meta(&self) -> &'static RuleMeta {
        &SOCIAL_OG_IMAGE_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        // `targets`: each distinct og:image once, entering `links` through `idx_links_element`.
        // `chain`: its redirect walk. `ends`: the last hop of each walk — SQLite returns the
        // bare column `cur_id` from the row that holds the `MAX`, which is the documented
        // behaviour this relies on.
        let sql = format!(
            "WITH RECURSIVE
               targets(id) AS (
                 SELECT DISTINCT to_url_id FROM links
                  WHERE element = 'meta' AND rel = 'og:image'
               ),
               chain(start_id, cur_id, hops) AS (
                 SELECT id, id, 0 FROM targets
                 UNION ALL
                 SELECT c.start_id, u.redirect_to, c.hops + 1
                   FROM chain c JOIN urls u ON u.id = c.cur_id
                  WHERE u.status_code >= 300 AND u.status_code < 400
                    AND u.redirect_to IS NOT NULL AND c.hops < {max_hops}
               ),
               ends(start_id, end_id, hops) AS (
                 SELECT start_id, cur_id, MAX(hops) FROM chain GROUP BY start_id
               )
             SELECT us.url_hash, us.url, ue.url, ue.status_code, ue.error_kind, e.hops,
                    (SELECT COUNT(DISTINCT l.from_url_id) FROM links l
                      WHERE l.to_url_id = us.id AND l.element = 'meta' AND l.rel = 'og:image'),
                    (SELECT MIN(uf.url) FROM links l JOIN urls uf ON uf.id = l.from_url_id
                      WHERE l.to_url_id = us.id AND l.element = 'meta' AND l.rel = 'og:image')
               FROM ends e
               JOIN urls us ON us.id = e.start_id
               JOIN urls ue ON ue.id = e.end_id
              WHERE (ue.is_internal = 1 AND ue.status_code >= 400)
                 OR (ue.is_internal = 0
                     AND ({externa_rota}
                          OR (ue.status_code IS NULL AND ue.error_kind = 'dns')))
              ORDER BY us.url",
            max_hops = MAX_OG_IMAGE_HOPS,
            externa_rota = crate::sql_external_gone("ue.status_code"),
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, url_final, status, error_kind, hops, paginas, ejemplo) = row?;
            let mut detalle = serde_json::json!({
                "url": url,
                "used_by_pages": paginas,
                "example_page": ejemplo,
            });
            if let Some(obj) = detalle.as_object_mut() {
                match status {
                    Some(status) => obj.insert("status_code".into(), status.into()),
                    // The dns branch: no status ever arrived, so the detail says why instead of
                    // faking a code.
                    None => obj.insert("reason".into(), error_kind.into()),
                };
                if hops > 0 {
                    obj.insert("final_url".into(), url_final.into());
                    obj.insert("hops".into(), hops.into());
                }
            }
            out.push((Some(hash), Issue::new(&SOCIAL_OG_IMAGE_BROKEN).with_detail(detalle)));
        }
        Ok(out)
    }
}

/// How far a redirect chain is followed before giving up. The same safety net as
/// `v_broken_links` (migration 009): it is what stops a loop, not a criterion.
const MAX_OG_IMAGE_HOPS: i64 = 10;

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![Box::new(SocialOgMissing)]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    vec![Box::new(SocialOgImageBroken)]
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

    // ------------------------------------------------------------ SOCIAL-OG-IMAGE-BROKEN

    /// The complete published schema, so a misspelled column or a lost index shows up here.
    fn conn() -> Connection {
        crate::test_schema::full_schema()
    }

    /// Inserts a URL row. The `url_hash` is the id times 1,000, so a test can tell which row a
    /// finding attaches to.
    fn url(conn: &Connection, id: i64, url: &str, status: Option<i64>, interna: bool) {
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, crawl_state,
                               status_code)
             VALUES (?1, ?2, ?3, 'https', 'ejemplo.es', '/', ?4, 'done', ?5)",
            rusqlite::params![id, url, id * 1000, interna, status],
        )
        .expect("insert url");
    }

    fn redirige(conn: &Connection, desde: i64, hacia: i64) {
        conn.execute("UPDATE urls SET redirect_to = ?2 WHERE id = ?1", [desde, hacia])
            .expect("set redirect");
    }

    /// A `links` row exactly as the engine writes the first `og:image` of a page.
    fn og_image(conn: &Connection, pagina: i64, imagen: i64) {
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, rel, element)
             VALUES (?1, ?2, 'og:image', 'meta')",
            [pagina, imagen],
        )
        .expect("insert og:image link");
    }

    fn detalle(issue: &Issue) -> String {
        issue.detail_json.clone().unwrap_or_default()
    }

    #[test]
    fn og_image_broken_reports_a_missing_image_once_with_its_pages() {
        let c = conn();
        url(&c, 1, "https://ejemplo.es/a", Some(200), true);
        url(&c, 2, "https://ejemplo.es/b", Some(200), true);
        url(&c, 3, "https://ejemplo.es/og.png", Some(404), true);
        og_image(&c, 1, 3);
        og_image(&c, 2, 3);

        let hallazgos = SocialOgImageBroken.evaluate(&c).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "one finding per image, not per page");
        assert_eq!(hallazgos[0].0, Some(3_000), "recorded on the image URL");
        assert_eq!(hallazgos[0].1.severity, Severity::Medium);
        let d = detalle(&hallazgos[0].1);
        assert!(d.contains("\"used_by_pages\":2"), "{d}");
        assert!(d.contains("\"status_code\":404"), "{d}");
        assert!(d.contains("\"example_page\":\"https://ejemplo.es/a\""), "{d}");
    }

    #[test]
    fn og_image_broken_stays_quiet_on_an_image_that_loads() {
        let c = conn();
        url(&c, 1, "https://ejemplo.es/a", Some(200), true);
        url(&c, 2, "https://ejemplo.es/og.png", Some(200), true);
        og_image(&c, 1, 2);
        assert!(SocialOgImageBroken.evaluate(&c).expect("evaluate").is_empty());
    }

    #[test]
    fn og_image_broken_follows_redirects_to_the_error() {
        // The migration that redirected the old uploads folder wholesale: the og:image answers
        // 301, and what is at the end is a 404. The networks' scrapers follow it and get nothing.
        let c = conn();
        url(&c, 1, "https://ejemplo.es/a", Some(200), true);
        url(&c, 2, "https://ejemplo.es/old/og.png", Some(301), true);
        url(&c, 3, "https://ejemplo.es/new/og.png", Some(301), true);
        url(&c, 4, "https://ejemplo.es/gone.png", Some(404), true);
        redirige(&c, 2, 3);
        redirige(&c, 3, 4);
        og_image(&c, 1, 2);

        let hallazgos = SocialOgImageBroken.evaluate(&c).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].0, Some(2_000), "recorded on the URL written in the tag");
        let d = detalle(&hallazgos[0].1);
        assert!(d.contains("\"final_url\":\"https://ejemplo.es/gone.png\""), "{d}");
        assert!(d.contains("\"hops\":2"), "{d}");
    }

    #[test]
    fn og_image_broken_stays_quiet_on_a_redirect_that_ends_well() {
        let c = conn();
        url(&c, 1, "https://ejemplo.es/a", Some(200), true);
        url(&c, 2, "http://ejemplo.es/og.png", Some(301), true);
        url(&c, 3, "https://ejemplo.es/og.png", Some(200), true);
        redirige(&c, 2, 3);
        og_image(&c, 1, 2);
        assert!(SocialOgImageBroken.evaluate(&c).expect("evaluate").is_empty());
    }

    #[test]
    fn og_image_broken_stays_quiet_on_a_redirect_loop() {
        // A loop never ends anywhere: that is HTTP-REDIRECT-LOOP's finding, not this one, and
        // the walk must stop instead of hanging.
        let c = conn();
        url(&c, 1, "https://ejemplo.es/a", Some(200), true);
        url(&c, 2, "https://ejemplo.es/x.png", Some(301), true);
        url(&c, 3, "https://ejemplo.es/y.png", Some(301), true);
        redirige(&c, 2, 3);
        redirige(&c, 3, 2);
        og_image(&c, 1, 2);
        assert!(SocialOgImageBroken.evaluate(&c).expect("evaluate").is_empty());
    }

    #[test]
    fn og_image_broken_only_trusts_gone_codes_from_a_foreign_host() {
        // A CDN that answers the bot probe with 403 may serve Facebook's crawler fine: only
        // 404/410 and a dead domain assert, the same criterion as HTTP-404-EXTERNAL.
        let c = conn();
        url(&c, 1, "https://ejemplo.es/a", Some(200), true);
        url(&c, 2, "https://cdn.otro.example/muro.png", Some(403), false);
        url(&c, 3, "https://cdn.otro.example/borrada.png", Some(410), false);
        url(&c, 4, "https://cerrado.example/og.png", None, false);
        c.execute("UPDATE urls SET error_kind = 'dns' WHERE id = 4", []).expect("dns");
        for imagen in [2, 3, 4] {
            og_image(&c, 1, imagen);
        }

        let hallazgos = SocialOgImageBroken.evaluate(&c).expect("evaluate");
        let urls: Vec<Option<i64>> = hallazgos.iter().map(|(h, _)| *h).collect();
        assert_eq!(urls, vec![Some(3_000), Some(4_000)], "the 403 wall is not broken");
        assert!(detalle(&hallazgos[1].1).contains("\"reason\":\"dns\""));
    }

    #[test]
    fn og_image_broken_ignores_other_elements_pointing_at_a_broken_url() {
        // Guard: a broken `<img>` or `<a>` is ASSET/HTTP territory. Only the meta row counts.
        let c = conn();
        url(&c, 1, "https://ejemplo.es/a", Some(200), true);
        url(&c, 2, "https://ejemplo.es/rota.png", Some(404), true);
        c.execute(
            "INSERT INTO links (from_url_id, to_url_id, element) VALUES (1, 2, 'img'), (1, 2, 'a')",
            [],
        )
        .expect("insert links");
        assert!(SocialOgImageBroken.evaluate(&c).expect("evaluate").is_empty());
    }

    #[test]
    fn og_image_broken_enters_links_through_the_element_index() {
        // `links` is the largest table of a crawl (6 M rows in the dense benchmark): reading it
        // whole to find one row per page is what migration 010 fixed for the embed rules.
        let c = conn();
        let mut stmt = c
            .prepare(
                "EXPLAIN QUERY PLAN SELECT DISTINCT to_url_id FROM links
                  WHERE element = 'meta' AND rel = 'og:image'",
            )
            .expect("prepare the plan");
        let plan: String = stmt
            .query_map([], |r| r.get::<_, String>(3))
            .expect("read the plan")
            .filter_map(Result::ok)
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(plan.contains("idx_links_element"), "the plan says: {plan}");
    }
}
