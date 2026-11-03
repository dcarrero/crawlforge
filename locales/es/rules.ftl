### CrawlForge — rule catalogue: names and descriptions.
###
### English (locales/en/) is the source and the authority; every other folder is a
### translation of it. CONTRIBUTING.md, "Adding or improving a language", explains the rules.
### The comments above each message are context for translators: they are not shown.

### Each rule is one message: its ID, the short name as the value, and the description
### as the `.description` attribute. Backticks and <tags> are literal text, keep them.

ASSET-BROKEN = Hoja de estilo o script que no carga
    .description = Un CSS o un JS del sitio devuelve 4xx o 5xx. Google renderiza la página para indexarla, así que una hoja de estilo ausente puede hacer que la vea sin maquetar —y por tanto no apta para móvil— y un script ausente puede dejar vacío el contenido que se pinta al hidratar. Es típico de un despliegue con los hashes de fichero desincronizados.
ASSET-CSS-HEAVY = Hoja de estilo demasiado pesada
    .description = La hoja de estilo supera los 100 KB tal como se sirve. El navegador no pinta nada hasta tenerla, así que estos bytes cuestan más que los de un script: retrasan la primera pintura entera. El umbral es más bajo por eso. Casi siempre es un framework completo servido para usar una décima parte.
ASSET-FORM-BROKEN = Formulario que envía a una URL rota
    .description = El `action` de un formulario apunta a una URL propia que devuelve 4xx o 5xx. El formulario se ve bien, se rellena bien y se pierde al enviarlo: es de los pocos defectos que cuestan clientes directamente, y nadie se entera porque quien lo sufre se va sin avisar. Solo se comprueban los formularios que se envían por GET —un buscador, un filtro de catálogo—: comprobar un POST exigiría enviarlo, y esta herramienta no envía formularios.
ASSET-IFRAME-BROKEN = Marco embebido que no carga
    .description = Un `<iframe>` de la página apunta a una URL propia que devuelve 4xx o 5xx, así que el visitante ve un hueco en blanco donde debería estar el mapa, el vídeo o el formulario. No deja rastro en la página que lo contiene —el HTML sigue siendo válido— y por eso pasa desapercibido durante meses.
ASSET-IMG-BROKEN = Imagen que no carga
    .description = Una imagen a la que apunta el sitio devuelve 4xx o 5xx. El hueco se ve en la página, la imagen no existe para la búsqueda de imágenes y cada visita gasta una petición en un error. Casi siempre es una migración que no se llevó la carpeta de subidas, o una ruta escrita a mano.
ASSET-IMG-EMPTY-ALT-LINK = Enlace con una sola imagen de alt vacío
    .description = Un enlace cuyo único contenido es una imagen con `alt=""` no tiene nombre accesible: quien navega con lector de pantalla oye «enlace» y nada más, y el buscador no recibe ninguna señal de a dónde lleva. El caso típico es el logotipo de la cabecera, que suele ser el enlace más repetido del sitio. Aquí el `alt` no es decorativo: es el texto del enlace.
ASSET-IMG-HEAVY = Imagen demasiado pesada
    .description = La imagen supera los 200 KB. Es la causa más frecuente de un LCP malo en móvil: retrasa la pintura del elemento principal y se come el ancho de banda de una conexión lenta. Suele bastar con exportarla al tamaño en que se muestra de verdad y servirla en WebP o AVIF.
ASSET-IMG-NO-ALT = Imagen sin atributo alt
    .description = Hay imágenes sin atributo `alt`. El lector de pantalla no tiene nada que leer y acaba deletreando el nombre del fichero, y Google pierde el único texto que describe la imagen: es lo que la posiciona en la búsqueda de imágenes y lo que se muestra cuando la foto no carga. Un `alt=""` vacío sí es válido, pero solo para imágenes decorativas.
ASSET-JS-HEAVY = Script demasiado pesado
    .description = El script supera los 250 KB tal como se sirve. Cada página que lo carga paga su descarga y, sobre todo, el tiempo de analizarlo y ejecutarlo en el hilo principal, que es de donde sale una mala respuesta al primer toque en un móvil corriente. Suele ser un paquete que nadie ha dividido: mira qué parte hace falta en la primera pantalla y carga el resto aparte.
