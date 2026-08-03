# Convenciones y decisiones cerradas

> Este documento es el contexto que el código da por sabido. Los comentarios del motor lo citan por
> sección —`CONVENTIONS.md §4`— así que **las secciones no se renumeran**: se añaden al final.

---

## 1. Qué es CrawlForge

Un auditor SEO técnico. Rastrea un sitio propio, extrae lo que importa para el posicionamiento,
evalúa un catálogo de reglas y guarda todo en un fichero SQLite.

Lo que lo diferencia de las herramientas de rastreo al uso no es la velocidad, sino **la gestión de
una cartera de sitios y la comparación entre rastreos**. Una auditoría suelta es una foto; quien
lleva decenas de proyectos necesita saber qué cambió desde la anterior.

**Es un auditor de sitios web propios.** No es una herramienta de extracción de datos ajenos: el
producto solo rastrea el sitio que se le indica, respeta `robots.txt` salvo permiso explícito del
dueño, y no persigue enlaces externos por defecto. Esa restricción es de diseño y condiciona el
código, no solo el discurso.

## 2. Decisiones cerradas

Estas no se replantean sin instrucción explícita.

**2.1 · El almacén de un rastreo es un fichero SQLite, siempre.** No es configurable. Es escritura
masiva de un solo escritor: contra una base de datos remota sería entre 20 y 50 veces más lento.
Además da FTS5, WAL y la propiedad que da nombre a todo lo demás: *un rastreo es un fichero
portable*.

**2.2 · SQLite es también la frontera entre el motor y la interfaz.** El core escribe; la interfaz
lee **el mismo fichero en solo lectura**, mientras se escribe. No es un caso raro que haya un lector
concurrente: **es la arquitectura**. Todo lo que cierre o mueva el fichero tiene que tolerarlo.

**2.3 · Un rastreo es un fichero portable.** Quien copia el `.sqlite` a otra máquina se lleva el
rastreo entero. Por eso el cierre limpio saca el fichero del modo WAL: si quedan ficheros `-wal`
sueltos, copiar solo el `.sqlite` pierde datos en silencio, y eso rompe la promesa de raíz.

**2.4 · El core no conoce ninguna interfaz.** Se compila y se prueba solo. Si hace falta un tipo de
interfaz dentro del core, el diseño está mal.

## 3. Stack

| Capa | Elección | Nota |
|---|---|---|
| Async | `tokio` | |
| HTTP | `reqwest` con rustls | rustls evita el infierno de compilación multiplataforma de OpenSSL |
| Parseo HTML | `lol_html` | Streaming. **No `scraper`**: construir el DOM completo es 5-10x más lento |
| robots.txt | `texting_robots` | |
| Rate limiting | `Throttle` propio | Un limitador por host, con freno adaptativo ante 429 y 503 |
| Almacén | `rusqlite` (bundled, WAL, FTS5) | |
| Serialización | `serde` | |
| Export | `rust_xlsxwriter`, `csv` | |
| Errores | `thiserror` en el core, `anyhow` en la CLI | |
| Logs | `tracing` | |

**El stack está cerrado.** Añadir una dependencia para algo que se resuelve en veinte líneas es una
decisión con coste: cada crate es superficie de auditoría, tiempo de compilación y una versión que
mantener. Por eso la CLI tiene su propio directorio temporal en vez de `tempfile`, y por eso el
base64 de la autenticación básica está escrito a mano.

## 4. Convenciones

**Idioma.** Los comentarios de decisión van en español. **Los identificadores, nombres de función,
tipos, columnas de base de datos, IDs de regla y mensajes de commit van en inglés.**

**El inglés es el idioma de origen del producto y el español una traducción**, no al revés. Es el
orden en que se publica y el que decide qué texto manda cuando los dos discrepan. La salida de la
línea de comandos es toda en inglés por coherencia —la plantilla del parser de argumentos y sus
errores no son localizables—; el español vive donde hay texto de verdad que traducir, el catálogo
de reglas y los informes, y se pide con `--lang es`.

Los nombres de los tests van en español, como los comentarios: describen una afirmación, no una API.

**Sin `unwrap()` ni `expect()`** fuera de tests y de `main`.

**Cada regla del catálogo necesita un fixture y un test. Sin excepción** — las reglas son el
producto: si una se equivoca, el usuario deja de fiarse del informe entero.

**Los errores de red por URL no abortan el rastreo.** Se guardan en la fila de esa URL y se sigue.

**Escrituras a SQLite por lotes desde un único hilo escritor** que consume un canal acotado. Nunca
desde los workers. El canal está acotado a propósito: la contrapresión es la función, no un efecto
secundario. Con un canal sin límite la cola se convierte en el almacén y el pico de memoria pasó,
medido, de 170 a 387 MB.

**Migraciones numeradas y hacia adelante.** Nunca se edita una publicada. Un rastreo de hace un año
debe seguir abriéndose.

**Conventional Commits**: `feat(core):`, `fix(rules):`, `docs:`.

## 4.bis Versionado

Semántico, y con el major en 0 la API no es estable. En la práctica:

- **Patch** — un arreglo que no cambia ningún comportamiento del que alguien pueda depender.
- **Minor** — una regla nueva, un comando, un flag, o un arreglo que cambia lo que una regla
  reporta. **Que una regla empiece o deje de dispararse es minor**, nunca patch: la puerta de
  integración continua de alguien depende de eso.
- **Major** — un cambio de esquema que los binarios anteriores no puedan abrir, o quitar un
  comando. `1.0.0` significará que el esquema y los IDs de regla son estables, así que **el salto
  se consulta antes de darlo**.

**Los IDs de regla no cambian de significado nunca.** Un diff histórico entre dos rastreos depende
de ello.

## 5. Antipatrones

Errores que este proyecto no puede permitirse:

1. **Cargar el resultado del rastreo entero en memoria para enseñarlo.** Siempre consulta paginada
   y tabla virtualizada. Es exactamente donde fallan las herramientas que cargan todo en RAM, y es
   la razón de existir de este producto.
2. **Almacén de rastreo en memoria.** Ver el punto anterior.
3. **Hacer configurable el motor de base de datos del rastreo.** Ver §2.1.
4. **Acumular resultados en un vector antes de escribirlos.** Aplica dentro del motor y también en
   la pasada final: sobre 500.000 URLs, las reglas de duplicados producen 971.000 hallazgos, y solo
   el vector son 330 MB. Se escribe por lotes según se evalúa.
5. **Abstraer prematuramente sobre varios dialectos de SQL.** `rusqlite` en el camino caliente y
   nada más.
6. **Un `JOIN` por fila en el camino de escritura.** Los extremos de `links` e `images` se resuelven
   contra un índice en memoria; volver al `JOIN` cuesta millones de búsquedas por índice en un sitio
   mediano.
7. **Una regla que afirma lo que no puede saber.** Si el rastreo está truncado, las reglas que
   dependen del grafo completo callan. Es preferible no decir nada a decir algo falso.

## 6. Cómo se prueba

`cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings` antes de cada
cambio. La regresión de rendimiento **solo afirma en `--release`**, y su suelo depende del entorno:
la misma versión da unas cinco veces más elementos por segundo en un portátil de desarrollo que en
un runner compartido de integración continua.

Y lo que ningún test sustituye: **rastrear un sitio real y comprobar si lo que la herramienta dice
es verdad**. Los defectos de bulto de este proyecto —falsos positivos sistemáticos, índices que
faltaban, extracción incorrecta— aparecieron todos ejecutando, no leyendo. A mil filas cualquier
plan de consulta parece bueno.
