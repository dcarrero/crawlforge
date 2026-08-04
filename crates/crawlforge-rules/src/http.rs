//! `HTTP` — status codes and redirects. `docs/04-CATALOGO-REGLAS.md §3`.
//!
//! # `HTTP-TEMP-REDIRECT` is not here, and that is not an oversight
//!
//! Its catalogued condition is "302/307 **permanent over time** (shows up in 2+ crawls)", and
//! comparing two crawls requires a comparison history that does not exist yet. With a single
//! crawl file in front of you there is no way to tell a legitimate 302 —a maintenance window, an
//! A/B test, a seasonal promotion— from one that has been in place for two years, and warning
//! about every 302 would be exactly the noise that gets an auditor ignored. It will be
//! implemented when the diff exists, not before.
//!
//! # How the redirects are walked
//!
//! The engine stores every hop as a row of `urls` with `redirect_to` pointing at the next one
//! (`docs/02-MODELO-DATOS.md §3.2`). The three chain rules load into memory **only the rows
//! that redirect** —a few dozen in any real crawl— and walk that graph in Rust, instead of with
//! a `WITH RECURSIVE`. Two reasons: the cycle detection stays readable, and a single walk
//! answers all three questions (how many hops, whether it comes back on itself, and where it
//! ends).

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};

/// TTFB above which a finding is raised, in milliseconds.
///
/// The catalogue threshold (§3) is 1,000 ms: the point where latency stops being a detail and
/// starts costing crawl budget and conversions. The comparison is `>`, so exactly 1,000 ms does
/// not warn.
pub const SLOW_RESPONSE_MS: u32 = 1_000;

/// HTML size above which a finding is raised. The catalogue's 500 KB, in KiB (512,000 bytes).
///
/// This is the HTML alone, without images, CSS or JS: half a megabyte of markup is almost
/// always a template dumping the entire database into the page.
pub const LARGE_PAGE_BYTES: u64 = 500 * 1024;

/// Hops at which a redirect stops being a redirect and becomes a chain.
///
/// One is normal and correct (`/old` → `/new`). Two is already a chain: it loses part of the
/// PageRank on every hop, multiplies the user's latency, and usually means two rewrite rules
/// are stepping on each other.
pub const REDIRECT_CHAIN_MIN_HOPS: usize = 2;

/// Cap on the hops walked before giving up.
///
/// It exists for safety, not semantics: the cycle detection already cuts the loops, and this
/// only protects against a pathological graph. A chain that reaches 20 hops has already been
/// reported.
const MAX_REDIRECT_HOPS: usize = 20;

/// How many sample URLs fit in a finding's `detail_json`.
///
/// A page with 400 resources over HTTP must not put 400 strings into the database: the count
/// travels separately and the list is a sample.
const MAX_SAMPLES: usize = 10;

// ---------------------------------------------------------------- Metadata

pub static HTTP_404_INTERNAL: RuleMeta = RuleMeta {
    id: "HTTP-404-INTERNAL",
    severity: Severity::Critical,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Enlace interno roto",
    name_en: "Broken internal link",
    desc_es: "Una URL del propio sitio devuelve 4xx y hay páginas enlazándola. Gasta presupuesto \
              de rastreo, corta el flujo de enlazado interno hacia lo que había ahí y deja al \
              visitante en una página de error.",
    desc_en: "A URL on this site returns 4xx and there are pages linking to it. It wastes crawl \
              budget, cuts the flow of internal links to whatever used to be there, and leaves \
              the visitor on an error page.",
    references: &[],
};

pub static HTTP_404_EXTERNAL: RuleMeta = RuleMeta {
    id: "HTTP-404-EXTERNAL",
    severity: Severity::Medium,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Enlace externo roto",
    name_en: "Broken external link",
    desc_es: "El sitio enlaza a una URL de otro dominio que ya no existe: responde 404 o 410, o \
              su dominio no resuelve. No penaliza como un 404 propio, pero manda al visitante \
              a una página de error y envejece el contenido: una guía llena de enlaces muertos \
              deja de parecer mantenida.",
    desc_en: "The site links to a URL on another domain that is gone: it answers 404 or 410, or \
              its domain does not resolve. It does not hurt like a 404 of your own, but it \
              sends the visitor to an error page and ages the content: a guide full of dead \
              links stops looking maintained.",
    references: &[],
};

pub static HTTP_5XX: RuleMeta = RuleMeta {
    id: "HTTP-5XX",
    severity: Severity::Critical,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Error de servidor",
    name_en: "Server error",
    desc_es: "La URL devuelve 5xx: el servidor ha fallado al construir la respuesta. Google \
              retira de su índice lo que responde así de forma sostenida y reduce el ritmo de \
              rastreo de todo el sitio mientras dure.",
    desc_en: "The URL returns 5xx: the server failed to build the response. Google drops pages \
              that answer this way for long from its index, and slows down crawling of the whole \
              site while it lasts.",
    references: &[],
};

pub static HTTP_REDIRECT_CHAIN: RuleMeta = RuleMeta {
    id: "HTTP-REDIRECT-CHAIN",
    severity: Severity::High,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Cadena de redirecciones",
    name_en: "Redirect chain",
    desc_es: "Se llega al destino final tras dos o más saltos seguidos. Cada salto suma latencia \
              para el visitante y gasto de rastreo, y suele delatar dos reglas de reescritura \
              que se pisan. El arreglo es apuntar el primer salto directamente al final.",
    desc_en: "The final destination is reached after two or more consecutive hops. Every hop \
              adds latency for the visitor and crawl cost, and usually means two rewrite rules \
              are stepping on each other. The fix is to point the first hop straight at the end.",
    references: &[],
};

pub static HTTP_REDIRECT_LOOP: RuleMeta = RuleMeta {
    id: "HTTP-REDIRECT-LOOP",
    severity: Severity::Critical,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Bucle de redirección",
    name_en: "Redirect loop",
    desc_es: "Las redirecciones vuelven sobre una URL ya visitada, así que el destino no se \
              alcanza nunca. El navegador corta con un error y el buscador no llega a ver \
              contenido: para todos los efectos, esa parte del sitio no existe.",
    desc_en: "The redirects come back to a URL already visited, so the destination is never \
              reached. The browser gives up with an error and the crawler never sees any \
              content: for all practical purposes that part of the site does not exist.",
    references: &[],
};

