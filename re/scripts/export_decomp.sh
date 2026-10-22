#!/usr/bin/env bash
# Exporta o C decompilado de cada funcao nao-thunk para re/db/decomp/ (Ghidra headless).
# Uso: re/scripts/export_decomp.sh   (pode rodar de qualquer diretorio)
# O projeto Ghidra e temporario (-deleteProject) e vive fora do repositorio.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EXE="$ROOT/re/bin/RT2_PLAT.EXE"
OUT="$ROOT/re/db/decomp"
SCRATCH="${RRT2_GHIDRA_DIR:-/tmp/claude-1000/-home-pedryvo-Decomp-railroad/6d7cf073-1b4e-458b-98c6-4163984526ea/scratchpad/ghidra}"
HEADLESS="${ANALYZE_HEADLESS:-/opt/ghidra/support/analyzeHeadless}"
mkdir -p "$OUT" "$SCRATCH"
START=$(date +%s)
"$HEADLESS" "$SCRATCH" "rt2_decomp_$$" \
    -import "$EXE" \
    -scriptPath "$ROOT/re/scripts/ghidra" \
    -postScript export_decomp.java "$OUT" "$ROOT/re/symbols.toml" "$ROOT/re/coverage.toml" \
    -deleteProject -analysisTimeoutPerFile 1200 \
    >"$SCRATCH/last-export.log" 2>&1 || { tail -40 "$SCRATCH/last-export.log"; exit 1; }
grep -E "exported|applied" "$SCRATCH/last-export.log" || true
echo "ok: $OUT ($(( $(date +%s) - START ))s)"
