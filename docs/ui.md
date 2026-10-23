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

- **Janela principal** (`mainwin.win`, redesenhada em 2026-10-23 sobre a arte original): o layout é o de 1024x768 do original escalado por `dz = min(altura/768, largura/1024)` e preso ao canto inferior (`Geometry`, `mainwin.rs`); em tela mais larga a largura extra (`ex`) entra no meio do quadro inferior, de modo que radar, ícones e trilho esquerdo ficam na borda esquerda, botões girar/zoom, caixa de atividade, bússola e trilho direito na borda direita e a caixa central cresce (mais colunas na lista). A escala da interface (100 a 300 %) só muda o tamanho lógico da tela, o quadro ocupa sempre a mesma fração dela. **Bancos**: `mwfrme.imb` 0/1 (trilho esquerdo 85x397 em (0,6); o registro 1, um trilho direito 70x340, não é desenhado: o original não tem nada na borda direita acima do quadro inferior; do esquerdo some o tanque da primeira chapa, ficando só o poste fino, e as 8 ferramentas ocupam as chapas 1..8, sem buraco entre globo e trem), 2/3 (quadro inferior em duas fileiras, 1024x105 em y 403 e 1024x260 em y 508, cortadas em esquerda / filler / direita: o filler é a janela x 402..515 da fileira 0 (entre dois postes, período 113) e x 520..633 da fileira 1, repetido por `Fit::Tile`; a lacuna entre a caixa de data e a faixa de abas em tela larga é preenchida com a faixa preta rebaixada da própria caixa de data, `MainArt::tab_fill`), `mwfrma.imb` 0..2 (sombras internas de radar, centro e atividade), `maintab.imb` 0..3 (faixa de abas 207x45 com a aba i acesa; as barras 257/258/260/259 são `Kind::Hotspot`), `compass.imb` (5 registros por rotação: seta e 4 placas N/E/S/W; `compass_index` escolhe pela direção do norte (-y) na tela), `brlogo.imb` 0 (logotipo RT2, 237x214), `mainbut.imb` (ícones 44x34 nas 9 chapas do trilho esquerdo, a 1ª chapa, no topo, fica vazia; girar/zoom 51x34 em (788|844|903|959, 432); botão de lista 38x36), a textura mostarda do centro é um recorte espelhado do chão da fileira 1. Posições (buracos transparentes do quadro, medidos): radar (21,467,236x244), lista (287,526,412x158), caixa de atividade (766,497,237x214), faixa de abas (490,687), caixas de caixa e data (287..488, 687). **Caixa central**: abre na aba Companies, que lista uma linha alta por empresa (logo `brlogo00..40`, nome, `Cash $299K`, `Profits $0K` = lucro do ano até agora, `Info::company_rows`); as outras abas usam a tabela de antes; Map Overview e ferramentas escurecem a textura e ocupam a caixa. Dica das abas: texto do LNG na linha de status e uma caixa flutuante com o nome. **Caixa de atividade**: logotipo com as últimas mensagens por cima; clicar na caixa troca pelo histórico (50 mensagens, rolagem pela roda) com os botões Briefing e Close (o cadeado de `mainmisc.imb` não tem função conhecida e não é desenhado). **Bússola**: `Image` não clicável, como no original (`mainwin.win` só tem imagens 280..284); gira com a câmera. Velocidade/pausa ficam à esquerda da bússola (extra). Testes: `state/mainwin_tests.rs` (cliques sintéticos nas abas, girar/zoom/bússola, histórico, 1080p, ultrawide, 4K e 300 %).
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

## Janelas de compra (2026-10-23)

Código: `artwork/buywin.rs` (arte), `trainview.rs` (imagem do trem), `state/buy_ui.rs` (janelas), `buildtools::engine_detail`. Tudo é posicionado em coordenadas de projeto do `.win` (774x578 centrado) multiplicadas por **um** fator `k = min(largura/774, altura/578)`, então a janela serve a qualquer resolução. `trainbuy.win` e `buildbuy.win` usam as mesmas peças de `trainbuy.imb`; o quadro é composto uma vez (`compose_frame`: controles `Image` na ordem do arquivo, cada registro no seu próprio deslocamento) num sprite só. A caixa da lista não tem arte própria: mostra o couro `trainbuy.imb` 8, como o painel da direita.

**E1 Compra de trem** (`trainbuy.win`; `0x4d9d20` janela, `0x4e3f90` painel 1707, `0x4d9bd0` linha, `0x4d9a50` botões) [V visual contra M:C609]:
- Lista (68,44, 229x450): linhas de 54 px, 197 de largura (27 px à direita são a barra de rolagem de `trainblb.imb`: setas 0/2, trilho 4, cursor 5). Linha = vista lateral `side11_8.imb` registro `vehicle_type +0x27` do veículo da locomotiva, desenhada em (x+7, y-39) mais o deslocamento do registro (fica a 11 px do topo), e o nome (LNG `0x5534f4[motor]`) em (x+10, y+41). O banco é o 9º da tabela de bancos de interface de `0x54fc64` (ground, SMOKE, mainmisc, tdmisc, `side05_0`..`side15_1`: `DAT_00567a38` = `side11_8`). Ordem: pelo primeiro ano do veículo (já na spec de comandos).
- Imagem (357,76, 293x153): terreno `trainbuy.imb` 6 e os sprites `cr1<nome>.imb` registro 3 da locomotiva e do tender (`+0x2b`/`+0x2d` do motor), tender primeiro. Com `A` = comprimento da locomotiva e `B` = do tender (veículo `+6`): `off = max(0, ftol((265,64 - (A+B)) * 0,5))`; âncora da locomotiva a `A/2 + off` e a do tender a `A + B/2 + off` do ponto (230,144) da caixa, na diagonal `(-0,862, -0,431)` por unidade. Sob a imagem, o primeiro ano do motor.
- Texto (esquerda/direita justificado, 13 px de passo): nome; Cost (LNG 154) = preço com modificadores; Maintenance (155) = `mod(0x15) * mod(0x1b+t) * nível_man * manut` "/ year" (LNG 209); Fuel (157) `(Steam|Diesel|Electric)` = `combustível * nível_comb * mod(0x11) * ((v*0,7+40)*0,5 * mod(0x12+t))` "/ year", `v` = `engine_fuel_speed` `0x4d6030` (acelerador 85); Acceleration (156) = LNG 1251 + `clamp((accel+1)/5, 0, 11)`; Reliability (2340) = LNG 2475 + classe 0..10. Os níveis vêm do cabeçalho do save (`+0x235 + tipo`, 1 = neutro; `rt2-sim` já os usa).
- "Top speed (mph)" (LNG 441) e "Cars @ N tons each" (LNG 2747) com as setas de `trainbuy.imb` 25..28 em (616,358) e (616,369): `0x4d9a50` soma ou tira 5 t (5..100). Toneladas iniciais pela data (`0x4e57c0`): < 1830 10, < 1850 20, < 1880 30, < 1910 40, depois 50.
- **Matriz**: 4 linhas de rampa (0, 2, 4, 6 %) x 6 colunas (1 a 6 vagões); célula = `engine_max_speed(motor, (c+1) * t + motor +0x2e, g * 0,02, curva 0, idade 0) >> 16` com os modificadores `0x2a`, `0x2b+t`, `0x23`, `0x24+t` (`0x4e3f90`; `trains.md` 4.2). O manual C609 (build antigo) mostra 47 42 37 33 29 26 na linha de 0 % para o 8-Wheeler; o EXE dá 48 44 40 36 33 30 com 30 t por vagão. **Vale o EXE.**
- Botões Purchase/Cancel em (336,472) e (534,472), 138x63 (`trainbuy.imb` 2/3 e 4/5, rótulo em (18,23,108,26)). Comprar recusa com LNG 353 se o preço passa do caixa. A janela original **não** tem seleção de rota a copiar: o clone a retirou (a compra usa as duas primeiras estações).

