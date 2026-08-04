//! `crawlforge rules` — el catálogo de reglas, en el idioma que se pida.
//!
//! Sirve para tres cosas: saber qué comprueba la herramienta antes de rastrear nada, tener los
//! IDs a mano para `--fail-on` en un pipeline de CI, y comprobar que las traducciones existen.
//! Los textos vienen del crate de reglas, no de aquí: si estuvieran en la CLI, la app de macOS
//! diría otra cosa.

use anyhow::{bail, Result};
use crawlforge_cli::i18n::msg;
use crawlforge_rules::{catalog, Lang, RuleMeta, Scope, Severity, Tier};

pub fn print_catalog(lang: Lang, categoria: Option<&str>, detalle: bool) -> Result<()> {
    let mut reglas = catalog();
    if let Some(filtro) = categoria {
        let filtro = filtro.to_ascii_lowercase();
        reglas.retain(|m| m.category.as_str() == filtro);
        if reglas.is_empty() {
            // Decir solo «ninguna regla» deja al usuario adivinando si escribió `metas` por
            // `meta`. Las categorías válidas están en el propio catálogo: se listan (y no se
            // traducen: son los valores que acepta `--category`).
            bail!(msg::error_unknown_category(lang, &filtro, category_names().join(", ")));
        }
    }

    // Por severidad y, dentro de ella, por ID: es el orden en el que se leen los hallazgos.
    reglas.sort_by_key(|m| (orden_severidad(m.severity), m.id));

    if detalle {
        for meta in &reglas {
            print_detalle(meta, lang);
        }
    } else {
        print_tabla(&reglas, lang);
    }

    print_resumen(&reglas, lang);
    Ok(())
}

/// La ficha completa de una regla, por su ID.
///
/// El bucle real del consultor es «el informe enseña un ID → quiero la explicación», y hasta la
/// revisión 2026-08-01 (§5.5) `crawlforge rules CANON-CHAIN` respondía `unexpected argument`:
/// había que buscar entre 58 fichas con `--detail`. Sin distinguir mayúsculas, porque el ID se
/// teclea de memoria tanto como se copia.
pub fn print_rule(lang: Lang, id: &str) -> Result<()> {
    let wanted = id.trim();
    match catalog().into_iter().find(|m| m.id.eq_ignore_ascii_case(wanted)) {
        Some(meta) => {
            print_detalle(meta, lang);
            Ok(())
        }
        // En inglés a propósito: es un error de argumento, el mismo terreno que los errores de
        // parseo de clap — la decisión está razonada en la cabecera de `main.rs`.
        None => bail!(
            "no rule has the ID {wanted:?}. List the catalog with: crawlforge rules"
        ),
    }
}

/// The catalog as JSON, for CI and for anything that consumes the rules as data — the
/// website's rule reference is generated from this output instead of keeping a copy that
/// would drift from the product.
///
/// Both languages are always included and `--lang` is ignored on purpose: a pipeline that
/// greps for a rule ID does not care, and a consumer that shows text picks its language from
/// the payload. The envelope carries `rules_version` and `count` so a consumer can assert
/// that what it generated matches the catalog it generated it from.
///
/// The order is the reading order of the human formats — severity, then ID — so the same
/// command with and without `--format json` lists the rules in the same sequence, and the
/// generated output is stable across runs.
pub fn print_json(id: Option<&str>, category: Option<&str>) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&catalog_json(id, category)?)?);
    Ok(())
}

/// Builds the JSON value so tests can assert on the structure without capturing stdout.
fn catalog_json(id: Option<&str>, category: Option<&str>) -> Result<serde_json::Value> {
    if let Some(wanted) = id {
        let wanted = wanted.trim();
        return match catalog().into_iter().find(|m| m.id.eq_ignore_ascii_case(wanted)) {
            Some(meta) => Ok(rule_json(meta)),
            // In English on purpose: an argument error, same ground as clap's parse errors.
            None => bail!("no rule has the ID {wanted:?}. List the catalog with: crawlforge rules"),
        };
    }

    let mut reglas = catalog();
    if let Some(filtro) = category {
        let filtro = filtro.to_ascii_lowercase();
        reglas.retain(|m| m.category.as_str() == filtro);
        if reglas.is_empty() {
            bail!(msg::error_unknown_category(
                Lang::En,
                &filtro,
                category_names().join(", ")
            ));
        }
    }
    reglas.sort_by_key(|m| (orden_severidad(m.severity), m.id));

    Ok(serde_json::json!({
        "rules_version": crawlforge_rules::RULES_VERSION,
        "count": reglas.len(),
        "rules": reglas.iter().map(|m| rule_json(m)).collect::<Vec<_>>(),
    }))
}

