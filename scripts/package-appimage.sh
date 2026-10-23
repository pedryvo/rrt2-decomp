#!/usr/bin/env bash
# Build an AppImage of `rrt2` (the release workflow runs this in the Linux job, after
# scripts/package-release.sh). See docs/packaging.md.
# Usage: scripts/package-appimage.sh <label> [target-dir] [out-dir]
#   label      version or nightly tag used in the file name, e.g. 0.3.0 or nightly-abc123def
#   target-dir where cargo put the x86_64 Linux binary (default: target/release)
#   out-dir    where the AppImage goes (default: dist)
# The AppImage holds only `rrt2` (fonts and translations are embedded in it), the desktop
# entry, the AppStream metadata, the project logo and the licences. No original game asset is
# ever included: the player points rrt2 at their own install (`rrt2 set-path`).
# appimagetool and the type-2 runtime are pinned by version and sha256, and cached in
# ${XDG_CACHE_HOME:-~/.cache}/rrt2/appimage-tools.
set -euo pipefail
cd "$(dirname "$0")/.."
label="${1:?usage: package-appimage.sh <label> [target-dir] [out-dir]}"
tdir="${2:-target/release}"
out="${3:-dist}"
[[ -f "$tdir/rrt2" ]] || { echo "missing $tdir/rrt2: run cargo build --release -p rt2-game first" >&2; exit 1; }

APPIMAGETOOL_VERSION=1.9.1
APPIMAGETOOL_SHA256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
RUNTIME_VERSION=20251108
RUNTIME_SHA256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

tools="${XDG_CACHE_HOME:-$HOME/.cache}/rrt2/appimage-tools"
mkdir -p "$tools"
fetch() { # url file sha256
    local url="$1" file="$tools/$2" sum="$3"
    if [[ ! -f "$file" ]] || ! echo "$sum  $file" | sha256sum -c --status; then
        curl -sSLf --retry 3 -o "$file.part" "$url"
        echo "$sum  $file.part" | sha256sum -c --quiet || { rm -f "$file.part"; exit 1; }
        mv "$file.part" "$file"
    fi
}
fetch "https://github.com/AppImage/appimagetool/releases/download/$APPIMAGETOOL_VERSION/appimagetool-x86_64.AppImage" \
    "appimagetool-$APPIMAGETOOL_VERSION-x86_64.AppImage" "$APPIMAGETOOL_SHA256"
fetch "https://github.com/AppImage/type2-runtime/releases/download/$RUNTIME_VERSION/runtime-x86_64" \
    "runtime-$RUNTIME_VERSION-x86_64" "$RUNTIME_SHA256"
chmod +x "$tools/appimagetool-$APPIMAGETOOL_VERSION-x86_64.AppImage"

id=io.github.pedryvo.rrt2
appdir="$out/rrt2.AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/metainfo" \
    "$appdir/usr/share/icons/hicolor/scalable/apps" "$appdir/usr/share/doc/rrt2/licenses"
install -m 755 "$tdir/rrt2" "$appdir/usr/bin/rrt2"
install -m 644 "packaging/linux/$id.desktop" "$appdir/usr/share/applications/$id.desktop"
install -m 644 "packaging/linux/$id.metainfo.xml" "$appdir/usr/share/metainfo/$id.metainfo.xml"
install -m 644 docs/assets/logo.svg "$appdir/usr/share/icons/hicolor/scalable/apps/$id.svg"
install -m 644 README.md CHANGELOG.md LICENSE "$appdir/usr/share/doc/rrt2/"
install -m 644 assets/fonts/OFL-*.txt "$appdir/usr/share/doc/rrt2/licenses/"
# AppDir root: the desktop entry, the icon and the entry point appimagetool looks for.
ln -s "usr/share/applications/$id.desktop" "$appdir/$id.desktop"
ln -s "usr/share/icons/hicolor/scalable/apps/$id.svg" "$appdir/$id.svg"
ln -s "$id.svg" "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<'SH'
#!/bin/sh
# rrt2 needs nothing but its own binary: fonts and translations are embedded. A `lang/` folder
# of extra translations can still be given with RRT2_LANG_DIR (docs/i18n.md).
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/rrt2" "$@"
SH
chmod 755 "$appdir/AppRun"

file="$out/rrt2-${label}-x86_64.AppImage"
rm -f "$file"
# APPIMAGE_EXTRACT_AND_RUN: CI runners (and containers) have no FUSE to mount appimagetool.
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 \
    "$tools/appimagetool-$APPIMAGETOOL_VERSION-x86_64.AppImage" --no-appstream \
    --runtime-file "$tools/runtime-$RUNTIME_VERSION-x86_64" "$appdir" "$file" >&2
rm -rf "$appdir"
echo "$file"
