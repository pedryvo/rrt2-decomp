#!/usr/bin/env python3 -I
"""Triagem de categorias para funcoes `category = "unknown"` de re/coverage.toml.

Fontes: re/coverage.toml (estado atual), re/db/decomp/INDEX.tsv e os cabecalhos
(callers/callees) + corpo dos arquivos re/db/decomp/*.c. Escreve re/pending/classify.tsv
(addr, category, confidence, why). Stdlib apenas: python3 -I re/scripts/propagate_categories.py

Ordem:
1. Thunks herdam a categoria final do alvo (`why = "thunk->0x..."`).
2. Sinais diretos no texto da funcao (nomes de API, nomes de globais/funcoes do projeto, strings).
   Alta confianca: um unico categoria com evidencia forte e nenhuma concorrente.
3. Propagacao pelo grafo de chamadas (ate ponto fixo):
   - por chamadores: todos os chamadores classificados e da mesma categoria (exceto lib);
   - por chamados: >= 2 chamados nao-lib, todos da mesma categoria.
   Sempre media confianca.
Funcoes sem evidencia ficam `unknown`. Entradas com `manual = true` nao sao tocadas.
"""
import re
import tomllib
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
COVERAGE = ROOT / "re/coverage.toml"
DECOMP = ROOT / "re/db/decomp"
INDEX = DECOMP / "INDEX.tsv"
OUT = ROOT / "re/pending/classify.tsv"
EXE = ROOT / "re/bin/RT2_PLAT.EXE"

# Sinais diretos: (regex sobre o texto em minusculas, peso). Cada categoria acumula peso.
SIGNALS = {
    "platform": [
        (r"createfile|readfile|writefile|findfirstfile|findnextfile|findclose|closehandle", 2),
        (r"regopen|regquery|regcreate|regclose|regset", 3),
        (r"createwindow|registerclass|defwindowproc|postmessage|dispatchmessage|peekmessage|getmessage|settimer|killtimer", 3),
        (r"timegettime|gettickcount|queryperformance|getcursorpos|showcursor|setcapture|releasecapture|getclientrect", 2),
        (r"getmodulefilename|getcommandline|getprofilestring|getprivateprofile|getwindowsdirectory", 2),
        (r"\bcrt_open_file|\bfopen|\bfread|\bfseek|\bgame_init_paths|\bpk2_", 2),
    ],
    "render": [
        (r"directdraw|ddraw|\bblt\b|lpddsurface|surface|\block\b|\bunlock\b|bitblt|stretchblt|getdc|releasedc|createcompatible", 3),
        (r"\bsprite|\bblit|framebuffer|\bscreen_buf|\bpalette|\bpixel|lad_get_image|lad_load|\bsmk|_smackopen|_smackdo|_smackgo", 3),
        (r"createpalette|setpaletteentries|\bgdi32|\bselectobject|\bcreatefont", 3),
    ],
    "audio": [
        (r"\bail_|_ail_|\bmiles|\bm3d|\bsound|\bmusic|\.wav|\.raw|\bmixer|\bvolume|\bstream_", 4),
    ],
    "ui": [
        (r"\.win\b|\.imb\b|\bdialog|\bdlg|\bbutton|\bbtn_|\blistbox|\bmenu|\bscroll|\bwidget|\bwnd_|\bwindow", 3),
        (r"\bfont|draw_text|\btextlayout|\btext_draw|\bkeyboard|\bkey_|\bmouse|\bclick|\bhover", 2),
        (r"\bcursor|\bhighlight|\bui_", 2),
    ],
    "sim": [
        (r"\btrain|\bcargo|\bcity|\bcities|\bindustr|\bcompan|\bstock|\bshare|\bloan|\bstation|\blocomot|\bconsist", 4),
        (r"\bcalendar|\bsim_|\bsim\b|\bdate_|\bmonth|\bweek|\byear_end|\btimetable|\bpassenger|\bmail\b|\bgoods", 3),
        (r"\brng_|\bai_|\bevent_|\bterrain|\btrack_|\bprice|\bincome|\bmoney|\bcash|\bbond|\bgame_frame", 3),
        (r"\bclock_ms|\btime_game|\btime_horizon|\bcmd_queue|\bcmd_execute", 1),
    ],
    "lib": [
        (r"\b_?_?(malloc|free|memset|memcpy|memmove|strlen|strncpy|strcpy|strncmp|strcmp|strcmpi|strchr|strstr)\b", 2),
        (r"\b__(aulldiv|aullrem|allmul|alldiv|allrem|cfltcvt|frnd|copysign|startoneargerrorhandling|exit|amsg_exit)\b", 4),
        (r"\b(fpatan|fscale|fsin|fcos|sqrt|nan|__chkstk|__alloca_probe|_chkstk|__nh_malloc|heapalloc|heapfree|heaprealloc)\b", 3),
        (r"\b(virtualalloc|virtualfree|getprocaddress|loadlibrarya|freelibrary|getlasterror|setlasterror|tls\w+|isbad\w+ptr)\b", 3),
        (r"\b__local_unwind2|__global_unwind2|\bflsall|\b__fload_withfb|\b__freebuf|\b__fassign|___add_12|\b_smackclose|\b_smackopen", 2),
    ],
}

