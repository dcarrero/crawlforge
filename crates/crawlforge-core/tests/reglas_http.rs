//! Las reglas que solo un servidor puede provocar, demostradas rastreando uno de verdad.
//!
//! El banco de fixtures (`fixtures_de_reglas.rs`) rastrea un árbol de ficheros, y el
//! `FilesystemFetcher` solo sabe devolver 200 o 404: no emite 3xx ni 5xx y no mide latencia. Ocho
//! reglas quedaban por eso cubiertas solo por sus tests unitarios, que demuestran su SQL pero no
//! que el motor llegue a escribir las filas de las que ese SQL vive. Aquí se cierra esa costura:
//! se levanta el servidor de pruebas, se rastrea con `CrawlJob::http` y se
//! comprueba el hallazgo en la URL que le toca.
//!
//! Los tests son deterministas y no salen de `127.0.0.1`: el servidor asigna su propio puerto y
//! sirve un mapa de rutas fijo.
//!
//! # Los huecos del motor que este fichero documentó con un test
//!
//! Rastrear de verdad enseña lo que ningún test unitario podía enseñar, y aquí salieron tres
//! huecos. Los tres están cerrados: las URLs externas se comprueban de estado por defecto
//! (`CrawlLimits::check_external`, demostrado de extremo a extremo en `tests/externas.rs`),
//! `follow_external` marca las ajenas como lo que son, y el destino de una redirección se
//! encola. Los tests que los documentaban fallaron el día del cierre —que era su función— y
//! hoy fijan el comportamiento nuevo; el de las externas fija además que
//! `check_external = false` recupera a propósito el comportamiento antiguo.

mod support;

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;
use std::time::Duration;
use support::servidor::{Respuesta, ServidorDePruebas};

/// Directorio temporal que se limpia solo. Mismo patrón que `pipeline.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-http-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear el directorio temporal");
        Self { path }
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Un rastreo terminado, abierto en solo lectura como lo abriría la UI.
struct Rastreo {
    _tmp: Temporal,
    conn: Connection,
}

impl Rastreo {
    /// URLs en las que se registró un hallazgo. Un hallazgo de sitio no tiene URL y aparece
    /// como `(sitio)`.
    fn urls_de(&self, rule_id: &str) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT COALESCE(u.url, '(sitio)') FROM issues i
                 LEFT JOIN urls u ON u.id = i.url_id
                 WHERE i.rule_id = ?1 ORDER BY 1",
            )
            .expect("consultar issues");
        let filas = stmt
            .query_map([rule_id], |r| r.get::<_, String>(0))
            .expect("leer issues")
            .filter_map(Result::ok)
            .collect();
        filas
    }

    /// El `detail_json` del primer hallazgo de una regla.
    fn detalle(&self, rule_id: &str) -> String {
        self.conn
            .query_row(
                "SELECT COALESCE(detail_json, '') FROM issues WHERE rule_id = ?1 LIMIT 1",
                [rule_id],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default()
    }

    fn estado(&self, url: &str) -> Option<i64> {
        self.conn
            .query_row("SELECT status_code FROM urls WHERE url = ?1", [url], |r| r.get(0))
            .ok()
            .flatten()
    }

    /// A dónde apunta `urls.redirect_to`, ya resuelto a URL.
    fn redirige_a(&self, url: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT d.url FROM urls o JOIN urls d ON d.id = o.redirect_to WHERE o.url = ?1",
                [url],
                |r| r.get(0),
            )
            .ok()
    }

    fn es_interna(&self, url: &str) -> Option<bool> {
        self.conn
            .query_row("SELECT is_internal FROM urls WHERE url = ?1", [url], |r| {
                r.get::<_, i64>(0)
            })
            .ok()
            .map(|v| v != 0)
    }

    fn existe_url(&self, url: &str) -> bool {
        self.conn
            .query_row("SELECT COUNT(*) FROM urls WHERE url = ?1", [url], |r| r.get::<_, i64>(0))
            .unwrap_or(0)
            > 0
    }
}

