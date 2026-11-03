//! `LINK` — the text of internal links. `docs/04-CATALOGO-REGLAS.md §9.bis`.
//!
//! The anchor text is how a page tells search engines —and screen readers— what the page it
//! links to is about. Google documents it plainly: «Good anchor text is descriptive, reasonably
//! concise, and relevant to the page that it's on and to the page it links to»
//! (developers.google.com/search/docs/crawling-indexing/links-crawlable). An internal link
//! with no text says nothing; one that says «read more» says the same thing about every page.
//!
//! **Only internal `<a>` links.** Resources (`<img>`, `<script>`…) have no anchor, and the text
//! of a link to someone else's site is not the owner's ranking signal to spend. Same-page
//! fragments (`href="#…"`) are left out too: they are buttons in link's clothing —tabs,
//! accordions, «back to top»— and nobody navigates anywhere through them.
//!
//! **One finding per page, with the count**, not one per link: the cause is a template or an
//! editor's habit, and forty rows from the same archive page say no more than one row with
//! `"links": 40`.

use crate::{
    Category, Issue, LinkView, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier,
};
use std::collections::HashSet;
use std::sync::OnceLock;

/// How many examples a finding keeps in its `detail_json`. The count stays complete.
const SAMPLE_LIMIT: usize = 10;

pub static LINK_ANCHOR_EMPTY: RuleMeta = RuleMeta {
    id: "LINK-ANCHOR-EMPTY",
    severity: Severity::Medium,
    category: Category::Links,
    min_tier: Tier::Free,
    scope: Scope::Page,
    references: &[],
};

pub static LINK_ANCHOR_GENERIC: RuleMeta = RuleMeta {
    id: "LINK-ANCHOR-GENERIC",
    severity: Severity::Low,
    category: Category::Links,
    min_tier: Tier::Free,
    scope: Scope::Page,
    references: &[],
};

// ---------------------------------------------------------------------------------------------
// Which links count
// ---------------------------------------------------------------------------------------------

/// An internal `<a>` that leads somewhere else: what both rules judge.
fn is_internal_navigation(link: &LinkView<'_>) -> bool {
    link.is_internal
        && !link.is_resource
        && !link.is_infrastructure
        && !link.href.trim_start().starts_with('#')
}

