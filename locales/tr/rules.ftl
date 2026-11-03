### CrawlForge — kural kataloğu: adlar ve açıklamalar (Türkçe).
###
### English (locales/en/) is the source and the authority; every other folder is a
### translation of it. CONTRIBUTING.md, "Adding or improving a language", explains the rules.
### The comments above each message are context for translators: they are not shown.

### Each rule is one message: its ID, the short name as the value, and the description
### as the `.description` attribute. Backticks and <tags> are literal text, keep them.

ASSET-BROKEN = Bozuk CSS veya JS
    .description = Sitedeki bir CSS ya da JS dosyası 4xx veya 5xx döndürüyor. Google sayfayı dizine eklemek için oluşturur (render eder); eksik bir stil dosyası, sayfayı stilsiz —dolayısıyla mobil uyumlu olmayan— bir sayfa olarak görmesine, eksik bir komut dosyası da istemci tarafında oluşturulan içeriğin boş kalmasına yol açabilir. Dosya karmalarının (hash) uyuşmadığı bir dağıtımın tipik sonucudur.
ASSET-CSS-HEAVY = Aşırı büyük stil dosyası
    .description = Stil dosyası, sunulduğu haliyle 100 KB'ın üzerinde. Tarayıcı bu dosya gelmeden hiçbir şey çizmez; bu yüzden bu baytlar bir komut dosyasınınkinden daha pahalıdır: ilk boyamanın tamamını geciktirir. Eşiğin daha düşük olmasının nedeni budur. Neredeyse her zaman, onda biri kullanılmak üzere eksiksiz yüklenmiş bir framework'tür.
ASSET-FORM-BROKEN = Bozuk URL'ye gönderilen form
    .description = Bir formun `action` özniteliği, sitenizde 4xx veya 5xx döndüren bir URL'ye işaret ediyor. Form düzgün görünür, sorunsuz doldurulur ve gönderilince kaybolur: doğrudan müşteri kaybettiren az sayıdaki hatadan biridir ve kimse fark etmez, çünkü bu hatayla karşılaşan kişi bir şey söylemeden ayrılır. Yalnızca GET ile gönderilen formlar kontrol edilir —bir arama kutusu, bir katalog filtresi—: POST'u kontrol etmek onu göndermek anlamına gelir ve bu araç form göndermez.
ASSET-IFRAME-BROKEN = Bozuk gömülü çerçeve
    .description = Sayfadaki bir `<iframe>`, sitenizde 4xx veya 5xx döndüren bir URL'ye işaret ediyor; ziyaretçi haritanın, videonun ya da formun olması gereken yerde boş bir alan görür. İçeren sayfada iz bırakmaz —HTML yine geçerlidir—; aylarca fark edilmemesinin nedeni budur.
ASSET-IMG-BROKEN = Bozuk görsel
    .description = Sitenin işaret ettiği bir görsel 4xx veya 5xx döndürüyor. Boşluk sayfada görünür, görsel, görsel aramasında var olmaz ve her ziyaret bir isteği hataya harcar. Neredeyse her zaman, yüklemeler klasörünü geride bırakan bir taşıma işlemi ya da elle yazılmış bir yoldur.
ASSET-IMG-EMPTY-ALT-LINK = Yalnızca boş alt'lı görsel içeren bağlantı
    .description = Tek içeriği `alt=""` olan bir görselden ibaret bir bağlantının erişilebilir adı yoktur: ekran okuyucu kullanan biri yalnızca «bağlantı» duyar, arama motoru da nereye gittiğine dair hiçbir sinyal almaz. Tipik örnek, genellikle sitede en çok tekrarlanan bağlantı olan başlıktaki logodur. Burada `alt` dekoratif değildir: bağlantı metninin kendisidir.
ASSET-IMG-HEAVY = Aşırı büyük görsel
    .description = Görsel 200 KB'ın üzerinde. Mobilde zayıf LCP'nin en yaygın nedenidir: ana öğenin çizilmesini geciktirir ve yavaş bir bağlantının bant genişliğini tüketir. Görseli gerçekte gösterildiği boyutta dışa aktarmak ve WebP ya da AVIF olarak sunmak genellikle yeterlidir.
