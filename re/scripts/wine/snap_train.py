#!/usr/bin/env python3
"""snap_train.py <sandbox> <in.GAM> <seed> <name> <outdir> <train slot> <dt> <steps,csv>
(docs/dynamic-analysis.md secao 6; usado para os casos tp* de tests/build_snapshots.rs; o trem
novo precisa do comando 0x2c: CALL_MARK=1). Carrega o save no original, chama train_step(trem, dt, 1.0)
varias vezes e grava <name>_pre.GAM e <name>_post<N>.GAM para cada N de steps; acrescenta as linhas
`nome_sN 0x4d0ce0 trem rng crt rng_depois crt_depois 0 dt N` a <outdir>/manifest.txt."""
import os, re, shutil, sys, io, contextlib
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import trace_save
sb, infile, seed, name, out, slot, dt, steps = sys.argv[1:9]
sb = os.path.realpath(sb); os.makedirs(out, exist_ok=True)
stepl = steps.split(",")
posts = ["saves\\_snap_post%s.GAM" % n for n in stepl]
for f in ["_snap_pre.GAM"] + ["_snap_post%s.GAM" % n for n in stepl]:
    try: os.remove(os.path.join(sb, "saves", f))
    except FileNotFoundError: pass
os.environ.update(TR_GDBPY=os.path.join(HERE, "snap_train_gdb.py"),
                  CALL_TRAIN=slot, CALL_DT=dt, CALL_STEPS=steps, SNAP_PRE="saves\\_snap_pre.GAM",
                  SNAP_POSTS=",".join(posts),
                  TR_FUNCS=os.path.join(HERE, "trace_funcs.json"), TR_LOG=os.devnull)
buf = io.StringIO()
with contextlib.redirect_stdout(buf):
    trace_save.run_once(sb, infile, 0, int(seed, 0), 320, 30, 600, os.path.join(out, name + "_end.GAM"))
log = buf.getvalue(); sys.stdout.write(log)
pre = re.search(r"SNAP hit=\d+ ecx=(\S+) edx=(\S+) a1=\S+ ret=\S+ rng=(\S+) crt=(\S+)", log)
if not pre: sys.exit("call failed")
shutil.copyfile(os.path.join(sb, "saves", "_snap_pre.GAM"), os.path.join(out, name + "_pre.GAM"))
for n in stepl:
    m = re.search(r"SNAP post%s rng=(\S+) crt=(\S+)" % n, log)
    if not m: sys.exit("missing post " + n)
    shutil.copyfile(os.path.join(sb, "saves", "_snap_post%s.GAM" % n), os.path.join(out, "%s_post%s.GAM" % (name, n)))
    line = "%s_s%s 0x4d0ce0 %s %s %s %s %s 0 %s %s" % (name, n, slot, pre.group(3), pre.group(4), m.group(1), m.group(2), dt, n)
    open(os.path.join(out, "manifest.txt"), "a").write(line + "\n")
    print("manifest:", line)
