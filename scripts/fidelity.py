#!/usr/bin/env python3
"""Placar de fidelidade automatizado (nivel 2: save como oraculo).

Para cada cenario de scripts/fidelity-scenarios.toml:
  1. base   = original(save, 0 dias, semente)      (conversao do save pelo original, em cache)
  2. orig_N = original(save, N dias, semente)      (Wine; em cache por EXE+cenario+dias+semente)
  3. clone  = rt2-advance(base, N, semente)
Opcoes por cenario: `preseed = K` (RNG do jogo e do CRT = K antes da carga, `advance_save.py
--preseed`) com `clone_from = "source"` (o clone converte o proprio arquivo do jogo, `rt2-advance
<save> --preseed K`, em vez de partir da conversao do original; testa rt2_sim::convert), e
`new_game = "SET:IDX:GSEED:RSEED"` (o original inicia o cenario da campanha por 0x410de0; o clone
roda `World::new_game` com as opcoes lidas da saida de 0 dias do original; `campaign_name` = nome
da tabela 0x552430); `skip_choices = true` (advance_save.py --skip-choices) para mapas com escolhas
modais de regras de cenario.
  4. rt2-savediff --json orig_N clone  ->  iguais / N campos / principais grupos
e reescreve a tabela de docs/fidelity-status.md entre <!-- fidelity:begin --> e <!-- fidelity:end -->.
Sai com 1 se algum caso que estava IGUAL no placar anterior regrediu.

Uso: python3 -I scripts/fidelity.py [--refresh] [--only NOME] [--no-build] [--no-write]
Veja docs/dynamic-analysis.md, secao 5.7.
"""
import argparse, hashlib, json, os, re, shutil, subprocess, sys, time, tomllib

ROOT = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
INSTALL = os.environ.get("RT2_INSTALL", "/home/pedryvo/Games/railroad-tycoon-2")
DOC = os.path.join(ROOT, "docs", "fidelity-status.md")
BEGIN, END = "<!-- fidelity:begin -->", "<!-- fidelity:end -->"
STATE_RE = re.compile(r"<!-- fidelity:state (.*?) -->", re.S)
BIN = os.path.join(ROOT, "target", "release")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def wait_wine(timeout):
    """Espera nao haver winedbg ativo (advance_save.py mata todos os winedbg da maquina)."""
    t0 = time.time()
    while True:
        r = subprocess.run(["pgrep", "-a", "winedbg"], capture_output=True, text=True)
        if not r.stdout.strip():
            return
        if time.time() - t0 > timeout:
            sys.exit("winedbg ainda ativo apos %ds, abortando:\n%s" % (timeout, r.stdout))
        print("  aguardando winedbg de outra execucao...", flush=True)
        time.sleep(10)


def ensure_sandbox(sb):
    if os.path.isfile(os.path.join(sb, "RT2_PLAT.EXE")):
        return
    print("criando sandbox do original em", sb, flush=True)
    r = subprocess.run([os.path.join(ROOT, "re/scripts/wine/run_original.sh"), sb, "--no-run"])
    if r.returncode:
        sys.exit("falha ao criar o sandbox")


def original(args, sb, src, days, seed, out, extra=()):
    """Roda o original (Wine) e grava `out`. Devolve None se ok, ou a mensagem de erro."""
    if os.path.exists(out) and not args.refresh:
        return None
    wait_wine(args.wait)
    ensure_sandbox(sb)
    tmp = out + ".part"
    if not src.upper().endswith((".GAM", ".MAP")):  # .MP2 tem o mesmo formato
        staged = os.path.join(os.path.dirname(out), "_stage.GAM")
        shutil.copyfile(src, staged)
        src = staged
    cmd = [sys.executable, "-I", os.path.join(ROOT, "re/scripts/wine/advance_save.py"),
           sb, src, str(days), str(seed), tmp, "--timeout", str(args.timeout)] + list(extra)
    t0 = time.time()
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode or not os.path.isfile(tmp):
        msg = (r.stdout + r.stderr).strip().splitlines()
        return "original falhou: " + (msg[-1] if msg else "rc=%d" % r.returncode)
    os.replace(tmp, out)
    print("  original %dd: %.0fs" % (days, time.time() - t0), flush=True)
    return None


