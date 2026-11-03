### CrawlForge — rule catalogue: names and descriptions, Brazilian Portuguese (pt-BR).
###
### English (locales/en/) is the source and the authority; this folder is a translation of it.
### CONTRIBUTING.md, "Adding or improving a language", explains the rules.

### Each rule is one message: its ID, the short name as the value, and the description
### as the `.description` attribute. Backticks and <tags> are literal text, keep them.

ASSET-BROKEN = CSS ou JS quebrado
    .description = Um arquivo CSS ou JS do site retorna 4xx ou 5xx. O Google renderiza a página para indexá-la, então uma folha de estilo ausente pode fazer com que ele veja uma página sem estilo —e, portanto, não otimizada para dispositivos móveis—, e um script ausente pode deixar vazio o conteúdo renderizado no cliente. É típico de um deploy com hashes de arquivo que não batem.
ASSET-CSS-HEAVY = Folha de estilo pesada
    .description = A folha de estilo passa de 100 KB do jeito que é entregue. O navegador não renderiza nada até recebê-la, então esses bytes custam mais que os de um script: atrasam toda a primeira renderização. Por isso o limite é mais baixo. Quase sempre é um framework inteiro enviado para usar um décimo dele.
ASSET-FORM-BROKEN = Formulário enviado para URL quebrada
    .description = O `action` de um formulário aponta para uma URL do seu site que retorna 4xx ou 5xx. O formulário parece normal, é preenchido normalmente e se perde no envio: um dos poucos defeitos que custam clientes diretamente, e ninguém fica sabendo, porque quem esbarra nele vai embora sem avisar. Só são verificados os formulários enviados com GET —uma caixa de busca, um filtro de catálogo—: verificar um POST significaria enviá-lo, e esta ferramenta não envia formulários.
ASSET-IFRAME-BROKEN = Iframe quebrado
    .description = Um `<iframe>` da página aponta para uma URL do seu site que retorna 4xx ou 5xx, então o visitante vê um buraco em branco onde deveriam estar o mapa, o vídeo ou o formulário. Não deixa rastro na página que o contém —o HTML continua válido—, e é por isso que passa despercebido por meses.
ASSET-IMG-BROKEN = Imagem quebrada
    .description = Uma imagem referenciada pelo site retorna 4xx ou 5xx. O buraco aparece na página, a imagem não existe para a busca de imagens e cada visita gasta uma requisição com um erro. Quase sempre é uma migração que deixou para trás a pasta de uploads, ou um caminho escrito à mão.
ASSET-IMG-EMPTY-ALT-LINK = Link só com imagem de alt vazio
    .description = Um link cujo único conteúdo é uma imagem com `alt=""` não tem nome acessível: quem usa leitor de tela ouve “link” e nada mais, e o buscador não recebe nenhum sinal sobre o destino. O caso típico é o logotipo do cabeçalho, geralmente o link mais repetido do site. Aqui o `alt` não é decorativo: é o texto do link.
ASSET-IMG-HEAVY = Imagem pesada
    .description = A imagem passa de 200 KB. É a causa mais comum de um LCP ruim no celular: atrasa a renderização do elemento principal e consome a banda de uma conexão lenta. Exportá-la no tamanho em que é realmente exibida e servi-la em WebP ou AVIF costuma bastar.
ASSET-IMG-NO-ALT = Imagem sem atributo alt
    .description = Algumas imagens não têm atributo `alt`. Um leitor de tela não tem o que ler e acaba soletrando o nome do arquivo, e o Google perde o único texto que descreve a imagem: é o que a posiciona na busca de imagens e o que aparece quando ela não carrega. Um `alt=""` vazio é válido, mas só para imagens decorativas.
ASSET-JS-HEAVY = Script pesado
    .description = O script passa de 250 KB do jeito que é entregue. Cada página que o carrega paga pelo download e, principalmente, pela análise e execução dele na thread principal, que é de onde vem a baixa responsividade em um celular comum. Geralmente é um bundle que ninguém dividiu: identifique o que é necessário para a primeira tela e carregue o resto separadamente.
CANON-CHAIN = Cadeia de canônicas
    .description = A declara B como canônica, e B declara C. Para o Google, canônicas não são transitivas: quando encontra uma cadeia, ele a ignora e decide por conta própria qual URL é a principal. Cada etapa deveria apontar direto para C.