/// Rastrea el servidor con la configuración por defecto de un trabajo HTTP.
async fn rastrear(nombre: &str, servidor: &ServidorDePruebas) -> Rastreo {
    rastrear_con(nombre, servidor, |_| {}).await
}

/// Igual, pero dejando ajustar el trabajo antes de lanzarlo.
async fn rastrear_con(
    nombre: &str,
    servidor: &ServidorDePruebas,
    ajustar: impl FnOnce(&mut CrawlJob),
) -> Rastreo {
    let tmp = Temporal::new(nombre);
    let mut job = CrawlJob::http(servidor.base());
    // El servidor no tiene sitemaps y las rutas convencionales devolverían 404: pedirlas no
    // aporta nada a estas reglas y son dos peticiones por rastreo.
    job.discover_sitemaps = false;
    ajustar(&mut job);

    let outcome = engine::run(job, &tmp.path.join("crawl.sqlite")).await.expect("rastrear");
    let conn = Connection::open_with_flags(
        &outcome.store_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el fichero de rastreo");

    Rastreo { _tmp: tmp, conn }
}

/// Portada con enlaces a las rutas que se le pasen.
fn portada(rutas: &[&str]) -> Respuesta {
    let enlaces: String =
        rutas.iter().map(|r| format!("<p><a href=\"{r}\">{r}</a></p>")).collect();
    Respuesta::pagina("Portada", &format!("<p>Portada del sitio de pruebas.</p>{enlaces}"))
}

// ---------------------------------------------------------------- HTTP-5XX

#[tokio::test]
async fn un_5xx_se_reporta_en_la_url_que_falla() {
    // Este test tarda unos segundos y no es un defecto del servidor: el motor reintenta los
    // estados de sobrecarga (`fetch::should_retry_status`) con backoff de 1, 2 y 4 segundos. Es
    // la única prueba que recorre esa política de extremo a extremo, así que la espera compra
    // algo.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/error"])),
        ("/error", Respuesta::error(500)),
    ])
    .await;

    let r = rastrear("5xx", &servidor).await;

    assert_eq!(
        r.urls_de("HTTP-5XX"),
        vec![servidor.url("/error")],
        "el 5xx se reporta en la URL que falló, y solo en ella"
    );
    assert_eq!(r.estado(&servidor.url("/error")), Some(500));
    assert!(
        r.detalle("HTTP-5XX").contains("\"status_code\":500"),
        "el detalle debe llevar el código: {}",
        r.detalle("HTTP-5XX")
    );

    assert_eq!(
        servidor.peticiones("/error"),
        1 + crawlforge_core::fetch::MAX_RETRIES as usize,
        "un 5xx se reintenta antes de darlo por bueno"
    );
}

// ---------------------------------------------------------------- HTTP-SLOW-RESPONSE

#[tokio::test]
async fn una_respuesta_lenta_se_reporta_con_su_ttfb() {
    // Un solo recurso lento, y con el retardo justo por encima del umbral: el test cuesta lo que
    // cuesta esa página y ni un milisegundo más.
    let lenta = Respuesta::pagina("Lenta", "<p>Esta página tarda.</p>")
        .con_retardo(Duration::from_millis(1_300));
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/lenta"])),
        ("/lenta", lenta),
    ])
    .await;

    let r = rastrear("lenta", &servidor).await;

    assert_eq!(
        r.urls_de("HTTP-SLOW-RESPONSE"),
        vec![servidor.url("/lenta")],
        "solo la página lenta; la portada responde al instante"
    );
    let detalle = r.detalle("HTTP-SLOW-RESPONSE");
    assert!(
        detalle.contains(&format!("\"threshold_ms\":{}", crawlforge_rules::http::SLOW_RESPONSE_MS)),
        "el detalle debe decir contra qué umbral se comparó: {detalle}"
    );
}

