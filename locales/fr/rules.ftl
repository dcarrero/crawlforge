### CrawlForge — rule catalogue: names and descriptions, French.
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules. Typography and
### glossary: see the header of cli.ftl in this folder.

### Each rule is one message: its ID, the short name as the value, and the description
### as the `.description` attribute. Backticks and <tags> are literal text, keep them.

ASSET-BROKEN = CSS ou JS cassé
    .description = Un fichier CSS ou JS du site renvoie une erreur 4xx ou 5xx. Google affiche la page pour l'indexer : une feuille de style absente peut lui faire voir une page sans mise en forme —donc pas adaptée aux mobiles—, et un script absent peut laisser vide le contenu affiché côté client. C'est typique d'un déploiement dont les empreintes de fichiers ne correspondent plus.
ASSET-CSS-HEAVY = Feuille de style trop lourde
    .description = La feuille de style dépasse 100 Ko telle qu'elle est servie. Le navigateur n'affiche rien tant qu'il ne l'a pas reçue : ces octets coûtent donc plus cher que ceux d'un script, car ils retardent tout le premier affichage. C'est pourquoi le seuil est plus bas. Il s'agit presque toujours d'un framework complet livré pour n'en utiliser qu'un dixième.
ASSET-FORM-BROKEN = Formulaire envoyé vers une URL cassée
    .description = L'`action` d'un formulaire pointe vers une URL de votre site qui renvoie une erreur 4xx ou 5xx. Le formulaire a l'air normal, se remplit normalement et se perd à l'envoi : l'un des rares défauts qui coûtent directement des clients, et personne ne s'en aperçoit, car celui qui tombe dessus s'en va sans rien dire. Seuls les formulaires envoyés en GET sont vérifiés (un champ de recherche, un filtre de catalogue) : vérifier un POST voudrait dire l'envoyer, et cet outil n'envoie pas de formulaires.
ASSET-IFRAME-BROKEN = Cadre intégré cassé
    .description = Un `<iframe>` de la page pointe vers une URL de votre site qui renvoie une erreur 4xx ou 5xx : le visiteur voit un trou blanc à la place de la carte, de la vidéo ou du formulaire. Cela ne laisse aucune trace dans la page qui le contient —le HTML reste valide—, c'est pourquoi le problème passe inaperçu pendant des mois.
ASSET-IMG-BROKEN = Image cassée
    .description = Une image vers laquelle pointe le site renvoie une erreur 4xx ou 5xx. Le vide se voit sur la page, l'image n'existe pas pour la recherche d'images, et chaque visite gaspille une requête sur une erreur. C'est presque toujours une migration qui a oublié le dossier des médias, ou un chemin écrit à la main.
ASSET-IMG-EMPTY-ALT-LINK = Lien avec une seule image à alt vide
    .description = Un lien dont le seul contenu est une image avec `alt=""` n'a pas de nom accessible : une personne qui utilise un lecteur d'écran entend « lien » et rien d'autre, et le moteur de recherche ne reçoit aucun signal sur sa destination. Le cas typique est le logo de l'en-tête, généralement le lien le plus répété du site. Ici, l'`alt` n'est pas décoratif : c'est le texte du lien.
ASSET-IMG-HEAVY = Image trop lourde
    .description = L'image dépasse 200 Ko. C'est la cause la plus fréquente d'un mauvais LCP sur mobile : elle retarde l'affichage de l'élément principal et consomme la bande passante d'une connexion lente. L'exporter à la taille où elle s'affiche réellement et la servir en WebP ou AVIF suffit généralement.
ASSET-IMG-NO-ALT = Image sans attribut alt
    .description = Certaines images n'ont pas d'attribut `alt`. Un lecteur d'écran n'a rien à lire et finit par épeler le nom du fichier, et Google perd le seul texte qui décrit l'image : c'est ce qui la positionne dans la recherche d'images et ce qui s'affiche quand elle ne se charge pas. Un `alt=""` vide est valide, mais seulement pour les images décoratives.
