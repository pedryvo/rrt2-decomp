#!/usr/bin/env python3
"""Validate assets/lang/*.toml against the game's default2.lng (see docs/i18n.md).

Usage: python3 -I scripts/check-translations.py [--lng PATH] [locale ...]
Exit code 1 if any file has errors. Stdlib only."""
import argparse
import os
import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
PLACEHOLDER = re.compile(r"%\d")
# Cheat codes the player types: must stay identical to the English source.
VERBATIM_IDS = range(2722, 2738)
FILENAME = re.compile(r"[A-Za-z0-9_%.\\-]+\.(?:PCX|TGA|WAV|IMB|WIN|SMK|RAW|BMP)", re.IGNORECASE)


def load_lng(path):
    """id -> text (unescaped like rt2_formats::lng), plus per-id length limits from ;t tables."""
    strings, limits, pending_limit = {}, {}, None
    for raw in path.read_text(encoding="latin-1").splitlines():
        line = raw.strip()
        if line.startswith(";"):
            m = re.match(r";t(\w+)_0\b(.*)", line)
            if m:
                lim = re.search(r"max (\d+) char|(\d+) character max", m.group(2))
                n = (lim.group(1) or lim.group(2)) if lim else None
                pending_limit = (m.group(1), int(n) if n else None)
            continue
        m = re.match(r"(\d+)\s+\"(.*)\"$", line)
        if not m:
            continue
        i = int(m.group(1))
        strings[i] = re.sub(r"\\(.)", lambda e: "\n" if e.group(1) == "n" else e.group(1), m.group(2))
        if pending_limit:
            limits[i] = pending_limit  # first id of the table; extends until the next table
            pending_limit = None
    # Propagate a table's limit to the ids that follow it until the next table start.
    starts = sorted(limits)
    per_id = {}
    for k, s in enumerate(starts):
        end = starts[k + 1] if k + 1 < len(starts) else max(strings) + 1
        name, lim = limits[s]
        if lim:
            for i in range(s, end):
                if i in strings:
                    per_id[i] = (name, lim)
    return strings, per_id


def check(path, source, limits):
    errors = []
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    tr = {int(k): v for k, v in data.get("strings", {}).items()}
    missing = sorted(set(source) - set(tr))
    extra = sorted(set(tr) - set(source))
    if missing:
        errors.append(f"{len(missing)} missing ids, e.g. {missing[:10]}")
    if extra:
        errors.append(f"{len(extra)} unknown ids, e.g. {extra[:10]}")
    for i, en in source.items():
        t = tr.get(i)
        if t is None:
            continue
        if sorted(PLACEHOLDER.findall(en)) != sorted(PLACEHOLDER.findall(t)):
            errors.append(f"id {i}: placeholders {PLACEHOLDER.findall(en)} -> {PLACEHOLDER.findall(t)}")
        if en.count("\n") != t.count("\n"):
            errors.append(f"id {i}: {en.count(chr(10))} line breaks -> {t.count(chr(10))}")
        if i in VERBATIM_IDS and t != en:
            errors.append(f"id {i}: cheat code must stay {en!r}")
        if FILENAME.fullmatch(en) and t != en:
            errors.append(f"id {i}: filename must stay {en!r}")
        if i in limits and len(t) > limits[i][1]:
            errors.append(f"id {i}: {limits[i][0]} limit {limits[i][1]} chars, got {len(t)} ({t!r})")
    return errors


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--lng", default=os.path.join(os.environ.get("RT2_DIR", ""), "default2.lng"))
    ap.add_argument("locales", nargs="*")
    args = ap.parse_args()
    lng = pathlib.Path(args.lng)
    if not lng.is_file():
        sys.exit(f"default2.lng not found at {lng} (set RT2_DIR or pass --lng)")
    source, limits = load_lng(lng)
    files = sorted((ROOT / "assets/lang").glob("*.toml"))
    if args.locales:
        files = [f for f in files if f.stem in args.locales]
    failed = False
    for f in files:
        errors = check(f, source, limits)
        status = "ok" if not errors else f"{len(errors)} errors"
        print(f"{f.name}: {status}")
        for e in errors[:40]:
            print("  " + e)
        failed |= bool(errors)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