**F2 Comprar melhorias** (`buildbuy.win`; `0x407e80`, `0x407540` eventos, `0x407910` painel, `0x407c70` linha) [V visual]:
- Só as linhas disponíveis, não compradas e não excluídas (`0x407e80`), 22 px cada: caixa de seleção `buildbuy.imb` 0/1 (15x15) e o nome (LNG 1043 + linha). Marcar uma de um par (1/2, 3/4, 5/6, 15/16) desmarca a outra (`0x407540`). Clicar no nome seleciona (`DAT_00539a74`, começa em -1: sem texto e com a foto da estação pequena).
- Foto da linha selecionada: `bm<chave>.imb` registro 0 (218x134) em (393,97) dentro de `trainbuy.imb` 19; a chave é o texto da linha de `improvement_table` (`STATNA`, `TELEP`, ...). Texto: nome, descrição (LNG 1076 + linha) e "Cost %1 (Available %2)" (LNG 137) ou, com outras marcadas, LNG 2704 com o total marcado; valores arredondados ao milhar `(x+500)/1000*1000`.
- Purchase compra as marcadas **e** a selecionada (o original soma o custo da selecionada duas vezes no teste de caixa se ela também estiver marcada; o clone não repete). Sem caixa: LNG 345. Lacuna: as linhas de tamanho (Medium/Large) aparecem no original; o clone não as lista porque `rt2-sim` ainda não troca o tamanho da estação.

**D2 Construir estação** (`bpaddstn.win`; `0x4941c0` caso 1, `0x4931b0`, `0x48d900` caso 0xe) [V visual]:
- Painel 458x166 (`bpaddstn.imb` 0, com as células de estilo e de direção) mais a faixa `bpmisc` 2. Três cartões (5/115/225, 3, 106x159) com a faixa de fotos `bpaddstn.imb` registro = valor do estilo (Victorian 1, Colonial 2, Mission 3, Tudor 4, Kyoto 6; 5 não tem arte; Default usa o estilo da cidade mais próxima, aqui Victorian) e o texto `"%s: %s"` com LNG 154 e o preço (`$50K`). Células de estilo: 2001..2007 (Default, Victorian, Colonial em (340,4/31/57), Mission, Tudor, Kyoto em (397,...)); de direção 2030..2038 numa grade 3x3 (N no meio de cima, NE, E, SE, S, SW, W, NW, Auto no centro). A célula escolhida ganha contorno.
- **Cores**: o círculo de captação é **dourado** (manual C209: "gold circle"; raios 2/4/8). As cores verde/amarelo/vermelho são do **fantasma da estação** (manual C405, `0x48d900` caso 0xe): `station_placement_valid(..., &flag)` inválido = vermelho (modo 3, som 0x16); válido com `flag == 0` = verde (modo 1); válido com `flag != 0` = amarelo (modo 2). O clone aproxima `flag` por "há trilho no tile-âncora ou num dos 8 vizinhos" porque `rt2-sim` não expõe a saída; o fantasma é a pegada colorida.

**D4/C3 Indústria**: clicar numa indústria (mapa ou lista) abre a caixa de águia `eagled.win` (`bldinfo.rs`) com Buy/Sell onde o modelo permite; a janela chata `Dialog::Industry` foi removida.

Capturas: `rrt2 --screenshot out.png --screen dialog --page train-buy|station-buy|station-buy-sel|industry SAVE.GAM`, `--screen game --tool station[:medium|:large] --hover X,Y`; as de `~/.cache/rrt2/shots/buy/` (1080p, 4K, 1024x768) e `compare-e1-manual-vs-clone.png`.

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

- Instalação: `--game-dir`, `RT2_DIR` ou `game_dir` em `~/.config/rrt2/config.toml`; `rrt2 set-path <pasta>` valida (`data/RT2.PK2`, `RT2_PLAT.EXE` e `default2.lng`) e grava. Sem nada configurado, ou com a pasta configurada sumida, abre a tela de pasta do jogo (próxima seção). Um `--game-dir`/`RT2_DIR` explícito e inválido ainda sai com erro. A instalação é só leitura.
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

## Tela de pasta do jogo (2026-10-23)

Aberta sozinha quando não há pasta válida (primeira execução, ou a configurada foi movida) e, depois, pelo botão "Pasta do jogo..." do rodapé das Opções (em qualquer aba; por cima do jogo, com Voltar/Esc devolvendo ao jogo). Código: `gamedir.rs` (tela), `install_detect.rs` (detecção), `logo.rs` (logo), integração em `app.rs` (`Boot::Setup`, `Effect::ChooseGameFolder`).

