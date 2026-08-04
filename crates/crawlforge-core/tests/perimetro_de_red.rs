//! El perímetro de red de la auditoría, de extremo a extremo.
//!
//! Cierra los seis agujeros que la segunda revisión de seguridad del 2026-08-04 abrió a mano,
//! **ejecutando cada caso**. El de 0.5.0 estaba razonado y no aguantó, así que cada test de
//! aquí se escribió reproduciendo primero y se comprobó en rojo revirtiendo su arreglo.
//!
//! Lo que hay que entender antes de tocar nada: **son dos líneas, no una.**
//!
//! 1. La criba **léxica** (`normalize::NetworkScreen::allows_host`), sobre el host escrito. Es
//!    la única que ve una dirección literal, porque el conector no llama al resolutor cuando el
//!    host ya parsea como IP.
//! 2. El **resolutor** (`dns::ScreeningResolver`), sobre cada dirección a la que se va a marcar.
//!    Es la única que ve un nombre. `localtest.me`, `lvh.me`, `nip.io` y `sslip.io` son
//!    servicios públicos de DNS comodín que devuelven la dirección escrita en el nombre: no
//!    hacen falta ni dominio propio ni infraestructura, y la criba léxica no puede verlos.
//!
//! El resolutor se inyecta (`dns::StaticLookup`) para no depender de que esos servicios sigan
//! existiendo: el test monta «un nombre público que responde `127.0.0.1`» por su cuenta.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::dns::{Lookup, StaticLookup};
use crawlforge_core::engine;
use crawlforge_core::job::{CrawlJob, CrawlMode};
use rusqlite::Connection;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use support::servidor::{Respuesta, ServidorDePruebas};

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
/// La dirección de metadatos de nube, la que se criba **siempre**.
const METADATA: IpAddr = IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254));

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-perimetro-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear el directorio temporal");
        Self { path }
    }

    fn store(&self) -> std::path::PathBuf {
        self.path.join("crawl.sqlite")
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn abrir(store: &std::path::Path) -> Connection {
    Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el fichero de rastreo")
}

/// `(status_code, error_kind, exclusion_reason)` de una URL, si tiene fila.
fn fila(conn: &Connection, url: &str) -> Option<(Option<i64>, Option<String>, Option<String>)> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT status_code, error_kind, exclusion_reason FROM urls WHERE url = ?1",
        [url],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .expect("consultar la fila")
}

/// El puerto en el que escucha un servidor de pruebas, como cadena.
fn puerto_de(servidor: &ServidorDePruebas) -> String {
    let base = servidor.base();
    base.trim_end_matches('/').rsplit(':').next().unwrap_or("0").to_string()
}

fn contar(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).expect("contar")
}

/// Un resolutor de mentira que manda todos estos nombres a `127.0.0.1`.
fn lookup_a_loopback(nombres: &[&str]) -> Arc<dyn Lookup> {
    Arc::new(StaticLookup::new(
        nombres.iter().map(|n| ((*n).to_string(), vec![LOOPBACK])),
    ))
}

// ─── 1. Un nombre público que resuelve a la red del usuario ──────────────────────────