CANON-CROSS-DOMAIN = Canônica para outro domínio
    .description = A canônica aponta para um host diferente do da própria página. Isso é legítimo para conteúdo sindicado, mas, quando não é proposital, entrega o posicionamento ao outro domínio: a própria página deixa de ser indexada. Costuma vir de uma migração pela metade ou de um ambiente de staging copiado.
CANON-MISSING = Canônica ausente
    .description = A página indexável não declara rel=canonical. Não é grave por si só, já que o Google deduz a canônica, mas sem ela qualquer parâmetro de URL — uma campanha, um filtro, uma ordenação — pode acabar indexado como uma página separada.
CANON-MULTIPLE = Várias canônicas
    .description = A página declara mais de um link rel=canonical. O Google não escolhe um: ignora todos, então o efeito é o mesmo de não ter nenhum. Quase sempre é o tema e um plugin de SEO emitindo a tag cada um por conta própria.
CANON-RELATIVE = Canônica relativa
    .description = A canônica é declarada como referência relativa em vez de URL absoluta. Funciona enquanto a página é servida de um único lugar, mas, se o HTML for reproduzido em outro host — um ambiente de staging, um proxy, um scraper —, a canônica é resolvida em relação a esse host e deixa de apontar para a original.
CANON-TO-4XX = Canônica para URL com erro
    .description = A canônica aponta para uma URL que responde com erro. A página se declara duplicata de algo que não existe, então nem ela nem o destino podem ser indexados: o conteúdo some dos resultados por completo.
CANON-TO-NOINDEX = Canônica para página com noindex
    .description = A canônica aponta para uma página marcada com noindex. Os dois sinais se contradizem: um diz “indexe aquela” e aquela diz “não me indexe”. O resultado habitual é perder as duas URLs.
CANON-TO-REDIRECT = Canônica para redirecionamento
    .description = A canônica aponta para uma URL que redireciona para outro lugar. O Google precisa escolher entre o sinal da canônica e o do redirecionamento, e costuma ficar com o destino final, o que torna a tag inútil. Aponte-a direto para a URL que responde 200.
CONTENT-H1-EMPTY = H1 vazio
    .description = Há um H1 no código, mas ele não traz nenhum texto: está vazio, ou seu único conteúdo é uma imagem que não diz nada. O cabeçalho ocupa o lugar do título principal sem cumprir a função dele, então nem o buscador nem um leitor de tela ficam sabendo o tema da página. O caso habitual é um logotipo dentro do H1.
CONTENT-H1-MISSING = H1 ausente
    .description = A página indexável não tem H1, ou tem um vazio. O H1 diz ao buscador do que trata a página com as palavras do próprio autor, e é o primeiro cabeçalho que um leitor de tela anuncia ao entrar no conteúdo.
CONTENT-H1-MULTIPLE = Vários H1
    .description = A página tem mais de um H1. O HTML5 permite, mas deixa de haver um único tema principal: o buscador precisa adivinhar do que a página trata, e um leitor de tela anuncia vários cabeçalhos de nível um como se fossem documentos separados. Costuma ser o template marcando o logotipo ou o nome do site como H1, além do título real.
CONTENT-HEADING-SKIP = Nível de cabeçalho pulado
    .description = Os níveis de cabeçalho descem mais de um degrau de uma vez, por exemplo de H2 para H4. A estrutura do documento fica com buracos: quem navega por cabeçalhos não consegue saber se o H4 pertence àquele H2 ou a outra seção, e o buscador perde a hierarquia que usa para entender o que pertence a quê. Costuma vir de escolher um cabeçalho pelo tamanho da fonte, e não pelo nível.
CONTENT-LANG-MISSING = Atributo lang ausente
    .description = O elemento <html> não declara o idioma do conteúdo. Sem ele, um leitor de tela pronuncia o texto com as regras do idioma do sistema, o navegador não sabe qual dicionário usar ao traduzir, e os sinais de idioma do site ficam por conta da detecção automática. É um atributo de uma linha.
CONTENT-THIN = Conteúdo raso
    .description = Uma página indexável com menos de 300 palavras de texto visível. Raramente tem substância suficiente para responder a uma busca, então compete mal e, em grande quantidade, dilui a qualidade média do site aos olhos do buscador. Muitas são arquivos, páginas de tag ou páginas geradas automaticamente que é melhor deixar fora do índice do que ampliar.
