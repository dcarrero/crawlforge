//! `HREFLANG` — internationalisation. `docs/04-CATALOGO-REGLAS.md §8`.
//!
//! A high-value block: multilingual sites — including the one-domain-per-language kind — are
//! plentiful, and today nobody audits their hreflang without paying for a licence.
//!
//! # This module's doctrine: zero false positives
//!
//! One wrong hreflang warning makes the user distrust the whole tool, because hreflang is
//! precisely the corner of technical SEO where the fewest people trust their own judgement.
//! So on every doubtful call, this module **stays quiet**:
//!
//! - Codes are validated against explicit lists, and legitimate but uncommon forms are accepted
//!   (script subtag, M.49 macroregion, historical ISO 639-1 aliases).
//! - URL comparisons tolerate `index.html` and the trailing slash, which are the same page.
//! - If an hreflang target was not crawled, nothing is said about it: it may be a domain outside
//!   the crawled set — the one-domain-per-language case — and we do not have its HTML.
//!
//! # About the `href` that arrives in the context
//!
//! [`PageContext::hreflang`] hands over the `href` **as it came in the HTML**: the engine does
//! not resolve `link rel=alternate` to absolute the way it does the canonical. Google requires
//! full URLs in hreflang, so on a well-built site they are already absolute, but the rules here
//! resolve relative ones on their own — `resolve_href` — to avoid inventing findings on sites
//! that use relative paths.

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::{Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

pub static HREFLANG_NO_SELF: RuleMeta = RuleMeta {
    id: "HREFLANG-NO-SELF",
    severity: Severity::High,
    category: Category::Hreflang,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Hreflang sin autorreferencia",
    name_en: "Hreflang without self-reference",
    desc_es: "La página declara alternativas de idioma pero ninguna apunta a ella misma. Google \
              exige que cada página del conjunto se incluya a sí misma; si falta, descarta el \
              grupo entero y ninguna de las traducciones se beneficia de las demás.",
    desc_en: "The page declares language alternates but none points to itself. Google requires \
              every page in the set to include itself; when that link is missing it discards the \
              whole group and none of the translations benefit from the others.",
    references: &[],
};

pub static HREFLANG_NOT_RECIPROCAL: RuleMeta = RuleMeta {
    id: "HREFLANG-NOT-RECIPROCAL",
    severity: Severity::High,
    category: Category::Hreflang,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Hreflang sin reciprocidad",
    name_en: "Hreflang not reciprocal",
    desc_es: "Esta página declara una alternativa de idioma que no la declara de vuelta. Las \
              anotaciones hreflang son votos que solo cuentan si son mutuos: Google ignora la \
              relación entera cuando uno de los dos lados no confirma al otro.",
    desc_en: "This page declares a language alternate that does not declare it back. Hreflang \
              annotations are votes that only count when they are mutual: Google ignores the \
              whole relationship when one of the two sides does not confirm the other.",
    references: &[],
};

pub static HREFLANG_INVALID_CODE: RuleMeta = RuleMeta {
    id: "HREFLANG-INVALID-CODE",
    severity: Severity::High,
    category: Category::Hreflang,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Código hreflang no válido",
    name_en: "Invalid hreflang code",
    desc_es: "El valor del atributo no es un idioma ISO 639-1 con región ISO 3166-1 opcional ni \
              el especial x-default. Google descarta en silencio la anotación mal escrita, así \
              que el conjunto queda incompleto sin que nada lo avise: es el fallo más frecuente \
              y el más difícil de ver a ojo.",
    desc_en: "The attribute value is neither an ISO 639-1 language with an optional ISO 3166-1 \
              region nor the special x-default. Google silently discards a malformed annotation, \
              so the set is left incomplete with no warning: it is the most frequent failure and \
              the hardest one to spot by eye.",
    references: &[],
};

pub static HREFLANG_TO_4XX: RuleMeta = RuleMeta {
    id: "HREFLANG-TO-4XX",
    severity: Severity::Critical,
    category: Category::Hreflang,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Hreflang a URL con error",
    name_en: "Hreflang to error URL",
    desc_es: "Una alternativa de idioma apunta a una URL que devuelve 4xx. La traducción que la \
              página promete no existe: el visitante que cambia de idioma aterriza en un error y \
              Google saca del conjunto a la página que apunta ahí.",
    desc_en: "A language alternate points to a URL that returns 4xx. The translation the page \
              promises does not exist: a visitor switching language lands on an error page and \
              Google drops the referring page from the set.",
    references: &[],
};

// ---------------------------------------------------------------------------------------------
// Code validation
// ---------------------------------------------------------------------------------------------

/// ISO 639-1 (alpha-2) language codes. This is the standard's complete two-letter set, which is
/// exactly what Google documents for hreflang.
///
/// **Does not include** three-letter ISO 639-2/639-3 codes beyond the short list in
/// [`LANGUAGES_3`]: accepting any three-letter code would let most real-world typos through,
/// which is exactly what this rule is hunting.
const LANGUAGES: &[&str] = &[
    "aa", "ab", "ae", "af", "ak", "am", "an", "ar", "as", "av", "ay", "az", //
    "ba", "be", "bg", "bh", "bi", "bm", "bn", "bo", "br", "bs", //
    "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv", "cy", //
    "da", "de", "dv", "dz", //
    "ee", "el", "en", "eo", "es", "et", "eu", //
    "fa", "ff", "fi", "fj", "fo", "fr", "fy", //
    "ga", "gd", "gl", "gn", "gu", "gv", //
    "ha", "he", "hi", "ho", "hr", "ht", "hu", "hy", "hz", //
    "ia", "id", "ie", "ig", "ii", "ik", "io", "is", "it", "iu", //
    "ja", "jv", //
    "ka", "kg", "ki", "kj", "kk", "kl", "km", "kn", "ko", "kr", "ks", "ku", "kv", "kw", "ky", //
    "la", "lb", "lg", "li", "ln", "lo", "lt", "lu", "lv", //
    "mg", "mh", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", //
    "na", "nb", "nd", "ne", "ng", "nl", "nn", "no", "nr", "nv", "ny", //
    "oc", "oj", "om", "or", "os", //
    "pa", "pi", "pl", "ps", "pt", //
    "qu", //
    "rm", "rn", "ro", "ru", "rw", //
    "sa", "sc", "sd", "se", "sg", "si", "sk", "sl", "sm", "sn", "so", "sq", "sr", "ss", "st", //
    "su", "sv", "sw", //
    "ta", "te", "tg", "th", "ti", "tk", "tl", "tn", "to", "tr", "ts", "tt", "tw", "ty", //
    "ug", "uk", "ur", "uz", //
    "ve", "vi", "vo", //
    "wa", "wo", //
    "xh", //
    "yi", "yo", //
    "za", "zh", "zu",
];

/// Historical ISO 639-1 aliases, withdrawn from the standard but still emitted by old CMSs and
/// by Java libraries. Accepted **on purpose**: Google interprets them, so flagging them would be
/// a false positive, even though the modern form is preferable.
///
/// `in` → `id`, `iw` → `he`, `ji` → `yi`, `jw` → `jv`, `sh` → `sr`/`hr`, `mo` → `ro`.
const LANGUAGES_DEPRECATED: &[&str] = &["in", "iw", "ji", "jw", "sh", "mo"];

/// Three-letter languages without an ISO 639-1 code that do show up in real hreflang sets.
/// Deliberately short list: only what is actually seen in production.
const LANGUAGES_3: &[&str] =
    &["fil", "haw", "ceb", "yue", "nds", "gsw", "ast", "arn", "hmn", "quz", "cnr"];

/// **Officially assigned** ISO 3166-1 alpha-2 region codes.
///
/// Deliberately excludes the "exceptionally reserved" and private-use ones, including the two
/// that sneak into hreflang the most: `UK` (the correct code is `GB`) and `EU` (not a country;
/// what exists for "the rest of Europe" is `x-default`). Google only honours the assigned codes,
/// so flagging those two is not a false positive — it is the finding.
const REGIONS: &[&str] = &[
    "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX",
    "AZ", //
    "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ", "BR",
    "BS", "BT", "BV", "BW", "BY", "BZ", //
    "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK", "CL", "CM", "CN", "CO", "CR", "CU", "CV",
    "CW", "CX", "CY", "CZ", //
    "DE", "DJ", "DK", "DM", "DO", "DZ", //
    "EC", "EE", "EG", "EH", "ER", "ES", "ET", //
    "FI", "FJ", "FK", "FM", "FO", "FR", //
    "GA", "GB", "GD", "GE", "GF", "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GS",
    "GT", "GU", "GW", "GY", //
    "HK", "HM", "HN", "HR", "HT", "HU", //
    "ID", "IE", "IL", "IM", "IN", "IO", "IQ", "IR", "IS", "IT", //
    "JE", "JM", "JO", "JP", //
    "KE", "KG", "KH", "KI", "KM", "KN", "KP", "KR", "KW", "KY", "KZ", //
    "LA", "LB", "LC", "LI", "LK", "LR", "LS", "LT", "LU", "LV", "LY", //
    "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK", "ML", "MM", "MN", "MO", "MP", "MQ", "MR",
    "MS", "MT", "MU", "MV", "MW", "MX", "MY", "MZ", //
    "NA", "NC", "NE", "NF", "NG", "NI", "NL", "NO", "NP", "NR", "NU", "NZ", //
    "OM", //
    "PA", "PE", "PF", "PG", "PH", "PK", "PL", "PM", "PN", "PR", "PS", "PT", "PW", "PY", //
    "QA", //
    "RE", "RO", "RS", "RU", "RW", //
    "SA", "SB", "SC", "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR",
    "SS", "ST", "SV", "SX", "SY", "SZ", //
    "TC", "TD", "TF", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO", "TR", "TT", "TV", "TW",
    "TZ", //
    "UA", "UG", "UM", "US", "UY", "UZ", //
    "VA", "VC", "VE", "VG", "VI", "VN", "VU", //
    "WF", "WS", //
    "YE", "YT", //
    "ZA", "ZM", "ZW",
];

/// UN M.49 macroregions admitted by BCP 47 as region subtags.
///
/// The one that truly matters is `419`, Latin America: `es-419` is valid and widely used, and
/// calling it wrong would be this rule's most expensive false positive. The rest of the M.49
/// geographic set is included for coherence, not because it comes up often.
const REGIONS_M49: &[&str] = &[
    "001", "002", "003", "005", "009", "011", "013", "014", "015", "017", "018", "019", "021",
    "029", "030", "034", "035", "039", "053", "054", "057", "061", "142", "143", "145", "150",
    "151", "154", "155", "202", "419",
];

/// ISO 15924 script subtags that appear in hreflang.
///
/// Accepted because Google interprets `zh-Hant`, `zh-Hans` and `sr-Latn`, and flagging them
/// would be a false positive. Not the standard's full list: these are the scripts with an actual
/// presence on real websites.
const SCRIPTS: &[&str] = &[
    "Latn", "Cyrl", "Hans", "Hant", "Hani", "Arab", "Hebr", "Grek", "Deva", "Jpan", "Kana",
    "Hira", "Kore", "Hang", "Thai", "Armn", "Geor", "Beng", "Guru", "Gujr", "Orya", "Taml",
    "Telu", "Knda", "Mlym", "Sinh", "Mymr", "Khmr", "Laoo", "Tibt", "Ethi", "Cans", "Cher",
    "Mong", "Tfng", "Syrc", "Thaa", "Nkoo", "Adlm", "Vaii", "Bopo", "Brai",
];

/// Common language mix-ups: the country code gets used where the language code belongs. Only
/// consulted once the code has already been ruled invalid.
const LANGUAGE_HINTS: &[(&str, &str)] = &[
    ("sp", "es"),
    ("jp", "ja"),
    ("cn", "zh"),
    ("gr", "el"),
    ("cz", "cs"),
    ("dk", "da"),
    ("ua", "uk"),
    ("ge", "ka"),
    ("ir", "fa"),
    ("il", "he"),
];

/// Common region mix-ups.
const REGION_HINTS: &[(&str, &str)] = &[("UK", "GB"), ("EU", "x-default"), ("EN", "GB")];

/// Why a code is invalid, and what was probably meant if it can be guessed.
///
/// `reason` is a stable identifier in English: the UI translates it, and the crawl-to-crawl diff
/// compares it. It is not user-facing text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeProblem {
    pub reason: &'static str,
    pub suggestion: Option<String>,
}

