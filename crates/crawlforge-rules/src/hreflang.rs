//! `HREFLANG` — internacionalización. `docs/04-CATALOGO-REGLAS.md §8`.
//!
//! Bloque de alto valor para sitios multiidioma con un dominio por idioma son sitios
//! multiidioma, y hoy nadie les audita el hreflang sin pagar una licencia.
//!
//! # Doctrina de este módulo: cero falsos positivos
//!
//! Un aviso equivocado sobre hreflang hace que el usuario desconfíe de toda la herramienta,
//! porque hreflang es precisamente la parte del SEO técnico en la que menos gente confía en su
//! propio criterio. Por eso, en cada decisión dudosa, este módulo **calla**:
//!
//! - Los códigos se validan contra listas explícitas y se aceptan formas legítimas poco
//!   frecuentes (subetiqueta de escritura, macrorregión M.49, alias históricos de ISO 639-1).
//! - Las comparaciones de URL toleran `index.html` y la barra final, que son la misma página.
//! - Si el destino de un hreflang no se rastreó, no se dice nada de él: puede ser un dominio
//!   distinto del conjunto —el caso de un dominio por idioma— y no tenemos su HTML.
//!
//! # Sobre el `href` que llega en el contexto
//!
//! [`PageContext::hreflang`] entrega el `href` **tal como venía en el HTML**: el motor no
//! resuelve los `link rel=alternate` a absoluto como hace con el canonical. Google exige URL
//! completas en hreflang, así que en un sitio bien montado ya son absolutas, pero las reglas de
//! aquí resuelven lo relativo por su cuenta —`resolve_href`— para no inventarse hallazgos en los
//! sitios que usan rutas relativas.

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
// Validación de códigos
// ---------------------------------------------------------------------------------------------

/// Códigos de idioma ISO 639-1 (alfa-2). Es el conjunto de dos letras completo de la norma,
/// que es exactamente lo que Google documenta para hreflang.
///
/// **No incluye** ISO 639-2/639-3 de tres letras salvo la lista corta de
/// [`LANGUAGES_3`]: aceptar cualquier código de tres letras dejaría pasar la mayoría de las
/// erratas reales, que es justo lo que la regla busca.
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

/// Alias históricos de ISO 639-1, retirados pero todavía emitidos por CMS antiguos y por
/// bibliotecas de Java. Se aceptan **a propósito**: Google los interpreta, así que avisar de
/// ellos sería un falso positivo, aunque la forma moderna sea preferible.
///
/// `in` → `id`, `iw` → `he`, `ji` → `yi`, `jw` → `jv`, `sh` → `sr`/`hr`, `mo` → `ro`.
const LANGUAGES_DEPRECATED: &[&str] = &["in", "iw", "ji", "jw", "sh", "mo"];

/// Idiomas de tres letras sin código ISO 639-1 que sí aparecen en conjuntos hreflang reales.
/// Lista deliberadamente corta: solo los que se ven en producción.
const LANGUAGES_3: &[&str] =
    &["fil", "haw", "ceb", "yue", "nds", "gsw", "ast", "arn", "hmn", "quz", "cnr"];

/// Códigos de región ISO 3166-1 alfa-2 **asignados oficialmente**.
///
/// Excluye a propósito los «excepcionalmente reservados» y los de uso privado, incluidos los dos
/// que más se cuelan en hreflang: `UK` (lo correcto es `GB`) y `EU` (no es un país; para «el
/// resto de Europa» lo que existe es `x-default`). Google solo honra los asignados, así que
/// avisar de esos dos no es un falso positivo, es el hallazgo.
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

/// Macrorregiones UN M.49 admitidas por BCP 47 como subetiqueta de región.
///
/// La que importa de verdad es `419`, Latinoamérica: `es-419` es válido y muy usado, y darlo por
/// erróneo sería el falso positivo más caro de esta regla. Se incluye el resto del conjunto
/// geográfico de M.49 por coherencia, no porque se vea a menudo.
const REGIONS_M49: &[&str] = &[
    "001", "002", "003", "005", "009", "011", "013", "014", "015", "017", "018", "019", "021",
    "029", "030", "034", "035", "039", "053", "054", "057", "061", "142", "143", "145", "150",
    "151", "154", "155", "202", "419",
];

