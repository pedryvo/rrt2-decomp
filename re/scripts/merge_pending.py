#!/usr/bin/env python3
"""Merge parallel agents' results from re/pending/ into the central registries.

  re/pending/*.toml       [[symbol]] (re/symbols.toml format) and [[coverage]] {addr, category, status, why}
  re/pending/*.tsv        addr, category, confidence, why  (bulk classification; header line required)
  re/pending/*.changelog  lines appended to CHANGELOG.md under "## Unreleased"

Merged coverage entries get `manual = true`, then classify.py rewrites re/coverage.toml
and its summary. Symbol name conflicts are reported, never overwritten. Merged pending
files are moved to re/pending/merged/. Stdlib only: python3 -I re/scripts/merge_pending.py
"""
import csv
import pathlib
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
PENDING = ROOT / "re" / "pending"
STATUS_RANK = {"unknown": 0, "named": 1, "spec": 2, "verified": 3}


def norm(addr):
    return hex(int(addr, 16))


def toml_str(v):
    out = []
    for ch in str(v):
        if ch in '"\\':
            out.append("\\" + ch)
        elif ch == "\n":
            out.append("\\n")
        elif ord(ch) < 0x20 or ord(ch) == 0x7F:
            out.append(f"\\u{ord(ch):04x}")
        else:
            out.append(ch)
    return '"' + "".join(out) + '"'


def write_coverage(path, header, entries):
    lines = header[:]
    for e in entries:
        lines += ["", "[[function]]"]
        for key in ("addr", "size", "category", "status", "why"):
            if key in e:
                v = e[key]
                lines.append(f"{key} = {v}" if isinstance(v, int) else f"{key} = {toml_str(v)}")
        if e.get("manual"):
            lines.append("manual = true")
    path.write_text("\n".join(lines) + "\n")


def main():
    cov_path = ROOT / "re" / "coverage.toml"
    sym_path = ROOT / "re" / "symbols.toml"
    text = cov_path.read_text()
    header = [l for l in text.splitlines() if l.startswith("#")]
    entries = tomllib.loads(text)["function"]
    by_addr = {norm(e["addr"]): e for e in entries}
    symbols = {norm(s["addr"]): s for s in tomllib.loads(sym_path.read_text()).get("symbol", [])}

    files = sorted(p for p in PENDING.glob("*") if p.is_file())
    if not files:
        print("nothing pending")
        return
    new_symbols, conflicts, cov_updates, changelog = [], [], 0, []

    def update(addr, category, status, why):
        nonlocal cov_updates
        e = by_addr.get(norm(addr))
        if e is None:
            print(f"warning: {addr} is not a known function, skipped", file=sys.stderr)
            return
        if category and category != "unknown":
            e["category"] = category
        if status and STATUS_RANK.get(status, 0) > STATUS_RANK.get(e.get("status", "unknown"), 0):
            e["status"] = status
        if why:
            e["why"] = why
        e["manual"] = True
        cov_updates += 1

    for p in files:
        if p.suffix == ".toml":
            data = tomllib.loads(p.read_text())
            for s in data.get("symbol", []):
                a = norm(s["addr"])
                if a in symbols and symbols[a]["name"] != s["name"]:
                    conflicts.append(f"{a}: {symbols[a]['name']} (kept) vs {s['name']} ({p.name})")
                elif a not in symbols:
                    symbols[a] = s
                    new_symbols.append(s)
            for c in data.get("coverage", []):
                update(c["addr"], c.get("category"), c.get("status"), c.get("why"))
        elif p.suffix == ".tsv":
            with p.open() as f:
                for row in csv.DictReader(f, delimiter="\t"):
                    if by_addr.get(norm(row["addr"]), {}).get("manual"):
                        continue
                    update(row["addr"], row["category"], None, f"{row['confidence']}: {row['why']}")
        elif p.suffix == ".changelog":
            changelog += [l for l in p.read_text().splitlines() if l.startswith("- ")]

    write_coverage(cov_path, header, entries)

    if new_symbols:
        with sym_path.open("a") as f:
            for s in new_symbols:
                f.write("\n[[symbol]]\n")
                for key in ("addr", "name", "kind", "comment", "spec"):
                    if key in s:
                        f.write(f"{key} = {toml_str(s[key])}\n")

    if changelog:
        cl = ROOT / "CHANGELOG.md"
        lines = cl.read_text().splitlines()
        i = lines.index("## Unreleased") + 1
        while i < len(lines) and (not lines[i].startswith("## ")):
            i += 1
        while lines[i - 1] == "":
            i -= 1
        lines[i:i] = changelog
        cl.write_text("\n".join(lines) + "\n")

    subprocess.run([sys.executable, "-I", str(ROOT / "re" / "scripts" / "classify.py")], check=True)

    done = PENDING / "merged"
    done.mkdir(exist_ok=True)
    for p in files:
        p.rename(done / p.name)
    print(f"merged {len(files)} files: {len(new_symbols)} symbols, {cov_updates} coverage updates, "
          f"{len(changelog)} changelog lines")
    for c in conflicts:
        print("CONFLICT", c)


if __name__ == "__main__":
    main()
