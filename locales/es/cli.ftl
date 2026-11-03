### CrawlForge — command-line messages.
###
### English (locales/en/) is the source and the authority; every other folder is a
### translation of it. CONTRIBUTING.md, "Adding or improving a language", explains the rules.
### The comments above each message are context for translators: they are not shown.

## Severity names

# Severity with an initial capital, for report headings.
severity_label_critical = Crítico
severity_label_high = Alto
severity_label_medium = Medio
severity_label_low = Bajo
severity_label_info = Informativo
# Severity in lower case, for table columns.
severity_word_critical = crítico
severity_word_high = alto
severity_word_medium = medio
severity_word_low = bajo
severity_word_info = info

## Resumen del rastreo (`report.rs`)

crawl_finished = Rastreo terminado: { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URLs
    } en { $secs } s
crawl_failed_suffix = {" "}({ $n } con error)
crawl_truncated = Rastreo truncado por { $reason }. Los hallazgos mostrados son los encontrados hasta ese punto.
# A 429 is not a page's status: the site asked the crawler to slow down. The pages
# were left pending, and the useful thing to say is how to get them.
crawl_rate_limited_note = El sitio limitó el rastreo: { $n ->
        [one] { $n } página seguía respondiendo 429 Too Many Requests tras los reintentos y quedó pendiente, sin darla por rota
       *[other] { $n } páginas seguían respondiendo 429 Too Many Requests tras los reintentos y quedaron pendientes, sin darlas por rotas
    }. Las reglas que necesitan el sitio entero no se evalúan hasta que se pidan. Más tarde, lanza:
    {"    "}crawlforge resume { $path }
    o rastrea de nuevo con menos --concurrency, o deja pasar el user-agent de CrawlForge en el cortafuegos del sitio.
report_rate_limited_note = > **El sitio limitó el rastreo.** { $n ->
        [one] { $n } página respondió 429 Too Many Requests y quedó pendiente: no se cuenta ni se da por rota
       *[other] { $n } páginas respondieron 429 Too Many Requests y quedaron pendientes: no se cuentan ni se dan por rotas
    }, y las reglas que necesitan el sitio entero no se evalúan. `crawlforge resume` pide las pendientes.
# `list_mode` no es un corte: el rastreo hizo exactamente lo que se le pidió, así que
# decirle al usuario «tu rastreo se truncó» sería mentir. Lo que sí hay que decirle es
# la consecuencia de auditar un conjunto y no un sitio.
crawl_list_mode_note = Rastreo en modo lista: solo se auditaron las URLs que diste. Las reglas que necesitan el grafo completo del sitio (huérfanas, enlaces entrantes, profundidad) no se evalúan.
# El tope de externas no trunca el rastreo del sitio —ese es `crawl_truncated`— pero
# dejar enlaces sin comprobar en silencio haría que el informe pareciera completo sin
# serlo, así que se dice cuántos quedaron fuera.
external_unchecked = { $n ->
        [one] { $n } enlace externo quedó
       *[other] { $n } enlaces externos quedaron
    } sin comprobar: se alcanzó el tope max_external. Súbelo con --max-external.
# El otro tope de externas: el del registro. Estas no están en el fichero, así que sin
# este aviso el informe no tendría ni una fila que delatara que faltan.
external_unregistered = { $n ->
        [one] { $n } enlace externo no llegó
       *[other] { $n } enlaces externos no llegaron
    } ni a registrarse: se alcanzó el tope max_external_urls. Una sola página que enlace a cientos de miles de hosts es justo lo que este tope acota.
# La variante de fichero para un rastreo cortado: aquí no se puede afirmar que el
# tope fuera el motivo — un truncado deja externas sin sondear igualmente.
external_never_checked = { $n ->
        [one] { $n } enlace externo quedó registrado pero sin comprobar.
       *[other] { $n } enlaces externos quedaron registrados pero sin comprobar.
    }
# Una auditoría de carpeta no tiene red: el motivo no es el tope, y `audit` no acepta
# --max-external. Lo que sí sirve es rastrear lo publicado.
external_offline = { $n ->
        [one] { $n } enlace externo quedó
       *[other] { $n } enlaces externos quedaron
    } sin comprobar: auditar una carpeta no hace peticiones de red. Para comprobar los enlaces externos, rastrea el sitio publicado con `crawlforge crawl <URL>`.
