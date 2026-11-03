### CrawlForge — Meldungen der Kommandozeile (Deutsch).
###
### Übersetzung von locales/en/cli.ftl. Englisch ist die Quelle und maßgeblich.
### CONTRIBUTING.md, "Adding or improving a language", erklärt die Regeln.

## Severity names

severity_label_critical = Kritisch
severity_label_high = Hoch
severity_label_medium = Mittel
severity_label_low = Niedrig
severity_label_info = Info
severity_word_critical = kritisch
severity_word_high = hoch
severity_word_medium = mittel
severity_word_low = niedrig
severity_word_info = info

## Crawl summary (`report.rs`)

crawl_finished = Crawl abgeschlossen: { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URLs
    } in { $secs } s
crawl_failed_suffix = {" "}({ $n } fehlgeschlagen)
crawl_truncated = Crawl abgebrochen durch { $reason }. Die angezeigten Befunde sind die bis zu diesem Punkt gefundenen.
crawl_rate_limited_note = Die Website hat den Crawl gedrosselt: { $n ->
        [one] { $n } Seite antwortete auch nach den Wiederholungsversuchen mit 429 Too Many Requests und bleibt ausstehend
       *[other] { $n } Seiten antworteten auch nach den Wiederholungsversuchen mit 429 Too Many Requests und bleiben ausstehend
    }, statt als defekt gemeldet zu werden. Regeln, die die gesamte Website benötigen, werden erst ausgewertet, wenn alles abgerufen ist. Führen Sie später aus:
    {"    "}crawlforge resume { $path }
    oder crawlen Sie erneut mit einer niedrigeren --concurrency, oder lassen Sie den User-Agent von CrawlForge in der Firewall der Website zu.
report_rate_limited_note = > **Die Website hat den Crawl gedrosselt.** { $n ->
        [one] { $n } Seite antwortete mit 429 Too Many Requests und blieb ausstehend
       *[other] { $n } Seiten antworteten mit 429 Too Many Requests und blieben ausstehend
    }: Solche Seiten werden weder gezählt noch als defekt gemeldet, und Regeln, die die gesamte Website benötigen, werden nicht ausgewertet. `crawlforge resume` ruft sie ab.
crawl_list_mode_note = Listen-Crawl: Nur die von Ihnen angegebenen URLs wurden auditiert. Regeln, die den vollständigen Linkgraphen der Website benötigen (verwaiste Seiten, eingehende Links, Klicktiefe), werden nicht ausgewertet.
external_unchecked = { $n ->
        [one] { $n } externer Link wurde
       *[other] { $n } externe Links wurden
    } nicht geprüft: Die Obergrenze max_external wurde erreicht. Erhöhen Sie sie mit --max-external.
external_unregistered = { $n ->
        [one] { $n } externer Link wurde
       *[other] { $n } externe Links wurden
    } nicht einmal erfasst: Die Obergrenze max_external_urls wurde erreicht. Sie ist für den Fall gedacht, dass eine einzelne Seite auf Hunderttausende Hosts verlinkt.
external_never_checked = { $n ->
        [one] { $n } externer Link wurde erfasst, aber nie geprüft.
       *[other] { $n } externe Links wurden erfasst, aber nie geprüft.
    }
external_offline = { $n ->
        [one] { $n } externer Link wurde
       *[other] { $n } externe Links wurden
    } nicht geprüft: Ein Ordner-Audit stellt keine Netzwerkanfragen. Um sie zu prüfen, crawlen Sie die veröffentlichte Website mit `crawlforge crawl <URL>`.
external_check_disabled = Externe Links wurden nicht geprüft (--no-external-check): Dass dieser Bericht keine defekten externen Links enthält, heißt nicht, dass es keine gibt.
external_checked_note = { $n ->
        [one] { $n } externer Link geprüft.
       *[other] { $n } externe Links geprüft.
    }
