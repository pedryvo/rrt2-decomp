#!/usr/bin/env python3
"""Refresh the reverse-engineering counts line in README.md and README.pt-BR.md from
re/coverage.toml and re/symbols.toml. Stdlib only: python3 -I scripts/update-readme-counts.py"""
import pathlib
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]

entries = tomllib.loads((ROOT / "re/coverage.toml").read_text())["function"]
symbols = len(tomllib.loads((ROOT / "re/symbols.toml").read_text()).get("symbol", []))
total = len(entries)
classified = sum(1 for e in entries if e["category"] != "unknown")
specified = [e for e in entries if e["status"] in ("spec", "verified")]
sim = sum(1 for e in specified if e["category"] == "sim")
verified = sum(1 for e in entries if e["status"] == "verified")
pct = classified * 100 // total
cov = "[`re/coverage.toml`](re/coverage.toml)"
sym = "[`re/symbols.toml`](re/symbols.toml)"

LINES = {
    "README.md": (
        "**Reverse engineering:**",
        f"**Reverse engineering:** {total} functions in the executable (1837 excluding linker thunks) · "
        f"{symbols} symbols in {sym} · {classified} classified ({pct}%), {total - classified} still "
        f"unclassified · {len(specified)} functions specified ({sim} of them simulation), {verified} "
        f"verified bit-exact against the original ({cov}). A simulation coverage percentage will be "
        "published once classification is complete.",
    ),
    "README.pt-BR.md": (
        "**Engenharia reversa:**",
        f"**Engenharia reversa:** {total} funções no executável (1837 sem os thunks do linker) · "
        f"{symbols} símbolos em {sym} · {classified} classificadas ({pct}%), {total - classified} "
        f"ainda sem classificação · {len(specified)} funções especificadas ({sim} delas de simulação), "
        f"{verified} verificadas bit a bit contra o original ({cov}). O percentual de cobertura da "
        "simulação será publicado quando a classificação estiver completa.",
    ),
}

for name, (prefix, line) in LINES.items():
    path = ROOT / name
    lines = path.read_text().split("\n")
    path.write_text("\n".join(line if l.startswith(prefix) else l for l in lines))
    print(f"{name}: {line[:120]}…")
