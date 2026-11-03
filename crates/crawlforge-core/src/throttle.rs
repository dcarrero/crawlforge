//! Freno adaptativo por host. Ver `docs/03-MOTOR-CRAWL.md §7`.
//!
//! «Ante tres 429 consecutivos del mismo host, reducir automáticamente su concurrencia a la
//! mitad y avisar en la UI. **Un crawler que tumba el servidor del cliente es un crawler
//! inservible.**»
//!
//! Se aplica también a los 503: un Varnish o un Cloudflare saturado responde 503, no 429, y el
//! efecto sobre el servidor es el mismo. Distinguirlos sería fiel a la letra del documento e
//! infiel a su motivo.
//!
//! También cuentan las conexiones que se caen y los timeouts: un servidor
//! saturado no siempre llega a contestar 429 o 503 — corta la conexión, la rechaza porque su
//! cola de aceptación está llena, o no responde a tiempo. Es la misma señal por otro camino.
//! Lo que **no** cuenta es lo que no habla de la carga del servidor: un nombre que no resuelve
//! o un certificado que no valida fallarían igual con el servidor vacío. Ver
//! [`is_overload_failure`].
//!
//! Y cuando el servidor dice cuánto esperar —`Retry-After` en un 429 o un 503— el host entero
//! se pausa hasta ese momento ([`Throttle::pause`]), no solo la URL que lo recibió: si la URL
//! esperase y el resto del host siguiera despachándose, la espera no aliviaría nada.
//!
//! La recuperación es deliberadamente más lenta que la reducción: se baja a la mitad de golpe
//! y se sube de uno en uno tras una racha de respuestas buenas. Frenar tarde castiga a un
//! servidor que ya va mal; acelerar pronto lo vuelve a tumbar.
//!
//! # Por qué este módulo y no `governor`
//!
//! El stack original (`CONVENTIONS.md §3`) proponía `governor` como limitador de ritmo. Se descartó
//! al implementar el freno, y la dependencia se retiró del `Cargo.toml`:
//!
//! - `governor` modela un **ritmo** (peticiones por segundo, GCRA). Lo que este motor limita
//!   son **conexiones simultáneas por host**, con el `Crawl-delay` de `robots.txt` como ritmo
//!   cuando el sitio lo pide. Son unidades distintas.
//! - El freno adaptativo ante 429/503 —bajar a la mitad, recuperar despacio— es un estado con
//!   memoria que `governor` no trae: habría que construir este módulo igualmente encima.
//! - Mantener las dos piezas era pagar una dependencia para usar la mitad que no aporta.
//!
//! Lo que la regla «un limitador por host, no global» exige de verdad lo cumple el motor:
//! [`Throttle::limit_for`] se consulta con el host **de cada URL**, no con el semilla.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Respuestas seguidas con señal de sobrecarga que disparan la reducción.
pub const OVERLOAD_STREAK: u32 = 3;
/// Respuestas buenas seguidas que hacen falta para recuperar un punto de concurrencia.
pub const RECOVERY_STREAK: u32 = 20;
/// Suelo de concurrencia. Por debajo de uno no se rastrea.
pub const MIN_CONCURRENCY: u8 = 1;

/// ¿Indica este código que el servidor va justo?
pub fn is_overload(status: u16) -> bool {
    matches!(status, 429 | 503)
}

/// ¿Indica este fallo de red que el servidor va justo?
///
/// - **Timeout**: el servidor no contestó a tiempo. Es la forma más común de ir ahogado.
/// - **Conexión**: reseteada a media respuesta, rechazada o cortada antes de contestar. Un
///   servidor con la cola de aceptación llena rechaza conexiones; un proxy saturado las corta.
///   El rechazo cuenta también aunque a veces signifique «no hay nadie escuchando»: frenar un
///   host caído no le quita nada a nadie.
/// - **DNS y TLS, no**: un nombre que no resuelve o un certificado que no valida fallarían
///   exactamente igual con el servidor ocioso. Frenarlos sería castigar al host por algo que
///   no es carga.
/// - **Demasiado grande y perímetro, tampoco**: son decisiones nuestras, no señales del
///   servidor.
pub fn is_overload_failure(kind: crate::fetch::ErrorKind) -> bool {
    use crate::fetch::ErrorKind;
    matches!(kind, ErrorKind::Timeout | ErrorKind::Connection)
}

