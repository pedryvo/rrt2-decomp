# Som: efeitos, música e vídeos

Implementação: `crates/rt2-formats/src/sound.rs` (tabela de descritores e decodificação do PCM), `crates/rt2-audio` (reprodução com kira, regras de música, tabela de eventos), ferramenta `rt2-play` (`rt2-play --list`, `rt2-play buttu61m`, `rt2-play music`, `rt2-play --video intro_01`). Testes contra os dados originais: `crates/rt2-formats/tests/sound.rs` e `crates/rt2-audio/tests/original.rs` (precisam de `RT2_DIR`).

Fonte: o EXE (`RT2_PLAT.EXE`); os endereços abaixo são virtuais. O áudio nunca influencia a simulação: o único `rand` do CRT ligado a som (`0x4d0400`, `0x4d2360`, `0x4d29e0`, `0x406470`, `0x47d100`, ver `trains.md` 5.4) só escolhe a variante do som; o clone usa um gerador separado (`rt2_audio::rng::AudioRng`).

## 1. Efeitos sonoros (`*.raw` nos PK2)

### 1.1 Formato resolvido

Os blobs de áudio dos PK2 são **PCM cru sem cabeçalho** (o loader `0x4f40e0` copia os bytes do PK2 direto para o buffer DirectSound, sem decodificar). O formato não está nos dados: fica numa **tabela de descritores de 0x12 bytes em `0x55e7b0`** (`sound_table`), 125 linhas. O índice da linha é o **id de som** que o resto do jogo passa adiante (tabelas de motor e de prédio, campos de controles de UI, chamadas diretas).

```
char[13]  nome       8.3 terminado em NUL ("buttu61m.raw"); blob do PK2 = name_hash(nome) (pk2.md)
u8        canais     1 em todas as linhas
u8        bits       16 (122 linhas) ou 8 (clock82m.raw, sem sinal)
u16       taxa       11025 Hz (122 linhas) ou 22050 Hz (clock82m.raw)
u8        pitch      1 = buffer com DSBCAPS_CTRLFREQUENCY (só chug1..6); senão só volume
```

- Leitura em `0x4ecb00` (monta o `WAVEFORMATEX`: canais, taxa, bits; `nBlockAlign = bits/8 * canais`) e `0x4eeeb0` (taxa padrão = `u16` em `+0xf`).
- 123 linhas têm nome; as linhas 0 e 114 são vagas (nome de 12 espaços). **Todos os 123 nomes casam com um blob** (121 em RT2.PK2, `airpt61m.raw` e `flyby61m.raw` em RT2_06.PK2): é todo o áudio PCM dos PK2.
- 16 bit é complemento de dois little-endian; 8 bit é sem sinal (centro em 128).
- Durações de 0,05 s (`horse`) a 19,5 s (`tr_in`); `flyby61m.raw` tem 320000 bytes = 14,5 s a 11025 Hz. **Correção de `pk2-contents.md`**: a taxa não é 22050 Hz (e a hipótese de 16 kHz para `RT2_06/0001` também cai: é o `flyby`).
- Os "39 blobs de ruído" do primeiro levantamento são sons de banda larga (chuva, vapor, insetos, máquinas) ou de tom muito agudo (sinos a 4,3 kHz têm autocorrelação lag-1 negativa), não outro codec: pico entre 1400 e 32000, sem clipping, e a leitura como 8 bit só piora. Os 123 decodificam como descrito.

### 1.2 Como o jogo toca (`sound_play`, `0x4eeeb0`)

`sound_play(descritor, em_loop, volume%, frequência|-1, atraso_ms)` (fastcall). Devolve o id de uma voz; `0x4ecb00` reutiliza uma voz livre do mesmo efeito ou cria outro buffer (vários disparos simultâneos do mesmo som são permitidos).