/// Subetiquetas de escritura ISO 15924 que se ven en hreflang.
///
/// Se aceptan porque Google interpreta `zh-Hant`, `zh-Hans` y `sr-Latn`, y avisar de ellas sería
/// un falso positivo. No es la lista completa de la norma: son las escrituras con presencia real
/// en sitios web.
const SCRIPTS: &[&str] = &[
    "Latn", "Cyrl", "Hans", "Hant", "Hani", "Arab", "Hebr", "Grek", "Deva", "Jpan", "Kana",
    "Hira", "Kore", "Hang", "Thai", "Armn", "Geor", "Beng", "Guru", "Gujr", "Orya", "Taml",
    "Telu", "Knda", "Mlym", "Sinh", "Mymr", "Khmr", "Laoo", "Tibt", "Ethi", "Cans", "Cher",
    "Mong", "Tfng", "Syrc", "Thaa", "Nkoo", "Adlm", "Vaii", "Bopo", "Brai",
];

/// Confusiones habituales de idioma: se toma el código del país cuando lo que toca es el del
/// idioma. Solo se consulta cuando el código ya se ha declarado inválido.
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

/// Confusiones habituales de región.
const REGION_HINTS: &[(&str, &str)] = &[("UK", "GB"), ("EU", "x-default"), ("EN", "GB")];

/// Por qué un código no vale, y qué se quiso escribir si se puede adivinar.
///
/// El `reason` es un identificador estable en inglés: la UI lo traduce, y el diff entre rastreos
/// lo compara. No es texto para el usuario.
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

/// Valida un valor de `hreflang`. `None` es «correcto».
///
/// Acepta `x-default` y `idioma[-Escritura][-REGIÓN]`, sin distinguir mayúsculas —BCP 47 no las
/// distingue, así que `ES-es` es válido aunque la forma canónica sea `es-ES`.
pub fn check_code(code: &str) -> Option<CodeProblem> {
    let bruto = code.trim();
    if bruto.is_empty() {
        return Some(CodeProblem::new("empty"));
    }

    // El guion bajo es el error de separador clásico de los CMS que reutilizan el «locale» de
    // PHP o de Java (`es_ES`). Se detecta antes que nada porque la sugerencia es evidente y
    // porque, si no, el código parecería una sola subetiqueta rarísima.
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

    // Cualquier otra etiqueta de uso privado (`x-*`, `i-*`) no la entiende Google en hreflang.
    let minuscula = bruto.to_ascii_lowercase();
    if minuscula.starts_with("x-") || minuscula == "x" {
        return Some(CodeProblem::with("private_use", "x-default"));
    }

    let partes: Vec<&str> = bruto.split('-').collect();
    if partes.len() > 3 {
        return Some(CodeProblem::new("structure"));
    }

    // --- Idioma ---
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

    // --- Escritura (opcional) y región (opcional) ---
    let resto = &partes[1..];
    let region = match resto {
        [] => None,
        [uno] if es_escritura(uno) => None,
        [uno] => Some(*uno),
        [escritura, r] if es_escritura(escritura) => Some(*r),
        // Dos subetiquetas donde la primera no es una escritura: `es-ES-valencia` sería una
        // variante legítima de BCP 47, pero Google no la usa en hreflang y confundirla con una
        // región daría un aviso incomprensible. Se marca como estructura.
        _ => return Some(CodeProblem::new("structure")),
    };

    // Sin subetiqueta de región el código ya está bien: `es` y `zh-Hant` son válidos.
    region.and_then(|region| check_region(&idioma, region))
}

/// Valida la subetiqueta de región de un código cuyo idioma ya se ha aceptado.
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

/// Sustituye la subetiqueta de idioma conservando el resto, para la sugerencia.
fn sustituir_idioma(partes: &[&str], idioma: &str) -> String {
    let mut salida = String::from(idioma);
    for parte in &partes[1..] {
        salida.push('-');
        salida.push_str(parte);
    }
    canonical_form(&salida)
}

/// Forma canónica de BCP 47: idioma en minúsculas, escritura en capital, región en mayúsculas.
/// Solo se usa para las sugerencias — la validación nunca depende de las mayúsculas.
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
// Comparación de URL
// ---------------------------------------------------------------------------------------------

/// Resuelve un `href` posiblemente relativo contra la URL de la página.
///
/// Es un resolutor mínimo y deliberado: este crate no depende de `url` —no conoce al motor— y su
/// único cometido es que un hreflang relativo no genere hallazgos falsos. Devuelve `None` cuando
/// no hay nada que comparar (href vacío, o base sin esquema).
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

    // Absoluta con esquema propio: se devuelve tal cual. También cubre `mailto:` y compañía,
    // que simplemente no van a coincidir con ninguna página.
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

    // Relativa al directorio de la página.
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

