### CrawlForge — command-line messages, Polish.
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules.
###
### Glossary: crawl = skanowanie (verb: skanować), finding = problem, template issue = błąd
### szablonu, rule = reguła, severity = waga, site = witryna, page = strona, canonical = adres
### kanoniczny, sitemap = mapa witryny, portfolio = portfolio, tier = plan, scope = zakres.
###
### Plurals: [one] 1, [few] 2–4, 22–24…, [many] 0, 5–21, 25…. CLDR's *[other] is for fractions,
### which no count here ever is; what does reach the default is a number that arrives as text
### (right-padded, or an ungrouped raw integer of four digits or more). So the default repeats
### the [many] form, the likeliest right one for such a number, and messages whose variable
### always arrives as text are phrased as "label: { $n }" instead.

## Severity names

severity_label_critical = Krytyczna
severity_label_high = Wysoka
severity_label_medium = Średnia
severity_label_low = Niska
severity_label_info = Info
severity_word_critical = krytyczna
severity_word_high = wysoka
severity_word_medium = średnia
severity_word_low = niska
severity_word_info = info

## Resumen del rastreo (`report.rs`)

crawl_finished = Skanowanie zakończone: { $urls ->
        [one] { $urls } adres URL
        [few] { $urls } adresy URL
        [many] { $urls } adresów URL
       *[other] { $urls } adresów URL
    } w { $secs } s
crawl_failed_suffix = {" "}(z błędem: { $n })
crawl_truncated = Skanowanie przerwane z powodu { $reason }. Pokazane problemy to te wykryte do tego momentu.
crawl_rate_limited_note = Witryna ograniczyła tempo skanowania: { $n ->
        [one] { $n } strona po ponownych próbach nadal odpowiadała 429 Too Many Requests i czeka na pobranie
        [few] { $n } strony po ponownych próbach nadal odpowiadały 429 Too Many Requests i czekają na pobranie
        [many] { $n } stron po ponownych próbach nadal odpowiadało 429 Too Many Requests i czeka na pobranie
       *[other] { $n } stron po ponownych próbach nadal odpowiadało 429 Too Many Requests i czeka na pobranie
    }. Strony oczekujące nie są zgłaszane jako niedziałające, a reguły wymagające całej witryny nie są oceniane, dopóki nie zostaną pobrane. Później uruchom:
    {"    "}crawlforge resume { $path }
    albo przeskanuj witrynę ponownie z niższym --concurrency, albo przepuść user-agent CrawlForge przez zaporę witryny.
report_rate_limited_note = > **Witryna ograniczyła tempo skanowania.** { $n ->
        [one] { $n } strona odpowiedziała 429 Too Many Requests i czeka na pobranie
        [few] { $n } strony odpowiedziały 429 Too Many Requests i czekają na pobranie
        [many] { $n } stron odpowiedziało 429 Too Many Requests i czeka na pobranie
       *[other] { $n } stron odpowiedziało 429 Too Many Requests i czeka na pobranie
    }. Strony oczekujące nie są liczone ani zgłaszane jako niedziałające, a reguły wymagające całej witryny nie są oceniane. `crawlforge resume` je pobiera.
crawl_list_mode_note = Skanowanie listy: sprawdzono tylko podane adresy URL. Reguły wymagające pełnego grafu linków witryny (strony osierocone, linki przychodzące, głębokość) nie są oceniane.
external_unchecked = Nie sprawdzono { $n ->
        [one] { $n } linku zewnętrznego
        [few] { $n } linków zewnętrznych
        [many] { $n } linków zewnętrznych
       *[other] { $n } linków zewnętrznych
    }: osiągnięto limit max_external. Zwiększ go za pomocą --max-external.
external_unregistered = Nie zarejestrowano nawet { $n ->
        [one] { $n } linku zewnętrznego
        [few] { $n } linków zewnętrznych
        [many] { $n } linków zewnętrznych
       *[other] { $n } linków zewnętrznych
    }: osiągnięto limit max_external_urls. Ten limit istnieje właśnie na wypadek pojedynczej strony linkującej do setek tysięcy hostów.
external_never_checked = Linki zewnętrzne zarejestrowane, ale nigdy niesprawdzone: { $n }.
external_offline = Nie sprawdzono { $n ->
        [one] { $n } linku zewnętrznego
        [few] { $n } linków zewnętrznych
        [many] { $n } linków zewnętrznych
       *[other] { $n } linków zewnętrznych
    }: audyt folderu nie wysyła żądań sieciowych. Aby sprawdzić linki zewnętrzne, przeskanuj opublikowaną witrynę poleceniem `crawlforge crawl <URL>`.