/// Up to [`SAMPLE_LIMIT`] distinct hrefs, in order of appearance.
fn sample<'a>(hrefs: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for href in hrefs {
        if !out.contains(&href) {
            out.push(href);
            if out.len() >= SAMPLE_LIMIT {
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// LINK-ANCHOR-EMPTY
// ---------------------------------------------------------------------------------------------

/// Internal links with no name at all: no text, no image, no label.
///
/// What gives a link a name, and why each one is excluded here:
///
/// - **Text**, including text hidden with CSS (`<span class="screen-reader-text">`) and the
///   `<title>` of an inline `<svg>`: the parser reads it all as the anchor.
/// - **An image inside.** Its name is the `alt`, and the `ASSET-IMG-*` rules already judge it:
///   an image with `alt=""` as the only content is `ASSET-IMG-EMPTY-ALT-LINK`, one without
///   `alt` is `ASSET-IMG-NO-ALT`. Looking at it here would report the same `<img>` twice.
/// - **`aria-label`, `aria-labelledby` or `title`** on the link, or an `<svg aria-label>`
///   inside it. Google does not document reading them, but a screen reader does, and the claim
///   this rule makes —"this link has no name"— would be false.
///
/// The `group_key` is the set of empty targets: when they are the template's (a header icon,
/// a footer badge) every page shares it and the report collapses them into one defect.
pub struct LinkAnchorEmpty;

impl PageRule for LinkAnchorEmpty {
    fn meta(&self) -> &'static RuleMeta {
        &LINK_ANCHOR_EMPTY
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // The 2xx keeps the error template out: its header would repeat on every broken URL.
        // Indexability is not required, as with the `alt` rules: a `noindex` page is still
        // crawled and visited, and its links still lead somewhere.
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        let vacios: Vec<&str> = ctx
            .links
            .iter()
            .filter(|l| is_internal_navigation(l))
            .filter(|l| l.anchor.is_none_or(|texto| texto.trim().is_empty()))
            .filter(|l| !l.has_image && !l.labelled)
            .map(|l| l.href)
            .collect();
        if vacios.is_empty() {
            return Vec::new();
        }

        let mut destinos: Vec<&str> = vacios.clone();
        destinos.sort_unstable();
        destinos.dedup();
        let clave = xxhash_rust::xxh3::xxh3_64(destinos.join("\n").as_bytes());

        vec![Issue::new(&LINK_ANCHOR_EMPTY)
            .with_detail(serde_json::json!({
                "links": vacios.len(),
                "sample": sample(vacios.iter().copied()),
            }))
            .with_group(format!("link-anchor-empty:{clave:016x}"))]
    }
}

// ---------------------------------------------------------------------------------------------
// LINK-ANCHOR-GENERIC
// ---------------------------------------------------------------------------------------------

/// The stock phrases of one language.
///
/// Two lists, because some words are generic only when we know the page is in that language:
///
/// - `phrases` are unmistakable in any page — «leer más», «click here», «klik hier» — and are
///   checked on every page, whatever its `lang`. A Spanish site whose theme left «Read more»
///   untranslated is the most common case of all, and it must be caught.
/// - `only_if_declared` are words that can be a proper noun or a real destination elsewhere:
///   «Go» is a programming language, «Link» a product, «Plus» a plan, «Suite» a room. They
///   count only on a page whose `<html lang>` says it is in that language.
///
/// **Adding a language is adding one entry to [`GENERIC_ANCHORS`].** Phrases are written as
/// people write them, accents and all; the comparison folds both sides (see [`fold_anchor`]),
/// so «leer mas» and «LEER MÁS…» match «leer más».
pub struct GenericAnchors {
    /// ISO 639-1 code, lowercase: compared with the primary subtag of `<html lang>`.
    pub lang: &'static str,
    pub phrases: &'static [&'static str],
    pub only_if_declared: &'static [&'static str],
}

/// The catalogue of generic anchor texts, one entry per language.
///
/// Whole-text matches only: «read more about pricing» names its target and is fine. Pagination
/// («next», «siguiente») is left out on purpose: it is a convention every visitor and every
/// search engine understands, and its targets are not pages anyone ranks for.
pub const GENERIC_ANCHORS: &[GenericAnchors] = &[
    GenericAnchors {
        lang: "en",
        phrases: &[
            "click here", "click this", "click", "here", "right here", "go here", "tap here",
            "read more", "read more here", "more", "learn more", "more info", "more information",
            "find out more", "see more", "view more", "continue reading", "keep reading",
            "this link", "this page", "more details", "see details", "view details",
        ],
        only_if_declared: &["link", "info", "read", "view", "this", "continue", "details"],
    },
    GenericAnchors {
        lang: "es",
        phrases: &[
            "haz clic aquí", "haz click aquí", "hacer clic aquí", "clic aquí", "click aquí",
            "clica aquí", "clicka aquí", "pincha aquí", "pulsa aquí", "presiona aquí", "aquí",
            "leer más", "seguir leyendo", "sigue leyendo", "continuar leyendo", "más", "ver más",
            "saber más", "más información", "más info", "más detalles", "ver detalles",
            "este enlace", "esta página", "enlace",
        ],
        only_if_declared: &["leer", "ir", "información", "detalles", "continuar"],
    },
    GenericAnchors {
        lang: "de",
        phrases: &[
            "hier klicken", "klicken sie hier", "klick hier", "hier", "mehr", "weiterlesen",
            "mehr lesen", "mehr erfahren", "mehr infos", "mehr informationen", "mehr details",
            "dieser link", "diese seite", "zum artikel", "jetzt lesen", "hier lesen",
        ],
        only_if_declared: &["weiter", "lesen", "link", "details", "infos"],
    },
    GenericAnchors {
        lang: "fr",
        phrases: &[
            "cliquez ici", "cliquer ici", "clique ici", "cliquez", "ici", "lire la suite",
            "lire plus", "en savoir plus", "voir plus", "plus d'infos", "plus d'informations",
            "voir les détails", "continuer la lecture", "ce lien", "cette page",
        ],
        only_if_declared: &["plus", "suite", "la suite", "lien", "détails", "lire"],
    },
    GenericAnchors {
        lang: "it",
        phrases: &[
            "clicca qui", "clicca", "leggi di più", "leggi tutto", "leggi altro",
            "scopri di più", "di più", "maggiori informazioni", "ulteriori informazioni",
            "continua a leggere", "vedi di più", "questo link", "questa pagina",
        ],
        // «qui» is also French for «who».
        only_if_declared: &["qui", "leggi", "altro", "più", "continua", "dettagli", "vai", "link"],
    },
    GenericAnchors {
        lang: "nl",
        phrases: &[
            "klik hier", "hier klikken", "lees meer", "lees verder", "verder lezen", "meer lezen",
            "meer info", "meer informatie", "bekijk meer", "deze link", "deze pagina",
        ],
        // «Meer» is also German for «sea».
        only_if_declared: &["meer", "klik", "lees", "link", "details", "ga", "verder"],
    },
    GenericAnchors {
        lang: "tr",
        phrases: &[
            "buraya tıklayın", "buraya tıkla", "tıklayın", "devamı", "devamını oku",
            "devamını okuyun", "daha fazla", "daha fazlası", "daha fazla bilgi", "detaylı bilgi",
            "bu bağlantı", "bu sayfa",
        ],
        only_if_declared: &[
            "tıkla", "burada", "buraya", "oku", "incele", "detaylar", "ayrıntılar", "bağlantı",
            "git",
        ],
    },
];