ASSET-IMG-NO-ALT = alt özniteliği olmayan görsel
    .description = Bazı görsellerin `alt` özniteliği yok. Ekran okuyucunun okuyacak bir şeyi olmaz ve sonunda dosya adını harf harf okur; Google da görseli tanımlayan tek metni kaybeder: görsel aramasında sıralamayı belirleyen ve görsel yüklenemediğinde görünen metin odur. Boş bir `alt=""` geçerlidir, ancak yalnızca dekoratif görseller için.
ASSET-JS-HEAVY = Aşırı büyük komut dosyası
    .description = Komut dosyası, sunulduğu haliyle 250 KB'ın üzerinde. Onu yükleyen her sayfa indirmenin bedelini öder; daha da önemlisi, ana iş parçacığında ayrıştırma ve çalıştırmanın bedelini öder ve sıradan bir telefonda zayıf yanıt verme hızı buradan gelir. Genellikle kimsenin bölmediği bir pakettir (bundle): ilk ekran için neyin gerektiğini belirleyin ve geri kalanını ayrı yükleyin.
CANON-CHAIN = Kanonik zinciri
    .description = A, kanonik URL olarak B'yi; B ise C'yi belirtiyor. Kanonik URL'ler Google için geçişli değildir: bir zincir bulduğunda onu yok sayar ve hangi URL'nin asıl olduğuna kendisi karar verir. Her adım doğrudan C'ye işaret etmelidir.
CANON-CROSS-DOMAIN = Alan adları arası kanonik
    .description = Kanonik URL, sayfanın kendi sunucusundan farklı bir sunucuya işaret ediyor. Bu, dağıtılan (syndication) içerik için meşrudur; ancak bilinçli değilse sıralamayı diğer alan adına devreder: sayfanın kendisi dizine eklenmez olur. Genellikle yarım kalmış bir taşımadan ya da kopyalanmış bir hazırlık (staging) ortamından kaynaklanır.
CANON-MISSING = Kanonik eksik
    .description = Dizine eklenebilir sayfa rel=canonical belirtmiyor. Tek başına ciddi değildir, çünkü Google kanonik URL'yi kendisi çıkarır; ancak o olmadan herhangi bir URL parametresi —bir kampanya, bir filtre, bir sıralama— ayrı bir sayfa olarak dizine eklenebilir.
CANON-MULTIPLE = Birden fazla kanonik
    .description = Sayfa birden fazla link rel=canonical belirtiyor. Google birini seçmez: hepsini yok sayar, yani etkisi hiç olmamasıyla aynıdır. Neredeyse her zaman tema ve bir SEO eklentisinin etiketi ayrı ayrı eklemesinden kaynaklanır.
CANON-RELATIVE = Göreli kanonik
    .description = Kanonik URL, mutlak bir URL yerine göreli bir başvuru olarak belirtilmiş. Sayfa tek bir yerden sunulduğu sürece çalışır; ancak HTML başka bir sunucu altında çoğaltılırsa —bir hazırlık ortamı, bir proxy, bir kazıyıcı— kanonik URL o sunucuya göre çözümlenir ve orijinali göstermez olur.
CANON-TO-4XX = Hata URL'sine işaret eden kanonik
    .description = Kanonik URL, hata yanıtı veren bir URL'ye işaret ediyor. Sayfa kendini var olmayan bir şeyin kopyası olarak tanımlar; bu yüzden ne kendisi ne de hedef dizine eklenebilir: içerik arama sonuçlarından tamamen kaybolur.
CANON-TO-NOINDEX = noindex sayfaya işaret eden kanonik
    .description = Kanonik URL, noindex olarak işaretlenmiş bir sayfaya işaret ediyor. İki sinyal birbiriyle çelişir: biri «şunu dizine ekle» der, o da «beni dizine ekleme» der. Olağan sonuç, iki URL'nin de kaybedilmesidir.
CANON-TO-REDIRECT = Yönlendirmeye işaret eden kanonik
    .description = Kanonik URL, başka bir yere yönlendiren bir URL'ye işaret ediyor. Google kanonik sinyali ile yönlendirme sinyali arasında seçim yapmak zorunda kalır ve genellikle son hedefi tutar; bu da etiketi anlamsız kılar. Onu doğrudan 200 yanıtı veren URL'ye yönlendirin.