pub static HTTP_REDIRECT_TO_404: RuleMeta = RuleMeta {
    id: "HTTP-REDIRECT-TO-404",
    severity: Severity::Critical,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "Redirección a un error",
    name_en: "Redirect to error page",
    desc_es: "La redirección acaba en una URL que devuelve 4xx. Es el peor de los dos mundos: se \
              conserva la redirección que hacía creer que el contenido se había movido, pero el \
              destino tampoco existe, así que el enlace y su autoridad se pierden igual.",
    desc_en: "The redirect ends on a URL that returns 4xx. It is the worst of both worlds: the \
              redirect that suggested the content had moved is still there, but the destination \
              does not exist either, so the link and its authority are lost anyway.",
    references: &[],
};

pub static HTTP_MIXED_CONTENT: RuleMeta = RuleMeta {
    id: "HTTP-MIXED-CONTENT",
    severity: Severity::High,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Contenido mixto",
    name_en: "Mixed content",
    desc_es: "Una página servida por HTTPS carga imágenes, hojas de estilo o scripts por HTTP. \
              El navegador bloquea los scripts y las hojas de estilo sin avisar al visitante, y \
              marca la conexión como no segura: la página se ve rota y deja de dar confianza.",
    desc_en: "A page served over HTTPS loads images, stylesheets or scripts over HTTP. The \
              browser silently blocks scripts and stylesheets and flags the connection as not \
              secure: the page looks broken and stops inspiring trust.",
    references: &[],
};

pub static HTTP_NO_HTTPS: RuleMeta = RuleMeta {
    id: "HTTP-NO-HTTPS",
    severity: Severity::Critical,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Site,
    name_es: "El sitio responde por HTTP",
    name_en: "Site answers over HTTP",
    desc_es: "Hay URLs internas que sirven contenido por HTTP sin redirigir a HTTPS. El sitio \
              queda accesible en dos direcciones distintas para la misma página —contenido \
              duplicado— y el navegador avisa al visitante de que la conexión no es segura.",
    desc_en: "Some internal URLs serve content over HTTP without redirecting to HTTPS. The site \
              stays reachable at two different addresses for the same page — duplicate content \
              — and the browser warns the visitor that the connection is not secure.",
    references: &[],
};

pub static HTTP_SLOW_RESPONSE: RuleMeta = RuleMeta {
    id: "HTTP-SLOW-RESPONSE",
    severity: Severity::Medium,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Respuesta lenta",
    name_en: "Slow response",
    desc_es: "El servidor tarda más de un segundo en enviar el primer byte. Ese tiempo se suma \
              íntegro al de carga que mide Core Web Vitals, y limita cuántas páginas alcanza a \
              rastrear el buscador en cada visita.",
    desc_en: "The server takes more than a second to send the first byte. That time is added in \
              full to the load time Core Web Vitals measures, and it caps how many pages the \
              crawler gets through on each visit.",
    references: &[],
};

pub static HTTP_LARGE_PAGE: RuleMeta = RuleMeta {
    id: "HTTP-LARGE-PAGE",
    severity: Severity::Medium,
    category: Category::Http,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "HTML demasiado grande",
    name_en: "Oversized HTML",
    desc_es: "El documento HTML pasa de 500 KB sin contar imágenes ni scripts. Retrasa el primer \
              pintado en conexiones móviles y suele indicar que la plantilla vuelca contenido \
              que la página no muestra, o que hay CSS y JS incrustados que deberían ir aparte.",
    desc_en: "The HTML document is over 500 KB, images and scripts aside. It delays first paint \
              on mobile connections and usually means the template dumps content the page never \
              shows, or that inlined CSS and JS should live in their own files.",
    references: &[],
};

// ---------------------------------------------------------------- Page rules

/// The URL returns 5xx.
///
/// Indexability is not required: a 5xx makes the page non-indexable by definition, so requiring
/// it would silence precisely this finding.
pub struct Http5xx;

impl PageRule for Http5xx {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_5XX
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !(500..600).contains(&ctx.status) {
            return Vec::new();
        }
        vec![Issue::new(&HTTP_5XX).with_detail(serde_json::json!({ "status_code": ctx.status }))]
    }
}

/// HTTPS page that loads some resource over an explicit `http://`.
///
/// One finding per page, with the count and a sample of the resources: twenty badly written
/// images are one template defect, not twenty problems.
pub struct HttpMixedContent;

impl PageRule for HttpMixedContent {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_MIXED_CONTENT
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // The 2xx gate cuts out the error template: if the theme loads a resource over
        // `http://`, every broken URL on the site would repeat this finding. The cause is one
        // and lives in the template, and the 404 already has its own rule. See
        // `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_https || !ctx.is_success() {
            return Vec::new();
        }
        // Resources only: an `<a href="http://...">` to another site is not mixed content, it
        // is a link. What breaks the browser's padlock is what the page *loads*.
        let inseguros: Vec<&str> = ctx
            .links
            .iter()
            .filter(|l| l.is_resource && is_plain_http(l.href))
            .map(|l| l.href)
            .collect();

        if inseguros.is_empty() {
            return Vec::new();
        }

        let muestra: Vec<&str> = inseguros.iter().copied().take(MAX_SAMPLES).collect();
        vec![Issue::new(&HTTP_MIXED_CONTENT).with_detail(serde_json::json!({
            "resources": inseguros.len(),
            "sample": muestra,
        }))]
    }
}

/// TTFB above [`SLOW_RESPONSE_MS`].
///
/// `ttfb_ms` is `None` in `filesystem` mode, where reading a file from disk is not a TTFB and
/// measuring it would be making up a finding. Absence of data is never a finding.
///
/// **It deliberately does not require a 2xx**, unlike the rules that audit the served HTML: the
/// TTFB measures the server, not the page, and a 404 that takes two seconds to arrive is the
/// same server problem as a 200 that takes two seconds.
pub struct HttpSlowResponse;

impl PageRule for HttpSlowResponse {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_SLOW_RESPONSE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        let Some(ttfb) = ctx.ttfb_ms else {
            return Vec::new();
        };
        if ttfb <= SLOW_RESPONSE_MS {
            return Vec::new();
        }
        vec![Issue::new(&HTTP_SLOW_RESPONSE).with_detail(serde_json::json!({
            "ttfb_ms": ttfb,
            "threshold_ms": SLOW_RESPONSE_MS,
        }))]
    }
}

