### CrawlForge — command-line messages, Brazilian Portuguese (pt-BR).
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules.
###
### Glossary: crawl = rastreamento (verb: rastrear), finding = ocorrência, rule = regra,
### severity = gravidade, site = site, page = página, link = link, canonical = canônica,
### template = template, portfolio = portfólio, tier = plano, scope = escopo.
###
### Plurals: CLDR puts 0 in [one] for Portuguese ("0 site"). Where a zero can really show up,
### a [0] variant keeps it plural.

## Severity names

severity_label_critical = Crítica
severity_label_high = Alta
severity_label_medium = Média
severity_label_low = Baixa
severity_label_info = Info
severity_word_critical = crítica
severity_word_high = alta
severity_word_medium = média
severity_word_low = baixa
severity_word_info = info

## Resumen del rastreo (`report.rs`)

crawl_finished = Rastreamento concluído: { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URLs
    } em { $secs } s
crawl_failed_suffix = {" "}({ $n } com falha)
crawl_truncated = Rastreamento truncado por { $reason }. As ocorrências exibidas são as encontradas até esse ponto.
crawl_rate_limited_note = O site limitou a taxa do rastreamento: { $n ->
        [one] { $n } página ainda respondia 429 Too Many Requests após as novas tentativas e ficou pendente, sem ser considerada quebrada
       *[other] { $n } páginas ainda respondiam 429 Too Many Requests após as novas tentativas e ficaram pendentes, sem serem consideradas quebradas
    }. As regras que precisam do site inteiro só são avaliadas depois que essas páginas forem buscadas. Mais tarde, execute:
    {"    "}crawlforge resume { $path }
    ou rastreie de novo com um --concurrency menor, ou libere o user-agent do CrawlForge no firewall do site.
report_rate_limited_note = > **O site limitou a taxa do rastreamento.** { $n ->
        [one] { $n } página respondeu 429 Too Many Requests e ficou pendente: não é contada nem considerada quebrada
       *[other] { $n } páginas responderam 429 Too Many Requests e ficaram pendentes: não são contadas nem consideradas quebradas
    }, e as regras que precisam do site inteiro não são avaliadas. `crawlforge resume` as busca.
crawl_list_mode_note = Rastreamento de lista: só as URLs informadas foram auditadas. As regras que precisam do grafo completo de links do site (páginas órfãs, links de entrada, profundidade) não são avaliadas.
external_unchecked = { $n ->
        [one] { $n } link externo não foi verificado
       *[other] { $n } links externos não foram verificados
    }: o limite max_external foi atingido. Aumente-o com --max-external.
external_unregistered = { $n ->
        [one] { $n } link externo nem chegou a ser registrado
       *[other] { $n } links externos nem chegaram a ser registrados
    }: o limite max_external_urls foi atingido. Esse limite existe justamente para o caso de uma única página com links para centenas de milhares de hosts.
external_never_checked = { $n ->
        [one] { $n } link externo foi registrado, mas nunca verificado.
       *[other] { $n } links externos foram registrados, mas nunca verificados.
    }
external_offline = { $n ->
        [one] { $n } link externo não foi verificado
       *[other] { $n } links externos não foram verificados
    }: auditar uma pasta não faz requisições de rede. Para verificar os links externos, rastreie o site publicado com `crawlforge crawl <URL>`.
external_check_disabled = Os links externos não foram verificados (--no-external-check): a ausência de links externos quebrados neste relatório não significa que eles não existam.
external_checked_note = { $n ->
        [one] { $n } link externo verificado.
       *[other] { $n } links externos verificados.
    }
rules_not_evaluated = As regras que precisam do grafo completo de links não foram avaliadas: { $rules }.
results_title = Resultados
label_urls = URLs
label_html_pages = Páginas HTML
label_links = Links
note_internal = { $n ->
        [one] { $n } interna
       *[other] { $n } internas
    }
note_indexable = { $n ->
        [one] { $n } indexável
       *[other] { $n } indexáveis
    }