external_check_disabled = Linki zewnętrzne nie zostały sprawdzone (--no-external-check): brak niedziałających linków zewnętrznych w tym raporcie nie oznacza, że ich nie ma.
external_checked_note = Sprawdzono { $n ->
        [one] { $n } link zewnętrzny
        [few] { $n } linki zewnętrzne
        [many] { $n } linków zewnętrznych
       *[other] { $n } linków zewnętrznych
    }.
rules_not_evaluated = Nie oceniono reguł wymagających pełnego grafu linków: { $rules }.
results_title = Wyniki
label_urls = URL-e
label_html_pages = Strony HTML
label_links = Linki
# Follows the URL count: "(N internal)" URLs, masculine (adres URL).
note_internal = { $n ->
        [one] { $n } wewnętrzny
        [few] { $n } wewnętrzne
        [many] { $n } wewnętrznych
       *[other] { $n } wewnętrznych
    }
# Follows the HTML page count: feminine (strona).
note_indexable = { $n ->
        [one] { $n } indeksowalna
        [few] { $n } indeksowalne
        [many] { $n } indeksowalnych
       *[other] { $n } indeksowalnych
    }
heading_status_codes = Kody stanu
label_no_response = brak odpowiedzi
note_request_failed = żądanie nie powiodło się
label_never_requested = Nigdy niepobrane
note_never_requested = (wykryte, ale skanowanie zatrzymało się przed ich pobraniem)
label_out_of_scope = Poza zakresem
note_out_of_scope = (linkowane ze sprawdzonych stron, ale poza zakresem tego skanowania)
heading_non_indexable = Nieindeksowalne — dlaczego
heading_findings = Problemy
no_findings = Brak problemów.
heading_excluded = Wykluczone
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = { $k ->
        [one] { $k } błąd szablonu
        [few] { $k } błędy szablonu
        [many] { $k } błędów szablonu
       *[other] { $k } błędów szablonu
    } ({ $pages ->
        [one] { $pages } strona
        [few] { $pages } strony
        [many] { $pages } stron
       *[other] { $pages } stron
    })
plus_more_findings = {" "}+ { $n ->
        [one] { $n } inny problem
        [few] { $n } inne problemy
        [many] { $n } innych problemów
       *[other] { $n } innych problemów
    }
example_urls = np. { $urls }

## Reformulación de reglas dominantes

pervasive_note = { $pct }% witryny
deep_pages_summary = { $pages ->
        [one] { $pages } strona
        [few] { $pages } strony
        [many] { $pages } stron
       *[other] { $pages } stron
    } głębiej niż { $max ->
        [one] { $max } kliknięcie
        [few] { $max } kliknięcia
        [many] { $max } kliknięć
       *[other] { $max } kliknięć
    } od strony głównej — { $pct }% witryny (typowa głębokość { $low }–{ $high }, największa { $deepest })
rule_deep_sorted = Posortowane według głębokości kliknięć, od najgłębszych; pierwsza kolumna to liczba kliknięć od strony głównej.
hint_full_lists = Pełna lista adresów URL dla reguły: { $cmd }

## Listado de una regla (`report --rule`)

rule_affected_urls = Adresy URL z problemem: { $n }
rule_template_group = Grupa szablonu — { $n ->
        [one] { $n } strona
        [few] { $n } strony
        [many] { $n } stron
       *[other] { $n } stron
    }
rule_group_cause = przyczyna: { $detail }
rule_other_pages = Inne strony z problemem — { $n }
rule_no_findings = Reguła { $rule } nie wykryła żadnych problemów w tym skanowaniu.
hint_who_links = Niedziałający link naprawia się na stronie, która do niego linkuje. Kto linkuje do adresu URL, wraz z tekstem kotwicy:
error_unknown_rule = { $rule } nie jest identyfikatorem reguły. Katalog wyświetla `crawlforge rules`.
error_all_blocked_by_robots = robots.txt blokuje wszystkie adresy URL: nie ma czego audytować. Jeśli zarządzasz tą witryną, możesz spróbować ponownie z --ignore-robots.
error_all_excluded_by_pattern = wszystkie adresy URL zostały pominięte przez wzorce --exclude/--include: nie ma czego audytować. Sprawdź wzorce.
error_no_urls_fetched = skanowanie nie pobrało żadnego adresu URL. Sprawdź cel.
error_dead_seed = nie udało się połączyć z { $url } ({ $cause }). Sprawdź adres URL.
cause_dns = domena nie istnieje albo nie rozwiązuje się w DNS
cause_tls = certyfikat TLS jest nieprawidłowy
cause_timeout = serwer nie odpowiedział na czas
cause_connection = połączenie odrzucone
cause_no_response = brak odpowiedzi
warn_base_mismatch = Uwaga: adresy kanoniczne wskazujące na { $origin } zamiast na podaną bazę ({ $base }): { $n } z { $total }.
    Wyniki indeksowalności zależą od tego ustawienia: jeśli witryna jest publikowana pod
    { $origin }, powtórz audyt z --base { $origin }