# La otra mitad de la honestidad de las externas: cuando la comprobación estaba
# apagada, «cero enlaces externos rotos» no significa que no los haya.
external_check_disabled = Los enlaces externos no se comprobaron (--no-external-check): que este informe no traiga enlaces externos rotos no significa que no los haya.
# El resumen del rastreo dice cuántas externas se sondearon aunque todo fuera bien:
# es el tiempo que el cierre atribuía en silencio al rastreo del sitio.
external_checked_note = { $n ->
        [one] { $n } enlace externo comprobado.
       *[other] { $n } enlaces externos comprobados.
    }
# Qué reglas calló el truncado, por su ID: sin la lista, «las reglas que necesitan el
# grafo» obliga a adivinar cuáles son.
rules_not_evaluated = Las reglas que necesitan el grafo de enlaces completo no se evaluaron: { $rules }.
results_title = Resultados
label_urls = URLs
label_html_pages = Páginas HTML
label_links = Enlaces
note_internal = { $n } internas
note_indexable = { $n } indexables
heading_status_codes = Códigos de estado
label_no_response = sin respuesta
note_request_failed = la petición falló
label_never_requested = No solicitadas
note_never_requested = (descubiertas, pero el rastreo se detuvo antes de pedirlas)
# Filas internas `skipped` sin motivo: en modo lista, los enlaces que apuntan fuera
# de la lista. Explican por qué «URLs 21» cuando se dieron 5.
label_out_of_scope = Fuera del alcance
note_out_of_scope = (enlazadas desde páginas auditadas, pero fuera del alcance de este rastreo)
heading_non_indexable = No indexables, por qué
heading_findings = Hallazgos
no_findings = Sin hallazgos.
heading_excluded = Excluidas
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = { $k ->
        [one] { $k } problema de plantilla ({ $pages } páginas)
       *[other] { $k } problemas de plantilla ({ $pages } páginas)
    }
# «Hallazgos» y no «páginas»: la misma página puede estar dentro del grupo de
# plantilla (el logo) y traer además un hallazgo propio (su imagen destacada), así
# que el resto no son páginas nuevas, son hallazgos fuera del grupo.
plus_more_findings = {" "}+ { $n } hallazgos más
example_urls = p. ej. { $urls }

## Reformulación de reglas dominantes

#
# El caso que el colapso de plantilla no cubre: hallazgos masivos **ciertos y sin causa
# común hashable** (202.392 páginas profundas de un archivo sin atajos, 173.654 títulos
# largos de la misma plantilla). El recuento se conserva —nada se oculta— y se le añade
# la cuota del sitio, que es lo que lo convierte en un problema de arquitectura y no en
# una lista de tareas. El criterio es `crawlforge_rules::is_pervasive`.
pervasive_note = { $pct }% del sitio
# La forma del problema de profundidad, dicha una vez: es la línea que sustituye a
# «202,392» a secas. Los números llegan ya formateados con el millar del idioma.
deep_pages_summary = { $pages } páginas a más de { $max } clics — { $pct }% del sitio (profundidad típica { $low }–{ $high }, máxima { $deepest })
rule_deep_sorted = Ordenadas por profundidad de clic, la más hundida primero; la primera columna son los clics desde la portada.
hint_full_lists = Lista completa de URLs de una regla: { $cmd }

## Listado de una regla (`report --rule`)

