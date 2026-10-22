"""Variante de advance_gdb.py com contagem/registro de chamadas (TR_FUNCS, TR_LOG), usada por trace_save.py.
Executado DENTRO do gdb (source) por trace_save.py. Nao rodar direto.
Parametros por variaveis de ambiente: ADV_NAME (save em games/<nome>.GAM), ADV_OUT (caminho
relativo ao sandbox, escrito pelo gravador), ADV_DAYS, ADV_SEED, ADV_STEP, ADV_GATES.
Sequencia (docs/dynamic-analysis.md secao 5): espera o menu (modo 0xb) no porteiro 0x404e30,
chama funcoes internas do original por 'register+jump' (fastcall, retorno em RET)."""
import gdb, os, struct, sys, time

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


# TRACE: contadores e log de chamadas
import json
FUNCS = json.loads(open(env["TR_FUNCS"]).read())   # {hexaddr: [name, logmode]}  logmode 0=conta, 1=loga
seq = [0]
counts = {}
outf = open(env["TR_LOG"], "w")
class TB(gdb.Breakpoint):
    def __init__(self, a, nm, lm):
        super().__init__("*0x%x" % a, internal=True); self.a=a; self.nm=nm; self.lm=lm
    def stop(self):
        counts[self.nm] = counts.get(self.nm, 0) + 1
        if self.lm:
            seq[0] += 1
            esp = reg("esp")
            outf.write("%d %s ret=%#x ecx=%#x edx=%#x a1=%#x date=%#x tod=%#x rng=%#x\n" % (
                seq[0], self.nm, rd32(esp), reg("ecx"), reg("edx"), rd32(esp+4),
                rd32(0x5704F8), rd32(0x5704FC) & 0xFFFF, rd32(0x55D2C0)))
        return False
gdb.execute("set breakpoint always-inserted on")
BPS=[TB(int(a,16), v[0], v[1]) for a, v in FUNCS.items()]
# 5. avanca: sim_advance(ECX = minutos) em passos fixos, sem o porteiro
remaining = 1440 * days
t1 = time.time()
while remaining > 0:
    d = min(step, remaining)
    fcall(0x4A5CB0, d)
    remaining -= d
snap("ADVANCED")
log("advance_seconds=%.2f" % (time.time() - t1))
end_min = rd32(0x5704F4)
if end_min - start_min != 1440 * days:
    log("ERRO: avanco de %d minutos, esperado %d" % (end_min - start_min, 1440 * days))
    raise RuntimeError("advance")

outf.close()
for k in sorted(counts): log("COUNT %s %d" % (k, counts[k]))
for b in BPS: b.delete()
# 6. grava: gravador 0x477540 (ECX = caminho, EDX = 0 -> arquivo em disco)
fcall(0x477540, buf + 0x200, 0)
snap("SAVED")
log("RESULT start_minutes=%d end_minutes=%d rng_end=%#x" % (start_min, end_min, rd32(0x55D2C0)))
log("DONE %.1f" % (time.time() - t0))
gdb.execute("quit")