impl CodeProblem {
    fn new(reason: &'static str) -> Self {
        Self { reason, suggestion: None }
    }

    fn with(reason: &'static str, suggestion: impl Into<String>) -> Self {
        Self { reason, suggestion: Some(suggestion.into()) }
    }
}

/// Validates an `hreflang` value. `None` means "correct".
///
/// Accepts `x-default` and `language[-Script][-REGION]`, case-insensitively — BCP 47 is
/// case-insensitive, so `ES-es` is valid even though the canonical form is `es-ES`.
pub fn check_code(code: &str) -> Option<CodeProblem> {
    let bruto = code.trim();
    if bruto.is_empty() {
        return Some(CodeProblem::new("empty"));
    }

    // The underscore is the classic separator mistake of CMSs that reuse PHP's or Java's
    // "locale" (`es_ES`). It is detected before anything else because the suggestion is obvious
    // and because, otherwise, the code would look like one single very odd subtag.
    if bruto.contains('_') {
        let corregido = bruto.replace('_', "-");
        return Some(if check_code(&corregido).is_none() {
            CodeProblem::with("separator", canonical_form(&corregido))
        } else {
            CodeProblem::new("separator")
        });
    }

    if bruto.eq_ignore_ascii_case("x-default") {
        return None;
    }

    // Any other private-use tag (`x-*`, `i-*`) means nothing to Google in hreflang.
    let minuscula = bruto.to_ascii_lowercase();
    if minuscula.starts_with("x-") || minuscula == "x" {
        return Some(CodeProblem::with("private_use", "x-default"));
    }

    let partes: Vec<&str> = bruto.split('-').collect();
    if partes.len() > 3 {
        return Some(CodeProblem::new("structure"));
    }

    // --- Language ---
    let idioma = partes[0].to_ascii_lowercase();
    let idioma_valido = LANGUAGES.contains(&idioma.as_str())
        || LANGUAGES_DEPRECATED.contains(&idioma.as_str())
        || LANGUAGES_3.contains(&idioma.as_str());
    if !idioma_valido {
        let pista = LANGUAGE_HINTS.iter().find(|(mal, _)| *mal == idioma).map(|(_, bien)| *bien);
        return Some(match pista {
            Some(bien) => CodeProblem::with("language", sustituir_idioma(&partes, bien)),
            None => CodeProblem::new("language"),
        });
    }

    // --- Script (optional) and region (optional) ---
    let resto = &partes[1..];
    let region = match resto {
        [] => None,
        [uno] if es_escritura(uno) => None,
        [uno] => Some(*uno),
        [escritura, r] if es_escritura(escritura) => Some(*r),
        // Two subtags where the first is not a script: `es-ES-valencia` would be a legitimate
        // BCP 47 variant, but Google does not use it in hreflang, and mistaking it for a region
        // would produce an incomprehensible warning. Flagged as structure.
        _ => return Some(CodeProblem::new("structure")),
    };

    // Without a region subtag the code is already fine: `es` and `zh-Hant` are valid.
    region.and_then(|region| check_region(&idioma, region))
}

