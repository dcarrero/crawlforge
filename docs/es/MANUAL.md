# Manual de CrawlForge (CLI)

> Versión en inglés: [read the manual in English](../MANUAL.md) — **la inglesa es la que manda.** Esta
> traducción puede ir por detrás; si las dos discrepan, la buena es la otra.

> La herramienta es hoy línea de comandos. Una interfaz gráfica está prevista y no existe todavía.
>
> Esto no repite el `--help`, que ya está completo y en inglés. Esto es **qué comando usar en
> cada situación real**, con los sitios de verdad.

---

## 0. Antes de empezar

El binario ya está instalado y en el `PATH`:

```bash
crawlforge --version
```

Si no responde, se reinstala desde el repositorio:

```bash
cd ~/Desarrollos/proyectos/proyectos-mini/crawlforge
cargo build --release -p crawlforge-cli && cp target/release/crawlforge ~/.cargo/bin/
```

**La salida de la herramienta está en inglés.** Es el idioma de origen; el español existe donde
hay texto de verdad que traducir —el catálogo de reglas y los informes— y se pide con `--lang es`:

```bash
crawlforge rules  --lang es
crawlforge report crawl.sqlite --lang es
```

Para no repetirlo en cada comando, en tu `~/.zshrc`:

```bash
export CRAWLFORGE_LANG=es
```

---

## 1. El flujo de un día normal

Cuatro comandos. Si solo lees una sección del manual, que sea esta.

```bash
# 1. Rastrear
crawlforge crawl https://tusitio.com/

# 2. Ver qué ha salido, aquí mismo
crawlforge report crawl-tusitio-com.sqlite --lang es

# 3. Llevárselo a alguien que no usa terminal
crawlforge export crawl-tusitio-com.sqlite --format xlsx --out auditoria.xlsx

# 4. Un informe para pegar en un ticket o mandar por correo
crawlforge report crawl-tusitio-com.sqlite --format html --out informe.html --lang es
```

Cada comando termina diciéndote cuál es el siguiente, con la orden lista para copiar. No hace
falta memorizar nada.

### Los ficheros que aparecen

| Fichero | Qué es |
|---|---|
| `crawl-<sitio>.sqlite` | **El rastreo.** Todo está aquí: URLs, enlaces, imágenes, hallazgos, cabeceras. Es un SQLite normal; se puede abrir con cualquier visor. |
| `crawl-<sitio>.prev.sqlite` | El rastreo **anterior**, apartado automáticamente al repetir. Es lo que `diff` necesita. No lo borres. |

Un rastreo se puede volver a mirar meses después sin volver a rastrear: `report`, `export` y
`diff` trabajan sobre el fichero, no sobre la red.

---

## 2. Recetas

### Rastrear un sitio entero

```bash
crawlforge crawl https://cliente.com/
```

Descubre los sitemaps, respeta `robots.txt` y para cuando agota el sitio. Va con 5 peticiones
simultáneas, que es prudente para un WordPress. En un sitio propio y bien dimensionado:

```bash
crawlforge crawl https://tusitio.com/ --concurrency 10
```

**No subas la concurrencia en sitios de terceros ni en hosting compartido.** Un servidor con
limitación por IP empieza a devolver 503 tras varios rastreos seguidos, y a partir de ahí las
mediciones no valen nada: lo que estás midiendo es su defensa, no tu sitio.

### Solo una parte del sitio

```bash
# Solo el blog
crawlforge crawl https://cliente.com/ --include "/blog/"

# Todo menos el área de administración y las URLs de respuesta a comentarios
crawlforge crawl https://cliente.com/ --exclude "/wp-admin/" --exclude "\?replytocom="
```

Son expresiones regulares sin anclar: una cadena normal funciona como «contiene». **`--exclude`
gana sobre `--include`**, y lo excluido queda registrado como excluido en el informe: no
desaparece sin dejar rastro.

### Una prueba rápida, sin rastrear las 176.000 URLs

