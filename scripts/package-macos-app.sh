#!/usr/bin/env bash
# Build RRT2.app (packaging/macos/Info.plist, icon made from docs/assets/logo.svg) and put it in
# a compressed .dmg with the docs, the licences and a link to /Applications. The release
# workflow runs this in the macOS jobs after scripts/package-release.sh; see docs/packaging.md,
# section 7. macOS only (iconutil, codesign, hdiutil).
# Usage: scripts/package-macos-app.sh <platform> <label> [target-dir] [out-dir]
#   platform   macos-aarch64 | macos-x86_64
#   label      version or nightly tag, e.g. 0.3.0 or nightly-abc123def
#   target-dir where cargo put the binary (default: target/release)
#   out-dir    where rrt2-<label>-<platform>.dmg goes (default: dist)
# The bundle is only ad-hoc signed (`codesign -s -`, no Apple identity) and not notarized: the
# first launch meets Gatekeeper (docs/packaging.md, "Code signing"). No original game asset is
# ever included.
set -euo pipefail
cd "$(dirname "$0")/.."
platform="${1:?usage: package-macos-app.sh <platform> <label> [target-dir] [out-dir]}"
label="${2:?missing label}"
tdir="${3:-target/release}"
out="${4:-dist}"
case "$platform" in
    macos-aarch64) min_macos=11.0 ;;
    macos-x86_64) min_macos=10.13 ;;
    *) echo "platform must be macos-aarch64 or macos-x86_64" >&2; exit 1 ;;
esac
[[ -f "$tdir/rrt2" ]] || { echo "missing $tdir/rrt2: run cargo build --release -p rt2-game first" >&2; exit 1; }
version="$(sed -n '/^\[workspace\.package\]/,/^\[/ s/^version = "\(.*\)"/\1/p' Cargo.toml)"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "cannot read the workspace version" >&2; exit 1; }

name="rrt2-${label}-${platform}"
work="$out/$name-dmg"
root="$work/root"
app="$root/RRT2.app"
rm -rf "$work"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$root/licenses"

# Icon: the logo rendered at every iconset size, then packed by iconutil.
command -v rsvg-convert >/dev/null || brew install librsvg >/dev/null
iconset="$work/rrt2.iconset"
mkdir -p "$iconset"
for s in 16 32 128 256 512; do
    rsvg-convert -w "$s" -h "$s" docs/assets/logo.svg -o "$iconset/icon_${s}x${s}.png"
    rsvg-convert -w $((s * 2)) -h $((s * 2)) docs/assets/logo.svg -o "$iconset/icon_${s}x${s}@2x.png"
done
iconutil -c icns -o "$app/Contents/Resources/rrt2.icns" "$iconset"

sed -e "s/__VERSION__/$version/g" -e "s/__LABEL__/$label/g" -e "s/__MIN_MACOS__/$min_macos/g" \
    packaging/macos/Info.plist > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
cp "$tdir/rrt2" "$app/Contents/MacOS/rrt2"
chmod +x "$app/Contents/MacOS/rrt2"
# Ad-hoc signature over the whole bundle (free, no identity): without it Apple Silicon reports a
# quarantined download as "damaged" instead of offering to open it.
codesign --force --sign - --timestamp=none "$app"
codesign --verify --strict "$app"

cp README.md CHANGELOG.md LICENSE "$root/"
cp assets/fonts/OFL-*.txt "$root/licenses/"
cat > "$root/INSTALL.txt" <<'T'
RRT2 - Railroad Tycoon II clone

1. Drag RRT2.app to Applications.
2. The app is not notarized by Apple. The first time, open it with right click (or
   Control-click) > Open > Open. On macOS 15 or newer: try to open it once, then
   System Settings > Privacy & Security > "Open Anyway". Or, in Terminal:
       xattr -dr com.apple.quarantine /Applications/RRT2.app
3. This package does not contain the original game. You need your own copy of
   Railroad Tycoon II (GOG, Steam or the CD install). Tell RRT2 where it is, once,
   in Terminal:
       /Applications/RRT2.app/Contents/MacOS/rrt2 set-path "/path/to/Railroad Tycoon II"
4. Start RRT2 from Applications or Launchpad.

Settings: ~/Library/Application Support/rrt2/config.toml; saves in
~/Library/Application Support/rrt2/saves (see README.md, "Installing").
T
ln -s /Applications "$root/Applications"

dmg="$out/$name.dmg"
rm -f "$dmg"
# hdiutil sometimes fails with "Resource busy" on CI runners: retry a few times.
for try in 1 2 3; do
    hdiutil create -quiet -volname "RRT2 $label" -srcfolder "$root" -fs HFS+ -format UDZO -ov "$dmg" && break
    [[ $try == 3 ]] && exit 1
    sleep 5
done
hdiutil verify -quiet "$dmg"
rm -rf "$work"
echo "$dmg"
