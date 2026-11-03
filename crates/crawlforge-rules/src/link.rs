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
//!
//! `LINK-FRAGMENT-BROKEN` is the third, and the odd one out: it is about where a link lands, not
//! what it says, so it does read same-page fragments, and it is a site rule. See
//! [`LinkFragmentBroken`].

use crate::{
    Category, Issue, LinkView, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier,
};
use rusqlite::Connection;
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

pub static LINK_FRAGMENT_BROKEN: RuleMeta = RuleMeta {
    id: "LINK-FRAGMENT-BROKEN",
    severity: Severity::Low,
    category: Category::Links,
    min_tier: Tier::Free,
    scope: Scope::Site,
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
    String::from_utf8_lossy(&percent_decode_bytes(s)).into_owned()
}

/// `%XX` escapes to bytes; a malformed escape is kept as written, as the URL standard does.
fn percent_decode_bytes(s: &str) -> Vec<u8> {
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
    out
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
    vec![Box::new(LinkFragmentBroken)]
}

// ---------------------------------------------------------------------------------------------
// LINK-FRAGMENT-BROKEN
// ---------------------------------------------------------------------------------------------

/// How long a fragment may be in a finding's detail. A fragment is the site's own text, but a
/// broken template can glue a whole URL or a JSON blob after the `#`.
const FRAGMENT_DETAIL_CHARS: usize = 200;

/// The hash a fragment target is stored under in `pages.fragment_ids`: the low 32 bits of the
/// xxh3 of its UTF-8 bytes.
///
/// **Duplicates `crawlforge_core::parse::fragment_id_hash` on purpose**, as
/// [`crate::INFRASTRUCTURE_PATH_PREFIXES`] duplicates the frontier's list: this crate does not
/// know the core. The fixture of `LINK-FRAGMENT-BROKEN` is crawled with the core's parser and
/// judged with this function, so the day they disagree that test reports every fragment broken.
pub fn fragment_id_hash(value: &str) -> u32 {
    xxhash_rust::xxh3::xxh3_64(value.as_bytes()) as u32
}

/// Is `hash` in a `pages.fragment_ids` blob? The blob is sorted little-endian `u32`s.
fn has_fragment_id(ids: &[u8], hash: u32) -> bool {
    let at = |i: usize| {
        let c = &ids[i * 4..i * 4 + 4];
        u32::from_le_bytes([c[0], c[1], c[2], c[3]])
    };
    let (mut lo, mut hi) = (0, ids.len() / 4);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        match at(mid).cmp(&hash) {
            std::cmp::Ordering::Equal => return true,
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
        }
    }
    false
}

/// Does a link to `#fragment` land on something in a page whose targets are `ids`? `true` also
/// for the fragments that cannot be broken, so `false` is exactly the finding.
///
/// It follows the HTML standard's "find a potential indicated element" and "select the indicated
/// part" (html.spec.whatwg.org/multipage/browsing-the-web.html#the-indicated-part-of-the-document):
///
/// 1. **A text fragment directive is cut first.** Everything from `:~:` on is for the browser's
///    text search, not an element: `#:~:text=precio` lands on the top of the page if the text is
///    gone, and `#planes:~:text=precio` is judged as `#planes`.
/// 2. **An empty fragment is the top of the page**, always valid.
/// 3. **Hash routing is not a fragment**: `#/carrito` and `#!/carrito` are routes of a
///    JavaScript application, which reads them itself and never looks for an element with that
///    id. Judging them would report every route of a single-page app as broken.
/// 4. **The fragment as written, then percent-decoded**: an element whose `id` —or an `<a>`
///    whose `name`— is exactly that, case and all. `#caf%C3%A9` finds `id="café"`.
/// 5. **`top`**, in any case, is the top of the page when nothing is called that.
pub fn fragment_lands(fragment: &str, ids: &[u8]) -> bool {
    let fragment = fragment.find(":~:").map_or(fragment, |i| &fragment[..i]);
    if fragment.is_empty() || fragment.starts_with('/') || fragment.starts_with('!') {
        return true;
    }
    if has_fragment_id(ids, fragment_id_hash(fragment)) {
        return true;
    }
    // "UTF-8 decode without BOM or fail": invalid bytes make no second lookup.
    let decoded = if fragment.contains('%') {
        String::from_utf8(percent_decode_bytes(fragment)).ok()
    } else {
        None
    };
    if let Some(d) = decoded.as_deref() {
        if has_fragment_id(ids, fragment_id_hash(d)) {
            return true;
        }
    }
    decoded.as_deref().unwrap_or(fragment).eq_ignore_ascii_case("top")
}

