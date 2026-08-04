//! El resolutor de nombres del motor, y el sitio donde el perímetro de red se hace cumplir.
//!
//! # Por qué el perímetro vive aquí y no en una criba de texto
//!
//! Hasta el 2026-08-04 el perímetro era una criba léxica sobre el host escrito
//! (`normalize::is_probeable_host`). Un nombre que resuelve a una dirección privada la
//! atravesaba entera, y eso **no exige que el atacante controle ningún dominio**: hay servicios
//! públicos de DNS comodín —`localtest.me`, `lvh.me`, `nip.io`, `sslip.io`— que devuelven la
//! dirección escrita en el propio nombre. Comprobado de extremo a extremo contra la sonda del
//! motor, con la criba encendida: `http://localtest.me:P/panel` llegó a un servicio en loopback
//! y respondió 200. Y `169.254.169.254.nip.io` se saltaba además `is_cloud_metadata`, que era la
//! única criba declarada incondicional.
//!
//! La decisión correcta se toma sobre **la dirección a la que se va a marcar**, no sobre el
//! texto del host. `reqwest` deja sustituir el resolutor (`reqwest::dns::Resolve`), y ese es el
//! único punto por el que pasan todas las conexiones de un `Client`: filtrar aquí significa que
//! la conexión ni siquiera se intenta.
//!
//! # Dos cosas que hay que hacer bien
//!
//! - **Todas las direcciones, no la primera.** Un nombre con varios registros `A`, uno público y
//!   otro privado, se rechaza entero. Quedarse con las públicas y marcar a esas parece más
//!   amable y es peor: deja al atacante elegir qué se conecta según qué devuelva el DNS en cada
//!   consulta, que es la mitad de un ataque de *rebinding*.
//! - **Cada conexión vuelve a pasar por aquí.** No hay caché propia: el resolutor se consulta
//!   por conexión nueva, así que un nombre que cambia de dirección entre dos peticiones se
//!   vuelve a juzgar. Es lo que se quiere, y es por lo que un salto de redirección —que el motor
//!   sigue a mano, con una petición nueva— no puede colarse por detrás del perímetro.
//!
//! # Lo que este resolutor **no** ve
//!
//! Un host que ya es una dirección literal (`http://127.0.0.1/`, `http://[::1]/`). El conector
//! de `hyper` no llama al resolutor cuando el host parsea como IP, así que para los literales la
//! única defensa sigue siendo la criba léxica de `normalize`. Por eso allí se cubren todas las
//! formas de escribir una dirección —octal, entera, mapeada, compatible, traducida, NAT64,
//! 6to4— y por eso esa parte no es trabajo redundante.

use crate::normalize::NetworkScreen;
use std::future::Future;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::pin::Pin;
use std::sync::Arc;

/// Marca que viaja dentro del error de conexión para que `fetch` sepa distinguir «el perímetro
/// lo rechazó» de «la red falló».
///
/// Va en el error y no en un canal aparte porque el rechazo ocurre dentro de `hyper`, varias
/// capas por debajo de quien hizo la petición. `fetch::classify_reqwest_error` recorre la cadena
/// de causas buscándola.
pub const PERIMETER_MARKER: &str = "crawlforge-perimeter-refused";

/// Cómo se averigua a qué direcciones responde un nombre.
///
/// Es un trait y no una llamada directa al sistema para poder inyectar un resolutor de mentira
/// en los tests: el caso que hay que demostrar es «un nombre público que responde 127.0.0.1», y
/// depender de que `localtest.me` siga existiendo convertiría el test en una prueba de internet.
pub trait Lookup: Send + Sync + 'static {
    fn lookup(
        &self,
        host: String,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Vec<IpAddr>>> + Send>>;
}

/// El resolutor del sistema (`getaddrinfo`), en el pool de bloqueo de `tokio`.
///
/// Es lo mismo que hace el resolutor por defecto de `reqwest` (`GaiResolver` de `hyper-util`,
/// que es `pub(crate)` y por eso no se puede envolver): `getaddrinfo` es una llamada bloqueante
/// y va a `spawn_blocking`. Sustituirlo por esto no cambia el coste — medido en la regresión.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemLookup;

