"""Decodificador experimental de sprites 'LAD' dos PK2 do RT2 (stdlib + Pillow).
Uso: python -I lad.py <blob> [paleta.bin] [saida_prefixo]
Hipoteses em docs/formats/pk2-contents.md"""
import struct, sys

def parse(b):
    """Arquivo = u32 N + N registros. Registro = 60 bytes de cabecalho [u32 tipo, u32 hx, 14 x u32 (pares 0/1 ou 0/-1)]
    + u32 tag (lixo de ponteiro 0x82xxxxxx) + u32 nlayers + nlayers x camada.
    Camada = u32 1, 7 bytes (01 00 2a 00 00 00 00), u32 ltype (3=RLE, 2=cru), 4 x i32 (x, y, largura, altura), u32 tag, u32 size, dados."""
    n = struct.unpack_from('<I', b, 0)[0]
    pos = 4
    recs = []
    for _ in range(n):
        if pos + 68 > len(b): break
        rtype, hx = struct.unpack_from('<II', b, pos)
        nl = struct.unpack_from('<I', b, pos + 64)[0]
        pos += 68
        layers = []
        while True:  # nl (u32) nem sempre bate com o numero de camadas; le ate falhar
            hit = None
            # forma (a): u32 k, u8 cnt, cnt bytes, flag, u32 0, u32 tipo ; forma (b): flag, u32 0, u32 tipo
            cands = []
            if pos + 5 <= len(b) and list(b[pos + 5: pos + 5 + b[pos + 4]]) == list(range(b[pos + 4])): cands.append(pos + 5 + b[pos + 4])
            cands.append(pos)
            for p in cands:
                if p + 33 > len(b): continue
                ltype = struct.unpack_from('<I', b, p + 5)[0]
                if struct.unpack_from('<I', b, p + 1)[0] != 0 or ltype not in (1, 2, 3): continue
                x0, y0, x1, y1 = struct.unpack_from('<4i', b, p + 9)
                tag, size = struct.unpack_from('<II', b, p + 25)
                if x1 < 0 or y1 < 0 or p + 33 + size > len(b): continue
                hit = (p, ltype, (x0, y0, x1, y1), tag, size, b[p]); break
            if not hit: break
            p, ltype, bbox, tag, size, flag = hit
            layers.append(dict(type=ltype, bbox=bbox, tag=tag, flag=flag, data=b[p + 33: p + 33 + size], off=pos, idx=list(b[pos + 5: p]) if p != pos else None))
            pos = p + 33 + size
        recs.append(dict(type=rtype, hx=hx, layers=layers))
    return dict(n=n, recs=recs, end=pos)

def decode_rle(data, w, h):
    """Linhas: [len][tokens...]; len inclui o proprio byte e o 00 final.
    token >= 0xC0: pula (b-0xC0) px (somam-se consecutivos); token < 0xC0 (>0): n literais."""
    out = bytearray([0] * (w * h)); mask = bytearray(w * h)
    i = 0; y = 0
    while i < len(data) and y < h:
        ln = data[i]; hdr = 1
        if ln & 0x80:                       # comprimento de 2 bytes (big-endian, 15 bits) para linhas longas
            if i + 1 >= len(data): break
            ln = ((ln & 0x7F) << 8) | data[i + 1]; hdr = 2
        if ln < hdr + 1: i += 1; continue
        row = data[i + hdr: i + ln]; i += ln
        j = 0; x = 0
        while j < len(row):
            t = row[j]; j += 1
            if t == 0: break
            if t >= 0xC0: x += t - 0xC0
            else:
                for q in range(t):
                    if j >= len(row): break
                    if 0 <= x < w:
                        out[y * w + x] = row[j]; mask[y * w + x] = 1
                    j += 1; x += 1
        y += 1
    return out, mask, y

def load_palette(blob):
    """blob de paleta: u32 n, 35 bytes de cabecalho, n*512 bytes (256 x u16). Retorna lista de n paletas RGB."""
    n = struct.unpack_from('<I', blob, 0)[0]
    pals = []
    for p in range(n):
        raw = blob[39 + 512 * p: 39 + 512 * (p + 1)]
        pal = []
        for k in range(256):
            v = struct.unpack_from('<H', raw, 2 * k)[0]
            pal.append((((v >> 11) & 31) * 255 // 31, ((v >> 5) & 63) * 255 // 63, (v & 31) * 255 // 31))
        pals.append(pal)
    return pals

if __name__ == '__main__':
    from PIL import Image
    b = open(sys.argv[1], 'rb').read()
    pal = load_palette(open(sys.argv[2], 'rb').read())[0] if len(sys.argv) > 2 else [(i, i, i) for i in range(256)]
    pre = sys.argv[3] if len(sys.argv) > 3 else 'out'
    s = parse(b)
    print('N', s['n'], 'registros', len(s['recs']), 'fim', s['end'], 'tamanho', len(b))
    for r, rec in enumerate(s['recs']):
        for i, f in enumerate(rec['layers']):
            x0, y0, w, h = f['bbox']
            if f['type'] == 3: px, m, rows = decode_rle(f['data'], w, h)
            else: px = f['data']; m = bytes([1]) * len(px); rows = h
            img = Image.new('RGBA', (w, h))
            img.putdata([tuple(pal[px[k]]) + ((255,) if m[k] else (0,)) for k in range(w * h)])
            img.save(f'{pre}_{r}_{i}.png')
            print(r, i, rec['type'], rec['hx'], f['type'], f['bbox'], w, h, len(f['data']), 'rows', rows)
