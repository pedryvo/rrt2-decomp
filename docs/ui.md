# Interface do clone (`rt2-ui`, `rt2-game`)

Esta página descreve a camada de UI do clone, não os formatos do original (para esses ver `formats/ui.md`, `formats/font.md`, `formats/sprite-catalog.md`).

## Arquitetura

```
rt2-game (binário rrt2)         estado do app, telas, janela winit, screenshots
  state.rs   AppState: menu / jogo, roteamento de entrada, passo fixo da simulação, HUD
  app.rs     laço de janela (winit + wgpu)         gfx.rs  mapa (rt2-render) + UI por cima
  artwork.rs arte original -> Skin                 config.rs / paths.rs / keys.rs / catalog.rs / text.rs
rt2-ui                          UI retida sobre wgpu
  ui.rs      árvore de widgets, layout, foco, hit test, eventos      (sem GPU, testável)
  scale.rs   escala 100-300 %      font.rs  10 fontes do jogo -> faces vetoriais
  skin.rs    atlas + 9-slice + tema    render.rs  quads (shader próprio) + texto (glyphon)
```

- `Ui` é um arena de nós (`Panel`, `Image`, `Label`, `Button`, `List`). Cada raiz é uma camada de desenho: o texto de uma camada é desenhado depois dos quads dela e antes da camada seguinte, então um painel sobreposto esconde o texto de baixo.
- Layout em unidades lógicas: âncora (9 posições + `Fill`) com deslocamento e tamanho; tamanho 0 estica; um nó pode ser `Stack` (coluna/linha, espaçamento, margem, centralizado).
- Entrada: `Ui::input` recebe cursor em pixels físicos (converte pela escala), clique (ativa ao soltar sobre o mesmo botão), roda (rola listas), teclado (Tab/Shift+Tab, setas, Enter/Espaço, Home/End/PgUp/PgDn em listas, atalho de letra por botão). Devolve `UiEvent::Activated(ação)` / `Selected(ação, linha)`; os ids de ação são do aplicativo. `over_ui()` diz se o mouse está sobre um widget (o mapa não reage).
- Toda mudança de estado do jogo passa por `rt2_sim::Command` (`TogglePause`, `SetSpeed`, `SeedRng`); a UI só os enfileira, como o original (`cmd_execute`, `time.md` 5.1).
- Simulação em passo fixo: a cada 100 ms de relógio (`NOMINAL_FRAME_MS`) chama `World::advance(minutos_por_quadro(velocidade))`, no máximo 5 quadros por quadro desenhado. O relógio de parede só existe em `AppState::update`; `rt2-sim` não o vê.
- Renderização: `Renderer` do `rt2-render` desenha o mapa; `UiRenderer` abre um segundo passe (`LoadOp::Load`) por cima. Texto com `glyphon`/`cosmic-text`, shapeado no tamanho físico (sem reamostragem: nítido em qualquer escala) e posicionado em pixels inteiros; sombra de 1 px (em unidades físicas arredondadas da escala) como o grupo 1 dos glifos originais.

## Escala

`pixels físicos = unidades lógicas x escala`, escala em múltiplos de 25 % entre 100 % e 300 %.

1. `--ui-scale N` ou `ui_scale` no `config.toml` (fixa);
2. senão o fator DPI do monitor (`window.scale_factor()`, arredondado), seguindo mudanças de monitor;
3. `Ctrl +` / `Ctrl -` mudam em tempo de execução (e fixam).

O tamanho da tela em unidades lógicas é `físico / escala`; todo o layout é refeito quando a janela ou a escala mudam. O fundo do menu é a arte original (1024x768) em "cover" ancorada no topo, ou seja, independe da escala; painéis, botões, listas e HUD são lógicos.

## Fontes

Todas SIL OFL 1.1, em `assets/fonts/` com a licença ao lado (`OFL-*.txt`). O `FontSystem` só enxerga as fontes vendorizadas (sem fontes do sistema: aparência igual em qualquer máquina).

