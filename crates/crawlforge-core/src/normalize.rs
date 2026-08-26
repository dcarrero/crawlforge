//! Normalización de URL. Ver `docs/03-MOTOR-CRAWL.md §3`.
//!
//! Rastrear la misma página cincuenta veces con querystrings distintas es el error más común
//! y el más caro de un crawler. Este módulo es la defensa.
//!
//! Se conservan **ambas** formas: la URL tal como aparecía en el HTML (para los informes) y la
//! normalizada (para deduplicar). Ver [`NormalizedUrl`].

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use url::Url;

/// Parámetros de query que se descartan por defecto: no cambian el contenido servido,
/// solo la atribución de marketing.
pub const DEFAULT_STRIPPED_PARAMS: &[&str] = &[
    "gclid", "fbclid", "msclkid", "mc_cid", "mc_eid", "_ga", "ref", "si",
];

/// Prefijos de parámetro que se descartan por defecto. `utm_*` es una familia entera.
pub const DEFAULT_STRIPPED_PREFIXES: &[&str] = &["utm_"];

/// Qué hacer con la barra final.
///
/// La regla 8 de `§3` dice que se normalice «según lo que responda el servidor en la primera
/// resolución del host, no según una suposición». Por eso el valor por defecto es
/// [`TrailingSlash::AsIs`]: hasta que el motor haya observado el comportamiento real del host,
/// tocar la barra final inventa URLs que pueden no existir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailingSlash {
    /// No tocar. Por defecto, hasta que el host haya sido observado.
    #[default]
    AsIs,
    /// El host sirve los directorios con barra final (301 de `/a` a `/a/`).
    Add,
    /// El host sirve los directorios sin barra final (301 de `/a/` a `/a`).
    Remove,
}

/// Política de normalización. Configurable por rastreo.
#[derive(Debug, Clone)]
pub struct NormalizePolicy {
    pub stripped_params: Vec<String>,
    pub stripped_prefixes: Vec<String>,
    pub trailing_slash: TrailingSlash,
    /// Si es `true`, también se elimina el fragmento hashbang `#!` (SPA legadas).
    pub strip_hashbang: bool,
}

impl Default for NormalizePolicy {
    fn default() -> Self {
        Self {
            stripped_params: DEFAULT_STRIPPED_PARAMS.iter().map(|s| s.to_string()).collect(),
            stripped_prefixes: DEFAULT_STRIPPED_PREFIXES.iter().map(|s| s.to_string()).collect(),
            trailing_slash: TrailingSlash::default(),
            strip_hashbang: false,
        }
    }
}

impl NormalizePolicy {
    fn should_strip(&self, key: &str) -> bool {
        let lower = key.to_ascii_lowercase();
        self.stripped_params.iter().any(|p| p.eq_ignore_ascii_case(&lower))
            || self.stripped_prefixes.iter().any(|p| lower.starts_with(p.as_str()))
    }
}

/// Una URL normalizada, junto con la forma original en que apareció.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedUrl {
    /// Forma canónica: la que deduplica y la que se indexa por hash.
    pub normalized: Url,
    /// Tal como aparecía en el HTML. Es lo que se enseña en los informes: si el enlace del
    /// sitio está mal escrito, el informe debe mostrarlo mal escrito.
    pub original: String,
}

impl NormalizedUrl {
    /// Hash xxh3 de la forma normalizada. Es la clave del hot path «¿ya la visitamos?».
    ///
    /// Se guarda como `INTEGER` con signo porque SQLite no tiene enteros sin signo: el
    /// `as i64` reinterpreta los bits, no pierde información y mantiene la biyección.
    pub fn hash(&self) -> i64 {
        xxhash_rust::xxh3::xxh3_64(self.normalized.as_str().as_bytes()) as i64
    }
}

/// Normaliza una URL absoluta.
pub fn normalize(input: &str, policy: &NormalizePolicy) -> Result<NormalizedUrl, url::ParseError> {
    let parsed = Url::parse(input)?;
    Ok(finish(parsed, input.to_string(), policy))
}

/// Resuelve una URL posiblemente relativa contra la página que la contenía, y la normaliza.
///
/// Es la vía que usa el extractor de enlaces: en el HTML la mayoría de `href` son relativos.
pub fn normalize_relative(
    base: &Url,
    href: &str,
    policy: &NormalizePolicy,
) -> Result<NormalizedUrl, url::ParseError> {
    let joined = base.join(href)?;
    Ok(finish(joined, href.to_string(), policy))
}

/// El grueso del trabajo, común a ambas entradas.
///
/// El crate `url` ya aplica las reglas 1 (minúsculas de esquema y host, respetando la ruta),
/// 2 (puerto por defecto), 3 (resolución de `.` y `..`), 4 (recodificación consistente del
/// percent-encoding) y 9 (IDN a Punycode) al parsear. Aquí se añaden 5, 6, 7 y 8.
fn finish(mut url: Url, original: String, policy: &NormalizePolicy) -> NormalizedUrl {
    // Las credenciales (`usuario:contraseña@`) se vacían siempre, y se vacían aquí porque este
    // es el único embudo por el que pasa toda URL antes de escribirse: la forma normalizada
    // acaba en `urls.url` y de ahí en el CSV, el XLSX y la cabecera del informe que se manda
    // al cliente (revisión 2026-08-01 §1.6). No forman parte de la identidad de la página
    // —dos URLs que solo difieren en credenciales son el mismo recurso— y sí son un secreto.
    // Consecuencia deliberada: la autenticación básica ya no puede viajar en la URL; si algún
    // día hace falta, la credencial debe vivir en el `CrawlJob` y aplicarla el `Fetcher` como
    // cabecera, nunca en el dato que se persiste. `let _ =`: solo falla en URLs sin autoridad
    // (`mailto:`), donde no hay nada que vaciar.
    let _ = url.set_username("");
    let _ = url.set_password(None);

    // Regla 5 — eliminar el fragmento, salvo hashbang legado.
    match url.fragment() {
        Some(f) if f.starts_with('!') && !policy.strip_hashbang => {}
        _ => url.set_fragment(None),
    }

    // Reglas 6 y 7 — descartar parámetros de marketing y ordenar el resto alfabéticamente.
    //
    // El orden es por (clave, valor) para que `?a=2&a=1` sea estable. Un `?` que se queda sin
    // pares se elimina por completo: `ejemplo.es/a?` y `ejemplo.es/a` son la misma página.
    if url.query().is_some() {
        let mut pairs: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !policy.should_strip(k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        if pairs.is_empty() {
            url.set_query(None);
        } else {
            pairs.sort();
            let mut query = String::new();
            for (i, (k, v)) in pairs.iter().enumerate() {
                if i > 0 {
                    query.push('&');
                }
                query.push_str(k);
                query.push('=');
                query.push_str(v);
            }
            url.set_query(Some(&query));
        }
    }

    // Regla 8 — barra final, solo si el host ya fue observado.
    apply_trailing_slash(&mut url, policy.trailing_slash);

    NormalizedUrl { normalized: url, original }
}

/// La raíz (`/`) nunca se toca: `https://ejemplo.es` y `https://ejemplo.es/` son la misma URL,
/// y dejarla vacía produce una forma inválida.
fn apply_trailing_slash(url: &mut Url, policy: TrailingSlash) {
    if policy == TrailingSlash::AsIs {
        return;
    }
    let path = url.path().to_string();
    if path == "/" {
        return;
    }
    // Un último segmento con punto parece un fichero (`/style.css`, `/doc.pdf`): no lleva barra.
    let looks_like_file = path.rsplit('/').next().is_some_and(|s| s.contains('.'));

    match policy {
        TrailingSlash::Add if !path.ends_with('/') && !looks_like_file => {
            url.set_path(&format!("{path}/"));
        }
        TrailingSlash::Remove if path.ends_with('/') => {
            url.set_path(path.trim_end_matches('/'));
        }
        _ => {}
    }
}

/// La autoridad de una URL a efectos de «mismo sitio»: el host, y el puerto si no es el que el
/// esquema da por supuesto.
///
/// `https://ejemplo.es` y `https://ejemplo.es:443` dan la misma cadena, porque son el mismo
/// sitio escrito de dos maneras: el `Url` normaliza el puerto por defecto a `None` y aquí se
/// aprovecha.
pub fn site_authority(url: &Url) -> String {
    match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_string(),
        (None, _) => String::new(),
    }
}