/// Resuelve `.` y `..` en una ruta ya unida.
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

/// Clave de comparación de dos URL que designan la misma página.
///
/// Iguala lo que cualquier servidor estático sirve igual: el fragmento no cuenta, `esquema` y
/// `host` no distinguen mayúsculas, `/a/index.html` es `/a/` y `/a` es `/a/`. Tolerar estas tres
/// formas evita el falso positivo más tonto de todos —«no se autorreferencia» en una página cuyo
/// hreflang apunta a sí misma escrita de otra manera—, a cambio de no distinguir dos URL que un
/// servidor exótico podría servir distintas. Es el intercambio correcto: aquí un falso negativo
/// cuesta mucho menos que un falso positivo.
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

/// Conjunto hreflang que no se incluye a sí mismo.
///
/// Cuenta como autorreferencia cualquier código —incluido `x-default`— cuyo destino sea la propia
/// URL **o su canonical**: el patrón `/a?utm=x` con canonical `/a` y hreflang a `/a` es correcto,
/// y avisar de él sería un falso positivo.
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

/// Valor de `hreflang` que no es un código válido.
///
/// Un hallazgo por código erróneo, agrupado por el código: en un sitio generado por plantilla el
/// mismo `es_ES` sale en miles de páginas y la UI tiene que poder decirlo en una línea.
pub struct HreflangInvalidCode;

impl PageRule for HreflangInvalidCode {
    fn meta(&self) -> &'static RuleMeta {
        &HREFLANG_INVALID_CODE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // El 2xx corta la plantilla de error: los hreflang del HTML de un 404 no los procesa
        // ningún buscador, y sin la puerta cada URL rota repetiría el defecto de la plantilla.
        // Ver `PageContext::is_success`.
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
// Lectura del conjunto hreflang desde el almacén
// ---------------------------------------------------------------------------------------------

/// Una página del rastreo con su conjunto hreflang ya resuelto a absoluto.
struct AlternateSet {
    url_hash: i64,
    url: String,
    is_indexable: bool,
    /// Claves de comparación de la propia página: su URL y su canonical.
    propias: HashSet<String>,
    /// `(código, URL absoluta, clave)` de cada alternativa declarada.
    targets: Vec<(String, String, String)>,
}

/// Lee de `pages` las páginas que declaran hreflang.
///
/// `hreflang_json` lo serializa `engine.rs` como `[[código, href], …]` con el `href` **tal como
/// venía en el HTML**, así que hay que resolverlo aquí. Un SQL puro no puede cruzar eso; se lee
/// y se cruza en Rust, que además solo carga el subconjunto multiidioma del rastreo.
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

        // Un JSON ilegible es un rastreo de otra versión o un fichero tocado a mano. No es
        // asunto de una regla de auditoría: se ignora esa página en vez de romper la pasada.
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

/// A declara a B y B no declara a A.
///
/// Solo se juzgan destinos que **se rastrearon y son HTML**: de un dominio del conjunto que no
/// entró en el rastreo —un dominio declarando a su hermano— no se sabe nada, y suponerlo roto sería
/// el falso positivo más caro de todo el bloque.
pub struct HreflangNotReciprocal;

impl SiteRule for HreflangNotReciprocal {
    fn meta(&self) -> &'static RuleMeta {
        &HREFLANG_NOT_RECIPROCAL
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let conjuntos = leer_conjuntos(conn)?;

        // Índice por clave de URL y de canonical. El primero gana: si dos páginas comparten
        // clave, la ambigüedad se resuelve en silencio hacia la que se leyó antes.
        let mut por_clave: HashMap<&str, usize> = HashMap::new();
        for (i, conjunto) in conjuntos.iter().enumerate() {
            for clave in &conjunto.propias {
                por_clave.entry(clave.as_str()).or_insert(i);
            }
        }