CANON-CHAIN = Cadena de canonical
    .description = A declara como canonical a B, y B declara como canonical a C. El canonical no es transitivo para Google: al encontrar una cadena la ignora y decide por su cuenta cuál es la URL principal. Toda la cadena debe apuntar directamente a C.
CANON-CROSS-DOMAIN = Canonical a otro dominio
    .description = El canonical apunta a un host distinto del de la página. Es legítimo en sindicación de contenido, pero cuando no es deliberado regala el posicionamiento al otro dominio: la propia página deja de indexarse. Suele venir de una migración a medias o de un entorno de pruebas copiado.
CANON-MISSING = Sin canonical
    .description = La página indexable no declara rel=canonical. No es grave por sí solo, porque Google infiere el canonical, pero sin él cualquier parámetro de URL —una campaña, un filtro, un orden— puede acabar indexado como una página distinta.
CANON-MULTIPLE = Varios canonical
    .description = La página declara más de un link rel=canonical. Google no elige entre ellos: los ignora todos, así que el efecto es el de no tener ninguno. Casi siempre es la plantilla y un plugin de SEO emitiendo la etiqueta cada uno por su cuenta.
CANON-RELATIVE = Canonical relativo
    .description = El canonical se declara como referencia relativa en vez de con URL absoluta. Funciona mientras la página se sirva en un solo sitio, pero si el HTML se reproduce bajo otro host —un entorno de pruebas, un proxy, un scraper— el canonical se resuelve contra ese host y deja de señalar al original.
CANON-TO-4XX = Canonical a una URL con error
    .description = El canonical apunta a una URL que responde con error. La página se está declarando duplicada de algo que no existe, así que ni ella ni el destino pueden indexarse: el contenido desaparece de los resultados por completo.
CANON-TO-NOINDEX = Canonical a una página con noindex
    .description = El canonical apunta a una página marcada con noindex. Las dos señales se contradicen: una dice «indexa aquella» y aquella dice «no me indexes». El resultado habitual es que se pierden las dos URLs.
CANON-TO-REDIRECT = Canonical a una redirección
    .description = El canonical apunta a una URL que redirige a otra. Google tiene que decidir entre la señal del canonical y la de la redirección, y suele quedarse con el destino final, con lo que la etiqueta no sirve para nada. Apúntala directamente a la URL que responde 200.
CONTENT-H1-EMPTY = H1 sin texto
    .description = Hay un H1 en el marcado, pero no aporta texto: está vacío o su único contenido es una imagen que no dice nada. El encabezado ocupa el sitio del titular sin cumplir su función, así que ni el buscador ni un lector de pantalla obtienen el tema de la página. El caso típico es un logotipo dentro del H1.
CONTENT-H1-MISSING = Sin H1
    .description = La página indexable no tiene H1, o lo tiene vacío. El H1 le dice al buscador de qué trata la página con las palabras del autor, y es el primer encabezado que anuncia un lector de pantalla al entrar en el contenido.
CONTENT-H1-MULTIPLE = Varios H1
    .description = La página tiene más de un H1. HTML5 lo permite, pero deja de haber un tema principal: el buscador tiene que adivinar de qué trata la página y el lector de pantalla anuncia varios encabezados de nivel uno como si fueran documentos distintos. Casi siempre es la plantilla marcando el logotipo o el nombre del sitio como H1 además del titular.
CONTENT-HEADING-SKIP = Salto de nivel de encabezado
    .description = Los encabezados bajan más de un nivel de golpe, por ejemplo de H2 a H4. El esquema del documento queda con huecos: quien navega por encabezados no sabe si el H4 es un apartado del H2 o de otra sección, y el buscador pierde la jerarquía con la que entiende qué depende de qué. Suele venir de elegir el encabezado por su tamaño de letra en vez de por su nivel.
CONTENT-LANG-MISSING = Sin atributo lang
    .description = El elemento <html> no declara el idioma del contenido. Sin él, el lector de pantalla pronuncia el texto con las reglas del idioma del sistema, el navegador no sabe qué diccionario usar para traducir y las señales de idioma del sitio quedan en manos de la detección automática. Es un atributo de una línea.
CONTENT-THIN = Contenido escaso
    .description = Una página indexable con menos de 300 palabras de texto visible. Rara vez tiene suficiente materia para responder a una consulta, así que compite mal y, en cantidad, diluye la calidad media del sitio a ojos del buscador. Muchas son archivos, etiquetas o fichas autogeneradas que convendría no indexar en vez de ampliar.