- **Não depende dos assets do original**, que ainda não existem nessa hora: usa só as fontes vendorizadas e `docs/assets/logo.svg`, embutido no binário e rasterizado (`resvg` 0.48.1 com `text`, `tiny-skia`) no tamanho em pixels em que é desenhado (logo lógico de até 150 unidades x escala da interface), então fica nítido em 1080p e 4K; muda a escala, o skin é refeito. A tela tem um skin plano próprio (`Theme` em tons de aço, latão e creme, a paleta do logo) e um `UiRenderer` separado, mantido ao lado do renderer do jogo (`Gfx::draw_setup`).
- **Detecção** (`install_detect::detect(&Roots)`, só leitura, em ordem determinística): a pasta configurada (sempre, primeiro, mesmo quebrada), bibliotecas do Steam (`steamapps/libraryfolders.vdf` e `config/libraryfolders.vdf`, formato novo e antigo, caminhos Linux e Windows; `appmanifest_*.acf` para o `installdir`; `steamapps/common`; prefixos Proton em `compatdata/*/pfx/drive_c`; Steam do Flatpak e do `Program Files (x86)`), GOG (registro do Windows `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`, lido com `reg query`; `GOG Games`), Heroic (`gog_store/installed.json`, `~/Games/Heroic`), Lutris (`games/*.yml`: `exe:` e `prefix:`; `~/Games`), prefixos do Wine (`~/.wine`, wineprefixes, Bottles, PlayOnLinux, `$WINEPREFIX`) e as pastas comuns do Windows (`Program Files`, `Program Files (x86)`, `GOG Games`, `Games`, em todas as unidades `A:`..`Z:`). Uma pasta conta quando o nome parece o jogo ("Railroad Tycoon", "RT2") ou, para executável do Lutris e registro do GOG, quando é uma instalação válida. `Roots::system` descreve a máquina; os testes montam árvores falsas em diretório temporário.
- **Validação** (`paths::missing_files`): `data/RT2.PK2` (gráficos e terreno), `RT2_PLAT.EXE` (tabelas e veículos lidos do executável) e `default2.lng` (todos os textos). A tela diz quais faltam ("Instalação inutilizável. Faltam: ..."). Os demais arquivos (`maps/`, `games/`, `anim/`, outros PK2) são opcionais: sem eles só as listas ficam vazias.
- **Lista clicável** com a origem de cada achado ("Steam", "GOG", "Wine prefix"...); a primeira instalação utilizável vem selecionada; um segundo clique ou Enter na linha selecionada joga. **"Escolher pasta..."** abre o seletor nativo (`rfd` 0.17.2, no Linux o portal `org.freedesktop.portal.FileChooser` sem GTK, o que também vale dentro do Flatpak), numa thread própria; a pasta escolhida entra no topo da lista e é validada (se a escolhida só contém a instalação num filho, como `GOG Games`, desce para ele). **Jogar** só habilita com pasta válida: grava `game_dir` no mesmo `config.toml` do `set-path` (os outros ajustes do arquivo ficam) e segue o fluxo normal (carrega os assets, menu e vídeo de abertura). Com a tela aberta pelas Opções e a mesma pasta, só fecha; com outra pasta, recarrega o jogo.
- Teclas: Cima/Baixo/PgUp/PgDn/Home/End na lista, Enter joga (ou aciona o botão focado), Tab/Shift+Tab entre os botões, Esc sai (primeira execução) ou volta ao jogo. Textos novos na tabela `[app]` (ids 100578..100601), nos seis idiomas; o idioma é o do `config.toml`, senão o do sistema.
- Screenshot sem instalação: `rrt2 --screenshot out.png --screen setup [--size 3840x2160 --ui-scale 200]` (mostra as detecções desta máquina).
- Testes: `gamedir_tests.rs` (eventos sintéticos de cursor, botão e teclado sobre o layout: lista com pasta falsa válida, seleção, mensagem de arquivos faltando, Jogar grava a config, seletor, layout de 640x480 a 4K), `install_detect.rs` (árvore falsa de Steam, GOG, Wine, Lutris, Heroic, Proton).

## Opções e saves (2026-10-23)

Código: `rt2-game/src/options.rs` (modelo: abas, linhas, faixas, como uma mudança edita o `Config`; sem UI e sem I/O), `keys.rs` (`KeyAction`, conflitos, `rebind`/`reset`), `saves.rs` (arquivos, metadados, rotação do autosave; I/O e relógio de parede ficam aqui, nunca no `rt2-sim`), `state/options_ui.rs` e `state/saves_ui.rs` (os painéis). Os dois painéis são *overlays* do `AppState` (`Overlay`), desenhados por cima do menu ou do jogo, seguram a simulação e recebem todas as teclas enquanto abertos. Textos novos: tabela `[app]` (ids 100238..100328; tradução pt-BR; os outros idiomas caem no inglês).

**Tela de Opções** (botão "Options" no canto superior direito de todas as páginas do menu, botão Options do File Options, `F10`). Cada mudança vale na hora e é gravada em `~/.config/rrt2/config.toml` (`$RRT2_CONFIG`). Seis abas (`←`/`→` trocam de aba):

| Aba | Itens | Chave no `config.toml` |
|---|---|---|
| Display | modo da janela (janela / tela cheia sem bordas), tamanho da janela (1024x768 a 3840x2160, mais o valor do arquivo), monitor, vsync, escala da interface automática (DPI do monitor) ou 100 a 300 % em passos de 25 | `[display] mode, width, height, monitor, vsync`; `ui_scale` |
| Audio | volume geral, música, efeitos (0 a 100, passo 5), mudo | `[audio] master, music, sfx, mute` |
| Language | um botão por idioma (troca na hora, relayout completo) | `language` |
| Keys | lista de ações com as teclas; Rebind (substitui), Add key (alternativa), Reset this key, Reset all keys | `[keys]` |
| Gameplay | velocidade ao iniciar (0 = a do save), autosave a cada N meses de jogo (0 = desligado), quantos autosaves manter, rolagem na borda da janela, velocidade da câmera (25 a 300 %), inverter arrasto do mouse, inverter zoom da roda, velocidade do zoom | `[gameplay] default_speed, autosave_months, autosave_keep, edge_scroll, pan_speed, invert_drag, invert_zoom, zoom_speed` |
| Notices | mostrar ou ocultar o aviso de cada categoria de evento (cenário e economia, finanças e ações, cidades e indústrias, concorrentes); padrão tudo ligado, como no original | `[notices] scenario, finance, towns, competitors` |