/// Validates the region subtag of a code whose language has already been accepted.
fn check_region(idioma: &str, region: &str) -> Option<CodeProblem> {
    let mayuscula = region.to_ascii_uppercase();
    if REGIONS.contains(&mayuscula.as_str()) || REGIONS_M49.contains(&region) {
        return None;
    }

    let pista = REGION_HINTS.iter().find(|(mal, _)| *mal == mayuscula).map(|(_, bien)| *bien);
    Some(match pista {
        Some("x-default") => CodeProblem::with("region", "x-default"),
        Some(bien) => CodeProblem::with("region", format!("{idioma}-{bien}")),
        None => CodeProblem::new("region"),
    })
}

fn es_escritura(sub: &str) -> bool {
    sub.len() == 4 && SCRIPTS.iter().any(|s| s.eq_ignore_ascii_case(sub))
}

/// Replaces the language subtag while keeping the rest, for the suggestion.
fn sustituir_idioma(partes: &[&str], idioma: &str) -> String {
    let mut salida = String::from(idioma);
    for parte in &partes[1..] {
        salida.push('-');
        salida.push_str(parte);
    }
    canonical_form(&salida)
}

/// BCP 47 canonical form: language lowercase, script capitalised, region uppercase. Used only
/// for suggestions — validation never depends on case.
fn canonical_form(code: &str) -> String {
    let mut salida = String::with_capacity(code.len());
    for (i, parte) in code.split('-').enumerate() {
        if i > 0 {
            salida.push('-');
        }
        if i == 0 {
            salida.push_str(&parte.to_ascii_lowercase());
        } else if es_escritura(parte) {
            let mut chars = parte.chars();
            if let Some(primera) = chars.next() {
                salida.extend(primera.to_uppercase());
                salida.push_str(&chars.as_str().to_ascii_lowercase());
            }
        } else {
            salida.push_str(&parte.to_ascii_uppercase());
        }
    }
    salida
}

// ---------------------------------------------------------------------------------------------
// URL comparison
// ---------------------------------------------------------------------------------------------