DUP-CONTENT-EXACT = Conteúdo idêntico
    .description = Duas ou mais URLs indexáveis retornam exatamente o mesmo HTML, byte a byte. Elas competem entre si pelas mesmas buscas e dividem os links de entrada em vez de somá-los. Corrija mantendo uma delas indexável e canonicalizando as demais para ela.
DUP-CONTENT-NEAR = Conteúdo quase duplicado
    .description = Duas ou mais URLs indexáveis têm quase o mesmo texto visível, embora o HTML não seja idêntico: muda uma data, um widget ou algumas frases. Costumam ser páginas de tag e de categoria que listam os mesmos posts, ou um post reescrito em cima de outro. O Google escolhe uma para mostrar e deixa as outras de lado. Mescle-as, diferencie-as ou canonicalize-as para a principal.
HREFLANG-INVALID-CODE = Código hreflang inválido
    .description = O valor do atributo não é um idioma ISO 639-1 com região ISO 3166-1 opcional, nem o valor especial x-default. O Google descarta em silêncio uma anotação malformada, então o conjunto fica incompleto sem aviso: é a falha mais frequente e a mais difícil de detectar a olho nu.
HREFLANG-LANG-MISMATCH = Hreflang próprio em idioma diferente do da página
    .description = A anotação hreflang que aponta para a própria página declara um idioma diferente do seu `<html lang>`: uma página em espanhol que se anuncia como inglês, por exemplo. Um dos dois está errado e, se for o hreflang, o Google mostra a página a quem busca no outro idioma. Costuma ser um plugin de tradução que copia o conjunto da página original sem trocar o código.
HREFLANG-NO-SELF = Hreflang sem autorreferência
    .description = A página declara alternativas de idioma, mas nenhuma aponta para ela mesma. O Google exige que cada página do conjunto inclua a si mesma; quando esse link falta, ele descarta o grupo inteiro e nenhuma das traduções se beneficia das outras.
HREFLANG-NOT-RECIPROCAL = Hreflang sem reciprocidade
    .description = Esta página declara uma alternativa de idioma que não a declara de volta. As anotações hreflang são votos que só contam quando são mútuos: o Google ignora a relação inteira quando um dos lados não confirma o outro.
HREFLANG-SELF-ONLY = Hreflang sem alternativas
    .description = A página declara hreflang, mas todas as anotações apontam para ela mesma: não há nenhuma tradução no conjunto, então não tem efeito algum. Se a página não tem tradução, a anotação é desnecessária; se tem, faltam os links, e é isso que realmente precisa ser verificado: costuma ser uma tradução que o plugin deixou de vincular.
HREFLANG-TO-4XX = Hreflang para URL com erro
    .description = Uma alternativa de idioma aponta para uma URL que retorna 4xx. A tradução que a página promete não existe: um visitante que troca de idioma cai em uma página de erro, e o Google remove do conjunto a página que fez a referência.
HTTP-404-EXTERNAL = Link externo quebrado
    .description = O site tem um link para uma URL de outro domínio que não existe mais: responde 404 ou 410, ou o domínio não resolve. Não prejudica tanto quanto um 404 próprio, mas manda o visitante para uma página de erro e envelhece o conteúdo: um guia cheio de links mortos deixa de parecer atualizado.
HTTP-404-INTERNAL = Link interno quebrado
    .description = Uma URL deste site retorna 4xx e há páginas com links para ela. Desperdiça orçamento de rastreamento, corta o fluxo de links internos para o que existia ali e deixa o visitante em uma página de erro.
HTTP-5XX = Erro de servidor
    .description = A URL retorna 5xx: o servidor não conseguiu montar a resposta. O Google remove do índice as páginas que respondem assim por muito tempo e desacelera o rastreamento do site inteiro enquanto isso durar.
HTTP-EXTERNAL-UNVERIFIABLE = Link externo não verificável
    .description = O site tem um link para uma URL de outro domínio que recusou a verificação: respondeu 401, 403 ou 429. Geralmente é uma barreira antibot, um paywall ou um limite de taxa, e a página quase sempre abre normalmente no navegador. Não é considerado quebrado porque daqui não há como saber: abra-o para confirmar que ainda existe.
