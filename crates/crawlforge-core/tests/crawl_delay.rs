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
async fn un_host_con_crawl_delay_no_esconde_la_cola_de_los_demas() {
    // El defecto que este test fija, y que el de abajo **no cazaba**: el planificador retenía
    // las URLs de hosts saturados en un búfer de 200 y, una vez lleno, dejaba de mirar el
    // frontier. Con `force_serial` un host con `Crawl-delay` queda a una petición en vuelo y
    // llena ese búfer enseguida, así que todo lo que estuviera detrás —incluidos hosts libres—
    // dejaba de despacharse. Reproducido: 250 URLs del host lento por delante de 5 del libre,
    // y a los veinte segundos el libre seguía a cero de cinco.
    //
    // `el_crawl_delay_de_un_host_no_frena_a_los_demas` usa seis URLs y nunca llena el búfer:
    // pasaba con el fallo puesto. Este necesita pasar de doscientas para significar algo.
    let lento = ServidorDePruebas::arrancar_con_puerto(|_| {
        let mut rutas = vec![(
            "/robots.txt".to_string(),
            Respuesta::texto("User-agent: *\nCrawl-delay: 1\n"),
        )];
        for i in 0..250 {
            rutas.push((format!("/lento-{i}"), Respuesta::pagina("Lento", "<p>x</p>")));
        }
        rutas
    })
    .await;
    let rapido = ServidorDePruebas::arrancar_como_otro_host_con_puerto(|_| {
        (0..5)
            .map(|i| (format!("/libre-{i}"), Respuesta::pagina("Libre", "<p>x</p>")))
            .collect()
    })
    .await;

    // Las 250 del host lento van **delante** en la lista: es el orden en que el frontier las
    // sirve, y por tanto el que enterraba a las cinco de detrás.
    let mut urls: Vec<String> = (0..250).map(|i| lento.url(&format!("/lento-{i}"))).collect();
    urls.extend((0..5).map(|i| rapido.url_como_otro_host(&format!("/libre-{i}"))));

    let tmp = Temporal::new("hambre");
    let mut job = CrawlJob::http(lento.base());
    job.mode = CrawlMode::List { urls };
    job.discover_sitemaps = false;
    // El presupuesto de tiempo acota el test: el host lento tardaría 250 segundos en drenar,
    // y lo que se afirma es que el libre no tiene que esperar a que lo haga.
    job.limits.max_duration = Some(Duration::from_secs(3));

    engine::run(job, &tmp.store()).await.expect("rastrear");

    for i in 0..5 {
        let ruta = format!("/libre-{i}");
        assert_eq!(
            rapido.peticiones(&ruta),
            1,
            "{ruta} no llegó a pedirse: el host libre quedó enterrado detrás del lento"
        );
    }
}

#[tokio::test]
async fn el_robots_txt_de_un_host_se_descarga_una_sola_vez() {
    // La caché era *check-then-fetch* sin puerta: las N tareas de la primera ola de un host
    // fallaban el `get` a la vez y las N descargaban el fichero. Medido en un rastreo real:
    // 122 peticiones para 25 hosts, 4,9x — exactamente la concurrencia por host.
    //
    // Vive en este fichero porque es el mismo problema por otro lado: esa ráfaga simultánea de
    // N peticiones es justo la que el `Crawl-delay` existe para evitar, y ocurre **antes** de
    // poder saber que el host lo declara.
    let mut rutas = vec![(
        "/robots.txt".to_string(),
        // El fichero tarda: sin la puerta, las tareas de la primera ola pasan por el `get`
        // mientras la primera descarga sigue en vuelo, y todas piden.
        Respuesta::texto("User-agent: *\nDisallow: /privado/\n")
            .con_retardo(Duration::from_millis(200)),
    )];
    for i in 0..12 {
        rutas.push((format!("/p{i}"), Respuesta::pagina("P", "<p>x</p>")));
    }
    let servidor = ServidorDePruebas::arrancar_con_puerto(|_| rutas).await;

    let tmp = Temporal::new("robots-single-flight");
    let mut job = CrawlJob::http(servidor.base());
    // Modo lista, y no una semilla que enlaza: con una sola semilla no hay primera ola —la
    // portada se pide sola y para cuando se descubren sus enlaces el fichero ya está en la
    // caché—. La ráfaga aparece cuando el motor despacha `concurrency` URLs del mismo host de
    // golpe, que es lo que pasa en un rastreo de lista y en uno de cartera: ahí se midieron
    // las 122 peticiones para 25 hosts.
    job.mode = CrawlMode::List {
        urls: (0..12).map(|i| servidor.url(&format!("/p{i}"))).collect(),
    };
    job.discover_sitemaps = false;

    let outcome = engine::run(job, &tmp.store()).await.expect("rastrear");
    assert_eq!(outcome.metrics.urls_fetched, 12, "la lista entera");
    assert_eq!(
        servidor.peticiones("/robots.txt"),
        1,
        "«se descarga una vez por host» tiene que ser verdad, no una promesa de la doc"
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
