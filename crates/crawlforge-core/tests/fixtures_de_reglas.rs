//! Cada fixture del catálogo se rastrea de verdad y tiene que producir su hallazgo.
//!
//! Es el test que convierte «cada regla necesita un fixture» en algo comprobable. Un test
//! unitario de una regla demuestra que su lógica es correcta *dado un `PageContext`*; esto
//! demuestra que el motor construye ese contexto con los datos que la regla espera. Los fallos
//! del pipeline estaban todos en esa costura, no dentro de los módulos.
//!
//! No se exige que un fixture dispare **solo** su regla: una página sin título tampoco tiene 300
//! palabras, así que también es contenido escaso. Lo que se exige es que dispare la suya.

use crawlforge_core::{engine, job::CrawlJob};
use rusqlite::Connection;

/// Marcador de relleno. Un fixture que escriba `<!--RELLENO:600000-->` se rastrea con 600.000
/// bytes en su lugar.
///
/// Dos reglas dependen del tamaño real de un fichero —`HTTP-LARGE-PAGE` con 500 KB de HTML y
/// `ASSET-IMG-HEAVY` con 200 KB de imagen— y sin esto el repositorio cargaría para siempre con
/// tres cuartos de megabyte de bytes de relleno versionados. El fixture guarda el marcador, que
/// además dice cuánto pesa y por qué; el peso se materializa al rastrearlo.
const MARCADOR: &str = "<!--RELLENO:";

/// Sustituye los marcadores de relleno por bytes de verdad.
fn expandir_relleno(contenido: &str) -> String {
    let mut salida = String::with_capacity(contenido.len());
    let mut resto = contenido;
    while let Some(inicio) = resto.find(MARCADOR) {
        salida.push_str(&resto[..inicio]);
        let tras_marcador = &resto[inicio + MARCADOR.len()..];
        let Some(fin) = tras_marcador.find("-->") else {
            // Marcador sin cerrar: se deja tal cual y se sigue. Un fixture mal escrito tiene que
            // fallar en su assert, no aquí.
            salida.push_str(&resto[inicio..]);
            return salida;
        };
        let bytes: usize = tras_marcador[..fin].trim().parse().unwrap_or(0);
        salida.push_str(&"x".repeat(bytes));
        resto = &tras_marcador[fin + 3..];
    }
    salida.push_str(resto);
    salida
}

/// Copia recursiva, expandiendo los marcadores de relleno. Los fixtures se rastrean sobre una
/// copia temporal para no escribir el fichero `.sqlite` dentro del repositorio.
fn copiar(origen: &std::path::Path, destino: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destino)?;
    for entrada in std::fs::read_dir(origen)? {
        let entrada = entrada?;
        let hacia = destino.join(entrada.file_name());
        if entrada.file_type()?.is_dir() {
            copiar(&entrada.path(), &hacia)?;
        } else {
            copiar_fichero(&entrada.path(), &hacia)?;
        }
    }
    Ok(())
}

fn copiar_fichero(origen: &std::path::Path, destino: &std::path::Path) -> std::io::Result<()> {
    let bytes = std::fs::read(origen)?;
    match std::str::from_utf8(&bytes) {
        Ok(texto) if texto.contains(MARCADOR) => {
            std::fs::write(destino, expandir_relleno(texto))
        }
        // Lo que no es texto, o no lleva marcador, se copia tal cual.
        _ => std::fs::write(destino, bytes),
    }
}

struct Temporal {
    path: std::path::PathBuf,
}

