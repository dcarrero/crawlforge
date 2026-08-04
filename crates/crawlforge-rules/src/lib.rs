//! The audit rule catalog. See `docs/04-CATALOGO-REGLAS.md`.
//!
//! A separate crate on purpose: **the rules are the product** and they evolve at a different
//! pace than the engine.
//!
//! Two evaluation modes:
//!
//! - [`PageRule`] is evaluated during the crawl, in streaming, over a single page. Cheap.
//! - [`SiteRule`] needs the complete crawl (duplicates, orphans, depth) and runs in a final
//!   pass with SQL over the store.
//!
//! # How a rule is added
//!
//! 1. Declare its [`RuleMeta`] as a `pub static` in its category's module. The ID is forever:
//!    a historical diff depends on it never changing meaning.
//! 2. Implement [`PageRule`] or [`SiteRule`] on an empty struct.
//! 3. Add it to its module's `page_rules()` or `site_rules()` function.
//! 4. Write its fixture in `fixtures/<RULE-ID>.html` and its test in the module. **Both, no
//!    exceptions.** One test checks that no rule is left without a fixture, and another that
//!    the fixture triggers the rule when actually crawled.
//!
//! `MetaTitleMissing` and `MetaTitleDuplicate` are the two examples to copy: one page rule and
//! one site rule, with their meta, their evaluation and their tests.

use rusqlite::Connection;

pub mod asset;
pub mod canon;
pub mod content;
pub mod hreflang;
pub mod http;
pub mod index;
pub mod meta;
pub mod social;

/// The complete published schema for unit tests. One list, guarded by a test that reads the
/// `migrations/` directory — never write a per-module migration list again.
#[cfg(test)]
mod test_schema;

pub use index::{deep_page_shape, DeepPageShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Info => "info",
        }
    }
}

/// The rule's family. Maps to the ID prefix and to the sections of
/// `docs/04-CATALOGO-REGLAS.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Indexability,
    Http,
    Meta,
    Canonical,
    Duplicate,
    Content,
    Asset,
    Hreflang,
    Schema,
    Social,
    Links,
    Accessibility,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Indexability => "indexability",
            Self::Http => "http",
            Self::Meta => "meta",
            Self::Canonical => "canonical",
            Self::Duplicate => "duplicate",
            Self::Content => "content",
            Self::Asset => "asset",
            Self::Hreflang => "hreflang",
            Self::Schema => "schema",
            Self::Social => "social",
            Self::Links => "links",
            Self::Accessibility => "accessibility",
        }
    }

    /// ID prefixes accepted for this category. A test checks that every rule's ID starts with
    /// one of them: it is how the category and the ID are kept from contradicting each other.
    pub fn id_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Indexability => &["INDEX"],
            Self::Http => &["HTTP"],
            Self::Meta => &["META"],
            Self::Canonical => &["CANON"],
            Self::Duplicate => &["DUP"],
            Self::Content => &["CONTENT"],
            Self::Asset => &["ASSET"],
            Self::Hreflang => &["HREFLANG"],
            Self::Schema => &["SCHEMA"],
            Self::Social => &["SOCIAL"],
            Self::Links => &["LINK"],
            Self::Accessibility => &["A11Y"],
        }
    }
}

/// Tier from which a rule applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Agency,
}

/// When the rule can be decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// With the page in front of you, during the crawl.
    Page,
    /// Only once the crawl is done: duplicates, orphans, redirect chains.
    Site,
}

/// Normative reference for a finding.
///
/// It exists from day one, empty in almost every rule, because the future accessibility block
/// has to cite WCAG 2.1 AA, EN 301 549 and EU Directive 2019/882, and adding the field then
/// would mean touching all ~85 rules. See `docs/04-CATALOGO-REGLAS.md §12`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    pub standard: &'static str,
    pub clause: &'static str,
    pub url: &'static str,
}

/// Everything known about a rule without evaluating it.
///
/// Kept separate from the implementation so the CLI and the UIs can list the catalog, and so
/// the texts live **in the crate** and not in each interface: if a rule's name lived in the
/// macOS app, Windows and the CLI would say different things.
#[derive(Debug, Clone, Copy)]
pub struct RuleMeta {
    /// `CATEGORY-SUBJECT-CONDITION`, in English and stable forever.
    pub id: &'static str,
    pub severity: Severity,
    pub category: Category,
    pub min_tier: Tier,
    pub scope: Scope,
    /// Short name, for a table column.
    pub name_es: &'static str,
    pub name_en: &'static str,
    /// What it is and why it matters. It is what the user reads to decide whether to act on it.
    pub desc_es: &'static str,
    pub desc_en: &'static str,
    pub references: &'static [Reference],
}

