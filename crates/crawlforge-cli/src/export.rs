//! Export a CSV.
//!
//! Un fichero por tabla lógica, con las columnas que un SEO usa de verdad. El XLSX está en
//! `src/xlsx.rs`; Parquet sigue pendiente.

// El XLSX vive en su propio fichero y se declara desde `src/lib.rs`, no desde aquí ni desde
// `main.rs`: son 700 líneas de formato y una docena de consultas distintas de las de este
// módulo. Al cablear el subcomando, la línea de `main.rs` es `use crawlforge_cli::xlsx;`
// (ver el porqué en `lib.rs`), y la función es `xlsx::to_xlsx(&store, &out)`.

use anyhow::{bail, Result};
use crawlforge_cli::store_check;
use rusqlite::Connection;
use std::path::Path;

/// Consultas de exportación: (nombre de fichero, SQL).
///
/// Las cabeceras salen de los alias del `SELECT`, así que están en inglés como el resto de
/// identificadores (`CONVENTIONS.md §4`) y son estables para quien automatice sobre ellas.
const EXPORTS: &[(&str, &str)] = &[
    (
        "urls",
        "SELECT u.url, u.status_code, u.content_type, u.depth, u.is_internal, u.in_sitemap,
                u.crawl_state, u.exclusion_reason, u.content_length, u.response_time_ms,
                u.error_kind, r.url AS redirect_to
         FROM urls u LEFT JOIN urls r ON r.id = u.redirect_to
         ORDER BY u.id",
    ),
    (
        "pages",
        "SELECT u.url, p.title, p.title_len, p.title_px, p.meta_description, p.meta_desc_len,
                p.h1, p.h1_count, p.h2_count, p.canonical, p.canonical_is_self, p.meta_robots,
                p.is_indexable, p.indexability_reason, p.lang, p.word_count, p.content_ratio,
                p.internal_links_in, p.internal_links_out, p.schema_types, p.crawl_depth_source
         FROM pages p JOIN urls u ON u.id = p.url_id
         ORDER BY p.url_id",
    ),
    (
        "issues",
        "SELECT i.rule_id, i.severity, i.category, u.url, i.detail_json, i.group_key
         FROM issues i LEFT JOIN urls u ON u.id = i.url_id
         ORDER BY CASE i.severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1
                                  WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END,
                  i.rule_id",
    ),
    (
        "links",
        "SELECT f.url AS from_url, t.url AS to_url, l.anchor, l.rel, l.is_nofollow,
                l.element, l.region, l.position, t.status_code AS to_status
         FROM links l JOIN urls f ON f.id = l.from_url_id JOIN urls t ON t.id = l.to_url_id
         ORDER BY l.from_url_id, l.position",
    ),
    (
        "images",
        "SELECT p.url AS page_url, s.url AS image_url, i.alt, i.alt_present, i.width_attr,
                i.height_attr, i.loading, i.in_srcset, i.format
         FROM images i JOIN urls p ON p.id = i.page_url_id JOIN urls s ON s.id = i.src_url_id
         ORDER BY i.page_url_id",
    ),
    (
        // Una fila por URL de recurso, no por par (página, recurso): esa arista solo existe
        // para las imágenes (`docs/02-MODELO-DATOS.md §3.5`). Lo pesado arriba, que es a lo
        // que se viene; los de tamaño desconocido caen al final, porque NULL ordena último
        // en DESC.
        "resources",
        "SELECT u.url AS resource_url, r.kind, r.status_code, r.size_bytes, r.mime
         FROM resources r JOIN urls u ON u.id = r.url_id
         ORDER BY r.size_bytes DESC, u.url",
    ),
    (
        "broken_links",
        "SELECT from_url, to_url, status_code, anchor FROM v_broken_links ORDER BY from_url",
    ),
];

/// Exporta un fichero de rastreo a CSV. Devuelve cuántos ficheros escribió.
pub fn to_csv(store: &Path, out_dir: &Path) -> Result<usize> {
    // Se comprueba todo antes de escribir nada: un error a medio export deja ficheros a medias
    // que luego alguien abre creyendo que están completos.
    store_check::ensure_crawl_store(store)?;
    ensure_csv_out_dir(out_dir)?;
    std::fs::create_dir_all(out_dir)?;

    let conn = Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;

    let mut written = 0;
    for (name, sql) in EXPORTS {
        let path = out_dir.join(format!("{name}.csv"));
        export_query(&conn, sql, &path)?;
        written += 1;
    }
    Ok(written)
}

