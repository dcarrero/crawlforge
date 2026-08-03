//! `CONTENT` — encabezados y contenido. `docs/04-CATALOGO-REGLAS.md §6`.

use crate::{Category, Issue, PageContext, PageRule, RuleMeta, Scope, Severity, SiteRule, Tier};

pub static CONTENT_H1_MISSING: RuleMeta = RuleMeta {
    id: "CONTENT-H1-MISSING",
    severity: Severity::High,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin H1",
    name_en: "Missing H1",
    desc_es: "La página indexable no tiene H1, o lo tiene vacío. El H1 le dice al buscador de qué \
              trata la página con las palabras del autor, y es el primer encabezado que anuncia \
              un lector de pantalla al entrar en el contenido.",
    desc_en: "The indexable page has no H1, or an empty one. The H1 tells the search engine what \
              the page is about in the author's own words, and it is the first heading a screen \
              reader announces when entering the content.",
    references: &[],
};

pub static CONTENT_H1_MULTIPLE: RuleMeta = RuleMeta {
    id: "CONTENT-H1-MULTIPLE",
    severity: Severity::Low,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Varios H1",
    name_en: "Multiple H1",
    desc_es: "La página tiene más de un H1. HTML5 lo permite, pero deja de haber un tema \
              principal: el buscador tiene que adivinar de qué trata la página y el lector de \
              pantalla anuncia varios encabezados de nivel uno como si fueran documentos \
              distintos. Casi siempre es la plantilla marcando el logotipo o el nombre del sitio \
              como H1 además del titular.",
    desc_en: "The page has more than one H1. HTML5 allows it, but there is no longer a single \
              main topic: the search engine has to guess what the page is about, and a screen \
              reader announces several level-one headings as if they were separate documents. It \
              is usually the template marking the logo or the site name as an H1 on top of the \
              real headline.",
    references: &[],
};

pub static CONTENT_H1_EMPTY: RuleMeta = RuleMeta {
    id: "CONTENT-H1-EMPTY",
    severity: Severity::Medium,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "H1 sin texto",
    name_en: "Empty H1",
    desc_es: "Hay un H1 en el marcado, pero no aporta texto: está vacío o su único contenido es \
              una imagen que no dice nada. El encabezado ocupa el sitio del titular sin \
              cumplir su función, así que ni el buscador ni un lector de pantalla obtienen el \
              tema de la página. El caso típico es un logotipo dentro del H1.",
    desc_en: "There is an H1 in the markup, but it contributes no text: it is empty, or its only \
              content is an image that says nothing. The heading takes the headline's place \
              without doing its job, so neither the search engine nor a screen reader gets the \
              topic of the page. The usual case is a logo inside the H1.",
    references: &[],
};

pub static CONTENT_HEADING_SKIP: RuleMeta = RuleMeta {
    id: "CONTENT-HEADING-SKIP",
    severity: Severity::Low,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Salto de nivel de encabezado",
    name_en: "Heading level skipped",
    desc_es: "Los encabezados bajan más de un nivel de golpe, por ejemplo de H2 a H4. El esquema \
              del documento queda con huecos: quien navega por encabezados no sabe si el H4 es \
              un apartado del H2 o de otra sección, y el buscador pierde la jerarquía con la que \
              entiende qué depende de qué. Suele venir de elegir el encabezado por su tamaño de \
              letra en vez de por su nivel.",
    desc_en: "Heading levels drop by more than one step at once, for example from H2 to H4. The \
              document outline ends up with holes: someone navigating by headings cannot tell \
              whether the H4 belongs to that H2 or to another section, and the search engine \
              loses the hierarchy it uses to understand what belongs to what. It usually comes \
              from picking a heading by its font size instead of its level.",
    references: &[],
};

