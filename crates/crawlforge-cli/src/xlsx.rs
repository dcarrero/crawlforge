//! Export a XLSX.
//!
//! Un solo fichero con una hoja por vista, en el orden en que se miran: primero los hallazgos,
//! después lo que hay que arreglar, y al final los datos en crudo. La calidad del fichero **es**
//! la funcionalidad: un SEO no vive dentro de la herramienta, exporta y trabaja en la hoja.
//!
//! Decisiones que condicionan todo el módulo:
//!
//! - **Los números se escriben como números.** Un `status_code` guardado como texto no se puede
//!   filtrar por «mayor que», y es exactamente el fallo que obliga a rehacer la hoja a mano.
//! - **Sin formato de millares.** Ningún número de aquí se lee mejor agrupado y en `status_code`
//!   sería directamente erróneo («2 000»). Todo en formato General.
//! - **Sin hiperenlaces.** Excel admite ~65.530 por hoja y cada uno engorda el fichero; un
//!   rastreo medio los desborda y el que sobra rompe el fichero. La URL en texto se pega igual.
//! - **Las cabeceras van en inglés**, como en el CSV (`CONVENTIONS.md §4`), y son estables para quien
//!   automatice sobre ellas.
//! - **Todas las hojas existen siempre**, aunque queden vacías: la estructura del fichero no
//!   depende del rastreo, y la hoja `Summary` dice cuántas filas trae cada una.
//!
//! ## Memoria en rastreos grandes
//!
//! Las hojas de datos se escriben con `Workbook::add_worksheet_with_constant_memory()`, que vuelca
//! cada fila a un temporal en vez de acumular el libro entero en RAM. No es una optimización
//! opcional: sin ella, exportar `fixtures/crawl-500k.sqlite` (500.000 URLs, 1,5 millones de
//! enlaces) costaba **5,2 GB de RSS**, que es exactamente el fallo de Screaming Frog que este
//! producto ataca (`CONVENTIONS.md §5.2`), trasladado del rastreo al export.
//!
//! Medido sobre ese mismo fichero, con la feature activada:
//!
//! | | Antes | Ahora |
//! |---|---|---|
//! | RSS pico | 5,2 GB | **1,13 GB** |
//! | Tiempo | 23,8 s | 24,0 s |
//! | Salida | 98,7 MB | 100,7 MB, 13 hojas |
//!
//! El gigabyte que queda no son las celdas: es el propio empaquetado del `.xlsx`, que comprime
//! sobre un centenar de megabytes de XML. Se puede seguir bajando, pero conviene recordar el
//! contexto: un libro con 1,5 millones de filas es un fichero que Excel abre con dificultad, y un
//! SEO exporta subconjuntos. La `Summary` es la única hoja que no usa memoria constante, porque
//! se rellena al final con los recuentos de las demás; son decenas de filas.

use anyhow::{Context, Result};
use rusqlite::Connection;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet};
use std::path::Path;

/// Límite de Excel para una celda de texto. Pasarse es un error duro de `rust_xlsxwriter`, así
/// que se recorta antes con una marca visible: el fichero vale más completo que exacto.
const MAX_CELL_CHARS: usize = 32_767;

/// Marca de recorte. En inglés, como las cabeceras.
const TRUNCATION_MARK: &str = " […truncated for Excel]";

/// Filas de datos por hoja. El límite de Excel es 1.048.576 **incluida la cabecera**.
const MAX_DATA_ROWS: usize = 1_048_575;

/// Ancho mínimo de columna, en caracteres. Por debajo la cabecera no se lee.
const MIN_COL_WIDTH: f64 = 9.0;

/// Ancho máximo. Una columna de URLs mide 300 caracteres y dejaría la hoja inservible; a 60 se
/// ve el dominio y buena parte de la ruta, que es lo que se necesita para orientarse.
const MAX_COL_WIDTH: f64 = 60.0;

/// Orden de severidad para ordenar en SQL. Se repite en varias consultas.
const SEVERITY_ORDER: &str = "CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1
                                            WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END";

/// Una hoja de datos: nombre, consulta y, si la tiene, la columna de severidad a colorear.
struct SheetSpec {
    /// Nombre de la pestaña. Máximo 31 caracteres y sin `[]:*?/\`.
    name: &'static str,
    sql: &'static str,
    /// Índice de la columna `severity`, para pintar la celda según su gravedad.
    severity_col: Option<usize>,
    /// Tabla que la consulta necesita y que puede no existir en un fichero de rastreo antiguo.
    /// Si falta, la hoja se queda con la cabecera en vez de reventar la exportación.
    requires_table: Option<&'static str>,
}

