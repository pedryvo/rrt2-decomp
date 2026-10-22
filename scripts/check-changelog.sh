#!/usr/bin/env bash
# Every entry in CHANGELOG.md must read "- <Kind>: text" (OpenRCT2 style).
set -euo pipefail
bad=$(grep -nE '^- ' CHANGELOG.md | grep -vE '^[0-9]+:- (Feature|Improved|Change|Fix|Docs|RE): .+' || true)
if [[ -n "$bad" ]]; then
    echo "Malformed CHANGELOG.md entries (expected '- Feature|Improved|Change|Fix|Docs|RE: text'):"
    echo "$bad"
    exit 1
fi
