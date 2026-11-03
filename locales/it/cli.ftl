### CrawlForge — command-line messages, Italian.
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules.
###
### Style: imperatives with «tu» (esegui, controlla), as Italian software usually does.
### Glossary: crawl = scansione (scansionare); crawl file = file di scansione;
### finding = rilievo; severity = gravità (critica, alta, media, bassa);
### canonical = canonical (il canonical, URL canonico); template = template;
### portfolio = portfolio; diff = confronto; tier = piano; scope = ambito;
### URL is invariable (gli URL).

## Severity names

# Severity with an initial capital, for report headings.
severity_label_critical = Critica
severity_label_high = Alta
severity_label_medium = Media
severity_label_low = Bassa
severity_label_info = Info
# Severity in lower case, for table columns.
severity_word_critical = critica
severity_word_high = alta
severity_word_medium = media
severity_word_low = bassa
severity_word_info = info

## Non-indexability reasons

reason_noindex = noindex
reason_canonicalised = canonicalizzata
reason_robots = robots
reason_redirect = reindirizzamento
reason_4xx = 4xx
reason_5xx = 5xx
reason_not_html = non HTML

## Exclusion reasons

exclusion_robots = robots.txt
exclusion_nofollow = nofollow
exclusion_depth = profondità massima
exclusion_pattern = pattern URL
exclusion_limit = limite di URL
exclusion_local_network = rete locale
exclusion_external_check_off = verifica esterna disattivata
exclusion_no_network = audit di cartella, senza rete

## Crawl summary (`report.rs`)

crawl_finished = Scansione completata: { $urls } URL in { $secs } s
crawl_failed_suffix = {" "}({ $n ->
        [one] { $n } non riuscito
       *[other] { $n } non riusciti
    })
crawl_truncated = Scansione troncata da { $reason }. I rilievi mostrati sono quelli trovati fino a quel punto.
# A 429 is not a page's status: the site asked the crawler to slow down. The pages
# were left pending, and the useful thing to say is how to get them.
crawl_rate_limited_note = Il sito ha limitato la frequenza della scansione: { $n ->
        [one] { $n } pagina ha continuato a rispondere 429 Too Many Requests, o ha chiesto di attendere più di due minuti, ed è rimasta in sospeso, senza essere segnalata come non funzionante.
       *[other] { $n } pagine hanno continuato a rispondere 429 Too Many Requests, o hanno chiesto di attendere più di due minuti, e sono rimaste in sospeso, senza essere segnalate come non funzionanti.
    } Le regole che richiedono l'intero sito non vengono valutate finché queste pagine non sono state recuperate. Più tardi, esegui:
    {"    "}crawlforge resume { $path }
    oppure ripeti la scansione con un --concurrency più basso, o lascia passare lo user agent di CrawlForge nel firewall del sito.
report_rate_limited_note = > **Il sito ha limitato la frequenza della scansione.** { $n ->
        [one] { $n } pagina ha continuato a rispondere 429 Too Many Requests, o ha chiesto di attendere più di due minuti, ed è rimasta in sospeso: non viene conteggiata né segnalata come non funzionante
       *[other] { $n } pagine hanno continuato a rispondere 429 Too Many Requests, o hanno chiesto di attendere più di due minuti, e sono rimaste in sospeso: non vengono conteggiate né segnalate come non funzionanti
    }, e le regole che richiedono l'intero sito non vengono valutate. `crawlforge resume` le recupera.
crawl_throttled_note = Il server ha mostrato segni di sovraccarico (429, 503, connessioni interrotte o timeout), quindi la scansione ha rallentato: la concorrenza è stata dimezzata { $n ->
        [one] una volta
       *[other] { $n } volte
    }. La prossima volta, prova con un --concurrency più basso.
