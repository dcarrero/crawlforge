### CrawlForge — command-line messages.
###
### English (locales/en/) is the source and the authority; every other folder is a
### translation of it. CONTRIBUTING.md, "Adding or improving a language", explains the rules.
### The comments above each message are context for translators: they are not shown.

## Severity names

# Severity with an initial capital, for report headings.
severity_label_critical = Critical
severity_label_high = High
severity_label_medium = Medium
severity_label_low = Low
severity_label_info = Info
# Severity in lower case, for table columns.
severity_word_critical = critical
severity_word_high = high
severity_word_medium = medium
severity_word_low = low
severity_word_info = info

## Resumen del rastreo (`report.rs`)

crawl_finished = Crawl finished: { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URLs
    } in { $secs } s
crawl_failed_suffix = {" "}({ $n } failed)
crawl_truncated = Crawl truncated by { $reason }. The findings shown are those found up to that point.
# A 429 is not a page's status: the site asked the crawler to slow down. The pages
# were left pending, and the useful thing to say is how to get them.
crawl_rate_limited_note = The site rate-limited the crawl: { $n ->
        [one] { $n } page still answered 429 Too Many Requests after the retries and was left pending
       *[other] { $n } pages still answered 429 Too Many Requests after the retries and were left pending
    }, not reported as broken. Rules that need the whole site are not evaluated until they are fetched. Later, run:
    {"    "}crawlforge resume { $path }
    or crawl again with a lower --concurrency, or let the CrawlForge user-agent through the site's firewall.
report_rate_limited_note = > **The site rate-limited the crawl.** { $n ->
        [one] { $n } page answered 429 Too Many Requests and was left pending: it is not counted or reported as broken
       *[other] { $n } pages answered 429 Too Many Requests and were left pending: they are not counted or reported as broken
    }, and rules that need the whole site are not evaluated. `crawlforge resume` fetches the pending pages.
# `list_mode` no es un corte: el rastreo hizo exactamente lo que se le pidió, así que
# decirle al usuario «tu rastreo se truncó» sería mentir. Lo que sí hay que decirle es
# la consecuencia de auditar un conjunto y no un sitio.
crawl_list_mode_note = List crawl: only the URLs you provided were audited. Rules that need the site's complete link graph (orphans, incoming links, depth) are not evaluated.
# El tope de externas no trunca el rastreo del sitio —ese es `crawl_truncated`— pero
# dejar enlaces sin comprobar en silencio haría que el informe pareciera completo sin
# serlo, así que se dice cuántos quedaron fuera.
external_unchecked = { $n ->
        [one] { $n } external link was
       *[other] { $n } external links were
    } not checked: the max_external cap was reached. Raise it with --max-external.
# El otro tope de externas: el del registro. Estas no están en el fichero, así que sin
# este aviso el informe no tendría ni una fila que delatara que faltan.
external_unregistered = { $n ->
        [one] { $n } external link was
       *[other] { $n } external links were
    } not even recorded: the max_external_urls cap was reached. A single page linking to hundreds of thousands of hosts is what this cap is for.
# La variante de fichero para un rastreo cortado: aquí no se puede afirmar que el
# tope fuera el motivo — un truncado deja externas sin sondear igualmente.
external_never_checked = { $n ->
        [one] { $n } external link was recorded but never checked.
       *[other] { $n } external links were recorded but never checked.
    }
# Una auditoría de carpeta no tiene red: el motivo no es el tope, y `audit` no acepta
# --max-external. Lo que sí sirve es rastrear lo publicado.
external_offline = { $n ->
        [one] { $n } external link was
       *[other] { $n } external links were
    } not checked: auditing a folder makes no network requests. To check external links, crawl the published site with `crawlforge crawl <URL>`.
# La otra mitad de la honestidad de las externas: cuando la comprobación estaba
# apagada, «cero enlaces externos rotos» no significa que no los haya.
external_check_disabled = External links were not checked (--no-external-check): the absence of broken external links in this report does not mean there are none.
# El resumen del rastreo dice cuántas externas se sondearon aunque todo fuera bien:
# es el tiempo que el cierre atribuía en silencio al rastreo del sitio.
external_checked_note = { $n ->
        [one] { $n } external link checked.
       *[other] { $n } external links checked.
    }