/// ¿Es interna esta URL respecto a la semilla?
///
/// `seed_authority` es lo que devuelve [`site_authority`] para la primera semilla.
///
/// Los subdominios cuentan como externos: `blog.ejemplo.es` es otro sitio a efectos de
/// auditoría, y mezclarlos falsea el recuento de enlaces internos.
///
/// **El puerto también distingue**, y hasta la 0.10.0 no lo hacía. Auditando
/// `http://localhost:3000` —lo normal al revisar antes de desplegar—, un enlace a
/// `http://localhost:8080` se rastreaba como si fuera el mismo sitio: sus páginas entraban en el
/// recuento de internas, sus 404 salían como `HTTP-404-INTERNAL` y el grafo mezclaba dos
/// aplicaciones distintas. En producción es raro; en desarrollo es el pan de cada día, y es
/// justo donde se usa el modo que audita un `dist/` antes de publicarlo.
pub fn is_internal(url: &Url, seed_authority: &str) -> bool {
    // Se compara **sin construir la cadena**, y no es purismo: esto se llama una vez por enlace
    // de cada página, así que un `format!` aquí son millones de asignaciones en un rastreo
    // grande. La primera versión sí construía la autoridad con `site_authority` y costó un 24%
    // del rendimiento del bucle —de 107.000 elementos por segundo a 81.000, medido en release—.
    // `site_authority` se queda para la semilla, que se calcula una sola vez.
    let Some(host) = url.host_str() else { return false };
    match url.port() {
        // Sin puerto explícito, la autoridad es el host pelado.
        None => host.eq_ignore_ascii_case(seed_authority),
        Some(port) => {
            // El último `:` separa el puerto también en IPv6, porque el host llega entre
            // corchetes (`[::1]:8080`).
            let Some((seed_host, seed_port)) = seed_authority.rsplit_once(':') else {
                return false;
            };
            seed_host.eq_ignore_ascii_case(host) && seed_port.parse::<u16>() == Ok(port)
        }
    }
}

/// Esquemas que el motor rastrea. El resto (`mailto:`, `tel:`, `javascript:`, `data:`) se
/// descarta al extraer enlaces: no son páginas.
pub fn is_crawlable_scheme(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Is this a host we are willing to ask for on the user's behalf?
///
/// The engine only crawls the audited site, but the external status probe
/// (`CrawlLimits::check_external`, on by default) sends a request to an address **the audited
/// site chose**. A page serving `<a href="http://169.254.169.254/latest/meta-data/">` would
/// otherwise make the tool ask the user's own network for it, and what comes back —status,
/// response time and the raw `error_message`, which tells *connection refused* apart from
/// *timeout*— lands in `urls`, inside a file whose whole point is being sent to the client.
/// That is a map of the consultant's internal network drawn by a third party.
///
/// It is the same perimeter the sitemap path already has (see `discover_sitemap_urls`),
/// reached through the other door.
///
/// # This is a lexical screen and it is only the first of two lines
///
/// It decides on the **parsed** host, so every spelling of an address is covered: `url`
/// canonicalises `http://2130706433/` to `127.0.0.1` and `http://[::ffff:127.0.0.1]/` to its
/// mapped form before this function ever sees them, and there is a test for it. Getting the
/// IP literals right is not optional work that the resolver makes redundant: the connector
/// **never calls the resolver for a host that already parses as an address**, so for a literal
/// this function is the only screen there is.
///
/// What it cannot do is decide on a name. `localtest.me`, `app.lvh.me`, `10.0.0.5.nip.io` and
/// `169-254-169-254.sslip.io` are public wildcard-DNS services that answer with whatever
/// address is written into the name, and they cost an attacker nothing: no domain of their own,
/// no infrastructure, forty-five characters inside an `<a href>`. Verified end to end against
/// the engine's own probe with this screen on: `http://localtest.me:P/panel` reached a service
/// on loopback and came back 200.
///
/// That is what [`NetworkScreen::allows_address`] is for, and it is the line that actually
/// holds: it decides on the address the connection is about to dial, after resolution and
/// before the socket. This one stays because it is free and it cuts the hand-written case
/// without opening a connection.
pub fn is_probeable_host(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => is_public_ipv4(ip),
        Some(url::Host::Ipv6(ip)) => is_public_ipv6(ip),
        Some(url::Host::Domain(name)) => is_public_domain(name),
        // No authority at all: there is nothing to ask.
        None => false,
    }
}