#[tokio::test]
async fn un_nombre_publico_que_responde_loopback_no_llega_al_servicio_local() {
    // El agujero estaba declarado como teórico y no lo es: `localtest.me` es un servicio
    // público de DNS comodín que responde `127.0.0.1`, no exige que el atacante controle
    // ningún dominio y cabe en un `<a href>` de 45 caracteres. Comprobado de extremo a extremo
    // con la criba encendida: `http://localtest.me:P/panel` llegó a un servicio en loopback y
    // respondió 200.
    let victima =
        ServidorDePruebas::arrancar(&[("/panel", Respuesta::pagina("Panel", "<p>interno</p>"))])
            .await;
    let puerto_victima = puerto_de(&victima);
    let auditado = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![(
            "/".to_string(),
            Respuesta::pagina(
                "Inicio",
                &format!("<a href=\"http://comodin.es:{puerto_victima}/panel\">x</a>"),
            ),
        )]
    })
    .await;
    let puerto_auditado = puerto_de(&auditado);

    let tmp = Temporal::new("comodin");
    let mut job = CrawlJob::http(format!("http://auditado.es:{puerto_auditado}/"));
    job.discover_sitemaps = false;

    let outcome = engine::run_http_with_lookup(
        job,
        &tmp.store(),
        // Los dos nombres responden `127.0.0.1`; solo uno es el objetivo del rastreo.
        lookup_a_loopback(&["auditado.es", "comodin.es"]),
    )
    .await
    .expect("rastrear");

    // El sitio auditado sí se alcanza: su nombre es público y **es el objetivo declarado**, así
    // que resolver a una dirección privada —DNS de horizonte partido— no lo deja fuera.
    assert_eq!(outcome.metrics.urls_fetched, 1, "la semilla tuvo que rastrearse");
    assert_eq!(victima.peticiones("/panel"), 0, "la víctima no puede recibir ni una petición");

    let conn = abrir(&tmp.store());
    let externa = format!("http://comodin.es:{puerto_victima}/panel");
    let (status, error, motivo) = fila(&conn, &externa).expect("la externa sí se registra");
    assert_eq!(status, None, "sin estado: la petición no llegó a hacerse");
    assert_eq!(
        error, None,
        "y sin error de red: no es un fallo del sitio ajeno, es una decisión nuestra"
    );
    assert_eq!(motivo.as_deref(), Some("local_network"));
    assert_eq!(outcome.metrics.externals_unchecked, 1, "el resumen tiene que decirlo");
}

#[tokio::test]
async fn el_endpoint_de_metadatos_no_se_alcanza_ni_por_un_nombre() {
    // `169.254.169.254.nip.io` se saltaba `is_cloud_metadata`, la única criba que el código
    // declaraba incondicional: la excepción que protege el endpoint de metadatos se anulaba
    // con un enlace. Ahora la decisión se toma sobre la dirección resuelta.
    let auditado = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![(
            "/".to_string(),
            Respuesta::pagina("Inicio", "<a href=\"http://meta-comodin.es/latest/\">x</a>"),
        )]
    })
    .await;
    let puerto = puerto_de(&auditado);

    let tmp = Temporal::new("metadatos-por-nombre");
    let mut job = CrawlJob::http(format!("http://auditado.es:{puerto}/"));
    job.discover_sitemaps = false;

    let lookup: Arc<dyn Lookup> = Arc::new(StaticLookup::new([
        ("auditado.es".to_string(), vec![LOOPBACK]),
        ("meta-comodin.es".to_string(), vec![METADATA]),
    ]));
    let outcome =
        engine::run_http_with_lookup(job, &tmp.store(), lookup).await.expect("rastrear");

    let conn = abrir(&tmp.store());
    let (status, error, motivo) =
        fila(&conn, "http://meta-comodin.es/latest/").expect("la externa se registra");
    assert_eq!(status, None);
    assert_eq!(error, None);
    assert_eq!(motivo.as_deref(), Some("local_network"));
    assert_eq!(outcome.metrics.externals_checked, 0);
}

