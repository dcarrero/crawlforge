//! Normalización de URL. Ver `docs/03-MOTOR-CRAWL.md §3`.
//!
//! Rastrear la misma página cincuenta veces con querystrings distintas es el error más común
//! y el más caro de un crawler. Este módulo es la defensa.
//!
//! Se conservan **ambas** formas: la URL tal como aparecía en el HTML (para los informes) y la
//! normalizada (para deduplicar). Ver [`NormalizedUrl`].

use url::Url;

/// Parámetros de query que se descartan por defecto: no cambian el contenido servido,
/// solo la atribución de marketing.
pub const DEFAULT_STRIPPED_PARAMS: &[&str] = &[
    "gclid", "fbclid", "msclkid", "mc_cid", "mc_eid", "_ga", "ref", "si",
];

/// Prefijos de parámetro que se descartan por defecto. `utm_*` es una familia entera.
pub const DEFAULT_STRIPPED_PREFIXES: &[&str] = &["utm_"];

/// Qué hacer con la barra final.
///
/// La regla 8 de `§3` dice que se normalice «según lo que responda el servidor en la primera
/// resolución del host, no según una suposición». Por eso el valor por defecto es
/// [`TrailingSlash::AsIs`]: hasta que el motor haya observado el comportamiento real del host,
/// tocar la barra final inventa URLs que pueden no existir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailingSlash {
    /// No tocar. Por defecto, hasta que el host haya sido observado.
    #[default]
    AsIs,
    /// El host sirve los directorios con barra final (301 de `/a` a `/a/`).
    Add,
    /// El host sirve los directorios sin barra final (301 de `/a/` a `/a`).
    Remove,
}

/// Política de normalización. Configurable por rastreo.
#[derive(Debug, Clone)]
pub struct NormalizePolicy {
    pub stripped_params: Vec<String>,
    pub stripped_prefixes: Vec<String>,
    pub trailing_slash: TrailingSlash,
    /// Si es `true`, también se elimina el fragmento hashbang `#!` (SPA legadas).
    pub strip_hashbang: bool,
}

impl Default for NormalizePolicy {
    fn default() -> Self {
        Self {
            stripped_params: DEFAULT_STRIPPED_PARAMS.iter().map(|s| s.to_string()).collect(),
            stripped_prefixes: DEFAULT_STRIPPED_PREFIXES.iter().map(|s| s.to_string()).collect(),
            trailing_slash: TrailingSlash::default(),
            strip_hashbang: false,
        }
    }
}

impl NormalizePolicy {
    fn should_strip(&self, key: &str) -> bool {
        let lower = key.to_ascii_lowercase();
        self.stripped_params.iter().any(|p| p.eq_ignore_ascii_case(&lower))
            || self.stripped_prefixes.iter().any(|p| lower.starts_with(p.as_str()))
    }
}

/// Una URL normalizada, junto con la forma original en que apareció.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedUrl {
    /// Forma canónica: la que deduplica y la que se indexa por hash.
    pub normalized: Url,
    /// Tal como aparecía en el HTML. Es lo que se enseña en los informes: si el enlace del
    /// sitio está mal escrito, el informe debe mostrarlo mal escrito.
    pub original: String,
}

impl NormalizedUrl {
    /// Hash xxh3 de la forma normalizada. Es la clave del hot path «¿ya la visitamos?».
    ///
    /// Se guarda como `INTEGER` con signo porque SQLite no tiene enteros sin signo: el
    /// `as i64` reinterpreta los bits, no pierde información y mantiene la biyección.
    pub fn hash(&self) -> i64 {
        xxhash_rust::xxh3::xxh3_64(self.normalized.as_str().as_bytes()) as i64
    }
}

/// Normaliza una URL absoluta.
pub fn normalize(input: &str, policy: &NormalizePolicy) -> Result<NormalizedUrl, url::ParseError> {
    let parsed = Url::parse(input)?;
    Ok(finish(parsed, input.to_string(), policy))
}