CONTENT-H1-EMPTY = Boş H1
    .description = İşaretlemede bir H1 var, ancak hiçbir metin katkısı yok: boş ya da tek içeriği hiçbir şey söylemeyen bir görsel. Başlık, ana başlığın yerini alır ama işini yapmaz; böylece ne arama motoru ne de ekran okuyucu sayfanın konusunu öğrenir. Olağan örnek, H1 içindeki bir logodur.
CONTENT-H1-MISSING = H1 eksik
    .description = Dizine eklenebilir sayfanın H1'i yok ya da boş. H1, sayfanın neyle ilgili olduğunu arama motoruna yazarın kendi sözleriyle anlatır ve ekran okuyucunun içeriğe girerken duyurduğu ilk başlıktır.
CONTENT-H1-MULTIPLE = Birden fazla H1
    .description = Sayfada birden fazla H1 var. HTML5 buna izin verir, ancak artık tek bir ana konu yoktur: arama motoru sayfanın neyle ilgili olduğunu tahmin etmek zorunda kalır, ekran okuyucu da birden fazla birinci düzey başlığı ayrı belgelermiş gibi duyurur. Genellikle şablonun, gerçek ana başlığa ek olarak logoyu ya da site adını H1 olarak işaretlemesinden kaynaklanır.
CONTENT-HEADING-SKIP = Atlanan başlık düzeyi
    .description = Başlık düzeyleri bir seferde birden fazla basamak düşüyor, örneğin H2'den H4'e. Belgenin ana hattında boşluklar oluşur: başlıklar arasında gezinen biri H4'ün o H2'ye mi yoksa başka bir bölüme mi ait olduğunu anlayamaz; arama motoru da neyin neye ait olduğunu anlamak için kullandığı hiyerarşiyi kaybeder. Genellikle başlığı düzeyine göre değil, yazı tipi boyutuna göre seçmekten kaynaklanır.
CONTENT-LANG-MISSING = lang özniteliği eksik
    .description = <html> öğesi içeriğin dilini belirtmiyor. Bu olmadan ekran okuyucu metni sistem dilinin kurallarıyla seslendirir, tarayıcı çeviri yaparken hangi sözlüğü kullanacağını bilmez ve sitenin dil sinyalleri otomatik algılamaya kalır. Tek satırlık bir özniteliktir.
CONTENT-THIN = Zayıf içerik
    .description = Görünür metni 300 kelimeden az olan, dizine eklenebilir bir sayfa. Bir sorguyu yanıtlayacak kadar öz nadiren taşır; bu yüzden zayıf rekabet eder ve toplu halde, arama motorunun gözünde sitenin ortalama kalitesini düşürür. Bunların çoğu arşivler, etiket sayfaları ya da otomatik oluşturulmuş taslaklardır; genişletmek yerine dizin dışında bırakmak daha iyidir.
DUP-CONTENT-EXACT = Aynı içerik
    .description = İki veya daha fazla dizine eklenebilir URL, bayt bayt tamamen aynı HTML'i döndürüyor. Aynı sorgular için birbirleriyle rekabet eder ve gelen bağlantıları toplamak yerine bölüşürler. Birini dizine eklenebilir bırakıp diğerlerini ona kanonikleştirerek düzeltin.
DUP-CONTENT-NEAR = Neredeyse yinelenen içerik
    .description = İki veya daha fazla dizine eklenebilir URL, HTML'leri aynı olmasa da neredeyse aynı görünür metne sahip: bir tarih, bir widget ya da birkaç cümle değişiyor. Genellikle aynı yazıları listeleyen etiket ve kategori sayfalarıdır ya da bir yazının üzerine yeniden yazılmış başka bir yazıdır. Google göstermek için birini seçer, diğerlerini kenara koyar. Onları birleştirin, birbirinden ayırın ya da asıl olana kanonikleştirin.