crawl_list_mode_note = Scansione in modalità elenco: sono stati verificati solo gli URL forniti. Le regole che richiedono il grafo completo dei link del sito (pagine orfane, link in entrata, profondità) non vengono valutate.
external_unchecked = { $n ->
        [one] { $n } link esterno non è stato verificato
       *[other] { $n } link esterni non sono stati verificati
    }: è stato raggiunto il limite max_external. Aumentalo con --max-external.
external_unregistered = { $n ->
        [one] { $n } link esterno non è stato nemmeno registrato
       *[other] { $n } link esterni non sono stati nemmeno registrati
    }: è stato raggiunto il limite max_external_urls. Questo limite esiste per il caso di una singola pagina che punta a centinaia di migliaia di host.
external_never_checked = { $n ->
        [one] { $n } link esterno è stato registrato ma mai verificato.
       *[other] { $n } link esterni sono stati registrati ma mai verificati.
    }
external_offline = { $n ->
        [one] { $n } link esterno non è stato verificato
       *[other] { $n } link esterni non sono stati verificati
    }: l'audit di una cartella non effettua richieste di rete. Per verificarli, scansiona il sito pubblicato con `crawlforge crawl <URL>`.
external_check_disabled = I link esterni non sono stati verificati (--no-external-check): l'assenza di link esterni non funzionanti in questo report non significa che non ce ne siano.
external_checked_note = { $n ->
        [one] { $n } link esterno verificato.
       *[other] { $n } link esterni verificati.
    }
rules_not_evaluated = Le regole che richiedono il grafo completo dei link non sono state valutate: { $rules }.
results_title = Risultati
label_urls = URL
label_html_pages = Pagine HTML
label_links = Link
note_internal = { $n ->
        [one] { $n } interno
       *[other] { $n } interni
    }
note_indexable = { $n ->
        [one] { $n } indicizzabile
       *[other] { $n } indicizzabili
    }