heading_status_codes = Códigos de status
label_no_response = sem resposta
note_request_failed = a requisição falhou
label_never_requested = Nunca solicitadas
note_never_requested = (descobertas, mas o rastreamento parou antes de buscá-las)
label_out_of_scope = Fora do escopo
note_out_of_scope = (com links a partir de páginas auditadas, mas fora do escopo deste rastreamento)
heading_non_indexable = Não indexáveis, por quê
heading_findings = Ocorrências
no_findings = Nenhuma ocorrência.
heading_excluded = Excluídas
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = { $k ->
        [one] { $k } problema de template
       *[other] { $k } problemas de template
    } ({ $pages ->
        [one] { $pages } página
       *[other] { $pages } páginas
    })
plus_more_findings = {" "}+ { $n ->
        [one] { $n } outra ocorrência
       *[other] { $n } outras ocorrências
    }
example_urls = ex.: { $urls }

## Reformulación de reglas dominantes

pervasive_note = { $pct }% do site
deep_pages_summary = { $pages ->
        [one] { $pages } página
       *[other] { $pages } páginas
    } a mais de { $max } cliques — { $pct }% do site (profundidade típica { $low }–{ $high }, máxima { $deepest })
rule_deep_sorted = Ordenadas por profundidade de clique, da mais profunda para a menos profunda; a primeira coluna é o número de cliques desde a página inicial.
hint_full_lists = Lista completa de URLs de uma regra: { $cmd }

## Listado de una regla (`report --rule`)

rule_affected_urls = { $n ->
        [one] { $n } URL afetada
       *[other] { $n } URLs afetadas
    }
rule_template_group = Grupo de template — { $n ->
        [one] { $n } página
       *[other] { $n } páginas
    }
rule_group_cause = causa: { $detail }
rule_other_pages = Outras páginas afetadas — { $n }
rule_no_findings = Nenhuma ocorrência de { $rule } neste rastreamento.
hint_who_links = Um link quebrado se corrige na página que tem o link. Quem aponta para uma URL, com o texto âncora:
error_unknown_rule = { $rule } não é um ID de regra. O catálogo é listado por `crawlforge rules`.
error_all_blocked_by_robots = o robots.txt bloqueia todas as URLs: não há nada para auditar. Se o site for seu, tente de novo com --ignore-robots.
error_all_excluded_by_pattern = todas as URLs foram descartadas pelos padrões de --exclude/--include: não há nada para auditar. Revise os padrões.
error_no_urls_fetched = o rastreamento não obteve nenhuma URL. Verifique o destino.
error_dead_seed = não foi possível conectar a { $url } ({ $cause }). Verifique a URL.
cause_dns = o domínio não existe ou não resolve
cause_tls = o certificado TLS não é válido
cause_timeout = o servidor não respondeu a tempo
cause_connection = conexão recusada
cause_no_response = sem resposta
warn_base_mismatch = Aviso: { $n } de { $total } canônicas apontam para { $origin }, não para a base informada ({ $base }).
    Os resultados de indexabilidade dependem desse parâmetro: se o site é publicado em
    { $origin }, repita a auditoria com --base { $origin }

## Ficha de una URL (`inspect.rs`)

inspect_error_not_found = { $url } não está neste rastreamento.
inspect_closest = As URLs mais parecidas do arquivo são:
inspect_status_title = Status
label_http_status = Status HTTP
label_content_type = Tipo de conteúdo
label_response_time = Tempo de resposta
label_depth = Profundidade
note_depth_clicks = cliques desde a URL inicial
label_in_sitemap = No sitemap
word_yes = sim
word_no = não
label_crawl_state = Estado do rastreamento
state_note_pending = descoberta, mas o rastreamento nunca a buscou
state_note_excluded = excluída do rastreamento ({ $reason })
state_note_external_checked = URL externa — status verificado, conteúdo não auditado
state_note_external_not_checked = URL externa — status não verificado ({ $reason })
state_note_external_unchecked = URL externa — nunca solicitada
state_note_out_of_scope = descoberta, mas fora do escopo deste rastreamento
state_note_error = a requisição falhou ({ $detail })
region_unknown = desconhecida
inspect_page_title = Página
label_page_title = Título
label_meta_description = Meta description
label_h1 = H1
label_word_count = Palavras
label_canonical = Canônica
canonical_self = a própria página
label_indexable = Indexável
indexable_no = não — { $reason }
chars_suffix = { $n ->
        [one] { $n } caractere
       *[other] { $n } caracteres
    }
