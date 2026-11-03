### CrawlForge — rule catalogue: names and descriptions, Italian.
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules. Style and glossary:
### see the header of cli.ftl in this folder.

### Each rule is one message: its ID, the short name as the value, and the description
### as the `.description` attribute. Backticks and <tags> are literal text, keep them.

ASSET-BROKEN = CSS o JS non funzionante
    .description = Un file CSS o JS del sito restituisce 4xx o 5xx. Google esegue il rendering della pagina per indicizzarla, quindi un foglio di stile mancante può fargli vedere una pagina senza stile —e quindi non ottimizzata per i dispositivi mobili—, e uno script mancante può lasciare vuoto il contenuto generato lato client. È tipico di un deploy con hash dei file non corrispondenti.
ASSET-CSS-HEAVY = Foglio di stile troppo pesante
    .description = Il foglio di stile supera i 100 KB così come viene servito. Il browser non mostra nulla finché non lo ha ricevuto, quindi questi byte costano più di quelli di uno script: ritardano l'intero primo rendering. Per questo la soglia è più bassa. Quasi sempre è un framework completo incluso per usarne un decimo.
ASSET-FORM-BROKEN = Modulo inviato a un URL non funzionante
    .description = L'`action` di un modulo punta a un URL del tuo sito che restituisce 4xx o 5xx. Il modulo sembra a posto, si compila senza problemi e va perso all'invio: uno dei pochi difetti che costano clienti in modo diretto, e nessuno se ne accorge, perché chi ci incappa se ne va senza dirlo. Vengono verificati solo i moduli inviati con GET —una casella di ricerca, un filtro di catalogo—: verificare un POST significherebbe inviarlo, e questo strumento non invia moduli.
ASSET-IFRAME-BROKEN = Frame incorporato non funzionante
    .description = Un `<iframe>` della pagina punta a un URL del tuo sito che restituisce 4xx o 5xx, quindi il visitatore vede un vuoto bianco dove dovrebbero esserci la mappa, il video o il modulo. Non lascia traccia nella pagina che lo contiene —l'HTML resta valido—, ed è per questo che passa inosservato per mesi.
ASSET-IMG-BROKEN = Immagine non funzionante
    .description = Un'immagine a cui punta il sito restituisce 4xx o 5xx. Il vuoto si vede nella pagina, l'immagine non esiste per la ricerca immagini e ogni visita spreca una richiesta su un errore. Quasi sempre è una migrazione che ha dimenticato la cartella dei caricamenti, o un percorso scritto a mano.
ASSET-IMG-EMPTY-ALT-LINK = Link con sola immagine ad alt vuoto
    .description = Un link il cui unico contenuto è un'immagine con `alt=""` non ha un nome accessibile: chi usa uno screen reader sente «link» e nient'altro, e il motore di ricerca non riceve alcun segnale su dove porta. Il caso tipico è il logo nell'header, di solito il link più ripetuto del sito. Qui l'`alt` non è decorativo: è il testo del link.
ASSET-IMG-HEAVY = Immagine troppo pesante
    .description = L'immagine supera i 200 KB. È la causa più comune di un LCP scadente su mobile: ritarda il rendering dell'elemento principale e consuma la banda di una connessione lenta. Di solito basta esportarla alle dimensioni in cui viene effettivamente mostrata e servirla in WebP o AVIF.
ASSET-IMG-NO-ALT = Immagine senza attributo alt
    .description = Alcune immagini non hanno l'attributo `alt`. Uno screen reader non ha nulla da leggere e finisce per pronunciare il nome del file, e Google perde l'unico testo che descrive l'immagine: è ciò che la posiziona nella ricerca immagini e ciò che compare quando l'immagine non si carica. Un `alt=""` vuoto è valido, ma solo per le immagini decorative.
