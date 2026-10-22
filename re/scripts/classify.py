#!/usr/bin/env python3 -I
"""Classificacao automatica de primeira passada das funcoes de RT2_PLAT.EXE.

Le re/db/program.json (dump_program.sh) e re/symbols.toml; escreve re/coverage.toml.
Entradas com `manual = true` sao preservadas como estao. Stdlib apenas: python3 -I re/scripts/classify.py
"""
import json
import re
import sys
import tomllib
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROGRAM = ROOT / "re/db/program.json"
SYMBOLS = ROOT / "re/symbols.toml"
COVERAGE = ROOT / "re/coverage.toml"

CATS = ["lib", "platform", "render", "audio", "ui", "sim", "unknown"]
STATUSES = ["unknown", "named", "spec", "verified"]

# Aliases de importacao: KERNEL32 usado so por CRT (heap, strings, TLS, ambiente, processo).
CRT_K32 = {
    "HeapAlloc", "HeapFree", "HeapReAlloc", "HeapCreate", "HeapDestroy", "HeapSize", "HeapValidate",
    "GetProcessHeap", "VirtualAlloc", "VirtualFree", "VirtualQuery", "GetLastError", "SetLastError",
    "TlsAlloc", "TlsFree", "TlsGetValue", "TlsSetValue", "GetModuleHandleA", "GetModuleFileNameA",
    "GetCommandLineA", "GetEnvironmentStrings", "GetEnvironmentStringsW", "FreeEnvironmentStringsA",
    "FreeEnvironmentStringsW", "GetEnvironmentVariableA", "ExitProcess", "TerminateProcess",
    "GetCurrentProcess", "GetCurrentThreadId", "GetVersion", "GetVersionExA", "GetStartupInfoA",
    "GetStdHandle", "SetHandleCount", "GetACP", "GetCPInfo", "GetOEMCP", "GetStringTypeA",
    "GetStringTypeW", "LCMapStringA", "LCMapStringW", "MultiByteToWideChar", "WideCharToMultiByte",
    "lstrlenA", "lstrcpyA", "lstrcatA", "lstrcmpA", "lstrcmpiA", "lstrcpynA", "RtlUnwind",
    "InitializeCriticalSection", "EnterCriticalSection", "LeaveCriticalSection",
    "DeleteCriticalSection", "UnhandledExceptionFilter", "SetUnhandledExceptionFilter",
    "FlsAlloc", "GetFileType", "SetFilePointer", "FlushFileBuffers", "WriteFile", "ReadFile",
    "CloseHandle", "SetStdHandle", "GetLocaleInfoA", "GetLocaleInfoW", "IsBadReadPtr",
    "IsBadWritePtr", "IsBadCodePtr", "FatalAppExitA", "SetEndOfFile", "GetTimeZoneInformation",
    "QueryPerformanceCounter", "GetSystemTimeAsFileTime", "GetCurrentProcessId", "GetTickCount",
    "SetConsoleCtrlHandler", "RaiseException", "InterlockedIncrement", "InterlockedDecrement",
    "FreeLibrary", "GetProcAddress", "LoadLibraryA", "Sleep", "SetErrorMode", "GlobalAlloc",
    "GlobalFree", "GlobalLock", "GlobalUnlock", "GlobalReAlloc", "LocalAlloc", "LocalFree",
    "MulDiv", "OutputDebugStringA", "DebugBreak",
}
# So estas contam como "plataforma" de verdade (arquivo, registro, tempo, janela, multimidia).
PLATFORM_DLLS = {"USER32", "WINMM", "ADVAPI32", "WSOCK32", "OLE32", "SHELL32", "COMDLG32", "COMCTL32", "DINPUT", "DSOUND", "VERSION"}
RENDER_APIS = {"GDI32"}
RENDER_DLLS = {"DDRAW", "GDI32"}

SIM_WORDS = re.compile(
    r"cargo|industr|stock|bond|train|station|locomotive|passenger|freight|revenue|dividend|"
    r"merger|takeover|shares?\b|interest|bankrupt|track|engine|railroad|company|compan|"
    r"tycoon|profit|income|loan|price|tax|cars?\b|mail|coal|oil|steel|grain|factory|mine\b|"
    r"competitor|\(NUMBER_OF_|\(TRIGGER|scenario|route|schedule|speed|demand|supply",
    re.I)
UI_STR = re.compile(r"\.(win|imb)\b|\{\[|\[[A-Za-z]\d\]|font.*\.dat|^%[sd]( %[sd])*$", re.I)
AUDIO_STR = re.compile(r"\.(raw|wav|mp3|mid|xmi)\b|music\\", re.I)
FILE_STR = re.compile(r"\.(pk2|pak|gam|map|ini|cfg|sav|lng)\b|\*\.|games\\|\\data\\", re.I)
IGNORE_SIM = re.compile(r"^(; |Invalid |No (end )?quote|No quote)|\.raw|\.imb|\.win")

