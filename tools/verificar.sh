#!/usr/bin/env bash
#
# Lo mismo que hace el CI, en local y antes de subir.
#
# Existe por un motivo práctico: enterarse de que clippy protesta cuando ya has hecho `push`
# cuesta un viaje de ida y vuelta.
#
# Desde que el repositorio es público (2026-08-10) el CI de GitHub Actions sí corre en cada push,
# así que esto ya no lo sustituye: lo adelanta. Mientras fue privado, los trabajos se encolaban y
# morían en doce segundos sin asignarse a ninguna máquina —`runner_name` vacío, cero pasos—, que
# es como se manifiesta una cuenta sin minutos de Actions.
#
# La regresión de rendimiento se ejecuta **siempre**, y en `--release`. No es opcional por una
# razón aprendida a base de romperlo: un arreglo de seguridad de tres líneas hundió el modo
# `filesystem` 10,7x y lo dejó por debajo de su propia puerta, el test de regresión estaba escrito
# precisamente para cazar eso, y no saltó porque solo afirma con optimizaciones y la verificación
# se había hecho en debug. Un test que no se ejecuta donde mide no protege de nada.
#
# Uso:
#   tools/verificar.sh          # todo, antes de cada push (~2 min)
#   tools/verificar.sh --rapido # sin la regresión, para iterar

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

RAPIDO=0
[ "${1:-}" = "--rapido" ] && RAPIDO=1

paso() {
    printf '\n\033[1m── %s\033[0m\n' "$1"
}

paso "Tests"
cargo test --workspace

paso "Clippy"
cargo clippy --workspace --all-targets -- -D warnings

paso "Banco de fixtures"
# Redundante con `cargo test --workspace`, pero se ejecuta aparte y con salida visible porque es
# el test que dice si el catálogo de reglas sigue detectando lo que dice detectar.
cargo test -p crawlforge-core --test fixtures_de_reglas -- --nocapture 2>&1 | tail -3

paso "Artefactos publicados"
# `rules.json` y `meta.json` van versionados porque el repositorio de la web los consume
# instalando este por tag. Si el catálogo cambia y nadie los regenera, la web publica un catálogo
# que ya no existe — y como se instalan por tag, el error viaja congelado hasta que alguien lo ve.
tools/gen-artefactos-web.sh > /dev/null
if ! git diff --quiet -- rules.json meta.json 2>/dev/null; then
    printf '\033[31m  ✗ rules.json o meta.json estaban desfasados; se han regenerado.\033[0m\n'
    printf '\033[33m    Revisa el diff y añádelos al commit.\033[0m\n'
    git --no-pager diff --stat -- rules.json meta.json
    exit 1
fi
printf '  \033[32m✓\033[0m rules.json y meta.json al día\n'

paso "Web sincronizada con el catálogo"
# La referencia de reglas de la web se genera del catálogo (`rules --format json`). Si una
# regla entra o cambia y nadie regenera, esto es lo que lo dice antes que un lector.
#
# El paso es condicional porque este fichero se publica y la web no: en el repositorio público
# no hay `comprobar-web.sh`, y con `set -e` esa línea mataba la verificación entera antes de
# llegar a la regresión de rendimiento. Quien clonara el proyecto para contribuir se encontraba
# con que el comando que la documentación manda ejecutar falla siempre.
if [ -x tools/comprobar-web.sh ]; then
    tools/comprobar-web.sh
else
    printf '  no aplica: la web no forma parte de este repositorio\n'
fi

if [ "$RAPIDO" = "0" ]; then
    paso "Regresión de rendimiento (release)"
    # Solo afirma sobre los números compilado con optimizaciones; en debug el motor va un orden de
    # magnitud más lento. Ver la cabecera de `tests/regresion_rendimiento.rs`.
    cargo test --release -p crawlforge-core --test regresion_rendimiento -- --nocapture 2>&1 \
        | grep -E "elementos/s|test result"
else
    printf '\n\033[33mSaltada la regresión de rendimiento (--rapido). No subas así.\033[0m\n'
fi

printf '\n\033[32mTodo en verde.\033[0m\n'