impl RuleMeta {
    pub fn name(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Es => self.name_es,
            Lang::En => self.name_en,
        }
    }

    pub fn description(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Es => self.desc_es,
            Lang::En => self.desc_en,
        }
    }
}

/// Catalog languages.
///
/// **English is the source language and Spanish a translation**, not the other way around: it
/// is the order the product ships in, and it decides which text wins when the two disagree.
///
/// Both exist from day one, not as a retrofit. When a third language comes in, the `name_*` and
/// `desc_*` fields of [`RuleMeta`] will stop working —you cannot add a pair of fields per
/// language to fifty rules— and the texts will have to move to a separate catalog indexed by
/// language. The [`RuleMeta::name`] and [`RuleMeta::description`] API is already shaped so that
/// change goes unnoticed outside this crate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    En,
    Es,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "es" | "es-es" | "spanish" | "español" => Some(Self::Es),
            "en" | "en-us" | "en-gb" | "english" => Some(Self::En),
            _ => None,
        }
    }
}

pub const RULES_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A finding. Maps to one row of `issues`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub rule_id: &'static str,
    pub severity: Severity,
    pub category: Category,
    pub detail_json: Option<String>,
    /// Groups equivalent findings: the hash of a duplicated title, for example. Lets the UI
    /// say "this title is on 14 pages" instead of listing 14 loose findings.
    pub group_key: Option<String>,
}

impl Issue {
    /// A finding for this rule, with no detail. Copies severity and category from the meta so
    /// they cannot drift apart.
    pub fn new(meta: &'static RuleMeta) -> Self {
        Self {
            rule_id: meta.id,
            severity: meta.severity,
            category: meta.category,
            detail_json: None,
            group_key: None,
        }
    }

    /// Detail as JSON. It is what the UI uses to explain the concrete finding: the title being
    /// duplicated, the milliseconds it took, the destination URL.
    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail_json = Some(detail.to_string());
        self
    }

    /// Adjusts the severity of **this finding**, moving it away from what the rule declares.
    ///
    /// The severity in [`RuleMeta`] is the general case's; there are findings where the same
    /// fact weighs differently and the rule knows it for certain —a `noindex` on the home page
    /// is not a `noindex` on a tag page; a title repeated within a paginated series is not the
    /// same defect as two articles competing—. This method exists for those cases and only for
    /// them: the adjustment has to be reasoned in the rule that makes it, and the `detail_json`
    /// has to say why, because a severity that changes without explanation in the report is
    /// worse than a wrong constant.
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_group(mut self, key: impl Into<String>) -> Self {
        self.group_key = Some(key.into());
        self
    }
}

/// An image on the page, as a rule sees it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImageView<'a> {
    pub src: &'a str,
    /// `None` is "no `alt` attribute"; `Some("")` is a deliberate decorative-image `alt=""`.
    /// They are different things and there is a rule for each.
    pub alt: Option<&'a str>,
    pub width_attr: Option<i64>,
    pub height_attr: Option<i64>,
    /// Text of the `<a>` wrapping the image. `None` if it is not inside a link.
    pub anchor_text: Option<&'a str>,
}

impl ImageView<'_> {
    pub fn in_anchor(&self) -> bool {
        self.anchor_text.is_some()
    }
}

/// A link or resource on the page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkView<'a> {
    /// The `href` **exactly as it came in the HTML**: it can be relative, absolute or
    /// scheme-less.
    ///
    /// It is not resolved to absolute on purpose. Resolving it would mean building a new
    /// string per link, and on a heavily linked site that is millions of allocations no rule
    /// needs: `is_internal` already arrives resolved, and the rules that look at the scheme
    /// —mixed content— want precisely to know whether the HTML says `http://` explicitly,
    /// because a relative link inherits the page's scheme and is never mixed content.
    pub href: &'a str,
    pub anchor: Option<&'a str>,
    pub is_nofollow: bool,
    pub is_internal: bool,
    /// `true` for `<img>`, `<script src>` and `<link rel=stylesheet>`: resources the page
    /// loads, not links the user follows.
    pub is_resource: bool,
    /// `true` if the destination is CDN infrastructure and not site content.
    ///
    /// Today it means `/cdn-cgi/`, Cloudflare's reserved prefix. What made it necessary:
    /// Cloudflare rewrites the email addresses in the HTML as
    /// `/cdn-cgi/l/email-protection#…` with `rel=nofollow`, and that made
    /// `INDEX-NOFOLLOW-INTERNAL` warn on 39 of 40 pages of a real site about something nobody
    /// put there and nobody can remove. A rule that talks about the site's links must ignore
    /// them.
    pub is_infrastructure: bool,
}