/// Rechaza un destino de CSV que no puede ser un directorio, **antes** de crear nada.
///
/// El destino cambia de significado entre formatos —directorio para `csv`, fichero para
/// `xlsx`— y cruzarlos daba un «File exists (os error 17)» de `create_dir_all`, o peor: con un
/// nombre como `miweb.xlsx` que aún no existía, la herramienta creaba un *directorio* llamado
/// así y metía los CSV dentro. El error tiene que decir qué se espera y qué hacer, no un errno.
fn ensure_csv_out_dir(out_dir: &Path) -> Result<()> {
    if out_dir.is_file() {
        bail!(
            "for csv, --out is a directory where {} files are written, one per table \
             (urls.csv, pages.csv, issues.csv…), and {} already exists and is a file.\n\
             Give a directory (--out ./export) or, for a single spreadsheet file, \
             use --format xlsx.",
            EXPORTS.len(),
            out_dir.display()
        );
    }
    if !out_dir.exists() && looks_like_file_name(out_dir) {
        bail!(
            "for csv, --out is a directory where {} files are written, and '{}' looks like a \
             file name.\n\
             Give a directory (--out ./export) or, if you wanted a single .xlsx file, \
             use --format xlsx.",
            EXPORTS.len(),
            out_dir.display()
        );
    }
    Ok(())
}

/// Si la ruta lleva una extensión de fichero de datos, quien la escribió pensaba en un fichero.
///
/// La lista es corta a propósito: un directorio legítimo puede llamarse `v1.2` y no hay que
/// molestarlo. Solo se frenan las extensiones con las que trabaja esta herramienta.
fn looks_like_file_name(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("csv" | "xlsx" | "xls" | "sqlite" | "parquet")
    )
}

/// Vuelca una consulta a un CSV, con las cabeceras que declare el `SELECT`.
fn export_query(conn: &Connection, sql: &str, path: &Path) -> Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();

    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record(&columns)?;

    let count = columns.len();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let mut record = Vec::with_capacity(count);
        for i in 0..count {
            record.push(value_to_string(row.get_ref(i)?));
        }
        writer.write_record(&record)?;
    }
    writer.flush()?;
    Ok(())
}

/// Un `NULL` se exporta como celda vacía, no como el literal «NULL»: una hoja de cálculo
/// trataría esa cadena como un valor.
fn value_to_string(value: rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => format!("{f}"),
        ValueRef::Text(t) => neutralize_formula(&String::from_utf8_lossy(t)),
        ValueRef::Blob(_) => "<blob>".to_string(),
    }
}

/// Caracteres con los que una celda de texto se convierte en fórmula al abrir el CSV.
///
/// El tabulador y el retorno de carro están porque algunas versiones de Excel los saltan antes de
/// decidir si la celda empieza por `=`.
const FORMULA_TRIGGERS: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

