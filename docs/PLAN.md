# Plano de desenvolvimento — clone moderno de Railroad Tycoon II em Rust

## 1. Objetivo

Reimplementar Railroad Tycoon II Platinum em Rust, fiel à jogabilidade e à perspectiva 2D isométrica ("falso 3D") do original, modernizado para hardware atual:

- Renderização nativa em qualquer resolução: 1080p, 1440p, 4K e ultrawide, com UI escalável (100–300%).
- Mesma simulação: economia, carga, cidades, indústrias, bolsa de valores, IA e cenários.
- Recursos de qualidade de vida modernos (seção 7).
- Assets carregados de uma instalação original do jogo em tempo de execução, sem redistribuir nada, no mesmo modelo de OpenRCT2 e OpenTTD.

**Restrição inviolável:** `/home/pedryvo/Games/railroad-tycoon-2` é somente leitura. A análise usa uma cópia em `re/bin/` (ignorada pelo git) e os assets extraídos ficam em `assets-extracted/` (também ignorada).

## 2. O que já sabemos (reconhecimento inicial)

| Item | Observação |
|---|---|
| Executável | `RT2_PLAT.EXE.orig`: PE32 i386, GUI, MSVC, base `0x400000`, 2.945 funções (1.837 sem thunks), sem símbolos. sha256 `510b9e44…7e474`. Os outros EXEs são versões com patch de resolução e serão ignorados. |
| Seções | `.text` 1,2 MB · `.data` ~300 KB (tabelas de balanceamento devem estar aqui) · `.rsrc` pequeno |
| Contêineres | `data/RT2.PK2` (85 MB), `RT2_06.PK2`, `RT2_09GO.PK2`, `RT2_11PL.PK2`. O jogo carrega todos via `.\data\*.pk2` (string em `0x55d500`). O formato é proprietário: o diretório começa com um campo de tamanho e os blobs têm entropia de ~7,1–7,3, então devem ser comprimidos. Este é o **primeiro alvo de RE**. |
| Mapas e saves | `maps/*.MAP`, `games/*.GAM`: o cabeçalho tem a mágica `0x233C` seguida do nome do cenário. |
| Texto | `default2.lng` e `credits2.lng`: texto puro com linhas numeradas, fácil de ler. |
| Vídeo | `anim/*.smk` (Smacker), que o ffmpeg decodifica. |
| Áudio | `data2/*.mp3` (músicas) e `.MP2` (efeitos, provavelmente dentro de PK2 ou soltos). |
| Gráficos | Sprites isométricos **pré-renderizados** em formato próprio ("LAD": 8 bits indexados, RLE por linha, várias camadas), mais bancos de paletas, em `docs/formats/pk2-contents.md`. Os `.m3d` em `data2/Miles` são só plugins de áudio 3D do Miles, não modelos. Para alta resolução, a estratégia é upscaling de alta qualidade (xBRZ/ScaleFX, ou modelos de IA rodando offline) gerado uma vez num cache, com pacotes de sprites HD opcionais no futuro. A UI, o texto e o terreno são gerados nativamente em qualquer resolução. |
| Docs | `Manual.html`, `Strategy Guide.html`, `editor3.txt`. São uma fonte valiosa de fórmulas e regras, a cruzar com o código. |

## 3. Arquitetura proposta

Workspace Cargo com crates pequenas e fronteiras claras:

```
railroad/
├─ crates/
│  ├─ rt2-formats/     # parsers puros: PK2, LNG, MAP/GAM, sprites LAD, paletas, áudio. Sem I/O de jogo, 100% testável.
│  ├─ rt2-sim/         # simulação determinística (sem gráficos, sem relógio de parede, RNG semeado, aritmética de ponto fixo onde o original usa)
│  ├─ rt2-render/      # wgpu: terreno, trilhos, sprites (com upscaling), câmera ortográfica isométrica
│  ├─ rt2-ui/          # UI escalável (layout em pontos lógicos × fator de escala), skin com a arte original
│  ├─ rt2-audio/       # música e efeitos (kira ou rodio)
│  ├─ rt2-game/        # binário: loop, estados, input, glue entre sim/render/ui
│  └─ rt2-tools/       # CLIs: rt2-extract, rt2-inspect, conversores, dumps de fórmulas
├─ re/                 # engenharia reversa
│  ├─ symbols.toml     # nomes/tipos/comentários das funções (fonte da verdade, versionado)
│  ├─ notes/           # notas por subsistema
│  └─ bin/             # cópia do EXE (gitignored)
└─ docs/
   ├─ PLAN.md
   ├─ formats/         # especificação de cada formato de arquivo
   └─ spec/            # especificação de cada subsistema (pseudocódigo + constantes + endereço de origem)
```

