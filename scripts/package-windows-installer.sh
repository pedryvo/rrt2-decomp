#!/usr/bin/env bash
# Build the Windows installer (Inno Setup 6, packaging/windows/rrt2.iss) from the folder that
# scripts/package-release.sh staged for the zip. The release workflow runs this in the Windows
# job (Git Bash); see docs/packaging.md, section 5.
# Usage: scripts/package-windows-installer.sh <label> [out-dir]
#   label    version or nightly tag, e.g. 0.3.0 or nightly-abc123def (same as package-release.sh)
#   out-dir  where package-release.sh staged rrt2-<label>-windows-x86_64/ (default: dist); the
#            installer rrt2-<label>-windows-x86_64-setup.exe is written there too
# Needs ISCC.exe (Inno Setup 6): on PATH, in Program Files, or installed with Chocolatey.
set -euo pipefail
cd "$(dirname "$0")/.."
label="${1:?usage: package-windows-installer.sh <label> [out-dir]}"
out="${2:-dist}"
stage="$out/rrt2-${label}-windows-x86_64"
[[ -f "$stage/rrt2.exe" ]] || { echo "missing $stage/rrt2.exe: run scripts/package-release.sh windows-x86_64 first" >&2; exit 1; }
# Numeric X.Y.Z for the version resource of the setup (the label may be nightly-<commit>).
version="$(sed -n '/^\[workspace\.package\]/,/^\[/ s/^version = "\(.*\)"/\1/p' Cargo.toml)"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "cannot read the workspace version" >&2; exit 1; }

iscc="$(command -v iscc || command -v ISCC || true)"
for c in "/c/Program Files (x86)/Inno Setup 6/ISCC.exe" "/c/Program Files/Inno Setup 6/ISCC.exe"; do
    [[ -z "$iscc" && -f "$c" ]] && iscc="$c"
done
if [[ -z "$iscc" ]]; then
    choco install innosetup -y --no-progress >/dev/null
    iscc="/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
fi
[[ -f "$iscc" ]] || [[ -x "$iscc" ]] || { echo "ISCC.exe (Inno Setup 6) not found" >&2; exit 1; }

winpath() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else echo "$1"; fi; }
# Git Bash would rewrite the /D... switches as POSIX paths: the paths are already Windows ones.
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q \
    "/DAppVersion=$label" "/DVersionNum=$version" \
    "/DStageDir=$(winpath "$(cd "$stage" && pwd)")" "/DOutputDir=$(winpath "$(cd "$out" && pwd)")" \
    "$(winpath packaging/windows/rrt2.iss)"
setup="$out/rrt2-${label}-windows-x86_64-setup.exe"
[[ -f "$setup" ]] || { echo "ISCC did not produce $setup" >&2; exit 1; }
echo "$setup"