## Ficha de una URL (`inspect.rs`)

inspect_error_not_found = { $url } nie występuje w tym skanowaniu.
inspect_closest = Najbardziej podobne adresy URL w pliku:
inspect_status_title = Stan
label_http_status = Kod stanu HTTP
label_content_type = Typ treści
label_response_time = Czas odpowiedzi
label_depth = Głębokość
note_depth_clicks = kliknięcia od adresu startowego
label_in_sitemap = W mapie witryny
word_yes = tak
word_no = nie
label_crawl_state = Stan skanowania
state_note_pending = wykryty, ale skanowanie nigdy go nie pobrało
state_note_excluded = wykluczony ze skanowania ({ $reason })
state_note_external_checked = zewnętrzny adres URL — stan sprawdzony, treść nie była audytowana
state_note_external_not_checked = zewnętrzny adres URL — stan niesprawdzony ({ $reason })
state_note_external_unchecked = zewnętrzny adres URL — nigdy nie był pobierany
state_note_out_of_scope = wykryty, ale poza zakresem tego skanowania
state_note_error = żądanie nie powiodło się ({ $detail })
region_unknown = nieznany
inspect_page_title = Strona
label_page_title = Tytuł
label_meta_description = Opis meta
label_h1 = H1
label_word_count = Liczba słów
label_canonical = Adres kanoniczny
canonical_self = wskazuje na siebie
label_indexable = Indeksowalna
indexable_no = nie — { $reason }
chars_suffix = { $n ->
        [one] { $n } znak
        [few] { $n } znaki
        [many] { $n } znaków
       *[other] { $n } znaków
    }
inspect_redirects_title = Łańcuch przekierowań
inlinks_title = Linki przychodzące ({ $n })
inlinks_by_region = Według obszaru: { $breakdown } · { $nofollow } nofollow
inlinks_none = Żadna przeskanowana strona nie linkuje do tego adresu URL.
inlinks_sample_heading = Strony linkujące, najpierw linki w treści:
no_anchor = (brak tekstu kotwicy)
inspect_truncated = … wyświetlono { $n } — pełna lista: { $cmd }
images_title = Obrazy ({ $n })
image_no_alt = (brak alt)
image_usage_title = Użyty jako obraz
# Outbound links header. The three variables are counts.
outlinks_title = Linki wychodzące ({ $total }: { $internal ->
        [one] { $internal } wewnętrzny
        [few] { $internal } wewnętrzne
        [many] { $internal } wewnętrznych
       *[other] { $internal } wewnętrznych
    }, { $external ->
        [one] { $external } zewnętrzny
        [few] { $external } zewnętrzne
        [many] { $external } zewnętrznych
       *[other] { $external } zewnętrznych
    })
# Suffix that keeps external URLs apart from the status-code count (adres URL, masculine).
note_external_status = +{ $n ->
        [one] { $n } zewnętrzny
        [few] { $n } zewnętrzne
        [many] { $n } zewnętrznych
       *[other] { $n } zewnętrznych
    }
image_usage_line = osadzony { $times ->
        [one] { $times } raz
       *[other] { $times } razy
    } na { $pages ->
        [one] { $pages } stronie
        [few] { $pages } stronach
        [many] { $pages } stronach
       *[other] { $pages } stronach
    }

## Comparación de rastreos (`diff.rs`)

diff_title = Porównanie skanowań
label_before = Przed
label_after = Po
label_site = Witryna
label_common = Wspólne
# $urls and $findings arrive right-padded as text: no selector can work on them.
diff_crawl_counts = { $urls ->
        [one] { $urls } adres URL
        [few] { $urls } adresy URL
        [many] { $urls } adresów URL
       *[other] { $urls } adresów URL
    }, { $findings ->
        [one] { $findings } problem
        [few] { $findings } problemy
        [many] { $findings } problemów
       *[other] { $findings } problemów
    }