heading_status_codes = Codici di stato
label_no_response = nessuna risposta
note_request_failed = la richiesta non è riuscita
label_never_requested = Mai richiesti
note_never_requested = (rilevati, ma la scansione si è fermata prima di recuperarli)
label_out_of_scope = Fuori ambito
note_out_of_scope = (linkati da pagine verificate, ma fuori dall'ambito di questa scansione)
heading_non_indexable = Non indicizzabili, perché
heading_findings = Rilievi
no_findings = Nessun rilievo.
heading_excluded = Esclusi
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = { $k ->
        [one] { $k } problema di template ({ $pages } pagine)
       *[other] { $k } problemi di template ({ $pages } pagine)
    }
plus_more_findings = {" "}+ { $n ->
        [one] { $n } altro rilievo
       *[other] altri { $n } rilievi
    }
example_urls = es. { $urls }

## Pervasive rules, reworded

pervasive_note = { $pct }% del sito
deep_pages_summary = { $pages } pagine a più di { $max } clic — { $pct }% del sito (profondità tipica { $low }–{ $high }, massima { $deepest })
rule_deep_sorted = Ordinate per profondità di clic, le più profonde per prime; la prima colonna indica i clic dalla home.
hint_full_lists = Elenco completo degli URL di una regola: { $cmd }

## One rule's listing (`report --rule`)

rule_affected_urls = { $n ->
        [one] { $n } URL interessato
       *[other] { $n } URL interessati
    }
rule_template_group = Gruppo di template — { $n ->
        [one] { $n } pagina
       *[other] { $n } pagine
    }
rule_group_cause = causa: { $detail }
rule_other_pages = Altre pagine interessate — { $n }
rule_no_findings = Nessun rilievo per { $rule } in questa scansione.
hint_who_links = Un link non funzionante si corregge nella pagina che lo contiene. Chi linka a un URL, con il suo anchor text:
error_unknown_rule = { $rule } non è un ID di regola. Il catalogo si visualizza con `crawlforge rules`.
error_all_blocked_by_robots = robots.txt blocca tutti gli URL: non c'è nulla da verificare. Se il sito è tuo, puoi riprovare con --ignore-robots.
error_all_excluded_by_pattern = tutti gli URL sono stati scartati dai tuoi pattern --exclude/--include: non c'è nulla da verificare. Controlla i pattern.
error_no_urls_fetched = la scansione non ha recuperato nessun URL. Controlla la destinazione.
error_dead_seed = impossibile connettersi a { $url } ({ $cause }). Controlla l'URL.
cause_dns = il dominio non esiste o non si risolve
cause_tls = il certificato TLS non è valido
cause_timeout = il server non ha risposto in tempo
cause_connection = connessione rifiutata
cause_no_response = nessuna risposta
warn_base_mismatch = Attenzione: { $n } canonical su { $total } puntano a { $origin }, non alla base indicata ({ $base }).
    I risultati di indicizzabilità dipendono da questa impostazione: se il sito è pubblicato su
    { $origin }, ripeti l'audit con --base { $origin }

## One URL's sheet (`inspect.rs`)

inspect_error_not_found = { $url } non fa parte di questa scansione.
inspect_closest = Gli URL più vicini nel file sono:
inspect_status_title = Stato
label_http_status = Stato HTTP
label_content_type = Tipo di contenuto
label_response_time = Tempo di risposta
label_depth = Profondità
note_depth_clicks = clic dall'URL di partenza
label_in_sitemap = Nella sitemap
word_yes = sì
word_no = no
label_crawl_state = Stato di scansione
state_note_pending = rilevato, ma la scansione non l'ha mai recuperato
state_note_excluded = escluso dalla scansione ({ $reason })
state_note_external_checked = URL esterno — stato verificato, contenuto non analizzato
state_note_external_not_checked = URL esterno — stato non verificato ({ $reason })
state_note_external_unchecked = URL esterno — mai richiesto
state_note_out_of_scope = rilevato, ma fuori dall'ambito di questa scansione
state_note_error = la richiesta non è riuscita ({ $detail })
region_unknown = sconosciuta
inspect_page_title = Pagina
label_page_title = Titolo
label_meta_description = Meta description
label_h1 = H1
label_word_count = Numero di parole
label_canonical = Canonical
canonical_self = se stesso
label_indexable = Indicizzabile
indexable_no = no — { $reason }
chars_suffix = { $n ->
        [one] { $n } carattere
       *[other] { $n } caratteri
    }
inspect_redirects_title = Catena di reindirizzamenti
inlinks_title = Link in entrata ({ $n })
inlinks_by_region = Per area: { $breakdown } · { $nofollow } nofollow
inlinks_none = Nessuna pagina scansionata linka a questo URL.
inlinks_sample_heading = Pagine che linkano, prima i link nel contenuto:
no_anchor = (nessun anchor text)
inspect_truncated = … { $n } mostrate — elenco completo: { $cmd }
images_title = Immagini ({ $n })
image_no_alt = (senza alt)
image_usage_title = Usata come immagine
# Outbound links header. The three variables are counts.
outlinks_title = Link in uscita ({ $total }: { $internal } interni, { $external } esterni)
# Suffix that keeps external URLs apart from the status-code count.
note_external_status = +{ $n ->
        [one] { $n } esterno
       *[other] { $n } esterni
    }
image_usage_line = incorporata { $times ->
        [one] { $times } volta
       *[other] { $times } volte
    } in { $pages ->
        [one] { $pages } pagina
       *[other] { $pages } pagine
    }

## Crawl comparison (`diff.rs`)

diff_title = Confronto tra scansioni
label_before = Prima
label_after = Dopo
label_site = Sito
label_common = In comune
diff_crawl_counts = { $urls } URL, { $findings ->
        [one] { $findings } rilievo
       *[other] { $findings } rilievi
    }
diff_common_note = { $n ->
        [one] { $n } URL presente in entrambe le scansioni
       *[other] { $n } URL presenti in entrambe le scansioni
    }
warnings_title = Avvisi
tag_inconclusive = NON CONCLUSIVO
tag_warning = AVVISO
side_before = prima
side_after = dopo
warn_truncated = La scansione «{ $side }» è troncata. Non si può affermare cosa sia sparito o sia stato risolto: la scansione potrebbe semplicemente non esserci arrivata.
warn_truncated_by = La scansione «{ $side }» è troncata da { $reason }. Non si può affermare cosa sia sparito o sia stato risolto: la scansione potrebbe semplicemente non esserci arrivata.
warn_list_mode = La scansione «{ $side }» è in modalità elenco: ha visto solo gli URL che le sono stati forniti. Non si può affermare cosa sia sparito o sia stato risolto.
warn_unfinished = La scansione «{ $side }» non è terminata (stato «{ $status }»). Le sue assenze non significano nulla.
warn_rules_changed = Il catalogo delle regole è cambiato ({ $before } → { $after }). Alcuni dei rilievi nuovi o risolti potrebbero dipendere dalle regole, non dal sito.
warn_config_changed = La configurazione della scansione è cambiata in: { $fields }. Se non lo tieni presente, i cambi di ambito verranno attribuiti al sito.
warn_scope_changed = Gli URL di base sono diversi: { $before } → { $after }. Stesso sito, punto di partenza diverso.
warn_mode_changed = Le modalità di scansione sono diverse: { $before } → { $after }.
warn_missing_table = La scansione «{ $side }» è precedente alla tabella `{ $table }`: quel confronto viene saltato.
warn_order_inverted = La scansione indicata come «dopo» ({ $after_started }) è iniziata prima di quella indicata come «prima» ({ $before_started }). Se non è voluto, inverti i due argomenti: «Peggiorato» e «Migliorato» risultano invertiti.
worse_title = Peggiorato
label_new_findings = Nuovi rilievi
more_examples = … e altri { $n }
label_pages_lost_index = Pagine non più indicizzabili
label_status_worse = Codici di stato peggiorati
robots_content_changed = { $host }: il contenuto è cambiato
robots_blocks_all = ATTENZIONE: ora blocca la scansione dell'intero sito.
nothing_worse = Niente. Nessun nuovo rilievo, nessuna pagina uscita dall'indice.
better_title = Migliorato
label_findings_resolved = Rilievi risolti
label_pages_indexable_again = Pagine di nuovo indicizzabili
label_status_better = Codici di stato migliorati
other_title = Altre modifiche
label_new_urls = Nuovi URL
label_urls_gone = URL scomparsi
label_titles_changed = Titoli modificati
label_meta_changed = Meta description modificate
label_canonicals_changed = Canonical modificati
label_findings_persisted = Rilievi ancora presenti
suppressed_title = Non si può affermare
# Multi-line: the caller prints each line with its indentation.
suppressed_intro = Una delle due scansioni è incompleta, quindi queste differenze potrebbero dipendere da ciò
    che non è mai stato scansionato anziché dal sito. Non vengono conteggiate come modifiche:
label_candidate_new_urls = Possibili nuovi URL
label_candidate_urls_gone = Possibili URL scomparsi
label_candidate_new_findings = Possibili nuovi rilievi
label_candidate_resolved = Possibili rilievi risolti
suppressed_advice = Ripeti la scansione di entrambi i lati con lo stesso limite per ottenere un confronto affidabile.
gate_title = Controllo CI
# Multi-line: the caller prints each line with its indentation.
gate_inconclusive = La scansione «prima» è incompleta: non si possono affermare nuovi rilievi e
    --fail-on non si pronuncia. Non è un esito positivo.
gate_fail_detail = { $n ->
        [one] { $n } nuovo rilievo
       *[other] { $n } nuovi rilievi
    } [{ $severity }] — corrisponde a --fail-on { $token }
gate_pass = PASS   nulla di ciò che è monitorato compare come nuovo rilievo.

## Reopening a saved diff and listing its rows (`diff_file.rs`)

label_saved = Salvato
diff_saved_note = confronto generato il { $generated_at } da crawlforge { $version } · { $path }
# Multi-line: the caller prints each line with its indentation.
hint_diff_full_lists = Gli elenchi qui sopra mostrano solo alcuni esempi. Per tutte le righe, aggiungi --rule <ID> o
    --change <type> allo stesso comando (es. --change url_removed).
warn_unknown_change_types = { $n ->
        [one] { $n } modifica di un tipo che questo crawlforge non conosce ({ $types }) è stata esclusa.
       *[other] { $n } modifiche di un tipo che questo crawlforge non conosce ({ $types }) sono state escluse.
    } Il file è stato scritto da una versione più recente.
label_status_changed = Codici di stato modificati
diff_no_matching_changes = Nessuna modifica di questo confronto corrisponde.
error_diff_missing = { $path } non esiste. I file di confronto vengono salvati da `crawlforge diff <before> <after> --out <file>`.
error_diff_needs_two = { $path } è un file di scansione, e un confronto ne richiede due: la scansione precedente e quella successiva. Un singolo file è accettato solo se è un confronto salvato con --out.
hint_diff_prev = La sua scansione precedente è accanto:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } non è né un file di scansione né un file di confronto. I file di confronto vengono salvati da `crawlforge diff <before> <after> --out <file>`.
error_diff_too_new = { $path } è un file di confronto in formato { $version }, più recente di quanto questo crawlforge comprenda ({ $supported }). Aggiorna crawlforge per aprirlo.
error_diff_reopen_flag = { $flag } si applica quando si confrontano due scansioni; un confronto salvato viene solo letto. Eseguilo sulle scansioni: crawlforge diff <before> <after> { $flag } …
error_unknown_change_type = { $value } non è un tipo di modifica. Disponibili: { $available }.
error_rule_with_change = --rule filtra i rilievi, quindi si combina solo con --change issue_appeared o issue_resolved, non con { $change }.
site_wide_finding = (rilievo a livello di sito)
site_placeholder = (sito)
error_missing_file = { $path } non esiste
error_store_missing = { $path } non esiste. I file di scansione vengono prodotti da `crawlforge crawl`, `crawlforge audit` e `crawlforge list`.
error_not_a_crawl = { $path } non sembra un file di scansione di CrawlForge
error_different_sites = le due scansioni riguardano siti diversi: { $a } e { $b }.
    Gli URL vengono abbinati in base al testo, quindi il confronto direbbe che l'intero sito è sparito e ne è comparso un altro. Confronta ogni sito con se stesso.
