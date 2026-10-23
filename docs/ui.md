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

- **Janela principal** (`mainwin.win`): 8 ícones à esquerda (arte de `mainbut.imb`), painel inferior com radar (clique/arraste move a câmera; o contorno do que a câmera mostra é desenhado nas células), caixa central com 4 abas (Stations, Trains, Players, Companies; duplo clique abre o detalhe) com data e caixa, Activity Box com mensagens e botão Briefing, girar/zoom acima dela, linha de status com a dica do widget (LNG 1772..1803) ou o que há sob o cursor. Velocidade/pausa ficam no canto superior direito (extra). Tamanhos derivam da altura lógica do canvas. As ferramentas de construção (seção abaixo) ocupam o lugar da caixa central.
- **Menus**: coluna de 6 botões, submenu Single Player, New Scenario (Game Setup, Map com lista/descrição/miniatura, Options), New Campaign (classic `#NN`, Second Century `#eNNcamp`; nomes e bônus da clássica em LNG 2765+4i), Load. Ao confirmar, o jogo começa por `World::new_game` (ver "Fluxo de jogo" abaixo); opções que a simulação ainda não recebe são guardadas e listadas como TODO em `setup.rs`.
- **Diálogos** (arte original `eagled`, `paper`, `yearend`, `winlose` decodificada com o decodificador de imagens de UI): briefing e metas, Start New Company, File Options, relatório anual (abre na virada de ano), jornal, mensagem, vitória/derrota.
- Screenshots: `rrt2 --screenshot out.png --screen menu --page main|single|new-scenario|map|options|campaign|load|scores|credits`, `--screen game`, `--screen dialog --page goals|founding|year-end[-1..3]|news|dialog|win-gold|win-silver|win-bronze|lose|file|tool-0..7|dismiss[-N] [arquivo]` (`dismiss` fecha os diálogos que a simulação enfileirou, para fotografar o seguinte); `--screen game` aceita `--opening` (briefing e Start New Company como os menus abrem), `--found NOME` (funda a empresa com os sliders iniciais) e `--campaign N` (inicia como jogo de campanha).

## Fluxo de jogo: iniciar, fundar, jogar (2026-10-22)

Código: `state.rs` (`start_mode`, `StartMode`), `setup.rs` (`Setup::new_game_options`), `state/menus.rs` (`start_flow`, `queue_opening_dialogs`), `state/dialogs.rs` (Start New Company, `poll_world`, `message_dialog`). Testes: `tests/play.rs` (só o simulador) e `state/play_tests.rs` (pelo estado da aplicação).

