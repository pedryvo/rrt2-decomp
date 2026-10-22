<div align="center">

<img src="docs/assets/logo.svg" alt="RRT2 — 日落时分的蒸汽机车" width="220">

# rrt2-decomp

**《铁路大亨 II》（_Railroad Tycoon II_）的逆向工程与现代 Rust 重新实现**

[![Rust](https://img.shields.io/badge/rust-2024_edition-b7410e?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-pre--alpha-orange)](#-progress)
[![Phase](https://img.shields.io/badge/phase-1_%E2%80%94_file_formats-blue)](docs/PLAN.md)
[![Platforms](https://img.shields.io/badge/platforms-Linux_%7C_Windows-lightgrey)](#-building)

_您熟悉的 1998 年经典之作，同样的等距视角，<br>如今原生支持 1080p、4K 和超宽屏。_

[English](README.md) · [Português](README.pt-BR.md) · [Español](README.es.md) · [Deutsch](README.de.md) · **中文** · [Русский](README.ru.md)

</div>

---

## 🚂 关于

**rrt2-decomp** 研究原版《铁路大亨 II 白金版》（_Railroad Tycoon II Platinum_，PopTop Software，1998–2001）的可执行文件，以弄清每一条游戏规则，然后用 Rust 从零重写这款游戏。它保留了游戏玩法和 2D 等距（“伪 3D”）视角，并对其余部分进行现代化改造：

- 🖥️ **任意分辨率**：支持 1080p、1440p、4K 和超宽屏，界面可缩放并支持 HiDPI。界面、文字和地形可在任意分辨率下原生渲染，原版精灵图会经过高质量放大处理（xBRZ 级算法或离线 AI 模型），并在首次运行时缓存。
- 🔍 **连续缩放**、平滑滚动和镜头旋转。
- 🧮 **忠实且确定性的模拟**：经济、货物、城市、产业、股市和 AI，均与原版游戏对照验证。
- 🛠️ **便利性改进**：拖动铺设轨道并预览费用、撤销、需求和利润叠加图层、车队管理器、自动存档以及可重新绑定的按键。
- 🌎 **模组与本地化**：游戏数据存放在人类可读的文件中，并提供巴西葡萄牙语翻译。

> [!IMPORTANT]
> 本项目**不包含原版游戏的任何资源**。要游玩，您需要一份正版的《铁路大亨 II》（例如 [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum) 上的白金版）。本克隆版从您的安装目录读取数据，与 [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2) 和 [OpenTTD](https://github.com/OpenTTD/OpenTTD) 采用的模式相同。

## 📊 进展

| 领域 | 状态 | 详情 |
|---|:---:|---|
| 计划与架构 | ✅ | [`docs/PLAN.md`](docs/PLAN.md) |
| **PK2** 归档 | ✅ | 布局、文件名哈希和归档优先级 · [spec](docs/formats/pk2.md) |
| **LNG** 字符串表 | ✅ | 2807 条字符串和具名表 · [spec](docs/formats/lng.md) |
| PK2 内的文件名 | 🟡 | 已从 EXE 中还原 328 个名称，共 2898 个哈希 |
| 精灵图（LAD）与调色板 | ✅ | 精灵图可正确解码并还原色彩，1990/1990 个图库 · [spec](docs/formats/lad.md) |
| 界面布局（`.win`）与位图字体 | ✅ | 全部 46 个窗口布局均可解析；10 种游戏字体可连同字体度量一起解码 · [ui](docs/formats/ui.md) · [fonts](docs/formats/font.md) |
| 音频（`.raw` 音乐） | ✅ | 按原版的分类/淡入淡出规则播放音乐，并支持游戏数据中的全部 123 个音效（单声道 16 位 11025 Hz PCM） · [spec](docs/formats/sound.md) |
| 地图与存档（`.MAP` `.MP2` `.GAM`） | 🟡 | 两个格式版本（0x233C、0x2348）：随游戏附带的全部 128 个文件以及原版写出的存档均可逐字节完整解析；地形、日期、种子、地点、公司、规则已解码；车站和列车部分解码 · [spec](docs/formats/map-gam.md) |
| 模拟规格（列车、货物、经济……） | 🟡 | 时间/日历/主循环、随机数生成器、货物经济、列车运行/故障/成本、轨道/车站建设及吸引范围、金融（股价、利息、股息）、城镇与产业增长、竞争对手 AI、世界刻顺序和场景规则语言，大部分已与原版逐位精确验证 · [time](docs/spec/time.md) · [rng](docs/spec/rng.md) · [cargo](docs/spec/cargo.md) · [trains](docs/spec/trains.md) · [track](docs/spec/terrain-track.md) · [finance](docs/spec/finance.md) · [growth](docs/spec/world-growth.md) · [AI](docs/spec/ai.md) · [world tick](docs/spec/world-tick.md) · [scenarios](docs/spec/scenarios.md) |
| 差分测试框架（Unicorn） | ✅ | 在模拟器中运行原版函数，以进行逐位精确比较 · [docs](docs/oracle.md) |
| 渲染引擎（wgpu） | 🟡 | 使用游戏原版地形纹理（已解码的 id 表）渲染原版地图，包括河流、道路、海滩、轨道、树木、城镇、产业和车站，视图与原版一致；精灵图针对高 DPI 放大；原生 1080p/4K。列车待完成 · `cargo run --release -p rt2-viewer -- <map>` |
| 可玩的模拟 | 🟡 | `rt2-sim` 世界循环：原版存档在克隆版中推进后，逐字段**与原版完全一致**：教程推进 1/7/30 天、示例存档（含运行中的列车）以及四张 .MP2 地图推进 1/7 天，计分板 13 个用例全部通过；参见[保真度计分板](docs/fidelity-status.md) |
| 游戏界面 | 🟡 | `rrt2` 应用：使用原版美术的主菜单、场景/存档列表、带日期和速度控制的游戏内 HUD、采用与原版相匹配的 OFL 字体的清晰矢量文字、100–300% 的界面缩放 · [docs](docs/ui.md) |
| 场景编辑器 | ⏳ | |

<sub>✅ 已完成 · 🟡 部分完成 · 🔍 分析中 · ⏳ 计划中</sub>

**保真度：** 克隆版与原版推进同一份存档，并逐字段比对结果。当前计分板： [`docs/fidelity-status.md`](docs/fidelity-status.md).

**逆向工程：** 可执行文件中共有 2945 个函数（不含链接器跳转桩为 1837 个） · [`re/symbols.toml`](re/symbols.toml) 中有 634 个符号 · 已分类 1176 个（39%），仍有 1769 个未分类 · 已编写规格的函数 320 个（其中 293 个属于模拟），126 个已与原版逐位精确验证（[`re/coverage.toml`](re/coverage.toml)）。待分类全部完成后，将公布模拟覆盖率百分比。

## 🧭 路线图

```
阶段 0  基础工作 ........................ ✅
阶段 1  文件格式与提取 .................. 🚧  ← 当前所在
阶段 2  按子系统反编译 .................. ⏳
阶段 3  渲染引擎 ........................ ⏳  （与阶段 2 并行）
阶段 4  可玩的垂直切片 .................. ⏳
阶段 5  完整游戏 ........................ ⏳
阶段 6  现代化与打磨 .................... ⏳
```

各阶段的细节、风险和完成标准见 [`docs/PLAN.md`](docs/PLAN.md)（葡萄牙语）。

## 🔧 构建

您需要稳定版 Rust（由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定版本）。在 Linux 上，您还需要 `clang` 和 `mold`，[`.cargo/config.toml`](.cargo/config.toml) 将其用作链接器。

```bash
git clone https://github.com/pedryvo/rrt2-decomp.git
cd rrt2-decomp
cargo build --release
```

当 `RT2_DIR` 指向您的安装目录时，会运行读取原版游戏数据的测试：

```bash
RT2_DIR=~/Games/railroad-tycoon-2 cargo nextest run
```

## 🎮 运行游戏（早期预览）

```bash
cargo run --release -p rt2-game -- set-path ~/Games/railroad-tycoon-2   # 仅需一次
cargo run --release -p rt2-game
```

目前您可以浏览场景、载入存档、观看模拟运行并调整速度。接下来将实现铺设轨道和列车。设置和按键绑定位于 `~/.config/rrt2/config.toml`；存档保存在 `~/.local/share/rrt2/saves`。

## 🧰 工具

目前本项目是一组检查工具，游戏将在之后推出。

```bash
# 列出 PK2 归档的内容，并为已知条目命名
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt

# 将全部内容提取到一个文件夹
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt -o assets-extracted/rt2

# 计算文件名的哈希值
python3 re/scripts/pk2_hash.py setup.imb
```

## 🗂️ 目录结构

```
crates/
  rt2-formats/   游戏文件格式的纯解析器（PK2、LNG 等）
  rt2-sim/       确定性模拟核心（无 I/O，无渲染）
  rt2-render/    wgpu 地形渲染器（网格、等距相机；不含游戏逻辑）
  rt2-viewer/    `rt2-viewer <map>`：窗口或 `--screenshot` 输出 PNG
  rt2-oracle/    差分测试框架（在 Unicorn 中运行原版 EXE）
  rt2-tools/     检查与提取命令行工具
docs/
  PLAN.md        开发计划
  formats/       每种文件格式一份规格
  spec/          每个游戏子系统一份规格
  reference/     从手册和攻略指南中提取的规则
re/
  symbols.toml   在可执行文件中识别出的函数和全局变量（唯一权威来源）
  names/         还原出的文件名
  scripts/       逆向工程辅助脚本
```

## 🔬 方法论

1. **锚点**：可执行文件中的字符串、导入表和数据表指明了每个子系统所在的位置。
2. **全面分类，定向反编译**：每个函数都会被分类（库、平台、渲染、界面或模拟），并从锚点出发，一次攻克一个子系统。库代码（CRT、Miles、Smacker、DirectX）会尽早搁置，而每个模拟函数都有一份规格。
3. **先写规格，后写代码**：规则以伪代码、常量及各自的源地址写在 [`docs/spec/`](docs/spec/) 中。Rust 代码依据规格编写，而不是依据汇编。
4. **以可执行文件为准，而非手册**：每个模拟函数都有规格，覆盖率公开跟踪。公式通过在 x86 模拟器中运行原版函数并与我们的 Rust 代码对比来证明等价，整体游戏行为则通过在原版（Wine 下）和克隆版中推进同一份存档并比对结果来检验。

分析通过 **REA** MCP 服务器使用 [Ghidra](https://ghidra-sre.org/)。工作由多个专门的 AI 智能体（[`.claude/agents/`](.claude/agents/)）分担，每个智能体运行最适合其任务的模型。

`docs/` 下的项目文档使用葡萄牙语编写。

## 🙏 灵感来源

- [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2), [OpenTTD](https://github.com/OpenTTD/OpenTTD) and [OpenMW](https://github.com/OpenMW/openmw): 在原版数据上运行的重新实现项目
- [n64decomp/sm64](https://github.com/n64decomp/sm64) and [zeldaret/oot](https://github.com/zeldaret/oot): 记录逆向工程工作的典范

## ⚖️ 法律声明

_Railroad Tycoon_ 是其各自所有者的商标。这是一个非营利的粉丝项目，与 PopTop Software、Take-Two Interactive 或任何权利人均无关联。此处不分发任何原版代码或资源。本仓库仅包含新编写的代码、为实现互操作性而通过逆向工程产出的文档，以及从正版副本读取数据所需的文件名。