DUP-CONTENT-EXACT = Contenido idéntico
    .description = Dos o más URLs indexables devuelven exactamente el mismo HTML, byte a byte. Compiten entre sí por las mismas consultas y reparten los enlaces entrantes en vez de sumarlos. Se resuelve dejando una indexable y canonizando las demás hacia ella.
DUP-CONTENT-NEAR = Contenido casi idéntico
    .description = Dos o más URLs indexables tienen casi el mismo texto visible aunque su HTML no sea idéntico: cambian una fecha, un widget o unas pocas frases. Suele tratarse de páginas de etiqueta y de categoría que listan los mismos artículos, o de una entrada reescrita sobre otra. Google elige una para mostrarla y aparta las demás. Conviene fusionarlas, diferenciarlas o canonizarlas hacia la principal.
HREFLANG-INVALID-CODE = Código hreflang no válido
    .description = El valor del atributo no es un idioma ISO 639-1 con región ISO 3166-1 opcional ni el especial x-default. Google descarta en silencio la anotación mal escrita, así que el conjunto queda incompleto sin que nada lo avise: es el fallo más frecuente y el más difícil de ver a ojo.
HREFLANG-LANG-MISMATCH = Hreflang propio en otro idioma que la página
    .description = La anotación hreflang que apunta a la propia página declara un idioma distinto del que dice su `<html lang>`: una página en español que se anuncia como inglesa, por ejemplo. Una de las dos está mal, y si es el hreflang, Google ofrece la página a quien busca en el otro idioma. Suele ser un plugin de traducción que copia el conjunto de la página original sin cambiar el código.
HREFLANG-NO-SELF = Hreflang sin autorreferencia
    .description = La página declara alternativas de idioma pero ninguna apunta a ella misma. Google exige que cada página del conjunto se incluya a sí misma; si falta, descarta el grupo entero y ninguna de las traducciones se beneficia de las demás.
HREFLANG-NOT-RECIPROCAL = Hreflang sin reciprocidad
    .description = Esta página declara una alternativa de idioma que no la declara de vuelta. Las anotaciones hreflang son votos que solo cuentan si son mutuos: Google ignora la relación entera cuando uno de los dos lados no confirma al otro.
HREFLANG-SELF-ONLY = Hreflang sin alternativas
    .description = La página declara hreflang, pero todas las anotaciones apuntan a ella misma: no hay ninguna traducción en el conjunto, así que no hace nada. Si la página no tiene traducción, sobra; si la tiene, faltan sus enlaces, que es lo que de verdad hay que mirar: suele ser una traducción que el plugin dejó de vincular.
HREFLANG-TO-4XX = Hreflang a URL con error
    .description = Una alternativa de idioma apunta a una URL que devuelve 4xx. La traducción que la página promete no existe: el visitante que cambia de idioma aterriza en un error y Google saca del conjunto a la página que apunta ahí.
HTTP-404-EXTERNAL = Enlace externo roto
    .description = El sitio enlaza a una URL de otro dominio que ya no existe: responde 404 o 410, o su dominio no resuelve. No penaliza como un 404 propio, pero manda al visitante a una página de error y envejece el contenido: una guía llena de enlaces muertos deja de parecer mantenida.
HTTP-404-INTERNAL = Enlace interno roto
    .description = Una URL del propio sitio devuelve 4xx y hay páginas enlazándola. Gasta presupuesto de rastreo, corta el flujo de enlazado interno hacia lo que había ahí y deja al visitante en una página de error.
HTTP-5XX = Error de servidor
    .description = La URL devuelve 5xx: el servidor ha fallado al construir la respuesta. Google retira de su índice lo que responde así de forma sostenida y reduce el ritmo de rastreo de todo el sitio mientras dure.
HTTP-EXTERNAL-UNVERIFIABLE = Enlace externo sin verificar
    .description = El sitio enlaza a una URL de otro dominio que rechazó la comprobación: respondió 401, 403 o 429. Suele ser un muro antibots, un muro de pago o un límite de peticiones, y la página casi siempre abre bien en un navegador. No se da por rota porque no hay forma de saberlo desde aquí: ábrela para confirmar que sigue viva.
