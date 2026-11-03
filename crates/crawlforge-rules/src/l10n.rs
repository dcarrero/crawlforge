//! Languages and the Fluent catalogues: the one localisation system shared by the rule catalogue
//! and the CLI.
//!
//! # Why Fluent, and why the files are embedded
//!
//! The texts lived in Rust: `name_es`/`name_en` fields in [`crate::RuleMeta`] and a `match` per
//! message in the CLI. That held for two languages and stops holding at ten: a pair of fields
//! per language does not scale, and Polish needs four plural forms (`one`, `few`, `many`,
//! `other`) where English has two and Japanese one. Hand-written `if n == 1` cannot express
//! that; Fluent's selectors over the CLDR plural rules can, and each translation chooses its own
//! forms without touching code.
//!
//! The files live in `locales/<lang>/{cli,rules}.ftl` at the root of the repository and are
//! compiled in with `include_str!`: one binary, nothing to install, and a translator touches
//! only their own folder. They are parsed **lazily, once per language and process**: English
//! plus the requested language, never the ten.
//!
//! # What replaces "an untranslated string does not compile"
//!
//! With the texts outside Rust the compiler no longer sees them, so the guarantee moves to a
//! test: [`check_translation`] lists, all at once, every key missing from a translation, every
//! key English does not have, and every message whose variables differ from the English one.
//! The CLI and this crate run it over every language folder in `cargo test`, which
//! `tools/verificar.sh` runs.
//!
//! # When a language counts as translated
//!
//! A file with **no messages** (only comments, or empty) means "not translated yet": the
//! language parses, can be selected, and every text falls back to English. A file with at least
//! one message must be **complete**, or the test fails. There is no list of finished languages
//! to keep in sync: filling the folder is what switches a language on, and the test is what
//! keeps a half-done folder out of `main`.
//!
//! # Fallback
//!
//! A message missing from a language, or one that fails to format, is taken from English. The
//! completeness test makes that path unreachable for a translated language; it exists so a
//! broken file degrades to English instead of showing an empty string or a raw key.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use fluent_syntax::ast;
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// Catalogue languages.
///
/// **English is the source language** and every other one a translation of it: it is the order
/// the product ships in, and it decides which text wins when two disagree. The variant order is
/// the index into the arrays of [`locale_sources!`](crate::locale_sources) and must match it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lang {
    #[default]
    En,
    Es,
    De,
    Fr,
    It,
    Nl,
    Tr,
    Pt,
    Ja,
    Pl,
}

impl Lang {
    /// Every language, in the order of the `locales/` arrays.
    pub const ALL: [Lang; 10] = [
        Lang::En,
        Lang::Es,
        Lang::De,
        Lang::Fr,
        Lang::It,
        Lang::Nl,
        Lang::Tr,
        Lang::Pt,
        Lang::Ja,
        Lang::Pl,
    ];