/// Resolves a possibly relative `href` against the page URL.
///
/// A minimal resolver, deliberately so: this crate does not depend on `url` — it does not know
/// the engine — and its only job is making sure a relative hreflang does not produce false
/// findings. Returns `None` when there is nothing to compare (empty href, or a base without a
/// scheme).
fn resolve_href(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }

    let (esquema_base, resto_base) = base.split_once("://")?;
    let (autoridad, ruta_base) = match resto_base.split_once('/') {
        Some((a, r)) => (a, format!("/{r}")),
        None => (resto_base, String::from("/")),
    };

    // Absolute with its own scheme: returned as is. This also covers `mailto:` and friends,
    // which will simply never match any page.
    if let Some((posible_esquema, _)) = href.split_once("://") {
        let parece_esquema = !posible_esquema.is_empty()
            && posible_esquema
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if parece_esquema {
            return Some(href.to_string());
        }
    }

    if let Some(sin_barras) = href.strip_prefix("//") {
        return Some(format!("{esquema_base}://{sin_barras}"));
    }
    if href.starts_with('#') {
        return Some(base.split('#').next().unwrap_or(base).to_string());
    }
    if let Some(ruta) = href.strip_prefix('/') {
        return Some(format!("{esquema_base}://{autoridad}/{ruta}"));
    }
    if href.starts_with('?') {
        let sin_query = ruta_base.split('?').next().unwrap_or(&ruta_base);
        return Some(format!("{esquema_base}://{autoridad}{sin_query}{href}"));
    }

    // Relative to the page's directory.
    let directorio = {
        let sin_query = ruta_base.split(['?', '#']).next().unwrap_or("/");
        match sin_query.rfind('/') {
            Some(i) => &sin_query[..=i],
            None => "/",
        }
    };
    let unida = format!("{directorio}{href}");
    Some(format!("{esquema_base}://{autoridad}{}", colapsar_puntos(&unida)))
}

/// Resolves `.` and `..` in an already-joined path.
fn colapsar_puntos(ruta: &str) -> String {
    let (camino, cola) = match ruta.find(['?', '#']) {
        Some(i) => (&ruta[..i], &ruta[i..]),
        None => (ruta, ""),
    };
    let acaba_en_barra = camino.ends_with('/');
    let mut pila: Vec<&str> = Vec::new();
    for segmento in camino.split('/') {
        match segmento {
            "" | "." => {}
            ".." => {
                pila.pop();
            }
            otro => pila.push(otro),
        }
    }
    let mut salida = String::from("/");
    salida.push_str(&pila.join("/"));
    if acaba_en_barra && !salida.ends_with('/') {
        salida.push('/');
    }
    salida.push_str(cola);
    salida
}

/// Comparison key for two URLs that designate the same page.
///
/// Equates what any static server serves identically: the fragment does not count, scheme and
/// host are case-insensitive, `/a/index.html` is `/a/` and `/a` is `/a/`. Tolerating these three
/// forms avoids the dumbest false positive of all — "no self-reference" on a page whose hreflang
/// points at itself spelled another way — at the price of not distinguishing two URLs that some
/// exotic server might serve differently. That is the right trade: here a false negative costs
/// far less than a false positive.
fn url_key(url: &str) -> String {
    let sin_fragmento = url.trim().split('#').next().unwrap_or("").trim();
    let (esquema, resto) = match sin_fragmento.split_once("://") {
        Some((e, r)) => (e.to_ascii_lowercase(), r),
        None => return sin_fragmento.to_ascii_lowercase(),
    };
    let (autoridad, resto) = match resto.split_once('/') {
        Some((a, r)) => (a.to_ascii_lowercase(), format!("/{r}")),
        None => (resto.to_ascii_lowercase(), String::from("/")),
    };
    let (ruta, query) = match resto.split_once('?') {
        Some((p, q)) => (p.to_string(), format!("?{q}")),
        None => (resto, String::new()),
    };

    let ruta = ruta
        .strip_suffix("index.html")
        .or_else(|| ruta.strip_suffix("index.htm"))
        .unwrap_or(&ruta)
        .trim_end_matches('/')
        .to_string();

    format!("{esquema}://{autoridad}{ruta}{query}")
}

// ---------------------------------------------------------------------------------------------
// HREFLANG-NO-SELF
// ---------------------------------------------------------------------------------------------

/// Hreflang set that does not include itself.
///
/// Any code — `x-default` included — whose target is the page's own URL **or its canonical**
/// counts as a self-reference: the pattern `/a?utm=x` with canonical `/a` and hreflang to `/a`
/// is correct, and flagging it would be a false positive.
pub struct HreflangNoSelf;

impl PageRule for HreflangNoSelf {
    fn meta(&self) -> &'static RuleMeta {
        &HREFLANG_NO_SELF
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable || ctx.hreflang.is_empty() {
            return Vec::new();
        }

        let mut propias = HashSet::new();
        propias.insert(url_key(ctx.url));
        if let Some(canonical) = ctx.canonical.filter(|c| !c.trim().is_empty()) {
            if let Some(absoluto) = resolve_href(ctx.url, canonical) {
                propias.insert(url_key(&absoluto));
            }
        }

        let se_referencia = ctx.hreflang.iter().any(|(_, href)| {
            resolve_href(ctx.url, href).map(|a| propias.contains(&url_key(&a))).unwrap_or(false)
        });
        if se_referencia {
            return Vec::new();
        }

        let codigos: Vec<&str> = ctx.hreflang.iter().map(|(codigo, _)| *codigo).collect();
        vec![Issue::new(&HREFLANG_NO_SELF).with_detail(serde_json::json!({
            "alternates": codigos.len(),
            "codes": codigos,
        }))]
    }
}

// ---------------------------------------------------------------------------------------------
// HREFLANG-INVALID-CODE
// ---------------------------------------------------------------------------------------------

/// An `hreflang` value that is not a valid code.
///
/// One finding per wrong code, grouped by the code: on a template-generated site the same
/// `es_ES` comes out on thousands of pages and the UI has to be able to say so in one line.
pub struct HreflangInvalidCode;

impl PageRule for HreflangInvalidCode {
    fn meta(&self) -> &'static RuleMeta {
        &HREFLANG_INVALID_CODE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // The 2xx cuts off the error template: no search engine processes the hreflang in a
        // 404's HTML, and without the gate every broken URL would repeat the template's defect.
        // See `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() || ctx.hreflang.is_empty() {
            return Vec::new();
        }

        let mut vistos = HashSet::new();
        let mut salida = Vec::new();
        for (codigo, href) in ctx.hreflang {
            let Some(problema) = check_code(codigo) else {
                continue;
            };
            if !vistos.insert(codigo.trim().to_ascii_lowercase()) {
                continue;
            }
            salida.push(
                Issue::new(&HREFLANG_INVALID_CODE)
                    .with_detail(serde_json::json!({
                        "code": codigo,
                        "reason": problema.reason,
                        "suggestion": problema.suggestion,
                        "href": href,
                    }))
                    .with_group(format!("hreflang-code:{}", codigo.trim().to_ascii_lowercase())),
            );
        }
        salida
    }
}

