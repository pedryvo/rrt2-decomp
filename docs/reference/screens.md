# Inventário das telas do Railroad Tycoon II Platinum (original) e estado no clone

Objetivo: saber exatamente que interface ainda falta construir. Compilado em 2026-10-22 a partir de (1) o manual da instalação, (2) as 46 janelas `.win` decodificadas (`formats/ui.md`), renderizadas com `rt2-ui`, (3) o código atual (`docs/ui.md`, `crates/rt2-game`) e (4) busca na web.

Legenda de status: ✅ pronto, 🟡 parcial, ⏳ ausente. "Extra" = existe só no clone (o original não tem).

## Fontes e como ler as referências

Capturas de tela são material com direitos autorais e **não** entram no repositório. Foram olhadas apenas numa pasta de rascunho fora dele.

| Chave | Fonte | O que mostra |
|---|---|---|
| `M:<arq>` | `Manual.html` da instalação (somente leitura), imagem `imagesm/<arq>.jpg` | capturas do próprio manual (em escala de cinza, 640x480). É a melhor referência visual: cobre menus, interface principal, painéis de construção, estação, trem, empresa, bolsa, editor |
| `SG:<arq>` | `Strategy Guide.html`, imagem `images sg/<arq>.jpg` | capturas do guia (ex. `C3_STATION`: vista 3D da estação) |
| `W:<nome>` | `rt2-ui <nome>.win` | layout decodificado (retângulos, ids), sem texto |
| R1 | https://steamcommunity.com/app/7620/screenshots/ | galeria da comunidade Steam (RT2 Platinum), capturas de jogadores da interface em jogo |
| R2 | https://www.gog.com/game/railroad_tycoon_2 | página da GOG: galeria oficial de capturas |
| R3 | https://www.mobygames.com/game/7930/railroad-tycoon-ii-platinum/ | MobyGames: ficha e galeria do Platinum (a página devolve 403 para ferramenta automática; abrir no navegador) |
| R4 | https://en.wikipedia.org/wiki/Railroad_Tycoon_II | descrição geral (bolsa, títulos, fusão, falência) |
| R5 | https://gamespot.com/reviews/railroad-tycoon-ii-review/1900-2532681/ | análise: melhorias de estação (hotel, restaurante, correio, torre d'água), cartão de referência |
| R6 | https://gamesfirst.com/articles/1999/rrtycoon2/ | análise de época com descrição das telas |

Limitação honesta: MobyGames e PCGamingWiki bloquearam o acesso automático e a busca não devolveu URLs diretas de imagem, então a identificação visual foi feita com as capturas do manual (que cobrem quase toda a interface) e com a geometria dos `.win`. Capturas de terceiros (R1, R2, R3) ficam como links para conferência manual.

## Como o original organiza a interface (resumo)

- Todas as janelas são `.win` (layout) + bancos `.imb` (arte). O texto vem do código/`default2.lng`; os `.win` só têm retângulos e ids de ação.
- `setup.win` (idêntico a `set.win`, `setupg.win`, `setupl.win`) é **uma só janela de 1024x768 que serve todas as páginas do menu**: botões principais (ids 3135..3140), submenu (campanha/cenário/jogo), lista de mapas (401), descrição (409), miniatura (412), setas de data (3237..3240), cavalos de dificuldade, opções, jogadores de computador. As "telas" do menu abaixo são estados dela.
- Janelas "livro" (`compndtl`, `playrdtl`, `yearend`) e "prancheta" (`stockdtl`) são cenas pré-renderizadas com texto por cima; as listas (`statnlst`, `trainlst`, `compnlst`, `playrlst`) são tabelas de tela cheia com a mesma barra inferior das telas de detalhe (4 abas + OK/Exit).
- Prefixo `bp` = painel inferior (a caixa central do HUD): um arquivo por ferramenta, todos com o mesmo rodapé (custo/data, botão de ação 2015, rótulo 2016).
- Há 46 nomes de `.win`; `buygold.win` e `setupl.win` não estão nos PK2 (só o nome no EXE), e `smackwi2/3/4/win` são janelas de vídeo.

## Inventário