/// Neutraliza una celda que una hoja de cálculo interpretaría como fórmula.
///
/// El contenido de estas columnas —títulos, meta descripciones, textos de enlace— lo escribe el
/// sitio rastreado, que es un tercero. Un `<title>` que empiece por `=` acaba evaluado al abrir el
/// CSV en Excel o LibreOffice. Comprobado con un sitio de prueba: un título
/// `=HYPERLINK("http://…","Pincha aqui")` sale literal en `pages.csv` y se renderiza como enlace
/// dentro del informe que el usuario reenvía a su cliente, con el nombre de CrawlForge encima.
///
/// El citado del crate `csv` no protege de esto: escapar comillas y saltos de línea es una cosa y
/// neutralizar fórmulas es otra.
///
/// **Solo hace falta en CSV.** El XLSX escribe cadenas tipadas (`t="inlineStr"`), que Excel nunca
/// evalúa: prefijar allí ensuciaría el entregable sin ganar nada.
fn neutralize_formula(valor: &str) -> String {
    match valor.chars().next() {
        Some(c) if FORMULA_TRIGGERS.contains(&c) => {
            // La comilla simple es la convención que entienden Excel, LibreOffice y Sheets: marca
            // la celda como texto y no se ve al abrirla.
            format!("'{valor}")
        }
        _ => valor.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_celda_que_empieza_por_igual_deja_de_ser_formula() {
        // El caso demostrado en la revisión de seguridad: un título del sitio rastreado.
        assert_eq!(
            neutralize_formula("=HYPERLINK(\"http://evil.example\",\"Pincha aqui\")"),
            "\'=HYPERLINK(\"http://evil.example\",\"Pincha aqui\")"
        );
        assert_eq!(neutralize_formula("=cmd|\'/c calc.exe\'!A1"), "\'=cmd|\'/c calc.exe\'!A1");
    }

    #[test]
    fn los_cuatro_disparadores_se_neutralizan() {
        for prefijo in ["=", "+", "-", "@"] {
            let valor = format!("{prefijo}SUM(1+1)");
            assert!(
                neutralize_formula(&valor).starts_with('\''),
                "{prefijo} must be neutralized"
            );
        }
    }

    #[test]
    fn un_texto_normal_no_se_toca() {
        // Neutralizar de más estropea el entregable: la mayoría de los títulos son texto normal.
        for valor in ["Página de inicio", "Título con = en medio", "10 consejos", "", "ñandú"] {
            assert_eq!(neutralize_formula(valor), valor);
        }
    }

    #[test]
    fn un_guion_inicial_se_neutraliza_aunque_parezca_inofensivo() {
        // «- Nuestros servicios» es un título plausible, y Excel lo convierte en número o en
        // error. Aquí neutralizar además evita corromper el dato.
        assert_eq!(neutralize_formula("- Nuestros servicios"), "\'- Nuestros servicios");
    }

    #[test]
    fn un_null_se_exporta_como_celda_vacia() {
        use rusqlite::types::ValueRef;
        assert_eq!(value_to_string(ValueRef::Null), "");
        assert_eq!(value_to_string(ValueRef::Integer(42)), "42");
        assert_eq!(value_to_string(ValueRef::Text(b"hola")), "hola");
    }

    #[test]
    fn hay_una_exportacion_por_cada_tabla_que_usa_un_seo() {
        let names: Vec<_> = EXPORTS.iter().map(|(n, _)| *n).collect();
        for expected in ["urls", "pages", "issues", "links", "images", "resources", "broken_links"] {
            assert!(names.contains(&expected), "the {expected} export is missing");
        }
    }

    /// Directorio temporal propio; la CLI no depende de `tempfile` (stack cerrado, `CONVENTIONS.md §3`).
    fn tmpdir(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("crawlforge-export-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn un_out_que_ya_es_fichero_se_rechaza_antes_de_escribir_nada() {
        // El cruce de la revisión de UX: `--format csv --out miweb.xlsx` daba
        // «File exists (os error 17)» porque se intentaba crear un directorio con ese nombre.
        let dir = tmpdir("out-fichero");
        let fichero = dir.join("miweb.xlsx");
        std::fs::write(&fichero, "ya existo").expect("create the file");

        let err = ensure_csv_out_dir(&fichero).expect_err("a file is not a directory");
        let msg = format!("{err:#}");
        assert!(msg.contains("--out is a directory"), "says what is expected: {msg}");
        assert!(msg.contains("already exists and is a file"), "and what is wrong: {msg}");
        assert!(msg.contains("--format xlsx"), "and the likely fix: {msg}");
        assert!(!msg.contains("os error"), "no errno: {msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_out_con_nombre_de_fichero_no_se_convierte_en_directorio_en_silencio() {
        let dir = tmpdir("out-nombre");
        let futuro = dir.join("informe.xlsx"); // no existe todavía

        let err = ensure_csv_out_dir(&futuro).expect_err("looks like a file");
        assert!(format!("{err:#}").contains("looks like a file name"), "{err:#}");
        assert!(!futuro.exists(), "the check creates nothing");

        // Un directorio normal, exista o no, pasa sin ruido.
        ensure_csv_out_dir(&dir).expect("an existing directory is fine");
        ensure_csv_out_dir(&dir.join("sub")).expect("and one yet to be created too");
        assert!(!dir.join("sub").exists(), "checking is not creating");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_directorio_con_punto_en_el_nombre_no_se_confunde_con_un_fichero() {
        // `v1.2` es un nombre de directorio legítimo: solo se frenan las extensiones de datos.
        assert!(!looks_like_file_name(Path::new("./v1.2")));
        assert!(looks_like_file_name(Path::new("miweb.xlsx")));
        assert!(looks_like_file_name(Path::new("datos.CSV")), "case-insensitive");
    }

    #[test]
    fn exportar_un_fichero_que_no_es_un_rastreo_falla_sin_jerga_y_sin_escribir() {
        let dir = tmpdir("no-rastreo");
        let ajeno = dir.join("ajeno.sqlite");
        {
            let conn = Connection::open(&ajeno).expect("create");
            conn.execute_batch("CREATE TABLE cosas (id INTEGER);").expect("foreign table");
        }
        let salida = dir.join("csv");

        let err = to_csv(&ajeno, &salida).expect_err("not a crawl");
        let msg = format!("{err:#}");
        assert!(msg.contains("is not a CrawlForge crawl file"), "{msg}");
        assert!(!msg.contains("no such table"), "no SQLite jargon: {msg}");
        assert!(!salida.exists(), "the check runs before creating the output directory");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