HTTP-LARGE-PAGE = HTML pesado
    .description = O documento HTML passa de 500 KB, sem contar imagens e scripts. Atrasa a primeira renderização em conexões móveis e geralmente significa que o template despeja conteúdo que a página nunca mostra, ou que CSS e JS embutidos deveriam estar em arquivos próprios.
HTTP-MIXED-CONTENT = Conteúdo misto
    .description = Uma página servida por HTTPS carrega imagens, folhas de estilo ou scripts por HTTP. O navegador bloqueia em silêncio os scripts e as folhas de estilo e marca a conexão como não segura: a página parece quebrada e deixa de inspirar confiança.
HTTP-NO-HTTPS = Site responde por HTTP
    .description = Algumas URLs internas servem conteúdo por HTTP sem redirecionar para HTTPS. O site fica acessível em dois endereços diferentes para a mesma página — conteúdo duplicado — e o navegador avisa o visitante de que a conexão não é segura.
HTTP-REDIRECT-CHAIN = Cadeia de redirecionamentos
    .description = O destino final só é alcançado após dois ou mais saltos consecutivos. Cada salto acrescenta latência para o visitante e custo de rastreamento, e geralmente significa que duas regras de reescrita estão se atropelando. A correção é apontar o primeiro salto direto para o destino final.
HTTP-REDIRECT-LOOP = Loop de redirecionamento
    .description = Os redirecionamentos voltam a uma URL já visitada, então o destino nunca é alcançado. O navegador desiste com um erro e o rastreador nunca vê conteúdo algum: para todos os efeitos, essa parte do site não existe.
HTTP-REDIRECT-TO-404 = Redirecionamento para página de erro
    .description = O redirecionamento termina em uma URL que retorna 4xx. É o pior dos dois mundos: o redirecionamento que sugeria que o conteúdo tinha mudado de lugar continua lá, mas o destino também não existe, então o link e a autoridade dele se perdem de qualquer forma.
HTTP-SLOW-RESPONSE = Resposta lenta
    .description = O servidor leva mais de um segundo para enviar o primeiro byte. Esse tempo se soma integralmente ao tempo de carregamento que as Core Web Vitals medem e limita quantas páginas o rastreador consegue percorrer a cada visita.
INDEX-BLOCKED-IN-SITEMAP = Bloqueada, mas no sitemap
    .description = O sitemap apresenta a URL como conteúdo que você quer indexado, enquanto o robots.txt proíbe rastreá-la. As duas instruções se contradizem, o Search Console marca isso como erro de cobertura e a URL fica fora do índice. Quase sempre é um Disallow escrito para outra coisa que acabou pegando uma seção publicada no caminho.
INDEX-DEEP-PAGE = Cliques demais a partir da página inicial
    .description = São necessários mais de quatro cliques a partir da página inicial para chegar a esta página. Quanto mais longe, menos autoridade interna ela recebe e menos vezes o buscador a revisita; em catálogos e arquivos, é sintoma de paginação sem atalhos, em que a página 40 só é alcançável passando pelas 39 anteriores.
INDEX-NO-INTERNAL-LINKS-IN = Sem links internos de entrada
    .description = Nenhuma outra página do site tem link para esta, embora ela seja indexável e esteja publicada. Sem links de entrada, ela não recebe autoridade interna, o buscador demora muito mais para revisitá-la e as mudanças nela passam despercebidas. É a primeira coisa a verificar quando uma página nova nunca chega a se posicionar.
INDEX-NOFOLLOW-INTERNAL = Link interno nofollow
    .description = Esta página tem um link com rel=nofollow para outra página do mesmo site. O link não transmite autoridade e não ajuda a descobrir o destino, então dentro de um mesmo domínio raramente faz sentido: o PageRank sculpting deixou de funcionar em 2009. Costuma vir de um plugin ou template que o adiciona a todos os links sem distinguir internos de externos.
INDEX-NOINDEX = Noindex
    .description = A página pede para não ser indexada, pela meta tag robots ou pelo cabeçalho X-Robots-Tag. O Google a rastreia e a descarta: ela não aparece para nenhuma busca. Em arquivos, tags e páginas utilitárias, costuma ser uma escolha deliberada do plugin de SEO, por isso o aviso é moderado; ele sobe para crítico na página inicial, e o conflito com o sitemap tem regra própria.