ASSET-JS-HEAVY = Script trop lourd
    .description = Le script dépasse 250 Ko tel qu'il est servi. Chaque page qui le charge paie le téléchargement et, surtout, son analyse et son exécution sur le thread principal, d'où vient la mauvaise réactivité sur un téléphone ordinaire. C'est généralement un bundle que personne n'a découpé : déterminez ce qui est nécessaire au premier écran et chargez le reste séparément.
CANON-CHAIN = Chaîne de canoniques
    .description = A déclare B comme URL canonique, et B déclare C. Pour Google, les canoniques ne sont pas transitives : quand il trouve une chaîne, il l'ignore et décide seul quelle URL est la principale. Chaque étape devrait pointer directement vers C.
CANON-CROSS-DOMAIN = Canonique vers un autre domaine
    .description = L'URL canonique pointe vers un autre hôte que celui de la page. C'est légitime pour du contenu syndiqué, mais quand ce n'est pas voulu, cela cède le positionnement à l'autre domaine : la page elle-même cesse d'être indexée. Cela vient généralement d'une migration inachevée ou d'un environnement de préproduction copié.
CANON-MISSING = Canonique absente
    .description = La page indexable ne déclare pas de rel=canonical. Ce n'est pas grave en soi, car Google déduit l'URL canonique, mais sans elle n'importe quel paramètre d'URL — une campagne, un filtre, un ordre de tri — peut finir indexé comme une page distincte.
CANON-MULTIPLE = Canoniques multiples
    .description = La page déclare plus d'un link rel=canonical. Google n'en choisit pas une : il les ignore toutes, ce qui revient à n'en avoir aucune. C'est presque toujours le thème et une extension SEO qui émettent chacun la balise de leur côté.
CANON-RELATIVE = Canonique relative
    .description = L'URL canonique est déclarée comme une référence relative au lieu d'une URL absolue. Cela fonctionne tant que la page est servie depuis un seul endroit, mais si le HTML est reproduit sous un autre hôte — une préproduction, un proxy, un scraper — la canonique se résout par rapport à cet hôte et cesse de pointer vers l'original.
CANON-TO-4XX = Canonique vers une URL en erreur
    .description = L'URL canonique pointe vers une URL qui répond par une erreur. La page se déclare doublon de quelque chose qui n'existe pas : ni elle ni la cible ne peuvent être indexées, et le contenu disparaît complètement des résultats.
CANON-TO-NOINDEX = Canonique vers une page en noindex
    .description = L'URL canonique pointe vers une page marquée noindex. Les deux signaux se contredisent : l'un dit « indexe celle-là » et celle-là dit « ne m'indexe pas ». Le résultat habituel est la perte des deux URL.
CANON-TO-REDIRECT = Canonique vers une redirection
    .description = L'URL canonique pointe vers une URL qui redirige ailleurs. Google doit choisir entre le signal de la canonique et celui de la redirection, et garde généralement la destination finale, ce qui rend la balise inutile. Faites-la pointer directement vers l'URL qui répond 200.
CONTENT-H1-EMPTY = H1 vide
    .description = Il y a un H1 dans le code, mais il n'apporte aucun texte : il est vide, ou son seul contenu est une image qui ne dit rien. Le titre occupe la place du titre principal sans en faire le travail : ni le moteur de recherche ni un lecteur d'écran n'obtiennent le sujet de la page. Le cas habituel est un logo à l'intérieur du H1.
CONTENT-H1-MISSING = H1 absent
    .description = La page indexable n'a pas de H1, ou il est vide. Le H1 dit au moteur de recherche de quoi parle la page avec les mots de l'auteur, et c'est le premier titre qu'annonce un lecteur d'écran en entrant dans le contenu.
CONTENT-H1-MULTIPLE = H1 multiples
    .description = La page a plus d'un H1. HTML5 le permet, mais il n'y a plus un seul sujet principal : le moteur de recherche doit deviner de quoi parle la page, et un lecteur d'écran annonce plusieurs titres de niveau un comme s'il s'agissait de documents distincts. C'est généralement le modèle qui balise le logo ou le nom du site en H1 en plus du vrai titre.