### A. Menus principais e preparação do jogo

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| A1 | Menu principal | Seis botões: Single Player, Multi Player, Editor, High Scores, Credits, Quit; fundo com logotipo | início do programa; "Main Menu" do File Options | `setup.win` (3135..3140) | M:C201, M:C301, M:C901 | ✅ (Multi Player e Editor desativados; `state/menus.rs`) |
| A2 | Submenu Single Player | New Campaign, Load Campaign, New Scenario, Load Scenario (colunas que se abrem à direita) | Single Player | `setup.win` | M:C301 | ✅ (`state/menus.rs`) |
| A3 | Novo cenário: seleção de mapa | Lista de mapas, descrição, miniatura, seta de data inicial (quando o mapa permite), OK/Cancel | New Scenario > botão Map | `setup.win` (401, 409, 412, 3237..3240) | M:C304 | 🟡 (lista, descrição, miniatura do terreno, data; sem setas de data: flag não decodificada) |
| A4 | Novo cenário: Game Setup | Botão Map, nível de dificuldade (5 cavalos: Easy, Medium, Hard, Expert, Custom), lista de jogadores (você + computador), contador de jogadores de computador, "Difficulty Rating: N %", OK/Cancel | New Scenario | `setup.win` | M:C303, M:C304 | 🟡 (cavalos, jogadores, contador e rating; opções guardadas mas não aplicadas à simulação; fórmula do rating heurística) |
| A5 | Opções avançadas do cenário | Sandbox mode; modelo financeiro (Basic/Advanced/Expert); modelo industrial (Basic/Advanced/Expert); controles deslizantes de Revenue Modifier (humano e computador) | botão Options do Game Setup | `setup.win` | M:C305 | 🟡 (todas as opções; não aplicadas à simulação; faixa do Revenue Modifier suposta) |
| A6 | Carregar jogo / cenário salvo | Requisitor de arquivo: lista, OK/Cancel (Load Game, Load Scenario, Load Campaign) | menu ou File Options > Load | `filerq.win` (401, 405, 406, 407) | M:C202a | ✅ (Carregar jogo; sem variante de campanha) |
| A7 | Salvar jogo | Mesmo requisitor com campo de nome | File Options > Save | `filerq.win` (405 = campo de nome) | M:C419 (ícone) | 🟡 (F5 grava direto em `~/.local/share/rrt2/saves`, sem tela de nome) |
| A8 | Créditos | Texto rolante em tela cheia | Credits | `credits.win` (4501, 4502) | R3 | ⏳ |
| A9 | Recordes (High Scores) | Tabela de recordes em tela cheia | High Scores; fim de jogo | `hiscore.win` (5101, 5102) | M:C301 (botão) | ⏳ |
| A10 | Vídeos de abertura / Smacker | Quatro janelas de vídeo (abertura, tutorial das ferramentas na Activity Box) | início; ao usar ferramentas | `smackwin.win`, `smackwi2/3/4.win` | M:C205a (Activity Box) | ⏳ (baixa prioridade) |
| A11 | Seletor de idioma | Lista de idiomas | menu | nenhum | n/a | Extra ✅ |

### B. Seleção de campanha / cenário

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| B1 | Nova campanha: dificuldade | Escolha Easy/Normal/Hard (fixa durante a campanha) | New Campaign | `setup.win` | M:C301 | 🟡 (Easy/Normal/Hard e escolha da campanha; dificuldade não aplicada) |
| B2 | Quadro da campanha (estação de trem rural) | Lista dos 18 cenários da campanha, retrato do chairman, bônus escolhíveis (ex. dinheiro, desconto de trilho, locomotiva antecipada), "Play this Scenario", mapa-miniatura, Exit | New/Load Campaign, entre cenários | `campaign.win` (4700.., barras 80h) | M:C3Ccamp | 🟡 (18 cenários, mapa, descrição, 3 bônus da campanha clássica, Play; progressão/medalhas não gravadas, bônus da 2ª campanha não decodificados) |
| B3 | Janela de objetivo ao iniciar | Pop-up com a meta do cenário (ex. patrimônio de $10 milhões até 1900) e OK | início de cenário | `eagled.win` (suposição: caixa de mensagem genérica) | M:C202a (texto do tutorial) | ✅ (briefing + condições de medalha das regras; `state/dialogs.rs`) |
| B4 | Fim de cenário: vitória/derrota e medalhas | Placa de madeira com imagem, texto do resultado e botão | meta atingida/prazo/falência | `winlose.win` (4601..4604) | R4 | 🟡 (placa pronta; falta o gancho `World::game_over()` no rt2-sim) |

