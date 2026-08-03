# CrawlForge

**Auditoría SEO técnica para quien gestiona muchos sitios, no uno.**

CrawlForge rastrea un sitio propio, extrae lo que importa para el posicionamiento, evalúa 59 reglas
de auditoría y lo guarda todo en un único fichero SQLite. Después puede decirte qué cambió desde el
rastreo anterior, que es lo que convierte una auditoría suelta en algo que puedes repetir cada
semana.

```
$ crawlforge diff antes.sqlite despues.sqlite

── Ha mejorado ──────────────────────────────
  + 11  HTTP-404-INTERNAL  resueltos
        404 → 301  /googlechrome
        404 → 301  /internetexplorer

── Ha empeorado ─────────────────────────────
  −  1  HTTP-REDIRECT-CHAIN  nueva
        /googlechrome → artículo → versión móvil
```

Once cosas arregladas, una creada. Ese es el informe que una foto no puede darte.

---

## Qué hace

**Rastrea de tres maneras.** Por HTTP, contra una carpeta compilada (`dist/`, antes de publicar) o
sobre una lista exacta de URLs.

**Evalúa 59 reglas**, cada una con su caso de prueba: indexabilidad, estado HTTP, meta, canonical,
contenido, recursos y hreflang. Las reglas son el producto: si una se equivoca, dejas de fiarte del
informe entero.

**Compara dos rastreos** y dice qué se resolvió y qué apareció. Con `--fail-on high` sirve de
puerta en integración continua: falla el build si un despliegue introduce algo grave.

**Guarda todo en SQLite normal.** Sin formato propietario. Cualquier pregunta que el informe no
responda, la responde SQL.

## Instalación

Necesita Rust 1.85 o posterior.

```bash
git clone https://github.com/dcarrero/crawlforge
cd crawlforge
cargo build --release -p crawlforge-cli
cp target/release/crawlforge ~/.cargo/bin/
```

Funciona en macOS, Linux y Windows.

## Cinco minutos

```bash
# Rastrear un sitio
crawlforge crawl https://ejemplo.com/

# Ver qué ha encontrado
crawlforge report crawl-ejemplo-com.sqlite --lang es

# Todas las URLs afectadas por una regla
crawlforge report crawl-ejemplo-com.sqlite --rule CONTENT-H1-MISSING

# ¿Quién enlaza a esta página?
crawlforge inspect crawl-ejemplo-com.sqlite '/precios/'

# Auditar una carpeta compilada antes de publicar: sin red, en segundos
crawlforge audit ./dist --base https://ejemplo.com/

# Dárselo a alguien que no usa terminal
crawlforge export crawl-ejemplo-com.sqlite --format xlsx --out auditoria.xlsx
```

Cada comando termina diciendo cuál es el siguiente. Guía completa en
[`docs/MANUAL.md`](docs/MANUAL.md).

**La salida de la herramienta está en inglés.** Es el idioma de origen; el español existe donde hay
texto de verdad que traducir —el catálogo de reglas y los informes— y se pide con `--lang es`.

## Mediciones, con sus condiciones

Un número vale lo que valga su método, así que aquí va el método al lado de cada uno.

**487.621 URLs en un solo rastreo con la memoria plana.** Un medio de comunicación con quince años
de archivo, 4,4 millones de imágenes, un fichero de 5,3 GB. La memoria sigue a la cola de
pendientes, no al tamaño del sitio: llegó a 259 MB con 155.000 URLs en cola y bajó a 123 MB cuando
la cola se vació.

**Cero diferencias de extracción en 1.800 comparaciones** frente a otro rastreador consolidado. Las
mismas 300 URLs a las dos herramientas en modo lista: código de estado, título, meta description,
H1, canonical e indexabilidad. La única diferencia que apareció era nuestra, y está
[en el historial](CHANGELOG.md): un `<br>` dentro de un `<h1>` juntaba las palabras de los extremos.

**59 reglas, cada una con fixture y test.** Hay un test que falla si alguna se publica sin él.

Cualquier otra cosa que leas sobre velocidad debería venir también con sus condiciones. Comparar
ejecuciones con concurrencias distintas, en máquinas distintas o sobre sitios distintos no es
comparar.

## Cómo está construido

```
crates/
  crawlforge-core/      rastreo, parseo, motor de reglas, almacén SQLite
  crawlforge-rules/     las 59 reglas de auditoría
  crawlforge-cli/       el binario
  crawlforge-adapters/  WordPress y Astro
  crawlforge-ffi/       bindings C y Swift
```

`crawlforge-core` no conoce ninguna interfaz. Se compila y se prueba solo.

Las decisiones de diseño y las convenciones están en [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md).
Léelo antes de abrir un pull request: explica las decisiones que el código da por sabidas.

## Lo que no hace

Dicho por delante, para que no lo busques:

- **No renderiza JavaScript.** Un sitio que monta su contenido en el navegador se verá vacío. Está
  previsto.
- **No comprueba enlaces externos rotos.** Los salientes se registran, no se piden.
- **No tiene interfaz gráfica** todavía.
- **No programa rastreos ni tiene panel de cartera.** Eso vive en las apps de pago, cuando existan.

## Licencia y qué se paga

El rastreador, las reglas, la línea de comandos y los adaptadores son **Apache 2.0**. Úsalos,
bifúrcalos, mételos dentro de tus propias herramientas.

Las aplicaciones nativas de escritorio y el servicio de sincronización de cartera no son de código
abierto y serán de pago. Una herramienta que te pide las credenciales de tu *staging* debería poder
leerse; la interfaz que va encima es el producto.

## Contribuir

Las reglas son el mejor sitio para empezar, y [`CONTRIBUTING.md`](CONTRIBUTING.md) explica el único
requisito que no se negocia: **una regla llega con su fixture, su test y su texto en inglés y en
español.** Una regla sin caso de prueba es una regla de la que nadie se puede fiar, tú incluido.

---

English version: [`README.md`](README.md)