impl Temporal {
    fn new(nombre: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("crawlforge-fx-{}-{}", std::process::id(), nombre.replace('/', "_")));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("crear temporal");
        Self { path }
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Rastrea el fixture de una regla y devuelve los IDs de los hallazgos encontrados.
async fn hallazgos_de(rule_id: &str) -> Vec<String> {
    let fixture = crawlforge_rules::fixture_path(rule_id)
        .unwrap_or_else(|| panic!("{rule_id} no tiene fixture"));

    let tmp = Temporal::new(rule_id);
    let sitio = tmp.path.join("sitio");
    if fixture.is_dir() {
        copiar(&fixture, &sitio).expect("copiar el fixture");
    } else {
        // Un fixture de un solo fichero se publica como la portada, para que su URL sea `/` y
        // un canonical a `/` sea correcto.
        std::fs::create_dir_all(&sitio).expect("crear sitio");
        copiar_fichero(&fixture, &sitio.join("index.html")).expect("copiar el fixture");
    }

    let job = CrawlJob::filesystem(&sitio, "https://fixture.local/");
    let outcome = engine::run(job, &tmp.path.join("crawl.sqlite")).await.expect("rastrear");

    let conn = Connection::open_with_flags(
        &outcome.store_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el rastreo");

    let mut stmt = conn.prepare("SELECT DISTINCT rule_id FROM issues").expect("consultar issues");
    let ids = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .expect("leer issues")
        .filter_map(Result::ok)
        .collect();
    ids
}

/// Reglas que un árbol de ficheros no puede provocar **y que se demuestran rastreando el
/// servidor local de pruebas** (`reglas_http.rs`), con el nombre del test que lo hace.
///
/// El `FilesystemFetcher` solo devuelve 200 o 404, no emite 3xx ni 5xx y no mide latencia
/// (`is_network()` es `false`, así que `ttfb_ms` llega vacío a propósito). Estas siete reglas
/// siguen sin disparar aquí, y eso está bien: su demostración de extremo a extremo vive en el otro
/// fichero, contra un servidor de verdad. Lo que **no** es aceptable es que el puntero se pudra,
/// así que `las_reglas_del_servidor_apuntan_a_un_test_que_existe` comprueba que cada test nombrado
/// sigue estando.
const DEMOSTRADAS_CONTRA_EL_SERVIDOR: &[(&str, &str)] = &[
    ("HTTP-5XX", "un_5xx_se_reporta_en_la_url_que_falla"),
    ("HTTP-SLOW-RESPONSE", "una_respuesta_lenta_se_reporta_con_su_ttfb"),
    ("HTTP-REDIRECT-CHAIN", "una_cadena_de_dos_saltos_se_reporta_en_su_cabeza"),
    ("HTTP-REDIRECT-LOOP", "un_bucle_de_redireccion_se_reporta_una_sola_vez"),
    ("HTTP-REDIRECT-TO-404", "una_redireccion_que_acaba_en_404_se_reporta_en_la_cabeza"),
    ("HTTP-NO-HTTPS", "un_sitio_servido_por_http_lo_reporta_una_sola_vez"),
    ("CANON-TO-REDIRECT", "un_canonical_que_apunta_a_una_redireccion_se_reporta"),
    (
        "HTTP-404-EXTERNAL",
        "con_follow_external_el_404_ajeno_se_reporta_como_externo_y_no_como_propio",
    ),
];

/// Reglas que **ningún rastreo demuestra todavía**, con el motivo exacto.
///
/// No es una lista de excusas: es el inventario de lo que no se puede afirmar. Ya no habla de la
/// falta de servidor —el servidor existe— sino de huecos del motor, que es donde de verdad está
/// el problema.
const SIN_FIXTURE_EN_FILESYSTEM: &[(&str, &str)] = &[
    (
        "INDEX-ROBOTS-TXT-MISSING",
        "la regla se limita al modo http a propósito: en un dist/ el robots.txt lo sirve el \
         alojamiento y no el generador, así que su ausencia en el directorio no dice nada del \
         sitio publicado. Su fixture existe y sirve para el día que se sirva por HTTP",
    ),
    (
        "INDEX-SITEMAP-MISSING",
        "la regla se limita al modo http a propósito: en una auditoría de un dist/ el sitio aún \
         no está publicado, y avisar de que no hay sitemap en cada compilación sería ruido en el \
         pipeline de CI. Su fixture existe y sirve para el día que se sirva por HTTP",
    ),
];

#[tokio::test]
async fn cada_fixture_dispara_la_regla_que_documenta() {
    let mut fallos = Vec::new();
    let mut excepciones_que_ya_no_lo_son = Vec::new();

    for meta in crawlforge_rules::catalog() {
        let encontrados = hallazgos_de(meta.id).await;
        let disparo = encontrados.iter().any(|id| id == meta.id);
        let declarada = SIN_FIXTURE_EN_FILESYSTEM.iter().any(|(id, _)| *id == meta.id)
            || DEMOSTRADAS_CONTRA_EL_SERVIDOR.iter().any(|(id, _)| *id == meta.id);

        match (disparo, declarada) {
            (false, false) => fallos.push(format!(
                "{}: su fixture no produjo el hallazgo. Sí produjo: {:?}",
                meta.id, encontrados
            )),
            // Si una excepción empieza a funcionar, hay que quitarla de la lista: una lista de
            // huecos que ya no existen deja de ser un inventario y pasa a ser ruido.
            (true, true) => excepciones_que_ya_no_lo_son.push(meta.id),
            _ => {}
        }
    }

    assert!(fallos.is_empty(), "fixtures que no demuestran su regla:\n{}", fallos.join("\n"));
    assert!(
        excepciones_que_ya_no_lo_son.is_empty(),
        "estas reglas ya disparan en modo filesystem: quítalas de la lista que las declara: {:?}",
        excepciones_que_ya_no_lo_son
    );
}

/// Rastrea el fixture de una regla y devuelve `(detail_json, group_key)` de sus hallazgos.
///
/// Es la versión con detalle de [`hallazgos_de`], para las reglas cuyo contrato incluye qué
/// escriben en esas dos columnas y no solo que disparan.
async fn detalles_de(rule_id: &str) -> Vec<(Option<String>, Option<String>)> {
    let fixture = crawlforge_rules::fixture_path(rule_id)
        .unwrap_or_else(|| panic!("{rule_id} no tiene fixture"));

    let tmp = Temporal::new(&format!("detalle-{rule_id}"));
    let sitio = tmp.path.join("sitio");
    if fixture.is_dir() {
        copiar(&fixture, &sitio).expect("copiar el fixture");
    } else {
        std::fs::create_dir_all(&sitio).expect("crear sitio");
        copiar_fichero(&fixture, &sitio.join("index.html")).expect("copiar el fixture");
    }

    let job = CrawlJob::filesystem(&sitio, "https://fixture.local/");
    let outcome = engine::run(job, &tmp.path.join("crawl.sqlite")).await.expect("rastrear");

    let conn = Connection::open_with_flags(
        &outcome.store_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("abrir el rastreo");
    let mut stmt = conn
        .prepare("SELECT detail_json, group_key FROM issues WHERE rule_id = ?1")
        .expect("consultar issues");
    let filas = stmt
        .query_map([rule_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("leer issues")
        .filter_map(Result::ok)
        .collect();
    filas
}

#[tokio::test]
async fn el_salto_de_encabezado_llega_al_almacen_con_su_texto_y_su_grupo() {
    // La costura que este test vigila: `parse.rs` extrae el texto de los encabezados y
    // `engine.rs` lo pasa a las reglas por `PageContext::heading_texts`. El texto es lo que
    // permitió diagnosticar a mano el `<h5>CONTACTO` de un rastreo real; sin este test, la
    // regla podría seguir disparando con el campo vacío y nadie lo vería en los unitarios.
    let filas = detalles_de("CONTENT-HEADING-SKIP").await;
    assert!(!filas.is_empty(), "el fixture tiene que disparar la regla");
    let (detalle, grupo) = &filas[0];
    let detalle = detalle.as_deref().unwrap_or_default();
    assert!(
        detalle.contains("\"text\":\"Cuánto tiempo de riego\""),
        "el detalle debe llevar el texto del encabezado culpable: {detalle}"
    );
    assert_eq!(
        grupo.as_deref(),
        Some("heading-skip:2>4:cuánto tiempo de riego"),
        "la clave de grupo es la forma del salto más el texto normalizado"
    );
}

#[tokio::test]
async fn las_reglas_de_plantilla_escriben_su_clave_de_grupo() {
    // `group_key` existió vacío mucho tiempo —la cuarta estructura del proyecto que
    // existía y mentía—. Esto fija que las dos reglas más ruidosas de los rastreos reales lo
    // escriben de verdad al pasar por el motor, no solo en sus tests unitarios.
    for (regla, prefijo) in
        [("INDEX-NOFOLLOW-INTERNAL", "nofollow:"), ("ASSET-IMG-EMPTY-ALT-LINK", "img-empty-alt:")]
    {
        let filas = detalles_de(regla).await;
        assert!(!filas.is_empty(), "el fixture de {regla} tiene que disparar la regla");
        for (_, grupo) in &filas {
            assert!(
                grupo.as_deref().is_some_and(|g| g.starts_with(prefijo)),
                "{regla} debe escribir un group_key con prefijo {prefijo}: {grupo:?}"
            );
        }
    }
}

#[test]
fn las_excepciones_declaradas_existen_y_estan_justificadas() {
    let ids: Vec<&str> = crawlforge_rules::catalog().iter().map(|m| m.id).collect();
    for (id, motivo) in SIN_FIXTURE_EN_FILESYSTEM {
        assert!(ids.contains(id), "{id} no está en el catálogo");
        assert!(motivo.len() > 30, "el motivo de {id} no explica nada: {motivo:?}");
    }
    for (id, _) in DEMOSTRADAS_CONTRA_EL_SERVIDOR {
        assert!(ids.contains(id), "{id} no está en el catálogo");
    }
}

#[test]
fn las_reglas_del_servidor_apuntan_a_un_test_que_existe() {
    // Un puntero a un test que ya no existe es peor que no tener puntero: convence de que algo
    // está demostrado cuando no lo está. Se lee el fichero de al lado y se busca la función.
    let fuente = include_str!("reglas_http.rs");
    for (id, test) in DEMOSTRADAS_CONTRA_EL_SERVIDOR {
        assert!(
            fuente.contains(&format!("async fn {test}()")),
            "{id} dice demostrarse en reglas_http.rs::{test}, y ese test no está"
        );
    }
}