CATS_ORDER = ["lib", "platform", "render", "audio", "ui", "sim"]
PROPAGATABLE = {"platform", "render", "audio", "ui", "sim"}

HDR = {
    "addr": re.compile(r"^ \* addr:\s+(0x[0-9a-fA-F]+)"),
    "name": re.compile(r"^ \* name:\s+(\S+)"),
    "callers": re.compile(r"^ \* callers:\s*(.*)$"),
    "callees": re.compile(r"^ \* callees:\s*(.*)$"),
}


def parse_coverage():
    funcs = {}
    cur = None
    for line in COVERAGE.read_text(encoding="utf-8").splitlines():
        if line.strip() == "[[function]]":
            cur = {}
            continue
        if cur is None:
            continue
        m = re.match(r'^(\w+)\s*=\s*(.*)$', line)
        if not m:
            continue
        k, v = m.group(1), m.group(2).strip()
        if v.startswith('"'):
            v = v[1:-1]
        elif v in ("true", "false"):
            v = v == "true"
        elif v.isdigit():
            v = int(v)
        cur[k] = v
        if k == "addr":
            funcs[int(v, 16)] = cur
    return funcs


def parse_index():
    by_name, by_addr = {}, {}
    for row in INDEX.read_text(encoding="utf-8").splitlines()[1:]:
        p = row.split("\t")
        if len(p) < 6:
            continue
        a = int(p[0], 16)
        by_name[p[1]] = a
        by_addr[a] = p[1]
    return by_name, by_addr


def pe_thunk_targets(addrs):
    """Thunks de salto do linker (E9 rel32) sem alvo em coverage.toml: decodifica do PE (so leitura)."""
    import struct
    data = EXE.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    opt = struct.unpack_from("<H", data, pe + 20)[0]
    base = struct.unpack_from("<I", data, pe + 24 + 28)[0]
    secs = []
    off = pe + 24 + opt
    for i in range(nsec):
        vsize, va, rawsz, rawptr = struct.unpack_from("<IIII", data, off + 8 + 40 * i)
        secs.append((va, max(vsize, rawsz), rawptr))
    out = {}
    for a in addrs:
        rva = a - base
        for va, size, rawptr in secs:
            if va <= rva < va + size:
                fo = rawptr + (rva - va)
                if data[fo] == 0xE9:
                    rel = struct.unpack_from("<i", data, fo + 1)[0]
                    out[a] = (a + 5 + rel) & 0xFFFFFFFF
                break
    return out


def parse_headers():
    info = {}
    for f in DECOMP.glob("*.c"):
        text = f.read_text(encoding="utf-8", errors="replace")
        head, _, body = text.partition("*/")
        rec = {"body": body, "callers": [], "callees": []}
        for line in head.splitlines():
            for key, rx in HDR.items():
                m = rx.match(line)
                if m:
                    if key in ("callers", "callees"):
                        val = m.group(1).strip()
                        rec[key] = [] if val in ("", "-") else [x.strip() for x in val.split(",") if x.strip()]
                    else:
                        rec[key] = m.group(1)
        if "addr" in rec:
            info[int(rec["addr"], 16)] = rec
    return info