/// HTML above [`LARGE_PAGE_BYTES`].
pub struct HttpLargePage;

impl PageRule for HttpLargePage {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_LARGE_PAGE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // The threshold is for the HTML document. Measuring the size of a PDF or an image with
        // this rule would produce a warning nobody can act on. The 2xx gate cuts out the error
        // template: an obese 404 template would be one row per broken URL with a single cause,
        // and the 404 already has its own rule. See `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() || ctx.html_bytes <= LARGE_PAGE_BYTES {
            return Vec::new();
        }
        vec![Issue::new(&HTTP_LARGE_PAGE).with_detail(serde_json::json!({
            "html_bytes": ctx.html_bytes,
            "threshold_bytes": LARGE_PAGE_BYTES,
        }))]
    }
}

/// Does the `href` explicitly ask for `http://`?
///
/// The `href` is inspected **as it came in the HTML**: a relative `href` (`/logo.png`) or a
/// protocol-relative one (`//cdn.example.com/logo.png`) inherits the page's scheme, so it is
/// never mixed content. Only one that spells out `http://` is. The check would still be correct
/// if the engine passed the `href` already resolved to absolute, because on an HTTPS page a
/// resolved relative starts with `https://`.
fn is_plain_http(href: &str) -> bool {
    let recortado = href.trim_start();
    recortado.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("http://"))
}

// ---------------------------------------------------------------- Site rules

/// An internal page of the site returns 4xx and internal `<a>` links point at it.
///
/// The finding is recorded **on the destination page**, with the count of pages linking to it:
/// that is what needs fixing, and the detail says how much damage it does.
///
/// **Only `element = 'a'` counts**, and it is not an optimization: the parser writes `<img>`,
/// `<script>`, `<link rel=stylesheet>`, `<iframe>` and `<form>` into `links` too, and without
/// the filter a broken stylesheet came out twice — as `ASSET-BROKEN` (`high`) *and* as this
/// rule (`critical`), whose description ("leaves the visitor on an error page") is false for a
/// resource nobody navigates to. On a real crawl with 2,193 broken images that was 2,193
/// spurious criticals. Broken resources have their own rules (`ASSET-BROKEN`,
/// `ASSET-IMG-BROKEN`); a broken `<iframe>`/`<form>` target has none today, which is a smaller
/// and honest gap — better than the same file carrying two contradictory severities.
pub struct Http404Internal;

impl SiteRule for Http404Internal {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_404_INTERNAL
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, u.url, u.status_code, COUNT(DISTINCT l.from_url_id) AS inlinks
             FROM urls u
             JOIN links l ON l.to_url_id = u.id
             WHERE u.is_internal = 1 AND u.status_code >= 400 AND u.status_code < 500
               AND l.element = 'a'
             GROUP BY u.id",
        )?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, status, inlinks) = row?;
            out.push((
                Some(hash),
                Issue::new(&HTTP_404_INTERNAL).with_detail(serde_json::json!({
                    "url": url,
                    "status_code": status,
                    "linked_from": inlinks
                })),
            ));
        }
        Ok(out)
    }
}

/// A URL on another domain that the site links to is gone: 404, 410, or a dead domain.
///
/// **Only the codes the probe can vouch for assert "broken"** — see
/// [`crate::sql_external_gone`] for the full list of what is excluded and why. The short
/// version: the probe is a bot `HEAD` from a datacenter IP, which is exactly what Cloudflare,
/// Akamai and DataDome answer with 401, 403 or 429 while the page opens fine in a browser.
/// Reporting those would have called Medium profiles and paywalled newspapers "broken" on the
/// very first real crawl. It is the same reasoning that keeps someone else's 5xx out: a code
/// that judges the request or the moment, not the resource, cannot back the claim this rule
/// makes. Those rows keep their status in `urls` — nothing is lost, the rule just does not
/// turn them into an accusation it cannot sustain.
///
/// **A DNS resolution failure does count** (`error_kind = 'dns'`, null status): the domain not
/// resolving is the number-one form of link rot, and a browser visit fails exactly like the
/// probe did. The other `error_kind`s — `timeout`, `connection`, `tls`, `toolarge` — stay
/// silent: they are transient or say nothing about the link being gone.
///
/// **Only `element = 'a'` counts**, same as [`Http404Internal`] and for the same reason: a
/// hotlinked external image or script in `links` is not a link the visitor clicks, and mixing
/// them here produced duplicate findings with contradictory severities.
///
/// It needs the engine to have checked the external URL, which it does by default since
/// 2026-08-04 (`check_external`: a `HEAD` probe, once per distinct URL). When that check is
/// off, or when the probe never completed, the row keeps a null `status_code`, no `error_kind`
/// worth asserting on, and the rule finds nothing — no data is not the same as a healthy link,
/// and saying otherwise would be lying by omission.
pub struct Http404External;

impl SiteRule for Http404External {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_404_EXTERNAL
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let sql = format!(
            "SELECT u.url_hash, u.url, u.status_code, u.error_kind,
                    COUNT(DISTINCT l.from_url_id) AS inlinks
             FROM urls u
             JOIN links l ON l.to_url_id = u.id
             WHERE u.is_internal = 0 AND l.element = 'a'
               AND ({estado_roto}
                    OR (u.status_code IS NULL AND u.error_kind = 'dns'))
             GROUP BY u.id",
            estado_roto = crate::sql_external_gone("u.status_code"),
        );
        let mut stmt = conn.prepare(&sql)?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (hash, url, status, error_kind, inlinks) = row?;
            let detail = match status {
                Some(status) => serde_json::json!({
                    "url": url,
                    "status_code": status,
                    "linked_from": inlinks
                }),
                // The dns branch: no status ever arrived, the finding rests on the domain not
                // resolving, and the detail says so instead of faking a code.
                None => serde_json::json!({
                    "url": url,
                    "reason": error_kind,
                    "linked_from": inlinks
                }),
            };
            out.push((Some(hash), Issue::new(&HTTP_404_EXTERNAL).with_detail(detail)));
        }
        Ok(out)
    }
}

