#!/usr/bin/env bash
#
# Suite de aceptación completa, para ejecutar en este Mac.
#
# `verificar.sh` es lo que se pasa antes de cada push: rápido, sobre el código. Esto es lo que se
# pasa antes de dar una fase por terminada: además de compilar y testear, **usa la herramienta**
# —rastrea, exporta, informa, compara, reanuda— y comprueba el resultado con otras herramientas,
# no con las suyas propias. La diferencia importa: todos los defectos de bulto de este proyecto
# aparecieron ejecutando, no leyendo.
#
# Solo macOS de momento. El pipeline de las tres plataformas está en `.github/workflows/ci.yml`
# y no se ejecuta: el repositorio es privado y no hay minutos de Actions. Cuando los haya, esto
# sigue valiendo para el equipo de desarrollo.
#
# Uso:
#   tools/suite.sh            # todo (~4 min)
#   tools/suite.sh --sin-red  # se salta el rastreo de un sitio real
#
# El único requisito además de Rust es `python3` con `openpyxl`, que se usa para abrir el `.xlsx`
# con una implementación ajena y comprobar que lo que escribimos se lee de verdad.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
RAIZ="$PWD"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

SIN_RED=0
[ "${1:-}" = "--sin-red" ] && SIN_RED=1

FALLOS=0
paso()  { printf '\n\033[1m── %s\033[0m\n' "$1"; }
ok()    { printf '  \033[32m✓\033[0m %s\n' "$1"; }
mal()   { printf '  \033[31m✗\033[0m %s\n' "$1"; FALLOS=$((FALLOS + 1)); }
nota()  { printf '  \033[33m•\033[0m %s\n' "$1"; }

# ---------------------------------------------------------------- 1. El código

paso "Código"
if cargo test --workspace --quiet >/dev/null 2>&1; then
    N=$(cargo test --workspace 2>&1 | grep -oE '^test result: ok\. [0-9]+' | grep -oE '[0-9]+' \
        | awk '{s+=$1} END {print s}')
    ok "$N tests en verde"
else
    mal "la suite de tests falla — ejecuta 'cargo test --workspace' para ver cuál"
fi

if cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1; then
    ok "clippy limpio"
else
    mal "clippy protesta — ejecuta 'cargo clippy --workspace --all-targets -- -D warnings'"
fi

# ---------------------------------------------------------------- 2. El rendimiento

paso "Rendimiento (release)"
SALIDA=$(cargo test --release -p crawlforge-core --test regresion_rendimiento -- --nocapture 2>&1)
if grep -q "test result: ok" <<<"$SALIDA"; then
    ok "$(grep -oE 'elementos/s [0-9]+ · páginas/s [0-9]+ · RSS [0-9.]+ MB' <<<"$SALIDA" | head -1)"
else
    mal "regresión de rendimiento: el motor ha perdido velocidad o memoria"
    grep -E "regresión|panicked" <<<"$SALIDA" | head -3
fi

# ---------------------------------------------------------------- 3. Las reglas

paso "Catálogo de reglas"
cargo build --release -p crawlforge-cli >/dev/null 2>&1
CF="$RAIZ/target/release/crawlforge"
REGLAS=$("$CF" rules 2>/dev/null | tail -1)
[ -n "$REGLAS" ] && ok "$REGLAS" || mal "el catálogo no se puede listar"

if cargo test -p crawlforge-core --test fixtures_de_reglas --quiet >/dev/null 2>&1; then
    ok "cada regla dispara su fixture al rastrearlo de verdad"
else
    mal "alguna regla ha dejado de detectar lo que dice detectar"
fi

# ---------------------------------------------------------------- 4. El flujo completo

paso "Flujo de trabajo, de principio a fin"
SITIO="$TMP/sitio"
cp -r "$RAIZ/crates/crawlforge-rules/fixtures/INDEX-ORPHAN-PAGE" "$SITIO"
cd "$TMP"

if "$CF" audit "$SITIO" --base https://fixture.local/ --out a.sqlite >/dev/null 2>&1; then
    ok "audit de un directorio"
else
    mal "audit falla"
fi

# El «despliegue»: alguien deja un noindex donde no debe.
python3 - "$SITIO/enlazada/index.html" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read().replace('<meta charset="utf-8">',
                           '<meta charset="utf-8">\n<meta name="robots" content="noindex">', 1)
