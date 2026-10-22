#!/usr/bin/env python3
"""Validacao do advance_save.py (docs/dynamic-analysis.md, secao 5): datas exatas, determinismo e
efeito da semente. Uso: advance_check.py <sandbox> <Tutorial.GAM> <pasta_saida>
O gravador do original escreve a versao 0x2348 (Save::parse so le 0x233C), por isso a data e lida
direto do bloco de estado: arquivo 0x2348 = versao(4) + cabecalho(0x898a) + 0x86 + vista(0x40) + estado."""
import hashlib, os, struct, subprocess, sys
HERE = os.path.dirname(os.path.realpath(__file__))
STATE = 4 + 0x898A + 0x86 + 0x40          # 0x8A54: game_running; +0xC minutos, +0x10 data, +0x14 hora
def info(path):
    b = open(path, "rb").read()
    assert struct.unpack_from("<I", b, 0)[0] == 0x2348, "versao inesperada"
    mins, = struct.unpack_from("<i", b, STATE + 0xC)
    year, mon, day = struct.unpack_from("<HBB", b, STATE + 0x10)
    hh, mm = struct.unpack_from("<BB", b, STATE + 0x14)
    return dict(minutes=mins, date=(year, mon, day, hh, mm), size=len(b), md5=hashlib.md5(b).hexdigest())
def adv(days, y, m, d, hh, mi):                   # calendario 12x28
    t = ((((y * 12 + m) * 28 + d - 29) * 24 + hh) * 60) + mi + days * 1440
    r = t % 483840
    return (t // 483840, r // 40320 + 1, (r % 40320) // 1440 + 1, (r % 1440) // 60, r % 60)
def run(sb, inp, days, seed, out):
    subprocess.run([sys.executable, os.path.join(HERE, "advance_save.py"), sb, inp, str(days), str(seed), out],
                   check=True, stdout=subprocess.DEVNULL)
    return info(out)
sb, inp, od = sys.argv[1:4]
os.makedirs(od, exist_ok=True)
start = (1870, 2, 4, 19, 3)
ok = True
res = {}
for days, seed, tag in [(1, 1234, "d1"), (7, 1234, "d7"), (30, 1234, "d30a"), (30, 1234, "d30b"), (30, 777, "d30c")]:
    r = run(sb, inp, days, seed, os.path.join(od, tag + ".GAM")); res[tag] = r
    want = adv(days, *start)
    good = r["date"] == want
    ok &= good
    print("%-5s dias=%-2d seed=%-4d data=%s esperado=%s minutos=%d md5=%s %s" %
          (tag, days, seed, r["date"], want, r["minutes"], r["md5"][:12], "OK" if good else "ERRO"))
same = res["d30a"]["md5"] == res["d30b"]["md5"]
diff = res["d30a"]["md5"] != res["d30c"]["md5"]
print("determinismo (mesma semente, 30 dias): %s" % ("byte a byte identico" if same else "DIFERENTE"))
print("semente diferente muda o save: %s" % ("sim" if diff else "NAO"))
sys.exit(0 if ok and same and diff else 1)