/// Is this a routable address of the public internet?
///
/// The entry point of the screen that decides on an **address** rather than on a host: it is
/// what the resolver applies to every address a name answers with.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => is_public_ipv6(v6),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0                                  // 0.0.0.0/8, «this network»
        || ip.is_loopback()                   // 127/8
        || ip.is_private()                    // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local()                 // 169.254/16 — where cloud metadata lives
        || (a == 100 && (64..128).contains(&b)) // 100.64/10, carrier-grade NAT
        || (a == 192 && b == 0 && c == 0)     // 192.0.0/24, IETF assignments: Oracle's 192.0.0.192
        || (a == 192 && b == 0 && c == 2)     // 192.0.2/24, TEST-NET-1
        || (a == 198 && (18..20).contains(&b)) // 198.18/15, benchmarking
        || (a == 198 && b == 51 && c == 100)  // 198.51.100/24, TEST-NET-2
        || (a == 203 && b == 0 && c == 113)   // 203.0.113/24, TEST-NET-3
        || ip.is_multicast()                  // 224/4
        || ip.is_broadcast()
        || a >= 240) // 240/4, reserved
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    // An address that carries an IPv4 address inside decides as that IPv4 address, or every
    // embedding —mapped, compatible, translated, NAT64, 6to4— would be a way around the screen.
    // Reviewed one by one against the engine: `[::127.0.0.1]`, `[64:ff9b::a9fe:a9fe]` and
    // `[2002:a9fe:a9fe::]` all passed before this.
    if let Some(v4) = embedded_ipv4(ip) {
        return is_public_ipv4(v4);
    }
    let s = ip.segments();
    !(ip.is_loopback()                // ::1
        || ip.is_unspecified()        // ::
        || ip.is_multicast()          // ff00::/8
        || s[0] & 0xfe00 == 0xfc00    // fc00::/7, unique local
        || s[0] & 0xffc0 == 0xfe80    // fe80::/10, link-local unicast
        || s[0] & 0xffc0 == 0xfec0    // fec0::/10, deprecated site-local — still routed on LANs
        || (s[0] == 0x0064 && s[1] == 0xff9b) // rest of the NAT64 block: local-use /48
        || (s[0] == 0x0100 && s[1] == 0 && s[2] == 0 && s[3] == 0) // 100::/64, discard-only
        || (s[0] == 0x2001 && s[1] == 0)      // 2001::/32, Teredo: the v4 inside is obfuscated
        || (s[0] == 0x2001 && s[1] == 0x0002 && s[2] == 0) // 2001:2::/48, benchmarking
        || (s[0] == 0x2001 && s[1] == 0x0db8)) // 2001:db8::/32, documentation
}

/// The IPv4 address an IPv6 address carries inside, in any of its standard embeddings.
///
/// Returns `None` for a native IPv6 address, which is the common case.
fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    let last_two = |s: &[u16; 8]| Ipv4Addr::from(((s[6] as u32) << 16) | s[7] as u32);
    // `::a.b.c.d` (IPv4-compatible) and `::ffff:a.b.c.d` (IPv4-mapped). `to_ipv4` covers both;
    // `::` and `::1` come out as 0.0.0.0 and 0.0.0.1, which `is_public_ipv4` refuses anyway.
    if let Some(v4) = ip.to_ipv4() {
        return Some(v4);
    }
    // `::ffff:0:a.b.c.d`, IPv4-translated (RFC 6145).
    if s[0..4] == [0, 0, 0, 0] && s[4] == 0xffff && s[5] == 0 {
        return Some(last_two(&s));
    }
    // `64:ff9b::a.b.c.d`, the well-known NAT64 prefix (RFC 6052). It is decided on the address
    // inside and **not** blocked wholesale: on an IPv6-only network with DNS64 this is what the
    // resolver answers for every A record, so refusing the prefix would refuse the internet.
    if s[0] == 0x0064 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        return Some(last_two(&s));
    }
    // `2002:a.b.c.d::/48`, 6to4 (RFC 3056).
    if s[0] == 0x2002 {
        return Some(Ipv4Addr::from(((s[1] as u32) << 16) | s[2] as u32));
    }
    None
}

/// Is this the cloud metadata endpoint — the one address that is screened **always**?
///
/// [`is_probeable_host`] is only consulted when the audited site is public, and the reasoning
/// for that is sound: auditing an `astro dev` on `localhost` or a client's staging on the office
/// LAN means whoever launched the crawl is already inside that network. Screening there protects
/// nobody and breaks a real use case.
///
/// This address is the exception, and the case that breaks the reasoning is concrete: a crawl of
/// `http://localhost:4321/` **from a cloud runner in CI**. The seed is local, so the screen is
/// off, and `169.254.169.254` answers with the instance's IAM credentials. Nothing legitimate
/// ever links there, so screening it costs nothing and closes the highest-value target in the
/// whole range.
///
/// The whole of 169.254/16 is screened and not just the single address: the same link-local
/// range holds the metadata endpoints of every provider, and enumerating them by address is a
/// list that goes stale.
fn is_cloud_metadata(name_or_ip: &Url) -> bool {
    match name_or_ip.host() {
        Some(url::Host::Ipv4(ip)) => is_metadata_ip(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => is_metadata_ip(IpAddr::V6(ip)),
        _ => false,
    }
}

/// The metadata addresses, decided on an address instead of on a host.
///
/// It is the same rule as [`is_cloud_metadata`] and it exists apart because the resolver needs
/// it: `169.254.169.254.nip.io` is a public name that answers `169.254.169.254`, so the whole
/// «screened always» exception was undone by an `<a href>` until the screen learnt to decide
/// after resolution.
fn is_metadata_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_link_local()
                // 192.0.0.192, Oracle Cloud Classic. 100.100.100.200, Alibaba Cloud.
                || v4 == Ipv4Addr::new(192, 0, 0, 192)
                || v4 == Ipv4Addr::new(100, 100, 100, 200)
        }
        IpAddr::V6(v6) => {
            embedded_ipv4(v6).is_some_and(|v4| is_metadata_ip(IpAddr::V4(v4)))
                // fd00:ec2::254, the IPv6 metadata address on AWS.
                || (v6.segments()[0] == 0xfd00 && v6.segments()[1] == 0x0ec2)
        }
    }
}

/// Should this URL be left unprobed, given whether the audited site is public?
///
/// Kept as the free-function form of the lexical screen — [`NetworkScreen`] is what the engine
/// uses, because «is the audited site public» turned out to be the wrong question: in list mode
/// it was answered by the first line of a file that often comes from outside.
///
/// The two halves are separate on purpose: the network screen is conditional and defensible,
/// the metadata screen is not negotiable. See [`is_cloud_metadata`].
pub fn is_probeable(url: &Url, screen_local_network: bool) -> bool {
    if is_cloud_metadata(url) {
        return false;
    }
    !screen_local_network || is_probeable_host(url)
}

/// Suffixes that never belong to a host on the public internet.
///
/// A name matches when it **is** one of these or ends in `.` plus one of them. The list is the
/// reserved and collision-prone top levels: RFC 6761/6762/8375 (`localhost`, `local`, `invalid`,
/// `test`, `example`, `home.arpa`), RFC 8375's parent `arpa`, ICANN's own private-use `internal`,
/// and the names that ICANN froze for name collision and that every home router and service mesh
/// helped itself to anyway.
///
/// `lan` is the cheapest and most common of the lot: it is the default local domain of OpenWrt
/// and of half the consumer routers on the market, and it sat right next to `local`, which was
/// screened.
/// Every entry has to be a top level that **nobody can register**: `host` and `zone` are real
/// gTLDs and were taken out of this list for that reason.
///
/// The documentation top levels of RFC 2606 —`example`, `test`, `invalid`— are deliberately
/// **not** here. They resolve to nothing, so screening them buys no protection, and they are the
/// vocabulary of half the fixtures in this repository.
const NON_PUBLIC_SUFFIXES: &[&str] = &[
    "localhost", "local", "internal", "intranet", "private", "lan", "corp", "home", "domain",
    "arpa", "onion", "alt", "default", "consul",
];

