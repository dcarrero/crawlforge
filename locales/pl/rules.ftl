### CrawlForge — rule catalogue: names and descriptions, Polish.
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules.

### Each rule is one message: its ID, the short name as the value, and the description
### as the `.description` attribute. Backticks and <tags> are literal text, keep them.

ASSET-BROKEN = Niedziałający plik CSS lub JS
    .description = Plik CSS lub JS w witrynie zwraca 4xx lub 5xx. Google renderuje stronę, aby ją zindeksować, więc brak arkusza stylów może sprawić, że zobaczy stronę bez stylów — a więc niedostosowaną do urządzeń mobilnych — a brak skryptu może zostawić pustą treść renderowaną po stronie klienta. To typowy skutek wdrożenia z niezgodnymi hashami plików.
ASSET-CSS-HEAVY = Zbyt duży arkusz stylów
    .description = Arkusz stylów w postaci przesyłanej do przeglądarki przekracza 100 KB. Przeglądarka nie wyświetla niczego, dopóki go nie otrzyma, więc te bajty kosztują więcej niż bajty skryptu: opóźniają całe pierwsze renderowanie. Dlatego próg jest niższy. Niemal zawsze jest to cały framework dołączony po to, by wykorzystać jedną dziesiątą jego możliwości.
ASSET-FORM-BROKEN = Formularz wysyłany na niedziałający adres URL
    .description = Atrybut `action` formularza wskazuje na adres URL w tej witrynie, który zwraca 4xx lub 5xx. Formularz wygląda dobrze, daje się wypełnić i przepada przy wysyłaniu: to jedna z niewielu usterek, które wprost kosztują klientów, a nikt się o niej nie dowiaduje, bo kto na nią trafi, odchodzi bez słowa. Sprawdzane są tylko formularze wysyłane metodą GET — pole wyszukiwania, filtr katalogu — bo sprawdzenie formularza POST oznaczałoby jego wysłanie, a to narzędzie nie wysyła formularzy.
ASSET-IFRAME-BROKEN = Niedziałająca ramka iframe
    .description = Element `<iframe>` na stronie wskazuje na adres URL w tej witrynie, który zwraca 4xx lub 5xx, więc odwiedzający widzi pustą lukę tam, gdzie powinna być mapa, film lub formularz. Nie zostawia śladu na stronie, która go zawiera — HTML pozostaje poprawny — i dlatego miesiącami pozostaje niezauważony.
ASSET-IMG-BROKEN = Niedziałający obraz
    .description = Obraz, do którego odwołuje się witryna, zwraca 4xx lub 5xx. Luka jest widoczna na stronie, obraz nie istnieje dla wyszukiwarki grafiki, a każda wizyta marnuje żądanie na błąd. Niemal zawsze jest to migracja, która pominęła folder z przesłanymi plikami, albo ścieżka wpisana ręcznie.
ASSET-IMG-EMPTY-ALT-LINK = Link zawierający tylko obraz z pustym alt
    .description = Link, którego jedyną treścią jest obraz z `alt=""`, nie ma nazwy dostępnej: osoba korzystająca z czytnika ekranu słyszy „link” i nic więcej, a wyszukiwarka nie dostaje żadnego sygnału, dokąd prowadzi. Typowy przypadek to logo w nagłówku, zwykle najczęściej powtarzany link w witrynie. Tutaj `alt` nie jest dekoracyjny: to tekst linku.
ASSET-IMG-HEAVY = Zbyt duży obraz
    .description = Obraz przekracza 200 KB. To najczęstsza przyczyna słabego LCP na urządzeniach mobilnych: opóźnia wyrenderowanie głównego elementu i zużywa przepustowość wolnego łącza. Zwykle wystarczy wyeksportować go w rozmiarze, w jakim jest faktycznie wyświetlany, i serwować w formacie WebP lub AVIF.