# Prefixo de simbolo -> categoria (para entradas ja nomeadas).
PREFIX_CAT = {
    "pk2": "platform", "game": "platform", "crt": "lib", "mss": "lib", "smk": "lib",
    "lad": "render", "palette": "render", "dd": "render", "ui": "ui",
    "train": "sim", "cargo": "sim", "track": "sim", "stock": "sim", "industry": "sim",
    "station": "sim", "rng": "sim", "econ": "sim",
}


def hexint(s):
    return int(s, 16)


def load_symbols():
    out = {}
    if SYMBOLS.exists():
        data = tomllib.loads(SYMBOLS.read_text(encoding="utf-8"))
        for s in data.get("symbol", []):
            if s.get("kind", "function") == "function":
                out[hexint(s["addr"])] = s["name"]
    return out


def load_manual():
    out = {}
    if COVERAGE.exists():
        data = tomllib.loads(COVERAGE.read_text(encoding="utf-8"))
        for e in data.get("function", []):
            if e.get("manual"):
                out[hexint(e["addr"])] = e
    return out


def main():
    prog = json.loads(PROGRAM.read_text(encoding="utf-8"))
    funcs = {hexint(f["addr"]): f for f in prog["functions"]}
    symbols = load_symbols()
    manual = load_manual()

    # Colapsa thunks (tabela de saltos do linker incremental): chamador -> thunk -> alvo.
    def resolve(a):
        seen = set()
        while a in funcs and funcs[a]["thunk"] and a not in seen and funcs[a]["callees"]:
            seen.add(a)
            a = hexint(funcs[a]["callees"][0])
        return a

    callers = defaultdict(set)  # chamadores reais (nao-thunk) de cada alvo real
    callees = defaultdict(set)
    for a, f in funcs.items():
        if f["thunk"]:
            continue
        for c in f["callees"]:
            t = resolve(hexint(c))
            if t != a:
                callees[a].add(t)
                callers[t].add(a)

    cat, why = {}, {}

    def put(a, c, w):
        if a not in cat:
            cat[a], why[a] = c, w

    fid_addrs = sorted(a for a, f in funcs.items() if f["fid"])
    crt_start = (fid_addrs[0] & ~0xFFF) if fid_addrs else None

    # ---- passo 1: evidencia direta, na ordem lib, platform, render, audio, ui, sim ----
    for a, f in sorted(funcs.items()):
        if f["thunk"]:
            continue
        apis = f["apis"]
        dlls = Counter(x.split("!")[0] for x in apis)
        names = {x.split("!", 1)[1] for x in apis}
        small = f["size"] <= 96
        if f["fid"]:
            put(a, "lib", "fid:" + f["fid_name"].split(",")[-1].strip())
            continue
        if crt_start and a >= crt_start:
            put(a, "lib", "crt-region>=0x%x" % crt_start)
            continue
        if "SMACKW32" in dlls:
            put(a, "lib", "smacker:" + ",".join(sorted(x for x in apis if x.startswith("SMACKW32"))[:2]))
            continue
        if "MSS32" in dlls:
            ex = ",".join(sorted(x for x in apis if x.startswith("MSS32"))[:2])
            put(a, "lib" if small else "audio", ("miles-wrapper:" if small else "miles:") + ex)
            continue
        if apis and set(dlls) == {"KERNEL32"} and names <= CRT_K32:
            put(a, "lib", "crt-api:" + ",".join(sorted(names)[:3]))
            continue
        plat = sorted(x for x in apis if x.split("!")[0] in PLATFORM_DLLS or
                      (x.startswith("KERNEL32") and x.split("!")[1] not in CRT_K32))
        if "DDRAW" in dlls or "GDI32" in dlls:
            put(a, "render", "api:" + ",".join(sorted(x for x in apis if x.split("!")[0] in RENDER_DLLS)[:3]))
            continue
        if plat:
            put(a, "platform", "api:" + ",".join(plat[:3]))
            continue
        if apis:
            put(a, "lib", "crt-api:" + ",".join(sorted(names)[:3]))
            continue
        texts = [s["text"] for s in f["strings"]]
        ui = [t for t in texts if UI_STR.search(t)]
        if ui:
            put(a, "ui", "string:" + ui[0][:40])
            continue
        au = [t for t in texts if AUDIO_STR.search(t)]
        if au:
            put(a, "audio", "string:" + au[0][:40])
            continue
        fl = [t for t in texts if FILE_STR.search(t)]
        if fl:
            put(a, "platform", "string:" + fl[0][:40])
            continue
        sim = [t for t in texts if SIM_WORDS.search(t) and not IGNORE_SIM.search(t)]
        if sim:
            put(a, "sim", "string:" + sim[0][:40].replace("\n", " "))

    # simbolos nomeados: o prefixo manda (so quando nao ha evidencia mais forte de lib)
    for a, name in symbols.items():
        if a in funcs and not funcs[a]["thunk"]:
            p = name.split("_")[0]
            if p in PREFIX_CAT and cat.get(a) != "lib":
                cat[a], why[a] = PREFIX_CAT[p], "symbol:" + name

    # ---- passo 2: propagacao para cima, so por exclusividade de chamadores ----
    # "f so e chamada por funcoes lib/render/audio" => f herda. Nunca o inverso (callees all lib != lib).
    for target in ("lib", "render", "audio"):
        changed = True
        while changed:
            changed = False
            for a, f in funcs.items():
                if f["thunk"] or a in cat:
                    continue
                cs = callers.get(a, set())
                # render/audio: so propagam de evidencia de API (strings sao pista fraca demais)
                strong = lambda c: target == "lib" or not why[c].startswith(("string:", "symbol:"))
                if cs and all(cat.get(c) == target and strong(c) for c in cs):
                    cat[a], why[a] = target, "only-called-by-%s:0x%x" % (target, min(cs))
                    changed = True

    # ---- thunks herdam do alvo ----
    for a, f in funcs.items():
        if f["thunk"]:
            t = resolve(a)
            if t != a and t in cat:
                cat[a], why[a] = cat[t], "thunk->0x%x" % t
            elif f["apis"]:
                cat[a], why[a] = "lib", "api-stub:" + f["apis"][0]

    # ---- escreve entradas ----
    entries = []
    for a in sorted(funcs):
        f = funcs[a]
        if a in manual:
            entries.append(manual[a])
            continue
        named = a in symbols
        e = {
            "addr": "0x%x" % a, "size": f["size"], "category": cat.get(a, "unknown"),
            "status": "named" if named else "unknown", "why": why.get(a, "no-evidence"),
        }
        if named:
            e["name"] = symbols[a]
        entries.append(e)
    for a, e in sorted(manual.items()):
        if a not in funcs:
            entries.append(e)

    cc = Counter(e["category"] for e in entries)
    sc = Counter(e["status"] for e in entries)
    sim = [e for e in entries if e["category"] == "sim"]
    sim_done = sum(1 for e in sim if e["status"] in ("spec", "verified"))
    pct = 100.0 * sim_done / len(sim) if sim else 0.0
    classified = len(entries) - cc["unknown"]
    nmanual = sum(1 for e in entries if e.get("manual"))
    thunks = sum(1 for a in funcs if funcs[a]["thunk"])

    lines = [
        "# GERADO por re/scripts/classify.py a partir de re/db/program.json e re/symbols.toml.",
        "# Edicoes manuais: ponha `manual = true` na entrada; ela e preservada a cada nova execucao.",
        "# Ver docs/re-workflow.md.",
        "#",
        "# Funcoes: %d (%d thunks do linker incremental, herdam a categoria do alvo)" % (len(entries), thunks),
        "# Classificadas (categoria != unknown): %d (%.1f%%)" % (classified, 100.0 * classified / len(entries)),
        "# Por categoria: " + ", ".join("%s=%d" % (c, cc[c]) for c in CATS),
        "# Por estado:   " + ", ".join("%s=%d" % (s, sc[s]) for s in STATUSES),
        "# Entradas manuais: %d" % nmanual,
        "# sim spec coverage: %.1f%% (%d de %d funcoes sim em spec/verified)" % (pct, sim_done, len(sim)),
        "",
    ]
    q = lambda s: json.dumps(s, ensure_ascii=True)
    for e in entries:
        lines.append("[[function]]")
        lines.append('addr = "%s"' % e["addr"])
        if "name" in e:
            lines.append("name = %s" % q(e["name"]))
        lines.append("size = %d" % e["size"])
        lines.append('category = "%s"' % e["category"])
        lines.append('status = "%s"' % e["status"])
        lines.append("why = %s" % q(e.get("why", "")))
        if e.get("manual"):
            lines.append("manual = true")
        lines.append("")
    COVERAGE.write_text("\n".join(lines), encoding="utf-8")

    print("\n".join(l[2:] for l in lines[3:10]))
    print("wrote %s" % COVERAGE.relative_to(ROOT))


if __name__ == "__main__":
    sys.exit(main())