inspect_redirects_title = Cadeia de redirecionamentos
inlinks_title = Links de entrada ({ $n })
inlinks_by_region = Por região: { $breakdown } · { $nofollow } nofollow
inlinks_none = Nenhuma página rastreada tem link para esta URL.
inlinks_sample_heading = Páginas com links para ela, primeiro os links de conteúdo:
no_anchor = (sem texto âncora)
inspect_truncated = … exibindo { $n } — lista completa: { $cmd }
images_title = Imagens ({ $n })
image_no_alt = (sem alt)
image_usage_title = Usada como imagem
# Outbound links header. The three variables are counts.
outlinks_title = Links de saída ({ $total }: { $internal ->
        [0] 0 internos
        [one] { $internal } interno
       *[other] { $internal } internos
    }, { $external ->
        [0] 0 externos
        [one] { $external } externo
       *[other] { $external } externos
    })
# Suffix that keeps external URLs apart from the status-code count.
note_external_status = +{ $n ->
        [one] { $n } externa
       *[other] { $n } externas
    }
image_usage_line = incorporada { $times ->
        [one] { $times } vez
       *[other] { $times } vezes
    } em { $pages ->
        [one] { $pages } página
       *[other] { $pages } páginas
    }

## Comparación de rastreos (`diff.rs`)

diff_title = Comparação de rastreamentos
label_before = Antes
label_after = Depois
label_site = Site
label_common = Em comum
# $urls and $findings arrive right-padded as text: no selector can work on them.
diff_crawl_counts = { $urls ->
        [0] { $urls } URLs
        [one] { $urls } URL
       *[other] { $urls } URLs
    }, { $findings ->
        [0] { $findings } ocorrências
        [one] { $findings } ocorrência
       *[other] { $findings } ocorrências
    }
diff_common_note = { $n ->
        [0] nenhuma URL presente nos dois rastreamentos
        [one] { $n } URL presente nos dois rastreamentos
       *[other] { $n } URLs presentes nos dois rastreamentos
    }
warnings_title = Avisos
tag_inconclusive = INCONCLUSIVO
tag_warning = AVISO
side_before = antes
side_after = depois
warn_truncated = O rastreamento “{ $side }” está truncado. Não é possível afirmar o que desapareceu ou foi resolvido: talvez o rastreamento simplesmente não tenha chegado lá.
warn_truncated_by = O rastreamento “{ $side }” está truncado por { $reason }. Não é possível afirmar o que desapareceu ou foi resolvido: talvez o rastreamento simplesmente não tenha chegado lá.
warn_list_mode = O rastreamento “{ $side }” é um rastreamento de lista: só viu as URLs que recebeu. Não é possível afirmar o que desapareceu ou foi resolvido.
warn_unfinished = O rastreamento “{ $side }” não terminou (status “{ $status }”). As ausências nele não significam nada.
warn_rules_changed = O catálogo de regras mudou ({ $before } → { $after }). Parte das ocorrências novas ou resolvidas pode vir das regras, não do site.
warn_config_changed = A configuração do rastreamento mudou em: { $fields }. Sem levar isso em conta, mudanças de escopo acabam atribuídas ao site.
warn_scope_changed = As URLs base diferem: { $before } → { $after }. Mesmo site, ponto de partida diferente.
warn_mode_changed = Os modos de rastreamento diferem: { $before } → { $after }.
warn_missing_table = O rastreamento “{ $side }” é anterior à tabela `{ $table }`: essa comparação foi omitida.
warn_order_inverted = O rastreamento passado como “depois” ({ $after_started }) começou antes do passado como “antes” ({ $before_started }). Se não for proposital, inverta os dois argumentos: “Piorou” e “Melhorou” saem trocados.
worse_title = Piorou
label_new_findings = Ocorrências novas
more_examples = … e mais { $n }
label_pages_lost_index = Páginas que deixaram de ser indexáveis
label_status_worse = Códigos de status que pioraram
robots_content_changed = { $host }: o conteúdo mudou
robots_blocks_all = ATENÇÃO: agora bloqueia o rastreamento do site inteiro.
nothing_worse = Nada. Nenhuma ocorrência nova, nenhuma página saiu do índice.
better_title = Melhorou
label_findings_resolved = Ocorrências resolvidas
label_pages_indexable_again = Páginas indexáveis novamente
label_status_better = Códigos de status que melhoraram
other_title = Outras mudanças
label_new_urls = URLs novas
label_urls_gone = URLs desaparecidas
label_titles_changed = Títulos alterados
label_meta_changed = Meta descriptions alteradas
label_canonicals_changed = Canônicas alteradas
label_findings_persisted = Ocorrências que persistem
suppressed_title = Não é possível afirmar
suppressed_intro = Um dos dois rastreamentos está incompleto, então estas diferenças podem vir
    do que nunca foi rastreado, e não do site. Elas não são contadas como mudanças:
label_candidate_new_urls = Possíveis URLs novas
label_candidate_urls_gone = Possíveis URLs desaparecidas
label_candidate_new_findings = Possíveis ocorrências novas
label_candidate_resolved = Possíveis ocorrências resolvidas
suppressed_advice = Rastreie os dois lados de novo com o mesmo limite para obter uma comparação conclusiva.
gate_title = Gate de CI
gate_inconclusive = O rastreamento “antes” está incompleto: não é possível afirmar ocorrências novas, e
    o --fail-on não se pronuncia. Isto não é uma aprovação.
gate_fail_detail = { $n ->
        [one] { $n } ocorrência nova
       *[other] { $n } ocorrências novas
    } [{ $severity }] — corresponde a --fail-on { $token }
gate_pass = PASS   nada do que é monitorado aparece como ocorrência nova.

## Reabrir un diff guardado y listar sus filas (`diff_file.rs`)

label_saved = Salvo
diff_saved_note = diff gerado em { $generated_at } pelo crawlforge { $version } · { $path }
hint_diff_full_lists = As listas acima mostram só alguns exemplos. Para ver todas as linhas, adicione --rule <ID> ou
    --change <tipo> ao mesmo comando (ex.: --change url_removed).
warn_unknown_change_types = { $n ->
        [one] { $n } mudança de um tipo que este crawlforge não conhece ({ $types }) ficou de fora
       *[other] { $n } mudanças de um tipo que este crawlforge não conhece ({ $types }) ficaram de fora
    }. O arquivo foi gravado por uma versão mais nova.
label_status_changed = Códigos de status alterados
diff_no_matching_changes = Nenhuma mudança deste diff corresponde.
error_diff_missing = { $path } não existe. Os arquivos de diff são salvos por `crawlforge diff <antes> <depois> --out <arquivo>`.
error_diff_needs_two = { $path } é um arquivo de rastreamento, e a comparação precisa de dois: o rastreamento anterior e o posterior. Um único arquivo só é aceito se for um diff salvo com --out.
hint_diff_prev = O rastreamento anterior está ao lado:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } não é nem um arquivo de rastreamento nem um arquivo de diff. Os arquivos de diff são salvos por `crawlforge diff <antes> <depois> --out <arquivo>`.
error_diff_too_new = { $path } é um arquivo de diff no formato { $version }, mais novo do que este crawlforge entende ({ $supported }). Atualize o crawlforge para abri-lo.
error_diff_reopen_flag = { $flag } se aplica ao comparar dois rastreamentos; um diff salvo é apenas lido. Execute-o sobre os rastreamentos: crawlforge diff <antes> <depois> { $flag } …
error_unknown_change_type = { $value } não é um tipo de mudança. Disponíveis: { $available }.
error_rule_with_change = --rule busca ocorrências, então só se combina com --change issue_appeared ou issue_resolved, não com { $change }.
site_wide_finding = (ocorrência do site inteiro)
site_placeholder = (site)
error_missing_file = { $path } não existe
error_store_missing = { $path } não existe. Os arquivos de rastreamento são gerados por `crawlforge crawl`, `crawlforge audit` e `crawlforge list`.
error_not_a_crawl = { $path } não parece um arquivo de rastreamento do CrawlForge
error_different_sites = os dois rastreamentos são de sites diferentes: { $a } e { $b }.
    As URLs são pareadas pelo texto, então a comparação diria que o site inteiro desapareceu e que outro apareceu. Compare cada site com ele mesmo.