HTTP-LARGE-PAGE = HTML demasiado grande
    .description = El documento HTML pasa de 500 KB sin contar imágenes ni scripts. Retrasa el primer pintado en conexiones móviles y suele indicar que la plantilla vuelca contenido que la página no muestra, o que hay CSS y JS incrustados que deberían ir aparte.
HTTP-MIXED-CONTENT = Contenido mixto
    .description = Una página servida por HTTPS carga imágenes, hojas de estilo o scripts por HTTP. El navegador bloquea los scripts y las hojas de estilo sin avisar al visitante, y marca la conexión como no segura: la página se ve rota y deja de dar confianza.
HTTP-NO-HTTPS = El sitio responde por HTTP
    .description = Hay URLs internas que sirven contenido por HTTP sin redirigir a HTTPS. El sitio queda accesible en dos direcciones distintas para la misma página —contenido duplicado— y el navegador avisa al visitante de que la conexión no es segura.
HTTP-REDIRECT-CHAIN = Cadena de redirecciones
    .description = Se llega al destino final tras dos o más saltos seguidos. Cada salto suma latencia para el visitante y gasto de rastreo, y suele delatar dos reglas de reescritura que se pisan. El arreglo es apuntar el primer salto directamente al final.
HTTP-REDIRECT-LOOP = Bucle de redirección
    .description = Las redirecciones vuelven sobre una URL ya visitada, así que el destino no se alcanza nunca. El navegador corta con un error y el buscador no llega a ver contenido: para todos los efectos, esa parte del sitio no existe.
HTTP-REDIRECT-TO-404 = Redirección a un error
    .description = La redirección acaba en una URL que devuelve 4xx. Es el peor de los dos mundos: se conserva la redirección que hacía creer que el contenido se había movido, pero el destino tampoco existe, así que el enlace y su autoridad se pierden igual.
HTTP-SLOW-RESPONSE = Respuesta lenta
    .description = El servidor tarda más de un segundo en enviar el primer byte. Ese tiempo se suma íntegro al de carga que mide Core Web Vitals, y limita cuántas páginas alcanza a rastrear el buscador en cada visita.
INDEX-BLOCKED-IN-SITEMAP = En el sitemap y bloqueada
    .description = El sitemap presenta la URL como contenido que se quiere indexar y robots.txt prohíbe rastrearla. Las dos instrucciones se contradicen, Search Console lo reporta como error de cobertura y la URL se queda fuera del índice. Casi siempre es un Disallow escrito para otra cosa que atrapó de paso a una sección publicada.
INDEX-DEEP-PAGE = Demasiados clics desde la portada
    .description = Hacen falta más de cuatro clics desde la portada para llegar a esta página. Cuanta más distancia, menos autoridad interna recibe y menos a menudo la revisita el buscador; en catálogos y archivos es el síntoma de una paginación sin atajos, donde la página 40 solo se alcanza pasando por las 39 anteriores.
INDEX-NO-INTERNAL-LINKS-IN = Sin enlaces internos entrantes
    .description = Ninguna otra página del sitio enlaza a esta, aunque es indexable y se publica. Sin enlaces entrantes no recibe autoridad interna, el buscador tarda mucho más en revisitarla y sus cambios pasan desapercibidos. Es lo primero que se mira cuando una página nueva no acaba de posicionar.
INDEX-NOFOLLOW-INTERNAL = Enlace interno con nofollow
    .description = Esta página enlaza a otra del mismo sitio con rel=nofollow. El enlace no transmite autoridad ni sirve para descubrir el destino, así que dentro de un mismo dominio rara vez tiene sentido: el «sculpting» de PageRank dejó de funcionar en 2009. Suele venir de un plugin o de una plantilla que lo pone en todos los enlaces sin distinguir internos de externos.
INDEX-NOINDEX = Con noindex
    .description = La página pide que no se indexe, por su meta robots o por la cabecera X-Robots-Tag. Google la rastrea y la descarta: no aparecerá en resultados por ninguna consulta. En archivos, etiquetas y páginas de sistema suele ser una decisión deliberada del plugin SEO, y por eso el aviso es moderado; se eleva a crítico si afecta a la portada, y el conflicto con el sitemap tiene su propia regla.
