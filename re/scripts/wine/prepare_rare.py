#!/usr/bin/env python3
"""Gera os saves preparados dos eventos raros do placar de fidelidade (docs/dynamic-analysis.md 5.9).

Cada receita parte de um save gravado PELO ORIGINAL (a conversao de 0 dias que scripts/fidelity.py
guarda em ~/.cache/rrt2/fidelity/<exe>/, ou uma saida N dias de advance_save.py) e escreve campos
crus com `rt2-savepatch` (checksum recalculado). O placar roda o original sobre o arquivo
preparado (0 dias = base do clone, N dias = oraculo): o carregador do original valida o arquivo e
a comparacao parte da conversao dele, nunca do patch.

Os saves contem dados do jogo: ficam em ~/.cache/rrt2/rare/ (fora do repositorio).

Uso: python3 -I re/scripts/wine/prepare_rare.py [--only NOME] [--list]
Pre-requisito: `python3 -I scripts/fidelity.py --only Example` (gera a base do Example).
"""
import argparse, hashlib, os, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.realpath(__file__)))))
INSTALL = os.environ.get("RT2_INSTALL", "/home/pedryvo/Games/railroad-tycoon-2")
OUT = os.path.expanduser("~/.cache/rrt2/rare")
PATCH = os.path.join(ROOT, "target", "release", "rt2-savepatch")
ADVANCE = os.path.join(ROOT, "re", "scripts", "wine", "advance_save.py")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def fidelity_cache():
    exe = os.path.join(INSTALL, "RT2_PLAT.EXE.orig")
    if not os.path.isfile(exe):
        exe = os.path.join(INSTALL, "RT2_PLAT.EXE")
    return os.path.expanduser("~/.cache/rrt2/fidelity/" + sha256(exe)[:16])


def example_base():
    """A conversao de 0 dias do Example pelo original (scripts/fidelity.py, cenario Example)."""
    src = os.path.join(INSTALL, "games", "Example save game.GAM")
    p = os.path.join(fidelity_cache(), "Example__%s__s1234__d0.GAM" % sha256(src)[:8])
    if not os.path.isfile(p):
        sys.exit("falta %s: rode `python3 -I scripts/fidelity.py --only Example`" % p)
    return p


def advanced(base, days, name):
    """`base` avancado `days` dias pelo original (semente 1234), em cache."""
    out = os.path.join(OUT, "_adv_%s_d%d.GAM" % (name, days))
    if os.path.isfile(out):
        return out
    sb = os.path.expanduser("~/.cache/rrt2/fidelity/wine/rt2sb")
    r = subprocess.run([sys.executable, "-I", ADVANCE, sb, base, str(days), "1234", out,
                        "--timeout", "900"])
    if r.returncode or not os.path.isfile(out):
        sys.exit("advance_save falhou para " + name)
    return out


def tr(slot, off, ty, val):
    return "trains[%d]+%s:%s=%s" % (slot, off, ty, val)


# Trens da IA (empresa 5) do Example: 3, 4, 5, 6, 8; do jogador (empresa 4): 1, 7, 9, 10.
AI_TRAINS = (3, 4, 5, 6, 8)


def crash_patches():
    """Locomotivas velhas (compradas em 1700), sem oleo e com acelerador 250: a chance de quebra
    passa de 1 nas primeiras verificacoes; a semente do placar decide quebra ou acidente."""
    out = []
    for t in AI_TRAINS:
        out += [tr(t, "0xbd", "u16", 1700), tr(t, "0xd3", "f32", 0), tr(t, "0xdb", "f32", 250)]
    return out


def crash_human_patches():
    """Como `crash`, nos trens do jogador (empresa 4, a da interface): mensagem, som e o teto de
    velocidade do acidente (0x4ce730) e da quebra (0x4ce4c0) da empresa vista."""
    out = []
    for t in (1, 7, 9, 10):
        out += [tr(t, "0xbd", "u16", 1700), tr(t, "0xd3", "f32", 0), tr(t, "0xdb", "f32", 250)]
    return out


def theft_patches():
    """Roubo de trem: `0x573e02` (state+0x391a) = 3000% e um ladrao nomeavel na tabela
    0x573da2 (entrada 0: nome, flags +0x15 = 2, ano +0x16 = 1800)."""
    name = b"Jesse James"
    out = ["state+0x391a:i16=3000"]
    for i, b in enumerate(name + b"\0"):
        out.append("state+%#x:u8=%d" % (0x38ba + i, b))
    out += ["state+%#x:u8=2" % (0x38ba + 0x15), "state+%#x:i16=1800" % (0x38ba + 0x16)]
    return out


def theft_year_patches():
    """Volta anual do roubo: `0x573e02` = 50 abaixo da base `0x573d9e` = 2000 (state+0x38b6, i32);
    no 1o de janeiro (dia ~19 do Example) sobe max((base - pct) * 0.05, 1)."""
    return ["state+0x391a:i16=50", "state+0x38b6:i32=2000"]