impl Lookup for SystemLookup {
    fn lookup(
        &self,
        host: String,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Vec<IpAddr>>> + Send>> {
        Box::pin(async move {
            let joined = tokio::task::spawn_blocking(move || {
                // El puerto no importa: `reqwest` lo sustituye por el de la URL. Se resuelve
                // con 0 y se conserva solo la dirección.
                (host.as_str(), 0u16)
                    .to_socket_addrs()
                    .map(|it| it.map(|s: SocketAddr| s.ip()).collect::<Vec<_>>())
            })
            .await;
            match joined {
                Ok(result) => result,
                Err(e) => Err(std::io::Error::other(e.to_string())),
            }
        })
    }
}

/// Un resolutor fijo, para tests: el nombre responde lo que diga la tabla.
///
/// Vive fuera de `#[cfg(test)]` porque lo usan los tests de integración, que compilan contra el
/// crate como un consumidor más.
pub struct StaticLookup {
    entries: Vec<(String, Vec<IpAddr>)>,
}

impl StaticLookup {
    pub fn new(entries: impl IntoIterator<Item = (String, Vec<IpAddr>)>) -> Self {
        Self { entries: entries.into_iter().collect() }
    }
}

impl Lookup for StaticLookup {
    fn lookup(
        &self,
        host: String,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Vec<IpAddr>>> + Send>> {
        let found = self
            .entries
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&host))
            .map(|(_, addrs)| addrs.clone());
        Box::pin(async move {
            found.ok_or_else(|| std::io::Error::other(format!("no static entry for {host}")))
        })
    }
}

/// El resolutor que el motor instala en su cliente HTTP: resuelve y **filtra antes de devolver**.
pub struct ScreeningResolver {
    screen: NetworkScreen,
    lookup: Arc<dyn Lookup>,
}

impl ScreeningResolver {
    pub fn new(screen: NetworkScreen, lookup: Arc<dyn Lookup>) -> Self {
        Self { screen, lookup }
    }

    /// Con el resolutor del sistema, que es el caso de producción.
    pub fn system(screen: NetworkScreen) -> Self {
        Self::new(screen, Arc::new(SystemLookup))
    }
}