// ---------------------------------------------------------------------------------------------
// Reading the hreflang set out of the store
// ---------------------------------------------------------------------------------------------

/// A crawled page with its hreflang set already resolved to absolute.
struct AlternateSet {
    url_hash: i64,
    url: String,
    is_indexable: bool,
    /// Comparison keys of the page itself: its URL and its canonical.
    propias: HashSet<String>,
    /// `(code, absolute URL, key)` for each declared alternate.
    targets: Vec<(String, String, String)>,
}

/// Reads from `pages` the pages that declare hreflang.
///
/// `hreflang_json` is serialised by `engine.rs` as `[[code, href], …]` with the `href` **as it
/// came in the HTML**, so it has to be resolved here. Pure SQL cannot cross-reference that; it
/// is read and crossed in Rust, which moreover only loads the crawl's multilingual subset.
fn leer_conjuntos(conn: &Connection) -> rusqlite::Result<Vec<AlternateSet>> {
    let mut stmt = conn.prepare(
        "SELECT u.url_hash, u.url, p.canonical, p.is_indexable, p.hreflang_json
         FROM pages p
         JOIN urls u ON u.id = p.url_id
         WHERE p.hreflang_json IS NOT NULL AND TRIM(p.hreflang_json) <> ''",
    )?;

    let filas = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, i64>(3)? != 0,
            r.get::<_, String>(4)?,
        ))
    })?;

    let mut salida = Vec::new();
    for fila in filas {
        let (url_hash, url, canonical, is_indexable, json) = fila?;

        // Unreadable JSON means a crawl from another version or a hand-edited file. Not an audit
        // rule's business: skip that page instead of breaking the whole pass.
        let Ok(pares) = serde_json::from_str::<Vec<(String, String)>>(&json) else {
            continue;
        };

        let mut propias = HashSet::new();
        propias.insert(url_key(&url));
        if let Some(canonical) = canonical.as_deref().filter(|c| !c.trim().is_empty()) {
            if let Some(absoluto) = resolve_href(&url, canonical) {
                propias.insert(url_key(&absoluto));
            }
        }

        let targets = pares
            .into_iter()
            .filter_map(|(codigo, href)| {
                let absoluto = resolve_href(&url, &href)?;
                let clave = url_key(&absoluto);
                Some((codigo, absoluto, clave))
            })
            .collect();

        salida.push(AlternateSet { url_hash, url, is_indexable, propias, targets });
    }
    Ok(salida)
}

// ---------------------------------------------------------------------------------------------
// HREFLANG-NOT-RECIPROCAL
// ---------------------------------------------------------------------------------------------

/// A declares B and B does not declare A back.
///
/// Only targets that **were crawled and are HTML** get judged: about a set member domain that
/// did not enter the crawl — one domain declaring its sibling — nothing is known, and assuming
/// it broken would be the most expensive false positive in the whole block.
pub struct HreflangNotReciprocal;

impl SiteRule for HreflangNotReciprocal {
    fn meta(&self) -> &'static RuleMeta {
        &HREFLANG_NOT_RECIPROCAL
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let conjuntos = leer_conjuntos(conn)?;

        // Index by URL key and canonical key. The first one wins: if two pages share a key, the
        // ambiguity resolves silently towards the one that was read first.
        let mut por_clave: HashMap<&str, usize> = HashMap::new();
        for (i, conjunto) in conjuntos.iter().enumerate() {
            for clave in &conjunto.propias {
                por_clave.entry(clave.as_str()).or_insert(i);
            }
        }

        // The page exists but declares no alternates at all: checked with an exact query so the
        // whole crawl's pages never have to be loaded into memory.
        let mut existe = conn.prepare(
            "SELECT 1 FROM pages p JOIN urls u ON u.id = p.url_id WHERE u.url = ?1 LIMIT 1",
        )?;

        let mut salida = Vec::new();
        for origen in conjuntos.iter().filter(|c| c.is_indexable) {
            let mut avisados = HashSet::new();
            for (codigo, absoluto, clave) in &origen.targets {
                if origen.propias.contains(clave) {
                    continue;
                }
                let razon = match por_clave.get(clave.as_str()) {
                    Some(&j) => {
                        let destino = &conjuntos[j];
                        if destino.targets.iter().any(|(_, _, k)| origen.propias.contains(k)) {
                            continue;
                        }
                        "target_omits_page"
                    }
                    None => {
                        let rastreada =
                            existe.exists(rusqlite::params![absoluto])?;
                        if !rastreada {
                            continue;
                        }
                        "target_declares_no_alternates"
                    }
                };
                if !avisados.insert(clave.clone()) {
                    continue;
                }
                salida.push((
                    Some(origen.url_hash),
                    Issue::new(&HREFLANG_NOT_RECIPROCAL)
                        .with_detail(serde_json::json!({
                            "code": codigo,
                            "target": absoluto,
                            "reason": razon,
                        }))
                        .with_group(clave_de_par(&origen.url, absoluto)),
                ));
            }
        }
        Ok(salida)
    }
}

/// Grouping key for a pair of pages, independent of direction: both directions of the same
/// failure are a single problem for the user.
fn clave_de_par(a: &str, b: &str) -> String {
    let (uno, dos) = if url_key(a) <= url_key(b) { (a, b) } else { (b, a) };
    let mezcla = format!("{}|{}", url_key(uno), url_key(dos));
    format!("hreflang-pair:{:016x}", xxhash_rust::xxh3::xxh3_64(mezcla.as_bytes()))
}

// ---------------------------------------------------------------------------------------------
// HREFLANG-TO-4XX
// ---------------------------------------------------------------------------------------------