def main():
    cov = parse_coverage()
    by_name, _ = parse_index()
    hdrs = parse_headers()

    # cat final por endereco (nao-thunk); thunks resolvidos depois
    cat = {a: f.get("category", "unknown") for a, f in cov.items()}
    conf = {a: "high" for a in cov}
    why = {a: f.get("why", "") for a, f in cov.items()}

    pe_targets = pe_thunk_targets([a for a in cov if a not in hdrs and cat.get(a) == "unknown"])

    def thunk_target(a):
        f = cov.get(a)
        if f and str(f.get("why", "")).startswith("thunk->"):
            return int(str(f["why"]).split("->")[1], 16)
        return pe_targets.get(a)

    def resolve(name):
        m = re.match(r"^thunk_(?:FUN_)?([0-9a-fA-F]{6,8})$", name)
        if m:
            return int(m.group(1), 16)
        m = re.match(r"^FUN_([0-9a-fA-F]{6,8})$", name)
        if m:
            return int(m.group(1), 16)
        return by_name.get(name)

    # grafo: chamados/chamadores resolvidos para enderecos de nao-thunk
    callees = {a: set() for a in hdrs}
    callers = defaultdict(set)
    for a, r in hdrs.items():
        for n in r["callees"]:
            t = resolve(n)
            if t is not None:
                callees[a].add(t)
                callers[t].add(a)
    for a, r in hdrs.items():
        for n in r["callers"]:
            t = resolve(n)
            if t is not None:
                callers[a].add(t)

    # Chamados/chamadores podem ser thunks: normalizar para o alvo final.
    def norm(t):
        seen = set()
        while t in cov and thunk_target(t) is not None and t not in seen:
            seen.add(t)
            t = thunk_target(t)
        return t

    callees = {a: {norm(x) for x in s} for a, s in callees.items()}
    callers = {a: {norm(x) for x in s} for a, s in callers.items()}

    # 2. sinais diretos
    direct = {}
    for a, r in hdrs.items():
        if cat.get(a) != "unknown" or cov[a].get("manual"):
            continue
        text = (r["name"] + "\n" + "\n".join(r["callees"]) + "\n" + r["body"]).lower()
        scores = {}
        ev = {}
        for c, rules in SIGNALS.items():
            s = 0
            hits = []
            for rx, w in rules:
                found = re.findall(rx, text)
                if found:
                    s += w
                    hits.append(found[0] if isinstance(found[0], str) else rx[:12])
            scores[c] = s
            ev[c] = hits
        ranked = sorted(scores.items(), key=lambda kv: -kv[1])
        top, second = ranked[0], ranked[1]
        if top[1] >= 3 and second[1] * 2 <= top[1]:
            c = top[0]
            direct[a] = (c, "high" if top[1] >= 6 and second[1] == 0 else "medium",
                         "direct:" + ",".join(ev[c][:4]))

    # 3. propagacao: grafo de chamadas, afinidade por globais, blocos CRT e wrappers; repete ate estabilizar
    assigned = {a: (v[0], v[1], v[2]) for a, v in direct.items()}

    def cur_cat(a):
        if a in assigned:
            return assigned[a][0]
        return cat.get(a, "unknown")

    # globais/strings referenciadas: quais categorias as usam (so funcoes ja classificadas)
    tok_re = re.compile(r"\b(DAT_[0-9a-fA-F]{8}|PTR_DAT_[0-9a-fA-F]{8}|s_[A-Za-z0-9_]{4,40}_[0-9a-fA-F]{6,8})\b")
    refs = {a: set(tok_re.findall(r["body"])) for a, r in hdrs.items()}

    def affinity_pass():
        by_tok = defaultdict(lambda: defaultdict(int))
        for a in hdrs:
            c = cur_cat(a)
            if c == "unknown" or c == "lib":
                continue
            for t in refs[a]:
                by_tok[t][c] += 1
        n = 0
        for a in hdrs:
            if cur_cat(a) != "unknown" or cov[a].get("manual"):
                continue
            votes = defaultdict(float)
            used = []
            for t in refs[a]:
                d = by_tok.get(t)
                if not d:
                    continue
                tot = sum(d.values())
                if tot < 4 or tot > 25:  # pouca evidencia, ou global muito compartilhado
                    continue
                top_c, top_n = max(d.items(), key=lambda kv: kv[1])
                if top_n / tot >= 0.95:
                    votes[top_c] += 1
                    used.append(t)
            if not votes:
                continue
            best = max(votes.items(), key=lambda kv: kv[1])
            if best[1] >= 4 and len(used) >= 3 and best[1] >= 3 * max([v for k, v in votes.items() if k != best[0]] or [0]):
                assigned[a] = (best[0], "medium", "globals:" + ",".join(sorted(used)[:3]))
                n += 1
        return n

    for _round in range(6):
        changed = True
        while changed:
            changed = False
            for a in hdrs:
                if cur_cat(a) != "unknown" or cov[a].get("manual"):
                    continue
                cs = [x for x in callers.get(a, set()) if x in hdrs or x in cov]
                if cs and (len(cs) >= 2 or next(iter(cs)) in direct):
                    cats = {cur_cat(x) for x in cs}
                    if len(cats) == 1:
                        c = next(iter(cats))
                        if c in PROPAGATABLE:
                            assigned[a] = (c, "medium", f"callers-all:{c}({len(cs)})")
                            changed = True
                            continue
                ce = [x for x in callees.get(a, set()) if cur_cat(x) not in ("unknown", "lib")]
                if len(ce) >= 2:
                    cats = {cur_cat(x) for x in ce}
                    if len(cats) == 1:
                        c = next(iter(cats))
                        if c in PROPAGATABLE:
                            assigned[a] = (c, "medium", f"callees-all:{c}({len(ce)})")
                            changed = True
        added = affinity_pass()
        added += _crt_and_wrappers(assigned, cur_cat, cov, hdrs, callees, thunk_target, lib_set_fn(cat, cov))
        if added == 0:
            break

    # 1. thunks herdam do alvo final
    rows = []
    for a, f in sorted(cov.items()):
        if cat.get(a) != "unknown" or f.get("manual"):
            continue
        t = thunk_target(a)
        if t is not None:
            tgt = norm(t)
            tc = cur_cat(tgt)
            if tc != "unknown":
                tconf = assigned[tgt][1] if tgt in assigned else "high"
                rows.append((a, tc, tconf, f"thunk->0x{tgt:x}"))
            continue
        if a in assigned:
            c, cf, w = assigned[a]
            rows.append((a, c, cf, w))

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8") as fh:
        fh.write("addr\tcategory\tconfidence\twhy\n")
        for a, c, cf, w in rows:
            fh.write(f"0x{a:x}\t{c}\t{cf}\t{w}\n")

    counts = defaultdict(lambda: defaultdict(int))
    for a, c, cf, w in rows:
        counts[c][cf] += 1
    unknown_left = sum(1 for a, f in cov.items() if cat.get(a) == "unknown" and not f.get("manual")) - len(rows)
    for c in CATS_ORDER:
        if c in counts:
            print(f"{c}: high={counts[c]['high']} medium={counts[c]['medium']}")
    print(f"unknown (de {sum(1 for a in cov if cat.get(a)=='unknown')}): classificados={len(rows)} restam={unknown_left}")
    print(f"escrito: {OUT}")


