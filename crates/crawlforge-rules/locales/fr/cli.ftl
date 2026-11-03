### CrawlForge — command-line messages, French.
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules.
###
### Typography: a plain space before « : ; ? ! » and before « % », and « » quotation marks with
### plain spaces inside. No non-breaking spaces: a terminal does not need them and they are
### invisible when editing. Straight apostrophe ('). Imperatives use « vous ».
###
### Glossary: crawl = exploration (explorer); crawl file = fichier d'exploration;
### finding = constat; severity = gravité (critique, élevée, moyenne, faible);
### canonical = URL canonique; template = modèle; portfolio = portefeuille;
### diff = comparaison; tier = offre; scope = portée; URL is invariable (les URL).

## Severity names

# Severity with an initial capital, for report headings.
severity_label_critical = Critique
severity_label_high = Élevée
severity_label_medium = Moyenne
severity_label_low = Faible
severity_label_info = Info
# Severity in lower case, for table columns.
severity_word_critical = critique
severity_word_high = élevée
severity_word_medium = moyenne
severity_word_low = faible
severity_word_info = info

## Non-indexability reasons

reason_noindex = noindex
reason_canonicalised = canonicalisée
reason_robots = robots
reason_redirect = redirection
reason_4xx = 4xx
reason_5xx = 5xx
reason_not_html = pas du HTML

## Exclusion reasons

exclusion_robots = robots.txt
exclusion_nofollow = nofollow
exclusion_depth = profondeur maximale
exclusion_pattern = motif d'URL
exclusion_limit = limite d'URL
exclusion_local_network = réseau local
exclusion_external_check_off = vérification externe désactivée
exclusion_no_network = audit de dossier, sans réseau

## Crawl summary (`report.rs`)

crawl_finished = Exploration terminée : { $urls } URL en { $secs } s
crawl_failed_suffix = {" "}({ $n } en échec)
crawl_truncated = Exploration tronquée par { $reason }. Les constats affichés sont ceux trouvés jusqu'à ce point.
# A 429 is not a page's status: the site asked the crawler to slow down. The pages
# were left pending, and the useful thing to say is how to get them.
crawl_rate_limited_note = Le site a limité le débit de l'exploration : { $n ->
        [one] { $n } page a continué de répondre 429 Too Many Requests, ou a demandé d'attendre plus de deux minutes, et est restée en attente, sans être signalée comme cassée.
       *[other] { $n } pages ont continué de répondre 429 Too Many Requests, ou ont demandé d'attendre plus de deux minutes, et sont restées en attente, sans être signalées comme cassées.
    } Les règles qui ont besoin du site entier ne sont pas évaluées tant que ces pages n'ont pas été récupérées. Plus tard, lancez :
    {"    "}crawlforge resume { $path }
    ou relancez l'exploration avec un --concurrency plus bas, ou laissez passer le user-agent de CrawlForge dans le pare-feu du site.
report_rate_limited_note = > **Le site a limité le débit de l'exploration.** { $n ->
        [one] { $n } page a continué de répondre 429 Too Many Requests, ou a demandé d'attendre plus de deux minutes, et est restée en attente : elle n'est ni comptée ni signalée comme cassée
       *[other] { $n } pages ont continué de répondre 429 Too Many Requests, ou ont demandé d'attendre plus de deux minutes, et sont restées en attente : elles ne sont ni comptées ni signalées comme cassées
    }, et les règles qui ont besoin du site entier ne sont pas évaluées. `crawlforge resume` les récupère.
crawl_throttled_note = Le serveur a montré des signes de surcharge (429, 503, connexions coupées ou délais dépassés), donc l'exploration a ralenti : sa concurrence a été divisée par deux { $n ->
        [one] une fois
       *[other] { $n } fois
    }. La prochaine fois, essayez un --concurrency plus bas.
