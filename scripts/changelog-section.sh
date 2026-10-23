#!/usr/bin/env bash
# Print the body of one CHANGELOG.md section (without its heading): a version such as 0.1.0 or
# v0.1.0, or "Unreleased". Used by the release workflow for the GitHub Release notes.
set -euo pipefail
cd "$(dirname "$0")/.."
want="${1:?usage: changelog-section.sh <version|Unreleased>}"
want="${want#v}"
awk -v want="$want" '
    /^## / {
        title = $0
        sub(/^## +/, "", title)
        sub(/^v/, "", title)
        sub(/[ ]+.*$/, "", title)   # "0.1.0 — 2026-10-23" -> "0.1.0"
        active = (tolower(title) == tolower(want))
        next
    }
    active { print }
' CHANGELOG.md | sed -e '/./,$!d' | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}'