#[tokio::test]
async fn una_sola_respuesta_privada_descarta_el_nombre_entero() {
    // Un nombre con varios registros `A`, uno público y otro privado, no puede pasar: quedarse
    // con las públicas deja que quien controla el DNS elija a qué dirección se conecta.
    let auditado = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![(
            "/".to_string(),
            Respuesta::pagina("Inicio", "<a href=\"http://mezcla.es/x\">x</a>"),
        )]
    })
    .await;
    let puerto = puerto_de(&auditado);

    let tmp = Temporal::new("mezcla");
    let mut job = CrawlJob::http(format!("http://auditado.es:{puerto}/"));
    job.discover_sitemaps = false;

    let lookup: Arc<dyn Lookup> = Arc::new(StaticLookup::new([
        ("auditado.es".to_string(), vec![LOOPBACK]),
        // Una pública y una de la red del usuario, en ese orden.
        (
            "mezcla.es".to_string(),
            vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))],
        ),
    ]));
    engine::run_http_with_lookup(job, &tmp.store(), lookup).await.expect("rastrear");

    let conn = abrir(&tmp.store());
    let (status, _, motivo) = fila(&conn, "http://mezcla.es/x").expect("la externa se registra");
    assert_eq!(status, None, "no se marca a ninguna de las dos");
    assert_eq!(motivo.as_deref(), Some("local_network"));
}

// ─── 2. `follow_external` no esquiva el perímetro ────────────────────────────────────

#[tokio::test]
async fn follow_external_no_es_un_permiso_para_llegar_a_la_red_del_usuario() {
    // `is_probeable` solo se consultaba desde `why_not_probe` y desde `ResumeScope`: la rama de
    // encolado de enlaces solo miraba `is_internal || follow_external`. Ejecutado: semilla
    // pública, `follow_external: true`, y el `GET` llegó a un servicio en loopback con
    // `status=200` y **el título y el h1 del panel guardados en el fichero**. Es peor que el
    // agujero que 0.5.0 cerró: cuerpo entero, parseado y almacenado.
    let victima = ServidorDePruebas::arrancar(&[(
        "/app/kibana",
        Respuesta::pagina("INTERNAL KIBANA", "<h1>elastic admin</h1>"),
    )])
    .await;
    let puerto_victima = puerto_de(&victima);
    // El enlace es una **dirección literal**, que es la forma del caso reproducido y la que el
    // resolutor no puede ver: el conector no lo llama cuando el host ya parsea como IP. Aquí
    // solo puede pararlo la rama de encolado, que es lo que este test mide.
    let auditado = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![(
            "/".to_string(),
            Respuesta::pagina(
                "Inicio",
                &format!("<a href=\"http://127.0.0.1:{puerto_victima}/app/kibana\">x</a>"),
            ),
        )]
    })
    .await;
    let puerto = puerto_de(&auditado);

    let tmp = Temporal::new("follow-external");
    let mut job = CrawlJob::http(format!("http://auditado.es:{puerto}/"));
    job.discover_sitemaps = false;
    job.limits.follow_external = true;

    let outcome = engine::run_http_with_lookup(
        job,
        &tmp.store(),
        lookup_a_loopback(&["auditado.es"]),
    )
    .await
    .expect("rastrear");

    assert_eq!(victima.peticiones("/app/kibana"), 0, "ni una petición al servicio interno");

    let conn = abrir(&tmp.store());
    let externa = format!("http://127.0.0.1:{puerto_victima}/app/kibana");
    let (status, _, motivo) = fila(&conn, &externa).expect("la externa se registra igual");
    assert_eq!(status, None);
    assert_eq!(motivo.as_deref(), Some("local_network"));
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM pages"),
        1,
        "solo la página del sitio auditado: del servicio interno no se guarda ni el título"
    );

    // Y no entra en la cola siquiera: queda como externa registrada (`skipped`), no como URL
    // interna que se intentó rastrear y se excluyó al conectar. `fetch.rs` tiene su propia
    // guarda para el caso del literal —defensa en profundidad— y esta comprobación es la que
    // distingue las dos: sin el arreglo de la rama de encolado, la fila sería `excluded`.
    assert_eq!(
        conn.query_row(
            "SELECT crawl_state FROM urls WHERE url = ?1",
            [&externa],
            |r| r.get::<_, String>(0)
        )
        .expect("estado de la fila"),
        "skipped"
    );
    assert_eq!(outcome.metrics.urls_excluded, 0, "no se llegó a encolar");
    assert_eq!(outcome.metrics.externals_unchecked, 1);
}