HREFLANG-INVALID-CODE = Geçersiz hreflang kodu
    .description = Özniteliğin değeri, isteğe bağlı bir ISO 3166-1 bölgesiyle birlikte bir ISO 639-1 dili de, özel x-default değeri de değil. Google hatalı bir ek açıklamayı sessizce yok sayar; böylece küme hiçbir uyarı olmadan eksik kalır: en sık görülen ve gözle fark edilmesi en zor hatadır.
HREFLANG-LANG-MISMATCH = Sayfanın dilinden farklı kendi hreflang'i
    .description = Sayfanın kendisine işaret eden hreflang ek açıklaması, `<html lang>` değerinden farklı bir dil belirtiyor: örneğin İspanyolca bir sayfa kendini İngilizce olarak tanıtıyor. İkisinden biri yanlıştır; yanlış olan hreflang ise Google sayfayı diğer dilde arama yapanlara sunar. Genellikle kodu değiştirmeden orijinal sayfanın kümesini kopyalayan bir çeviri eklentisinden kaynaklanır.
HREFLANG-NO-SELF = Kendine başvurmayan hreflang
    .description = Sayfa dil alternatifleri belirtiyor, ancak hiçbiri sayfanın kendisine işaret etmiyor. Google kümedeki her sayfanın kendini de içermesini şart koşar; bu bağlantı eksik olduğunda grubun tamamını yok sayar ve çevirilerin hiçbiri diğerlerinden yararlanamaz.
HREFLANG-NOT-RECIPROCAL = Karşılıklı olmayan hreflang
    .description = Bu sayfa, kendisini karşılığında belirtmeyen bir dil alternatifi belirtiyor. Hreflang ek açıklamaları yalnızca karşılıklı olduğunda geçerli olan oylardır: iki taraftan biri diğerini onaylamadığında Google ilişkinin tamamını yok sayar.
HREFLANG-SELF-ONLY = Alternatifi olmayan hreflang
    .description = Sayfa hreflang belirtiyor, ancak tüm ek açıklamalar sayfanın kendisine işaret ediyor: kümede hiçbir çeviri yok, yani hiçbir işe yaramıyor. Sayfanın çevirisi yoksa ek açıklama gereksizdir; varsa bağlantıları eksiktir ve asıl kontrol edilmesi gereken de budur: genellikle eklentinin bağlantı vermeyi bıraktığı bir çeviridir.
HREFLANG-TO-4XX = Hata URL'sine işaret eden hreflang
    .description = Bir dil alternatifi 4xx döndüren bir URL'ye işaret ediyor. Sayfanın vaat ettiği çeviri yok: dil değiştiren ziyaretçi bir hata sayfasına düşer ve Google, başvuran sayfayı kümeden çıkarır.
HTTP-404-EXTERNAL = Bozuk harici bağlantı
    .description = Site, artık var olmayan başka bir alan adındaki bir URL'ye bağlantı veriyor: 404 veya 410 yanıtı veriyor ya da alan adı çözümlenmiyor. Kendi sitenizdeki bir 404 kadar zarar vermez, ancak ziyaretçiyi bir hata sayfasına gönderir ve içeriği eskitir: ölü bağlantılarla dolu bir rehber, bakımı yapılıyormuş gibi görünmez.
HTTP-404-INTERNAL = Bozuk dahili bağlantı
    .description = Bu sitedeki bir URL 4xx döndürüyor ve ona bağlantı veren sayfalar var. Tarama bütçesini boşa harcar, eskiden orada olan şeye giden dahili bağlantı akışını keser ve ziyaretçiyi bir hata sayfasında bırakır.
HTTP-5XX = Sunucu hatası
    .description = URL 5xx döndürüyor: sunucu yanıtı oluşturamadı. Google uzun süre bu şekilde yanıt veren sayfaları dizininden çıkarır ve sorun sürdüğü sürece sitenin tamamının taranmasını yavaşlatır.
HTTP-EXTERNAL-UNVERIFIABLE = Doğrulanamayan harici bağlantı
    .description = Site, başka bir alan adında kontrolü reddeden bir URL'ye bağlantı veriyor: 401, 403 veya 429 yanıtı verdi. Bu genellikle bir bot engeli, bir ödeme duvarı ya da bir hız sınırıdır ve sayfa bir tarayıcıda neredeyse her zaman sorunsuz açılır. Buradan anlamanın bir yolu olmadığı için bozuk sayılmaz: hâlâ yerinde olduğunu doğrulamak için açın.