diff_common_note = { $n ->
        [one] { $n } adres URL obecny w obu skanowaniach
        [few] { $n } adresy URL obecne w obu skanowaniach
        [many] { $n } adresów URL obecnych w obu skanowaniach
       *[other] { $n } adresów URL obecnych w obu skanowaniach
    }
warnings_title = Ostrzeżenia
tag_inconclusive = NIEPEWNE
tag_warning = UWAGA
side_before = przed
side_after = po
warn_truncated = Skanowanie „{ $side }” zostało przerwane. Nie można stwierdzić, co zniknęło lub zostało rozwiązane: skanowanie mogło po prostu tam nie dotrzeć.
warn_truncated_by = Skanowanie „{ $side }” zostało przerwane z powodu { $reason }. Nie można stwierdzić, co zniknęło lub zostało rozwiązane: skanowanie mogło po prostu tam nie dotrzeć.
warn_list_mode = Skanowanie „{ $side }” to skanowanie listy: widziało tylko podane mu adresy URL. Nie można stwierdzić, co zniknęło lub zostało rozwiązane.
warn_unfinished = Skanowanie „{ $side }” nie zostało ukończone (stan „{ $status }”). Braki w nim nic nie znaczą.
warn_rules_changed = Katalog reguł się zmienił ({ $before } → { $after }). Część nowych lub rozwiązanych problemów może wynikać ze zmiany reguł, a nie z witryny.
warn_config_changed = Konfiguracja skanowania zmieniła się w polach: { $fields }. Jeśli się tego nie uwzględni, zmiany zakresu zostaną przypisane witrynie.
warn_scope_changed = Bazowe adresy URL się różnią: { $before } → { $after }. Ta sama witryna, inny punkt startowy.
warn_mode_changed = Tryby skanowania się różnią: { $before } → { $after }.
warn_missing_table = Skanowanie „{ $side }” jest starsze niż tabela `{ $table }`: to porównanie zostaje pominięte.
warn_order_inverted = Skanowanie podane jako „po” ({ $after_started }) rozpoczęło się wcześniej niż podane jako „przed” ({ $before_started }). Jeśli to niezamierzone, zamień oba argumenty miejscami: sekcje „Pogorszyło się” i „Poprawiło się” wychodzą odwrotnie.
worse_title = Pogorszyło się
label_new_findings = Nowe problemy
more_examples = … i jeszcze { $n }
label_pages_lost_index = Strony, które przestały być indeksowalne
label_status_worse = Kody stanu, które się pogorszyły
robots_content_changed = { $host }: treść się zmieniła
robots_blocks_all = UWAGA: teraz blokuje skanowanie całej witryny.
nothing_worse = Nic. Brak nowych problemów i stron usuniętych z indeksu.
better_title = Poprawiło się
label_findings_resolved = Rozwiązane problemy
label_pages_indexable_again = Strony ponownie indeksowalne
label_status_better = Kody stanu, które się poprawiły
other_title = Inne zmiany
label_new_urls = Nowe adresy URL
label_urls_gone = Zniknięte adresy URL
label_titles_changed = Zmienione tytuły
label_meta_changed = Zmienione opisy meta
label_canonicals_changed = Zmienione adresy kanoniczne
label_findings_persisted = Problemy nadal obecne
suppressed_title = Nie można stwierdzić
suppressed_intro = Jedno z dwóch skanowań jest niepełne, więc te różnice mogą wynikać z tego,
    czego nie przeskanowano, a nie z witryny. Nie są liczone jako zmiany:
label_candidate_new_urls = Możliwe nowe adresy URL
label_candidate_urls_gone = Możliwe zniknięte adresy URL
label_candidate_new_findings = Możliwe nowe problemy
label_candidate_resolved = Możliwe rozwiązane problemy
suppressed_advice = Powtórz oba skanowania z tym samym limitem, aby uzyskać rozstrzygające porównanie.
gate_title = Bramka CI
gate_inconclusive = Skanowanie „przed” jest niepełne: nie można stwierdzić nowych problemów, a
    --fail-on nie rozstrzyga. To nie jest wynik pozytywny.
