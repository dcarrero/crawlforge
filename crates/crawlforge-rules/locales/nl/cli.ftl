### CrawlForge — meldingen van de opdrachtregel (Nederlands).
###
### Vertaling van locales/en/cli.ftl. Engels is de bron en is leidend.
### CONTRIBUTING.md, "Adding or improving a language", legt de regels uit.

## Severity names

severity_label_critical = Kritiek
severity_label_high = Hoog
severity_label_medium = Gemiddeld
severity_label_low = Laag
severity_label_info = Info
severity_word_critical = kritiek
severity_word_high = hoog
severity_word_medium = gemiddeld
severity_word_low = laag
severity_word_info = info

## Non-indexability reasons

reason_noindex = noindex
reason_canonicalised = gecanonicaliseerd
reason_robots = robots
reason_redirect = doorverwijzing
reason_4xx = 4xx
reason_5xx = 5xx
reason_not_html = geen HTML

## Exclusion reasons

exclusion_robots = robots.txt
exclusion_nofollow = nofollow
exclusion_depth = maximale klikdiepte
exclusion_pattern = URL-patroon
exclusion_limit = URL-limiet
exclusion_local_network = lokaal netwerk
exclusion_external_check_off = externe controle uit
exclusion_no_network = audit van een map, geen netwerk

## Crawl summary (`report.rs`)

crawl_finished = Crawl voltooid: { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URL's
    } in { $secs } s
crawl_failed_suffix = {" "}({ $n } mislukt)
crawl_truncated = Crawl afgebroken door { $reason }. De getoonde bevindingen zijn die tot dat moment zijn gevonden.
crawl_rate_limited_note = De site heeft de crawl afgeremd: { $n ->
        [one] { $n } pagina bleef 429 Too Many Requests geven, of vroeg om meer dan twee minuten te wachten, en blijft in de wachtrij
       *[other] { $n } pagina's bleven 429 Too Many Requests geven, of vroegen om meer dan twee minuten te wachten, en blijven in de wachtrij
    }, in plaats van als kapot te worden gemeld. Regels die de hele site nodig hebben, worden pas geëvalueerd als alles is opgehaald. Voer later uit:
    {"    "}crawlforge resume { $path }
    of crawl opnieuw met een lagere --concurrency, of laat de user-agent van CrawlForge door de firewall van de site.
report_rate_limited_note = > **De site heeft de crawl afgeremd.** { $n ->
        [one] { $n } pagina bleef 429 Too Many Requests geven, of vroeg om meer dan twee minuten te wachten, en bleef in de wachtrij
       *[other] { $n } pagina's bleven 429 Too Many Requests geven, of vroegen om meer dan twee minuten te wachten, en bleven in de wachtrij
    }: zulke pagina's worden niet meegeteld of als kapot gemeld, en regels die de hele site nodig hebben, worden niet geëvalueerd. `crawlforge resume` haalt ze op.
crawl_throttled_note = De server raakte overbelast (429, 503, verbroken verbindingen of time-outs), dus de crawl ging langzamer: de gelijktijdigheid is { $n ->
        [one] één keer
       *[other] { $n } keer
    } gehalveerd. Probeer de volgende keer een lagere --concurrency.
crawl_list_mode_note = Lijstcrawl: alleen de URL's die je hebt opgegeven, zijn geaudit. Regels die de volledige linkstructuur van de site nodig hebben (weespagina's, inkomende links, klikdiepte), worden niet geëvalueerd.
external_unchecked = { $n ->
        [one] { $n } externe link is
       *[other] { $n } externe links zijn
    } niet gecontroleerd: de limiet max_external is bereikt. Verhoog die met --max-external.
external_unregistered = { $n ->
        [one] { $n } externe link is
       *[other] { $n } externe links zijn
    } niet eens vastgelegd: de limiet max_external_urls is bereikt. Die limiet is bedoeld voor één pagina die naar honderdduizenden hosts linkt.
external_never_checked = { $n ->
        [one] { $n } externe link is vastgelegd maar nooit gecontroleerd.
       *[other] { $n } externe links zijn vastgelegd maar nooit gecontroleerd.
    }
external_offline = { $n ->
        [one] { $n } externe link is
       *[other] { $n } externe links zijn
    } niet gecontroleerd: een audit van een map doet geen netwerkverzoeken. Crawl de gepubliceerde site met `crawlforge crawl <URL>` om ze te controleren.