ASSET-JS-HEAVY = Script troppo pesante
    .description = Lo script supera i 250 KB così come viene servito. Ogni pagina che lo carica paga il download e, soprattutto, l'analisi e l'esecuzione sul thread principale, da cui deriva la scarsa reattività su un telefono qualunque. Di solito è un bundle che nessuno ha suddiviso: individua ciò che serve per la prima schermata e carica il resto separatamente.
CANON-CHAIN = Catena di canonical
    .description = A dichiara B come canonical, e B dichiara C. Per Google i canonical non sono transitivi: quando trova una catena la ignora e decide da solo quale URL è quello principale. Ogni passaggio dovrebbe puntare direttamente a C.
CANON-CROSS-DOMAIN = Canonical verso un altro dominio
    .description = Il canonical punta a un host diverso da quello della pagina. È legittimo per i contenuti in syndication, ma quando non è voluto cede il posizionamento all'altro dominio: la pagina stessa smette di essere indicizzata. Di solito deriva da una migrazione lasciata a metà o da un ambiente di staging copiato.
CANON-MISSING = Canonical mancante
    .description = La pagina indicizzabile non dichiara alcun rel=canonical. Di per sé non è grave, perché Google deduce il canonical, ma senza di esso qualsiasi parametro dell'URL — una campagna, un filtro, un ordinamento — può finire indicizzato come pagina separata.
CANON-MULTIPLE = Canonical multipli
    .description = La pagina dichiara più di un link rel=canonical. Google non ne sceglie uno: li ignora tutti, quindi l'effetto è come non averne nessuno. Quasi sempre sono il tema e un plugin SEO che emettono il tag ciascuno per conto proprio.
CANON-RELATIVE = Canonical relativo
    .description = Il canonical è dichiarato come riferimento relativo anziché come URL assoluto. Funziona finché la pagina è servita da un solo posto, ma se l'HTML viene riprodotto sotto un altro host — un ambiente di staging, un proxy, uno scraper — il canonical si risolve rispetto a quell'host e smette di puntare all'originale.
CANON-TO-4XX = Canonical verso un URL in errore
    .description = Il canonical punta a un URL che risponde con un errore. La pagina si dichiara duplicata di qualcosa che non esiste, quindi né lei né la destinazione possono essere indicizzate: il contenuto sparisce del tutto dai risultati.
CANON-TO-NOINDEX = Canonical verso una pagina noindex
    .description = Il canonical punta a una pagina contrassegnata come noindex. I due segnali si contraddicono: uno dice «indicizza quella» e quella dice «non indicizzarmi». Il risultato abituale è la perdita di entrambi gli URL.
CANON-TO-REDIRECT = Canonical verso un reindirizzamento
    .description = Il canonical punta a un URL che reindirizza altrove. Google deve scegliere tra il segnale del canonical e quello del reindirizzamento, e di solito tiene la destinazione finale, il che rende inutile il tag. Fallo puntare direttamente all'URL che risponde 200.
CONTENT-H1-EMPTY = H1 vuoto
    .description = Nel markup c'è un H1, ma non contiene testo: è vuoto, oppure il suo unico contenuto è un'immagine che non dice nulla. L'intestazione occupa il posto del titolo principale senza svolgerne la funzione, quindi né il motore di ricerca né uno screen reader ricavano l'argomento della pagina. Il caso abituale è un logo dentro l'H1.
CONTENT-H1-MISSING = H1 mancante
    .description = La pagina indicizzabile non ha un H1, oppure è vuoto. L'H1 dice al motore di ricerca di cosa parla la pagina con le parole dell'autore, ed è la prima intestazione che uno screen reader annuncia entrando nel contenuto.
CONTENT-H1-MULTIPLE = H1 multipli
    .description = La pagina ha più di un H1. HTML5 lo consente, ma non c'è più un unico argomento principale: il motore di ricerca deve indovinare di cosa parla la pagina, e uno screen reader annuncia più intestazioni di primo livello come se fossero documenti separati. Di solito è il template che marca come H1 il logo o il nome del sito oltre al vero titolo.