### C. Interface principal (mapa)

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| C1 | Janela principal | Mapa (A); radar (B) que recentra ao clicar; caixa central de listas (C) com 4 abas (Stations, Trains, Players, Companies); Activity Box (D) com vídeos e mensagens; 4 botões girar/zoom; bússola; 8 ícones de ação à esquerda; controle de velocidade | ao iniciar o jogo | `mainwin.win` (ids 166..194, 216..219, 255..269, 322..352) | M:C205a, M:C401.call | 🟡 (8 ícones, radar clicável, caixa central, Activity Box, girar/zoom; ferramentas de construção abrem painel provisório; sem bússola, vídeos nem caixa expandida) |
| C2 | Caixa central: abas de resumo | Stations (oferta e receita From/To), Trains (classe Express/Normal/Slow/Halted, composição, velocímetro, barra de progresso), Players, Companies; duplo clique abre o detalhe | caixa central | `mainwin.win` + `bp*` | M:C401.call | 🟡 (4 abas com tabelas; sem classe/consist/velocímetro do trem; filtra todas as empresas) |
| C3 | Informação de prédio (clique esquerdo) | Dono, lucratividade, preço de compra, produção/demanda; botão Buy no modelo industrial avançado, OK | clique em indústria | `eagled.win` (suposição) | M:C402 | 🟡 (painel de indústria somente leitura, sem Buy) |
| C4 | Informação de prédio (clique direito) / terreno | Terreno, território, direitos de acesso, coordenadas X,Y | clique direito | sem `.win` identificado | M (cap. 4) | 🟡 (território por clique esquerdo, ver D) |
| C5 | Cursor e rótulo sob o mouse | Tipo de indústria sob as abas da caixa central | passar o mouse | n/a | M (cap. 4) | ⏳ |
| C6 | Mapa: sobreposições (Map Overview) | Lista de 5 camadas: Resource Map (oferta branca/demanda amarela, All/Clear), Station Statuses, Track Grade, Traffic Density, Track by Company | ícone 4 (globo) | `bpovervw.win` (13605) | M:C205a, M:C206a, M:C407 | ⏳ |
| C7 | Demolir (Bulldoze) | Seleção de tamanho do pincel e clique no objeto | ícone 3 | `bpbulldz.win` | M:C406 | ⏳ |
| C8 | Ticker / cúpula de bolsa | Fita de cotações na cúpula de vidro (decoração da bolsa) | Stock Market | `stockdtl.win` | M:C2TICKER01 | ⏳ |

### D. Construção (trilho, estação, prédios)

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| D1 | Assentar trilho | Painel: tipo (Single, Double, eletrificado), tipo de ponte (madeira, pedra, ferro), modo (livre, linha reta, peça a peça), custo e grade por tile sobre o mapa, botão Add Track/Cancel | ícone 1 | `bpaddtra.win`, `bpaddtr2.win` | M:C208a, M:C404, M:SCTracklaying | ⏳ (a simulação de construção está em curso em `rt2-sim`, sem UI) |
| D2 | Construir estação | Tamanho (Small $50K, Medium $100K, Large $200K), arquitetura (Default, Victorian, Colonial, Mission Revival, Tudor, Kyoto Revival), orientação (9 posições), círculo de captação verde/amarelo/vermelho no mapa | ícone 2 | `bpaddstn.win` (1813..1815, 2008..2047) | M:C209new, M:C405, M:C6Station | ⏳ |
| D3 | Informação da estação ao construir | Painel com oferta, demanda, From/To da estação recém-criada | logo após construir | `bpaddstn.win` / `statndtl.win` | M:C210new | ⏳ |
| D4 | Comprar prédio (indústria) | Compra de indústria no modelo Expert | clique no prédio | `eagled.win` | M:C402 | ⏳ |