- `em_loop = 0`: disparo único (a voz é recolhida por `0x4ee650` a cada 500 ms); `!= 0`: DSBPLAY_LOOPING. Com o contador global `0x57b41c != 0` os disparos únicos são suprimidos (telas que se montam em silêncio).
- `volume%` 0..100 → `0x4ed320` converte para centibels do DirectSound (limite -10000..0). **Lacuna**: a fórmula está em instruções x87 que a decompilação perdeu; o clone usa `20·log10(v/100)` dB.
- `frequência`: `-1` = a do descritor; `0x4d0870` passa a velocidade do trem (e `0x2b11` = 11025 Hz quando quebrado).
- `atraso_ms`: início agendado (sinos de estação, 150 ms entre si).
- Trens e sons de mapa usam o volume por distância de `0x49b610` (fórmula não decodificada) e só tocam se o resultado for **maior que 5** (`0x4d0400`, `0x4d29e0`); o jogo não tem pan. O clone adiciona pan e uma queda linear (`rt2_audio::spatial`), como opção moderna.

### 1.3 Tabela de ids (nome → evento)

Chamadores lidos na decompilação (endereço da primeira ocorrência). "Sem chamador direto" = o id chega por um campo de controle da UI (`+0x19` botão pressionado, `+0x1a` solto, `+0x1b` outro, em `0x508160`, `0x50ed50`, `0x50fe50`, `0x50ff50`; vêm dos layouts `.win`), por tabela ou por faixa.