**Decisões técnicas recomendadas:**

- **Renderização:** `wgpu` + `winit`, sem Bevy. É um jogo 2D/ortográfico com simulação determinística, e um motor grande traria mudanças de API frequentes e acoplamento desnecessário. Com wgpu temos controle total para gerar o terreno e os trilhos proceduralmente e renderizar modelos com a câmera ortográfica isométrica.
- **Simulação separada do render** desde o primeiro dia, com tick fixo. Isso permite testes headless, replays, acelerar e pausar o tempo e, mais tarde, multiplayer lockstep.
- **UI:** um sistema próprio, retido e simples, com layouts em unidades lógicas e escala arbitrária. A arte original é reaproveitada com filtragem de alta qualidade (xBR/FSR-like em shader) e as fontes são vetoriais (`cosmic-text` ou `fontdue`). `egui` fica só para as ferramentas de debug.
- **Assets:** na primeira execução o jogo aponta para a instalação original, extrai os assets e gera um cache convertido em `~/.local/share/railroad/`.
- **Dados de jogo** (locomotivas, cargas, indústrias, cenários) ficam em RON/TOML extraídos do original, o que habilita mods.

## 4. Fases e marcos

Cada fase termina com um entregável verificável e commits convencionais.

### Fase 0 — Fundação (curta)
- Workspace Cargo, `rust-toolchain.toml` e CI local (`cargo fmt`, `clippy -D warnings`, `cargo nextest`).
- Definição dos agentes em `.claude/agents/*.md` com o modelo de cada um (seção 6).
- Persistência da RE: as sessões do Ghidra pelo REA são efêmeras e as anotações se perdem ao fechar, então `re/symbols.toml` é a fonte da verdade. Um script aplica tudo de volta via `set_addresses_names`/`set_comment` no início de cada sessão.
- **Pronto quando:** `cargo build` e os checks passam, e um ciclo abrir → aplicar símbolos → fechar funciona.

### Fase 1 — Formatos de arquivo e extração
1. **PK2:** achar a rotina que abre `.\data\*.pk2`, documentar o diretório (provável hash de nome, offset e tamanho) e o algoritmo de compressão, e recuperar a função de hash para mapear nomes. Entrega: `rt2-extract` lista e extrai todos os arquivos.
2. **Imagens e paletas** dentro do PK2 (formatos a identificar).
3. **Sprites LAD e paletas:** decodificar as camadas, ligar cada sprite à sua paleta e montar o pipeline de upscaling para o cache.
4. **MAP/GAM:** heightmap, tipos de terreno, cidades, indústrias, trilhos e estado completo do save.
5. **LNG:** tabela de strings, incluindo uma tradução PT-BR como bônus.
6. Smacker e áudio via ffmpeg (conversão no cache).
- **Pronto quando:** 100% das entradas dos PK2 extraídas, um visualizador de debug mostrando sprites, paletas e mapas, testes de round-trip nos parsers, e `docs/formats/*.md` escritos.

### Fase 2 — Decompilação por subsistema
**Todas** as 2.945 funções (1.837 sem thunks) são classificadas (biblioteca, render/UI, simulação), e **toda função de simulação precisa ser especificada**, sem exceção (ver seção 11). A ordem de ataque segue strings e xrefs, com esta prioridade:

| Prioridade | Subsistema | Saída |
|---|---|---|
| P0 | Loop principal, tempo e calendário, RNG | `spec/time.md`, `spec/rng.md` |
| P0 | Terreno, mapa e custo de construção | `spec/terrain.md` |
| P0 | Trilhos, estações e sinalização | `spec/track.md` |
| P0 | Movimento de trens, física e quebras | `spec/trains.md` |
| P0 | Carga: produção, demanda, preços e entrega | `spec/cargo.md` |
| P1 | Cidades e indústrias: crescimento e construção | `spec/economy-world.md` |
| P1 | Finanças: empresa, ações, títulos e bolsa | `spec/finance.md` |
| P1 | IA dos concorrentes | `spec/ai.md` |
| P2 | Cenários, eventos, condições de vitória e editor | `spec/scenarios.md` |
| P2 | UI: telas, fluxos e atalhos (comportamento, não código) | `spec/ui.md` |

- Cada spec traz pseudocódigo, constantes e tabelas de `.data`, e o endereço de origem de cada regra.
- Validação: com `wine` + `gdb`/`winedbg` rodando o EXE original, capturamos valores reais (ex.: receita de uma entrega) e comparamos com a spec.
- **Pronto quando:** as specs P0 estão revisadas e cada fórmula tem um teste de referência com dados do original.

