//! `Crawl-delay` anula la concurrencia configurada para su host, como promete `robots.rs`.
//!
//! El defecto que fija este fichero: el retardo se aplicaba como un `sleep` **dentro de cada
//! tarea**, así que con concurrencia 5 las cinco tareas dormían el retardo en paralelo y las
//! cinco peticiones salían juntas. El retardo espaciaba lotes, no peticiones: para el servidor
//! era exactamente la ráfaga que el `Crawl-delay` pedía evitar.
//!
//! El comportamiento correcto, y lo que afirman estos tests: un host que declara `Crawl-delay`
//! se rastrea con una sola petición en vuelo, entre el arranque de una petición y el de la
//! siguiente pasa al menos el retardo declarado, y los demás hosts del rastreo conservan la
//! concurrencia que pidió el usuario.
//!
//! Los tests afirman orden de sucesos y cotas holgadas, nunca duraciones exactas: en una
//! máquina cargada las esperas solo pueden crecer, y las aserciones están orientadas para que
//! ese crecimiento no las vuelva rojas.

// El servidor de pruebas es compartido y cada binario de test usa solo una parte de su API.
#[allow(dead_code)]
mod support;

use crawlforge_core::{
    engine,
    job::{CrawlJob, CrawlMode},
};
use std::time::{Duration, Instant};
use support::servidor::{Respuesta, ServidorDePruebas};

/// Directorio temporal que se limpia solo. Mismo patrón que `pipeline.rs`.
struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-delay-{}-{nombre}", std::process::id()));
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

/// Los instantes de llegada de varias rutas, mezclados y ordenados. Cada ruta de estos tests
/// se pide exactamente una vez, así que la lista resultante es «cuándo arrancó cada petición».
fn llegadas_ordenadas(servidor: &ServidorDePruebas, rutas: &[&str]) -> Vec<Instant> {
    let mut todas: Vec<Instant> = rutas
        .iter()
        .flat_map(|ruta| {
            let l = servidor.llegadas(ruta);
            assert_eq!(l.len(), 1, "la ruta {ruta} debería haberse pedido exactamente una vez");
            l
        })
        .collect();
    todas.sort();
    todas
}

#[tokio::test]
async fn un_crawl_delay_espacia_los_arranques_en_vez_de_agruparlos() {
    // Con el defecto, las tres tareas dormían el segundo en paralelo y las tres peticiones
    // llegaban juntas: los huecos entre llegadas eran de milisegundos.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/robots.txt", Respuesta::texto("User-agent: *\nCrawl-delay: 1\n")),
        ("/a", Respuesta::pagina("A", "<p>a</p>")),
        ("/b", Respuesta::pagina("B", "<p>b</p>")),
        ("/c", Respuesta::pagina("C", "<p>c</p>")),
    ])
    .await;

    let tmp = Temporal::new("espaciado");
    let mut job = CrawlJob::http(servidor.base());
    // Modo lista: el conjunto exacto de URLs, sin descubrimiento que meta ruido en los tiempos.
    job.mode = CrawlMode::List {
        urls: vec![servidor.url("/a"), servidor.url("/b"), servidor.url("/c")],
    };
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 3, "el retardo no puede perder URLs");

    let llegadas = llegadas_ordenadas(&servidor, &["/a", "/b", "/c"]);
    for par in llegadas.windows(2) {
        let hueco = par[1].duration_since(par[0]);
        // La cota es holgada (700 ms para un retardo de 1 s): el reloj del test y el del motor
        // no son el mismo instante exacto. Con el defecto el hueco era de milisegundos.
        assert!(
            hueco >= Duration::from_millis(700),
            "dos arranques a {hueco:?} el uno del otro: el Crawl-delay no los espació"
        );
    }
}

#[tokio::test]
async fn un_crawl_delay_mantiene_una_sola_peticion_en_vuelo() {
    // El servidor tarda en responder más que el propio retardo. Si el motor solo espaciara los
    // arranques sin retener el permiso durante la petición, la segunda arrancaría al segundo,
    // con la primera todavía en vuelo, y el hueco entre llegadas sería ~1 s. Reteniéndolo, la
    // segunda no puede arrancar hasta que la primera responde: el hueco es ~3 s.
    let servidor = ServidorDePruebas::arrancar(&[
        ("/robots.txt", Respuesta::texto("User-agent: *\nCrawl-delay: 1\n")),
        ("/a", Respuesta::pagina("A", "<p>a</p>").con_retardo(Duration::from_secs(3))),
        ("/b", Respuesta::pagina("B", "<p>b</p>").con_retardo(Duration::from_secs(3))),
    ])
    .await;

    let tmp = Temporal::new("en-vuelo");
    let mut job = CrawlJob::http(servidor.base());
    job.mode = CrawlMode::List { urls: vec![servidor.url("/a"), servidor.url("/b")] };
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 2);

    let llegadas = llegadas_ordenadas(&servidor, &["/a", "/b"]);
    let hueco = llegadas[1].duration_since(llegadas[0]);
    assert!(
        hueco >= Duration::from_secs(2),
        "la segunda petición arrancó a {hueco:?} de la primera, con ella aún en vuelo"
    );
}

#[tokio::test]
async fn el_crawl_delay_de_un_host_no_frena_a_los_demas() {
    // Dos hosts en el mismo rastreo: `127.0.0.1` declara Crawl-delay y `localhost` no. El
    // lento se serializa; el rápido conserva la concurrencia del usuario. Sin esta propiedad,
    // un solo host con retardo serializaría un rastreo de cartera entero.
    let lento = ServidorDePruebas::arrancar(&[
        ("/robots.txt", Respuesta::texto("User-agent: *\nCrawl-delay: 1\n")),
        ("/a1", Respuesta::pagina("A1", "<p>a</p>")),
        ("/a2", Respuesta::pagina("A2", "<p>a</p>")),
        ("/a3", Respuesta::pagina("A3", "<p>a</p>")),
    ])
    .await;
    // Sin ruta `/robots.txt`: responde 404 y se permite todo, como un sitio sin fichero.
    let rapido = ServidorDePruebas::arrancar_como_otro_host(&[
        ("/b1", Respuesta::pagina("B1", "<p>b</p>")),
        ("/b2", Respuesta::pagina("B2", "<p>b</p>")),
        ("/b3", Respuesta::pagina("B3", "<p>b</p>")),
    ])
    .await;

    let tmp = Temporal::new("dos-hosts");
    let mut job = CrawlJob::http(lento.base());
    job.mode = CrawlMode::List {
        urls: vec![
            lento.url("/a1"),
            rapido.url_como_otro_host("/b1"),
            lento.url("/a2"),
            rapido.url_como_otro_host("/b2"),
            lento.url("/a3"),
            rapido.url_como_otro_host("/b3"),
        ],
    };
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 6, "los dos hosts se rastrean enteros");

    // Orden de sucesos, no duraciones: el host rápido termina entero antes de que el lento
    // arranque su segunda petición, que por el retardo llega al menos un segundo tarde.
    let llegadas_lento = llegadas_ordenadas(&lento, &["/a1", "/a2", "/a3"]);
    let llegadas_rapido = llegadas_ordenadas(&rapido, &["/b1", "/b2", "/b3"]);
    let ultima_rapida = llegadas_rapido[2];
    let segunda_lenta = llegadas_lento[1];
    assert!(
        ultima_rapida < segunda_lenta,
        "el host sin Crawl-delay quedó retenido detrás del host con retardo"
    );
}
