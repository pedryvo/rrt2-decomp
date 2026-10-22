"""Snapshot de uma chamada do original (usado por snap_call.py via trace_save.py, TR_GDBPY).
Na K-esima chamada de SNAP_FUNC grava o jogo antes (SNAP_PRE) e depois do retorno (SNAP_POST),
chamando o gravador 0x477540 com a pilha abaixo do quadro corrente e restaurando os registradores;
registra o trem/vagao de ECX, o RNG do jogo (0x55d2c0) e o rand do CRT (0x5609f0). Com SNAP_CALLS e
SNAP_FUNCS registra tambem as funcoes chamadas durante a chamada (arvore por profundidade de pilha).
Executado DENTRO do gdb (source). Nao rodar direto. docs/dynamic-analysis.md, secao "Snapshots".
Parametros por variaveis de ambiente: ADV_NAME (save em games/<nome>.GAM), ADV_OUT (caminho
relativo ao sandbox, escrito pelo gravador), ADV_DAYS, ADV_SEED, ADV_STEP, ADV_GATES.
Sequencia (docs/dynamic-analysis.md secao 5): espera o menu (modo 0xb) no porteiro 0x404e30,
chama funcoes internas do original por 'register+jump' (fastcall, retorno em RET)."""
import gdb, os, struct, sys, time, json

RET = 0x4A5940        # codigo morto sem chamadores: endereco de retorno das chamadas
GATE = 0x404E30       # game_idle_tick
def log(*a):
    print("ADV", *a); sys.stdout.flush()
def reg(n): return int(gdb.parse_and_eval("$" + n)) & 0xFFFFFFFF
def setreg(n, v): gdb.execute("set $%s = %d" % (n, v & 0xFFFFFFFF))
mem = gdb.selected_inferior()
def rd(a, n=4): return bytes(mem.read_memory(a, n))
def rd32(a): return struct.unpack("<I", rd(a, 4))[0]
def wr(a, b): mem.write_memory(a, b)
def wr32(a, v): wr(a, struct.pack("<I", v & 0xFFFFFFFF))
SP0 = 0
def fcall(addr, ecx=0, edx=0, args=()):
    """Chama addr (fastcall: ECX, EDX; resto na pilha, cdecl) e devolve EAX."""
    sp = SP0 - 0x1000 - 4 * len(args) - 4
    wr32(sp, RET)
    for i, a in enumerate(args): wr32(sp + 4 + 4 * i, a)
    setreg("esp", sp); setreg("ecx", ecx); setreg("edx", edx); setreg("eip", addr)
    gdb.execute("continue", to_string=True)
    if reg("eip") != RET:
        log("ERRO fcall %#x parou em eip=%#x" % (addr, reg("eip")))
        raise RuntimeError("fcall")
    return reg("eax")
def snap(tag):
    log("%s mode=%d running=%d date=%#x tod=%#x minutes=%d speed=%d rng=%#x" % (
        tag, rd32(0x566E14), rd32(0x5704E8), rd32(0x5704F8), rd32(0x5704FC) & 0xFFFF,
        rd32(0x5704F4), rd(0x57050E, 1)[0], rd32(0x55D2C0)))

env = os.environ
name, out = env["ADV_NAME"], env["ADV_OUT"]
days, seed, step = int(env["ADV_DAYS"]), int(env["ADV_SEED"]) & 0xFFFFFFFF, int(env["ADV_STEP"])
gates = int(env.get("ADV_GATES", "30"))
gdb.execute("set pagination off"); gdb.execute("set confirm off"); gdb.execute("set architecture i386")

# 1. ponto seguro: porteiro de quadros no menu principal (modo 0xb), UI toda inicializada
bp = gdb.Breakpoint("*0x%x" % GATE, internal=True)
bp.ignore_count = gates
while True:
    gdb.execute("continue", to_string=True)
    if reg("eip") == GATE and rd32(0x566E14) == 0xB: break
bp.delete()
SP0 = reg("esp")
gdb.Breakpoint("*0x%x" % RET, internal=True)
t0 = time.time()
snap("MENU")

# 2. buffer na heap do jogo (CRT _malloc 0x51b6b7, cdecl) para os nomes
buf = fcall(0x51B6B7, 0, 0, (0x400,))
wr(buf, name.encode() + b"\0")
wr(buf + 0x200, out.encode() + b"\0")

# 2b. 0x574c04 = 1 bloqueia o porteiro (game_idle_tick exige 0x574c04 == 0): as rotinas de UI da
#     carga/gravacao chamam o porteiro de forma aninhada e, sem isto, o jogo avancaria sozinho
wr32(0x574C04, 1)