INDEX-NOINDEX-IN-SITEMAP = Noindex, mas no sitemap
    .description = A URL está no sitemap, que é a lista do que o site quer indexado, e o próprio cabeçalho ou a meta robots dela diz o contrário. Um dos dois está errado: ou ela não deveria estar no sitemap, ou o noindex é uma sobra que ninguém removeu. Enquanto os dois coexistirem, o site se contradiz diante do buscador.
INDEX-ORPHAN-PAGE = Página órfã
    .description = O site declara esta URL no sitemap, mas nenhuma de suas páginas tem link para ela. Um visitante não consegue chegar até ela navegando, e o buscador vê um conteúdo sem contexto e sem autoridade interna. É a ocorrência que surge ao comparar o que é declarado com o que é alcançado, e nenhuma página isolada pode revelá-la.
INDEX-ROBOTS-BLOCKED = Bloqueada pelo robots.txt
    .description = O site tem links para esta URL nas próprias páginas e, ao mesmo tempo, a proíbe no robots.txt. O Google não consegue lê-la, então não sabe o que ela contém nem segue seus links: o link interno não leva a lugar nenhum e a autoridade que ele transmite se perde. Não é o mesmo que noindex, que ainda permite ler a página.
INDEX-ROBOTS-TXT-BLOCKS-ALL = robots.txt bloqueia o site inteiro
    .description = O robots.txt proíbe rastrear a raiz do site, então nenhuma página pode ser lida nem indexada. É a forma mais rápida e silenciosa de sumir do Google, e quase sempre é o mesmo acidente: o arquivo do ambiente de staging, que traz Disallow: /, foi para produção em um deploy.
INDEX-ROBOTS-TXT-MISSING = robots.txt ausente
    .description = O site não serve /robots.txt. Isso não impede a indexação —sem o arquivo, tudo pode ser rastreado—, mas abre mão do lugar onde o sitemap é anunciado e onde se excluem as áreas que não acrescentam nada para um buscador, como resultados de busca interna ou páginas de carrinho.
INDEX-SECTION-DISCONNECTED = Seção desconectada da linkagem interna
    .description = Um grupo de páginas tem links entre si, mas não pode ser alcançado a partir da página inicial seguindo links comuns: a ponte que as liga ao resto do site é JavaScript, um formulário, ou simplesmente não existe. Os buscadores só as descobrem pelo sitemap e pouca autoridade interna chega até elas. A causa é uma só —falta um link rastreável para a seção— e se corrige uma vez, não página por página.
INDEX-SITEMAP-ERROR = Sitemap com erros
    .description = Um sitemap não responde, tem XML malformado ou ultrapassa os limites do protocolo (50.000 URLs ou 50 MB). Os buscadores param de ler no erro, então tudo o que vem depois nunca é descoberto por esse caminho, e nada avisa você: o sitemap continua lá, aparentemente normal.
INDEX-SITEMAP-MISSING = Sem sitemap
    .description = Nenhum sitemap foi encontrado: o robots.txt não anuncia nenhum, não há nenhum nos caminhos habituais e nenhum declara qualquer URL. Sem ele, o buscador só alcança o que tem link, no ritmo que a linkagem interna permitir; e você perde a comparação entre o que o site diz publicar e o que um rastreamento realmente alcança.
LINK-ANCHOR-EMPTY = Link interno sem texto
    .description = Alguns links internos não têm texto, nem imagem, nem aria-label: um ícone desenhado com CSS, um bloco inteiro transformado em link vazio. O buscador não recebe nenhuma pista sobre o destino, e quem usa leitor de tela ouve “link” e nada mais. Dê a ele um texto, mesmo que visualmente oculto, ou um aria-label que diga para onde leva.
LINK-ANCHOR-GENERIC = Texto âncora genérico
    .description = Alguns links internos consistem inteiramente em uma frase pronta como “leia mais”, “aqui” ou “clique aqui”. Esse texto não diz nada sobre a página de destino, que é exatamente o que o buscador lê nele, e fora de contexto —do jeito que um leitor de tela os lista— todos os links dizem a mesma coisa. Basta que o texto nomeie o destino: “leia mais sobre preços” em vez de “leia mais”.
META-DESC-DUPLICATE = Meta description duplicada
    .description = Duas ou mais páginas indexáveis repetem a mesma description, então o resultado de busca não as diferencia e nenhuma delas promete nada específico. Costuma vir de um valor padrão do site inteiro que ninguém substituiu.