/// Las hojas, en el orden en que se abren.
///
/// Primero «¿qué está mal?», después «¿qué arreglo?», y los volcados al final. La primera hoja
/// que ve quien abre el fichero no puede ser un listado de 50.000 URLs.
fn sheets() -> Vec<SheetSpec> {
    vec![
        // ------------------------------------------------------ Hallazgos
        // Lo que se pega en un correo: una fila por regla, los críticos arriba.
        SheetSpec {
            name: "Issues by rule",
            sql: "SELECT severity, rule_id, category, n AS count
                  FROM v_issue_summary
                  ORDER BY {SEV}, n DESC, rule_id",
            severity_col: Some(0),
            requires_table: None,
        },
        // La severidad va primera a propósito: es la columna por la que se filtra antes que nada.
        SheetSpec {
            name: "Issues",
            sql: "SELECT i.severity, i.rule_id, i.category, u.url, i.detail_json, i.group_key
                  FROM issues i LEFT JOIN urls u ON u.id = i.url_id
                  ORDER BY {SEV}, i.rule_id, u.url",
            severity_col: Some(0),
            requires_table: None,
        },
        // ------------------------------------------------------ Lo que hay que arreglar
        // Ordenado por destino roto, no por origen: se arregla una vez el destino y se sabe de
        // golpe cuántas páginas hay que tocar. `element` distingue un <a> de un <img>.
        SheetSpec {
            name: "Broken links",
            sql: "SELECT ut.status_code AS to_status, ut.url AS to_url, uf.url AS from_url,
                         l.anchor, l.element, l.region
                  FROM links l
                  JOIN urls uf ON uf.id = l.from_url_id
                  JOIN urls ut ON ut.id = l.to_url_id
                  WHERE ut.status_code >= 400
                  ORDER BY ut.status_code DESC, ut.url, uf.url",
            severity_col: None,
            requires_table: None,
        },
        // `inbound_links` es el número accionable: cuántos enlaces internos hay que reescribir
        // para dejar de pasar por el redirect.
        SheetSpec {
            name: "Redirects",
            sql: "SELECT u.status_code, u.url, r.url AS redirect_to, u.redirect_chain_len,
                         (SELECT COUNT(*) FROM links l WHERE l.to_url_id = u.id) AS inbound_links
                  FROM urls u LEFT JOIN urls r ON r.id = u.redirect_to
                  WHERE u.status_code >= 300 AND u.status_code < 400
                  ORDER BY u.status_code, u.url",
            severity_col: None,
            requires_table: None,
        },
        // «Por qué esto no sale en Google», que es la pregunta con la que se abre la herramienta.
        SheetSpec {
            name: "Non-indexable",
            sql: "SELECT p.indexability_reason, u.url, u.status_code, p.title, p.canonical,
                         p.meta_robots, u.depth, p.internal_links_in
                  FROM pages p JOIN urls u ON u.id = p.url_id
                  WHERE p.is_indexable = 0
                  ORDER BY p.indexability_reason, u.url",
            severity_col: None,
            requires_table: None,
        },
        // Huérfanas: en el sitemap y sin un solo enlace interno que las alcance.
        SheetSpec {
            name: "Orphans",
            sql: "SELECT o.url, u.status_code, p.title, p.word_count, u.depth
                  FROM v_orphans o
                  JOIN urls u ON u.id = o.id
                  LEFT JOIN pages p ON p.url_id = o.id
                  ORDER BY o.url",
            severity_col: None,
            requires_table: None,
        },
        // ------------------------------------------------------ Datos en crudo
        SheetSpec {
            name: "Pages",
            sql: "SELECT u.url, u.status_code, p.is_indexable, p.title, p.title_len, p.title_px,
                         p.meta_description, p.meta_desc_len, p.h1, p.h1_count, p.h2_count,
                         p.canonical, p.canonical_is_self, p.meta_robots, p.lang, p.word_count,
                         p.content_ratio, p.internal_links_in, p.internal_links_out, u.depth,
                         p.schema_types, p.crawl_depth_source
                  FROM pages p JOIN urls u ON u.id = p.url_id
                  ORDER BY u.url",
            severity_col: None,
            requires_table: None,
        },
        // `alt_present` ascendente deja arriba las imágenes sin alt, que es a lo que se viene.
        SheetSpec {
            name: "Images",
            sql: "SELECT i.alt_present, s.url AS image_url, i.alt, i.format, i.loading,
                         i.width_attr, i.height_attr, i.in_srcset, p.url AS page_url
                  FROM images i
                  JOIN urls p ON p.id = i.page_url_id
                  JOIN urls s ON s.id = i.src_url_id
                  ORDER BY i.alt_present, s.url",
            severity_col: None,
            requires_table: None,
        },
        // Orden de documento dentro de cada página: la posición del enlace es información.
        SheetSpec {
            name: "Links",
            sql: "SELECT f.url AS from_url, t.url AS to_url, t.status_code AS to_status,
                         l.anchor, l.rel, l.is_nofollow, l.element, l.region, l.position
                  FROM links l
                  JOIN urls f ON f.id = l.from_url_id
                  JOIN urls t ON t.id = l.to_url_id
                  ORDER BY l.from_url_id, l.position",
            severity_col: None,
            requires_table: None,
        },
        SheetSpec {
            name: "URLs",
            sql: "SELECT u.url, u.status_code, u.content_type, u.depth, u.is_internal,
                         u.in_sitemap, u.crawl_state, u.exclusion_reason, u.content_length,
                         u.response_time_ms, u.error_kind, u.error_message, r.url AS redirect_to
                  FROM urls u LEFT JOIN urls r ON r.id = u.redirect_to
                  ORDER BY u.url",
            severity_col: None,
            requires_table: None,
        },
        // ------------------------------------------------------ Qué dijeron robots y sitemaps
        // Migración 004. Un rastreo de esquema anterior no tiene estas tablas: la hoja se queda
        // con la cabecera en vez de abortar el fichero entero.
        SheetSpec {
            name: "Robots",
            sql: "SELECT host, status_code, blocks_all, sitemap_count, fetched_at, content
                  FROM robots_txt ORDER BY host",
            severity_col: None,
            requires_table: Some("robots_txt"),
        },
        SheetSpec {
            name: "Sitemaps",
            sql: "SELECT url, status_code, is_index, is_valid, url_count, bytes,
                         discovered_from, parse_error, fetched_at
                  FROM sitemaps ORDER BY url",
            severity_col: None,
            requires_table: Some("sitemaps"),
        },
    ]
}

/// Formatos reutilizados en todo el libro. `rust_xlsxwriter` deduplica los formatos iguales,
/// así que construirlos una vez es por claridad, no por tamaño del fichero.
struct Styles {
    header: Format,
    critical: Format,
    high: Format,
    medium: Format,
    low: Format,
    info: Format,
}

impl Styles {
    fn new() -> Self {
        let header = Format::new()
            .set_bold()
            .set_font_color(Color::White)
            .set_background_color(Color::RGB(0x30_5496))
            .set_border_bottom(FormatBorder::Thin)
            .set_align(FormatAlign::Left);

        Self {
            header,
            // Los tres primeros son la paleta clásica de Excel para «malo» y «regular»: se
            // reconocen sin leyenda.
            critical: Format::new()
                .set_font_color(Color::RGB(0x9C_00_06))
                .set_background_color(Color::RGB(0xFF_C7_CE))
                .set_bold(),
            high: Format::new()
                .set_font_color(Color::RGB(0x9C_57_00))
                .set_background_color(Color::RGB(0xFF_EB_9C)),
            medium: Format::new().set_font_color(Color::RGB(0x80_60_00)),
            low: Format::new().set_font_color(Color::RGB(0x59_59_59)),
            info: Format::new().set_font_color(Color::RGB(0x80_80_80)),
        }
    }