rules_not_evaluated = Regeln, die den vollständigen Linkgraphen benötigen, wurden nicht ausgewertet: { $rules }.
results_title = Ergebnisse
label_urls = URLs
label_html_pages = HTML-Seiten
label_links = Links
note_internal = { $n } intern
note_indexable = { $n } indexierbar
heading_status_codes = Statuscodes
label_no_response = keine Antwort
note_request_failed = die Anfrage ist fehlgeschlagen
label_never_requested = Nie abgerufen
note_never_requested = (gefunden, aber der Crawl endete, bevor sie abgerufen wurden)
label_out_of_scope = Nicht im Umfang
note_out_of_scope = (von auditierten Seiten verlinkt, aber außerhalb des Umfangs dieses Crawls)
heading_non_indexable = Nicht indexierbar, warum
heading_findings = Befunde
no_findings = Keine Befunde.
heading_excluded = Ausgeschlossen
template_issues = { $k ->
        [one] { $k } Template-Problem ({ $pages } Seiten)
       *[other] { $k } Template-Probleme ({ $pages } Seiten)
    }
plus_more_findings = {" "}+ { $n ->
        [one] { $n } weiterer Befund
       *[other] { $n } weitere Befunde
    }
example_urls = z. B. { $urls }

## Dominant rules

pervasive_note = { $pct } % der Website
deep_pages_summary = { $pages } Seiten tiefer als { $max } Klicks – { $pct } % der Website (typische Klicktiefe { $low }–{ $high }, maximal { $deepest })
rule_deep_sorted = Nach Klicktiefe sortiert, tiefste zuerst; die erste Spalte zeigt die Klicks ab der Startseite.
hint_full_lists = Vollständige URL-Liste einer Regel: { $cmd }

## One rule (`report --rule`)

rule_affected_urls = { $n ->
        [one] { $n } betroffene URL
       *[other] { $n } betroffene URLs
    }
rule_template_group = Template-Gruppe – { $n ->
        [one] { $n } Seite
       *[other] { $n } Seiten
    }
rule_group_cause = Ursache: { $detail }
rule_other_pages = Weitere betroffene Seiten – { $n }
rule_no_findings = Keine Befunde für { $rule } in diesem Crawl.
hint_who_links = Ein defekter Link wird auf der Seite behoben, die ihn setzt. Wer auf eine URL verlinkt, mit Ankertext:
error_unknown_rule = { $rule } ist keine Regel-ID. Den Katalog zeigt `crawlforge rules`.
error_all_blocked_by_robots = robots.txt blockiert jede URL: Es gibt nichts zu auditieren. Wenn die Website Ihnen gehört, können Sie es mit --ignore-robots erneut versuchen.
error_all_excluded_by_pattern = jede URL wurde durch Ihre --exclude/--include-Muster übersprungen: Es gibt nichts zu auditieren. Prüfen Sie die Muster.
error_no_urls_fetched = der Crawl hat keine einzige URL abgerufen. Prüfen Sie das Ziel.
error_dead_seed = keine Verbindung zu { $url } möglich ({ $cause }). Prüfen Sie die URL.
cause_dns = die Domain existiert nicht oder wird nicht aufgelöst
cause_tls = das TLS-Zertifikat ist ungültig
cause_timeout = der Server hat nicht rechtzeitig geantwortet
cause_connection = Verbindung abgelehnt
cause_no_response = keine Antwort
warn_base_mismatch = Warnung: { $n } von { $total } Canonicals { $n ->
        [one] verweist
       *[other] verweisen
    } auf { $origin }, nicht auf die angegebene Basis ({ $base }).
    Die Ergebnisse zur Indexierbarkeit hängen von dieser Angabe ab: Wenn die Website unter
    { $origin } veröffentlicht ist, wiederholen Sie das Audit mit --base { $origin }

## URL card (`inspect.rs`)

