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

Todas SIL OFL 1.1, em `assets/fonts/` com a licença ao lado (`OFL-*.txt`). O `FontSystem` só enxerga estas quatro (sem fontes do sistema: aparência igual em qualquer máquina).

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

## Screenshots

`rrt2 --screenshot out.png [--screen menu|new-game|load-game|game] [--size 3840x2160] [--ui-scale 200] [--days N] [arquivo]` renderiza fora da tela (sem janela). O `game` usa o `Example save game.GAM` e avança N dias simulados (semente de RNG fixa `1`).

## Lacunas conhecidas

- A cena do mapa (`rt2-render::Scene`) é montada uma vez no carregamento: trens e construções criados pela simulação ainda não aparecem.
- Qual empresa é a do jogador não está decodificado; o caixa mostrado é a conta 0 da primeira empresa (`finance.md` 2).
- Início de um jogo novo (data, empresa, economia) não está especificado: um mapa de cenário abre como mundo só com relógio, em 1860.