# 3. carga: o mesmo que o botao Load Game do menu (0x4b75b0 caso 0xc57, LAB_004b8c9a)
for a in (0x57FEE0, 0x57BC34, 0x57B41C): wr32(a, rd32(a) + 1)       # contadores de reentrada/UI
fcall(0x495700, 0)                    # prepara a janela de carga (UI, param_1 = 0)
fcall(0x479710, buf, 0)               # carrega games\<nome>.GM2|.GAM -> chama 0x477a60 (loader)
wr32(0x57FEE0, rd32(0x57FEE0) - 1)
fcall(0x502200, 1, 0)
wr32(0x57B41C, rd32(0x57B41C) - 1)
wr32(0x574BC4, 1)
wr32(0x57BC34, rd32(0x57BC34) - 1)
fcall(0x4F6130, 1)
snap("LOADED")
if rd32(0x5704E8) != 1 or rd32(0x566E14) != 1:
    log("ERRO: carga nao colocou o jogo em andamento"); raise RuntimeError("load")
start_min = rd32(0x5704F4)

# 4. RNG: a carga nao restaura a sequencia; fixa a semente do jogo (0x55d2c0) e a do CRT (0x5609f0)
wr32(0x55D2C0, seed); wr32(0x5609F0, seed)



# SNAPSHOT: na K-esima chamada de SNAP_FUNC, grava o save antes (SNAP_PRE) e depois (SNAP_POST)
snap_func = int(env["SNAP_FUNC"], 16)
snap_hit = int(env.get("SNAP_HIT", "1"))
pre_rel, post_rel = env["SNAP_PRE"], env["SNAP_POST"]
wr(buf + 0x280, pre_rel.encode() + b"\0")
wr(buf + 0x300, post_rel.encode() + b"\0")
REGS = ["eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "eip", "eflags"]
def save_regs(): return {r: reg(r) for r in REGS}
def restore_regs(d):
    for r in REGS: setreg(r, d[r])
def nested_save(path_ptr, regs):
    global SP0
    old = SP0
    SP0 = min(regs["esp"], old) - 0x2000
    fcall(0x477540, path_ptr, 0)
    SP0 = old
bpf = gdb.Breakpoint("*0x%x" % snap_func, internal=True)
hits = [0]
done = [False]
def run_call(addr, ecx):
    sp = SP0 - 0x1000 - 4
    wr32(sp, RET)
    setreg("esp", sp); setreg("ecx", ecx); setreg("edx", 0); setreg("eip", addr)
    while True:
        gdb.execute("continue", to_string=True)
        e = reg("eip")
        if e == RET: return
        if e == snap_func and not done[0]:
            hits[0] += 1
            if hits[0] != snap_hit: continue
            regs = save_regs()
            ret = rd32(regs["esp"])
            for pool_g, nm in ((0x574c64, "train"), (0x574c70, "car")):
                pb = rd32(pool_g); base = rd32(pb + 0x28); size = rd32(pb + 8)
                if size and base <= regs["ecx"] < base + size * rd32(pb + 0x10):
                    log("SNAP this=%s slot=%d off=%d" % (nm, (regs["ecx"] - base) // size, (regs["ecx"] - base) % size))
            log("SNAP hit=%d ecx=%#x edx=%#x a1=%#x ret=%#x rng=%#x crt=%#x minutes=%d" % (hits[0], regs["ecx"], regs["edx"], rd32(regs["esp"]+4), ret, rd32(0x55D2C0), rd32(0x5609F0), rd32(0x5704F4)))
            if env.get("SNAP_SAVE", "1") == "1":
                nested_save(buf + 0x280, regs)
                restore_regs(regs)
            bpf.enabled = False
            calllog = open(env.get("SNAP_CALLS", "/dev/null"), "w")
            FUNCS = json.loads(open(env["SNAP_FUNCS"]).read()) if env.get("SNAP_FUNCS") else {}
            class CB(gdb.Breakpoint):
                def __init__(self, a, nm):
                    super().__init__("*0x%x" % a, internal=True); self.nm = nm
                def stop(self):
                    esp = reg("esp")
                    calllog.write("%s depth=%d ret=%#x ecx=%#x edx=%#x a1=%#x a2=%#x\n" % (self.nm, (regs["esp"] - esp) // 4, rd32(esp), reg("ecx"), reg("edx"), rd32(esp+4), rd32(esp+8)))
                    return False
            gdb.execute("set breakpoint always-inserted on")
            cbs = [CB(int(a, 16), v[0]) for a, v in FUNCS.items() if int(a, 16) != snap_func]
            tb = gdb.Breakpoint("*0x%x" % ret, internal=True)
            while True:
                gdb.execute("continue", to_string=True)
                if reg("eip") == ret and reg("esp") > regs["esp"]: break
            tb.delete()
            for b in cbs: b.delete()
            calllog.close()
            regs2 = save_regs()
            log("SNAP post rng=%#x crt=%#x" % (rd32(0x55D2C0), rd32(0x5609F0)))
            nested_save(buf + 0x300, regs2)
            restore_regs(regs2)
            done[0] = True
            continue
        if e == snap_func: continue
        log("ERRO parada inesperada eip=%#x" % e); raise RuntimeError("stop")
remaining = 1440 * days
while remaining > 0 and not done[0]:
    d = min(step, remaining)
    run_call(0x4A5CB0, d)
    remaining -= d
log("SNAP done=%d hits=%d remaining=%d" % (done[0], hits[0], remaining))
bpf.delete()
fcall(0x477540, buf + 0x200, 0)
log("DONE snap")
gdb.execute("quit")