rule_affected_urls = { $n } URLs afectadas
rule_template_group = Grupo de plantilla — { $n } páginas
rule_group_cause = causa: { $detail }
rule_other_pages = Otras páginas afectadas — { $n }
rule_no_findings = Sin hallazgos de { $rule } en este rastreo.
# El paso siguiente de un enlace roto: se arregla en la página que lo enlaza, y ese
# dato lo da `inspect`. El comando lo imprime el llamador, listo para copiar.
hint_who_links = Un enlace roto se arregla en la página que lo enlaza. Quién enlaza a una URL, con su texto de ancla:
error_unknown_rule = «{ $rule }» no es un ID de regla. El catálogo lo lista `crawlforge rules`.
error_all_blocked_by_robots = robots.txt bloquea todas las URLs: no hay nada que auditar. Si el sitio es tuyo, puedes reintentarlo con --ignore-robots.
error_all_excluded_by_pattern = los patrones de --exclude/--include han descartado todas las URLs: no hay nada que auditar. Revisa los patrones.
error_no_urls_fetched = el rastreo no obtuvo ninguna URL. Revisa el objetivo.
error_dead_seed = no se pudo conectar con { $url } ({ $cause }). Revisa la URL.
cause_dns = el dominio no existe o no resuelve
cause_tls = el certificado TLS no es válido
cause_timeout = el servidor no respondió a tiempo
cause_connection = conexión rechazada
cause_no_response = sin respuesta
warn_base_mismatch = Aviso: { $n } de { $total } canonicals apuntan a { $origin }, no a la base indicada ({ $base }).
    Los resultados de indexabilidad dependen de ese flag: si el sitio se publica en
    { $origin }, repite la auditoría con --base { $origin }

## Ficha de una URL (`inspect.rs`)

inspect_error_not_found = { $url } no está en este rastreo.
inspect_closest = Las URLs más parecidas del fichero son:
inspect_status_title = Estado
label_http_status = Código HTTP
label_content_type = Tipo de contenido
label_response_time = Tiempo de respuesta
label_depth = Profundidad
note_depth_clicks = clics desde la semilla
label_in_sitemap = En el sitemap
word_yes = sí
word_no = no
label_crawl_state = Estado de rastreo
state_note_pending = descubierta, pero el rastreo no llegó a pedirla
state_note_excluded = excluida del rastreo ({ $reason })
# Una externa con código o con error de sonda **sí se pidió**: decir de ella
# «excluida del rastreo» era falso. Se comprobó su estado y no se auditó su contenido.
state_note_external_checked = URL externa — estado comprobado, contenido no auditado
state_note_external_not_checked = URL externa — estado sin comprobar ({ $reason })
state_note_external_unchecked = URL externa — nunca solicitada
# Interna `skipped` sin motivo: fuera del alcance del rastreo (el caso del modo lista).
state_note_out_of_scope = descubierta, pero fuera del alcance de este rastreo
state_note_error = la petición falló ({ $detail })
# `unknown` no es una región del HTML, es el relleno de «no se supo»: se traduce,
# a diferencia de `main`/`nav`/`footer`, que son identificadores del documento.
region_unknown = desconocida
inspect_page_title = Página
label_page_title = Título
label_meta_description = Meta description
label_h1 = H1
label_word_count = Palabras
label_canonical = Canonical
canonical_self = la propia página
label_indexable = Indexable
indexable_no = no — { $reason }
chars_suffix = { $n ->
        [one] { $n } carácter
       *[other] { $n } caracteres
    }
inspect_redirects_title = Cadena de redirecciones
inlinks_title = Enlaces entrantes ({ $n })
inlinks_by_region = Por región: { $breakdown } · { $nofollow } nofollow
inlinks_none = Ninguna página rastreada enlaza a esta URL.
inlinks_sample_heading = Páginas que enlazan, primero los enlaces de contenido:
no_anchor = (sin texto de ancla)
# El corte dice cuántas se enseñan y el comando exacto de la lista completa: cortar
# sin decir cómo ver el resto es el error del que nació `report --rule` (§5.1).
inspect_truncated = … se muestran { $n } — lista completa: { $cmd }
images_title = Imágenes ({ $n })
image_no_alt = (sin alt)
image_usage_title = Usada como imagen
# Outbound links header. The three variables are counts.
outlinks_title = Enlaces salientes ({ $total }: { $internal ->
        [one] { $internal } interno
       *[other] { $internal } internos
    }, { $external ->
        [one] { $external } externo
       *[other] { $external } externos
    })
# Suffix that keeps external URLs apart from the status-code count.
note_external_status = +{ $n ->
        [one] { $n } externa
       *[other] { $n } externas
    }
image_usage_line = incrustada { $times ->
        [one] { $times } vez
       *[other] { $times } veces
    } en { $pages ->
        [one] { $pages } página
       *[other] { $pages } páginas
    }

## Comparación de rastreos (`diff.rs`)