CONTENT-HEADING-SKIP = Livello di intestazione saltato
    .description = I livelli delle intestazioni scendono di più di un gradino alla volta, per esempio da H2 a H4. La struttura del documento resta con dei buchi: chi naviga per intestazioni non può capire se l'H4 appartiene a quell'H2 o a un'altra sezione, e il motore di ricerca perde la gerarchia che usa per capire cosa appartiene a cosa. Di solito deriva dalla scelta di un'intestazione per la dimensione del carattere anziché per il livello.
CONTENT-LANG-MISSING = Attributo lang mancante
    .description = L'elemento <html> non dichiara la lingua del contenuto. Senza di esso, uno screen reader pronuncia il testo con le regole della lingua del sistema, il browser non sa quale dizionario usare per tradurre e i segnali di lingua del sito sono lasciati al rilevamento automatico. È un attributo di una riga.
CONTENT-THIN = Contenuto scarno
    .description = Una pagina indicizzabile con meno di 300 parole di testo visibile. Raramente ha abbastanza sostanza per rispondere a una query, quindi si posiziona male e, in massa, diluisce la qualità media del sito agli occhi del motore di ricerca. Molte sono archivi, pagine di tag o bozze generate automaticamente che conviene escludere dall'indice piuttosto che ampliare.
DUP-CONTENT-EXACT = Contenuto identico
    .description = Due o più URL indicizzabili restituiscono esattamente lo stesso HTML, byte per byte. Competono tra loro per le stesse query e si dividono i link in entrata invece di sommarli. Si risolve lasciandone indicizzabile uno solo e impostando su di esso il canonical degli altri.
DUP-CONTENT-NEAR = Contenuto quasi duplicato
    .description = Due o più URL indicizzabili hanno quasi lo stesso testo visibile anche se il loro HTML non è identico: cambiano una data, un widget o qualche frase. Di solito sono pagine di tag e di categoria che elencano gli stessi articoli, o un articolo riscritto sopra un altro. Google ne sceglie uno da mostrare e mette da parte gli altri. Uniscili, differenziali o imposta il loro canonical sulla pagina principale.
HREFLANG-INVALID-CODE = Codice hreflang non valido
    .description = Il valore dell'attributo non è né una lingua ISO 639-1 con una regione ISO 3166-1 facoltativa né il valore speciale x-default. Google scarta in silenzio un'annotazione malformata, quindi l'insieme resta incompleto senza alcun avviso: è l'errore più frequente e il più difficile da notare a occhio.
HREFLANG-LANG-MISMATCH = Hreflang della pagina in un'altra lingua
    .description = L'annotazione hreflang che punta alla pagina stessa dichiara una lingua diversa dal suo `<html lang>`: una pagina in spagnolo che si presenta come inglese, per esempio. Uno dei due è sbagliato, e se è l'hreflang, Google mostra la pagina a chi cerca nell'altra lingua. Di solito è un plugin di traduzione che copia l'insieme della pagina originale senza cambiare il codice.
HREFLANG-NO-SELF = Hreflang senza autoriferimento
    .description = La pagina dichiara versioni alternative per lingua, ma nessuna punta a sé stessa. Google richiede che ogni pagina dell'insieme includa sé stessa; quando quel link manca, scarta l'intero gruppo e nessuna traduzione beneficia delle altre.
HREFLANG-NOT-RECIPROCAL = Hreflang non reciproco
    .description = Questa pagina dichiara una versione alternativa per lingua che non la dichiara a sua volta. Le annotazioni hreflang sono voti che contano solo quando sono reciproci: Google ignora l'intera relazione quando uno dei due lati non conferma l'altro.