open(p, "w").write(s)
PY

"$CF" audit "$SITIO" --base https://fixture.local/ --out a.sqlite >/dev/null 2>&1
[ -f a.prev.sqlite ] && ok "el rastreo anterior se conserva como .prev" \
                     || mal "no se conservó el rastreo anterior: el diff se queda sin «antes»"

"$CF" diff a.prev.sqlite a.sqlite --fail-on critical >/dev/null 2>&1
[ $? -ne 0 ] && ok "diff --fail-on detecta el noindex y sale con error (puerta de CI)" \
             || mal "diff --fail-on no detectó un noindex nuevo"

"$CF" export a.sqlite --format xlsx --out a.xlsx >/dev/null 2>&1 && ok "export a XLSX" || mal "export a XLSX falla"
"$CF" report a.sqlite --format html --out a.html >/dev/null 2>&1 && ok "informe HTML" || mal "informe HTML falla"

# ---------------------------------------------------------------- 5. El XLSX, con ojos ajenos

paso "El XLSX se lee con otra implementación"
if python3 -c "import openpyxl" 2>/dev/null; then
    python3 - "$TMP/a.xlsx" <<'PY'
import sys, openpyxl
wb = openpyxl.load_workbook(sys.argv[1])
hojas = wb.sheetnames
problemas = [n for n in hojas if not wb[n].freeze_panes or not wb[n].auto_filter.ref]
print(f"  \033[32m✓\033[0m openpyxl lo abre sin reparar nada: {len(hojas)} hojas")
if problemas:
    print(f"  \033[31m✗\033[0m sin cabecera fija o sin filtro: {problemas}")
    sys.exit(1)
print("  \033[32m✓\033[0m cabecera congelada y autofiltro en todas")
wb.close()
PY
    [ $? -ne 0 ] && FALLOS=$((FALLOS + 1))
else
    nota "sin openpyxl (pip3 install openpyxl): no se puede validar el .xlsx con ojos ajenos"
fi

# ---------------------------------------------------------------- 6. Contra un sitio real

if [ "$SIN_RED" = "0" ]; then
    paso "Contra un sitio real"
    # `SITIO_REAL` permite apuntar a un sitio propio. Por defecto va a example.com, que es el
    # dominio reservado para ejemplos y no molesta a nadie — pero tiene tres páginas, así que
    # para probar de verdad conviene poner uno tuyo.
    if "$CF" crawl "${SITIO_REAL:-https://example.com/}" --max-urls 40 --concurrency 3 --out real.sqlite \
        >/dev/null 2>&1; then
        HALLAZGOS=$("$CF" report real.sqlite 2>/dev/null | grep -cE "^    (critical|high|medium|low)")
        ok "rastreo real completado, $HALLAZGOS reglas con hallazgos"
    else
        mal "el rastreo de un sitio real falla"
    fi

    # Un dominio que no existe tiene que fallar, no devolver una auditoría vacía.
    "$CF" crawl https://este-dominio-no-existe-crawlforge.invalid/ --out muerto.sqlite >/dev/null 2>&1
    [ $? -ne 0 ] && ok "un dominio inexistente falla en vez de fingir una auditoría" \
                 || mal "un dominio inexistente devolvió éxito"
else
    nota "saltado el rastreo real (--sin-red)"
fi

# ---------------------------------------------------------------- Veredicto

cd "$RAIZ"
printf '\n'
if [ "$FALLOS" -eq 0 ]; then
    printf '\033[32m── Suite completa en verde ─────────────────\033[0m\n'
    printf 'Lo que esto NO comprueba:\n'
    printf '  · Windows y Linux: el pipeline está escrito y necesita minutos de Actions.\n'
    printf '  · Usarla una semana de verdad y anotar cada vez que acabes abriendo Screaming Frog.\n'
    printf '\n'
    printf 'El .xlsx se abrió limpio en Microsoft Excel 16 (macOS) el 2026-08-01, sobre un rastreo\n'
    printf 'real de 5.823 URLs. Si tocas `xlsx.rs`, ábrelo otra vez a mano: openpyxl es tolerante\n'
    printf 'y Excel no.\n'
    exit 0
else
    printf '\033[31m── %s comprobaciones fallaron ──────────────\033[0m\n' "$FALLOS"
    exit 1
fi