/// Resuelve una URL posiblemente relativa contra la página que la contenía, y la normaliza.
///
/// Es la vía que usa el extractor de enlaces: en el HTML la mayoría de `href` son relativos.
pub fn normalize_relative(
    base: &Url,
    href: &str,
    policy: &NormalizePolicy,
) -> Result<NormalizedUrl, url::ParseError> {
    let joined = base.join(href)?;
    Ok(finish(joined, href.to_string(), policy))
}

/// El grueso del trabajo, común a ambas entradas.
///
/// El crate `url` ya aplica las reglas 1 (minúsculas de esquema y host, respetando la ruta),
/// 2 (puerto por defecto), 3 (resolución de `.` y `..`), 4 (recodificación consistente del
/// percent-encoding) y 9 (IDN a Punycode) al parsear. Aquí se añaden 5, 6, 7 y 8.
fn finish(mut url: Url, original: String, policy: &NormalizePolicy) -> NormalizedUrl {
    // Las credenciales (`usuario:contraseña@`) se vacían siempre, y se vacían aquí porque este
    // es el único embudo por el que pasa toda URL antes de escribirse: la forma normalizada
    // acaba en `urls.url` y de ahí en el CSV, el XLSX y la cabecera del informe que se manda
    // al cliente (revisión 2026-08-01 §1.6). No forman parte de la identidad de la página
    // —dos URLs que solo difieren en credenciales son el mismo recurso— y sí son un secreto.
    // Consecuencia deliberada: la autenticación básica ya no puede viajar en la URL; si algún
    // día hace falta, la credencial debe vivir en el `CrawlJob` y aplicarla el `Fetcher` como
    // cabecera, nunca en el dato que se persiste. `let _ =`: solo falla en URLs sin autoridad
    // (`mailto:`), donde no hay nada que vaciar.
    let _ = url.set_username("");
    let _ = url.set_password(None);

    // Regla 5 — eliminar el fragmento, salvo hashbang legado.
    match url.fragment() {
        Some(f) if f.starts_with('!') && !policy.strip_hashbang => {}
        _ => url.set_fragment(None),
    }

    // Reglas 6 y 7 — descartar parámetros de marketing y ordenar el resto alfabéticamente.
    //
    // El orden es por (clave, valor) para que `?a=2&a=1` sea estable. Un `?` que se queda sin
    // pares se elimina por completo: `ejemplo.es/a?` y `ejemplo.es/a` son la misma página.
    if url.query().is_some() {
        let mut pairs: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !policy.should_strip(k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        if pairs.is_empty() {
            url.set_query(None);
        } else {
            pairs.sort();
            let mut query = String::new();
            for (i, (k, v)) in pairs.iter().enumerate() {
                if i > 0 {
                    query.push('&');
                }
                query.push_str(k);
                query.push('=');
                query.push_str(v);
            }
            url.set_query(Some(&query));
        }
    }

    // Regla 8 — barra final, solo si el host ya fue observado.
    apply_trailing_slash(&mut url, policy.trailing_slash);

    NormalizedUrl { normalized: url, original }
}

/// La raíz (`/`) nunca se toca: `https://ejemplo.es` y `https://ejemplo.es/` son la misma URL,
/// y dejarla vacía produce una forma inválida.
fn apply_trailing_slash(url: &mut Url, policy: TrailingSlash) {
    if policy == TrailingSlash::AsIs {
        return;
    }
    let path = url.path().to_string();
    if path == "/" {
        return;
    }
    // Un último segmento con punto parece un fichero (`/style.css`, `/doc.pdf`): no lleva barra.
    let looks_like_file = path.rsplit('/').next().is_some_and(|s| s.contains('.'));

    match policy {
        TrailingSlash::Add if !path.ends_with('/') && !looks_like_file => {
            url.set_path(&format!("{path}/"));
        }
        TrailingSlash::Remove if path.ends_with('/') => {
            url.set_path(path.trim_end_matches('/'));
        }
        _ => {}
    }
}

/// ¿Es interna esta URL respecto al host semilla?
///
/// Los subdominios cuentan como externos: `blog.ejemplo.es` es otro sitio a efectos de
/// auditoría, y mezclarlos falsea el recuento de enlaces internos.
pub fn is_internal(url: &Url, seed_host: &str) -> bool {
    url.host_str().is_some_and(|h| h.eq_ignore_ascii_case(seed_host))
}

/// Esquemas que el motor rastrea. El resto (`mailto:`, `tel:`, `javascript:`, `data:`) se
/// descarta al extraer enlaces: no son páginas.
pub fn is_crawlable_scheme(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> String {
        normalize(s, &NormalizePolicy::default())
            .expect("URL de test válida")
            .normalized
            .to_string()
    }

    fn norm_with(s: &str, policy: &NormalizePolicy) -> String {
        normalize(s, policy).expect("URL de test válida").normalized.to_string()
    }

    // --- Regla 1: minúsculas en esquema y host, nunca en la ruta ---

    #[test]
    fn baja_esquema_y_host_pero_respeta_la_ruta() {
        assert_eq!(
            norm("HTTPS://EJEMPLO.ES/Ruta/Con-Mayusculas"),
            "https://ejemplo.es/Ruta/Con-Mayusculas"
        );
    }

    #[test]
    fn dos_rutas_que_solo_difieren_en_caja_son_urls_distintas() {
        // Un servidor Linux sirve /Foo y /foo como recursos distintos. Unificarlas
        // perdería páginas reales.
        assert_ne!(norm("https://ejemplo.es/Foo"), norm("https://ejemplo.es/foo"));
    }

    // --- Regla 2: puerto por defecto ---

    #[test]
    fn elimina_el_puerto_por_defecto_de_cada_esquema() {
        assert_eq!(norm("https://ejemplo.es:443/a"), "https://ejemplo.es/a");
        assert_eq!(norm("http://ejemplo.es:80/a"), "http://ejemplo.es/a");
    }

    #[test]
    fn conserva_un_puerto_no_estandar() {
        assert_eq!(norm("https://ejemplo.es:8443/a"), "https://ejemplo.es:8443/a");
    }

    // --- Regla 3: resolución de . y .. ---

    #[test]
    fn resuelve_segmentos_relativos() {
        assert_eq!(norm("https://ejemplo.es/a/b/../c"), "https://ejemplo.es/a/c");
        assert_eq!(norm("https://ejemplo.es/a/./b"), "https://ejemplo.es/a/b");
    }

    #[test]
    fn un_exceso_de_puntos_no_escapa_de_la_raiz() {
        assert_eq!(norm("https://ejemplo.es/../../../etc"), "https://ejemplo.es/etc");
    }

    // --- Regla 5: fragmento ---

    #[test]
    fn elimina_el_fragmento() {
        assert_eq!(norm("https://ejemplo.es/a#seccion"), "https://ejemplo.es/a");
    }

    #[test]
    fn dos_anclas_de_la_misma_pagina_colapsan_en_una_sola_url() {
        assert_eq!(norm("https://ejemplo.es/a#uno"), norm("https://ejemplo.es/a#dos"));
    }

    #[test]
    fn conserva_el_hashbang_legado() {
        assert_eq!(norm("https://ejemplo.es/a#!/ruta"), "https://ejemplo.es/a#!/ruta");
    }

    #[test]
    fn con_strip_hashbang_activo_tambien_lo_elimina() {
        let policy = NormalizePolicy { strip_hashbang: true, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/a#!/ruta", &policy), "https://ejemplo.es/a");
    }

    // --- Regla 6: orden de los parámetros ---

    #[test]
    fn ordena_los_parametros_alfabeticamente() {
        assert_eq!(norm("https://ejemplo.es/a?z=1&a=2"), "https://ejemplo.es/a?a=2&z=1");
    }

    #[test]
    fn el_mismo_recurso_con_los_parametros_en_otro_orden_es_una_sola_url() {
        // Este test es la razón de ser del módulo: sin él, el crawler rastrea la misma
        // página tantas veces como permutaciones tenga su querystring.
        assert_eq!(
            norm("https://ejemplo.es/p?b=2&a=1&c=3"),
            norm("https://ejemplo.es/p?c=3&a=1&b=2")
        );
    }

    #[test]
    fn un_parametro_repetido_se_ordena_de_forma_estable() {
        assert_eq!(norm("https://ejemplo.es/a?x=2&x=1"), "https://ejemplo.es/a?x=1&x=2");
    }

    // --- Regla 7: parámetros descartados ---

    #[test]
    fn descarta_la_familia_utm_completa() {
        assert_eq!(
            norm("https://ejemplo.es/a?utm_source=x&utm_medium=y&id=7"),
            "https://ejemplo.es/a?id=7"
        );
    }

    #[test]
    fn descarta_los_identificadores_de_clic_conocidos() {
        for p in DEFAULT_STRIPPED_PARAMS {
            assert_eq!(
                norm(&format!("https://ejemplo.es/a?{p}=abc")),
                "https://ejemplo.es/a",
                "no se descartó el parámetro {p}"
            );
        }
    }

    #[test]
    fn descartar_parametros_no_distingue_mayusculas() {
        assert_eq!(norm("https://ejemplo.es/a?UTM_Source=x"), "https://ejemplo.es/a");
    }

    #[test]
    fn si_solo_habia_parametros_descartables_desaparece_la_query_entera() {
        // Debe quedar sin `?` colgando: `/a?` y `/a` son la misma página.
        assert_eq!(norm("https://ejemplo.es/a?utm_source=x"), "https://ejemplo.es/a");
    }

    #[test]
    fn conserva_los_parametros_que_si_cambian_el_contenido() {
        assert_eq!(
            norm("https://ejemplo.es/buscar?pagina=2&q=sofa"),
            "https://ejemplo.es/buscar?pagina=2&q=sofa"
        );
    }

    #[test]
    fn se_pueden_configurar_otros_parametros_a_descartar() {
        let policy = NormalizePolicy {
            stripped_params: vec!["sesion".into()],
            stripped_prefixes: vec![],
            ..Default::default()
        };
        // Descarta el configurado y, al vaciarse la lista por defecto, ya no descarta utm_.
        assert_eq!(
            norm_with("https://ejemplo.es/a?sesion=1&utm_source=x", &policy),
            "https://ejemplo.es/a?utm_source=x"
        );
    }

    // --- Regla 8: barra final ---

    #[test]
    fn por_defecto_no_toca_la_barra_final() {
        // Hasta observar al servidor, inventar la barra inventa URLs que pueden no existir.
        assert_eq!(norm("https://ejemplo.es/a"), "https://ejemplo.es/a");
        assert_eq!(norm("https://ejemplo.es/a/"), "https://ejemplo.es/a/");
    }

    #[test]
    fn anade_barra_cuando_el_host_asi_lo_sirve() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Add, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/blog", &policy), "https://ejemplo.es/blog/");
    }

    #[test]
    fn quita_barra_cuando_el_host_asi_lo_sirve() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Remove, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/blog/", &policy), "https://ejemplo.es/blog");
    }

    #[test]
    fn no_anade_barra_a_lo_que_parece_un_fichero() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Add, ..Default::default() };
        assert_eq!(
            norm_with("https://ejemplo.es/style.css", &policy),
            "https://ejemplo.es/style.css"
        );
    }

    #[test]
    fn nunca_deja_la_raiz_sin_barra() {
        let policy = NormalizePolicy { trailing_slash: TrailingSlash::Remove, ..Default::default() };
        assert_eq!(norm_with("https://ejemplo.es/", &policy), "https://ejemplo.es/");
    }

    // --- Regla 9: IDN ---

    #[test]
    fn convierte_el_host_idn_a_punycode() {
        assert_eq!(norm("https://diseño.es/a"), "https://xn--diseo-rta.es/a");
    }

    #[test]
    fn el_host_idn_y_su_punycode_son_la_misma_url() {
        assert_eq!(norm("https://diseño.es/a"), norm("https://xn--diseo-rta.es/a"));
    }

    // --- Resolución de enlaces relativos ---

    #[test]
    fn resuelve_las_formas_relativas_habituales_del_html() {
        let base = Url::parse("https://ejemplo.es/blog/post-1").expect("base válida");
        let p = NormalizePolicy::default();
        let r = |h: &str| {
            normalize_relative(&base, h, &p).expect("relativa válida").normalized.to_string()
        };
        assert_eq!(r("/contacto"), "https://ejemplo.es/contacto");
        assert_eq!(r("otro-post"), "https://ejemplo.es/blog/otro-post");
        assert_eq!(r("../inicio"), "https://ejemplo.es/inicio");
        assert_eq!(r("//cdn.ejemplo.es/img.png"), "https://cdn.ejemplo.es/img.png");
        assert_eq!(r("https://externo.com/x"), "https://externo.com/x");
    }

    #[test]
    fn conserva_el_href_original_para_el_informe() {
        // Si el sitio enlaza con `../inicio`, el informe debe poder enseñar eso literalmente.
        let base = Url::parse("https://ejemplo.es/blog/post-1").expect("base válida");
        let n = normalize_relative(&base, "../inicio?utm_source=x", &NormalizePolicy::default())
            .expect("relativa válida");
        assert_eq!(n.original, "../inicio?utm_source=x");
        assert_eq!(n.normalized.as_str(), "https://ejemplo.es/inicio");
    }

    // --- Credenciales ---

    #[test]
    fn las_credenciales_de_la_url_no_sobreviven_a_la_normalizacion() {
        // Revisión 2026-08-01 §1.6: la forma normalizada acaba en el fichero de rastreo, el
        // CSV, el XLSX y el informe que se entrega al cliente. Una contraseña no viaja ahí.
        assert_eq!(
            norm("https://staging:S3cret@pre.cliente.es/privada"),
            "https://pre.cliente.es/privada"
        );
        assert_eq!(norm("https://solo-usuario@ejemplo.es/a"), "https://ejemplo.es/a");
    }

    #[test]
    fn con_o_sin_credenciales_es_la_misma_pagina() {
        // El hash es la clave de deduplicación: si difirieran, el crawler visitaría dos veces
        // el mismo recurso según cómo estuviera escrito el enlace.
        let p = NormalizePolicy::default();
        let con = normalize("https://user:pass@ejemplo.es/a", &p).expect("válida");
        let sin = normalize("https://ejemplo.es/a", &p).expect("válida");
        assert_eq!(con.hash(), sin.hash());
        assert_eq!(con.normalized, sin.normalized);
    }

    // --- Hash ---

    #[test]
    fn urls_equivalentes_comparten_hash_y_distintas_no() {
        let p = NormalizePolicy::default();
        let a = normalize("https://ejemplo.es/a?x=1&utm_source=z", &p).expect("válida");
        let b = normalize("https://ejemplo.es/a?utm_medium=q&x=1", &p).expect("válida");
        let c = normalize("https://ejemplo.es/b", &p).expect("válida");
        assert_eq!(a.hash(), b.hash());
        assert_ne!(a.hash(), c.hash());
    }

    // --- Clasificación ---

    #[test]
    fn el_subdominio_cuenta_como_externo() {
        let u = Url::parse("https://blog.ejemplo.es/x").expect("válida");
        assert!(!is_internal(&u, "ejemplo.es"));
        assert!(is_internal(&u, "blog.ejemplo.es"));
    }

    #[test]
    fn descarta_los_esquemas_que_no_son_paginas() {
        for s in
            ["mailto:hola@ejemplo.es", "tel:+34600000000", "javascript:void(0)", "data:text/plain,x"]
        {
            let u = Url::parse(s).expect("válida");
            assert!(!is_crawlable_scheme(&u), "{s} no debería rastrearse");
        }
        assert!(is_crawlable_scheme(&Url::parse("https://ejemplo.es").expect("válida")));
    }
}