HREFLANG-SELF-ONLY = Hreflang senza alternative
    .description = La pagina dichiara hreflang, ma tutte le annotazioni puntano alla pagina stessa: nell'insieme non c'è alcuna traduzione, quindi non serve a nulla. Se la pagina non ha traduzioni, l'annotazione è superflua; se le ha, mancano i loro link, ed è questo che va davvero controllato: di solito è una traduzione che il plugin ha smesso di collegare.
HREFLANG-TO-4XX = Hreflang verso un URL in errore
    .description = Una versione alternativa per lingua punta a un URL che restituisce 4xx. La traduzione promessa dalla pagina non esiste: un visitatore che cambia lingua finisce su una pagina di errore e Google toglie dall'insieme la pagina che la indica.
HTTP-404-EXTERNAL = Link esterno non funzionante
    .description = Il sito linka a un URL di un altro dominio che non esiste più: risponde 404 o 410, oppure il suo dominio non si risolve. Non penalizza come un 404 del tuo sito, ma manda il visitatore su una pagina di errore e fa invecchiare il contenuto: una guida piena di link morti smette di sembrare aggiornata.
HTTP-404-INTERNAL = Link interno non funzionante
    .description = Un URL di questo sito restituisce 4xx e ci sono pagine che vi linkano. Spreca crawl budget, interrompe il flusso di link interni verso ciò che c'era prima e lascia il visitatore su una pagina di errore.
HTTP-5XX = Errore del server
    .description = L'URL restituisce 5xx: il server non è riuscito a costruire la risposta. Google rimuove dal suo indice le pagine che rispondono così troppo a lungo, e rallenta la scansione dell'intero sito finché il problema dura.
HTTP-EXTERNAL-UNVERIFIABLE = Link esterno non verificabile
    .description = Il sito linka a un URL di un altro dominio che ha rifiutato la verifica: ha risposto 401, 403 o 429. Di solito è una protezione anti-bot, un paywall o un limite di frequenza, e la pagina quasi sempre si apre normalmente in un browser. Non viene definito non funzionante perché da qui non c'è modo di saperlo: aprilo per confermare che esiste ancora.
HTTP-LARGE-PAGE = HTML troppo pesante
    .description = Il documento HTML supera i 500 KB, escluse immagini e script. Ritarda il primo rendering sulle connessioni mobili e di solito significa che il template riversa contenuti che la pagina non mostra mai, o che CSS e JS inline dovrebbero stare in file propri.
HTTP-MIXED-CONTENT = Contenuti misti
    .description = Una pagina servita in HTTPS carica immagini, fogli di stile o script in HTTP. Il browser blocca in silenzio script e fogli di stile e segnala la connessione come non sicura: la pagina sembra rotta e smette di ispirare fiducia.
HTTP-NO-HTTPS = Sito raggiungibile in HTTP
    .description = Alcuni URL interni servono contenuti in HTTP senza reindirizzare a HTTPS. Il sito resta raggiungibile a due indirizzi diversi per la stessa pagina — contenuto duplicato — e il browser avverte il visitatore che la connessione non è sicura.
HTTP-REDIRECT-CHAIN = Catena di reindirizzamenti
    .description = La destinazione finale si raggiunge dopo due o più passaggi consecutivi. Ogni passaggio aggiunge latenza per il visitatore e costo di scansione, e di solito significa che due regole di riscrittura si pestano i piedi. La soluzione è far puntare il primo passaggio direttamente alla fine.
HTTP-REDIRECT-LOOP = Loop di reindirizzamenti
    .description = I reindirizzamenti tornano a un URL già visitato, quindi la destinazione non viene mai raggiunta. Il browser si arrende con un errore e il crawler non vede mai alcun contenuto: a tutti gli effetti, quella parte del sito non esiste.
HTTP-REDIRECT-TO-404 = Reindirizzamento a una pagina di errore
    .description = Il reindirizzamento termina su un URL che restituisce 4xx. È il peggio dei due mondi: il reindirizzamento che lasciava intendere che il contenuto si fosse spostato è ancora lì, ma nemmeno la destinazione esiste, quindi il link e la sua autorevolezza vanno persi comunque.
