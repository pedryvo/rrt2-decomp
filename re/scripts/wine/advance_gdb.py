"""Executado DENTRO do gdb (source) por advance_save.py. Nao rodar direto.
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
def fcall(addr, ecx=0, edx=0, args=(), hooks=None):
    """Chama addr (fastcall: ECX, EDX; resto na pilha, cdecl) e devolve EAX. `hooks`: endereco ->
    funcao chamada quando um breakpoint (ja criado) para nele no meio da chamada."""
    sp = SP0 - 0x1000 - 4 * len(args) - 4
    wr32(sp, RET)
    for i, a in enumerate(args): wr32(sp + 4 + 4 * i, a)
    setreg("esp", sp); setreg("ecx", ecx); setreg("edx", edx); setreg("eip", addr)
    gdb.execute("continue", to_string=True)
    while hooks and reg("eip") in hooks:
        hooks[reg("eip")]()
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

# 2c. ADV_PRESEED: fixa o RNG do jogo e o rand do CRT ANTES da carga (a conversao de versoes antigas
#     e as passadas de terreno do carregador consomem RNG; com isto o resultado e reproduzivel no clone)
pre = env.get("ADV_PRESEED")
if pre is not None:
    wr32(0x55D2C0, int(pre) & 0xFFFFFFFF); wr32(0x5609F0, int(pre) & 0xFFFFFFFF)
    log("PRESEED %#x" % (int(pre) & 0xFFFFFFFF))

newgame = env.get("ADV_NEWGAME")
if newgame:
    # 3'. New Game de um cenario da campanha: 0x410de0 (inicio da campanha, docs/dynamic-analysis.md
    #     5.8) com [0x566d3d] = conjunto (0 = .MAP originais, 1 = #E..CAMP.MP2) e [0x539c84] = indice.
    #     Monta o bloco de opcoes 0x574840 (0x4b3ac0, 0x4b45b0), sorteia game_seed e o RNG
    #     (crt_rand_range + clock_ms: trocados aqui por ADV_GSEED e ADV_RSEED) e chama 0x479b20.
    cset, cidx = (int(v) for v in newgame.split(":"))
    gseed, rseed = int(env["ADV_GSEED"]) & 0xFFFFFFFF, int(env["ADV_RSEED"]) & 0xFFFFFFFF
    wr(0x566D3D, bytes([cset])); wr32(0x539C84, cidx)
    gdb.Breakpoint("*0x410f64", internal=True)    # mov [0x57486e], esi  (game_seed)
    gdb.Breakpoint("*0x410f89", internal=True)    # call rng_seed(ecx)
    def set_gseed(): setreg("esi", gseed)
    def set_rseed(): setreg("ecx", rseed)
    hooks = {0x410F64: set_gseed, 0x410F89: set_rseed}
    # ADV_TRACE="0xaddr,0xaddr": registra ECX, EDX, u16 [ECX] e o RNG em cada chamada (sem parar)
    for a in filter(None, env.get("ADV_TRACE", "").split(",")):
        a = int(a, 0)
        gdb.Breakpoint("*0x%x" % a, internal=True)
        def tr(a=a):
            e = reg("ecx")
            try: w = struct.unpack("<H", rd(e, 2))[0]
            except Exception: w = -1
            log("TRACE %#x ecx=%#x edx=%#x [ecx]=%d rng=%#x" % (a, e, reg("edx"), w, rd32(0x55D2C0)))
        hooks[a] = tr
    # ADV_HOOKPY=<arquivo.py>: executado aqui; pode registrar mais breakpoints em `hooks`
    if env.get("ADV_HOOKPY"):
        exec(open(env["ADV_HOOKPY"]).read())
    fcall(0x410DE0, 0, 0, hooks=hooks)
    wr32(0x574C04, 1)                     # 0x479b20 liberou o porteiro no fim
    log("NEWGAME set=%d idx=%d game_seed=%#x rng_seed=%#x" % (cset, cidx, gseed, rseed))
else:
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
log("LOAD_RNG rng=%#x crt=%#x" % (rd32(0x55D2C0), rd32(0x5609F0)))
if rd32(0x5704E8) != 1 or rd32(0x566E14) != 1:
    log("ERRO: carga nao colocou o jogo em andamento"); raise RuntimeError("load")
start_min = rd32(0x5704F4)

# 4. RNG: a carga nao restaura a sequencia; fixa a semente do jogo (0x55d2c0) e a do CRT (0x5609f0)
wr32(0x55D2C0, seed); wr32(0x5609F0, seed)
if env.get("ADV_CRT"):  # env ADV_CRT=0x..: CRT state of a continuous run (resume from its save)
    wr32(0x5609F0, int(env["ADV_CRT"], 0) & 0xFFFFFFFF)

# 5. avanca: sim_advance(ECX = minutos) em passos fixos, sem o porteiro.
#    ADV_SKIP_CHOICES=1 (advance_save.py --skip-choices): a escolha de regra de cenario
#    (rule_show_message 0x455e70 -> 0x4510a0, janela MODAL que esperaria um clique) e pulada na
#    chamada 0x455ef7: devolve o primeiro botao (EAX = 0, nao 0x67), como o clone fora do modo
#    interativo (scenarios.md 2.4.1). 9 argumentos, ret 0x24. So quando pedido (o breakpoint
#    deixa cada passo mais lento; antes da janela de mensagens fixa abaixo isso mudava o som da
#    caixa registradora e o rand do CRT em runs longos).
CHOICE_CALL = 0x455EF7
if env.get("ADV_SKIP_CHOICES"):
    gdb.Breakpoint("*0x%x" % CHOICE_CALL, internal=True)
def skip_choice():
    setreg("eax", 0); setreg("esp", reg("esp") + 0x24); setreg("eip", CHOICE_CALL + 5)
    log("CHOICE pulada (botao 0) minutes=%d" % rd32(0x5704F4))
adv_hooks = {CHOICE_CALL: skip_choice}
# Janela das caixas de mensagem (0x46aa90): uma mensagem de prioridade menor que a anterior e
# descartada enquanto clock_ms() <= [0x574d24] (prazo de 9 s apos uma quebra, 0x46adb0); a caixa
# aceita guarda a mensagem em 0x574430, de onde 0x46abf0 tira o valor da entrega que decide o som
# da caixa registradora (0x47d100, um crt_rand_range(0, 2)). Sem jogo em tempo real o prazo
# depende da velocidade da maquina; aqui a janela fica sempre aberta ([0x574d20] = 0 na entrada),
# para o oraculo nao depender do relogio de parede (docs/fidelity-status.md, longo prazo).
class MsgWindowOpen(gdb.Breakpoint):
    def __init__(self):
        super().__init__("*0x46aa90", internal=True)
    def stop(self):
        wr32(0x574D20, 0)
        return False
MSGBP = MsgWindowOpen()
# ADV_ADVHOOKPY=<arquivo.py>: executado antes do avanco (depois da carga e das sementes); pode criar
# breakpoints e registrar funcoes em `adv_hooks[endereco]` (ex.: despejar listas de sobreposicao).
if env.get("ADV_ADVHOOKPY"):
    exec(open(env["ADV_ADVHOOKPY"]).read())
remaining = 1440 * days
t1 = time.time()
while remaining > 0:
    d = min(step, remaining)
    fcall(0x4A5CB0, d, hooks=adv_hooks)
    remaining -= d
snap("ADVANCED")
log("advance_seconds=%.2f" % (time.time() - t1))
end_min = rd32(0x5704F4)
if end_min - start_min != 1440 * days:
    log("ERRO: avanco de %d minutos, esperado %d" % (end_min - start_min, 1440 * days))
    raise RuntimeError("advance")

# 6. grava: gravador 0x477540 (ECX = caminho, EDX = 0 -> arquivo em disco)
fcall(0x477540, buf + 0x200, 0)
snap("SAVED")
log("RESULT start_minutes=%d end_minutes=%d rng_end=%#x crt_end=%#x" % (start_min, end_min, rd32(0x55D2C0), rd32(0x5609F0)))
log("DONE %.1f" % (time.time() - t0))
gdb.execute("quit")