// ---------------------------------------------------------------- HTTP-REDIRECT-CHAIN

#[tokio::test]
async fn una_cadena_de_dos_saltos_se_reporta_en_su_cabeza() {
    // `/a` → `/b` → `/c`, y `/c` responde 200. Los tres cuelgan de la portada porque el motor no
    // encola el destino de una redirección; ver
    // `una_redireccion_a_una_url_que_nadie_enlaza_deja_la_cadena_sin_recorrer`.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/a", "/b", "/c"])),
        ("/a", Respuesta::redirige(301, "/b")),
        ("/b", Respuesta::redirige(301, "/c")),
        ("/c", Respuesta::pagina("Destino", "<p>Aquí termina la cadena.</p>")),
    ])
    .await;

    let r = rastrear("cadena", &servidor).await;

    assert_eq!(r.redirige_a(&servidor.url("/a")).as_deref(), Some(servidor.url("/b").as_str()));
    assert_eq!(r.redirige_a(&servidor.url("/b")).as_deref(), Some(servidor.url("/c").as_str()));

    assert_eq!(
        r.urls_de("HTTP-REDIRECT-CHAIN"),
        vec![servidor.url("/a")],
        "el hallazgo va en la cabeza de la cadena, que es la URL que se enlaza"
    );
    let detalle = r.detalle("HTTP-REDIRECT-CHAIN");
    assert!(detalle.contains("\"hops\":2"), "detalle inesperado: {detalle}");
    assert!(detalle.contains(&servidor.url("/c")), "falta el destino final: {detalle}");
}

// ---------------------------------------------------------------- HTTP-REDIRECT-LOOP

#[tokio::test]
async fn un_bucle_de_redireccion_se_reporta_una_sola_vez() {
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/x", "/y"])),
        ("/x", Respuesta::redirige(301, "/y")),
        ("/y", Respuesta::redirige(301, "/x")),
    ])
    .await;

    let r = rastrear("bucle", &servidor).await;

    let hallazgos = r.urls_de("HTTP-REDIRECT-LOOP");
    assert_eq!(hallazgos.len(), 1, "un ciclo, un hallazgo: {hallazgos:?}");
    assert!(
        hallazgos[0] == servidor.url("/x") || hallazgos[0] == servidor.url("/y"),
        "el hallazgo va en una de las dos URLs del ciclo: {hallazgos:?}"
    );
    let detalle = r.detalle("HTTP-REDIRECT-LOOP");
    assert!(detalle.contains("\"length\":2"), "detalle inesperado: {detalle}");

    // Un bucle no es una cadena: lo cuenta la regla más grave y más concreta.
    assert!(r.urls_de("HTTP-REDIRECT-CHAIN").is_empty());
}

// ---------------------------------------------------------------- HTTP-REDIRECT-TO-404

#[tokio::test]
async fn una_redireccion_que_acaba_en_404_se_reporta_en_la_cabeza() {
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/viejo", "/no-existe"])),
        ("/viejo", Respuesta::redirige(301, "/no-existe")),
        // `/no-existe` no está declarada: el servidor devuelve 404, como haría el de verdad.
    ])
    .await;

    let r = rastrear("redirige-a-404", &servidor).await;

    assert_eq!(r.estado(&servidor.url("/no-existe")), Some(404));
    assert_eq!(
        r.urls_de("HTTP-REDIRECT-TO-404"),
        vec![servidor.url("/viejo")],
        "se reporta la URL que se enlaza, que es la que hay que reapuntar"
    );
    let detalle = r.detalle("HTTP-REDIRECT-TO-404");
    assert!(detalle.contains("\"final_status_code\":404"), "detalle inesperado: {detalle}");
}

// ---------------------------------------------------------------- HTTP-NO-HTTPS