ASSET-IMG-NO-ALT = Obraz bez atrybutu alt
    .description = Niektóre obrazy nie mają atrybutu `alt`. Czytnik ekranu nie ma czego odczytać i w końcu literuje nazwę pliku, a Google traci jedyny tekst opisujący obraz: to on decyduje o pozycji w wyszukiwarce grafiki i to on pojawia się, gdy obraz się nie wczyta. Pusty `alt=""` jest poprawny, ale tylko dla obrazów dekoracyjnych.
ASSET-JS-HEAVY = Zbyt duży skrypt
    .description = Skrypt w postaci przesyłanej do przeglądarki przekracza 250 KB. Każda strona, która go ładuje, płaci za pobranie, a przede wszystkim za jego parsowanie i wykonanie w wątku głównym — stąd bierze się słaba responsywność na zwykłym telefonie. Zwykle to pakiet (bundle), którego nikt nie podzielił: ustal, co jest potrzebne do pierwszego ekranu, a resztę ładuj osobno.
CANON-CHAIN = Łańcuch adresów kanonicznych
    .description = A wskazuje B jako swój adres kanoniczny, a B wskazuje C. Dla Google adresy kanoniczne nie są przechodnie: gdy trafia na łańcuch, ignoruje go i sam decyduje, który adres URL jest główny. Każdy krok powinien wskazywać bezpośrednio na C.
CANON-CROSS-DOMAIN = Adres kanoniczny w innej domenie
    .description = Adres kanoniczny wskazuje na inny host niż host samej strony. Jest to uzasadnione przy treściach syndykowanych, ale gdy nie jest zamierzone, oddaje pozycje innej domenie: sama strona przestaje być indeksowana. Zwykle wynika z niedokończonej migracji lub skopiowanego środowiska testowego (staging).
CANON-MISSING = Brak adresu kanonicznego
    .description = Indeksowalna strona nie deklaruje rel=canonical. Samo w sobie nie jest to poważne, bo Google sam ustala adres kanoniczny, ale bez niego dowolny parametr URL — kampania, filtr, sortowanie — może zostać zindeksowany jako osobna strona.
CANON-MULTIPLE = Wiele adresów kanonicznych
    .description = Strona deklaruje więcej niż jeden link rel=canonical. Google nie wybiera żadnego: ignoruje wszystkie, więc skutek jest taki, jakby nie było żadnego. Niemal zawsze to motyw i wtyczka SEO, które niezależnie od siebie wstawiają ten tag.
CANON-RELATIVE = Względny adres kanoniczny
    .description = Adres kanoniczny jest zadeklarowany jako odwołanie względne zamiast bezwzględnego adresu URL. Działa, dopóki strona jest serwowana z jednego miejsca, ale jeśli HTML zostanie odtworzony pod innym hostem — w środowisku testowym, przez proxy, przez scraper — adres kanoniczny zostanie rozwiązany względem tego hosta i przestanie wskazywać na oryginał.
CANON-TO-4XX = Adres kanoniczny wskazuje na błąd
    .description = Adres kanoniczny wskazuje na adres URL, który odpowiada błędem. Strona deklaruje się jako duplikat czegoś, co nie istnieje, więc ani ona, ani cel nie mogą zostać zindeksowane: treść całkowicie znika z wyników.
CANON-TO-NOINDEX = Adres kanoniczny wskazuje na stronę z noindex
    .description = Adres kanoniczny wskazuje na stronę oznaczoną noindex. Te dwa sygnały sobie przeczą: jeden mówi „indeksuj tamtą”, a tamta mówi „nie indeksuj mnie”. Zwykle kończy się to utratą obu adresów URL.
CANON-TO-REDIRECT = Adres kanoniczny wskazuje na przekierowanie
    .description = Adres kanoniczny wskazuje na adres URL, który przekierowuje gdzie indziej. Google musi wybrać między sygnałem kanonicznym a sygnałem przekierowania i zwykle wybiera ostateczny cel, przez co tag traci sens. Wskaż bezpośrednio adres URL, który odpowiada kodem 200.
