<div align="center">

<img src="docs/assets/logo.svg" alt="RRT2 — steam locomotive at sunset" width="220">

# rrt2-decomp

**Reverse engineering and a modern Rust reimplementation of _Railroad Tycoon II_**

[![Rust](https://img.shields.io/badge/rust-2024_edition-b7410e?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-pre--alpha-orange)](#-progress)
[![Phase](https://img.shields.io/badge/phase-1_%E2%80%94_file_formats-blue)](docs/PLAN.md)
[![Platforms](https://img.shields.io/badge/platforms-Linux_%7C_Windows-lightgrey)](#-building)

_The 1998 classic you know, from the same isometric viewpoint,<br>now native at 1080p, 4K and ultrawide._

**English** · [Português](README.pt-BR.md)

</div>

---

## 🚂 About

**rrt2-decomp** studies the original _Railroad Tycoon II Platinum_ executable (PopTop Software, 1998–2001) to understand every game rule, then rewrites the game from scratch in Rust. It keeps the gameplay and the 2D isometric ("fake 3D") view, and modernizes everything else:

- 🖥️ **Any resolution**: 1080p, 1440p, 4K and ultrawide, with a scalable UI and HiDPI support. UI, text and terrain render natively at any resolution, and the original sprites go through a high-quality upscaling pass (xBRZ-class or offline AI models) cached on first run.
- 🔍 **Continuous zoom**, smooth scrolling and camera rotation.
- 🧮 **Faithful, deterministic simulation**: economy, cargo, cities, industries, stock market and AI, validated against the original game.
- 🛠️ **Quality of life**: drag-to-build track with cost preview, undo, demand and profit overlays, a fleet manager, autosave and rebindable keys.
- 🌎 **Mods and localization**: game data in human-readable files, plus a Brazilian Portuguese translation.

> [!IMPORTANT]
> This project contains **no assets from the original game**. To play, you need a legitimate copy of _Railroad Tycoon II_ (for example the Platinum edition from [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum)). The clone reads the data from your install, the same model used by [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2) and [OpenTTD](https://github.com/OpenTTD/OpenTTD).

## 📊 Progress

| Area | Status | Details |
|---|:---:|---|
| Plan and architecture | ✅ | [`docs/PLAN.md`](docs/PLAN.md) |
| **PK2** archives | ✅ | Layout, filename hash and archive precedence · [spec](docs/formats/pk2.md) |
| **LNG** string tables | ✅ | 2807 strings and named tables · [spec](docs/formats/lng.md) |
| Filenames inside PK2 | 🟡 | 328 names recovered from the EXE, out of 2898 hashes |
| Sprites (LAD) and palettes | ✅ | Sprites decode with correct colours, 1990/1990 banks · [spec](docs/formats/lad.md) |
| UI layouts (`.imb` `.win`) | 🔍 | Identified, not decoded |
| Audio (`.raw` `.wav`) | 🔍 | Most look like 16-bit mono PCM |
| Maps and saves (`.MAP` `.GAM`) | 🟡 | All 33 files parse to the last byte: terrain, date, seed, places, companies, rules; stations and trains partial · [spec](docs/formats/map-gam.md) |
| Simulation specs (trains, cargo, economy…) | 🟡 | Time/calendar/main loop, RNGs, cargo economy and train movement/breakdowns/costs, mostly verified bit-exact against the original · [time](docs/spec/time.md) · [rng](docs/spec/rng.md) · [cargo](docs/spec/cargo.md) · [trains](docs/spec/trains.md) |
| Differential test harness (Unicorn) | ✅ | Runs original functions in an emulator for bit-exact comparison · [docs](docs/oracle.md) |
| Rendering engine (wgpu) | 🟡 | Isometric terrain viewer: `cargo run --release -p rt2-viewer -- <map>` |
| Playable simulation | ⏳ | |
| Game UI | ⏳ | |
| Scenario editor | ⏳ | |

<sub>✅ done · 🟡 partial · 🔍 under analysis · ⏳ planned</sub>

**Reverse engineering:** 2945 functions in the executable (1837 excluding linker thunks) · 206 symbols in [`re/symbols.toml`](re/symbols.toml) · 833 classified (28%), 2112 still unclassified · 90 functions specified (81 of them simulation), 22 verified bit-exact against the original ([`re/coverage.toml`](re/coverage.toml)). A simulation coverage percentage will be published once classification is complete.

## 🧭 Roadmap

```
Phase 0  Foundation ........................ ✅
Phase 1  File formats and extraction ....... 🚧  ← we are here
Phase 2  Per-subsystem decompilation ....... ⏳
Phase 3  Rendering engine .................. ⏳  (alongside phase 2)
Phase 4  Playable vertical slice ........... ⏳
Phase 5  Complete game ..................... ⏳
Phase 6  Modernization and polish .......... ⏳
```

Phase details, risks and definitions of done are in [`docs/PLAN.md`](docs/PLAN.md) (Portuguese).

## 🔧 Building

You need stable Rust (pinned by [`rust-toolchain.toml`](rust-toolchain.toml)). On Linux you also need `clang` and `mold`, which [`.cargo/config.toml`](.cargo/config.toml) uses as the linker.

```bash
git clone https://github.com/pedryvo/rrt2-decomp.git
cd rrt2-decomp
cargo build --release
```

Tests that read the original game data run when `RT2_DIR` points at your install:

```bash
RT2_DIR=~/Games/railroad-tycoon-2 cargo nextest run
```

## 🧰 Tools

For now the project is a set of inspection tools. The game comes later.

```bash
# list a PK2 archive, naming the entries we know
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt

# extract everything into a folder
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt -o assets-extracted/rt2

# compute the hash of a filename
python3 re/scripts/pk2_hash.py setup.imb
```

## 🗂️ Layout

```
crates/
  rt2-formats/   pure parsers for the game's file formats (PK2, LNG, …)
  rt2-render/    wgpu terrain renderer (mesh, isometric camera; no game logic)
  rt2-viewer/    `rt2-viewer <map>`: window or `--screenshot` PNG
  rt2-oracle/    differential-test harness (original EXE in Unicorn)
  rt2-tools/     inspection and extraction CLIs
docs/
  PLAN.md        development plan
  formats/       one spec per file format
  spec/          one spec per game subsystem
  reference/     rules extracted from the manual and strategy guide
re/
  symbols.toml   functions and globals identified in the executable (source of truth)
  names/         recovered filenames
  scripts/       reverse-engineering helper scripts
```

## 🔬 Methodology

1. **Anchors**: strings, imports and data tables in the executable show where each subsystem lives.
2. **Full classification, targeted decompilation**: every function is classified (library, platform, render, UI or simulation), and we work outward from the anchors one subsystem at a time. Library code (CRT, Miles, Smacker, DirectX) is set aside early, and every simulation function gets a spec.
3. **Spec before code**: rules are written up in [`docs/spec/`](docs/spec/) with pseudocode, constants and the source address of each one. The Rust code is written from the specs, not from the assembly.
4. **The executable is the reference, not the manual**: every simulation function is specified, with coverage tracked publicly. Formulas are proven equal by running the original functions in an x86 emulator against our Rust code, and whole-game behavior is checked by advancing the same save in the original (under Wine) and in the clone and diffing the results.

The analysis uses [Ghidra](https://ghidra-sre.org/) through the **REA** MCP server. The work is split between specialized AI agents ([`.claude/agents/`](.claude/agents/)), each running the model best suited to its task.

The project docs under `docs/` are written in Portuguese.

## 🙏 Inspiration

- [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2), [OpenTTD](https://github.com/OpenTTD/OpenTTD) and [OpenMW](https://github.com/OpenMW/openmw): reimplementations that run on the original data
- [n64decomp/sm64](https://github.com/n64decomp/sm64) and [zeldaret/oot](https://github.com/zeldaret/oot): models for documenting reverse-engineering work

## ⚖️ Legal

_Railroad Tycoon_ is a trademark of its respective owners. This is a non-profit fan project with no affiliation to PopTop Software, Take-Two Interactive or any rights holder. No original code or assets are distributed here. The repository holds only new code, documentation produced by reverse engineering for interoperability, and the filenames needed to read data from a legitimate copy.
