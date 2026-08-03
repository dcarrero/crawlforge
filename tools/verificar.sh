#!/usr/bin/env bash
#
# Lo mismo que hace el CI, en local y antes de subir.
#
# Existe por dos motivos. El primero es práctico: enterarse de que clippy protesta cuando ya has
# hecho `push` cuesta un viaje de ida y vuelta. El segundo es que **el CI de GitHub Actions no
# está operativo en este repositorio**: los trabajos se encolan y mueren en doce segundos sin
# asignarse a ninguna máquina (`runner_name` vacío, cero pasos ejecutados), que es como se
# manifiesta un repositorio privado sin minutos de Actions disponibles. El fichero
# `.github/workflows/ci.yml` está escrito y es correcto; en cuanto la cuenta tenga minutos, corre
# solo. Mientras tanto, esto.
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
