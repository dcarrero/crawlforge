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
use std::sync::Mutex;

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
}