/// The fragment as a person reads it: percent-decoded when that is valid UTF-8, and cut to
/// [`FRAGMENT_DETAIL_CHARS`].
fn readable_fragment(fragment: &str) -> String {
    let decoded = String::from_utf8(percent_decode_bytes(fragment))
        .unwrap_or_else(|_| fragment.to_string());
    decoded.chars().take(FRAGMENT_DETAIL_CHARS).collect()
}

/// The links [`LinkFragmentBroken`] judges, ordered so each (target, fragment) pair streams
/// past in one run.
///
/// `l.fragment IS NOT NULL` is a literal so SQLite can use the partial index
/// `idx_links_fragment` (migration 012): the links with a fragment are a sliver of `links`. The
/// `+` in `+l.element` keeps it from picking `idx_links_element` instead, which it did on the
/// test schema: with no statistics an equality looks selective, and `element = 'a'` is most of
/// the table.
fn fragment_links_sql() -> String {
    format!(
        "SELECT t.id, t.url_hash, l.fragment, f.url
         FROM links l
         JOIN urls t ON t.id = l.to_url_id
         JOIN pages p ON p.url_id = t.id
         JOIN urls f ON f.id = l.from_url_id
         WHERE l.fragment IS NOT NULL AND +l.element = 'a'
           AND t.is_internal = 1 AND t.status_code = 200 AND p.fragment_ids IS NOT NULL
           AND f.status_code BETWEEN 200 AND 299
           AND {no_infra}
         ORDER BY t.id, l.fragment, f.url",
        no_infra = crate::sql_not_infrastructure("t.path"),
    )
}

/// Internal links to a `#fragment` that nothing in the target page is called.
///
/// The link reaches the page and stops at the top instead of the section it promised: a table
/// of contents whose heading was renamed, a «see pricing» to a block that was removed, a «skip to
/// content» to an id the theme no longer prints. Google uses those anchors for its "jump to"
/// links in results, and a visitor lands somewhere else than the text said.
///
/// **One finding per target page and fragment**, on the target page, like `HTTP-404-INTERNAL`
/// —the page that lacks the anchor is where it is fixed, or where it is decided that the links
/// have to change—, with how many links and source pages lead there and a sample of them. The
/// `group_key` is the fragment: a template's skip link to a missing `#content` gives the same
/// key on every page and the report collapses it into one defect, while two pages that each lack
/// `#precios` stay apart in a diff, which tells findings apart by URL and key.
///
/// What counts, and what is left out on purpose:
///
/// - **`<a href>` only**, internal and from a 2xx page, as every link rule here. A
///   **same-page link** (`href="#section"`) counts: its target is the page itself.
/// - **The target must have been fetched as HTML with a 200** and parsed by a binary that
///   records its ids (schema 12 onward). A redirect, a 404 or a PDF is judged by other rules, and a
///   page with no recorded ids —an old file, the older half of a resumed crawl— is unknown, not
///   empty: the rule stays silent rather than calling every link to it broken.
/// - **What [`fragment_lands`] says is never broken**: `#`, `#top`, text fragments and hash
///   routes.
/// - **CDN infrastructure**: Cloudflare's `/cdn-cgi/l/email-protection#…` carries the encoded
///   address in the fragment.
///
/// What it cannot see: **an id added by JavaScript** —some table-of-contents plugins and tab
/// widgets do it in the browser— is not in the served HTML, and the finding says "not in the
/// page" about a jump that works for a visitor. That is why the severity is `low`, and why the
/// first real crawls are the measure of it.
pub struct LinkFragmentBroken;

impl SiteRule for LinkFragmentBroken {
    fn meta(&self) -> &'static RuleMeta {
        &LINK_FRAGMENT_BROKEN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let sql = fragment_links_sql();
        let mut stmt = conn.prepare(&sql)?;
        let mut ids_of = conn.prepare("SELECT fragment_ids FROM pages WHERE url_id = ?1")?;

        /// The links to one (target, fragment) pair, as they stream past in order.
        struct Pair {
            target_id: i64,
            target_hash: i64,
            fragment: String,
            links: usize,
            sources: usize,
            sample: Vec<String>,
            last_source: String,
        }

        let mut out = Vec::new();
        let mut ids: (i64, Vec<u8>) = (i64::MIN, Vec::new());
        let mut current: Option<Pair> = None;
        let close = |pair: Pair, ids: &[u8], out: &mut Vec<(Option<i64>, Issue)>| {
            if fragment_lands(&pair.fragment, ids) {
                return;
            }
            let fragment = readable_fragment(&pair.fragment);
            let clave = xxhash_rust::xxh3::xxh3_64(pair.fragment.as_bytes());
            out.push((
                Some(pair.target_hash),
                Issue::new(&LINK_FRAGMENT_BROKEN)
                    .with_detail(serde_json::json!({
                        "fragment": fragment,
                        "links": pair.links,
                        "linked_from": pair.sources,
                        "sample": pair.sample,
                    }))
                    .with_group(format!("link-fragment-broken:{clave:016x}")),
            ));
        };

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let target_id: i64 = row.get(0)?;
            let target_hash: i64 = row.get(1)?;
            let fragment: String = row.get(2)?;
            let source: String = row.get(3)?;

