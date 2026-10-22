#!/usr/bin/env bash
# Dump do banco de dados do programa (Ghidra headless) para re/db/program.json.
# Uso: re/scripts/dump_program.sh   (pode rodar de qualquer diretório)
# O projeto Ghidra é temporário (-deleteProject) e vive fora do repositório.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EXE="$ROOT/re/bin/RT2_PLAT.EXE"
OUT="$ROOT/re/db/program.json"
SCRATCH="${RRT2_GHIDRA_DIR:-/tmp/claude-1000/-home-pedryvo-Decomp-railroad/6d7cf073-1b4e-458b-98c6-4163984526ea/scratchpad/ghidra}"
HEADLESS="${ANALYZE_HEADLESS:-/opt/ghidra/support/analyzeHeadless}"
mkdir -p "$ROOT/re/db" "$SCRATCH"
START=$(date +%s)
"$HEADLESS" "$SCRATCH" "rt2_dump_$$" \
    -import "$EXE" \
    -scriptPath "$ROOT/re/scripts/ghidra" \
    -postScript dump_program.java "$OUT" "$ROOT/re/symbols.toml" \
    -deleteProject -analysisTimeoutPerFile 1200 \
    >"$SCRATCH/last-run.log" 2>&1 || { tail -40 "$SCRATCH/last-run.log"; exit 1; }
grep -E "dumped|applied" "$SCRATCH/last-run.log" || true
echo "ok: $OUT ($(( $(date +%s) - START ))s)"