/// Whole names, not suffixes: the top level is a real gTLD that someone sells.
///
/// `fritz.box` is the AVM router, and `box` is a delegated gTLD, so the suffix cannot be
/// screened without screening real sites under it.
const NON_PUBLIC_NAMES: &[&str] = &["fritz.box"];

fn is_public_domain(name: &str) -> bool {
    // `url` already lowercases the host of an http(s) URL; the trailing dot of an absolute
    // name (`ejemplo.es.`) it does keep.
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    // A single-label name has no public top level to be under: it can only resolve through the
    // machine's own search domain, which is exactly what makes `metadata` —the short name
    // Google's own documentation uses— and `kubernetes` reach what they reach.
    if !name.contains('.') {
        return false;
    }
    if NON_PUBLIC_NAMES.contains(&name.as_str()) {
        return false;
    }
    !NON_PUBLIC_SUFFIXES
        .iter()
        .any(|suffix| name.len() > suffix.len() + 1 && name.ends_with(suffix) && {
            // `milocal.es` does not end in the suffix `local`; `nas.local` does. The character
            // before has to be the separator.
            name.as_bytes()[name.len() - suffix.len() - 1] == b'.'
        })
}

// ---------------------------------------------------------------- The audit perimeter

/// Which addresses this crawl is allowed to dial, and the two lines that decide it.
///
/// # Why a set of targets and not a switch
///
/// The screen used to be a single `bool` —«is the audited site public»— computed from the
/// **first** seed. In list mode the seeds are the user's file in file order and nothing
/// validates its lines, so the first line of a file that often arrives from outside decided the
/// perimeter for every other line in it. Measured, three passes: with a public first line the
/// victim on loopback got 0 requests; with `http://localhost:P/dev` first, 1; and with
/// `mailto:contacto@cliente.es` first —no host at all, so the switch read «local»— also 1.
///
/// # The criterion
///
/// In order, and the first one that answers wins:
///
/// 1. **The cloud metadata range is never reachable.** Not through a target, not through a
///    wholly local crawl, never. Nothing legitimate links there and that address answers with
///    the instance's credentials.
/// 2. **A public address is always reachable.** That is the job.
/// 3. **A host the user named is reachable at its own address.** This is what keeps a public
///    name that answers a private address through split-horizon DNS working — auditing
///    `pre.cliente.es` from inside the office — which a bare address screen would break.
/// 4. **A wholly local audit reaches its own network.** Only when **every** host the user named
///    is itself a local address: auditing an `astro dev` on `localhost` or a client's staging
///    on the office LAN means whoever launched the crawl is already inside that network. This
///    is the old exemption, kept, and narrowed to the case that justified it. A crawl that
///    names one public site does not get it, however many local ones it also names — which is
///    what turns the three list-mode passes above into zero requests.
/// 5. Otherwise, no.
///
/// An empty target set is «nothing is exempt»: a `Default` value must not open the perimeter,
/// and a list with no usable host at all is not a local audit.
#[derive(Debug, Clone, Default)]
pub struct NetworkScreen {
    /// Hosts the user named when launching the crawl, lowercased.
    targets: Arc<HashSet<String>>,
    /// Rule 4: every named host is itself a local address, so this is a local audit.
    local_audit: bool,
}

impl NetworkScreen {
    /// The perimeter of a crawl whose declared targets are these hosts.
    pub fn for_targets<I, S>(hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let targets: HashSet<String> =
            hosts.into_iter().map(|h| h.as_ref().to_ascii_lowercase()).collect();
        let local_audit = !targets.is_empty() && targets.iter().all(|h| !host_is_public(h));
        Self { targets: Arc::new(targets), local_audit }
    }

    /// The perimeter of a crawl seeded with these URLs. A seed without a host contributes
    /// nothing — and, unlike before, takes nothing away either.
    pub fn for_seeds<'a>(seeds: impl IntoIterator<Item = &'a Url>) -> Self {
        let mut targets: HashSet<String> = HashSet::new();
        let mut local_audit = true;
        let mut any = false;
        for url in seeds {
            let Some(host) = url.host_str() else { continue };
            any = true;
            targets.insert(host.to_ascii_lowercase());
            local_audit &= !is_probeable_host(url);
        }
        Self { targets: Arc::new(targets), local_audit: any && local_audit }
    }

    /// Did the user name this host when launching the crawl?
    pub fn is_target(&self, host: &str) -> bool {
        if self.targets.is_empty() {
            return false;
        }
        // The common case is already lowercase: `url` lowercases the host of an http(s) URL.
        self.targets.contains(host) || self.targets.contains(&host.to_ascii_lowercase())
    }

    /// Is every host this crawl was pointed at a local address? See rule 4 above.
    pub fn is_local_audit(&self) -> bool {
        self.local_audit
    }

    /// The same perimeter without rule 4: only the declared targets are exempt.
    ///
    /// # Known gap, and why this is not wired into `resume`
    ///
    /// Rule 4 is a statement about what the **user** typed. On a resume the user typed a path:
    /// the target comes out of `crawl_meta`, and «a crawl = a portable file» means that file is
    /// shared and therefore untrusted. A `.sqlite` that declares `base_url =
    /// http://localhost:4321/` and carries one injected external row makes the machine that
    /// opens it probe a service on its own loopback — reproduced, and still reproducible.
    ///
    /// Applying this on the resume path closes it and costs something real: resuming a wholly
    /// local audit stops re-probing the external links that pointed at other addresses on that
    /// same local network, which is a feature that landed this same week. The engine therefore
    /// **logs a warning instead**, and the proper fix belongs one level up — `resume` on a file
    /// whose target is local should ask the user, the way `ignore_robots` does, rather than
    /// having the core guess. Until then this method is the piece that fix will need.
    pub fn without_local_audit(self) -> Self {
        Self { targets: self.targets, local_audit: false }
    }

    /// First line: does the URL's **host as written** clear the perimeter?
    ///
    /// Cheap, and it decides without opening a connection. It cannot catch a name, and for an
    /// IP literal it is the only screen there is — the connector skips the resolver when the
    /// host already parses as an address.
    pub fn allows_host(&self, url: &Url) -> bool {
        if is_cloud_metadata(url) {
            return false;
        }
        if is_probeable_host(url) {
            return true;
        }
        self.local_audit || url.host_str().is_some_and(|h| self.is_target(h))
    }

    /// Second line: may this connection be made to this address?
    ///
    /// Applied by [`crate::dns::ScreeningResolver`] to **every** address a name resolves to,
    /// before any of them reaches a socket. `host` is the name being resolved, so a target of
    /// the crawl still reaches its own private address.
    pub fn allows_address(&self, host: &str, ip: IpAddr) -> bool {
        if is_metadata_ip(ip) {
            return false;
        }
        if is_public_ip(ip) || self.is_target(host) {
            return true;
        }
        // Rule 4, with the one shape it must not cover: **a public name that answers a private
        // address**.
        //
        // A local audit reaches its own network, and that is deliberate — whoever launched it is
        // inside that network already. What is never legitimate there is `10.0.0.5.nip.io`: a
        // name anyone can point anywhere, aimed at an address the operator never wrote down.
        // Everything the exemption actually exists for —`localhost`, `nas.lan`, a literal
        // `192.168.1.10`, a host named on the command line— is a name that is not public, and
        // goes through.
        //
        // It is the shape of a rebinding attack, and it costs nothing to refuse: no real local
        // service is reached by a public name that resolves privately and was not named.
        self.local_audit && !host_is_public(host)
    }
}