crawl_list_mode_note = Exploration en mode liste : seules les URL fournies ont été auditées. Les règles qui ont besoin du graphe de liens complet du site (pages orphelines, liens entrants, profondeur) ne sont pas évaluées.
external_unchecked = { $n ->
        [one] { $n } lien externe n'a pas été vérifié
       *[other] { $n } liens externes n'ont pas été vérifiés
    } : la limite max_external a été atteinte. Augmentez-la avec --max-external.
external_unregistered = { $n ->
        [one] { $n } lien externe n'a même pas été enregistré
       *[other] { $n } liens externes n'ont même pas été enregistrés
    } : la limite max_external_urls a été atteinte. Cette limite existe pour le cas d'une seule page qui pointe vers des centaines de milliers d'hôtes.
external_never_checked = { $n ->
        [one] { $n } lien externe a été enregistré mais jamais vérifié.
       *[other] { $n } liens externes ont été enregistrés mais jamais vérifiés.
    }
external_offline = { $n ->
        [one] { $n } lien externe n'a pas été vérifié
       *[other] { $n } liens externes n'ont pas été vérifiés
    } : l'audit d'un dossier ne fait aucune requête réseau. Pour les vérifier, explorez le site publié avec `crawlforge crawl <URL>`.
external_check_disabled = Les liens externes n'ont pas été vérifiés (--no-external-check) : l'absence de liens externes cassés dans ce rapport ne veut pas dire qu'il n'y en a pas.
external_checked_note = { $n ->
        [one] { $n } lien externe vérifié.
       *[other] { $n } liens externes vérifiés.
    }
rules_not_evaluated = Les règles qui ont besoin du graphe de liens complet n'ont pas été évaluées : { $rules }.
results_title = Résultats
label_urls = URL
label_html_pages = Pages HTML
label_links = Liens
note_internal = { $n ->
        [one] { $n } interne
       *[other] { $n } internes
    }
note_indexable = { $n ->
        [one] { $n } indexable
       *[other] { $n } indexables
    }
heading_status_codes = Codes de statut
label_no_response = sans réponse
note_request_failed = la requête a échoué
label_never_requested = Jamais demandées
note_never_requested = (découvertes, mais l'exploration s'est arrêtée avant de les récupérer)
label_out_of_scope = Hors périmètre
note_out_of_scope = (liées depuis des pages auditées, mais hors du périmètre de cette exploration)
heading_non_indexable = Non indexables, pourquoi
heading_findings = Constats
no_findings = Aucun constat.
heading_excluded = Exclues
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = { $k ->
        [one] { $k } problème de modèle ({ $pages } pages)
       *[other] { $k } problèmes de modèle ({ $pages } pages)
    }
plus_more_findings = {" "}+ { $n ->
        [one] { $n } autre constat
       *[other] { $n } autres constats
    }
example_urls = ex. { $urls }

## Pervasive rules, reworded

pervasive_note = { $pct } % du site
deep_pages_summary = { $pages } pages à plus de { $max } clics — { $pct } % du site (profondeur typique { $low }–{ $high }, maximale { $deepest })
rule_deep_sorted = Triées par profondeur de clic, les plus profondes d'abord ; la première colonne indique les clics depuis l'accueil.
hint_full_lists = Liste complète des URL d'une règle : { $cmd }

## One rule's listing (`report --rule`)

rule_affected_urls = { $n ->
        [one] { $n } URL concernée
       *[other] { $n } URL concernées
    }
rule_template_group = Groupe de modèle — { $n ->
        [one] { $n } page
       *[other] { $n } pages
    }