/// A language alternate points at a URL that returned 4xx.
///
/// The target is looked up by **exact** match on `urls.url`, not by the tolerant key: the claim
/// here is "that URL returns an error", and it can only be sustained about the URL that was
/// actually requested. 5xx are left out on purpose — they tend to be transient and the rule's ID
/// names 4xx — and the finding is recorded on the page declaring the hreflang, which is where
/// the line to fix lives.
///
/// **A cross-domain target only asserts on 404/410** (see [`crate::sql_external_gone`]): the
/// hreflang between two of your own domains is the textbook multi-domain setup, its target got
/// the bot `HEAD` probe, and a wall's 401/403/429 to that probe would otherwise escalate to a
/// `critical` about a page every browser opens fine. Same criterion as `HTTP-404-EXTERNAL`.
pub struct HreflangTo4xx;

impl SiteRule for HreflangTo4xx {
    fn meta(&self) -> &'static RuleMeta {
        &HREFLANG_TO_4XX
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let conjuntos = leer_conjuntos(conn)?;
        let sql = format!(
            "SELECT status_code FROM urls
             WHERE url = ?1
               AND ((is_internal = 1 AND status_code >= 400 AND status_code < 500)
                 OR (is_internal = 0 AND {externa_rota}))
             LIMIT 1",
            externa_rota = crate::sql_external_gone("status_code"),
        );
        let mut estado = conn.prepare(&sql)?;

        let mut salida = Vec::new();
        for origen in &conjuntos {
            let mut avisados = HashSet::new();
            for (codigo, absoluto, clave) in &origen.targets {
                let status: Option<i64> = estado
                    .query_row(rusqlite::params![absoluto], |r| r.get::<_, i64>(0))
                    .optional()?;
                let Some(status) = status else {
                    continue;
                };
                if !avisados.insert(clave.clone()) {
                    continue;
                }
                salida.push((
                    Some(origen.url_hash),
                    Issue::new(&HREFLANG_TO_4XX).with_detail(serde_json::json!({
                        "code": codigo,
                        "target": absoluto,
                        "status_code": status,
                    })),
                ));
            }
        }
        Ok(salida)
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![Box::new(HreflangNoSelf), Box::new(HreflangInvalidCode)]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    vec![Box::new(HreflangNotReciprocal), Box::new(HreflangTo4xx)]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------------------- codes

    #[test]
    fn accepts_the_ordinary_codes() {
        for codigo in ["es", "en", "es-ES", "en-GB", "pt-BR", "zh-CN", "x-default"] {
            assert!(check_code(codigo).is_none(), "{codigo} should be valid");
        }
    }

    #[test]
    fn is_case_insensitive() {
        // BCP 47 is case-insensitive: `ES-es` is valid even though the canonical form is
        // `es-ES`.
        for codigo in ["ES", "ES-es", "es-es", "X-Default"] {
            assert!(check_code(codigo).is_none(), "{codigo} should be valid");
        }
    }

    #[test]
    fn accepts_the_latin_america_macroregion() {
        // `es-419` is valid and widely used: calling it wrong would be the most expensive false
        // positive.
        assert!(check_code("es-419").is_none());
        assert!(check_code("es-150").is_none());
    }

    #[test]
    fn accepts_the_script_subtag() {
        for codigo in ["zh-Hant", "zh-Hans", "zh-Hant-TW", "sr-Latn-RS"] {
            assert!(check_code(codigo).is_none(), "{codigo} should be valid");
        }
    }

    #[test]
    fn accepts_the_historical_aliases_of_the_standard() {
        // `iw` for `he` is still emitted by old libraries and Google interprets it.
        for codigo in ["iw", "in", "sh"] {
            assert!(check_code(codigo).is_none(), "{codigo} should be accepted");
        }
    }

    #[test]
    fn detects_the_locale_underscore() {
        let problema = check_code("es_ES").expect("es_ES is not a valid code");
        assert_eq!(problema.reason, "separator");
        assert_eq!(problema.suggestion.as_deref(), Some("es-ES"));
    }

    #[test]
    fn detects_a_country_code_used_as_a_language() {
        let problema = check_code("sp").expect("sp is not a language");
        assert_eq!(problema.reason, "language");
        assert_eq!(problema.suggestion.as_deref(), Some("es"));

        let problema = check_code("jp-JP").expect("jp is not a language");
        assert_eq!(problema.suggestion.as_deref(), Some("ja-JP"));
    }

    #[test]
    fn detects_the_nonexistent_region() {
        // `UK` is reserved in the ISO standard but not assigned: the correct code is `GB`.
        let problema = check_code("en-UK").expect("UK is not an assigned region");
        assert_eq!(problema.reason, "region");
        assert_eq!(problema.suggestion.as_deref(), Some("en-GB"));

        // `EU` is not a country; what exists for "the rest" is x-default.
        let problema = check_code("en-EU").expect("EU is not an assigned region");
        assert_eq!(problema.suggestion.as_deref(), Some("x-default"));
    }

    #[test]
    fn rejects_what_is_not_shaped_like_a_code() {
        for codigo in ["", "   ", "español", "es-ES-MX-AR", "x-es"] {
            assert!(check_code(codigo).is_some(), "{codigo} should not be valid");
        }
    }

    // ----------------------------------------------------------------------------- URL

    #[test]
    fn resolves_relative_hrefs() {
        let base = "https://ejemplo.es/es/pagina/";
        assert_eq!(
            resolve_href(base, "https://otro.es/a").as_deref(),
            Some("https://otro.es/a")
        );
        assert_eq!(resolve_href(base, "//otro.es/a").as_deref(), Some("https://otro.es/a"));
        assert_eq!(resolve_href(base, "/en/").as_deref(), Some("https://ejemplo.es/en/"));
        assert_eq!(
            resolve_href(base, "../en/").as_deref(),
            Some("https://ejemplo.es/es/en/")
        );
        assert_eq!(
            resolve_href(base, "hija/").as_deref(),
            Some("https://ejemplo.es/es/pagina/hija/")
        );
        assert!(resolve_href(base, "  ").is_none());
    }