```bash
crawlforge crawl https://tumedio.com/ --max-urls 500
```

Ojo con esto: **un rastreo truncado no puede juzgar el grafo de enlaces.** La herramienta lo sabe
y silencia las reglas que dependen de tenerlo completo —páginas huérfanas, profundidad excesiva—
en lugar de inventarse el veredicto. Si el informe no las menciona, no es que estén bien; es que
no se han podido evaluar.

### Auditar antes de desplegar

Sobre la carpeta compilada, sin subir nada a ningún sitio:

```bash
crawlforge audit ./dist --base https://tusitio.com/
```

`--base` es obligatorio y por un motivo: los `canonical` absolutos del sitio se comparan contra
esa URL. Con una falsa, la auditoría de indexabilidad no significa nada, y la herramienta avisa
si el 80% de los canonicals contradicen lo que le has dicho.

Es el modo más rápido de todos —no hay red— y es el que se usa en CI.

### Comparar dos rastreos: ¿el despliegue ha empeorado algo?

```bash
crawlforge diff crawl-sitio.prev.sqlite crawl-sitio.sqlite --lang es
```

Esto es lo que Screaming Frog no te da. Un rastreo es una foto; el diff te dice si lo de ayer
sigue igual. Como puerta de CI, que falle si aparece algo grave:

```bash
crawlforge diff antes.sqlite despues.sqlite --fail-on high
```

Sale con código distinto de cero si aparece un hallazgo nuevo de severidad `high` o peor. Un
`--fail-on INDEX-NOINDEX` vigila una regla concreta.

### Un sitio protegido con contraseña (staging)

```bash
CRAWLFORGE_AUTH='usuario:contraseña' crawlforge crawl https://pre.cliente.es/
```

También se acepta en la URL como atajo, aunque así queda en el historial del shell:

```bash
crawlforge crawl https://usuario:contraseña@pre.cliente.es/
```

En los dos casos, **la credencial no se guarda en el fichero de rastreo** y no viaja a ningún host
que no sea el de la semilla, ni siquiera si el sitio enlaza a otro dominio. Por eso `resume`
también necesita la variable: el fichero no la lleva dentro a propósito.

Se aplica también al `robots.txt` y a los sitemaps — si no, un staging protegido devolvería 401 al
pedirlos y el rastreo se comportaría de forma rara sin decir por qué.

### Una lista exacta de URLs

```bash
crawlforge list urls.txt
```

Un fichero con una URL por línea. Sirve para revisar un conjunto concreto —las 40 landings de una
campaña— y es también el modo que hace justa una comparación con otra herramienta: las dos reciben
exactamente el mismo conjunto.

### Se ha cortado el rastreo

```bash
crawlforge resume crawl-tumedio-com.sqlite
```

Sigue exactamente donde se quedó, con la configuración guardada en el propio fichero. No repite lo
ya rastreado. Un rastreo terminado no se puede reanudar: para eso se relanza.

### Rastreos que se repiten: guarda la configuración

```bash
cp docs/crawl-config.example.yaml cliente.yaml   # y edítalo
crawlforge crawl https://cliente.com/ --config cliente.yaml
```

El fichero describe **el sitio**, la línea de comandos describe **la ejecución**: los flags ganan
sobre el YAML. Un campo mal escrito es un error, no una opción ignorada en silencio.

### Ver qué comprueba la herramienta

```bash
crawlforge rules --lang es                    # las 59, en tabla
crawlforge rules INDEX-ORPHAN-PAGE            # la ficha de una regla concreta, por su ID
crawlforge rules --lang es --category canonical
crawlforge rules --lang es --detail           # con la explicación completa de cada una
crawlforge rules --format json                # el catálogo entero como datos, en ambos idiomas
```

El JSON es para lo que consuma el catálogo como datos — un script de CI que comprueba que un
ID de regla sigue existiendo, o una página generada del catálogo en vez de copiarlo. Lleva
siempre los dos idiomas, y su sobre dice la versión del catálogo y el número de reglas.

---

