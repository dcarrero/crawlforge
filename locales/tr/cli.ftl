### CrawlForge — komut satırı mesajları (Türkçe).
###
### English (locales/en/) is the source and the authority; every other folder is a
### translation of it. CONTRIBUTING.md, "Adding or improving a language", explains the rules.
### The comments above each message are context for translators: they are not shown.
###
### Turkish notes: suffixes are never attached to a variable (its vowel harmony is unknown);
### the sentence is built around it instead ("Dosya yok: { $path }").

## Severity names

# Severity with an initial capital, for report headings.
severity_label_critical = Kritik
severity_label_high = Yüksek
severity_label_medium = Orta
severity_label_low = Düşük
severity_label_info = Bilgi
# Severity in lower case, for table columns.
severity_word_critical = kritik
severity_word_high = yüksek
severity_word_medium = orta
severity_word_low = düşük
severity_word_info = bilgi

## Non-indexability reasons

reason_noindex = noindex
reason_canonicalised = kanonikleştirilmiş
reason_robots = robots
reason_redirect = yönlendirme
reason_4xx = 4xx
reason_5xx = 5xx
reason_not_html = HTML değil

## Resumen del rastreo (`report.rs`)

crawl_finished = Tarama tamamlandı: { $urls } URL, { $secs } sn
crawl_failed_suffix = {" "}({ $n } başarısız)
crawl_truncated = Tarama { $reason } nedeniyle kesildi. Gösterilen bulgular o ana kadar bulunanlardır.
# A 429 is not a page's status: the site asked the crawler to slow down. The pages
# were left pending, and the useful thing to say is how to get them.
crawl_rate_limited_note = Site, taramaya hız sınırı uyguladı: { $n } sayfa yeniden denemelerden sonra da 429 Too Many Requests yanıtı verdi; bozuk olarak raporlanmadı, beklemede bırakıldı. Sitenin tamamını gerektiren kurallar, bu sayfalar alınana kadar değerlendirilmez. Daha sonra şunu çalıştırın:
    {"    "}crawlforge resume { $path }
    ya da daha düşük bir --concurrency ile yeniden tarayın veya sitenin güvenlik duvarında CrawlForge user-agent'ına izin verin.
report_rate_limited_note = > **Site, taramaya hız sınırı uyguladı.** { $n } sayfa 429 Too Many Requests yanıtı verdi ve beklemede bırakıldı: sayılmıyor, bozuk olarak da raporlanmıyor; sitenin tamamını gerektiren kurallar değerlendirilmiyor. `crawlforge resume` bu sayfaları alır.
crawl_list_mode_note = Liste taraması: yalnızca verdiğiniz URL'ler denetlendi. Sitenin eksiksiz bağlantı grafiğini gerektiren kurallar (yetim sayfalar, gelen bağlantılar, derinlik) değerlendirilmez.
external_unchecked = { $n } harici bağlantı kontrol edilmedi: max_external sınırına ulaşıldı. Sınırı --max-external ile yükseltebilirsiniz.
external_unregistered = { $n } harici bağlantı kaydedilmedi bile: max_external_urls sınırına ulaşıldı. Bu sınır, yüz binlerce farklı sunucuya bağlantı veren tek bir sayfa gibi durumlar için vardır.
external_never_checked = { $n } harici bağlantı kaydedildi ancak hiç kontrol edilmedi.
external_offline = { $n } harici bağlantı kontrol edilmedi: bir klasörün denetimi ağ isteği yapmaz. Bunları kontrol etmek için yayındaki siteyi `crawlforge crawl <URL>` ile tarayın.
external_check_disabled = Harici bağlantılar kontrol edilmedi (--no-external-check): bu raporda bozuk harici bağlantı görünmemesi, hiç olmadığı anlamına gelmez.
external_checked_note = { $n } harici bağlantı kontrol edildi.
rules_not_evaluated = Eksiksiz bağlantı grafiğini gerektiren kurallar değerlendirilmedi: { $rules }.
results_title = Sonuçlar
label_urls = URL'ler
label_html_pages = HTML sayfaları
label_links = Bağlantılar
note_internal = { $n } dahili
note_indexable = { $n } dizine eklenebilir
heading_status_codes = Durum kodları
label_no_response = yanıt yok
note_request_failed = istek başarısız oldu
label_never_requested = Hiç istenmedi
note_never_requested = (keşfedildi, ancak tarama bunları almadan durdu)
label_out_of_scope = Kapsam dışı
note_out_of_scope = (denetlenen sayfalardan bağlantı verilmiş, ancak bu taramanın kapsamı dışında)
heading_non_indexable = Dizine eklenemeyenler, nedeni
heading_findings = Bulgular
no_findings = Bulgu yok.
heading_excluded = Hariç tutulanlar
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = { $k } şablon sorunu ({ $pages } sayfa)
plus_more_findings = {" "}+ { $n } bulgu daha
example_urls = örn. { $urls }