META-DESC-MISSING = Meta description ausente
    .description = A página indexável não declara description, então o Google recorta um trecho arbitrário do corpo para o resultado de busca. Não é um fator de ranqueamento, mas é o texto que decide o clique, e não escrevê-lo é abrir mão de controlá-lo.
META-DESC-TOO-LONG = Meta description longa demais
    .description = A description não cabe no resultado de busca e é truncada, então a chamada para ação no final nunca é lida. É medida em pixels, não em caracteres, porque o corte é decidido pela largura renderizada.
META-DESC-TOO-SHORT = Meta description curta demais
    .description = A description é tão breve que o Google tende a ignorá-la e montar o snippet com texto do corpo. Com tão pouco espaço usado, não cabem nem o argumento nem a variação da busca que justificariam o clique.
META-OUTSIDE-HEAD = Metadados fora do head
    .description = A canônica, a meta robots, o hreflang, o título ou a description ficam fora do <head>. Um elemento que não pertence ao head —um <img> de rastreamento, um <div>, um texto solto deixado por um plugin— o fecha naquele ponto, e o Google ignora tudo o que vem depois, mesmo que continue no código-fonte. O efeito é o mesmo de não tê-los.
META-REFRESH = Redirecionamento por meta refresh
    .description = A página redireciona com <meta http-equiv=refresh> em vez de um 301. O Google o lê com reservas, não transmite autoridade de forma tão confiável, e o usuário vê um salto brusco; com um atraso diferente de zero, ele também prende o botão Voltar do navegador.
META-TITLE-DUPLICATE = Título duplicado
    .description = Duas ou mais páginas indexáveis compartilham o mesmo <title>. O Google precisa escolher qual mostrar para a mesma busca, então elas competem entre si em vez de se somarem. É o sintoma mais comum de paginação ou arquivos mal configurados.
META-TITLE-MISSING = Título ausente
    .description = A página é indexável e não tem <title>, ou tem um vazio. É o fator on-page de maior peso que você controla diretamente, e é o texto azul no resultado de busca: sem ele, o Google inventa um com o que encontrar.
META-TITLE-MULTIPLE = Várias tags title
    .description = A página traz mais de um <title>. Os navegadores e o Google ficam com o primeiro e descartam o resto em silêncio, então o título escrito com cuidado pode não ser o publicado. Quase sempre é um template que o imprime duas vezes, ou um plugin de SEO que acrescenta o seu sem remover o do tema.
META-TITLE-TOO-LONG = Título longo demais
    .description = O título não cabe no resultado de busca e o Google o trunca com reticências, então o final —muitas vezes a marca, ou a palavra-chave deixada para o fim— nunca é lido. É medido em pixels, não em caracteres: o corte é decidido pela largura renderizada.
META-TITLE-TOO-SHORT = Título curto demais
    .description = O título deixa sem uso a maior parte do espaço que o resultado de busca oferece. Não é um erro, mas é espaço desperdiçado: cabe uma variação da busca ou um motivo para clicar, e muitas vezes indica um template que só imprime o nome da seção.
META-VIEWPORT-MISSING = Meta viewport ausente
    .description = Sem <meta name=viewport>, um celular monta a página com 980 px de largura e a reduz, deixando o texto ilegível e obrigando a dar zoom com os dedos. A indexação é mobile-first: o que o Google avalia é essa versão encolhida.
SOCIAL-OG-IMAGE-BROKEN = Imagem Open Graph quebrada
    .description = A imagem declarada em og:image retorna um erro, diretamente ou ao final de seus redirecionamentos. A tag está lá e nenhuma ferramenta que só lê o HTML percebe, mas, quando a página é compartilhada no WhatsApp, no LinkedIn ou no Slack, o card de prévia aparece sem imagem, ou com um quadro vazio. Costuma ser uma imagem apagada da biblioteca de mídia ou um caminho que não sobreviveu a uma migração.
SOCIAL-OG-MISSING = Open Graph incompleto
    .description = Falta og:title, og:description ou og:image. Quando alguém compartilha a página no WhatsApp, no LinkedIn ou no Slack, a rede não consegue montar o card de prévia e o link aparece como uma URL crua: não afeta o posicionamento, mas o clique se perde.
