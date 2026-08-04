//! `CONTENT` — headings and content. `docs/04-CATALOGO-REGLAS.md §6`.

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};

pub static CONTENT_H1_MISSING: RuleMeta = RuleMeta {
    id: "CONTENT-H1-MISSING",
    severity: Severity::High,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin H1",
    name_en: "Missing H1",
    desc_es: "La página indexable no tiene H1, o lo tiene vacío. El H1 le dice al buscador de qué \
              trata la página con las palabras del autor, y es el primer encabezado que anuncia \
              un lector de pantalla al entrar en el contenido.",
    desc_en: "The indexable page has no H1, or an empty one. The H1 tells the search engine what \
              the page is about in the author's own words, and it is the first heading a screen \
              reader announces when entering the content.",
    references: &[],
};

pub static CONTENT_H1_MULTIPLE: RuleMeta = RuleMeta {
    id: "CONTENT-H1-MULTIPLE",
    severity: Severity::Low,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Varios H1",
    name_en: "Multiple H1",
    desc_es: "La página tiene más de un H1. HTML5 lo permite, pero deja de haber un tema \
              principal: el buscador tiene que adivinar de qué trata la página y el lector de \
              pantalla anuncia varios encabezados de nivel uno como si fueran documentos \
              distintos. Casi siempre es la plantilla marcando el logotipo o el nombre del sitio \
              como H1 además del titular.",
    desc_en: "The page has more than one H1. HTML5 allows it, but there is no longer a single \
              main topic: the search engine has to guess what the page is about, and a screen \
              reader announces several level-one headings as if they were separate documents. It \
              is usually the template marking the logo or the site name as an H1 on top of the \
              real headline.",
    references: &[],
};

pub static CONTENT_H1_EMPTY: RuleMeta = RuleMeta {
    id: "CONTENT-H1-EMPTY",
    severity: Severity::Medium,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "H1 sin texto",
    name_en: "Empty H1",
    desc_es: "Hay un H1 en el marcado, pero no aporta texto: está vacío o su único contenido es \
              una imagen que no dice nada. El encabezado ocupa el sitio del titular sin \
              cumplir su función, así que ni el buscador ni un lector de pantalla obtienen el \
              tema de la página. El caso típico es un logotipo dentro del H1.",
    desc_en: "There is an H1 in the markup, but it contributes no text: it is empty, or its only \
              content is an image that says nothing. The heading takes the headline's place \
              without doing its job, so neither the search engine nor a screen reader gets the \
              topic of the page. The usual case is a logo inside the H1.",
    references: &[],
};

pub static CONTENT_HEADING_SKIP: RuleMeta = RuleMeta {
    id: "CONTENT-HEADING-SKIP",
    severity: Severity::Low,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Salto de nivel de encabezado",
    name_en: "Heading level skipped",
    desc_es: "Los encabezados bajan más de un nivel de golpe, por ejemplo de H2 a H4. El esquema \
              del documento queda con huecos: quien navega por encabezados no sabe si el H4 es \
              un apartado del H2 o de otra sección, y el buscador pierde la jerarquía con la que \
              entiende qué depende de qué. Suele venir de elegir el encabezado por su tamaño de \
              letra en vez de por su nivel.",
    desc_en: "Heading levels drop by more than one step at once, for example from H2 to H4. The \
              document outline ends up with holes: someone navigating by headings cannot tell \
              whether the H4 belongs to that H2 or to another section, and the search engine \
              loses the hierarchy it uses to understand what belongs to what. It usually comes \
              from picking a heading by its font size instead of its level.",
    references: &[],
};

pub static CONTENT_THIN: RuleMeta = RuleMeta {
    id: "CONTENT-THIN",
    severity: Severity::High,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Contenido escaso",
    name_en: "Thin content",
    desc_es: "Una página indexable con menos de 300 palabras de texto visible. Rara vez tiene \
              suficiente materia para responder a una consulta, así que compite mal y, en \
              cantidad, diluye la calidad media del sitio a ojos del buscador. Muchas son \
              archivos, etiquetas o fichas autogeneradas que convendría no indexar en vez de \
              ampliar.",
    desc_en: "An indexable page with fewer than 300 words of visible text. It rarely has enough \
              substance to answer a query, so it competes poorly and, in bulk, dilutes the \
              site's average quality in the search engine's eyes. Many of these are archives, \
              tag pages or auto-generated stubs that are better left unindexed than expanded.",
    references: &[],
};