    /// Position in [`Lang::ALL`].
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The code, which is also the folder name under `locales/` and the `--lang` value.
    pub const fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::It => "it",
            Lang::Nl => "nl",
            Lang::Tr => "tr",
            Lang::Pt => "pt",
            Lang::Ja => "ja",
            Lang::Pl => "pl",
        }
    }

    /// Reads a language from a flag or an environment variable: the code (`es`), a code with a
    /// region or encoding (`pt-BR`, `es_ES.UTF-8`, `ES-es`) or the language's own name.
    /// There is one Portuguese: `pt-BR` and `pt-PT` both select it.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        let primary = s.split(['-', '_', '.', '@']).next().unwrap_or("");
        let by_code = Lang::ALL.into_iter().find(|l| l.code() == primary);
        if by_code.is_some() {
            return by_code;
        }
        match s.as_str() {
            "english" => Some(Lang::En),
            "spanish" | "español" | "espanol" => Some(Lang::Es),
            "german" | "deutsch" => Some(Lang::De),
            "french" | "français" | "francais" => Some(Lang::Fr),
            "italian" | "italiano" => Some(Lang::It),
            "dutch" | "nederlands" => Some(Lang::Nl),
            "turkish" | "türkçe" | "turkce" => Some(Lang::Tr),
            "portuguese" | "português" | "portugues" => Some(Lang::Pt),
            "japanese" | "日本語" => Some(Lang::Ja),
            "polish" | "polski" => Some(Lang::Pl),
            _ => None,
        }
    }

    /// The codes of every language, for help and error messages: `en, es, de…`.
    pub fn codes() -> String {
        Lang::ALL.map(Lang::code).join(", ")
    }

    /// Thousands separator. French and Polish use a space (CLDR: narrow no-break in French,
    /// no-break in Polish), which is one terminal column like the dot or the comma.
    pub const fn thousands_separator(self) -> char {
        match self {
            Lang::En | Lang::Ja => ',',
            Lang::Fr => '\u{202F}',
            Lang::Pl => '\u{A0}',
            Lang::Es | Lang::De | Lang::It | Lang::Nl | Lang::Tr | Lang::Pt => '.',
        }
    }

    /// Decimal separator. It goes with [`Lang::thousands_separator`]: a `3.816` of thousands next
    /// to a `1.5` decimal would be ambiguous.
    pub const fn decimal_separator(self) -> char {
        match self {
            Lang::En | Lang::Ja => '.',
            _ => ',',
        }
    }

    /// An integer with this language's thousands separator: `3,816` / `3.816`. Every group is
    /// separated, four-digit numbers too, as the CLI always did.
    pub fn format_integer(self, n: u64) -> String {
        let digits = n.to_string();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3 * 3);
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push(self.thousands_separator());
            }
            out.push(c);
        }
        out
    }

    /// The inverse of [`Lang::format_integer`], and only of it: `Some` when `text` is exactly
    /// what `format_integer` would print. That is how a count that reaches a message already
    /// formatted becomes a number again for the plural selectors without changing a byte of
    /// what is shown.
    pub fn parse_formatted_integer(self, text: &str) -> Option<u64> {
        let sep = self.thousands_separator();
        if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit() || c == sep) {
            return None;
        }
        let digits: String = text.chars().filter(|c| *c != sep).collect();
        let n: u64 = digits.parse().ok()?;
        (self.format_integer(n) == text).then_some(n)
    }
}

// ─────────────────────────────────────────────────────────────────── The files

/// The ten embedded copies of one file of `locales/`, in [`Lang::ALL`] order.
///
/// A macro and not a function because `include_str!` resolves at compile time, from the crate
/// that expands it: `env!("CARGO_MANIFEST_DIR")` is then that crate's folder, and both crates
/// sit two levels below the root that holds `locales/`.
#[macro_export]
macro_rules! locale_sources {
    ($file:literal) => {
        [
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/en/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/es/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/de/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/fr/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/it/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/nl/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/tr/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/pt/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/ja/", $file)),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../locales/pl/", $file)),
        ]
    };
}

type Bundle = FluentBundle<FluentResource>;

/// One file of `locales/` in every language, parsed on first use.
pub struct Catalog {
    sources: [&'static str; 10],
    bundles: [OnceLock<Option<Bundle>>; 10],
}

impl Catalog {
    pub const fn new(sources: [&'static str; 10]) -> Self {
        Self { sources, bundles: [const { OnceLock::new() }; 10] }
    }

    /// The embedded text of one language's file.
    pub fn source(&self, lang: Lang) -> &'static str {
        self.sources[lang.index()]
    }

    /// Whether the language has its own texts in this file. English always has.
    pub fn is_translated(&self, lang: Lang) -> bool {
        lang == Lang::En || self.bundle(lang).is_some()
    }

    fn bundle(&self, lang: Lang) -> Option<&Bundle> {
        self.bundles[lang.index()].get_or_init(|| build_bundle(lang, self.source(lang))).as_ref()
    }