/// What a page rule needs to know in order to decide.
///
/// Deliberately flat and borrowed: it is built once per page during the crawl and must not
/// force string copies. Implements [`Default`] so a test only has to write the fields it cares
/// about; for the normal case, [`PageContext::indexable_html`].
#[derive(Debug, Clone, Default)]
pub struct PageContext<'a> {
    pub url: &'a str,
    pub status: u16,
    pub is_html: bool,
    pub is_internal: bool,
    pub is_https: bool,
    /// The URL is blocked by `robots.txt` but was reached through an internal link.
    pub blocked_by_robots: bool,
    pub content_type: Option<&'a str>,
    /// Time to first byte. `None` in `filesystem` mode, where it means nothing.
    pub ttfb_ms: Option<u32>,
    pub html_bytes: u64,
    pub title: Option<&'a str>,
    pub title_count: u32,
    pub meta_description: Option<&'a str>,
    pub meta_robots: Option<&'a str>,
    pub x_robots_tag: Option<&'a str>,
    pub meta_refresh: Option<&'a str>,
    pub viewport: Option<&'a str>,
    pub lang: Option<&'a str>,
    pub h1: Option<&'a str>,
    pub h1_count: u32,
    /// Heading levels in the order they appear. `[1, 2, 4]` is a skip.
    pub heading_levels: &'a [u8],
    /// Text of each heading, in the same order as [`Self::heading_levels`].
    ///
    /// It exists because the diagnosis of a heading skip **is its text**: the `detail_json` of
    /// `CONTENT-HEADING-SKIP` said `{"from":1,"to":4}` on 16,764 pages of a real crawl and the
    /// HTML had to be opened by hand to discover that the culprit was a single `<h4>` in the
    /// author's signature. Tests may leave it empty: the rule treats missing text as "unknown",
    /// never as an error.
    pub heading_texts: &'a [&'a str],
    /// Canonical resolved to absolute.
    pub canonical: Option<&'a str>,
    /// Canonical exactly as it came in the HTML, to tell relative from absolute.
    pub canonical_raw: Option<&'a str>,
    pub canonical_count: u32,
    pub is_indexable: bool,
    pub word_count: u32,
    pub images: &'a [ImageView<'a>],
    pub links: &'a [LinkView<'a>],
    /// `(code, href)` of each `link rel=alternate hreflang`, with the href **exactly as it
    /// came in the HTML**: as in [`LinkView::href`], it can be relative. Whoever compares
    /// destinations has to resolve it against [`Self::url`].
    pub hreflang: &'a [(&'a str, &'a str)],
    /// Open Graph keys present: `og:title`, `og:image`…
    pub og_keys: &'a [&'a str],
}

impl<'a> PageContext<'a> {
    /// Did the response serve content successfully (2xx)?
    ///
    /// It is the entry gate for the rules that audit **the served HTML** —images, links,
    /// canonicals, hreflang— and that do not filter on `is_indexable`. Without it, the theme's
    /// error template gets audited once per broken URL: in a real crawl, every 404 produced
    /// three findings —the 404, the logo with no accessible name and the footer's nofollow—
    /// when the only actionable one is the 404, which already has its `HTTP` rule. A 301 with
    /// an HTML body is not audited either: nobody sees that body, the browser and Google
    /// follow the redirect.
    ///
    /// The rules whose conclusion **is** the status code (`HTTP-5XX`) or that measure the
    /// server and not the HTML (`HTTP-SLOW-RESPONSE`: a slow TTFB is slow whatever the status)
    /// do not use it.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// An internal, indexable, healthy HTML page. **Meant for tests:** lets each one write
    /// only the defect it wants to provoke, instead of thirty fields.
    ///
    /// The `word_count` sits above the `CONTENT-THIN` threshold on purpose, so one rule's test
    /// does not trip another by accident.
    pub fn indexable_html(url: &'a str) -> Self {
        Self {
            url,
            status: 200,
            is_html: true,
            is_internal: true,
            is_https: url.starts_with("https://"),
            is_indexable: true,
            word_count: 500,
            html_bytes: 20_000,
            ..Default::default()
        }
    }
}

