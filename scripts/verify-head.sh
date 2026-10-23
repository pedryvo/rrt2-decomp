#!/usr/bin/env bash
# Verify the committed HEAD in a throwaway worktree, isolated from uncommitted work of
# parallel agents: fmt, clippy -D warnings and the full test suite.
# Usage: scripts/verify-head.sh   (env RT2_DIR / RT2_SAVE_2348 are passed through)
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
wt=$(mktemp -d "${TMPDIR:-/tmp}/rrt2-verify.XXXXXX")
trap 'git -C "$repo" worktree remove --force "$wt" >/dev/null 2>&1 || true' EXIT
git -C "$repo" worktree add -q --detach "$wt" HEAD
if [[ -f "$repo/re/bin/RT2_PLAT.EXE" ]]; then
    mkdir -p "$wt/re/bin" && cp "$repo/re/bin/RT2_PLAT.EXE" "$wt/re/bin/"
fi
cd "$wt"
nice -n 10 cargo fmt --all --check >/dev/null || { cargo fmt --all --check | head -20; echo "verify-head: fmt FAILED" >&2; exit 1; }
CARGO_TARGET_DIR="$repo/target" nice -n 10 cargo clippy --workspace --all-targets -q -- -D warnings
log="$wt/.nextest.log"
if ! CARGO_TARGET_DIR="$repo/target" nice -n 10 cargo nextest run --workspace >"$log" 2>&1; then
    tail -40 "$log"
    echo "verify-head: tests FAILED" >&2
    exit 1
fi
tail -2 "$log"
scripts/check-changelog.sh
echo "verify-head: OK"