    fn for_severity(&self, severity: &str) -> Option<&Format> {
        match severity {
            "critical" => Some(&self.critical),
            "high" => Some(&self.high),
            "medium" => Some(&self.medium),
            "low" => Some(&self.low),
            "info" => Some(&self.info),
            _ => None,
        }
    }
}

/// Cuántas filas acabó teniendo cada hoja, para el índice de la hoja `Summary`.
struct SheetStats {
    name: &'static str,
    rows: usize,
    truncated: bool,
}

/// Exporta un fichero de rastreo a un único `.xlsx`.
///
/// `store` es el `.sqlite` del rastreo, que se abre en **solo lectura**: exportar nunca modifica
/// un rastreo. `out` es el fichero de destino; se sobrescribe si existe.
///
/// Devuelve cuántas hojas escribió (incluida `Summary`).
///
/// Toda hoja lleva la fila de cabecera congelada, autofiltro y anchos de columna calculados a
/// partir del contenido. Las celdas que superan el límite de Excel se recortan con una marca
/// visible en vez de hacer fallar la exportación.
pub fn to_xlsx(store: &Path, out: &Path) -> Result<usize> {
    // Se comprueba todo antes de trabajar: el libro tarda segundos en escribirse y fallar al
    // guardarlo, al final, con un «Is a directory (os error 21)», era tirar ese trabajo y no
    // decir qué se esperaba.
    crate::store_check::ensure_crawl_store(store)?;
    ensure_xlsx_out_file(out)?;

    let conn = Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open the crawl file {}", store.display()))?;

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let styles = Styles::new();
    let mut workbook = Workbook::new();

    // `Summary` se crea primero para que quede como primera pestaña, pero se rellena al final:
    // su índice de hojas necesita los recuentos que solo se conocen tras escribirlas.
    workbook.add_worksheet().set_name("Summary")?;

    let specs = sheets();
    let mut stats = Vec::with_capacity(specs.len());

    for spec in &specs {
        let available = match spec.requires_table {
            Some(table) => table_exists(&conn, table)?,
            None => true,
        };
        // Memoria constante: la hoja se vuelca a un temporal fila a fila y no se queda en RAM.
        // Es lo que hace que exportar 500.000 URLs no cueste gigabytes, y su única condición
        // —escribir en orden de fila y no volver atrás— ya se cumplía aquí.
        let worksheet = workbook.add_worksheet_with_constant_memory();
        worksheet.set_name(spec.name)?;
        let (rows, truncated) = if available {
            write_query(worksheet, &conn, spec, &styles)
                .with_context(|| format!("write the {} sheet", spec.name))?
        } else {
            write_missing_table(worksheet, spec, &styles)?;
            (0, false)
        };
        stats.push(SheetStats { name: spec.name, rows, truncated });
    }

    let summary = workbook.worksheet_from_index(0)?;
    write_summary(summary, &conn, &stats, &styles).context("write the Summary sheet")?;

    workbook.save(out).with_context(|| format!("save {}", out.display()))?;
    Ok(specs.len() + 1)
}

/// Rechaza un destino de XLSX que no puede ser un fichero, **antes** de generar el libro.
///
/// El destino cambia de significado entre formatos —fichero para `xlsx`, directorio para
/// `csv`— y cruzarlos daba «Is a directory (os error 21)» al final del export. El error tiene
/// que decir qué se espera y proponer la orden correcta.
fn ensure_xlsx_out_file(out: &Path) -> anyhow::Result<()> {
    if out.is_dir() {
        anyhow::bail!(
            "for xlsx, --out is a file, not a directory: --out audit.xlsx.\n\
             {} is a directory; if you want the workbook inside it, name the file: --out {}\n\
             (the one that writes several files into a directory is --format csv)",
            out.display(),
            out.join("audit.xlsx").display()
        );
    }
    if out.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("csv")) {
        anyhow::bail!(
            "you asked for --format xlsx but --out ends in .csv.\n\
             For a spreadsheet workbook: --out audit.xlsx. For CSV files use \
             --format csv with a directory: --out ./export",
        );
    }
    Ok(())
}

/// Vuelca una consulta a una hoja. Devuelve `(filas escritas, si se truncó por el límite)`.
fn write_query(
    worksheet: &mut Worksheet,
    conn: &Connection,
    spec: &SheetSpec,
    styles: &Styles,
) -> Result<(usize, bool)> {
    let sql = spec.sql.replace("{SEV}", SEVERITY_ORDER);
    let mut stmt = conn.prepare(&sql)?;
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();

    let mut widths = write_header(worksheet, &columns, styles)?;

    let mut rows_written = 0usize;
    let mut truncated = false;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        if rows_written >= MAX_DATA_ROWS {
            truncated = true;
            break;
        }
        // +1 por la cabecera. `MAX_DATA_ROWS` garantiza que cabe en el límite de Excel.
        let excel_row = (rows_written + 1) as u32;
        for (col, width) in widths.iter_mut().enumerate() {
            let value = row.get_ref(col)?;
            let severity_fmt = match spec.severity_col {
                Some(sev) if sev == col => severity_format(value, styles),
                _ => None,
            };
            let used = write_cell(worksheet, excel_row, col as u16, value, severity_fmt)?;
            *width = (*width).max(used);
        }
        rows_written += 1;
    }

    finish_sheet(worksheet, &widths, rows_written)?;
    Ok((rows_written, truncated))
}