HTTP-SLOW-RESPONSE = Risposta lenta
    .description = Il server impiega più di un secondo a inviare il primo byte. Quel tempo si somma per intero al tempo di caricamento misurato dai Core Web Vitals, e limita quante pagine il crawler riesce a scansionare a ogni visita.
INDEX-BLOCKED-IN-SITEMAP = Bloccato ma nella sitemap
    .description = La sitemap presenta l'URL come contenuto da indicizzare mentre robots.txt ne vieta la scansione. Le due istruzioni si contraddicono, Search Console lo segnala come errore di copertura e l'URL resta fuori dall'indice. Quasi sempre è un Disallow scritto per altro che ha coinvolto per sbaglio una sezione pubblicata.
INDEX-DEEP-PAGE = Troppi clic dalla home
    .description = Servono più di quattro clic dalla home page per raggiungere questa pagina. Più è lontana, meno autorevolezza interna riceve e meno spesso il motore di ricerca la rivisita; in cataloghi e archivi è il sintomo di una paginazione senza scorciatoie, in cui la pagina 40 è raggiungibile solo attraverso le 39 precedenti.
INDEX-NO-INTERNAL-LINKS-IN = Nessun link interno in entrata
    .description = Nessun'altra pagina del sito linka a questa, anche se è indicizzabile e pubblicata. Senza link in entrata non riceve autorevolezza interna, il motore di ricerca impiega molto più tempo a rivisitarla e le sue modifiche passano inosservate. È la prima cosa da controllare quando una pagina nuova non riesce mai a posizionarsi davvero.
INDEX-NOFOLLOW-INTERNAL = Link interno nofollow
    .description = Questa pagina linka a un'altra pagina dello stesso sito con rel=nofollow. Il link non trasmette autorevolezza e non aiuta a scoprire la destinazione, quindi all'interno di un dominio ha raramente senso: il PageRank sculpting ha smesso di funzionare nel 2009. Di solito deriva da un plugin o da un template che lo aggiunge a tutti i link senza distinguere gli interni dagli esterni.
INDEX-NOINDEX = Noindex
    .description = La pagina chiede di non essere indicizzata, tramite il meta tag robots o l'intestazione X-Robots-Tag. Google la scansiona e la scarta: non comparirà per nessuna query. Su archivi, tag e pagine di servizio è di solito una scelta deliberata del plugin SEO, ed è per questo che l'avviso è moderato; diventa critico sulla home page, e il conflitto con la sitemap ha una regola a sé.
INDEX-NOINDEX-IN-SITEMAP = Noindex ma nella sitemap
    .description = L'URL è nella sitemap, che è l'elenco di ciò che il sito vuole indicizzare, e la sua stessa intestazione o il meta robots dice il contrario. Uno dei due è sbagliato: o non va nella sitemap, o il noindex è un residuo che nessuno ha rimosso. Finché coesistono, il sito si contraddice davanti al motore di ricerca.
INDEX-ORPHAN-PAGE = Pagina orfana
    .description = Il sito dichiara questo URL nella sua sitemap, ma nessuna delle sue pagine vi linka. Un visitatore non può raggiungerlo navigando e il motore di ricerca vede un contenuto senza contesto e senza autorevolezza interna. È il rilievo che nasce dal confronto tra ciò che è dichiarato e ciò che è raggiunto, e nessuna singola pagina può rivelarlo.
INDEX-ROBOTS-BLOCKED = Bloccato da robots.txt
    .description = Il sito linka a questo URL dalle proprie pagine e allo stesso tempo lo vieta in robots.txt. Google non può leggerlo, quindi non sa cosa contiene né segue i suoi link: il link interno non porta da nessuna parte e l'autorevolezza che trasmette va persa. Non è lo stesso di noindex, che consente comunque di leggere la pagina.