## 3. Cómo se lee el resultado

### En el terminal

`report` sin más da el resumen: cuántas URLs, cuántas indexables, y los hallazgos agrupados por
severidad. Es para responder «¿cómo está esto?» en diez segundos.

### El XLSX

```bash
crawlforge export crawl.sqlite --format xlsx --out auditoria.xlsx
```

Trece hojas, cada una con la cabecera congelada y el autofiltro puesto. Los códigos de estado son
números de verdad, así que un filtro «mayor que 399» funciona como esperas. Una celda de estado
vacía es una URL que se registró y nunca se pidió: un enlace interno fuera del alcance del rastreo,
o uno externo con la comprobación apagada.

Está comprobado que abre limpio en Microsoft Excel 16, sin aviso de reparación.

### El SQLite, si quieres ir más allá

Es la ventaja de que el formato no sea propietario. Cualquier pregunta que el informe no responda,
la responde SQL.

Las tres tablas que se usan el 90% de las veces:

| Tabla | Qué guarda | Clave |
|---|---|---|
| `urls` | Toda URL vista: `url`, `status_code`, `depth`, `content_type`, `response_time_ms` | `id` |
| `pages` | Lo extraído del HTML: `title`, `h1`, `word_count`, `canonical`, `is_indexable` | `url_id` → `urls.id` |
| `issues` | Un hallazgo por fila: `rule_id`, `severity` | `url_id` → `urls.id` |

```bash
# Las URLs que fallan
sqlite3 crawl-cliente-com.sqlite \
  "SELECT url, status_code FROM urls WHERE status_code >= 400 ORDER BY status_code DESC LIMIT 20;"

# Qué páginas dispararon una regla concreta
sqlite3 crawl-cliente-com.sqlite \
  "SELECT u.url FROM issues i JOIN urls u ON u.id = i.url_id
   WHERE i.rule_id = 'CONTENT-H1-MISSING' LIMIT 20;"

# Las páginas más lentas
sqlite3 crawl-cliente-com.sqlite \
  "SELECT url, response_time_ms FROM urls ORDER BY response_time_ms DESC LIMIT 10;"
```

Hay vistas ya preparadas para las preguntas habituales: `v_orphans`, `v_broken_links`,
`v_indexable_pages` y `v_issue_summary`.

Dos avisos que ahorran un rato: la columna es **`status_code`**, no `status`, y está a `NULL` en
toda URL que se registró sin pedirse: un enlace interno al que el rastreo no llegó, o uno externo
si va `--no-external-check`.

---

## 3.bis Cómo se lee un informe

El resumen es una línea por regla, ordenado por severidad. Tres cosas que conviene saber para
interpretarlo:

### La cuota del sitio

```
medium  META-TITLE-TOO-LONG  173,654  (80% of the site)
```

Cuando una regla afecta al 40% o más de las páginas, la línea añade su cuota. **Es la diferencia
entre una lista de páginas que arreglar una a una y un problema sistémico**: 2.193 imágenes rotas
son 2.193 arreglos; 173.654 títulos largos al 80% del sitio son una plantilla o un patrón de
publicación, y se arregla en un sitio.

### Los problemas de plantilla

```
high  ASSET-IMG-EMPTY-ALT-LINK  13 template issues (567 pages) + 90 more findings
      e.g. https://ejemplo.com/a · https://ejemplo.com/b
```

Cuando el mismo defecto aparece por la misma causa en muchas páginas —el logo de la cabecera, un
enlace del pie, el `<h4>` de la firma del autor— se cuenta como **un problema**, con ejemplos. Las
filas siguen todas en el fichero: lo que cambia es el recuento del informe, no lo que se guarda.

### Ver todas las URLs de una regla

El resumen nunca enumera. Para la lista completa:

```bash
crawlforge report crawl.sqlite --rule HTTP-404-INTERNAL
```

Sale ordenada, con los grupos de plantilla primero y su causa al lado. Es el comando que sustituye
al «… y 26 más» que no llevaba a ninguna parte.