# $n arrives as a raw, ungrouped integer: from 1000 up it is text and would take the default.
gate_fail_detail = { $n ->
        [one] { $n } nowy problem
        [few] { $n } nowe problemy
        [many] { $n } nowych problemów
       *[other] { $n } nowych problemów
    } [{ $severity }] — pasuje do --fail-on { $token }
gate_pass = PASS   nic z obserwowanego nie pojawia się jako nowy problem.

## Reabrir un diff guardado y listar sus filas (`diff_file.rs`)

label_saved = Zapisano
diff_saved_note = porównanie wygenerowane { $generated_at } przez crawlforge { $version } · { $path }
hint_diff_full_lists = Powyższe listy pokazują tylko kilka przykładów. Aby zobaczyć wszystkie wiersze, dodaj --rule <ID> lub
    --change <typ> do tego samego polecenia (np. --change url_removed).
warn_unknown_change_types = Pominięto { $n ->
        [one] { $n } zmianę
        [few] { $n } zmiany
        [many] { $n } zmian
       *[other] { $n } zmian
    } typu, którego ta wersja crawlforge nie zna ({ $types }). Plik zapisała nowsza wersja.
label_status_changed = Zmienione kody stanu
diff_no_matching_changes = Żadna zmiana w tym porównaniu nie pasuje.
error_diff_missing = { $path } nie istnieje. Pliki porównań zapisuje `crawlforge diff <przed> <po> --out <plik>`.
error_diff_needs_two = { $path } to plik skanowania, a porównanie wymaga dwóch: wcześniejszego i późniejszego skanowania. Pojedynczy plik jest akceptowany tylko wtedy, gdy jest porównaniem zapisanym z --out.
hint_diff_prev = Poprzednie skanowanie leży obok:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } nie jest ani plikiem skanowania, ani plikiem porównania. Pliki porównań zapisuje `crawlforge diff <przed> <po> --out <plik>`.
error_diff_too_new = { $path } to plik porównania w formacie { $version }, nowszym niż obsługuje ta wersja crawlforge ({ $supported }). Zaktualizuj crawlforge, aby go otworzyć.
error_diff_reopen_flag = { $flag } dotyczy porównywania dwóch skanowań; zapisane porównanie jest tylko odczytywane. Uruchom polecenie na skanowaniach: crawlforge diff <przed> <po> { $flag } …
error_unknown_change_type = { $value } nie jest typem zmiany. Dostępne: { $available }.
error_rule_with_change = --rule dopasowuje problemy, więc łączy się tylko z --change issue_appeared lub issue_resolved, a nie z { $change }.
site_wide_finding = (problem całej witryny)
site_placeholder = (witryna)
error_missing_file = { $path } nie istnieje
error_store_missing = { $path } nie istnieje. Pliki skanowania tworzą `crawlforge crawl`, `crawlforge audit` i `crawlforge list`.
error_not_a_crawl = { $path } nie wygląda na plik skanowania CrawlForge
error_different_sites = oba skanowania dotyczą różnych witryn: { $a } i { $b }.
    Adresy URL są dopasowywane po tekście, więc porównanie twierdziłoby, że cała witryna zniknęła, a pojawiła się inna. Porównuj każdą witrynę samą ze sobą.
error_fail_on_unknown = --fail-on: „{ $token }” nie jest ani regułą, ani wagą.
    Wagi to critical, high, medium, low i info; identyfikatory reguł wyświetla `crawlforge rules`.

## Panel de cartera (`portfolio.rs`)

portfolio_title = Panel portfolio
portfolio_range = skanowania od { $oldest } do { $newest }
warn_portfolio_date_spread = Najstarsze skanowanie ({ $oldest }) i najnowsze ({ $newest }) dzieli { $days ->
        [one] { $days } dzień
        [few] { $days } dni
        [many] { $days } dni
       *[other] { $days } dni
    }: ten panel nie jest migawką portfolio, a „co się zmieniło” obejmuje w każdej witrynie inny okres.