HTTP-LARGE-PAGE = Aşırı büyük HTML
    .description = HTML belgesi, görseller ve komut dosyaları hariç 500 KB'ın üzerinde. Mobil bağlantılarda ilk boyamayı geciktirir ve genellikle şablonun sayfanın hiç göstermediği içeriği döktüğü ya da satır içi CSS ve JS'nin kendi dosyalarında olması gerektiği anlamına gelir.
HTTP-MIXED-CONTENT = Karma içerik
    .description = HTTPS üzerinden sunulan bir sayfa görselleri, stil dosyalarını veya komut dosyalarını HTTP üzerinden yüklüyor. Tarayıcı komut dosyalarını ve stil dosyalarını sessizce engeller ve bağlantıyı güvenli değil olarak işaretler: sayfa bozuk görünür ve güven vermez olur.
HTTP-NO-HTTPS = Site HTTP üzerinden yanıt veriyor
    .description = Bazı dahili URL'ler, HTTPS'ye yönlendirmeden içeriği HTTP üzerinden sunuyor. Site, aynı sayfa için iki farklı adresten erişilebilir kalır —yinelenen içerik— ve tarayıcı ziyaretçiyi bağlantının güvenli olmadığı konusunda uyarır.
HTTP-REDIRECT-CHAIN = Yönlendirme zinciri
    .description = Son hedefe iki veya daha fazla ardışık atlamadan sonra ulaşılıyor. Her atlama ziyaretçi için gecikme ve tarama maliyeti ekler ve genellikle iki yeniden yazma kuralının birbirine çakıştığı anlamına gelir. Çözüm, ilk atlamayı doğrudan son hedefe yönlendirmektir.
HTTP-REDIRECT-LOOP = Yönlendirme döngüsü
    .description = Yönlendirmeler daha önce ziyaret edilmiş bir URL'ye geri dönüyor; bu yüzden hedefe hiç ulaşılmıyor. Tarayıcı bir hatayla vazgeçer ve tarayıcı bot hiçbir içerik görmez: pratikte sitenin o kısmı yok demektir.
HTTP-REDIRECT-TO-404 = Hata sayfasına yönlendirme
    .description = Yönlendirme 4xx döndüren bir URL'de sona eriyor. İki dünyanın en kötüsüdür: içeriğin taşındığını ima eden yönlendirme hâlâ yerindedir, ancak hedef de yoktur; bu yüzden bağlantı da otoritesi de yine kaybolur.
HTTP-SLOW-RESPONSE = Yavaş yanıt
    .description = Sunucunun ilk baytı göndermesi bir saniyeden uzun sürüyor. Bu süre, Core Web Vitals'ın ölçtüğü yükleme süresine olduğu gibi eklenir ve tarayıcı botun her ziyarette kaç sayfa tarayabileceğini sınırlar.
INDEX-BLOCKED-IN-SITEMAP = Engellenmiş ama site haritasında
    .description = Site haritası URL'yi dizine eklenmesini istediğiniz içerik olarak sunarken robots.txt onun taranmasını yasaklıyor. İki talimat birbiriyle çelişir, Search Console bunu bir kapsam hatası olarak işaretler ve URL dizin dışında kalır. Neredeyse her zaman, başka bir şey için yazılmış ve yayındaki bir bölümü de yakalamış bir Disallow kuralıdır.
INDEX-DEEP-PAGE = Ana sayfadan çok fazla tıklama
    .description = Ana sayfadan bu sayfaya ulaşmak dörtten fazla tıklama gerektiriyor. Sayfa ne kadar uzaktaysa o kadar az dahili otorite alır ve arama motoru onu o kadar seyrek ziyaret eder; kataloglarda ve arşivlerde, 40. sayfaya yalnızca önceki 39 sayfadan geçerek ulaşılabildiği, kısayolu olmayan sayfalamanın belirtisidir.
