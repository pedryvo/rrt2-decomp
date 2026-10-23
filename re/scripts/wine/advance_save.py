#!/usr/bin/env python3
"""Avanca um save N dias no ORIGINAL (Wine + gdb), sem GUI: oraculo de nivel 2.
Uso: advance_save.py <sandbox> <in.GAM> <dias> <semente> <out.GAM> [--step MIN] [--timeout S]
O sandbox deve existir (run_original.sh <sandbox> --no-run). Roda uma instancia do jogo por vez
(mata winedbg/wineserver do prefixo do sandbox no fim). Detalhes: docs/dynamic-analysis.md, secao 5."""
import argparse, os, re, shutil, subprocess, sys, time

INSTALL = os.environ.get("RT2_INSTALL", "/home/pedryvo/Games/railroad-tycoon-2")
HERE = os.path.dirname(os.path.realpath(__file__))

def run_once(sb, infile, days, seed, step, gates, timeout, outp, quiet=False, preseed=None,
             newgame=None):
    """Uma instancia do jogo: carrega, avanca, grava. Devolve 0 se ok."""
    name = "_advin"
    shutil.copyfile(infile, os.path.join(sb, "games", name + ".GAM"))
    try: os.remove(os.path.join(sb, "games", name + ".GM2"))
    except FileNotFoundError: pass
    os.makedirs(os.path.join(sb, "saves"), exist_ok=True)
    rel_out = "saves\\_advout.GAM"
    host_out = os.path.join(sb, "saves", "_advout.GAM")
    if os.path.exists(host_out): os.remove(host_out)
    prefix = os.path.join(os.path.dirname(sb), "wineprefix")
    env = dict(os.environ, WINEPREFIX=prefix, WINEDEBUG="-all",
               WINEDLLOVERRIDES="ddraw=n,b;mscoree,mshtml=")
    subprocess.run(["pkill", "-x", "winedbg.exe"])
    exe = "Z:" + os.path.join(sb, "RT2_PLAT.EXE").replace("/", "\\")
    wlog = os.path.join(sb, "..", "winedbg.log")
    t0 = time.time()
    with open(wlog, "w") as lf:
        wd = subprocess.Popen(["wine", "winedbg", "--gdb", "--no-start", exe], cwd=sb, env=env,
                              stdout=lf, stderr=subprocess.STDOUT, start_new_session=True)
    port = None
    for _ in range(80):
        time.sleep(0.5)
        m = re.search(r"target remote localhost:(\d+)", open(wlog).read())
        if m: port = m.group(1); break
    if not port:
        sys.exit("winedbg nao abriu a porta; ver " + wlog)
    script = os.path.join(sb, "..", "_advance_run.gdb")
    with open(script, "w") as f:
        f.write("set pagination off\nset architecture i386\ntarget remote localhost:%s\nsource %s\n"
                % (port, os.path.join(HERE, "advance_gdb.py")))
    env.update(ADV_NAME=name, ADV_OUT=rel_out, ADV_DAYS=str(days), ADV_SEED=str(seed & 0xFFFFFFFF),
               ADV_STEP=str(step), ADV_GATES=str(gates))
    if preseed is not None:
        env["ADV_PRESEED"] = str(preseed & 0xFFFFFFFF)
    if newgame is not None:
        cset, cidx, gseed, rseed = newgame
        env.update(ADV_NEWGAME="%d:%d" % (cset, cidx), ADV_GSEED=str(gseed & 0xFFFFFFFF),
                   ADV_RSEED=str(rseed & 0xFFFFFFFF))
    lines, ok = [], False
    try:
        r = subprocess.run(["gdb", "-q", "-batch", "-x", script], env=env, timeout=timeout,
                           capture_output=True, text=True)
        lines = [l for l in r.stdout.splitlines() if l.startswith("ADV ")]
        ok = any(l.startswith("ADV DONE") for l in lines)
        if not ok: sys.stderr.write(r.stdout[-3000:] + r.stderr[-2000:])
    except subprocess.TimeoutExpired as e:
        sys.stderr.write("TIMEOUT (possivel dialogo modal bloqueando sim_advance)\n")
        out = e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
        lines = [l for l in out.splitlines() if l.startswith("ADV ")]
    finally:
        subprocess.run(["pkill", "-x", "winedbg.exe"]); wd.terminate()
        subprocess.run(["wineserver", "-k"], env=env)
    if not quiet:
        for l in lines: print(l[4:])
    if not ok: sys.exit(2)
    if not os.path.isfile(host_out) or os.path.getsize(host_out) == 0:
        sys.exit("o gravador nao produziu " + host_out)
    os.makedirs(os.path.dirname(outp) or ".", exist_ok=True)
    shutil.copyfile(host_out, outp)
    return time.time() - t0

