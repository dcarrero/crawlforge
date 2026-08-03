//! `HTTP` — códigos de estado y redirecciones. `docs/04-CATALOGO-REGLAS.md §3`.
//!
//! # `HTTP-TEMP-REDIRECT` no está aquí, y no es un olvido
//!
//! Su condición catalogada es «302/307 **permanente en el tiempo** (aparece en 2+ rastreos)», y
//! comparar dos rastreos exige un histórico de comparaciones que todavía no existe. Con un solo
//! fichero de rastreo delante no hay forma de distinguir un 302 legítimo —un mantenimiento, un
//! A/B, una promoción de temporada— de uno que lleva dos años puesto, y avisar de todos los 302
//! sería exactamente el ruido que hace que un auditor se ignore. Se implementará cuando exista
//! el diff, no antes.
//!
//! # Cómo se recorren las redirecciones
//!
//! El motor guarda cada salto como una fila de `urls` con `redirect_to` apuntando a la
//! siguiente (`docs/02-MODELO-DATOS.md §3.2`). Las tres reglas de cadena cargan en memoria
//! **solo las filas que redirigen** —unas pocas docenas en cualquier rastreo real— y recorren
//! ese grafo en Rust, en vez de con un `WITH RECURSIVE`. Dos motivos: la detección de ciclos se
//! lee, y un solo recorrido resuelve las tres preguntas (cuántos saltos, si vuelve sobre sí
//! mismo, y en qué acaba).

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};

/// TTFB por encima del cual se avisa, en milisegundos.
///
/// El umbral del catálogo (§3) es 1.000 ms: es el punto en que la latencia deja de ser un
/// detalle y empieza a costar rastreo y conversión. Se compara con `>`, así que 1.000 ms justos
/// no avisan.
pub const SLOW_RESPONSE_MS: u32 = 1_000;

/// Tamaño de HTML por encima del cual se avisa. 500 KB del catálogo, en KiB (512.000 bytes).
///
/// Es el HTML solo, sin imágenes ni CSS ni JS: medio megabyte de marcado es casi siempre una
/// plantilla que vuelca la base de datos entera en la página.
pub const LARGE_PAGE_BYTES: u64 = 500 * 1024;

/// Saltos a partir de los cuales una redirección deja de ser una redirección y es una cadena.
///
/// Uno es normal y correcto (`/viejo` → `/nuevo`). Dos ya es una cadena: pierde parte del
/// PageRank en cada salto, multiplica la latencia del usuario y suele significar que hay dos
/// reglas de reescritura pisándose.
pub const REDIRECT_CHAIN_MIN_HOPS: usize = 2;

/// Tope de saltos que se recorren antes de rendirse.
///
/// Existe por seguridad, no por semántica: la detección de ciclos ya corta los bucles, y esto
/// solo protege de un grafo patológico. Una cadena que llega a 20 saltos ya está denunciada.
const MAX_REDIRECT_HOPS: usize = 20;

/// Cuántas URLs de ejemplo caben en el `detail_json` de un hallazgo.
///
/// Una página con 400 recursos por HTTP no debe meter 400 cadenas en la base de datos: la
/// cuenta va aparte y la lista es una muestra.
const MAX_SAMPLES: usize = 10;

// ---------------------------------------------------------------- Metadatos

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
    desc_es: "El sitio enlaza a una URL de otro dominio que ya devuelve 4xx. No penaliza como un \
              404 propio, pero manda al visitante a una página de error y envejece el contenido: \
              una guía llena de enlaces muertos deja de parecer mantenida.",
    desc_en: "The site links to a URL on another domain that now returns 4xx. It does not hurt \
              like a 404 of your own, but it sends the visitor to an error page and ages the \
              content: a guide full of dead links stops looking maintained.",
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

// ---------------------------------------------------------------- Reglas de página

/// La URL devuelve 5xx.
///
/// No se exige que la página sea indexable: un 5xx la hace no indexable por definición, así que
/// exigirlo silenciaría precisamente el hallazgo.
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

/// Página HTTPS que carga algún recurso por `http://` explícito.
///
/// Un solo hallazgo por página, con la cuenta y una muestra de los recursos: veinte imágenes mal
/// escritas son un mismo defecto de plantilla, no veinte problemas.
pub struct HttpMixedContent;

