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
            "bu bağlantı", "bu sayfa", "burayı tıklayın", "buraya tıklayınız", "tıklayınız",
            "devamını gör", "devamını görüntüle", "devamı için tıklayın", "daha fazla oku",
            "daha fazlasını gör", "daha fazlasını öğren", "ayrıntılı bilgi", "detaylar için tıklayın",
            "detaylı bilgi için tıklayın", "bu link", "linke tıklayın",
        ],
        only_if_declared: &[
            "tıkla", "burada", "buraya", "oku", "incele", "detaylar", "ayrıntılar", "bağlantı",
            "git", "link", "detay", "devam",
        ],
    },
    GenericAnchors {
        lang: "pt",
        phrases: &[
            "clique aqui", "clique neste link", "leia mais", "leia mais aqui", "saiba mais",
            "saiba mais aqui", "veja mais", "ver mais", "veja aqui", "mais", "continue lendo",
            "continuar lendo", "leia o artigo", "mais informações", "mais detalhes", "ver detalhes",
            "acesse aqui", "confira", "este link", "esta página", "aqui",
        ],
        // «Clique» is French and English for a clique (and a brand); «Veja» a Brazilian
        // magazine and a sneaker brand.
        only_if_declared: &[
            "clique", "link", "ler", "leia", "veja", "detalhes", "continuar", "acesse", "informações",
        ],
    },
    GenericAnchors {
        lang: "pl",
        phrases: &[
            "kliknij tutaj", "kliknij tu", "kliknij", "kliknij w link", "tutaj", "czytaj więcej",
            "czytaj dalej", "przeczytaj więcej", "dowiedz się więcej", "więcej", "zobacz więcej",
            "pokaż więcej", "więcej informacji", "więcej szczegółów", "szczegóły",
            "zobacz szczegóły", "zobacz", "sprawdź", "przejdź", "przejdź tutaj", "ten link",
            "ta strona", "tę stronę",
        ],
        // «Tu» is French and Spanish for «you»; «info» and «link» are words in half the list.
        only_if_declared: &["tu", "link", "czytaj", "info", "detale"],
    },
    GenericAnchors {
        lang: "ja",
        phrases: &[
            "こちら", "コチラ", "ここ", "こちらをクリック", "ここをクリック", "クリック",
            "クリックしてください", "こちらから", "続き", "続きを読む", "続きはこちら", "続きを見る",
            "もっと", "もっと見る", "もっとみる", "もっと読む", "さらに表示", "詳しく",
            "詳しくはこちら", "詳しく見る", "詳細はこちら", "詳細を見る", "全文を読む", "記事を読む",
            "このリンク", "このページ",
        ],
        // The same ideographs mean the same in Chinese: «詳細» (details) and «情報»
        // (information) are a section title on a Chinese page as often as a stock link.
        only_if_declared: &["詳細", "詳細情報", "情報", "リンク", "参照"],
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
/// straight, full-width Latin letters made ASCII, whitespace collapsed, and the decoration at
/// both ends dropped —«Leer más »», «[Read more]», «Read more…», «→ Ver más»—.
///
/// Folding is deliberately crude and only covers the letters of the languages in the
/// catalogue: it exists to make «leer mas» equal «leer más», not to be a Unicode normaliser.
/// The full-width forms (`ＲＥＡＤ ＭＯＲＥ`, the ideographic space) are the one piece of
/// NFKC a Japanese page needs, and they are a fixed offset from ASCII.
pub fn fold_anchor(text: &str) -> String {
    let mut plano = String::with_capacity(text.len());
    let ancho_completo = |c: char| match c {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        '\u{3000}' => ' ',
        otro => otro,
    };
    for c in text.chars().map(ancho_completo).flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ą' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'ę' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'ı' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' | 'ć' => 'c',
            'ş' | 'ś' => 's',
            'ğ' => 'g',
            'ł' => 'l',
            'ń' => 'n',
            'ź' | 'ż' => 'z',
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
/// path segment of the `href`, percent-decoded and with hyphens as spaces, against the folded
/// text: a Japanese or Polish tag travels as `/tag/%E3%82%82%E3%81%A3%E3%81%A8/` or
/// `/tag/wi%C4%99cej/`.
fn names_its_slug(href: &str, texto: &str) -> bool {
    let ruta = href.split(['?', '#']).next().unwrap_or(href);
    let Some(slug) = ruta.split('/').rev().find(|s| !s.is_empty()) else {
        return false;
    };
    fold_anchor(&percent_decode(slug).replace(['-', '_'], " ")) == fold_anchor(texto)
}

/// `%XX` escapes to bytes, read as UTF-8 (lossily: a malformed escape is left as is and the
/// comparison simply fails). This crate does not depend on `crawlforge-core`, whose fetcher
/// has its own, private, for file paths.
fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
            assert!(is_generic_anchor(texto, Some("ko")), "{texto} on an uncatalogued language");
        }
    }

    #[test]
    fn generic_knows_portuguese() {
        for texto in ["Leia mais", "Saiba mais", "Clique aqui", "Continue lendo", "Mais informações", "Confira"] {
            assert!(is_generic_anchor(texto, Some("pt-BR")), "{texto}");
            assert!(is_generic_anchor(texto, None), "{texto}: unmistakable anywhere");
        }
        // «Clique» is a clique in French and English, «Veja» a magazine: Portuguese pages only.
        for texto in ["Clique", "Veja", "Leia", "Detalhes"] {
            assert!(is_generic_anchor(texto, Some("pt")), "{texto}");
            assert!(!is_generic_anchor(texto, Some("fr")), "{texto} on a French page");
            assert!(!is_generic_anchor(texto, None), "{texto} with no language");
        }
    }

    #[test]
    fn generic_knows_polish() {
        for texto in ["Czytaj więcej", "Kliknij tutaj", "Więcej", "Dowiedz się więcej", "WIĘCEJ SZCZEGÓŁÓW", "Tę stronę"] {
            assert!(is_generic_anchor(texto, Some("pl")), "{texto}");
            assert!(is_generic_anchor(texto, None), "{texto}: unmistakable anywhere");
        }
        assert!(is_generic_anchor("czytaj wiecej", Some("pl")), "typed without the ogonek");
        // «Tu» is «you» in French and Spanish.
        assert!(is_generic_anchor("Tu", Some("pl")));
        assert!(!is_generic_anchor("Tu", Some("fr")));
        assert!(!is_generic_anchor("Tu", None));
    }

    #[test]
    fn generic_knows_japanese() {
        for texto in ["こちら", "続きを読む", "詳しくはこちら", "もっと見る", "ここをクリック", "続きを読む »"] {
            assert!(is_generic_anchor(texto, Some("ja")), "{texto}");
            assert!(is_generic_anchor(texto, None), "{texto}: unmistakable anywhere");
        }
        // «詳細» and «情報» are ordinary headings in Chinese.
        assert!(is_generic_anchor("詳細", Some("ja")));
        assert!(!is_generic_anchor("詳細", Some("zh-TW")));
        assert!(!is_generic_anchor("情報", None));
    }

    #[test]
    fn generic_knows_the_turkish_additions() {
        for texto in ["Devamını gör", "Burayı tıklayın", "Daha fazlasını öğren", "Detaylı bilgi için tıklayın"] {
            assert!(is_generic_anchor(texto, Some("tr")), "{texto}");
            assert!(is_generic_anchor(texto, None), "{texto}");
        }
        assert!(is_generic_anchor("Devam", Some("tr")));
        assert!(!is_generic_anchor("Devam", None));
    }

    #[test]
    fn generic_folds_full_width_latin() {
        // A Japanese page typing the English phrase with its full-width keyboard.
        assert!(is_generic_anchor("ＲＥＡＤ　ＭＯＲＥ", Some("ja")));
        assert!(is_generic_anchor("Ｃｌｉｃｋ ｈｅｒｅ", None));
    }

    #[test]
    fn generic_leaves_a_percent_encoded_tag_named_like_its_text() {
        // The tag-name exception compared the encoded slug, so a non-ASCII tag never matched.
        let links = [
            enlace("/tag/%E3%82%82%E3%81%A3%E3%81%A8/", Some("もっと")),
            enlace("https://przyklad.pl/tag/wi%C4%99cej/", Some("Więcej")),
            enlace("/post-1/", Some("もっと")),
        ];
        let issues = LinkAnchorGeneric.evaluate(&ctx(&links, Some("ja")));
        assert_eq!(issues.len(), 1);
        let d = detalle(&issues[0]);
        assert!(d.contains("\"links\":1"), "only the post link counts: {d}");
        assert_eq!(percent_decode("wi%C4%99cej"), "więcej");
        assert_eq!(percent_decode("100%"), "100%", "a stray percent is left as is");
        assert_eq!(percent_decode("%4"), "%4");
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