/// Two or more consecutive hops until the final destination.
///
/// The finding is recorded on the **head** of the chain: the URL that nobody redirects to and
/// which is therefore the one that shows up in the site's links. Also reporting the
/// intermediate links would repeat the same chain three times without adding anything to act
/// on.
///
/// Loops are skipped: [`HttpRedirectLoop`] counts those, and it is more severe and more
/// specific.
pub struct HttpRedirectChain;

impl SiteRule for HttpRedirectChain {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_REDIRECT_CHAIN
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let saltos = load_redirects(conn)?;
        let mut resolver = UrlLookup::new(conn)?;
        let mut out = Vec::new();

        for cabeza in chain_heads(&saltos) {
            let (camino, final_de_cadena) = walk(cabeza, &saltos);
            if matches!(final_de_cadena, ChainEnd::Loop { .. }) {
                continue;
            }
            let n_saltos = camino.len().saturating_sub(1);
            if n_saltos < REDIRECT_CHAIN_MIN_HOPS {
                continue;
            }
            let Some(nodo) = saltos.get(&cabeza) else {
                continue;
            };
            let destino = camino.last().copied().unwrap_or(cabeza);
            out.push((
                Some(nodo.url_hash),
                Issue::new(&HTTP_REDIRECT_CHAIN).with_detail(serde_json::json!({
                    "url": nodo.url,
                    "hops": n_saltos,
                    "final_url": resolver.url(destino)?,
                    "chain": resolver.urls(&camino)?,
                })),
            ));
        }
        Ok(out)
    }
}

/// The redirects come back to a URL already visited.
///
/// One finding per cycle, on the URL with the lowest `id` in the cycle, so a four-hop loop does
/// not show up four times. The `group_key` uses that URL's `url_hash` —not its `id`, which is a
/// detail of the row and changes between crawls— so the crawl-to-crawl comparison can recognize
/// the same loop from one week to the next.
pub struct HttpRedirectLoop;

impl SiteRule for HttpRedirectLoop {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_REDIRECT_LOOP
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let saltos = load_redirects(conn)?;
        let mut resolver = UrlLookup::new(conn)?;
        let mut reportados: BTreeSet<i64> = BTreeSet::new();
        let mut out = Vec::new();

        // Start from every node that redirects, not just the heads: a pure cycle (A → B → A)
        // has no head, because every one of its nodes is the target of another.
        for inicio in saltos.keys().copied() {
            let (_, final_de_cadena) = walk(inicio, &saltos);
            let ChainEnd::Loop { cycle } = final_de_cadena else {
                continue;
            };
            let Some(&clave) = cycle.iter().min() else {
                continue;
            };
            if !reportados.insert(clave) {
                continue;
            }
            let Some(nodo) = saltos.get(&clave) else {
                continue;
            };
            out.push((
                Some(nodo.url_hash),
                Issue::new(&HTTP_REDIRECT_LOOP)
                    .with_detail(serde_json::json!({
                        "url": nodo.url,
                        "length": cycle.len(),
                        "cycle": resolver.urls(&cycle)?,
                    }))
                    .with_group(format!("redirect-loop:{:016x}", nodo.url_hash as u64)),
            ));
        }
        Ok(out)
    }
}

/// The redirect chain ends on a URL that returns 4xx.
///
/// As in [`HttpRedirectChain`], the finding goes on the head of the chain: it is the URL being
/// linked and the one that must be repointed. A loop does not end anywhere, so it does not
/// count here.
pub struct HttpRedirectTo404;

impl SiteRule for HttpRedirectTo404 {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_REDIRECT_TO_404
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let saltos = load_redirects(conn)?;
        let mut resolver = UrlLookup::new(conn)?;
        let mut out = Vec::new();

        for cabeza in chain_heads(&saltos) {
            let (camino, final_de_cadena) = walk(cabeza, &saltos);
            let ChainEnd::Final(destino) = final_de_cadena else {
                continue;
            };
            let Some((url_final, Some(estado))) = resolver.row(destino)? else {
                continue;
            };
            if !(400..500).contains(&estado) {
                continue;
            }
            let Some(nodo) = saltos.get(&cabeza) else {
                continue;
            };
            out.push((
                Some(nodo.url_hash),
                Issue::new(&HTTP_REDIRECT_TO_404).with_detail(serde_json::json!({
                    "url": nodo.url,
                    "final_url": url_final,
                    "final_status_code": estado,
                    "hops": camino.len().saturating_sub(1),
                })),
            ));
        }
        Ok(out)
    }
}

/// Some internal URLs serve content over HTTP without taking the visitor to HTTPS.
///
/// A single site finding (null `url_id`), with the count and a sample: it is not a defect of
/// each page, it is a server configuration, and listing 40,000 URLs does not help fix it.
///
/// What counts as "answering over HTTP without redirecting":
///
/// - A 2xx over `http://` — serves the content as is.
/// - A 3xx over `http://` whose target is still `http://` — redirects, but not to HTTPS.
///
/// A 4xx or a 5xx does not count: it says nothing about how HTTPS is configured.
pub struct HttpNoHttps;

impl SiteRule for HttpNoHttps {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_NO_HTTPS
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url, u.status_code
             FROM urls u
             LEFT JOIN urls t ON t.id = u.redirect_to
             WHERE u.is_internal = 1 AND u.scheme = 'http' AND u.crawl_state = 'done'
               AND u.status_code IS NOT NULL
               AND (
                    (u.status_code >= 200 AND u.status_code < 300)
                 OR (u.status_code >= 300 AND u.status_code < 400 AND t.scheme = 'http')
               )
             ORDER BY u.url",
        )?;

        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<Vec<(String, i64)>>>()?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let muestra: Vec<&str> = rows.iter().take(MAX_SAMPLES).map(|(u, _)| u.as_str()).collect();
        Ok(vec![(
            None,
            Issue::new(&HTTP_NO_HTTPS).with_detail(serde_json::json!({
                "http_urls": rows.len(),
                "sample": muestra,
            })),
        )])
    }
}

// ---------------------------------------------------------------- Redirect graph

/// A row of `urls` that redirects to another.
#[derive(Debug, Clone)]
struct RedirectHop {
    url_hash: i64,
    url: String,
    redirect_to: i64,
}

/// Where a walk over the redirect graph ends.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChainEnd {
    /// A URL that does not redirect was reached. This is that URL's `id`.
    Final(i64),
    /// The walk came back to a URL already visited. `cycle` holds the nodes of the cycle, in
    /// order.
    Loop { cycle: Vec<i64> },
    /// [`MAX_REDIRECT_HOPS`] ran out without closing a cycle or finishing.
    TooLong,
}