INDEX-NO-INTERNAL-LINKS-IN = Gelen dahili bağlantı yok
    .description = Dizine eklenebilir ve yayında olmasına rağmen sitedeki başka hiçbir sayfa bu sayfaya bağlantı vermiyor. Gelen bağlantı olmadan dahili otorite almaz, arama motoru onu çok daha geç yeniden ziyaret eder ve değişiklikleri fark edilmez. Yeni bir sayfa bir türlü sıralama alamıyorsa ilk kontrol edilecek şey budur.
INDEX-NOFOLLOW-INTERNAL = nofollow dahili bağlantı
    .description = Bu sayfa, aynı sitedeki başka bir sayfaya rel=nofollow ile bağlantı veriyor. Bağlantı otorite aktarmaz ve hedefin keşfedilmesine yardımcı olmaz; bu yüzden tek bir alan adı içinde nadiren anlamlıdır: PageRank yontma (sculpting) 2009'da işe yaramaz oldu. Genellikle dahiliyi hariciden ayırmadan her bağlantıya bunu ekleyen bir eklenti ya da şablondan kaynaklanır.
INDEX-NOINDEX = noindex
    .description = Sayfa, meta robots etiketiyle ya da X-Robots-Tag başlığıyla dizine eklenmemeyi istiyor. Google onu tarar ve dizinden çıkarır: hiçbir sorguda görünmez. Arşivlerde, etiketlerde ve yardımcı sayfalarda genellikle SEO eklentisinin bilinçli bir tercihidir; uyarının orta düzeyde olmasının nedeni budur. Ana sayfada kritik düzeye yükselir; site haritasıyla çelişki ise ayrı bir kuraldır.
INDEX-NOINDEX-IN-SITEMAP = noindex ama site haritasında
    .description = URL, sitenin dizine eklenmesini istediği şeylerin listesi olan site haritasında yer alıyor; kendi başlığı ya da meta robots etiketi ise tersini söylüyor. İkisinden biri yanlıştır: ya site haritasında yeri yoktur ya da noindex kimsenin kaldırmadığı bir kalıntıdır. İkisi bir arada durdukça site, arama motorunun önünde kendisiyle çelişir.
INDEX-ORPHAN-PAGE = Yetim sayfa
    .description = Site bu URL'yi site haritasında belirtiyor, ancak sayfalarının hiçbiri ona bağlantı vermiyor. Ziyaretçi gezinerek ona ulaşamaz, arama motoru da bağlamı ve dahili otoritesi olmayan bir içerik görür. Bildirilen ile ulaşılanı karşılaştırmaktan çıkan bir bulgudur ve tek bir sayfa bunu ortaya çıkaramaz.
INDEX-ROBOTS-BLOCKED = robots.txt tarafından engellendi
    .description = Site, kendi sayfalarından bu URL'ye bağlantı veriyor ve aynı zamanda robots.txt'de onu yasaklıyor. Google onu okuyamaz; bu yüzden ne içerdiğini bilmez ve bağlantılarını izlemez: dahili bağlantı bir yere çıkmaz ve aktardığı otorite kaybolur. Bu, sayfanın okunmasına yine de izin veren noindex ile aynı şey değildir.
INDEX-ROBOTS-TXT-BLOCKS-ALL = robots.txt tüm siteyi engelliyor
    .description = robots.txt sitenin kökünün taranmasını yasaklıyor; bu yüzden hiçbir sayfa okunamaz ve dizine eklenemez. Google'dan kaybolmanın en hızlı ve en sessiz yoludur ve neredeyse her zaman aynı kazadır: Disallow: / içeren hazırlık ortamı dosyası bir dağıtımla canlıya çıkmıştır.
INDEX-ROBOTS-TXT-MISSING = robots.txt eksik
    .description = Site /robots.txt sunmuyor. Dizine eklemeyi engellemez —dosya yoksa her şey taranabilir—, ancak site haritasının duyurulduğu ve dahili arama sonuçları ya da sepet sayfaları gibi arama motoruna bir şey katmayan alanların hariç tutulduğu yerden vazgeçilmiş olur.