    /// Formats a message (or one of its attributes) in `lang`, falling back to English when
    /// the language lacks it or it fails to format. `None` only if English lacks it too, which
    /// the completeness tests rule out.
    pub fn format(
        &self,
        lang: Lang,
        id: &str,
        attribute: Option<&str>,
        args: Option<&FluentArgs>,
    ) -> Option<String> {
        let tried = if lang == Lang::En { &[Lang::En][..] } else { &[lang, Lang::En][..] };
        for &l in tried {
            let Some(bundle) = self.bundle(l) else { continue };
            let Some(message) = bundle.get_message(id) else { continue };
            let pattern = match attribute {
                Some(a) => message.get_attribute(a).map(|attr| attr.value()),
                None => message.value(),
            };
            let Some(pattern) = pattern else { continue };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            if errors.is_empty() {
                return Some(text.into_owned());
            }
        }
        None
    }
}

/// Parses one file into a bundle. `None` for a file with no messages: that language is not
/// translated yet. A syntax error does not stop the program —the parser skips the broken
/// entry and the rest still works— because the completeness test is where it must be caught.
fn build_bundle(lang: Lang, source: &str) -> Option<Bundle> {
    let resource = match FluentResource::try_new(source.to_owned()) {
        Ok(r) => r,
        Err((r, _)) => r,
    };
    if !resource.entries().any(|e| matches!(e, ast::Entry::Message(_))) {
        return None;
    }
    let langid = lang.code().parse().ok()?;
    let mut bundle = Bundle::new_concurrent(vec![langid]);
    // Without this, Fluent wraps every placeable in Unicode isolation marks (U+2068/U+2069):
    // invisible on screen, but they change every byte of the output and break `grep`.
    bundle.set_use_isolating(false);
    bundle.set_formatter(Some(number_formatter(lang)));
    // Duplicate IDs are an error in the file; the first definition stays and the test reports it.
    let _ = bundle.add_resource(resource);
    Some(bundle)
}

/// Fluent prints numbers without grouping; this prints them as the rest of the CLI does.
fn number_formatter<M>(lang: Lang) -> fn(&FluentValue, &M) -> Option<String> {
    fn format<const L: usize, M>(value: &FluentValue, _: &M) -> Option<String> {
        match value {
            FluentValue::Number(n)
                if n.value >= 0.0 && n.value.fract() == 0.0 && n.value < u64::MAX as f64 =>
            {
                Some(Lang::ALL[L].format_integer(n.value as u64))
            }
            _ => None,
        }
    }
    match lang {
        Lang::En => format::<0, M>,
        Lang::Es => format::<1, M>,
        Lang::De => format::<2, M>,
        Lang::Fr => format::<3, M>,
        Lang::It => format::<4, M>,
        Lang::Nl => format::<5, M>,
        Lang::Tr => format::<6, M>,
        Lang::Pt => format::<7, M>,
        Lang::Ja => format::<8, M>,
        Lang::Pl => format::<9, M>,
    }
}

/// The value for one message argument. A text that is exactly a count as this language prints
/// it (`1,234` in English, `1.234` in Spanish, `7`) becomes a Fluent number, so the plural
/// selectors work and the formatter prints it back identically; anything else stays text.
pub fn arg_value(lang: Lang, text: String) -> FluentValue<'static> {
    match lang.parse_formatted_integer(&text) {
        Some(n) => FluentValue::from(n),
        None => FluentValue::from(text),
    }
}

// ─────────────────────────────────────────────────────── The completeness check

/// What one message declares: its value's variables and, per attribute, the attribute's.
#[derive(Debug, PartialEq, Eq)]
struct Shape {
    has_value: bool,
    value_vars: BTreeSet<String>,
    attributes: Vec<(String, BTreeSet<String>)>,
}

/// The messages of a file, by ID, plus the syntax errors found parsing it.
fn shapes(source: &str) -> (Vec<(String, Shape)>, Vec<String>) {
    let (resource, errors) = match fluent_syntax::parser::parse(source) {
        Ok(r) => (r, Vec::new()),
        Err((r, errs)) => (r, errs.iter().map(|e| format!("{:?}", e.kind)).collect()),
    };
    let mut out = Vec::new();
    let mut problems = errors;
    for entry in &resource.body {
        match entry {
            ast::Entry::Message(m) => {
                let mut value_vars = BTreeSet::new();
                if let Some(p) = &m.value {
                    pattern_vars(p, &mut value_vars);
                }
                let mut attributes: Vec<(String, BTreeSet<String>)> = m
                    .attributes
                    .iter()
                    .map(|a| {
                        let mut vars = BTreeSet::new();
                        pattern_vars(&a.value, &mut vars);
                        (a.id.name.to_string(), vars)
                    })
                    .collect();
                attributes.sort();
                out.push((m.id.name.to_string(), Shape { has_value: m.value.is_some(), value_vars, attributes }));
            }
            ast::Entry::Junk { content } => {
                problems.push(format!("unparsable text: {:?}", content.lines().next().unwrap_or("")));
            }
            _ => {}
        }
    }
    (out, problems)
}