### La ficha de una URL: ¿quién enlaza aquí?

La pregunta que más se repite en una auditoría tiene su propio comando:

```bash
crawlforge inspect crawl.sqlite 'https://cliente.com/pagina/'
```

Vale también la ruta sola (`/pagina/`), el dominio sin esquema, y con o sin la barra final. Si te
equivocas, el error sugiere las URLs más parecidas del fichero en vez de decir solo «no está».

La ficha enseña el estado HTTP, lo extraído (título, meta description, H1, palabras, canonical,
indexabilidad), los hallazgos de esa URL, su cadena de redirecciones si redirige, sus imágenes, y
—la sección estrella— **quién enlaza a esa página**: deduplicado por página que enlaza, con su
texto de ancla, si es `nofollow` y desde qué región (los enlaces de contenido primero, el ruido de
`nav` y `footer` después). Salida real, recortada:

```
$ crawlforge inspect crawl-tumedio-com.sqlite '/nueva-piscina-municipal-abre-en-junio'

── Inlinks (24) ─────────────────────────────
  By region: unknown 13 · main 11 · 0 nofollow
  Linking pages, content links first:
    main     "la nueva piscina abre en junio" — https://tumedio.com/obras-del-polideportivo-terminadas/
    main     "la piscina municipal" — https://tumedio.com/presupuesto-municipal-2026/
    unknown  (no anchor text) ×4 — https://tumedio.com/tag/deportes/

── Outlinks (156: 108 internal, 48 external) ─
     200  https://tumedio.com/quienes-somos/ "¿Quiénes somos?"
```

Los salientes enseñan el **código de estado del destino** con los rotos primero: la ficha de una
página es también su triaje de enlaces rotos. Y si la URL inspeccionada es una imagen, la ficha
dice en qué páginas se usa — la dirección contraria a la sección de imágenes.

Cada lista corta en 20 filas y el corte dice el comando exacto que la completa (`--limit all`;
también acepta un número). `--lang es` la traduce, y `--format md` con `--out ficha.md` produce
una ficha para pegar en un ticket:

```bash
crawlforge inspect crawl.sqlite '/pagina/' --format md --out ficha.md
```

---

## 3.ter Escenarios completos

### Auditoría de un cliente, de principio a fin

```bash
crawlforge crawl https://cliente.com/
crawlforge report crawl-cliente-com.sqlite --lang es          # ¿qué tiene?
crawlforge report crawl-cliente-com.sqlite --rule CONTENT-H1-MISSING   # ¿dónde?
crawlforge inspect crawl-cliente-com.sqlite '/esa-pagina/' --lang es   # ¿quién enlaza aquí?
crawlforge export crawl-cliente-com.sqlite --format xlsx --out cliente.xlsx
crawlforge report crawl-cliente-com.sqlite --format html --out cliente.html --lang es
```

El `.xlsx` es para trabajar; el `.html` es para enviar.

### Vigilar un despliegue

```bash
crawlforge crawl https://cliente.com/                         # antes de publicar
# … se publica …
crawlforge crawl https://cliente.com/                         # el anterior pasa a .prev.sqlite
crawlforge diff crawl-cliente-com.prev.sqlite crawl-cliente-com.sqlite --lang es
```

Y en un pipeline de CI, sobre la carpeta compilada y sin red:

```bash
crawlforge audit ./dist --base https://cliente.com/ --out nuevo.sqlite
crawlforge diff referencia.sqlite nuevo.sqlite --fail-on high || exit 1
```

### Revisar una cartera de sitios

```bash
for s in blog1.com blog2.com blog3.com; do
  crawlforge crawl "https://$s/" --out "cartera/$s.sqlite"
done
crawlforge portfolio ./cartera --lang es
```

Un solo panel sobre todos los ficheros: qué cambió desde el rastreo anterior de cada sitio,
qué reglas fallan en cuántos sitios, y una línea por sitio. Entero en §3.quater.

### Un conjunto concreto de URLs

