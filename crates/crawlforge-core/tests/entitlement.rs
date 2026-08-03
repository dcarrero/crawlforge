//! El nivel se hace cumplir en el motor, no en la interfaz.
//!
//! Lo que este test demuestra: que un rastreo con `Tier::Free` corta a
//! 1.000 URLs y no evalúe reglas por encima de su nivel. Es un test de integración y no unitario
//! a propósito: lo que hay que demostrar es que el tope llega hasta el bucle de rastreo, porque
//! si la comprobación viviera solo en la UI, la CLI la esquivaría.

use crawlforge_core::engine::{self, TruncationReason};
use crawlforge_core::entitlement::{Limits, Tier, FREE_MAX_URLS};
use crawlforge_core::job::CrawlJob;

struct Sitio {
    path: std::path::PathBuf,
}

impl Sitio {
    /// Sitio de `n` páginas mínimas, encadenadas para que el rastreo las alcance todas.
    fn nuevo(nombre: &str, n: usize) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-ent-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear directorio");

        for i in 0..n {
            let siguiente = (i + 1) % n;
            let html = format!(
                "<!DOCTYPE html><html lang=\"es\"><head><title>Página {i} del sitio de prueba</title>\
                 <link rel=\"canonical\" href=\"https://fixture.local/p/{i}/\"></head>\
                 <body><main><h1>Página {i}</h1><a href=\"/p/{siguiente}/\">Siguiente</a></main></body></html>"
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

fn abrir(path: &std::path::Path) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el rastreo")
}

#[tokio::test]
async fn el_nivel_gratuito_corta_el_rastreo_en_su_tope() {
    // Un sitio con más páginas que el tope del nivel gratuito. Cinco de más bastan para
    // demostrar el corte y mantienen el test en menos de un segundo.
    let n = FREE_MAX_URLS as usize + 5;
    let sitio = Sitio::nuevo("free", n);

    let mut job = CrawlJob::filesystem(&sitio.path, "https://fixture.local/");
    job.tier = Tier::Free;

    let outcome = engine::run(job, &sitio.store()).await.expect("rastrear");

    assert_eq!(
        outcome.truncated,
        Some(TruncationReason::MaxUrls),
        "el rastreo debe terminar por el tope del nivel, no por agotar la cola"
    );
    assert!(
        outcome.metrics.urls_fetched <= FREE_MAX_URLS,
        "el nivel gratuito rastreó {} URLs, por encima de su tope de {FREE_MAX_URLS}",
        outcome.metrics.urls_fetched
    );
}

#[tokio::test]
async fn el_tope_del_nivel_no_se_puede_subir_desde_el_trabajo() {
    let n = FREE_MAX_URLS as usize + 5;
    let sitio = Sitio::nuevo("free-forzado", n);

    let mut job = CrawlJob::filesystem(&sitio.path, "https://fixture.local/");
    job.tier = Tier::Free;
    // Lo que haría un build modificado o un fichero de configuración a mano.
    job.limits.max_urls = Some(50_000);

    let outcome = engine::run(job, &sitio.store()).await.expect("rastrear");

    assert!(
        outcome.metrics.urls_fetched <= FREE_MAX_URLS,
        "pedir 50.000 URLs en el nivel gratuito dio {}",
        outcome.metrics.urls_fetched
    );
}

#[tokio::test]
async fn un_tope_mas_bajo_que_el_del_nivel_si_se_respeta() {
    // El nivel es un techo, no un suelo: un rastreo de prueba de 10 URLs sigue siendo de 10.
    let sitio = Sitio::nuevo("free-bajo", 40);

    let mut job = CrawlJob::filesystem(&sitio.path, "https://fixture.local/");
    job.tier = Tier::Free;
    job.limits.max_urls = Some(10);

    let outcome = engine::run(job, &sitio.store()).await.expect("rastrear");
    assert!(outcome.metrics.urls_fetched <= 10, "rastreó {}", outcome.metrics.urls_fetched);
}

#[tokio::test]
async fn los_niveles_de_pago_no_cortan() {
    let sitio = Sitio::nuevo("agency", 30);

    let mut job = CrawlJob::filesystem(&sitio.path, "https://fixture.local/");
    job.tier = Tier::Agency;

    let outcome = engine::run(job, &sitio.store()).await.expect("rastrear");
    assert_eq!(outcome.truncated, None, "sin tope no debe truncarse");
    assert_eq!(outcome.metrics.urls_fetched, 30);
}

#[test]
fn el_nivel_gratuito_no_evalua_reglas_de_pago() {
    // La otra mitad de lo que pide §2.7. Hoy el catálogo es todo `free`, así que esto no filtra
    // nada; el test existe para que el día que entre la primera regla Pro, un descuido en el
    // filtrado se vea aquí y no en un rastreo de un cliente que ve hallazgos que no ha pagado.
    for rule in crawlforge_rules::page_rules_for_tier(Tier::Free) {
        assert_eq!(rule.min_tier(), Tier::Free, "{} no es del nivel gratuito", rule.id());
    }
    for rule in crawlforge_rules::site_rules_for_tier(Tier::Free) {
        assert_eq!(rule.min_tier(), Tier::Free, "{} no es del nivel gratuito", rule.id());
    }

    // Y el nivel superior no pierde ninguna: Pro es un superconjunto de Free.
    assert!(
        crawlforge_rules::page_rules_for_tier(Tier::Agency).len()
            >= crawlforge_rules::page_rules_for_tier(Tier::Free).len()
    );
}

#[test]
fn los_limites_del_nivel_gratuito_son_los_del_documento() {
    // `docs/07-MONETIZACION.md §2`: un proyecto, 1.000 URLs, sin panel de cartera.
    let free = Limits::for_tier(Tier::Free);
    assert_eq!(free.max_urls, Some(1_000));
    assert_eq!(free.max_projects, Some(1));
    assert_eq!(free.max_portfolio_sites, None);
}

#[tokio::test]
async fn el_texto_completo_para_busquedas_es_de_pago() {
    // `02-MODELO-DATOS.md §3.7` dice que el índice FTS «solo se puebla en nivel Pro», y desde
    // que la tabla se puebla de verdad eso hay que hacerlo cumplir en el core: un trabajo
    // gratuito podía pedir el cuerpo, guardarlo e indexarlo.
    let sitio = Sitio::nuevo("fts-free", 3);

    let mut job = CrawlJob::filesystem(&sitio.path, "https://fixture.local/");
    job.tier = Tier::Free;
    job.collect_body_text = true;
    let outcome = engine::run(job, &sitio.store()).await.expect("rastrear");

    let conn = abrir(&outcome.store_path);
    let indexadas: i64 =
        conn.query_row("SELECT COUNT(*) FROM pages_fts", [], |r| r.get(0)).expect("contar");
    assert_eq!(indexadas, 0, "el nivel gratuito no indexa texto aunque lo pida el trabajo");

    // El texto no se guarda en `pages`: va directo al índice FTS, que es lo que engorda el
    // fichero. Con el nivel gratuito no se recoge, así que no hay nada que indexar.
}

#[tokio::test]
async fn un_nivel_de_pago_si_indexa_el_texto() {
    let sitio = Sitio::nuevo("fts-pro", 3);

    let mut job = CrawlJob::filesystem(&sitio.path, "https://fixture.local/");
    job.tier = Tier::Pro;
    job.collect_body_text = true;
    let outcome = engine::run(job, &sitio.store()).await.expect("rastrear");

    let conn = abrir(&outcome.store_path);
    let indexadas: i64 =
        conn.query_row("SELECT COUNT(*) FROM pages_fts", [], |r| r.get(0)).expect("contar");
    assert_eq!(indexadas, 3, "en Pro se indexa lo que se pidió");
}