error_fail_on_unknown = --fail-on: «{ $token }» non è né una regola né una gravità.
    Le gravità sono critical, high, medium, low e info; gli ID delle regole si visualizzano con `crawlforge rules`.

## Portfolio panel (`portfolio.rs`)

portfolio_title = Pannello del portfolio
portfolio_range = scansioni dal { $oldest } al { $newest }
# Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
# changed" would silently mean a different period on every site.
warn_portfolio_date_spread = La scansione più vecchia ({ $oldest }) e la più recente ({ $newest }) distano { $days } giorni: questo pannello non è un'istantanea del portfolio, e «cosa è cambiato» copre un periodo diverso per ogni sito.
# Trap 3.2: two files crawled with different catalogs are not comparable without
# saying so — a rule can be "missing" on a site because it did not exist yet.
warn_portfolio_rules_versions = Non tutti i siti sono stati scansionati con lo stesso catalogo di regole ({ $versions }). Una regola può mancare su un sito perché non esisteva quando quel sito è stato scansionato.
warn_portfolio_core_versions = Le scansioni provengono da versioni diverse del motore ({ $versions }).
portfolio_skipped_title = File scartati
# A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
portfolio_prev_not_input = è un .prev.sqlite: il «prima» della scansione accanto, già usato dal confronto. Non conta come sito.
error_portfolio_newer_schema = il suo schema (v{ $version }) è più recente di quanto questa build comprenda (v{ $supported }). Aggiorna crawlforge per aprirlo.
error_portfolio_no_sites = nei percorsi indicati non è stato trovato nessun file di scansione utilizzabile come portfolio. I file di scansione vengono prodotti da `crawlforge crawl`, `crawlforge audit` e `crawlforge list`; in una cartella vengono cercati i file *.sqlite.
error_portfolio_tier = il pannello del portfolio non fa parte del piano gratuito: richiede Pro. I comandi per un singolo sito (`report`, `diff`, `export`) restano disponibili.
error_portfolio_too_many = il portfolio ha { $n } siti e questo piano lo limita a { $max }.
portfolio_changes_title = Cosa è cambiato
portfolio_no_pairs = Nessun sito ha una scansione precedente accanto. Ripeti la scansione sullo stesso file di output e quella precedente viene conservata come .prev.sqlite: è il «prima» con cui si confronta questa sezione.
portfolio_new_critical_high = Nuovi rilievi di gravità critica e alta
portfolio_none_critical_high = Nessun nuovo rilievo di gravità critica o alta nei siti confrontati.
portfolio_rest_title = Il resto, sito per sito
portfolio_no_changes = nessuna modifica
# The diff of that pair cannot assert absences: one side is incomplete.
portfolio_pair_inconclusive = uno dei due lati è incompleto: non si può affermare cosa sia sparito o sia stato risolto
portfolio_pair_failed = il confronto con la scansione precedente non è riuscito: { $reason }
portfolio_spread_title = Problemi diffusi nel portfolio
portfolio_spread_intro = Una regola che scatta sulla maggior parte dei siti raramente dipende dai contenuti: di solito è un template o un plugin condiviso, e una sola correzione li sistema tutti.
portfolio_spread_none = Nessuna regola scatta su nessun sito del portfolio.
portfolio_glance_title = Il portfolio in sintesi
th_site = sito
th_crawled = scansione
th_indexable = index.
th_sev_critical = crit.
th_sev_high = alta
th_sev_medium = media
th_sev_low = bassa
th_sev_info = info
flag_truncated = (troncata)
flag_list_mode = (modalità elenco)
flag_unfinished = (non terminata: { $status })
hint_site_diff = Confronto completo di un sito:
portfolio_html_title = Pannello del portfolio
portfolio_sites_count = { $n ->
        [one] { $n } sito
       *[other] { $n } siti
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = salvata nel file di scansione
rules_config_source_after = salvata nella scansione più recente
rules_config_source_file = da { $path }
warn_rules_config_differs = Le due scansioni salvano configurazioni di regole diverse. Entrambi i lati sono stati valutati con la stessa ({ $source }), così un cambio di configurazione non compare come rilievi nuovi o risolti.
warn_portfolio_rules_config = Alcuni siti vengono letti con la propria configurazione di regole (regole disattivate / gravità modificate): { $sites }. I loro conteggi non sono confrontabili regola per regola con gli altri.
# "Rule configuration (stored in the crawl file): 2 rules disabled, 1 severity changed."
rules_config_note = Configurazione delle regole ({ $source }): { $disabled ->
        [one] { $disabled } regola disattivata
       *[other] { $disabled } regole disattivate
    }, { $changed ->
        [one] { $changed } gravità modificata
       *[other] { $changed } gravità modificate
    }. Il file di scansione conserva tutti i rilievi.
# "9 of 12 sites": the numerator of the spread table. The noun follows $total.
portfolio_sites_of = { $fired } su { $total ->
        [one] { $total } sito
       *[other] { $total } siti
    }
# " (2 inconclusive)": sites where the rule could not be evaluated. Starts with a space.
# Never shown for zero.
portfolio_inconclusive_suffix = {" "}({ $n ->
        [one] { $n } non conclusivo
       *[other] { $n } non conclusivi
    })
# The noun follows $total and the verb follows $pairs: "1 su 5 siti ha".
portfolio_pairs_line = { $pairs } su { $total ->
        [one] { $total } sito
       *[other] { $total } siti
    } { $pairs ->
        [one] ha
       *[other] hanno
    } una scansione precedente (.prev.sqlite) con cui confrontarsi.

## Telling files apart (`store_check.rs`)

error_store_is_diff = { $path } non è un file di scansione: è un file di confronto, del tipo che salva `crawlforge diff --out`. Aprilo con `crawlforge diff { $path }`.
    I file di scansione vengono prodotti da `crawlforge crawl` e `crawlforge audit`.
error_store_foreign = { $path } non è un file di scansione di CrawlForge: non ne ha le tabelle. Viene da un altro programma?
    I file di scansione vengono prodotti da `crawlforge crawl` e `crawlforge audit`.
error_store_not_sqlite = { $path } non è un file di CrawlForge: non è nemmeno un database SQLite.
    I file di scansione vengono prodotti da `crawlforge crawl` e `crawlforge audit`.

## Markdown/HTML report (`audit_report.rs`)

report_audit_of = Audit di
report_audit_of_sites = Audit di { $n } siti: { $sites }
sites_and_more = { $sites } e altri { $n }
report_mode_line = Modalità `{ $mode }` · avviata il { $started }
report_truncated_note = > **Scansione troncata** (`{ $reason }`). I conteggi coprono ciò che è stato scansionato fino a quel punto, non l'intero sito.
report_list_mode_note = > **Scansione in modalità elenco.** Sono stati verificati solo gli URL elencati: i conteggi descrivono quell'insieme, non l'intero sito, e le regole che richiedono il grafo completo dei link del sito non vengono valutate.
th_metric = Metrica
th_value = Valore
row_pages = Pagine
row_indexable = Indicizzabili
row_errors_4xx_5xx = 4xx/5xx
report_more = … altri { $n }
report_more_run = … altri { $n } — esegui: { $cmd }
report_full_list_run = … elenco completo: { $cmd }
html_title = Audit SEO

## Rule catalogue (`rules.rs`)

th_severity = GRAVITÀ
th_category = CATEGORIA
th_scope = AMBITO
th_tier = PIANO
th_name = NOME
lbl_category = categoria
lbl_scope = ambito
lbl_tier = piano
lbl_reference = riferimento
scope_page = pagina
scope_site = sito
rules_summary = { $total } regole: { $free } nel piano gratuito, { $page } a livello di pagina e { $site } a livello di sito.
error_unknown_category = nessuna regola nella categoria «{ $category }». Le categorie sono: { $list }.

## `main.rs`

crawling = Scansione di { $target }
error_invalid_seed = { $url } non è un URL scansionabile: è previsto qualcosa come https://example.com/. Non è stato creato nulla.
file_line = File:       { $path }
previous_kept = Precedente: { $path } (conservato, dall'ultima esecuzione)
report_written = Report scritto in { $path }
markdown_report_written = Report Markdown scritto in { $path }
exported_csv = { $n ->
        [one] { $n } file CSV esportato in { $dir }
       *[other] { $n } file CSV esportati in { $dir }
    }
exported_sheets = { $n ->
        [one] { $n } foglio esportato in { $path }
       *[other] { $n } fogli esportati in { $path }
    }
bench_appended = Misurazione delle prestazioni aggiunta a { $path }
next_steps_title = Passi successivi
next_explain = Spiegare ogni rilievo, con gli URL interessati:
next_spreadsheet = Aprire tutti i dati in un foglio di calcolo:
next_compare_previous = Confrontare con l'esecuzione precedente di questa scansione:
next_compare_later = Ripetere questa stessa scansione dopo il prossimo deploy, poi confrontare le due:
next_reread = Rileggere questo riepilogo in qualsiasi momento:

## HTTP Basic authentication (protected staging)

auth_url_note = Autenticazione HTTP Basic presa dall'URL: verrà inviata solo a { $host } e non verrà salvata nel file di scansione.
auth_env_note = Autenticazione HTTP Basic presa da CRAWLFORGE_AUTH: verrà inviata solo a { $host } e non verrà salvata nel file di scansione.
auth_base_ignored = Attenzione: le credenziali in --base sono state rimosse: l'audit di una cartella legge dal disco e non effettua richieste HTTP.
auth_list_ignored = Attenzione: le credenziali all'interno delle voci dell'elenco di URL vengono ignorate e rimosse. Per autenticare la scansione, imposta CRAWLFORGE_AUTH=user:password; si applica solo all'host del primo URL.
progress_sitemaps = ricerca delle sitemap…
progress_finalize = passaggio finale: link in entrata e regole a livello di sito…
progress_crawl = { $fetched } scansionati · { $queued } in coda · { $rate } URL/s · { $findings } rilievi
progress_failed_suffix = {" "}· { $n ->
        [one] { $n } non riuscito
       *[other] { $n } non riusciti
    }

## Resuming an interrupted crawl (`main.rs`, `report.rs`)

resuming = Ripresa della scansione di { $target }
resume_counts = Si riprende da dove si era fermata: { $done } URL già scansionati, { $pending } ancora in sospeso.
resume_config_note = Viene usata la configurazione salvata nel file: valgono le impostazioni dell'esecuzione originale.
error_resume_schema_blocking = { $file } è una scansione con schema v{ $version }, e la migrazione { $blocking } cambia ciò che scrive il motore: metà direbbe una cosa e metà un'altra. Ripeti la scansione. Puoi comunque aprire questo file con `report`, `export` e `diff`.
resume_robots_not_inherited = Nota: la scansione originale ignorava robots.txt. Questa no: quel permesso lo concedi tu, non un file. Riesegui il comando di scansione originale se ti serve.
resume_finished = Scansione ripresa e completata: altri { $urls } URL in { $secs } s
interrupt_flushing = Interrotto: salvataggio di ciò che è già stato scansionato… (Ctrl+C di nuovo per uscire senza attendere)
interrupt_after_done = Interrotto. Il file di scansione è completo; potrebbe restare un'esportazione scritta a metà.
warn_wal_kept = Nota: un altro programma ha { $path } aperto, quindi resta in modalità WAL. I file -wal e -shm accanto fanno parte di questa scansione: copiali tutti e tre insieme, oppure chiudi l'altro programma ed esegui `crawlforge report` sul file per reintegrarli.
error_store_locked = un altro processo crawlforge sta scrivendo { $path } in questo momento. Attendi che finisca, o fermalo, prima di scansionare o riprendere questo file.
interrupted_saved = Scansione interrotta. Tutto ciò che è stato scansionato finora è salvato in { $path }. Per continuarla:
hint_previous_unfinished = La scansione precedente non era terminata. Per continuarla invece di ricominciare:
error_resume_done = { $path } è una scansione terminata: non c'è nulla da riprendere.
    Per verificare di nuovo il sito, esegui il comando di scansione originale; il file terminato verrà conservato accanto come .prev.sqlite.
error_resume_status = { $path } non può essere ripresa: il suo stato è «{ $status }». Si possono riprendere solo le scansioni interrotte (stato «running» o «paused»).
error_resume_schema = { $path } usa lo schema v{ $found } e questa build scrive v{ $expected }: una scansione interrotta può essere ripresa solo dalla versione che l'ha avviata. Ripeti la scansione.
error_resume_config = la configurazione di scansione salvata in { $path } non è leggibile. Il file è stato scritto da un'altra versione? Ripeti la scansione.
