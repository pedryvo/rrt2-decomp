#!/usr/bin/env python3
"""Snapshot de uma funcao do original para teste diferencial (docs/dynamic-analysis.md, "Snapshots").
Uso: snap_call.py <sandbox> <in.GAM> <dias> <semente> <0xfuncao> <K> <nome> <saida_dir> [funcao_clone]
Roda o original (trace_save.run_once com snap_gdb.py), para na K-esima chamada da funcao, grava
<saida_dir>/<nome>_pre.GAM e <nome>_post.GAM e acrescenta a linha do manifesto
'nome funcao_clone trem rng crt rng_depois crt_depois [dt]' em <saida_dir>/manifest.txt
(lido por crates/rt2-sim/tests/trainstep_snapshots.rs). Nao mata winedbg de outros sandboxes."""
import os, re, shutil, sys

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, HERE)
import trace_save  # noqa: E402

def main():
    if len(sys.argv) < 9:
        sys.exit(__doc__)
    sb, infile, days, seed, func, hit, name, out = sys.argv[1:9]
    clone_func = sys.argv[9] if len(sys.argv) > 9 else "step"
    sb = os.path.realpath(sb)
    os.makedirs(out, exist_ok=True)
    for f in ("_snap_pre.GAM", "_snap_post.GAM"):
        try: os.remove(os.path.join(sb, "saves", f))
        except FileNotFoundError: pass
    os.environ.update(TR_GDBPY=os.path.join(HERE, "snap_gdb.py"), SNAP_FUNC=func, SNAP_HIT=hit,
                      SNAP_PRE="saves\\_snap_pre.GAM", SNAP_POST="saves\\_snap_post.GAM",
                      TR_FUNCS=os.path.join(HERE, "trace_funcs.json"), TR_LOG=os.devnull)
    import io, contextlib
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        trace_save.run_once(sb, infile, int(days), int(seed, 0), 320, 30, 1700,
                            os.path.join(out, name + "_end.GAM"))
    log = buf.getvalue()
    sys.stdout.write(log)
    pre = re.search(r"SNAP hit=\d+ ecx=\S+ edx=\S+ a1=(\S+) ret=\S+ rng=(\S+) crt=(\S+)", log)
    post = re.search(r"SNAP post rng=(\S+) crt=(\S+)", log)
    who = re.search(r"SNAP this=(\w+) slot=(\d+)", log)
    if not (pre and post and who):
        sys.exit("a funcao nao foi atingida K vezes")
    for f, suf in (("_snap_pre.GAM", "_pre.GAM"), ("_snap_post.GAM", "_post.GAM")):
        shutil.copyfile(os.path.join(sb, "saves", f), os.path.join(out, name + suf))
    line = "%s %s %s %s %s %s %s" % (name, clone_func, who.group(2), pre.group(2), pre.group(3),
                                      post.group(1), post.group(2))
    if clone_func == "step":
        line += " %d" % int(pre.group(1), 16)
    with open(os.path.join(out, "manifest.txt"), "a") as m:
        m.write(line + "\n")
    print("manifesto:", line)

if __name__ == "__main__":
    main()