### E. Trens: compra, composição e rota

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| E1 | Compra de trem | Lista de locomotivas disponíveis; custo, manutenção, combustível, aceleração, confiabilidade; matriz de velocidade máxima por rampa e vagões (peso por vagão ajustável); Purchase/Cancel | ícone 5 (locomotiva) | `trainbuy.win` (1701, 1705..1707) | M:C211a, M:C609, M:C408 | ⏳ |
| E2 | Detalhe do trem | Painel de instrumentos (óleo, areia, água, acelerador), lista rota/composição (marcador Next Station, Delete, semáforo verde/amarelo/vermelho de espera), painel de informação (receita, valor da carga, peso, idade, chance de pane, Retire/Replace), botões de abas | após comprar; duplo clique em trem | `traindtl.win` (716..746, 33001..33010) | M:C212a, M:C610 | 🟡 (lista e detalhe somente leitura: composição, paradas, peso, estado) |
| E3 | Roteador de mapa | Mapa do mundo: estrelas verdes (suas estações) e amarelas (rivais); clicar adiciona parada, arrastar reordena | alavanca no detalhe do trem | `traindtl.win` | M:C212a, M:C610 | ⏳ |
| E4 | Lista de vagões (Car List) | Composição por parada: vagões já no trem (clique remove, Clear), vagões da estação, outros; OK/Cancel | duplo clique em parada | `traindtl.win` | M:C213new, M:C612 | ⏳ |
| E5 | Lista de trens | Tabela de tela cheia (nome, rota, velocidade, ...), prioridade | caixa central / aba Trains | `trainlst.win` (1301, 1371..1386) | M:C401.call | ✅ (tecla T) |