fn pattern_vars(p: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    for el in &p.elements {
        if let ast::PatternElement::Placeable { expression } = el {
            expression_vars(expression, out);
        }
    }
}

fn expression_vars(e: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
    match e {
        ast::Expression::Select { selector, variants } => {
            inline_vars(selector, out);
            for v in variants {
                pattern_vars(&v.value, out);
            }
        }
        ast::Expression::Inline(i) => inline_vars(i, out),
    }
}

fn inline_vars(i: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match i {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_string());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => call_vars(arguments, out),
        ast::InlineExpression::TermReference { arguments: Some(arguments), .. } => {
            call_vars(arguments, out)
        }
        ast::InlineExpression::Placeable { expression } => expression_vars(expression, out),
        _ => {}
    }
}

fn call_vars(a: &ast::CallArguments<&str>, out: &mut BTreeSet<String>) {
    for p in &a.positional {
        inline_vars(p, out);
    }
    for n in &a.named {
        inline_vars(&n.value, out);
    }
}

fn vars_list(v: &BTreeSet<String>) -> String {
    if v.is_empty() {
        "none".to_string()
    } else {
        v.iter().map(|s| format!("${s}")).collect::<Vec<_>>().join(", ")
    }
}

/// The message IDs of a file, in order. For the tests that tie the files to the code.
pub fn message_ids(source: &str) -> Vec<String> {
    shapes(source).0.into_iter().map(|(id, _)| id).collect()
}

/// The variables of one message's value, for the test that ties the CLI's declarations to the
/// English file. `None` if the message does not exist.
pub fn message_variables(source: &str, id: &str) -> Option<Vec<String>> {
    shapes(source).0.into_iter().find(|(i, _)| i == id).map(|(_, s)| s.value_vars.into_iter().collect())
}

/// Whether a file has any message at all (see "When a language counts as translated").
pub fn has_messages(source: &str) -> bool {
    !shapes(source).0.is_empty()
}

/// Every problem of a translation against English, each one a line naming the file and key.
///
/// `file` is only used to label the lines. An English file with problems of its own (syntax
/// errors, duplicate IDs) is reported too, so the same call checks the source.
pub fn check_translation(file: &str, en_source: &str, source: &str) -> Vec<String> {
    let (en, _) = shapes(en_source);
    let (tr, errors) = shapes(source);
    let mut problems: Vec<String> = errors.into_iter().map(|e| format!("{file}: {e}")).collect();

    let mut seen = BTreeSet::new();
    for (id, _) in &tr {
        if !seen.insert(id.as_str()) {
            problems.push(format!("{file}: {id}: defined twice"));
        }
    }
    for (id, en_shape) in &en {
        let Some((_, shape)) = tr.iter().find(|(i, _)| i == id) else {
            problems.push(format!("{file}: {id}: missing (present in English)"));
            continue;
        };
        if en_shape.has_value != shape.has_value {
            problems.push(format!("{file}: {id}: English has {} value", if en_shape.has_value { "a" } else { "no" }));
        }
        if en_shape.value_vars != shape.value_vars {
            problems.push(format!(
                "{file}: {id}: variables {} differ from English ({})",
                vars_list(&shape.value_vars),
                vars_list(&en_shape.value_vars)
            ));
        }
        let names = |s: &Shape| s.attributes.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
        if names(en_shape) != names(shape) {
            problems.push(format!(
                "{file}: {id}: attributes {:?} differ from English ({:?})",
                names(shape),
                names(en_shape)
            ));
        } else {
            for ((name, en_vars), (_, vars)) in en_shape.attributes.iter().zip(&shape.attributes) {
                if en_vars != vars {
                    problems.push(format!(
                        "{file}: {id}.{name}: variables {} differ from English ({})",
                        vars_list(vars),
                        vars_list(en_vars)
                    ));
                }
            }
        }
    }
    for (id, _) in &tr {
        if !en.iter().any(|(i, _)| i == id) {
            problems.push(format!("{file}: {id}: not in English (renamed or removed there?)"));
        }
    }
    problems.extend(whitespace_problems(file, en_source, source, &en));
    problems
}