inspect_error_not_found = { $url } ist nicht in diesem Crawl enthalten.
inspect_closest = Die ähnlichsten URLs in der Datei sind:
inspect_status_title = Status
label_http_status = HTTP-Status
label_content_type = Content-Type
label_response_time = Antwortzeit
label_depth = Klicktiefe
note_depth_clicks = Klicks ab der Start-URL
label_in_sitemap = In der Sitemap
word_yes = ja
word_no = nein
label_crawl_state = Crawl-Status
state_note_pending = gefunden, aber vom Crawl nie abgerufen
state_note_excluded = vom Crawl ausgeschlossen ({ $reason })
state_note_external_checked = externe URL – Status geprüft, Inhalt nicht auditiert
state_note_external_not_checked = externe URL – Status nicht geprüft ({ $reason })
state_note_external_unchecked = externe URL – nie abgerufen
state_note_out_of_scope = gefunden, aber außerhalb des Umfangs dieses Crawls
state_note_error = die Anfrage ist fehlgeschlagen ({ $detail })
region_unknown = k. A.
inspect_page_title = Seite
label_page_title = Title
label_meta_description = Meta Description
label_h1 = H1
label_word_count = Wortanzahl
label_canonical = Canonical
canonical_self = selbstreferenzierend
label_indexable = Indexierbar
indexable_no = nein – { $reason }
chars_suffix = { $n } Zeichen
inspect_redirects_title = Weiterleitungskette
inlinks_title = Eingehende Links ({ $n })
inlinks_by_region = Nach Seitenbereich: { $breakdown } · { $nofollow } nofollow
inlinks_none = Keine gecrawlte Seite verlinkt auf diese URL.
inlinks_sample_heading = Verlinkende Seiten, Content-Links zuerst:
no_anchor = (kein Ankertext)
inspect_truncated = … { $n } angezeigt – vollständige Liste: { $cmd }
images_title = Bilder ({ $n })
image_no_alt = (kein Alt-Text)
image_usage_title = Als Bild verwendet
outlinks_title = Ausgehende Links ({ $total }: { $internal } intern, { $external } extern)
note_external_status = +{ $n } extern
image_usage_line = { $times }-mal eingebunden auf { $pages ->
        [one] { $pages } Seite
       *[other] { $pages } Seiten
    }

## Crawl comparison (`diff.rs`)

diff_title = Crawl-Vergleich
label_before = Vorher
label_after = Nachher
label_site = Website
label_common = Gemeinsam
diff_crawl_counts = { $urls } URLs, { $findings } Befunde
diff_common_note = { $n ->
        [one] { $n } URL in beiden Crawls vorhanden
       *[other] { $n } URLs in beiden Crawls vorhanden
    }
