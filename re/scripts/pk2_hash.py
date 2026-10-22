#!/usr/bin/env python3
"""PK2 filename hash of RT2_PLAT.EXE (stdlib only).

Source: FUN_004f4e60 / FUN_004f5070 / FUN_004f48e0 (loop with 0x41c64e6e),
case folder FUN_004f0d70. See docs/formats/pk2.md.

Usage:
  pk2_hash.py NAME...                      print hash of each name
  pk2_hash.py --check DATA_DIR NAMES_FILE  count names present in the *.PK2 directories
"""
import glob
import os
import struct
import sys

M = 0xFFFFFFFF


def fold(c: int) -> int:
    """FUN_004f0d70: 'a'..'z' and bytes > 0xEF lose 0x20 (ASCII upper-case)."""
    return (c - 0x20) & 0xFF if (96 < c < 123 or c > 0xEF) else c


def pk2_hash(name: str) -> int:
    h = 7
    for c in name.encode("latin-1"):
        h = (fold(c) + 0x3039 + h * 0x41C64E6E) & M
    return h


def read_dir(path):
    with open(path, "rb") as f:
        n = struct.unpack("<I", f.read(4))[0]
        raw = f.read(12 * n)
    return [struct.unpack_from("<III", raw, 12 * i) for i in range(n)]


def check(data_dir, names_file):
    hashes = set()
    for p in glob.glob(os.path.join(data_dir, "*.PK2")) + glob.glob(os.path.join(data_dir, "*.pk2")):
        hashes |= {e[0] for e in read_dir(p)}
    names = [l.strip() for l in open(names_file, encoding="latin-1") if l.strip()]
    hit = [n for n in names if pk2_hash(n.replace("/", "\\").split("\\")[-1]) in hashes]
    print(f"{len(hit)}/{len(names)} names found in {len(hashes)} distinct hashes")


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["--check"]:
        check(a[1], a[2])
    else:
        for n in a:
            print(f"{pk2_hash(n):#010x}  {n}")