/// Hoja de una tabla que este fichero de rastreo no tiene (esquema anterior a la migración 004).
/// Se deja la cabecera y una nota: una pestaña vacía sin explicación parece un fallo.
fn write_missing_table(worksheet: &mut Worksheet, spec: &SheetSpec, styles: &Styles) -> Result<()> {
    let note = format!(
        "Not available: this crawl file predates the `{}` table (schema migration 004).",
        spec.requires_table.unwrap_or("")
    );
    let mut widths = write_header(worksheet, &["note".to_string()], styles)?;
    worksheet.write_string(1, 0, &note)?;
    widths[0] = widths[0].max(note.chars().count());
    finish_sheet(worksheet, &widths, 1)?;
    Ok(())
}

/// Escribe la fila de cabecera y devuelve el ancho inicial de cada columna.
fn write_header(worksheet: &mut Worksheet, columns: &[String], styles: &Styles) -> Result<Vec<usize>> {
    let mut widths = Vec::with_capacity(columns.len());
    for (col, name) in columns.iter().enumerate() {
        worksheet.write_string_with_format(0, col as u16, name, &styles.header)?;
        widths.push(name.chars().count());
    }
    Ok(widths)
}

/// Congela la cabecera, pone el autofiltro y ajusta los anchos. Sin esto una hoja de 50.000
/// filas es inservible: al desplazarte pierdes de vista qué columna estás mirando.
fn finish_sheet(worksheet: &mut Worksheet, widths: &[usize], rows_written: usize) -> Result<()> {
    for (col, width) in widths.iter().enumerate() {
        worksheet.set_column_width(col as u16, column_width(*width))?;
    }
    worksheet.set_freeze_panes(1, 0)?;
    if !widths.is_empty() {
        let last_col = (widths.len() - 1) as u16;
        // El autofiltro necesita al menos la fila de cabecera; con datos abarca todo el rango.
        let last_row = rows_written as u32;
        worksheet.autofilter(0, 0, last_row, last_col)?;
    }
    Ok(())
}

/// Traduce el contenido más largo de una columna a un ancho de Excel.
///
/// El `+2` compensa el desplegable del autofiltro, que si no tapa la cabecera.
fn column_width(max_chars: usize) -> f64 {
    ((max_chars as f64) + 2.0).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH)
}

/// Escribe una celda con el tipo que le corresponde. Devuelve cuántos caracteres ocupa, para
/// calcular el ancho de la columna.
fn write_cell(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: rusqlite::types::ValueRef<'_>,
    format: Option<&Format>,
) -> Result<usize> {
    use rusqlite::types::ValueRef;
    let text = match value {
        // Un NULL se deja en blanco, no como el literal «NULL»: una hoja de cálculo trataría esa
        // cadena como un valor y la contaría en los filtros.
        ValueRef::Null => return Ok(0),
        ValueRef::Integer(i) => {
            write_number(worksheet, row, col, i as f64, format)?;
            return Ok(i.to_string().chars().count());
        }
        ValueRef::Real(f) => {
            write_number(worksheet, row, col, f, format)?;
            return Ok(format!("{f}").chars().count());
        }
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(_) => "<blob>".to_string(),
    };

    let text = clamp_cell(&text);
    let used = text.chars().count();
    match format {
        Some(fmt) => worksheet.write_string_with_format(row, col, &text, fmt)?,
        None => worksheet.write_string(row, col, &text)?,
    };
    Ok(used)
}

fn write_number(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: f64,
    format: Option<&Format>,
) -> Result<()> {
    match format {
        Some(fmt) => worksheet.write_number_with_format(row, col, value, fmt)?,
        None => worksheet.write_number(row, col, value)?,
    };
    Ok(())
}

/// Recorta una celda al límite de Excel dejando una marca visible.
///
/// Cuenta **caracteres**, no bytes: el límite de Excel es de caracteres y un título con acentos
/// o emojis ocupa más bytes que caracteres. Cortar por bytes partiría un carácter por la mitad.
fn clamp_cell(text: &str) -> String {
    if text.chars().count() <= MAX_CELL_CHARS {
        return text.to_string();
    }
    let keep = MAX_CELL_CHARS - TRUNCATION_MARK.chars().count();
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(TRUNCATION_MARK);
    out
}