CONTENT-HEADING-SKIP = Niveau de titre sauté
    .description = Les niveaux de titre descendent de plus d'un cran d'un coup, par exemple de H2 à H4. Le plan du document se retrouve troué : une personne qui navigue par titres ne peut pas savoir si le H4 appartient à ce H2 ou à une autre section, et le moteur de recherche perd la hiérarchie qui lui sert à comprendre ce qui dépend de quoi. Cela vient généralement du choix d'un titre pour sa taille de police plutôt que pour son niveau.
CONTENT-LANG-MISSING = Attribut lang absent
    .description = L'élément <html> ne déclare pas la langue du contenu. Sans lui, un lecteur d'écran prononce le texte selon les règles de la langue du système, le navigateur ne sait pas quel dictionnaire utiliser pour traduire, et les signaux de langue du site sont laissés à la détection automatique. C'est un attribut d'une ligne.
CONTENT-THIN = Contenu pauvre
    .description = Une page indexable avec moins de 300 mots de texte visible. Elle a rarement assez de substance pour répondre à une requête : elle se positionne mal et, en masse, dilue la qualité moyenne du site aux yeux du moteur de recherche. Beaucoup sont des archives, des pages d'étiquettes ou des ébauches générées automatiquement, qu'il vaut mieux désindexer qu'étoffer.
DUP-CONTENT-EXACT = Contenu identique
    .description = Deux URL indexables ou plus renvoient exactement le même HTML, octet pour octet. Elles se font concurrence sur les mêmes requêtes et se partagent les liens entrants au lieu de les additionner. Corrigez-le en gardant une seule URL indexable et en faisant pointer la canonique des autres vers elle.
DUP-CONTENT-NEAR = Contenu quasi dupliqué
    .description = Deux URL indexables ou plus ont presque le même texte visible alors que leur HTML n'est pas identique : seuls une date, un widget ou quelques phrases changent. Ce sont généralement des pages d'étiquettes et de catégories qui listent les mêmes articles, ou un article réécrit par-dessus un autre. Google en choisit une à afficher et met les autres de côté. Fusionnez-les, différenciez-les ou faites pointer leur canonique vers la page principale.
HREFLANG-INVALID-CODE = Code hreflang invalide
    .description = La valeur de l'attribut n'est ni une langue ISO 639-1 suivie éventuellement d'une région ISO 3166-1, ni la valeur spéciale x-default. Google écarte sans rien dire une annotation mal formée, et l'ensemble reste incomplet sans avertissement : c'est l'échec le plus fréquent et le plus difficile à repérer à l'œil.
HREFLANG-LANG-MISMATCH = Hreflang propre dans une autre langue que la page
    .description = L'annotation hreflang qui pointe vers la page elle-même déclare une langue différente de son `<html lang>` : une page en espagnol qui s'annonce en anglais, par exemple. L'un des deux est faux, et si c'est le hreflang, Google sert la page à ceux qui cherchent dans l'autre langue. C'est généralement une extension de traduction qui copie l'ensemble de la page d'origine sans changer le code.
HREFLANG-NO-SELF = Hreflang sans autoréférence
    .description = La page déclare des alternatives linguistiques, mais aucune ne pointe vers elle-même. Google exige que chaque page de l'ensemble s'inclue elle-même ; quand ce lien manque, il écarte tout le groupe et aucune traduction ne profite des autres.
HREFLANG-NOT-RECIPROCAL = Hreflang non réciproque
    .description = Cette page déclare une alternative linguistique qui ne la déclare pas en retour. Les annotations hreflang sont des votes qui ne comptent que s'ils sont mutuels : Google ignore toute la relation quand l'un des deux côtés ne confirme pas l'autre.
HREFLANG-SELF-ONLY = Hreflang sans alternative
    .description = La page déclare un hreflang, mais toutes les annotations pointent vers la page elle-même : l'ensemble ne contient aucune traduction, il ne sert donc à rien. Si la page n'a pas de traduction, l'annotation est inutile ; si elle en a, ses liens manquent, et c'est cela qu'il faut vraiment vérifier : c'est généralement une traduction que l'extension a cessé de lier.