/// Rule evaluable over a single page, during the crawl.
pub trait PageRule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue>;

    fn id(&self) -> &'static str {
        self.meta().id
    }
    fn severity(&self) -> Severity {
        self.meta().severity
    }
    fn category(&self) -> Category {
        self.meta().category
    }
    fn min_tier(&self) -> Tier {
        self.meta().min_tier
    }
}

/// Rule that needs the whole crawl. Runs at the end, with SQL over the store.
pub trait SiteRule: Send + Sync {
    fn meta(&self) -> &'static RuleMeta;
    /// Returns `(url_hash, issue)`. A `None` hash is a site-wide finding.
    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>>;

    fn id(&self) -> &'static str {
        self.meta().id
    }
    fn severity(&self) -> Severity {
        self.meta().severity
    }
    fn category(&self) -> Category {
        self.meta().category
    }
    fn min_tier(&self) -> Tier {
        self.meta().min_tier
    }
}

/// Every page rule in the catalog, in category order.
pub fn page_rules() -> Vec<Box<dyn PageRule>> {
    let mut out = Vec::new();
    out.extend(index::page_rules());
    out.extend(http::page_rules());
    out.extend(meta::page_rules());
    out.extend(canon::page_rules());
    out.extend(content::page_rules());
    out.extend(asset::page_rules());
    out.extend(hreflang::page_rules());
    out.extend(social::page_rules());
    out
}

/// Every site rule in the catalog, in category order.
pub fn site_rules() -> Vec<Box<dyn SiteRule>> {
    let mut out = Vec::new();
    out.extend(index::site_rules());
    out.extend(http::site_rules());
    out.extend(meta::site_rules());
    out.extend(canon::site_rules());
    out.extend(content::site_rules());
    out.extend(asset::site_rules());
    out.extend(hreflang::site_rules());
    out.extend(social::site_rules());
    out
}

/// The complete catalog, for `crawlforge rules` and for the UI's list.
///
/// Derived from the registry instead of maintaining a separate list: a rule implemented but
/// not registered would not show up, and a registered one cannot be missing here.
pub fn catalog() -> Vec<&'static RuleMeta> {
    let mut out: Vec<&'static RuleMeta> = page_rules().iter().map(|r| r.meta()).collect();
    out.extend(site_rules().iter().map(|r| r.meta()));
    out
}

/// Rules that cannot be asserted over a truncated crawl.
///
/// Their conclusion depends on the link graph being **complete**. If the crawl was cut short
/// —by the free tier's cap, by `--max-urls` or by time—, the URLs left pending have no
/// outgoing links recorded, so the graph has holes and the two questions these rules ask get
/// answered wrong:
///
/// - "how many clicks away is this page?" — unreachable in the partial graph is not the same
///   as deep in the site.
/// - "does nobody link to this page?" — one of the pages that never got crawled might.
///
/// **This was discovered by running**, not by writing: a 40-URL crawl of a real blog flagged
/// `INDEX-DEEP-PAGE` on 39 of 40 pages. The pages came from the sitemap and the home page only
/// linked to one of them, so the traversal reached none. On the free tier, which cuts at
/// 1,000 URLs, that false positive would have shown up on every large site.
///
/// The engine drops them when `crawl_meta.truncated` is not null. Saying nothing beats saying
/// something false: an auditor with 97% false positives in one rule gets its whole report
/// ignored.
/// `INDEX-ORPHAN-PAGE` joined on 2026-08-01 for the same reason, found once again by crawling:
/// a downloaded page that nobody links to **among what got crawled** is not an orphan, it is a
/// page whose linker fell outside the cut. Migration 005 removed that rule's other false
/// positive, the images one; this one can only be removed by staying silent.
/// `INDEX-SECTION-DISCONNECTED` was born inside this list: "unreachable from the home page" is
/// exactly the claim a graph with holes cannot sustain.
pub const REQUIERE_GRAFO_COMPLETO: &[&str] = &[
    "INDEX-DEEP-PAGE",
    "INDEX-NO-INTERNAL-LINKS-IN",
    "INDEX-ORPHAN-PAGE",
    "INDEX-SECTION-DISCONNECTED",
];