rule_group_cause = cause : { $detail }
rule_other_pages = Autres pages concernées — { $n }
rule_no_findings = Aucun constat pour { $rule } dans cette exploration.
hint_who_links = Un lien cassé se corrige sur la page qui le contient. Qui pointe vers une URL, avec son texte d'ancre :
error_unknown_rule = { $rule } n'est pas un ID de règle. Le catalogue s'affiche avec `crawlforge rules`.
error_all_blocked_by_robots = robots.txt bloque toutes les URL : il n'y a rien à auditer. Si le site vous appartient, vous pouvez réessayer avec --ignore-robots.
error_all_excluded_by_pattern = toutes les URL ont été écartées par vos motifs --exclude/--include : il n'y a rien à auditer. Vérifiez les motifs.
error_no_urls_fetched = l'exploration n'a récupéré aucune URL. Vérifiez la cible.
error_dead_seed = impossible de se connecter à { $url } ({ $cause }). Vérifiez l'URL.
cause_dns = le domaine n'existe pas ou ne se résout pas
cause_tls = le certificat TLS n'est pas valide
cause_timeout = le serveur n'a pas répondu à temps
cause_connection = connexion refusée
cause_no_response = aucune réponse
warn_base_mismatch = Attention : { $n } URL canoniques sur { $total } pointent vers { $origin }, et non vers la base indiquée ({ $base }).
    Les résultats d'indexabilité dépendent de ce paramètre : si le site est publié sur
    { $origin }, relancez l'audit avec --base { $origin }

## One URL's sheet (`inspect.rs`)

inspect_error_not_found = { $url } ne fait pas partie de cette exploration.
inspect_closest = Les URL les plus proches dans le fichier sont :
inspect_status_title = Statut
label_http_status = Statut HTTP
label_content_type = Type de contenu
label_response_time = Temps de réponse
label_depth = Profondeur
note_depth_clicks = clics depuis l'URL de départ
label_in_sitemap = Dans le sitemap
word_yes = oui
word_no = non
label_crawl_state = État d'exploration
state_note_pending = découverte, mais l'exploration ne l'a jamais récupérée
state_note_excluded = exclue de l'exploration ({ $reason })
state_note_external_checked = URL externe — statut vérifié, contenu non audité
state_note_external_not_checked = URL externe — statut non vérifié ({ $reason })
state_note_external_unchecked = URL externe — jamais demandée
state_note_out_of_scope = découverte, mais hors du périmètre de cette exploration
state_note_error = la requête a échoué ({ $detail })
region_unknown = inconnue
inspect_page_title = Page
label_page_title = Titre
label_meta_description = Meta description
label_h1 = H1
label_word_count = Nombre de mots
label_canonical = URL canonique
canonical_self = elle-même
label_indexable = Indexable
indexable_no = non — { $reason }
chars_suffix = { $n ->
        [one] { $n } caractère
       *[other] { $n } caractères
    }
