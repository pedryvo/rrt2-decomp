# Changelog

Entry format: `- <Kind>: text`, where Kind is Feature, Improved, Change, Fix, Docs or RE (reverse-engineering finding). Append `(original bug)` to fixes of bugs present in the original game.

## Unreleased

- RE: headless Ghidra export of every non-thunk function as decompiled C with our names (re/scripts/export_decomp.sh, re/db/decomp) for parallel grep-based analysis.
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
- RE: cargo table found in the executable's `.data` (`0x5404b1`, 46 records of 33 bytes: price, delivery days, distance sensitivity, first year, town/city cut-off years, demand per house, storage-upgrade bit); prices are multiplied by 1.4 at start-up, so the effective base price differs from the manual (passengers 56000 raw, 78400 effective).
- RE: delivery revenue formula recovered and verified bit-exact against the original (`0x4bf760`, 8000 random deliveries): distance factor `X = 1 - s + s*1.4*d/(15 + 0.5*(year-1800))` applied twice, a time penalty `0.3 + 0.7*(allowed - elapsed)/allowed`, demand-level price table 0.5..1.4 (expert), manager/event effects, economy state, human/computer sliders and a progressive cut above $80000 per load.
- RE: manager, event and territory effect multiplier (`0x46b480`) verified: product of `(1 + level/100)` over global, company, person and territory levels with a 0.01 floor.
- RE: industry production decoded: a table of 53 types x 4 output slots (`0x541ae0`) gives primary rates, converter ratios, bonus inputs (fertilizer 0.5/0.25, grain 0.5) and demand weights; lots are created from `483840 / (rate * effects * economy)` minutes (faster after 1930, 1950 and 1970), at most 50 waiting per slot; rate and input-list rebuild verified against the original.
- RE: cargo waits in the industry's pile, not in the station; stations load the oldest lot in their catchment, price levels 0..9 come from demand weight over a delivery counter that decays by 0.94 per month, and the economic state (`0x46a010`, every six months) scales production (0.84..1.16) and revenue (0.8..1.2).
- RE: code and manual disagree on cargo economy details: $2000 station fee per loaded car at each end (manual $1000), passenger-bonus buildings pay the station owner 10/15/5/15/25% (manual 20/30/10/20/40%) and several cargo prices and cut-off years differ; see `docs/spec/cargo.md` section 11.
- Docs: `docs/spec/cargo.md`, the cargo economy spec, with the cargo and industry tables extracted from the executable, differential tests in `crates/rt2-oracle/tests/cargo.rs` and open questions (x87 precision changes about 10% of revenues, commit-path functions untested).
- RE: formato completo do .MAP/.GAM decodificado a partir do carregador `0x477a60` e do gravador `0x477540`: blocos globais, tiles, elevacao, 15 pools de slots (estruturas, estacoes, pessoas, empresas, objetos, pecas de trilho, trens, lugares, territorios...), extras de trens e regras de cenario; data/hora e `game_seed` localizados (o estado do RNG nao e salvo)
- Feature: `rt2_formats::map::Save` le data e hora do jogo, `game_seed`, lugares (cidades) com coordenadas, empresas, pessoas, estacoes, trens e regras de cenario; os 33 `.MAP`/`.GAM` originais sao consumidos ate o ultimo byte
- RE: simulação de trens em `docs/spec/trains.md`: tabela de 66 locomotivas (`0x53cec0`) e 175 tipos de veículo (`0x53ddb8`) no `.data`, curva de velocidade `0x4cfea0` (reproduz a tabela A-1 do guia), rampa por trecho `0x4284e0`, passo de aceleração `0x4d0400`, probabilidade e consequências de quebra/acidente (`0x4cf030`, `0x4cf390`), custos mensais de combustível e manutenção (`0x4d4390`)
- RE: o `crt_rand_range(0, 3)` dos trens (`0x4d0400`, `0x4d2360`, `0x4d29e0`) só escolhe som; quebras usam o RNG do jogo; `/nowreck` multiplica a chance por 1e-4 em vez de zerar
- RE: oráculo `tests/trains.rs`: rampa, curva de velocidade, passo de aceleração e chance de quebra idênticos ao original em 70 000 casos aleatórios
- Docs: preços do apêndice do guia que divergem do executável (GP9, SD45, E60CP, Eurostar, TGVs) anotados em `docs/spec/trains.md`
- RE: precisão x87 medida no original sob Wine (winedbg --gdb + gdb): thread principal em FPCW 0x27F (53 bits) no porteiro de quadros e em sim_advance; sandbox e sondas em re/scripts/wine/ (docs/dynamic-analysis.md)
- Feature: novo crate `rt2-sim`, nucleo de simulacao deterministico: RNG do jogo (LCG e hash), calendario e velocidade, multiplicador de efeitos, receita de entrega, taxa de producao e reconstrucao de entradas das industrias, curva de velocidade, aceleracao, chance de quebra e custos mensais dos trens, camada de comandos (`Command`, `World::apply`) e carregador em tempo de execucao das tabelas do EXE
- Improved: os testes diferenciais do oraculo (`sim_time_rng`, `cargo`, `trains`) agora exercitam as funcoes publicas do `rt2-sim` em vez de copias locais, e conferem o carregador de tabelas contra a memoria emulada
- Docs: `docs/spec/time.md` registra a divergencia intencional do clone (quadro nominal fixo de 100 ms)
- RE: bitmap fonts: `font%s.dat` is only a name (hash) selecting one of 10 built-in definitions (`0x4f24c0`) over LAD banks `copp6/8/10/12/14/24.imb`, `scri11`, `fran13`, `haet46`, `htra46`; glyph = record `ch-0x20`, group 0 alpha mask, group 1 shadow; metrics in `docs/formats/font.md`
- RE: glyph RLE (`0x4fff00`) has tokens beyond plain LAD: 0x80-0x9f darken the destination, 0xa0-0xbf are alpha runs over the text colour
- RE: `.win` UI layouts (46 files) decoded from the loader `0x507600`: 65-byte window struct, 10 control types with action id, rectangle, font and sprite bank; see `docs/formats/ui.md`
- Feature: bitmap font reader (`rt2_formats::font`) and the `rt2-font` tool that renders text to PNG
- Feature: UI layout parser (`rt2_formats::ui`) and the `rt2-ui` tool that draws a window's geometry with its sprites
- Docs: `docs/formats/font.md` and `docs/formats/ui.md`
- RE: advance_save.py avança um save N dias no original (Wine+gdb, sem GUI) via carga do menu 0x479710, sim_advance 0x4a5cb0 e gravador 0x477540; determinístico byte a byte com a mesma semente (docs/dynamic-analysis.md seção 5)
- RE: especificacao de trilho, pontes, terraplenagem e estacoes em `docs/spec/terrain-track.md`: custo por tile (`0x463520`), formas de aresta validas (`0x5365e8`), pontes por tipo (`0x53ce60`), custo de estacao e captacao (`0x407030`, raios 2/4/8)
- RE: oraculo `tests/track.rs`: custo de trilho por tile (30 000 casos) e teste de captacao de estacao (20 000 casos) identicos ao original
- Docs: divergencias do manual no custo de trilho (base 2x2000 por tile, terreno por classe, ponte por tile, vao 120 000) anotadas em `docs/spec/terrain-track.md`
- Feature: `rt2_formats::map::Save::parse` e `Map::parse` leem a versao 0x2348 (os 95 `.MP2` e os saves gravados pelo proprio jogo) alem da 0x233C, via `Layout`; checksum do arquivo (`checksum`, `file_checksum`) confere nos 95 `.MP2`
- RE: diferencas 0x233C vs 0x2348 do carregador `0x477a60`: cabecalho 0x898a, estado 0x3a74, registros de pool maiores (estacao 0x4c7, pessoa 0x576, empresa 0x4e86, trem 0x14d, lugar 0xb4, territorio 0x314), carro de trem de 14 bytes, passo de rota de 8 bytes, parte fixa de regra 0x362; checksum em +0x86f7 (`0x4ef4d0`)