pub static CONTENT_THIN: RuleMeta = RuleMeta {
    id: "CONTENT-THIN",
    severity: Severity::High,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Contenido escaso",
    name_en: "Thin content",
    desc_es: "Una página indexable con menos de 300 palabras de texto visible. Rara vez tiene \
              suficiente materia para responder a una consulta, así que compite mal y, en \
              cantidad, diluye la calidad media del sitio a ojos del buscador. Muchas son \
              archivos, etiquetas o fichas autogeneradas que convendría no indexar en vez de \
              ampliar.",
    desc_en: "An indexable page with fewer than 300 words of visible text. It rarely has enough \
              substance to answer a query, so it competes poorly and, in bulk, dilutes the \
              site's average quality in the search engine's eyes. Many of these are archives, \
              tag pages or auto-generated stubs that are better left unindexed than expanded.",
    references: &[],
};

pub static CONTENT_LANG_MISSING: RuleMeta = RuleMeta {
    id: "CONTENT-LANG-MISSING",
    severity: Severity::Medium,
    category: Category::Content,
    min_tier: Tier::Free,
    scope: Scope::Page,
    name_es: "Sin atributo lang",
    name_en: "Missing lang attribute",
    desc_es: "El elemento <html> no declara el idioma del contenido. Sin él, el lector de \
              pantalla pronuncia el texto con las reglas del idioma del sistema, el navegador no \
              sabe qué diccionario usar para traducir y las señales de idioma del sitio quedan \
              en manos de la detección automática. Es un atributo de una línea.",
    desc_en: "The <html> element does not declare the language of the content. Without it, a \
              screen reader pronounces the text using the system language's rules, the browser \
              does not know which dictionary to use when translating, and the site's language \
              signals are left to automatic detection. It is a one-line attribute.",
    references: &[],
};

/// Número de palabras por debajo del cual una página indexable se considera escasa.
///
/// 300 es el umbral del catálogo (`§6`) y el que usa el resto de la industria. No se baja para
/// silenciar los fixtures cortos de las demás reglas: ahí el aviso también es correcto.
const THIN_MIN_WORDS: u32 = 300;

/// Página indexable sin `<h1>`.
pub struct ContentH1Missing;

impl PageRule for ContentH1Missing {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_H1_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        if ctx.h1_count > 0 && ctx.h1.map(|h| !h.trim().is_empty()).unwrap_or(false) {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_H1_MISSING)
            .with_detail(serde_json::json!({ "h1_count": ctx.h1_count }))]
    }
}

/// Más de un `<h1>` en la misma página.
pub struct ContentH1Multiple;

impl PageRule for ContentH1Multiple {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_H1_MULTIPLE
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        if ctx.h1_count <= 1 {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_H1_MULTIPLE)
            .with_detail(serde_json::json!({ "h1_count": ctx.h1_count }))]
    }
}

/// `<h1>` presente pero sin texto: vacío, con solo espacios, o con solo una imagen.
///
/// Nótese la frontera con [`ContentH1Missing`]: esa regla habla de la página que **no tiene**
/// titular, y también avisa aquí porque el efecto es el mismo. Ésta añade el dato que cambia la
/// reparación: el H1 ya existe en la plantilla, así que no hay que añadir uno, hay que darle
/// texto.
///
/// **Límite conocido.** El catálogo dice «H1 vacío o solo con una imagen sin alt». El
/// [`PageContext`] no dice qué imágenes están dentro del H1 —`images` es la lista de la página
/// entera— así que la regla no puede distinguir el H1 cuyo único contenido es una imagen **con**
/// `alt` (aceptable: el `alt` es el titular) del que la tiene **sin** `alt`. Se avisa en los dos
/// casos, que es el lado conservador: `ASSET-IMG-NO-ALT` cubre la parte de la imagen. Para
/// separarlos haría falta un dato nuevo en el contexto, y añadirlo no es trabajo de este módulo.
pub struct ContentH1Empty;

impl PageRule for ContentH1Empty {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_H1_EMPTY
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        // Sin ningún H1 no hay nada vacío que señalar: eso es `CONTENT-H1-MISSING`.
        if ctx.h1_count == 0 {
            return Vec::new();
        }
        let con_texto = ctx.h1.map(|h| !h.trim().is_empty()).unwrap_or(false);
        if con_texto {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_H1_EMPTY)
            .with_detail(serde_json::json!({ "h1_count": ctx.h1_count }))]
    }
}