| Fonte original | Estilo | Face vetorial | Tamanho lógico (px) | Por quê |
|---|---|---|---|---|
| 1 `copp6` | Copperplate | Cinzel | 11 | versaletes serifados com ar de gravura, parente próximo do Copperplate Gothic; piso de 11 px para ler |
| 2 `copp8` (padrão dos controles) | idem | Cinzel | 13 | |
| 5 `copp10` | idem | Cinzel | 17 | |
| 6 `copp12` | idem | Cinzel | 20 | usada no HUD e nas listas |
| 3 `copp14` | idem | Cinzel | 23 | botões e títulos de diálogo |
| 4 `copp24` | idem | Cinzel | 40 | |
| 7 `scri11` | cursiva | Dancing Script | 19 | única script livre legível em corpo pequeno |
| 8 `fran13` | Franklin Gothic | Libre Franklin | 17 | derivada diretamente do Franklin Gothic |
| 9 `haet46` | Haettenschweiler (título) | Anton | 53, caixa alta | gótica condensada pesada; dobra para maiúsculas como o original |
| 10 `htra46` | idem, "transparente" | Anton | 53, caixa alta | mesma face (a transparência vira alfa da cor) |

Tamanho = altura original x fator por face (Copperplate 1,65; Franklin 1,3; script 1,7; Haettenschweiler 1,15), calibrado visualmente nos screenshots (`game_font` em `rt2-ui/src/font.rs`). As fontes Cinzel, Libre Franklin e Dancing Script são variáveis; o peso 700 é pedido para a família Cinzel. Não há kerning nem as métricas bit a bit do bitmap: a largura de texto vem da face vetorial, e o layout não depende das larguras originais.

### Idiomas e fontes

Cinzel, Anton e Dancing Script não têm cirílico; nada acima tem CJK. O `rt2-ui` escolhe a família de cada estilo pelo *sistema de escrita* do idioma atual (`Script::of_locale`, `UiRenderer::set_script`; o `Gfx` informa o script do `AppState` a cada quadro, então trocar de idioma em tempo de execução refaz todos os textos):

| Estilo | Latim | Cirílico (`ru`) | CJK (`zh-CN`) |
|---|---|---|---|
| Copperplate | Cinzel | Cormorant SC (Bold, o peso que o `game_font` pede; subconjunto Latim+Cirílico, 0,3 MB) | Noto Sans SC |
| Franklin | Libre Franklin | Libre Franklin (já tem cirílico) | Noto Sans SC |
| Display | Anton | Oswald (variável, 0,17 MB) | Noto Sans SC |
| Script | Dancing Script | Marck Script (0,08 MB) | Noto Sans SC |

Cormorant SC é o versalete serifado de aparência mais próxima de Cinzel entre os OFL com cirílico (Forum e Playfair Display SC foram os outros candidatos; Cormorant tem o desenho mais fino e gravado). Oswald é a condensada pesada com cirílico mais próxima de Anton.

**CJK: fonte vendorizada, não do sistema.** Usamos `NotoSansSC-subset.ttf` (2,3 MB): Noto Sans SC instanciada no peso 500 (a variável original tem 17 MB) e reduzida a GB2312 (6763 hanzi), pontuação CJK, Latim básico/estendido e símbolos comuns. Motivos: o resto do projeto já não depende de fontes do sistema (aparência e testes iguais em qualquer máquina, o screenshot é reproduzível), e o subconjunto custa só 2,3 MB no binário. Limite conhecido: um hanzi fora do GB2312 (raro em texto de jogo) aparece como "tofu"; se isso virar problema, o próximo passo é carregar também uma fonte CJK do sistema via fontconfig como último recurso. Um único peso e uma única família sem serifa para os quatro estilos em chinês: o Copperplate e o script não têm equivalente CJK com a mesma personalidade, e a hierarquia continua pelo tamanho e pela sombra.

A cadeia de fallback é a do cosmic-text: a família do estilo primeiro, depois qualquer fonte vendorizada que tenha o glifo (por exemplo, letras latinas dentro do texto chinês, ou "中文" no seletor de idioma enquanto o jogo está em inglês).

## Janela principal e fluxo de um jogador (2026-10-22)

Código: `rt2-game/src/state/mainwin.rs` (janela principal), `state/menus.rs` (menus), `state/dialogs.rs` (diálogos modais), com dados em `scenario_info.rs` (descrição, briefing e metas lidos das regras do `.MAP`/`.MP2`, `scenarios.md` 2.2 e 5), `setup.rs` (opções de New Scenario), `founding.rs` (sliders e capital inicial, `finance.md` 17), `radar.rs` (minimapa do terreno). Novos widgets em `rt2-ui`: `Kind::Toggle`, `Kind::Icon` (botão de dois sprites), `Kind::Cells` (grade de cores com `UiEvent::Pointer`), `Kind::Text` (linhas rolantes), `Fill::Frame`.