### Fase 3 — Motor gráfico (em paralelo à Fase 2)
- Janela, câmera isométrica com zoom contínuo e scroll suave, e renderização do terreno a partir do heightmap.
- Trilhos, cidades, indústrias e trens renderizados a partir dos sprites com upscaling, com filtragem nítida em qualquer zoom.
- HiDPI e escala: teste em 1080p, 1440p, 4K e ultrawide.
- **Pronto quando:** um `.MAP` original abre e é navegável a 60+ fps em 4K.

### Fase 4 — Simulação jogável (vertical slice)
- Construir trilho e estação, comprar trem, montar rota, entregar carga e receber dinheiro.
- O tempo passa, a economia reage e há um relatório financeiro básico.
- **Pronto quando:** é possível jogar 10 anos num mapa original com números próximos aos do original nos testes de referência.

### Fase 5 — Jogo completo
- Todas as telas e menus, bolsa de valores, IA, eventos, cenários e campanha.
- Save/load em formato próprio (versionado) e **importação de `.GAM` original**.
- Editor de mapas e cenários.
- Música, efeitos e vídeos de introdução.

### Fase 6 — Modernização e polimento
- Recursos da seção 7, performance, empacotamento (AppImage/Flatpak e build Windows) e documentação para jogadores.

## 5. Estratégia de engenharia reversa

1. **Âncoras:** strings (nomes de arquivos, textos de UI e de erro), imports (DirectDraw, DirectSound, Miles `Mss32.dll`, Smacker) e tabelas em `.data`.
2. **Propagação:** subir e descer pelo grafo de chamadas a partir das âncoras, nomeando funções e estruturas e registrando tudo em `re/symbols.toml`.
3. **Especificação, não tradução:** o objetivo é entender as regras e reescrevê-las em Rust idiomático, não transliterar o assembly.
4. **Verificação dinâmica:** usar breakpoints no original via wine+gdb para confirmar valores.
5. **Rastreabilidade:** cada regra no código Rust cita a spec, e cada spec cita o endereço de origem.

## 6. Agentes e modelos (economia de tokens)

Cada tipo de tarefa vai para o modelo mais barato que a resolve bem. Os agentes ficam definidos em `.claude/agents/` com o `model` fixado no frontmatter.

| Agente | Modelo | Tarefas |
|---|---|---|
| `architect` (sessão principal) | Opus 5.5 | Arquitetura, decisões, revisão das specs P0, integração, triagem do que delegar |
| `re-analyst` | Sonnet 5.5 | Decompilar e entender um subsistema via REA, escrever `docs/spec/*.md` e `re/symbols.toml` |
| `re-deep` | Opus 5.5 | Só para o que é algoritmicamente difícil: compressão do PK2, função de hash, física de trens, IA |
| `format-engineer` | Sonnet 5.5 | Implementar parsers em `rt2-formats` a partir das specs, com testes |
| `rust-implementer` | Sonnet 5.5 | Implementar módulos de `rt2-sim`, `rt2-render` e `rt2-ui` a partir das specs |
| `bulk-labeler` | Haiku 5.5 | Tarefas mecânicas: varrer strings e xrefs, nomear funções triviais (wrappers, getters, CRT), catalogar arquivos extraídos |
| `test-writer` | Haiku 5.5 | Testes de referência a partir de tabelas e valores já documentados |
| `reviewer` | Sonnet 5.5 | Revisão de código e de specs antes de cada commit relevante |

**Regras de economia:**
- Os agentes recebem uma tarefa estreita e os arquivos exatos que devem ler, e devolvem só a conclusão (spec ou diff), não dumps.
- As specs em `docs/spec/` são a memória compartilhada: um agente de implementação lê a spec e não refaz a RE.
- Funções da CRT/MSVC e de bibliotecas (Miles, Smacker, DirectX) são identificadas cedo e ignoradas. Isso deve cortar uma fração grande das 2.945 funções (1.837 sem thunks).
- Paralelismo só entre subsistemas independentes (ex.: RE de `cargo` enquanto se implementa `terrain`).

## 7. Modernizações propostas

**Essenciais**
- Qualquer resolução e proporção, UI escalável, HiDPI e modo janela/tela cheia sem bordas.
- Zoom contínuo (o original tem níveis fixos), scroll suave e rotação da câmera em 90°.
- Velocidade de jogo ajustável e pausa em qualquer tela.
- Autosave, vários slots e saves com metadados e miniatura.
- Atalhos remapeáveis.