#[tokio::test]
async fn una_reanudacion_no_hereda_follow_external_del_fichero() {
    // El fichero es entrada no confiable —«un rastreo = un fichero portable» que se comparte— y
    // `read_resume_plan` ya neutralizaba `tier` e `ignore_robots` del `config_json`. A
    // `follow_external` no lo tocaba, y era la puerta más ancha: reanudar un `.sqlite` con
    // `"follow_external":true` rastreaba entero el host que dijera el fichero.
    let ajeno = ServidorDePruebas::arrancar_como_otro_host(&[(
        "/privado",
        Respuesta::pagina("PANEL INTERNO", "<h1>secreto</h1>"),
    )])
    .await;
    let enlace = ajeno.url_como_otro_host("/privado");
    let propio = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![("/".to_string(), Respuesta::pagina("Inicio", &format!("<a href=\"{enlace}\">x</a>")))]
    })
    .await;

    let tmp = Temporal::new("resume-follow-external");
    let store = tmp.store();
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;
    engine::run(job, &store).await.expect("rastrear");

    // El fichero se manipula: se enciende `follow_external` en el `config_json` y se rearma el
    // rastreo como si el corte hubiera llegado antes de registrar la externa. Es lo que hace un
    // `.sqlite` compartido, y `read_resume_plan` ya neutralizaba `tier` e `ignore_robots` así.
    {
        let conn = Connection::open(&store).expect("abrir para escribir");
        let cfg: String = conn
            .query_row("SELECT config_json FROM crawl_meta LIMIT 1", [], |r| r.get(0))
            .expect("config");
        assert!(cfg.contains("\"follow_external\":false"), "el rastreo original no lo pedía");
        let cfg = cfg.replace("\"follow_external\":false", "\"follow_external\":true");
        conn.execute("UPDATE crawl_meta SET config_json = ?1, status = 'paused'", [&cfg])
            .expect("manipular la configuración");
        conn.execute("DELETE FROM links", []).expect("soltar las aristas");
        conn.execute("DELETE FROM urls WHERE is_internal = 0", []).expect("olvidar la externa");
        conn.execute("UPDATE urls SET crawl_state = 'pending'", []).expect("rearmar la semilla");
    }

    engine::resume(&store).await.expect("reanudar");

    // La externa se vuelve a descubrir y se le comprueba el estado —eso es lo normal— pero
    // **no se rastrea**: sin cuerpo, sin parseo y sin fila en `pages`.
    let metodos = ajeno.metodos("/privado");
    assert!(
        metodos.iter().all(|m| m == "HEAD"),
        "reanudar no concede un permiso que nadie ha vuelto a dar en esta sesión: {metodos:?}"
    );
    let conn = abrir(&store);
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM pages"),
        1,
        "solo la página del sitio auditado: del panel ajeno no se guarda ni el título"
    );
}

// ─── 3. La criba de metadatos, también al reanudar ───────────────────────────────────