/// Formato de la celda de severidad, si el valor es una severidad conocida.
fn severity_format<'a>(
    value: rusqlite::types::ValueRef<'_>,
    styles: &'a Styles,
) -> Option<&'a Format> {
    match value {
        rusqlite::types::ValueRef::Text(t) => {
            styles.for_severity(&String::from_utf8_lossy(t))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------- Hoja de resumen

/// La hoja que se pega en un correo. Tabla plana `Section | Metric | Value` a propósito: un
/// resumen con bloques de distinta forma no se puede filtrar ni ordenar, y el autofiltro de
/// Excel solo cubre un rango contiguo.
fn write_summary(
    worksheet: &mut Worksheet,
    conn: &Connection,
    stats: &[SheetStats],
    styles: &Styles,
) -> Result<()> {
    let columns = ["Section".to_string(), "Metric".to_string(), "Value".to_string()];
    let mut widths = write_header(worksheet, &columns, styles)?;
    let mut row = 1u32;

    // -------------------------------------------------------------- Datos del rastreo
    let meta = read_meta(conn)?;
    for (metric, value) in meta {
        put_row(
            worksheet,
            &mut row,
            &mut widths,
            "Crawl",
            &metric,
            SummaryValue::Text(value),
            None,
        )?;
    }

    // -------------------------------------------------------------- Totales
    for (metric, sql) in [
        ("URLs", "SELECT COUNT(*) FROM urls"),
        ("Internal URLs", "SELECT COUNT(*) FROM urls WHERE is_internal = 1"),
        ("External URLs", "SELECT COUNT(*) FROM urls WHERE is_internal = 0"),
        ("HTML pages", "SELECT COUNT(*) FROM pages"),
        ("Indexable pages", "SELECT COUNT(*) FROM pages WHERE is_indexable = 1"),
        ("Non-indexable pages", "SELECT COUNT(*) FROM pages WHERE is_indexable = 0"),
        ("Links", "SELECT COUNT(*) FROM links"),
        ("Images", "SELECT COUNT(*) FROM images"),
        ("Issues", "SELECT COUNT(*) FROM issues"),
    ] {
        let n = scalar_i64(conn, sql)?;
        put_row(worksheet, &mut row, &mut widths, "Totals", metric, SummaryValue::Number(n), None)?;
    }

    // -------------------------------------------------------------- Códigos de estado
    // `skipped` fuera: nunca se pidieron, contarlas como «sin respuesta» sería mentir.
    for (metric, n) in grouped(
        conn,
        "SELECT CASE
                    WHEN status_code IS NULL THEN 'No response'
                    WHEN status_code < 300 THEN '2xx'
                    WHEN status_code < 400 THEN '3xx'
                    WHEN status_code < 500 THEN '4xx'
                    ELSE '5xx' END AS grupo,
                COUNT(*)
         FROM urls WHERE crawl_state != 'skipped'
         GROUP BY grupo ORDER BY grupo",
    )? {
        put_row(
            worksheet,
            &mut row,
            &mut widths,
            "Status codes",
            &metric,
            SummaryValue::Number(n),
            None,
        )?;
    }

    // -------------------------------------------------------------- Hallazgos por severidad
    let severities = grouped(
        conn,
        "SELECT severity, COUNT(*) FROM issues GROUP BY severity
         ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2
                                WHEN 'low' THEN 3 ELSE 4 END",
    )?;
    for (severity, n) in &severities {
        // La severidad se pinta también aquí: es la fila que se mira de reojo.
        put_row(
            worksheet,
            &mut row,
            &mut widths,
            "Issues by severity",
            severity,
            SummaryValue::Number(*n),
            styles.for_severity(severity),
        )?;
    }

    // -------------------------------------------------------------- Por qué no son indexables
    for (metric, n) in grouped(
        conn,
        "SELECT indexability_reason, COUNT(*) FROM pages
         WHERE is_indexable = 0 AND indexability_reason IS NOT NULL
         GROUP BY indexability_reason ORDER BY COUNT(*) DESC",
    )? {
        put_row(
            worksheet,
            &mut row,
            &mut widths,
            "Non-indexable, why",
            &metric,
            SummaryValue::Number(n),
            None,
        )?;
    }

    // -------------------------------------------------------------- Qué quedó fuera
    // Saber qué se excluyó es un hallazgo en sí mismo, no un detalle de implementación.
    for (metric, n) in grouped(
        conn,
        "SELECT exclusion_reason, COUNT(*) FROM urls
         WHERE crawl_state = 'excluded' AND exclusion_reason IS NOT NULL
         GROUP BY exclusion_reason ORDER BY COUNT(*) DESC",
    )? {
        put_row(
            worksheet,
            &mut row,
            &mut widths,
            "Excluded, why",
            &metric,
            SummaryValue::Number(n),
            None,
        )?;
    }

    // -------------------------------------------------------------- Índice de hojas
    // Dónde mirar y cuánto hay en cada sitio, sin tener que pinchar trece pestañas.
    for stat in stats {
        let metric = if stat.truncated {
            format!("{} (truncated at Excel's row limit)", stat.name)
        } else {
            stat.name.to_string()
        };
        put_row(
            worksheet,
            &mut row,
            &mut widths,
            "Sheets",
            &metric,
            SummaryValue::Number(stat.rows as i64),
            None,
        )?;
    }

    let data_rows = (row - 1) as usize;
    finish_sheet(worksheet, &widths, data_rows)?;
    Ok(())
}

enum SummaryValue {
    Text(String),
    Number(i64),
}

/// Escribe una fila del resumen y adelanta el cursor. `metric_format` pinta la celda del medio,
/// que es donde va la severidad.
#[allow(clippy::too_many_arguments)]
fn put_row(
    worksheet: &mut Worksheet,
    row: &mut u32,
    widths: &mut [usize],
    section: &str,
    metric: &str,
    value: SummaryValue,
    metric_format: Option<&Format>,
) -> Result<()> {
    worksheet.write_string(*row, 0, section)?;
    widths[0] = widths[0].max(section.chars().count());

    let metric = clamp_cell(metric);
    match metric_format {
        Some(fmt) => worksheet.write_string_with_format(*row, 1, &metric, fmt)?,
        None => worksheet.write_string(*row, 1, &metric)?,
    };
    widths[1] = widths[1].max(metric.chars().count());

    let used = match value {
        SummaryValue::Text(t) => {
            let t = clamp_cell(&t);
            worksheet.write_string(*row, 2, &t)?;
            t.chars().count()
        }
        SummaryValue::Number(n) => {
            worksheet.write_number(*row, 2, n as f64)?;
            n.to_string().chars().count()
        }
    };
    widths[2] = widths[2].max(used);

    *row += 1;
    Ok(())
}

/// Metadatos que se muestran en el resumen: columna de `crawl_meta` → etiqueta en inglés.
const META_FIELDS: &[(&str, &str)] = &[
    ("project_name", "Project"),
    ("base_url", "Base URL"),
    ("mode", "Mode"),
    ("source_path", "Source path"),
    ("adapter", "Adapter"),
    ("started_at", "Started at"),
    ("finished_at", "Finished at"),
    ("status", "Status"),
    ("core_version", "Core version"),
    ("rules_version", "Rules version"),
    ("tier_at_runtime", "Tier"),
];

/// Lee los metadatos del rastreo.
///
/// Se pide `SELECT *` y se resuelve cada campo **por nombre de columna**, no por posición ni con
/// un `SELECT` que las nombre: un rastreo anterior a la migración 002 no tiene `truncated`, y
/// nombrarla en el `SELECT` hace fallar la exportación entera con «no such column». Encontrado
/// exportando el fixture de 500.000 URLs, que es de antes de esa migración. `CONVENTIONS.md §4`:
/// un rastreo antiguo debe seguir abriéndose.
///
/// Un fichero sin fila en `crawl_meta` —rastreo abortado antes de escribirla— tampoco es motivo
/// para no exportar el resto.
fn read_meta(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT * FROM crawl_meta ORDER BY id LIMIT 1")?;
    let names: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    let index = |name: &str| names.iter().position(|c| c == name);

    let mut rows = stmt.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(vec![("crawl_meta".to_string(), "missing".to_string())]);
    };

    let mut out = Vec::with_capacity(META_FIELDS.len() + 2);
    for (column, label) in META_FIELDS {
        let Some(i) = index(column) else { continue };
        let value: Option<String> = row.get(i)?;
        // Un campo vacío o nulo no aporta una fila: `source_path` y `adapter` solo aplican a
        // algunos modos de rastreo.
        match value {
            Some(v) if !v.is_empty() => out.push((label.to_string(), v)),
            _ => {}
        }
    }

    // Que el rastreo esté truncado cambia cómo se leen todos los recuentos de abajo, así que se
    // dice aquí arriba y no en una nota al pie.
    let truncated = index("truncated").map(|i| row.get::<_, i64>(i)).transpose()?;
    if truncated.unwrap_or(0) != 0 {
        out.push(("Truncated".to_string(), "yes".to_string()));
        if let Some(i) = index("truncated_reason") {
            if let Some(reason) = row.get::<_, Option<String>>(i)? {
                out.push(("Truncated because".to_string(), reason));
            }
        }
    }
    Ok(out)
}

fn scalar_i64(conn: &Connection, sql: &str) -> Result<i64> {
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

fn grouped(conn: &Connection, sql: &str) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// ¿Existe la tabla? Un rastreo de esquema anterior no tiene `robots_txt` ni `sitemaps`, y
/// abrimos en solo lectura: migrar no es una opción.
fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// Nombre único para un fichero temporal, sin depender de un crate de tests.
    fn temp_path(nombre: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("crawlforge-xlsx-{}-{n}-{nombre}", std::process::id()))
    }

    /// Un fichero de rastreo con el **esquema real**, leído del core en tiempo de compilación.
    /// Copiar el esquema a mano haría pasar el test con una columna que el motor no escribe.
    ///
    /// **Al añadir una migración al core hay que añadirla también aquí.**
    fn crawl_file(path: &std::path::Path) -> Connection {
        let conn = Connection::open(path).expect("crear el fichero de rastreo");
        for sql in [
            include_str!("../../crawlforge-core/migrations/001_initial.sql"),
            include_str!("../../crawlforge-core/migrations/002_truncated.sql"),
            include_str!("../../crawlforge-core/migrations/003_orphans_exclude_seed.sql"),
            include_str!("../../crawlforge-core/migrations/004_robots_y_sitemaps.sql"),
        ] {
            conn.execute_batch(sql).expect("aplicar la migración");
        }
        conn
    }

    /// Rastreo de ejemplo con lo que hace daño de verdad: acentos, comillas, emojis, una celda
    /// que pasa del límite de Excel, un NULL en cada tipo de columna y un enlace roto.
    fn poblar(conn: &Connection) {
        conn.execute(
            "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode, started_at,
                                     finished_at, status, config_json, core_version,
                                     rules_version, tier_at_runtime, truncated, truncated_reason)
             VALUES ('c1','p1','Diseño «ñ» 🎨','https://sitio.es/','http',
                     '2026-07-30T10:00:00Z','2026-07-30T10:01:00Z','done','{}','0.1','0.1',
                     'free', 1, 'max_urls')",
            [],
        )
        .expect("meta");

        let urls = [
            (1, "https://sitio.es/", 200, 1),
            (2, "https://sitio.es/página-con-acentos/", 200, 1),
            (3, "https://sitio.es/rota/", 404, 1),
            (4, "https://sitio.es/vieja/", 301, 1),
            (5, "https://externo.com/", 200, 0),
            (6, "https://sitio.es/huérfana/", 200, 1),
        ];
        for (id, url, status, internal) in urls {
            conn.execute(
                "INSERT INTO urls (id, url, url_hash, scheme, host, path, depth, is_internal,
                                   in_sitemap, crawl_state, status_code, content_type,
                                   content_length, response_time_ms)
                 VALUES (?1, ?2, ?1, 'https', 'sitio.es', '/', 1, ?4, 1, 'done', ?3,
                         'text/html', 1234, 42)",
                params![id, url, status, internal],
            )
            .expect("url");
        }
        // Redirect con destino.
        conn.execute("UPDATE urls SET redirect_to = 1 WHERE id = 4", []).expect("redirect");

        // Un título larguísimo: por encima del límite de celda de Excel.
        let titulo_gigante = "á".repeat(MAX_CELL_CHARS + 500);
        conn.execute(
            "INSERT INTO pages (url_id, title, title_len, meta_description, h1, canonical,
                                is_indexable, indexability_reason, lang, word_count,
                                internal_links_in, internal_links_out)
             VALUES (1, ?1, 10, 'Descripción con «comillas» y emoji 🚀', 'H1 ñ', NULL, 1, NULL,
                     'es', 500, 3, 12)",
            params![titulo_gigante],
        )
        .expect("page 1");
        conn.execute(
            "INSERT INTO pages (url_id, title, is_indexable, indexability_reason, word_count)
             VALUES (2, 'Página normal', 0, 'noindex', 300)",
            [],
        )
        .expect("page 2");
        conn.execute(
            "INSERT INTO pages (url_id, title, is_indexable, word_count)
             VALUES (6, 'Huérfana', 1, 100)",
            [],
        )
        .expect("page 6");

        conn.execute(
            "INSERT INTO links (from_url_id, to_url_id, anchor, is_nofollow, element, region,
                                position)
             VALUES (1, 3, 'enlace «roto» 🔗', 0, 'a', 'main', 1),
                    (1, 4, 'a la vieja', 0, 'a', 'footer', 2),
                    (2, 1, 'inicio', 1, 'a', 'nav', 0)",
            [],
        )
        .expect("links");

        conn.execute(
            "INSERT INTO images (page_url_id, src_url_id, alt, alt_present, format, loading)
             VALUES (1, 5, NULL, 0, 'webp', 'lazy'),
                    (1, 5, 'Foto de diseño ñ', 1, 'avif', NULL)",
            [],
        )
        .expect("images");

        conn.execute(
            "INSERT INTO issues (url_id, rule_id, severity, category, detail_json)
             VALUES (1, 'SEO-TITLE-LONG', 'high', 'seo', '{\"len\": 90}'),
                    (2, 'INDEX-NOINDEX', 'critical', 'indexability', NULL),
                    (3, 'HTTP-404-INTERNAL', 'critical', 'http', NULL),
                    (NULL, 'SITE-NO-SITEMAP', 'medium', 'indexability', NULL),
                    (1, 'A11Y-IMG-NO-ALT', 'low', 'accessibility', NULL),
                    (1, 'INFO-SOMETHING', 'info', 'misc', NULL)",
            [],
        )
        .expect("issues");

        conn.execute(
            "INSERT INTO robots_txt (host, status_code, content, blocks_all, sitemap_count)
             VALUES ('sitio.es', 200, 'User-agent: *\nDisallow: /privado/\n', 0, 1)",
            [],
        )
        .expect("robots");
        conn.execute(
            "INSERT INTO sitemaps (url, status_code, is_index, is_valid, url_count, bytes,
                                   discovered_from)
             VALUES ('https://sitio.es/sitemap.xml', 200, 0, 1, 6, 900, 'robots')",
            [],
        )
        .expect("sitemap");
    }

    fn exportar_ejemplo(nombre: &str) -> (std::path::PathBuf, usize) {
        let store = temp_path(&format!("{nombre}.sqlite"));
        {
            let conn = crawl_file(&store);
            poblar(&conn);
        }
        let out = temp_path(&format!("{nombre}.xlsx"));
        let sheets = to_xlsx(&store, &out).expect("exportar");
        let _ = std::fs::remove_file(&store);
        (out, sheets)
    }

    #[test]
    fn exporta_un_fichero_con_todas_las_hojas() {
        let (out, sheets) = exportar_ejemplo("completo");
        assert_eq!(sheets, sheets_esperadas(), "una hoja por vista más Summary");
        assert!(out.exists(), "el fichero no se escribió");

        let bytes = std::fs::metadata(&out).expect("metadata").len();
        // Un xlsx vacío ronda los 3 KB; con doce hojas y una celda de 32k caracteres tiene que
        // pesar bastante más, y aun así no dispararse.
        assert!(bytes > 6_000, "el fichero pesa {bytes} bytes, parece vacío");
        assert!(bytes < 5_000_000, "el fichero pesa {bytes} bytes, algo se ha desbordado");

        // Es un ZIP: la firma local de fichero.
        let cabecera = std::fs::read(&out).expect("leer");
        assert_eq!(&cabecera[..4], b"PK\x03\x04", "no parece un xlsx");

        let _ = std::fs::remove_file(&out);
    }

    /// El orden es el producto: primero «¿qué está mal?», después «¿qué arreglo?» y los
    /// volcados al final. Que se cambie sin querer al añadir una hoja es un fallo de diseño.
    ///
    /// Se comprueba sobre la lista que construye el módulo, no sobre el fichero: `workbook.xml`
    /// va comprimido dentro del zip y sin un lector de zip no se puede releer.
    #[test]
    fn el_libro_declara_las_hojas_en_el_orden_previsto() {
        let esperado = [
            "Summary",
            "Issues by rule",
            "Issues",
            "Broken links",
            "Redirects",
            "Non-indexable",
            "Orphans",
            "Pages",
            "Images",
            "Links",
            "URLs",
            "Robots",
            "Sitemaps",
        ];
        let real: Vec<&str> =
            std::iter::once("Summary").chain(sheets().iter().map(|s| s.name)).collect();
        assert_eq!(real, esperado);
    }

    #[test]
    fn los_nombres_de_hoja_son_validos_para_excel() {
        for spec in sheets() {
            assert!(
                spec.name.chars().count() <= 31,
                "«{}» pasa de 31 caracteres",
                spec.name
            );
            for c in ['[', ']', ':', '*', '?', '/', '\\'] {
                assert!(!spec.name.contains(c), "«{}» contiene {c}", spec.name);
            }
        }
    }

    #[test]
    fn todas_las_consultas_son_sql_valido_contra_el_esquema_real() {
        // Un error de tipografía en una consulta no debe salir a la luz con un rastreo real
        // delante. Se preparan todas contra el esquema del core.
        let store = temp_path("sql.sqlite");
        {
            let conn = crawl_file(&store);
            poblar(&conn);
        }
        let conn = Connection::open(&store).expect("abrir");
        for spec in sheets() {
            let sql = spec.sql.replace("{SEV}", SEVERITY_ORDER);
            conn.prepare(&sql)
                .unwrap_or_else(|e| panic!("la consulta de «{}» no compila: {e}", spec.name));
        }
        drop(conn);
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn una_celda_larguisima_se_recorta_con_marca_en_vez_de_fallar() {
        let largo = "ñ".repeat(MAX_CELL_CHARS + 1_000);
        let recortado = clamp_cell(&largo);
        assert_eq!(recortado.chars().count(), MAX_CELL_CHARS);
        assert!(recortado.ends_with(TRUNCATION_MARK), "falta la marca de recorte");
        // Y no parte un carácter multibyte por la mitad: quedan exactamente los caracteres que
        // caben, no los bytes.
        let enes = recortado.chars().filter(|c| *c == 'ñ').count();
        assert_eq!(enes, MAX_CELL_CHARS - TRUNCATION_MARK.chars().count());
    }

    #[test]
    fn una_celda_corta_no_se_toca() {
        assert_eq!(clamp_cell("Diseño «ñ» 🎨"), "Diseño «ñ» 🎨");
        assert_eq!(clamp_cell(""), "");
    }

    #[test]
    fn el_ancho_de_columna_se_mantiene_entre_lo_legible_y_lo_usable() {
        // Una columna de URLs no puede ocupar 300 caracteres...
        assert_eq!(column_width(300), MAX_COL_WIDTH);
        // ...ni una de códigos de estado quedarse en 3.
        assert_eq!(column_width(3), MIN_COL_WIDTH);
        // Y `status_code` (11 caracteres de cabecera) no llega ni de lejos a 40.
        assert!(column_width(11) < 20.0);
        assert_eq!(column_width(20), 22.0);
    }

    /// Un rastreo del esquema **inicial**: sin `crawl_meta.truncated` (migración 002) y sin las
    /// tablas `robots_txt` ni `sitemaps` (migración 004).
    ///
    /// Es una regresión con nombre y apellidos: la primera versión nombraba `truncated` en el
    /// `SELECT` del resumen y la exportación del fixture real de 500.000 URLs —anterior a la
    /// 002— moría con «no such column». `CONVENTIONS.md §4`: un rastreo antiguo debe seguir
    /// abriéndose.
    #[test]
    fn un_rastreo_del_esquema_inicial_se_exporta_igual() {
        let store = temp_path("viejo.sqlite");
        {
            let conn = Connection::open(&store).expect("crear");
            conn.execute_batch(include_str!(
                "../../crawlforge-core/migrations/001_initial.sql"
            ))
            .expect("migración 001");
            conn.execute(
                "INSERT INTO crawl_meta (id, project_id, project_name, base_url, mode,
                                         started_at, status, config_json, core_version,
                                         rules_version, tier_at_runtime)
                 VALUES ('c1','p1','Antiguo','https://sitio.es/','http','2025-01-01T00:00:00Z',
                         'done','{}','0.0','0.0','free')",
                [],
            )
            .expect("meta");
        }
        let out = temp_path("viejo.xlsx");
        let sheets = to_xlsx(&store, &out).expect("exportar un rastreo antiguo");
        assert_eq!(sheets, sheets_esperadas());
        assert!(out.exists());
        let _ = std::fs::remove_file(&store);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn un_rastreo_vacio_no_revienta() {
        let store = temp_path("vacio.sqlite");
        {
            let _ = crawl_file(&store);
        }
        let out = temp_path("vacio.xlsx");
        let sheets = to_xlsx(&store, &out).expect("exportar un rastreo vacío");
        assert_eq!(sheets, sheets_esperadas());
        // Aun sin datos, cada hoja lleva su cabecera y su autofiltro.
        assert!(std::fs::metadata(&out).expect("metadata").len() > 3_000);
        let _ = std::fs::remove_file(&store);
        let _ = std::fs::remove_file(&out);
    }

    fn sheets_esperadas() -> usize {
        sheets().len() + 1
    }

    #[test]
    fn un_out_que_es_directorio_se_rechaza_antes_de_generar_el_libro() {
        // El cruce de la revisión de UX: `--format xlsx --out ./csv-out` daba
        // «Is a directory (os error 21)» al guardar, con el libro entero ya generado.
        let dir = std::env::temp_dir().join(format!("crawlforge-xlsx-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear el directorio");

        let err = ensure_xlsx_out_file(&dir).expect_err("un directorio no es un fichero");
        let msg = format!("{err:#}");
        assert!(msg.contains("--out is a file"), "dice qué se espera: {msg}");
        assert!(msg.contains("audit.xlsx"), "y propone la orden correcta: {msg}");
        assert!(msg.contains("--format csv"), "y a dónde iba lo del directorio: {msg}");
        assert!(!msg.contains("os error"), "sin errno: {msg}");

        // Un fichero normal, exista o no, pasa.
        ensure_xlsx_out_file(&dir.join("auditoria.xlsx")).expect("un .xlsx vale");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_out_con_extension_csv_avisa_del_cruce_de_formatos() {
        let err = ensure_xlsx_out_file(std::path::Path::new("datos.csv"))
            .expect_err("xlsx con salida .csv es un cruce");
        assert!(format!("{err:#}").contains("--format csv"), "{err:#}");
    }

    #[test]
    fn exportar_un_fichero_que_no_es_un_rastreo_falla_sin_jerga() {
        let store = temp_path("ajeno.sqlite");
        {
            let conn = Connection::open(&store).expect("crear");
            conn.execute_batch("CREATE TABLE cosas (id INTEGER);").expect("tabla ajena");
        }
        let out = temp_path("ajeno.xlsx");
        let err = to_xlsx(&store, &out).expect_err("no es un rastreo");
        let msg = format!("{err:#}");
        assert!(msg.contains("is not a CrawlForge crawl file"), "{msg}");
        assert!(!msg.contains("no such table"), "sin jerga de SQLite: {msg}");
        assert!(!out.exists(), "no se escribe nada");
        let _ = std::fs::remove_file(&store);
    }
}

#[cfg(test)]
mod prueba_manual {
    /// Exporta un rastreo real para abrirlo con Excel y mirarlo con los ojos. Lo que se juzga
    /// aquí no lo puede comprobar un test: si las columnas cuadran, si el filtro sirve y si la
    /// primera hoja responde «¿qué está mal?».
    ///
    /// `CRAWLFORGE_STORE=x.sqlite CRAWLFORGE_XLSX=/tmp/x.xlsx \
    ///  cargo test -p crawlforge-cli -- --ignored --nocapture ver_xlsx_real`
    #[test]
    #[ignore = "necesita un fichero de rastreo real; se lanza a mano"]
    fn ver_xlsx_real() {
        let Ok(store) = std::env::var("CRAWLFORGE_STORE") else {
            eprintln!("define CRAWLFORGE_STORE con la ruta de un .sqlite");
            return;
        };
        let out = std::env::var("CRAWLFORGE_XLSX")
            .unwrap_or_else(|_| std::env::temp_dir().join("crawlforge.xlsx").display().to_string());
        let inicio = std::time::Instant::now();
        let hojas = super::to_xlsx(std::path::Path::new(&store), std::path::Path::new(&out))
            .expect("exportar");
        let bytes = std::fs::metadata(&out).expect("metadata").len();
        println!(
            "{hojas} hojas, {:.1} MB, {:.1} s → {out}",
            bytes as f64 / 1_048_576.0,
            inicio.elapsed().as_secs_f64()
        );
    }
}