### F. Estação (detalhe e melhorias)

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| F1 | Detalhe da estação (cena 3D) | Vista 3D pré-renderizada da estação com os prédios comprados compostos; botão Buy; painel inferior Supply / Demand (vagões ofertados, demanda com nível 0..9 no modelo Expert) / From / To (moedas) | duplo clique na estação; ao construir | `statndtl.win` (601, 602, 680, 671..676) | M:C522, SG:C3_STATION, M:C6Station | 🟡 (lista com dono, melhorias, oferta, espera, demanda, preços; falta a composição visual e o Buy) |
| F2 | Comprar melhorias | Lista de estruturas disponíveis (torre d'água, areia, rotunda, hotel, restaurante, correio, armazéns...) com caixas de seleção e Purchase/Cancel; compra uma ou várias | botão Buy | `buildbuy.win` (3301, 3305..3307; banco `buildbuy.imb`) | M (cap. 5), R5 | ⏳ |
| F3 | Lista de estações | Tabela de tela cheia | aba Stations | `statnlst.win` (1201, 1271..1286) | M:C401.call | ✅ (tecla G) |

### G. Empresa, finanças, bolsa e títulos

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| G1 | Iniciar empresa | Diálogo "Start New Company": investimento do jogador e externo (barras), nome, logotipo, OK/Cancel | primeira ação (aba Companies "<<<Start a New Company>>>") | `eagled.win` (suposição: moldura com águia) | M:C207a, M:C420 | 🟡 (sliders `finance.md` 17.2; nome e logotipo fixos; criação de empresa (cmd 0x26) ainda não existe no rt2-sim) |
| G2 | Livro da empresa: Overview | Chairman e gerente (salários), bônus do gerente, receita, lucro, caixa, dívida, milhas de trilho, velocidade média, goodwill; ações Resign as Chairman, View/Hire Managers, Change Name/Logo | aba Companies, Company Detail | `compndtl.win` (1601, 1602, 1619, 1620, 1671..1676) | M:C214, M:C724 | 🟡 (cabeçalho de finanças; sem chairman/gerente) |
| G3 | Livro: Income (demonstração de resultados) | Receitas e despesas, vida inteira e últimos 3 anos | aba Income | `compndtl.win` | M (cap. 7) | ✅ (tela Finanças, tecla F) |
| G4 | Livro: Balance (balanço) | Ativos, dívida, patrimônio | aba Balance | `compndtl.win` | M (cap. 7) | ✅ |
| G5 | Livro: Territories | Political View, Access Rights View, Goodwill por território; compra de direitos | aba Territories | `compndtl.win` | M (cap. 7) | ⏳ |
| G6 | Livro: Finances (ações) | Taxa básica, rating de crédito, preço da ação, caixa, títulos, ações; links Issue Bonds, Issue Stock, Buy Back Stock, Change Dividends, Attempt Merger, Declare Bankruptcy | aba Finances | `compndtl.win` | M:C215, M:C725 | 🟡 (indicadores só leitura; sem ações) |
| G7 | Emitir títulos | Diálogo de taxa/valor, OK | Issue Bonds | `eagled.win` (suposição) | M (cap. 7) | ⏳ |
| G8 | Emitir / recomprar ações, dividendos | Diálogos de quantidade/valor e efeito no preço | links do Finances | `eagled.win` (suposição) | M (cap. 7) | ⏳ |
| G9 | Fusão (Attempt Merger) | Escolha da empresa alvo, preço oferecido, votação de acionistas | Finances | `eagled.win` (suposição) | M (cap. 7) | ⏳ |
| G10 | Declarar falência | Confirmação e consequências | Finances | `eagled.win` (suposição) | M (cap. 7) | ⏳ |
| G11 | Gerentes: ver/contratar | Dois candidatos com bônus e salário | Overview > View/Hire | sem `.win` identificado | M (cap. 7) | ⏳ |
| G12 | Nome e logotipo da empresa | Edição do nome e seleção do logotipo | Overview | sem `.win` identificado | M:C207a | ⏳ |
| G13 | Lista de empresas | Tabela de tela cheia | aba Companies | `compnlst.win` (1401, 1471..1486) | M:C401.call | 🟡 (as empresas aparecem na Bolsa) |
| G14 | Bolsa: Personal | Prancheta: caixa, ações, total, poder de compra; tabela por ação (preço, quantidade, valor) com Sell, View, Buy; margem e venda a descoberto no modelo Expert | ícone 6 (gráfico) | `stockdtl.win` (1903..1908) | M:C216, M:C814, M:C413 | 🟡 (tabela de empresas com preço, dividendo, rating; sem comprar/vender) |
| G15 | Bolsa: Corporate | Gráficos de 5 anos: preço, valor contábil, receita, lucro, dividendo por ação, retorno ponderado | aba Corporate | `stockdtl.win` | M (cap. 8) | 🟡 (valores atuais, sem gráficos) |

### H. Jogador, chairman e portfólio

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| H1 | Detalhe do jogador | Livro com retrato e biografia do magnata, posição, riqueza (caixa, ações, total), poder de compra, estilo | aba Players, duplo clique | `playrdtl.win` (1001, 1002, 1005, 1071..1076) | M:C723 | ⏳ |
| H2 | Lista de jogadores | Tabela de tela cheia | aba Players | `playrlst.win` (1501, 1571..1586) | M:C401.call | ⏳ |
| H3 | Seleção/edição do personagem (multiplayer) | Retrato, nome, descrição (botão Change) | lobby | sem `.win` identificado | M (cap. 9) | ⏳ |

### I. Relatórios, jornais, eventos e mensagens

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| I1 | Relatório anual (Year End) | Caderno com 4 abas: resumo do ano e status (TSC) | virada de ano | `yearend.win` (34702, 34712, 34740) | M (cap. 4: "News Frequency"), M:C214 (estilo) | 🟡 (4 abas com dados do arquivo anual; abre na virada de ano) |
| I2 | Jornal (Newspaper) | Manchete com texto: mudança econômica, eventos; botão Exit | evento do tipo Newspaper | `paper.win` (4201..4208) | M (cap. 10, Message Text) | 🟡 (moldura e texto prontos; sem fonte de mensagens: `show_message` do rt2-sim é stub) |
| I3 | Evento em diálogo / com escolha | Texto e botões; no tipo Choice, duas escolhas com efeitos | evento | `eagled.win`, `helpd.win` (suposição) | M (cap. 10) | 🟡 (diálogo eagle pronto; sem escolha com efeitos nem fonte de mensagens) |
| I4 | Mensagens na Activity Box | Mensagens e vídeos do jogo na caixa D | durante o jogo | `mainwin.win` | M:C205a | 🟡 (mensagens de texto; sem vídeos) |
| I5 | Aviso de ajuda | Janela pequena com texto e 3 botões | ajuda/dicas | `helpd.win` | n/a | ⏳ |
| I6 | Dicas de jogo / tutorial | Dicas ao iniciar partida; mini-tutorial que se reinicia no Control Panel | início de jogo | sem `.win` identificado | M (cap. 4, Game Play) | ⏳ |
| I7 | Chamada de margem (margin call) | Aviso para vender ações | bolsa Expert | sem `.win` identificado | M (cap. 8) | ⏳ |

### J. Opções e preferências

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| J1 | Opções de arquivo (File Options) | Load, Save, Main Menu, Quit, Cancel | ícone 8 (disquete) | `fileopt.win` (501..507) | M:C419 | ✅ (Load, Save, New, Main Menu, Quit, Cancel; `state/dialogs.rs`) |
| J2 | Confirmação de saída | "Tem certeza?" Sim/Não | Quit | `eagled.win` (suposição) | M (cap. 4) | ⏳ |
| J3 | Control Panel: General | Velocidade do jogo, volume de som e música, profundidade de cor, uso de memória, tela cheia, linhas de grade, cursor colorido, vídeo de abertura | ícone 7 (controles) | `settings.win` (34900.., sliders) | M:C417new, M:C416 | 🟡 (velocidade/pausa no HUD; volumes em `config.toml`; sem tela) |
| J4 | Control Panel: Graphics | Detalhe geral (5 níveis) e sliders por item (sombreamento, textura do solo, trilho, trens, prédios, árvores, sombras, animação, borda) | idem | `settings.win` | M:C418new | ⏳ |
| J5 | Control Panel: Game Play | Sino de chegada, frequência de notícias, logotipos nos trens, manter o jogo em diálogos, diálogos somem sozinhos, dicas, tutorial | idem | `settings.win` | M (cap. 4) | ⏳ |
| J6 | Escala da interface, tela cheia, mudo | Atalhos do clone | teclas | n/a | n/a | Extra ✅ |

### K. Multiplayer

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| K1 | Menu multiplayer | TCP/IP, IPX, Modem, Serial | Multi Player | `setup.win` | M:C901 | ⏳ |
| K2 | Lobby: servidores | Lista de servidores, Change, Server IP, Join, Create, Refresh | escolha do protocolo | `setup.win` | M (cap. 9) | ⏳ |
| K3 | Preparação do jogo multiplayer | Igual ao Game Setup + luzes de aprovação por jogador e ping; só o anfitrião escolhe mapa | após conectar | `setup.win` | M (cap. 9) | ⏳ |
| K4 | Chat em jogo | Linha de entrada (TAB, Enter) e texto na tela | TAB | `chat.win` (4900, 4901) | M (cap. 9) | ⏳ |

Fora de escopo do clone de jogo único: K1 a K4, mas ficam registradas.

### L. Editor de cenários (menu Editor)

| # | Tela | Propósito e conteúdo | Como se chega | .win | Referências | Status |
|---|---|---|---|---|---|---|
| L1 | Menu do editor | New Map / Load Map | Editor | `setup.win` | M:C9_01 | ⏳ |
| L2 | Novo mapa: dimensões | Largura e altura (sliders), From Scratch, From Image | New Map | `setup.win` | M:C9_01 | ⏳ |
| L3 | Importar imagem e escala de altura | Caminho do PCX (cinza indexado) e Height Scale | From Image | `filerq.win` (suposição) | M (cap. 10) | ⏳ |
| L4 | Janela do editor | Mapa + caixa central + coluna direita de ferramentas (Undo, Map Options...) | entrar no editor | `mainwin.win` | M:C9_03.call | ⏳ |
| L5 | Elevar/baixar terreno | Alisar, elevar, baixar ao nível do mar; pincel | ferramenta | `bpraisea.win` (2304..2306) | M:C101_raise_lower | ⏳ |
| L6 | Pintar terreno | Paleta sólida/mista, pincel | ferramenta | `bppaintt.win` (2448..2457) | M:C102_paint_terrain | ⏳ |
| L7 | Preencher terreno | All/Adj. e Match/Above/Below | ferramenta | `bpfillte.win` (2436..2451) | M:C103_fill_terrain | ⏳ |
| L8 | Árvores | Adicionar e cortar, pincel | ferramenta | `bpaddtre.win`, `bpremtre.win` | M:C104_addtrees | ⏳ |
| L9 | Territórios | Lista e diálogo Territory Setup (nome, goodwill padrão, borda visível, direitos separados, comprável) | ferramenta | `bpterrit.win` (4301) | M:C105_territories | 🟡 (o painel "Território" do clone mostra região/tiles ao clicar no mapa, não é o editor) |
| L10 | Construir prédio / cidade / região / rótulo / rio / células reservadas | Lista de prédios e 5 comandos especiais | ferramenta | `bpaddbui.win` (2600..2605) | M:C106_build_new | ⏳ |
| L11 | Add City / Add Region | Abas General (tipo, nome, tamanho, arquitetura) e Industry (pesos 0..200) | Add City/Region | `cityset.win` (3401..3407) | M (cap. 10) | ⏳ |
| L12 | Map Options | Seções: General, Victory Conditions, Events, Players, Player Pool, Managers, Economy, Engines, Industries, Restrictions, Ports, Robbers | botão Map Options | `mapset.win` (3501..3507) | M:C9_13 | ⏳ |
| L13 | Editor de eventos | Trigger, Message Text (Dialog/Newspaper/Choice), Frequency, Effect (sliders), Comments | Map Options > Events | `eventset.win` (33801..33869; banco `buildbuy.imb`) | `editor3.txt` da instalação | ⏳ |

## Resumo e contagem

Contagem das linhas da tabela acima:

Total: 81 telas do original, 9 ✅, 25 🟡, 47 ⏳ (mais 2 extras do clone).

Telas do clone que não existem no original (extras): seletor de idioma, Cidades (lista), atalhos de escala/mudo/tela cheia. A tela "Indústrias" (lista) e "Território" são equivalentes parciais de C3/C4.

## Prioridade das telas que faltam para o jogo de um jogador

Ordem pensada para fechar o primeiro ciclo jogável (comprar empresa, construir, comprar trem, ver lucro):

1. **Janela principal completa (C1, C2)**: os 8 ícones de ação, a caixa central com as 4 abas, o radar e os botões de girar/zoom. Todas as ferramentas pendem dela.
2. **Iniciar empresa (G1)** e **janela de objetivo (B3)**: sem empresa não se constrói; usa o `eagled.win`, que serve de molde para os demais diálogos.
3. **Assentar trilho (D1)**: painel `bpaddtra/bpaddtr2` e feedback de custo e grade sobre o mapa.
4. **Construir estação (D2, D3)**: tamanhos, arquitetura, orientação, círculo de captação colorido.
5. **Compra de trem (E1)**: lista de locomotivas e matriz de velocidade.
6. **Detalhe do trem editável (E2, E3, E4)**: roteador de mapa, lista de vagões, semáforo de espera, acelerador, Retire/Replace.
7. **Detalhe da estação composto e compra de melhorias (F1, F2)**: cena com camadas por prédio (pendência já registrada em `docs/ui.md`) e `buildbuy.win`.
8. **Livro da empresa com ações financeiras (G2, G6 a G10) e bolsa operável (G14)**: títulos, emissão e recompra de ações, dividendos, fusão, falência, compra e venda de ações.
9. **Fluxo do menu de um jogador (A1 a A5, B1, B2)**: Single Player, New Scenario com dificuldade e opções, campanha com bônus. Define o modelo financeiro e industrial de cada partida.
10. **Relatório anual, jornal e eventos (I1 a I3)** e **Control Panel + File Options (J1 a J5)**, **fim de cenário (B4)**, depois demolir (C7), sobreposições do mapa (C6) e compra de indústria (C3, D4).

Em seguida (menor prioridade): jogadores e chairman (H1, H2), territórios e direitos de acesso (G5), recordes e créditos (A8, A9), vídeos (A10); multiplayer (K) e editor (L) por último.
