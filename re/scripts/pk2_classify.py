"""Classifica os blobs desempacotados dos PK2 (NNNN_hash.bin) por formato hipotetico.
Uso: python -I pk2_classify.py <dir_com_subpastas_RT2,RT2_06,...>
Categorias: lad (sprite RLE), pal16 (banco de paletas 16 bit), rgb768, lut32768, imb (layout de UI),
tabela de paletas, audio-pcm16 (heuristica) e other."""
import os, sys, struct, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lad

def classify(b):
    n = struct.unpack_from('<I', b, 0)[0] if len(b) >= 4 else 0
    if len(b) >= 72 and b[4:8] in (b'\x03\0\0\0', b'\x01\0\0\0') and n < 100000:
        try:
            s = lad.parse(b)
            return 'lad' if s['end'] == len(b) and len(s['recs']) == s['n'] else 'lad-parcial'
        except Exception:
            return 'lad-erro'
    if n in (1, 2, 3, 4, 32, 35) and len(b) == 39 + 512 * n: return 'pal16'
    if len(b) == 768: return 'rgb768'
    if len(b) == 32768: return 'lut32768'
    if b[:4] == b'\xd0\x07\0\0': return 'imb'
    if len(b) == 4 + 12 * n and n > 0 and all(struct.unpack_from('<I', b, 4 + 12 * i)[0] == 0 for i in range(min(n, 8))): return 'tab-paletas'
    if n == len(b) - 4: return 'tabela-len'
    return 'other'

if __name__ == '__main__':
    R = sys.argv[1]
    cl = collections.defaultdict(list)
    for d in sorted(os.listdir(R)):
        for f in sorted(os.listdir(os.path.join(R, d))):
            b = open(os.path.join(R, d, f), 'rb').read()
            cl[classify(b)].append((d, f, len(b)))
    for k, v in sorted(cl.items(), key=lambda x: -len(x[1])):
        sz = [x[2] for x in v]
        print(f'{k:12} n={len(v):5} min={min(sz):8} max={max(sz):8} exemplo={v[0][0]}/{v[0][1]}')