impl reqwest::dns::Resolve for ScreeningResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        let screen = self.screen.clone();
        let lookup = Arc::clone(&self.lookup);
        Box::pin(async move {
            let addrs = lookup.lookup(host.clone()).await?;
            // Basta con que **una** dirección esté fuera del perímetro para rechazar el nombre.
            // Devolver solo las buenas dejaría que el DNS eligiera, que es justo lo que no puede
            // decidir nadie de fuera.
            if let Some(refused) = addrs.iter().find(|ip| !screen.allows_address(&host, **ip)) {
                // La dirección va al log y **no** al error: el mensaje de error acaba en
                // `urls.error_message`, dentro del fichero que se le entrega al cliente, y
                // decirle a un tercero qué hay en la red del consultor es exactamente el mapa
                // que este perímetro existe para no dibujar.
                tracing::warn!(
                    host,
                    address = %refused,
                    "el nombre resuelve fuera del perímetro de la auditoría; no se conecta"
                );
                return Err(Box::new(std::io::Error::other(format!(
                    "{PERIMETER_MARKER}: {host}"
                ))) as Box<dyn std::error::Error + Send + Sync>);
            }
            let iter = addrs.into_iter().map(|ip| SocketAddr::new(ip, 0));
            Ok(Box::new(iter) as reqwest::dns::Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::dns::Resolve;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;

    fn resolver(screen: NetworkScreen, entries: &[(&str, &[IpAddr])]) -> ScreeningResolver {
        let entries: Vec<(String, Vec<IpAddr>)> =
            entries.iter().map(|(n, a)| ((*n).to_string(), a.to_vec())).collect();
        ScreeningResolver::new(screen, Arc::new(StaticLookup::new(entries)))
    }

    async fn resolves(r: &ScreeningResolver, host: &str) -> bool {
        let name = reqwest::dns::Name::from_str(host).expect("test host");
        r.resolve(name).await.is_ok()
    }

    const V4_LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    const V4_PUBLIC: IpAddr = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    const V4_PRIVATE: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
    const V4_METADATA: IpAddr = IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254));

    #[tokio::test]
    async fn a_public_name_that_answers_loopback_is_refused() {
        // This is `localtest.me`, and the reason the lexical screen was never enough: the name
        // is public in every sense a text filter can check.
        let r = resolver(NetworkScreen::default(), &[("localtest.me", &[V4_LOOPBACK])]);
        assert!(!resolves(&r, "localtest.me").await);
    }

    #[tokio::test]
    async fn a_public_name_that_answers_a_public_address_resolves() {
        let r = resolver(NetworkScreen::default(), &[("ejemplo.es", &[V4_PUBLIC])]);
        assert!(resolves(&r, "ejemplo.es").await);
    }

    #[tokio::test]
    async fn one_private_answer_among_public_ones_refuses_the_whole_name() {
        // Keeping the public addresses and dialling those lets whoever controls the DNS pick
        // which one the connection uses.
        let r = resolver(
            NetworkScreen::default(),
            &[("mixto.es", &[V4_PUBLIC, V4_PRIVATE])],
        );
        assert!(!resolves(&r, "mixto.es").await);
    }

    #[tokio::test]
    async fn the_metadata_address_is_refused_even_for_a_declared_target() {
        // `169.254.169.254.nip.io` is a public name whose answer is the metadata endpoint, and
        // it went through the exception that was declared unconditional.
        let screen = NetworkScreen::for_targets(["meta.cliente.es"]);
        let r = resolver(screen, &[("meta.cliente.es", &[V4_METADATA])]);
        assert!(!resolves(&r, "meta.cliente.es").await);
    }

    #[tokio::test]
    async fn a_target_of_the_crawl_reaches_its_own_private_address() {
        // Split-horizon DNS: `pre.cliente.es` answers 10.0.0.5 from inside the office. The user
        // named it, so it is inside the perimeter — a bare address screen would break this.
        let screen = NetworkScreen::for_targets(["pre.cliente.es"]);
        let r = resolver(screen, &[("pre.cliente.es", &[V4_PRIVATE])]);
        assert!(resolves(&r, "pre.cliente.es").await);
        // …and a different name answering the same address is not.
        let screen = NetworkScreen::for_targets(["pre.cliente.es"]);
        let r = resolver(screen, &[("ajeno.es", &[V4_PRIVATE])]);
        assert!(!resolves(&r, "ajeno.es").await);
    }

    #[tokio::test]
    async fn an_ipv6_answer_is_screened_too() {
        let r = resolver(
            NetworkScreen::default(),
            &[("v6.es", &[IpAddr::V6(Ipv6Addr::LOCALHOST)])],
        );
        assert!(!resolves(&r, "v6.es").await);
    }

    #[tokio::test]
    async fn the_refusal_never_names_the_address_it_refused() {
        // The message ends up in `urls.error_message`, inside the file that is handed to the
        // client. Telling a third party what lives on the consultant's network is the map this
        // perimeter exists not to draw.
        let r = resolver(NetworkScreen::default(), &[("localtest.me", &[V4_PRIVATE])]);
        let name = reqwest::dns::Name::from_str("localtest.me").expect("test host");
        let err = r.resolve(name).await.err().expect("should be refused").to_string();
        assert!(err.contains(PERIMETER_MARKER), "{err}");
        assert!(!err.contains("10.0.0.5"), "the address must not travel in the error: {err}");
    }

    #[tokio::test]
    async fn a_name_that_does_not_resolve_is_a_plain_dns_failure() {
        let r = resolver(NetworkScreen::default(), &[]);
        let name = reqwest::dns::Name::from_str("nada.es").expect("test host");
        let err = r.resolve(name).await.err().expect("should fail").to_string();
        assert!(!err.contains(PERIMETER_MARKER), "not a perimeter refusal: {err}");
    }
}
