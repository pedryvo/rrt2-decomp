#!/usr/bin/env python3
"""Leitor de primeira passada de maps/*.MAP e games/*.GAM do RT2 (numpy + Pillow).

Ver docs/formats/map-gam.md. Tudo little-endian.

Uso:
  mapgam.py info  ARQ...                 cabecalho, tamanho do mapa, secoes conhecidas
  mapgam.py png   ARQ SAIDA_PREFIXO      gera PNGs: elevacao, tipo de chao, flags, ids, regioes
  mapgam.py sheet DIR SAIDA.png          contact sheet da elevacao de todos os *.MAP de DIR
  mapgam.py terrain ARQ SAIDA.png [ESC]  relevo sombreado: agua azul, terra por altura, arvores verdes
  mapgam.py strings ARQ                  strings ASCII da cauda (apos a elevacao) com offsets

Layout (confianca alta salvo indicacao):
  0x000 u32 0x233C, nome (NUL-padded)
  0x231 u16 largura, u16 altura (tiles); 0x23b nome curto; 0x25a descricao
  0x390 preview/minimapa (baixa confianca)
  0xB2D6 1 byte, 0xB2D7: w*h registros de 10 bytes, linha a linha (campos em FIELDS)
  depois: elevacao i16 (w+1)*(h+1) (cantos dos tiles), colada ao fim dos tiles
  depois: estado/objetos (jogadores, empresas, lugares, eventos) - ver docs
"""
import struct
import sys

import numpy as np

TILE_START = 0xB2D7
REC = 10
FIELDS = ["var", "ground", "flags2", "trees", "struct_id", "zero5", "flags6", "track_id", "zero8", "region"]


def parse(path):
    raw = open(path, "rb").read()
    magic = struct.unpack_from("<I", raw, 0)[0]
    if magic != 0x233C:
        raise ValueError("magic inesperada %#x" % magic)
    name = raw[4:raw.index(0, 4)].decode("latin1")
    w, h = struct.unpack_from("<HH", raw, 0x231)
    n = w * h
    buf = np.frombuffer(raw, np.uint8)
    tiles = buf[TILE_START:TILE_START + REC * n].reshape(h, w, REC)
    eo = TILE_START + REC * n
    elev = buf[eo:eo + 2 * (w + 1) * (h + 1)].view("<i2").reshape(h + 1, w + 1)
    mini = buf[0x390:0x390 + 170 * 170].reshape(170, 170)
    return dict(raw=raw, name=name, w=w, h=h, tiles=tiles, elev=elev, elev_off=eo,
                after_off=eo + 2 * (w + 1) * (h + 1), mini=mini)


def png_gray(a, path, lo=None, hi=None, scale=1):
    from PIL import Image
    a = a.astype(np.float64)
    if lo is None:
        lo, hi = np.percentile(a, [1, 99])
    b = np.clip((a - lo) / max(hi - lo, 1e-9) * 255, 0, 255).astype(np.uint8)
    im = Image.fromarray(b)
    if scale != 1:
        im = im.resize((im.width * scale, im.height * scale), 0)
    im.save(path)


def hashcolor(a):
    """Falsa cor deterministica para ids/categorias (0 = preto)."""
    a = a.astype(np.uint32)
    r = (a * 97 + 53) & 255
    g = (a * 57 + 131) & 255
    b = (a * 29 + 199) & 255
    rgb = np.stack([r, g, b], -1).astype(np.uint8)
    rgb[a == 0] = 0
    return rgb


def cmd_info(paths):
    for p in paths:
        m = parse(p)
        print("%-24s %4dx%-4d tiles@%#x elev@%#x fim_elev@%#x tamanho=%d resto=%d" % (
            m["name"][:24], m["w"], m["h"], TILE_START, m["elev_off"], m["after_off"],
            len(m["raw"]), len(m["raw"]) - m["after_off"]))


def cmd_png(path, prefix):
    from PIL import Image
    m = parse(path)
    t, e = m["tiles"], m["elev"].astype(np.int32)
    e2 = np.where(e > 2000, 0, e)
    png_gray(e2, prefix + "_elev.png")
    Image.fromarray(hashcolor(t[:, :, 1])).save(prefix + "_ground.png")
    Image.fromarray(hashcolor(t[:, :, 9])).save(prefix + "_region.png")
    png_gray(t[:, :, 3], prefix + "_trees.png", 0, 30)
    Image.fromarray(hashcolor(t[:, :, 6])).save(prefix + "_flags6.png")
    Image.fromarray(hashcolor(t[:, :, 4])).save(prefix + "_struct.png")
    Image.fromarray(hashcolor(t[:, :, 7])).save(prefix + "_track.png")


WATER = (0x61, 0x62)


def cmd_terrain(path, out, scale=2):
    from PIL import Image
    m = parse(path)
    t = m["tiles"]
    e = m["elev"].astype(np.float64)
    ev = (e[:-1, :-1] + e[1:, :-1] + e[:-1, 1:] + e[1:, 1:]) / 4
    water = np.isin(t[:, :, 1], WATER)
    # sombreamento simples (luz NW) a partir do gradiente
    gy, gx = np.gradient(ev)
    shade = np.clip(1 + (gx + gy) * 0.08, 0.5, 1.5)
    lo, hi = np.percentile(ev[~water], [2, 98]) if (~water).any() else (0, 1)
    k = np.clip((ev - lo) / max(hi - lo, 1), 0, 1)
    land = np.stack([90 + 120 * k, 140 + 60 * k - 40 * k * k, 60 + 130 * k * k], -1) * shade[..., None]
    wat = np.zeros_like(land)
    wat[...] = (30, 70, 140)
    rgb = np.where(water[..., None], wat, land)
    trees = t[:, :, 3] > 0
    rgb[trees & ~water] *= (0.7, 0.9, 0.6)
    im = Image.fromarray(np.clip(rgb, 0, 255).astype(np.uint8))
    im = im.resize((im.width * scale, im.height * scale), 0)
    im.save(out)


def cmd_strings(path):
    import re
    m = parse(path)
    for x in re.finditer(rb"[A-Za-z][ -~\n]{4,}", m["raw"][m["after_off"]:]):
        print("%#x  %s" % (m["after_off"] + x.start(), x.group()[:70].decode("latin1").replace("\n", " ")))


def cmd_sheet(d, out):
    import glob
    from PIL import Image
    fs = sorted(glob.glob(d + "/*.MAP"))
    sheet = Image.new("L", (6 * 260, ((len(fs) + 5) // 6) * 200))
    for k, f in enumerate(fs):
        e = parse(f)["elev"].astype(np.float64)
        e[e > 2000] = 0
        lo, hi = np.percentile(e, [1, 99])
        im = Image.fromarray(np.clip((e - lo) / (hi - lo) * 255, 0, 255).astype(np.uint8))
        im.thumbnail((256, 196))
        sheet.paste(im, ((k % 6) * 260, (k // 6) * 200))
    sheet.save(out)


if __name__ == "__main__":
    c = sys.argv[1] if len(sys.argv) > 1 else ""
    if c == "info":
        cmd_info(sys.argv[2:])
    elif c == "png":
        cmd_png(sys.argv[2], sys.argv[3])
    elif c == "terrain":
        cmd_terrain(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 2)
    elif c == "strings":
        cmd_strings(sys.argv[2])
    elif c == "sheet":
        cmd_sheet(sys.argv[2], sys.argv[3])
    else:
        print(__doc__)