warnings_title = Warnungen
tag_inconclusive = NICHT SCHLÜSSIG
tag_warning = WARNUNG
side_before = vorher
side_after = nachher
warn_truncated = Der Crawl „{ $side }“ wurde{ $reason_suffix } abgebrochen. Was verschwunden ist oder behoben wurde, lässt sich nicht feststellen: Der Crawl hat es womöglich einfach nicht erreicht.
warn_truncated_by = {" "}durch { $reason }
warn_list_mode = Der Crawl „{ $side }“ ist ein Listen-Crawl: Er hat nur die übergebenen URLs gesehen. Was verschwunden ist oder behoben wurde, lässt sich nicht feststellen.
warn_unfinished = Der Crawl „{ $side }“ wurde nicht abgeschlossen (Status „{ $status }“). Was darin fehlt, sagt nichts aus.
warn_rules_changed = Der Regelkatalog hat sich geändert ({ $before } → { $after }). Manche neuen oder behobenen Befunde können von den Regeln stammen, nicht von der Website.
warn_config_changed = Die Crawl-Konfiguration hat sich geändert bei: { $fields }. Wer das nicht berücksichtigt, schreibt Änderungen am Crawl-Umfang der Website zu.
warn_scope_changed = Die Basis-URLs unterscheiden sich: { $before } → { $after }. Gleiche Website, anderer Ausgangspunkt.
warn_mode_changed = Die Crawl-Modi unterscheiden sich: { $before } → { $after }.
warn_missing_table = Der Crawl „{ $side }“ ist älter als die Tabelle `{ $table }`: Dieser Vergleich entfällt.
warn_order_inverted = Der als „nachher“ angegebene Crawl ({ $after_started }) hat früher begonnen als der als „vorher“ angegebene ({ $before_started }). Falls das nicht beabsichtigt ist, vertauschen Sie die beiden Argumente: „Verschlechtert“ und „Verbessert“ kommen sonst vertauscht heraus.
worse_title = Verschlechtert
label_new_findings = Neue Befunde
more_examples = … und { $n } weitere
label_pages_lost_index = Nicht mehr indexierbare Seiten
label_status_worse = Verschlechterte Statuscodes
robots_content_changed = { $host }: Der Inhalt hat sich geändert
robots_blocks_all = ACHTUNG: robots.txt blockiert jetzt das Crawling der gesamten Website.
nothing_worse = Nichts. Keine neuen Befunde, keine Seiten aus dem Index gefallen.
better_title = Verbessert
label_findings_resolved = Behobene Befunde
label_pages_indexable_again = Wieder indexierbare Seiten
label_status_better = Verbesserte Statuscodes
other_title = Weitere Änderungen
label_new_urls = Neue URLs
label_urls_gone = Entfallene URLs
label_titles_changed = Geänderte Titles
label_meta_changed = Geänderte Meta Descriptions
label_canonicals_changed = Geänderte Canonicals
label_findings_persisted = Weiterhin vorhandene Befunde
suppressed_title = Nicht feststellbar
suppressed_intro = Einer der beiden Crawls ist unvollständig, daher könnten diese Unterschiede von dem stammen,
    was nie gecrawlt wurde, und nicht von der Website. Sie zählen nicht als Änderungen:
label_candidate_new_urls = Mögliche neue URLs
label_candidate_urls_gone = Möglicherweise entfallene URLs
label_candidate_new_findings = Mögliche neue Befunde
label_candidate_resolved = Möglicherweise behobene Befunde
suppressed_advice = Crawlen Sie beide erneut mit demselben Limit, um einen belastbaren Vergleich zu erhalten.
gate_title = CI-Gate
gate_inconclusive = Der Crawl „vorher“ ist unvollständig: Neue Befunde lassen sich nicht feststellen, und
    --fail-on bezieht keine Stellung. Das ist kein PASS.
gate_fail_detail = { $n ->
        [one] { $n } neuer Befund
       *[other] { $n } neue Befunde
    } [{ $severity }] – entspricht --fail-on { $token }
gate_pass = PASS   nichts Überwachtes taucht als neuer Befund auf.

## Saved diff (`diff_file.rs`)

label_saved = Datei
diff_saved_note = Vergleich erstellt am { $generated_at } mit crawlforge { $version } · { $path }
hint_diff_full_lists = Die Listen oben zeigen nur einige Beispiele. Für alle Zeilen ergänzen Sie --rule <ID> oder
    --change <Typ> im selben Befehl (z. B. --change url_removed).
warn_unknown_change_types = { $n ->
        [one] { $n } Änderung eines Typs, den diese crawlforge-Version nicht kennt ({ $types }), wurde ausgelassen.
       *[other] { $n } Änderungen eines Typs, den diese crawlforge-Version nicht kennt ({ $types }), wurden ausgelassen.
    } Die Datei wurde von einer neueren Version geschrieben.