- **Controles**: botões Toggle (ligar/desligar), Toggle como botão de rádio (escolhas), e "sliders" feitos de `Kind::Cells` (clique ou arraste na barra) com botões `-`/`+`. Cada aba tem "Reset this tab"; a de teclas restaura todas as teclas.
- **Remapear teclas**: selecione a ação, Rebind (ou Enter) e pressione a nova tecla (`ctrl+` conta como parte da tecla; `Esc` cancela; modificadores sozinhos são ignorados). Se outra ação já usa a tecla, nada muda e a tela avisa quais; pressionar a mesma tecla **de novo** tira-a da outra ação e passa para esta. Tecla = par (ctrl, `KeyCode`), então `q` e `KeyQ` são a mesma tecla e `Equal` difere de `ctrl+Equal`. Reset de uma ação devolve também a tecla padrão que outra ação tenha tomado. Os padrões nunca colidem entre si (teste).
- **Janela**: o laço da janela recebe `Effect::ApplyDisplay` (modo, tamanho, monitor, `Fifo` ou `AutoNoVsync`) e `Effect::SaveConfig` (só o último de cada lote vale). F11 também passa pelo config. A janela abre com o tamanho e o modo gravados. O tamanho da janela não vale em tela cheia sem bordas (usa o do monitor).
- **Gameplay**: nenhuma opção muda regras. A velocidade inicial vai pelo mesmo `Command::SetSpeed(Absolute)` dos botões do HUD. Rolagem na borda: cursor a até 4 px da borda e fora de qualquer widget (a barra do HUD cobre as bordas esquerda e inferior no jogo; em tela cheia o topo e a direita funcionam sempre).
- Limite conhecido: em escala lógica muito pequena (300 % em 1080p) as linhas encolhem até 26 unidades e a aba Gameplay (8 linhas) pode passar do painel.

**Saves próprios** (`~/.local/share/rrt2/saves`, `$RRT2_SAVES`; nunca dentro da instalação):

