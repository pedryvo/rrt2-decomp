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
- Feature: `.MAP`/`.GAM` parser (`rt2_formats::map`): header, tile records and corner elevations for all 31 maps and both saves.
- Feature: `rt2-render` (wgpu terrain mesh, 2:1 isometric camera) and `rt2-viewer`, which opens an original map with pan, zoom and 90-degree rotation, or writes a screenshot with `--screenshot`.
- RE: calendar recovered from the executable: 12 months of 28 days, minute resolution, absolute minutes `((((y*12+m)*28+d-29)*24+h)*60)+min` (0x4a4950, 0x4a4990, 0x4a4a30), checked against the original in the emulator for 20000 dates.
- RE: game speed is an index 0..10 (default 3) into the table 0, 0.3, 0.6, 1, 1.7, 2.1, 3, 5, 8, 12, 18; game minutes per frame = ftol(table * ms * 3.2), so one day lasts 450 ms at speed 3 (0x46c650, 0x536508).
- RE: main loop and tick structure documented in `docs/spec/time.md`: frame gate every >100 ms (0x404e30), `sim_advance` (0x4a5cb0) with week end on days 7/14/21/28, month end on day 28 and year end on 28 December, all at 23:59, plus the scenario-event trigger kinds fired around each.
- RE: the game RNG is an ANSI C LCG (state 0x55d2c0, `s*0x41c64e6d+0x3039`, output `(s>>16)&0x7fff`) with an inclusive-range wrapper (0x4f0660), a stateless seed-hash variant (0x4f06f0) and the MSVC CRT `rand` (0x51c137) for audio, UI and session ids; all four match the original in 20000+ emulated calls each (`docs/spec/rng.md`).
- RE: first simulation functions moved to spec/verified in `re/coverage.toml` (22 of 31 classified simulation functions), 73 symbols in `re/symbols.toml`.
- Docs: `docs/spec/time.md` and `docs/spec/rng.md`, the first P0 simulation specs, with open questions (x87 precision, entity list identities, CRT rand in train code).