error_fail_on_unknown = --fail-on: “{ $token }” não é uma regra nem uma gravidade.
    As gravidades são critical, high, medium, low e info; os IDs de regra são listados por `crawlforge rules`.

## Panel de cartera (`portfolio.rs`)

portfolio_title = Painel do portfólio
portfolio_range = rastreamentos de { $oldest } a { $newest }
warn_portfolio_date_spread = O rastreamento mais antigo ({ $oldest }) e o mais recente ({ $newest }) estão separados por { $days ->
        [one] { $days } dia
       *[other] { $days } dias
    }: este painel não é um retrato do portfólio, e “o que mudou” cobre um período diferente em cada site.
warn_portfolio_rules_versions = Nem todos os sites foram rastreados com o mesmo catálogo de regras ({ $versions }). Uma regra pode faltar em um site porque ainda não existia quando ele foi rastreado.
warn_portfolio_core_versions = Os rastreamentos vêm de versões diferentes do motor ({ $versions }).
portfolio_skipped_title = Arquivos deixados de lado
portfolio_prev_not_input = é um .prev.sqlite: o “antes” do rastreamento ao lado, já usado pela comparação. Não conta como site.
error_portfolio_newer_schema = o esquema dele (v{ $version }) é mais novo do que esta versão entende (v{ $supported }). Atualize o crawlforge para abri-lo.
error_portfolio_no_sites = nenhum arquivo de rastreamento utilizável como portfólio foi encontrado nos caminhos informados. Os arquivos de rastreamento são gerados por `crawlforge crawl`, `crawlforge audit` e `crawlforge list`; em um diretório, são procurados arquivos *.sqlite.
error_portfolio_tier = o painel do portfólio não faz parte do plano gratuito: requer o Pro. Os comandos de um único site (`report`, `diff`, `export`) continuam disponíveis.
error_portfolio_too_many = o portfólio tem { $n } sites e este plano permite no máximo { $max }.
portfolio_changes_title = O que mudou
portfolio_no_pairs = Nenhum site tem um rastreamento anterior ao lado. Rastreie de novo para o mesmo arquivo de saída e o rastreamento anterior é mantido como .prev.sqlite — esse é o “antes” com que esta seção compara.
portfolio_new_critical_high = Ocorrências novas de gravidade crítica e alta
portfolio_none_critical_high = Nenhuma ocorrência nova de gravidade crítica ou alta nos sites comparados.
portfolio_rest_title = O restante, site a site
portfolio_no_changes = sem mudanças
portfolio_pair_inconclusive = um dos lados está incompleto: não é possível afirmar o que desapareceu ou foi resolvido
portfolio_pair_failed = a comparação com o rastreamento anterior falhou: { $reason }
portfolio_spread_title = Falhas em todo o portfólio
portfolio_spread_intro = Uma regra que dispara na maioria dos sites raramente é questão de conteúdo: costuma ser um template ou plugin compartilhado — uma correção que serve para todos.
portfolio_spread_none = Nenhuma regra dispara em nenhum site do portfólio.
portfolio_glance_title = Visão geral do portfólio
th_site = site
th_crawled = rastreado
th_indexable = index.
flag_truncated = (truncado)
flag_list_mode = (rastreamento de lista)
flag_unfinished = (não terminou: { $status })
hint_site_diff = Comparação completa de um site:
portfolio_html_title = Painel do portfólio
portfolio_sites_count = { $n ->
        [one] { $n } site
       *[other] { $n } sites
    }

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = salva no arquivo de rastreamento
rules_config_source_after = salva no rastreamento posterior
rules_config_source_file = de { $path }
warn_rules_config_differs = Os dois rastreamentos guardam configurações de regras diferentes. Os dois lados foram avaliados com a mesma ({ $source }), então uma mudança de configuração não aparece como ocorrências novas ou resolvidas.
warn_portfolio_rules_config = Alguns sites são lidos com a própria configuração de regras (regras desativadas / gravidades alteradas): { $sites }. As contagens deles não são comparáveis regra a regra com as dos demais.
rules_config_note = Configuração de regras ({ $source }): { $disabled ->
        [0] nenhuma regra desativada
        [one] { $disabled } regra desativada
       *[other] { $disabled } regras desativadas
    }, { $changed ->
        [0] nenhuma gravidade alterada
        [one] { $changed } gravidade alterada
       *[other] { $changed } gravidades alteradas
    }. O arquivo de rastreamento mantém todas as ocorrências.