| id | arquivo | s | evento |
|---|---|---|---|
| 1 | `buttu61m.raw` | 0.13 | botão solto (`ButtonUp`, 0x50cbd0; também 0x4012da/0x401884) |
| 2 | `buttd61m.raw` | 0.23 | botão apertado (`ButtonDown`, 0x50cc60/0x50ccc0) |
| 3 | `scrol61m.raw` | 0.26 | clique em lista/rolagem (0x506220, 0x411150) |
| 4 | `scro261m.raw` | 0.18 | rolagem suave, volume 90 (0x5062f0) |
| 5 | `clock82m.raw` | 4.44 | relógio em loop nas telas de carga/espera, volume 70 ou 50 (0x40de20) |
| 6 | `birds61m.raw` | 7.30 | ambiência de terreno 0 (loop, 0x49bf70/0x49b700) |
| 7 | `waves61m.raw` | 10.50 | ambiência 1 |
| 8 | `swamp61m.raw` | 6.01 | ambiência 2 |
| 9 | `river61m.raw` | 4.20 | ambiência 3 |
| 10 | `blizz61m.raw` | 5.83 | ambiência 4 |
| 11 | `jungL61m.raw` | 6.65 | ambiência 5 |
| 12 | `paget61m.raw` | 0.31 | troca de página principal (0x4022c5, 0x4eb590) |
| 13 | `page261m.raw` | 0.43 | virar página secundária (0x4027ac, 0x4c5550, 0x4c58c0) |
| 14 | `addbu61m.raw` | 1.79 | construir prédio da própria empresa (0x4b0210) |
| 15 | `chain61m.raw` | 2.15 | corrente (ferramenta de trilho, 0x48eb30 caso 6) |
| 16 | `bulld61m.raw` | 2.76 | demolir (0x4b0210) |
| 17 | `swtch61m.raw` | 0.28 | sem chamador direto; id vindo de campo de controle |
| 18 | `tab__61m.raw` | 0.34 | aba (0x401884) |
| 19 | `chug161m.raw` | 1.68 | loop de motor (`chug`), escolhido por `engine_table +0x29`; pitch segue a velocidade |
| 20 | `chug261m.raw` | 1.68 | loop de motor (`chug`), escolhido por `engine_table +0x29`; pitch segue a velocidade |
| 21 | `chug361m.raw` | 3.33 | loop de motor (`chug`), escolhido por `engine_table +0x29`; pitch segue a velocidade |
| 22 | `chug461m.raw` | 1.67 | loop de motor (`chug`), escolhido por `engine_table +0x29`; pitch segue a velocidade |
| 23 | `chug561m.raw` | 1.52 | loop de motor (`chug`), escolhido por `engine_table +0x29`; pitch segue a velocidade |
| 24 | `chug661m.raw` | 1.61 | loop de motor (`chug`), escolhido por `engine_table +0x29`; pitch segue a velocidade |
| 25 | `whis161m.raw` | 1.82 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 26 | `whis261m.raw` | 2.16 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 27 | `whis361m.raw` | 2.18 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 28 | `whis461m.raw` | 4.45 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 29 | `whis561m.raw` | 1.05 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 30 | `whis661m.raw` | 2.71 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 31 | `whis761m.raw` | 3.92 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 32 | `whis861m.raw` | 3.17 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 33 | `whis961m.raw` | 1.23 | apito de partida por motor (`engine_table +0x2a`, 0x4d0400) |
| 34 | `horse61m.raw` | 0.05 | cavalo (0x4b74f0, evento de época) |
| 35 | `trdtl61m.raw` | 15.12 | loop do projeto de trem a vapor (0x4de210) |
| 36 | `cargo61m.raw` | 1.06 | carga selecionada (0x4012da) |
| 37 | `radio61m.raw` | 0.46 | rádio/mensagem (0x4012da, 0x401884, 0x494f40) |
| 38 | `nexts61m.raw` | 0.23 | próxima estação / trem selecionado (0x4b0210) |
| 39 | `brake61m.raw` | 2.23 | freio na chegada à estação, `rand(0,3)` (0x4d29e0) |
| 40 | `brak261m.raw` | 2.30 | freio na chegada à estação, `rand(0,3)` (0x4d29e0) |
| 41 | `brak361m.raw` | 2.61 | freio na chegada à estação, `rand(0,3)` (0x4d29e0) |
| 42 | `brak461m.raw` | 3.07 | freio na chegada à estação, `rand(0,3)` (0x4d29e0) |
| 43 | `LUMBR61M.RAW` | 7.11 | loop de prédio, tipos 22 (`building_type_table +0`, 0x40b8b0) |
| 44 | `PAPER61M.RAW` | 0.71 | loop de prédio, tipos 27 (`building_type_table +0`, 0x40b8b0) |
| 45 | `GRAVL61M.RAW` | 6.19 | loop de prédio, tipos 17 (`building_type_table +0`, 0x40b8b0) |
| 46 | `CMINE61M.RAW` | 3.38 | loop de prédio, tipos 9 (`building_type_table +0`, 0x40b8b0) |
| 47 | `STEEL61M.RAW` | 3.16 | loop de prédio, tipos 32 (`building_type_table +0`, 0x40b8b0) |
| 48 | `CATTL61M.RAW` | 9.02 | loop de prédio, tipos 6,12 (`building_type_table +0`, 0x40b8b0) |
| 49 | `LOGGI61M.RAW` | 2.03 | loop de prédio, tipos 21 (`building_type_table +0`, 0x40b8b0) |
| 50 | `cemnt61M.RAW` | 3.60 | loop de prédio, tipos 7 (`building_type_table +0`, 0x40b8b0) |
| 51 | `auto_61M.RAW` | 8.44 | loop de prédio, tipos 2 (`building_type_table +0`, 0x40b8b0) |
| 52 | `caner61M.RAW` | 3.26 | loop de prédio, tipos 5 (`building_type_table +0`, 0x40b8b0) |
| 53 | `grain61M.RAW` | 5.76 | loop de prédio, tipos 16 (`building_type_table +0`, 0x40b8b0) |
| 54 | `nuker61M.RAW` | 6.44 | loop de prédio, tipos 24 (`building_type_table +0`, 0x40b8b0) |
| 55 | `textl61M.RAW` | 4.22 | loop de prédio, tipos 34 (`building_type_table +0`, 0x40b8b0) |
| 56 | `elect61M.RAW` | 3.98 | loop de prédio, tipos 14 (`building_type_table +0`, 0x40b8b0) |
| 57 | `baux_61M.RAW` | 3.04 | loop de prédio, tipos 4,19 (`building_type_table +0`, 0x40b8b0) |
| 58 | `MEATP61M.RAW` | 2.80 | loop de prédio, tipos 23 (`building_type_table +0`, 0x40b8b0) |
| 59 | `LANDF61M.RAW` | 8.25 | loop de prédio, tipos 20 (`building_type_table +0`, 0x40b8b0) |
| 60 | `STATN61M.RAW` | 4.99 | loop de prédio, tipos 0 (`building_type_table +0`, 0x40b8b0) |
| 61 | `SHEEP61M.RAW` | 8.47 | loop de prédio, tipos 31 (`building_type_table +0`, 0x40b8b0) |
| 62 | `TIREF62M.RAW` | 2.74 | loop de prédio, tipos 35,36 (`building_type_table +0`, 0x40b8b0) |
| 63 | `OILRF61M.RAW` | 5.45 | loop de prédio, tipos 25 (`building_type_table +0`, 0x40b8b0) |
| 64 | `BAKER61M.RAW` | 3.96 | loop de prédio, tipos 3 (`building_type_table +0`, 0x40b8b0) |
| 65 | `alum_61M.RAW` | 2.14 | loop de prédio, tipos 1 (`building_type_table +0`, 0x40b8b0) |
| 66 | `uran_61m.raw` | 4.56 | loop de prédio, tipos 37 (`building_type_table +0`, 0x40b8b0) |
| 67 | `sugar61m.raw` | 4.26 | loop de prédio, tipos 10,11,29,30,33 (`building_type_table +0`, 0x40b8b0) |
| 68 | `oilwl61m.raw` | 3.77 | loop de prédio, tipos 8,26 (`building_type_table +0`, 0x40b8b0) |
| 69 | `port_61m.raw` | 8.90 | loop de prédio, tipos 28 (`building_type_table +0`, 0x40b8b0) |
| 70 | `dairp61m.raw` | 3.66 | loop de prédio, tipos 13 (`building_type_table +0`, 0x40b8b0) |
| 71 | `fert_61m.raw` | 4.80 | loop de prédio, tipos 15 (`building_type_table +0`, 0x40b8b0) |
| 72 | `chin161m.raw` | 0.67 | sino de estação 1 (0x47d100, rand 0..2) |
| 73 | `chin261m.raw` | 0.82 | sino de estação 2 |
| 74 | `chin361m.raw` | 0.45 | sino de estação 3 |
| 75 | `laytr61m.raw` | 2.44 | assentar trilho (0x495d50) |
| 76 | `tr_in61m.raw` | 19.46 | trem chegando, volume 90 (0x406470 telas 4, 5, 10) |
| 77 | `layrv61m.raw` | 2.61 | assentar rio/terreno (0x48eb30) |
| 78 | `addtr61m.raw` | 1.49 | adicionar trilho (0x48eb30 caso 5) |
| 79 | `raise61m.raw` | 3.73 | elevar terreno (0x48eb30 caso 1) |
| 80 | `buckt61m.raw` | 1.16 | balde (0x48eb30) |
| 81 | `bella61m.raw` | 1.80 | sino A, volume 70 (0x404e30) |
| 82 | `bellb61m.raw` | 1.83 | sino B, volume 70 (0x404e30) |
| 83 | `adds161m.raw` | 0.51 | trem: ordem adicionada (0x4d11f0) |
| 84 | `adds261m.raw` | 0.17 | (sem chamador direto) |
| 85 | `adds361m.raw` | 0.16 | trem: parada adicionada (0x4dd6c0) |
| 86 | `whtab61m.raw` | 0.91 | mesa (`whtab`): 0x4eeeb0 chama 0x45cd60 ao tocar; chamador por campo de controle |
| 87 | `hawk_61m.raw` | 1.74 | gavião/ave de caça (0x406470, estações) |
| 88 | `cargr61m.raw` | 0.84 | carga removida (0x4012da) |
| 89 | `book_61m.raw` | 0.47 | livro (0x4ebbd0) |
| 90 | `noteb61m.raw` | 0.42 | caderno (sem chamador direto) |
| 91 | `buybu61m.raw` | 2.44 | comprar (0x4c1f80, 0x4c2050, 0x4010fa) |
| 92 | `brush61m.raw` | 1.06 | pincel (0x48eb30) |
| 93 | `resou61m.raw` | 0.48 | recurso (0x493970, 0x494f40) |
| 94 | `trdtd61m.raw` | 8.99 | loop do projeto de trem diesel (0x4de210) |
| 95 | `papfl61m.raw` | 1.49 | papel: virar (inferido) |
| 96 | `papnw61m.raw` | 1.11 | papel: jornal novo (0x4a4050) |
| 97 | `stock61m.raw` | 1.56 | bolsa: negócio (0x4c57d0) |
| 98 | `roost61m.raw` | 1.67 | poleiro/galo (0x406470) |
| 99 | `trdte61m.raw` | 6.61 | loop do projeto de trem elétrico (0x4de210) |
| 100 | `LOCKC61m.raw` | 0.40 | cadeado fechado (0x401884) |
| 101 | `LOCKO61m.raw` | 0.25 | cadeado aberto (0x401884) |
| 102 | `GEESE61m.raw` | 7.59 | gansos (0x406470, meses 3..5 e 9..11 do campo de mês) |
| 103 | `AFRIC61m.raw` | 3.19 | África (0x406470, região) |
| 104 | `GAMES61m.raw` | 0.20 | modo de jogo mudou, volume 85 (0x4b0210) |
| 105 | `pigeo61m.raw` | 5.55 | pombos (0x406470 tela 0xc) |
| 106 | `ENBRE61m.raw` | 3.40 | quebra de trem (0x4ce4c0) |
| 107 | `ENBR261m.raw` | 3.01 | loop de trem a vapor quebrado (0x4d0870) |
| 108 | `ENCRA61m.raw` | 3.74 | acidente (0x4ce730) |
| 109 | `ENFIR61m.raw` | 3.13 | loop de incêndio, trem acidentado (0x4d0870) |
| 110 | `ENROB61m.raw` | 5.67 | roubo (0x4ceaa0) |
| 111 | `ANNCL61m.raw` | 0.46 | anúncio (sem chamador direto) |
| 112 | `ENBR361m.raw` | 10.16 | loop de trem diesel/elétrico quebrado (0x4d0870) |
| 113 | `CMBKG61m.raw` | 8.80 | fundo da tela 0xc (loop, 0x40de20, `DAT_00566d3d` 0 ou 2) |
| 115 | `STOCS61m.raw` | 1.56 | bolsa: venda, volume 82 (0x4c56e0) |
| 116 | `campg61m.raw` | 1.28 | campanha (0x411150) |
| 117 | `cmbk261m.raw` | 10.31 | fundo da tela 0xc (loop, `DAT_00566d3d` outro valor) |
| 118 | `barrk61m.raw` | 5.85 | loop de prédio, tipos 43 (`building_type_table +0`, 0x40b8b0) |
| 119 | `Distl61m.raw` | 3.01 | loop de prédio, tipos 42 (`building_type_table +0`, 0x40b8b0) |
| 120 | `geoc_61m.raw` | 4.16 | loop de prédio, tipos 38 (`building_type_table +0`, 0x40b8b0) |
| 121 | `Munit61m.raw` | 4.81 | loop de prédio, tipos 45 (`building_type_table +0`, 0x40b8b0) |
| 122 | `weapf61m.raw` | 3.02 | loop de prédio, tipos 46 (`building_type_table +0`, 0x40b8b0) |
| 123 | `airpt61m.raw` | 11.38 | loop de prédio, tipos 47 (`building_type_table +0`, 0x40b8b0) |
| 124 | `flyby61m.raw` | 14.51 | voo rasante de avião (0x406470, `DAT_00574bb0`) |