/// One language's lists, folded once.
struct FoldedAnchors {
    lang: &'static str,
    phrases: HashSet<String>,
    only_if_declared: HashSet<String>,
}

struct FoldedCatalogue {
    languages: Vec<FoldedAnchors>,
    /// Longest folded phrase, in bytes. An anchor much longer than this cannot be one, and is
    /// discarded before paying for folding it.
    longest: usize,
}

fn folded_catalogue() -> &'static FoldedCatalogue {
    static CATALOGO: OnceLock<FoldedCatalogue> = OnceLock::new();
    CATALOGO.get_or_init(|| {
        let languages: Vec<FoldedAnchors> = GENERIC_ANCHORS
            .iter()
            .map(|g| FoldedAnchors {
                lang: g.lang,
                phrases: g.phrases.iter().map(|p| fold_anchor(p)).collect(),
                only_if_declared: g.only_if_declared.iter().map(|p| fold_anchor(p)).collect(),
            })
            .collect();
        let longest = languages
            .iter()
            .flat_map(|l| l.phrases.iter().chain(&l.only_if_declared))
            .map(String::len)
            .max()
            .unwrap_or(0);
        FoldedCatalogue { languages, longest }
    })
}

/// The comparison form of an anchor text: lowercase, accents folded, curly apostrophes made
/// straight, whitespace collapsed, and the decoration at both ends dropped —«Leer más »»,
/// «[Read more]», «Read more…», «→ Ver más»—.
///
/// Folding is deliberately crude and only covers the letters of the languages in the
/// catalogue: it exists to make «leer mas» equal «leer más», not to be a Unicode normaliser.
pub fn fold_anchor(text: &str) -> String {
    let mut plano = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'ı' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ş' => 's',
            'ğ' => 'g',
            '’' | '‘' | '´' | '`' => '\'',
            // What Turkish `İ` leaves behind when lowercased: `i` plus a combining dot.
            '\u{0307}' => continue,
            otro => otro,
        };
        plano.push(c);
    }
    plano
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .trim()
        .to_string()
}

