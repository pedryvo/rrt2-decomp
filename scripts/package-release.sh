#!/usr/bin/env bash
# Stage and archive a release build of `rrt2` (the same commands the release workflow runs).
# Usage: scripts/package-release.sh <platform> <label> [target-dir] [out-dir]
#   platform   linux-x86_64 | windows-x86_64 | macos-aarch64 | macos-x86_64
#   label      version or nightly tag used in the file name, e.g. 0.1.0 or nightly-abc123def
#   target-dir where cargo put the binary (default: target/release; for --target builds pass
#              target/<triple>/release)
#   out-dir    where the archive goes (default: dist)
# Only `rrt2`, the docs and the OFL licences of the vendored fonts are packaged. Fonts and
# translations are already embedded in the binary; no original game asset is ever included
# (the game reads them from the player's own install, see `rrt2 set-path`).
set -euo pipefail
cd "$(dirname "$0")/.."
platform="${1:?usage: package-release.sh <platform> <label> [target-dir] [out-dir]}"
label="${2:?missing label}"
tdir="${3:-target/release}"
out="${4:-dist}"
name="rrt2-${label}-${platform}"
exe=rrt2
[[ "$platform" == windows-* ]] && exe=rrt2.exe
[[ -f "$tdir/$exe" ]] || { echo "missing $tdir/$exe: run cargo build --release -p rt2-game first" >&2; exit 1; }

stage="$out/$name"
rm -rf "$stage"
mkdir -p "$stage/licenses"
cp "$tdir/$exe" "$stage/"
cp README.md CHANGELOG.md "$stage/"
cp assets/fonts/OFL-*.txt "$stage/licenses/"
cp LICENSE "$stage/LICENSE"
cat > "$stage/INSTALL.txt" <<'T'
Railroad Tycoon II clone (rrt2)

1. This package does not contain the original game. You need your own copy of
   Railroad Tycoon II (GOG, Steam or the CD install).
2. Tell rrt2 where it is, once:
       rrt2 set-path "/path/to/Railroad Tycoon II"
   (or pass --game-dir, or set RT2_DIR).
3. Run rrt2.

Settings: config.toml in your user config folder, saves in your user data folder
(see README.md, "Installing").
T
[[ "$exe" == rrt2 ]] && chmod +x "$stage/$exe"

case "$platform" in
    windows-*)
        if command -v zip >/dev/null; then (cd "$out" && zip -qr "$name.zip" "$name")
        else (cd "$out" && 7z a -tzip -bso0 "$name.zip" "$name"); fi
        echo "$out/$name.zip" ;;
    *)
        tar -C "$out" -czf "$out/$name.tar.gz" "$name"
        echo "$out/$name.tar.gz" ;;
esac