pub static CONTENT_LANG_MISSING: RuleMeta = RuleMeta {
    id: "CONTENT-LANG-MISSING",
    severity: Severity::Medium,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin atributo lang",
    name_en: "Missing lang attribute",
    desc_es: "El elemento <html> no declara el idioma del contenido. Sin él, el lector de \
              pantalla pronuncia el texto con las reglas del idioma del sistema, el navegador no \
              sabe qué diccionario usar para traducir y las señales de idioma del sitio quedan \
              en manos de la detección automática. Es un atributo de una línea.",
    desc_en: "The <html> element does not declare the language of the content. Without it, a \
              screen reader pronounces the text using the system language's rules, the browser \
              does not know which dictionary to use when translating, and the site's language \
              signals are left to automatic detection. It is a one-line attribute.",
    references: &[],
};

/// Word count below which an indexable page is considered thin.
///
/// 300 is the catalog threshold (`§6`) and the one the rest of the industry uses. It is not
/// lowered to silence the other rules' short fixtures: the warning is correct there too.
const THIN_MIN_WORDS: u32 = 300;

/// Indexable page without an `<h1>`.
pub struct ContentH1Missing;

impl PageRule for ContentH1Missing {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_H1_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        if ctx.h1_count > 0 && ctx.h1.map(|h| !h.trim().is_empty()).unwrap_or(false) {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_H1_MISSING)
            .with_detail(serde_json::json!({ "h1_count": ctx.h1_count }))]
    }
}

/// More than one `<h1>` on the same page.
pub struct ContentH1Multiple;

impl PageRule for ContentH1Multiple {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_H1_MULTIPLE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        if ctx.h1_count <= 1 {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_H1_MULTIPLE)
            .with_detail(serde_json::json!({ "h1_count": ctx.h1_count }))]
    }
}

/// `<h1>` present but contributing no text: empty, whitespace-only, or with only an image.
///
/// Note the boundary with [`ContentH1Missing`]: that rule talks about the page that **has no**
/// headline, and it also warns here because the effect is the same. This one adds the datum
/// that changes the fix: the H1 already exists in the template, so there is no heading to add,
/// only text to give it.
///
/// **Known limit.** The catalog says "H1 empty or with only an alt-less image". The
/// [`PageContext`] does not say which images sit inside the H1 —`images` is the whole page's
/// list— so the rule cannot tell the H1 whose only content is an image **with** `alt`
/// (acceptable: the `alt` is the headline) from the one whose image has **no** `alt`. It warns
/// in both cases, which is the conservative side: `ASSET-IMG-NO-ALT` covers the image's half.
/// Separating them would need a new field in the context, and adding it is not this module's
/// job.
pub struct ContentH1Empty;

impl PageRule for ContentH1Empty {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_H1_EMPTY
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        // With no H1 at all there is nothing empty to point at: that is `CONTENT-H1-MISSING`.
        if ctx.h1_count == 0 {
            return Vec::new();
        }
        let con_texto = ctx.h1.map(|h| !h.trim().is_empty()).unwrap_or(false);
        if con_texto {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_H1_EMPTY)
            .with_detail(serde_json::json!({ "h1_count": ctx.h1_count }))]
    }
}

/// Level skip between two consecutive headings: an H2 followed by an H4.
///
/// It only looks at consecutive pairs. The document's first heading being an H3 is not counted
/// as a skip from an implicit level one: `CONTENT-H1-MISSING` already warns about that, and
/// counting it twice would only add noise to the report.
pub struct ContentHeadingSkip;

impl PageRule for ContentHeadingSkip {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_HEADING_SKIP
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        // Going back up is free: from an H4 you can return to an H2 when opening another
        // section. What breaks the outline is going deeper by more than one step at once.
        let saltos: Vec<(usize, u8, u8)> = ctx
            .heading_levels
            .windows(2)
            .enumerate()
            .filter(|(_, par)| par[1] > par[0].saturating_add(1))
            .map(|(i, par)| (i + 1, par[0], par[1]))
            .collect();

        let Some(&(indice, desde, hasta)) = saltos.first() else {
            return Vec::new();
        };