- **Abrir**: `.GAM` carrega por `World::from_save_seeded` (qualquer save do jogo; o layout antigo `0x233C` é convertido como o carregador original, e a cena, o radar e as telas leem o estado convertido que o mundo guarda). `.MAP` / `.MP2` iniciam por `World::new_game(mapa, &NewGameOptions, dados, semente)`. As telas de informação e a construção funcionam em qualquer save: os avisos de "layout antigo" e "construção exige 0x2348" não existem mais. Sem empresa humana a construção avisa para abrir uma.
- **Opções**: `Setup::new_game_options` parte do bloco de opções do próprio mapa e grava por cima: semente do jogo (`+0x2e`, também RNG e CRT do início), ano inicial (`+0x22` = ano do cenário; o bloco do arquivo traz um valor velho, B&O 1880), modelos financeiro (bits 3..5 de `+0x32`: 0/8/0x10) e industrial (bits 0..2: 0/1/2). Campanha: bloco de campanha (`+0` dificuldade, `+0x16` índice, `+0x1a` em andamento, `+0x2a` bônus, `+0x3d` conjunto; larguras inferidas do espaçamento dos endereços) e as opções fixas `(op & 0xd2) | 0x12` de `0x410de0`. A semente é fixa em testes e screenshots (`RNG_SEED`); a janela passa o relógio (`AppState::set_seed`), o simulador nunca vê o relógio.
- **TODO (sem gancho em `rt2-sim`)**: sandbox, modificadores de receita (humano e computador), computadores extras (a montagem das vagas `0x4b45b0` e `player_remove` `0x4a6a00` não estão portadas: cenários de vários jogadores humanos, como Amaplanca, começam com todas as pessoas do mapa marcadas como humanas), bônus escolhido da campanha, escolha do ano inicial nos mapas que permitem. Os campos ficam em `Setup`.
- **Cenários com empresa pronta**: os mapas da campanha (B&O...) trazem a empresa do jogador; só os que não trazem (por exemplo "Africa After the Flood") abrem Start New Company. A pilha de diálogos ao iniciar: Start New Company embaixo, o briefing (a mensagem do gatilho 6 da sequência de início, ou o texto das regras num save carregado) em cima.
- **Start New Company**: sliders de `founding.rs` (conferidos contra os de `rt2_sim::found` em teste); os números (capital, preço, ações, "You buy") vêm de `rt2_sim::found::founding_preview`. O nome é digitado (ASCII, 30 bytes, Backspace apaga, Enter confirma, Esc cancela) com o padrão `TEMP_COMPANY_NAMES[contador % 40]`. OK chama `World::apply_found` (não `Command::StartCompany`, que descarta o erro) e mostra a recusa no próprio diálogo: já é chairman (LNG 2863), 36 empresas (144), menos de dois anos (2708), caixa abaixo de $100.000 (145), valores inválidos. Logotipo e cor: automáticos (`-1`), TODO seletor.
- **Mensagens e fim de jogo** (`poll_world`, todo quadro depois dos passos): `take_messages()` vira linha da Activity Box (primeiro parágrafo) e, por tipo, diálogo (regra tipo Dialog, avisos históricos, falência), jornal (regra tipo Newspaper: manchete = texto, subtítulo = rótulo 1; economia, primeiro prédio de cada tipo, oferta de bônus de cidade: manchete curta e a frase como notícia) ou só a linha (empresa de IA começou ou conectou, desdobramento). Escolha (Choice): só o botão 0, que é o que o motor aplica. `game_over()` abre a placa uma vez, com `game_over_text()` (ou LNG 0xa83 se outro jogador venceu); fechar volta ao menu. Os diálogos seguram a simulação.
- Lacunas: a manchete da mudança de economia usa o nome do estado (LNG 1215..1219) porque os ids de `0x5538dc` não estão na spec; o fim de jogo não pode ser forçado de fora (`GameOver` é privado do `rt2-sim`), então a placa foi fotografada pelo caminho `--page win-gold`.

## Construção e trens (2026-10-22)

Código: `rt2-game/src/buildtools.rs` (regras das janelas emissoras, sem UI: permissões `0x5706b4..b9`, direitos, lista de motores, melhorias, prévia), `state/build_ui.rs` (ferramentas Lay Track, Build Station e Bulldoze, painel e sobreposição no mapa) e `state/build_dialogs.rs` (detalhe da estação com Buy, compra de trem, detalhe editável do trem com mapa-roteador, indústria com Buy/Sell, direitos de território). Toda ação é um `BuildCmd` enviado por `World::apply_build`; a **prévia** é o mesmo comando aplicado a um clone do mundo (`buildtools::preview`), então o caminho, o custo e a recusa mostrados são exatamente os do comando real e o mundo verdadeiro não é tocado.