### 1.4 Famílias escolhidas por índice

- **Ambiência de terreno** (`0x49bf70`, `0x49b700`): ids 6..11 (`birds, waves, swamp, river, blizz, jungL`), em loop, índice de ambiência 0..5, volume pela posição.
- **Motor** (`engine_table` `0x53cec0`, 66 registros de 0x3a, `trains.md` 2.2): `+0x28` tipo (0 vapor, 1 diesel, 2 elétrica), `+0x29` id do loop e `+0x2a` id do apito. Loop (`0x4d0870`): ids 19/20 alternam pela paridade do id do trem (`19 + (id & 1)`), 22 vira `22 + (id & 1)`, 21/23/24 fixos; trem quebrado usa 107 (vapor) ou 112 (outros), acidentado usa 109; o apito de partida (ids 25..33, `0x4d0400`) toca quando o trem sai da estação com volume > 5. Valores por motor: `rt2_audio::events::ENGINE_SOUNDS`.
- **Freio** (`0x4d29e0`): ids 39..42 (`brake`, `brak2..4`), `rand(0,3)`, na chegada com volume > 5.
- **Sinos de estação** (`0x47d100`): ids 72..74, `rand(0,2)`, um por vagão com 150 ms de intervalo.
- **Prédios e indústrias** (`building_type_table` `0x535c38`, campo `+0`, 53 tipos): ids 43..71 e 118..123 em loop (`0x40b8b0`, `0x49b700`); `0` = sem som. Tabela: `rt2_audio::events::BUILDING_LOOPS`.
- **Ambiente aleatório** (`0x406470`, no máximo a cada ~65 s e quando o tempo desde o último teste passa de `rand(0,300)` s): tela 1 (mapa): poleiro (id 98, volume `DAT_00574ba8`) em certas condições de zoom, África (103, volume `DAT_00574bac`) se esse volume é não nulo, senão por nível de zoom (volumes 30..100) gavião (87) ou gansos (102, quando o campo de mês do jogo está em 3..5 ou 9..11) ou voo rasante (124, se `DAT_00574bb0`); telas 4, 5, 10: `tr_in` (76, volume 90); tela 0xc: pombos (105, volume 25..100 pela posição).