HREFLANG-TO-4XX = Hreflang vers une URL en erreur
    .description = Une alternative linguistique pointe vers une URL qui renvoie une erreur 4xx. La traduction promise par la page n'existe pas : un visiteur qui change de langue arrive sur une page d'erreur et Google retire de l'ensemble la page qui y renvoie.
HTTP-404-EXTERNAL = Lien externe cassé
    .description = Le site pointe vers une URL d'un autre domaine qui n'existe plus : elle répond 404 ou 410, ou son domaine ne se résout pas. Cela ne pénalise pas comme une 404 de votre propre site, mais cela envoie le visiteur sur une page d'erreur et vieillit le contenu : un guide plein de liens morts n'a plus l'air entretenu.
HTTP-404-INTERNAL = Lien interne cassé
    .description = Une URL de ce site renvoie une erreur 4xx et des pages pointent vers elle. Cela gaspille du budget d'exploration, coupe le flux de liens internes vers ce qui s'y trouvait, et laisse le visiteur sur une page d'erreur.
HTTP-5XX = Erreur serveur
    .description = L'URL renvoie une erreur 5xx : le serveur n'a pas réussi à construire la réponse. Google retire de son index les pages qui répondent ainsi trop longtemps, et ralentit l'exploration de tout le site tant que cela dure.
HTTP-EXTERNAL-UNVERIFIABLE = Lien externe invérifiable
    .description = Le site pointe vers une URL d'un autre domaine qui a refusé la vérification : elle a répondu 401, 403 ou 429. Il s'agit généralement d'une protection anti-robots, d'un paywall ou d'une limite de débit, et la page s'ouvre presque toujours normalement dans un navigateur. Elle n'est pas déclarée cassée parce qu'il est impossible de le savoir d'ici : ouvrez-la pour confirmer qu'elle existe toujours.
HTTP-LARGE-PAGE = HTML trop lourd
    .description = Le document HTML dépasse 500 Ko, hors images et scripts. Il retarde le premier affichage sur les connexions mobiles et signifie généralement que le modèle déverse du contenu que la page n'affiche jamais, ou que du CSS et du JS intégrés devraient vivre dans leurs propres fichiers.
HTTP-MIXED-CONTENT = Contenu mixte
    .description = Une page servie en HTTPS charge des images, des feuilles de style ou des scripts en HTTP. Le navigateur bloque sans rien dire les scripts et les feuilles de style et signale la connexion comme non sécurisée : la page a l'air cassée et n'inspire plus confiance.
HTTP-NO-HTTPS = Site accessible en HTTP
    .description = Certaines URL internes servent du contenu en HTTP sans rediriger vers HTTPS. Le site reste accessible à deux adresses différentes pour la même page — du contenu dupliqué — et le navigateur avertit le visiteur que la connexion n'est pas sécurisée.
HTTP-REDIRECT-CHAIN = Chaîne de redirections
    .description = La destination finale n'est atteinte qu'après deux sauts consécutifs ou plus. Chaque saut ajoute de la latence pour le visiteur et du coût d'exploration, et signifie généralement que deux règles de réécriture se marchent dessus. La solution est de faire pointer le premier saut directement vers la fin.
HTTP-REDIRECT-LOOP = Boucle de redirections
    .description = Les redirections reviennent à une URL déjà visitée : la destination n'est jamais atteinte. Le navigateur abandonne avec une erreur et le robot d'exploration ne voit jamais aucun contenu : en pratique, cette partie du site n'existe pas.
HTTP-REDIRECT-TO-404 = Redirection vers une page d'erreur
    .description = La redirection aboutit à une URL qui renvoie une erreur 4xx. C'est le pire des deux mondes : la redirection qui laissait entendre que le contenu avait déménagé est toujours là, mais la destination n'existe pas non plus, si bien que le lien et son autorité sont perdus quand même.
HTTP-SLOW-RESPONSE = Réponse lente
    .description = Le serveur met plus d'une seconde à envoyer le premier octet. Ce temps s'ajoute en entier au temps de chargement que mesurent les Core Web Vitals, et il limite le nombre de pages que le robot d'exploration parcourt à chaque visite.