diff_title = Comparación de rastreos
label_before = Antes
label_after = Después
label_site = Sitio
label_common = En común
diff_crawl_counts = { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URLs
    }, { $findings ->
        [one] { $findings } hallazgo
       *[other] { $findings } hallazgos
    }
diff_common_note = { $n } URLs presentes en los dos rastreos
warnings_title = Avisos
tag_inconclusive = NO CONCLUYENTE
tag_warning = AVISO
side_before = antes
side_after = después
warn_truncated = El rastreo «{ $side }» está truncado. Lo que desapareció o se resolvió no se puede afirmar: puede que el rastreo simplemente no llegara hasta ahí.
warn_truncated_by = El rastreo «{ $side }» está truncado por { $reason }. Lo que desapareció o se resolvió no se puede afirmar: puede que el rastreo simplemente no llegara hasta ahí.
# La variante de `warn_truncated` para `list_mode`: el rastreo no se cortó — es que
# un rastreo en modo lista nunca ve más que su lista. La consecuencia para el diff es
# la misma (las ausencias no se pueden afirmar) y el motivo, distinto.
warn_list_mode = El rastreo «{ $side }» es de modo lista: solo vio las URLs que se le dieron. Lo que desapareció o se resolvió no se puede afirmar.
warn_unfinished = El rastreo «{ $side }» no terminó (estado «{ $status }»). Sus ausencias no significan nada.
warn_rules_changed = El catálogo de reglas cambió ({ $before } → { $after }). Parte de los hallazgos nuevos o resueltos puede venir de las reglas, no del sitio.
warn_config_changed = La configuración del rastreo cambió en: { $fields }. Sin tenerlo en cuenta, los cambios de alcance se atribuyen al sitio.
warn_scope_changed = Las URLs base difieren: { $before } → { $after }. El mismo sitio, distinto punto de partida.
warn_mode_changed = Los modos de rastreo difieren: { $before } → { $after }.
warn_missing_table = El rastreo «{ $side }» es anterior a la tabla `{ $table }`: esa comparación se omite.
warn_order_inverted = El rastreo pasado como «después» ({ $after_started }) empezó antes que el pasado como «antes» ({ $before_started }). Si no es deliberado, intercambia los dos argumentos: «Ha empeorado» y «Ha mejorado» salen del revés.
worse_title = Ha empeorado
label_new_findings = Hallazgos nuevos
more_examples = … y { $n } más
label_pages_lost_index = Páginas que dejan de ser indexables
label_status_worse = Códigos de estado que empeoran
robots_content_changed = { $host }: el contenido cambió
robots_blocks_all = ATENCIÓN: ahora bloquea el rastreo del sitio entero.
nothing_worse = Nada: ni hallazgos nuevos ni páginas fuera del índice.
better_title = Ha mejorado
label_findings_resolved = Hallazgos resueltos
label_pages_indexable_again = Páginas indexables de nuevo
label_status_better = Códigos de estado que mejoran
other_title = Otros cambios
label_new_urls = URLs nuevas
label_urls_gone = URLs desaparecidas
label_titles_changed = Títulos cambiados
label_meta_changed = Meta descriptions cambiadas
label_canonicals_changed = Canonicals cambiados
label_findings_persisted = Hallazgos que persisten
suppressed_title = No se puede afirmar
# Multilínea: el punto de llamada imprime cada línea con su sangría.
suppressed_intro = Uno de los dos rastreos está incompleto, así que estas diferencias podrían
    venir de lo que no se llegó a rastrear y no del sitio. No se cuentan como cambios:
label_candidate_new_urls = URLs nuevas candidatas
label_candidate_urls_gone = URLs desaparecidas candidatas
label_candidate_new_findings = Hallazgos nuevos candidatos
label_candidate_resolved = Hallazgos resueltos candidatos
suppressed_advice = Vuelve a rastrear los dos lados con el mismo límite para obtener un diff firme.
gate_title = Puerta de CI
# Multilínea: el punto de llamada imprime cada línea con su sangría.
gate_inconclusive = El rastreo «antes» está incompleto: los hallazgos nuevos no se pueden afirmar
    y --fail-on no se pronuncia. Esto no es un aprobado.
gate_fail_detail = { $n ->
        [one] { $n } hallazgo nuevo
       *[other] { $n } hallazgos nuevos
    } [{ $severity }] — coincide con --fail-on { $token }