        // The text of the heading that lands wrong is the whole diagnosis: in a real crawl,
        // 16,764 rows said `{"from":1,"to":4}` and the HTML had to be opened to discover that
        // the culprit was the `<h4>` of the author's signature. Tests may bring no texts
        // (`heading_texts` empty); then the field is omitted rather than invented.
        let texto = ctx.heading_texts.get(indice).map(|t| t.trim()).filter(|t| !t.is_empty());

        let mut detalle = serde_json::json!({
            "from": desde,
            "to": hasta,
            "index": indice,
            "skips": saltos.len(),
        });
        if let (Some(texto), Some(obj)) = (texto, detalle.as_object_mut()) {
            obj.insert("text".into(), serde_json::json!(truncate_chars(texto, 120)));
        }

        // One finding per page, with the first skip as the sample: it is the one to look at to
        // understand the pattern, and the count says whether it is an oversight or the whole
        // template.
        //
        // The `group_key` identifies **the cause, not the page**: the shape of the skip plus
        // the text of the offending heading. All the `H1→H4` author signatures with the same
        // text are one template defect; two `H1→H4` with different texts are two defects.
        // Without text the cause cannot be claimed to be the same, so the key leaves it empty
        // and those findings only group among themselves.
        vec![Issue::new(&CONTENT_HEADING_SKIP)
            .with_detail(detalle)
            .with_group(format!(
                "heading-skip:{desde}>{hasta}:{}",
                normalize_group_text(texto.unwrap_or(""))
            ))]
    }
}

/// The first `max` characters —not bytes: cutting mid-codepoint in accented text would break
/// the string— of a text, for the `detail_json`.
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Normalizes a heading's text for use in a `group_key`: lowercase, whitespace collapsed and
/// 80 characters at most. "CONTACTO" and "Contacto " are the same cause.
fn normalize_group_text(s: &str) -> String {
    let colapsado = s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    truncate_chars(&colapsado, 80)
}

/// Indexable page with fewer than [`THIN_MIN_WORDS`] words of visible text.
pub struct ContentThin;

impl PageRule for ContentThin {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_THIN
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        if ctx.word_count >= THIN_MIN_WORDS {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_THIN).with_detail(
            serde_json::json!({ "word_count": ctx.word_count, "threshold": THIN_MIN_WORDS }),
        )]
    }
}

/// `<html>` without a `lang` attribute.
pub struct ContentLangMissing;