/// Loads only the rows that redirect. In a crawl of 100,000 URLs that is a few dozen.
fn load_redirects(conn: &Connection) -> rusqlite::Result<BTreeMap<i64, RedirectHop>> {
    let mut stmt = conn.prepare(
        "SELECT id, url_hash, url, redirect_to FROM urls
         WHERE redirect_to IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            RedirectHop { url_hash: r.get(1)?, url: r.get(2)?, redirect_to: r.get(3)? },
        ))
    })?;
    rows.collect()
}

/// The chain heads: nodes that redirect and that nobody redirects to.
fn chain_heads(saltos: &BTreeMap<i64, RedirectHop>) -> Vec<i64> {
    let destinos: BTreeSet<i64> = saltos.values().map(|h| h.redirect_to).collect();
    saltos.keys().copied().filter(|id| !destinos.contains(id)).collect()
}

/// Walks the chain from `inicio`. Returns the path (with `inicio` included) and its ending.
fn walk(inicio: i64, saltos: &BTreeMap<i64, RedirectHop>) -> (Vec<i64>, ChainEnd) {
    let mut camino = vec![inicio];
    let mut actual = inicio;

    for _ in 0..MAX_REDIRECT_HOPS {
        let Some(salto) = saltos.get(&actual) else {
            return (camino, ChainEnd::Final(actual));
        };
        let siguiente = salto.redirect_to;
        if let Some(pos) = camino.iter().position(|id| *id == siguiente) {
            let cycle = camino[pos..].to_vec();
            return (camino, ChainEnd::Loop { cycle });
        }
        camino.push(siguiente);
        actual = siguiente;
    }
    (camino, ChainEnd::TooLong)
}

/// Resolves `id` → `(url, status_code)` by primary key, with a cache.
///
/// It is queried once per chain, not once per crawl row: loading the entire `urls` table to
/// decorate twenty findings would be exactly antipattern §9.2.
struct UrlLookup<'c> {
    stmt: rusqlite::Statement<'c>,
    cache: BTreeMap<i64, Option<(String, Option<i64>)>>,
}

impl<'c> UrlLookup<'c> {
    fn new(conn: &'c Connection) -> rusqlite::Result<Self> {
        Ok(Self {
            stmt: conn.prepare("SELECT url, status_code FROM urls WHERE id = ?1")?,
            cache: BTreeMap::new(),
        })
    }

    fn row(&mut self, id: i64) -> rusqlite::Result<Option<(String, Option<i64>)>> {
        if let Some(cached) = self.cache.get(&id) {
            return Ok(cached.clone());
        }
        let fila = self
            .stmt
            .query_row([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                otro => Err(otro),
            })?;
        self.cache.insert(id, fila.clone());
        Ok(fila)
    }

    fn url(&mut self, id: i64) -> rusqlite::Result<Option<String>> {
        Ok(self.row(id)?.map(|(u, _)| u))
    }

    fn urls(&mut self, ids: &[i64]) -> rusqlite::Result<Vec<String>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(u) = self.url(*id)? {
                out.push(u);
            }
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------- Registry

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![
        Box::new(Http5xx),
        Box::new(HttpMixedContent),
        Box::new(HttpSlowResponse),
        Box::new(HttpLargePage),
    ]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    vec![
        Box::new(Http404Internal),
        Box::new(Http404External),
        Box::new(HttpRedirectChain),
        Box::new(HttpRedirectLoop),
        Box::new(HttpRedirectTo404),
        Box::new(HttpNoHttps),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LinkView;
    use rusqlite::params;

    // ------------------------------------------------------------ Page rules

    fn ctx<'a>() -> PageContext<'a> {
        PageContext::indexable_html("https://ejemplo.es/a")
    }

    #[test]
    fn a_successful_response_is_not_flagged_as_5xx() {
        assert!(Http5xx.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn a_server_error_produces_a_finding() {
        let mut c = ctx();
        c.status = 503;
        let issues = Http5xx.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-5XX");
        assert_eq!(issues[0].severity, Severity::Critical);
    }

    #[test]
    fn a_4xx_is_not_a_server_error() {
        let mut c = ctx();
        c.status = 404;
        assert!(Http5xx.evaluate(&c).is_empty());
    }

    #[test]
    fn the_5xx_range_boundaries_are_exact() {
        for (estado, espera) in [(499u16, 0), (500, 1), (599, 1), (600, 0)] {
            let mut c = ctx();
            c.status = estado;
            assert_eq!(Http5xx.evaluate(&c).len(), espera, "status {estado}");
        }
    }

    #[test]
    fn a_non_indexable_page_still_reports_5xx() {
        // A 5xx makes the page non-indexable by definition: requiring indexability would
        // silence the only finding that matters.
        let mut c = ctx();
        c.status = 500;
        c.is_indexable = false;
        assert_eq!(Http5xx.evaluate(&c).len(), 1);
    }

    fn recurso<'a>(href: &'a str) -> LinkView<'a> {
        LinkView { href, is_resource: true, ..Default::default() }
    }

    #[test]
    fn no_mixed_content_finding_without_insecure_resources() {
        let enlaces = [recurso("https://cdn.ejemplo.es/x.js"), recurso("/estilo.css")];
        let mut c = ctx();
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn an_http_resource_on_an_https_page_produces_a_finding() {
        let enlaces = [recurso("http://cdn.ejemplo.com/analitica.js")];
        let mut c = ctx();
        c.links = &enlaces;
        let issues = HttpMixedContent.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-MIXED-CONTENT");
    }

    #[test]
    fn several_insecure_resources_are_a_single_finding() {
        let enlaces = [
            recurso("http://cdn.ejemplo.com/a.js"),
            recurso("http://cdn.ejemplo.com/b.css"),
            recurso("HTTP://CDN.EJEMPLO.COM/c.png"),
        ];
        let mut c = ctx();
        c.links = &enlaces;
        let issues = HttpMixedContent.evaluate(&c);
        assert_eq!(issues.len(), 1, "it is a template defect, not three problems");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"resources\":3"), "unexpected detail: {detalle}");
    }