CONTENT-H1-EMPTY = Pusty nagłówek H1
    .description = W kodzie jest H1, ale nie wnosi żadnego tekstu: jest pusty albo jego jedyną treścią jest obraz, który nic nie mówi. Nagłówek zajmuje miejsce tytułu, nie spełniając jego funkcji, więc ani wyszukiwarka, ani czytnik ekranu nie poznają tematu strony. Typowy przypadek to logo wewnątrz H1.
CONTENT-H1-MISSING = Brak nagłówka H1
    .description = Indeksowalna strona nie ma H1 albo ma pusty. H1 mówi wyszukiwarce, o czym jest strona, słowami samego autora, i jest pierwszym nagłówkiem, który czytnik ekranu odczytuje po przejściu do treści.
CONTENT-H1-MULTIPLE = Wiele nagłówków H1
    .description = Strona ma więcej niż jeden H1. HTML5 na to pozwala, ale nie ma już jednego głównego tematu: wyszukiwarka musi zgadywać, o czym jest strona, a czytnik ekranu odczytuje kilka nagłówków pierwszego poziomu, jakby były osobnymi dokumentami. Zwykle to szablon oznacza logo lub nazwę witryny jako H1 oprócz właściwego tytułu.
CONTENT-HEADING-SKIP = Pominięty poziom nagłówka
    .description = Poziomy nagłówków spadają o więcej niż jeden stopień naraz, na przykład z H2 do H4. W strukturze dokumentu powstają dziury: osoba nawigująca po nagłówkach nie wie, czy H4 należy do tego H2, czy do innej sekcji, a wyszukiwarka traci hierarchię, z której korzysta, by zrozumieć, co do czego należy. Zwykle wynika to z wybierania nagłówka według rozmiaru czcionki zamiast poziomu.
CONTENT-LANG-MISSING = Brak atrybutu lang
    .description = Element <html> nie deklaruje języka treści. Bez niego czytnik ekranu wymawia tekst według zasad języka systemu, przeglądarka nie wie, jakiego słownika użyć przy tłumaczeniu, a sygnały językowe witryny zależą od automatycznego wykrywania. To atrybut na jedną linię.
CONTENT-THIN = Uboga treść
    .description = Indeksowalna strona z mniej niż 300 słowami widocznego tekstu. Rzadko ma dość treści, by odpowiedzieć na zapytanie, więc słabo konkuruje, a w dużej liczbie obniża w oczach wyszukiwarki średnią jakość witryny. Wiele z nich to archiwa, strony tagów lub automatycznie generowane zalążki, które lepiej wyłączyć z indeksu niż rozbudowywać.
DUP-CONTENT-EXACT = Identyczna treść
    .description = Dwa lub więcej indeksowalnych adresów URL zwraca dokładnie ten sam HTML, bajt w bajt. Konkurują ze sobą o te same zapytania i dzielą linki przychodzące zamiast je sumować. Napraw to, zostawiając jeden indeksowalny i wskazując go jako adres kanoniczny pozostałych.
DUP-CONTENT-NEAR = Prawie zduplikowana treść
    .description = Dwa lub więcej indeksowalnych adresów URL ma niemal ten sam widoczny tekst, choć ich HTML nie jest identyczny: zmienia się data, widżet lub kilka zdań. Zwykle są to strony tagów i kategorii z listą tych samych wpisów albo jeden wpis przepisany na bazie innego. Google wybiera jeden do wyświetlania, a pozostałe odkłada na bok. Połącz je, zróżnicuj albo wskaż główny jako ich adres kanoniczny.
HREFLANG-INVALID-CODE = Nieprawidłowy kod hreflang
    .description = Wartość atrybutu nie jest ani językiem ISO 639-1 z opcjonalnym regionem ISO 3166-1, ani specjalną wartością x-default. Google po cichu odrzuca błędnie sformatowaną adnotację, więc zestaw pozostaje niekompletny bez ostrzeżenia: to najczęstszy błąd i najtrudniejszy do wychwycenia gołym okiem.