        // Existe la página pero sin ninguna alternativa declarada: se comprueba con una consulta
        // exacta para no tener que cargar en memoria todas las páginas del rastreo.
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

/// Clave de agrupación de una pareja de páginas, independiente del sentido: las dos direcciones
/// del mismo fallo son un solo problema para el usuario.
fn clave_de_par(a: &str, b: &str) -> String {
    let (uno, dos) = if url_key(a) <= url_key(b) { (a, b) } else { (b, a) };
    let mezcla = format!("{}|{}", url_key(uno), url_key(dos));
    format!("hreflang-pair:{:016x}", xxhash_rust::xxh3::xxh3_64(mezcla.as_bytes()))
}

// ---------------------------------------------------------------------------------------------
// HREFLANG-TO-4XX
// ---------------------------------------------------------------------------------------------

/// Una alternativa de idioma apunta a una URL que devolvió 4xx.
///
/// El destino se busca por coincidencia **exacta** con `urls.url`, no por la clave tolerante:
/// aquí la afirmación es «esa URL devuelve un error», y solo se puede sostener sobre la URL que
/// de verdad se pidió. Los 5xx quedan fuera a propósito —suelen ser transitorios y el ID de la
/// regla nombra 4xx—; el hallazgo se registra en la página que declara el hreflang, que es donde
/// está la línea a corregir.
pub struct HreflangTo4xx;

impl SiteRule for HreflangTo4xx {
    fn meta(&self) -> &'static RuleMeta {
        &HREFLANG_TO_4XX
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let conjuntos = leer_conjuntos(conn)?;
        let mut estado = conn.prepare(
            "SELECT status_code FROM urls
             WHERE url = ?1 AND status_code >= 400 AND status_code < 500 LIMIT 1",
        )?;

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

    // ----------------------------------------------------------------------------- códigos

    #[test]
    fn acepta_los_codigos_normales() {
        for codigo in ["es", "en", "es-ES", "en-GB", "pt-BR", "zh-CN", "x-default"] {
            assert!(check_code(codigo).is_none(), "{codigo} debería ser válido");
        }
    }

    #[test]
    fn no_distingue_mayusculas() {
        // BCP 47 no las distingue: `ES-es` es válido aunque la forma canónica sea `es-ES`.
        for codigo in ["ES", "ES-es", "es-es", "X-Default"] {
            assert!(check_code(codigo).is_none(), "{codigo} debería ser válido");
        }
    }

    #[test]
    fn acepta_la_macrorregion_de_latinoamerica() {
        // `es-419` es válido y muy usado: darlo por erróneo sería el falso positivo más caro.
        assert!(check_code("es-419").is_none());
        assert!(check_code("es-150").is_none());
    }

    #[test]
    fn acepta_la_subetiqueta_de_escritura() {
        for codigo in ["zh-Hant", "zh-Hans", "zh-Hant-TW", "sr-Latn-RS"] {
            assert!(check_code(codigo).is_none(), "{codigo} debería ser válido");
        }
    }

    #[test]
    fn acepta_los_alias_historicos_de_la_norma() {
        // `iw` por `he` lo siguen emitiendo bibliotecas antiguas y Google lo interpreta.
        for codigo in ["iw", "in", "sh"] {
            assert!(check_code(codigo).is_none(), "{codigo} debería aceptarse");
        }
    }

    #[test]
    fn detecta_el_guion_bajo_del_locale() {
        let problema = check_code("es_ES").expect("es_ES no es un código válido");
        assert_eq!(problema.reason, "separator");
        assert_eq!(problema.suggestion.as_deref(), Some("es-ES"));
    }

    #[test]
    fn detecta_el_codigo_de_pais_usado_como_idioma() {
        let problema = check_code("sp").expect("sp no es un idioma");
        assert_eq!(problema.reason, "language");
        assert_eq!(problema.suggestion.as_deref(), Some("es"));

        let problema = check_code("jp-JP").expect("jp no es un idioma");
        assert_eq!(problema.suggestion.as_deref(), Some("ja-JP"));
    }

    #[test]
    fn detecta_la_region_inexistente() {
        // `UK` está reservado en la ISO pero no asignado: lo correcto es `GB`.
        let problema = check_code("en-UK").expect("UK no es una región asignada");
        assert_eq!(problema.reason, "region");
        assert_eq!(problema.suggestion.as_deref(), Some("en-GB"));

        // `EU` no es un país; para «el resto» lo que existe es x-default.
        let problema = check_code("en-EU").expect("EU no es una región asignada");
        assert_eq!(problema.suggestion.as_deref(), Some("x-default"));
    }