**Jogabilidade (sem alterar o balanceamento padrão)**
- Construção de trilho por arrastar com pathfinding e prévia de custo, e desfazer da última construção.
- Overlays de mapa: demanda e oferta de cada carga, lucro por trecho, congestionamento.
- Gerenciador de rotas e de frota: templates de consist, substituição em massa de locomotivas.
- Gráficos e relatórios financeiros mais completos.
- Notificações configuráveis e um histórico de eventos.

**Plataforma**
- Linux e Windows nativos, suporte a controle e Steam Deck.
- Mods via dados em RON/TOML.
- Localização: inglês e PT-BR.
- Acessibilidade: paletas para daltonismo e tamanho de fonte.
- Extra (pós-1.0): multiplayer lockstep, viável por causa da simulação determinística.

## 8. Riscos

| Risco | Mitigação |
|---|---|
| Compressão ou ofuscação do PK2 difícil | Atacar primeiro, com o modelo mais forte. Plano B: capturar arquivos já descomprimidos da memória via wine+gdb |
| Fidelidade da simulação | Testes de referência contra o original rodando no wine |
| Volume de funções | Triagem por âncoras e descarte de CRT e bibliotecas |
| Sessões Ghidra efêmeras | `re/symbols.toml` versionado e reaplicado a cada sessão |
| Questão legal | Nenhum asset original no repositório; o clone exige uma cópia legítima do jogo |

## 9. Próximos passos imediatos

1. Fase 0: workspace, agentes e persistência de símbolos.
2. Fase 1.1: RE do PK2 (`re-deep`) em paralelo à triagem de funções CRT e bibliotecas (`bulk-labeler`).
3. `rt2-extract` funcional, para destravar todo o resto.

## 10. Práticas adotadas do OpenRCT2

Pesquisa feita em 2026-10-22, quando o OpenRCT2 estava na v0.5.6. O que vale replicar, em ordem de prioridade:

| # | Prática | No OpenRCT2 | Aqui | Quando |
|---|---|---|---|---|
| 1 | **Camada de comandos** | Toda mutação de estado é uma `GameAction` serializável | Toda mudança na simulação passa por um `enum Command`. É a base de replays, desfazer e multiplayer | Ao criar `rt2-sim` |
| 2 | **Replays como teste de regressão** | `.parkrep`: semente, comandos por tick e checksum do estado; a suíte roda em todo PR; `replay_normalise` regrava os checksums depois de mudanças intencionais | Formato próprio de replay, testes "golden" no `rt2-sim` e um subcomando `rt2 replay bless` | Formato junto com `rt2-sim`; volume de replays durante a Fase 4 |
| 3 | **Determinismo verificável** | Os checksums de x86 e x64 divergem, então o CI só confere x64 | Na simulação: sem iteração de `HashMap`, sem relógio de parede e com RNG semeado. Inteiros ou ponto fixo por padrão; float só onde o original usa, e sem funções transcendentais da libm. CI em duas arquiteturas | Desde o primeiro commit do `rt2-sim` |
| 4 | **Snapshots e diff de estado** | Buffer circular de 32 snapshots para depurar dessincronização | Helper de teste que compara dois estados campo a campo. Também serve para comparar com o original no Wine | Fase 4 |
| 5 | **Teste diferencial contra o original** | O OpenRCT2 começou injetando uma DLL no `rct2.exe` e trocando função por função | Não vamos hospedar o EXE. Em vez disso, comparamos o estado da nossa simulação com o do original (wine+gdb) para as mesmas entradas, um subsistema por vez | Fases 2 e 4 |
| 6 | **Localizar a instalação original** | `rct2Path`, comando `set-rct2` e detecção automática de Steam e Flatpak | `rt2 set-path`, detecção de GOG, Lutris e Steam, e checagem de sha256 dos PK2 | Fase 3 |
| 7 | **Manifesto de assets livres** | `assets.json` com URL e sha256 de repositórios separados (OpenMusic, OpenSFX, objects) | Mesmo modelo para qualquer dado gerado ou livre que não venha do jogo | Pós-1.0 |
| 8 | **Save próprio e importador separado** | `.park` em chunks com `target-version`, `min-version` e digest; `.sv6`/`.sc6` são só importados | Save nativo em chunks com versão alvo e mínima. O importador de `.GAM`/`.MAP` fica em `rt2-formats`, separado | Fase 5 |
| 9 | **Objetos orientados a dados** | `.parkobj` em JSON, com fallback para os objetos binários originais | Locomotivas, cargas e indústrias definidas em RON/TOML, com o loader dos dados originais como um backend | Fase 4 |
| 10 | **Correções de bugs do original atrás de flags** | Bugs corrigidos marcados com `(original bug)` no changelog; opções de jogo e cheats salvos no save | Modo fiel por padrão. Cada desvio tem uma flag nomeada, e o save e o replay guardam a versão das regras, para replays antigos continuarem válidos | Fase 4 |
| 11 | **CLI como ferramenta de RE e de CI** | `simulate`, `convert`, `sprite`, `scan-objects`, `--headless` | `rt2 simulate <save> <ticks>` (imprime o checksum), `convert`, `sprite`, `scan-assets` | `sprite` e `convert` na Fase 1; `simulate` na Fase 4 |
| 12 | **Changelog e higiene de PR** | `changelog.txt` com `- Feature/Improved/Change/Fix: [#n] texto`, checado no CI; template de PR com pergunta sobre uso de IA | `CHANGELOG.md` no mesmo estilo, templates de PR e issue e CI com fmt, clippy e testes | **Agora** |
| 13 | **Divisão em crates documentada** | `libopenrct2`, `openrct2-ui`, `openrct2-cli`, com um readme por diretório | Já seguimos essa divisão. A regra: `rt2-sim` não depende de UI nem de I/O | Contínuo |
| 14 | **API de scripts versionada** | Plugins em JS (Duktape) com `kPluginApiVersion` e tipos TypeScript publicados | Rhai ou WASM pós-1.0. Até lá, reservar uma fronteira de determinismo clara | Pós-1.0 |
| 15 | **Fluxo de localização** | Só `en-GB.txt` no repositório principal; traduções num repositório separado, mescladas toda noite | Todo texto de UI em tabelas de strings desde o início. Repositório separado quando houver tradutores | Estrutura agora, fluxo pós-1.0 |