```bash
printf '%s\n' https://cliente.com/landing-a https://cliente.com/landing-b > urls.txt
crawlforge list urls.txt --lang es
```

---

## 3.quater La cartera: muchos sitios a la vez

Una auditoría suelta es una foto. Quien lleva muchos sitios necesita otras dos respuestas:
**qué se rompió desde la semana pasada** y **qué falla en todos a la vez**. Eso es
`portfolio`:

```bash
crawlforge portfolio ./rastreos/               # un directorio se recorre buscando *.sqlite
crawlforge portfolio a.sqlite b.sqlite c.sqlite
```

Los `.prev.sqlite` que hay junto a tus rastreos **no** cuentan como sitios: cada uno es el
«antes» del rastreo de al lado, y el panel compara la pareja solo. Es el mismo fichero que
usa `diff`, producido de la misma manera: repitiendo el rastreo sobre el mismo fichero de
salida.

Salida real, recortada (una cartera de prueba de cinco sitios; dos ficheros los rastreó una
versión anterior, un rastreo quedó truncado y otro es de modo lista):

```
$ crawlforge portfolio ./cartera --lang es

── Panel de cartera ─────────────────────────
  5 sitios · rastreos del 2026-08-04 al 2026-08-04

── Avisos ───────────────────────────────────
  AVISO     No todos los sitios se rastrearon con el mismo catálogo de reglas (0.4.0,
            0.6.2). Una regla puede faltar en un sitio porque no existía cuando se rastreó.

── Qué cambió ───────────────────────────────
  1 de 5 sitios tiene un rastreo anterior (.prev.sqlite) con el que comparar.

  Hallazgos nuevos críticos y altos:
    https://alpha.example/
      crítico   HTTP-404-INTERNAL                   2
        https://alpha.example/p/000005/
        https://alpha.example/p/000006/

  El resto, sitio a sitio:
    https://alpha.example/
      Hallazgos resueltos 2 · Códigos de estado que empeoran 2

── Qué falla en toda la cartera ─────────────
  Una regla que salta en la mayoría de los sitios rara vez es contenido: suele ser una
  plantilla o un plugin compartido — un arreglo que sirve para todos.

  medio     CANON-CROSS-DOMAIN             3 de 5 sitios
  crítico   HTTP-NO-HTTPS                  2 de 5 sitios
  medio     INDEX-DEEP-PAGE                1 de 5 sitios (2 no concluyentes)

── La cartera de un vistazo ─────────────────
       URLs  index.  crit  high   med   low  info  rastreado   sitio
        240       0     2     0   118     0     0  2026-08-04  https://alpha.example/
          8       0     1     1     7     0     0  2026-08-04  http://127.0.0.1:8912/  (truncado)
```

Tres cosas de esa salida son deliberadas, y son las que hacen fiable el panel:

- **«1 de 5 sitios (2 no concluyentes)»** — un rastreo truncado o de modo lista nunca evaluó
  las reglas que necesitan el grafo de enlaces completo, así que para esas reglas el panel
  separa tres estados: dispara, no dispara, y **no se pudo evaluar**. Una regla que no
  aparece en un sitio truncado no es una regla que ahí pase.
- **El aviso del catálogo va arriba.** Ficheros rastreados con catálogos de reglas distintos
  no se comparan en silencio: una regla puede «faltar» en un sitio porque aún no existía.
- **El rango de fechas se dice siempre**, y si entre el rastreo más viejo y el más nuevo hay
  más de una semana el panel lo avisa: eso no es una foto de la cartera, y «qué cambió»
  cubriría un periodo distinto en cada sitio.

Un fichero que no se puede abrir —no es un rastreo, es una base de otro programa, tiene un
esquema más nuevo que el binario— sale en «Ficheros apartados» con su motivo, y el resto del
panel se produce igual. Un fichero malo no te cuesta los otros once.

Lo demás funciona como `report`: `--lang es` traduce el panel, y `--format md` o
`--format html` con `--out` producen un fichero para pegar en un ticket o enviar:

```bash
crawlforge portfolio ./cartera --format html --out panel.html --lang es
```

El panel no es del nivel gratuito. La CLI corre por defecto como el nivel más alto; solo
importa si defines `CRAWLFORGE_TIER` (ver §5).

---

## 4. Lo que hoy **no** hace

Dicho por delante, para que no pierdas tiempo buscándolo:

- **No renderiza JavaScript.** Un sitio cuyo contenido se monta en el navegador se verá vacío.
  Está previsto.
- **No sigue los sitios externos.** De los enlaces salientes se comprueba el estado —que es lo que
  hace saltar `HTTP-404-EXTERNAL`—, pero de otro dominio no se parsea ni se rastrea nada. **No hay
  flag `--follow-external`**: rastrear entero un sitio ajeno no es algo que la herramienta ofrezca
  desde la línea de comandos. La clave `follow_external` existe en el fichero de configuración y
  sigue apagada por defecto; una reanudación ignora lo que el fichero diga de ella, igual que
  ignora un `ignore_robots` guardado.
- **No hay interfaz gráfica** todavía.
- **No hay programación de rastreos.** El panel de cartera (§3.quater) lee los ficheros que
  ya tienes; producirlos a un ritmo sigue siendo trabajo de tu cron.
- **`HTTP-TEMP-REDIRECT`** no existe todavía: necesita un histórico de rastreos que aún no existe.

Del catálogo gratuito están implementadas 59 de 60 reglas.

---

## 5. Cuando algo va mal

| Síntoma | Qué pasa |
|---|---|
| El rastreo acaba con muchas menos URLs de las que esperas | Algo lo cortó: `--max-urls`, `--max-depth`, `--max-duration`, o el tope de 1.000 del nivel gratuito si está puesto `CRAWLFORGE_TIER=free`. El informe dice que el rastreo quedó truncado y por qué límite. |
| Un `report` menciona hallazgos «no evaluados» | El rastreo se truncó y las reglas que necesitan el grafo completo se han silenciado a propósito. |
| El sitio devuelve 429 o 503 | Baja `--concurrency`. La herramienta respeta el `Crawl-delay` del `robots.txt`, pero un WAF puede ser más estricto que el robots. |
| «no such file» al hacer `diff` | Falta el `.prev.sqlite`: solo aparece al **repetir** un rastreo sobre el mismo fichero de salida. |

| El programa parece colgado tras rastrear | Está en la pasada final: enlaces entrantes y reglas de conjunto. Dice por qué regla va (`final pass · rule 7/29 · …`). En sitios grandes tarda minutos. |
| Un rastreo antiguo no se puede reanudar | Solo se rechaza si el fichero es **más nuevo** que el programa, o si le falta cruzar una migración que cambia lo que el motor escribe. Se sigue abriendo con `report`, `export` y `diff`. |
| Aparecen `.sqlite-wal` y `.sqlite-shm` al lado | El rastreo no cerró limpiamente. **No copies solo el `.sqlite`**: te faltarían datos. Vuelve a abrirlo con `crawlforge report` para que los consolide. |

Los errores dicen qué fichero falta y qué comando lo genera. Si alguno no lo dice, es un fallo del
producto y merece anotarse.

---

## 6. Referencia rápida

```bash
crawlforge crawl  <URL>       # rastrear por HTTP
crawlforge audit  <DIR> --base <URL>   # auditar una carpeta compilada
crawlforge list   <FICHERO>   # una lista exacta de URLs
crawlforge resume <FICHERO>   # continuar un rastreo cortado
crawlforge report <FICHERO>   # resumen, o --format md|html
crawlforge export <FICHERO> --format xlsx --out a.xlsx
crawlforge diff   <ANTES> <DESPUES> [--fail-on high]
crawlforge portfolio <RUTA>... [--format md|html --out f]   # panel sobre muchos rastreos
crawlforge rules  [--category X] [--detail] [--format json]
```

Cualquiera de ellos con `--help` da la lista completa de opciones.