            let same = current
                .as_ref()
                .is_some_and(|c| c.target_id == target_id && c.fragment == fragment);
            if !same {
                if let Some(pair) = current.take() {
                    close(pair, &ids.1, &mut out);
                }
                if ids.0 != target_id {
                    let blob: Option<Vec<u8>> =
                        ids_of.query_row([target_id], |r| r.get(0))?;
                    ids = (target_id, blob.unwrap_or_default());
                }
                current = Some(Pair {
                    target_id,
                    target_hash,
                    fragment,
                    links: 0,
                    sources: 0,
                    sample: Vec::new(),
                    last_source: String::new(),
                });
            }
            if let Some(pair) = current.as_mut() {
                pair.links += 1;
                // Sorted by source within the pair, so a repeated source is the previous row.
                if pair.last_source != source {
                    pair.sources += 1;
                    if pair.sample.len() < SAMPLE_LIMIT {
                        pair.sample.push(source.clone());
                    }
                    pair.last_source = source;
                }
            }
        }
        if let Some(pair) = current.take() {
            close(pair, &ids.1, &mut out);
        }
        Ok(out)
    }
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

    // ---------------------------------------------------------------- LINK-FRAGMENT-BROKEN

    fn ids(names: &[&str]) -> Vec<u8> {
        let mut hashes: Vec<u32> = names.iter().map(|n| fragment_id_hash(n)).collect();
        hashes.sort_unstable();
        hashes.dedup();
        hashes.iter().flat_map(|h| h.to_le_bytes()).collect()
    }

    struct Sitio(Connection);

    impl Sitio {
        fn new() -> Self {
            Self(crate::test_schema::full_schema())
        }

        /// A URL; `ids` is its `pages` row: `None` for no row, `Some(None)` for a row written
        /// before migration 012.
        fn url(&self, id: i64, path: &str, status: i64, internal: bool, ids: Option<Option<Vec<u8>>>) {
            let host = if internal { "ejemplo.es" } else { "otro.example" };
            self.0
                .execute(
                    "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                                       crawl_state, status_code, redirect_chain_len)
                     VALUES (?1, ?2, ?1 * 1000, 'https', ?3, ?4, ?5, 0, 'done', ?6, 0)",
                    rusqlite::params![
                        id,
                        format!("https://{host}{path}"),
                        host,
                        path,
                        internal as i64,
                        status
                    ],
                )
                .expect("insert url");
            if let Some(blob) = ids {
                self.0
                    .execute(
                        "INSERT INTO pages (url_id, is_indexable, fragment_ids) VALUES (?1, 1, ?2)",
                        rusqlite::params![id, blob],
                    )
                    .expect("insert page");
            }
        }

        fn link(&self, from: i64, to: i64, element: &str, fragment: Option<&str>) {
            self.0
                .execute(
                    "INSERT INTO links (from_url_id, to_url_id, element, fragment)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![from, to, element, fragment],
                )
                .expect("insert link");
        }

        fn findings(&self) -> Vec<(Option<i64>, Issue)> {
            LinkFragmentBroken.evaluate(&self.0).expect("evaluate")
        }
    }

    /// Home (1) and a guide (2) with `#instalacion` and an `<a name="viejo">`, both crawled.
    fn sitio_con_guia() -> Sitio {
        let s = Sitio::new();
        s.url(1, "/", 200, true, Some(Some(ids(&["contenido"]))));
        s.url(2, "/guia/", 200, true, Some(Some(ids(&["instalacion", "viejo", "café", "Planes"]))));
        s
    }

    #[test]
    fn fragment_reports_a_missing_target_once_per_page_and_fragment_with_counts() {
        let s = sitio_con_guia();
        s.url(3, "/blog/", 200, true, Some(Some(Vec::new())));
        s.link(1, 2, "a", Some("precios"));
        s.link(1, 2, "a", Some("precios"));
        s.link(3, 2, "a", Some("precios"));
        s.link(1, 2, "a", Some("instalacion"));
        let found = s.findings();
        assert_eq!(found.len(), 1, "{found:?}");
        let (hash, issue) = &found[0];
        assert_eq!(*hash, Some(2000), "on the target page, like HTTP-404-INTERNAL");
        let d = detalle(issue);
        assert!(d.contains("\"fragment\":\"precios\""), "{d}");
        assert!(d.contains("\"links\":3"), "{d}");
        assert!(d.contains("\"linked_from\":2"), "{d}");
        assert!(d.contains("https://ejemplo.es/blog/"), "{d}");
        let clave = xxhash_rust::xxh3::xxh3_64(b"precios");
        assert_eq!(issue.group_key, Some(format!("link-fragment-broken:{clave:016x}")));
    }

    #[test]
    fn fragment_finds_ids_and_a_names_as_written_or_percent_decoded_case_and_all() {
        let s = sitio_con_guia();
        s.link(1, 2, "a", Some("instalacion"));
        s.link(1, 2, "a", Some("viejo"));
        s.link(1, 2, "a", Some("caf%C3%A9"));
        s.link(1, 2, "a", Some("Planes"));
        s.link(1, 2, "a", Some("planes"));
        let found = s.findings();
        assert_eq!(found.len(), 1, "only the wrong case is broken: {found:?}");
        assert!(detalle(&found[0].1).contains("\"fragment\":\"planes\""));
    }

    #[test]
    fn fragment_never_judges_the_top_text_fragments_or_hash_routes() {
        for never in ["", "top", "TOP", "Top", ":~:text=precio", "/carrito", "!/carrito", "%74op"] {
            assert!(fragment_lands(never, &ids(&[])), "{never:?} is never broken");
        }
        assert!(!fragment_lands("planes:~:text=precio", &ids(&[])), "judged as #planes");
        assert!(fragment_lands("planes:~:text=precio", &ids(&["planes"])));
        assert!(!fragment_lands("%FF", &ids(&[])), "invalid UTF-8 is looked up only as written");
    }

    #[test]
    fn fragment_a_same_page_link_is_judged_against_its_own_page() {
        let s = sitio_con_guia();
        s.link(1, 1, "a", Some("contenido"));
        s.link(1, 1, "a", Some("principal"));
        let found = s.findings();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, Some(1000));
        assert!(detalle(&found[0].1).contains("\"fragment\":\"principal\""));
    }

    #[test]
    fn fragment_a_page_with_no_ids_is_judged_but_one_with_unknown_ids_is_not() {
        // An empty set is "this page has no anchors"; NULL is a page written before the column
        // existed —an old file, or the older half of a resumed crawl—, and judging it would call
        // every link to it broken.
        let s = sitio_con_guia();
        s.url(3, "/vacia/", 200, true, Some(Some(Vec::new())));
        s.url(4, "/antigua/", 200, true, Some(None));
        s.link(1, 3, "a", Some("x"));
        s.link(1, 4, "a", Some("x"));
        let found = s.findings();
        assert_eq!(found.iter().map(|(h, _)| *h).collect::<Vec<_>>(), vec![Some(3000)]);
    }

    #[test]
    fn fragment_only_judges_internal_html_targets_that_answered_200() {
        let s = sitio_con_guia();
        s.url(3, "/movida/", 301, true, Some(Some(Vec::new())));
        s.url(4, "/rota/", 404, true, Some(Some(Vec::new())));
        s.url(5, "/fuera/", 200, false, Some(Some(Vec::new())));
        s.url(6, "/doc.pdf", 200, true, None);
        for target in 3..=6 {
            s.link(1, target, "a", Some("x"));
        }
        assert!(s.findings().is_empty());
    }

    #[test]
    fn fragment_ignores_links_from_error_pages_resources_and_cdn_infrastructure() {
        let s = sitio_con_guia();
        s.url(3, "/rota/", 404, true, Some(Some(Vec::new())));
        s.link(3, 2, "a", Some("desde-un-404"));
        s.link(1, 2, "link", Some("una-hoja"));
        s.url(4, "/cdn-cgi/l/email-protection", 200, true, Some(Some(Vec::new())));
        s.link(1, 4, "a", Some("a1b2c3"));
        assert!(s.findings().is_empty(), "{:?}", s.findings());
    }

    #[test]
    fn fragment_links_without_a_fragment_are_not_read() {
        let s = sitio_con_guia();
        s.link(1, 2, "a", None);
        assert!(s.findings().is_empty());
    }

    #[test]
    fn fragment_query_uses_the_partial_index() {
        // A real crawl has millions of links and a sliver of them with a fragment: a scan of
        // `links` would be the whole cost of the rule.
        let s = sitio_con_guia();
        let sql = format!("EXPLAIN QUERY PLAN {}", fragment_links_sql());
        let plan: Vec<String> = s
            .0
            .prepare(&sql)
            .expect("prepare")
            .query_map([], |r| r.get::<_, String>(3))
            .expect("plan")
            .collect::<rusqlite::Result<_>>()
            .expect("read");
        assert!(plan.iter().any(|p| p.contains("idx_links_fragment")), "{plan:?}");
    }
}