HREFLANG-LANG-MISMATCH = Własny hreflang w innym języku niż strona
    .description = Adnotacja hreflang wskazująca na samą stronę deklaruje inny język niż jej `<html lang>`: na przykład strona po hiszpańsku, która przedstawia się jako angielska. Jedno z dwojga jest błędne, a jeśli to hreflang, Google pokazuje stronę osobom szukającym w innym języku. Zwykle to wtyczka tłumaczeniowa, która kopiuje zestaw strony oryginalnej bez zmiany kodu.
HREFLANG-NO-SELF = Hreflang bez odwołania do siebie
    .description = Strona deklaruje wersje językowe, ale żadna nie wskazuje na nią samą. Google wymaga, aby każda strona zestawu uwzględniała samą siebie; gdy brakuje tego linku, odrzuca całą grupę i żadne z tłumaczeń nie korzysta z pozostałych.
HREFLANG-NOT-RECIPROCAL = Hreflang bez wzajemności
    .description = Ta strona deklaruje wersję językową, która nie odwzajemnia tej deklaracji. Adnotacje hreflang to głosy, które liczą się tylko wtedy, gdy są wzajemne: Google ignoruje całą relację, gdy jedna z dwóch stron nie potwierdza drugiej.
HREFLANG-SELF-ONLY = Hreflang bez wersji alternatywnych
    .description = Strona deklaruje hreflang, ale każda adnotacja wskazuje na nią samą: w zestawie nie ma żadnego tłumaczenia, więc nic to nie daje. Jeśli strona nie ma tłumaczenia, adnotacja jest zbędna; jeśli ma, brakuje linków do niego i to właśnie trzeba sprawdzić: zwykle jest to tłumaczenie, do którego wtyczka przestała linkować.
HREFLANG-TO-4XX = Hreflang wskazuje na błąd
    .description = Wersja językowa wskazuje na adres URL, który zwraca 4xx. Tłumaczenie obiecywane przez stronę nie istnieje: odwiedzający, który zmienia język, trafia na stronę błędu, a Google usuwa stronę odsyłającą z zestawu.
HTTP-404-EXTERNAL = Niedziałający link zewnętrzny
    .description = Witryna linkuje do adresu URL w innej domenie, który już nie istnieje: odpowiada 404 lub 410 albo jego domena się nie rozwiązuje. Nie szkodzi tak jak własny błąd 404, ale wysyła odwiedzającego na stronę błędu i postarza treść: poradnik pełen martwych linków przestaje wyglądać na aktualizowany.
HTTP-404-INTERNAL = Niedziałający link wewnętrzny
    .description = Adres URL w tej witrynie zwraca 4xx, a niektóre strony do niego linkują. Marnuje to budżet indeksowania (crawl budget), przerywa przepływ linków wewnętrznych do tego, co tam wcześniej było, i zostawia odwiedzającego na stronie błędu.
HTTP-5XX = Błąd serwera
    .description = Adres URL zwraca 5xx: serwer nie zdołał przygotować odpowiedzi. Google usuwa z indeksu strony, które długo tak odpowiadają, i spowalnia skanowanie całej witryny, dopóki to trwa.
HTTP-EXTERNAL-UNVERIFIABLE = Nieweryfikowalny link zewnętrzny
    .description = Witryna linkuje do adresu URL w innej domenie, który odmówił sprawdzenia: odpowiedział 401, 403 lub 429. Zwykle to zabezpieczenie przed botami, paywall lub limit żądań, a strona niemal zawsze otwiera się normalnie w przeglądarce. Nie jest uznawany za niedziałający, bo stąd nie da się tego stwierdzić: otwórz go, aby potwierdzić, że nadal istnieje.
HTTP-LARGE-PAGE = Zbyt duży HTML
    .description = Dokument HTML przekracza 500 KB, nie licząc obrazów i skryptów. Opóźnia pierwsze renderowanie przy połączeniach mobilnych i zwykle oznacza, że szablon wrzuca treść, której strona nigdy nie pokazuje, albo że osadzone CSS i JS powinny trafić do osobnych plików.