INDEX-BLOCKED-IN-SITEMAP = Bloquée mais dans le sitemap
    .description = Le sitemap présente l'URL comme un contenu que vous voulez voir indexé, alors que robots.txt en interdit l'exploration. Les deux instructions se contredisent, la Search Console le signale comme une erreur de couverture et l'URL reste hors de l'index. C'est presque toujours un Disallow écrit pour autre chose qui a emporté au passage une section publiée.
INDEX-DEEP-PAGE = Trop de clics depuis l'accueil
    .description = Il faut plus de quatre clics depuis la page d'accueil pour atteindre cette page. Plus elle est loin, moins elle reçoit d'autorité interne et moins souvent le moteur de recherche y revient ; dans les catalogues et les archives, c'est le symptôme d'une pagination sans raccourcis, où la page 40 n'est accessible qu'à travers les 39 précédentes.
INDEX-NO-INTERNAL-LINKS-IN = Aucun lien interne entrant
    .description = Aucune autre page du site ne pointe vers celle-ci, bien qu'elle soit indexable et publiée. Sans liens entrants, elle ne reçoit aucune autorité interne, le moteur de recherche met beaucoup plus de temps à y revenir et ses modifications passent inaperçues. C'est la première chose à vérifier quand une nouvelle page n'arrive jamais vraiment à se positionner.
INDEX-NOFOLLOW-INTERNAL = Lien interne en nofollow
    .description = Cette page pointe vers une autre page du même site avec rel=nofollow. Le lien ne transmet aucune autorité et n'aide pas à découvrir la cible : au sein d'un même domaine, cela a rarement du sens, et le PageRank sculpting ne fonctionne plus depuis 2009. Cela vient généralement d'une extension ou d'un modèle qui l'ajoute à tous les liens sans distinguer les internes des externes.
INDEX-NOINDEX = Noindex
    .description = La page demande à ne pas être indexée, par sa balise meta robots ou par l'en-tête X-Robots-Tag. Google l'explore et l'écarte : elle n'apparaîtra pour aucune requête. Sur les archives, les étiquettes et les pages utilitaires, c'est généralement un choix délibéré de l'extension SEO, c'est pourquoi l'alerte est modérée ; elle passe à critique sur la page d'accueil, et le conflit avec le sitemap a sa propre règle.
INDEX-NOINDEX-IN-SITEMAP = Noindex mais dans le sitemap
    .description = L'URL figure dans le sitemap, qui est la liste de ce que le site veut voir indexé, et son propre en-tête ou sa meta robots dit le contraire. L'un des deux est faux : soit elle n'a pas sa place dans le sitemap, soit le noindex est un reste que personne n'a retiré. Tant que les deux coexistent, le site se contredit face au moteur de recherche.
INDEX-ORPHAN-PAGE = Page orpheline
    .description = Le site déclare cette URL dans son sitemap, mais aucune de ses pages ne pointe vers elle. Un visiteur ne peut pas l'atteindre en naviguant, et le moteur de recherche voit un contenu sans contexte et sans autorité interne. C'est le constat qui naît de la comparaison entre ce qui est déclaré et ce qui est atteint, et aucune page prise isolément ne peut le révéler.
INDEX-ROBOTS-BLOCKED = Bloquée par robots.txt
    .description = Le site pointe vers cette URL depuis ses propres pages et en même temps l'interdit dans robots.txt. Google ne peut pas la lire : il ne sait pas ce qu'elle contient et ne suit pas ses liens, si bien que le lien interne ne mène nulle part et l'autorité qu'il transmet est perdue. Ce n'est pas la même chose qu'un noindex, qui permet toujours de lire la page.
INDEX-ROBOTS-TXT-BLOCKS-ALL = robots.txt bloque tout le site
    .description = Le robots.txt interdit l'exploration de la racine du site : aucune page ne peut être lue ni indexée. C'est la façon la plus rapide et la plus discrète de disparaître de Google, et c'est presque toujours le même accident : le fichier de préproduction, qui contient Disallow: /, passé en production lors d'un déploiement.