#[tokio::test]
async fn la_criba_de_metadatos_sobrevive_a_una_reanudacion() {
    // `engine.rs` usaba `is_probeable_host` con guarda en la ruta de reanudación, y esa guarda
    // deja fuera `is_cloud_metadata`. Agravante: `NotProbed::LocalNetwork` no dejaba motivo, de
    // modo que la fila quedaba **idéntica** a una sonda interrumpida y el `SELECT` de
    // reanudación la volvía a coger. Ejecutado sin tocar el fichero: rastreo local con enlace a
    // `169.254.169.254`, Ctrl-C, `resume`, y la petición sale.
    const META: &str = "http://169.254.169.254/latest/meta-data/";
    let propio = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![("/".to_string(), Respuesta::pagina("dev", &format!("<a href=\"{META}\">m</a>")))]
    })
    .await;

    let tmp = Temporal::new("metadatos-reanudacion");
    let store = tmp.store();
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;
    // La semilla es local, así que la criba de red **está apagada** por diseño: auditar un
    // `astro dev` no puede romperse. La de metadatos no se apaga nunca, y eso es lo que se mide.
    engine::run(job, &store).await.expect("rastrear");

    let motivo_tras_el_rastreo = {
        let conn = abrir(&store);
        fila(&conn, META).expect("la externa se registra").2
    };
    assert_eq!(
        motivo_tras_el_rastreo.as_deref(),
        Some("local_network"),
        "la fila necesita motivo propio: sin él es indistinguible de una sonda a medias"
    );

    // Primera defensa: con motivo, el `SELECT` de la reanudación ya no la devuelve.
    {
        let conn = Connection::open(&store).expect("abrir para escribir");
        conn.execute("UPDATE crawl_meta SET status = 'paused'", []).expect("pausar");
    }
    engine::resume(&store).await.expect("reanudar");
    {
        let conn = abrir(&store);
        let (status, error, _) = fila(&conn, META).expect("sigue ahí");
        assert_eq!(status, None);
        assert_eq!(error, None, "no se pidió: un error de red probaría que sí");
    }

    // Segunda defensa: aunque la fila llegue sin motivo —de un binario anterior, o inyectada—
    // el perímetro se vuelve a aplicar al releerla.
    {
        let conn = Connection::open(&store).expect("abrir para escribir");
        conn.execute(
            "UPDATE urls SET exclusion_reason = NULL WHERE url = ?1",
            [META],
        )
        .expect("borrar el motivo");
        conn.execute("UPDATE crawl_meta SET status = 'paused'", []).expect("pausar");
    }
    let reanudado = engine::resume(&store).await.expect("reanudar");
    assert_eq!(reanudado.metrics.externals_checked, 0, "no se sonda el endpoint de metadatos");
    let conn = abrir(&store);
    let (status, error, _) = fila(&conn, META).expect("sigue ahí");
    assert_eq!(status, None);
    assert_eq!(error, None);
}

// ─── 4. Modo lista: una línea no decide por todas las demás ──────────────────────────

#[tokio::test]
async fn la_primera_linea_de_la_lista_no_apaga_la_criba_para_el_resto() {
    // `screen_local_network = seeds.first().is_probeable_host()`. En modo lista `seeds` es el
    // fichero del usuario en orden de fichero y la CLI no valida sus líneas. Ejecutado, tres
    // pasadas: con la primera línea pública la víctima recibió 0 peticiones; con
    // `http://localhost:P/dev` de primera, 1; y con `mailto:contacto@cliente.es` de primera,
    // 1 — sin host, la criba se apagaba para toda la lista.
    //
    // Un fichero de lista muchas veces llega de fuera. El criterio ahora es del conjunto: la
    // excepción de la red local exige que **todos** los objetivos sean locales.
    let victima =
        ServidorDePruebas::arrancar(&[("/admin", Respuesta::pagina("Admin", "<p>x</p>"))]).await;
    let puerto_victima = puerto_de(&victima);
    let sitio = ServidorDePruebas::arrancar_como_otro_host_con_puerto(|_| {
        vec![(
            "/dev".to_string(),
            Respuesta::pagina(
                "Dev",
                &format!("<a href=\"http://127.0.0.1:{puerto_victima}/admin\">x</a>"),
            ),
        )]
    })
    .await;

    // La lista empieza por un objetivo local y sigue por uno de internet. `.invalid` no resuelve
    // —RFC 2606—, así que el test no toca la red: lo único que importa de esa línea es que su
    // host es público y por tanto el rastreo ya no es una auditoría enteramente local.
    let tmp = Temporal::new("lista-primera-linea");
    let urls = vec![sitio.url_como_otro_host("/dev"), "https://cliente.invalid/".to_string()];
    let mut job = CrawlJob::http(urls[0].clone());
    job.mode = CrawlMode::List { urls };
    job.discover_sitemaps = false;

    engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(victima.peticiones("/admin"), 0, "la víctima no recibe ninguna petición");

    let conn = abrir(&tmp.store());
    let externa = format!("http://127.0.0.1:{puerto_victima}/admin");
    let (status, _, motivo) = fila(&conn, &externa).expect("la externa se registra");
    assert_eq!(status, None);
    assert_eq!(motivo.as_deref(), Some("local_network"));
}