/// Salto de nivel entre dos encabezados consecutivos: H2 seguido de H4.
///
/// Solo mira pares consecutivos. Que el primer encabezado del documento sea un H3 no se cuenta
/// como salto desde un nivel uno implícito: de eso ya avisa `CONTENT-H1-MISSING`, y contarlo dos
/// veces solo añadiría ruido al informe.
pub struct ContentHeadingSkip;

impl PageRule for ContentHeadingSkip {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_HEADING_SKIP
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        // Bajar de nivel es libre: de un H4 se puede volver a un H2 al abrir otra sección. Lo
        // que rompe el esquema es subir de profundidad más de un paso de golpe.
        let saltos: Vec<(usize, u8, u8)> = ctx
            .heading_levels
            .windows(2)
            .enumerate()
            .filter(|(_, par)| par[1] > par[0].saturating_add(1))
            .map(|(i, par)| (i + 1, par[0], par[1]))
            .collect();

        let Some(&(indice, desde, hasta)) = saltos.first() else {
            return Vec::new();
        };

        // El texto del encabezado que aterriza mal es el diagnóstico entero: en un rastreo real,
        // 16.764 filas decían `{"from":1,"to":4}` y hubo que abrir el HTML para descubrir que el
        // culpable era el `<h4>` de la firma del autor. Los tests pueden no traer textos
        // (`heading_texts` vacío); entonces el campo se omite en vez de inventarse.
        let texto = ctx.heading_texts.get(indice).map(|t| t.trim()).filter(|t| !t.is_empty());

        let mut detalle = serde_json::json!({
            "from": desde,
            "to": hasta,
            "index": indice,
            "skips": saltos.len(),
        });
        if let (Some(texto), Some(obj)) = (texto, detalle.as_object_mut()) {
            obj.insert("text".into(), serde_json::json!(truncate_chars(texto, 120)));
        }

        // Un hallazgo por página, con el primer salto como muestra: es el que hay que mirar para
        // entender el patrón, y el recuento dice si es un descuido o la plantilla entera.
        //
        // El `group_key` identifica **la causa y no la página**: la forma del salto más el texto
        // del encabezado culpable. Todas las firmas de autor `H1→H4` con el mismo texto son un
        // solo defecto de plantilla; dos `H1→H4` con textos distintos son dos defectos. Sin
        // texto no se puede afirmar que la causa sea la misma, así que la clave lo deja vacío y
        // esos hallazgos solo agrupan entre sí.
        vec![Issue::new(&CONTENT_HEADING_SKIP)
            .with_detail(detalle)
            .with_group(format!(
                "heading-skip:{desde}>{hasta}:{}",
                normalize_group_text(texto.unwrap_or(""))
            ))]
    }
}

/// Los primeros `max` caracteres —no bytes: cortar un byte a mitad de una «ñ» rompería la
/// cadena— de un texto, para el `detail_json`.
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Normaliza el texto de un encabezado para usarlo en un `group_key`: minúsculas, espacios
/// colapsados y 80 caracteres como mucho. «CONTACTO» y «Contacto » son la misma causa.
fn normalize_group_text(s: &str) -> String {
    let colapsado = s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    truncate_chars(&colapsado, 80)
}

/// Página indexable con menos de [`THIN_MIN_WORDS`] palabras de texto visible.
pub struct ContentThin;

impl PageRule for ContentThin {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_THIN
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        if ctx.word_count >= THIN_MIN_WORDS {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_THIN).with_detail(
            serde_json::json!({ "word_count": ctx.word_count, "threshold": THIN_MIN_WORDS }),
        )]
    }
}

/// `<html>` sin atributo `lang`.
pub struct ContentLangMissing;