def lib_set_fn(cat, cov):
    return {a for a in cov if cat.get(a) == "lib"}


def _crt_and_wrappers(assigned, cur_cat, cov, hdrs, callees, thunk_target, lib_set):
    n = 0
    addrs_sorted = sorted(a for a in cov if not cov[a].get("manual"))
    for i, a in enumerate(addrs_sorted):
        if a in assigned or cur_cat(a) != "unknown" or not (0x510000 <= a < 0x530000):
            continue
        if thunk_target(a) is not None or a not in hdrs:
            continue
        prev_ = addrs_sorted[i - 1] if i > 0 else None
        next_ = addrs_sorted[i + 1] if i + 1 < len(addrs_sorted) else None
        if prev_ in lib_set and next_ in lib_set:
            assigned[a] = ("lib", "medium", "crt-block:between-lib-neighbors")
            n += 1
    for a in hdrs:
        if a in assigned or cur_cat(a) != "unknown" or cov[a].get("manual"):
            continue
        size = int(cov[a].get("size", 0))
        ce = callees.get(a, set())
        if ce and size <= 120 and all(cur_cat(x) == "lib" for x in ce):
            assigned[a] = ("lib", "medium", f"small-wrapper-of-lib({len(ce)})")
            n += 1
    return n


if __name__ == "__main__":
    main()
