#!/usr/bin/env python3
"""Lanca o original sob winedbg --gdb --no-start, conecta o gdb e roda um script .gdb.
Uso: probe.py <sandbox> [script.gdb] [segundos]
O sandbox deve existir (run_original.sh <sandbox> --no-run). Fechar a janela encerra a sessao."""
import os, re, subprocess, sys, time
sb = os.path.realpath(sys.argv[1])
gdbs = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(__file__), "fpu_probe.gdb")
secs = int(sys.argv[3]) if len(sys.argv) > 3 else 60
env = dict(os.environ, WINEPREFIX=os.path.join(os.path.dirname(sb), "wineprefix"),
           WINEDEBUG="-all", WINEDLLOVERRIDES="ddraw=n,b;mscoree,mshtml=")
subprocess.run(["pkill", "-x", "winedbg.exe"])
exe = "Z:" + os.path.join(sb, "RT2_PLAT.EXE").replace("/", "\\")
log = open(os.path.join(sb, "..", "winedbg.log"), "w")
wd = subprocess.Popen(["wine", "winedbg", "--gdb", "--no-start", exe], cwd=sb, env=env,
                      stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
port = None
for _ in range(60):
    time.sleep(0.5)
    m = re.search(r"target remote localhost:(\d+)", open(log.name).read())
    if m: port = m.group(1); break
if not port: sys.exit("winedbg nao abriu a porta; ver " + log.name)
script = open(gdbs).read()
full = os.path.join(sb, "..", "_probe_run.gdb")
open(full, "w").write(script.split("\n", 1)[0] + "\nset pagination off\nset architecture i386\n"
    f"target remote localhost:{port}\n" + "\n".join(script.split("\n")[1:]) + "\ncontinue\n")
try:
    subprocess.run(["gdb", "-q", "-batch", "-x", full], env=env, timeout=secs)
except subprocess.TimeoutExpired:
    pass
subprocess.run(["pkill", "-x", "winedbg.exe"]); wd.terminate()
subprocess.run(["wineserver", "-k"], env=env)