HTTP-MIXED-CONTENT = Treści mieszane
    .description = Strona serwowana przez HTTPS ładuje obrazy, arkusze stylów lub skrypty przez HTTP. Przeglądarka po cichu blokuje skrypty i arkusze stylów i oznacza połączenie jako niezabezpieczone: strona wygląda na uszkodzoną i przestaje budzić zaufanie.
HTTP-NO-HTTPS = Witryna odpowiada przez HTTP
    .description = Niektóre wewnętrzne adresy URL serwują treść przez HTTP bez przekierowania na HTTPS. Witryna pozostaje dostępna pod dwoma różnymi adresami dla tej samej strony — zduplikowana treść — a przeglądarka ostrzega odwiedzającego, że połączenie nie jest bezpieczne.
HTTP-REDIRECT-CHAIN = Łańcuch przekierowań
    .description = Ostateczny cel zostaje osiągnięty dopiero po dwóch lub więcej kolejnych przeskokach. Każdy przeskok dodaje opóźnienie dla odwiedzającego i koszt skanowania, a zwykle oznacza, że dwie reguły przepisywania (rewrite) wchodzą sobie w drogę. Rozwiązanie: skieruj pierwszy przeskok bezpośrednio na ostateczny cel.
HTTP-REDIRECT-LOOP = Pętla przekierowań
    .description = Przekierowania wracają do już odwiedzonego adresu URL, więc cel nigdy nie zostaje osiągnięty. Przeglądarka poddaje się z błędem, a crawler nigdy nie widzi żadnej treści: w praktyce ta część witryny nie istnieje.
HTTP-REDIRECT-TO-404 = Przekierowanie na stronę błędu
    .description = Przekierowanie kończy się na adresie URL, który zwraca 4xx. To najgorsze z obu światów: przekierowanie, które sugerowało, że treść została przeniesiona, nadal istnieje, ale cel też nie istnieje, więc link i jego autorytet i tak przepadają.
HTTP-SLOW-RESPONSE = Wolna odpowiedź
    .description = Serwer potrzebuje ponad sekundy na wysłanie pierwszego bajtu. Ten czas w całości dolicza się do czasu ładowania mierzonego przez Core Web Vitals i ogranicza liczbę stron, które crawler zdąży przejrzeć podczas każdej wizyty.
INDEX-BLOCKED-IN-SITEMAP = Zablokowany, ale w mapie witryny
    .description = Mapa witryny przedstawia adres URL jako treść przeznaczoną do zindeksowania, a robots.txt zabrania jego skanowania. Te dwie instrukcje sobie przeczą, Search Console zgłasza to jako błąd stanu indeksowania, a adres URL pozostaje poza indeksem. Niemal zawsze to reguła Disallow napisana z myślą o czymś innym, która przy okazji objęła opublikowaną sekcję.
INDEX-DEEP-PAGE = Zbyt wiele kliknięć od strony głównej
    .description = Dotarcie do tej strony wymaga ponad czterech kliknięć od strony głównej. Im dalej, tym mniej autorytetu wewnętrznego otrzymuje i tym rzadziej wyszukiwarka ją odwiedza; w katalogach i archiwach to objaw paginacji bez skrótów, gdzie do strony 40 można dotrzeć tylko przez 39 poprzednich.
INDEX-NO-INTERNAL-LINKS-IN = Brak przychodzących linków wewnętrznych
    .description = Żadna inna strona w witrynie nie linkuje do tej, choć jest indeksowalna i opublikowana. Bez linków przychodzących nie otrzymuje autorytetu wewnętrznego, wyszukiwarka znacznie rzadziej do niej wraca, a jej zmiany pozostają niezauważone. To pierwsza rzecz do sprawdzenia, gdy nowa strona nie chce się wypozycjonować.