portfolio_sites_of = { $fired } de { $total ->
        [one] { $total } site
       *[other] { $total } sites
    }
portfolio_inconclusive_suffix = {" "}({ $n ->
        [one] { $n } inconclusivo
       *[other] { $n } inconclusivos
    })
portfolio_pairs_line = { $pairs } de { $total ->
        [one] { $total } site
       *[other] { $total } sites
    } { $pairs ->
        [one] tem
       *[other] têm
    } um rastreamento anterior (.prev.sqlite) para comparar.

## Identificación de ficheros (`store_check.rs`)

error_store_is_diff = { $path } não é um arquivo de rastreamento: é um arquivo de diff, do tipo que `crawlforge diff --out` salva. Abra-o com `crawlforge diff { $path }`.
    Os arquivos de rastreamento são gerados por `crawlforge crawl` e `crawlforge audit`.
error_store_foreign = { $path } não é um arquivo de rastreamento do CrawlForge: não tem as tabelas dele. Será de outro programa?
    Os arquivos de rastreamento são gerados por `crawlforge crawl` e `crawlforge audit`.
error_store_not_sqlite = { $path } não é um arquivo do CrawlForge: nem sequer é um banco de dados SQLite.
    Os arquivos de rastreamento são gerados por `crawlforge crawl` e `crawlforge audit`.

## Informe Markdown/HTML (`audit_report.rs`)

report_audit_of = Auditoria de
report_audit_of_sites = Auditoria de { $n } sites: { $sites }
sites_and_more = { $sites } e mais { $n }
report_mode_line = Modo `{ $mode }` · iniciada em { $started }
report_truncated_note = > **Rastreamento truncado** (`{ $reason }`). As contagens cobrem o que foi rastreado até esse ponto, não o site inteiro.
report_list_mode_note = > **Rastreamento de lista.** Só as URLs listadas foram auditadas: as contagens descrevem esse conjunto, não o site inteiro, e as regras que precisam do grafo completo de links do site não são avaliadas.
th_metric = Métrica
th_value = Valor
row_pages = Páginas
row_indexable = Indexáveis
row_errors_4xx_5xx = 4xx/5xx
report_more = … mais { $n }
report_more_run = … mais { $n } — execute: { $cmd }
report_full_list_run = … lista completa: { $cmd }
html_title = Auditoria de SEO

## Catálogo de reglas (`rules.rs`)

th_severity = GRAVIDADE
th_category = CATEGORIA
th_scope = ESCOPO
th_tier = PLANO
th_name = NOME
lbl_category = categoria
lbl_scope = escopo
lbl_tier = plano
lbl_reference = referência
scope_page = página
scope_site = site
rules_summary = { $total } regras: { $free } no plano gratuito, { $page } de página e { $site } do site inteiro.
error_unknown_category = nenhuma regra na categoria “{ $category }”. As categorias são: { $list }.

## Cadenas de `main.rs`, listas para cablear

crawling = Rastreando { $target }
error_invalid_seed = { $url } não é uma URL rastreável: o esperado é algo como https://example.com/. Nada foi criado.
file_line = Arquivo:  { $path }
previous_kept = Anterior: { $path } (mantido, da última execução)
report_written = Relatório gravado em { $path }
markdown_report_written = Relatório em Markdown gravado em { $path }
exported_csv = { $n ->
        [one] { $n } arquivo CSV exportado
       *[other] { $n } arquivos CSV exportados
    } para { $dir }
exported_sheets = { $n ->
        [one] { $n } aba exportada
       *[other] { $n } abas exportadas
    } para { $path }
