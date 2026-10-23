#!/usr/bin/env bash
# Cut a release section: moves the entries under "## Unreleased" of CHANGELOG.md into
# "## v<version> — <date>", leaves a fresh empty "## Unreleased" on top and bumps the workspace
# version (Cargo.toml and Cargo.lock). It does NOT commit or tag: review the diff, commit, then
# `git tag v<version> && git push --tags` to run the release workflow.
# Usage: scripts/cut-release.sh 0.2.0 [YYYY-MM-DD]
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?usage: cut-release.sh <version> [date]}"
version="${version#v}"
date="${2:-$(date +%F)}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must be X.Y.Z" >&2; exit 1; }
grep -q '^## Unreleased' CHANGELOG.md || { echo "CHANGELOG.md has no '## Unreleased' section" >&2; exit 1; }
grep -qE "^## v?${version//./\\.}( |$)" CHANGELOG.md && { echo "v$version already has a section" >&2; exit 1; }
[[ -n "$(scripts/changelog-section.sh Unreleased)" ]] || { echo "Unreleased is empty" >&2; exit 1; }

tmp="$(mktemp)"
awk -v v="$version" -v d="$date" '
    /^## Unreleased/ && !done { print "## Unreleased\n"; print "## v" v " — " d; done = 1; next }
    { print }
' CHANGELOG.md > "$tmp"
mv "$tmp" CHANGELOG.md

# Workspace version: the only `version = "..."` line under [workspace.package].
sed -i -E '/^\[workspace\.package\]/,/^\[/ s/^version = "[^"]*"/version = "'"$version"'"/' Cargo.toml
cargo update --workspace --offline >/dev/null 2>&1 || cargo metadata --offline --format-version 1 >/dev/null

scripts/check-changelog.sh
echo "Cut v$version ($date). Review 'git diff', commit, then: git tag v$version && git push --tags"