/// Is this anchor text a stock phrase, for a page whose `<html lang>` is `page_lang`?
///
/// With a declared language that is in the catalogue, its whole list counts plus every other
/// language's unmistakable phrases. Without one —no `lang`, or a language not catalogued yet—
/// only the unmistakable phrases of all languages: the fallback the catalogue was designed for.
pub fn is_generic_anchor(text: &str, page_lang: Option<&str>) -> bool {
    let catalogo = folded_catalogue();
    // Room for the decoration `fold_anchor` strips: «Leer más »» is longer than «leer más».
    if text.len() > catalogo.longest * 2 + 16 {
        return false;
    }
    let plano = fold_anchor(text);
    if plano.is_empty() {
        return false;
    }
    let declarado = page_lang
        .and_then(crate::hreflang::primary_language)
        .and_then(|idioma| catalogo.languages.iter().find(|l| l.lang == idioma));
    if declarado.is_some_and(|l| l.only_if_declared.contains(&plano)) {
        return true;
    }
    catalogo.languages.iter().any(|l| l.phrases.contains(&plano))
}

/// Is the anchor text the name of the target itself? `<a href="/tag/mas/">más</a>` is a tag
/// someone called «más», and its link text names it exactly: the text is a poor tag name, not
/// a generic link. Found on the first real crawl, a news site's tag cloud. Compares the last
/// path segment of the `href`, with hyphens as spaces, against the folded text.
fn names_its_slug(href: &str, texto: &str) -> bool {
    let ruta = href.split(['?', '#']).next().unwrap_or(href);
    let Some(slug) = ruta.split('/').rev().find(|s| !s.is_empty()) else {
        return false;
    };
    // `%C3%A1` is how an accented slug travels; it is left as is and simply will not match.
    fold_anchor(&slug.replace(['-', '_'], " ")) == fold_anchor(texto)
}

/// Internal links whose whole text is a stock phrase.
///
/// The `group_key` is the set of phrases found, folded: the archive template that prints
/// «Leer más» under every excerpt gives the same key on every listing page, and the report can
/// say so in one line. `aria-label` does not rescue a generic text here, unlike in
/// [`LinkAnchorEmpty`]: the claim is about what a search engine reads, and that is the text.
pub struct LinkAnchorGeneric;

impl PageRule for LinkAnchorGeneric {
    fn meta(&self) -> &'static RuleMeta {
        &LINK_ANCHOR_GENERIC
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_success() {
            return Vec::new();
        }
        let genericos: Vec<(&str, &str)> = ctx
            .links
            .iter()
            .filter(|l| is_internal_navigation(l))
            .filter_map(|l| Some((l.href, l.anchor?)))
            .filter(|(_, texto)| is_generic_anchor(texto, ctx.lang))
            .filter(|(href, texto)| !names_its_slug(href, texto))
            .collect();
        if genericos.is_empty() {
            return Vec::new();
        }

        let mut frases: Vec<String> = genericos.iter().map(|(_, t)| fold_anchor(t)).collect();
        frases.sort_unstable();
        frases.dedup();
        let textos = sample(genericos.iter().map(|(_, t)| *t));