gate_pass = PASS   nada de lo vigilado aparece como hallazgo nuevo.

## Reabrir un diff guardado y listar sus filas (`diff_file.rs`)

label_saved = Guardado
diff_saved_note = diff generado el { $generated_at } con crawlforge { $version } · { $path }
# Multilínea: el punto de llamada imprime cada línea con su sangría.
hint_diff_full_lists = Las listas de arriba solo enseñan unos ejemplos. Para ver todas las filas, añade
    --rule <ID> o --change <tipo> al mismo comando (p. ej. --change url_removed).
warn_unknown_change_types = { $n ->
        [one] Se ha dejado fuera { $n } cambio
       *[other] Se han dejado fuera { $n } cambios
    } de un tipo que este crawlforge no conoce ({ $types }). El fichero lo escribió una versión más nueva.
label_status_changed = Códigos de estado cambiados
diff_no_matching_changes = Ningún cambio de este diff coincide.
error_diff_missing = { $path } no existe. Los ficheros de diff los guarda `crawlforge diff <antes> <después> --out <fichero>`.
error_diff_needs_two = { $path } es un fichero de rastreo, y para comparar hacen falta dos: el rastreo anterior y el posterior. Un fichero solo se acepta si es un diff guardado con --out.
hint_diff_prev = Su rastreo anterior está al lado:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } no es ni un fichero de rastreo ni un fichero de diff. Los ficheros de diff los guarda `crawlforge diff <antes> <después> --out <fichero>`.
error_diff_too_new = { $path } es un fichero de diff de formato { $version }, más nuevo de lo que entiende este crawlforge ({ $supported }). Actualiza crawlforge para abrirlo.
error_diff_reopen_flag = { $flag } se aplica al comparar dos rastreos; un diff guardado solo se lee. Lánzalo sobre los rastreos: crawlforge diff <antes> <después> { $flag } …
error_unknown_change_type = «{ $value }» no es un tipo de cambio. Disponibles: { $available }.
error_rule_with_change = --rule busca hallazgos, así que solo se combina con --change issue_appeared o issue_resolved, no con { $change }.
site_wide_finding = (hallazgo de sitio)
site_placeholder = (sitio)
error_missing_file = { $path } no existe
# El contrato del manual §5: todo error de fichero dice qué comando lo genera.
error_store_missing = { $path } no existe. Los ficheros de rastreo los generan `crawlforge crawl`, `crawlforge audit` y `crawlforge list`.
error_not_a_crawl = { $path } no parece un fichero de rastreo de CrawlForge
error_different_sites = los dos rastreos son de sitios distintos: { $a } y { $b }.
    Las URLs se emparejan por su texto, así que el diff diría que el sitio entero desapareció y que apareció otro. Compara cada sitio consigo mismo.
error_fail_on_unknown = --fail-on: «{ $token }» no es una regla ni una severidad.
    Las severidades son critical, high, medium, low e info; los IDs de regla los lista `crawlforge rules`.

## Panel de cartera (`portfolio.rs`)