INDEX-SECTION-DISCONNECTED = Dahili bağlantı yapısından kopuk bölüm
    .description = Bir grup sayfa birbirine bağlantı veriyor, ancak normal bağlantılar izlenerek ana sayfadan bunlara ulaşılamıyor: onları sitenin geri kalanına bağlayan köprü JavaScript, bir form ya da hiç yok. Arama motorları bu sayfaları yalnızca site haritası üzerinden keşfeder ve onlara çok az dahili otorite akar. Nedeni tektir —bölüme taranabilir bir bağlantı eksiktir— ve sayfa sayfa değil, bir kez düzeltilir.
INDEX-SITEMAP-ERROR = Hatalı site haritası
    .description = Bir site haritası yanıt vermiyor, XML'i hatalı ya da protokol sınırlarını (50.000 URL veya 50 MB) aşıyor. Arama motorları hatada okumayı bırakır; bu yüzden ondan sonraki her şey bu yolla hiç keşfedilmez ve hiçbir şey sizi uyarmaz: site haritası hâlâ oradadır, görünüşte sorunsuz.
INDEX-SITEMAP-MISSING = Site haritası yok
    .description = Site haritası bulunamadı: robots.txt bir site haritası duyurmuyor, olağan yollarda değil ve hiçbiri bir URL belirtmiyor. Site haritası olmadan arama motoru yalnızca bağlantı verilen sayfalara, dahili bağlantı yapısının izin verdiği hızda ulaşır; sitenin yayınladığını iddia ettiği ile bir taramanın gerçekte ulaştığı arasındaki karşılaştırmayı da kaybedersiniz.
LINK-ANCHOR-EMPTY = Metni olmayan dahili bağlantı
    .description = Bazı dahili bağlantıların metni, görseli ve aria-label'ı yok: CSS ile çizilmiş bir simge, boş bir bağlantıya dönüştürülmüş koca bir blok. Arama motoru nereye gittiğine dair hiçbir ipucu almaz, ekran okuyucu kullanan biri de yalnızca «bağlantı» duyar. Görsel olarak gizli olsa bile bir metin ya da nereye gittiğini söyleyen bir aria-label ekleyin.
LINK-ANCHOR-GENERIC = Genel bağlantı metni
    .description = Bazı dahili bağlantılar tamamen «devamını oku», «burada» ya da «buraya tıklayın» gibi kalıp bir ifadeden oluşuyor. Bu metin hedef sayfa hakkında hiçbir şey söylemez —arama motorunun onda okuduğu da tam olarak budur— ve bağlamdan koparıldığında —ekran okuyucunun onları listelediği gibi— her bağlantı aynı şeyi söyler. Metnin hedefi adlandırması yeterlidir: «devamını oku» yerine «fiyatlandırma hakkında devamını oku».
META-DESC-DUPLICATE = Yinelenen meta açıklama
    .description = İki veya daha fazla dizine eklenebilir sayfa aynı açıklamayı tekrarlıyor; bu yüzden arama sonucu onları birbirinden ayırmaz ve hiçbiri belirli bir şey vaat etmez. Genellikle kimsenin değiştirmediği site geneli bir varsayılandan kaynaklanır.
META-DESC-MISSING = Meta açıklama eksik
    .description = Dizine eklenebilir sayfa bir açıklama belirtmiyor; bu yüzden Google arama sonucu için gövdeden rastgele bir snippet keser. Bir sıralama faktörü değildir, ancak tıklamaya karar verdiren metindir; onu yazmamak, onu kontrol etmemek demektir.
META-DESC-TOO-LONG = Meta açıklama çok uzun
    .description = Açıklama arama sonucuna sığmıyor ve kesiliyor; bu yüzden sondaki harekete geçirici ifade hiç okunmaz. Karakterle değil pikselle ölçülür, çünkü kesme noktasını oluşturulan genişlik belirler.
META-DESC-TOO-SHORT = Meta açıklama çok kısa
    .description = Açıklama o kadar kısa ki Google onu yok sayıp snippet'i gövde metninden oluşturma eğiliminde. Bu kadar az alan kullanıldığında, ne tıklamayı haklı çıkaracak argümana ne de sorgu varyantına yer kalır.