- **Janela principal** (`mainwin.win`): 8 ícones à esquerda (arte de `mainbut.imb`), painel inferior com radar (clique/arraste move a câmera; o contorno do que a câmera mostra é desenhado nas células), caixa central com 4 abas (Stations, Trains, Players, Companies; duplo clique abre o detalhe) com data e caixa, Activity Box com mensagens e botão Briefing, girar/zoom acima dela, linha de status com a dica do widget (LNG 1772..1803) ou o que há sob o cursor. Velocidade/pausa ficam no canto superior direito (extra). Tamanhos derivam da altura lógica do canvas. Ferramentas de construção abrem um painel provisório marcado.
- **Menus**: coluna de 6 botões, submenu Single Player, New Scenario (Game Setup, Map com lista/descrição/miniatura, Options), New Campaign (classic `#NN`, Second Century `#eNNcamp`; nomes e bônus da clássica em LNG 2765+4i), Load. Opções e bônus são guardados mas ainda não aplicados (sem gancho na simulação).
- **Diálogos** (arte original `eagled`, `paper`, `yearend`, `winlose` decodificada com o decodificador de imagens de UI): briefing e metas, Start New Company, File Options, relatório anual (abre na virada de ano), jornal, mensagem, vitória/derrota.
- **Lacuna do rt2-sim**: `show_message` é stub e `Scenario::game_over` não é legível de fora (`view.rs` não alcança o campo privado de `SimState`). Pontos de entrada prontos: `AppState::show_event` e `show_game_over`; faltam `World::take_messages()` e `World::game_over()`.
- Screenshots: `rrt2 --screenshot out.png --screen menu --page main|single|new-scenario|map|options|campaign|load|scores|credits`, `--screen game`, `--screen dialog --page goals|founding|year-end[-1..3]|news|dialog|win-gold|win-silver|win-bronze|lose|file|tool-0..7 [arquivo]`.

## Skin

Arte original lida da instalação em tempo de execução (nada é versionado), decodificada por `rt2-render::LoadedBank`, ampliada com Scale2x (como os sprites do mapa) e empacotada num atlas único:

| Uso | Banco / registro | Notas |
|---|---|---|
| fundo do menu | `setup.imb` 0 (1024x768) | foto, só filtro bilinear |
| botão solto / apertado | `setupb.imb` 0 / 1 (106x43/44) | 9-slice, bordas de 8 px |
| painel | `setupp.imb` 2 (490x417) | 9-slice, bordas de 18 px |

Sem a arte (ou com um registro ausente) o `Skin::flat()` desenha preenchimentos e bordas do `Theme` (verde-oliva escuro, bronze). Hover clareia o botão, apertado escurece, desabilitado apaga; o foco do teclado é um contorno amarelo.

## Telas e os `.win` originais

| Tela do clone | `.win` de referência | Observação |
|---|---|---|
| Menu principal | `setup.win` (botões `3135..3140`, fundo `setup.imb`) | usa o mesmo fundo; os 6 encaixes do original ficam como decoração e os botões são um painel centrado em unidades lógicas (o original posiciona em pixels de uma tela de design 1024x768) |
| Novo jogo / Carregar | `filerq.win` (lista 401, rótulo 405, botões 406/407) | lista + OK/Cancelar |
| HUD do jogo | `mainwin.win` (botões de velocidade `216..219`) | barra superior com data, caixa, velocidade e pausa; a barra de ferramentas original ainda não existe |

Textos: ids do `default2.lng` quando conhecidos (`New Game` 1686, `Load Game` 1688, `Quit` 1684, `OK` 451, `Cancel` 454, `Cash` 31, `Speed:` 202, `Paused` 991, meses 1441..1452); "Menu", "Save", "-", "+" são placeholders em inglês (TODO).

## `rrt2`: instalação, teclas, saves

- Instalação: `--game-dir`, `RT2_DIR` ou `game_dir` em `~/.config/rrt2/config.toml`; `rrt2 set-path <pasta>` valida (`data/RT2.PK2` e `RT2_PLAT.EXE`) e grava. Sem nada configurado, o erro diz o que fazer. A instalação é só leitura.
- Saves próprios: `~/.local/share/rrt2/saves/` (`$RRT2_SAVES` para mudar), `Nome-AAAA-MM-DD.GAM` via `World::to_save`; o jogo recusa escrever dentro da instalação. Um mapa de cenário puro (Novo jogo) ainda não tem estado de jogo e não pode ser salvo.
- Teclas (`[keys]` no `config.toml`, nomes de `KeyCode` do winit, prefixo `ctrl+` opcional; padrões):

