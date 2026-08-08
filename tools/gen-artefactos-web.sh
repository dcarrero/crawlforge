#!/usr/bin/env bash
#
# Los datos que este repositorio publica para que otros los consuman.
#
#   tools/gen-artefactos-web.sh
#
# Emite dos ficheros en la raíz:
#
#   rules.json   el catálogo entero — la salida literal de `crawlforge rules --format json`
#   meta.json    versión, User-Agent y recuento de reglas
#
# **Existen porque la web vive en otro repositorio.** Antes leía el catálogo ejecutando el CLI y
# sacaba el User-Agent del propio `fetch.rs` con `sed`, lo cual solo funciona teniendo el código
# Rust al lado y una cadena de herramientas de Rust instalada. Un sitio Astro que se compila en
# Cloudflare no tiene ninguna de las dos cosas.
#
# `meta.json` no es un fichero de conveniencia: es la diferencia entre publicar un dato pensado
# para ser leído y dejar que otro repositorio espíe una constante dentro de un `.rs`. Lo segundo
# se rompe en cuanto alguien reformatea la línea.
#
# Se versionan los dos. Quien instale este repositorio por tag recibe el catálogo de esa versión
# exacta, sin compilar nada.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

BIN="cargo run -q -p crawlforge-cli --"
command -v cargo > /dev/null || { printf '\033[31m✗ hace falta cargo para regenerar estos ficheros\033[0m\n'; exit 1; }

# ── El catálogo ───────────────────────────────────────────────────────────────
$BIN rules --format json > rules.json
N=$(node -p "require('./rules.json').count" 2>/dev/null || echo "?")
V=$(node -p "require('./rules.json').rules_version" 2>/dev/null || echo "?")

# ── Los metadatos ─────────────────────────────────────────────────────────────
#
# El User-Agent sale de la constante del motor, y esta es la **única** copia que se hace de ella:
# el repositorio de la web compara contra este fichero, no contra el código.
UA=$(sed -n 's/^pub const DEFAULT_USER_AGENT: &str = "\(.*\)";$/\1/p' \
    crates/crawlforge-core/src/fetch.rs)
[ -n "$UA" ] || { printf '\033[31m✗ no se pudo leer DEFAULT_USER_AGENT de fetch.rs\033[0m\n'; exit 1; }

cat > meta.json <<JSON
{
  "version": "$V",
  "user_agent": "$UA",
  "rules_count": $N
}
JSON

printf '\033[32m✓\033[0m rules.json  (%s reglas, versión %s)\n' "$N" "$V"
printf '\033[32m✓\033[0m meta.json   (%s)\n' "$UA"