    #[test]
    fn an_http_link_is_not_mixed_content() {
        // Linking to a third-party site served over HTTP does not break the browser's padlock:
        // only what the page *loads* does.
        let enlaces = [LinkView { href: "http://otro.example/", ..Default::default() }];
        let mut c = ctx();
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn a_protocol_relative_resource_is_not_mixed_content() {
        // `//host/x.js` inherits the page's scheme, so on HTTPS it goes over HTTPS.
        let enlaces = [recurso("//cdn.ejemplo.com/x.js")];
        let mut c = ctx();
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn an_http_page_cannot_have_mixed_content() {
        // Without HTTPS there is no mixing: that page's problem is a different one, and
        // HTTP-NO-HTTPS reports it.
        let enlaces = [recurso("http://cdn.ejemplo.com/x.js")];
        let mut c = PageContext::indexable_html("http://ejemplo.es/a");
        c.links = &enlaces;
        assert!(!c.is_https);
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn mixed_content_is_not_checked_outside_html() {
        let enlaces = [recurso("http://cdn.ejemplo.com/x.js")];
        let mut c = ctx();
        c.is_html = false;
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn the_html_of_an_error_page_is_not_audited() {
        // The theme's error template is served once per broken URL: without the 2xx gate, an
        // `http://` resource or an obese HTML in that template would be one finding per 404 on
        // the site, with a single cause. See `PageContext::is_success`.
        let enlaces = [recurso("http://cdn.ejemplo.com/x.js")];
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            c.links = &enlaces;
            assert!(
                HttpMixedContent.evaluate(&c).is_empty(),
                "HTTP-MIXED-CONTENT should not audit the HTML of a {status}"
            );

            let mut c = ctx();
            c.status = status;
            c.html_bytes = LARGE_PAGE_BYTES + 1;
            assert!(
                HttpLargePage.evaluate(&c).is_empty(),
                "HTTP-LARGE-PAGE should not audit the HTML of a {status}"
            );
        }
    }

    #[test]
    fn a_slow_response_is_reported_whatever_the_status() {
        // The TTFB measures the server, not the HTML: a slow 404 is the same server problem as
        // a slow 200, which is why this rule does not carry the 2xx gate.
        let mut c = ctx();
        c.status = 404;
        c.ttfb_ms = Some(SLOW_RESPONSE_MS + 500);
        assert_eq!(HttpSlowResponse.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_slowness_finding_without_a_ttfb_measurement() {
        // `filesystem` mode: no network, so no TTFB and no possible finding.
        let mut c = ctx();
        c.ttfb_ms = None;
        assert!(HttpSlowResponse.evaluate(&c).is_empty());
    }

    #[test]
    fn a_slow_response_produces_a_finding() {
        let mut c = ctx();
        c.ttfb_ms = Some(1_500);
        let issues = HttpSlowResponse.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-SLOW-RESPONSE");
    }

    #[test]
    fn the_slowness_threshold_is_exclusive() {
        let mut c = ctx();
        c.ttfb_ms = Some(SLOW_RESPONSE_MS);
        assert!(HttpSlowResponse.evaluate(&c).is_empty());
        c.ttfb_ms = Some(SLOW_RESPONSE_MS + 1);
        assert_eq!(HttpSlowResponse.evaluate(&c).len(), 1);
    }

    #[test]
    fn a_normal_sized_page_is_not_flagged() {
        assert!(HttpLargePage.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn an_oversized_html_produces_a_finding() {
        let mut c = ctx();
        c.html_bytes = 700_000;
        let issues = HttpLargePage.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-LARGE-PAGE");
    }

    #[test]
    fn the_size_threshold_is_exclusive() {
        let mut c = ctx();
        c.html_bytes = LARGE_PAGE_BYTES;
        assert!(HttpLargePage.evaluate(&c).is_empty());
        c.html_bytes = LARGE_PAGE_BYTES + 1;
        assert_eq!(HttpLargePage.evaluate(&c).len(), 1);
    }

    #[test]
    fn the_size_threshold_only_applies_to_html() {
        let mut c = ctx();
        c.is_html = false;
        c.html_bytes = 5_000_000;
        assert!(HttpLargePage.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ Site rules
    //
    // None of these can be triggered with a filesystem fixture: without a server there are no
    // 3xx, no 5xx, and no checking of third-party URLs. The in-memory database is the only way
    // to exercise the SQL, so the real schema is set up here and the minimal rows inserted.

    fn db() -> Connection {
        // The full published schema, from the shared helper guarded against `migrations/`.
        let conn = crate::test_schema::full_schema();
        // Rows are inserted with `redirect_to` pointing at `id`s that do not exist yet, and a
        // redirect loop is circular by definition: no insertion order satisfies the foreign
        // key. The real engine writes in batches inside a transaction; here it is enough not to
        // enforce it.
        conn.pragma_update(None, "foreign_keys", false).expect("disable foreign keys");
        conn
    }

    fn hash(url: &str) -> i64 {
        xxhash_rust::xxh3::xxh3_64(url.as_bytes()) as i64
    }

    /// Minimal `urls` row. Explicit `id` so `redirect_to` chains can be wired by hand.
    struct Fila<'a> {
        id: i64,
        url: &'a str,
        internal: bool,
        status: Option<i64>,
        redirect_to: Option<i64>,
        error_kind: Option<&'a str>,
    }

    impl<'a> Fila<'a> {
        fn interna(id: i64, url: &'a str, status: i64) -> Self {
            Self {
                id,
                url,
                internal: true,
                status: Some(status),
                redirect_to: None,
                error_kind: None,
            }
        }

        fn externa(id: i64, url: &'a str, status: Option<i64>) -> Self {
            Self { id, url, internal: false, status, redirect_to: None, error_kind: None }
        }

        /// An external URL whose probe failed: no status, only the failure kind, exactly as
        /// `engine::external_check_failed_row` leaves it.
        fn externa_fallida(id: i64, url: &'a str, kind: &'a str) -> Self {
            Self {
                id,
                url,
                internal: false,
                status: None,
                redirect_to: None,
                error_kind: Some(kind),
            }
        }

        fn hacia(mut self, destino: i64) -> Self {
            self.redirect_to = Some(destino);
            self
        }
    }

    fn insertar(conn: &Connection, f: Fila<'_>) {
        let esquema = if f.url.starts_with("https://") { "https" } else { "http" };
        let resto = f.url.trim_start_matches("https://").trim_start_matches("http://");
        let (host, path) = match resto.find('/') {
            Some(i) => (&resto[..i], &resto[i..]),
            None => (resto, "/"),
        };
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state, status_code, redirect_to, redirect_chain_len,
                               error_kind)
             VALUES (?1,?2,?3,?4,?5,?6,?7,0,'done',?8,?9,0,?10)",
            params![
                f.id,
                f.url,
                hash(f.url),
                esquema,
                host,
                path,
                f.internal as i64,
                f.status,
                f.redirect_to,
                f.error_kind
            ],
        )
        .expect("insert url");
    }

    fn enlazar(conn: &Connection, desde: i64, hacia: i64) {
        enlazar_como(conn, desde, hacia, "a");
    }

    fn enlazar_como(conn: &Connection, desde: i64, hacia: i64, element: &str) {
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, element) VALUES (?1, ?2, ?3)",
            params![desde, hacia, element],
        )
        .expect("insert link");
    }

    fn ids(hallazgos: &[(Option<i64>, Issue)]) -> Vec<Option<i64>> {
        hallazgos.iter().map(|(h, _)| *h).collect()
    }

    // --- HTTP-404-EXTERNAL ---

    #[test]
    fn a_broken_external_link_produces_a_finding() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::externa(2, "https://otro.example/muerta", Some(404)));
        enlazar(&conn, 1, 2);

        let hallazgos = Http404External.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.rule_id, "HTTP-404-EXTERNAL");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://otro.example/muerta"))]);
    }

    #[test]
    fn an_internal_404_is_not_counted_by_the_external_rule() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/no-existe", 404));
        enlazar(&conn, 1, 2);

        assert!(Http404External.evaluate(&conn).expect("evaluate").is_empty());
        assert_eq!(Http404Internal.evaluate(&conn).expect("evaluate").len(), 1);
    }