#[derive(Debug, Clone, Copy)]
struct HostState {
    limit: u8,
    overload_streak: u32,
    good_streak: u32,
    /// Techo original, para no recuperar por encima de lo que pidió el usuario.
    ceiling: u8,
    /// Hasta cuándo no se despacha nada de este host: lo pidió con `Retry-After`.
    paused_until: Option<tokio::time::Instant>,
}

impl HostState {
    fn new(limit: u8) -> Self {
        Self { limit, overload_streak: 0, good_streak: 0, ceiling: limit, paused_until: None }
    }
}

/// Estado de frenado de todos los hosts del rastreo.
pub struct Throttle {
    hosts: Mutex<HashMap<String, HostState>>,
    default_limit: u8,
    /// Cuántas veces se ha reducido a la mitad la concurrencia de algún host. Va al resumen
    /// del rastreo ([`crate::engine::CrawlMetrics::throttle_reductions`]).
    reductions: std::sync::atomic::AtomicU64,
}

impl Throttle {
    pub fn new(default_limit: u8) -> Self {
        Self {
            hosts: Mutex::new(HashMap::new()),
            default_limit,
            reductions: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Cuántas veces se ha reducido la concurrencia de un host en todo el rastreo.
    pub fn reductions(&self) -> u64 {
        self.reductions.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// ¿Puede despacharse una petición más a este host, con `in_flight` ya en vuelo?
    ///
    /// Es [`Self::limit_for`] más la pausa de `Retry-After`. Va aparte y no dentro de
    /// `limit_for` porque la pausa solo la respeta el despacho de páginas: los demás que
    /// consultan el límite —el descubrimiento de sitemaps, las sondas de externas— no la
    /// necesitan, y un límite de cero ahí sería una forma nueva de colgarse.
    pub fn has_room(&self, host: &str, in_flight: usize) -> bool {
        let Ok(mut guard) = self.hosts.lock() else {
            // Mismo criterio que `limit_for`: un mutex envenenado no aborta el rastreo.
            return in_flight < self.default_limit as usize;
        };
        let Some(state) = guard.get_mut(host) else {
            return in_flight < self.default_limit as usize;
        };
        // El reloj solo se lee si hay pausa: el caso normal no paga nada por ella.
        if let Some(until) = state.paused_until {
            if tokio::time::Instant::now() < until {
                return false;
            }
            state.paused_until = None;
        }
        in_flight < state.limit as usize
    }

    /// No despachar nada de este host hasta `until`: su `Retry-After` lo pidió.
    ///
    /// Si ya había una pausa más larga, se conserva. **Quien pausa tiene que esperar él mismo
    /// hasta `until` sin soltar su hueco** —es lo que hace el fetcher, que llama a esto justo
    /// antes de dormir—: así, mientras haya un host en pausa queda al menos una tarea en vuelo,
    /// y el bucle del motor no puede confundir «todo pausado» con «no queda nada que hacer».
    pub fn pause(&self, host: &str, until: tokio::time::Instant) {
        let Ok(mut guard) = self.hosts.lock() else {
            return;
        };
        let state =
            guard.entry(host.to_string()).or_insert_with(|| HostState::new(self.default_limit));
        state.paused_until = Some(state.paused_until.map_or(until, |p| p.max(until)));
    }

    /// Registra un fallo de red. Cuenta para la racha de sobrecarga solo si
    /// [`is_overload_failure`] lo dice; los demás no tocan nada — ni suman ni rompen la racha,
    /// porque no dicen nada de la carga del servidor. Devuelve el nuevo límite si cambió.
    pub fn record_failure(&self, host: &str, kind: crate::fetch::ErrorKind) -> Option<u8> {
        if !is_overload_failure(kind) {
            return None;
        }
        // 503 es el representante de «sobrecarga»: el mismo camino que una respuesta.
        self.record(host, 503)
    }

    /// Concurrencia vigente para un host.
    pub fn limit_for(&self, host: &str) -> u8 {
        let guard = self.hosts.lock();
        match guard {
            Ok(g) => g.get(host).map(|s| s.limit).unwrap_or(self.default_limit),
            // Un mutex envenenado no debe abortar un rastreo de horas: se sigue con el
            // valor por defecto, que es el que pidió el usuario.
            Err(_) => self.default_limit,
        }
    }

    /// Registra el resultado de una petición. Devuelve el nuevo límite si cambió.
    pub fn record(&self, host: &str, status: u16) -> Option<u8> {
        let Ok(mut guard) = self.hosts.lock() else {
            return None;
        };
        let state =
            guard.entry(host.to_string()).or_insert_with(|| HostState::new(self.default_limit));

        if is_overload(status) {
            state.good_streak = 0;
            state.overload_streak += 1;
            if state.overload_streak >= OVERLOAD_STREAK && state.limit > MIN_CONCURRENCY {
                state.limit = (state.limit / 2).max(MIN_CONCURRENCY);
                state.overload_streak = 0;
                self.reductions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Some(state.limit);
            }
            return None;
        }

        state.overload_streak = 0;
        // Solo una respuesta realmente buena cuenta para recuperar. Un 404 no dice nada
        // sobre la salud del servidor.
        if status < 400 {
            state.good_streak += 1;
            if state.good_streak >= RECOVERY_STREAK && state.limit < state.ceiling {
                state.limit += 1;
                state.good_streak = 0;
                return Some(state.limit);
            }
        }
        None
    }

    /// Deja el host en una sola petición en vuelo, sin recuperación posible.
    ///
    /// Es la mitad «concurrencia» de la promesa de `robots.rs`: **`Crawl-delay` anula la
    /// concurrencia configurada por el usuario para ese host**. El techo baja también a 1
    /// para que la recuperación tras una racha de respuestas buenas no lo deshaga: el
    /// retardo lo declaró el sitio en su `robots.txt`, no una sobrecarga pasajera, y sigue
    /// vigente durante todo el rastreo.
    ///
    /// Esto solo deja de *despachar* en paralelo a partir de que se conozca el
    /// `robots.txt` del host; las peticiones que ya estaban en vuelo cuando se leyó las
    /// serializa [`CrawlDelayGate`].
    pub fn force_serial(&self, host: &str) {
        let Ok(mut guard) = self.hosts.lock() else {
            // Mismo criterio que `limit_for`: un mutex envenenado no aborta el rastreo.
            return;
        };
        let state =
            guard.entry(host.to_string()).or_insert_with(|| HostState::new(self.default_limit));
        state.limit = MIN_CONCURRENCY;
        state.ceiling = MIN_CONCURRENCY;
    }

    /// Hosts que se han frenado y su límite actual. Para informar al usuario.
    pub fn throttled_hosts(&self) -> Vec<(String, u8, u8)> {
        let Ok(guard) = self.hosts.lock() else {
            return Vec::new();
        };
        let mut out: Vec<_> = guard
            .iter()
            .filter(|(_, s)| s.limit < s.ceiling)
            .map(|(h, s)| (h.clone(), s.limit, s.ceiling))
            .collect();
        out.sort();
        out
    }
}

/// El permiso de una petición a un host con `Crawl-delay`: mientras vive, ninguna otra
/// petición de ese host puede arrancar. Se suelta al terminar la petición.
pub type CrawlDelayPermit = tokio::sync::OwnedMutexGuard<Option<tokio::time::Instant>>;

/// Puerta de espaciado por host: la mitad «ritmo» de la promesa de `robots.rs`.
///
/// [`Throttle::force_serial`] no basta por sí solo, y el motivo es de orden temporal: el
/// motor consulta [`Throttle::limit_for`] al **despachar**, antes de que la tarea de la URL
/// haya leído el `robots.txt` de su host. En el primer rellenado del pool, con concurrencia 5,
/// las cinco URLs del host ya están en vuelo cuando se descubre el `Crawl-delay` — forzar el
/// límite a 1 llega tarde para ellas. Esta puerta las serializa igualmente:
///
/// - El permiso es un candado por host que se mantiene **durante toda la petición**: una
///   sola en vuelo, aunque el servidor tarde más que el propio retardo.
/// - Entre el arranque de una petición y el de la siguiente pasa al menos el retardo
///   declarado. Se mide de arranque a arranque, no de final a arranque: es lo que pide
///   `Crawl-delay` y no castiga de más a un servidor rápido.
///
/// La espera vive dentro de la tarea del pool a propósito: el `tokio::select!` del bucle del
/// motor la cancela al vencer `max_duration` o al llegar una [`crate::engine::CancelSignal`],
/// igual que cancelaba el `sleep` antiguo. Los demás hosts no pasan por aquí: cada uno tiene
/// su candado y un host sin `Crawl-delay` ni siquiera lo crea.
#[derive(Default)]
pub struct CrawlDelayGate {
    hosts: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<Option<tokio::time::Instant>>>>>,
}

impl CrawlDelayGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Espera el turno del host y devuelve el permiso de la petición.
    ///
    /// El permiso debe vivir hasta que la petición termine: soltarlo antes de que responda
    /// el servidor volvería a permitir dos en vuelo.
    pub async fn acquire(&self, host: &str, delay: Duration) -> CrawlDelayPermit {
        let slot = {
            let mut hosts = self.hosts.lock().await;
            Arc::clone(hosts.entry(host.to_string()).or_default())
        };
        let mut permit = slot.lock_owned().await;
        if let Some(previous_start) = *permit {
            tokio::time::sleep_until(previous_start + delay).await;
        }
        *permit = Some(tokio::time::Instant::now());
        permit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_las_senales_de_sobrecarga() {
        assert!(is_overload(429), "demasiadas peticiones");
        assert!(is_overload(503), "Varnish o Cloudflare saturados responden esto");
        assert!(!is_overload(200));
        assert!(!is_overload(404), "un 404 no dice nada de la salud del servidor");
        assert!(!is_overload(500), "un error de aplicación no es sobrecarga");
    }

    #[test]
    fn empieza_con_la_concurrencia_configurada() {
        let t = Throttle::new(10);
        assert_eq!(t.limit_for("ejemplo.es"), 10);
    }

    #[test]
    fn reduce_a_la_mitad_tras_tres_sobrecargas_seguidas() {
        let t = Throttle::new(10);
        assert_eq!(t.record("ejemplo.es", 429), None, "una no basta");
        assert_eq!(t.record("ejemplo.es", 429), None, "dos tampoco");
        assert_eq!(t.record("ejemplo.es", 429), Some(5), "la tercera sí");
        assert_eq!(t.limit_for("ejemplo.es"), 5);
    }

    #[test]
    fn una_respuesta_buena_rompe_la_racha() {
        let t = Throttle::new(10);
        t.record("ejemplo.es", 429);
        t.record("ejemplo.es", 429);
        t.record("ejemplo.es", 200);
        assert_eq!(t.record("ejemplo.es", 429), None, "la racha se reinició");
        assert_eq!(t.limit_for("ejemplo.es"), 10);
    }

    #[test]
    fn sigue_bajando_si_el_servidor_sigue_ahogado() {
        let t = Throttle::new(16);
        for esperado in [8, 4, 2, 1] {
            for _ in 0..OVERLOAD_STREAK - 1 {
                t.record("ejemplo.es", 503);
            }
            assert_eq!(t.record("ejemplo.es", 503), Some(esperado));
        }
    }

    #[test]
    fn nunca_baja_de_uno() {
        let t = Throttle::new(2);
        for _ in 0..50 {
            t.record("ejemplo.es", 429);
        }
        assert_eq!(t.limit_for("ejemplo.es"), MIN_CONCURRENCY);
    }

    #[test]
    fn recupera_despacio_tras_una_racha_de_respuestas_buenas() {
        let t = Throttle::new(10);
        for _ in 0..OVERLOAD_STREAK {
            t.record("ejemplo.es", 429);
        }
        assert_eq!(t.limit_for("ejemplo.es"), 5);

        for _ in 0..RECOVERY_STREAK - 1 {
            assert_eq!(t.record("ejemplo.es", 200), None, "todavía no");
        }
        assert_eq!(t.record("ejemplo.es", 200), Some(6), "sube de uno en uno, no de golpe");
    }

    #[test]
    fn no_recupera_por_encima_de_lo_que_pidio_el_usuario() {
        let t = Throttle::new(3);
        for _ in 0..OVERLOAD_STREAK {
            t.record("ejemplo.es", 503);
        }
        for _ in 0..RECOVERY_STREAK * 10 {
            t.record("ejemplo.es", 200);
        }
        assert_eq!(t.limit_for("ejemplo.es"), 3, "el techo es la configuración original");
    }

    #[test]
    fn un_404_no_cuenta_como_recuperacion() {
        let t = Throttle::new(10);
        for _ in 0..OVERLOAD_STREAK {
            t.record("ejemplo.es", 429);
        }
        for _ in 0..RECOVERY_STREAK * 2 {
            assert_eq!(t.record("ejemplo.es", 404), None);
        }
        assert_eq!(t.limit_for("ejemplo.es"), 5, "sigue frenado");
    }

    #[test]
    fn cada_host_se_frena_por_separado() {
        let t = Throttle::new(10);
        for _ in 0..OVERLOAD_STREAK {
            t.record("lento.es", 503);
        }
        assert_eq!(t.limit_for("lento.es"), 5);
        assert_eq!(t.limit_for("rapido.es"), 10, "un host ahogado no frena a los demás");
    }

    #[test]
    fn informa_de_los_hosts_frenados() {
        let t = Throttle::new(8);
        for _ in 0..OVERLOAD_STREAK {
            t.record("lento.es", 429);
        }
        t.record("rapido.es", 200);

        let frenados = t.throttled_hosts();
        assert_eq!(frenados, vec![("lento.es".to_string(), 4, 8)]);
    }

    #[test]
    fn force_serial_deja_el_host_en_uno_y_sin_recuperacion() {
        let t = Throttle::new(8);
        t.force_serial("pausado.es");
        assert_eq!(t.limit_for("pausado.es"), 1);

        // El Crawl-delay no caduca por buenas respuestas: la recuperación no lo levanta.
        for _ in 0..RECOVERY_STREAK * 3 {
            t.record("pausado.es", 200);
        }
        assert_eq!(t.limit_for("pausado.es"), 1, "el techo bajó a 1 con el límite");
        assert_eq!(t.limit_for("otro.es"), 8, "los demás hosts conservan lo configurado");
    }
}

#[cfg(test)]
mod tests_fallos_y_pausas {
    use super::*;
    use crate::fetch::ErrorKind;

    #[test]
    fn las_conexiones_caidas_y_los_timeouts_son_sobrecarga_y_el_dns_no() {
        assert!(is_overload_failure(ErrorKind::Timeout));
        assert!(is_overload_failure(ErrorKind::Connection));
        assert!(!is_overload_failure(ErrorKind::Dns), "un nombre que no resuelve no es carga");
        assert!(!is_overload_failure(ErrorKind::Tls), "un certificado roto tampoco");
        assert!(!is_overload_failure(ErrorKind::TooLarge));
        assert!(!is_overload_failure(ErrorKind::Blocked), "el perímetro es decisión nuestra");
    }

    #[test]
    fn tres_conexiones_caidas_reducen_a_la_mitad() {
        let t = Throttle::new(10);
        assert_eq!(t.record_failure("ejemplo.es", ErrorKind::Connection), None);
        assert_eq!(t.record_failure("ejemplo.es", ErrorKind::Timeout), None);
        assert_eq!(t.record_failure("ejemplo.es", ErrorKind::Connection), Some(5));
        assert_eq!(t.reductions(), 1);
    }

    #[test]
    fn un_fallo_de_dns_ni_suma_ni_rompe_la_racha() {
        let t = Throttle::new(10);
        t.record("ejemplo.es", 429);
        t.record_failure("ejemplo.es", ErrorKind::Connection);
        assert_eq!(t.record_failure("ejemplo.es", ErrorKind::Dns), None);
        assert_eq!(t.record_failure("ejemplo.es", ErrorKind::Tls), None);
        assert_eq!(t.limit_for("ejemplo.es"), 10, "DNS y TLS no cuentan");
        assert_eq!(
            t.record_failure("ejemplo.es", ErrorKind::Timeout),
            Some(5),
            "y tampoco rompieron la racha que llevaban el 429 y la conexión"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn un_host_en_pausa_no_tiene_hueco_hasta_que_vence() {
        let t = Throttle::new(4);
        assert!(t.has_room("ejemplo.es", 0));
        t.pause("ejemplo.es", tokio::time::Instant::now() + Duration::from_secs(2));
        assert!(!t.has_room("ejemplo.es", 0), "en pausa no se despacha ni la primera");
        assert!(t.has_room("otro.es", 0), "la pausa es de un host, no del rastreo");
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(t.has_room("ejemplo.es", 0), "vencida la pausa, vuelve a haber hueco");
        assert!(!t.has_room("ejemplo.es", 4), "y el límite sigue mandando");
    }

    #[tokio::test(start_paused = true)]
    async fn una_pausa_mas_corta_no_acorta_la_vigente() {
        let t = Throttle::new(4);
        let ahora = tokio::time::Instant::now();
        t.pause("ejemplo.es", ahora + Duration::from_secs(10));
        t.pause("ejemplo.es", ahora + Duration::from_secs(1));
        tokio::time::advance(Duration::from_secs(5)).await;
        assert!(!t.has_room("ejemplo.es", 0), "manda la pausa más larga");
    }
}

#[cfg(test)]
mod tests_crawl_delay_gate {
    use super::*;

    // `start_paused`: el tiempo de tokio avanza solo cuando todo está dormido, así que estos
    // tests afirman duraciones exactas de reloj virtual sin tardar nada de verdad.

    #[tokio::test(start_paused = true)]
    async fn la_primera_peticion_de_un_host_no_espera() {
        let gate = CrawlDelayGate::new();
        let t0 = tokio::time::Instant::now();
        let _permit = gate.acquire("ejemplo.es", Duration::from_secs(10)).await;
        assert_eq!(t0.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn entre_dos_arranques_del_mismo_host_pasa_el_retardo() {
        let gate = CrawlDelayGate::new();
        let t0 = tokio::time::Instant::now();
        let permit = gate.acquire("ejemplo.es", Duration::from_secs(10)).await;
        drop(permit); // la petición terminó al instante; aun así el arranque siguiente espera
        let _p2 = gate.acquire("ejemplo.es", Duration::from_secs(10)).await;
        assert!(
            t0.elapsed() >= Duration::from_secs(10),
            "el segundo arranque llegó a los {:?}",
            t0.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn mientras_una_peticion_esta_en_vuelo_el_mismo_host_no_arranca_otra() {
        // El permiso vive durante la petición entera: aunque el retardo ya haya pasado,
        // no puede haber dos en vuelo. Es lo que distingue esta puerta de un simple sleep.
        let gate = CrawlDelayGate::new();
        let _en_vuelo = gate.acquire("ejemplo.es", Duration::from_secs(1)).await;
        let intento = tokio::time::timeout(
            Duration::from_secs(60),
            gate.acquire("ejemplo.es", Duration::from_secs(1)),
        )
        .await;
        assert!(intento.is_err(), "arrancó una segunda petición con otra en vuelo");
    }

    #[tokio::test(start_paused = true)]
    async fn cada_host_tiene_su_propia_puerta() {
        let gate = CrawlDelayGate::new();
        let _lento = gate.acquire("lento.es", Duration::from_secs(30)).await;
        let t0 = tokio::time::Instant::now();
        let _otro = gate.acquire("otro.es", Duration::from_secs(30)).await;
        assert_eq!(t0.elapsed(), Duration::ZERO, "un host con retardo no frena a los demás");
    }
}
