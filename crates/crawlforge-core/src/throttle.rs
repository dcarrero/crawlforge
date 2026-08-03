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

#[derive(Debug, Clone, Copy)]
struct HostState {
    limit: u8,
    overload_streak: u32,
    good_streak: u32,
    /// Techo original, para no recuperar por encima de lo que pidió el usuario.
    ceiling: u8,
}

/// Estado de frenado de todos los hosts del rastreo.
pub struct Throttle {
    hosts: Mutex<HashMap<String, HostState>>,
    default_limit: u8,
}

impl Throttle {
    pub fn new(default_limit: u8) -> Self {
        Self { hosts: Mutex::new(HashMap::new()), default_limit }
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
        let state = guard.entry(host.to_string()).or_insert(HostState {
            limit: self.default_limit,
            overload_streak: 0,
            good_streak: 0,
            ceiling: self.default_limit,
        });

        if is_overload(status) {
            state.good_streak = 0;
            state.overload_streak += 1;
            if state.overload_streak >= OVERLOAD_STREAK && state.limit > MIN_CONCURRENCY {
                state.limit = (state.limit / 2).max(MIN_CONCURRENCY);
                state.overload_streak = 0;
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
        let state = guard.entry(host.to_string()).or_insert(HostState {
            limit: self.default_limit,
            overload_streak: 0,
            good_streak: 0,
            ceiling: self.default_limit,
        });
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