    #[test]
    fn rechaza_lo_que_no_tiene_forma_de_codigo() {
        for codigo in ["", "   ", "español", "es-ES-MX-AR", "x-es"] {
            assert!(check_code(codigo).is_some(), "{codigo} no debería ser válido");
        }
    }

    // ----------------------------------------------------------------------------- URL

    #[test]
    fn resuelve_href_relativos() {
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
    fn la_clave_de_url_iguala_las_formas_de_la_misma_pagina() {
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
    fn no_self_no_avisa_si_el_conjunto_se_incluye() {
        let c = ctx(&[
            ("es", "https://ejemplo.es/es/"),
            ("en", "https://ejemplo.es/en/"),
        ]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_avisa_cuando_falta_la_autorreferencia() {
        let c = ctx(&[("en", "https://ejemplo.es/en/"), ("fr", "https://ejemplo.es/fr/")]);
        let issues = HreflangNoSelf.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HREFLANG-NO-SELF");
        assert_eq!(issues[0].severity, Severity::High);
    }

    #[test]
    fn no_self_acepta_una_autorreferencia_relativa() {
        // Un href relativo a sí misma es correcto: resolverlo mal daría un aviso falso.
        let c = ctx(&[("es", "/es/"), ("en", "/en/")]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_acepta_la_autorreferencia_escrita_de_otra_forma() {
        let c = ctx(&[("es", "https://ejemplo.es/es/index.html"), ("en", "/en/")]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_acepta_que_el_conjunto_apunte_al_canonical() {
        let mut c = ctx(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        c.url = "https://ejemplo.es/es/?utm_source=x";
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_cuenta_x_default_como_autorreferencia() {
        let c = ctx(&[("x-default", "https://ejemplo.es/es/"), ("en", "/en/")]);
        assert!(HreflangNoSelf.evaluate(&c).is_empty());
    }

    #[test]
    fn no_self_calla_en_una_pagina_sin_hreflang() {
        // La mayoría de las páginas del mundo. No hay conjunto, no hay nada que decir.
        assert!(HreflangNoSelf.evaluate(&ctx(&[])).is_empty());
    }

    #[test]
    fn no_self_calla_si_no_es_html_o_no_es_indexable() {
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
    fn invalid_code_calla_con_codigos_correctos() {
        let c = ctx(&[
            ("es", "https://ejemplo.es/es/"),
            ("en-GB", "https://ejemplo.es/en/"),
            ("es-419", "https://ejemplo.es/latam/"),
            ("x-default", "https://ejemplo.es/"),
        ]);
        assert!(HreflangInvalidCode.evaluate(&c).is_empty());
    }

    #[test]
    fn invalid_code_avisa_una_vez_por_codigo_erroneo() {
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
    fn invalid_code_no_repite_el_mismo_codigo_dos_veces() {
        let c = ctx(&[("sp", "https://ejemplo.es/a/"), ("SP", "https://ejemplo.es/b/")]);
        assert_eq!(HreflangInvalidCode.evaluate(&c).len(), 1);
    }

    #[test]
    fn invalid_code_calla_en_una_pagina_sin_hreflang() {
        assert!(HreflangInvalidCode.evaluate(&ctx(&[])).is_empty());
    }

    #[test]
    fn invalid_code_calla_sobre_algo_que_no_es_html() {
        let mut c = ctx(&[("es_ES", "https://ejemplo.es/es/")]);
        c.is_html = false;
        assert!(HreflangInvalidCode.evaluate(&c).is_empty());
    }

    #[test]
    fn invalid_code_no_audita_la_plantilla_de_error() {
        // Los hreflang del HTML de un 404 no los procesa ningún buscador: sin la puerta del
        // 2xx, el código erróneo de la plantilla saldría una vez por cada URL rota del sitio.
        for status in [301, 404, 410, 500] {
            let mut c = ctx(&[("es_ES", "https://ejemplo.es/es/")]);
            c.status = status;
            assert!(
                HreflangInvalidCode.evaluate(&c).is_empty(),
                "no debería auditar el HTML de un {status}"
            );
        }
    }

    // ----------------------------------------------------------------------------- almacén

    /// Esquema mínimo con las columnas que consultan las reglas de conjunto de este módulo.
    /// El esquema de verdad se prueba rastreando los fixtures en
    /// `crawlforge-core/tests/fixtures_de_reglas.rs`: aquí solo se prueba el cruce.
    fn db(paginas: &[(&str, Option<u16>, bool, Option<&str>)]) -> Connection {
        let conn = Connection::open_in_memory().expect("abrir memoria");
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
        .expect("crear esquema");

        for (i, (url, status, es_pagina, hreflang)) in paginas.iter().enumerate() {
            let id = i as i64 + 1;
            let hash = xxhash_rust::xxh3::xxh3_64(url.as_bytes()) as i64;
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, status_code) VALUES (?1,?2,?3,?4)",
                rusqlite::params![id, url, hash, status],
            )
            .expect("insertar url");
            if *es_pagina {
                conn.execute(
                    "INSERT INTO pages (url_id, canonical, is_indexable, hreflang_json)
                     VALUES (?1,?2,1,?3)",
                    rusqlite::params![id, url, hreflang],
                )
                .expect("insertar página");
            }
        }
        conn
    }

    /// `[[código, href], …]`, el formato exacto que escribe `engine.rs`.
    fn json(pares: &[(&str, &str)]) -> String {
        serde_json::to_string(&pares.iter().map(|(c, h)| (*c, *h)).collect::<Vec<_>>())
            .expect("serializar")
    }

    // ----------------------------------------------------------------------------- RECIPROCAL

    #[test]
    fn reciprocal_calla_cuando_las_dos_se_declaran() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let en = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, Some(&en)),
        ]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn reciprocal_avisa_cuando_el_destino_omite_el_origen() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let en = json(&[("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, Some(&en)),
        ]);
        let hallazgos = HreflangNotReciprocal.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        let (hash, issue) = &hallazgos[0];
        assert_eq!(
            *hash,
            Some(xxhash_rust::xxh3::xxh3_64(b"https://ejemplo.es/es/") as i64),
            "el hallazgo va en la página que declara de más"
        );
        assert!(issue.detail_json.as_deref().unwrap_or_default().contains("target_omits_page"));
    }

    #[test]
    fn reciprocal_avisa_cuando_el_destino_no_declara_nada() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, None),
        ]);
        let hallazgos = HreflangNotReciprocal.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert!(hallazgos[0]
            .1
            .detail_json
            .as_deref()
            .unwrap_or_default()
            .contains("target_declares_no_alternates"));
    }

    #[test]
    fn reciprocal_calla_sobre_un_destino_que_no_se_rastreo() {
        // El caso de un dominio por idioma: el otro no entró en el rastreo, así que no
        // sabemos qué declara. Suponerlo roto sería el peor falso positivo del bloque.
        let es = json(&[("es", "https://ejemplo.es/"), ("en", "https://ejemplo.me/")]);
        let conn = db(&[("https://ejemplo.es/", Some(200), true, Some(&es))]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn reciprocal_tolera_que_las_dos_escriban_la_url_de_otra_forma() {
        let es = json(&[("es", "/es/index.html"), ("en", "/en/")]);
        let en = json(&[("es", "https://ejemplo.es/es"), ("en", "/en/index.html")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, Some(&en)),
        ]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn reciprocal_calla_en_un_rastreo_sin_hreflang() {
        let conn = db(&[("https://ejemplo.es/", Some(200), true, None)]);
        assert!(HreflangNotReciprocal.evaluate(&conn).expect("evaluar").is_empty());
    }

    // ----------------------------------------------------------------------------- TO-4XX

    #[test]
    fn to_4xx_avisa_cuando_el_destino_es_un_404() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(404), false, None),
        ]);
        let hallazgos = HreflangTo4xx.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.severity, Severity::Critical);
        assert!(hallazgos[0].1.detail_json.as_deref().unwrap_or_default().contains("404"));
    }

    #[test]
    fn to_4xx_calla_cuando_el_destino_responde() {
        let es = json(&[("es", "https://ejemplo.es/es/"), ("en", "https://ejemplo.es/en/")]);
        let conn = db(&[
            ("https://ejemplo.es/es/", Some(200), true, Some(&es)),
            ("https://ejemplo.es/en/", Some(200), true, None),
        ]);
        assert!(HreflangTo4xx.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn to_4xx_calla_sobre_un_destino_no_rastreado_y_sobre_los_5xx() {
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
        assert!(HreflangTo4xx.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn to_4xx_calla_en_un_rastreo_sin_hreflang() {
        let conn = db(&[("https://ejemplo.es/", Some(200), true, None)]);
        assert!(HreflangTo4xx.evaluate(&conn).expect("evaluar").is_empty());
    }
}