INDEX-ROBOTS-TXT-BLOCKS-ALL = robots.txt blocca l'intero sito
    .description = Il robots.txt vieta la scansione della radice del sito, quindi nessuna pagina può essere letta o indicizzata. È il modo più rapido e silenzioso di sparire da Google, e quasi sempre è lo stesso incidente: il file dell'ambiente di staging, che contiene Disallow: /, finito in produzione con un deploy.
INDEX-ROBOTS-TXT-MISSING = robots.txt mancante
    .description = Il sito non serve /robots.txt. Non impedisce l'indicizzazione —senza file, tutto è scansionabile— ma rinuncia al posto in cui si annuncia la sitemap e in cui si escludono le aree che non apportano nulla a un motore di ricerca, come i risultati della ricerca interna o le pagine del carrello.
INDEX-SECTION-DISCONNECTED = Sezione scollegata dai link interni
    .description = Un gruppo di pagine si linkano tra loro ma non si possono raggiungere dalla home page seguendo link normali: il ponte che le unisce al resto del sito è JavaScript, un modulo, oppure manca del tutto. I motori di ricerca le scoprono solo tramite la sitemap e ricevono poca autorevolezza interna. La causa è una sola —manca un link scansionabile verso la sezione— e si corregge una volta, non pagina per pagina.
INDEX-SITEMAP-ERROR = Sitemap con errori
    .description = Una sitemap non risponde, contiene XML malformato o supera i limiti del protocollo (50.000 URL o 50 MB). I motori di ricerca interrompono la lettura all'errore, quindi tutto ciò che segue non viene mai scoperto per quella via, e nulla ti avverte: la sitemap è ancora lì, apparentemente a posto.
INDEX-SITEMAP-MISSING = Nessuna sitemap
    .description = Non è stata trovata alcuna sitemap: robots.txt non ne annuncia una, non si trova nei percorsi abituali e nessuna dichiara URL. Senza sitemap il motore di ricerca raggiunge solo ciò che è linkato, al ritmo consentito dai link interni; e si perde il confronto tra ciò che il sito dichiara di pubblicare e ciò che una scansione raggiunge davvero.
LINK-ANCHOR-EMPTY = Link interno senza testo
    .description = Alcuni link interni non hanno testo, immagine né aria-label: un'icona disegnata in CSS, un intero blocco trasformato in un link vuoto. Il motore di ricerca non ha alcun indizio su dove portano, e chi usa uno screen reader sente «link» e nient'altro. Dagli un testo, anche nascosto visivamente, o un aria-label che dica dove porta.
LINK-ANCHOR-GENERIC = Anchor text generico
    .description = Alcuni link interni consistono solo in una formula fatta come «leggi di più», «qui» o «clicca qui». Quel testo non dice nulla della pagina di destinazione, che è proprio ciò che il motore di ricerca vi legge, e fuori contesto —come li elenca uno screen reader— tutti i link dicono la stessa cosa. Basta che il testo nomini la destinazione: «scopri di più sui prezzi» invece di «leggi di più».
META-DESC-DUPLICATE = Meta description duplicata
    .description = Due o più pagine indicizzabili ripetono la stessa description, quindi il risultato di ricerca non le distingue e nessuna promette qualcosa di specifico. Di solito deriva da un valore predefinito per tutto il sito che nessuno ha sostituito.
META-DESC-MISSING = Meta description mancante
    .description = La pagina indicizzabile non dichiara una description, quindi Google ritaglia uno snippet arbitrario dal corpo per il risultato di ricerca. Non è un fattore di ranking, ma è il testo che decide il clic, e non scriverlo significa non controllarlo.
META-DESC-TOO-LONG = Meta description troppo lunga
    .description = La description non entra nel risultato di ricerca e viene troncata, quindi l'invito all'azione alla fine non viene mai letto. Si misura in pixel, non in caratteri, perché il taglio dipende dalla larghezza visualizzata.
