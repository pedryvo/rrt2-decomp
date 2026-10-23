#!/usr/bin/env bash
# Fill the winget manifest templates of packaging/winget/ for one release (docs/packaging.md,
# section 6). Nothing is submitted anywhere: the output is meant to be reviewed and sent by hand
# to microsoft/winget-pkgs (manifests/p/pedryvo/RRT2/<version>/), e.g. with `wingetcreate`.
# Usage: scripts/fill-winget.sh <version> <setup.exe> [out-dir] [YYYY-MM-DD]
#   version    X.Y.Z of the release (tag v<version>)
#   setup.exe  the installer uploaded to that release (its SHA-256 goes in the manifest)
#   out-dir    default: dist/winget/<version>
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?usage: fill-winget.sh <version> <setup.exe> [out-dir] [date]}"
version="${version#v}"
setup="${2:?missing installer path}"
out="${3:-dist/winget/$version}"
date="${4:-$(date +%F)}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must be X.Y.Z" >&2; exit 1; }
[[ -f "$setup" ]] || { echo "missing $setup" >&2; exit 1; }
sha="$(sha256sum "$setup" | cut -d' ' -f1 | tr '[:lower:]' '[:upper:]')"
mkdir -p "$out"
for f in packaging/winget/*.yaml; do
    sed -e "s/__VERSION__/$version/g" -e "s/__INSTALLER_SHA256__/$sha/g" \
        -e "s/__RELEASE_DATE__/$date/g" -e '/^# Template:/,/^[^#]/{/^#/d}' "$f" > "$out/$(basename "$f")"
done
if grep -q '__[A-Z0-9_]*__' "$out"/*.yaml; then echo "unfilled placeholder in $out" >&2; exit 1; fi
echo "$out"