def entity(path):
    i = path.find(".")
    return path[:i] if i >= 0 else path.split("[")[0]


def group_name(path):
    """`trains[3].x` -> `trains`; `tiles[1,2].rest` -> `tiles`."""
    return entity(path).split("[")[0]


def compare(orig, clone):
    r = subprocess.run([os.path.join(BIN, "rt2-savediff"), orig, clone, "--json"],
                       capture_output=True, text=True)
    if r.returncode not in (0, 1):
        return {"error": "rt2-savediff: " + (r.stderr.strip().splitlines() or ["?"])[-1]}
    d = json.loads(r.stdout)
    groups = {}
    for f in d["fields"]:
        g = group_name(f["path"])
        groups[g] = groups.get(g, 0) + 1
    top = sorted(groups.items(), key=lambda kv: (-kv[1], kv[0]))[:3]
    return {"n": d["differences"], "top": top}


def read_state():
    try:
        text = open(DOC, encoding="utf-8").read()
        m = STATE_RE.search(text)
        return json.loads(m.group(1)) if m else {}
    except (OSError, ValueError):
        return {}


def render(rows, state):
    out = [BEGIN,
           "<!-- Gerado por scripts/fidelity.py; nao editar a mao. -->",
           "| Cenário | Dias | Resultado | Campos diferentes | Principais grupos |",
           "|---|---|---|---|---|"]
    for r in rows:
        if "error" in r:
            res, n, top = "**erro**", "-", r["error"].replace("|", "/")
        elif r["n"] == 0:
            res, n, top = "**igual**", "0", ""
        else:
            res, n = "diferente", str(r["n"])
            top = ", ".join("`%s` (%d)" % kv for kv in r["top"])
        out.append("| %s | %d | %s | %s | %s |" % (r["desc"], r["days"], res, n, top))
    out.append("")
    out.append("<!-- fidelity:state %s -->" % json.dumps(state, sort_keys=True))
    out.append(END)
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--scenarios", default=os.path.join(ROOT, "scripts/fidelity-scenarios.toml"))
    ap.add_argument("--cache", default=os.path.expanduser("~/.cache/rrt2/fidelity"))
    ap.add_argument("--refresh", action="store_true", help="refaz as execucoes do original")
    ap.add_argument("--only", action="append", help="so este cenario (repetivel)")
    ap.add_argument("--no-build", action="store_true", help="nao roda cargo build")
    ap.add_argument("--no-write", action="store_true", help="nao reescreve o documento")
    ap.add_argument("--wait", type=int, default=900, help="espera maxima por winedbg alheio (s)")
    ap.add_argument("--timeout", type=int, default=240, help="timeout por run do original (s)")
    args = ap.parse_args()

    exe = os.path.join(INSTALL, "RT2_PLAT.EXE.orig")
    if not os.path.isfile(exe):
        exe = os.path.join(INSTALL, "RT2_PLAT.EXE")
    key = sha256(exe)[:16]
    cache = os.path.join(os.path.expanduser(args.cache), key)
    os.makedirs(cache, exist_ok=True)
    sb = os.path.join(os.path.expanduser(args.cache), "wine", "rt2sb")
    with open(args.scenarios, "rb") as f:
        scenarios = tomllib.load(f)["scenario"]

    if not args.no_build:
        r = subprocess.run(["cargo", "build", "--release", "-p", "rt2-tools",
                            "--bin", "rt2-advance", "--bin", "rt2-savediff"], cwd=ROOT)
        if r.returncode:
            sys.exit("cargo build falhou")

    prev = read_state()
    rows, state, regress = [], dict(prev), []
    for sc in scenarios:
        if args.only and sc["name"] not in args.only:
            continue
        src = os.path.join(INSTALL, sc["save"])
        seed = sc.get("seed", 1234)
        tag = "%s__%s__s%d" % (re.sub(r"\W+", "_", sc["name"]), sha256(src)[:8], seed)
        extra, clone_src, clone_extra = [], None, []
        if "preseed" in sc:
            extra += ["--preseed", str(sc["preseed"])]
            tag += "__p%d" % sc["preseed"]
        if sc.get("skip_choices"):
            extra += ["--skip-choices"]  # escolhas modais respondidas com o botao 0
            tag += "__sc"
        if sc.get("new_game"):
            extra += ["--new-game", sc["new_game"]]
            tag += "__ng" + re.sub(r"\W+", "_", sc["new_game"])
        print("== %s (%s)" % (sc["name"], sc["save"]), flush=True)
        base = os.path.join(cache, tag + "__d0.GAM")
        err = original(args, sb, src, 0, seed, base, extra)
        if sc.get("new_game"):
            _, _, _, rseed = (int(v, 0) for v in sc["new_game"].split(":"))
            crt = sc.get("preseed", 0) & 0xFFFFFFFF
            for _ in range(2):  # os dois crt_rand_range(1, 32000) de 0x410de0
                crt = (crt * 214013 + 2531011) & 0xFFFFFFFF
            clone_src = src
            clone_extra = ["--new-game", "--options-from", base, "--campaign", "--game-rng",
                           str(rseed), "--crt", str(crt)]
            if sc.get("campaign_name"):
                clone_extra += ["--name", sc["campaign_name"]]
        elif sc.get("clone_from") == "source":
            clone_src = src
            clone_extra = ["--preseed", str(sc.get("preseed", 0))]
        for days in sc["days"]:
            row = {"name": sc["name"], "desc": sc.get("desc", sc["name"]), "days": days}
            rows.append(row)
            orig = os.path.join(cache, "%s__d%d.GAM" % (tag, days))
            e = err or original(args, sb, src, days, seed, orig, extra)
            if e:
                row["error"] = e
                print("  %dd: %s" % (days, e))
                continue
            clone = os.path.join(cache, "clone_%s_d%d.GAM" % (tag, days))
            r = subprocess.run([os.path.join(BIN, "rt2-advance"), clone_src or base, str(days),
                                str(seed), clone, "--quiet"] + clone_extra,
                               capture_output=True, text=True)
            if r.returncode:
                row["error"] = "rt2-advance: " + (r.stderr.strip().splitlines() or ["?"])[-1]
                print("  %dd: %s" % (days, row["error"]))
                continue
            row.update(compare(orig, clone))
            if "error" in row:
                print("  %dd: %s" % (days, row["error"]))
                continue
            k = "%s|%d" % (sc["name"], days)
            was = prev.get(k)
            if row["n"] == 0:
                state[k] = {"equal": True}
                print("  %dd: IGUAL" % days)
            else:
                top = ", ".join("%s (%d)" % kv for kv in row["top"])
                print("  %dd: %d campos diferentes: %s" % (days, row["n"], top))
                if was and was.get("equal"):
                    regress.append(k)  # mantem o estado anterior (igual) como linha de base
                else:
                    state[k] = {"equal": False}
            if row.get("error") is None and k in regress:
                row["regressed"] = True

    if not args.no_write and rows:
        text = open(DOC, encoding="utf-8").read()
        if BEGIN not in text or END not in text:
            sys.exit("marcadores %s / %s ausentes em %s" % (BEGIN, END, DOC))
        if args.only:  # execucao parcial: mantem as linhas dos outros cenarios? so reescreve se completa
            print("(--only: documento nao reescrito)")
        else:
            a, b = text.index(BEGIN), text.index(END) + len(END)
            open(DOC, "w", encoding="utf-8").write(text[:a] + render(rows, state) + text[b:])
            print("placar escrito em", os.path.relpath(DOC, ROOT))

    eq = sum(1 for r in rows if r.get("n") == 0)
    bad = sum(1 for r in rows if "error" in r)
    print("\nResumo: %d casos, %d iguais, %d diferentes, %d erros" %
          (len(rows), eq, len(rows) - eq - bad, bad))
    if regress:
        print("REGRESSAO (estavam IGUAIS): " + ", ".join(regress))
        sys.exit(1)


if __name__ == "__main__":
    main()