    #[test]
    fn the_url_key_equates_the_spellings_of_the_same_page() {
        let esperada = url_key("https://ejemplo.es/es/");
        for forma in [
            "https://ejemplo.es/es",
            "https://ejemplo.es/es/",
            "https://ejemplo.es/es/index.html",
            "https://EJEMPLO.es/es/#top",
            "HTTPS://ejemplo.es/es/",
        ] {
            assert_eq!(url_key(forma), esperada, "{forma}");
        }
        assert_ne!(url_key("https://ejemplo.es/es/"), url_key("https://ejemplo.es/en/"));
        assert_ne!(url_key("https://ejemplo.es/a"), url_key("https://ejemplo.es/a?p=1"));
    }

    // ----------------------------------------------------------------------------- NO-SELF

    fn ctx<'a>(hreflang: &'a [(&'a str, &'a str)]) -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/es/");
        c.canonical = Some("https://ejemplo.es/es/");
        c.canonical_raw = Some("https://ejemplo.es/es/");
        c.canonical_count = 1;
        c.hreflang = hreflang;
        c
    }

    #[test]
    fn no_self_stays_quiet_when_the_set_includes_itself() {
        let c = ctx(&[
            ("es", "https://ejemplo.es/es/"),
            ("en", "https://ejemplo.es/en/"),
        ]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_reports_a_missing_self_reference() {
        let c = ctx(&[("en", "https://ejemplo.es/en/"), ("fr", "https://ejemplo.es/fr/")]);
        let issues = HreflangNoSelf.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HREFLANG-NO-SELF");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn no_self_accepts_a_relative_self_reference() {
        // A relative href to itself is correct: resolving it wrong would yield a false warning.
        let c = ctx(&[("es", "/es/"), ("en", "/en/")]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_accepts_a_self_reference_spelled_differently() {
        let c = ctx(&[("es", "https://ejemplo.es/es/index.html"), ("en", "/en/")]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_accepts_the_set_pointing_at_the_canonical() {
        let mut c = ctx(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        c.url = "https://ejemplo.es/es/?utm_source=x";
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_counts_x_default_as_a_self_reference() {
        let c = ctx(&[("x-default", "https://ejemplo.es/es/"), ("en", "/en/")]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_stays_quiet_on_a_page_without_hreflang() {
        // Most pages in the world. No set, nothing to say.
        assert!(HreflangNoSelf.evaluate(&ctx(&[])).is_empty());
    }

    #[test]
    fn no_self_stays_quiet_when_not_html_or_not_indexable() {
        let sin_self: &[(&str, &str)] = &[("en", "https://ejemplo.es/en/")];
        let mut c = ctx(sin_self);
        c.is_indexable = false;
        assert!(HreflangNoSelf.evaluate(&c).is_empty());

        let mut c = ctx(sin_self);
        c.is_html = false;
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    // ----------------------------------------------------------------------------- INVALID-CODE

    #[test]
    fn invalid_code_stays_quiet_on_correct_codes() {
        let c = ctx(&[
            ("es", "https://ejemplo.es/es/"),
            ("en-GB", "https://ejemplo.es/en/"),
            ("es-419", "https://ejemplo.es/latam/"),
            ("x-default", "https://ejemplo.es/"),
        ]);
        assert!(HreflangInvalidCode.evaluate(&c).is_empty());
    }

    #[test]
    fn invalid_code_reports_once_per_wrong_code() {
        let c = ctx(&[
            ("es", "https://ejemplo.es/es/"),
            ("es_ES", "https://ejemplo.es/es-es/"),
            ("en-UK", "https://ejemplo.es/uk/"),
        ]);
        let issues = HreflangInvalidCode.evaluate(&c);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.rule_id == "HREFLANG-INVALID-CODE"));
        assert_eq!(issues[0].group_key.as_deref(), Some("hreflang-code:es_es"));
    }

    #[test]
    fn invalid_code_does_not_repeat_the_same_code_twice() {
        let c = ctx(&[("sp", "https://ejemplo.es/a/"), ("SP", "https://ejemplo.es/b/")]);
        assert_eq!(HreflangInvalidCode.evaluate(&c).len(), 1);
    }

    #[test]
    fn invalid_code_stays_quiet_on_a_page_without_hreflang() {
        assert!(HreflangInvalidCode.evaluate(&ctx(&[])).is_empty());
    }

    #[test]
    fn invalid_code_stays_quiet_on_something_that_is_not_html() {
        let mut c = ctx(&[("es_ES", "https://ejemplo.es/es/")]);
        c.is_html = false;
        assert!(HreflangInvalidCode.evaluate(&c).is_empty());
    }

    #[test]
    fn invalid_code_does_not_audit_the_error_template() {
        // No search engine processes the hreflang in a 404's HTML: without the 2xx gate, the
        // template's wrong code would come out once per broken URL on the site.
        for status in [301, 404, 410, 500] {
            let mut c = ctx(&[("es_ES", "https://ejemplo.es/es/")]);
            c.status = status;
            assert!(
                HreflangInvalidCode.evaluate(&c).is_empty(),
                "should not audit the HTML of a {status}"
            );
        }
    }

    // ----------------------------------------------------------------------------- store

    /// Minimal schema with the columns this module's site rules query. The real schema gets
    /// exercised by crawling the fixtures in `crawlforge-core/tests/fixtures_de_reglas.rs`:
    /// here only the cross-referencing is under test.
    fn db(paginas: &[(&str, Option<u16>, bool, Option<&str>)]) -> Connection {
        let conn = Connection::open_in_memory().expect("open in memory");
        conn.execute_batch(
            "CREATE TABLE urls (
                 id INTEGER PRIMARY KEY,
                 url TEXT NOT NULL UNIQUE,
                 url_hash INTEGER NOT NULL,
                 status_code INTEGER,
                 is_internal INTEGER NOT NULL DEFAULT 1);
             CREATE TABLE pages (
                 url_id INTEGER PRIMARY KEY REFERENCES urls(id),
                 canonical TEXT,
                 is_indexable INTEGER NOT NULL,
                 hreflang_json TEXT);",
        )
        .expect("create schema");

        for (i, (url, status, es_pagina, hreflang)) in paginas.iter().enumerate() {
            let id = i as i64 + 1;
            let hash = xxhash_rust::xxh3::xxh3_64(url.as_bytes()) as i64;
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, status_code) VALUES (?1,?2,?3,?4)",
                rusqlite::params![id, url, hash, status],
            )
            .expect("insert url");
            if *es_pagina {
                conn.execute(
                    "INSERT INTO pages (url_id, canonical, is_indexable, hreflang_json)
                     VALUES (?1,?2,1,?3)",
                    rusqlite::params![id, url, hreflang],
                )
                .expect("insert page");
            }
        }
        conn
    }

    /// `[[code, href], …]`, the exact format `engine.rs` writes.
    fn json(pares: &[(&str, &str)]) -> String {
        serde_json::to_string(&pares.iter().map(|(c, h)| (*c, *h)).collect::<Vec<_>>())
            .expect("serialise")
    }

    // ----------------------------------------------------------------------------- RECIPROCAL

    #[test]
    fn reciprocal_stays_quiet_when_both_declare_each_other() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let en = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, Some(&en)),
        ]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn reciprocal_reports_a_target_that_omits_the_page() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let en = json(&[("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, Some(&en)),
        ]);
        let hallazgos = HreflangNotReciprocal.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        let (hash, issue) = &hallazgos[0];
        assert_eq!(
            *hash,
            Some(xxhash_rust::xxh3::xxh3_64(b"https://ejemplo.es/es/") as i64),
            "the finding goes on the page that over-declares"
        );
        assert!(issue.detail_json.as_deref().unwrap_or_default().contains("target_omits_page"));
    }

    #[test]
    fn reciprocal_reports_a_target_that_declares_nothing() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, None),
        ]);
        let hallazgos = HreflangNotReciprocal.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert!(hallazgos[0]
            .1
            .detail_json
            .as_deref()
            .unwrap_or_default()
            .contains("target_declares_no_alternates"));
    }

    #[test]
    fn reciprocal_stays_quiet_on_an_uncrawled_target() {
        // The one-domain-per-language case: the other one did not enter the crawl, so we do not
        // know what it declares. Assuming it broken would be the block's worst false positive.
        let es = json(&[("es", "https://ejemplo.es/"), ("en", "https://ejemplo.me/")]);
        let conn = db(&[("https://ejemplo.es/", Some(200), true, Some(&es))]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn reciprocal_tolerates_both_sides_spelling_the_url_differently() {
        let es = json(&[("es", "/es/index.html"), ("en", "/en/")]);
        let en = json(&[("es", "https://ejemplo.es/es"), ("en", "/en/index.html")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, Some(&en)),
        ]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn reciprocal_stays_quiet_on_a_crawl_without_hreflang() {
        let conn = db(&[("https://ejemplo.es/", Some(200), true, None)]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluate").is_empty());
    }

    // ----------------------------------------------------------------------------- TO-4XX

    #[test]
    fn to_4xx_reports_a_target_that_is_a_404() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(404), false, None),
        ]);
        let hallazgos = HreflangTo4xx.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.severity, Severity::Critical);
        assert!(hallazgos[0].1.detail_json.as_deref().unwrap_or_default().contains("404"));
    }

    #[test]
    fn to_4xx_stays_quiet_when_the_target_responds() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, None),
        ]);
        assert!(HreflangTo4xx.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn to_4xx_stays_quiet_on_uncrawled_targets_and_on_5xx() {
        let es = json(&[
            ("es", "https://ejemplo.es/es/"),
            ("en", "https://ejemplo.es/en/"),
            ("fr", "https://ejemplo.es/fr/"),
        ]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", None, false, None),
            ("https://ejemplo.es/fr/", Some(503), false, None),
        ]);
        assert!(HreflangTo4xx.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn to_4xx_stays_quiet_on_a_crawl_without_hreflang() {
        let conn = db(&[("https://ejemplo.es/", Some(200), true, None)]);
        assert!(HreflangTo4xx.evaluate(&conn).expect("evaluate").is_empty());
    }

    /// Marks an already inserted URL as belonging to another host, as the external status
    /// probe leaves it.
    fn marcar_externa(conn: &Connection, url: &str) {
        let cambiadas = conn
            .execute("UPDATE urls SET is_internal = 0 WHERE url = ?1", rusqlite::params![url])
            .expect("mark external");
        assert_eq!(cambiadas, 1, "the URL to mark external must exist");
    }

    #[test]
    fn to_4xx_does_not_escalate_a_cross_domain_bot_wall() {
        // Proves the fix. The hreflang between two of your own domains is the textbook
        // multi-domain setup; the cross-domain target got the bot HEAD probe, and its wall's
        // 401/403/429 was escalating to a critical about a page every browser opens fine.
        // Same criterion as HTTP-404-EXTERNAL.
        for status in [401u16, 403, 429] {
            let es = json(&[("es", "https://ejemplo.es/"), ("en", "https://ejemplo.co.uk/")]);
            let conn = db(&[
                ("https://ejemplo.es/", Some(200), true, Some(&es)),
                ("https://ejemplo.co.uk/", Some(status), false, None),
            ]);
            marcar_externa(&conn, "https://ejemplo.co.uk/");

            assert!(
                HreflangTo4xx.evaluate(&conn).expect("evaluate").is_empty(),
                "a foreign {status} cannot back a critical about the hreflang target"
            );
        }
    }

    #[test]
    fn to_4xx_still_reports_a_cross_domain_target_that_is_gone() {
        // Guard: a 404/410 is the foreign origin itself stating the alternate is gone.
        let es = json(&[("es", "https://ejemplo.es/"), ("en", "https://ejemplo.co.uk/")]);
        let conn = db(&[
            ("https://ejemplo.es/", Some(200), true, Some(&es)),
            ("https://ejemplo.co.uk/", Some(410), false, None),
        ]);
        marcar_externa(&conn, "https://ejemplo.co.uk/");

        let hallazgos = HreflangTo4xx.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert!(hallazgos[0].1.detail_json.as_deref().unwrap_or_default().contains("410"));
    }
}