portfolio_title = Panel de cartera
portfolio_range = rastreos del { $oldest } al { $newest }
# Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
# changed" would silently mean a different period on every site.
warn_portfolio_date_spread = Entre el rastreo más viejo ({ $oldest }) y el más nuevo ({ $newest }) hay { $days } días: este panel no es una foto de la cartera, y «qué cambió» cubre un periodo distinto en cada sitio.
# Trap 3.2: two files crawled with different catalogs are not comparable without
# saying so — a rule can be "missing" on a site because it did not exist yet.
warn_portfolio_rules_versions = No todos los sitios se rastrearon con el mismo catálogo de reglas ({ $versions }). Una regla puede faltar en un sitio porque no existía cuando se rastreó.
warn_portfolio_core_versions = Los rastreos vienen de versiones distintas del motor ({ $versions }).
portfolio_skipped_title = Ficheros apartados
# A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
portfolio_prev_not_input = es un .prev.sqlite: el «antes» del rastreo de al lado, que la comparación ya usa. No cuenta como sitio.
error_portfolio_newer_schema = su esquema (v{ $version }) es más nuevo de lo que esta versión entiende (v{ $supported }). Actualiza crawlforge para abrirlo.
error_portfolio_no_sites = en las rutas indicadas no se encontró ningún fichero de rastreo utilizable como cartera. Los ficheros de rastreo los generan `crawlforge crawl`, `crawlforge audit` y `crawlforge list`; un directorio se recorre buscando ficheros *.sqlite.
error_portfolio_tier = el panel de cartera no es del nivel gratuito: necesita Pro. Los comandos de un solo sitio (`report`, `diff`, `export`) siguen disponibles.
error_portfolio_too_many = la cartera tiene { $n } sitios y este nivel admite { $max }.
portfolio_changes_title = Qué cambió
portfolio_no_pairs = Ningún sitio tiene un rastreo anterior al lado. Repite el rastreo sobre el mismo fichero de salida y el anterior se conserva como .prev.sqlite: ese es el «antes» con el que compara esta sección.
portfolio_new_critical_high = Hallazgos nuevos críticos y altos
portfolio_none_critical_high = Ningún hallazgo nuevo crítico o alto en los sitios comparados.
portfolio_rest_title = El resto, sitio a sitio
portfolio_no_changes = sin cambios
# The diff of that pair cannot assert absences: one side is incomplete.
portfolio_pair_inconclusive = un lado está incompleto: lo que desapareció o se resolvió no se puede afirmar
portfolio_pair_failed = su comparación con el rastreo anterior falló: { $reason }
portfolio_spread_title = Qué falla en toda la cartera
portfolio_spread_intro = Una regla que salta en la mayoría de los sitios rara vez es contenido: suele ser una plantilla o un plugin compartido — un arreglo que sirve para todos.
portfolio_spread_none = Ninguna regla dispara en ningún sitio de la cartera.
portfolio_glance_title = La cartera de un vistazo
th_site = sitio
th_crawled = rastreado
th_indexable = index.
flag_truncated = (truncado)
flag_list_mode = (modo lista)
flag_unfinished = (sin terminar: { $status })
hint_site_diff = Diff completo de un sitio:
portfolio_html_title = Panel de cartera
portfolio_sites_count = { $n ->
        [one] { $n } sitio
       *[other] { $n } sitios
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = guardada en el fichero de rastreo
rules_config_source_after = guardada en el rastreo posterior
rules_config_source_file = de { $path }
warn_rules_config_differs = Los dos rastreos guardan configuraciones de reglas distintas. Los dos lados se evaluaron con la misma ({ $source }), así que un cambio de configuración no aparece como hallazgos nuevos ni resueltos.
warn_portfolio_rules_config = Algunos sitios se leen con su propia configuración de reglas (reglas desactivadas / severidades cambiadas): { $sites }. Sus recuentos no son comparables regla a regla con los del resto.
# "Rule configuration (stored in the crawl file): 2 rules disabled, 1 severity changed."
rules_config_note = Configuración de reglas ({ $source }): { $disabled ->
        [one] { $disabled } regla desactivada
       *[other] { $disabled } reglas desactivadas
    }, { $changed ->
        [one] { $changed } severidad cambiada
       *[other] { $changed } severidades cambiadas
    }. El fichero de rastreo conserva todos los hallazgos.
# "9 of 12 sites": the numerator of the spread table. The noun follows $total.
portfolio_sites_of = { $fired } de { $total ->
        [one] { $total } sitio
       *[other] { $total } sitios
    }
# " (2 inconclusive)": sites where the rule could not be evaluated. Starts with a space.
# Never shown for zero.
portfolio_inconclusive_suffix = {" "}({ $n ->
        [one] { $n } no concluyente
       *[other] { $n } no concluyentes
    })
# The noun follows $total and the verb follows $pairs: "1 of 5 sites has".
portfolio_pairs_line = { $pairs } de { $total ->
        [one] { $total } sitio
       *[other] { $total } sitios
    } { $pairs ->
        [one] tiene
       *[other] tienen
    } un rastreo anterior (.prev.sqlite) con el que comparar.

## Identificación de ficheros (`store_check.rs`)

error_store_is_diff = { $path } no es un fichero de rastreo: es un fichero de diff, de los que guarda `crawlforge diff --out`. Se abre con `crawlforge diff { $path }`.
    Los ficheros de rastreo los generan `crawlforge crawl` y `crawlforge audit`.
error_store_foreign = { $path } no es un fichero de rastreo de CrawlForge: no tiene sus tablas. ¿Es de otro programa?
    Los ficheros de rastreo los generan `crawlforge crawl` y `crawlforge audit`.
error_store_not_sqlite = { $path } no es un fichero de CrawlForge: ni siquiera es una base de datos SQLite.
    Los ficheros de rastreo los generan `crawlforge crawl` y `crawlforge audit`.

## Informe Markdown/HTML (`audit_report.rs`)

report_audit_of = Auditoría de
# Un rastreo `list` que mezcla sitios: titularlo con el primero decía que los demás
# no se habían auditado.
report_audit_of_sites = Auditoría de { $n } sitios: { $sites }
sites_and_more = { $sites } y { $n } más
report_mode_line = Modo `{ $mode }` · iniciada { $started }
report_truncated_note = > **Rastreo truncado** (`{ $reason }`). Los recuentos son de lo rastreado hasta ese punto, no del sitio entero.
# Para `truncated_reason = 'list_mode'`: el rastreo no se cortó, auditó su lista
# entera. El informe tiene que decir eso, no «truncado».
report_list_mode_note = > **Rastreo en modo lista.** Solo se auditaron las URLs de la lista: los recuentos describen ese conjunto, no el sitio entero, y las reglas que necesitan el grafo completo del sitio no se evalúan.
th_metric = Métrica
th_value = Valor
row_pages = Páginas
row_indexable = Indexables
row_errors_4xx_5xx = Errores 4xx/5xx
report_more = … { $n } más
# El corte del informe dice dónde están las que no enseña: sin el comando, la única
# vía para verlas era abrir el XLSX o hacer SQL a mano (revisión de UX §5.1).
report_more_run = … { $n } más — ejecuta: { $cmd }
# La variante para una regla colapsada: el titular ya dio los números («13 problemas
# de plantilla (567 páginas)») y repetir aquí un recuento crudo de filas los
# contradiría.
report_full_list_run = … lista completa: { $cmd }
html_title = Auditoría SEO

## Catálogo de reglas (`rules.rs`)

th_severity = SEVERIDAD
th_category = CATEGORÍA
th_scope = ALCANCE
th_tier = NIVEL
th_name = NOMBRE
lbl_category = categoría
lbl_scope = alcance
lbl_tier = nivel
lbl_reference = referencia
scope_page = página
scope_site = sitio
rules_summary = { $total } reglas: { $free } en el nivel gratuito, { $page } de página y { $site } de conjunto.
error_unknown_category = no hay reglas en la categoría «{ $category }». Las categorías son: { $list }.

## Cadenas de `main.rs`, listas para cablear

crawling = Rastreando { $target }
# Se valida **antes de tocar el disco**: una semilla imposible no debe costar una
# rotación de ficheros ni dejar un `.sqlite` de aspecto válido con un `.lock` al lado.
error_invalid_seed = { $url } no es una URL rastreable: se esperaba algo como https://example.com/. No se ha creado nada.
file_line = Fichero:  { $path }
previous_kept = Anterior: { $path } (conservado, de la ejecución anterior)
report_written = Informe escrito en { $path }
# / Con `--out` y el formato de terminal, lo que se escribe es Markdown: un volcado del
# / terminal en un fichero conserva sus cajas y se lee peor. Se dice **qué** se escribió
# / para que nadie abra el fichero esperando lo que acaba de ver en pantalla.
markdown_report_written = Informe Markdown escrito en { $path }
exported_csv = Exportados { $n } ficheros CSV a { $dir }
exported_sheets = Exportadas { $n } hojas a { $path }
bench_appended = Registro de benchmark añadido a { $path }
next_steps_title = Pasos siguientes
next_explain = Explicar cada hallazgo, con sus URLs afectadas:
next_spreadsheet = Abrir todos los datos como hoja de cálculo:
next_compare_previous = Comparar con la ejecución anterior de este rastreo:
next_compare_later = Repite este mismo rastreo tras tu próximo despliegue y compara los dos:
next_reread = Releer este resumen en cualquier momento:

## Autenticación básica HTTP (staging protegido)

#
# Los tres avisos van por stderr: hablan de secretos y no deben ensuciar un pipe.
# Ninguno repite la credencial, como corresponde a un aviso sobre secretos.
auth_url_note = Se usa la autenticación básica HTTP de la URL: se enviará solo a { $host } y no se guardará en el fichero de rastreo.
auth_env_note = Se usa la autenticación básica HTTP de CRAWLFORGE_AUTH: se enviará solo a { $host } y no se guardará en el fichero de rastreo.
auth_base_ignored = Aviso: las credenciales de --base se han retirado: una auditoría de directorio lee del disco y no hace peticiones HTTP.
auth_list_ignored = Aviso: las credenciales dentro de las URLs de la lista se ignoran y se retiran. Para autenticar el rastreo, define CRAWLFORGE_AUTH=usuario:contraseña; se aplica solo al host de la primera URL.
progress_sitemaps = descubriendo sitemaps…
progress_finalize = pasada final: enlaces entrantes y reglas de conjunto…
progress_crawl = { $fetched } rastreadas · { $queued } en cola · { $rate } URL/s · { $findings } hallazgos
progress_failed_suffix = {" "}· { $n } con error

## Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`)

resuming = Reanudando el rastreo de { $target }
resume_counts = Se retoma donde se quedó: { $done } URLs ya rastreadas, { $pending } pendientes.
resume_config_note = Se usa la configuración guardada en el fichero: mandan los ajustes de la ejecución original.
error_resume_schema_blocking = { $file } es un rastreo de esquema v{ $version }, y la migración { $blocking } cambia lo que el motor escribe: la mitad diría una cosa y la otra mitad otra. Vuelve a lanzar el rastreo. El fichero se sigue abriendo con `report`, `export` y `diff`.
resume_robots_not_inherited = Aviso: el rastreo original ignoraba robots.txt. Este no lo hace: ese permiso lo concedes tú, no un fichero. Vuelve a lanzar el rastreo original si lo necesitas.
resume_finished = Rastreo reanudado y completado: { $urls ->
        [one] { $urls } URL más
       *[other] { $urls } URLs más
    } en { $secs } s
interrupt_flushing = Interrumpido: guardando lo ya rastreado… (Ctrl+C otra vez para salir sin esperar)
# Un Ctrl+C cuando el motor ya terminó —durante un export largo, por ejemplo— no
# tiene nada que guardar: se sale ya, diciendo que el rastreo no corre peligro.
interrupt_after_done = Interrumpido. El fichero de rastreo está completo; puede quedar un export a medio escribir.
# La degradación del cierre (§3.1/§3.2 de la revisión): con otro programa leyendo el
# fichero no se puede salir del modo WAL, y copiar el `.sqlite` suelto perdería lo
# que quede en el `-wal`. El aviso da el dato y las dos salidas.
warn_wal_kept = Aviso: otro programa tiene abierto { $path }, así que se queda en modo WAL. Los ficheros -wal y -shm de al lado forman parte de este rastreo: cópialos los tres juntos, o cierra el otro programa y ejecuta `crawlforge report` sobre él para reintegrarlos.
# El cerrojo del fichero (§3.3): el que llega segundo no espera ni rota nada, se le
# dice que hay otro rastreo escribiendo y qué hacer.
error_store_locked = otro proceso de crawlforge está escribiendo { $path } ahora mismo. Espera a que termine, o deténlo, antes de rastrear o reanudar este fichero.
interrupted_saved = Rastreo interrumpido. Todo lo rastreado queda guardado en { $path }. Continúalo con:
hint_previous_unfinished = El rastreo anterior no había terminado. Para continuarlo en vez de empezar de cero:
error_resume_done = { $path } es un rastreo terminado: no hay nada que reanudar.
    Para auditar el sitio otra vez, repite el comando de rastreo original; el fichero terminado se conservará al lado como .prev.sqlite.
error_resume_status = { $path } no se puede reanudar: su estado es «{ $status }». Solo se reanudan los rastreos interrumpidos (estado «running» o «paused»).
error_resume_schema = { $path } es del esquema v{ $found } y esta versión escribe v{ $expected }: un rastreo interrumpido solo lo reanuda la versión que lo empezó. Repite el rastreo.
error_resume_config = la configuración de rastreo guardada en { $path } no se puede leer. ¿El fichero es de otra versión? Repite el rastreo.