INDEX-NOFOLLOW-INTERNAL = Wewnętrzny link nofollow
    .description = Ta strona linkuje do innej strony w tej samej witrynie z rel=nofollow. Link nie przekazuje autorytetu i nie pomaga odkryć celu, więc w obrębie jednej domeny rzadko ma sens: PageRank sculpting przestał działać w 2009 roku. Zwykle dodaje go wtyczka lub szablon do każdego linku, nie odróżniając wewnętrznych od zewnętrznych.
INDEX-NOINDEX = Noindex
    .description = Strona prosi, by jej nie indeksować — przez tag meta robots albo nagłówek X-Robots-Tag. Google ją skanuje i odrzuca: nie pojawi się dla żadnego zapytania. W archiwach, tagach i stronach pomocniczych to zwykle świadomy wybór wtyczki SEO, dlatego ostrzeżenie ma umiarkowaną wagę; na stronie głównej rośnie ona do krytycznej, a konflikt z mapą witryny ma osobną regułę.
INDEX-NOINDEX-IN-SITEMAP = Noindex, ale w mapie witryny
    .description = Adres URL jest w mapie witryny, czyli na liście tego, co witryna chce zindeksować, a jego własny nagłówek lub meta robots mówi coś przeciwnego. Jedno z dwojga jest błędne: albo nie powinien być w mapie witryny, albo noindex to pozostałość, której nikt nie usunął. Dopóki oba współistnieją, witryna przeczy sama sobie wobec wyszukiwarki.
INDEX-ORPHAN-PAGE = Strona osierocona
    .description = Witryna deklaruje ten adres URL w mapie witryny, ale żadna z jej stron do niego nie linkuje. Odwiedzający nie dotrze do niego, przeglądając witrynę, a wyszukiwarka widzi treść bez kontekstu i bez autorytetu wewnętrznego. To problem, który wynika z porównania tego, co zadeklarowane, z tym, do czego da się dotrzeć, i żadna pojedyncza strona nie może go ujawnić.
INDEX-ROBOTS-BLOCKED = Zablokowany przez robots.txt
    .description = Witryna linkuje do tego adresu URL z własnych stron, a jednocześnie blokuje go w robots.txt. Google nie może go odczytać, więc nie wie, co zawiera, ani nie podąża za jego linkami: link wewnętrzny prowadzi donikąd, a przekazywany przez niego autorytet przepada. To nie to samo co noindex, który nadal pozwala odczytać stronę.
INDEX-ROBOTS-TXT-BLOCKS-ALL = robots.txt blokuje całą witrynę
    .description = Plik robots.txt zabrania skanowania katalogu głównego witryny, więc żadna strona nie może zostać odczytana ani zindeksowana. To najszybszy i najcichszy sposób na zniknięcie z Google i niemal zawsze ten sam wypadek: plik ze środowiska testowego, zawierający Disallow: /, trafił na produkcję podczas wdrożenia.
INDEX-ROBOTS-TXT-MISSING = Brak robots.txt
    .description = Witryna nie udostępnia pliku /robots.txt. Nie blokuje to indeksowania — bez pliku wszystko można skanować — ale oznacza rezygnację z miejsca, w którym ogłasza się mapę witryny i wyklucza obszary nic niewnoszące dla wyszukiwarki, takie jak wyniki wyszukiwania wewnętrznego czy strony koszyka.
INDEX-SECTION-DISCONNECTED = Sekcja odcięta od linkowania wewnętrznego
    .description = Grupa stron linkuje do siebie nawzajem, ale nie da się do niej dotrzeć ze strony głównej zwykłymi linkami: most łączący ją z resztą witryny to JavaScript, formularz albo w ogóle go nie ma. Wyszukiwarki odkrywają te strony tylko przez mapę witryny i dociera do nich niewiele autorytetu wewnętrznego. Przyczyna jest jedna — brakuje skanowalnego linku do sekcji — i naprawia się ją raz, a nie strona po stronie.
INDEX-SITEMAP-ERROR = Mapa witryny z błędami
    .description = Mapa witryny nie odpowiada, zawiera błędnie sformatowany XML albo przekracza limity protokołu (50 000 adresów URL lub 50 MB). Wyszukiwarki przestają czytać w miejscu błędu, więc wszystko, co jest dalej, nigdy nie zostanie odkryte tą drogą, a nic o tym nie ostrzega: mapa witryny nadal tam jest, pozornie w porządku.
