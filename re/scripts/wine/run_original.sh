#!/usr/bin/env bash
# Roda o RT2_PLAT.EXE original (.orig) no Wine, janelado, numa copia sandbox.
# Uso: run_original.sh <sandbox_dir> [--no-run] [-- args extras do wine]
# NUNCA escreve na instalacao (somente leitura).
set -euo pipefail
INSTALL=${RT2_INSTALL:-/home/pedryvo/Games/railroad-tycoon-2}
SB=${1:?uso: run_original.sh <sandbox_dir> [--no-run]}; shift || true
NORUN=0; [ "${1:-}" = "--no-run" ] && { NORUN=1; shift; }
SB=$(realpath -m "$SB")
case "$SB" in "$INSTALL"|"$INSTALL"/*) echo "recusado: sandbox dentro da instalacao" >&2; exit 1;; esac
if [ ! -f "$SB/RT2_PLAT.EXE" ]; then
  mkdir -p "$SB"
  cp "$INSTALL/RT2_PLAT.EXE.orig" "$SB/RT2_PLAT.EXE"
  cp "$INSTALL"/*.dll "$INSTALL"/*.DLL "$INSTALL"/*.lng "$SB/" 2>/dev/null || true
  for d in data data2 maps games presets Shaders; do
    [ -d "$INSTALL/$d" ] && cp -r --reflink=auto "$INSTALL/$d" "$SB/$d"
  done
  mkdir -p "$SB/anim" "$SB/saves"
  # janelado: cnc-ddraw sem fullscreen, renderer gdi (sem OpenGL/shader)
  cat > "$SB/ddraw.ini" <<'INI'
[ddraw]
width=800
height=600
fullscreen=false
windowed=true
maintas=false
renderer=gdi
adjmouse=true
devmode=false
nonexclusive=true
maxgameticks=60
minfps=0
[RT2]
adjmouse=true
INI
fi
export WINEPREFIX="${WINEPREFIX:-$SB/../wineprefix}"
export WINEDEBUG=${WINEDEBUG:--all}
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:-ddraw=n,b;mscoree,mshtml=}"
[ $NORUN = 1 ] && { echo "sandbox pronto em $SB (WINEPREFIX=$WINEPREFIX)"; exit 0; }
cd "$SB"
exec wine RT2_PLAT.EXE "$@"
