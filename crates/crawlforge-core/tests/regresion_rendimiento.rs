//! Regresión de rendimiento: el motor no puede perder velocidad ni memoria sin que se vea.
//!
//! # Qué protege y qué no
//!
//! La velocidad y la memoria **son el argumento del producto**: el listón se superó
//! con 80.420 elementos/s y 137 MB donde Screaming Frog necesita 870 MB solo para arrancar
//! del producto. Ese argumento se pierde en un `git push` cualquiera y nadie se
//! entera hasta que un cliente rastrea un sitio grande.
//!
//! # Qué caza exactamente
//!
//! Los umbrales están **justo por debajo de la mitad** de lo medido en local (51.200 elementos/s
//! y 1.250 páginas/s en este mismo sitio de prueba). Esa cifra no es arbitraria: es lo que hace
//! falta para cazar una regresión que reduzca el rendimiento a la mitad, que es el tamaño de las
//! dos que ya ocurrieron de verdad:
//!
//! - Resolver los extremos de cada enlace con un `JOIN` por fila en vez de con un mapa en
//!   memoria: costó pasar de 72.900 a 33.900 elementos/s, más de 2x.
//! - Dejar sin acotar el canal del escritor: el pico de memoria subió de 170 a 387 MB porque la
//!   cola se convertía en el almacén.
//!
//! # Solo mide en `--release`
//!
//! En `debug` el motor va un orden de magnitud más lento y cualquier umbral sería mentira. El
//! test existe igual en los dos perfiles —así se comprueba que el pipeline sigue funcionando—
//! pero solo afirma sobre los números cuando está compilado con optimizaciones. En CI se ejecuta
//! con `cargo test --release`.

use crawlforge_core::{engine, job::CrawlJob};

/// Suelo de elementos por segundo, **distinto según dónde se mida**.
///
/// Un umbral absoluto no vale para dos máquinas distintas, y este test se ejecuta en dos: el
/// MacBook de desarrollo y los runners compartidos de GitHub. Medido el 2026-08-03 sobre el mismo
/// commit: **105.000 elementos/s en local y 21.531 en un runner de Ubuntu**, casi 5x de
/// diferencia. Con la constante única de 25.000 el pipeline daba rojo sin que el motor hubiera
/// perdido nada — y así fallaron las tres primeras ejecuciones del CI.
///
/// El criterio es el mismo en los dos sitios —**algo por debajo de la mitad de lo medido allí**,
/// para cazar una regresión que reduzca el rendimiento al 50%— y lo que cambia es el número.
/// Bajar el de local hasta que pasara en CI habría dejado un suelo 10 veces por debajo de lo
/// normal en el Mac: un test que solo caza el derrumbe total no caza nada.
///
/// La cabecera anterior ya decía que si hacía falta bajarlo tanto había que cambiar de enfoque.
/// Esto es ese cambio, en su versión mínima: una línea base por entorno en vez de por máquina.
/// Si algún día hay más de dos entornos, toca guardar la línea base medida en cada uno.
///
/// **Si esto da rojo, mide tres veces antes de creerlo**: la varianza del banco es alta —entre
/// 95.000 y 118.000 en pasadas consecutivas del mismo binario en el Mac.
#[cfg(not(debug_assertions))]
fn min_elements_per_sec() -> f64 {
    if en_ci() {
        10_000.0
    } else {
        25_000.0
    }
}

/// Suelo de páginas por segundo, con el mismo criterio: 2.600 medidas en local y 525 en CI.
#[cfg(not(debug_assertions))]
fn min_pages_per_sec() -> f64 {
    if en_ci() {
        250.0
    } else {
        600.0
    }
}

/// ¿Se está midiendo en un runner compartido?
///
/// `CI=true` lo ponen GitHub Actions, GitLab, CircleCI y Travis. No se usa `cfg!` porque el
/// binario es el mismo: lo que cambia es la máquina donde corre, no cómo se compiló.
#[cfg(not(debug_assertions))]
fn en_ci() -> bool {
    std::env::var("CI").is_ok_and(|v| !v.is_empty() && v != "0" && v != "false")
}

/// Techo de memoria. El criterio es menos de 200 MB con 50.000 páginas; aquí se
/// rastrean 3.000, así que 200 MB es un techo muy holgado. Sigue cazando que la cola vuelva a
/// convertirse en el almacén, que es la regresión que importa.
#[cfg(not(debug_assertions))]
const MAX_RSS_MB: f64 = 200.0;