label_status_changed = Geänderte Statuscodes
diff_no_matching_changes = Keine Änderung in diesem Vergleich passt.
error_diff_missing = { $path } existiert nicht. Vergleichsdateien speichert `crawlforge diff <before> <after> --out <file>`.
error_diff_needs_two = { $path } ist eine Crawl-Datei, und ein Vergleich braucht zwei: den früheren Crawl und den späteren. Eine einzelne Datei wird nur akzeptiert, wenn sie ein mit --out gespeicherter Vergleich ist.
hint_diff_prev = Der vorherige Crawl liegt daneben:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } ist weder eine Crawl-Datei noch eine Vergleichsdatei. Vergleichsdateien speichert `crawlforge diff <before> <after> --out <file>`.
error_diff_too_new = { $path } ist eine Vergleichsdatei im Format { $version }, neuer als diese crawlforge-Version versteht ({ $supported }). Aktualisieren Sie crawlforge, um sie zu öffnen.
error_diff_reopen_flag = { $flag } gilt beim Vergleichen zweier Crawls; ein gespeicherter Vergleich wird nur gelesen. Wenden Sie es auf die Crawls an: crawlforge diff <before> <after> { $flag } …
error_unknown_change_type = { $value } ist kein Änderungstyp. Verfügbar: { $available }.
error_rule_with_change = --rule filtert Befunde und lässt sich daher nur mit --change issue_appeared oder issue_resolved kombinieren, nicht mit { $change }.
site_wide_finding = (websiteweiter Befund)
site_placeholder = (Website)
error_missing_file = { $path } existiert nicht
error_store_missing = { $path } existiert nicht. Crawl-Dateien erzeugen `crawlforge crawl`, `crawlforge audit` und `crawlforge list`.
error_not_a_crawl = { $path } sieht nicht nach einer CrawlForge-Crawl-Datei aus
error_different_sites = die beiden Crawls stammen von verschiedenen Websites: { $a } und { $b }.
    URLs werden über ihren Text zugeordnet, also würde der Vergleich melden, die ganze Website sei verschwunden und eine andere aufgetaucht. Vergleichen Sie jede Website mit sich selbst.
error_fail_on_unknown = --fail-on: „{ $token }“ ist weder eine Regel noch eine Priorität.
    Die Prioritäten sind critical, high, medium, low und info; Regel-IDs listet `crawlforge rules` auf.

## Portfolio (`portfolio.rs`)

portfolio_title = Portfolio-Übersicht
portfolio_range = Crawls vom { $oldest } bis { $newest }
warn_portfolio_date_spread = Der älteste Crawl ({ $oldest }) und der neueste ({ $newest }) liegen { $days ->
        [one] { $days } Tag
       *[other] { $days } Tage
    } auseinander: Diese Übersicht ist keine Momentaufnahme des Portfolios, und „Was sich geändert hat“ deckt auf jeder Website einen anderen Zeitraum ab.