| Ação | Padrão | Origem |
|---|---|---|
| pausa | `Pause`, `Space` | `Pause` do manual |
| velocidade + / - | `Equal`, `NumpadAdd` / `Minus`, `NumpadSubtract` | teclas `+`/`-` do manual |
| salvar | `F5` | |
| menu | `Escape` | |
| girar | `KeyQ` / `KeyE` | |
| enquadrar | `Home` | |
| mover | `WASD`, setas; arrastar com o mouse; roda = zoom | |
| escala da UI | `ctrl+Equal` / `ctrl+Minus` | |
| tela cheia | `F11` | |
| mudo | `ctrl+KeyM` | extra moderno (`[keys] mute`) |
| próxima faixa | `ctrl+KeyN` | extra moderno (`[keys] next_track`) |

### Som

`rrt2` abre o áudio (`rt2-audio`) na partida; sem dispositivo de som ou sem os arquivos, registra uma linha em stderr e roda em silêncio. O fluxo é de mão única: `AppState` acumula `sound::Cue` (`take_cues()`), o laço da janela os entrega ao `SoundSystem`; o áudio nunca lê nem altera a simulação.

- Música (`sound.md` 2.2): categoria 0 no menu principal e nas subtelas (Novo/Carregar jogo), categoria 1 no jogo; só há pedido quando a categoria muda. Fades de 1300 ms (saída) e 500 ms (entrada), nunca repete as 3 últimas, voltar a uma categoria retoma a faixa dela (tudo em `MusicDirector`).
- Sons de UI (`sound.md` 1.3): botão apertado = id 2 `ButtonDown` (ao pressionar o botão esquerdo sobre um widget), solto/ativado = id 1 `ButtonUp`, seleção de lista por clique = id 3 `ListClick`, por roda/teclado = id 4 `ListHover` (volume 90).
- `[audio]` no `config.toml`: `master`, `music`, `sfx` (0 a 100, padrão 100) e `mute` (padrão `false`). O atalho de mudo alterna só durante a sessão.

## Screenshots

`rrt2 --screenshot out.png [--screen menu|new-game|load-game|game] [--size 3840x2160] [--ui-scale 200] [--days N] [arquivo]` renderiza fora da tela (sem janela). O `game` usa o `Example save game.GAM` e avança N dias simulados (semente de RNG fixa `1`).

## Telas de informacao (somente leitura)

Painéis sobre o jogo que só olham o estado da simulação (`World::save_ref`); nenhum enfileira `Command`. O código: `rt2-game/src/info.rs` (registros do save -> tabelas de texto, sem UI, testável), `state/info_ui.rs` (widgets, teclas, cliques) e `pick.rs` (tile sob o cursor).

| Tela | Atalho | `.win` de referência | Conteúdo |
|---|---|---|---|
| Finanças | `F` (ou `Previous`/`Next`, setas) | `compndtl.win`, `yearend.win` | demonstração de resultados por categoria (este ano `+0x4286`, ano passado `+0x42ee`, vida inteira `+0x46fe`; rótulos LNG 961..973, `finance.md` 1), balanço (contas 0..5, dívida, patrimônio; LNG 974..981), preço da ação (`trade_price(0)`, `finance.md` 3.2), valor contábil, receita e lucro por ação, dividendo, ações, valor de mercado, rating (`finance.md` 4.2), títulos |
| Bolsa | `M` | `stockdtl.win` | uma linha por empresa: preço, ações, valor, dividendo, valor contábil, rating; clique abre as finanças dela |
| Trens | `T` | `trainlst.win`, `traindtl.win` | lista (nome, empresa, locomotiva, vagões, rota, velocidade, idade) e detalhe (composição, paradas com espera, peso, estado) |
| Estações | `G` | `statnlst.win`, `statndtl.win` | lista e detalhe (dono, tamanho, melhorias decodificadas pela tabela de `terrain-track.md` 3.4, oferta, espera, demanda, nível de preço e entregas por carga) |
| Cidades | `C` | `cityset.win` | lista e detalhe (tipo, classe de tamanho, casas, índice de crescimento, lotes entregues e despachados, indústrias do lugar) |
| Indústrias | `I` | - | lista e detalhe (tipo, lugar, dono, lucratividade, produção por carga, estoques de entrada) |
| Território | clique no mapa | `bpterrit.win` | região, tiles, cidades e indústrias dentro |