#[tokio::test]
async fn una_lista_enteramente_local_sigue_alcanzando_su_propia_red() {
    // El reverso, y es la razón de que la excepción exista: auditar un `astro dev` o el pre de
    // un cliente en la LAN significa que quien lanzó el rastreo ya está dentro de esa red.
    let ajeno =
        ServidorDePruebas::arrancar_como_otro_host(&[("/guia", Respuesta::pagina("Guía", "<p>x</p>"))])
            .await;
    let propio = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![("/dev".to_string(), Respuesta::pagina("Dev", "<p>x</p>"))]
    })
    .await;
    let enlace = ajeno.url_como_otro_host("/guia");
    let _ = &enlace;

    let tmp = Temporal::new("lista-toda-local");
    let urls = vec![propio.url("/dev")];
    let mut job = CrawlJob::http(urls[0].clone());
    job.mode = CrawlMode::List { urls };
    job.discover_sitemaps = false;
    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 1, "la lista se audita como siempre");
}

// ─── 6. El registro de externas también tiene tope ───────────────────────────────────

#[tokio::test]
async fn el_registro_de_externas_tiene_tope_y_lo_dice() {
    // Medido en `--release`: **una sola página** de 9,3 MB con 350.000 `<a>` a hosts distintos
    // dejaba 350.001 filas en `urls`, 87 MB de fichero, 279 MB de RSS y 10,8 s, con
    // `max_urls: Some(1000)`. Las externas no cuentan contra `max_urls` a propósito, y hasta
    // este tope nada más las acotaba. Avisa en vez de truncar en silencio, como el resto.
    let enlaces: String =
        (0..20).map(|i| format!("<a href=\"https://h{i}.invalid/\">x</a> ")).collect();
    let propio = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![("/".to_string(), Respuesta::pagina("Inicio", &enlaces))]
    })
    .await;

    let tmp = Temporal::new("tope-registro");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;
    job.limits.check_external = false;
    job.limits.max_external_urls = 5;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_unregistered, 15, "quince quedaron fuera y se dicen");

    let conn = abrir(&tmp.store());
    assert_eq!(
        contar(&conn, "SELECT COUNT(*) FROM urls WHERE is_internal = 0"),
        5,
        "solo se registran las que caben en el tope"
    );
    // El tope de externas **no** trunca el rastreo del sitio: marcarlo apagaría las reglas de
    // `REQUIERE_GRAFO_COMPLETO`, y el grafo del sitio del usuario está completo.
    assert_eq!(contar(&conn, "SELECT truncated FROM crawl_meta"), 0);
}

#[tokio::test]
async fn por_defecto_el_tope_del_registro_no_estorba() {
    // Veinte externas en un rastreo normal no pueden verse afectadas por un tope de 100.000.
    let enlaces: String =
        (0..20).map(|i| format!("<a href=\"https://h{i}.invalid/\">x</a> ")).collect();
    let propio = ServidorDePruebas::arrancar_con_puerto(|_| {
        vec![("/".to_string(), Respuesta::pagina("Inicio", &enlaces))]
    })
    .await;

    let tmp = Temporal::new("tope-por-defecto");
    let mut job = CrawlJob::http(propio.base());
    job.discover_sitemaps = false;
    job.limits.check_external = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.externals_unregistered, 0);
    let conn = abrir(&tmp.store());
    assert_eq!(contar(&conn, "SELECT COUNT(*) FROM urls WHERE is_internal = 0"), 20);
}