    #[test]
    fn an_external_url_with_unchecked_status_produces_no_finding() {
        // With `--no-external-check`, and with any probe that fails to complete, the external
        // URL is recorded without a status. No data, no finding: the rule stays silent instead
        // of claiming the link is fine, which would be lying by omission.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::externa(2, "https://otro.example/quizas", None));
        enlazar(&conn, 1, 2);

        assert!(Http404External.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn an_external_5xx_does_not_count_as_a_broken_link() {
        // Almost always a temporary problem on the other party's server: reporting it would
        // make the report change from one crawl to the next without anyone having touched
        // anything.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::externa(2, "https://otro.example/caida", Some(503)));
        enlazar(&conn, 1, 2);

        assert!(Http404External.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_broken_external_url_nobody_links_to_produces_no_finding() {
        let conn = db();
        insertar(&conn, Fila::externa(1, "https://otro.example/muerta", Some(410)));
        assert!(Http404External.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_410_is_a_broken_external_link() {
        // Non-regression guard: 410 is the origin stating "gone", even more explicitly than a
        // 404, and it must keep firing after narrowing the asserted codes.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::externa(2, "https://otro.example/retirada", Some(410)));
        enlazar(&conn, 1, 2);

        assert_eq!(Http404External.evaluate(&conn).expect("evaluate").len(), 1);
    }

    #[test]
    fn a_bot_wall_status_is_not_a_broken_external_link() {
        // Proves the fix. Cloudflare, Akamai and DataDome answer the probe's bot HEAD with
        // 401/403/429 while the page opens fine in a browser — measured against medium.com
        // (403), wsj.com (401) and ft.com (403) with the probe's own method and user-agent.
        // Calling those "broken" would be false on the very first real crawl. Same reasoning
        // as the excluded foreign 5xx; the full list lives in `sql_external_gone`.
        for status in [401, 403, 407, 429, 451] {
            let conn = db();
            insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
            insertar(&conn, Fila::externa(2, "https://otro.example/paywall", Some(status)));
            enlazar(&conn, 1, 2);

            assert!(
                Http404External.evaluate(&conn).expect("evaluate").is_empty(),
                "a {status} from a host we do not control proves nothing about the link"
            );
        }
    }

    #[test]
    fn a_400_is_not_asserted_as_broken_either() {
        // Deliberate: a 400 to a bodyless bot HEAD frequently judges the request, not the
        // resource — servers that dislike HEAD answer 400/405 while a browser GET succeeds.
        // Only 404 and 410 state that the resource itself is gone.
        for status in [400, 405, 406, 418] {
            let conn = db();
            insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
            insertar(&conn, Fila::externa(2, "https://otro.example/rara", Some(status)));
            enlazar(&conn, 1, 2);

            assert!(
                Http404External.evaluate(&conn).expect("evaluate").is_empty(),
                "a {status} judges the request, not the resource"
            );
        }
    }

    #[test]
    fn a_domain_that_does_not_resolve_is_a_broken_external_link() {
        // Proves the fix. NXDOMAIN is the number-one form of link rot — and a security smell,
        // because dead domains get re-registered. The visitor's browser fails exactly like the
        // probe did, so this the rule *can* assert, unlike a wall's 403.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(
            &conn,
            Fila::externa_fallida(2, "https://dominio-que-cerro.example/post", "dns"),
        );
        enlazar(&conn, 1, 2);

        let hallazgos = Http404External.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.rule_id, "HTTP-404-EXTERNAL");
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"reason\":\"dns\""), "unexpected detail: {detalle}");
        assert!(!detalle.contains("status_code"), "there is no status to report: {detalle}");
    }

    #[test]
    fn a_transient_probe_failure_is_not_a_broken_external_link() {
        // Guard: only DNS asserts. A timeout, a refused connection or a TLS handshake failure
        // is transient or observer-dependent, and no data is not a finding.
        for kind in ["timeout", "connection", "tls", "toolarge"] {
            let conn = db();
            insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
            insertar(&conn, Fila::externa_fallida(2, "https://otro.example/lenta", kind));
            enlazar(&conn, 1, 2);

            assert!(
                Http404External.evaluate(&conn).expect("evaluate").is_empty(),
                "a '{kind}' failure says nothing about the link being gone"
            );
        }
    }

    #[test]
    fn a_broken_resource_is_not_a_broken_link() {
        // Proves the fix for the duplicate-findings defect: the parser writes stylesheets,
        // scripts, images and iframes into `links` too, and without the `element = 'a'` filter
        // a broken CSS was reported both as ASSET-BROKEN (high) and as HTTP-404-INTERNAL
        // (critical) — two severities for one file, and the critical description ("leaves the
        // visitor on an error page") is false for a resource nobody navigates to.
        for element in ["link", "script", "img", "iframe"] {
            let conn = db();
            insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
            insertar(&conn, Fila::interna(2, "https://ejemplo.es/estilo.css", 404));
            insertar(&conn, Fila::externa(3, "https://cdn.example/lib.js", Some(404)));
            enlazar_como(&conn, 1, 2, element);
            enlazar_como(&conn, 1, 3, element);

            assert!(
                Http404Internal.evaluate(&conn).expect("evaluate").is_empty(),
                "a broken <{element}> resource is ASSET territory, not a broken internal link"
            );
            assert!(
                Http404External.evaluate(&conn).expect("evaluate").is_empty(),
                "a broken <{element}> resource is ASSET territory, not a broken external link"
            );
        }
    }

    // --- HTTP-REDIRECT-CHAIN ---

    #[test]
    fn a_two_hop_chain_is_reported_at_its_head() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/c", 200));

        let hallazgos = HttpRedirectChain.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "only the head of the chain");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"hops\":2"), "unexpected detail: {detalle}");
        assert!(detalle.contains("https://ejemplo.es/c"), "the target is missing: {detalle}");
    }

    #[test]
    fn a_single_hop_is_not_a_chain() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/nuevo", 200));

        assert!(HttpRedirectChain.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_loop_does_not_count_as_a_chain() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(1));

        assert!(HttpRedirectChain.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_crawl_without_redirects_produces_no_chain_findings() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        assert!(HttpRedirectChain.evaluate(&conn).expect("evaluate").is_empty());
    }

    // --- HTTP-REDIRECT-LOOP ---

    #[test]
    fn a_two_url_loop_is_reported_exactly_once() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(1));

        let hallazgos = HttpRedirectLoop.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "one cycle, one finding");
        assert_eq!(hallazgos[0].1.rule_id, "HTTP-REDIRECT-LOOP");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"length\":2"), "unexpected detail: {detalle}");
        assert!(hallazgos[0].1.group_key.is_some(), "the loop must group for the diff");
    }

    #[test]
    fn a_url_redirecting_to_itself_produces_a_finding() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 302).hacia(1));

        let hallazgos = HttpRedirectLoop.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn a_chain_entering_a_loop_does_not_duplicate_it() {
        // X → A → B → A. The cycle is {A, B}, and from X you reach that same one: one finding.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/x", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/a", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/b", 301).hacia(2));

        let hallazgos = HttpRedirectLoop.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
    }

    #[test]
    fn a_chain_that_ends_well_is_not_a_loop() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/c", 200));

        assert!(HttpRedirectLoop.evaluate(&conn).expect("evaluate").is_empty());
    }

    // --- HTTP-REDIRECT-TO-404 ---

    #[test]
    fn a_redirect_ending_in_a_404_produces_a_finding() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/no-existe", 404));

        let hallazgos = HttpRedirectTo404.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.rule_id, "HTTP-REDIRECT-TO-404");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/viejo"))]);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"final_status_code\":404"), "unexpected detail: {detalle}");
    }

    #[test]
    fn a_chain_ending_in_a_404_is_reported_at_the_head() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/c", 410));

        let hallazgos = HttpRedirectTo404.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
    }

    #[test]
    fn a_redirect_that_ends_well_produces_no_finding() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/nuevo", 200));

        assert!(HttpRedirectTo404.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn a_redirect_to_an_uncrawled_destination_produces_no_finding() {
        // Without the destination's status it cannot be claimed to be an error.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state)
             VALUES (2, 'https://ejemplo.es/pendiente', 0, 'https', 'ejemplo.es', '/pendiente',
                     1, 0, 'pending')",
            [],
        )
        .expect("insert pending");

        assert!(HttpRedirectTo404.evaluate(&conn).expect("evaluate").is_empty());
    }

    // --- HTTP-NO-HTTPS ---

    #[test]
    fn a_site_answering_over_http_is_reported_exactly_once() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/", 200));
        insertar(&conn, Fila::interna(2, "http://ejemplo.es/otra", 200));

        let hallazgos = HttpNoHttps.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1, "it is a server configuration, not a per-URL defect");
        assert_eq!(hallazgos[0].0, None, "it is a site finding");
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"http_urls\":2"), "unexpected detail: {detalle}");
    }

    #[test]
    fn a_fully_https_site_produces_no_finding() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        assert!(HttpNoHttps.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn an_http_url_redirecting_to_https_is_the_right_setup() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/", 200));

        assert!(HttpNoHttps.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn an_http_url_redirecting_to_another_http_url_is_still_wrong() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "http://ejemplo.es/nuevo", 200));

        let hallazgos = HttpNoHttps.evaluate(&conn).expect("evaluate");
        assert_eq!(hallazgos.len(), 1);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"http_urls\":2"), "unexpected detail: {detalle}");
    }

    #[test]
    fn an_http_404_says_nothing_about_how_https_is_configured() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/no-existe", 404));
        assert!(HttpNoHttps.evaluate(&conn).expect("evaluate").is_empty());
    }

    #[test]
    fn an_external_url_over_http_is_not_this_sites_concern() {
        let conn = db();
        insertar(&conn, Fila::externa(1, "http://otro.example/", Some(200)));
        assert!(HttpNoHttps.evaluate(&conn).expect("evaluate").is_empty());
    }

    // --- Graph walk ---

    #[test]
    fn the_walk_cuts_off_a_pathological_graph() {
        // A chain longer than the cap: it stops and says so, instead of spinning forever.
        let mut saltos = BTreeMap::new();
        for id in 1..=(MAX_REDIRECT_HOPS as i64 + 5) {
            saltos.insert(
                id,
                RedirectHop {
                    url_hash: id,
                    url: format!("https://ejemplo.es/{id}"),
                    redirect_to: id + 1,
                },
            );
        }
        let (camino, fin) = walk(1, &saltos);
        assert_eq!(fin, ChainEnd::TooLong);
        assert_eq!(camino.len(), MAX_REDIRECT_HOPS + 1);
    }
}