warn_portfolio_rules_versions = Nicht jede Website wurde mit demselben Regelkatalog gecrawlt ({ $versions }). Eine Regel kann auf einer Website fehlen, weil es sie beim Crawl dieser Website noch nicht gab.
warn_portfolio_core_versions = Die Crawls stammen aus verschiedenen Engine-Versionen ({ $versions }).
portfolio_skipped_title = Übergangene Dateien
portfolio_prev_not_input = es ist eine .prev.sqlite: das „Vorher“ des Crawls daneben, das der Vergleich bereits verwendet. Sie zählt nicht als Website.
error_portfolio_newer_schema = ihr Schema (v{ $version }) ist neuer, als dieser Build versteht (v{ $supported }). Aktualisieren Sie crawlforge, um sie zu öffnen.
error_portfolio_no_sites = in den angegebenen Pfaden wurden keine als Portfolio nutzbaren Crawl-Dateien gefunden. Crawl-Dateien erzeugen `crawlforge crawl`, `crawlforge audit` und `crawlforge list`; ein Verzeichnis wird nach *.sqlite-Dateien durchsucht.
error_portfolio_tier = die Portfolio-Übersicht ist nicht im kostenlosen Tarif enthalten: Sie erfordert Pro. Befehle für eine einzelne Website (`report`, `diff`, `export`) bleiben verfügbar.
error_portfolio_too_many = das Portfolio umfasst { $n } Websites, und dieser Tarif begrenzt es auf { $max }.
portfolio_changes_title = Was sich geändert hat
portfolio_no_pairs = Keine Website hat einen vorherigen Crawl daneben. Crawlen Sie erneut in dieselbe Ausgabedatei, dann wird der vorherige Crawl als .prev.sqlite behalten – das ist das „Vorher“, mit dem dieser Abschnitt vergleicht.
portfolio_new_critical_high = Neue Befunde mit Priorität kritisch und hoch
portfolio_none_critical_high = Keine neuen Befunde mit Priorität kritisch oder hoch auf den verglichenen Websites.
portfolio_rest_title = Der Rest, Website für Website
portfolio_no_changes = keine Änderungen
portfolio_pair_inconclusive = einer der beiden Crawls ist unvollständig: Was verschwunden ist oder behoben wurde, lässt sich nicht feststellen
portfolio_pair_failed = der Vergleich mit dem vorherigen Crawl ist fehlgeschlagen: { $reason }
portfolio_spread_title = Probleme im ganzen Portfolio
portfolio_spread_intro = Eine Regel, die auf den meisten Websites anschlägt, liegt selten am Inhalt: Meist ist es ein gemeinsames Template oder Plugin – eine Korrektur, die allen hilft.
portfolio_spread_none = Keine Regel schlägt auf einer Website des Portfolios an.
portfolio_glance_title = Das Portfolio auf einen Blick
th_site = Website
th_crawled = gecrawlt
th_indexable = index.
flag_truncated = (abgebrochen)
flag_list_mode = (Listen-Crawl)
flag_unfinished = (nicht abgeschlossen: { $status })
hint_site_diff = Vollständiger Vergleich einer Website:
portfolio_html_title = Portfolio-Übersicht
portfolio_sites_count = { $n ->
        [one] { $n } Website
       *[other] { $n } Websites
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = in der Crawl-Datei gespeichert
rules_config_source_after = im späteren Crawl gespeichert
rules_config_source_file = aus { $path }
warn_rules_config_differs = Die beiden Crawls speichern unterschiedliche Regelkonfigurationen. Beide wurden mit derselben ausgewertet ({ $source }), daher erscheint eine Konfigurationsänderung nicht als neue oder behobene Befunde.
warn_portfolio_rules_config = Einige Websites werden mit ihrer eigenen Regelkonfiguration gelesen (Regeln deaktiviert / Prioritäten geändert): { $sites }. Ihre Zahlen sind nicht Regel für Regel mit den übrigen vergleichbar.
rules_config_note = Regelkonfiguration ({ $source }): { $disabled ->
        [one] { $disabled } Regel
       *[other] { $disabled } Regeln
    } deaktiviert, { $changed ->
        [one] { $changed } Priorität
       *[other] { $changed } Prioritäten
    } geändert. Die Crawl-Datei behält alle Befunde.
portfolio_sites_of = { $fired } von { $total ->
        [one] { $total } Website
       *[other] { $total } Websites
    }
portfolio_inconclusive_suffix = {" "}({ $n } nicht schlüssig)
portfolio_pairs_line = { $pairs } von { $total ->
        [one] { $total } Website
       *[other] { $total } Websites
    } { $pairs ->
        [one] hat
       *[other] haben
    } einen vorherigen Crawl (.prev.sqlite) zum Vergleich.

## File identification (`store_check.rs`)

error_store_is_diff = { $path } ist keine Crawl-Datei, sondern eine Vergleichsdatei, wie sie `crawlforge diff --out` speichert. Öffnen Sie sie mit `crawlforge diff { $path }`.
    Crawl-Dateien erzeugen `crawlforge crawl` und `crawlforge audit`.
error_store_foreign = { $path } ist keine CrawlForge-Crawl-Datei: Ihr fehlen die nötigen Tabellen. Stammt sie aus einem anderen Programm?
    Crawl-Dateien erzeugen `crawlforge crawl` und `crawlforge audit`.
error_store_not_sqlite = { $path } ist keine CrawlForge-Datei: Es ist nicht einmal eine SQLite-Datenbank.
    Crawl-Dateien erzeugen `crawlforge crawl` und `crawlforge audit`.

## Markdown/HTML report (`audit_report.rs`)

report_audit_of = Audit von
report_audit_of_sites = Audit von { $n } Websites: { $sites }
sites_and_more = { $sites } und { $n } weitere
report_mode_line = Modus `{ $mode }` · gestartet am { $started }
report_truncated_note = > **Abgebrochener Crawl** (`{ $reason }`). Die Zahlen beziehen sich auf das bis dahin Gecrawlte, nicht auf die gesamte Website.
report_list_mode_note = > **Listen-Crawl.** Nur die aufgelisteten URLs wurden auditiert: Die Zahlen beschreiben diese Menge, nicht die gesamte Website, und Regeln, die den vollständigen Linkgraphen der Website benötigen, werden nicht ausgewertet.
th_metric = Kennzahl
th_value = Wert
row_pages = Seiten
row_indexable = Indexierbar
row_errors_4xx_5xx = 4xx/5xx
report_more = … { $n } weitere
report_more_run = … { $n } weitere – ausführen: { $cmd }
report_full_list_run = … vollständige Liste: { $cmd }
html_title = SEO-Audit

## Rule catalogue (`rules.rs`)

th_severity = PRIORITÄT
th_category = KATEGORIE
th_scope = EBENE
th_tier = TARIF
th_name = NAME
lbl_category = Kategorie
lbl_scope = Ebene
lbl_tier = Tarif
lbl_reference = Referenz
scope_page = Seite
scope_site = Website
rules_summary = { $total } Regeln: { $free } im kostenlosen Tarif, { $page } auf Seitenebene und { $site } auf Websiteebene.
error_unknown_category = keine Regeln in der Kategorie „{ $category }“. Die Kategorien sind: { $list }.

## `main.rs`

crawling = Crawle { $target }
error_invalid_seed = { $url } ist keine crawlbare URL: Erwartet wird etwas wie https://example.com/. Es wurde nichts angelegt.
file_line = Datei:     { $path }
previous_kept = Vorherige: { $path } (behalten, vom letzten Lauf)
report_written = Bericht gespeichert unter { $path }
markdown_report_written = Markdown-Bericht gespeichert unter { $path }
exported_csv = { $n ->
        [one] { $n } CSV-Datei
       *[other] { $n } CSV-Dateien
    } nach { $dir } exportiert
exported_sheets = { $n ->
        [one] { $n } Tabellenblatt
       *[other] { $n } Tabellenblätter
    } nach { $path } exportiert
bench_appended = Benchmark-Eintrag an { $path } angehängt
next_steps_title = Nächste Schritte
next_explain = Jeden Befund erklären, mit betroffenen URLs:
next_spreadsheet = Alle Daten als Tabelle öffnen:
next_compare_previous = Mit dem vorherigen Lauf dieses Crawls vergleichen:
next_compare_later = Denselben Crawl nach dem nächsten Deployment erneut ausführen und beide vergleichen:
next_reread = Diese Zusammenfassung jederzeit erneut lesen:

## HTTP Basic auth

auth_url_note = HTTP-Basic-Auth aus der URL wird verwendet: Sie wird nur an { $host } gesendet und nicht in der Crawl-Datei gespeichert.
auth_env_note = HTTP-Basic-Auth aus CRAWLFORGE_AUTH wird verwendet: Sie wird nur an { $host } gesendet und nicht in der Crawl-Datei gespeichert.
auth_base_ignored = Warnung: Zugangsdaten in --base wurden entfernt: Ein Verzeichnis-Audit liest von der Festplatte und stellt keine HTTP-Anfragen.
auth_list_ignored = Warnung: Zugangsdaten in Einträgen der URL-Liste werden ignoriert und entfernt. Um den Crawl zu authentifizieren, setzen Sie CRAWLFORGE_AUTH=user:password; das gilt nur für den Host der ersten URL.
progress_sitemaps = Sitemaps werden gesucht…
progress_finalize = Abschlussdurchlauf: eingehende Links und websiteweite Regeln…
progress_crawl = { $fetched } gecrawlt · { $queued } in der Warteschlange · { $rate } URL/s · { $findings } Befunde
progress_failed_suffix = {" "}· { $n } fehlgeschlagen

## Resuming an interrupted crawl

resuming = Crawl von { $target } wird fortgesetzt
resume_counts = Fortsetzung an der Abbruchstelle: { $done } URLs bereits gecrawlt, { $pending } noch ausstehend.
resume_config_note = Die in der Datei gespeicherte Konfiguration wird verwendet: Es gelten die Einstellungen des ursprünglichen Laufs.
error_resume_schema_blocking = { $file } ist ein Crawl mit Schema v{ $version }, und Migration { $blocking } ändert, was die Engine schreibt: Die eine Hälfte würde das eine sagen, die andere etwas anderes. Führen Sie den Crawl erneut aus. Mit `report`, `export` und `diff` lässt sich diese Datei weiterhin öffnen.
resume_robots_not_inherited = Hinweis: Der ursprüngliche Crawl hat robots.txt ignoriert. Dieser nicht – diese Erlaubnis erteilen Sie, nicht eine Datei. Führen Sie den ursprünglichen Crawl-Befehl erneut aus, falls Sie sie brauchen.
resume_finished = Crawl fortgesetzt und abgeschlossen: { $urls ->
        [one] { $urls } weitere URL
       *[other] { $urls } weitere URLs
    } in { $secs } s
interrupt_flushing = Unterbrochen: Bereits Gecrawltes wird gespeichert… (erneut Strg+C zum Beenden ohne Warten)
interrupt_after_done = Unterbrochen. Die Crawl-Datei ist vollständig; ein halb geschriebener Export kann zurückbleiben.
warn_wal_kept = Hinweis: Ein anderes Programm hat { $path } geöffnet, daher bleibt die Datei im WAL-Modus. Die Dateien -wal und -shm daneben gehören zu diesem Crawl: Kopieren Sie alle drei zusammen, oder schließen Sie das andere Programm und führen Sie `crawlforge report` darauf aus, um sie wieder zusammenzuführen.
error_store_locked = ein anderer crawlforge-Prozess schreibt gerade in { $path }. Warten Sie, bis er fertig ist, oder beenden Sie ihn, bevor Sie diese Datei crawlen oder fortsetzen.
interrupted_saved = Crawl unterbrochen. Alles bisher Gecrawlte ist in { $path } gespeichert. Fortsetzen mit:
hint_previous_unfinished = Der vorherige Crawl war nicht abgeschlossen. Um ihn fortzusetzen, statt neu zu beginnen:
error_resume_done = { $path } ist ein abgeschlossener Crawl: Es gibt nichts fortzusetzen.
    Um die Website erneut zu auditieren, führen Sie den ursprünglichen Crawl-Befehl aus; die abgeschlossene Datei wird daneben als .prev.sqlite behalten.
error_resume_status = { $path } kann nicht fortgesetzt werden: Ihr Status ist „{ $status }“. Nur unterbrochene Crawls (Status „running“ oder „paused“) lassen sich fortsetzen.
error_resume_schema = { $path } verwendet Schema v{ $found }, und dieser Build schreibt v{ $expected }: Ein unterbrochener Crawl kann nur von der Version fortgesetzt werden, die ihn begonnen hat. Führen Sie den Crawl erneut aus.
error_resume_config = die in { $path } gespeicherte Crawl-Konfiguration kann nicht gelesen werden. Wurde die Datei von einer anderen Version geschrieben? Führen Sie den Crawl erneut aus.