# Qué reglas calló el truncado, por su ID: sin la lista, «las reglas que necesitan el
# grafo» obliga a adivinar cuáles son.
rules_not_evaluated = Rules that need the complete link graph were not evaluated: { $rules }.
results_title = Results
label_urls = URLs
label_html_pages = HTML pages
label_links = Links
note_internal = { $n } internal
note_indexable = { $n } indexable
heading_status_codes = Status codes
label_no_response = no response
note_request_failed = the request failed
label_never_requested = Never requested
note_never_requested = (discovered, but the crawl stopped before fetching them)
# Filas internas `skipped` sin motivo: en modo lista, los enlaces que apuntan fuera
# de la lista. Explican por qué «URLs 21» cuando se dieron 5.
label_out_of_scope = Out of scope
note_out_of_scope = (linked from audited pages, but outside this crawl's scope)
heading_non_indexable = Non-indexable, why
heading_findings = Findings
no_findings = No findings.
heading_excluded = Excluded
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = { $k ->
        [one] { $k } template issue ({ $pages } pages)
       *[other] { $k } template issues ({ $pages } pages)
    }
# «Hallazgos» y no «páginas»: la misma página puede estar dentro del grupo de
# plantilla (el logo) y traer además un hallazgo propio (su imagen destacada), así
# que el resto no son páginas nuevas, son hallazgos fuera del grupo.
plus_more_findings = {" "}+ { $n } more findings
example_urls = e.g. { $urls }

## Reformulación de reglas dominantes

#
# El caso que el colapso de plantilla no cubre: hallazgos masivos **ciertos y sin causa
# común hashable** (202.392 páginas profundas de un archivo sin atajos, 173.654 títulos
# largos de la misma plantilla). El recuento se conserva —nada se oculta— y se le añade
# la cuota del sitio, que es lo que lo convierte en un problema de arquitectura y no en
# una lista de tareas. El criterio es `crawlforge_rules::is_pervasive`.
pervasive_note = { $pct }% of the site
# La forma del problema de profundidad, dicha una vez: es la línea que sustituye a
# «202,392» a secas. Los números llegan ya formateados con el millar del idioma.
deep_pages_summary = { $pages } pages deeper than { $max } clicks — { $pct }% of the site (typical depth { $low }–{ $high }, deepest { $deepest })
rule_deep_sorted = Sorted by click depth, deepest first; the first column is clicks from home.
hint_full_lists = Full URL list for a rule: { $cmd }

## Listado de una regla (`report --rule`)

rule_affected_urls = { $n } affected URLs
rule_template_group = Template group — { $n } pages
rule_group_cause = cause: { $detail }
rule_other_pages = Other affected pages — { $n }
rule_no_findings = No findings for { $rule } in this crawl.
# El paso siguiente de un enlace roto: se arregla en la página que lo enlaza, y ese
# dato lo da `inspect`. El comando lo imprime el llamador, listo para copiar.
hint_who_links = A broken link is fixed on the page that links to it. Who links to a URL, with its anchor text:
error_unknown_rule = { $rule } is not a rule ID. The catalog is listed by `crawlforge rules`.
error_all_blocked_by_robots = robots.txt blocks every URL: there is nothing to audit. If the site is yours, you can retry with --ignore-robots.
error_all_excluded_by_pattern = every URL was skipped by your --exclude/--include patterns: there is nothing to audit. Check the patterns.
error_no_urls_fetched = the crawl fetched no URLs. Check the target.
error_dead_seed = could not connect to { $url } ({ $cause }). Check the URL.
cause_dns = the domain does not exist or does not resolve
cause_tls = the TLS certificate is not valid
cause_timeout = the server did not answer in time
cause_connection = connection refused
cause_no_response = no response
warn_base_mismatch = Warning: { $n } of { $total } canonicals point at { $origin }, not at the given base ({ $base }).
    Indexability results depend on that flag: if the site is published at
    { $origin }, repeat the audit with --base { $origin }

## Ficha de una URL (`inspect.rs`)

inspect_error_not_found = { $url } is not in this crawl.
inspect_closest = The closest URLs in the file are:
inspect_status_title = Status
label_http_status = HTTP status
label_content_type = Content type
label_response_time = Response time
label_depth = Depth
note_depth_clicks = clicks from the seed
label_in_sitemap = In sitemap
word_yes = yes
word_no = no
label_crawl_state = Crawl state
state_note_pending = discovered, but the crawl never fetched it
state_note_excluded = excluded from the crawl ({ $reason })
# Una externa con código o con error de sonda **sí se pidió**: decir de ella
# «excluida del rastreo» era falso. Se comprobó su estado y no se auditó su contenido.
state_note_external_checked = external URL — status checked, content not audited
state_note_external_not_checked = external URL — status not checked ({ $reason })
state_note_external_unchecked = external URL — never requested
# Interna `skipped` sin motivo: fuera del alcance del rastreo (el caso del modo lista).
state_note_out_of_scope = discovered, but outside this crawl's scope
state_note_error = the request failed ({ $detail })
# `unknown` no es una región del HTML, es el relleno de «no se supo»: se traduce,
# a diferencia de `main`/`nav`/`footer`, que son identificadores del documento.
region_unknown = unknown
inspect_page_title = Page
label_page_title = Title
label_meta_description = Meta description
label_h1 = H1
label_word_count = Word count
label_canonical = Canonical
canonical_self = self
label_indexable = Indexable
indexable_no = no — { $reason }
chars_suffix = { $n ->
        [one] { $n } char
       *[other] { $n } chars
    }
inspect_redirects_title = Redirect chain
inlinks_title = Inlinks ({ $n })
inlinks_by_region = By region: { $breakdown } · { $nofollow } nofollow
inlinks_none = No crawled page links to this URL.
inlinks_sample_heading = Linking pages, content links first:
no_anchor = (no anchor text)
# El corte dice cuántas se enseñan y el comando exacto de la lista completa: cortar
# sin decir cómo ver el resto es el error del que nació `report --rule` (§5.1).
inspect_truncated = … showing { $n } — full list: { $cmd }
images_title = Images ({ $n })
image_no_alt = (no alt)
image_usage_title = Used as an image
# Outbound links header. The three variables are counts.
outlinks_title = Outlinks ({ $total }: { $internal } internal, { $external } external)
# Suffix that keeps external URLs apart from the status-code count.
note_external_status = +{ $n } external
image_usage_line = embedded { $times ->
        [one] { $times } time
       *[other] { $times } times
    } on { $pages ->
        [one] { $pages } page
       *[other] { $pages } pages
    }

## Comparación de rastreos (`diff.rs`)

diff_title = Crawl comparison
label_before = Before
label_after = After
label_site = Site
label_common = Common
diff_crawl_counts = { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URLs
    }, { $findings ->
        [one] { $findings } finding
       *[other] { $findings } findings
    }
diff_common_note = { $n } URLs present in both crawls
warnings_title = Warnings
tag_inconclusive = INCONCLUSIVE
tag_warning = WARNING
side_before = before
side_after = after
warn_truncated = The '{ $side }' crawl is truncated. What disappeared or got resolved cannot be asserted: the crawl may simply not have reached it.
warn_truncated_by = The '{ $side }' crawl is truncated by { $reason }. What disappeared or got resolved cannot be asserted: the crawl may simply not have reached it.
# La variante de `warn_truncated` para `list_mode`: el rastreo no se cortó — es que
# un rastreo en modo lista nunca ve más que su lista. La consecuencia para el diff es
# la misma (las ausencias no se pueden afirmar) y el motivo, distinto.
warn_list_mode = The '{ $side }' crawl is a list crawl: it only saw the URLs it was given. What disappeared or got resolved cannot be asserted.
warn_unfinished = The '{ $side }' crawl did not finish (status '{ $status }'). Its absences mean nothing.
warn_rules_changed = The rule catalog changed ({ $before } → { $after }). Some of the new or resolved findings may come from the rules, not from the site.
warn_config_changed = The crawl configuration changed in: { $fields }. Without keeping that in mind, scope changes get attributed to the site.
warn_scope_changed = The base URLs differ: { $before } → { $after }. Same site, different starting point.
warn_mode_changed = The crawl modes differ: { $before } → { $after }.
warn_missing_table = The '{ $side }' crawl predates the `{ $table }` table: that comparison is skipped.
warn_order_inverted = The crawl given as 'after' ({ $after_started }) started earlier than the one given as 'before' ({ $before_started }). If that is not deliberate, swap the two arguments: 'Got worse' and 'Got better' come out reversed.
worse_title = Got worse
label_new_findings = New findings
more_examples = … and { $n } more
label_pages_lost_index = Pages no longer indexable
label_status_worse = Status codes that got worse
robots_content_changed = { $host }: the content changed
robots_blocks_all = ATTENTION: it now blocks crawling of the entire site.
nothing_worse = Nothing. No new findings, no pages dropped from the index.
better_title = Got better
label_findings_resolved = Findings resolved
label_pages_indexable_again = Pages indexable again
label_status_better = Status codes that improved
other_title = Other changes
label_new_urls = New URLs
label_urls_gone = URLs gone
label_titles_changed = Titles changed
label_meta_changed = Meta descriptions changed
label_canonicals_changed = Canonicals changed
label_findings_persisted = Findings still present
suppressed_title = Cannot be asserted
# Multilínea: el punto de llamada imprime cada línea con su sangría.
suppressed_intro = One of the two crawls is incomplete, so these differences could come from
    what was never crawled rather than from the site. They are not counted as changes:
label_candidate_new_urls = Candidate new URLs
label_candidate_urls_gone = Candidate URLs gone
label_candidate_new_findings = Candidate new findings
label_candidate_resolved = Candidate resolved findings
suppressed_advice = Re-crawl both sides with the same limit to get a firm diff.
gate_title = CI gate
# Multilínea: el punto de llamada imprime cada línea con su sangría.
gate_inconclusive = The 'before' crawl is incomplete: new findings cannot be asserted and
    --fail-on takes no stance. This is not a pass.
gate_fail_detail = { $n ->
        [one] { $n } new finding
       *[other] { $n } new findings
    } [{ $severity }] — matches --fail-on { $token }
gate_pass = PASS   nothing watched shows up as a new finding.

## Reabrir un diff guardado y listar sus filas (`diff_file.rs`)

label_saved = Saved
diff_saved_note = diff generated { $generated_at } by crawlforge { $version } · { $path }
# Multilínea: el punto de llamada imprime cada línea con su sangría.
hint_diff_full_lists = The lists above show only a few examples. For every row, add --rule <ID> or
    --change <type> to the same command (e.g. --change url_removed).
warn_unknown_change_types = { $n ->
        [one] { $n } change of a type this crawlforge does not know ({ $types }) was left out.
       *[other] { $n } changes of a type this crawlforge does not know ({ $types }) were left out.
    } The file was written by a newer version.
label_status_changed = Status codes changed
diff_no_matching_changes = No change in this diff matches.
error_diff_missing = { $path } does not exist. Diff files are saved by `crawlforge diff <before> <after> --out <file>`.
error_diff_needs_two = { $path } is a crawl file, and comparing needs two: the earlier crawl and the later one. A single file is only accepted if it is a diff saved with --out.
hint_diff_prev = Its previous crawl is next to it:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } is neither a crawl file nor a diff file. Diff files are saved by `crawlforge diff <before> <after> --out <file>`.
error_diff_too_new = { $path } is a diff file of format { $version }, newer than this crawlforge understands ({ $supported }). Update crawlforge to open it.
error_diff_reopen_flag = { $flag } applies when comparing two crawls; a saved diff is only read. Run it on the crawls: crawlforge diff <before> <after> { $flag } …
error_unknown_change_type = { $value } is not a change type. Available: { $available }.
error_rule_with_change = --rule matches findings, so it only combines with --change issue_appeared or issue_resolved, not with { $change }.
site_wide_finding = (site-wide finding)
site_placeholder = (site)
error_missing_file = { $path } does not exist
# El contrato del manual §5: todo error de fichero dice qué comando lo genera.
error_store_missing = { $path } does not exist. Crawl files are produced by `crawlforge crawl`, `crawlforge audit` and `crawlforge list`.
error_not_a_crawl = { $path } does not look like a CrawlForge crawl file
error_different_sites = the two crawls are of different sites: { $a } and { $b }.
    URLs are matched by their text, so the diff would say the whole site disappeared and another one appeared. Compare each site against itself.
