<div align="center">

<img src="docs/assets/logo.svg" alt="RRT2 — locomotora de vapor al atardecer" width="220">

# rrt2-decomp

**Ingeniería inversa y reimplementación moderna en Rust de _Railroad Tycoon II_**

[![Rust](https://img.shields.io/badge/rust-2024_edition-b7410e?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-pre--alpha-orange)](#-progreso)
[![Phase](https://img.shields.io/badge/phase-1_%E2%80%94_file_formats-blue)](docs/PLAN.md)
[![Platforms](https://img.shields.io/badge/platforms-Linux_%7C_Windows-lightgrey)](#-compilación)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

_El clásico de 1998 que conoces, desde el mismo punto de vista isométrico,<br>ahora nativo en 1080p, 4K y ultraancho._

[English](README.md) · [Português](README.pt-BR.md) · **Español** · [Deutsch](README.de.md) · [中文](README.zh-CN.md) · [Русский](README.ru.md)

</div>

---

## 🚂 Acerca del proyecto

**rrt2-decomp** estudia el ejecutable original de _Railroad Tycoon II Platinum_ (PopTop Software, 1998–2001) para entender cada regla del juego, y después reescribe el juego desde cero en Rust. Conserva la jugabilidad y la vista isométrica 2D («falso 3D»), y moderniza todo lo demás:

- 🖥️ **Cualquier resolución**: 1080p, 1440p, 4K y ultraancho, con interfaz escalable y compatibilidad con HiDPI. La interfaz, el texto y el terreno se renderizan de forma nativa a cualquier resolución, y los sprites originales pasan por un proceso de escalado de alta calidad (clase xBRZ o modelos de IA sin conexión) que se guarda en caché la primera vez que se ejecuta.
- 🔍 **Zoom continuo**, desplazamiento suave y rotación de la cámara.
- 🧮 **Simulación fiel y determinista**: economía, carga, ciudades, industrias, bolsa e IA, validadas contra el juego original.
- 🛠️ **Calidad de vida**: tendido de vía arrastrando con vista previa del costo, deshacer, superposiciones de demanda y beneficios, un gestor de flota, autoguardado y teclas reasignables.
- 🌎 **Mods y localización**: datos del juego en archivos legibles por humanos, además de una traducción al portugués de Brasil.

> [!IMPORTANT]
> Este proyecto **no contiene recursos del juego original**. Para jugar necesitas una copia legítima de _Railroad Tycoon II_ (por ejemplo, la edición Platinum de [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum)). El clon lee los datos de tu instalación, el mismo modelo que usan [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2) y [OpenTTD](https://github.com/OpenTTD/OpenTTD).

## 📊 Progreso

| Área | Estado | Detalles |
|---|:---:|---|
| Plan y arquitectura | ✅ | [`docs/PLAN.md`](docs/PLAN.md) |
| Archivos **PK2** | ✅ | Estructura, hash de nombres de archivo y precedencia de archivos · [especificación](docs/formats/pk2.md) |
| Tablas de cadenas **LNG** | ✅ | 2807 cadenas y tablas con nombre · [especificación](docs/formats/lng.md) |
| Nombres de archivo dentro de PK2 | 🟡 | 328 nombres recuperados del EXE, de 2898 hashes |
| Sprites (LAD) y paletas | ✅ | Los sprites se decodifican con los colores correctos, 1990/1990 bancos · [especificación](docs/formats/lad.md) |
| Diseños de interfaz (`.win`) y fuentes bitmap | ✅ | Los 46 diseños de ventana se leen correctamente; las 10 fuentes del juego se decodifican con sus métricas · [interfaz](docs/formats/ui.md) · [fuentes](docs/formats/font.md) |
| Audio (música `.raw`) | ✅ | Música con las reglas originales de categorías y fundidos, y los 123 efectos de sonido (PCM mono de 16 bits a 11025 Hz) a partir de los datos del juego · [especificación](docs/formats/sound.md) |
| Mapas y partidas guardadas (`.MAP` `.MP2` `.GAM`) | 🟡 | Ambas versiones del formato (0x233C, 0x2348): los 128 archivos incluidos y las partidas guardadas por el original se leen hasta el último byte; terreno, fecha, semilla, lugares, compañías y reglas decodificados; estaciones y trenes parcialmente · [especificación](docs/formats/map-gam.md) |
| Especificaciones de simulación (trenes, carga, economía…) | 🟡 | Tiempo/calendario/bucle principal, generadores de números aleatorios, economía de la carga, movimiento/averías/costos de los trenes, construcción de vías y estaciones y su área de captación, finanzas (precio de las acciones, intereses, dividendos), crecimiento de pueblos e industrias, IA de la competencia, el orden del ciclo del mundo y el lenguaje de reglas de los escenarios, en su mayoría verificados bit a bit contra el original · [tiempo](docs/spec/time.md) · [rng](docs/spec/rng.md) · [carga](docs/spec/cargo.md) · [trenes](docs/spec/trains.md) · [vías](docs/spec/terrain-track.md) · [finanzas](docs/spec/finance.md) · [crecimiento](docs/spec/world-growth.md) · [IA](docs/spec/ai.md) · [ciclo del mundo](docs/spec/world-tick.md) · [escenarios](docs/spec/scenarios.md) |
| Banco de pruebas diferenciales (Unicorn) | ✅ | Ejecuta funciones originales en un emulador para compararlas bit a bit · [documentación](docs/oracle.md) |
| Motor de renderizado (wgpu) | 🟡 | Mapas originales con las texturas exactas de terreno, ríos, caminos, playas, vías, árboles, ciudades, industrias y estaciones, además de **trenes en movimiento y humo** con interpolación suave; sprites escalados para alta resolución; ~70 fps en 4K · `cargo run --release -p rt2-viewer -- <mapa>` |
| Simulación jugable | 🟡 | Bucle del mundo de `rt2-sim` con trenes, estaciones, economía, finanzas, crecimiento de ciudades, reglas de escenario y la IA rival: las partidas originales avanzadas en el clon son **idénticas al original** campo por campo en **los 131** casos del marcador, desde 1 día hasta 2 años de juego, incluidos eventos raros (accidentes, robos, liquidaciones, la IA operando en bolsa) (Tutorial, Example con trenes, cuatro mapas .MP2 y partidas y mapas antiguos convertidos al cargar); incluidas partidas nuevas de los 18 escenarios de la campaña original; y los 18 escenarios de Second Century · [marcador de fidelidad](docs/fidelity-status.md) |
| Interfaz del juego | 🟡 | Aplicación `rrt2`: menú principal con el arte original, listas de escenarios y partidas guardadas, HUD en juego con fecha y control de velocidad, texto vectorial nítido con fuentes OFL equivalentes a las originales, escala de interfaz del 100 al 300% · [documentación](docs/ui.md) |
| Editor de mapas y escenarios | 🟡 | Terreno, árboles, territorios, edificios, lugares, ríos y caminos; opciones del mapa, metas y editor de reglas con validación en vivo; guardar conserva todos los mapas incluidos. Faltan la importación PCX y algunas páginas de opciones |

<sub>✅ hecho · 🟡 parcial · 🔍 en análisis · ⏳ planeado</sub>

**Fidelidad:** el clon y el original hacen avanzar la misma partida guardada y los resultados se comparan campo por campo. Marcador actual: [`docs/fidelity-status.md`](docs/fidelity-status.md).

**Ingeniería inversa:** 2945 funciones en el ejecutable (1837 sin contar los thunks del enlazador) · 833 símbolos en [`re/symbols.toml`](re/symbols.toml) · 1310 clasificadas (44%), 1635 aún sin clasificar · 472 funciones especificadas (429 de ellas de simulación), 301 verificadas bit a bit contra el original ([`re/coverage.toml`](re/coverage.toml)). Se publicará un porcentaje de cobertura de la simulación cuando termine la clasificación.

## 🧭 Hoja de ruta

```
Phase 0  Foundation ........................ ✅
Phase 1  File formats and extraction ....... 🚧  ← we are here
Phase 2  Per-subsystem decompilation ....... ⏳
Phase 3  Rendering engine .................. ⏳  (alongside phase 2)
Phase 4  Playable vertical slice ........... ⏳
Phase 5  Complete game ..................... ⏳
Phase 6  Modernization and polish .......... ⏳
```

Los detalles de cada fase, los riesgos y las definiciones de «terminado» están en [`docs/PLAN.md`](docs/PLAN.md) (en portugués).

## 🔧 Compilación

Necesitas Rust estable (fijado por [`rust-toolchain.toml`](rust-toolchain.toml)). En Linux también necesitas `clang` y `mold`, que [`.cargo/config.toml`](.cargo/config.toml) usa como enlazador.

```bash
git clone https://github.com/pedryvo/rrt2-decomp.git
cd rrt2-decomp
cargo build --release
```

Las pruebas que leen los datos del juego original se ejecutan cuando `RT2_DIR` apunta a tu instalación:

```bash
RT2_DIR=~/Games/railroad-tycoon-2 cargo nextest run
```

## 📦 Instalación

1. Descarga el archivo para tu sistema desde la [página de Releases](https://github.com/pedryvo/rrt2-decomp/releases) (Linux x86_64 `.tar.gz`, Windows x86_64 `.zip`, macOS Apple Silicon o Intel `.tar.gz`). Cada push a `main` también genera compilaciones de desarrollo, en los artefactos de las ejecuciones del workflow "Release" en la pestaña Actions.
2. Descomprímelo donde quieras. El paquete solo contiene `rrt2` (las fuentes y las traducciones van incluidas), la documentación y las licencias de las fuentes. **No incluye el juego original**: necesitas tu propia copia de Railroad Tycoon II (GOG, Steam o instalación en CD).
3. En la primera ejecución, indica dónde está instalado el original. La ruta se guarda en `config.toml`:

```bash
./rrt2 set-path ~/Games/railroad-tycoon-2     # Windows: rrt2.exe set-path "C:\Games\Railroad Tycoon II"
./rrt2
```

`rrt2 --version` muestra la versión y el commit de git. En lugar de `set-path` puedes pasar `--game-dir <carpeta>` o definir `RT2_DIR`.

| | Linux | Windows | macOS |
|---|---|---|---|
| Ajustes y teclas (`config.toml`) | `~/.config/rrt2` | `%APPDATA%\rrt2` | `~/Library/Application Support/rrt2` |
| Partidas guardadas | `~/.local/share/rrt2/saves` | `%APPDATA%\rrt2\saves` | `~/Library/Application Support/rrt2/saves` |

`RRT2_CONFIG` y `RRT2_SAVES` sustituyen esas dos ubicaciones. Las compilaciones de macOS y Windows salen del CI y el autor aún no las ha probado.

## 🎮 Ejecutar el juego (vista previa temprana)

```bash
cargo run --release -p rt2-game -- set-path ~/Games/railroad-tycoon-2   # una sola vez
cargo run --release -p rt2-game
```

Ya puedes empezar cualquier escenario o campaña desde cero, fundar tu compañía, tender vías, construir estaciones y edificios de apoyo, comprar trenes y fijar sus rutas, comprar industrias y seguir los periódicos, las finanzas y la bolsa mientras corre la simulación. Aún faltan los bonos, la compraventa de acciones y algunas pantallas (ver el inventario en `docs/reference/screens.md`). Los ajustes y las asignaciones de teclas están en `~/.config/rrt2/config.toml`; las partidas guardadas van a `~/.local/share/rrt2/saves`.

## 🧰 Herramientas

Por ahora el proyecto es un conjunto de herramientas de inspección. El juego llegará más adelante.

```bash
# list a PK2 archive, naming the entries we know
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt

# extract everything into a folder
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt -o assets-extracted/rt2

# compute the hash of a filename
python3 re/scripts/pk2_hash.py setup.imb
```

## 🗂️ Estructura

```
crates/
  rt2-formats/   pure parsers for the game's file formats (PK2, LNG, …)
  rt2-sim/       deterministic simulation core (no I/O, no rendering)
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

## 🔬 Metodología

1. **Anclas**: las cadenas, las importaciones y las tablas de datos del ejecutable muestran dónde vive cada subsistema.
2. **Clasificación completa, decompilación dirigida**: cada función se clasifica (biblioteca, plataforma, renderizado, interfaz o simulación), y avanzamos desde las anclas hacia fuera, un subsistema a la vez. El código de bibliotecas (CRT, Miles, Smacker, DirectX) se aparta pronto, y cada función de simulación recibe su especificación.
3. **Especificación antes que código**: las reglas se documentan en [`docs/spec/`](docs/spec/) con pseudocódigo, constantes y la dirección de origen de cada una. El código en Rust se escribe a partir de las especificaciones, no del ensamblador.
4. **El ejecutable es la referencia, no el manual**: cada función de simulación se especifica, y la cobertura se sigue públicamente. Las fórmulas se demuestran iguales ejecutando las funciones originales en un emulador x86 frente a nuestro código en Rust, y el comportamiento del juego completo se comprueba haciendo avanzar la misma partida guardada en el original (bajo Wine) y en el clon y comparando los resultados.

El análisis usa [Ghidra](https://ghidra-sre.org/) a través del servidor MCP **REA**. El trabajo se reparte entre agentes de IA especializados ([`.claude/agents/`](.claude/agents/)), cada uno con el modelo más adecuado para su tarea.

La documentación del proyecto en `docs/` está escrita en portugués.

## 🙏 Inspiración

- [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2), [OpenTTD](https://github.com/OpenTTD/OpenTTD) y [OpenMW](https://github.com/OpenMW/openmw): reimplementaciones que funcionan con los datos originales
- [n64decomp/sm64](https://github.com/n64decomp/sm64) y [zeldaret/oot](https://github.com/zeldaret/oot): modelos para documentar el trabajo de ingeniería inversa

## ⚖️ Aviso legal

_Railroad Tycoon_ es una marca registrada de sus respectivos propietarios. Este es un proyecto de aficionados sin fines de lucro, sin ninguna afiliación con PopTop Software, Take-Two Interactive ni ningún titular de derechos. Aquí no se distribuye código ni recursos originales. El repositorio contiene únicamente código nuevo, documentación producida mediante ingeniería inversa con fines de interoperabilidad y los nombres de archivo necesarios para leer datos de una copia legítima.

El código fuente de rrt2-decomp se distribuye bajo la [GNU General Public License v3.0 o posterior](LICENSE). Las fuentes incluidas usan la SIL Open Font License (ver `assets/fonts/`).
