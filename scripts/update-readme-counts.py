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
    "README.es.md": (
        "**Ingeniería inversa:**",
        f"**Ingeniería inversa:** {total} funciones en el ejecutable (1837 sin contar los thunks del "
        f"enlazador) · {symbols} símbolos en {sym} · {classified} clasificadas ({pct}%), "
        f"{total - classified} aún sin clasificar · {len(specified)} funciones especificadas ({sim} de "
        f"ellas de simulación), {verified} verificadas bit a bit contra el original ({cov}). Se "
        "publicará un porcentaje de cobertura de la simulación cuando termine la clasificación.",
    ),
    "README.de.md": (
        "**Reverse Engineering:**",
        f"**Reverse Engineering:** {total} Funktionen in der ausführbaren Datei (1837 ohne "
        f"Linker-Thunks) · {symbols} Symbole in {sym} · {classified} klassifiziert ({pct} %), "
        f"{total - classified} noch nicht klassifiziert · {len(specified)} Funktionen spezifiziert "
        f"(davon {sim} Simulation), {verified} bitgenau gegen das Original verifiziert ({cov}). Ein "
        "Prozentwert für die Simulationsabdeckung wird veröffentlicht, sobald die Klassifizierung "
        "abgeschlossen ist.",
    ),
    "README.zh-CN.md": (
        "**逆向工程：**",
        f"**逆向工程：** 可执行文件中共有 {total} 个函数（不含链接器跳转桩为 1837 个） · {sym} 中有 "
        f"{symbols} 个符号 · 已分类 {classified} 个（{pct}%），仍有 {total - classified} 个未分类 · "
        f"已编写规格的函数 {len(specified)} 个（其中 {sim} 个属于模拟），{verified} 个已与原版逐位精确"
        f"验证（{cov}）。待分类全部完成后，将公布模拟覆盖率百分比。",
    ),
    "README.ru.md": (
        "**Обратная разработка:**",
        f"**Обратная разработка:** функций в исполняемом файле: {total} (без компоновочных заглушек: "
        f"1837) · символов в {sym}: {symbols} · классифицировано: {classified} ({pct}%), ещё не "
        f"классифицировано: {total - classified} · описано функций: {len(specified)} (из них симуляции: "
        f"{sim}), проверено побитово по оригиналу: {verified} ({cov}). Процент покрытия симуляции "
        "будет опубликован после завершения классификации.",
    ),
}

for name, (prefix, line) in LINES.items():
    path = ROOT / name
    lines = path.read_text().split("\n")
    path.write_text("\n".join(line if l.startswith(prefix) else l for l in lines))
    print(f"{name}: {line[:120]}…")