/// Path prefixes that are CDN infrastructure, not site content.
///
/// **Duplicates `crawlforge_core::frontier::INFRASTRUCTURE_PATH_PREFIXES` on purpose**, just
/// as `index::declares_noindex` duplicates `job::has_noindex`: this crate does not know the
/// core and the dependency points the other way. The two lists have to match.
///
/// The **site** rules need it: the page-level filter already arrives resolved in
/// [`LinkView::is_infrastructure`], but a SQL rule like `INDEX-ROBOTS-BLOCKED` reads `urls`
/// directly and has to exclude these paths itself. What made it necessary: Cloudflare injects
/// links to `/cdn-cgi/` **and** blocks them with `Disallow: /cdn-cgi/` in the robots.txt it
/// manages itself, so the three `critical` findings of a real crawl were things the site
/// owner did not put there and cannot fix.
pub const INFRASTRUCTURE_PATH_PREFIXES: &[&str] = &["/cdn-cgi/"];

/// SQL condition "the path column is not CDN infrastructure", derived from
/// [`INFRASTRUCTURE_PATH_PREFIXES`] so the list lives in a single place. The prefixes are
/// literals from this very crate —never user input—, which is why they can be interpolated.
pub fn sql_not_infrastructure(path_column: &str) -> String {
    INFRASTRUCTURE_PATH_PREFIXES
        .iter()
        .map(|prefix| format!("{path_column} NOT LIKE '{prefix}%'"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Status codes that let the external probe assert "this URL is gone": **404 and 410 only**.
///
/// The probe is a `HEAD` (or a bodyless `GET`) sent with a bot user-agent from whatever IP the
/// crawl runs on — very often a datacenter one. That is exactly the signature Cloudflare,
/// Akamai and DataDome challenge, and their walls answer it with 401, 403 or 429 while the
/// page opens fine in any browser. Measured against real hosts with the probe's own method and
/// user-agent (2026-08): `medium.com/@rustlang` `HEAD` → 403, `wsj.com` → 401, `ft.com` and
/// `openai.com` → 403. Every one of those pages loads for a visitor.
///
/// This is the same reasoning that already keeps someone else's 5xx out of
/// `HTTP-404-EXTERNAL`: a code that says "the *server* refused or failed *this request*" is
/// not a code that says "the *resource* is gone", and reporting it would make the report
/// change from one crawl to the next without anyone having touched anything — or worse, call
/// a living page dead. Only 404 and 410 state, from the origin itself, that the resource does
/// not exist.
///
/// The excluded 4xx, so nobody re-adds them in a year without meeting the reasoning first:
///
/// - **401, 403, 407**: authentication or refusal — paywalls, anti-bot walls, hotlink
///   protection. Say nothing about a browser visit.
/// - **429**: rate limiting. Transient by definition, the 4xx twin of the excluded 5xx.
/// - **451**: legally blocked *for this observer*; usually geo-dependent, not gone.
/// - **400 and the rest (405, 406, 418…)**: they judge the request, not the resource. A
///   server that dislikes a bodyless `HEAD` answers 400/405 while a browser `GET` succeeds.
pub const EXTERNAL_GONE_STATUS: &[i64] = &[404, 410];

/// SQL condition "this status code proves the external URL is gone", derived from
/// [`EXTERNAL_GONE_STATUS`] so the list lives in one place. Numeric literals from this very
/// crate — never user input — which is why they can be interpolated.
pub fn sql_external_gone(status_column: &str) -> String {
    let lista = EXTERNAL_GONE_STATUS
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{status_column} IN ({lista})")
}

/// Does the rule need a complete crawl to assert what it asserts?
pub fn requiere_grafo_completo(rule_id: &str) -> bool {
    REQUIERE_GRAFO_COMPLETO.contains(&rule_id)
}

/// Pages from which a group of findings with the same `group_key` is considered **a template
/// defect** and is presented as a single finding with a count.
///
/// The number comes from measuring five real crawls (2026-08-01): the groups that really were
/// the template —a news site's footer link, the `<h5>CONTACTO` in an agency's footer— had
/// 645, 11,799 and 18,085 pages; the groups that were coincidence had at most 7. With 30
/// there is a 4x margin over the largest observed coincidence and 20x under the smallest
/// observed template: if a future crawl lands in between, what is wrong is this number, not
/// the criterion.
pub const TEMPLATE_GROUP_MIN_PAGES: i64 = 30;

/// The small-crawl clause: below [`TEMPLATE_GROUP_MIN_PAGES`] affected pages, a group is
/// still a template if it covers at least this percentage of the crawled pages. A 20-page
/// test crawl with the footer defect on 18 of them is the same template as on the full site.
pub const TEMPLATE_GROUP_MIN_SHARE_PCT: i64 = 80;

/// Absolute floor of the percentage clause: two pages with the same `group_key` are not a
/// template, they are two pages.
pub const TEMPLATE_GROUP_FLOOR_PAGES: i64 = 5;

/// Is a group of `group_pages` findings sharing a `group_key`, in a crawl with `total_pages`
/// HTML pages, a template defect?
///
/// It lives in this crate and not in the CLI because it is finding semantics, not layout: the
/// macOS and Windows apps have to collapse exactly the same groups as the CLI report, or the
/// same file would count different things depending on where it is opened.
///
/// The collapse is **presentation only**: every affected page keeps its row in `issues`,
/// because whoever exports or queries via SQL needs to know exactly which pages they are.
pub fn is_template_group(group_pages: i64, total_pages: i64) -> bool {
    if group_pages >= TEMPLATE_GROUP_MIN_PAGES {
        return true;
    }
    group_pages >= TEMPLATE_GROUP_FLOOR_PAGES
        && total_pages > 0
        && group_pages * 100 >= total_pages * TEMPLATE_GROUP_MIN_SHARE_PCT
}

/// Percentage of crawled pages from which a rule is **pervasive**: the problem is a property
/// of the site, not a list of pages to fix one by one.
///
/// It is the second presentation collapse, sibling of [`is_template_group`] and meant for the
/// case that one cannot cover: massive findings that are **true and share no hashable common
/// cause**. In the full crawl of a real news site (216,349 pages), `INDEX-DEEP-PAGE` produced
/// 202,392 findings, all true —each page is genuinely different, there is no `group_key` they
/// share— and a report opening with that figure does not get read, exactly as when they were
/// false positives.
///
/// The number comes from measuring six real crawls (2026-08-03): the rules whose cause really
/// was systemic —the archive's architecture, the template's title suffix, the server's
/// slowness, a plugin's `noindex`— affected between 41.6% and 100% of the pages; the ones
/// that were lists of pages to fix one by one (heavy images, long descriptions, thin content)
/// stayed at 37.6% or less. 40 splits that gap: if a future crawl lands in between, what is
/// wrong is this number, not the criterion.
pub const PERVASIVE_MIN_SHARE_PCT: i64 = 40;

/// Absolute floor of [`is_pervasive`]: below this many affected pages, the count reads at a
/// glance and restating it as a percentage adds nothing (3 of 6 pages are not "50% of the
/// site", they are three pages).
pub const PERVASIVE_MIN_PAGES: i64 = 20;

/// Is a rule with `affected_pages` affected pages, in a crawl of `total_pages` HTML pages, a
/// pervasive problem of the site?
///
/// It lives in this crate for the same reason as [`is_template_group`]: it is finding
/// semantics, not layout, and the apps have to reformulate exactly the same rules as the CLI.
///
/// **The collapse it governs is presentation only and never subtracts information**: the
/// report line keeps the count and adds the percentage; every affected page keeps its row in
/// `issues`, the export carries it, and `report --rule` lists it. That is why it is safe to
/// apply at any severity: a `critical` rule affecting 90% of the site still shows its full
/// count, it just also says it is 90%.
pub fn is_pervasive(affected_pages: i64, total_pages: i64) -> bool {
    affected_pages >= PERVASIVE_MIN_PAGES
        && total_pages > 0
        && affected_pages * 100 >= total_pages * PERVASIVE_MIN_SHARE_PCT
}

/// Path of a rule's fixture, if it exists.
///
/// Every rule has its test case in `fixtures/`, in one of these two forms:
///
/// - `fixtures/<RULE-ID>.html` — one page is enough to provoke the defect.
/// - `fixtures/<RULE-ID>/` — several pages are needed: duplicates, broken links, orphans.
///
/// The actual crawl of these files lives in `crawlforge-core/tests/fixtures_de_reglas.rs`: it
/// cannot happen here, because the parser lives in the core and the core depends on this
/// crate, not the other way around.
pub fn fixture_path(rule_id: &str) -> Option<std::path::PathBuf> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let fichero = base.join(format!("{rule_id}.html"));
    if fichero.is_file() {
        return Some(fichero);
    }
    let directorio = base.join(rule_id);
    directorio.is_dir().then_some(directorio)
}

/// The page rules that apply to a tier. The limit is enforced **in the core**, not in the UI.
pub fn page_rules_for_tier(tier: Tier) -> Vec<Box<dyn PageRule>> {
    page_rules().into_iter().filter(|r| r.min_tier() <= tier).collect()
}

/// The site rules that apply to a tier.
pub fn site_rules_for_tier(tier: Tier) -> Vec<Box<dyn SiteRule>> {
    site_rules().into_iter().filter(|r| r.min_tier() <= tier).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn no_id_is_duplicated() {
        let mut vistos = HashSet::new();
        for meta in catalog() {
            assert!(vistos.insert(meta.id), "duplicate ID in the catalog: {}", meta.id);
        }
    }

    #[test]
    fn the_id_matches_its_category() {
        for meta in catalog() {
            let prefijos = meta.category.id_prefixes();
            assert!(
                prefijos.iter().any(|p| meta.id.starts_with(p)),
                "{} is in category {:?}, which expects an ID starting with {:?}",
                meta.id,
                meta.category,
                prefijos
            );
        }
    }

    #[test]
    fn every_rule_has_texts_in_both_languages() {
        // UI strings always go through the localization system, and the catalog is the first
        // of those surfaces. An untranslated rule would show up in English inside the Spanish
        // app, which is exactly what this project does not do.
        for meta in catalog() {
            for (lang, nombre, desc) in [
                (Lang::Es, meta.name_es, meta.desc_es),
                (Lang::En, meta.name_en, meta.desc_en),
            ] {
                assert!(!nombre.trim().is_empty(), "{} has no name in {:?}", meta.id, lang);
                assert!(!desc.trim().is_empty(), "{} has no description in {:?}", meta.id, lang);
                assert!(
                    desc.trim().chars().count() > 20,
                    "the description of {} in {:?} explains nothing: {:?}",
                    meta.id,
                    lang,
                    desc
                );
            }
        }
    }

    #[test]
    fn the_declared_scope_matches_the_implemented_trait() {
        for rule in page_rules() {
            assert_eq!(rule.meta().scope, Scope::Page, "{} is a PageRule", rule.id());
        }
        for rule in site_rules() {
            assert_eq!(rule.meta().scope, Scope::Site, "{} is a SiteRule", rule.id());
        }
    }

    #[test]
    fn the_free_tier_includes_no_paid_rules() {
        for rule in page_rules_for_tier(Tier::Free) {
            assert_eq!(rule.min_tier(), Tier::Free, "{}", rule.id());
        }
        for rule in site_rules_for_tier(Tier::Free) {
            assert_eq!(rule.min_tier(), Tier::Free, "{}", rule.id());
        }
    }

    #[test]
    fn the_pro_tier_includes_the_free_rules() {
        assert!(
            page_rules_for_tier(Tier::Pro).len() >= page_rules_for_tier(Tier::Free).len(),
            "Pro must be a superset of Free"
        );
    }

    #[test]
    fn languages_parse_from_a_string() {
        assert_eq!(Lang::parse("es"), Some(Lang::Es));
        assert_eq!(Lang::parse("ES-es"), Some(Lang::Es));
        assert_eq!(Lang::parse("en"), Some(Lang::En));
        assert_eq!(Lang::parse("fr"), None);
    }

    #[test]
    fn no_rule_is_left_without_a_fixture() {
        // "Every rule in the catalog needs an HTML fixture and a test. No exceptions — the
        // rules are the product." This is that sentence, executable.
        let sin_fixture: Vec<&str> = catalog()
            .iter()
            .filter(|m| fixture_path(m.id).is_none())
            .map(|m| m.id)
            .collect();
        assert!(
            sin_fixture.is_empty(),
            "these rules have no fixture in crates/crawlforge-rules/fixtures/: {sin_fixture:?}"
        );
    }

    #[test]
    fn there_are_no_orphan_fixtures() {
        // A fixture whose name matches no rule is almost always a misspelled ID, and the
        // previous test would not see it.
        let ids: HashSet<&str> = catalog().iter().map(|m| m.id).collect();
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let Ok(entradas) = std::fs::read_dir(&base) else {
            return;
        };
        let mut huerfanos = Vec::new();
        for entrada in entradas.flatten() {
            let nombre = entrada.file_name().to_string_lossy().to_string();
            let id = nombre.strip_suffix(".html").unwrap_or(&nombre).to_string();
            if !ids.contains(id.as_str()) {
                huerfanos.push(nombre);
            }
        }
        assert!(huerfanos.is_empty(), "fixtures that do not correspond to any rule: {huerfanos:?}");
    }

    #[test]
    fn a_template_group_is_recognized_by_absolute_size() {
        // The ones measured in real crawls: 645, 11,799 and 18,085 pages are a template…
        assert!(is_template_group(645, 1_549));
        assert!(is_template_group(11_799, 18_134));
        assert!(is_template_group(18_085, 18_134));
        // …and the observed coincidences (≤7 pages out of 18,134) are not.
        assert!(!is_template_group(7, 18_134));
        assert!(!is_template_group(2, 18_134));
    }

    #[test]
    fn in_a_small_crawl_the_percentage_rules() {
        // 18 of 20 pages is the template's footer even without reaching the absolute threshold.
        assert!(is_template_group(18, 20));
        // 4 of 5 meets the percentage but not the floor: two or four pages are not a template.
        assert!(!is_template_group(4, 5));
        // 10 of 40 neither covers the site nor reaches the absolute threshold.
        assert!(!is_template_group(10, 40));
        // With no pages there is no percentage to speak of.
        assert!(!is_template_group(10, 0));
    }

    #[test]
    fn a_pervasive_rule_is_recognized_by_its_share_of_the_site() {
        // The ones measured in real crawls: systemic causes ranged from 41.6% to 100%…
        assert!(is_pervasive(202_392, 216_349)); // INDEX-DEEP-PAGE, the archive with no shortcuts
        assert!(is_pervasive(103_028, 216_349)); // HTTP-SLOW-RESPONSE, the server
        assert!(is_pervasive(848, 1_549)); // INDEX-NOINDEX, the SEO plugin
        assert!(is_pervasive(645, 1_549)); // CONTENT-HEADING-SKIP, the template
        // …and the lists of pages to fix one by one stayed at 37.6% or less.
        assert!(!is_pervasive(61_479, 216_349)); // META-DESC-TOO-LONG, 28.4%
        assert!(!is_pervasive(213, 567)); // META-DESC-TOO-LONG, 37.6%
        assert!(!is_pervasive(1_384, 3_975)); // CONTENT-THIN, 34.8%
    }

    #[test]
    fn few_pages_are_not_pervasive_however_high_their_percentage() {
        // 3 of 6 pages are not "50% of the site": they are three pages, and read at a glance.
        assert!(!is_pervasive(3, 6));
        assert!(!is_pervasive(19, 20));
        // The exact floor does qualify if it covers the share.
        assert!(is_pervasive(20, 40));
        // With no pages there is no percentage to speak of.
        assert!(!is_pervasive(20, 0));
    }

    #[test]
    fn the_test_context_triggers_no_page_rule() {
        // If a healthy page produced findings, every test of every rule would be measuring
        // noise. This test is the one holding up all the others.
        let mut ctx = PageContext::indexable_html("https://ejemplo.es/a");
        ctx.title = Some("Un título suficientemente largo para no avisar");
        ctx.title_count = 1;
        ctx.meta_description = Some(
            "Una descripción de longitud razonable, con más de setenta caracteres para no \
             disparar la regla de descripción corta.",
        );
        ctx.h1 = Some("Un encabezado");
        ctx.h1_count = 1;
        ctx.heading_levels = &[1, 2, 2];
        ctx.canonical = Some("https://ejemplo.es/a");
        ctx.canonical_raw = Some("https://ejemplo.es/a");
        ctx.canonical_count = 1;
        ctx.viewport = Some("width=device-width, initial-scale=1");
        ctx.lang = Some("es");
        ctx.og_keys = &["og:title", "og:description", "og:image"];

        let hallazgos: Vec<&str> =
            page_rules().iter().flat_map(|r| r.evaluate(&ctx)).map(|i| i.rule_id).collect();
        assert!(hallazgos.is_empty(), "a healthy page must not yield findings: {hallazgos:?}");
    }
}