INDEX-NOINDEX-IN-SITEMAP = En el sitemap y con noindex
    .description = La URL está en el sitemap, que es la lista de lo que el sitio quiere ver indexado, y su propia cabecera o meta robots dice lo contrario. Una de las dos cosas está mal: o sobra del sitemap, o el noindex es un resto que nadie ha retirado. Mientras conviven, el sitio se contradice ante el buscador.
INDEX-ORPHAN-PAGE = Página huérfana
    .description = El sitio declara esta URL en su sitemap pero ninguna de sus páginas la enlaza. Un visitante no puede llegar a ella navegando y el buscador la ve como contenido sin contexto ni autoridad interna. Es el hallazgo que aparece al cruzar lo declarado con lo alcanzado, y no se puede obtener mirando una página suelta.
INDEX-ROBOTS-BLOCKED = Bloqueada por robots.txt
    .description = El sitio enlaza esta URL desde sus propias páginas y a la vez la prohíbe en robots.txt. Google no puede leerla, así que no sabe qué contiene ni sigue sus enlaces: el enlace interno no lleva a ninguna parte y la autoridad que le pasa se pierde. Es distinto de un noindex, que sí permite leer la página.
INDEX-ROBOTS-TXT-BLOCKS-ALL = robots.txt bloquea el sitio entero
    .description = El robots.txt prohíbe rastrear la raíz del sitio, así que ninguna página puede leerse ni indexarse. Es la forma más rápida y silenciosa de desaparecer de Google, y casi siempre es el mismo accidente: el fichero del entorno de pruebas, que lleva Disallow: /, subido a producción en un despliegue.
INDEX-ROBOTS-TXT-MISSING = Sin robots.txt
    .description = El sitio no sirve /robots.txt. No impide que se le indexe —a falta de fichero se rastrea todo— pero se pierde el sitio donde se anuncia el sitemap y donde se excluyen las zonas que no aportan nada al buscador, como los resultados de búsqueda interna o las páginas de carrito.
INDEX-SECTION-DISCONNECTED = Sección desconectada del enlazado interno
    .description = Hay un grupo de páginas que se enlazan entre sí pero al que no se llega desde la portada siguiendo enlaces normales: el puente que las une al resto del sitio es JavaScript, un formulario o no existe. El buscador solo las descubre por el sitemap y les llega poca autoridad interna. La causa es una —falta un enlace rastreable hacia la sección— y se arregla una vez, no página a página.
INDEX-SITEMAP-ERROR = Sitemap con errores
    .description = Un sitemap no responde, tiene el XML mal formado o se pasa de los límites del protocolo (50.000 URLs o 50 MB). El buscador deja de leerlo donde encuentra el error, así que todo lo que venga detrás no llega a descubrirse por esa vía y nadie avisa: el sitemap sigue ahí, aparentemente correcto.
INDEX-SITEMAP-MISSING = Sin sitemap
    .description = No se ha encontrado ningún sitemap: ni robots.txt lo anuncia, ni está en las rutas habituales, ni declara ninguna URL. Sin él, el buscador solo llega a lo que esté enlazado y a la velocidad que le permita el enlazado interno; y se pierde el contraste entre lo que el sitio dice publicar y lo que se alcanza rastreando.
LINK-ANCHOR-EMPTY = Enlace interno sin texto
    .description = Hay enlaces internos sin texto, sin imagen y sin aria-label: un icono pintado con CSS, un bloque entero convertido en enlace vacío. El buscador no recibe ninguna pista de a qué lleva y quien usa lector de pantalla oye «enlace» y nada más. Dale un texto, aunque sea oculto a la vista, o un aria-label que diga a dónde va.
LINK-ANCHOR-GENERIC = Texto de enlace genérico
    .description = Hay enlaces internos cuyo texto entero es una fórmula como «leer más», «aquí» o «haz clic aquí». Ese texto no dice nada de la página de destino, que es justo lo que el buscador lee en él, y fuera de contexto —como lo recorre un lector de pantalla— todos los enlaces dicen lo mismo. Basta con que el texto nombre el destino: «leer más sobre tarifas» en vez de «leer más».
META-DESC-DUPLICATE = Meta descripción duplicada
    .description = Dos o más páginas indexables repiten la misma descripción, con lo que el resultado de búsqueda no distingue entre ellas y ninguna promete nada concreto. Suele venir de una descripción por defecto puesta a nivel de sitio que nadie sobrescribió.