`Esc` fecha, `Backspace` volta do detalhe para a lista, roda / `PgUp` / `PgDn` / `Home` / `End` rolam as listas, o atalho da tela aberta fecha-a. Teclas configuráveis em `[keys]` (`finance`, `stock`, `trains`, `stations`, `cities`, `industries`). Um clique do botão esquerdo no mapa (sem arrastar) abre a página do que está sob o cursor: indústria ou estação (caixa do prédio `+0x26..+0x2d`), cidade ou região (caixa do lugar), senão o território do tile (`pick.rs` desfaz a projeção da câmera, corrigindo pela elevação).

Layout (referência: geometria dos `.win`; as janelas de lista são uma lista 974x686 numa tela de 1024x768, as de detalhe dois painéis lado a lado): o painel preenche a tela abaixo do HUD (largura máxima 1400 unidades lógicas), a altura da linha encolhe (25 a 14) para caber numa tela lógica pequena (escala 200 % em 1080p) e as duas colunas viram uma abaixo de 860 unidades. Arte: o painel 9-slice da skin; botões da skin; tabelas em caixas escuras. O número de linhas visíveis das listas vem da altura; o resto rola.

Screenshots: `rrt2 --screenshot out.png --screen info --info finance|stock|trains|train|stations|station|cities|city|industries|industry|territory [--days N] [--size WxH] [--ui-scale N] [arquivo]`; `--click X,Y` (pixels físicos, com `--screen game`) simula o clique no mapa.

Lacunas (marcadas TODO na tela): lucro do trem e estado da programação; população e demanda por carga das cidades; estado econômico (usa 2, `cargo.md` 9) e conjunto de indústrias (usa 0); qual empresa é a do jogador (usa a primeira ativa com chairman humano); direitos e efeitos de território; **saves no layout antigo 0x233C** (os `.GAM` do jogo): só nomes e posições são lidos, o resto aparece como `-` com um aviso, porque a conversão dos registros no carregamento do original (empresa 0x196C -> 0x4E86, estação 0x39B -> 0x4C7, trem 0x143 -> 0x14D...) não está decodificada. Os saves que o próprio jogo grava (0x2348) mostram tudo.

## Lacunas conhecidas

- A cena do mapa (`rt2-render::Scene`) é montada uma vez no carregamento: trens e construções criados pela simulação ainda não aparecem.
- Qual empresa é a do jogador não está decodificado; o caixa mostrado é a conta 0 da primeira empresa (`finance.md` 2).
- Início de um jogo novo (data, empresa, economia) não está especificado: um mapa de cenário abre como mundo só com relógio, em 1860.

## Pendências de revisão (coordenador, 2026-10-22)

- Tela de finanças, Example 0x2348 após 30 dias: o patrimônio (book value, $3.493.896) supera os ativos ($3.434.896) em $59.000 com dívida zero. Conferir a fórmula de valor contábil contra `finance.md`: o original pode incluir um item não listado na tela (participações, terrenos?) ou a tela soma algo errado.
- A coluna "This year" mostra $0 em 12 de janeiro com trens rodando. Confirmar se o original só acumula no fechamento do mês (finance.md) ou se a tela lê o campo errado.
- A barra de atalhos abaixo do HUD fica parcialmente atrás do painel aberto.
- A altura da fumaça foi tratada como pixels de sprite; a spec não dá a unidade.
- Trens com vagões "empilhados" (Example, curva da estação, verificado em 2026-10-22): não é bug de desenho. A direção de mapa diagonal (d3, d7) é vertical na tela (`sprite-catalog.md` 8), os vagões espaçam 24 unidades ao longo dela, e os trens 1 e 10 ocupam as duas vias (lado 1/2, ±0,27 tile) de uma via dupla; a posição lida do save (`tile + sub/64`, sub pode passar de 63 em curva) bate com `trains.md` 10.1/10.2.
- **Tela de detalhe da estação** (screenshot do usuário, 2026-10-22): vista pré-renderizada da estação com os prédios de apoio comprados (restaurante, rotunda, caixa d'água...) compostos por camadas sobre o prédio principal (tamanho e estilo), painel de oferta e demanda e botão Buy para os prédios de apoio. Hoje a nossa tela de estação é só uma lista: reproduzir a composição a partir dos bancos de sprite de estação (`sprite-catalog.md`, 480 bancos `sta*`) e da tabela de melhorias `0x5500c8`.
- Start New Company: com investimento do jogador de $150.000, "You buy" mostra 3000 ações por $157.500 (preço $52,50). Conferir contra `finance.md` 17 e `company_init` se o original arredonda as ações para cima ou se a fórmula está errada.
- Lista central da janela principal: cabeçalhos de coluna truncados ("IMPROVEMEN", "CARGO WAITI") em 1080p.