external_check_disabled = Externe links zijn niet gecontroleerd (--no-external-check): dat dit rapport geen kapotte externe links bevat, betekent niet dat ze er niet zijn.
external_checked_note = { $n ->
        [one] { $n } externe link gecontroleerd.
       *[other] { $n } externe links gecontroleerd.
    }
rules_not_evaluated = Regels die de volledige linkstructuur nodig hebben, zijn niet geëvalueerd: { $rules }.
results_title = Resultaten
label_urls = URL's
label_html_pages = HTML-pagina's
label_links = Links
note_internal = { $n } intern
note_indexable = { $n } indexeerbaar
heading_status_codes = Statuscodes
label_no_response = geen antwoord
note_request_failed = het verzoek is mislukt
label_never_requested = Nooit opgevraagd
note_never_requested = (gevonden, maar de crawl stopte voordat ze werden opgehaald)
label_out_of_scope = Buiten bereik
note_out_of_scope = (gelinkt vanaf geaudite pagina's, maar buiten het bereik van deze crawl)
heading_non_indexable = Niet indexeerbaar, waarom
heading_findings = Bevindingen
no_findings = Geen bevindingen.
heading_excluded = Uitgesloten
template_issues = { $k ->
        [one] { $k } templateprobleem ({ $pages } pagina's)
       *[other] { $k } templateproblemen ({ $pages } pagina's)
    }
plus_more_findings = {" "}+ nog { $n ->
        [one] { $n } bevinding
       *[other] { $n } bevindingen
    }
example_urls = bijv. { $urls }

## Dominant rules

pervasive_note = { $pct }% van de site
deep_pages_summary = { $pages } pagina's dieper dan { $max } klikken — { $pct }% van de site (gebruikelijke klikdiepte { $low }–{ $high }, diepste { $deepest })
rule_deep_sorted = Gesorteerd op klikdiepte, diepste eerst; de eerste kolom is het aantal klikken vanaf de homepage.
hint_full_lists = Volledige URL-lijst van een regel: { $cmd }

## One rule (`report --rule`)

rule_affected_urls = { $n ->
        [one] { $n } getroffen URL
       *[other] { $n } getroffen URL's
    }
rule_template_group = Templategroep — { $n ->
        [one] { $n } pagina
       *[other] { $n } pagina's
    }
rule_group_cause = oorzaak: { $detail }
rule_other_pages = Andere getroffen pagina's — { $n }
rule_no_findings = Geen bevindingen voor { $rule } in deze crawl.
hint_who_links = Een kapotte link herstel je op de pagina die ernaar linkt. Wie naar een URL linkt, met ankertekst:
error_unknown_rule = { $rule } is geen regel-ID. De catalogus staat in `crawlforge rules`.
error_all_blocked_by_robots = robots.txt blokkeert elke URL: er valt niets te auditen. Als de site van jou is, kun je het opnieuw proberen met --ignore-robots.
error_all_excluded_by_pattern = elke URL is overgeslagen door je --exclude/--include-patronen: er valt niets te auditen. Controleer de patronen.
error_no_urls_fetched = de crawl heeft geen enkele URL opgehaald. Controleer het doel.
error_dead_seed = kan geen verbinding maken met { $url } ({ $cause }). Controleer de URL.
cause_dns = het domein bestaat niet of wordt niet omgezet
cause_tls = het TLS-certificaat is ongeldig
cause_timeout = de server antwoordde niet op tijd
cause_connection = verbinding geweigerd
cause_no_response = geen antwoord
warn_base_mismatch = Waarschuwing: { $n } van de { $total } canonicals { $n ->
        [one] wijst
       *[other] wijzen
    } naar { $origin }, niet naar de opgegeven basis ({ $base }).
    De resultaten over indexeerbaarheid hangen van die instelling af: als de site gepubliceerd is op
    { $origin }, herhaal de audit dan met --base { $origin }

## URL card (`inspect.rs`)

inspect_error_not_found = { $url } zit niet in deze crawl.
inspect_closest = De meest gelijkende URL's in het bestand zijn:
inspect_status_title = Status
label_http_status = HTTP-status
label_content_type = Content-type
label_response_time = Responstijd
label_depth = Klikdiepte
note_depth_clicks = klikken vanaf de start-URL
label_in_sitemap = In sitemap
word_yes = ja
word_no = nee
label_crawl_state = Crawlstatus
state_note_pending = gevonden, maar nooit opgehaald door de crawl
state_note_excluded = uitgesloten van de crawl ({ $reason })
state_note_external_checked = externe URL — status gecontroleerd, content niet geaudit
state_note_external_not_checked = externe URL — status niet gecontroleerd ({ $reason })
state_note_external_unchecked = externe URL — nooit opgevraagd
state_note_out_of_scope = gevonden, maar buiten het bereik van deze crawl
state_note_error = het verzoek is mislukt ({ $detail })
region_unknown = onbekend
inspect_page_title = Pagina
label_page_title = Title
label_meta_description = Meta description
label_h1 = H1
label_word_count = Aantal woorden
label_canonical = Canonical
canonical_self = zelfverwijzend
label_indexable = Indexeerbaar
indexable_no = nee — { $reason }
chars_suffix = { $n ->
        [one] { $n } teken
       *[other] { $n } tekens
    }
inspect_redirects_title = Redirectketen
inlinks_title = Inkomende links ({ $n })
inlinks_by_region = Per paginadeel: { $breakdown } · { $nofollow } nofollow
inlinks_none = Geen enkele gecrawlde pagina linkt naar deze URL.
inlinks_sample_heading = Linkende pagina's, contentlinks eerst:
no_anchor = (geen ankertekst)
inspect_truncated = … { $n } getoond — volledige lijst: { $cmd }
images_title = Afbeeldingen ({ $n })
image_no_alt = (geen alt)
image_usage_title = Gebruikt als afbeelding
outlinks_title = Uitgaande links ({ $total }: { $internal } intern, { $external } extern)
note_external_status = +{ $n } extern
image_usage_line = { $times } keer ingesloten op { $pages ->
        [one] { $pages } pagina
       *[other] { $pages } pagina's
    }

## Crawl comparison (`diff.rs`)

diff_title = Crawlvergelijking
label_before = Voor
label_after = Na
label_site = Site
label_common = Gedeeld
diff_crawl_counts = { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URL's
    }, { $findings ->
        [one] { $findings } bevinding
       *[other] { $findings } bevindingen
    }
diff_common_note = { $n ->
        [one] { $n } URL komt in beide crawls voor
       *[other] { $n } URL's komen in beide crawls voor
    }
warnings_title = Waarschuwingen
tag_inconclusive = NIET SLUITEND
tag_warning = WAARSCHUWING
side_before = voor
side_after = na
warn_truncated = De crawl ‘{ $side }’ is afgebroken. Wat verdwenen of opgelost is, valt niet vast te stellen: de crawl is er misschien gewoon niet aan toegekomen.
warn_truncated_by = De crawl ‘{ $side }’ is afgebroken door { $reason }. Wat verdwenen of opgelost is, valt niet vast te stellen: de crawl is er misschien gewoon niet aan toegekomen.
warn_list_mode = De crawl ‘{ $side }’ is een lijstcrawl: hij zag alleen de URL's die hij kreeg. Wat verdwenen of opgelost is, valt niet vast te stellen.
warn_unfinished = De crawl ‘{ $side }’ is niet voltooid (status ‘{ $status }’). Wat erin ontbreekt, zegt niets.
warn_rules_changed = De regelcatalogus is gewijzigd ({ $before } → { $after }). Een deel van de nieuwe of opgeloste bevindingen kan van de regels komen, niet van de site.
warn_config_changed = De crawlconfiguratie is gewijzigd in: { $fields }. Wie daar geen rekening mee houdt, schrijft wijzigingen in het crawlbereik toe aan de site.
warn_scope_changed = De basis-URL's verschillen: { $before } → { $after }. Dezelfde site, een ander startpunt.
warn_mode_changed = De crawlmodi verschillen: { $before } → { $after }.
warn_missing_table = De crawl ‘{ $side }’ is ouder dan de tabel `{ $table }`: die vergelijking wordt overgeslagen.
warn_order_inverted = De crawl die als ‘na’ is opgegeven ({ $after_started }), begon eerder dan die als ‘voor’ ({ $before_started }). Als dat niet de bedoeling is, verwissel dan de twee argumenten: ‘Verslechterd’ en ‘Verbeterd’ komen anders omgekeerd uit.
worse_title = Verslechterd
label_new_findings = Nieuwe bevindingen
more_examples = … en nog { $n }
label_pages_lost_index = Niet meer indexeerbare pagina's
label_status_worse = Verslechterde statuscodes
robots_content_changed = { $host }: de inhoud is gewijzigd
robots_blocks_all = LET OP: robots.txt blokkeert nu het crawlen van de hele site.
nothing_worse = Niets. Geen nieuwe bevindingen, geen pagina's uit de index verdwenen.
better_title = Verbeterd
label_findings_resolved = Opgeloste bevindingen
label_pages_indexable_again = Weer indexeerbare pagina's
label_status_better = Verbeterde statuscodes
other_title = Overige wijzigingen
label_new_urls = Nieuwe URL's
label_urls_gone = Verdwenen URL's
label_titles_changed = Gewijzigde titles
label_meta_changed = Gewijzigde meta descriptions
label_canonicals_changed = Gewijzigde canonicals
label_findings_persisted = Nog aanwezige bevindingen
suppressed_title = Niet vast te stellen
suppressed_intro = Een van de twee crawls is onvolledig, dus deze verschillen kunnen komen van wat nooit is
    gecrawld in plaats van van de site. Ze tellen niet als wijzigingen:
label_candidate_new_urls = Mogelijk nieuwe URL's
label_candidate_urls_gone = Mogelijk verdwenen URL's
label_candidate_new_findings = Mogelijk nieuwe bevindingen
label_candidate_resolved = Mogelijk opgeloste bevindingen
suppressed_advice = Crawl beide opnieuw met dezelfde limiet voor een betrouwbare vergelijking.
gate_title = CI-gate
gate_inconclusive = De crawl ‘voor’ is onvolledig: nieuwe bevindingen vallen niet vast te stellen en
    --fail-on neemt geen standpunt in. Dit is geen PASS.
gate_fail_detail = { $n ->
        [one] { $n } nieuwe bevinding
       *[other] { $n } nieuwe bevindingen
    } [{ $severity }] — komt overeen met --fail-on { $token }
gate_pass = PASS   niets wat bewaakt wordt, duikt op als nieuwe bevinding.

## Saved diff (`diff_file.rs`)

label_saved = Opgeslagen
diff_saved_note = vergelijking gemaakt op { $generated_at } door crawlforge { $version } · { $path }
hint_diff_full_lists = De lijsten hierboven tonen maar een paar voorbeelden. Voeg voor alle rijen --rule <ID> of
    --change <type> toe aan hetzelfde commando (bijv. --change url_removed).
warn_unknown_change_types = { $n ->
        [one] { $n } wijziging van een type dat deze crawlforge niet kent ({ $types }), is weggelaten.
       *[other] { $n } wijzigingen van een type dat deze crawlforge niet kent ({ $types }), zijn weggelaten.
    } Het bestand is door een nieuwere versie geschreven.
label_status_changed = Gewijzigde statuscodes
diff_no_matching_changes = Geen enkele wijziging in deze vergelijking komt overeen.
error_diff_missing = { $path } bestaat niet. Vergelijkingsbestanden worden opgeslagen met `crawlforge diff <before> <after> --out <file>`.
error_diff_needs_two = { $path } is een crawlbestand, en vergelijken vraagt er twee: de eerdere crawl en de latere. Eén bestand wordt alleen geaccepteerd als het een met --out opgeslagen vergelijking is.
hint_diff_prev = De vorige crawl staat ernaast:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } is geen crawlbestand en ook geen vergelijkingsbestand. Vergelijkingsbestanden worden opgeslagen met `crawlforge diff <before> <after> --out <file>`.
error_diff_too_new = { $path } is een vergelijkingsbestand in formaat { $version }, nieuwer dan deze crawlforge begrijpt ({ $supported }). Werk crawlforge bij om het te openen.
error_diff_reopen_flag = { $flag } geldt bij het vergelijken van twee crawls; een opgeslagen vergelijking wordt alleen gelezen. Gebruik het op de crawls: crawlforge diff <before> <after> { $flag } …
error_unknown_change_type = { $value } is geen wijzigingstype. Beschikbaar: { $available }.
error_rule_with_change = --rule filtert bevindingen en is dus alleen te combineren met --change issue_appeared of issue_resolved, niet met { $change }.
site_wide_finding = (sitebrede bevinding)
site_placeholder = (site)
error_missing_file = { $path } bestaat niet
error_store_missing = { $path } bestaat niet. Crawlbestanden worden gemaakt door `crawlforge crawl`, `crawlforge audit` en `crawlforge list`.
error_not_a_crawl = { $path } lijkt geen crawlbestand van CrawlForge
error_different_sites = de twee crawls zijn van verschillende sites: { $a } en { $b }.
    URL's worden op hun tekst gekoppeld, dus de vergelijking zou zeggen dat de hele site is verdwenen en er een andere is verschenen. Vergelijk elke site met zichzelf.
error_fail_on_unknown = --fail-on: ‘{ $token }’ is geen regel en geen ernstniveau.
    De ernstniveaus zijn critical, high, medium, low en info; de regel-ID's staan in `crawlforge rules`.

## Portfolio (`portfolio.rs`)

portfolio_title = Portfolio-overzicht
portfolio_range = crawls van { $oldest } tot { $newest }
warn_portfolio_date_spread = De oudste crawl ({ $oldest }) en de nieuwste ({ $newest }) liggen { $days ->
        [one] { $days } dag
       *[other] { $days } dagen
    } uit elkaar: dit overzicht is geen momentopname van het portfolio, en ‘Wat is er veranderd’ beslaat per site een andere periode.
warn_portfolio_rules_versions = Niet elke site is met dezelfde regelcatalogus gecrawld ({ $versions }). Een regel kan op een site ontbreken omdat hij nog niet bestond toen die site werd gecrawld.
warn_portfolio_core_versions = De crawls komen van verschillende engineversies ({ $versions }).
portfolio_skipped_title = Overgeslagen bestanden
portfolio_prev_not_input = het is een .prev.sqlite: de ‘voor’ van de crawl ernaast, die de vergelijking al gebruikt. Het telt niet als site.
error_portfolio_newer_schema = het schema (v{ $version }) is nieuwer dan deze build begrijpt (v{ $supported }). Werk crawlforge bij om het te openen.
error_portfolio_no_sites = in de opgegeven paden zijn geen crawlbestanden gevonden die als portfolio bruikbaar zijn. Crawlbestanden worden gemaakt door `crawlforge crawl`, `crawlforge audit` en `crawlforge list`; in een map wordt gezocht naar *.sqlite-bestanden.
error_portfolio_tier = het portfolio-overzicht zit niet in het gratis pakket: daarvoor is Pro nodig. Commando's voor één site (`report`, `diff`, `export`) blijven beschikbaar.
error_portfolio_too_many = het portfolio telt { $n } sites en dit pakket staat er maximaal { $max } toe.
portfolio_changes_title = Wat is er veranderd
portfolio_no_pairs = Geen enkele site heeft een vorige crawl ernaast. Crawl opnieuw naar hetzelfde uitvoerbestand, dan blijft de vorige crawl bewaard als .prev.sqlite — dat is de ‘voor’ waarmee deze sectie vergelijkt.
portfolio_new_critical_high = Nieuwe bevindingen met ernst kritiek en hoog
portfolio_none_critical_high = Geen nieuwe bevindingen met ernst kritiek of hoog op de vergeleken sites.
portfolio_rest_title = De rest, site voor site
portfolio_no_changes = geen wijzigingen
portfolio_pair_inconclusive = een van de twee crawls is onvolledig: wat verdwenen of opgelost is, valt niet vast te stellen
portfolio_pair_failed = de vergelijking met de vorige crawl is mislukt: { $reason }
portfolio_spread_title = Problemen in het hele portfolio
portfolio_spread_intro = Een regel die op de meeste sites afgaat, ligt zelden aan de content: meestal is het een gedeelde template of plug-in — één oplossing die ze allemaal helpt.
portfolio_spread_none = Geen enkele regel gaat af op een site van het portfolio.
portfolio_glance_title = Het portfolio in één oogopslag
th_site = site
th_crawled = gecrawld
th_indexable = index.
th_sev_critical = krit.
th_sev_high = hoog
th_sev_medium = gem.
th_sev_low = laag
th_sev_info = info
flag_truncated = (afgebroken)
flag_list_mode = (lijstcrawl)
flag_unfinished = (niet voltooid: { $status })
hint_site_diff = Volledige vergelijking van één site:
portfolio_html_title = Portfolio-overzicht
portfolio_sites_count = { $n ->
        [one] { $n } site
       *[other] { $n } sites
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = opgeslagen in het crawlbestand
rules_config_source_after = opgeslagen in de latere crawl
rules_config_source_file = uit { $path }
warn_rules_config_differs = De twee crawls bevatten verschillende regelconfiguraties. Beide zijn met dezelfde geëvalueerd ({ $source }), zodat een configuratiewijziging niet als nieuwe of opgeloste bevindingen verschijnt.
warn_portfolio_rules_config = Sommige sites worden gelezen met hun eigen regelconfiguratie (regels uitgeschakeld / ernstniveaus gewijzigd): { $sites }. Hun aantallen zijn niet per regel vergelijkbaar met de rest.
rules_config_note = Regelconfiguratie ({ $source }): { $disabled ->
        [one] { $disabled } regel
       *[other] { $disabled } regels
    } uitgeschakeld, { $changed ->
        [one] { $changed } ernstniveau
       *[other] { $changed } ernstniveaus
    } gewijzigd. Het crawlbestand bewaart alle bevindingen.
portfolio_sites_of = { $fired } van { $total ->
        [one] { $total } site
       *[other] { $total } sites
    }
portfolio_inconclusive_suffix = {" "}({ $n } niet sluitend)
portfolio_pairs_line = { $pairs } van { $total ->
        [one] { $total } site
       *[other] { $total } sites
    } { $pairs ->
        [one] heeft
       *[other] hebben
    } een vorige crawl (.prev.sqlite) om mee te vergelijken.

## File identification (`store_check.rs`)

error_store_is_diff = { $path } is geen crawlbestand: het is een vergelijkingsbestand, zoals `crawlforge diff --out` dat opslaat. Open het met `crawlforge diff { $path }`.
    Crawlbestanden worden gemaakt door `crawlforge crawl` en `crawlforge audit`.
error_store_foreign = { $path } is geen crawlbestand van CrawlForge: de tabellen ontbreken. Komt het van een ander programma?
    Crawlbestanden worden gemaakt door `crawlforge crawl` en `crawlforge audit`.
error_store_not_sqlite = { $path } is geen bestand van CrawlForge: het is niet eens een SQLite-database.
    Crawlbestanden worden gemaakt door `crawlforge crawl` en `crawlforge audit`.

## Markdown/HTML report (`audit_report.rs`)

report_audit_of = Audit van
report_audit_of_sites = Audit van { $n } sites: { $sites }
sites_and_more = { $sites } en nog { $n }
report_mode_line = Modus `{ $mode }` · gestart op { $started }
report_truncated_note = > **Afgebroken crawl** (`{ $reason }`). De aantallen gaan over wat tot dat moment is gecrawld, niet over de hele site.
report_list_mode_note = > **Lijstcrawl.** Alleen de opgegeven URL's zijn geaudit: de aantallen beschrijven die set, niet de hele site, en regels die de volledige linkstructuur van de site nodig hebben, worden niet geëvalueerd.
th_metric = Metriek
th_value = Waarde
row_pages = Pagina's
row_indexable = Indexeerbaar
row_errors_4xx_5xx = 4xx/5xx
report_more = … nog { $n }
report_more_run = … nog { $n } — voer uit: { $cmd }
report_full_list_run = … volledige lijst: { $cmd }
html_title = SEO-audit

## Rule catalogue (`rules.rs`)

th_severity = ERNST
th_category = CATEGORIE
th_scope = NIVEAU
th_tier = PAKKET
th_name = NAAM
lbl_category = categorie
lbl_scope = niveau
lbl_tier = pakket
lbl_reference = referentie
scope_page = pagina
scope_site = site
rules_summary = { $total } regels: { $free } in het gratis pakket, { $page } op paginaniveau en { $site } op siteniveau.
error_unknown_category = geen regels in categorie ‘{ $category }’. De categorieën zijn: { $list }.

## `main.rs`

crawling = { $target } wordt gecrawld
error_invalid_seed = { $url } is geen crawlbare URL: verwacht wordt iets als https://example.com/. Er is niets aangemaakt.
file_line = Bestand: { $path }
previous_kept = Vorige:  { $path } (bewaard, van de vorige run)
report_written = Rapport geschreven naar { $path }
markdown_report_written = Markdown-rapport geschreven naar { $path }
exported_csv = { $n ->
        [one] { $n } CSV-bestand
       *[other] { $n } CSV-bestanden
    } geëxporteerd naar { $dir }
exported_sheets = { $n ->
        [one] { $n } werkblad
       *[other] { $n } werkbladen
    } geëxporteerd naar { $path }
bench_appended = Benchmarkregistratie toegevoegd aan { $path }
next_steps_title = Volgende stappen
next_explain = Elke bevinding toelichten, met getroffen URL's:
next_spreadsheet = Alle data openen als spreadsheet:
next_compare_previous = Vergelijken met de vorige run van deze crawl:
next_compare_later = Dezelfde crawl opnieuw uitvoeren na je volgende deploy, en de twee vergelijken:
next_reread = Deze samenvatting op elk moment opnieuw lezen:

## HTTP Basic auth

auth_url_note = HTTP Basic-authenticatie uit de URL wordt gebruikt: die wordt alleen naar { $host } gestuurd en niet in het crawlbestand opgeslagen.
auth_env_note = HTTP Basic-authenticatie uit CRAWLFORGE_AUTH wordt gebruikt: die wordt alleen naar { $host } gestuurd en niet in het crawlbestand opgeslagen.
auth_base_ignored = Waarschuwing: inloggegevens in --base zijn verwijderd: een audit van een map leest van schijf en doet geen HTTP-verzoeken.
auth_list_ignored = Waarschuwing: inloggegevens in items van de URL-lijst worden genegeerd en verwijderd. Stel CRAWLFORGE_AUTH=user:password in om de crawl te authenticeren; dat geldt alleen voor de host van de eerste URL.
progress_sitemaps = sitemaps zoeken…
progress_finalize = laatste ronde: inkomende links en sitebrede regels…
progress_crawl = { $fetched } gecrawld · { $queued } in wachtrij · { $rate } URL/s · { $findings } bevindingen
progress_failed_suffix = {" "}· { $n } mislukt

## Resuming an interrupted crawl

resuming = Crawl van { $target } wordt hervat
resume_counts = Verder waar hij stopte: { $done } URL's al gecrawld, { $pending } nog in de wachtrij.
resume_config_note = De configuratie uit het bestand wordt gebruikt: de instellingen van de oorspronkelijke run gelden.
error_resume_schema_blocking = { $file } is een crawl met schema v{ $version }, en migratie { $blocking } verandert wat de engine schrijft: de ene helft zou het ene zeggen en de andere helft iets anders. Voer de crawl opnieuw uit. Je kunt dit bestand nog steeds openen met `report`, `export` en `diff`.
resume_robots_not_inherited = Let op: de oorspronkelijke crawl negeerde robots.txt. Deze niet — die toestemming geef jij, niet een bestand. Voer het oorspronkelijke crawlcommando opnieuw uit als je dat nodig hebt.
resume_finished = Crawl hervat en voltooid: nog { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URL's
    } in { $secs } s
interrupt_flushing = Onderbroken: wat al gecrawld is, wordt opgeslagen… (nogmaals Ctrl+C om te stoppen zonder te wachten)
interrupt_after_done = Onderbroken. Het crawlbestand is volledig; er kan een half geschreven export achterblijven.
warn_wal_kept = Let op: een ander programma heeft { $path } open, dus het blijft in WAL-modus. De bestanden -wal en -shm ernaast horen bij deze crawl: kopieer ze alle drie samen, of sluit het andere programma en voer er `crawlforge report` op uit om ze weer samen te voegen.
error_store_locked = een ander crawlforge-proces schrijft op dit moment naar { $path }. Wacht tot het klaar is of stop het, voordat je dit bestand crawlt of hervat.
interrupted_saved = Crawl onderbroken. Alles wat tot nu toe is gecrawld, staat in { $path }. Ga verder met:
hint_previous_unfinished = De vorige crawl was niet voltooid. Om hem te hervatten in plaats van opnieuw te beginnen:
error_resume_done = { $path } is een voltooide crawl: er valt niets te hervatten.
    Voer het oorspronkelijke crawlcommando uit om de site opnieuw te auditen; het voltooide bestand blijft ernaast bewaard als .prev.sqlite.
error_resume_status = { $path } kan niet worden hervat: de status is ‘{ $status }’. Alleen onderbroken crawls (status ‘running’ of ‘paused’) kunnen worden hervat.
error_resume_schema = { $path } gebruikt schema v{ $found } en deze build schrijft v{ $expected }: een onderbroken crawl kan alleen worden hervat door de versie die hem startte. Voer de crawl opnieuw uit.
error_resume_config = de crawlconfiguratie in { $path } kan niet worden gelezen. Is het bestand door een andere versie geschreven? Voer de crawl opnieuw uit.