/// Is this host string —as it comes out of [`Url::host_str`]— a public one?
///
/// It has to go back through `Url` so that every spelling of an address is canonicalised the
/// same way the rest of the screen sees it. An IPv6 host arrives without its brackets, which is
/// why they are put back before parsing.
fn host_is_public(host: &str) -> bool {
    let raw =
        if host.contains(':') { format!("http://[{host}]/") } else { format!("http://{host}/") };
    Url::parse(&raw).is_ok_and(|u| is_probeable_host(&u))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> String {
        normalize(s, &NormalizePolicy::default())
            .expect("URL de test válida")
            .normalized
            .to_string()
    }

    fn norm_with(s: &str, policy: &NormalizePolicy) -> String {
        normalize(s, policy).expect("URL de test válida").normalized.to_string()
    }

    // --- Regla 1: minúsculas en esquema y host, nunca en la ruta ---

    #[test]
    fn baja_esquema_y_host_pero_respeta_la_ruta() {
        assert_eq!(
            norm("HTTPS://EJEMPLO.ES/Ruta/Con-Mayusculas"),
            "https://ejemplo.es/Ruta/Con-Mayusculas"
        );
    }

    #[test]
    fn dos_rutas_que_solo_difieren_en_caja_son_urls_distintas() {
        // Un servidor Linux sirve /Foo y /foo como recursos distintos. Unificarlas
        // perdería páginas reales.
        assert_ne!(norm("https://ejemplo.es/Foo"), norm("https://ejemplo.es/foo"));
    }

    // --- Regla 2: puerto por defecto ---

    #[test]
    fn elimina_el_puerto_por_defecto_de_cada_esquema() {
        assert_eq!(norm("https://ejemplo.es:443/a"), "https://ejemplo.es/a");
        assert_eq!(norm("http://ejemplo.es:80/a"), "http://ejemplo.es/a");
    }

    #[test]
    fn conserva_un_puerto_no_estandar() {
        assert_eq!(norm("https://ejemplo.es:8443/a"), "https://ejemplo.es:8443/a");
    }

    // --- Regla 3: resolución de . y .. ---

    #[test]
    fn resuelve_segmentos_relativos() {
        assert_eq!(norm("https://ejemplo.es/a/b/../c"), "https://ejemplo.es/a/c");
        assert_eq!(norm("https://ejemplo.es/a/./b"), "https://ejemplo.es/a/b");
    }

    #[test]
    fn un_exceso_de_puntos_no_escapa_de_la_raiz() {
        assert_eq!(norm("https://ejemplo.es/../../../etc"), "https://ejemplo.es/etc");
    }

    // --- Regla 5: fragmento ---

    #[test]
    fn elimina_el_fragmento() {
        assert_eq!(norm("https://ejemplo.es/a#seccion"), "https://ejemplo.es/a");
    }

    #[test]
    fn dos_anclas_de_la_misma_pagina_colapsan_en_una_sola_url() {
        assert_eq!(norm("https://ejemplo.es/a#uno"), norm("https://ejemplo.es/a#dos"));
    }

    #[test]
    fn conserva_el_hashbang_legado() {
        assert_eq!(norm("https://ejemplo.es/a#!/ruta"), "https://ejemplo.es/a#!/ruta");
    }

    #[test]
    fn con_strip_hashbang_activo_tambien_lo_elimina() {
        let policy = NormalizePolicy { strip_hashbang: true, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/a#!/ruta", &policy), "https://ejemplo.es/a");
    }

    // --- Regla 6: orden de los parámetros ---

    #[test]
    fn ordena_los_parametros_alfabeticamente() {
        assert_eq!(norm("https://ejemplo.es/a?z=1&a=2"), "https://ejemplo.es/a?a=2&z=1");
    }

    #[test]
    fn el_mismo_recurso_con_los_parametros_en_otro_orden_es_una_sola_url() {
        // Este test es la razón de ser del módulo: sin él, el crawler rastrea la misma
        // página tantas veces como permutaciones tenga su querystring.
        assert_eq!(
            norm("https://ejemplo.es/p?b=2&a=1&c=3"),
            norm("https://ejemplo.es/p?c=3&a=1&b=2")
        );
    }

    #[test]
    fn un_parametro_repetido_se_ordena_de_forma_estable() {
        assert_eq!(norm("https://ejemplo.es/a?x=2&x=1"), "https://ejemplo.es/a?x=1&x=2");
    }

    // --- Regla 7: parámetros descartados ---

    #[test]
    fn descarta_la_familia_utm_completa() {
        assert_eq!(
            norm("https://ejemplo.es/a?utm_source=x&utm_medium=y&id=7"),
            "https://ejemplo.es/a?id=7"
        );
    }

    #[test]
    fn descarta_los_identificadores_de_clic_conocidos() {
        for p in DEFAULT_STRIPPED_PARAMS {
            assert_eq!(
                norm(&format!("https://ejemplo.es/a?{p}=abc")),
                "https://ejemplo.es/a",
                "no se descartó el parámetro {p}"
            );
        }
    }

    #[test]
    fn descartar_parametros_no_distingue_mayusculas() {
        assert_eq!(norm("https://ejemplo.es/a?UTM_Source=x"), "https://ejemplo.es/a");
    }

    #[test]
    fn si_solo_habia_parametros_descartables_desaparece_la_query_entera() {
        // Debe quedar sin `?` colgando: `/a?` y `/a` son la misma página.
        assert_eq!(norm("https://ejemplo.es/a?utm_source=x"), "https://ejemplo.es/a");
    }

    #[test]
    fn conserva_los_parametros_que_si_cambian_el_contenido() {
        assert_eq!(
            norm("https://ejemplo.es/buscar?pagina=2&q=sofa"),
            "https://ejemplo.es/buscar?pagina=2&q=sofa"
        );
    }

    #[test]
    fn se_pueden_configurar_otros_parametros_a_descartar() {
        let policy = NormalizePolicy {
            stripped_params: vec!["sesion".into()],
            stripped_prefixes: vec![],
            ..Default::default()
        };
        // Descarta el configurado y, al vaciarse la lista por defecto, ya no descarta utm_.
        assert_eq!(
            norm_with("https://ejemplo.es/a?sesion=1&utm_source=x", &policy),
            "https://ejemplo.es/a?utm_source=x"
        );
    }

    // --- Regla 8: barra final ---

    #[test]
    fn por_defecto_no_toca_la_barra_final() {
        // Hasta observar al servidor, inventar la barra inventa URLs que pueden no existir.
        assert_eq!(norm("https://ejemplo.es/a"), "https://ejemplo.es/a");
        assert_eq!(norm("https://ejemplo.es/a/"), "https://ejemplo.es/a/");
    }

    #[test]
    fn anade_barra_cuando_el_host_asi_lo_sirve() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Add, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/blog", &policy), "https://ejemplo.es/blog/");
    }

    #[test]
    fn quita_barra_cuando_el_host_asi_lo_sirve() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Remove, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/blog/", &policy), "https://ejemplo.es/blog");
    }

    #[test]
    fn no_anade_barra_a_lo_que_parece_un_fichero() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Add, ..Default::default() };
        assert_eq!(
            norm_with("https://ejemplo.es/style.css", &policy),
            "https://ejemplo.es/style.css"
        );
    }

    #[test]
    fn nunca_deja_la_raiz_sin_barra() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Remove, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/", &policy), "https://ejemplo.es/");
    }

    // --- Regla 9: IDN ---

    #[test]
    fn convierte_el_host_idn_a_punycode() {
        assert_eq!(norm("https://diseño.es/a"), "https://xn--diseo-rta.es/a");
    }

    #[test]
    fn el_host_idn_y_su_punycode_son_la_misma_url() {
        assert_eq!(norm("https://diseño.es/a"), norm("https://xn--diseo-rta.es/a"));
    }

    // --- Resolución de enlaces relativos ---

    #[test]
    fn resuelve_las_formas_relativas_habituales_del_html() {
        let base = Url::parse("https://ejemplo.es/blog/post-1").expect("base válida");
        let p = NormalizePolicy::default();
        let r = |h: &str| {
            normalize_relative(&base, h, &p).expect("relativa válida").normalized.to_string()
        };
        assert_eq!(r("/contacto"), "https://ejemplo.es/contacto");
        assert_eq!(r("otro-post"), "https://ejemplo.es/blog/otro-post");
        assert_eq!(r("../inicio"), "https://ejemplo.es/inicio");
        assert_eq!(r("//cdn.ejemplo.es/img.png"), "https://cdn.ejemplo.es/img.png");
        assert_eq!(r("https://externo.com/x"), "https://externo.com/x");
    }

    #[test]
    fn conserva_el_href_original_para_el_informe() {
        // Si el sitio enlaza con `../inicio`, el informe debe poder enseñar eso literalmente.
        let base = Url::parse("https://ejemplo.es/blog/post-1").expect("base válida");
        let n = normalize_relative(&base, "../inicio?utm_source=x", &NormalizePolicy::default())
            .expect("relativa válida");
        assert_eq!(n.original, "../inicio?utm_source=x");
        assert_eq!(n.normalized.as_str(), "https://ejemplo.es/inicio");
    }

    // --- Credenciales ---

    #[test]
    fn las_credenciales_de_la_url_no_sobreviven_a_la_normalizacion() {
        // Revisión 2026-08-01 §1.6: la forma normalizada acaba en el fichero de rastreo, el
        // CSV, el XLSX y el informe que se entrega al cliente. Una contraseña no viaja ahí.
        assert_eq!(
            norm("https://staging:S3cret@pre.cliente.es/privada"),
            "https://pre.cliente.es/privada"
        );
        assert_eq!(norm("https://solo-usuario@ejemplo.es/a"), "https://ejemplo.es/a");
    }

    #[test]
    fn con_o_sin_credenciales_es_la_misma_pagina() {
        // El hash es la clave de deduplicación: si difirieran, el crawler visitaría dos veces
        // el mismo recurso según cómo estuviera escrito el enlace.
        let p = NormalizePolicy::default();
        let con = normalize("https://user:pass@ejemplo.es/a", &p).expect("válida");
        let sin = normalize("https://ejemplo.es/a", &p).expect("válida");
        assert_eq!(con.hash(), sin.hash());
        assert_eq!(con.normalized, sin.normalized);
    }

    // --- Hash ---

    #[test]
    fn urls_equivalentes_comparten_hash_y_distintas_no() {
        let p = NormalizePolicy::default();
        let a = normalize("https://ejemplo.es/a?x=1&utm_source=z", &p).expect("válida");
        let b = normalize("https://ejemplo.es/a?utm_medium=q&x=1", &p).expect("válida");
        let c = normalize("https://ejemplo.es/b", &p).expect("válida");
        assert_eq!(a.hash(), b.hash());
        assert_ne!(a.hash(), c.hash());
    }

    // --- Clasificación ---

    #[test]
    fn el_subdominio_cuenta_como_externo() {
        let u = Url::parse("https://blog.ejemplo.es/x").expect("válida");
        assert!(!is_internal(&u, "ejemplo.es"));
        assert!(is_internal(&u, "blog.ejemplo.es"));
    }

    /// Dos aplicaciones en puertos distintos de la misma máquina son dos sitios, y hasta la
    /// 0.10.0 se rastreaban como uno. En producción es raro; en desarrollo —`localhost:3000`
    /// contra `localhost:8080`— es lo normal, y es donde se audita un `dist/` antes de publicar.
    #[test]
    fn el_puerto_distingue_dos_sitios_en_la_misma_maquina() {
        let tres_mil = Url::parse("http://localhost:3000/panel").expect("URL válida");
        let ocho_mil = Url::parse("http://localhost:8080/api/estado").expect("URL válida");

        assert!(is_internal(&tres_mil, "localhost:3000"));
        assert!(!is_internal(&ocho_mil, "localhost:3000"), "otro puerto es otro sitio");

        // Y el puerto por defecto del esquema no cambia nada: `https://ejemplo.es` y
        // `https://ejemplo.es:443` son la misma cosa escrita de dos maneras.
        let con_puerto = Url::parse("https://ejemplo.es:443/a").expect("URL válida");
        let sin_puerto = Url::parse("https://ejemplo.es/a").expect("URL válida");
        assert_eq!(site_authority(&con_puerto), site_authority(&sin_puerto));
        assert!(is_internal(&con_puerto, "ejemplo.es"));
    }

    #[test]
    fn descarta_los_esquemas_que_no_son_paginas() {
        for s in
            ["mailto:hola@ejemplo.es", "tel:+34600000000", "javascript:void(0)", "data:text/plain,x"]
        {
            let u = Url::parse(s).expect("válida");
            assert!(!is_crawlable_scheme(&u), "{s} no debería rastrearse");
        }
        assert!(is_crawlable_scheme(&Url::parse("https://ejemplo.es").expect("válida")));
    }

    // --- The perimeter of the external probe ---

    fn probeable(s: &str) -> bool {
        is_probeable_host(&Url::parse(s).expect("test URL"))
    }

    #[test]
    fn a_public_host_is_probeable() {
        for s in [
            "https://ejemplo.es/guia",
            "http://www.wikipedia.org/",
            "https://8.8.8.8/",
            "https://[2606:4700:4700::1111]/",
            "https://xn--diseo-rta.es/",
            // A name ending in something that merely *contains* a blocked suffix is public.
            "https://milocal.es/",
            "https://internal-affairs.es/",
        ] {
            assert!(probeable(s), "{s} should be probeable");
        }
    }

    #[test]
    fn the_probe_refuses_addresses_of_the_users_own_network() {
        for s in [
            "http://127.0.0.1:8080/panel",
            "http://169.254.169.254/latest/meta-data/", // cloud metadata
            "http://10.0.0.5/",
            "http://172.16.3.4/",
            "http://192.168.1.1/",
            "http://0.0.0.0/",
            "http://100.64.0.1/",
            "http://[::1]:9200/",
            "http://[fe80::1]/",
            "http://[fd00::1]/",
            "http://localhost:5432/",
            "http://nas.local/",
            "http://api.internal/",
            "http://db.localhost/",
            // Loopback written as IPv6: the host keeps this spelling, so the screen has to
            // look through the mapping instead of at the text.
            "http://[::ffff:127.0.0.1]/",
        ] {
            assert!(!probeable(s), "{s} must not be asked for");
        }
    }

    #[test]
    fn the_screen_decides_on_the_parsed_host_and_not_on_the_text() {
        // `url` canonicalises every spelling of an IPv4 address, so a filter written against
        // the raw text —«does it start with 127.»— would let all of these through.
        for (raw, canonical) in [
            ("http://2130706433/", "127.0.0.1"),
            ("http://0x7f.0.0.1/", "127.0.0.1"),
            ("http://017700000001/", "127.0.0.1"),
            ("http://127.1/", "127.0.0.1"),
        ] {
            let url = Url::parse(raw).expect("test URL");
            assert_eq!(
                url.host_str(),
                Some(canonical),
                "{raw} should already reach us as {canonical}"
            );
            assert!(!is_probeable_host(&url), "{raw} is {canonical} and must not be asked for");
        }
    }

    #[test]
    fn the_trailing_dot_of_an_absolute_name_does_not_get_around_the_screen() {
        assert!(!probeable("http://localhost./"));
        assert!(!probeable("http://nas.local./"));
    }

    /// Auditing a local site turns the network screen off — that is deliberate — but the cloud
    /// metadata endpoint stays screened regardless. The case is a crawl of `localhost` from a
    /// cloud runner in CI: the seed is local, the screen is off, and that address answers with
    /// the instance's credentials.
    #[test]
    fn the_metadata_endpoint_is_screened_even_when_the_screen_is_off() {
        let meta = Url::parse("http://169.254.169.254/latest/meta-data/").expect("parse");
        assert!(!is_probeable(&meta, true), "with the screen on");
        assert!(!is_probeable(&meta, false), "and with it off, which is the point");

        // Written as an integer, which is how it slips past a text-based filter.
        let entero = Url::parse("http://2852039166/").expect("parse");
        assert_eq!(entero.host_str(), Some("169.254.169.254"), "url canonicalises it");
        assert!(!is_probeable(&entero, false));

        // And the IPv6 address AWS answers on.
        let v6 = Url::parse("http://[fd00:ec2::254]/").expect("parse");
        assert!(!is_probeable(&v6, false));
    }

    /// The rest of the local network keeps working when the audited site is local: that is the
    /// `astro dev` and the client's staging, and screening there protects nobody.
    #[test]
    fn auditing_a_local_site_still_reaches_the_rest_of_its_network() {
        for url in ["http://localhost:4321/", "http://192.168.1.10/", "http://10.0.0.5/"] {
            let u = Url::parse(url).expect("parse");
            assert!(is_probeable(&u, false), "{url} should be reachable with the screen off");
            assert!(!is_probeable(&u, true), "{url} should be screened when the site is public");
        }
    }

    // --- Every spelling of an address, because the resolver never sees a literal ---

    /// The connector short-circuits the resolver when the host already parses as an address, so
    /// for these the lexical screen is the only screen there is. Every one of them went through
    /// before 2026-08-04, and `[64:ff9b::a9fe:a9fe]` and `[2002:a9fe:a9fe::]` went through even
    /// the metadata exception.
    #[test]
    fn an_ipv6_that_carries_an_ipv4_inside_decides_as_that_ipv4() {
        for s in [
            "http://[::127.0.0.1]/",          // IPv4-compatible
            "http://[::7f00:1]/",             // the same, compressed
            "http://[0:0:0:0:0:0:7f00:1]/",   // and expanded
            "http://[::ffff:127.0.0.1]/",     // IPv4-mapped
            "http://[::ffff:0:127.0.0.1]/",   // IPv4-translated, RFC 6145
            "http://[64:ff9b::7f00:1]/",      // NAT64, RFC 6052
            "http://[64:ff9b::a00:5]/",       // NAT64 -> 10.0.0.5
            "http://[2002:7f00:1::]/",        // 6to4, RFC 3056
            "http://[::a9fe:a9fe]/",          // v4-compatible metadata
        ] {
            assert!(!probeable(s), "{s} carries a private IPv4 and must not be asked for");
        }
        // A NAT64 address whose embedded IPv4 is public is fine, and it has to be: on an
        // IPv6-only network with DNS64 that is what the resolver answers for every A record.
        assert!(probeable("http://[64:ff9b::808:808]/"), "NAT64 of 8.8.8.8 is public");
    }

    #[test]
    fn the_metadata_endpoint_is_screened_through_every_ipv6_embedding() {
        for s in ["http://[64:ff9b::a9fe:a9fe]/", "http://[2002:a9fe:a9fe::]/", "http://[::a9fe:a9fe]/"]
        {
            let u = Url::parse(s).expect("parse");
            assert!(!is_probeable(&u, false), "{s} is the metadata endpoint, screened always");
        }
    }

    #[test]
    fn the_ipv6_ranges_that_are_not_the_internet() {
        for s in [
            "http://[fec0::1]/",       // deprecated site-local, still routed on LANs
            "http://[2001::1]/",       // Teredo
            "http://[2001:db8::1]/",   // documentation
            "http://[100::1]/",        // discard-only
            "http://[ff02::1]/",       // multicast
            "http://[64:ff9b:1::1]/",  // local-use NAT64
        ] {
            assert!(!probeable(s), "{s} is not an address of the public internet");
        }
    }

    #[test]
    fn the_ipv4_ranges_that_are_not_the_internet() {
        for s in [
            "http://192.0.0.192/",    // Oracle Cloud Classic metadata, inside 192.0.0.0/24
            "http://198.18.0.1/",     // benchmarking
            "http://192.0.2.1/",      // TEST-NET-1
            "http://198.51.100.1/",   // TEST-NET-2
            "http://203.0.113.1/",    // TEST-NET-3
            "http://224.0.0.1/",      // multicast
        ] {
            assert!(!probeable(s), "{s} is not an address of the public internet");
        }
        // And the two provider metadata addresses that are screened always, not just when the
        // audited site is public.
        for s in ["http://192.0.0.192/", "http://100.100.100.200/latest/meta-data/"] {
            let u = Url::parse(s).expect("parse");
            assert!(!is_probeable(&u, false), "{s} is a metadata endpoint");
        }
    }

    #[test]
    fn the_names_of_a_local_network_are_not_public_hosts() {
        // `lan` is the cheapest of the lot: the default local domain of OpenWrt and of half the
        // consumer routers on the market, and it sat right next to `local`, which was screened.
        for s in [
            "http://nas.lan/",
            "http://router.home.arpa/",
            "http://printer.home/",
            "http://intranet.corp/",
            "http://fileserver.intranet/",
            "http://fritz.box/",
            "http://kubernetes.default/",
            "http://consul.service.consul/",
            "http://algo.private/",
            "http://x.domain/",
        ] {
            assert!(!probeable(s), "{s} is a name of somebody's local network");
        }
    }

    #[test]
    fn a_single_label_name_can_only_resolve_through_a_search_domain() {
        // `metadata` is the short name Google's own documentation uses for its metadata server,
        // and it has no public top level to be under.
        for s in ["http://metadata/computeMetadata/v1/", "http://kubernetes/", "http://wiki/"] {
            assert!(!probeable(s), "{s} has no public top level");
        }
    }

    #[test]
    fn a_name_that_merely_ends_in_the_letters_of_a_blocked_suffix_is_public() {
        // The character before the suffix has to be the separator, or `milocal.es` and
        // `deportes.lan` —a real name under a real top level— would be screened by accident.
        for s in [
            "https://milocal.es/",
            "https://internal-affairs.es/",
            "https://micorp.es/",
            "https://midomain.es/",
            // The documentation top levels are deliberately not screened: they resolve to
            // nothing, so screening them buys nothing, and they are the vocabulary of the
            // fixtures in this repository.
            "https://ejemplo.example/",
            "https://ejemplo.test/",
            "https://ejemplo.invalid/",
        ] {
            assert!(probeable(s), "{s} is a public name");
        }
    }

    // --- The perimeter as a whole ---

    fn url(s: &str) -> Url {
        Url::parse(s).expect("test URL")
    }

    #[test]
    fn a_declared_target_reaches_its_own_private_address() {
        // Split-horizon DNS: `pre.cliente.es` answers 10.0.0.5 from inside the office. A screen
        // that decided only on the address would break auditing it.
        let screen = NetworkScreen::for_targets(["pre.cliente.es"]);
        assert!(screen.allows_address("pre.cliente.es", "10.0.0.5".parse().expect("ip")));
        assert!(!screen.allows_address("ajeno.es", "10.0.0.5".parse().expect("ip")));
    }

    #[test]
    fn a_wholly_local_audit_reaches_its_own_network_and_a_mixed_one_does_not() {
        // Rule 4, and the list-mode hole it closes: the exemption used to be a switch thrown by
        // the first seed. Now every named host has to be local for it to apply.
        let local = NetworkScreen::for_seeds([&url("http://localhost:4321/")]);
        assert!(local.is_local_audit());
        assert!(local.allows_host(&url("http://192.168.1.10/")));

        let mixed =
            NetworkScreen::for_seeds([&url("http://localhost:4321/"), &url("https://cliente.es/")]);
        assert!(!mixed.is_local_audit(), "one public target takes the exemption away");
        assert!(!mixed.allows_host(&url("http://192.168.1.10/")));
        assert!(mixed.allows_host(&url("http://localhost:4321/dev")), "its own target, though");
    }

    /// Una auditoría local alcanza su red —es la excepción deliberada— pero **no** a través de un
    /// nombre público que resuelva ahí dentro. Es la forma de un ataque de rebinding, y no hay
    /// servicio local legítimo al que se llegue así.
    #[test]
    fn a_local_audit_is_not_reached_through_a_public_name_that_answers_privately() {
        let local = NetworkScreen::for_seeds([&url("http://localhost:4321/")]);
        let lan: IpAddr = "10.0.0.5".parse().expect("ip");

        assert!(local.allows_address("nas.lan", lan), "un nombre local suyo, sí");
        assert!(local.allows_address("10.0.0.5", lan), "un literal suyo, también");
        assert!(local.allows_address("localhost", "127.0.0.1".parse().expect("ip")), "su objetivo");
        assert!(
            !local.allows_address("10-0-0-5.sslip.io", lan),
            "un nombre público apuntado a su red, no: nadie llega así a un servicio de verdad"
        );

        // Y en un rastreo público sigue bloqueado, que ya lo estaba.
        let publico = NetworkScreen::for_seeds([&url("https://cliente.es/")]);
        assert!(!publico.allows_address("10-0-0-5.sslip.io", lan));
    }

    #[test]
    fn an_empty_perimeter_is_the_strict_one() {
        // A `Default` value must not open the perimeter, and a list whose only line is a
        // `mailto:` is not a local audit — that spelling used to turn the screen off.
        let empty = NetworkScreen::default();
        assert!(!empty.is_local_audit());
        assert!(!empty.allows_host(&url("http://127.0.0.1:8080/panel")));
        assert!(empty.allows_host(&url("https://ejemplo.es/")));

        let no_hosts = NetworkScreen::for_seeds([&url("mailto:hola@cliente.es")]);
        assert!(!no_hosts.is_local_audit());
        assert!(!no_hosts.allows_host(&url("http://127.0.0.1:8080/panel")));
    }

    #[test]
    fn the_metadata_endpoint_is_outside_every_perimeter() {
        for screen in [
            NetworkScreen::default(),
            NetworkScreen::for_targets(["localhost"]),
            NetworkScreen::for_targets(["169.254.169.254"]),
        ] {
            assert!(!screen.allows_host(&url("http://169.254.169.254/latest/meta-data/")));
            assert!(!screen.allows_address("169.254.169.254", "169.254.169.254".parse().expect("ip")));
            // And through a public name that answers it, which is the `nip.io` case.
            assert!(!screen.allows_address("meta.ejemplo.es", "169.254.169.254".parse().expect("ip")));
        }
    }

    #[test]
    fn a_host_string_with_an_ipv6_literal_round_trips_through_the_target_set() {
        // `Url::host_str` gives an IPv6 host without its brackets, so the target set has to put
        // them back before deciding whether that target is local.
        let screen = NetworkScreen::for_targets(["::1"]);
        assert!(screen.is_local_audit(), "[::1] is a local target");
        let public = NetworkScreen::for_targets(["2606:4700:4700::1111"]);
        assert!(!public.is_local_audit());
    }
}