## Reformulación de reglas dominantes

pervasive_note = sitenin %{ $pct } kadarı
deep_pages_summary = { $pages } sayfa { $max } tıklamadan daha derinde — sitenin %{ $pct } kadarı (tipik derinlik { $low }–{ $high }, en derin { $deepest })
rule_deep_sorted = Tıklama derinliğine göre sıralı, en derin olan önce; ilk sütun ana sayfadan uzaklığı tıklama sayısı olarak verir.
hint_full_lists = Bir kuralın tam URL listesi: { $cmd }

## Listado de una regla (`report --rule`)

rule_affected_urls = { $n } etkilenen URL
rule_template_group = Şablon grubu — { $n } sayfa
rule_group_cause = neden: { $detail }
rule_other_pages = Etkilenen diğer sayfalar — { $n }
rule_no_findings = Bu taramada { $rule } kuralı için bulgu yok.
hint_who_links = Bozuk bir bağlantı, ona bağlantı veren sayfada düzeltilir. Bir URL'ye bağlantı veren sayfalar, bağlantı metinleriyle birlikte:
error_unknown_rule = { $rule } bir kural kimliği değil. Katalog `crawlforge rules` ile listelenir.
error_all_blocked_by_robots = robots.txt tüm URL'leri engelliyor: denetlenecek bir şey yok. Site sizinse --ignore-robots ile yeniden deneyebilirsiniz.
error_all_excluded_by_pattern = tüm URL'ler --exclude/--include kalıplarınız nedeniyle atlandı: denetlenecek bir şey yok. Kalıpları kontrol edin.
error_no_urls_fetched = tarama hiçbir URL alamadı. Hedefi kontrol edin.
error_dead_seed = bağlantı kurulamadı: { $url } ({ $cause }). URL'yi kontrol edin.
cause_dns = alan adı yok ya da çözümlenmiyor
cause_tls = TLS sertifikası geçerli değil
cause_timeout = sunucu zamanında yanıt vermedi
cause_connection = bağlantı reddedildi
cause_no_response = yanıt yok
warn_base_mismatch = Uyarı: { $total } kanonik URL içinden { $n } tanesi verilen tabana ({ $base }) değil, şu adrese işaret ediyor: { $origin }.
    Dizine eklenebilirlik sonuçları bu seçeneğe bağlıdır: site
    { $origin } adresinde yayındaysa denetimi --base { $origin } ile tekrarlayın

## Ficha de una URL (`inspect.rs`)