- Entrada: com uma ferramenta ativa o botão esquerdo age no mapa (arrastar = trilho, clique = estação ou demolição); o direito continua movendo a câmera; `Esc` cancela o arrasto e depois a ferramenta. A sobreposição é feita de nós `Fill::Overlay` do `rt2-ui` (translúcidos, não pegam o mouse), um por tile, posicionados com `pick::tile_to_screen`.
- Só com uma empresa humana: em qualquer save carregado (o layout antigo é convertido na carga) ou depois de Start New Company. Sem empresa a construção avisa para abrir uma.
- Screenshots: `rrt2 --screenshot out.png --screen game --tool track|station[:medium|:large]|bulldoze --drag-from X,Y --hover X,Y [--apply] SAVE.GAM` (tiles do mapa; `--apply` envia o comando) e `--screen dialog --page station|station-buy|train-buy|train-edit|industry|rights`.
- **Finanças (ações do jogador)**: `state/dialogs/fin.rs`. Diálogos sobre a moldura da águia (`Dialog::Fin(FinDlg)`), ids de ação 6000..6011 (botões) e 6100..6111 (botões do livro da empresa, `A_FINACT`); texto e números do `rt2-sim` (`World::fin_*`), comandos por `World::apply_fin` (checagens da janela original antes do executor). Capturas: `--screen dialog --page fin-bond|fin-stock|fin-buyback|fin-dividend|fin-merger|fin-bankrupt|fin-repay|fin-manager|fin-resign|fin-trade|fin-assume|fin-election|fin-vote [SAVE.GAM]` (o save precisa ter uma empresa do jogador, por exemplo o Example do conjunto de fidelidade) e `--screen info --info finance|stock`. A votação do acionista humano (`World::pending_votes`) abre sozinha o diálogo Yes/No (LNG 164/165).
- Teste de ponta a ponta: `crates/rt2-game/tests/e2e_build.rs` (só o simulador, a partir do Example carregado: trilho, estação, trem com duas paradas, 120 dias) e `state/build_tests.rs` (as mesmas ações pelos botões do estado da aplicação).

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
- Saves próprios: `~/.local/share/rrt2/saves/` (`$RRT2_SAVES` para mudar), `<nome>.GAM` via `World::to_save` com sidecars `.toml` e `.png` (ver "Opções e saves"); o jogo recusa escrever dentro da instalação. Um jogo novo (cenário ou campanha) já tem estado e salva como qualquer outro.
- Teclas (`[keys]` no `config.toml`, nomes de `KeyCode` do winit, prefixo `ctrl+` opcional; padrões):

| Ação | Padrão | Origem |
|---|---|---|
| pausa | `Pause`, `Space` | `Pause` do manual |
| velocidade + / - | `Equal`, `NumpadAdd` / `Minus`, `NumpadSubtract` | teclas `+`/`-` do manual |
| salvar rápido | `F5` | |
| menu | `Escape` | |
| girar | `KeyQ` / `KeyE` | |
| enquadrar | `Home` | |
| mover | `WASD`, setas; arrastar com o mouse; roda = zoom | |
| escala da UI | `ctrl+Equal` / `ctrl+Minus` | |
| tela cheia | `F11` | |
| mudo | `ctrl+KeyM` | extra moderno (`[keys] mute`) |
| próxima faixa | `ctrl+KeyN` | extra moderno (`[keys] next_track`) |
| salvar / carregar rápido | `F5` / `F9` | `[keys] save` / `quick_load`; um slot fixo (`quicksave`) |
| diálogos Salvar / Carregar | `ctrl+KeyS` / `ctrl+KeyL` | `[keys] save_dialog` / `load_dialog` |
| Opções | `F10` | `[keys] options`; tudo é remapeável pela tela (ver abaixo) |

### Som

`rrt2` abre o áudio (`rt2-audio`) na partida; sem dispositivo de som ou sem os arquivos, registra uma linha em stderr e roda em silêncio. O fluxo é de mão única: `AppState` acumula `sound::Cue` (`take_cues()`), o laço da janela os entrega ao `SoundSystem`; o áudio nunca lê nem altera a simulação.

- Música (`sound.md` 2.2): categoria 0 no menu principal e nas subtelas (Novo/Carregar jogo), categoria 1 no jogo; só há pedido quando a categoria muda. Fades de 1300 ms (saída) e 500 ms (entrada), nunca repete as 3 últimas, voltar a uma categoria retoma a faixa dela (tudo em `MusicDirector`).
- Sons de UI (`sound.md` 1.3): botão apertado = id 2 `ButtonDown` (ao pressionar o botão esquerdo sobre um widget), solto/ativado = id 1 `ButtonUp`, seleção de lista por clique = id 3 `ListClick`, por roda/teclado = id 4 `ListHover` (volume 90).
- `[audio]` no `config.toml`: `master`, `music`, `sfx` (0 a 100, padrão 100) e `mute` (padrão `false`). O atalho de mudo alterna `mute` e grava no `config.toml` (a tela de Opções mostra o mesmo estado).

## Opções e saves (2026-10-23)

