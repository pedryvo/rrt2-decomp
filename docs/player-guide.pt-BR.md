<div align="center">

# Railroad Tycoon II (rrt2-decomp) — Guia do Jogador

[English](player-guide.md) · **Português** · [Español](player-guide.es.md) · [Deutsch](player-guide.de.md) · [中文](player-guide.zh-CN.md) · [Русский](player-guide.ru.md)

</div>

---

Bem-vindo ao **rrt2-decomp**, uma reimplementação moderna e de código aberto do clássico de estratégia *Railroad Tycoon II Platinum* (1998), reescrita do zero em Rust. Este guia aborda tudo o que você precisa saber para jogar: instalação e configuração inicial, conceitos essenciais da ferrovia, atalhos de controle, as melhorias tecnológicas em relação ao original de 1998 e os recursos modernos opcionais.

> [!IMPORTANT]
> O **rrt2-decomp** **não contém nenhum asset proprietário**. Para jogar, é necessário possuir uma cópia legítima do *Railroad Tycoon II Platinum* (adquirida no [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum), Steam ou disco original de instalação). O motor lê os arquivos de dados, áudio, mapas e textos diretamente da sua instalação em modo somente leitura, exatamente como projetos como OpenRCT2 e OpenTTD.

---

## Índice

1. [Instalação e Execução](#1-instalação-e-execução)
   - [Requisitos de Sistema e Arquivos](#requisitos-de-sistema-e-arquivos)
   - [Detecção Automática de Instalações](#detecção-automática-de-instalações)
   - [Configuração Manual da Pasta](#configuração-manual-da-pasta)
   - [Pacotes por Sistema Operacional](#pacotes-por-sistema-operacional)
   - [Pastas de Configuração e Saves](#pastas-de-configuração-e-saves)
2. [Primeiros Passos](#2-primeiros-passos)
   - [Modos de Jogo: Campanhas, Cenários e Sandbox](#modos-de-jogo-campanhas-cenários-e-sandbox)
   - [Fundando sua Companhia Ferroviária](#fundando-sua-companhia-ferroviária)
   - [Construção de Trilhos](#construção-de-trilhos)
   - [Construção de Estações e Melhorias](#construção-de-estações-e-melhorias)
   - [Compra de Locomotivas](#compra-de-locomotivas)
   - [Montagem de Composições e Rotas](#montagem-de-composições-e-rotas)
   - [Gestão Financeira e Mercado de Ações](#gestão-financeira-e-mercado-de-ações)
3. [Controles e Atalhos de Teclado](#3-controles-e-atalhos-de-teclado)
   - [Navegação de Câmera e Zoom](#navegação-de-câmera-e-zoom)
   - [Velocidade da Simulação e Pausa](#velocidade-da-simulação-e-pausa)
   - [Telas de Informação e Acesso Rápido](#telas-de-informação-e-acesso-rápido)
   - [Relatórios Financeiros e Gráficos Interativos](#relatórios-financeiros-e-gráficos-interativos)
   - [Histórico de Eventos e Notificações](#histórico-de-eventos-e-notificações)
   - [Controle Gamepad e Steam Deck](#controle-gamepad-e-steam-deck)
4. [Diferenças em Relação ao Original de 1998](#4-diferenças-em-relação-ao-original-de-1998)
   - [Simulação Bit a Bit Fiel e Determinística](#simulação-bit-a-bit-fiel-e-determinística)
   - [Resoluções Modernas, Widescreen e 4K](#resoluções-modernas-widescreen-e-4k)
   - [Tipografia Vetorial Nítida e HiDPI](#tipografia-vetorial-nítida-e-hidpi)
   - [Multiplayer Moderno em Lockstep via TCP/IP](#multiplayer-moderno-em-lockstep-via-tcpip)
   - [Localização Completa em 6 Idiomas](#localização-completa-em-6-idiomas)
5. [Recursos Modernos Opcionais](#5-recursos-modernos-opcionais)
   - [Correções de Bugs Originais por Regra (`Fix`)](#correções-de-bugs-originais-por-regra-fix)
   - [Desfazer Construção (`Ctrl+Z`)](#desfazer-construção-ctrlz)
   - [Gerenciador de Frota e Troca de Locomotivas](#gerenciador-de-frota-e-troca-de-locomotivas)
   - [Overlays Avançados de Mapa](#overlays-avançados-de-mapa)
   - [Suporte a Mods via TOML](#suporte-a-mods-via-toml)
   - [Recursos de Acessibilidade](#recursos-de-acessibilidade)

---

## 1. Instalação e Execução

### Requisitos de Sistema e Arquivos

Para rodar o `rrt2`, seu computador precisa de:
- **Sistema Operacional**: Linux (x86_64), Windows 10/11 (x86_64) ou macOS 11+ (Apple Silicon ou Intel).
- **Placa de Vídeo / Gráficos**: Suporte a Vulkan, DirectX 12 ou Metal via biblioteca gráfica `wgpu`.
- **Arquivos Originais do Jogo**: Uma cópia válida do *Railroad Tycoon II Platinum*. O motor precisa dos seguintes arquivos:
  - `data/RT2.PK2`: Arquivo principal de sprites, texturas de terreno e interface.
  - `RT2_PLAT.EXE`: Executável original de onde são lidas tabelas de veículos, custos e balanceamento.
  - `default2.lng`: Tabela de textos originais do jogo.
  - Pastas opcionais: `maps/` (cenários adicionais), `games/` (saves pré-fabricados) e `anim/` (vídeos `.SMK`; requer `ffmpeg` e `ffplay` no sistema para reprodução).

### Detecção Automática de Instalações

Na primeira execução, o `rrt2` abre automaticamente a **Tela de Pasta do Jogo**. O sistema examina seu computador em busca de instalações existentes:
- **Steam**: Pastas de bibliotecas Steam (`steamapps/common/Railroad Tycoon 2`), incluindo Steam via Flatpak e prefixos do Proton (`compatdata/284840/pfx/drive_c`).
- **GOG**: Registros do Windows (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`), pastas padrão `GOG Games/Railroad Tycoon II` e biblioteca do Heroic Games Launcher.
- **Lutris e Wine**: Prefixos cadastrados no Lutris (`games/*.yml`), pastas padrão do Wine (`~/.wine`), Bottles e PlayOnLinux.
- **Pastas Comuns no Windows**: `C:\Games\`, `C:\Program Files (x86)\`, em todas as letras de unidade de `A:` a `Z:`.

Ao encontrar instalações válidas, elas são exibidas em uma lista clicável indicando a origem (ex: "Steam", "GOG", "Lutris"). Basta clicar na instalação desejada e pressionar **Jogar**.

### Configuração Manual da Pasta

Se o seu jogo estiver em outro diretório:
1. **Pela Interface**: Clique no botão **Escolher pasta...** na tela inicial para abrir o seletor nativo do sistema (usando o portal XDG no Linux, compatível com Flatpak).
2. **Pela Linha de Comando**:
   ```bash
   ./rrt2 set-path ~/Jogos/railroad-tycoon-2
   # No Windows:
   rrt2.exe set-path "C:\Jogos\Railroad Tycoon II"
   ```
3. **Por Parâmetro ou Variável de Ambiente**:
   ```bash
   ./rrt2 --game-dir /caminho/do/jogo
   # Ou definindo no terminal:
   export RT2_DIR=/caminho/do/jogo
   ./rrt2
   ```

### Pacotes por Sistema Operacional

- **Linux**:
  - **AppImage**: Binário único e independente com fontes vetoriais e traduções embutidas. Dê permissão de execução (`chmod +x rrt2-*.AppImage`) e execute diretamente.
  - **Flatpak**: Disponível pelo identificador `io.github.pedryvo.rrt2.flatpak`. Roda isolado no sistema; use o seletor de pastas da tela inicial para conceder acesso à pasta dos assets do jogo.
  - **Tarball (`.tar.gz`)**: Descompacte e execute `./rrt2`. Requer apenas `glibc` e `libasound.so.2` (Vulkan e Wayland/X11 são carregados dinamicamente).
- **Windows**:
  - **Instalador Inno Setup (`-setup.exe`)**: Não requer privilégios de administrador, cria atalho no menu Iniciar, registra desinstalador nas Configurações do Windows e memoriza a pasta do jogo.
  - **winget**: Instalação direta pelo gerenciador de pacotes do Windows:
    ```cmd
    winget install pedryvo.rrt2
    ```
  - **ZIP Portátil**: Basta extrair em qualquer pasta e rodar `rrt2.exe`.
- **macOS**:
  - **Imagem de Disco (`.dmg`)**: Abra o `.dmg` e arraste `RRT2.app` para a pasta `/Applications`. Suporta Apple Silicon (`aarch64`) e Intel (`x86_64`).
  - *Aviso do Gatekeeper*: Como as compilações de desenvolvimento não possuem assinatura paga da Apple, clique com o botão direito em `RRT2.app` e escolha **Abrir**, ou rode no terminal:
    ```bash
    xattr -dr com.apple.quarantine /Applications/RRT2.app
    ```

### Pastas de Configuração e Saves

O `rrt2` nunca grava arquivos dentro da pasta do jogo original. Todas as preferências e saves ficam organizados nas pastas de dados do usuário:

| Finalidade | Linux | Windows | macOS |
|---|---|---|---|
| Configurações (`config.toml`) | `~/.config/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |
| Jogos Salvos (`*.GAM`, `*.toml`, `*.png`) | `~/.local/share/rrt2/saves/` | `%APPDATA%\rrt2\saves\` | `~/Library/Application Support/rrt2/saves/` |
| Relatórios Exportados (`*.csv`) | `~/.local/share/rrt2/reports/` | `%APPDATA%\rrt2\reports\` | `~/Library/Application Support/rrt2/reports/` |
| Recordes Globais (`hiscore.dat`) | `~/.local/share/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |

> [!TIP]
> Você pode redirecionar esses caminhos definindo as variáveis de ambiente `RRT2_CONFIG`, `RRT2_SAVES` e `RRT2_REPORTS`.

---

## 2. Primeiros Passos

### Modos de Jogo: Campanhas, Cenários e Sandbox

No menu **Single Player**, escolha seu desafio:
- **Campanha Clássica (Original)**: Conquiste os 18 cenários históricos que cobrem os primeiros passos da ferrovia na América e no mundo (desde a histórica ferrovia *Baltimore & Ohio*). Ao iniciar cada mapa, escolha um bônus de abertura (subsídios para compra de locomotivas, desconto na construção de pontes ou redução de juros).
- **Campanha The Second Century**: Vivencie os 18 cenários da expansão com foco no século XX e XXI. Traz tração elétrica moderna, trens de alta velocidade para passageiros, logística industrial pesada e desafios econômicos do pós-guerra.
- **Cenários Avulsos**: Escolha qualquer mapa `.MAP` ou `.MP2`. Leia o briefing de abertura, confira as condições de vitória (Bronze, Prata e Ouro) e configure o número de competidores e os modelos industriais/financeiros.
- **Modo Sandbox**: Construção livre sem pressão de tempo, risco de falência ou tomada hostil por concorrentes.
- **Tutorial e Cenários de Exemplo**: Ideais para aprender. Carregue o mapa de tutorial para praticar o lançamento de vias e a montagem de rotas de carga sem comprometer suas finanças.

### Fundando sua Companhia Ferroviária

Ao iniciar um mapa sem empresa pré-definida:
1. **Nome e Marca**: Digite o nome da sua ferrovia (até 30 caracteres) e selecione o brasão/logotipo e a cor da sua empresa.
2. **Capital Inicial e Ações**: Ajuste os controles deslizantes de fundação. O painel pré-calcula:
   - Capital inicial da tesouraria da empresa.
   - Volume de ações emitidas e valor nominal de abertura.
   - Seu investimento pessoal como presidente ("You buy"): percentual das ações que pertencerão a você desde o primeiro dia.
3. **Cadeira da Presidência**: Mantenha as finanças saudáveis! Se a empresa acumular prejuízos excessivos ou você vender suas próprias ações, os acionistas rivais poderão aprovar sua destituição em assembleia geral.

### Construção de Trilhos

A malha ferroviária é a artéria do seu império:
1. Selecione a ferramenta **Lay Track** (Construir Trilhos) na barra lateral esquerda.
2. Clique e segure o **botão esquerdo do mouse** no tile de partida e arraste pelo mapa até o destino desejado.
3. **Prévia de Custo e Traçado em Tempo Real**: Conforme você move o cursor, o motor de busca de rotas desenha o caminho planejado, projeta pontes sobre rios e calcula o custo total exato. Destaques em vermelho indicam barreiras intransponíveis (relevo íngreme demais ou território sem direitos de passagem).
4. Solte o botão do mouse para comprar e assentar os trilhos imediatamente.
5. Pressione `Esc` enquanto arrasta para cancelar sem gastar dinheiro.

> [!TIP]
> Procure manter a inclinação dos trilhos abaixo de 2%. Rampas acentuadas (3% a 6%) derrubam a velocidade máxima das locomotivas a vapor e aumentam drasticamente o consumo de combustível.

### Construção de Estações e Melhorias

As estações captam passageiros, correspondência e cargas industriais em sua vizinhança:
1. Clique em **Build Station** (Construir Estação) na barra de ferramentas.
2. **Portes de Estação**:
   - **Depósito Pequeno (Small)**: Custo baixo ($50K), raio de captação de 2 tiles. Recomendado para indústrias rurais isoladas.
   - **Estação Média (Medium)**: Custo moderado ($100K), raio de 4 tiles. Excelente para cidades em crescimento.
   - **Terminal Grande (Large)**: Custo alto ($200K), raio de 8 tiles. Essencial para grandes centros metropolitanos.
3. **Círculo Dourado e Fantasma de Posicionamento**: Ao passar o cursor pelo mapa, um **círculo dourado** delimita a área de captação, e o prédio aparece destacado:
   - **Verde**: Localização válida e desimpedida.
   - **Amarelo**: Localização válida e conectada aos trilhos existentes.
   - **Vermelho**: Localização inválida (terreno irregular, água ou sobreposição com outra estação).
4. **Estilos Arquitetônicos**: Escolha entre vitoriano, colonial, missionário, tudor ou kyoto para personalizar a identidade da estação.
5. **Melhorias de Estação**: Clique em uma estação existente para adquirir melhorias cruciais:
   - **Operacionais**: *Torre de Água* (Water Tower) e *Depósito de Areia* (Sandhouse) para reposição rápida em locomotivas a vapor; *Rotunda* (Roundhouse), que reduz o custo de manutenção dos motores em 50%.
   - **Comerciais**: *Telégrafo* (aumenta velocidade de faturamento), *Hotel* e *Restaurante* (geram bônus substanciais com passageiros), *Alfândega* (Customs House) para linhas internacionais.
   - **Logísticas**: *Silo de Grãos*, *Câmara Frigorífica* e *Galpão de Mercadorias* para estocar excedentes e evitar perdas de carga.

### Compra de Locomotivas

Clique em uma estação ou abra a tela de trens e selecione **Purchase Train**:
- **Catálogo de Motores**: Locomotivas históricas classificadas por era e tipo de tração (Vapor, Diesel e Elétrica).
- **Matriz de Desempenho**: Examine a matriz de velocidade em função da inclinação (0%, 2%, 4% e 6%) com cargas de 1 a 6 vagões de 30 a 50 toneladas.
- **Custos Operacionais**: Avalie com atenção a taxa de manutenção anual, custo de combustível, índice de confiabilidade e curvas de aceleração antes da compra.
- **Eletrificação**: Lembre-se de que locomotivas elétricas exigem trilhos previamente eletrificados!

### Montagem de Composições e Rotas

1. **Agendamento de Paradas**: Abra o painel do trem e selecione as paradas na lista ou clicando nas estações do mapa.
2. **Tipos de Vagões**:
   - **Cargas Expressas**: Passageiros e Correio (altamente lucrativos, sensíveis ao tempo de viagem e à velocidade média).
   - **Cargas a Granel e Industriais**: Carvão, Minério de Ferro, Grãos, Gado, Madeira, Petróleo, Aço, Bens Manufaturados, Automóveis.
3. **Política de Embarque**: Defina se o trem parte imediatamente, espera meia carga ou aguarda lotação total (`Wait for Full Cargo`).

### Gestão Financeira e Mercado de Ações

O sucesso como magnata exige domínio financeiro:
- **Títulos da Dívida (Bonds)**: Emita debêntures para financiar grandes expansões de malha. A taxa de juros exigida pelo mercado depende da classificação de crédito da sua empresa (de AAA até grau especulativo F). Amortize as dívidas assim que o fluxo de caixa permitir.
- **Mercado Acionário (`M`)**:
  - **Negociação de Ações**: Compre e venda ações pessoais da sua ferrovia ou de companhias concorrentes para lucrar com dividendos e valorização.
  - **Recompra de Ações (Buyback)**: Use o caixa da empresa para recomprar as próprias ações no mercado aberto, aumentando o lucro por ação e valorizando a cotação.
  - **Desdobramentos (Stock Splits)**: Conforme o valor da ação dispara, ocorrem desdobramentos automáticos de 2 para 1, facilitando a liquidez.
- **Dividendos**: Pague dividendos anuais para satisfazer os investidores e inflar o preço das ações, sem comprometer o caixa de giro.
- **Fusões e Aquisições**: Adquira mais de 50% das ações de um concorrente para forçar uma fusão, absorvendo toda a infraestrutura, estações e trens do rival.

---

## 3. Controles e Atalhos de Teclado

Todos os atalhos podem ser personalizados em **Opções (`F10`) > Teclas**.

### Navegação de Câmera e Zoom

| Ação | Tecla / Entrada | Observações |
|---|---|---|
| Mover Câmera | `W`, `A`, `S`, `D` ou Setas | Movimento direcional contínuo |
| Arrasto com o Mouse | Segurar Botão Direito e Arrastar | Arrasto natural da visão isométrica |
| Rolagem na Borda | Cursor a menos de 4px da borda | Ativável na aba Jogabilidade |
| Girar Câmera à Esquerda | `Q` | Rotação isométrica de 90° anti-horária |
| Girar Câmera à Direita | `E` | Rotação isométrica de 90° horária |
| Zoom In (Aproximar) | Roda do mouse para cima / `+` | Zoom suave e contínuo |
| Zoom Out (Afastar) | Roda do mouse para baixo / `-` | Zoom suave e contínuo |
| Centralizar Câmera | `Home` | Volta o enquadramento ao ponto central |

### Velocidade da Simulação e Pausa

| Ação | Atalho | Observações |
|---|---|---|
| Pausar / Despausar | `Pause` ou `Espaço` | Congela o avanço do calendário |
| Diminuir Velocidade | `-` ou `Numpad -` | Reduz o ritmo do tempo simulado |
| Aumentar Velocidade | `+` ou `Numpad +` | Acelera o tempo simulado |
| Tela Cheia | `F11` | Alterna modo janela e tela cheia sem bordas |
| Ajustar Escala da UI | `Ctrl +` / `Ctrl -` | Passos de 25% (entre 100% e 300%) |
| Silenciar Áudio | `Ctrl + M` | Ativa/desativa música e efeitos |
| Próxima Faixa Musical | `Ctrl + N` | Pula para a próxima canção da trilha |

### Telas de Informação e Acesso Rápido

| Janela | Tecla | Conteúdo |
|---|---|---|
| **Finanças** | `F` | DRE detalhado, balanço patrimonial, crédito e dívida |
| **Bolsa de Valores** | `M` | Cotações em tempo real, painel de ações e negociação |
| **Lista de Trens** | `T` | Todas as locomotivas ativas, rotas, desgaste e idade |
| **Lista de Estações** | `G` | Movimento de cargas, melhorias, demanda local |
| **Cidades** | `C` | População, índice de crescimento, suprimentos demandados |
| **Indústrias** | `I` | Produção fabril, estoques e compra/venda de instalações |
| **Visão Geral do Mapa** | Barra lateral (Ícone do Globo) | Modos temáticos de mapa em tela cheia |
| **Salvar Rápido** | `F5` | Grava no slot fixo quicksave |
| **Carregar Rápido** | `F9` | Carrega o último quicksave |
| **Diálogo Salvar** | `Ctrl + S` | Abre gerenciador de saves com nome personalizado |
| **Diálogo Carregar** | `Ctrl + L` | Navegador de saves com miniatura e resumo |
| **Opções** | `F10` ou `Esc` | Configurações de vídeo, som, teclas e jogabilidade |

### Relatórios Financeiros e Gráficos Interativos

Acessíveis a partir do Livro da Empresa (botão **Reports** ou teclas `1`, `2`, `3`):
- **Gráficos Interativos (Aba 1)**: Traça até 12 variáveis ao longo do tempo: Faturamento, Despesas, Lucro Líquido, Caixa, Dívida Total, Patrimônio Líquido, Preço da Ação, Valor Contábil por Ação, Lucro por Ação e Dividendos. Utiliza a paleta de alto contraste Okabe-Ito com marcadores geométricos, permitindo ligar e desligar competidores e inspecionar meses passados.
- **DRE por Categoria (Aba 2)**: Discriminativo das 13 categorias de receita e despesa comparando o acumulado histórico, o ano atual e os últimos 5 anos fiscais.
- **Rankings (Aba 3)**: Tabelas de classificação com os trens mais lucrativos e as estações de maior movimento. Clicar em qualquer linha move a câmera diretamente para o local correspondente.
- **Exportação CSV**: O botão **Export CSV** salva instantaneamente os dados em formato tabular em `~/.local/share/rrt2/reports/*.csv` para análise em planilhas eletrônicas.

### Histórico de Eventos e Notificações

Localizado na Caixa de Atividade (Activity Box) no HUD inferior:
- Clique em **History** para acessar o registro de até 500 mensagens do jogo (eventos de cenário, manchetes econômicas, bolsa de valores, crescimento urbano e passos dos concorrentes).
- Filtre por categoria ou clique em **Show on Map** para levar a câmera instantaneamente ao tile exato do evento.
- Você pode personalizar quais categorias abrem janelas ou jornais em **Opções > Avisos**.

### Controle Gamepad e Steam Deck

Para partidas no Steam Deck ou com controles comuns:
- **Analógico Esquerdo**: Movimentação suave da câmera pelo mapa.
- **Analógico Direito**: Cursor virtual com mira assistida para seleção de trilhos e prédios.
- **Gatilhos (L2 / R2)**: Controle de velocidade do jogo e pausa rápida.
- **Botões de Ombro (L1 / R1)**: Rotação da câmera (`Q` / `E`) e zoom.
- **Botões Frontais (A / B / X / Y)**: Confirmação, Cancelamento/Fechar, Atalho de Trilho, Atalho de Estação.

---

## 4. Diferenças em Relação ao Original de 1998

O projeto mantém rigorosamente toda a alma, estética e charme do *Railroad Tycoon II Platinum*, modernizando os alicerces técnicos:

```
┌─────────────────────────┬─────────────────────────────────┬─────────────────────────────────┐
│ Recurso                 │ Jogo Original (1998–2001)       │ rrt2-decomp (Motor em Rust)     │
├─────────────────────────┼─────────────────────────────────┼─────────────────────────────────┤
│ Resolução de Tela       │ Fixo em 800x600 ou 1024x768     │ Nativo em 1080p, 1440p, 4K e UW │
│ Escala de Interface     │ Inexistente (pixels minúsculos) │ Fontes vetoriais (100% a 300%)  │
│ Fidelidade da Simulação │ Binário Win32 x86 fechado       │ 100% determinístico e bit a bit │
│ Renderização            │ DirectDraw / Software GDI       │ Aceleração nativa por GPU (wgpu)│
│ Navegação da Câmera     │ Zoom rígido e scroll travado    │ Zoom contínuo e arrasto suave   │
│ Conexão Multiplayer     │ DirectPlay 4, IPX, Modem, Serial│ TCP/IP moderno em lockstep      │
│ Suporte de Rede         │ Queda do host encerra a partida │ Migração de host e reconexão    │
│ Plataformas Suportadas  │ Apenas Windows 95/98/XP         │ Linux nativo, Windows e macOS   │
│ Idiomas                 │ Um idioma fixo por disco        │ 6 idiomas alternáveis no jogo   │
└─────────────────────────┴─────────────────────────────────┴─────────────────────────────────┘
```

### Simulação Bit a Bit Fiel e Determinística

O motor de simulação (`rt2-sim`) não faz aproximações arbitrárias. Cada cálculo matemático — curvas de frenagem dos comboios, quebras mecânicas, produção fabril, cotações de ações e inteligência artificial concorrente — foi obtido por engenharia reversa do executável original e validado contra emulação direta (Unicorn). Saves carregados no motor comportam-se de forma 100% idêntica ao jogo de 1998.

### Resoluções Modernas, Widescreen e 4K

Diga adeus ao estiramento de imagens antigas. A malha de terreno em perspectiva isométrica, os rios e os trilhos são renderizados pelo `wgpu` na resolução nativa do seu monitor. Os sprites 2D originais passam por filtros de alta fidelidade (xBRZ e interpolação inteligente) que geram um cache em disco na primeira inicialização, oferecendo nitidez impecável em monitores 4K e ultrawide.

### Tipografia Vetorial Nítida e HiDPI

Em vez de fontes bitmap borradas, o `rrt2` traz fontes vetoriais licenciadas SIL OFL, calibradas milimetricamente para respeitar o design clássico:
- **Cinzel**: Versaletes clássicos em substituição à Copperplate Gothic original.
- **Libre Franklin**: Sans-serif limpa para tabelas e caixas de dados numéricos.
- **Dancing Script**: Cursiva elegante para condecorações e cartas de vitória.
- **Anton & Oswald**: Títulos pesados e de forte impacto visual.
- **Noto Sans SC**: Tipografia CJK integrada para suporte perfeito a caracteres chineses.
- **Cormorant SC**: Versalete com suporte cirílico para o idioma russo.

### Multiplayer Moderno em Lockstep via TCP/IP

Todo o sistema antiquado baseado em DirectPlay, IPX e conexões por modem dial-up foi substituído por uma arquitetura TCP/IP robusta e direta:
- **Lockstep Sequenciado pelo Anfitrião**: As máquinas trocam apenas pacotes compactos de comandos, executando a simulação sincronizada minuto a minuto.
- **Entrada em Partidas em Andamento e Reconexão**: Jogadores que sofrerem queda de conexão podem retornar à partida; o anfitrião envia um instantâneo comprimido do estado do mundo.
- **Migração de Anfitrião**: Se quem estiver hospedando sair, o jogador de menor identificador assume a hospedagem automaticamente, sem interromper o jogo.
- **Transição Suave para IA**: Se um participante abandonar a sessão, a sua empresa passa imediatamente para o controle da IA, garantindo a continuidade da partida.

### Localização Completa em 6 Idiomas

Mude instantaneamente entre **English**, **Português**, **Español**, **Deutsch**, **中文 (Chinês Simplificado)** e **Русский** no menu **Opções > Idioma**. Todos os menus, caixas de diálogo e elementos textuais são reformatados na hora sem necessidade de reiniciar.

---

## 5. Recursos Modernos Opcionais

Todas as melhorias de qualidade de vida são modulares e opcionais, preservando as regras clássicas por padrão.

### Correções de Bugs Originais por Regra (`Fix`)

O executável original de 1998 continha certas anomalias conhecidas (como arredondamentos incorretos em ações de fundadores, cobrança duplicada em certas melhorias de estação e estouros de inteiros no caixa em partidas centenárias). No `rrt2`, você escolhe:
- **Modo Clássico Estrito (Padrão)**: Reproduz fielmente todas as idiossincrasias originais para preservação histórica estrita.
- **Modo com Correções (`Fix`)**: Ativa soluções matemáticas auditadas, registradas no cabeçalho do save para manter replays perfeitamente válidos.

### Desfazer Construção (`Ctrl+Z`)

Assentou uma ponte caríssima no lugar errado ou clicou em um tile indesejado? Pressione `Ctrl + Z` para desfazer a última ação de construção de trilhos ou estações. Desde que nenhum trem tenha cruzado o trecho recém-construído, seu dinheiro é 100% estornado.

### Gerenciador de Frota e Troca de Locomotivas

Administrar dezenas de trens em malhas gigantescas deixa de ser uma tarefa repetitiva:
- **Modelos de Composição (Consist Templates)**: Salve predefinições de vagões (ex: "Expresso de Luxo de Passageiros" ou "Linha Pesada de Carvão") e aplique-as a novos trens com um clique.
- **Troca de Locomotivas em Massa**: Marque uma classe obsoleta de locomotivas e agende a modernização automática para toda a frota assim que cada comboio passar por uma oficina ou rotunda.

### Overlays Avançados de Mapa

Além das visões clássicas de recursos e relevo, aproveite novas camadas analíticas:
- **Mapa de Calor de Oferta e Demanda**: Identifique em segundos regiões ricas em excedentes e cidades com carência urgente de produtos.
- **Gradiente de Trilhos**: Mapa cromático de inclinação para detectar gargalos de velocidade.
- **Rentabilidade por Trecho**: Destaque visual dos ramais mais lucrativos e das linhas com prejuízo operacional.
- **Densidade e Congestionamento**: Visualize os cruzamentos e trechos de via única com filas e esperas excessivas.

### Suporte a Mods via TOML

Os dados do jogo deixaram de ser prisioneiros de tabelas binárias:
- Atributos de locomotivas, custos de vagões, cadeias produtivas industriais e regras de cenários podem ser extraídos e carregados a partir de arquivos legíveis em formato **TOML**.
- Modders e criadores de conteúdo podem introduzir trens regionais inéditos, novas cargas e cadeias fabris sem tocar no código fonte.

### Recursos de Acessibilidade

- **Paletas Adaptadas para Daltonismo**: Gráficos e radares usam as cores Okabe-Ito, complementadas por símbolos geométricos (círculos, triângulos, quadrados) e traçados variados.
- **Escala de Texto Independente**: Ajuste o tamanho da tipografia de leitura sem afetar o tamanho dos painéis do HUD.
- **Alertas Sonoros com Legenda Visual**: Avisos visuais na tela acompanham eventos críticos (quebras mecânicas graves, descarrilamentos, assaltos ou avisos de margem financeira).

---

*Railroad Tycoon é marca registrada de seus respectivos detentores. Este projeto livre desenvolvido por fãs não possui vínculo com Take-Two Interactive, PopTop Software ou Gathering of Developers.*
