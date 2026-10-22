# Changelog

Entry format: `- <Kind>: text`, where Kind is Feature, Improved, Change, Fix, Docs or RE (reverse-engineering finding). Append `(original bug)` to fixes of bugs present in the original game.

## Unreleased

- Feature: PK2 archive reader with filename-hash lookup and the `rt2-extract` tool.
- Feature: LNG string-table parser (last definition wins, named `;t<NAME>_0` tables).
- RE: PK2 filename hash, archive load order and precedence recovered from the executable.
- Docs: development plan, file-format specs for PK2 and LNG, project README.
- RE: PK2 contents classified: LAD sprite format (8-bit indexed, per-row RLE, layered), palette banks, RGB palettes, IMB layouts and raw PCM audio.
- Docs: game rules, formulas and data tables extracted from the manual, strategy guide and editor notes (docs/reference/manual-rules.md).
- RE: sprite palette rule recovered: a LAD record's second header word selects the bank `PALT%04d.PAL` (loader at 0x4f8b80, palette reader at 0x4f8ed0); palette words are XRGB1555.
- RE: LAD layout corrected (records of image groups with frame maps); all 1990 sprite banks parse to the last byte.
- Feature: LAD sprite and palette-bank parsers with RGBA conversion in `rt2-formats`, and the `rt2-sprite` tool that renders an entry to PNG.
- Docs: `docs/formats/lad.md` with the sprite layout, palette rule and source addresses.
- Feature: `rt2-oracle` differential-testing harness: a PE32 loader plus Unicorn emulation that runs single functions of the original executable (cdecl/stdcall/thiscall, IAT stubs that name any OS import touched, .data snapshot/restore, x87 ST0 readout).
- RE: the PK2 name-hash case fold (0x4f0d70) matches `rt2_formats::pk2::fold_name_byte` for all 256 byte values, checked against the original in the emulator.
- Docs: `docs/oracle.md` explains how to write a differential test.
- RE: `maps/*.MAP` and `games/*.GAM` are uncompressed: header with map size at 0x231, 10-byte tile records (ground, trees, structure, track, territory) from 0xB2D7 and an i16 corner heightmap of (w+1)*(h+1); renders reproduce Britain, Chesapeake and the other 31 maps, see `docs/formats/map-gam.md` and `re/scripts/mapgam.py`.
- RE: automatic first-pass classification of all 2945 functions Ghidra finds in RT2_PLAT.EXE (`re/coverage.toml`: 680 classified, 262 lib, 4 sim candidates), produced by `re/scripts/dump_program.sh` and `re/scripts/classify.py`.