inspect_error_not_found = bu taramada yok: { $url }
inspect_closest = Dosyadaki en yakın URL'ler:
inspect_status_title = Durum
label_http_status = HTTP durumu
label_content_type = İçerik türü
label_response_time = Yanıt süresi
label_depth = Derinlik
note_depth_clicks = başlangıç URL'sinden tıklama
label_in_sitemap = Site haritasında
word_yes = evet
word_no = hayır
label_crawl_state = Tarama durumu
state_note_pending = keşfedildi, ancak tarama onu hiç almadı
state_note_excluded = taramadan hariç tutuldu ({ $reason })
state_note_external_checked = harici URL — durumu kontrol edildi, içeriği denetlenmedi
state_note_external_not_checked = harici URL — durumu kontrol edilmedi ({ $reason })
state_note_external_unchecked = harici URL — hiç istenmedi
state_note_out_of_scope = keşfedildi, ancak bu taramanın kapsamı dışında
state_note_error = istek başarısız oldu ({ $detail })
region_unknown = belirsiz
inspect_page_title = Sayfa
label_page_title = Başlık
label_meta_description = Meta açıklama
label_h1 = H1
label_word_count = Kelime sayısı
label_canonical = Kanonik
canonical_self = kendisi
label_indexable = Dizine eklenebilir
indexable_no = hayır — { $reason }
chars_suffix = { $n } karakter
inspect_redirects_title = Yönlendirme zinciri
inlinks_title = Gelen bağlantılar ({ $n })
inlinks_by_region = Bölgeye göre: { $breakdown } · { $nofollow } nofollow
inlinks_none = Taranan hiçbir sayfa bu URL'ye bağlantı vermiyor.
inlinks_sample_heading = Bağlantı veren sayfalar, önce içerik bağlantıları:
no_anchor = (bağlantı metni yok)
inspect_truncated = … { $n } tanesi gösteriliyor — tam liste: { $cmd }
images_title = Görseller ({ $n })
image_no_alt = (alt yok)
image_usage_title = Görsel olarak kullanımı
# Outbound links header. The three variables are counts.
outlinks_title = Giden bağlantılar ({ $total }: { $internal } dahili, { $external } harici)
# Suffix that keeps external URLs apart from the status-code count.
note_external_status = +{ $n } harici
image_usage_line = { $pages } sayfada { $times } kez gömülü

## Comparación de rastreos (`diff.rs`)

diff_title = Tarama karşılaştırması
label_before = Önce
label_after = Sonra
label_site = Site
label_common = Ortak
diff_crawl_counts = { $urls } URL, { $findings } bulgu
diff_common_note = iki taramada da bulunan { $n } URL
warnings_title = Uyarılar
tag_inconclusive = SONUÇSUZ
tag_warning = UYARI
side_before = önce
side_after = sonra
warn_truncated = '{ $side }' taraması kesildi. Neyin kaybolduğu ya da çözüldüğü kesin olarak söylenemez: tarama oraya hiç ulaşmamış olabilir.
warn_truncated_by = '{ $side }' taraması şu nedenle kesildi: { $reason }. Neyin kaybolduğu ya da çözüldüğü kesin olarak söylenemez: tarama oraya hiç ulaşmamış olabilir.
warn_list_mode = '{ $side }' taraması bir liste taraması: yalnızca kendisine verilen URL'leri gördü. Neyin kaybolduğu ya da çözüldüğü kesin olarak söylenemez.
warn_unfinished = '{ $side }' taraması tamamlanmadı (durum: '{ $status }'). Bu taramada bir şeyin bulunmaması hiçbir şey ifade etmez.
warn_rules_changed = Kural kataloğu değişti ({ $before } → { $after }). Yeni ya da çözülen bulguların bir kısmı siteden değil, kurallardan kaynaklanıyor olabilir.
warn_config_changed = Tarama yapılandırması şu alanlarda değişti: { $fields }. Bu göz önünde bulundurulmazsa kapsam değişiklikleri siteye mal edilir.
warn_scope_changed = Taban URL'ler farklı: { $before } → { $after }. Aynı site, farklı başlangıç noktası.
warn_mode_changed = Tarama modları farklı: { $before } → { $after }.
warn_missing_table = '{ $side }' taraması `{ $table }` tablosundan daha eski: bu karşılaştırma atlanıyor.
warn_order_inverted = 'sonra' olarak verilen tarama ({ $after_started }), 'önce' olarak verilenden ({ $before_started }) daha erken başlamış. Bu bilinçli değilse iki argümanın yerini değiştirin: 'Kötüleşenler' ile 'İyileşenler' ters çıkar.
worse_title = Kötüleşenler
label_new_findings = Yeni bulgular
more_examples = … ve { $n } tane daha
label_pages_lost_index = Artık dizine eklenemeyen sayfalar
label_status_worse = Kötüleşen durum kodları
robots_content_changed = { $host }: içerik değişti
robots_blocks_all = DİKKAT: artık sitenin tamamının taranmasını engelliyor.
nothing_worse = Hiçbir şey. Yeni bulgu yok, dizinden düşen sayfa yok.
better_title = İyileşenler
label_findings_resolved = Çözülen bulgular
label_pages_indexable_again = Yeniden dizine eklenebilen sayfalar
label_status_better = İyileşen durum kodları
other_title = Diğer değişiklikler
label_new_urls = Yeni URL'ler
label_urls_gone = Kaybolan URL'ler
label_titles_changed = Değişen başlıklar
label_meta_changed = Değişen meta açıklamalar
label_canonicals_changed = Değişen kanonik URL'ler
label_findings_persisted = Hâlâ süren bulgular
suppressed_title = Kesin olarak söylenemeyenler
# Multilínea: el punto de llamada imprime cada línea con su sangría.
suppressed_intro = İki taramadan biri eksik; bu farklar siteden değil, hiç taranmamış kısımlardan
    kaynaklanıyor olabilir. Değişiklik olarak sayılmazlar:
label_candidate_new_urls = Olası yeni URL'ler
label_candidate_urls_gone = Olası kaybolan URL'ler
label_candidate_new_findings = Olası yeni bulgular
label_candidate_resolved = Olası çözülen bulgular
suppressed_advice = Kesin bir karşılaştırma için iki tarafı da aynı sınırla yeniden tarayın.
gate_title = CI kontrolü
# Multilínea: el punto de llamada imprime cada línea con su sangría.
gate_inconclusive = 'önce' taraması eksik: yeni bulgular kesin olarak söylenemez ve
    --fail-on bir karar vermez. Bu bir PASS değildir.
gate_fail_detail = { $n } yeni bulgu [{ $severity }] — --fail-on { $token } ile eşleşiyor
gate_pass = PASS   izlenen hiçbir şey yeni bulgu olarak görünmüyor.

## Reabrir un diff guardado y listar sus filas (`diff_file.rs`)

label_saved = Kaydedildi
diff_saved_note = karşılaştırma { $generated_at } tarihinde crawlforge { $version } ile oluşturuldu · { $path }
# Multilínea: el punto de llamada imprime cada línea con su sangría.
hint_diff_full_lists = Yukarıdaki listeler yalnızca birkaç örnek gösterir. Tüm satırlar için aynı komuta --rule <ID> ya da
    --change <type> ekleyin (örn. --change url_removed).