impl PageRule for HttpMixedContent {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_MIXED_CONTENT
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // El 2xx corta la plantilla de error: si el tema carga un recurso por `http://`, cada
        // URL rota del sitio repetiría este hallazgo. La causa es una y vive en la plantilla,
        // y del 404 ya avisa su regla. Ver `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_https || !ctx.is_success() {
            return Vec::new();
        }
        // Solo los recursos: un `<a href="http://...">` a otro sitio no es contenido mixto, es
        // un enlace. Lo que rompe el candado del navegador es lo que la página *carga*.
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

/// TTFB por encima de [`SLOW_RESPONSE_MS`].
///
/// `ttfb_ms` es `None` en el modo `filesystem`, donde leer un fichero del disco no es un TTFB y
/// medirlo sería inventarse un hallazgo. Ausencia de dato nunca es un hallazgo.
///
/// **No exige un 2xx a propósito**, al contrario que las reglas que auditan el HTML servido: el
/// TTFB mide el servidor, no la página, y un 404 que tarda dos segundos en llegar es el mismo
/// problema de servidor que un 200 que tarda dos segundos.
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

/// HTML por encima de [`LARGE_PAGE_BYTES`].
pub struct HttpLargePage;

impl PageRule for HttpLargePage {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_LARGE_PAGE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        // El umbral es del documento HTML. Medir el tamaño de un PDF o de una imagen con esta
        // regla daría un aviso que no se puede accionar. El 2xx corta la plantilla de error:
        // una plantilla de 404 obesa sería una fila por cada URL rota con una única causa, y
        // del 404 ya avisa su regla. Ver `PageContext::is_success`.
        if !ctx.is_html || !ctx.is_success() || ctx.html_bytes <= LARGE_PAGE_BYTES {
            return Vec::new();
        }
        vec![Issue::new(&HTTP_LARGE_PAGE).with_detail(serde_json::json!({
            "html_bytes": ctx.html_bytes,
            "threshold_bytes": LARGE_PAGE_BYTES,
        }))]
    }
}

/// ¿El `href` pide explícitamente `http://`?
///
/// Se mira el `href` **tal como venía en el HTML**: un `href` relativo (`/logo.png`) o
/// protocol-relative (`//cdn.ejemplo.com/logo.png`) hereda el esquema de la página, así que
/// nunca es contenido mixto. Solo lo es el que escribe `http://` a mano. La comprobación sigue
/// siendo correcta si el motor pasara el `href` ya resuelto a absoluto, porque en una página
/// HTTPS un relativo resuelto empieza por `https://`.
fn is_plain_http(href: &str) -> bool {
    let recortado = href.trim_start();
    recortado.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("http://"))
}

// ---------------------------------------------------------------- Reglas de conjunto

/// Una página interna del sitio devuelve 4xx y hay enlaces internos apuntándole.
///
/// El hallazgo se registra **en la página de destino**, con la cuenta de páginas que la
/// enlazan: es lo que hay que arreglar, y el detalle dice cuánto daño hace.
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

/// Una URL de otro dominio a la que el sitio enlaza devuelve 4xx.
///
/// Mismo criterio que [`Http404Internal`] —solo 4xx, no 5xx— y por el mismo motivo: un 5xx
/// ajeno es casi siempre un problema temporal del servidor del otro, y avisar de él haría que
/// el informe cambiara de un rastreo a otro sin que nadie hubiera tocado nada.
///
/// Requiere que el motor compruebe el estado de las URLs externas. Mientras no lo haga, sus
/// filas quedan con `status_code` nulo y la regla no encuentra nada, que es el comportamiento
/// correcto: no inventa hallazgos con datos que no tiene.
pub struct Http404External;

impl SiteRule for Http404External {
    fn meta(&self) -> &'static RuleMeta {
        &HTTP_404_EXTERNAL
    }

    fn evaluate(&self, conn: &Connection) -> rusqlite::Result<Vec<(Option<i64>, Issue)>> {
        let mut stmt = conn.prepare(
            "SELECT u.url_hash, u.url, u.status_code, COUNT(DISTINCT l.from_url_id) AS inlinks
             FROM urls u
             JOIN links l ON l.to_url_id = u.id
             WHERE u.is_internal = 0 AND u.status_code >= 400 AND u.status_code < 500
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
                Issue::new(&HTTP_404_EXTERNAL).with_detail(serde_json::json!({
                    "url": url,
                    "status_code": status,
                    "linked_from": inlinks
                })),
            ));
        }
        Ok(out)
    }
}