bench_appended = Registro de benchmark adicionado a { $path }
next_steps_title = Próximos passos
next_explain = Explicar cada ocorrência, com as URLs afetadas:
next_spreadsheet = Abrir os dados completos como planilha:
next_compare_previous = Comparar com a execução anterior deste rastreamento:
next_compare_later = Repita este mesmo rastreamento depois do próximo deploy e compare os dois:
next_reread = Reler este resumo a qualquer momento:

## Autenticación básica HTTP (staging protegido)

auth_url_note = Usando autenticação HTTP Basic da URL: ela será enviada apenas para { $host } e não será armazenada no arquivo de rastreamento.
auth_env_note = Usando autenticação HTTP Basic de CRAWLFORGE_AUTH: ela será enviada apenas para { $host } e não será armazenada no arquivo de rastreamento.
auth_base_ignored = Aviso: as credenciais em --base foram removidas: uma auditoria de diretório lê do disco e não faz requisições HTTP.
auth_list_ignored = Aviso: credenciais dentro das URLs da lista são ignoradas e removidas. Para autenticar o rastreamento, defina CRAWLFORGE_AUTH=usuario:senha; isso se aplica apenas ao host da primeira URL.
progress_sitemaps = descobrindo sitemaps…
progress_finalize = passada final: links de entrada e regras do site inteiro…
progress_crawl = { $fetched } rastreadas · { $queued } na fila · { $rate } URL/s · { $findings } ocorrências
progress_failed_suffix = {" "}· { $n } com falha

## Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`)

resuming = Retomando o rastreamento de { $target }
resume_counts = Continuando de onde parou: { $done } URLs já rastreadas, { $pending } ainda pendentes.
resume_config_note = Usando a configuração salva no arquivo: valem as configurações da execução original.
error_resume_schema_blocking = { $file } é um rastreamento do esquema v{ $version }, e a migração { $blocking } muda o que o motor grava: metade diria uma coisa e metade outra. Rastreie de novo. Este arquivo ainda pode ser aberto com `report`, `export` e `diff`.
resume_robots_not_inherited = Observação: o rastreamento original ignorou o robots.txt. Este não ignora — essa permissão é concedida por você, não por um arquivo. Execute de novo o comando do rastreamento original se precisar dela.
resume_finished = Rastreamento retomado e concluído: mais { $urls ->
        [one] { $urls } URL
       *[other] { $urls } URLs
    } em { $secs } s
interrupt_flushing = Interrompido: salvando o que já foi rastreado… (Ctrl+C de novo para sair sem esperar)
interrupt_after_done = Interrompido. O arquivo de rastreamento está completo; pode ter ficado uma exportação pela metade.
warn_wal_kept = Observação: outro programa está com { $path } aberto, então ele continua em modo WAL. Os arquivos -wal e -shm ao lado fazem parte deste rastreamento: copie os três juntos, ou feche o outro programa e execute `crawlforge report` sobre ele para reincorporá-los.
error_store_locked = outro processo do crawlforge está gravando { $path } neste momento. Espere ele terminar, ou interrompa-o, antes de rastrear ou retomar este arquivo.
interrupted_saved = Rastreamento interrompido. Tudo o que foi rastreado até agora está salvo em { $path }. Continue com:
hint_previous_unfinished = O rastreamento anterior não tinha terminado. Para continuá-lo em vez de começar do zero:
error_resume_done = { $path } é um rastreamento concluído: não há nada para retomar.
    Para auditar o site de novo, execute o comando do rastreamento original; o arquivo concluído será mantido ao lado como .prev.sqlite.
error_resume_status = { $path } não pode ser retomado: o status dele é “{ $status }”. Só rastreamentos interrompidos (status “running” ou “paused”) podem ser retomados.
error_resume_schema = { $path } usa o esquema v{ $found } e esta versão grava v{ $expected }: um rastreamento interrompido só pode ser retomado pela versão que o iniciou. Rastreie de novo.
error_resume_config = a configuração de rastreamento salva em { $path } não pode ser lida. O arquivo foi gravado por outra versão? Rastreie de novo.
