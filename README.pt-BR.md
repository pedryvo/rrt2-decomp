<div align="center">

<img src="docs/assets/logo.svg" alt="RRT2 — locomotiva a vapor ao pôr do sol" width="220">

# rrt2-decomp

**Engenharia reversa e reimplementação moderna de _Railroad Tycoon II_ em Rust**

[![Rust](https://img.shields.io/badge/rust-2024_edition-b7410e?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-pr%C3%A9--alpha-orange)](#-progresso)
[![Fase](https://img.shields.io/badge/fase-1_%E2%80%94_formatos_de_arquivo-blue)](docs/PLAN.md)
[![Plataformas](https://img.shields.io/badge/plataformas-Linux_%7C_Windows-lightgrey)](#-compilando)

_O mesmo jogo de 1998 que você conhece, a mesma perspectiva isométrica,<br>agora nativo em 1080p, 4K e ultrawide._

[English](README.md) · **Português** · [Español](README.es.md) · [Deutsch](README.de.md) · [中文](README.zh-CN.md) · [Русский](README.ru.md)

</div>

---

## 🚂 Sobre

O **rrt2-decomp** estuda o executável original de _Railroad Tycoon II Platinum_ (PopTop Software, 1998–2001) para entender cada regra do jogo e reescrevê-lo do zero em Rust, mantendo a jogabilidade e a visão 2D isométrica ("falso 3D") do original e modernizando o resto:

- 🖥️ **Qualquer resolução**: 1080p, 1440p, 4K e ultrawide, com interface escalável e HiDPI. A interface, o texto e o terreno são gerados nativamente em qualquer resolução, e os sprites originais passam por um upscaling de alta qualidade (classe xBRZ ou modelos de IA offline), feito uma vez e guardado em cache.
- 🔍 **Zoom contínuo**, scroll suave e rotação da câmera.
- 🧮 **Simulação fiel e determinística**: economia, carga, cidades, indústrias, bolsa de valores e IA, validadas contra o jogo original.
- 🛠️ **Qualidade de vida**: trilho por arrastar com prévia de custo, desfazer, overlays de demanda e lucro, gerenciador de frota, autosave e atalhos remapeáveis.
- 🌎 **Mods e localização**: dados de jogo em arquivos legíveis e tradução PT-BR.

> [!IMPORTANT]
> Este projeto **não contém nenhum asset do jogo original**. Para jogar, você precisa de uma cópia legítima de _Railroad Tycoon II_ (por exemplo, a versão Platinum da [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum)). O clone lê os arquivos da sua instalação, no mesmo modelo de [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2) e [OpenTTD](https://github.com/OpenTTD/OpenTTD).

## 📊 Progresso

| Área | Status | Detalhes |
|---|:---:|---|
| Plano e arquitetura | ✅ | [`docs/PLAN.md`](docs/PLAN.md) |
| Contêiner **PK2** | ✅ | Layout, hash de nomes e precedência entre pacotes · [spec](docs/formats/pk2.md) |
| Textos **LNG** | ✅ | 2807 strings e tabelas nomeadas · [spec](docs/formats/lng.md) |
| Nomes dos arquivos nos PK2 | 🟡 | 328 nomes recuperados do EXE, de 2898 hashes |
| Sprites (LAD) e paletas | ✅ | Sprites decodificam com as cores certas, 1990/1990 bancos · [spec](docs/formats/lad.md) |
| Layouts de interface (`.win`) e fontes bitmap | ✅ | Os 46 layouts de janela são lidos; as 10 fontes do jogo decodificam com métricas · [ui](docs/formats/ui.md) · [fontes](docs/formats/font.md) |
| Áudio (`.raw`, músicas) | ✅ | Músicas com as regras originais de categoria e fade e os 123 efeitos sonoros (PCM mono 16 bits, 11025 Hz) a partir dos dados do jogo · [spec](docs/formats/sound.md) |
| Mapas e saves (`.MAP` `.MP2` `.GAM`) | 🟡 | As duas versões do formato (0x233C e 0x2348): os 128 arquivos da instalação e os saves gravados pelo original são lidos até o último byte; terreno, data, semente, lugares, empresas e regras decodificados; estações e trens parciais · [spec](docs/formats/map-gam.md) |
| Specs de simulação (trens, carga, economia…) | 🟡 | Tempo, calendário, laço principal, RNGs, economia de carga, movimento, quebras e custos dos trens, construção de trilhos e estações, área de captação e finanças (preço das ações, juros, dividendos), crescimento de cidades e indústrias, IA dos concorrentes, a ordem do tick e a linguagem de regras dos cenários, em boa parte verificados bit a bit contra o original · [tempo](docs/spec/time.md) · [rng](docs/spec/rng.md) · [carga](docs/spec/cargo.md) · [trens](docs/spec/trains.md) · [trilhos](docs/spec/terrain-track.md) · [finanças](docs/spec/finance.md) · [crescimento](docs/spec/world-growth.md) · [IA](docs/spec/ai.md) · [ordem do tick](docs/spec/world-tick.md) · [cenários](docs/spec/scenarios.md) |
| Harness de teste diferencial (Unicorn) | ✅ | Roda funções do original num emulador para comparação bit a bit · [docs](docs/oracle.md) |
| Motor gráfico (wgpu) | 🟡 | Mapas originais com as texturas exatas de terreno, rios, estradas, praias, trilhos, árvores, cidades, indústrias e estações, mais **trens em movimento e fumaça** com interpolação suave; sprites com upscaling; ~70 fps em 4K · `cargo run --release -p rt2-viewer -- <mapa>` |
| Simulação jogável | 🟡 | Laço do mundo do `rt2-sim` com trens, estações, economia, finanças, crescimento das cidades, regras de cenário e a IA dos concorrentes: saves originais avançados no clone ficam **idênticos ao original** campo a campo em **todos os 98** casos do placar (Tutorial, Example com trens, quatro mapas .MP2 e saves e mapas antigos convertidos na carga); inclusive New Games dos 18 cenários da campanha original; e os 18 cenários da Second Century · [placar de fidelidade](docs/fidelity-status.md) |
| Interface do jogo | 🟡 | App `rrt2`: menu principal com a arte original, listas de cenários e saves, HUD com data e controle de velocidade, texto vetorial nítido com fontes OFL equivalentes às originais, escala de UI de 100–300% · [docs](docs/ui.md) |
| Editor de cenários | ⏳ | |

<sub>✅ pronto · 🟡 parcial · 🔍 em análise · ⏳ planejado</sub>

**Fidelidade:** o clone e o original avançam o mesmo save e os resultados são comparados campo a campo. Placar atual: [`docs/fidelity-status.md`](docs/fidelity-status.md).

**Engenharia reversa:** 2945 funções no executável (1837 sem os thunks do linker) · 774 símbolos em [`re/symbols.toml`](re/symbols.toml) · 1298 classificadas (44%), 1647 ainda sem classificação · 458 funções especificadas (420 delas de simulação), 275 verificadas bit a bit contra o original ([`re/coverage.toml`](re/coverage.toml)). O percentual de cobertura da simulação será publicado quando a classificação estiver completa.

## 🧭 Roteiro

```
Fase 0  Fundação ......................... ✅
Fase 1  Formatos de arquivo e extração ... 🚧  ← estamos aqui
Fase 2  Decompilação por subsistema ...... ⏳
Fase 3  Motor gráfico .................... ⏳  (em paralelo à fase 2)
Fase 4  Fatia vertical jogável ........... ⏳
Fase 5  Jogo completo .................... ⏳
Fase 6  Modernização e polimento ......... ⏳
```

Detalhes de cada fase, riscos e critérios de pronto em [`docs/PLAN.md`](docs/PLAN.md).

## 🔧 Compilando

Requisitos: Rust estável (fixado por [`rust-toolchain.toml`](rust-toolchain.toml)) e, no Linux, `clang` + `mold` (configurados em [`.cargo/config.toml`](.cargo/config.toml)).

```bash
git clone https://github.com/pedryvo/rrt2-decomp.git
cd rrt2-decomp
cargo build --release
```

Testes (os que leem o jogo original rodam quando `RT2_DIR` aponta para a instalação):

```bash
RT2_DIR=~/Games/railroad-tycoon-2 cargo nextest run
```

## 📦 Instalação

1. Baixe o arquivo do seu sistema na [página de Releases](https://github.com/pedryvo/rrt2-decomp/releases) (Linux x86_64 `.tar.gz`, Windows x86_64 `.zip`, macOS Apple Silicon ou Intel `.tar.gz`). Cada push na `main` também gera builds de desenvolvimento, nos artefatos das execuções do workflow "Release" na aba Actions.
2. Descompacte onde quiser. O pacote contém apenas o `rrt2` (fontes e traduções já vão embutidas), a documentação e as licenças das fontes. **Ele não inclui o jogo original**: você precisa da sua própria cópia de Railroad Tycoon II (GOG, Steam ou instalação em CD).
3. Na primeira execução, informe onde o original está instalado. O caminho fica guardado no `config.toml`:

```bash
./rrt2 set-path ~/Games/railroad-tycoon-2     # Windows: rrt2.exe set-path "C:\Games\Railroad Tycoon II"
./rrt2
```

`rrt2 --version` mostra a versão e o commit do git. Em vez de `set-path`, você pode passar `--game-dir <pasta>` ou definir `RT2_DIR`.

| | Linux | Windows | macOS |
|---|---|---|---|
| Configurações e teclas (`config.toml`) | `~/.config/rrt2` | `%APPDATA%\rrt2` | `~/Library/Application Support/rrt2` |
| Saves | `~/.local/share/rrt2/saves` | `%APPDATA%\rrt2\saves` | `~/Library/Application Support/rrt2/saves` |

`RRT2_CONFIG` e `RRT2_SAVES` substituem os dois locais. As builds de macOS e Windows vêm do CI e ainda não foram testadas pelo autor.

## 🎮 Rodando o jogo (prévia)

```bash
cargo run --release -p rt2-game -- set-path ~/Games/railroad-tycoon-2   # uma vez
cargo run --release -p rt2-game
```

Já dá para começar qualquer cenário ou campanha do zero, fundar a empresa, construir trilhos, estações e prédios de apoio, comprar trens e definir rotas, comprar indústrias e acompanhar jornais, finanças e a bolsa enquanto a simulação roda. Ainda faltam títulos, negociação de ações e algumas telas (veja o inventário em `docs/reference/screens.md`). Configurações e atalhos ficam em `~/.config/rrt2/config.toml`; os saves vão para `~/.local/share/rrt2/saves`.

## 🧰 Ferramentas

Por enquanto o projeto é um conjunto de ferramentas de inspeção. O jogo vem depois.

```bash
# listar o conteúdo de um PK2, nomeando o que for conhecido
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt

# extrair tudo para uma pasta
cargo run --bin rt2-extract -- ~/Games/railroad-tycoon-2/data/RT2.PK2 -n re/names/exe_filenames.txt -o assets-extracted/rt2

# calcular o hash de um nome de arquivo
python3 re/scripts/pk2_hash.py setup.imb
```

## 🗂️ Estrutura

```
crates/
  rt2-formats/   parsers puros dos formatos do jogo (PK2, LNG, …)
  rt2-sim/       núcleo determinístico da simulação (sem I/O, sem render)
  rt2-render/    renderizador de terreno em wgpu (malha, câmera isométrica; sem lógica de jogo)
  rt2-viewer/    `rt2-viewer <mapa>`: janela ou PNG com `--screenshot`
  rt2-oracle/    harness de teste diferencial (EXE original no Unicorn)
  rt2-tools/     CLIs de inspeção e extração
docs/
  PLAN.md        plano de desenvolvimento
  formats/       especificação de cada formato de arquivo
  spec/          especificação de cada subsistema do jogo
  reference/     regras extraídas do manual e do guia de estratégia
re/
  symbols.toml   funções e globais identificadas no executável (fonte da verdade)
  names/         nomes de arquivo recuperados
  scripts/       scripts auxiliares de engenharia reversa
```

## 🔬 Metodologia

1. **Âncoras**: strings, imports e tabelas do executável indicam onde cada subsistema vive.
2. **Classificação total, decompilação dirigida**: toda função é classificada (biblioteca, plataforma, render, UI ou simulação), e cada subsistema é entendido a partir das âncoras. As rotinas de bibliotecas (CRT, Miles, Smacker, DirectX) são descartadas cedo, e toda função de simulação ganha uma spec.
3. **Especificação antes do código**: as regras viram documentos em [`docs/spec/`](docs/spec/) com pseudocódigo, constantes e o endereço de origem de cada uma. O código Rust é escrito a partir das specs, não do assembly.
4. **O executável é a referência, não o manual**: toda função da simulação é especificada, com a cobertura acompanhada publicamente. As fórmulas são provadas iguais rodando as funções originais num emulador x86 contra o nosso código Rust, e o comportamento do jogo inteiro é conferido avançando o mesmo save no original (no Wine) e no clone e comparando os resultados.

A análise usa [Ghidra](https://ghidra-sre.org/) por meio do servidor MCP **REA**, e o trabalho é dividido entre agentes de IA especializados ([`.claude/agents/`](.claude/agents/)), cada um com o modelo mais adequado à tarefa.

## 🙏 Inspiração

- [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2), [OpenTTD](https://github.com/OpenTTD/OpenTTD) e [OpenMW](https://github.com/OpenMW/openmw): reimplementações que exigem os dados originais
- [n64decomp/sm64](https://github.com/n64decomp/sm64) e [zeldaret/oot](https://github.com/zeldaret/oot): referências em documentação de engenharia reversa

## ⚖️ Aviso legal

_Railroad Tycoon_ é marca registrada de seus respectivos donos. Este é um projeto de fãs, sem fins lucrativos e sem afiliação com a PopTop Software, a Take-Two Interactive ou qualquer detentor de direitos. Nenhum código ou asset original é distribuído aqui: o repositório contém apenas código novo, documentação produzida por engenharia reversa para fins de interoperabilidade e nomes de arquivo necessários para ler os dados de uma cópia legítima.