inspect_redirects_title = Chaîne de redirections
inlinks_title = Liens entrants ({ $n })
inlinks_by_region = Par zone : { $breakdown } · { $nofollow } nofollow
inlinks_none = Aucune page explorée ne pointe vers cette URL.
inlinks_sample_heading = Pages qui pointent vers elle, liens de contenu en premier :
no_anchor = (sans texte d'ancre)
inspect_truncated = … { $n } affichées — liste complète : { $cmd }
images_title = Images ({ $n })
image_no_alt = (sans alt)
image_usage_title = Utilisée comme image
# Outbound links header. The three variables are counts.
outlinks_title = Liens sortants ({ $total } : { $internal } internes, { $external } externes)
# Suffix that keeps external URLs apart from the status-code count.
note_external_status = +{ $n ->
        [one] { $n } externe
       *[other] { $n } externes
    }
image_usage_line = intégrée { $times } fois sur { $pages ->
        [one] { $pages } page
       *[other] { $pages } pages
    }

## Crawl comparison (`diff.rs`)

diff_title = Comparaison des explorations
label_before = Avant
label_after = Après
label_site = Site
label_common = En commun
diff_crawl_counts = { $urls } URL, { $findings ->
        [one] { $findings } constat
       *[other] { $findings } constats
    }
diff_common_note = { $n ->
        [one] { $n } URL présente dans les deux explorations
       *[other] { $n } URL présentes dans les deux explorations
    }
warnings_title = Avertissements
tag_inconclusive = NON CONCLUANT
tag_warning = ATTENTION
side_before = avant
side_after = après
warn_truncated = L'exploration « { $side } » est tronquée. Impossible d'affirmer ce qui a disparu ou a été résolu : l'exploration n'est peut-être simplement pas allée jusque-là.
warn_truncated_by = L'exploration « { $side } » est tronquée par { $reason }. Impossible d'affirmer ce qui a disparu ou a été résolu : l'exploration n'est peut-être simplement pas allée jusque-là.
warn_list_mode = L'exploration « { $side } » est en mode liste : elle n'a vu que les URL qui lui ont été données. Impossible d'affirmer ce qui a disparu ou a été résolu.
warn_unfinished = L'exploration « { $side } » ne s'est pas terminée (statut « { $status } »). Ses absences ne veulent rien dire.
warn_rules_changed = Le catalogue de règles a changé ({ $before } → { $after }). Une partie des constats nouveaux ou résolus peut venir des règles, et non du site.
warn_config_changed = La configuration de l'exploration a changé dans : { $fields }. Si vous n'en tenez pas compte, des changements de périmètre seront attribués au site.
warn_scope_changed = Les URL de base diffèrent : { $before } → { $after }. Même site, point de départ différent.
warn_mode_changed = Les modes d'exploration diffèrent : { $before } → { $after }.
warn_missing_table = L'exploration « { $side } » est antérieure à la table `{ $table }` : cette comparaison est ignorée.
warn_order_inverted = L'exploration donnée comme « après » ({ $after_started }) a commencé avant celle donnée comme « avant » ({ $before_started }). Si ce n'est pas voulu, inversez les deux arguments : « Dégradations » et « Améliorations » sortent à l'envers.
worse_title = Dégradations
label_new_findings = Nouveaux constats
more_examples = … et { $n } de plus
label_pages_lost_index = Pages qui ne sont plus indexables
label_status_worse = Codes de statut dégradés
robots_content_changed = { $host } : le contenu a changé
robots_blocks_all = ATTENTION : il bloque désormais l'exploration de tout le site.
nothing_worse = Rien. Aucun nouveau constat, aucune page sortie de l'index.
better_title = Améliorations
label_findings_resolved = Constats résolus
label_pages_indexable_again = Pages de nouveau indexables
label_status_better = Codes de statut améliorés
other_title = Autres changements
label_new_urls = Nouvelles URL
label_urls_gone = URL disparues
label_titles_changed = Titres modifiés
label_meta_changed = Meta descriptions modifiées
label_canonicals_changed = URL canoniques modifiées
label_findings_persisted = Constats toujours présents
suppressed_title = Impossible à affirmer
# Multi-line: the caller prints each line with its indentation.
suppressed_intro = L'une des deux explorations est incomplète : ces différences peuvent venir de ce
    qui n'a jamais été exploré plutôt que du site. Elles ne sont pas comptées comme des changements :
label_candidate_new_urls = Nouvelles URL possibles
label_candidate_urls_gone = URL disparues possibles
label_candidate_new_findings = Nouveaux constats possibles
label_candidate_resolved = Constats résolus possibles
suppressed_advice = Explorez à nouveau les deux côtés avec la même limite pour obtenir une comparaison fiable.
gate_title = Contrôle CI
# Multi-line: the caller prints each line with its indentation.
gate_inconclusive = L'exploration « avant » est incomplète : impossible d'affirmer de nouveaux constats, et
    --fail-on ne se prononce pas. Ce n'est pas un succès.
gate_fail_detail = { $n ->
        [one] { $n } nouveau constat
       *[other] { $n } nouveaux constats
    } [{ $severity }] — correspond à --fail-on { $token }
gate_pass = PASS   rien de ce qui est surveillé n'apparaît comme nouveau constat.

## Reopening a saved diff and listing its rows (`diff_file.rs`)

label_saved = Enregistrée
diff_saved_note = comparaison générée le { $generated_at } par crawlforge { $version } · { $path }
# Multi-line: the caller prints each line with its indentation.
hint_diff_full_lists = Les listes ci-dessus ne montrent que quelques exemples. Pour toutes les lignes, ajoutez --rule <ID> ou
    --change <type> à la même commande (ex. --change url_removed).
warn_unknown_change_types = { $n ->
        [one] { $n } changement d'un type que ce crawlforge ne connaît pas ({ $types }) a été écarté.
       *[other] { $n } changements d'un type que ce crawlforge ne connaît pas ({ $types }) ont été écartés.
    } Le fichier a été écrit par une version plus récente.
label_status_changed = Codes de statut modifiés
diff_no_matching_changes = Aucun changement de cette comparaison ne correspond.
error_diff_missing = { $path } n'existe pas. Les fichiers de comparaison sont enregistrés par `crawlforge diff <before> <after> --out <file>`.
error_diff_needs_two = { $path } est un fichier d'exploration, et une comparaison en demande deux : l'exploration précédente et la suivante. Un fichier seul n'est accepté que s'il s'agit d'une comparaison enregistrée avec --out.
hint_diff_prev = Son exploration précédente se trouve à côté :  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } n'est ni un fichier d'exploration ni un fichier de comparaison. Les fichiers de comparaison sont enregistrés par `crawlforge diff <before> <after> --out <file>`.
error_diff_too_new = { $path } est un fichier de comparaison au format { $version }, plus récent que ce que ce crawlforge comprend ({ $supported }). Mettez crawlforge à jour pour l'ouvrir.
error_diff_reopen_flag = { $flag } s'applique quand on compare deux explorations ; une comparaison enregistrée est seulement lue. Lancez-le sur les explorations : crawlforge diff <before> <after> { $flag } …
error_unknown_change_type = { $value } n'est pas un type de changement. Disponibles : { $available }.
error_rule_with_change = --rule filtre des constats, il ne se combine donc qu'avec --change issue_appeared ou issue_resolved, pas avec { $change }.
site_wide_finding = (constat à l'échelle du site)
site_placeholder = (site)
error_missing_file = { $path } n'existe pas
error_store_missing = { $path } n'existe pas. Les fichiers d'exploration sont produits par `crawlforge crawl`, `crawlforge audit` et `crawlforge list`.
error_not_a_crawl = { $path } ne ressemble pas à un fichier d'exploration CrawlForge
error_different_sites = les deux explorations portent sur des sites différents : { $a } et { $b }.
    Les URL sont appariées par leur texte, donc la comparaison dirait que tout le site a disparu et qu'un autre est apparu. Comparez chaque site avec lui-même.
error_fail_on_unknown = --fail-on : « { $token } » n'est ni une règle ni une gravité.
    Les gravités sont critical, high, medium, low et info ; les ID de règle s'affichent avec `crawlforge rules`.

## Portfolio panel (`portfolio.rs`)

portfolio_title = Tableau de bord du portefeuille
portfolio_range = explorations du { $oldest } au { $newest }
# Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
# changed" would silently mean a different period on every site.
warn_portfolio_date_spread = L'exploration la plus ancienne ({ $oldest }) et la plus récente ({ $newest }) sont séparées de { $days } jours : ce tableau de bord n'est pas un instantané du portefeuille, et « ce qui a changé » couvre une période différente pour chaque site.
# Trap 3.2: two files crawled with different catalogs are not comparable without
# saying so — a rule can be "missing" on a site because it did not exist yet.
warn_portfolio_rules_versions = Les sites n'ont pas tous été explorés avec le même catalogue de règles ({ $versions }). Une règle peut manquer sur un site parce qu'elle n'existait pas lors de son exploration.
warn_portfolio_core_versions = Les explorations proviennent de versions différentes du moteur ({ $versions }).
portfolio_skipped_title = Fichiers écartés
# A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
portfolio_prev_not_input = c'est un .prev.sqlite : l'« avant » de l'exploration voisine, déjà utilisé par la comparaison. Il ne compte pas comme un site.
error_portfolio_newer_schema = son schéma (v{ $version }) est plus récent que ce que cette version comprend (v{ $supported }). Mettez crawlforge à jour pour l'ouvrir.
error_portfolio_no_sites = aucun fichier d'exploration utilisable comme portefeuille n'a été trouvé dans les chemins indiqués. Les fichiers d'exploration sont produits par `crawlforge crawl`, `crawlforge audit` et `crawlforge list` ; un dossier est parcouru à la recherche de fichiers *.sqlite.
error_portfolio_tier = le tableau de bord du portefeuille ne fait pas partie de l'offre gratuite : il nécessite Pro. Les commandes pour un seul site (`report`, `diff`, `export`) restent disponibles.
error_portfolio_too_many = le portefeuille compte { $n } sites et cette offre le limite à { $max }.
portfolio_changes_title = Ce qui a changé
portfolio_no_pairs = Aucun site n'a d'exploration précédente à côté de lui. Explorez à nouveau vers le même fichier de sortie et l'exploration précédente est conservée comme .prev.sqlite : c'est l'« avant » auquel cette section se compare.
portfolio_new_critical_high = Nouveaux constats de gravité critique ou élevée
portfolio_none_critical_high = Aucun nouveau constat critique ou de gravité élevée sur les sites comparés.
portfolio_rest_title = Le reste, site par site
portfolio_no_changes = aucun changement
# The diff of that pair cannot assert absences: one side is incomplete.
portfolio_pair_inconclusive = l'un des deux côtés est incomplet : impossible d'affirmer ce qui a disparu ou a été résolu
portfolio_pair_failed = sa comparaison avec l'exploration précédente a échoué : { $reason }
portfolio_spread_title = Problèmes communs au portefeuille
portfolio_spread_intro = Une règle qui se déclenche sur la plupart des sites relève rarement du contenu : c'est généralement un modèle ou une extension partagés — une seule correction les règle tous.
portfolio_spread_none = Aucune règle ne se déclenche sur aucun site du portefeuille.
portfolio_glance_title = Le portefeuille en un coup d'œil
th_site = site
th_crawled = exploré
th_indexable = index.
th_sev_critical = crit.
th_sev_high = élev.
th_sev_medium = moy.
th_sev_low = faible
th_sev_info = info
flag_truncated = (tronquée)
flag_list_mode = (mode liste)
flag_unfinished = (inachevée : { $status })
hint_site_diff = Comparaison complète d'un site :
portfolio_html_title = Tableau de bord du portefeuille
portfolio_sites_count = { $n ->
        [one] { $n } site
       *[other] { $n } sites
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = enregistrée dans le fichier d'exploration
rules_config_source_after = enregistrée dans l'exploration la plus récente
rules_config_source_file = depuis { $path }
warn_rules_config_differs = Les deux explorations enregistrent des configurations de règles différentes. Les deux côtés ont été évalués avec la même ({ $source }), afin qu'un changement de configuration n'apparaisse pas comme des constats nouveaux ou résolus.
warn_portfolio_rules_config = Certains sites sont lus avec leur propre configuration de règles (règles désactivées / gravités modifiées) : { $sites }. Leurs totaux ne sont pas comparables règle par règle avec les autres.
# "Rule configuration (stored in the crawl file): 2 rules disabled, 1 severity changed."
rules_config_note = Configuration des règles ({ $source }) : { $disabled ->
        [one] { $disabled } règle désactivée
       *[other] { $disabled } règles désactivées
    }, { $changed ->
        [one] { $changed } gravité modifiée
       *[other] { $changed } gravités modifiées
    }. Le fichier d'exploration conserve tous les constats.
# "9 sites sur 12": in French the noun follows $fired, not $total.
portfolio_sites_of = { $fired ->
        [one] { $fired } site
       *[other] { $fired } sites
    } sur { $total }
# " (2 inconclusive)": sites where the rule could not be evaluated. Starts with a space.
# Never shown for zero.
portfolio_inconclusive_suffix = {" "}({ $n ->
        [one] { $n } non concluant
       *[other] { $n } non concluants
    })
# "1 site sur 5 a" / "3 sites sur 5 ont": noun and verb follow $pairs.
portfolio_pairs_line = { $pairs ->
        [one] { $pairs } site sur { $total } a
       *[other] { $pairs } sites sur { $total } ont
    } une exploration précédente (.prev.sqlite) avec laquelle se comparer.

## Telling files apart (`store_check.rs`)

error_store_is_diff = { $path } n'est pas un fichier d'exploration : c'est un fichier de comparaison, du type qu'enregistre `crawlforge diff --out`. Ouvrez-le avec `crawlforge diff { $path }`.
    Les fichiers d'exploration sont produits par `crawlforge crawl` et `crawlforge audit`.
error_store_foreign = { $path } n'est pas un fichier d'exploration CrawlForge : il n'en a pas les tables. Viendrait-il d'un autre programme ?
    Les fichiers d'exploration sont produits par `crawlforge crawl` et `crawlforge audit`.
error_store_not_sqlite = { $path } n'est pas un fichier CrawlForge : ce n'est même pas une base de données SQLite.
    Les fichiers d'exploration sont produits par `crawlforge crawl` et `crawlforge audit`.

## Markdown/HTML report (`audit_report.rs`)

report_audit_of = Audit de
report_audit_of_sites = Audit de { $n } sites : { $sites }
sites_and_more = { $sites } et { $n } de plus
report_mode_line = Mode `{ $mode }` · lancé le { $started }
report_truncated_note = > **Exploration tronquée** (`{ $reason }`). Les totaux couvrent ce qui a été exploré jusqu'à ce point, pas le site entier.
report_list_mode_note = > **Exploration en mode liste.** Seules les URL listées ont été auditées : les totaux décrivent cet ensemble, pas le site entier, et les règles qui ont besoin du graphe de liens complet du site ne sont pas évaluées.
th_metric = Indicateur
th_value = Valeur
row_pages = Pages
row_indexable = Indexables
row_errors_4xx_5xx = 4xx/5xx
report_more = … { $n } de plus
report_more_run = … { $n } de plus — lancez : { $cmd }
report_full_list_run = … liste complète : { $cmd }
html_title = Audit SEO

## Rule catalogue (`rules.rs`)

th_severity = GRAVITÉ
th_category = CATÉGORIE
th_scope = PORTÉE
th_tier = OFFRE
th_name = NOM
lbl_category = catégorie
lbl_scope = portée
lbl_tier = offre
lbl_reference = référence
scope_page = page
scope_site = site
rules_summary = { $total } règles : { $free } dans l'offre gratuite, { $page } au niveau de la page et { $site } à l'échelle du site.
error_unknown_category = aucune règle dans la catégorie « { $category } ». Les catégories sont : { $list }.

## `main.rs`

crawling = Exploration de { $target }
error_invalid_seed = { $url } n'est pas une URL explorable : une adresse comme https://example.com/ est attendue. Rien n'a été créé.
file_line = Fichier :   { $path }
previous_kept = Précédent : { $path } (conservé, de la dernière exécution)
report_written = Rapport écrit dans { $path }
markdown_report_written = Rapport Markdown écrit dans { $path }
exported_csv = { $n ->
        [one] { $n } fichier CSV exporté dans { $dir }
       *[other] { $n } fichiers CSV exportés dans { $dir }
    }
exported_sheets = { $n ->
        [one] { $n } feuille exportée dans { $path }
       *[other] { $n } feuilles exportées dans { $path }
    }
bench_appended = Mesure de performance ajoutée à { $path }
next_steps_title = Étapes suivantes
next_explain = Expliquer chaque constat, avec les URL concernées :
next_spreadsheet = Ouvrir toutes les données dans un tableur :
next_compare_previous = Comparer avec l'exécution précédente de cette exploration :
next_compare_later = Relancer cette même exploration après votre prochain déploiement, puis comparer les deux :
next_reread = Relire ce résumé à tout moment :

## HTTP Basic authentication (protected staging)

auth_url_note = Authentification HTTP Basic prise dans l'URL : elle ne sera envoyée qu'à { $host } et ne sera pas enregistrée dans le fichier d'exploration.
auth_env_note = Authentification HTTP Basic prise dans CRAWLFORGE_AUTH : elle ne sera envoyée qu'à { $host } et ne sera pas enregistrée dans le fichier d'exploration.
auth_base_ignored = Attention : les identifiants de --base ont été retirés : l'audit d'un dossier lit le disque et ne fait aucune requête HTTP.
auth_list_ignored = Attention : les identifiants inclus dans les URL de la liste sont ignorés et retirés. Pour authentifier l'exploration, définissez CRAWLFORGE_AUTH=user:password ; ils ne s'appliquent qu'à l'hôte de la première URL.
progress_sitemaps = recherche des sitemaps…
progress_finalize = passe finale : liens entrants et règles à l'échelle du site…
progress_crawl = { $fetched } explorées · { $queued } en file · { $rate } URL/s · { $findings } constats
progress_failed_suffix = {" "}· { $n } en échec

## Resuming an interrupted crawl (`main.rs`, `report.rs`)

resuming = Reprise de l'exploration de { $target }
resume_counts = Reprise là où elle s'était arrêtée : { $done } URL déjà explorées, { $pending } encore en attente.
resume_config_note = Configuration enregistrée dans le fichier utilisée : les paramètres de l'exécution d'origine s'appliquent.
error_resume_schema_blocking = { $file } est une exploration au schéma v{ $version }, et la migration { $blocking } change ce que le moteur écrit : une moitié dirait une chose et l'autre moitié une autre. Relancez l'exploration. Vous pouvez toujours ouvrir ce fichier avec `report`, `export` et `diff`.
resume_robots_not_inherited = Remarque : l'exploration d'origine ignorait robots.txt. Celle-ci non — cette autorisation, c'est vous qui la donnez, pas un fichier. Relancez la commande d'exploration d'origine si vous en avez besoin.
resume_finished = Exploration reprise et terminée : { $urls } URL de plus en { $secs } s
interrupt_flushing = Interrompu : enregistrement de ce qui a déjà été exploré… (Ctrl+C à nouveau pour quitter sans attendre)
interrupt_after_done = Interrompu. Le fichier d'exploration est complet ; un export à moitié écrit peut rester sur le disque.
warn_wal_kept = Remarque : un autre programme a { $path } ouvert, il reste donc en mode WAL. Les fichiers -wal et -shm à côté font partie de cette exploration : copiez les trois ensemble, ou fermez l'autre programme et lancez `crawlforge report` dessus pour les réintégrer.
error_store_locked = un autre processus crawlforge écrit dans { $path } en ce moment. Attendez qu'il termine, ou arrêtez-le, avant d'explorer ou de reprendre ce fichier.
interrupted_saved = Exploration interrompue. Tout ce qui a été exploré jusqu'ici est enregistré dans { $path }. Pour la poursuivre :
hint_previous_unfinished = L'exploration précédente n'était pas terminée. Pour la poursuivre au lieu de recommencer :
error_resume_done = { $path } est une exploration terminée : il n'y a rien à reprendre.
    Pour auditer à nouveau le site, lancez la commande d'exploration d'origine ; le fichier terminé sera conservé à côté comme .prev.sqlite.
error_resume_status = { $path } ne peut pas être reprise : son statut est « { $status } ». Seules les explorations interrompues (statut « running » ou « paused ») peuvent être reprises.
error_resume_schema = { $path } utilise le schéma v{ $found } et cette version écrit v{ $expected } : une exploration interrompue ne peut être reprise que par la version qui l'a lancée. Relancez l'exploration.
error_resume_config = la configuration d'exploration enregistrée dans { $path } est illisible. Le fichier a-t-il été écrit par une autre version ? Relancez l'exploration.