INDEX-ROBOTS-TXT-MISSING = robots.txt absent
    .description = Le site ne sert pas de /robots.txt. Cela n'empêche pas l'indexation —sans fichier, tout est explorable— mais cela renonce à l'endroit où l'on annonce le sitemap et où l'on exclut les zones qui n'apportent rien à un moteur de recherche, comme les résultats de recherche interne ou les pages de panier.
INDEX-SECTION-DISCONNECTED = Section déconnectée du maillage interne
    .description = Un groupe de pages se lient entre elles, mais on ne peut pas les atteindre depuis la page d'accueil en suivant des liens ordinaires : le pont qui les relie au reste du site est en JavaScript, un formulaire, ou n'existe pas du tout. Les moteurs de recherche ne les découvrent que par le sitemap et peu d'autorité interne leur parvient. La cause est unique —il manque un lien explorable vers la section— et elle se corrige une fois, pas page par page.
INDEX-SITEMAP-ERROR = Sitemap en erreur
    .description = Un sitemap ne répond pas, contient du XML mal formé ou dépasse les limites du protocole (50 000 URL ou 50 Mo). Les moteurs de recherche arrêtent la lecture à l'erreur : tout ce qui suit n'est jamais découvert par cette voie, et rien ne vous avertit, car le sitemap est toujours là, apparemment en bon état.
INDEX-SITEMAP-MISSING = Aucun sitemap
    .description = Aucun sitemap n'a été trouvé : robots.txt n'en annonce pas, il n'est pas aux emplacements habituels, et aucun ne déclare d'URL. Sans lui, le moteur de recherche n'atteint que ce qui est lié, au rythme que permet le maillage interne ; et vous perdez la comparaison entre ce que le site prétend publier et ce qu'une exploration atteint réellement.
LINK-ANCHOR-EMPTY = Lien interne sans texte
    .description = Certains liens internes n'ont ni texte, ni image, ni aria-label : une icône dessinée en CSS, tout un bloc transformé en lien vide. Le moteur de recherche n'a aucun indice sur leur destination, et une personne qui utilise un lecteur d'écran entend « lien » et rien d'autre. Donnez-leur un texte, même visuellement masqué, ou un aria-label qui dit où ils mènent.
LINK-ANCHOR-GENERIC = Texte d'ancre générique
    .description = Certains liens internes se résument à une formule toute faite comme « lire la suite », « ici » ou « cliquez ici ». Ce texte ne dit rien de la page cible, alors que c'est justement ce que le moteur de recherche y lit, et hors contexte —tels qu'un lecteur d'écran les liste— tous les liens disent la même chose. Il suffit que le texte nomme la cible : « en savoir plus sur les tarifs » au lieu de « lire la suite ».
LINK-FRAGMENT-BROKEN = Lien vers une ancre inexistante
    .description = Certains liens internes pointent vers un `#fragment` qu'aucun élément de la page cible ne porte comme `id` ni comme `<a name>`. Le lien ouvre la page tout en haut au lieu de la section promise : une table des matières dont le titre a été renommé, un lien vers un bloc supprimé, un « aller au contenu » vers un id que le thème n'imprime plus. Un id ajouté par JavaScript n'est pas dans le HTML servi et est tout de même signalé : vérifiez ce cas dans un navigateur. Corrigez le lien ou rendez son id à la section.
META-DESC-DUPLICATE = Meta description dupliquée
    .description = Deux pages indexables ou plus répètent la même description : le résultat de recherche ne les distingue pas et aucune ne promet quelque chose de précis. Cela vient généralement d'une valeur par défaut pour tout le site que personne n'a remplacée.
META-DESC-MISSING = Meta description absente
    .description = La page indexable ne déclare pas de description : Google découpe un extrait arbitraire du corps pour le résultat de recherche. Ce n'est pas un facteur de positionnement, mais c'est le texte qui décide du clic, et ne pas l'écrire, c'est renoncer à le maîtriser.
META-DESC-TOO-LONG = Meta description trop longue
    .description = La description ne tient pas dans le résultat de recherche et se retrouve tronquée : l'appel à l'action placé à la fin n'est jamais lu. Elle se mesure en pixels et non en caractères, car la coupe dépend de la largeur affichée.