warn_unknown_change_types = Bu crawlforge sürümünün tanımadığı türde { $n } değişiklik ({ $types }) dışarıda bırakıldı. Dosya daha yeni bir sürümle yazılmış.
label_status_changed = Değişen durum kodları
diff_no_matching_changes = Bu karşılaştırmada eşleşen değişiklik yok.
error_diff_missing = dosya yok: { $path }. Karşılaştırma dosyaları `crawlforge diff <before> <after> --out <file>` ile kaydedilir.
error_diff_needs_two = { $path } bir tarama dosyası; karşılaştırma için ise iki dosya gerekir: önceki tarama ve sonraki tarama. Tek bir dosya yalnızca --out ile kaydedilmiş bir karşılaştırmaysa kabul edilir.
hint_diff_prev = Önceki taraması hemen yanında:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } ne bir tarama dosyası ne de bir karşılaştırma dosyası. Karşılaştırma dosyaları `crawlforge diff <before> <after> --out <file>` ile kaydedilir.
error_diff_too_new = { $path } bir karşılaştırma dosyası, biçimi { $version }: bu crawlforge sürümünün anladığından ({ $supported }) daha yeni. Açmak için crawlforge'u güncelleyin.
error_diff_reopen_flag = { $flag } iki taramayı karşılaştırırken geçerlidir; kaydedilmiş bir karşılaştırma yalnızca okunur. Taramalar üzerinde çalıştırın: crawlforge diff <before> <after> { $flag } …
error_unknown_change_type = { $value } bir değişiklik türü değil. Kullanılabilenler: { $available }.
error_rule_with_change = --rule bulgularla eşleşir; bu yüzden yalnızca --change issue_appeared ya da issue_resolved ile birlikte kullanılabilir. Şununla kullanılamaz: { $change }.
site_wide_finding = (site geneli bulgu)
site_placeholder = (site)
error_missing_file = dosya yok: { $path }
error_store_missing = dosya yok: { $path }. Tarama dosyaları `crawlforge crawl`, `crawlforge audit` ve `crawlforge list` ile oluşturulur.
error_not_a_crawl = bir CrawlForge tarama dosyasına benzemiyor: { $path }
error_different_sites = iki tarama farklı sitelere ait: { $a } ve { $b }.
    URL'ler metinlerine göre eşleştirilir; karşılaştırma tüm sitenin kaybolduğunu ve başka bir sitenin ortaya çıktığını söylerdi. Her siteyi kendisiyle karşılaştırın.
error_fail_on_unknown = --fail-on: '{ $token }' bir kural ya da önem derecesi değil.
    Önem dereceleri: critical, high, medium, low ve info; kural kimlikleri `crawlforge rules` ile listelenir.

## Panel de cartera (`portfolio.rs`)