def ensure_warm(sb, infile, step, gates, timeout):
    """Os primeiros runs de um sandbox gravam bytes diferentes (o jogo cria/reescreve rt2_tsc.CFG e
    ddraw.ini, o que muda o layout do heap e os ponteiros gravados no save). Repete runs de 0 dias
    ate dois consecutivos serem identicos; marca o sandbox em .advance_warm."""
    marker = os.path.join(sb, ".advance_warm")
    if os.path.exists(marker): return
    prev = None
    for i in range(6):
        tmp = os.path.join(sb, "saves", "_warm%d.GAM" % (i % 2))
        run_once(sb, infile, 0, 1, step, gates, timeout, tmp, quiet=True)
        cur = open(tmp, "rb").read()
        print("aquecimento %d: %d bytes" % (i + 1, len(cur)))
        if prev is not None and cur == prev:
            open(marker, "w").write("ok\n"); return
        prev = cur
    sys.exit("o sandbox nao estabilizou em 6 runs de aquecimento")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("sandbox"); ap.add_argument("infile"); ap.add_argument("days", type=int)
    ap.add_argument("seed", type=lambda s: int(s, 0)); ap.add_argument("outfile")
    ap.add_argument("--step", type=int, default=320,
                    help="minutos de jogo por chamada de sim_advance (padrao 320 = 100 ms a velocidade 3)")
    ap.add_argument("--timeout", type=int, default=240, help="limite total em segundos")
    ap.add_argument("--gates", type=int, default=30, help="quadros do porteiro a deixar passar antes do menu")
    ap.add_argument("--preseed", type=lambda s: int(s, 0), default=None,
                    help="semente do RNG do jogo e do CRT gravada ANTES da carga (conversao reproduzivel)")
    ap.add_argument("--new-game", metavar="SET:IDX:GSEED:RSEED", default=None,
                    help="em vez de carregar <in>, inicia o cenario IDX (0..17) da campanha SET (0 = maps/#NN*.MAP,"
                         " 1 = #ENNcamp.MP2) por 0x410de0 com game_seed GSEED e RNG RSEED; <in> e ignorado"
                         " (use o .MAP correspondente so como nome)")
    ap.add_argument("--no-warmup", action="store_true", help="nao estabiliza o sandbox antes (ver ensure_warm)")
    a = ap.parse_args()
    sb = os.path.realpath(a.sandbox)
    outp = os.path.realpath(a.outfile)
    for p in (sb, outp):
        if p == INSTALL or p.startswith(INSTALL + "/"):
            sys.exit("recusado: caminho dentro da instalacao (somente leitura): " + p)
    if not os.path.isfile(os.path.join(sb, "RT2_PLAT.EXE")):
        sys.exit("sandbox inexistente: rode run_original.sh %s --no-run" % sb)
    if not a.infile.upper().endswith((".GAM", ".MAP", ".MP2")):
        sys.exit("so .GAM/.MAP/.MP2 (o arquivo e copiado como games/_advin.GAM e carregado como save)")
    if not a.no_warmup:
        ensure_warm(sb, a.infile, a.step, a.gates, a.timeout)
    ng = None
    if a.new_game:
        ng = tuple(int(v, 0) for v in a.new_game.split(":"))
        if len(ng) != 4: sys.exit("--new-game SET:IDX:GSEED:RSEED")
    t = run_once(sb, a.infile, a.days, a.seed, a.step, a.gates, a.timeout, outp, preseed=a.preseed,
                 newgame=ng)
    print("OK %s (%d bytes) em %.1f s" % (outp, os.path.getsize(outp), t))

if __name__ == "__main__":
    main()