META-DESC-TOO-SHORT = Meta description trop courte
    .description = La description est si brève que Google tend à l'ignorer et à construire l'extrait à partir du texte de la page. Avec si peu d'espace utilisé, il n'y a de place ni pour l'argument ni pour la variante de la requête qui justifierait le clic.
META-OUTSIDE-HEAD = Métadonnées hors du head
    .description = La canonique, la meta robots, le hreflang, le title ou la description se retrouvent hors du <head>. Un élément qui n'a pas sa place dans le head —une <img> de suivi, une <div>, du texte égaré laissé par une extension— le ferme à cet endroit, et Google ignore tout ce qui suit, même si c'est toujours dans le code source. L'effet est le même que si ces balises n'existaient pas.
META-REFRESH = Redirection meta refresh
    .description = La page redirige avec <meta http-equiv=refresh> au lieu d'une 301. Google la lit avec réserve, ne transmet pas l'autorité de façon aussi fiable, et l'utilisateur subit un saut brusque ; avec un délai non nul, elle piège en plus le bouton Retour du navigateur.
META-TITLE-DUPLICATE = Title dupliqué
    .description = Deux pages indexables ou plus partagent le même <title>. Google doit choisir laquelle afficher pour une même requête : elles se font concurrence au lieu de s'additionner. C'est le symptôme le plus courant d'une pagination ou d'archives mal configurées.
META-TITLE-MISSING = Title absent
    .description = La page est indexable et n'a pas de <title>, ou il est vide. C'est le facteur on-page qui a le plus de poids parmi ceux que vous maîtrisez directement, et c'est le texte bleu du résultat de recherche : sans lui, Google en invente un à partir de ce qu'il trouve.
META-TITLE-MULTIPLE = Balises title multiples
    .description = La page contient plus d'un <title>. Les navigateurs et Google gardent le premier et écartent les autres sans rien dire : le titre rédigé avec soin n'est peut-être pas celui qui est publié. C'est presque toujours un modèle qui l'affiche deux fois, ou une extension SEO qui ajoute le sien sans retirer celui du thème.
META-TITLE-TOO-LONG = Title trop long
    .description = Le titre ne tient pas dans le résultat de recherche et Google le tronque avec des points de suspension : la fin —souvent la marque, ou le mot-clé laissé pour la fin— n'est jamais lue. Il se mesure en pixels et non en caractères : la coupe dépend de la largeur affichée.
META-TITLE-TOO-SHORT = Title trop court
    .description = Le titre laisse inutilisée la plus grande partie de l'espace qu'offre le résultat de recherche. Ce n'est pas une erreur, mais de l'espace perdu : il y a de la place pour une variante de la requête ou une raison de cliquer, et cela révèle souvent un modèle qui n'affiche que le nom de la rubrique.
META-VIEWPORT-MISSING = Meta viewport absente
    .description = Sans <meta name=viewport>, un téléphone met la page en page sur 980 px de large et la réduit, ce qui rend le texte illisible et oblige à zoomer avec les doigts. L'indexation se fait d'abord sur mobile : c'est cette version réduite que Google évalue.
SOCIAL-OG-IMAGE-BROKEN = Image Open Graph cassée
    .description = L'image déclarée dans og:image renvoie une erreur, directement ou au bout de ses redirections. La balise est là et aucun outil qui se contente de lire le HTML ne s'en aperçoit, mais quand la page est partagée sur WhatsApp, LinkedIn ou Slack, la carte s'affiche sans image, ou avec un cadre vide. C'est généralement une image supprimée de la médiathèque ou un chemin qui n'a pas survécu à une migration.
SOCIAL-OG-MISSING = Open Graph incomplet
    .description = og:title, og:description ou og:image est absent. Quand quelqu'un partage la page sur WhatsApp, LinkedIn ou Slack, le réseau ne peut pas construire la carte d'aperçu et le lien s'affiche comme une URL nue : cela n'affecte pas le positionnement, mais le clic est perdu.