fn rule_json(meta: &RuleMeta) -> serde_json::Value {
    serde_json::json!({
        "id": meta.id,
        "severity": meta.severity,
        "category": meta.category,
        "scope": meta.scope,
        "min_tier": meta.min_tier,
        "name": { "en": meta.name_en, "es": meta.name_es },
        "description": { "en": meta.desc_en, "es": meta.desc_es },
        "references": meta.references.iter().map(|r| serde_json::json!({
            "standard": r.standard,
            "clause": r.clause,
            "url": r.url,
        })).collect::<Vec<_>>(),
    })
}

/// Las categorías que existen de verdad, deducidas del catálogo y no de una lista aparte que
/// se quedaría vieja al añadir una regla.
fn category_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = catalog().iter().map(|m| m.category.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Las cabeceras y etiquetas de la tabla siguen al `--lang` pedido, igual que los nombres de
/// las reglas: con el idioma por defecto (`en`) salían cabeceras en español sobre nombres en
/// inglés, dos idiomas en la misma pantalla.
fn print_tabla(reglas: &[&'static RuleMeta], lang: Lang) {
    println!(
        "{:<32} {:<9} {:<14} {:<7} {:<6} {}",
        "ID",
        msg::th_severity(lang),
        msg::th_category(lang),
        msg::th_scope(lang),
        msg::th_tier(lang),
        msg::th_name(lang)
    );
    println!("{}", "─".repeat(110));
    for meta in reglas {
        println!(
            "{:<32} {:<9} {:<14} {:<7} {:<6} {}",
            meta.id,
            meta.severity.as_str(),
            meta.category.as_str(),
            alcance(meta.scope, lang),
            nivel(meta.min_tier),
            meta.name(lang),
        );
    }
}

fn print_detalle(meta: &RuleMeta, lang: Lang) {
    println!("\n{}  [{}]", meta.id, meta.severity.as_str());
    println!("  {}", meta.name(lang));
    for linea in envolver(meta.description(lang), 92) {
        println!("  {linea}");
    }
    println!(
        "  {}: {}   {}: {}   {}: {}",
        msg::lbl_category(lang),
        meta.category.as_str(),
        msg::lbl_scope(lang),
        alcance(meta.scope, lang),
        msg::lbl_tier(lang),
        nivel(meta.min_tier)
    );
    for r in meta.references {
        println!("  {}: {} {} — {}", msg::lbl_reference(lang), r.standard, r.clause, r.url);
    }
}

fn print_resumen(reglas: &[&'static RuleMeta], lang: Lang) {
    let free = reglas.iter().filter(|m| m.min_tier == Tier::Free).count();
    let de_pagina = reglas.iter().filter(|m| m.scope == Scope::Page).count();
    println!(
        "\n{}",
        msg::rules_summary(lang, reglas.len(), free, de_pagina, reglas.len() - de_pagina)
    );
}

/// De más grave a menos, que es como se ordena un informe.
fn orden_severidad(s: Severity) -> u8 {
    match s {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
        Severity::Info => 4,
    }
}

fn alcance(s: Scope, lang: Lang) -> String {
    match s {
        Scope::Page => msg::scope_page(lang),
        Scope::Site => msg::scope_site(lang),
    }
}

fn nivel(t: Tier) -> &'static str {
    match t {
        Tier::Free => "free",
        Tier::Pro => "pro",
        Tier::Agency => "agency",
    }
}

/// Parte un texto en líneas sin cortar palabras. Cuenta caracteres, no bytes: en español el
/// texto lleva tildes y un corte por bytes descuadraría la columna.
fn envolver(texto: &str, ancho: usize) -> Vec<String> {
    let mut lineas = Vec::new();
    let mut actual = String::new();
    for palabra in texto.split_whitespace() {
        let largo = actual.chars().count();
        if largo > 0 && largo + 1 + palabra.chars().count() > ancho {
            lineas.push(std::mem::take(&mut actual));
        }
        if !actual.is_empty() {
            actual.push(' ');
        }
        actual.push_str(palabra);
    }
    if !actual.is_empty() {
        lineas.push(actual);
    }
    lineas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envolver_no_corta_palabras_ni_pierde_texto() {
        let texto = "La página es indexable y no tiene título, que es el factor con más peso";
        let lineas = envolver(texto, 20);
        assert!(lineas.iter().all(|l| l.chars().count() <= 20), "{lineas:?}");
        assert_eq!(lineas.join(" "), texto, "nothing is lost or duplicated");
    }

    #[test]
    fn envolver_cuenta_caracteres_y_no_bytes() {
        // Diez caracteres con tilde son veinte bytes: si se contaran bytes, esto se partiría.
        let lineas = envolver("áéíóú áéíóú", 11);
        assert_eq!(lineas.len(), 1, "{lineas:?}");
    }

    #[test]
    fn una_categoria_inexistente_es_un_error() {
        assert!(print_catalog(Lang::Es, Some("no-existe"), false).is_err());
    }

    #[test]
    fn una_categoria_inexistente_lista_las_que_si_existen() {
        // El caso de la revisión de UX: `--category metas` respondía «ninguna regla» y se
        // callaba; el usuario no tenía forma de saber que era `meta`.
        let err = print_catalog(Lang::Es, Some("metas"), false).expect_err("metas does not exist");
        let msg = err.to_string();
        for categoria in ["meta", "http", "canonical", "content"] {
            assert!(msg.contains(categoria), "it must list «{categoria}»: {msg}");
        }
    }

    #[test]
    fn las_categorias_salen_del_catalogo_sin_repetirse() {
        let names = category_names();
        assert!(names.contains(&"meta") && names.contains(&"http"), "{names:?}");
        let mut unicas = names.clone();
        unicas.dedup();
        assert_eq!(names, unicas, "no duplicates");
    }

    #[test]
    fn el_catalogo_se_imprime_en_los_dos_idiomas() {
        assert!(print_catalog(Lang::Es, None, false).is_ok());
        assert!(print_catalog(Lang::En, None, true).is_ok());
    }

    #[test]
    fn la_ficha_de_una_regla_se_encuentra_por_su_id_sin_distinguir_mayusculas() {
        // Revisión 2026-08-01 §5.5: el informe enseña el ID y el usuario lo teclea tal cual,
        // a veces en minúsculas. Ambos deben abrir la ficha.
        assert!(print_rule(Lang::En, "CANON-CHAIN").is_ok());
        assert!(print_rule(Lang::Es, "canon-chain").is_ok());
        assert!(print_rule(Lang::En, "  CANON-CHAIN  ").is_ok(), "a copy-paste brings spaces");
    }

    #[test]
    fn un_id_inexistente_es_un_error_que_dice_como_listar_el_catalogo() {
        let err = print_rule(Lang::En, "CANON-CADENA").expect_err("does not exist");
        let msg = err.to_string();
        assert!(msg.contains("CANON-CADENA"), "it names the culprit: {msg}");
        assert!(msg.contains("crawlforge rules"), "and states the next step: {msg}");
    }

    #[test]
    fn the_json_catalog_carries_every_rule_in_both_languages() {
        // The website's rule reference is generated from this output: a rule missing here is
        // a rule missing from the published reference, and an empty text is a blank card.
        let json = catalog_json(None, None).expect("the full catalog serializes");
        assert_eq!(json["rules_version"], crawlforge_rules::RULES_VERSION);
        let rules = json["rules"].as_array().expect("rules is an array");
        assert_eq!(rules.len(), catalog().len(), "one JSON entry per catalog rule");
        assert_eq!(json["count"], rules.len(), "the envelope count matches the array");
        for rule in rules {
            let id = rule["id"].as_str().expect("every rule has an ID");
            for field in ["name", "description"] {
                for lang in ["en", "es"] {
                    let text = rule[field][lang].as_str().unwrap_or("");
                    assert!(!text.trim().is_empty(), "{id} has no {field}.{lang}");
                }
            }
            for field in ["severity", "category", "scope", "min_tier"] {
                assert!(rule[field].is_string(), "{id} has no {field}");
            }
        }
    }

    #[test]
    fn the_json_of_one_rule_is_found_case_insensitively() {
        // Same contract as the human card: the ID is typed from memory as often as copied.
        let json = catalog_json(Some("canon-chain"), None).expect("the rule exists");
        assert_eq!(json["id"], "CANON-CHAIN");
        assert!(json["name"]["en"].is_string() && json["name"]["es"].is_string());
    }

    #[test]
    fn the_json_errors_match_the_table_ones() {
        // Unknown ID and unknown category fail the same way in both formats, so a pipeline
        // switching to json does not lose the diagnosis.
        let err = catalog_json(Some("CANON-CADENA"), None).expect_err("does not exist");
        assert!(err.to_string().contains("crawlforge rules"), "{err}");
        let err = catalog_json(None, Some("metas")).expect_err("metas does not exist");
        assert!(err.to_string().contains("meta"), "it lists the real categories: {err}");
    }

    #[test]
    fn the_json_category_filter_returns_only_that_category() {
        let json = catalog_json(None, Some("canonical")).expect("canonical exists");
        let rules = json["rules"].as_array().expect("rules is an array");
        assert!(!rules.is_empty());
        assert!(rules.iter().all(|r| r["category"] == "canonical"), "{json}");
    }
}