- Um save é o `.GAM` do original (`World::to_save().to_bytes()`) mais dois arquivos de mesmo nome: `<nome>.toml` (versão, tipo manual/quick/auto, cenário, empresa, data do jogo, caixa, hora da gravação em tempo Unix) e `<nome>.png` (miniatura = as cores do radar de terreno, RGB 8 bits, uma célula por pixel). Os dois são opcionais: um `.GAM` sem eles lista e carrega do mesmo jeito. Gravação atômica (arquivo temporário e `rename`). Nomes digitados passam por `sanitize_stem` (sem `/`, `\`, `:` etc., até 60 caracteres).
- **Save Game** (File Options > Save, `ctrl+S`): lista dos saves próprios, campo de nome (digite; Backspace apaga; clicar numa linha copia o nome), botão vira "Overwrite" se o nome já existe, "Delete" com confirmação em dois toques, caixa de detalhe com o jogo atual. **Load Game** (File Options > Load, `ctrl+L`, menu Single Player > Load Scenario): saves próprios (mais novos primeiro, marcados `(auto)` e `(quick)`) e depois os `.GAM` originais de `games/` (`(original)`); a caixa mostra miniatura, cenário, empresa, data do jogo, caixa e hora; sem sidecar, lê o mapa do arquivo para a miniatura e o título. Duplo clique ou Enter carrega. Um save próprio continua de onde estava (sem repetir o briefing); um original abre o briefing como o Load Game do menu.
- **Quick save/load**: `F5` grava `quicksave` (toast "Quick saved"), `F9` carrega (ou avisa que não há).
- **Autosave**: a cada `autosave_months` meses de jogo (o contador parte da data de início ou do último autosave; carregar uma data anterior reinicia) grava `autosave-NNNN` e apaga os mais antigos além de `autosave_keep` (sempre fica ao menos um). Só o laço da janela liga a gravação (`AppState::set_persistence`), então testes e screenshots nunca escrevem nas pastas do usuário. Não grava com um Choice pendente nem depois do fim do jogo.
- A hora dos saves é gravada como tempo Unix (UTC) no sidecar e mostrada no fuso local do sistema (`jiff`, `saves::format_time`); a data de jogo é a do calendário do jogo.

**Mensagens Choice** (resolvido): o laço da janela liga `World::set_interactive_choices(true)`. Com um `World::pending_choice()` o `poll_world` abre o diálogo com os rótulos 1 e 2 da regra por cima de tudo (a simulação espera); o botão envia `Command::AnswerChoice { message_id, button }`; fechar o diálogo de outro jeito faz o `poll_world` reabri-lo. A mensagem repetida pelo passo reexecutado só vira linha da Activity Box (não abre outro diálogo). Sem o modo interativo (testes, screenshots, `rt2-sim` puro) vale o botão 0 de imediato, e `take_choice_answers()` só devolve respostas que o mundo não aceitou (diálogos de exemplo).

Screenshots: `rrt2 --screenshot out.png --screen options --page display|audio|language|keys|gameplay|keys-rebind|keys-conflict [arquivo.GAM]` (sem arquivo, sobre o menu; com arquivo, sobre o jogo) e `--screen saves --page save|load [--save-as NOME]` (`--save-as` grava um save em `$RRT2_SAVES` antes, para ter o que listar). Capturas em `~/.cache/rrt2/shots/options/`.

Lacunas: sem controle (gamepad) nem Steam Deck; sem as opções de gráficos do Control Panel original (detalhe, sombras, texturas: a renderização do clone não tem esses níveis); sem a aba "General" do original (profundidade de cor, uso de memória, vídeo de abertura, linhas de grade); acessibilidade (daltonismo, tamanho de fonte) além da escala da interface ainda não existe.

## Histórico de eventos e notificações (2026-10-23)

Recurso do clone, sem equivalente no original (que mostra todas as mensagens e não guarda nada além da caixa de atividade).

- **Fonte**: nenhuma nova. Cada entrada é uma linha que a caixa de atividade já recebia: `World::take_messages()` no `poll_world` (`GameMessage`, categoria por variante em `notices.rs`) e as ações do próprio jogador (empresa fundada, relatório anual, operações de ações e títulos). `AppState::add_event` grava a linha em `GameState::news` (até 500 entradas, mais nova primeiro) com data do jogo, categoria, texto completo e, quando existe, a posição (tile): lugar de `FirstOfType` e `TownBonusOffer`, partida de `CompanyConnected`. A caixa de atividade continua mostrando só as 3 (ou 50) últimas linhas **visíveis**.
- **Categorias**: Scenario and economy (regras do cenário, avisos históricos, estado da economia), Finance and stocks (desdobramento, margem, dividendo, ações e títulos do jogador), Towns and industries (primeiro prédio de cada tipo, bônus de cidade), Competitors (início, conexão, fusão, eleição, falência de empresas). **Lacuna**: não há categoria de trens/chegadas porque `rt2-sim` não emite mensagem de trem (quebra, chegada, atraso); quando emitir, basta uma variante em `Category` e a linha em `Category::of`.
- **Janela** (`state/history.rs`, `Dialog::History`): botão History na caixa de atividade (ao lado de Briefing e Close; `Message history`), moldura de painel com a fonte e os botões das outras telas. Linha de filtros (All e uma por categoria), lista com `data [categoria] texto` (mais nova primeiro, roda do mouse e setas rolam), painel de detalhe com a mensagem inteira e botões Show on map / Close. Clicar numa linha a seleciona e, se o evento tem posição, centraliza a câmera ali (a janela fica aberta, o mapa se vê ao fechar); Show on map, Enter ou segundo clique fecha a janela na posição; Esc fecha. A simulação fica parada enquanto a janela está aberta, como em qualquer diálogo. Screenshot: `rrt2 --screen dialog --page history`.
- **Notificações**: aba Notices das Opções, um liga/desliga por categoria (`[notices]` no `config.toml`, padrão tudo ligado = comportamento original). Categoria desligada: a mensagem não vira linha na caixa de atividade e não abre diálogo nem jornal; a entrada continua no histórico. Nunca suprimidos: o fim do jogo, a escolha (Choice) que a simulação espera e as respostas às ações do próprio jogador (toast). **Som**: nenhuma mensagem tem som associado hoje (a tabela de `docs/spec/sound.md` não tem cue de mensagem e o clone só toca os de interface), então não há opção de som por categoria.
- **Persistência**: o histórico vive só na memória da sessão e não entra no save: o formato é o do original e não há lugar natural para ele; carregar um jogo recomeça o histórico (com a linha "Game started"). Só as opções de notificação persistem (`config.toml`).
- **Testes** (`state/history_tests.rs`, eventos sintéticos de mouse e teclado sobre a UI montada): abrir o histórico pelo botão da caixa de atividade, filtrar, clicar numa entrada e ver a câmera ir ao tile, Show on map, Esc; desligar uma categoria pela aba Notices (clique no toggle), ver o `config.toml` pedido para gravar, e o briefing sem diálogo nem linha mas presente no histórico.

## Relatórios e gráficos (2026-10-23)

Recurso do clone sobre o original, sem mexer em regra nem em balanceamento. Abre pelo botão **Reports** da linha de seções do livro da empresa (Overview, Finances, Territories, Companies; `state/companybook.rs`), em `Dialog::Reports` (`state/reports_ui.rs`), com a moldura de painel do histórico de eventos. Três abas (teclas 1..3):

- **Charts**: gráfico de linhas ao longo do tempo (eixo em anos de jogo, mês como fração de 1/12) com doze grandezas em dois renques de botões: Revenue, Expenses, Profit, Cash, Debt, Equity (LNG 984, 35, 36, 974, 979, 980), Share Price, Book/Revenue/Earnings/Dividend per share e 5 Year Weighted Return (LNG 1005..1010). Uma linha por empresa existente (a do jogador mais grossa); a legenda à direita liga e desliga cada empresa (o texto do original, LNG 421, serve de dica) e mostra o valor mais recente ou o do cursor. Clicar ou arrastar no gráfico põe o cursor na amostra mais próxima e a legenda passa a mostrar os valores daquela data (Left/Right movem o cursor).
- **Income by category** (`Info::company_reports`): a demonstração de resultados por categoria (as 13 do livro, com os totais Revenue, Expenses e Profit) com as colunas Lifetime, This year e os cinco últimos anos do arquivo; as setas (botões ou Left/Right) trocam de empresa.
- **Rankings**: trens por resultado este ano / ano passado / vida inteira e estações por lotes movidos. Clicar numa linha a seleciona e centra a câmera no trem ou na estação; um segundo clique, Enter ou o botão Open fecha o relatório e abre a janela que já existe (trem do jogador: o painel editável `TrainEdit`; trem de outra empresa: a página `TrainDetail`; estação própria: a janela da estação, de outra empresa: a página `StationDetail`). Page up/down, PgUp/PgDn, setas e a roda rolam.
- **Export CSV**: grava o que a aba mostra em `<diretório de dados>/rrt2/reports/rrt2-<relatório>-<ano>-<mês>.csv` (`~/.local/share/rrt2/reports`, ou `$RRT2_REPORTS`) e mostra o caminho (ou o erro) no rodapé da janela e num aviso. Gráfico: `year,month,company,metric,value,year_to_date`, só as empresas visíveis; income: `company,category,lifetime,year_to_date,<anos>`; rankings: `rank,train|station,...`. Os cabeçalhos do CSV são sempre em inglês.

De onde vêm as séries (`reports.rs`, `info/reports.rs`):

| Série | Fonte |
|---|---|
| Revenue, Expenses, Profit | as 13 categorias do arquivo anual `+0x42ee` (`finance.md` 1 e 16.2; Revenue = categorias 0..4, Expenses = 5..12 mostradas como custo positivo) mais o ano corrente `+0x4286` como último ponto (marcador vazado). Com `year >= founded`, até 10 anos. |
| Share Price, Book, Revenue, Earnings e Dividend per share, 5 Year Weighted Return | métricas arquivadas `+0x49b6` (`finance.md` 16.2: 0..5) mais o valor vivo (cotação, patrimônio/ação, receita/ação, lucro/ação, dividendo); o retorno ponderado só existe no arquivo (`0x43abc0` não está no `rt2-sim`). |
| Cash, Debt, Equity e o preço entre os anos | **o save não guarda histórico**: o jogo amostra todas as empresas uma vez por mês de jogo (`AppState::sample_reports`, também durante `advance_days`), em memória; não entram no save (como o histórico de eventos) e carregar um jogo recomeça a série. Com menos de duas amostras o gráfico avisa. |
| Trens | resultado = soma das quatro categorias do razão do trem (`trains.md` 6.2: `+0xfd` este ano, `+0x10d` ano passado, `+0x13d` acumulado desde a compra; `train_year_step` desloca os anos e não toca no acumulado). |
| Estações | `+0x4b7` lotes despachados + `+0x4b9` recebidos no ano (`finance.md` 13.4), e `+0x4b5` lotes gerados para a estação. |

Cores e formas: paleta Okabe-Ito (distinguível nas deficiências de cor comuns) e, além da cor, um marcador (círculo, quadrado, triângulo, losango, X, +) e um tracejado (cheio, traços, pontos) por empresa; as combinações só se repetem a cada 24 empresas. O gráfico é um raster de células (`Kind::Cells`, 400x250), desenhado por `reports::rasterize` sem relógio nem iteração sobre `HashMap`.

**Lacunas e limites**:

- O `rt2-sim` não guarda lucro por trem fora do razão do trem, nem receita por tipo de carga: a divisão da demonstração é a das 13 categorias do original (passageiros/correio e carga num só `Freight`, categorias `0`/`1`). Receita por tipo de carga ficaria para uma mudança no `rt2-sim`.
- As faixas do razão do trem (`+0xfd..+0x14d`) do `Example save game.GAM` trazem bytes não inicializados (o padrão de heap de depuração `0xF00DBAAD` e palavras soltas nas faixas dos anos antigos e no acumulado), que a simulação só soma. Um resultado com alguma categoria fora do verossímil (acima de US$ 5 milhões num ano ou US$ 100 milhões desde a compra) aparece como `-` e vai para o fim da lista, em vez de bilhões. Num jogo novo (trens comprados no clone) os campos são zerados na compra e os três períodos aparecem.
- G15 do original (Bolsa: Corporate, gráficos de 5 anos de preço, valor contábil, receita, lucro, dividendo e retorno ponderado): os mesmos dados estão na aba Charts (as seis séries por ação e as das concorrentes), mas **sem** a arte do `stockdtl.win`, porque a tela Bolsa inteira ainda é tabular (não há a prancheta do original).

**Testes** (`state/reports_tests.rs`, `reports.rs`, `info/reports.rs`; eventos sintéticos de mouse e teclado sobre a UI montada, com o `Example save game.GAM`): abrir pelo livro da empresa, trocar de aba e de grandeza pelos botões, esconder uma empresa na legenda (o raster muda), clicar no gráfico e mover o cursor pelas setas, trocar de empresa na demonstração, clicar numa linha de trem e ver a câmera ir ao tile, segundo clique abrindo `TrainEdit`/`TrainDetail`, a estação pelo botão Open, teclas (1..3, setas, Enter, PgUp/PgDn, roda), exportar cada aba para um diretório temporário e conferir o conteúdo, erro de diretório; amostragem mensal durante `advance_days`; unitários da escala dos eixos, das séries, do raster (marcadores, cursor, vazado) e do CSV.

Screenshots: `~/.cache/rrt2/shots/reports/` (`rrt2 --screen dialog --page reports[-aba[-N]] --days 400`; `N` = grandeza 0..11 nos gráficos ou ranking 0..3).

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

## Editor de mapas e cenários (2026-10-23)

Código: `rt2-sim/src/editor.rs` (regras das ferramentas, portes das rotinas do clique `0x48eb30`), `rt2-sim/src/tick/editor.rs` (blends do terreno, prédios, escavadeira, recontagens), `rt2-sim/src/scenario/edit.rs` (validação da condição, montagem do registro de regra), `rt2-game/src/state/editor_ui.rs` (janela do editor, ferramentas, painel e sobreposição) e `editor_modals.rs` (diálogos). Testes: `rt2-sim/tests/editor.rs`, `rt2-formats/tests/map_roundtrip.rs`, `state/editor_tests.rs`.

- **Entrada**: menu principal, Editor, New Map ou Load Map (`Page::Editor`). O editor é a janela principal com a simulação parada (`World` nunca avança; `update` só atualiza câmera e reconstrói a cena em ritmo limitado).
- **Painel inferior**: grupos de ferramentas à esquerda (Select, Height, Paint, Fill, Trees, Territories, Add, Bulldoze, Overview), paleta do grupo no meio (as janelas `bp*.win`: tamanho do pincel `1..5`, classes de terreno, misturas, tipos de árvore, lista de territórios, lista de prédios e os 6 comandos especiais), radar e botões (Undo, Map Options, Rules, Save, Save As, Open, New Map, Quit) à direita. Com a arte da instalação o painel mostra o fundo `bp*.imb` do grupo (controle 2000 do `.win`, 458x166) sobre a faixa `bpmisc` (458x50), escalado por igual à área do meio (`state/editor_panel.rs`); as `Bar` do original viram `Kind::Hotspot` (áreas de clique invisíveis) nas coordenadas de projeto, e as miniaturas de terreno, que o original desenha em tempo de execução, são quadrados de cor com o nome. Sem a arte: botões do tema do clone.
- **Ferramentas** (todas chamam o porte da rotina original): Height arrasta (`0x426460`), alisa/nivela com Shift ou Ctrl (`0x426430`), baixa ao nível 0 (`0x426410`); Paint sólida (`0x42a3e0` + `0x426d30`) e mista (`0x48cc00`); Fill adjacente (`0x490490`) ou tudo (`0x490bd0`) com Match/Above/Below e água que nivela os cantos; Trees `0x429fc0` com os 8 tipos; Territories (`0x42c320`) com New/Edit e a janela Territory Setup; Add: 98 prédios (comando `0x17` com dono 0, campos e elos de estação), City/Region/Label (janela de lugar, Cancel remove), River `0x429ea0`, Reserve Cells `0x428630`, Road `0x429e30`; Bulldoze (`0x0a` com dono 0, demole prédios); Overview colore o radar por território. Cada clique empilha uma cópia do mapa para Undo (12 passos, Ctrl+Z).
- **Map Options**: nome, texto de briefing, ano inicial, caixa inicial das pessoas, quatro metas (bronze, prata, ouro, derrota; cada uma é uma regra com o efeito `Game Over` 1..4), as 22 restrições (`0x5706a9+i`), modelos financeiro e industrial (`+0x32` do bloco `0x574840`) e conjunto de indústrias (`0x5704ac`). Quatorze abas (`OPT_TABS`): General, Victory Conditions, Players (lista, Add/Delete, persona anterior/próxima, obrigatório/opcional, humano/computador/qualquer, trocar persona, caixa por jogador; o sumário do cabeçalho `0x46b890` é refeito a cada mudança e ao salvar), Player Pool (40 personas, presets América/Europa/Mundo), Managers (40), Economy, Engines (66, presets), Industries, Restrictions, Ports (4 linhas: oferta anual ou manufaturada, cargo ofertado e demandado, quantidade 1..7), Robbers (nível geral 0..401, 4 ladrões com nome e ano), Special Restrictions (10 caixas e o deslocamento de início em dias), Regional (custos de vapor, diesel e elétrico e tráfego de passageiros) e Computer A.I. (expansão de trilho e dividendos). Byte maps em `docs/formats/map-gam.md` seção 10; código `rt2_sim::editor_opts` e `state/editor_options_ui.rs`.
- **Rules**: lista de regras (adicionar, copiar, apagar, subir, descer), gatilho (8), condição validada ao vivo pelo compilador (`scenario::edit::validate`, erros com os textos do original), flags de laço derivadas como `rule_set_condition` (`0x457a30`), tipo de mensagem, textos, e os efeitos das linhas nomeadas da tabela de 350 com faixa `min..max`, valor `-100` = valor do gatilho, permanente/temporário e aviso de escopo (`0x458050`). Uma regra que o jogador não toca volta byte a byte (`edit::build` só troca os campos que o editor modela: 3164 de 3191 regras dos 95 `.MP2` idênticas; o resto são as de condição malformada).
- **Salvar**: `.MP2` (layout `0x2348`, como o gravador do original) em `<saves>/maps/`; a visão geral 170x170 é gerada como `0x477040` (idêntica nos `.MP2` salvos pelo editor original) e o checksum é refeito. Nunca grava dentro da instalação. Um `.MAP` antigo é convertido na carga e salvo como `.MP2`. Carregar e salvar um `.MP2` sem editar dá o mesmo arquivo, salvo as contagens de jogadores do cabeçalho e o checksum (95 de 95).
- **Screenshots**: `rrt2 --screenshot out.png --screen editor [mapa] --page height|paint|fill|trees|territory|add|bulldoze|overview|select [--sub N] [--item N] [--edit-at X,Y]... [--hover X,Y] [--tool new|open|save-as|options|goals|restrictions|economy|industries|rules|place|territory]` (`--sub` na lista Add é a linha: 0..5 especiais, 6.. prédios).
- **From Image** (2026-10-23): New Map tem os botões From Scratch e From Image (arquivo PCX e escala de altura 1..2000, padrão 100; `rt2_sim::editor_pcx`, `0x4694f0`/`0x469bc0`/`0x469670`): alturas por interpolação bilinear da luminosidade, mapa repintado com a classe 6. **Remover território**: botão Remove Territory no diálogo Territory Setup (extensão do clone; o original não remove).
- **Special Restrictions, Regional, Computer A.I. e lista de indústrias** (2026-10-23): três abas novas em Map Options (14 no total) e a aba Industries ganha a lista dos 48 tipos com a caixa "Allow this building type for this map" (byte maps em `docs/formats/map-gam.md` 10.8..10.11; `rt2_sim::editor_opts`, `state/editor_options_ui.rs`). A página Robbers lia o campo errado (o nível de assaltos é `state+0x38b6`, 0..401; o campo do header `+0x8970` é o "Start offset days" de Special Restrictions); corrigido.
- **Overview do editor**: o painel usa o fundo de 4 botões (registro 8 de `bpovervw.imb`, como o original em modo editor, `0x492e40` caso 0xb) e as barras laterais `2008..2011` escolhem o modo da sobreposição (`0x494f40`, `0x48f520(0xb, 1, modo)`; dicas LNG 1936..1939: Resource Map, Station Statuses, Track Grade, Traffic Density; o quinto, Track by Company, só existe no jogo). Estado em `EditorState.sub`.
- **Lacunas**: os modos 1..3 da sobreposição ainda não desenham a camada (compartilham o código do Map Overview do jogo, tela C6 de `screens.md`; o radar do editor segue colorindo por território), a lista de recursos do modo Resource Map (`0xe15`, botões All/Clear `0xe13`/`0xe14`), confirmação da escala de altura contra o original no Wine (o PCX de exemplo não existe na instalação), validação no Wine das páginas de opções (os byte maps vêm da leitura do código e dos 95 `.MP2`). O faixa 0..401 do slider de assaltos vem do campo máximo do controle (`0x191`) e não foi conferida no Wine.

## Ajustes de interface (2026-10-23, segunda passada)

- **Nomes no mapa**: como no original, só cidades e regiões têm nome (nada de legenda nas estações, `maplabels::SHOW_STATION_NAMES = false`, sobreposição moderna desligada). `place_labels` recebe os retângulos que o HUD cobre (painel inferior, os 8 ícones à esquerda, girar/zoom e velocidade; `mainwin::hud_blockers`, a mesma geometria de `build_mainwin`) e descarta o nome que encostaria neles, em vez de deixá-lo meio escondido ("CAMBRI..."). Testes em `state/maplabels.rs`.
- **Opções em escala alta**: uma aba de configurações com mais linhas do que cabem (altura mínima da linha 26 unidades lógicas) mostra uma janela de linhas, com botões ^ / v, contador `a-b/n` e a roda do mouse; trocar de aba volta ao topo (`options_ui.rs`, `row_window`). Teste de entrada real em `state/options_tests.rs` (300% numa janela curta).
- **Horário dos saves**: no fuso local (`saves::format_time`, crate `jiff` com `tz-system`); o `.toml` continua com o tempo Unix.

## Livro da empresa, painel do trem e campanha (grupo B, 2026-10-23)

- **Livro da empresa** (`state/companybook.rs`, `info/groupb.rs`): uma linha de seções sob as abas (Visão geral, Finanças, Territórios, Companhias). Visão geral: retrato do chairman (`brplyr`), chairman e gerente com salários, bônus, números da operação; botões do gerente, renúncia e Change Company Name/Logo. Territórios: três visões do mapa (política, direitos de acesso, goodwill) misturadas com o terreno do radar, tabela por território; clique no mapa ou na linha abre a janela de direitos. Companhias: lista de tela cheia (`compnlst.win`).
- **Name/Logo** (`state/dialogs/brand.rs`): digitação do nome e setas do logotipo (`artwork/brand.rs` carrega `brlogo`/`brplyr` 00..40 no skin). O comando `0x27` falta no `rt2-sim`: OK só avisa.
- **Painel do trem** (`state/train_panel.rs`): medidores de óleo, areia, água e acelerador; botões de próxima parada, espera, modo de venda, classe e a Car list (`Dialog::CarList`, comando `0x1c`). O acelerador é só leitura (falta `0x29`).
- **Fita de cotações** (`state/ticker.rs`) na tela Mercado: avança com o relógio de parede, sem tocar na simulação.
- **Campanha**: medalhas por cenário em `campaign-medals.txt` (`campaign.rs`, `state/campaign.rs`), gravadas no fim do jogo de campanha e mostradas no quadro.
- Screenshots de conferência: `rrt2 --screen info --info overview|territories|territories-access|territories-goodwill|companies` e `--screen dialog --page train-edit|car-list|brand`.
- Testes de entrada real em `state/groupb_tests.rs` (cliques e teclas sintéticos).

## Mapa, créditos, recordes, vídeos e caixas de informação (grupo A, 2026-10-23)

Código: `state/credits.rs`, `state/hiscores.rs` + `hiscore.rs`, `state/overview.rs` + `overview.rs`, `state/bldinfo.rs`, `state/video.rs`, estado reunido em `state/groupa.rs` (`AppState::ga`). Testes pelo caminho real de entrada (cursor, botões, roda e teclas sobre o layout): `state/groupa_tests.rs`.

- **Créditos (A8)**: página `Page::Credits` do menu; a foto `credits.imb` ao fundo e o texto de `credits2.lng` (`{[c11][u1]X}` = título, `{[c11]X}` = subtítulo) rolando sozinho, a roda e `PgUp/PgDn/Home` movem, Esc ou Cancelar voltam. As linhas são posicionadas por quadro (`credits_tick`), só as que cabem inteiras na janela são criadas.
- **Recordes (A9)**: foto `hiscore.imb` e tabela de 20 linhas (#, empresa, cenário, medalha, data, pontos; em largura menor que 720 px some o cenário e a data). A instalação não traz `hiscore.dat` e o layout dele não foi decodificado, então o clone grava o seu em `~/.local/share/rrt2/hiscore.dat` (`$RRT2_HISCORE`): texto, uma linha por vitória, ordenado por pontos; só vitórias entram (bronze, prata, ouro) e a pontuação é o caixa da empresa no fim (lacuna: a fórmula original). O fim de cenário chama `game_over_hook` (grava a linha e toca o vídeo).
- **Map Overview (C6)**: o ícone do globo (4º) troca a caixa central pelo painel das 5 camadas (LNG 1931 título, 1932..1935 All/Clear, 1936..1940 dicas, 1929 Cancel); o mapa recebe quadrados translúcidos nos tiles visíveis (como a prévia das ferramentas, no máximo 3000, recalculados quando a câmera mexe). Dados (`overview.rs`): prédios (caixa `+0x26..+0x2d`, tipo `+7`, saída por slot e entradas de `industry_type_table`), lotes esperando por estação (`+0x61 + 2c`), peças de trilho (`+2` dono, `+0x12` rampa em décimos de %, `+0x17/+0x1b` desgaste). Recalculados ao abrir, ao mudar o mapa e a cada dia. Passar o mouse numa carga a acende; clicar mantém; Esc, Cancelar ou o ícone de novo saem; escolher uma ferramenta de construção também sai. Lacunas: a medida original de densidade de tráfego, as cores das faixas e os ícones gráficos das cargas não estão nas specs/arte decodificada.
- **Caixas de informação (C3, C4, C5)**: clique esquerdo numa indústria abre a caixa de águia (o mesmo diálogo `Message`) com Ownership, Profitability (LNG 2466..2471 pela faixa `+0x4b`: <75, <100, <125, <150, <180, >=180 Gushing Cash; as faixas são do clone), Purchase Price, Supplies e Demands e os botões Buy/Sell (`Act::Trade`, só onde `industry_trade` permite) e OK. Clique direito (sem arrastar) abre a mesma caixa sem Buy; em terreno livre mostra tipo de terreno (`TERRAIN_TYPES`, água = oceano), território, direitos de acesso (LNG 2326..2329 pelos dois primeiros bits) e X,Y; o arrasto com o direito segue movendo a câmera. A dica de C5 é a linha de status ("x, y  nome") que já existia.
- **Vídeos (A10)**: `video.rs` decodifica `anim\*.SMK` (nomes sem distinguir maiúsculas) com o `ffmpeg` da linha de comando para frames RGBA 640x480 (clipes mais largos que 2:1 esticam, os outros ganham barras), que entram num sprite reservado do atlas (`DialogArt::video_frame`, `upload_sprite`); o som sai de um `ffplay -nodisp` (volume = mestre x efeitos). Em tela cheia com Esc, Espaço, Enter ou clique para pular (a entrada não chega ao resto da interface). `INTRO_01` ao abrir a janela (opção `[gameplay] intro_video`, aba Gameplay), `scenwin` / `scenlose` no fim do cenário. Só o laço de janela liga os vídeos (`set_videos`); testes e screenshots não. Sem `ffmpeg`/`ffprobe` o clipe é ignorado. Lacuna: as janelas de vídeo da Activity Box (tutorial das ferramentas) e os clipes de campanha (`cvNN`, `caNN`): o gatilho de cada um não está decodificado (`formats/sound.md` seção 4).
- **Screenshots**: `rrt2 --screenshot out.png --screen menu --page credits[@PX]|scores|video:NOME[@ms]`, `--screen game --overview resource[:all|:ID,ID]|stations|grade|traffic|company`, `--click X,Y` (esquerdo, pixel físico) e `--right-click X,Y`.