portfolio_title = Portföy paneli
portfolio_range = { $oldest } – { $newest } arası taramalar
# Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
# changed" would silently mean a different period on every site.
warn_portfolio_date_spread = En eski tarama ({ $oldest }) ile en yenisi ({ $newest }) arasında { $days } gün var: bu panel portföyün anlık görüntüsü değil ve 'Neler değişti' her sitede farklı bir dönemi kapsıyor.
# Trap 3.2: two files crawled with different catalogs are not comparable without
# saying so — a rule can be "missing" on a site because it did not exist yet.
warn_portfolio_rules_versions = Sitelerin hepsi aynı kural kataloğuyla taranmadı ({ $versions }). Bir kural, o site tarandığında henüz var olmadığı için o sitede görünmeyebilir.
warn_portfolio_core_versions = Taramalar farklı motor sürümlerinden geliyor ({ $versions }).
portfolio_skipped_title = Kenara ayrılan dosyalar
# A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
portfolio_prev_not_input = bu bir .prev.sqlite: yanındaki taramanın 'önce' hali, karşılaştırmada zaten kullanıldı. Site olarak sayılmaz.
error_portfolio_newer_schema = şeması (v{ $version }) bu derlemenin anladığından (v{ $supported }) daha yeni. Açmak için crawlforge'u güncelleyin.
error_portfolio_no_sites = verilen yollarda portföy olarak kullanılabilecek tarama dosyası bulunamadı. Tarama dosyaları `crawlforge crawl`, `crawlforge audit` ve `crawlforge list` ile oluşturulur; bir dizinde *.sqlite dosyaları aranır.
error_portfolio_tier = portföy paneli ücretsiz plana dahil değil: Pro gerektirir. Tek site komutları (`report`, `diff`, `export`) kullanılmaya devam eder.
error_portfolio_too_many = portföyde { $n } site var; bu plan en fazla { $max } site destekliyor.
portfolio_changes_title = Neler değişti
portfolio_no_pairs = Hiçbir sitenin yanında önceki bir tarama yok. Aynı çıktı dosyasına yeniden tarayın; önceki tarama .prev.sqlite olarak saklanır — bu bölümün karşılaştırdığı 'önce' odur.
portfolio_new_critical_high = Yeni kritik ve yüksek bulgular
portfolio_none_critical_high = Karşılaştırılan hiçbir sitede yeni kritik ya da yüksek bulgu yok.
portfolio_rest_title = Geri kalanı, site site
portfolio_no_changes = değişiklik yok
# The diff of that pair cannot assert absences: one side is incomplete.
portfolio_pair_inconclusive = taraflardan biri eksik: neyin kaybolduğu ya da çözüldüğü kesin olarak söylenemez
portfolio_pair_failed = önceki taramayla karşılaştırması başarısız oldu: { $reason }
portfolio_spread_title = Portföy genelinde başarısız olanlar
portfolio_spread_intro = Sitelerin çoğunda tetiklenen bir kuralın kaynağı nadiren içeriktir: genellikle ortak bir şablon ya da eklentidir — hepsini düzelten tek bir değişiklik.
portfolio_spread_none = Portföyün hiçbir sitesinde tetiklenen kural yok.
portfolio_glance_title = Bir bakışta portföy
th_site = site
th_crawled = tarandı
th_indexable = dizin.
th_sev_critical = kritik
th_sev_high = yüksek
th_sev_medium = orta
th_sev_low = düşük
th_sev_info = bilgi
flag_truncated = (kesildi)
flag_list_mode = (liste taraması)
flag_unfinished = (tamamlanmadı: { $status })
hint_site_diff = Tek bir sitenin tam karşılaştırması:
portfolio_html_title = Portföy paneli
portfolio_sites_count = { $n } site

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = tarama dosyasında saklanan
rules_config_source_after = sonraki taramada saklanan
rules_config_source_file = kaynak: { $path }
warn_rules_config_differs = İki tarama farklı kural yapılandırmaları saklıyor. İki taraf da aynı yapılandırmayla ({ $source }) değerlendirildi; böylece bir yapılandırma değişikliği yeni ya da çözülen bulgu olarak görünmez.
warn_portfolio_rules_config = Bazı siteler kendi kural yapılandırmalarıyla okunuyor (devre dışı kurallar / değiştirilmiş önem dereceleri): { $sites }. Bu sitelerin sayıları diğerleriyle kural kural karşılaştırılamaz.
# "Rule configuration (stored in the crawl file): 2 rules disabled, 1 severity changed."
rules_config_note = Kural yapılandırması ({ $source }): { $disabled } kural devre dışı, { $changed } önem derecesi değiştirildi. Tarama dosyası tüm bulguları saklar.
# "9 of 12 sites": the numerator of the spread table. The noun follows $total.
portfolio_sites_of = { $fired } / { $total } site
# " (2 inconclusive)": sites where the rule could not be evaluated. Starts with a space.
# Never shown for zero.
portfolio_inconclusive_suffix = {" "}({ $n } sonuçsuz)
# The noun follows $total and the verb follows $pairs: "1 of 5 sites has".
portfolio_pairs_line = { $pairs } / { $total } sitenin yanında karşılaştırma için önceki bir tarama (.prev.sqlite) var.

## Identificación de ficheros (`store_check.rs`)

error_store_is_diff = { $path } bir tarama dosyası değil: `crawlforge diff --out` komutunun kaydettiği türden bir karşılaştırma dosyası. `crawlforge diff { $path }` ile açın.
    Tarama dosyaları `crawlforge crawl` ve `crawlforge audit` ile oluşturulur.