#[tokio::test]
async fn un_sitio_servido_por_http_lo_reporta_una_sola_vez() {
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/otra"])),
        ("/otra", Respuesta::pagina("Otra", "<p>Otra página del sitio.</p>")),
    ])
    .await;

    let r = rastrear("no-https", &servidor).await;

    assert_eq!(
        r.urls_de("HTTP-NO-HTTPS"),
        vec!["(sitio)".to_string()],
        "es una configuración del servidor, no un defecto por página"
    );
    let detalle = r.detalle("HTTP-NO-HTTPS");
    assert!(detalle.contains("\"http_urls\":2"), "detalle inesperado: {detalle}");
}

// ---------------------------------------------------------------- CANON-TO-REDIRECT

#[tokio::test]
async fn un_canonical_que_apunta_a_una_redireccion_se_reporta() {
    let con_canonical = Respuesta::html(
        "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\">\
         <title>Página con canonical torcido</title>\
         <link rel=\"canonical\" href=\"/vieja\">\
         </head><body><main><h1>Página con canonical torcido</h1>\
         <p>Su canonical apunta a una URL que redirige.</p>\
         <p><a href=\"/vieja\">La URL del canonical</a></p>\
         <p><a href=\"/nueva\">El destino de verdad</a></p>\
         </main></body></html>",
    );
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/pagina"])),
        ("/pagina", con_canonical),
        ("/vieja", Respuesta::redirige(301, "/nueva")),
        ("/nueva", Respuesta::pagina("Nueva", "<p>La URL buena.</p>")),
    ])
    .await;

    let r = rastrear("canon-a-redireccion", &servidor).await;

    assert_eq!(r.estado(&servidor.url("/vieja")), Some(301));
    assert_eq!(
        r.urls_de("CANON-TO-REDIRECT"),
        vec![servidor.url("/pagina")],
        "el hallazgo va en la página que declara el canonical"
    );
    let detalle = r.detalle("CANON-TO-REDIRECT");
    assert!(detalle.contains(&servidor.url("/vieja")), "detalle inesperado: {detalle}");
    assert!(detalle.contains("\"status\":301"), "detalle inesperado: {detalle}");
}

// ---------------------------------------------------------------- Huecos del motor

#[tokio::test]
async fn por_defecto_la_externa_rota_recibe_su_404_y_la_regla_dispara() {
    // Este test documentaba el hueco contrario —«las externas se registran pero nunca se
    // piden, así que HTTP-404-EXTERNAL no puede disparar»— y falló el día que el hueco se
    // cerró, que era su función. Hoy fija el cierre: con `check_external` (el defecto), la
    // externa recibe una sonda de estado y el 404 ajeno por fin es un hallazgo. El detalle
    // fino (HEAD, deduplicación, tope, robots ajeno) vive en `tests/externas.rs`.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[]).await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        portada(&[&ajeno.url_como_otro_host("/muerta")]),
    )])
    .await;

    let r = rastrear("externa-por-defecto", &propio).await;

    let externa = ajeno.url_como_otro_host("/muerta");
    assert!(r.existe_url(&externa), "la URL externa se registra");
    assert_eq!(r.estado(&externa), Some(404), "y su estado se comprueba");
    assert_eq!(ajeno.peticiones("/muerta"), 1, "una sonda, no un rastreo");
    assert_eq!(
        r.urls_de("HTTP-404-EXTERNAL"),
        vec![externa],
        "con el estado en la fila, la regla ya tiene de qué avisar"
    );
}

#[tokio::test]
async fn sin_check_external_la_externa_queda_sin_estado_y_la_regla_calla() {
    // El comportamiento antiguo, ahora elegido a propósito con `--no-external-check`: la URL
    // ajena se registra sin estado, y la regla calla en vez de suponer.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[]).await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        portada(&[&ajeno.url_como_otro_host("/muerta")]),
    )])
    .await;

    let r = rastrear_con("externa-apagada", &propio, |job| {
        job.limits.check_external = false;
    })
    .await;

    let externa = ajeno.url_como_otro_host("/muerta");
    assert!(r.existe_url(&externa), "la URL externa sí se registra");
    assert_eq!(r.estado(&externa), None, "sin comprobación no hay estado");
    assert_eq!(ajeno.peticiones("/muerta"), 0, "el servidor ajeno no recibe ninguna petición");
    assert!(
        r.urls_de("HTTP-404-EXTERNAL").is_empty(),
        "sin estado no puede haber hallazgo, y la regla calla en vez de suponer"
    );
}