META-DESC-TOO-SHORT = Meta description troppo corta
    .description = La description è così breve che Google tende a ignorarla e a costruire lo snippet dal testo della pagina. Con così poco spazio usato non c'è posto né per l'argomentazione né per la variante della query che giustificherebbe il clic.
META-OUTSIDE-HEAD = Metadati fuori dall'head
    .description = Il canonical, il meta robots, l'hreflang, il title o la description finiscono fuori dall'<head>. Un elemento che non appartiene all'head —un <img> di tracciamento, un <div>, del testo vagante lasciato da un plugin— lo chiude in quel punto, e Google ignora tutto ciò che segue anche se è ancora nel sorgente. L'effetto è lo stesso di non averli.
META-REFRESH = Reindirizzamento meta refresh
    .description = La pagina reindirizza con <meta http-equiv=refresh> invece di un 301. Google lo legge con riserva, non trasmette l'autorevolezza in modo altrettanto affidabile e l'utente vede un salto brusco; con un ritardo diverso da zero blocca anche il pulsante Indietro del browser.
META-TITLE-DUPLICATE = Title duplicato
    .description = Due o più pagine indicizzabili condividono lo stesso <title>. Google deve scegliere quale mostrare per la stessa query, quindi competono tra loro invece di sommarsi. È il sintomo più comune di paginazioni o archivi configurati male.
META-TITLE-MISSING = Title mancante
    .description = La pagina è indicizzabile e non ha un <title>, oppure è vuoto. È il fattore on-page con più peso tra quelli che controlli direttamente, ed è il testo blu nel risultato di ricerca: senza di esso, Google ne inventa uno da ciò che trova.
META-TITLE-MULTIPLE = Tag title multipli
    .description = La pagina contiene più di un <title>. I browser e Google tengono il primo e scartano gli altri in silenzio, quindi il titolo scritto con cura potrebbe non essere quello pubblicato. Quasi sempre è un template che lo stampa due volte, o un plugin SEO che aggiunge il proprio senza rimuovere quello del tema.
META-TITLE-TOO-LONG = Title troppo lungo
    .description = Il title non entra nel risultato di ricerca e Google lo tronca con i puntini di sospensione, quindi la parte finale —spesso il brand, o la keyword lasciata per ultima— non viene mai letta. Si misura in pixel, non in caratteri: il taglio dipende dalla larghezza visualizzata.
META-TITLE-TOO-SHORT = Title troppo corto
    .description = Il title lascia inutilizzata la maggior parte dello spazio offerto dal risultato di ricerca. Non è un errore ma spazio sprecato: c'è posto per una variante della query o per un motivo per cliccare, e spesso indica un template che stampa solo il nome della sezione.
META-VIEWPORT-MISSING = Meta viewport mancante
    .description = Senza <meta name=viewport> un telefono impagina la pagina a 980 px di larghezza e la rimpicciolisce, rendendo il testo illeggibile e costringendo a zoomare con le dita. L'indicizzazione è mobile-first: ciò che Google valuta è quella versione rimpicciolita.
SOCIAL-OG-IMAGE-BROKEN = Immagine Open Graph non funzionante
    .description = L'immagine dichiarata in og:image restituisce un errore, direttamente o alla fine dei suoi reindirizzamenti. Il tag c'è e nessuno strumento che legge solo l'HTML se ne accorge, ma quando la pagina viene condivisa su WhatsApp, LinkedIn o Slack la scheda compare senza immagine, o con un riquadro vuoto. Di solito è un'immagine eliminata dalla libreria media o un percorso che non è sopravvissuto a una migrazione.
SOCIAL-OG-MISSING = Open Graph incompleto
    .description = Manca og:title, og:description o og:image. Quando qualcuno condivide la pagina su WhatsApp, LinkedIn o Slack, il social network non riesce a costruire la scheda di anteprima e il link compare come un semplice URL: non influisce sul posizionamento, ma il clic va perso.