INDEX-SITEMAP-MISSING = Brak mapy witryny
    .description = Nie znaleziono mapy witryny: robots.txt żadnej nie ogłasza, nie ma jej pod typowymi ścieżkami i żadna nie deklaruje adresów URL. Bez niej wyszukiwarka dociera tylko do tego, co jest podlinkowane, w tempie, na jakie pozwala linkowanie wewnętrzne; tracisz też porównanie tego, co witryna deklaruje jako opublikowane, z tym, do czego skanowanie faktycznie dociera.
LINK-ANCHOR-EMPTY = Link wewnętrzny bez tekstu
    .description = Niektóre linki wewnętrzne nie mają tekstu, obrazu ani aria-label: ikona narysowana w CSS, cały blok zamieniony w pusty link. Wyszukiwarka nie dostaje żadnej wskazówki, dokąd prowadzą, a osoba korzystająca z czytnika ekranu słyszy „link” i nic więcej. Dodaj tekst, nawet ukryty wizualnie, albo aria-label mówiący, dokąd link prowadzi.
LINK-ANCHOR-GENERIC = Ogólnikowy tekst kotwicy
    .description = Niektóre linki wewnętrzne składają się wyłącznie z utartej frazy, takiej jak „czytaj więcej”, „tutaj” czy „kliknij tutaj”. Taki tekst nic nie mówi o stronie docelowej, a właśnie to wyszukiwarka z niego odczytuje; poza kontekstem — tak, jak wymienia je czytnik ekranu — każdy link mówi to samo. Wystarczy, że tekst nazwie cel: „czytaj więcej o cenach” zamiast „czytaj więcej”.
LINK-FRAGMENT-BROKEN = Link do nieistniejącej kotwicy
    .description = Niektóre linki wewnętrzne prowadzą do `#fragmentu`, którego żaden element strony docelowej nie ma jako `id` ani jako `<a name>`. Link otwiera stronę na samej górze zamiast w obiecanej sekcji: spis treści, w którym zmieniono nagłówek, link do usuniętego bloku, „przejdź do treści” do id, którego motyw już nie wypisuje. Id dodawane przez JavaScript nie występuje w serwowanym HTML i mimo to zostaje zgłoszone, więc taki przypadek sprawdź w przeglądarce. Popraw link albo przywróć sekcji jej id.
META-DESC-DUPLICATE = Zduplikowany opis meta
    .description = Dwie lub więcej indeksowalnych stron powtarza ten sam opis, więc wynik wyszukiwania ich nie rozróżnia i żadna nie obiecuje niczego konkretnego. Zwykle wynika to z domyślnej wartości dla całej witryny, której nikt nie nadpisał.
META-DESC-MISSING = Brak opisu meta
    .description = Indeksowalna strona nie deklaruje opisu, więc Google wycina do wyniku wyszukiwania dowolny fragment treści. Nie jest to czynnik rankingowy, ale to tekst, który decyduje o kliknięciu, a niepisanie go oznacza rezygnację z kontroli nad nim.
META-DESC-TOO-LONG = Zbyt długi opis meta
    .description = Opis nie mieści się w wyniku wyszukiwania i zostaje obcięty, więc wezwanie do działania na końcu nigdy nie zostaje przeczytane. Mierzy się go w pikselach, nie w znakach, bo o miejscu cięcia decyduje szerokość po wyrenderowaniu.
META-DESC-TOO-SHORT = Zbyt krótki opis meta
    .description = Opis jest tak krótki, że Google zwykle go ignoruje i buduje fragment (snippet) z tekstu strony. Przy tak małym wykorzystaniu miejsca nie mieści się ani argument, ani wariant zapytania, który uzasadniałby kliknięcie.