META-OUTSIDE-HEAD = head dışında meta veriler
    .description = Kanonik, meta robots, hreflang, başlık ya da açıklama <head> dışında kalıyor. head'e ait olmayan bir öğe —bir izleme <img>'si, bir <div>, bir eklentinin bıraktığı başıboş metin— head'i o noktada kapatır ve Google, kaynakta durmaya devam etse de sonrasındaki her şeyi yok sayar. Etkisi, bunların hiç olmamasıyla aynıdır.
META-REFRESH = Meta refresh yönlendirmesi
    .description = Sayfa 301 yerine <meta http-equiv=refresh> ile yönlendiriyor. Google bunu çekinceyle okur, otoriteyi o kadar güvenilir biçimde aktarmaz ve kullanıcı ani bir sıçrama görür; sıfır olmayan bir gecikmeyle tarayıcının geri düğmesini de tuzağa düşürür.
META-TITLE-DUPLICATE = Yinelenen başlık
    .description = İki veya daha fazla dizine eklenebilir sayfa aynı <title>'ı paylaşıyor. Google aynı sorgu için hangisini göstereceğini seçmek zorunda kalır; bu yüzden güçlerini birleştirmek yerine birbirleriyle rekabet ederler. Sayfalamanın ya da yanlış yapılandırılmış arşivlerin en yaygın belirtisidir.
META-TITLE-MISSING = Başlık eksik
    .description = Sayfa dizine eklenebilir ve <title>'ı yok ya da boş. Doğrudan kontrol ettiğiniz en ağırlıklı sayfa içi faktördür ve arama sonucundaki mavi metindir: o olmadan Google bulduğu herhangi bir şeyden bir başlık uydurur.
META-TITLE-MULTIPLE = Birden fazla title etiketi
    .description = Sayfada birden fazla <title> var. Tarayıcılar ve Google ilkini tutar, gerisini sessizce yok sayar; bu yüzden özenle yazılmış başlık yayınlanan başlık olmayabilir. Neredeyse her zaman başlığı iki kez yazdıran bir şablon ya da temanınkini kaldırmadan kendi başlığını ekleyen bir SEO eklentisidir.
META-TITLE-TOO-LONG = Başlık çok uzun
    .description = Başlık arama sonucuna sığmıyor ve Google onu üç noktayla kesiyor; bu yüzden sonu —çoğunlukla marka ya da sona bırakılan anahtar kelime— hiç okunmaz. Karakterle değil pikselle ölçülür: kesme noktasını oluşturulan genişlik belirler.
META-TITLE-TOO-SHORT = Başlık çok kısa
    .description = Başlık, arama sonucunun sunduğu alanın çoğunu kullanmıyor. Bir hata değil, boşa harcanan alandır: bir sorgu varyantına ya da tıklamak için bir nedene yer vardır ve çoğu zaman yalnızca bölüm adını yazdıran bir şablona işaret eder.
META-VIEWPORT-MISSING = Meta viewport eksik
    .description = <meta name=viewport> olmadan telefon sayfayı 980 px genişliğinde düzenler ve küçültür; metin okunamaz hale gelir ve iki parmakla yakınlaştırma zorunlu olur. Dizine ekleme mobil önceliklidir: Google'ın değerlendirdiği, bu küçültülmüş sürümdür.
SOCIAL-OG-IMAGE-BROKEN = Bozuk Open Graph görseli
    .description = og:image'de belirtilen görsel, doğrudan ya da yönlendirmelerinin sonunda bir hata döndürüyor. Etiket yerindedir ve yalnızca HTML'i okuyan hiçbir araç bunu fark etmez; ancak sayfa WhatsApp, LinkedIn veya Slack'te paylaşıldığında kartta görsel görünmez ya da boş bir çerçeve görünür. Genellikle medya kitaplığından silinmiş bir görsel ya da bir taşımadan sağ çıkamamış bir yoldur.
SOCIAL-OG-MISSING = Eksik Open Graph
    .description = og:title, og:description ya da og:image eksik. Biri sayfayı WhatsApp, LinkedIn veya Slack'te paylaştığında ağ önizleme kartını oluşturamaz ve bağlantı çıplak bir URL olarak görünür: sıralamayı etkilemez, ancak tıklama kaybedilir.