Código: `rt2-game/src/options.rs` (modelo: abas, linhas, faixas, como uma mudança edita o `Config`; sem UI e sem I/O), `keys.rs` (`KeyAction`, conflitos, `rebind`/`reset`), `saves.rs` (arquivos, metadados, rotação do autosave; I/O e relógio de parede ficam aqui, nunca no `rt2-sim`), `state/options_ui.rs` e `state/saves_ui.rs` (os painéis). Os dois painéis são *overlays* do `AppState` (`Overlay`), desenhados por cima do menu ou do jogo, seguram a simulação e recebem todas as teclas enquanto abertos. Textos novos: tabela `[app]` (ids 100238..100328; tradução pt-BR; os outros idiomas caem no inglês).

**Tela de Opções** (botão "Options" no canto superior direito de todas as páginas do menu, botão Options do File Options, `F10`). Cada mudança vale na hora e é gravada em `~/.config/rrt2/config.toml` (`$RRT2_CONFIG`). Cinco abas (`←`/`→` trocam de aba):

| Aba | Itens | Chave no `config.toml` |
|---|---|---|
| Display | modo da janela (janela / tela cheia sem bordas), tamanho da janela (1024x768 a 3840x2160, mais o valor do arquivo), monitor, vsync, escala da interface automática (DPI do monitor) ou 100 a 300 % em passos de 25 | `[display] mode, width, height, monitor, vsync`; `ui_scale` |
| Audio | volume geral, música, efeitos (0 a 100, passo 5), mudo | `[audio] master, music, sfx, mute` |
| Language | um botão por idioma (troca na hora, relayout completo) | `language` |
| Keys | lista de ações com as teclas; Rebind (substitui), Add key (alternativa), Reset this key, Reset all keys | `[keys]` |
| Gameplay | velocidade ao iniciar (0 = a do save), autosave a cada N meses de jogo (0 = desligado), quantos autosaves manter, rolagem na borda da janela, velocidade da câmera (25 a 300 %), inverter arrasto do mouse, inverter zoom da roda, velocidade do zoom | `[gameplay] default_speed, autosave_months, autosave_keep, edge_scroll, pan_speed, invert_drag, invert_zoom, zoom_speed` |

- **Controles**: botões Toggle (ligar/desligar), Toggle como botão de rádio (escolhas), e "sliders" feitos de `Kind::Cells` (clique ou arraste na barra) com botões `-`/`+`. Cada aba tem "Reset this tab"; a de teclas restaura todas as teclas.
- **Remapear teclas**: selecione a ação, Rebind (ou Enter) e pressione a nova tecla (`ctrl+` conta como parte da tecla; `Esc` cancela; modificadores sozinhos são ignorados). Se outra ação já usa a tecla, nada muda e a tela avisa quais; pressionar a mesma tecla **de novo** tira-a da outra ação e passa para esta. Tecla = par (ctrl, `KeyCode`), então `q` e `KeyQ` são a mesma tecla e `Equal` difere de `ctrl+Equal`. Reset de uma ação devolve também a tecla padrão que outra ação tenha tomado. Os padrões nunca colidem entre si (teste).
- **Janela**: o laço da janela recebe `Effect::ApplyDisplay` (modo, tamanho, monitor, `Fifo` ou `AutoNoVsync`) e `Effect::SaveConfig` (só o último de cada lote vale). F11 também passa pelo config. A janela abre com o tamanho e o modo gravados. O tamanho da janela não vale em tela cheia sem bordas (usa o do monitor).
- **Gameplay**: nenhuma opção muda regras. A velocidade inicial vai pelo mesmo `Command::SetSpeed(Absolute)` dos botões do HUD. Rolagem na borda: cursor a até 4 px da borda e fora de qualquer widget (a barra do HUD cobre as bordas esquerda e inferior no jogo; em tela cheia o topo e a direita funcionam sempre).
- Limite conhecido: em escala lógica muito pequena (300 % em 1080p) as linhas encolhem até 26 unidades e a aba Gameplay (8 linhas) pode passar do painel.

**Saves próprios** (`~/.local/share/rrt2/saves`, `$RRT2_SAVES`; nunca dentro da instalação):