error_store_foreign = { $path } bir CrawlForge tarama dosyası değil: gerekli tabloları içermiyor. Başka bir programa mı ait?
    Tarama dosyaları `crawlforge crawl` ve `crawlforge audit` ile oluşturulur.
error_store_not_sqlite = { $path } bir CrawlForge dosyası değil: bir SQLite veritabanı bile değil.
    Tarama dosyaları `crawlforge crawl` ve `crawlforge audit` ile oluşturulur.

## Informe Markdown/HTML (`audit_report.rs`)

report_audit_of = Denetim:
report_audit_of_sites = { $n } sitenin denetimi: { $sites }
sites_and_more = { $sites } ve { $n } tane daha
report_mode_line = Mod `{ $mode }` · başlangıç: { $started }
report_truncated_note = > **Kesilmiş tarama** (`{ $reason }`). Sayılar sitenin tamamını değil, o ana kadar taranan kısmı kapsar.
report_list_mode_note = > **Liste taraması.** Yalnızca listelenen URL'ler denetlendi: sayılar sitenin tamamını değil bu kümeyi anlatır ve sitenin eksiksiz bağlantı grafiğini gerektiren kurallar değerlendirilmez.
th_metric = Ölçüt
th_value = Değer
row_pages = Sayfalar
row_indexable = Dizine eklenebilir
row_errors_4xx_5xx = 4xx/5xx
report_more = … { $n } tane daha
report_more_run = … { $n } tane daha — çalıştırın: { $cmd }
report_full_list_run = … tam liste: { $cmd }
html_title = SEO denetimi

## Catálogo de reglas (`rules.rs`)

th_severity = ÖNEM
th_category = KATEGORİ
th_scope = KAPSAM
th_tier = PLAN
th_name = AD
lbl_category = kategori
lbl_scope = kapsam
lbl_tier = plan
lbl_reference = kaynak
scope_page = sayfa
scope_site = site
rules_summary = { $total } kural: { $free } tanesi ücretsiz planda; { $page } tanesi sayfa düzeyinde, { $site } tanesi site genelinde.
error_unknown_category = '{ $category }' kategorisinde kural yok. Kategoriler: { $list }.

## Cadenas de `main.rs`, listas para cablear

crawling = Taranıyor: { $target }
error_invalid_seed = { $url } taranabilir bir URL değil: https://example.com/ gibi bir adres bekleniyordu. Hiçbir şey oluşturulmadı.
file_line = Dosya:    { $path }
previous_kept = Önceki:   { $path } (korundu, son çalıştırmadan)
report_written = Rapor yazıldı: { $path }
markdown_report_written = Markdown raporu yazıldı: { $path }
exported_csv = { $n } CSV dosyası dışa aktarıldı: { $dir }
exported_sheets = { $n } çalışma sayfası dışa aktarıldı: { $path }
bench_appended = Performans ölçümü kaydı eklendi: { $path }
next_steps_title = Sonraki adımlar
next_explain = Her bulguyu, etkilenen URL'leriyle birlikte açıklayın:
next_spreadsheet = Tüm verileri bir elektronik tablo olarak açın:
next_compare_previous = Bu taramanın bir önceki çalıştırmasıyla karşılaştırın:
next_compare_later = Bir sonraki yayına almanızdan sonra aynı taramayı yeniden çalıştırın ve ikisini karşılaştırın:
next_reread = Bu özeti istediğiniz zaman yeniden okuyun:

## Autenticación básica HTTP (staging protegido)