impl PageRule for ContentLangMissing {
    fn meta(&self) -> &'static RuleMeta {
        &CONTENT_LANG_MISSING
    }

    fn evaluate(&self, ctx: &PageContext<'_>) -> Vec<Issue> {
        if !ctx.is_html || !ctx.is_indexable {
            return Vec::new();
        }
        // `lang=""` es tan inútil como no ponerlo, y es lo que deja una plantilla a la que no se
        // le pasó el idioma.
        if ctx.lang.map(|l| !l.trim().is_empty()).unwrap_or(false) {
            return Vec::new();
        }
        vec![Issue::new(&CONTENT_LANG_MISSING)]
    }
}

pub(crate) fn page_rules() -> Vec<Box<dyn PageRule>> {
    vec![
        Box::new(ContentH1Missing),
        Box::new(ContentH1Multiple),
        Box::new(ContentH1Empty),
        Box::new(ContentHeadingSkip),
        Box::new(ContentThin),
        Box::new(ContentLangMissing),
    ]
}

pub(crate) fn site_rules() -> Vec<Box<dyn SiteRule>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageView;

    /// Una página sana de la que partir: un solo H1 con texto, esquema de encabezados sin
    /// huecos, idioma declarado y las 500 palabras de `indexable_html`, por encima del umbral de
    /// `CONTENT-THIN`. Cada test rompe solo lo que le interesa.
    fn ctx<'a>() -> PageContext<'a> {
        let mut c = PageContext::indexable_html("https://ejemplo.es/a");
        c.h1 = Some("Un encabezado");
        c.h1_count = 1;
        c.heading_levels = &[1, 2];
        c.lang = Some("es");
        c
    }

    #[test]
    fn no_avisa_cuando_hay_h1() {
        assert!(ContentH1Missing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_cuando_no_hay_h1() {
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        let issues = ContentH1Missing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-H1-MISSING");
    }

    #[test]
    fn un_h1_vacio_cuenta_como_ausente() {
        let mut c = ctx();
        c.h1 = Some("  ");
        assert_eq!(ContentH1Missing.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_avisa_en_una_pagina_no_indexable() {
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        c.is_indexable = false;
        assert!(ContentH1Missing.evaluate(&c).is_empty());
    }

    // --- CONTENT-H1-MULTIPLE ---

    #[test]
    fn un_solo_h1_no_es_multiple() {
        assert!(ContentH1Multiple.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn una_pagina_sin_h1_no_es_multiple() {
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        assert!(ContentH1Multiple.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_con_dos_h1() {
        let mut c = ctx();
        c.h1_count = 2;
        c.heading_levels = &[1, 1, 2];
        let issues = ContentH1Multiple.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-H1-MULTIPLE");
        assert_eq!(issues[0].severity, Severity::Low);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"h1_count\":2"), "{detalle}");
    }

    #[test]
    fn varios_h1_en_una_pagina_no_indexable_no_avisan() {
        let mut c = ctx();
        c.h1_count = 3;
        c.is_indexable = false;
        assert!(ContentH1Multiple.evaluate(&c).is_empty());
    }

    #[test]
    fn varios_h1_en_algo_que_no_es_html_no_avisan() {
        let mut c = ctx();
        c.h1_count = 3;
        c.is_html = false;
        assert!(ContentH1Multiple.evaluate(&c).is_empty());
    }

    // --- CONTENT-H1-EMPTY ---

    #[test]
    fn no_avisa_cuando_el_h1_tiene_texto() {
        assert!(ContentH1Empty.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_cuando_el_h1_esta_vacio() {
        let mut c = ctx();
        c.h1 = Some("");
        let issues = ContentH1Empty.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-H1-EMPTY");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn un_h1_de_solo_espacios_esta_vacio() {
        let mut c = ctx();
        c.h1 = Some(" \n\t ");
        assert_eq!(ContentH1Empty.evaluate(&c).len(), 1);
    }

    #[test]
    fn un_h1_con_solo_una_imagen_sin_alt_esta_vacio() {
        // El motor solo mete en `h1` los nodos de texto del encabezado, así que un H1 cuyo único
        // hijo es un `<img>` llega aquí con la cadena vacía. Es el caso que de verdad importa: la
        // página parece tener titular y no lo tiene.
        let mut c = ctx();
        c.h1 = Some("");
        let imagenes =
            [ImageView { src: "/logo.svg", alt: None, ..Default::default() }];
        c.images = &imagenes;
        assert_eq!(ContentH1Empty.evaluate(&c).len(), 1);
    }

    #[test]
    fn sin_ningun_h1_no_avisa_de_h1_vacio() {
        // Es el terreno de `CONTENT-H1-MISSING`. Las dos reglas no dicen lo mismo.
        let mut c = ctx();
        c.h1 = None;
        c.h1_count = 0;
        assert!(ContentH1Empty.evaluate(&c).is_empty());
    }

    #[test]
    fn un_h1_vacio_en_una_pagina_no_indexable_no_avisa() {
        let mut c = ctx();
        c.h1 = Some("");
        c.is_indexable = false;
        assert!(ContentH1Empty.evaluate(&c).is_empty());
    }

    #[test]
    fn un_h1_vacio_en_algo_que_no_es_html_no_avisa() {
        let mut c = ctx();
        c.h1 = Some("");
        c.is_html = false;
        assert!(ContentH1Empty.evaluate(&c).is_empty());
    }

    // --- CONTENT-HEADING-SKIP ---

    #[test]
    fn un_esquema_consecutivo_no_tiene_saltos() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 3, 3, 4];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    #[test]
    fn volver_a_un_nivel_superior_no_es_un_salto() {
        // H1 → H2 → H3 → H2: la última bajada abre otra sección, no rompe nada.
        let mut c = ctx();
        c.heading_levels = &[1, 2, 3, 2];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    #[test]
    fn avisa_de_h2_a_h4() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 4];
        let issues = ContentHeadingSkip.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-HEADING-SKIP");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"from\":2"), "{detalle}");
        assert!(detalle.contains("\"to\":4"), "{detalle}");
        assert!(detalle.contains("\"index\":2"), "{detalle}");
        assert!(detalle.contains("\"skips\":1"), "{detalle}");
    }

    #[test]
    fn un_h1_seguido_de_h3_tambien_es_un_salto() {
        let mut c = ctx();
        c.heading_levels = &[1, 3];
        assert_eq!(ContentHeadingSkip.evaluate(&c).len(), 1);
    }

    #[test]
    fn varios_saltos_dan_un_solo_hallazgo_y_se_cuentan() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 4, 2, 5];
        let issues = ContentHeadingSkip.evaluate(&c);
        assert_eq!(issues.len(), 1, "un hallazgo por página, no uno por salto");
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"skips\":2"), "{detalle}");
    }

    #[test]
    fn el_detalle_incluye_el_texto_del_encabezado_culpable() {
        // Fue el texto lo que permitió diagnosticar a mano el `<h5>CONTACTO` del pie de una agencia;
        // sin él hay que ir a mirar el HTML de cada página.
        let mut c = ctx();
        c.heading_levels = &[1, 4];
        c.heading_texts = &["El título", "Firma del autor"];
        let issues = ContentHeadingSkip.evaluate(&c);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"text\":\"Firma del autor\""), "{detalle}");
    }

    #[test]
    fn sin_textos_el_detalle_no_inventa_un_campo() {
        let mut c = ctx();
        c.heading_levels = &[1, 4];
        let issues = ContentHeadingSkip.evaluate(&c);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(!detalle.contains("\"text\""), "{detalle}");
        // La clave existe igualmente, con el texto vacío: agrupa entre sí lo que no se conoce.
        assert_eq!(issues[0].group_key.as_deref(), Some("heading-skip:1>4:"));
    }

    #[test]
    fn la_clave_de_grupo_es_la_forma_del_salto_y_el_texto_culpable() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 5];
        c.heading_texts = &["Título", "Sección", "  CONTACTO "];
        let issues = ContentHeadingSkip.evaluate(&c);
        // Minúsculas y espacios colapsados: «CONTACTO» y «contacto » son la misma causa.
        assert_eq!(issues[0].group_key.as_deref(), Some("heading-skip:2>5:contacto"));
    }

    #[test]
    fn dos_textos_distintos_son_dos_causas_distintas() {
        let mut a = ctx();
        a.heading_levels = &[1, 4];
        a.heading_texts = &["Título", "Firma del autor"];
        let mut b = ctx();
        b.heading_levels = &[1, 4];
        b.heading_texts = &["Título", "Entradas relacionadas"];
        let ka = ContentHeadingSkip.evaluate(&a)[0].group_key.clone();
        let kb = ContentHeadingSkip.evaluate(&b)[0].group_key.clone();
        assert!(ka.is_some() && kb.is_some());
        assert_ne!(ka, kb, "el mismo salto con otro texto no es la misma plantilla");
    }

    #[test]
    fn una_pagina_con_un_solo_encabezado_no_puede_saltar() {
        let mut c = ctx();
        c.heading_levels = &[1];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
        c.heading_levels = &[];
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    #[test]
    fn un_salto_en_una_pagina_no_indexable_no_avisa() {
        let mut c = ctx();
        c.heading_levels = &[1, 2, 4];
        c.is_indexable = false;
        assert!(ContentHeadingSkip.evaluate(&c).is_empty());
    }

    // --- CONTENT-THIN ---

    #[test]
    fn no_avisa_con_contenido_suficiente() {
        assert!(ContentThin.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn el_umbral_es_de_trescientas_palabras() {
        let mut c = ctx();
        c.word_count = 300;
        assert!(ContentThin.evaluate(&c).is_empty(), "300 palabras ya no es contenido escaso");
        c.word_count = 299;
        let issues = ContentThin.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-THIN");
        assert_eq!(issues[0].severity, Severity::High);
        let detalle = issues[0].detail_json.as_deref().unwrap_or_default();
        assert!(detalle.contains("\"word_count\":299"), "{detalle}");
        assert!(detalle.contains("\"threshold\":300"), "{detalle}");
    }

    #[test]
    fn una_pagina_sin_texto_es_contenido_escaso() {
        let mut c = ctx();
        c.word_count = 0;
        assert_eq!(ContentThin.evaluate(&c).len(), 1);
    }

    #[test]
    fn una_pagina_corta_pero_no_indexable_no_avisa() {
        // Una ficha con `noindex` no compite en resultados: su longitud no es un problema.
        let mut c = ctx();
        c.word_count = 10;
        c.is_indexable = false;
        assert!(ContentThin.evaluate(&c).is_empty());
    }

    #[test]
    fn un_pdf_corto_no_es_contenido_escaso() {
        let mut c = ctx();
        c.word_count = 10;
        c.is_html = false;
        assert!(ContentThin.evaluate(&c).is_empty());
    }

    // --- CONTENT-LANG-MISSING ---

    #[test]
    fn no_avisa_cuando_el_idioma_esta_declarado() {
        assert!(ContentLangMissing.evaluate(&ctx()).is_empty());
    }

    #[test]
    fn avisa_cuando_falta_el_atributo_lang() {
        let mut c = ctx();
        c.lang = None;
        let issues = ContentLangMissing.evaluate(&c);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "CONTENT-LANG-MISSING");
        assert_eq!(issues[0].severity, Severity::Medium);
    }

    #[test]
    fn un_lang_vacio_cuenta_como_ausente() {
        let mut c = ctx();
        c.lang = Some("  ");
        assert_eq!(ContentLangMissing.evaluate(&c).len(), 1);
    }

    #[test]
    fn no_avisa_del_idioma_en_una_pagina_no_indexable() {
        let mut c = ctx();
        c.lang = None;
        c.is_indexable = false;
        assert!(ContentLangMissing.evaluate(&c).is_empty());
    }

    #[test]
    fn no_avisa_del_idioma_de_algo_que_no_es_html() {
        let mut c = ctx();
        c.lang = None;
        c.is_html = false;
        assert!(ContentLangMissing.evaluate(&c).is_empty());
    }
}