/// Dos o más saltos seguidos hasta el destino final.
///
/// El hallazgo se registra en la **cabeza** de la cadena: la URL que nadie redirige hacia ella y
/// que por tanto es la que aparece en los enlaces del sitio. Reportar también los eslabones
/// intermedios repetiría la misma cadena tres veces sin añadir nada que accionar.
///
/// Los bucles se saltan: los cuenta [`HttpRedirectLoop`], que es más grave y más concreto.
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

/// Las redirecciones vuelven sobre una URL ya visitada.
///
/// Un solo hallazgo por ciclo, en la URL de menor `id` del ciclo, para que un bucle de cuatro
/// saltos no aparezca cuatro veces. El `group_key` va por el `url_hash` de esa URL —no por su
/// `id`, que es un detalle de la fila y cambia entre rastreos— para que la comparación entre rastreos
/// pueda reconocer el mismo bucle de una semana a otra.
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

        // Se parte de cada nodo que redirige, no solo de las cabezas: un ciclo puro (A → B → A)
        // no tiene cabeza, porque todos sus nodos son destino de otro.
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

/// La cadena de redirecciones acaba en una URL que devuelve 4xx.
///
/// Como en [`HttpRedirectChain`], el hallazgo va en la cabeza de la cadena: es la URL que se
/// enlaza y la que hay que reapuntar. Un bucle no termina en ningún sitio, así que no cuenta
/// aquí.
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

/// Hay URLs internas que sirven contenido por HTTP sin llevar al visitante a HTTPS.
///
/// Un único hallazgo de sitio (`url_id` nulo), con la cuenta y una muestra: no es un defecto de
/// cada página, es una configuración del servidor, y listar 40.000 URLs no ayuda a arreglarla.
///
/// Qué cuenta como «responder por HTTP sin redirigir»:
///
/// - Un 2xx por `http://` — sirve el contenido tal cual.
/// - Un 3xx por `http://` cuyo destino sigue siendo `http://` — redirige, pero no a HTTPS.
///
/// Un 4xx o un 5xx no cuenta: no dice nada sobre cómo está configurado HTTPS.
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

// ---------------------------------------------------------------- Grafo de redirecciones

/// Una fila de `urls` que redirige a otra.
#[derive(Debug, Clone)]
struct RedirectHop {
    url_hash: i64,
    url: String,
    redirect_to: i64,
}

/// Dónde acaba un recorrido por el grafo de redirecciones.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChainEnd {
    /// Se llegó a una URL que no redirige. Es el `id` de esa URL.
    Final(i64),
    /// Se volvió sobre una URL ya visitada. `cycle` son los nodos del ciclo, en orden.
    Loop { cycle: Vec<i64> },
    /// Se agotó [`MAX_REDIRECT_HOPS`] sin cerrar ni terminar.
    TooLong,
}

/// Carga solo las filas que redirigen. En un rastreo de 100.000 URLs son unas pocas docenas.
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

/// Las cabezas de cadena: nodos que redirigen y a los que no redirige nadie.
fn chain_heads(saltos: &BTreeMap<i64, RedirectHop>) -> Vec<i64> {
    let destinos: BTreeSet<i64> = saltos.values().map(|h| h.redirect_to).collect();
    saltos.keys().copied().filter(|id| !destinos.contains(id)).collect()
}

/// Recorre la cadena desde `inicio`. Devuelve el camino (con `inicio` incluido) y su final.
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

/// Resuelve `id` → `(url, status_code)` por clave primaria, con caché.
///
/// Se consulta una vez por cadena, no una vez por fila del rastreo: cargar la tabla `urls`
/// entera para adornar veinte hallazgos sería justo el antipatrón §9.2.
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