warn_portfolio_rules_versions = Nie wszystkie witryny przeskanowano z tym samym katalogiem reguł ({ $versions }). Reguły może brakować w witrynie, bo nie istniała, gdy tę witrynę skanowano.
warn_portfolio_core_versions = Skanowania pochodzą z różnych wersji silnika ({ $versions }).
portfolio_skipped_title = Pominięte pliki
portfolio_prev_not_input = to plik .prev.sqlite: stan „przed” skanowania leżącego obok, już użyty w porównaniu. Nie liczy się jako witryna.
error_portfolio_newer_schema = jego schemat (v{ $version }) jest nowszy, niż obsługuje ta wersja (v{ $supported }). Zaktualizuj crawlforge, aby go otworzyć.
error_portfolio_no_sites = w podanych ścieżkach nie znaleziono żadnych plików skanowania nadających się do portfolio. Pliki skanowania tworzą `crawlforge crawl`, `crawlforge audit` i `crawlforge list`; katalog jest przeszukiwany pod kątem plików *.sqlite.
error_portfolio_tier = panel portfolio nie należy do planu darmowego: wymaga wersji Pro. Polecenia dla jednej witryny (`report`, `diff`, `export`) pozostają dostępne.
error_portfolio_too_many = portfolio obejmuje { $n ->
        [one] { $n } witrynę
        [few] { $n } witryny
        [many] { $n } witryn
       *[other] { $n } witryn
    }, a ten plan pozwala na maksymalnie { $max }.