error_fail_on_unknown = --fail-on: '{ $token }' is not a rule or a severity.
    The severities are critical, high, medium, low and info; rule IDs are listed by `crawlforge rules`.

## Panel de cartera (`portfolio.rs`)

portfolio_title = Portfolio panel
portfolio_range = crawls from { $oldest } to { $newest }
# Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
# changed" would silently mean a different period on every site.
warn_portfolio_date_spread = The oldest crawl ({ $oldest }) and the newest ({ $newest }) are { $days } days apart: this panel is not a snapshot of the portfolio, and 'what changed' covers a different period on each site.
# Trap 3.2: two files crawled with different catalogs are not comparable without
# saying so — a rule can be "missing" on a site because it did not exist yet.
warn_portfolio_rules_versions = Not every site was crawled with the same rule catalog ({ $versions }). A rule can be missing on a site because it did not exist when that site was crawled.
warn_portfolio_core_versions = The crawls come from different engine versions ({ $versions }).
portfolio_skipped_title = Files set aside
# A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
portfolio_prev_not_input = it is a .prev.sqlite: the 'before' of the crawl next to it, already used by the comparison. It does not count as a site.
error_portfolio_newer_schema = its schema (v{ $version }) is newer than this build understands (v{ $supported }). Update crawlforge to open it.
error_portfolio_no_sites = no crawl files usable as a portfolio were found in the given paths. Crawl files are produced by `crawlforge crawl`, `crawlforge audit` and `crawlforge list`; a directory is scanned for *.sqlite files.
error_portfolio_tier = the portfolio panel is not part of the free tier: it needs Pro. Single-site commands (`report`, `diff`, `export`) remain available.
error_portfolio_too_many = the portfolio has { $n } sites and this tier caps it at { $max }.
portfolio_changes_title = What changed
portfolio_no_pairs = No site has a previous crawl next to it. Re-crawl into the same output file and the previous crawl is kept as .prev.sqlite — that is the 'before' this section compares against.
portfolio_new_critical_high = New critical and high findings
portfolio_none_critical_high = No new critical or high findings on any compared site.
portfolio_rest_title = The rest, site by site
portfolio_no_changes = no changes
# The diff of that pair cannot assert absences: one side is incomplete.
portfolio_pair_inconclusive = one side is incomplete: what disappeared or got resolved cannot be asserted
portfolio_pair_failed = its comparison against the previous crawl failed: { $reason }
portfolio_spread_title = Failing across the portfolio
portfolio_spread_intro = A rule firing on most sites is rarely content: it is usually a shared template or plugin — one fix that serves them all.
portfolio_spread_none = No rule fires on any site of the portfolio.
portfolio_glance_title = The portfolio at a glance
th_site = site
th_crawled = crawled
th_indexable = index.
flag_truncated = (truncated)
flag_list_mode = (list crawl)
flag_unfinished = (did not finish: { $status })
hint_site_diff = Full diff of one site:
portfolio_html_title = Portfolio panel
portfolio_sites_count = { $n ->
        [one] { $n } site
       *[other] { $n } sites
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = stored in the crawl file
rules_config_source_after = stored in the later crawl
rules_config_source_file = from { $path }
warn_rules_config_differs = The two crawls store different rule configurations. Both sides were evaluated with the same one ({ $source }), so a configuration change does not show up as new or resolved findings.
warn_portfolio_rules_config = Some sites are read with their own rule configuration (rules disabled / severities changed): { $sites }. Their counts are not comparable rule by rule with the rest.
# "Rule configuration (stored in the crawl file): 2 rules disabled, 1 severity changed."
rules_config_note = Rule configuration ({ $source }): { $disabled ->
        [one] { $disabled } rule
       *[other] { $disabled } rules
    } disabled, { $changed ->
        [one] { $changed } severity
       *[other] { $changed } severities
    } changed. The crawl file keeps every finding.
# "9 of 12 sites": the numerator of the spread table. The noun follows $total.
portfolio_sites_of = { $fired } of { $total ->
        [one] { $total } site
       *[other] { $total } sites
    }
# " (2 inconclusive)": sites where the rule could not be evaluated. Starts with a space.
# Never shown for zero.
portfolio_inconclusive_suffix = {" "}({ $n } inconclusive)
# The noun follows $total and the verb follows $pairs: "1 of 5 sites has".
portfolio_pairs_line = { $pairs } of { $total ->
        [one] { $total } site
       *[other] { $total } sites
    } { $pairs ->
        [one] has
       *[other] have
    } a previous crawl (.prev.sqlite) to compare against.

## Identificación de ficheros (`store_check.rs`)

error_store_is_diff = { $path } is not a crawl file: it is a diff file, the kind `crawlforge diff --out` saves. Open it with `crawlforge diff { $path }`.
    Crawl files are produced by `crawlforge crawl` and `crawlforge audit`.
error_store_foreign = { $path } is not a CrawlForge crawl file: it does not have its tables. Is it from another program?
    Crawl files are produced by `crawlforge crawl` and `crawlforge audit`.
error_store_not_sqlite = { $path } is not a CrawlForge file: it is not even a SQLite database.
    Crawl files are produced by `crawlforge crawl` and `crawlforge audit`.

## Informe Markdown/HTML (`audit_report.rs`)

report_audit_of = Audit of
# Un rastreo `list` que mezcla sitios: titularlo con el primero decía que los demás
# no se habían auditado.
report_audit_of_sites = Audit of { $n } sites: { $sites }
sites_and_more = { $sites } and { $n } more
report_mode_line = Mode `{ $mode }` · started { $started }
report_truncated_note = > **Truncated crawl** (`{ $reason }`). The counts cover what was crawled up to that point, not the whole site.
# Para `truncated_reason = 'list_mode'`: el rastreo no se cortó, auditó su lista
# entera. El informe tiene que decir eso, no «truncado».
report_list_mode_note = > **List crawl.** Only the listed URLs were audited: the counts describe that set, not the whole site, and rules that need the site's complete link graph are not evaluated.
th_metric = Metric
th_value = Value
row_pages = Pages
row_indexable = Indexable
row_errors_4xx_5xx = 4xx/5xx
report_more = … { $n } more
# El corte del informe dice dónde están las que no enseña: sin el comando, la única
# vía para verlas era abrir el XLSX o hacer SQL a mano (revisión de UX §5.1).
report_more_run = … { $n } more — run: { $cmd }
# La variante para una regla colapsada: el titular ya dio los números («13 problemas
# de plantilla (567 páginas)») y repetir aquí un recuento crudo de filas los
# contradiría.
report_full_list_run = … full list: { $cmd }
html_title = SEO audit

## Catálogo de reglas (`rules.rs`)

th_severity = SEVERITY
th_category = CATEGORY
th_scope = SCOPE
th_tier = TIER
th_name = NAME
lbl_category = category
lbl_scope = scope
lbl_tier = tier
lbl_reference = reference
scope_page = page
scope_site = site
rules_summary = { $total } rules: { $free } in the free tier, { $page } page-level and { $site } site-wide.
error_unknown_category = no rules in category '{ $category }'. The categories are: { $list }.

## Cadenas de `main.rs`, listas para cablear

crawling = Crawling { $target }
# Se valida **antes de tocar el disco**: una semilla imposible no debe costar una
# rotación de ficheros ni dejar un `.sqlite` de aspecto válido con un `.lock` al lado.
error_invalid_seed = { $url } is not a crawlable URL: expected something like https://example.com/. Nothing was created.
file_line = File:     { $path }
previous_kept = Previous: { $path } (kept, from the last run)
report_written = Report written to { $path }
# / Con `--out` y el formato de terminal, lo que se escribe es Markdown: un volcado del
# / terminal en un fichero conserva sus cajas y se lee peor. Se dice **qué** se escribió
# / para que nadie abra el fichero esperando lo que acaba de ver en pantalla.
markdown_report_written = Markdown report written to { $path }
exported_csv = Exported { $n } CSV files to { $dir }
exported_sheets = Exported { $n } sheets to { $path }
bench_appended = Benchmark record appended to { $path }
next_steps_title = Next steps
next_explain = Explain every finding, with affected URLs:
next_spreadsheet = Open the full data as a spreadsheet:
next_compare_previous = Compare against the previous run of this crawl:
next_compare_later = Re-run this same crawl after your next deploy, then compare the two:
next_reread = Re-read this summary any time:

## Autenticación básica HTTP (staging protegido)

#
# Los tres avisos van por stderr: hablan de secretos y no deben ensuciar un pipe.
# Ninguno repite la credencial, como corresponde a un aviso sobre secretos.
auth_url_note = Using HTTP Basic auth from the URL: it will be sent only to { $host } and will not be stored in the crawl file.
auth_env_note = Using HTTP Basic auth from CRAWLFORGE_AUTH: it will be sent only to { $host } and will not be stored in the crawl file.
auth_base_ignored = Warning: credentials in --base were removed: a directory audit reads from disk and makes no HTTP requests.
auth_list_ignored = Warning: credentials inside URL list entries are ignored and removed. To authenticate the crawl, set CRAWLFORGE_AUTH=user:password; it applies only to the host of the first URL.
progress_sitemaps = discovering sitemaps…
progress_finalize = final pass: incoming links and site-wide rules…
progress_crawl = { $fetched } crawled · { $queued } queued · { $rate } URL/s · { $findings } findings
progress_failed_suffix = {" "}· { $n } failed

## Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`)

resuming = Resuming crawl of { $target }
resume_counts = Picking up where it stopped: { $done } URLs already crawled, { $pending } still pending.
resume_config_note = Using the configuration saved in the file: the original run's settings apply.
error_resume_schema_blocking = { $file } is a schema v{ $version } crawl, and migration { $blocking } changes what the engine writes: half of it would say one thing and half another. Run the crawl again. You can still open this file with `report`, `export` and `diff`.
resume_robots_not_inherited = Note: the original crawl ignored robots.txt. This one does not — that permission is granted by you, not by a file. Re-run the original crawl command if you need it.
resume_finished = Crawl resumed and completed: { $urls ->
        [one] { $urls } more URL
       *[other] { $urls } more URLs
    } in { $secs } s
interrupt_flushing = Interrupted: saving what is already crawled… (Ctrl+C again to quit without waiting)
# Un Ctrl+C cuando el motor ya terminó —durante un export largo, por ejemplo— no
# tiene nada que guardar: se sale ya, diciendo que el rastreo no corre peligro.
interrupt_after_done = Interrupted. The crawl file is complete; a half-written export may be left behind.
# La degradación del cierre (§3.1/§3.2 de la revisión): con otro programa leyendo el
# fichero no se puede salir del modo WAL, y copiar el `.sqlite` suelto perdería lo
# que quede en el `-wal`. El aviso da el dato y las dos salidas.
warn_wal_kept = Note: another program has { $path } open, so it stays in WAL mode. The -wal and -shm files next to it are part of this crawl: copy the three together, or close the other program and run `crawlforge report` on it to fold them back in.
# El cerrojo del fichero (§3.3): el que llega segundo no espera ni rota nada, se le
# dice que hay otro rastreo escribiendo y qué hacer.
error_store_locked = another crawlforge process is writing { $path } right now. Wait for it to finish, or stop it, before crawling or resuming this file.
interrupted_saved = Crawl interrupted. Everything crawled so far is saved in { $path }. Continue it with:
hint_previous_unfinished = The previous crawl had not finished. To continue it instead of starting over:
error_resume_done = { $path } is a finished crawl: there is nothing to resume.
    To audit the site again, run the original crawl command; the finished file will be kept next to it as .prev.sqlite.
error_resume_status = { $path } cannot be resumed: its status is '{ $status }'. Only interrupted crawls (status 'running' or 'paused') can be resumed.
error_resume_schema = { $path } uses schema v{ $found } and this build writes v{ $expected }: an interrupted crawl can only be resumed by the version that started it. Run the crawl again.
error_resume_config = the crawl configuration stored in { $path } cannot be read. Was the file written by another version? Run the crawl again.