META-OUTSIDE-HEAD = Metadane poza sekcją head
    .description = Adres kanoniczny, meta robots, hreflang, tytuł lub opis znajdują się poza <head>. Element, który nie należy do head — śledzący <img>, <div>, zabłąkany tekst pozostawiony przez wtyczkę — zamyka go w tym miejscu, a Google ignoruje wszystko, co następuje dalej, mimo że nadal jest w kodzie źródłowym. Skutek jest taki sam, jakby ich nie było.
META-REFRESH = Przekierowanie meta refresh
    .description = Strona przekierowuje za pomocą <meta http-equiv=refresh> zamiast 301. Google traktuje je z rezerwą, nie przekazuje autorytetu równie niezawodnie, a użytkownik widzi gwałtowny przeskok; przy niezerowym opóźnieniu blokuje też przycisk Wstecz w przeglądarce.
META-TITLE-DUPLICATE = Zduplikowany tytuł
    .description = Dwie lub więcej indeksowalnych stron ma ten sam <title>. Google musi wybrać, którą pokazać dla tego samego zapytania, więc konkurują ze sobą zamiast się wzmacniać. To najczęstszy objaw źle skonfigurowanej paginacji lub archiwów.
META-TITLE-MISSING = Brak tytułu
    .description = Strona jest indeksowalna i nie ma <title> albo ma pusty. To czynnik on-page o największej wadze, nad którym masz bezpośrednią kontrolę, i to niebieski tekst w wyniku wyszukiwania: bez niego Google wymyśla tytuł z tego, co znajdzie.
META-TITLE-MULTIPLE = Wiele tagów title
    .description = Strona zawiera więcej niż jeden <title>. Przeglądarki i Google biorą pierwszy, a resztę po cichu odrzucają, więc starannie napisany tytuł może nie być tym opublikowanym. Niemal zawsze to szablon wypisujący go dwa razy albo wtyczka SEO dodająca własny bez usuwania tytułu z motywu.
META-TITLE-TOO-LONG = Zbyt długi tytuł
    .description = Tytuł nie mieści się w wyniku wyszukiwania i Google obcina go wielokropkiem, więc końcówka — często marka albo słowo kluczowe zostawione na koniec — nigdy nie zostaje przeczytana. Mierzy się go w pikselach, nie w znakach: o miejscu cięcia decyduje szerokość po wyrenderowaniu.
META-TITLE-TOO-SHORT = Zbyt krótki tytuł
    .description = Tytuł nie wykorzystuje większości miejsca, jakie daje wynik wyszukiwania. To nie błąd, lecz zmarnowane miejsce: zmieściłby się wariant zapytania albo powód do kliknięcia, a często wskazuje to na szablon, który wypisuje tylko nazwę sekcji.
META-VIEWPORT-MISSING = Brak meta viewport
    .description = Bez <meta name=viewport> telefon układa stronę na szerokość 980 px i ją pomniejsza, przez co tekst jest nieczytelny i trzeba powiększać go palcami. Indeksowanie jest mobile-first: Google ocenia właśnie tę pomniejszoną wersję.
SOCIAL-OG-IMAGE-BROKEN = Niedziałający obraz Open Graph
    .description = Obraz zadeklarowany w og:image zwraca błąd, bezpośrednio lub na końcu przekierowań. Tag jest na miejscu i żadne narzędzie, które czyta tylko HTML, tego nie zauważa, ale gdy strona zostanie udostępniona na WhatsAppie, LinkedInie czy Slacku, karta podglądu wyświetla się bez obrazu albo z pustą ramką. Zwykle to obraz usunięty z biblioteki mediów albo ścieżka, która nie przetrwała migracji.
SOCIAL-OG-MISSING = Niekompletne znaczniki Open Graph
    .description = Brakuje og:title, og:description lub og:image. Gdy ktoś udostępnia stronę na WhatsAppie, LinkedInie czy Slacku, serwis nie może zbudować karty podglądu i link wyświetla się jako goły adres URL: nie wpływa to na pozycję, ale kliknięcie przepada.