portfolio_changes_title = Co się zmieniło
portfolio_no_pairs = Żadna witryna nie ma obok siebie poprzedniego skanowania. Przeskanuj witrynę ponownie do tego samego pliku wyjściowego, a poprzednie skanowanie zostanie zachowane jako .prev.sqlite — to z nim porównuje ta sekcja.
portfolio_new_critical_high = Nowe problemy o wadze krytycznej i wysokiej
portfolio_none_critical_high = Brak nowych problemów o wadze krytycznej lub wysokiej w porównanych witrynach.
portfolio_rest_title = Pozostałe, witryna po witrynie
portfolio_no_changes = bez zmian
portfolio_pair_inconclusive = jedno ze skanowań jest niepełne: nie można stwierdzić, co zniknęło lub zostało rozwiązane
portfolio_pair_failed = porównanie z poprzednim skanowaniem nie powiodło się: { $reason }
portfolio_spread_title = Problemy w całym portfolio
portfolio_spread_intro = Reguła, która zgłasza problemy w większości witryn, rzadko dotyczy treści: zwykle chodzi o wspólny szablon lub wtyczkę — jedna poprawka załatwia wszystkie.
portfolio_spread_none = Żadna reguła nie zgłasza problemów w żadnej witrynie portfolio.
portfolio_glance_title = Portfolio w skrócie
th_site = witryna
th_crawled = data skanu
th_indexable = indeks.
flag_truncated = (przerwane)
flag_list_mode = (skanowanie listy)
flag_unfinished = (nieukończone: { $status })
hint_site_diff = Pełne porównanie jednej witryny:
portfolio_html_title = Panel portfolio
portfolio_sites_count = { $n ->
        [one] { $n } witryna
        [few] { $n } witryny
        [many] { $n } witryn
       *[other] { $n } witryn
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = zapisana w pliku skanowania
rules_config_source_after = zapisana w późniejszym skanowaniu
rules_config_source_file = z { $path }
warn_rules_config_differs = Oba skanowania mają zapisane różne konfiguracje reguł. Oba oceniono z tą samą konfiguracją ({ $source }), więc zmiana konfiguracji nie pojawia się jako nowe ani rozwiązane problemy.
warn_portfolio_rules_config = Niektóre witryny są odczytywane z własną konfiguracją reguł (wyłączone reguły / zmienione wagi): { $sites }. Ich liczby nie są porównywalne z pozostałymi reguła po regule.
rules_config_note = Konfiguracja reguł ({ $source }): { $disabled ->
        [one] { $disabled } wyłączona reguła
        [few] { $disabled } wyłączone reguły
        [many] { $disabled } wyłączonych reguł
       *[other] { $disabled } wyłączonych reguł
    }, { $changed ->
        [one] { $changed } zmieniona waga
        [few] { $changed } zmienione wagi
        [many] { $changed } zmienionych wag
       *[other] { $changed } zmienionych wag
    }. Plik skanowania zachowuje wszystkie problemy.
# "9 of 12 sites": the noun after "z" is genitive (z 1 witryny, z 5 witryn).
portfolio_sites_of = { $fired } z { $total ->
        [one] { $total } witryny
        [few] { $total } witryn
        [many] { $total } witryn
       *[other] { $total } witryn
    }
# Sites where the rule could not be evaluated (witryna, feminine). Starts with a space.
portfolio_inconclusive_suffix = {" "}({ $n ->
        [one] { $n } niepewna
        [few] { $n } niepewne
        [many] { $n } niepewnych
       *[other] { $n } niepewnych
    })
portfolio_pairs_line = { $pairs } z { $total ->
        [one] { $total } witryny
        [few] { $total } witryn
        [many] { $total } witryn
       *[other] { $total } witryn
    } { $pairs ->
        [one] ma
        [few] mają
        [many] ma
       *[other] ma
    } poprzednie skanowanie (.prev.sqlite) do porównania.

## Identificación de ficheros (`store_check.rs`)

error_store_is_diff = { $path } nie jest plikiem skanowania: to plik porównania, taki, jaki zapisuje `crawlforge diff --out`. Otwórz go poleceniem `crawlforge diff { $path }`.
    Pliki skanowania tworzą `crawlforge crawl` i `crawlforge audit`.
error_store_foreign = { $path } nie jest plikiem skanowania CrawlForge: nie ma jego tabel. Czy pochodzi z innego programu?
    Pliki skanowania tworzą `crawlforge crawl` i `crawlforge audit`.
error_store_not_sqlite = { $path } nie jest plikiem CrawlForge: nie jest nawet bazą danych SQLite.
    Pliki skanowania tworzą `crawlforge crawl` i `crawlforge audit`.

## Informe Markdown/HTML (`audit_report.rs`)

report_audit_of = Audyt witryny
report_audit_of_sites = Audyt { $n ->
        [one] { $n } witryny
        [few] { $n } witryn
        [many] { $n } witryn
       *[other] { $n } witryn
    }: { $sites }
sites_and_more = { $sites } i jeszcze { $n }
report_mode_line = Tryb `{ $mode }` · rozpoczęto { $started }
report_truncated_note = > **Skanowanie przerwane** (`{ $reason }`). Liczby obejmują to, co przeskanowano do tego momentu, a nie całą witrynę.
report_list_mode_note = > **Skanowanie listy.** Sprawdzono tylko adresy URL z listy: liczby opisują ten zbiór, a nie całą witrynę, a reguły wymagające pełnego grafu linków witryny nie są oceniane.
th_metric = Wskaźnik
th_value = Wartość
row_pages = Strony
row_indexable = Indeksowalne
row_errors_4xx_5xx = 4xx/5xx
report_more = … i jeszcze { $n }
report_more_run = … i jeszcze { $n } — uruchom: { $cmd }
report_full_list_run = … pełna lista: { $cmd }
html_title = Audyt SEO

## Catálogo de reglas (`rules.rs`)

th_severity = WAGA
th_category = KATEGORIA
th_scope = ZAKRES
th_tier = PLAN
th_name = NAZWA
lbl_category = kategoria
lbl_scope = zakres
lbl_tier = plan
lbl_reference = źródło
scope_page = strona
scope_site = witryna
rules_summary = Reguły: { $total } — { $free } w planie darmowym, { $page } na poziomie strony i { $site } na poziomie witryny.
error_unknown_category = brak reguł w kategorii „{ $category }”. Dostępne kategorie: { $list }.

## Cadenas de `main.rs`, listas para cablear

crawling = Skanowanie { $target }
error_invalid_seed = { $url } nie jest adresem URL, który można przeskanować: oczekiwano czegoś w rodzaju https://example.com/. Nic nie zostało utworzone.
file_line = Plik:      { $path }
previous_kept = Poprzedni: { $path } (zachowany, z ostatniego uruchomienia)
report_written = Raport zapisano w { $path }
markdown_report_written = Raport Markdown zapisano w { $path }
exported_csv = Wyeksportowano { $n ->
        [one] { $n } plik CSV
        [few] { $n } pliki CSV
        [many] { $n } plików CSV
       *[other] { $n } plików CSV
    } do { $dir }
exported_sheets = Wyeksportowano { $n ->
        [one] { $n } arkusz
        [few] { $n } arkusze
        [many] { $n } arkuszy
       *[other] { $n } arkuszy
    } do { $path }
bench_appended = Dopisano rekord benchmarku do { $path }
next_steps_title = Następne kroki
next_explain = Wyjaśnij każdy problem, z adresami URL, których dotyczy:
next_spreadsheet = Otwórz pełne dane jako arkusz kalkulacyjny:
next_compare_previous = Porównaj z poprzednim uruchomieniem tego skanowania:
next_compare_later = Po następnym wdrożeniu uruchom to samo skanowanie ponownie, a potem porównaj oba:
next_reread = Wyświetl to podsumowanie ponownie w dowolnej chwili:

## Autenticación básica HTTP (staging protegido)

auth_url_note = Używane jest uwierzytelnianie HTTP Basic z adresu URL: zostanie wysłane tylko do { $host } i nie zostanie zapisane w pliku skanowania.
auth_env_note = Używane jest uwierzytelnianie HTTP Basic z CRAWLFORGE_AUTH: zostanie wysłane tylko do { $host } i nie zostanie zapisane w pliku skanowania.
auth_base_ignored = Uwaga: usunięto dane logowania z --base: audyt katalogu czyta z dysku i nie wysyła żądań HTTP.
auth_list_ignored = Uwaga: dane logowania w adresach URL z listy są ignorowane i usuwane. Aby uwierzytelnić skanowanie, ustaw CRAWLFORGE_AUTH=użytkownik:hasło; dotyczy to tylko hosta pierwszego adresu URL.
progress_sitemaps = wykrywanie map witryny…
progress_finalize = ostatni przebieg: linki przychodzące i reguły całej witryny…
progress_crawl = pobrano: { $fetched } · w kolejce: { $queued } · { $rate } URL/s · problemy: { $findings }
progress_failed_suffix = {" "}· z błędem: { $n }

## Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`)

resuming = Wznawianie skanowania { $target }
resume_counts = Kontynuacja od miejsca przerwania: przeskanowano już { $done ->
        [one] { $done } adres URL
        [few] { $done } adresy URL
        [many] { $done } adresów URL
       *[other] { $done } adresów URL
    }, oczekujące: { $pending }.
resume_config_note = Używana jest konfiguracja zapisana w pliku: obowiązują ustawienia pierwotnego uruchomienia.
error_resume_schema_blocking = { $file } to skanowanie w schemacie v{ $version }, a migracja { $blocking } zmienia to, co zapisuje silnik: połowa pliku mówiłaby jedno, a połowa drugie. Uruchom skanowanie od nowa. Ten plik nadal można otworzyć poleceniami `report`, `export` i `diff`.
resume_robots_not_inherited = Uwaga: pierwotne skanowanie ignorowało robots.txt. To skanowanie go nie ignoruje — na to pozwalasz ty, a nie plik. Jeśli tego potrzebujesz, uruchom ponownie pierwotne polecenie skanowania.
resume_finished = Skanowanie wznowione i ukończone: pobrano jeszcze { $urls ->
        [one] { $urls } adres URL
        [few] { $urls } adresy URL
        [many] { $urls } adresów URL
       *[other] { $urls } adresów URL
    } w { $secs } s
interrupt_flushing = Przerwano: zapisywanie tego, co już przeskanowano… (naciśnij ponownie Ctrl+C, aby wyjść bez czekania)
interrupt_after_done = Przerwano. Plik skanowania jest kompletny; może pozostać częściowo zapisany eksport.
warn_wal_kept = Uwaga: inny program ma otwarty plik { $path }, więc pozostaje on w trybie WAL. Pliki -wal i -shm obok niego są częścią tego skanowania: kopiuj wszystkie trzy razem albo zamknij tamten program i uruchom na tym pliku `crawlforge report`, aby je scalić.
error_store_locked = inny proces crawlforge właśnie zapisuje { $path }. Zaczekaj, aż skończy, albo go zatrzymaj, zanim zaczniesz skanować lub wznawiać ten plik.
interrupted_saved = Skanowanie przerwane. Wszystko, co przeskanowano do tej pory, zapisano w { $path }. Kontynuuj poleceniem:
hint_previous_unfinished = Poprzednie skanowanie nie zostało ukończone. Aby je kontynuować zamiast zaczynać od nowa:
error_resume_done = { $path } to ukończone skanowanie: nie ma czego wznawiać.
    Aby ponownie przeprowadzić audyt witryny, uruchom pierwotne polecenie skanowania; ukończony plik zostanie zachowany obok jako .prev.sqlite.
error_resume_status = Nie można wznowić { $path }: jego stan to „{ $status }”. Wznawiać można tylko przerwane skanowania (stan „running” lub „paused”).
error_resume_schema = { $path } używa schematu v{ $found }, a ta wersja zapisuje v{ $expected }: przerwane skanowanie może wznowić tylko wersja, która je rozpoczęła. Uruchom skanowanie od nowa.
error_resume_config = nie można odczytać konfiguracji skanowania zapisanej w { $path }. Czy plik zapisała inna wersja? Uruchom skanowanie od nowa.