META-DESC-MISSING = Sin meta descripción
    .description = La página indexable no declara descripción, así que Google recorta un fragmento cualquiera del cuerpo para el resultado de búsqueda. No es un factor de posicionamiento, pero sí el texto que decide el clic, y renunciar a escribirlo es renunciar a controlarlo.
META-DESC-TOO-LONG = Meta descripción demasiado ancha
    .description = La descripción no cabe en el resultado de búsqueda y se corta, de modo que la llamada a la acción del final no llega a leerse. Se mide en píxeles, no en caracteres, porque el corte lo decide el ancho renderizado.
META-DESC-TOO-SHORT = Meta descripción demasiado corta
    .description = La descripción es tan breve que Google tiende a ignorarla y a componer el fragmento con texto del cuerpo. Con tan poco espacio usado no cabe ni el argumento ni la variante de la consulta que justificarían el clic.
META-OUTSIDE-HEAD = Metadatos fuera del head
    .description = El canonical, el meta robots, el hreflang, el título o la descripción quedan fuera del <head>. Un elemento que no pertenece al head —un <img> de seguimiento, un <div>, un texto suelto que deja un plugin— lo cierra en ese punto, y Google ignora todo lo que venga después aunque siga en el código fuente. El efecto es el de no tenerlos.
META-REFRESH = Redirección por meta refresh
    .description = La página redirige con <meta http-equiv=refresh> en lugar de con un 301. Google lo interpreta con reservas, no traslada la autoridad con la misma fiabilidad y el usuario ve un salto brusco; con un retardo distinto de cero, además, atrapa el botón de volver del navegador.
META-TITLE-DUPLICATE = Título duplicado
    .description = Dos o más páginas indexables comparten el mismo <title>. Google tiene que elegir cuál de ellas mostrar para la misma consulta, y compiten entre sí en vez de sumar. Es el síntoma más común de paginación o de archivos mal configurados.
META-TITLE-MISSING = Sin título
    .description = La página es indexable y no tiene <title>, o lo tiene vacío. Es el factor on-page con más peso que se controla directamente, y es el texto en azul del resultado de búsqueda: sin él, Google inventa uno con lo que encuentra en la página.
META-TITLE-MULTIPLE = Varias etiquetas de título
    .description = La página trae más de un <title>. El navegador y Google se quedan con el primero y descartan el resto en silencio, de modo que el título que se escribió con cuidado puede no ser el que se publica. Casi siempre es una plantilla que lo imprime dos veces, o un plugin de SEO que añade el suyo sin quitar el del tema.
META-TITLE-TOO-LONG = Título demasiado ancho
    .description = El título no cabe en el resultado de búsqueda y Google lo corta con puntos suspensivos, así que la parte final —muchas veces la marca, o la palabra clave que se dejó para el final— no se lee. Se mide en píxeles, no en caracteres: el corte lo decide el ancho renderizado.
META-TITLE-TOO-SHORT = Título demasiado corto
    .description = El título deja sin usar buena parte del espacio que el resultado de búsqueda ofrece. No es un error, es espacio desaprovechado: cabe una variante de la consulta o un motivo para hacer clic, y a menudo indica una plantilla que solo imprime el nombre de la sección.
META-VIEWPORT-MISSING = Sin meta viewport
    .description = Sin <meta name=viewport> el móvil dibuja la página a 980 px de ancho y la reduce, así que el texto queda ilegible y hay que hacer zoom. La indexación es móvil primero: lo que Google evalúa es esa versión encogida.
SOCIAL-OG-IMAGE-BROKEN = Imagen Open Graph que no carga
    .description = La imagen declarada en og:image devuelve un error, directamente o al final de sus redirecciones. La etiqueta está y ninguna herramienta que solo lea el HTML lo nota, pero al compartir la página en WhatsApp, LinkedIn o Slack la tarjeta sale sin imagen, o con un hueco. Suele ser una imagen borrada de la biblioteca de medios o una ruta que no sobrevivió a una migración.
SOCIAL-OG-MISSING = Open Graph incompleto
    .description = Falta og:title, og:description u og:image. Cuando alguien comparte la página en WhatsApp, LinkedIn o Slack, la red no puede montar la tarjeta y el enlace aparece como una URL desnuda: no afecta al posicionamiento, pero se pierde el clic.