## 2. Música

### 2.1 Arquivos e motor

`data2\*.mp3` (a pasta `data2` também guarda o runtime Miles em `Miles\`: `Mp3dec.asi`, filtros, drivers 3D). O jogo usa **Miles Sound System (mss32.dll)**: `0x4f56f0` faz `AIL_startup` e `AIL_quick_startup` (44100 Hz, 16 bit, estéreo) e abre cada faixa com `AIL_open_stream` (`0x4f5a80`), com callback de fim (`0x4f5a70`). Há ainda uma referência a `%smusic\track03.wav` (faixa de CD em `game_init_paths`, `0x465050`), sem uso no instalador de disco. O volume de música das opções (`+0x30` da estrutura de opções `0x57b400`) em 0 desliga a música e fecha o Miles (`0x4f5860`).

### 2.2 Regras de seleção

Tabela de faixas `music_track_table` em `0x55d578`: **60 registros de 0x44 bytes** (habilitada `+0`, loop `+1`, categoria `+0x11`, rank `i32` `+0x13`, volume `i32` `+0x17` = 0x41).

1. **Varredura** (`0x4f5490`): `data2\*.mp3`, na ordem da listagem do Windows (alfabética sem distinção de caixa); nomes com mais de 40 caracteres são ignorados. Todas começam habilitadas, categoria 1. O nome **menor byte a byte** (`1AIntro.mp3`) vira **categoria 0**; o primeiro que começa com `C` vira **categoria 2** e o primeiro com `W` **categoria 3**; as três ficam em **loop** (nesta ordem; a última atribuição vence). Na instalação: 0 = `1AIntro.mp3`, 2 = `Chi-town Blues.mp3`, 3 = `When am I goin' Home.mp3`, as demais faixas na categoria 1.
2. **Ranks iniciais** (`0x4f56f0`): `rand(0,1000)` por faixa; o contador de rank (`0x55e578`) começa em 2000.
3. **Pedido de categoria** (`music_request_category`, `0x4f6130`): é a única interface do resto do jogo. Valores vistos: `0` no menu principal (`game_main_loop` `0x464f60`), `1` ao entrar nas telas de jogo (`0x40de20`, `0x411070`, `0x479b20`, `0x479d80`, `0x4b75b0`), `-2` = manter a categoria atual (1 se não há). Categorias 2 e 3 não têm chamador nas funções lidas. Se a categoria pedida já está em vigor só se guarda o pedido. Senão escolhe a **faixa de maior rank da categoria** (a mais recente: voltar a uma categoria **retoma a faixa em que ela estava**) ou, se nenhuma tem rank, sorteia com `0x4f5ca0`.
4. **Troca** (`music_fade_tick`, `0x4f5f30`): com música tocando, a faixa atual faz **fade-out de 1300 ms** (`0x514`; volume = restante·100/1300), então a nova começa com **fade-in de 500 ms**; sem música começa na hora. A tela de combate/cenário (`0x40de20` caso `0xc`) para o stream (`0x4f5bd0`); cargas de tela suspendem (`0x4f6230`) e retomam (`0x4f62e0`, contador de aninhamento) a categoria pedida.
5. **Sorteio** (`music_pick_random`, `0x4f5ca0`): candidatas = faixas habilitadas da categoria (se não há nenhuma, todas as habilitadas); descarta as `min(n-1, 3)` de **maior rank** (as últimas tocadas) e sorteia uma das restantes com `crt_rand_range`; grava `rank = contador` e soma 1000. Resultado: nunca repete as 3 últimas faixas da categoria.
6. **Fim de faixa** (`0x4f5de0`, callback do Miles): se a faixa tem loop, recomeça; senão chama o sorteio para a categoria atual.

### 2.3 No clone (`rt2_audio::music`, `rt2_audio::Audio`)

`MusicDirector` implementa as regras acima sem dispositivo de áudio (testado) e devolve comandos (`FadeOut`, `Play`, `Stop`) que `Audio` executa com kira (`StreamingSoundData`, MP3 via symphonia). Opções modernas: volume mestre, de música e de efeitos, `music_skip()`, ordem `PlayOrder::Sequential` (próxima faixa da categoria em ordem de nome) em vez do sorteio do original, e `music_play_file`. `Audio::update()` deve ser chamado uma vez por quadro.

## 3. Vídeos Smacker (`anim\*.SMK`)

104 arquivos, ~218 MB, 640x480, vídeo `smackvideo` a ~12 fps e áudio `smackaudio` mono 22050 Hz (`INTRO_01.SMK` dura 87 s). O jogo usa `SmackOpen` (smackw32.dll) em `video_open` (`0x5043c0`): abre `anim\<nome>.SMK` na pasta do jogo e, se falta, no CD (`%1anim\%2.SMK`). Os nomes ficam em `video_table` (`0x55fed8`, 9 bytes por registro: ponteiro para o nome + 5 bytes de flags não decodificados), índices 1..104: `br_01..09`, `scenwin`, `scenlose`, `intro_01`, `cv01..18`, `cvwin`, `ca01..18`, `cabrok01..06`, `cagetst2`, `calookle`, `calookri`, `caloss_1`, `caloss_2`, `casnorea`, `casnores`, `caepopto`, `cve01..18`, `cvewin`, `cae01..18`, `cawait1..4` (`rt2_audio::video::animation_name`). `0x464bb0` só testa se `cv07`, `cve07`, `ca01` e `cae14` existem para saber que conjuntos de vídeo estão instalados.

No clone: `rt2_audio::video::ensure_converted` converte com o `ffmpeg` da linha de comando (H.264 + AAC, `-crf 20`; cai para MPEG-4 se faltar libx264) para `~/.cache/rrt2/video/<nome>.mp4` (`$XDG_CACHE_HOME`) na primeira vez, de forma atômica, e devolve o caminho; a reprodução do MP4 é da camada gráfica (`rt2-play --video NOME --show` abre com `ffplay` ou `mpv`). Converter o `INTRO_01` leva ~2 s.

## 4. Lacunas

- Fórmula do volume em centibels (`0x4ed320`) e da distância ao centro da vista (`0x49b610`, `0x4f0ca0`): só os argumentos float se perderam na decompilação.
- Quando as categorias de música 2 e 3 (e o loop de `Chi-town Blues` / `When am I goin' Home`) são pedidas: nenhum chamador de `0x4f6130` passa 2 ou 3 nas funções lidas.
- Quem escolhe os ids vindos de campos de controle (`.win`): os ids de `swtch`, `adds2`, `noteb`, `whtab`, `ANNCL`, `LOCK*` por controle não foram seguidos até o layout.
- Significado dos 5 bytes de flags de `video_table` e qual tela dispara cada vídeo além de `intro_01`, `scenwin`/`scenlose`.