impl PageRule for ContentLangMissing {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_LANG_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        // `lang=""` is as useless as not setting it, and it is what a template that never got
        // handed the language leaves behind.
        if ctx.lang.map(|l| !l.trim().is_empty()).unwrap_or(false) {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_LANG_MISSING)]
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![
        Box::new(ContentH1Missing),
        Box::new(ContentH1Multiple),
        Box::new(ContentH1Empty),
        Box::new(ContentHeadingSkip),
        Box::new(ContentThin),
        Box::new(ContentLangMissing),
    ]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageView;

    /// A healthy page to start from: a single H1 with text, a heading outline with no holes, a
    /// declared language and the 500 words from `indexable_html`, above the `CONTENT-THIN`
    /// threshold. Each test breaks only what it cares about.
    fn ctx<'a>() -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.h1 = Some("Un encabezado");
        c.h1_count = 1;
        c.heading_levels = &[1, 2];
        c.lang = Some("es");
        c
    }

    #[test]
    fn does_not_warn_when_there_is_an_h1() {
        assert!(ContentH1Missing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn warns_when_there_is_no_h1() {
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        let issues = ContentH1Missing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-H1-MISSING");
    }

    #[test]
    fn an_empty_h1_counts_as_missing() {
        let mut c = ctx();
        c.h1 = Some("  ");
        assert_eq!(ContentH1Missing.evaluate(&c).len(), 1);
    }

    #[test]
    fn does_not_warn_on_a_non_indexable_page() {
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        c.is_indexable = false;
        assert!(ContentH1Missing.evaluate(&c).is_empty());
    }

    // --- CONTENT-H1-MULTIPLE ---

    #[test]
    fn a_single_h1_is_not_multiple() {
        assert!(ContentH1Multiple.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_page_with_no_h1_is_not_multiple() {
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        assert!(ContentH1Multiple.evaluate(&c).is_empty());
    }

    #[test]
    fn warns_with_two_h1s() {
        let mut c = ctx();
        c.h1_count = 2;
        c.heading_levels = &[1, 1, 2];
        let issues = ContentH1Multiple.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-H1-MULTIPLE");
        assert_eq!(issues[0].severity, Severity::Low);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"h1_count\":2"), "{detalle}");
    }

    #[test]
    fn multiple_h1s_on_a_non_indexable_page_do_not_warn() {
        let mut c = ctx();
        c.h1_count = 3;
        c.is_indexable = false;
        assert!(ContentH1Multiple.evaluate(&c).is_empty());
    }

    #[test]
    fn multiple_h1s_on_something_that_is_not_html_do_not_warn() {
        let mut c = ctx();
        c.h1_count = 3;
        c.is_html = false;
        assert!(ContentH1Multiple.evaluate(&c).is_empty());
    }

    // --- CONTENT-H1-EMPTY ---

    #[test]
    fn does_not_warn_when_the_h1_has_text() {
        assert!(ContentH1Empty.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn warns_when_the_h1_is_empty() {
        let mut c = ctx();
        c.h1 = Some("");
        let issues = ContentH1Empty.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-H1-EMPTY");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn a_whitespace_only_h1_is_empty() {
        let mut c = ctx();
        c.h1 = Some(" \n\t ");
        assert_eq!(ContentH1Empty.evaluate(&c).len(), 1);
    }

    #[test]
    fn an_h1_with_only_an_image_and_no_alt_is_empty() {
        // The engine only puts the heading's text nodes into `h1`, so an H1 whose only child
        // is an `<img>` arrives here as the empty string. It is the case that actually
        // matters: the page looks like it has a headline and it does not.
        let mut c = ctx();
        c.h1 = Some("");
        let imagenes =
            [ImageView { src: "/logo.svg", alt: None, ..Default::default() }];
        c.images = &imagenes;
        assert_eq!(ContentH1Empty.evaluate(&c).len(), 1);
    }

    #[test]
    fn with_no_h1_at_all_it_does_not_warn_about_an_empty_h1() {
        // That is `CONTENT-H1-MISSING` territory. The two rules do not say the same thing.
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        assert!(ContentH1Empty.evaluate(&c).is_empty());
    }

    #[test]
    fn an_empty_h1_on_a_non_indexable_page_does_not_warn() {
        let mut c = ctx();
        c.h1 = Some("");
        c.is_indexable = false;
        assert!(ContentH1Empty.evaluate(&c).is_empty());
    }

    #[test]
    fn an_empty_h1_on_something_that_is_not_html_does_not_warn() {
        let mut c = ctx();
        c.h1 = Some("");
        c.is_html = false;
        assert!(ContentH1Empty.evaluate(&c).is_empty());
    }

    // --- CONTENT-HEADING-SKIP ---

    #[test]
    fn a_consecutive_outline_has_no_skips() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 3, 3, 4];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    #[test]
    fn going_back_up_a_level_is_not_a_skip() {
        // H1 → H2 → H3 → H2: the last drop opens another section, it breaks nothing.
        let mut c = ctx();
        c.heading_levels = &[1, 2, 3, 2];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    #[test]
    fn warns_from_h2_to_h4() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 4];
        let issues = ContentHeadingSkip.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-HEADING-SKIP");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"from\":2"), "{detalle}");
        assert!(detalle.contains("\"to\":4"), "{detalle}");
        assert!(detalle.contains("\"index\":2"), "{detalle}");
        assert!(detalle.contains("\"skips\":1"), "{detalle}");
    }

    #[test]
    fn an_h1_followed_by_an_h3_is_also_a_skip() {
        let mut c = ctx();
        c.heading_levels = &[1, 3];
        assert_eq!(ContentHeadingSkip.evaluate(&c).len(), 1);
    }

    #[test]
    fn several_skips_produce_one_finding_and_are_counted() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 4, 2, 5];
        let issues = ContentHeadingSkip.evaluate(&c);
        assert_eq!(issues.len(), 1, "one finding per page, not one per jump");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"skips\":2"), "{detalle}");
    }

    #[test]
    fn the_detail_includes_the_text_of_the_offending_heading() {
        // It was the text that made it possible to diagnose the agency footer's `<h5>CONTACTO`
        // by hand; without it you have to go look at the HTML of every page.
        let mut c = ctx();
        c.heading_levels = &[1, 4];
        c.heading_texts = &["El título", "Firma del autor"];
        let issues = ContentHeadingSkip.evaluate(&c);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"text\":\"Firma del autor\""), "{detalle}");
    }

    #[test]
    fn without_texts_the_detail_does_not_invent_a_field() {
        let mut c = ctx();
        c.heading_levels = &[1, 4];
        let issues = ContentHeadingSkip.evaluate(&c);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(!detalle.contains("\"text\""), "{detalle}");
        // The key exists anyway, with an empty text: what is unknown groups only with itself.
        assert_eq!(issues[0].group_key.as_deref(), Some("heading-skip:1>4:"));
    }

    #[test]
    fn the_group_key_is_the_skip_shape_plus_the_offending_text() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 5];
        c.heading_texts = &["Título", "Sección", "  CONTACTO "];
        let issues = ContentHeadingSkip.evaluate(&c);
        // Lowercase and collapsed whitespace: "CONTACTO" and "contacto " are the same cause.
        assert_eq!(issues[0].group_key.as_deref(), Some("heading-skip:2>5:contacto"));
    }

    #[test]
    fn two_different_texts_are_two_different_causes() {
        let mut a = ctx();
        a.heading_levels = &[1, 4];
        a.heading_texts = &["Título", "Firma del autor"];
        let mut b = ctx();
        b.heading_levels = &[1, 4];
        b.heading_texts = &["Título", "Entradas relacionadas"];
        let ka = ContentHeadingSkip.evaluate(&a)[0].group_key.clone();
        let kb = ContentHeadingSkip.evaluate(&b)[0].group_key.clone();
        assert!(ka.is_some() && kb.is_some());
        assert_ne!(ka, kb, "the same jump with different text is not the same template");
    }

    #[test]
    fn a_page_with_a_single_heading_cannot_skip() {
        let mut c = ctx();
        c.heading_levels = &[1];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
        c.heading_levels = &[];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    #[test]
    fn a_skip_on_a_non_indexable_page_does_not_warn() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 4];
        c.is_indexable = false;
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    // --- CONTENT-THIN ---

    #[test]
    fn does_not_warn_with_enough_content() {
        assert!(ContentThin.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn the_threshold_is_three_hundred_words() {
        let mut c = ctx();
        c.word_count = 300;
        assert!(ContentThin.evaluate(&c).is_empty(), "300 words is no longer thin content");
        c.word_count = 299;
        let issues = ContentThin.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-THIN");
        assert_eq!(issues[0].severity, Severity::High);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"word_count\":299"), "{detalle}");
        assert!(detalle.contains("\"threshold\":300"), "{detalle}");
    }

    #[test]
    fn a_page_with_no_text_is_thin_content() {
        let mut c = ctx();
        c.word_count = 0;
        assert_eq!(ContentThin.evaluate(&c).len(), 1);
    }

    #[test]
    fn a_short_but_non_indexable_page_does_not_warn() {
        // A `noindex` page does not compete in the results: its length is not a problem.
        let mut c = ctx();
        c.word_count = 10;
        c.is_indexable = false;
        assert!(ContentThin.evaluate(&c).is_empty());
    }

    #[test]
    fn a_short_pdf_is_not_thin_content() {
        let mut c = ctx();
        c.word_count = 10;
        c.is_html = false;
        assert!(ContentThin.evaluate(&c).is_empty());
    }

    // --- CONTENT-LANG-MISSING ---

    #[test]
    fn does_not_warn_when_the_language_is_declared() {
        assert!(ContentLangMissing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn warns_when_the_lang_attribute_is_missing() {
        let mut c = ctx();
        c.lang = None;
        let issues = ContentLangMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-LANG-MISSING");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn an_empty_lang_counts_as_missing() {
        let mut c = ctx();
        c.lang = Some("  ");
        assert_eq!(ContentLangMissing.evaluate(&c).len(), 1);
    }

    #[test]
    fn does_not_warn_about_language_on_a_non_indexable_page() {
        let mut c = ctx();
        c.lang = None;
        c.is_indexable = false;
        assert!(ContentLangMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn does_not_warn_about_language_on_something_that_is_not_html() {
        let mut c = ctx();
        c.lang = None;
        c.is_html = false;
        assert!(ContentLangMissing.evaluate(&c).is_empty());
    }
}