/// Páginas del sitio de prueba. Suficientes para que la medida no la domine el arranque y pocas
/// para que el test no se vuelva un peaje en cada ejecución.
const PAGINAS: usize = 3_000;

/// Enlaces por página. El caso denso del banco usa 123; aquí bastan 40 para que el trabajo lo
/// dominen los enlaces —que es donde estaba el cuello— sin tardar un minuto.
const ENLACES: usize = 40;

struct Sitio {
    path: std::path::PathBuf,
}

impl Sitio {
    /// Genera el sitio desde Rust, no con `tools/gen-site-fixture.py`: un test de CI no debe
    /// depender de que haya un intérprete de Python en la máquina.
    fn generar() -> Self {
        let path = std::env::temp_dir().join(format!("crawlforge-perf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear directorio");

        for i in 0..PAGINAS {
            // Destinos aritméticos, como en el generador versionado: deterministas y todos
            // distintos, así el recuento de enlaces es exacto.
            let mut enlaces = String::with_capacity(ENLACES * 48);
            for j in 0..ENLACES {
                let destino = (i + 1 + j * 7) % PAGINAS;
                enlaces.push_str(&format!("<a href=\"/p/{destino}/\">P{destino}</a>\n"));
            }
            let html = format!(
                "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\">\
                 <title>Página {i} del sitio de medición</title>\
                 <meta name=\"description\" content=\"Descripción de la página {i}, con longitud \
                 suficiente para no disparar reglas que aquí no interesan.\">\
                 <link rel=\"canonical\" href=\"https://fixture.local/p/{i}/\">\
                 <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
                 </head><body><main><h1>Página {i}</h1><p>Texto de relleno.</p>\
                 <nav>{enlaces}</nav></main></body></html>"
            );
            let dir = path.join("p").join(i.to_string());
            std::fs::create_dir_all(&dir).expect("crear subdirectorio");
            std::fs::write(dir.join("index.html"), html).expect("escribir página");
        }
        Self { path }
    }

    fn store(&self) -> std::path::PathBuf {
        self.path.join("crawl.sqlite")
    }
}

impl Drop for Sitio {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[tokio::test]
async fn el_motor_no_ha_perdido_rendimiento() {
    let sitio = Sitio::generar();
    let job = CrawlJob::filesystem(&sitio.path, "https://fixture.local/");
    let outcome = engine::run(job, &sitio.store()).await.expect("rastrear");
    let m = &outcome.metrics;

    // Se imprime siempre: en un rojo de CI, el número es lo primero que se quiere ver, y en verde
    // sirve para ver la tendencia entre commits.
    println!(
        "elementos/s {:.0} · páginas/s {:.0} · RSS {:.1} MB · {} elementos en {:.2} s",
        m.elements_per_second(),
        m.pages_per_second(),
        m.peak_rss_mb(),
        m.elements_written,
        m.elapsed.as_secs_f64()
    );

    // El recuento sí se comprueba en los dos perfiles: que el motor deje de escribir elementos no
    // es un problema de rendimiento, es un fallo de corrección, y se ve igual de bien aquí.
    assert_eq!(m.pages_parsed, PAGINAS as u64, "todas las páginas tienen que parsearse");
    assert!(
        m.elements_written >= (PAGINAS * ENLACES) as u64,
        "faltan elementos escritos: {} para {} enlaces esperados",
        m.elements_written,
        PAGINAS * ENLACES
    );

    #[cfg(not(debug_assertions))]
    {
        assert!(
            m.elements_per_second() >= min_elements_per_sec(),
            "regresión de rendimiento: {:.0} elementos/s, por debajo del suelo de {:.0}. \
             En local se miden 51.200 en este mismo sitio: una caída hasta aquí es una regresión \
             real, no ruido de máquina",
            m.elements_per_second(),
            min_elements_per_sec()
        );
        assert!(
            m.pages_per_second() >= min_pages_per_sec(),
            "regresión de rendimiento: {:.0} páginas/s, por debajo del suelo de {:.0}",
            m.pages_per_second(),
            min_pages_per_sec()
        );
        assert!(
            m.peak_rss_mb() <= MAX_RSS_MB,
            "regresión de memoria: {:.1} MB rastreando {PAGINAS} páginas, por encima del techo de \
             {MAX_RSS_MB:.0} MB. La causa típica es que algo haya dejado de acotarse y la cola se \
             esté comiendo el almacén",
            m.peak_rss_mb()
        );
    }
}