- Um save é o `.GAM` do original (`World::to_save().to_bytes()`) mais dois arquivos de mesmo nome: `<nome>.toml` (versão, tipo manual/quick/auto, cenário, empresa, data do jogo, caixa, hora da gravação em UTC) e `<nome>.png` (miniatura = as cores do radar de terreno, RGB 8 bits, uma célula por pixel). Os dois são opcionais: um `.GAM` sem eles lista e carrega do mesmo jeito. Gravação atômica (arquivo temporário e `rename`). Nomes digitados passam por `sanitize_stem` (sem `/`, `\`, `:` etc., até 60 caracteres).
- **Save Game** (File Options > Save, `ctrl+S`): lista dos saves próprios, campo de nome (digite; Backspace apaga; clicar numa linha copia o nome), botão vira "Overwrite" se o nome já existe, "Delete" com confirmação em dois toques, caixa de detalhe com o jogo atual. **Load Game** (File Options > Load, `ctrl+L`, menu Single Player > Load Scenario): saves próprios (mais novos primeiro, marcados `(auto)` e `(quick)`) e depois os `.GAM` originais de `games/` (`(original)`); a caixa mostra miniatura, cenário, empresa, data do jogo, caixa e hora; sem sidecar, lê o mapa do arquivo para a miniatura e o título. Duplo clique ou Enter carrega. Um save próprio continua de onde estava (sem repetir o briefing); um original abre o briefing como o Load Game do menu.
- **Quick save/load**: `F5` grava `quicksave` (toast "Quick saved"), `F9` carrega (ou avisa que não há).
- **Autosave**: a cada `autosave_months` meses de jogo (o contador parte da data de início ou do último autosave; carregar uma data anterior reinicia) grava `autosave-NNNN` e apaga os mais antigos além de `autosave_keep` (sempre fica ao menos um). Só o laço da janela liga a gravação (`AppState::set_persistence`), então testes e screenshots nunca escrevem nas pastas do usuário. Não grava com um Choice pendente nem depois do fim do jogo.
- Limite: a hora dos saves é UTC (não há banco de fusos); a data de jogo é a do calendário do jogo.

**Mensagens Choice** (resolvido): o laço da janela liga `World::set_interactive_choices(true)`. Com um `World::pending_choice()` o `poll_world` abre o diálogo com os rótulos 1 e 2 da regra por cima de tudo (a simulação espera); o botão envia `Command::AnswerChoice { message_id, button }`; fechar o diálogo de outro jeito faz o `poll_world` reabri-lo. A mensagem repetida pelo passo reexecutado só vira linha da Activity Box (não abre outro diálogo). Sem o modo interativo (testes, screenshots, `rt2-sim` puro) vale o botão 0 de imediato, e `take_choice_answers()` só devolve respostas que o mundo não aceitou (diálogos de exemplo).

Screenshots: `rrt2 --screenshot out.png --screen options --page display|audio|language|keys|gameplay|keys-rebind|keys-conflict [arquivo.GAM]` (sem arquivo, sobre o menu; com arquivo, sobre o jogo) e `--screen saves --page save|load [--save-as NOME]` (`--save-as` grava um save em `$RRT2_SAVES` antes, para ter o que listar). Capturas em `~/.cache/rrt2/shots/options/`.

Lacunas: sem controle (gamepad) nem Steam Deck; sem as opções de gráficos do Control Panel original (detalhe, sombras, texturas: a renderização do clone não tem esses níveis); sem a aba "General" do original (profundidade de cor, uso de memória, vídeo de abertura, linhas de grade); acessibilidade (daltonismo, tamanho de fonte) além da escala da interface ainda não existe.

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

Lacunas (marcadas TODO na tela): lucro do trem e estado da programação; população e demanda por carga das cidades; estado econômico (usa 2, `cargo.md` 9) e conjunto de indústrias (usa 0); qual empresa é a do jogador (usa a primeira ativa com chairman humano); direitos e efeitos de território; (resolvido) saves no layout antigo 0x233C: o mundo converte na carga e as telas leem o estado convertido; os avisos de layout antigo em `info.rs` ficaram sem uso.

## Lacunas conhecidas

- A cena do mapa (`rt2-render::Scene`) é montada uma vez no carregamento: trens e construções criados pela simulação ainda não aparecem.
- Qual empresa é a do jogador não está decodificado; o caixa mostrado é a conta 0 da empresa do primeiro humano ativo (`finance.md` 2), e nada antes de a empresa existir.
- Opções da tela de configuração sem gancho na simulação: ver "Fluxo de jogo".

## Polimento da interface (2026-10-22)

Resolvidos nesta passada (capturas em `~/.cache/rrt2/shots/polish/`, antes e depois):

- **Minimapa (radar)**: a cor de cada célula vem da classe de terreno do mapa montado pelo carregador (`rt2_render::build_grid`, `class_of_id`), não do byte de chão cru: água = classe 18 (era `GROUND_WATER`, que não vale para os mapas de ids novos como "Africa After the Flood"), 17 classes de terra com cor própria (praia, deserto, ..., rochas) e um sombreado leve pela elevação. Conferido em Africa After the Flood, Britain, Europe, Atlantis e France (`radar.rs`).
- **Texto que não cabe**: o `rt2-ui` encolhe o texto de uma linha até 55 % do tamanho para caber na caixa e, abaixo disso, corta com reticências (cabeçalhos "IMPROVEMENTS" e "CARGO WAITING" da lista central, rótulos de botão, células). Vale em qualquer escala e nas seis línguas.
- **Detalhe da estação**: a imagem é a cena original de 1024x569 (fundo da classe do sítio mais uma camada por prédio e melhoria, `stationview.rs`; mapeamento dos 55 bancos em `formats/sprite-catalog.md` 6.1). As camadas são compostas na CPU num sprite reservado do atlas (`SkinBuilder::reserve`, `UiRenderer::upload_sprite`) quando a estação, o estilo ou as melhorias mudam.
- **Lista da bolsa (`M`)**: botões Buy e Sell por linha, ativos conforme a cotação (`fin_trade_quote`), que abrem o diálogo de negociação da empresa (o mesmo do botão Trade do relatório).
- **Mensagens Choice dos cenários**: o diálogo mostra os dois botões (rótulos 1 e 2 da regra). (Resolvido em 2026-10-23: ver "Opções e saves", o motor espera `Command::AnswerChoice`.)

## Pendências de revisão (coordenador, 2026-10-22)

- Tela de finanças, Example 0x2348 após 30 dias: o patrimônio (book value, $3.493.896) supera os ativos ($3.434.896) em $59.000 com dívida zero. Conferir a fórmula de valor contábil contra `finance.md`: o original pode incluir um item não listado na tela (participações, terrenos?) ou a tela soma algo errado.
- A coluna "This year" mostra $0 em 12 de janeiro com trens rodando. Confirmar se o original só acumula no fechamento do mês (finance.md) ou se a tela lê o campo errado.
- A barra de atalhos abaixo do HUD fica parcialmente atrás do painel aberto.
- A altura da fumaça foi tratada como pixels de sprite; a spec não dá a unidade.
- Trens com vagões "empilhados" (Example, curva da estação, verificado em 2026-10-22): não é bug de desenho. A direção de mapa diagonal (d3, d7) é vertical na tela (`sprite-catalog.md` 8), os vagões espaçam 24 unidades ao longo dela, e os trens 1 e 10 ocupam as duas vias (lado 1/2, ±0,27 tile) de uma via dupla; a posição lida do save (`tile + sub/64`, sub pode passar de 63 em curva) bate com `trains.md` 10.1/10.2.

- ~~Start New Company "You buy" maior que o investimento~~: correto. O original arredonda as ações do fundador para o milhar mais próximo; a UI deve usar `rt2_sim::found::founding_preview`.
- Opções: em 300% de escala numa tela 1080p, a aba Gameplay pode transbordar o painel.
- Horários dos saves aparecem em UTC; usar o fuso local.

## Editor de mapas e cenários (2026-10-23)

Código: `rt2-sim/src/editor.rs` (regras das ferramentas, portes das rotinas do clique `0x48eb30`), `rt2-sim/src/tick/editor.rs` (blends do terreno, prédios, escavadeira, recontagens), `rt2-sim/src/scenario/edit.rs` (validação da condição, montagem do registro de regra), `rt2-game/src/state/editor_ui.rs` (janela do editor, ferramentas, painel e sobreposição) e `editor_modals.rs` (diálogos). Testes: `rt2-sim/tests/editor.rs`, `rt2-formats/tests/map_roundtrip.rs`, `state/editor_tests.rs`.

- **Entrada**: menu principal, Editor, New Map ou Load Map (`Page::Editor`). O editor é a janela principal com a simulação parada (`World` nunca avança; `update` só atualiza câmera e reconstrói a cena em ritmo limitado).
- **Painel inferior**: grupos de ferramentas à esquerda (Select, Height, Paint, Fill, Trees, Territories, Add, Bulldoze, Overview), paleta do grupo no meio (as janelas `bp*.win`: tamanho do pincel `1..5`, classes de terreno, misturas, tipos de árvore, lista de territórios, lista de prédios e os 6 comandos especiais), radar e botões (Undo, Map Options, Rules, Save, Save As, Open, New Map, Quit) à direita. Sem a arte `bp*.imb`: botões do tema do clone.
- **Ferramentas** (todas chamam o porte da rotina original): Height arrasta (`0x426460`), alisa/nivela com Shift ou Ctrl (`0x426430`), baixa ao nível 0 (`0x426410`); Paint sólida (`0x42a3e0` + `0x426d30`) e mista (`0x48cc00`); Fill adjacente (`0x490490`) ou tudo (`0x490bd0`) com Match/Above/Below e água que nivela os cantos; Trees `0x429fc0` com os 8 tipos; Territories (`0x42c320`) com New/Edit e a janela Territory Setup; Add: 98 prédios (comando `0x17` com dono 0, campos e elos de estação), City/Region/Label (janela de lugar, Cancel remove), River `0x429ea0`, Reserve Cells `0x428630`, Road `0x429e30`; Bulldoze (`0x0a` com dono 0, demole prédios); Overview colore o radar por território. Cada clique empilha uma cópia do mapa para Undo (12 passos, Ctrl+Z).
- **Map Options**: nome, texto de briefing, ano inicial, caixa inicial das pessoas, quatro metas (bronze, prata, ouro, derrota; cada uma é uma regra com o efeito `Game Over` 1..4), as 22 restrições (`0x5706a9+i`), modelos financeiro e industrial (`+0x32` do bloco `0x574840`) e conjunto de indústrias (`0x5704ac`). Jogadores, gerentes, motores, portos e ladrões ficam como o mapa os tem (sem mapeamento de bytes na spec).
- **Rules**: lista de regras (adicionar, copiar, apagar, subir, descer), gatilho (8), condição validada ao vivo pelo compilador (`scenario::edit::validate`, erros com os textos do original), flags de laço derivadas como `rule_set_condition` (`0x457a30`), tipo de mensagem, textos, e os efeitos das linhas nomeadas da tabela de 350 com faixa `min..max`, valor `-100` = valor do gatilho, permanente/temporário e aviso de escopo (`0x458050`). Uma regra que o jogador não toca volta byte a byte (`edit::build` só troca os campos que o editor modela: 3164 de 3191 regras dos 95 `.MP2` idênticas; o resto são as de condição malformada).
- **Salvar**: `.MP2` (layout `0x2348`, como o gravador do original) em `<saves>/maps/`; a visão geral 170x170 é gerada como `0x477040` (idêntica nos `.MP2` salvos pelo editor original) e o checksum é refeito. Nunca grava dentro da instalação. Um `.MAP` antigo é convertido na carga e salvo como `.MP2`. Carregar e salvar um `.MP2` sem editar dá o mesmo arquivo, salvo as contagens de jogadores do cabeçalho e o checksum (95 de 95).
- **Screenshots**: `rrt2 --screenshot out.png --screen editor [mapa] --page height|paint|fill|trees|territory|add|bulldoze|overview|select [--sub N] [--item N] [--edit-at X,Y]... [--hover X,Y] [--tool new|open|save-as|options|goals|restrictions|economy|industries|rules|place|territory]` (`--sub` na lista Add é a linha: 0..5 especiais, 6.. prédios).
- **Lacunas**: From Image (PCX e escala de altura), Players/Player Pool/Managers/Engines/Ports/Robbers de Map Options, a arte original dos painéis, remover um território, tela de overview do original (`bpovervw`), validação contra o original no Wine das ferramentas (os efeitos seguem as fórmulas portadas; o oráculo não foi usado).