#[tokio::test]
async fn con_follow_external_el_404_ajeno_se_reporta_como_externo_y_no_como_propio() {
    // Regresión de un falso positivo crítico. La URL ajena se encolaba con
    // `pending_row(&n, link_hash, depth, true, false)` —ese `true` era `is_internal`, escrito a
    // pelo— y `writer.rs::insert_urls` no incluía `is_internal` en su `ON CONFLICT DO UPDATE`,
    // así que el valor correcto que sí calcula `build_result` no llegaba nunca a la fila.
    //
    // Resultado: el enlace roto de otro dominio se reportaba como `HTTP-404-INTERNAL`, que es
    // severidad crítica. En un informe real, un enlace ajeno acusando al sitio del cliente.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[]).await;
    let propio = ServidorDePruebas::arrancar(&[(
        "/",
        portada(&[&ajeno.url_como_otro_host("/muerta")]),
    )])
    .await;

    let r = rastrear_con("externa-seguida", &propio, |job| {
        job.limits.follow_external = true;
    })
    .await;

    let externa = ajeno.url_como_otro_host("/muerta");
    assert_eq!(r.estado(&externa), Some(404), "con follow_external sí se pide");
    assert_eq!(ajeno.peticiones("/muerta"), 1);
    assert_eq!(
        r.es_interna(&externa),
        Some(false),
        "y queda marcada como lo que es: una URL de otro dominio"
    );

    assert!(
        r.urls_de("HTTP-404-INTERNAL").is_empty(),
        "un enlace ajeno roto no puede acusar al sitio propio"
    );
    assert_eq!(
        r.urls_de("HTTP-404-EXTERNAL"),
        vec![externa],
        "y sí sale con su regla, que es de severidad media"
    );
}

#[tokio::test]
async fn se_recorre_una_cadena_cuyos_eslabones_no_enlaza_nadie() {
    // Regresión: solo se encolaban las URLs que salen de un `<a>` de una página parseada, y un
    // 3xx no trae HTML, así que el destino de una redirección no se pedía nunca. Si ningún otro
    // sitio del menú enlazaba el eslabón intermedio, `/oculta` no llegaba a existir como fila,
    // el hash de `urls.redirect_to` se quedaba sin resolver y las tres reglas de redirección se
    // quedaban sin grafo. Las cadenas se detectaban por suerte, no por diseño.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/", portada(&["/entrada"])),
        ("/entrada", Respuesta::redirige(301, "/oculta")),
        ("/oculta", Respuesta::redirige(301, "/final")),
        ("/final", Respuesta::pagina("Final", "<p>El final de la cadena.</p>")),
    ])
    .await;

    let r = rastrear("redireccion-no-enlazada", &servidor).await;

    assert_eq!(r.estado(&servidor.url("/entrada")), Some(301));
    assert_eq!(
        servidor.peticiones("/oculta"),
        1,
        "el destino de la redirección se pide aunque no lo enlace nadie"
    );
    assert_eq!(
        r.redirige_a(&servidor.url("/entrada")),
        Some(servidor.url("/oculta")),
        "y con la fila de destino, `redirect_to` se resuelve"
    );
    assert_eq!(
        r.urls_de("HTTP-REDIRECT-CHAIN"),
        vec![servidor.url("/entrada")],
        "la cadena de dos saltos se reporta en su cabeza"
    );
    assert_eq!(r.estado(&servidor.url("/final")), Some(200), "y se llega hasta el final");
}