/// A message that starts or ends with a space in English (a suffix such as ` (3 failed)`) must
/// do so in the translation too: Fluent trims plain spaces at both ends, so the translator has
/// to write `{" "}`, and forgetting it glues two words together on screen.
fn whitespace_problems(file: &str, en_source: &str, source: &str, en: &[(String, Shape)]) -> Vec<String> {
    // The language only matters for plural rules and numbers; the arguments here are text.
    let (Some(en_bundle), Some(bundle)) = (build_bundle(Lang::En, en_source), build_bundle(Lang::En, source))
    else {
        return Vec::new();
    };
    let render = |bundle: &Bundle, id: &str, vars: &BTreeSet<String>| -> Option<String> {
        let message = bundle.get_message(id)?;
        let mut args = FluentArgs::new();
        for v in vars {
            args.set(v.clone(), FluentValue::from(v.clone()));
        }
        let mut errors = Vec::new();
        Some(bundle.format_pattern(message.value()?, Some(&args), &mut errors).into_owned())
    };
    let mut out = Vec::new();
    for (id, shape) in en {
        let (Some(a), Some(b)) = (render(&en_bundle, id, &shape.value_vars), render(&bundle, id, &shape.value_vars))
        else {
            continue;
        };
        if a.starts_with(' ') != b.starts_with(' ') {
            out.push(format!("{file}: {id}: English starts with a space and this does not, or the other way round (write {{\" \"}})"));
        }
        if a.ends_with(' ') != b.ends_with(' ') {
            out.push(format!("{file}: {id}: English ends with a space and this does not, or the other way round (write {{\" \"}})"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_parse_from_codes_regions_and_names() {
        assert_eq!(Lang::parse("es"), Some(Lang::Es));
        assert_eq!(Lang::parse("ES-es"), Some(Lang::Es));
        assert_eq!(Lang::parse("es_ES.UTF-8"), Some(Lang::Es));
        assert_eq!(Lang::parse("en-GB"), Some(Lang::En));
        assert_eq!(Lang::parse("pt-BR"), Some(Lang::Pt));
        assert_eq!(Lang::parse("pt_PT"), Some(Lang::Pt));
        assert_eq!(Lang::parse("ja"), Some(Lang::Ja));
        assert_eq!(Lang::parse("日本語"), Some(Lang::Ja));
        assert_eq!(Lang::parse("Polski"), Some(Lang::Pl));
        assert_eq!(Lang::parse("de-AT"), Some(Lang::De));
        assert_eq!(Lang::parse("klingon"), None);
        assert_eq!(Lang::parse(""), None);
    }

    #[test]
    fn the_index_matches_all() {
        for (i, l) in Lang::ALL.iter().enumerate() {
            assert_eq!(l.index(), i, "{l:?}");
            assert_eq!(Lang::parse(l.code()), Some(*l));
        }
    }

    #[test]
    fn integers_round_trip_through_the_formatted_text() {
        for lang in Lang::ALL {
            for n in [0, 7, 999, 1000, 3816, 1_234_567] {
                let text = lang.format_integer(n);
                assert_eq!(lang.parse_formatted_integer(&text), Some(n), "{lang:?} {text}");
            }
        }
        // A number printed without grouping is not what `format_integer` prints: it stays text,
        // so what is shown never changes.
        assert_eq!(Lang::En.parse_formatted_integer("2026"), None);
        assert_eq!(Lang::En.parse_formatted_integer("1,5"), None);
        assert_eq!(Lang::Es.parse_formatted_integer("12,3"), None);
        assert_eq!(Lang::En.parse_formatted_integer("0.19.0"), None);
        assert_eq!(Lang::En.parse_formatted_integer(""), None);
        assert_eq!(Lang::Pl.format_integer(1_234_567), "1\u{A0}234\u{A0}567");
    }

    const EN: &str = "hello = Hello { $name }\n\
                      pages = { $n ->\n    [one] { $n } page\n   *[other] { $n } pages\n}\n\
                      suffix = {\" \"}({ $n } failed)\n";

    #[test]
    fn a_complete_translation_has_no_problems() {
        let es = "hello = Hola { $name }\n\
                  pages = { $n ->\n    [one] { $n } página\n   *[other] { $n } páginas\n}\n\
                  suffix = {\" \"}({ $n } con error)\n";
        assert_eq!(check_translation("es", EN, es), Vec::<String>::new());
    }

    #[test]
    fn every_problem_is_reported_at_once_with_its_key() {
        let broken = "hello = Hola { $nombre }\nextra = Sobra\nsuffix = ({ $n } con error)\n";
        let problems = check_translation("locales/xx/cli.ftl", EN, broken);
        let all = problems.join("\n");
        assert!(all.contains("locales/xx/cli.ftl: pages: missing"), "{all}");
        assert!(all.contains("hello: variables $nombre differ from English ($name)"), "{all}");
        assert!(all.contains("extra: not in English"), "{all}");
        assert!(all.contains("suffix: English starts with a space"), "{all}");
        assert_eq!(problems.len(), 4, "{all}");
    }

    #[test]
    fn syntax_errors_and_duplicates_are_problems() {
        let broken = "hello = Hola { $name\npages = x\npages = y\nsuffix = {\" \"}x { $n }\n";
        let all = check_translation("xx", EN, broken).join("\n");
        assert!(all.contains("unparsable"), "{all}");
        assert!(all.contains("pages: defined twice"), "{all}");
        assert!(all.contains("pages: variables none differ"), "{all}");
    }

    #[test]
    fn a_file_without_messages_is_untranslated_and_falls_back_to_english() {
        static CAT: Catalog = Catalog::new([
            "hello = Hello { $name }\nonly-en = Only English\n",
            "hello = Hola { $name }\n",
            "### Not translated yet.\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
        ]);
        assert!(CAT.is_translated(Lang::Es));
        assert!(!CAT.is_translated(Lang::De));
        assert!(!CAT.is_translated(Lang::Fr));
        let mut args = FluentArgs::new();
        args.set("name", "Ana");
        assert_eq!(CAT.format(Lang::Es, "hello", None, Some(&args)).as_deref(), Some("Hola Ana"));
        assert_eq!(CAT.format(Lang::De, "hello", None, Some(&args)).as_deref(), Some("Hello Ana"));
        // A key the translation lacks comes from English, never empty.
        assert_eq!(CAT.format(Lang::Es, "only-en", None, None).as_deref(), Some("Only English"));
        assert_eq!(CAT.format(Lang::Es, "nowhere", None, None), None);
    }

    #[test]
    fn plurals_follow_cldr_in_polish_and_japanese() {
        static CAT: Catalog = Catalog::new([
            "pages = { $n ->\n    [one] { $n } page\n   *[other] { $n } pages\n}\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "pages = { $n } ページ\n",
            "pages = { $n ->\n    [one] { $n } strona\n    [few] { $n } strony\n    [many] { $n } stron\n   *[other] { $n } strony\n}\n",
        ]);
        let say = |lang: Lang, n: &str| {
            let mut args = FluentArgs::new();
            args.set("n", arg_value(lang, n.to_string()));
            CAT.format(lang, "pages", None, Some(&args)).unwrap_or_default()
        };
        assert_eq!(say(Lang::Pl, "1"), "1 strona");
        assert_eq!(say(Lang::Pl, "3"), "3 strony");
        assert_eq!(say(Lang::Pl, "5"), "5 stron");
        assert_eq!(say(Lang::Pl, "22"), "22 strony");
        assert_eq!(say(Lang::Pl, "25"), "25 stron");
        // Grouped as Polish prints it, and still a number for the selector.
        assert_eq!(say(Lang::Pl, "1\u{A0}000"), "1\u{A0}000 stron");
        assert_eq!(say(Lang::Ja, "1"), "1 ページ");
        assert_eq!(say(Lang::En, "1"), "1 page");
        assert_eq!(say(Lang::En, "1,234"), "1,234 pages");
    }
}