def co(slot, off, ty, val):
    return "companies[%d]+%s:%s=%s" % (slot, off, ty, val)


def fire_patches():
    """Demissao do chairman da IA na revisao anual (dia ~19): empresa 5 com reputacao -60
    (+0x46a), ja avisada este ano (+0x472 = 1875), metricas arquivadas do ano anterior ruins
    (reputacao -50, lucro por acao 0) e prejuizo no ano (categoria 0 = -1e6)."""
    return [co(5, "0x46a", "i32", -60), co(5, "0x472", "i16", 1875),
            co(5, "0x4a86", "f64", -50), co(5, "0x4a3e", "f64", 0),
            co(5, "0x4286", "f64", -1e6)]


def liquidate_patches():
    """Liquidacao da empresa 5 pelos credores (13.8): tres anos seguidos com preco baixo,
    patrimonio negativo, prejuizo e caixa negativo (ano corrente e arquivos 1 e 2)."""
    out = [co(5, "0x4766", "f64", -5e7), co(5, "0x479e", "f64", 5e7),
           co(5, "0x4286", "f64", -1e7)]
    for n in (1, 2):
        out += [co(5, hex(0x49b6 + 0x70 * n), "f64", 1.0),
                co(5, hex(0x47a6 + 0x40 * (n - 1)), "f64", -5e7),
                co(5, hex(0x42ee + 0x68 * (n - 1)), "f64", -1e7)]
    return out


def stocks_patches():
    """Bolsa da IA (ai_person_trade_stocks 0x4a8af0): a pessoa 2 (chairman da empresa 5) com
    $5 000 000 em caixa (+0x12a) compra lotes de 1000 acoes nos turnos mensais."""
    return ["persons[2]+0x12a:f64=5000000"]


def engine_patches():
    """Troca de locomotiva no trilho (0x4d4e90): os trens 3 e 4 da IA com motor de 1800 (+0xa9);
    o turno de dezembro (ai_manage_trains) troca os de mais de 25 anos (+0x4243 = 0: revisao
    dos motores ainda nao feita no ano)."""
    return [tr(3, "0xa9", "u16", 1800), tr(4, "0xa9", "u16", 1800),
            co(5, "0x4243", "u16", 0)]


def flood_patches():
    """Inundacao de territorio (efeito 233, 0x4c8a30 -> 0x490490 modo 3): regra de inicio de mes,
    uma vez so, no territorio 1 com nivel minimo + 12 (alaga o mar, a costa, 47 tiles de trilho e
    3 estacoes da empresa 5)."""
    return ["rule:2|TerritoryId=1|233=12"]


def confstations_patches():
    """Destruicao das estacoes da empresa 5 (efeito 302, 0x437f50(5, 0, 1) -> building_destroy
    0x40adb0 -> station_remove 0x4bf4e0) no inicio do mes."""
    return ["rule:2|CompanyId=5|302=1"]


def deactivate_patches():
    """Desativacao da empresa 5 (efeito 189, 0x438170(5, 1)): trens apagados, estacoes demolidas
    e trilho removido (0x437ab0(5, 0, 1)) no inicio do mes."""
    return ["rule:2|CompanyId=5|189=1"]


RECIPES = {
    # nome: (funcao da base, funcao dos patches)
    "crash": (example_base, crash_patches),
    "crashhuman": (example_base, crash_human_patches),
    "theft": (example_base, theft_patches),
    "theftyear": (example_base, theft_year_patches),
    "fire": (example_base, fire_patches),
    "liquidate": (example_base, liquidate_patches),
    "stocks": (example_base, stocks_patches),
    "engine": (example_base, engine_patches),
    "flood": (example_base, flood_patches),
    "confstations": (example_base, confstations_patches),
    "deactivate": (example_base, deactivate_patches),
}


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--only", action="append")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()
    if args.list:
        for k, (_, f) in RECIPES.items():
            print("%-12s %s" % (k, (f.__doc__ or "").split("\n")[0]))
        return
    r = subprocess.run(["cargo", "build", "--release", "-p", "rt2-tools", "--bin", "rt2-savepatch"],
                       cwd=ROOT)
    if r.returncode:
        sys.exit("cargo build falhou")
    os.makedirs(OUT, exist_ok=True)
    for name, (base, patches) in RECIPES.items():
        if args.only and name not in args.only:
            continue
        out = os.path.join(OUT, name + ".GAM")
        cmd = [PATCH, base(), out]
        for p in patches():
            if p.startswith("rule:"):
                cmd += ["--add-rule", p[5:]]  # regra de cenario (rt2-savepatch --add-rule)
            else:
                cmd += ["--set", p]
        r = subprocess.run(cmd)
        if r.returncode:
            sys.exit("rt2-savepatch falhou em " + name)
        print("%s: %s" % (name, out))


if __name__ == "__main__":
    main()