        vec![Issue::new(&LINK_ANCHOR_GENERIC)
            .with_detail(serde_json::json!({
                "links": genericos.len(),
                "texts": textos,
                "sample": sample(genericos.iter().map(|(h, _)| *h)),
            }))
            .with_group(format!("link-anchor-generic:{}", frases.join(",")))]
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![Box::new(LinkAnchorEmpty), Box::new(LinkAnchorGeneric)]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enlace(href: &'static str, anchor: Option<&'static str>) -> LinkView<'static> {
        LinkView { href, anchor, is_internal: true, ..Default::default() }
    }

    fn ctx<'a>(links: &'a [LinkView<'a>], lang: Option<&'a str>) -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/blog/");
        c.links = links;
        c.lang = lang;
        c
    }

    fn detalle(issue: &Issue) -> String {
        issue.detail_json.clone().unwrap_or_default()
    }

    // ------------------------------------------------------------------ LINK-ANCHOR-EMPTY

    #[test]
    fn empty_reports_internal_links_with_no_name_once_per_page() {
        let links = [
            enlace("/carrito", None),
            enlace("/carrito", None),
            enlace("/buscar", Some("   ")),
            enlace("/contacto", Some("Contacto")),
        ];
        let issues = LinkAnchorEmpty.evaluate(&ctx(&links, Some("es")));
        assert_eq!(issues.len(), 1, "one finding per page, not per link");
        assert_eq!(issues[0].rule_id, "LINK-ANCHOR-EMPTY");
        let d = detalle(&issues[0]);
        assert!(d.contains("\"links\":3"), "{d}");
        assert!(d.contains("\"sample\":[\"/carrito\",\"/buscar\"]"), "{d}");
        assert!(issues[0].group_key.as_deref().is_some_and(|g| g.starts_with("link-anchor-empty:")));
    }

    #[test]
    fn empty_leaves_image_links_to_the_asset_rules() {
        // `<a><img alt=""></a>` is ASSET-IMG-EMPTY-ALT-LINK, `<a><img></a>` ASSET-IMG-NO-ALT:
        // the same `<img>` must not be reported twice.
        let mut con_imagen = enlace("/", None);
        con_imagen.has_image = true;
        assert!(LinkAnchorEmpty.evaluate(&ctx(&[con_imagen], None)).is_empty());
    }

    #[test]
    fn empty_accepts_a_label_as_a_name() {
        let mut etiquetado = enlace("/carrito", None);
        etiquetado.labelled = true;
        assert!(LinkAnchorEmpty.evaluate(&ctx(&[etiquetado], None)).is_empty());
    }

    #[test]
    fn empty_ignores_what_is_not_internal_navigation() {
        let mut externo = enlace("https://otro.example/", None);
        externo.is_internal = false;
        let mut recurso = enlace("/estilo.css", None);
        recurso.is_resource = true;
        let mut cdn = enlace("/cdn-cgi/l/email-protection#x", None);
        cdn.is_infrastructure = true;
        let ancla = enlace("#top", None);
        let links = [externo, recurso, cdn, ancla];
        assert!(LinkAnchorEmpty.evaluate(&ctx(&links, None)).is_empty());
    }

    #[test]
    fn empty_groups_by_the_set_of_targets() {
        // The header's empty icon on two pages is one template defect: same key.
        let a = [enlace("/carrito", None), enlace("/buscar", None)];
        let b = [enlace("/buscar", None), enlace("/carrito", None), enlace("/x", Some("X"))];
        let ga = LinkAnchorEmpty.evaluate(&ctx(&a, None))[0].group_key.clone();
        let gb = LinkAnchorEmpty.evaluate(&ctx(&b, None))[0].group_key.clone();
        assert_eq!(ga, gb);
    }

    #[test]
    fn empty_does_not_audit_the_error_template() {
        let links = [enlace("/carrito", None)];
        for status in [301, 404, 500] {
            let mut c = ctx(&links, None);
            c.status = status;
            assert!(LinkAnchorEmpty.evaluate(&c).is_empty(), "{status}");
        }
    }

    // ------------------------------------------------------------------ LINK-ANCHOR-GENERIC

    #[test]
    fn generic_reports_stock_phrases_once_per_page() {
        let links = [
            enlace("/post-1", Some("Leer más")),
            enlace("/post-2", Some("Leer más »")),
            enlace("/post-3", Some("aquí")),
            enlace("/tarifas", Some("Leer más sobre tarifas")),
            enlace("/contacto", Some("Contacto")),
        ];
        let issues = LinkAnchorGeneric.evaluate(&ctx(&links, Some("es")));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "LINK-ANCHOR-GENERIC");
        assert_eq!(issues[0].severity, Severity::Low);
        let d = detalle(&issues[0]);
        assert!(d.contains("\"links\":3"), "{d}");
        assert_eq!(issues[0].group_key.as_deref(), Some("link-anchor-generic:aqui,leer mas"));
    }

    #[test]
    fn generic_folds_case_accents_and_decoration() {
        for texto in ["LEER MÁS", "leer mas", "Leer más…", "[Leer más]", "→ Ver más", "Read more ›"]
        {
            assert!(is_generic_anchor(texto, Some("es")), "{texto}");
        }
        assert!(is_generic_anchor("Plus d’infos", Some("fr")), "the curly apostrophe");
        assert!(is_generic_anchor("DEVAMINI OKU", Some("tr")), "dotless i, uppercased");
        assert!(is_generic_anchor("Devamını oku", Some("tr")));
    }

    #[test]
    fn generic_needs_the_whole_text_to_be_the_phrase() {
        for texto in ["Leer más sobre tarifas", "Aquí vivimos", "More than a shop", "Here & now"] {
            assert!(!is_generic_anchor(texto, Some("es")), "{texto}");
            assert!(!is_generic_anchor(texto, None), "{texto}");
        }
    }

    #[test]
    fn generic_catches_another_language_left_untranslated() {
        // The theme's English «Read more» on a Spanish site: the most common case of all.
        assert!(is_generic_anchor("Read more", Some("es")));
        assert!(is_generic_anchor("Weiterlesen", Some("es")));
    }

    #[test]
    fn generic_ambiguous_words_need_the_declared_language() {
        // «Go» is a programming language, «Plus» a plan, «Suite» a room: generic only when the
        // page says it is in the language where they are.
        assert!(!is_generic_anchor("Go", None));
        assert!(!is_generic_anchor("Plus", Some("en")));
        assert!(is_generic_anchor("Plus", Some("fr")));
        assert!(!is_generic_anchor("Suite", Some("en")));
        assert!(is_generic_anchor("La suite", Some("fr-CA")));
        assert!(!is_generic_anchor("Link", None));
        assert!(is_generic_anchor("Link", Some("de")));
        assert!(!is_generic_anchor("Git", Some("en")));
        assert!(is_generic_anchor("Git", Some("tr")));
    }

    #[test]
    fn generic_without_a_language_checks_every_list() {
        for texto in ["Click here", "Leer más", "Hier klicken", "Lire la suite", "Leggi di più",
            "Lees meer", "Devamını oku"]
        {
            assert!(is_generic_anchor(texto, None), "{texto}");
            assert!(is_generic_anchor(texto, Some("pt")), "{texto} on an uncatalogued language");
        }
    }

    #[test]
    fn generic_leaves_a_link_whose_text_is_its_targets_name() {
        // A tag called «más», linked from a news site's tag cloud: the text names the target
        // exactly. A poor tag name, not a generic link. The «Más» next to it is.
        let links = [
            enlace("https://ejemplo.es/tag/mas/", Some("más")),
            enlace("/etiqueta/leer-mas?p=2", Some("Leer más")),
            enlace("/post-1/", Some("Más")),
        ];
        let issues = LinkAnchorGeneric.evaluate(&ctx(&links, Some("es")));
        assert_eq!(issues.len(), 1);
        let d = detalle(&issues[0]);
        assert!(d.contains("\"links\":1"), "{d}");
        assert!(d.contains("/post-1/"), "{d}");
    }

    #[test]
    fn generic_ignores_external_links_resources_and_fragments() {
        let mut externo = enlace("https://otro.example/", Some("Read more"));
        externo.is_internal = false;
        let ancla = enlace("#comentarios", Some("Leer más"));
        let links = [externo, ancla];
        assert!(LinkAnchorGeneric.evaluate(&ctx(&links, Some("es"))).is_empty());
    }

    #[test]
    fn generic_does_not_audit_the_error_template() {
        let links = [enlace("/post", Some("Leer más"))];
        let mut c = ctx(&links, Some("es"));
        c.status = 404;
        assert!(LinkAnchorGeneric.evaluate(&c).is_empty());
    }

    #[test]
    fn every_catalogued_phrase_matches_itself() {
        // A phrase whose folded form came out empty, or that the length shortcut discards,
        // would sit in the list doing nothing.
        for idioma in GENERIC_ANCHORS {
            for frase in idioma.phrases.iter().chain(idioma.only_if_declared) {
                assert!(is_generic_anchor(frase, Some(idioma.lang)), "{}: {frase}", idioma.lang);
            }
        }
    }

    #[test]
    fn every_catalogue_language_is_a_language_code() {
        let mut vistos = HashSet::new();
        for idioma in GENERIC_ANCHORS {
            assert_eq!(
                crate::hreflang::primary_language(idioma.lang).as_deref(),
                Some(idioma.lang),
                "{} must be a lowercase ISO 639-1 code",
                idioma.lang
            );
            assert!(vistos.insert(idioma.lang), "{} is listed twice", idioma.lang);
        }
    }
}