// ---------------------------------------------------------------- Registro

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

    // ------------------------------------------------------------ Reglas de página

    fn ctx<'a>() -> PageContext<'a> {
        PageContext::indexable_html("https://ejemplo.es/a")
    }

    #[test]
    fn no_avisa_de_5xx_en_una_respuesta_correcta() {
        assert!(Http5xx.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_de_un_error_de_servidor() {
        let mut c = ctx();
        c.status = 503;
        let issues = Http5xx.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-5XX");
        assert_eq!(issues[0].severity, Severity::Critical);
    }

    #[test]
    fn un_4xx_no_es_un_error_de_servidor() {
        let mut c = ctx();
        c.status = 404;
        assert!(Http5xx.evaluate(&c).is_empty());
    }

    #[test]
    fn los_limites_del_rango_5xx() {
        for (estado, espera) in [(499u16, 0), (500, 1), (599, 1), (600, 0)] {
            let mut c = ctx();
            c.status = estado;
            assert_eq!(Http5xx.evaluate(&c).len(), espera, "estado {estado}");
        }
    }

    #[test]
    fn una_pagina_no_indexable_sigue_avisando_de_5xx() {
        // Un 5xx hace la página no indexable por definición: exigir indexabilidad silenciaría
        // el único hallazgo que importa.
        let mut c = ctx();
        c.status = 500;
        c.is_indexable = false;
        assert_eq!(Http5xx.evaluate(&c).len(), 1);
    }

    fn recurso<'a>(href: &'a str) -> LinkView<'a> {
        LinkView { href, is_resource: true, ..Default::default() }
    }

    #[test]
    fn no_avisa_de_contenido_mixto_sin_recursos_inseguros() {
        let enlaces = [recurso("https://cdn.ejemplo.es/x.js"), recurso("/estilo.css")];
        let mut c = ctx();
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_de_un_recurso_por_http_en_una_pagina_https() {
        let enlaces = [recurso("http://cdn.ejemplo.com/analitica.js")];
        let mut c = ctx();
        c.links = &enlaces;
        let issues = HttpMixedContent.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-MIXED-CONTENT");
    }

    #[test]
    fn varios_recursos_inseguros_son_un_solo_hallazgo() {
        let enlaces = [
            recurso("http://cdn.ejemplo.com/a.js"),
            recurso("http://cdn.ejemplo.com/b.css"),
            recurso("HTTP://CDN.EJEMPLO.COM/c.png"),
        ];
        let mut c = ctx();
        c.links = &enlaces;
        let issues = HttpMixedContent.evaluate(&c);
        assert_eq!(issues.len(), 1, "es un defecto de plantilla, no tres problemas");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"resources\":3"), "detalle inesperado: {detalle}");
    }

    #[test]
    fn un_enlace_por_http_no_es_contenido_mixto() {
        // Enlazar a un sitio ajeno que va por HTTP no rompe el candado del navegador: solo lo
        // rompe lo que la página *carga*.
        let enlaces = [LinkView { href: "http://otro.example/", ..Default::default() }];
        let mut c = ctx();
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn un_recurso_protocol_relative_no_es_contenido_mixto() {
        // `//host/x.js` hereda el esquema de la página, así que en HTTPS va por HTTPS.
        let enlaces = [recurso("//cdn.ejemplo.com/x.js")];
        let mut c = ctx();
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn una_pagina_http_no_tiene_contenido_mixto() {
        // Sin HTTPS no hay mezcla: el problema de esa página es otro, y lo cuenta HTTP-NO-HTTPS.
        let enlaces = [recurso("http://cdn.ejemplo.com/x.js")];
        let mut c = PageContext::indexable_html("http://ejemplo.es/a");
        c.links = &enlaces;
        assert!(!c.is_https);
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn no_se_busca_contenido_mixto_en_algo_que_no_es_html() {
        let enlaces = [recurso("http://cdn.ejemplo.com/x.js")];
        let mut c = ctx();
        c.is_html = false;
        c.links = &enlaces;
        assert!(HttpMixedContent.evaluate(&c).is_empty());
    }

    #[test]
    fn el_html_de_una_pagina_de_error_no_se_audita() {
        // La plantilla de error del tema se sirve una vez por cada URL rota: sin la puerta del
        // 2xx, un recurso `http://` o un HTML obeso en esa plantilla serían un hallazgo por
        // cada 404 del sitio, con una única causa. Ver `PageContext::is_success`.
        let enlaces = [recurso("http://cdn.ejemplo.com/x.js")];
        for status in [301, 404, 410, 500] {
            let mut c = ctx();
            c.status = status;
            c.links = &enlaces;
            assert!(
                HttpMixedContent.evaluate(&c).is_empty(),
                "HTTP-MIXED-CONTENT no debería auditar el HTML de un {status}"
            );

            let mut c = ctx();
            c.status = status;
            c.html_bytes = LARGE_PAGE_BYTES + 1;
            assert!(
                HttpLargePage.evaluate(&c).is_empty(),
                "HTTP-LARGE-PAGE no debería auditar el HTML de un {status}"
            );
        }
    }

    #[test]
    fn una_respuesta_lenta_avisa_con_cualquier_estado() {
        // El TTFB mide el servidor, no el HTML: un 404 lento es el mismo problema de servidor
        // que un 200 lento, y por eso esta regla no lleva la puerta del 2xx.
        let mut c = ctx();
        c.status = 404;
        c.ttfb_ms = Some(SLOW_RESPONSE_MS + 500);
        assert_eq!(HttpSlowResponse.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_avisa_de_lentitud_sin_medida_de_ttfb() {
        // Modo `filesystem`: no hay red, así que no hay TTFB y no hay hallazgo posible.
        let mut c = ctx();
        c.ttfb_ms = None;
        assert!(HttpSlowResponse.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_de_una_respuesta_lenta() {
        let mut c = ctx();
        c.ttfb_ms = Some(1_500);
        let issues = HttpSlowResponse.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-SLOW-RESPONSE");
    }

    #[test]
    fn el_umbral_de_lentitud_no_es_inclusivo() {
        let mut c = ctx();
        c.ttfb_ms = Some(SLOW_RESPONSE_MS);
        assert!(HttpSlowResponse.evaluate(&c).is_empty());
        c.ttfb_ms = Some(SLOW_RESPONSE_MS + 1);
        assert_eq!(HttpSlowResponse.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_avisa_del_tamano_de_una_pagina_normal() {
        assert!(HttpLargePage.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_de_un_html_demasiado_grande() {
        let mut c = ctx();
        c.html_bytes = 700_000;
        let issues = HttpLargePage.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "HTTP-LARGE-PAGE");
    }

    #[test]
    fn el_umbral_de_tamano_no_es_inclusivo() {
        let mut c = ctx();
        c.html_bytes = LARGE_PAGE_BYTES;
        assert!(HttpLargePage.evaluate(&c).is_empty());
        c.html_bytes = LARGE_PAGE_BYTES + 1;
        assert_eq!(HttpLargePage.evaluate(&c).len(), 1);
    }

    #[test]
    fn el_umbral_de_tamano_solo_aplica_al_html() {
        let mut c = ctx();
        c.is_html = false;
        c.html_bytes = 5_000_000;
        assert!(HttpLargePage.evaluate(&c).is_empty());
    }

    // ------------------------------------------------------------ Reglas de conjunto
    //
    // Ninguna de estas se puede provocar con un fixture de sistema de ficheros: sin servidor no
    // hay 3xx, ni 5xx, ni comprobación de URLs ajenas. La base en memoria es la única forma de
    // demostrar el SQL, así que aquí se monta el esquema real y se insertan las filas mínimas.

    fn db() -> Connection {
        let conn = Connection::open_in_memory().expect("abrir en memoria");
        conn.execute_batch(include_str!("../../crawlforge-core/migrations/001_initial.sql"))
            .expect("cargar el esquema 001");
        // Las filas se insertan con `redirect_to` apuntando a `id` que aún no existen, y un
        // bucle de redirección es circular por definición: no hay ningún orden de inserción que
        // satisfaga la clave ajena. El motor real escribe por lotes en una transacción; aquí
        // basta con no exigirla.
        conn.pragma_update(None, "foreign_keys", false).expect("desactivar claves ajenas");
        conn
    }

    fn hash(url: &str) -> i64 {
        xxhash_rust::xxh3::xxh3_64(url.as_bytes()) as i64
    }

    /// Fila mínima de `urls`. `id` explícito para poder encadenar `redirect_to` a mano.
    struct Fila<'a> {
        id: i64,
        url: &'a str,
        internal: bool,
        status: Option<i64>,
        redirect_to: Option<i64>,
    }

    impl<'a> Fila<'a> {
        fn interna(id: i64, url: &'a str, status: i64) -> Self {
            Self { id, url, internal: true, status: Some(status), redirect_to: None }
        }

        fn externa(id: i64, url: &'a str, status: Option<i64>) -> Self {
            Self { id, url, internal: false, status, redirect_to: None }
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
                               crawl_state, status_code, redirect_to, redirect_chain_len)
             VALUES (?1,?2,?3,?4,?5,?6,?7,0,'done',?8,?9,0)",
            params![
                f.id,
                f.url,
                hash(f.url),
                esquema,
                host,
                path,
                f.internal as i64,
                f.status,
                f.redirect_to
            ],
        )
        .expect("insertar url");
    }

    fn enlazar(conn: &Connection, desde: i64, hacia: i64) {
        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, element) VALUES (?1, ?2, 'a')",
            params![desde, hacia],
        )
        .expect("insertar enlace");
    }

    fn ids(hallazgos: &[(Option<i64>, Issue)]) -> Vec<Option<i64>> {
        hallazgos.iter().map(|(h, _)| *h).collect()
    }

    // --- HTTP-404-EXTERNAL ---

    #[test]
    fn avisa_de_un_enlace_externo_roto() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::externa(2, "https://otro.example/muerta", Some(404)));
        enlazar(&conn, 1, 2);

        let hallazgos = Http404External.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.rule_id, "HTTP-404-EXTERNAL");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://otro.example/muerta"))]);
    }

    #[test]
    fn un_404_interno_no_lo_cuenta_la_regla_de_externos() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/no-existe", 404));
        enlazar(&conn, 1, 2);

        assert!(Http404External.evaluate(&conn).expect("evaluar").is_empty());
        assert_eq!(Http404Internal.evaluate(&conn).expect("evaluar").len(), 1);
    }

    #[test]
    fn una_url_externa_sin_estado_comprobado_no_da_hallazgo() {
        // Con `--no-external-check`, y con cualquier sonda que no llegue a completarse, la
        // externa queda registrada sin estado. Sin dato no hay hallazgo: la regla calla en vez
        // de afirmar que el enlace está bien, que sería mentir por omisión.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::externa(2, "https://otro.example/quizas", None));
        enlazar(&conn, 1, 2);

        assert!(Http404External.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn un_5xx_externo_no_cuenta_como_enlace_roto() {
        // Casi siempre es un problema temporal del servidor ajeno: reportarlo haría que el
        // informe cambiara de un rastreo a otro sin que nadie hubiera tocado nada.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        insertar(&conn, Fila::externa(2, "https://otro.example/caida", Some(503)));
        enlazar(&conn, 1, 2);

        assert!(Http404External.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_externa_rota_a_la_que_nadie_enlaza_no_da_hallazgo() {
        let conn = db();
        insertar(&conn, Fila::externa(1, "https://otro.example/muerta", Some(410)));
        assert!(Http404External.evaluate(&conn).expect("evaluar").is_empty());
    }

    // --- HTTP-REDIRECT-CHAIN ---

    #[test]
    fn avisa_de_una_cadena_de_dos_saltos_en_su_cabeza() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/c", 200));

        let hallazgos = HttpRedirectChain.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1, "solo la cabeza de la cadena");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"hops\":2"), "detalle inesperado: {detalle}");
        assert!(detalle.contains("https://ejemplo.es/c"), "falta el destino: {detalle}");
    }

    #[test]
    fn un_solo_salto_no_es_una_cadena() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/nuevo", 200));

        assert!(HttpRedirectChain.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn un_bucle_no_se_cuenta_como_cadena() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(1));

        assert!(HttpRedirectChain.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn un_rastreo_sin_redirecciones_no_da_hallazgos_de_cadena() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        assert!(HttpRedirectChain.evaluate(&conn).expect("evaluar").is_empty());
    }

    // --- HTTP-REDIRECT-LOOP ---

    #[test]
    fn avisa_una_sola_vez_de_un_bucle_de_dos_urls() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(1));

        let hallazgos = HttpRedirectLoop.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1, "un ciclo, un hallazgo");
        assert_eq!(hallazgos[0].1.rule_id, "HTTP-REDIRECT-LOOP");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"length\":2"), "detalle inesperado: {detalle}");
        assert!(hallazgos[0].1.group_key.is_some(), "el bucle debe agrupar para el diff");
    }

    #[test]
    fn avisa_de_una_url_que_redirige_a_si_misma() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 302).hacia(1));

        let hallazgos = HttpRedirectLoop.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
    }

    #[test]
    fn una_cadena_que_entra_en_un_bucle_no_lo_duplica() {
        // X → A → B → A. El ciclo es {A, B}, y desde X se llega al mismo: un solo hallazgo.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/x", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/a", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/b", 301).hacia(2));

        let hallazgos = HttpRedirectLoop.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
    }

    #[test]
    fn una_cadena_que_termina_bien_no_es_un_bucle() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/c", 200));

        assert!(HttpRedirectLoop.evaluate(&conn).expect("evaluar").is_empty());
    }

    // --- HTTP-REDIRECT-TO-404 ---

    #[test]
    fn avisa_de_una_redireccion_que_acaba_en_404() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/no-existe", 404));

        let hallazgos = HttpRedirectTo404.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].1.rule_id, "HTTP-REDIRECT-TO-404");
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/viejo"))]);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"final_status_code\":404"), "detalle inesperado: {detalle}");
    }

    #[test]
    fn una_cadena_que_acaba_en_404_se_reporta_en_la_cabeza() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/a", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/b", 301).hacia(3));
        insertar(&conn, Fila::interna(3, "https://ejemplo.es/c", 410));

        let hallazgos = HttpRedirectTo404.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        assert_eq!(ids(&hallazgos), vec![Some(hash("https://ejemplo.es/a"))]);
    }

    #[test]
    fn una_redireccion_que_acaba_bien_no_da_hallazgo() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/nuevo", 200));

        assert!(HttpRedirectTo404.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_redireccion_a_un_destino_sin_rastrear_no_da_hallazgo() {
        // Sin estado del destino no se puede afirmar que sea un error.
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/viejo", 301).hacia(2));
        conn.execute(
            "INSERT INTO urls (id, url, url_hash, scheme, host, path, is_internal, in_sitemap,
                               crawl_state)
             VALUES (2, 'https://ejemplo.es/pendiente', 0, 'https', 'ejemplo.es', '/pendiente',
                     1, 0, 'pending')",
            [],
        )
        .expect("insertar pendiente");

        assert!(HttpRedirectTo404.evaluate(&conn).expect("evaluar").is_empty());
    }

    // --- HTTP-NO-HTTPS ---

    #[test]
    fn avisa_una_sola_vez_de_que_el_sitio_responde_por_http() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/", 200));
        insertar(&conn, Fila::interna(2, "http://ejemplo.es/otra", 200));

        let hallazgos = HttpNoHttps.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1, "es una configuración del servidor, no un defecto por URL");
        assert_eq!(hallazgos[0].0, None, "es un hallazgo de sitio");
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"http_urls\":2"), "detalle inesperado: {detalle}");
    }

    #[test]
    fn un_sitio_entero_por_https_no_da_hallazgo() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "https://ejemplo.es/", 200));
        assert!(HttpNoHttps.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_url_http_que_redirige_a_https_es_lo_correcto() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "https://ejemplo.es/", 200));

        assert!(HttpNoHttps.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_url_http_que_redirige_a_otra_http_sigue_estando_mal() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/viejo", 301).hacia(2));
        insertar(&conn, Fila::interna(2, "http://ejemplo.es/nuevo", 200));

        let hallazgos = HttpNoHttps.evaluate(&conn).expect("evaluar");
        assert_eq!(hallazgos.len(), 1);
        let detalle = hallazgos[0].1.detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"http_urls\":2"), "detalle inesperado: {detalle}");
    }

    #[test]
    fn un_404_por_http_no_dice_nada_de_como_esta_configurado_https() {
        let conn = db();
        insertar(&conn, Fila::interna(1, "http://ejemplo.es/no-existe", 404));
        assert!(HttpNoHttps.evaluate(&conn).expect("evaluar").is_empty());
    }

    #[test]
    fn una_url_externa_por_http_no_es_asunto_del_sitio() {
        let conn = db();
        insertar(&conn, Fila::externa(1, "http://otro.example/", Some(200)));
        assert!(HttpNoHttps.evaluate(&conn).expect("evaluar").is_empty());
    }

    // --- Recorrido del grafo ---

    #[test]
    fn el_recorrido_corta_un_grafo_patologico() {
        // Cadena más larga que el tope: se para y lo dice, en vez de girar para siempre.
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