auth_url_note = URL'deki HTTP Basic kimlik doğrulaması kullanılıyor: yalnızca şu sunucuya gönderilecek ve tarama dosyasında saklanmayacak: { $host }
auth_env_note = CRAWLFORGE_AUTH değişkenindeki HTTP Basic kimlik doğrulaması kullanılıyor: yalnızca şu sunucuya gönderilecek ve tarama dosyasında saklanmayacak: { $host }
auth_base_ignored = Uyarı: --base içindeki kimlik bilgileri kaldırıldı: bir dizin denetimi diskten okur ve HTTP isteği yapmaz.
auth_list_ignored = Uyarı: URL listesi girdilerindeki kimlik bilgileri yok sayılır ve kaldırılır. Taramada kimlik doğrulaması için CRAWLFORGE_AUTH=user:password ayarlayın; yalnızca ilk URL'nin sunucusuna uygulanır.
progress_sitemaps = site haritaları keşfediliyor…
progress_finalize = son geçiş: gelen bağlantılar ve site geneli kurallar…
progress_crawl = { $fetched } tarandı · { $queued } kuyrukta · { $rate } URL/sn · { $findings } bulgu
progress_failed_suffix = {" "}· { $n } başarısız

## Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`)

resuming = Tarama sürdürülüyor: { $target }
resume_counts = Kaldığı yerden devam ediliyor: { $done } URL zaten tarandı, { $pending } URL beklemede.
resume_config_note = Dosyada kayıtlı yapılandırma kullanılıyor: ilk çalıştırmanın ayarları geçerli.
error_resume_schema_blocking = { $file } şema v{ $version } ile yazılmış bir tarama ve { $blocking } numaralı geçiş motorun yazdığı veriyi değiştiriyor: dosyanın yarısı bir şey, diğer yarısı başka bir şey söylerdi. Taramayı yeniden çalıştırın. Bu dosyayı yine de `report`, `export` ve `diff` ile açabilirsiniz.
resume_robots_not_inherited = Not: ilk tarama robots.txt dosyasını yok saymıştı. Bu tarama saymıyor — bu izni bir dosya değil, siz verirsiniz. Gerekirse ilk tarama komutunu yeniden çalıştırın.
resume_finished = Tarama sürdürüldü ve tamamlandı: { $urls } URL daha, { $secs } sn
interrupt_flushing = Kesildi: taranmış olanlar kaydediliyor… (beklemeden çıkmak için yeniden Ctrl+C)
interrupt_after_done = Kesildi. Tarama dosyası eksiksiz; yarım yazılmış bir dışa aktarma dosyası kalmış olabilir.
warn_wal_kept = Not: başka bir program şu dosyayı açık tutuyor, bu yüzden dosya WAL modunda kalıyor: { $path }. Yanındaki -wal ve -shm dosyaları bu taramanın parçasıdır: üçünü birlikte kopyalayın ya da diğer programı kapatıp dosya üzerinde `crawlforge report` çalıştırarak onları birleştirin.
error_store_locked = başka bir crawlforge işlemi şu anda bu dosyaya yazıyor: { $path }. Bu dosyayı taramadan ya da sürdürmeden önce işlemin bitmesini bekleyin veya onu durdurun.
interrupted_saved = Tarama kesildi. O ana kadar taranan her şey kaydedildi: { $path }. Devam etmek için:
hint_previous_unfinished = Önceki tarama tamamlanmamıştı. Baştan başlamak yerine ona devam etmek için:
error_resume_done = { $path } tamamlanmış bir tarama: sürdürülecek bir şey yok.
    Siteyi yeniden denetlemek için ilk tarama komutunu çalıştırın; tamamlanmış dosya yanında .prev.sqlite olarak saklanır.
error_resume_status = { $path } sürdürülemez: durumu '{ $status }'. Yalnızca kesilmiş taramalar (durumu 'running' ya da 'paused' olanlar) sürdürülebilir.
error_resume_schema = { $path } şema v{ $found } kullanıyor, bu derleme ise v{ $expected } yazıyor: kesilmiş bir tarama yalnızca onu başlatan sürümle sürdürülebilir. Taramayı yeniden çalıştırın.
error_resume_config = şu dosyada saklanan tarama yapılandırması okunamıyor: { $path }. Dosya başka bir sürümle mi yazıldı? Taramayı yeniden çalıştırın.