**Não verificado na pesquisa:** o modelo de sandbox dos plugins e se existe uma opção única de "corrigir bugs originais".

## 11. Fidelidade: o executável é a referência

O manual e o guia de estratégia (`docs/reference/manual-rules.md`) servem para dar pistas e nomes. **A regra vale como está no executável.** Quando o manual diverge do código, vale o código, e a divergência fica anotada na spec.

**Cobertura total da simulação**
- `re/coverage.toml` classifica cada uma das 2.945 funções (1.837 sem thunks): `lib` (CRT, Miles, Smacker, DirectX), `platform` (janela, input, arquivos), `render`, `ui` ou `sim`, com um estado: `unknown`, `named`, `spec` ou `verified`.
- Meta da Fase 2: 100% das funções classificadas e 100% das funções `sim` em `spec`. A porcentagem aparece no README.
- Como regenerar e editar: `docs/re-workflow.md` (`re/scripts/dump_program.sh` → `re/scripts/classify.py`).
- Constantes e tabelas (atributos de locomotivas, cargas, indústrias, custos) são extraídas dos dados (`.data` do EXE e arquivos nos PK2), nunca digitadas a partir do manual.

**Três níveis de verificação**
1. **Função por função, com emulação:** funções puras do EXE (fórmulas de preço, juros, física, produção) rodam isoladas num emulador x86 (Unicorn). Elas recebem milhares de entradas aleatórias, e a saída é comparada bit a bit com a implementação Rust. Ferramenta: `rt2-oracle`, em `crates/`.
2. **Save como oráculo:** carregar um `.GAM` no original (Wine), avançar N dias por automação e salvar, depois fazer o mesmo no clone e comparar os dois estados campo a campo. Isso exige o importador completo de `.GAM`, que vira prioridade junto com a simulação.
3. **Replays de longo prazo:** detectam divergências acumuladas. Toda divergência precisa de uma causa documentada; sem causa, é tratada como bug.

**Determinismo igual ao do original**
- O RNG é replicado exatamente: mesmo algoritmo, semente e ordem de chamadas.
- **Ponto flutuante:** é preciso descobrir se a simulação usa float x87 e com qual precisão. O DirectX costuma colocar o FPU em precisão simples. Se usar, a aritmética é emulada com o mesmo arredondamento.
- Bugs do original são replicados no modo fiel e corrigidos só atrás de flags (seção 10, item 10).

**Meta realista:** fórmulas, regras e RNG idênticos, provados pelos níveis 1 e 2. Uma partida inteira idêntica bit a bit por décadas é a meta máxima, mas a IA e a ordem das atualizações podem tornar isso difícil. Por isso a regra é que nenhuma divergência fica sem explicação.
