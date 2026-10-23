# Formato MAP / GAM (mapas de cenário e saves) — primeira passada

Escopo: `maps/*.MAP` (31 arquivos; os `*.MP2` do Platinum parecem ser o mesmo formato, não analisados) e `games/*.GAM` (Tutorial, Example save game). Tudo little-endian, **sem compressão** (entropia ~3 bits/byte, regiões inteiras de zeros). Ferramenta: `re/scripts/mapgam.py` (numpy + Pillow). A primeira passada (seções 3 e 4) veio só dos dados; a segunda (seções 1, 2, 5 a 8) vem das rotinas de carga/gravação do EXE: carregador `game_load_file` `0x477a60` e gravador `game_save_file` `0x477540` (o mundo é lido para os globais a partir de `0x567b38`).

PNGs de prova (scratchpad, não versionados), pasta `/tmp/claude-1000/-home-pedryvo-Decomp-railroad/6d7cf073-1b4e-458b-98c6-4163984526ea/scratchpad/maps/`:
`t_britain.png` (Grã-Bretanha/Irlanda reconhecíveis), `t_bo.png` (Chesapeake e Delaware Bay), `elev_all.png` (os 31 mapas: Alpes/Itália, Índia, Japão, Austrália, África, América do Sul, Escandinávia, América do Norte), `bo_sheet.png` (cada coluna do registro de tile da B&O).

## 1. Versões e cabeçalho

Todos os 33 `.MAP`/`.GAM` do jogo têm a versão `0x233C` no primeiro u32 (`DAT_00567b38`). O carregador aceita versões até `0x2348` e converte as antigas (muitos `if (versao < N)`), e o gravador **sempre grava `0x2348`**. Os 95 `.MP2` do Platinum e qualquer save gravado pelo jogo são `0x2348` (seção 1.1); `Save::parse` lê as duas versões. Tudo abaixo é a versão `0x233C` salvo indicação.

O arquivo é a imagem de blocos de globais, na ordem (confiança alta, vem do código):

| Offset (0x233C) | Tamanho | Destino (global) | Conteúdo |
|---|---|---|---|
| 0x0000 | 4 | `0x567b38` | versão (`0x233C`) |
| 0x0004 | 0x878F | `0x567b3c..` | bloco de cabeçalho (ver abaixo); endereço = `0x567b38 + offset` |
| 0x8793 | 0x86 | `0x566d00` | bloco extra (só versão > `0x2334`), não decodificado |
| 0x8819 | 0x40 | `0x577b98` | vista/câmera: 16 dwords (`0x577b98..0x577bd8`) passados à janela `0xfd` (posição/zoom) e a `0x467bd0`/`0x467fd0` |
| 0x8859 | 0x29C8 | `game_running` `0x5704e8` (via conversão) | **estado global do jogo** (data, hora, velocidade, ...) |
| 0xB221 | 0xB6 | `0x574840` | bloco com `game_seed` (`+0x2e` = `0x57486e`, arquivo 0xB24F) |
| 0xB2D7 | 10*w*h | `DAT_00574c24` | tiles (10 bytes no arquivo, 15 na memória: 5 bytes de estado de execução zerados no load) |
| depois | 2*(w+1)*(h+1) | `DAT_00574c28` | elevação (i16; 3 bytes por canto na memória) |
| depois | 15 pools | ver seção 5 | entidades |
| depois | extras dos trens | | consist e rota de cada trem (seção 5) |
| depois | u32 n + n regras | pool `0x574c9c` | regras de cenário/vitória |

O último byte da última regra é o fim do arquivo (verificado nos 33 arquivos pelo teste `every_map_and_save_parses_with_state_to_the_last_byte`). O gravador `0x477540` ainda escreve um checksum no buffer em `+0x86f7` (arquivos `0x2348`).

**Bloco de cabeçalho** (endereço de memória = `0x567b38 + offset do arquivo`, confiança média salvo indicação):

| Offset | Conteúdo |
|---|---|
| 0x004 | nome curto (cópia em `0x23b` para versões < `0x2339`) |
| 0x131 | título do cenário (médio) |
| 0x231 | u16 largura, u16 altura (o gravador copia de `0x55e7a4`/`0x55e7a0`; alta) |
| 0x235..0x238 | 4 flags (valem 1 para versões < `0x233c`) |
| 0x25a | descrição (texto livre) |
| 0x387/0x390 | **preview** 170x170 bytes (`0x70E4`) gerado por `0x477040` ao salvar; termina em **0x7474** (alta: bate com a 1ª passada) |
| 0x7474..0x8793 | restante do bloco (opções, tabelas por tipo); em maps é quase só zeros. Inclui **53 registros de 9 bytes** em `0x5702cb` (arquivo 0x2793..0x297c): u8 ligado (1), u32 = 100, u32 = 2 por padrão, indexados por tipo de indústria (`0x433370`, `0x433550`: testam o byte 0 e pulam o tipo se for 0) |

O "bloco quase todo zero até 0xB2D6" da primeira passada **não é um bloco só**: é o fim do cabeçalho, o bloco `0x86`, a vista, o **estado global** e o bloco do `game_seed`.

### 1.1 Diferenças 0x233C vs 0x2348 (alta; verificado em 95 `.MP2` e em 3 saves gerados pelo oráculo)

As versões `0x2341..0x2348` têm os mesmos tamanhos de bloco de nível alto; as diferenças vêm dos `if (versao < N)` do carregador `0x477a60`. Todos os `.MP2` são `0x2348`; `Save::parse` os consome até o último byte e o checksum confere nos 95. As amostras do oráculo (Tutorial avançado 0 e 1 dia, Example avançado 1 dia, por `re/scripts/wine/advance_save.py`; não versionadas) também.

| Item | 0x233C | 0x2348 |
|---|---|---|
| bloco de cabeçalho (após o u32 de versão) | 0x878F | **0x898A** (<0x2341 o loader converte: as 38 entradas de 9 bytes de `0x5702cb` viram 53) |
| bloco 0x86, vista 0x40 | iguais | iguais |
| estado global | 0x29C8 | **0x3A74** (o início, incluindo data/hora/velocidade, tem os mesmos offsets) |
| bloco `game_seed` (0xB6) | igual, `game_seed` em +0x2E | igual |
| início dos tiles | 0xB2D7 | **0xC57E** (`4 + 0x898A + 0x86 + 0x40 + 0x3A74 + 0xB6`) |
| estado em | 0x8859 | 0x8A54 |
| tile 10 bytes, elevação i16 | iguais | iguais |
| cabeçalho de cada pool | igual | igual (o tamanho de registro gravado no arquivo é o novo) |
| tamanho de registro das pools no arquivo | estações 0x39B, pessoas 0x4DC, empresas 0x196C, pool `c7c` 0x11, trens 0x143, lugares 0x96, territórios 0x27A | estações **0x4C7**, pessoas **0x576**, empresas **0x4E86**, `c7c` **0x15**, trens **0x14D**, lugares **0xB4**, territórios **0x314** |
| demais pools (`c54`, `c70`, `c74`, `c78`, `c58`, `c80`, `c84`, `c88`) | | mesmos tamanhos (7, 0x4F, 0x9F, 0x23, 0xE, 5, 0x24, 7) |
| carro de trem (extra) | 10 bytes | **14 bytes** (= o registro da memória) |
| passo de rota | u16, u16, u8, u8, u8 = 7 | u16, u16, u8, u8, **u16** = 8 (o último array foi alargado em 0x2348; o loader zera e recalcula para <0x2348) |
| parte fixa da regra (`rule_read` `0x457c90`) | 0x2C8 | **0x362**; contagem de tokens continua em +4; o resto (texto + arrays + 4 textos) é igual |
| checksum | campo em +0x86F7 é 0 | u32 em **+0x86F7**, soma sobre o arquivo todo com esse campo zerado, algoritmo `0x4ef4d0` (`rt2_formats::map::checksum`) |

Os campos já decodificados mantêm os offsets nos registros maiores (os conversores só inserem campos novos depois deles: nome da estação em +26, nome do trem em +12, lugar em +4..+54, empresa +4/+0x148, pessoa +8/+0x27), então `place_list`, `company_list`, `station_names` etc. funcionam nas duas versões.

Pendências (as conversões (1) e (2) estão resolvidas na seção 9): (1) os bytes de tile de `0x2348` não passam pelo remapeamento de terreno (`<0x233d` e `<0x2342` usam as tabelas `0x552b00`/`0x552cd4`), então os ids de `ground` dos `.MP2` podem não ser os mesmos dos `.MAP` (o teste `is_water` usa os ids de `0x233C`); (2) os campos novos dos registros de `0x2348` (estação +0x39B.., empresa +0x4DEC.., pessoa +0x4DC.., trem +0x143.., lugar +0x96.., território +0x27A..) ficam crus; (3) a versão intermediária `0x2341..0x2347` não é aceita (nenhum arquivo do jogo a usa).

## 2. Mapa de seções do arquivo (alta)

```
0x00000  versão + bloco de cabeçalho (0x8793)      0x08793  bloco 0x86      0x08819  vista (0x40)
0x08859  estado global (0x29C8)                    0x0B221  bloco game_seed (0xB6)
0x0B2D7  TILES: w*h x 10 bytes                     fim      ELEVAÇÃO (w+1)*(h+1) x i16
fim_elev POOLS (15), extras de trens, regras       (ver seção 5)
```

Offsets por arquivo: `mapgam.py info`. O início fixo `0xB2D7` vale para os 33 arquivos (cabeçalho de tamanho fixo).

## 3. Registro de tile (10 bytes) — confirmado visualmente na B&O e Britain

| Byte | Campo (nome provisório) | Observado | Conf. |
|---|---|---|---|
| 0 | `var` | **bits 0-5 do u16 de terreno**: bit 0 = rio, bits 1-2 = direção do rio, bit 3 = ?, bits 4-5 = giros da textura (`>> 4 & 3`); bits 6-7 = 2 bits baixos do id de terreno (seção 3.2) | alta |
| 1 | `ground` | **8 bits altos do id de terreno de 10 bits** (id = `ground << 2 | var >> 6`, seção 3.2); na versão `0x233C` é o id antigo (0x00..0x62). **0x61 e 0x62 = água** (oceano/lago/baía; mascara Chesapeake, Delaware Bay e o mar da Grã-Bretanha). 0x00 contorna a costa (tiles de borda). Ex.: 0x2d, 0x36, 0x24, 0x12, 0x1b = vegetação/solo; 0x5b/0x5c/0x3f = rocha/montanha (altitude média 225–347) | alta (água), média (resto) |
| 2 | `flags2` | bits 0-3 = classe de vegetação; bit 6 (0x40) = derivado: o tile tem base de transição ou overlays (recalculado por `0x4258e0`); bit 4 = tem prédio, bit 5 = limpo, bit 7 = ponte (`terrain-track.md` 1.1) | média |
| 3 | `trees` | bits 0-4 = 0..28, densidade de árvores (nenhum em água); **bit 5 (0x20) = estrada** (overlays de `road.imb`, seção 3.2) | média |
| 4 | `struct_id` | 0 ou id único crescente (B&O: ~73 ids, agrupados em (55..77, 50..72) = Baltimore) = índice no pool de estruturas `0x574c54` (confirmado: nº de ids distintos == nº de registros) | alta |
| 5 | — | sempre 0 | alta |
| 6 | `flags6` | bits 3 e 4 sempre ligados na água; bit 2 nunca na água; **bit 7 = o tile tem overlays** (derivado, `0x4258e0`); demais: `0x424c10` recalcula bits 2-4 (terreno não construível/borda) | média |
| 7 | `track_id` | 0 ou id único; na B&O forma um traçado contínuo (78,47)→(73,52)→(60,53) = índice no pool de peças de trilho `0x574c84` = **trilho pré-construído** (confirmado: ids distintos == registros) | alta |
| 8 | — | sempre 0 | alta |
| 9 | `region` | 1..N (B&O: 1..5, até 31 no Wheel/…): territórios (blobs retangulares/arredondados em `bo_sheet.png`: Harpers Ferry, Baltimore, Washington, Filadélfia) | alta |

Observação: na primeira posição (0xB2D6) há um byte 0 antes do primeiro registro; com a fase certa as colunas 5 e 8 são zero em 100% dos tiles.

### 3.2 Id de terreno, classes e overlays (decodificado, validado contra o original)

O tile na memória tem **15 bytes**: os 10 do arquivo, um `u32` ponteiro (`+10`) para a lista de **overlays** do tile (nós de 8 bytes) e 1 byte. `tile[0..2]` (u16) = `id << 6 | bits baixos`; **id de terreno** = `u16 >> 6` (10 bits, 0..612). Um id dá uma **classe** (`0x581090[id]`, 0..18, 18 = água) e um **registro** de `ground.imb` (`0x5804ac[id]`; registros >= 221 são as 4 variantes do oceano, `groceana.imb`). As duas tabelas são montadas em `0x471b10` a partir dos 19 descritores de classe de `0x540aa0` (passo 0xc0); o algoritmo exato, reimplementado em `rt2_formats::terrain::Tables` e conferido com as tabelas despejadas do original, está em `sprite-catalog.md` 4.1.

**Versões antigas (`0x233C`)**: o id do arquivo é o id **antigo**; o carregador (`0x477a60`) o converte: classe = `0x552b00[id]` (corridas de 27, 22, 22, 37, 34, 37, 37, 35, 21, 31, 31, 31, 24 e 18 ids para as classes 0..12 e 13; a 13 vira 18 = água), id novo = primeiro id da classe, e então `0x42a3e0` sorteia a variante (grupo comum `n0`; o grupo raro `n1` quando `(y*0x41c64e6d - x*0x39b19300) % 40 == 1`) e zera os giros (bits 4-5). Ids antigos de transição (costa etc.) **não** são lidos: as transições são recalculadas. Água antiga = ids 389..406 (`ground` 0x61 com `var>>6 != 0` e 0x62..0x65); o id 388 (`ground` 0x61, `var>>6 == 0`) é a última montanha. Versões `0x233D..0x2341` usam `0x552cd4` (não suportadas por `Map::parse`); `0x2342+` já usam ids novos. Em `0x2348` os ids vêm prontos, mas o carregador também reduz qualquer id fora do intervalo próprio da classe (transições) a uma variante comum e recalcula.

**Passes do carregador** (nesta ordem, depois de `0x4cb090`): rios `0x4294d0(0,0,W,H)`, estradas `0x428fe0(0,0,W,H)` e transições `0x42a7d0(0,0,W-1,H-1)` (os dois últimos argumentos são **tamanho**: a última coluna e linha ficam de fora). Cada um monta **nós de overlay** (`u16 word = registro << 6 | banco << 3 | camada`, `u8 giros`, `i8 sombra`; os 8 bytes são `word`, giros, sombra, próximo): camada 0 = transição de solo/costa, 1 = fronteira de território (`0x42bf90`), 2 = rio, 3 = estrada; trilho usa camada 4/5 com banco 4/5 (`tracks`/`trackd`). Banco = índice na tabela `0x580fb0`: 0 `ground`, 1 `territor`, 2 `riveran`, 3 `road`, 4 `tracks`, 5 `trackd`, 6 groceana, 7 grbeacha. A lista fica ordenada por camada decrescente (inserção antes do primeiro nó de camada <= à nova). **Desenho** (`0x511a90`): os overlays são desenhados na ordem da lista, o **primeiro que pinta um pixel vence** (pixel de índice 0 = transparente), e a textura base por último; giros do nó (+ rotação da visão) escolhem a orientação como na base; `sombra` soma ao nível de paleta. Animação: rio = registro + `(contador % 5) * 18` em `riveran.imb`; registros 5..12 de `ground.imb` (peças de margem) viram `(registro - 5) + (contador & 31) * 8` em `grbeacha.imb`; oceano `registro + 4 * (contador & 15)`.

`0x42a7d0` por tile: limpa os nós de camada 0 (`0x4259e0`; se o id base não é de classe própria, sorteia uma variante comum com o RNG do jogo), depois, para cada classe vizinha `j` (0..18), exceto: `j == classe`, `j == 18`, `j == 0` salvo se o tile é água, e tile de classe 9, 10, 11, 17 (campos: sem overlays). **Modo A** (donos != 0, `j` não é campo): examina N, NO, O (`(x,y-1)`, `(x-1,y-1)`, `(x-1,y)`) por classe `j`; a máscara de 3 bits dá tipo (0 canto, 1 lado, 2 cheio) e giro; se a classe do tile tem transição com `j` (`transition_start != 0`) e o tile ainda não tem overlays, o **id base é trocado** pelo id de transição (`transition_start[c][j] + peça`, giros nos bits 4-5); senão cria um nó com a peça `S2[j] + tipo` (o tipo cheio troca a base para o filler `S4 + j` e usa `S2[c]`). Máscara 5 (N+O) aplica duas vezes. **Modo B** (tile de praia classe 0, ou `j` campo): examina os 8 vizinhos, pega corridas de bits com `0x42a640` (tipo 0 canto, 1 lado, 2 três lados, 3 sete lados, 4 anel completo; giro) e cria nós: em tile de praia a peça `S2[j] + tipo` (tipo 2: base := filler `S4 + j`); em tile de **água** vizinho da praia (`j = 0`) as **peças de margem** `S3[0] + tipo` (+5 em metade dos tiles por `(y*0x6d + x*0x17) & 0xff < 0x80`), registros 5..12. Rios (`0x4294d0`, bit 0 do tile): máscara de 4 vizinhos com rio (N, L, S, O; borda do mapa conta), direção `tile[0] >> 1 & 3`, peça/giro de `0x53a25c`, registro de `0x53a2dc` com 6 variantes por `rng_hash_range(x*7 + y*19, 0, 5)`; em água, foz com o registro 7 ou 8 conforme o vizinho de destino. Estradas (`0x428fe0`, bit 5 de `tile[3]`): máscara de vizinhos, peça de `0x53a13c`/`0x53a15c`, registro de `0x53a314` (9 variantes), duas passadas. Tabelas e portagem em `crates/rt2-formats/src/terrain.rs`.

Validação: o porte em Rust foi comparado tile a tile (ids, bits derivados, lista de nós) com a execução das funções do original no emulador (Unicorn) sobre os 33 `.MAP`/`.GAM` e os `.MP2`: 0 tiles diferentes (com o mesmo RNG, inclusive o estado final). O Example avançado 1 dia no original (`0x2348`) coincide com a conversão em 99,7% dos tiles; o resto são variantes aleatórias do grupo raro. Teste: `crates/rt2-formats/tests/terrain.rs` (com `RT2_DIR`; os `.MP2` guardam as transições que o editor gerou: 97%+ reproduzidas).

## 4. Elevação (alta)

`i16` com dimensão `(w+1)*(h+1)` (altura nos cantos; verificado por suavidade: menor diferença média entre vizinhos com stride `w+1`). Valores pequenos negativos (65535 = -1, -2) aparecem no fundo do mar (Britain, Silésia…): **ler como i16**. O nível do mar varia por mapa (B&O ~21–50, Britain 0; máx típico 120–510). Para o renderizador: altura do tile = média dos 4 cantos; água = `ground in (0x61,0x62)`.

## 5. Estado global, pools de entidades e regras

### 5.1 Estado global (0x29C8 bytes em 0x8859; alta para os campos listados)

O carregador copia este bloco para o layout novo (`game_running`, `0x5704e8`) deslocando trechos (+0xC a partir de `0xdf`, +0x18 a partir de `0x1ce`, etc.). Para os campos no início do bloco (`< 0xdf`) o deslocamento é zero, então **o offset no bloco é o offset a partir de `0x5704e8`**:

| Offset no bloco | Arquivo | Global | Conteúdo |
|---|---|---|---|
| +0x00 | 0x8859 | `game_running` `0x5704e8` | u32 (1 nos saves/maps) |
| +0x0C | 0x8865 | `game_time_minutes` `0x5704f4` | i32 minutos absolutos desde o ano 0 (`ano*483840` em 1/jan 00:00) |
| +0x10 | 0x8869 | `game_date` `0x5704f8` | u16 ano, u8 mês 1..12, u8 dia 1..28 |
| +0x14 | 0x886D | `game_time_of_day` `0x5704fc` | u8 hora, u8 minuto |
| +0x26 | 0x887F | `game_speed` `0x57050e` | u8 índice 0..10 (0 em maps, 3 nos saves) |
| +0x18..+0x29 | | | cópias da data (início do cenário? `1870-01-01` em Britain; datas de início/fim do jogo) |

Verificado: Tutorial = 1870-02-04 19:03 (minutos 904 826 583), Example = 1875-12-10 05:21 (907 656 801), maps começam em 1/jan 00:00 e o `minutos` bate com `date_to_minutes` (teste). O resto do bloco (tabelas de preços, opções, jogador atual, ...) está guardado cru em `Save::state` e **ainda não está decodificado**.

### 5.2 `game_seed` e RNG (alta)

- `game_seed` (`0x57486e`) = u32 no arquivo em **0xB24F** (versão >= `0x2334`). Valores: 183274 (Britain, Tutorial e Example), 16451 (B&O).
- **O estado do RNG (`0x55d2c0`) não é gravado em nenhum bloco** (nenhum bloco serializado o cobre). No load o carregador faz `s = rng_get_state(); gerar_mundo(); rng_seed(s)` (`0x477c75..0x477c8f`), ou seja o `.GAM` não restaura a sequência do jogo: ela é a que o processo já tinha. Para o teste "save como oráculo" o clone e o original precisam ser re-semeados explicitamente (por exemplo no Wine, via `rng_seed` no mesmo ponto) logo após o load; a semente do arquivo só alimenta `rng_hash_range` (terreno/decoração) e a re-semeadura diária **em rede** (`game_time_minutes + game_seed`).

### 5.3 Pools de slots (formato, alta)

Todas as entidades vivem em "pools" de registros de tamanho fixo com índice (slot) >= 1, bitmap de livres e, às vezes, lista ligada. Formato no arquivo (`pool_read` `0x4f1bc0`, `pool_write` `0x4f1a40`), little-endian:

```
i32 n            ; número de slots
i32 tam          ; tamanho do registro no arquivo
i32 ligada       ; 1 = tem arrays next/prev
i32 +0x00        ; capacidade inicial
i32 +0x04        ; maior slot usado
i32 +0x0c        ; passo de crescimento
i32 contagem     ; registros vivos (+0x14)
i32 tipo         ; id do pool (+0x18)
u16 head, u16 tail                    ; só faz sentido se ligada
u8  livre[(n+8)>>3]                   ; bit i = 1 -> slot i livre
registro[tam]  para cada slot i com bit 0, em ordem de slot
u16 next[n], u16 prev[n]              ; só se ligada
```

Os ids guardados em outras estruturas são o número do slot. O jogo itera os pools ligados na ordem da lista (`0x4f19e0`), os outros por slot (`0x4f1e30`).

Ordem no arquivo, global do pool, tipo, tamanho (arquivo antigo / memória), uso. Confiança: nomes de campos e contagens altos quando dizem "alta".

| # | Pool | Tipo | Tam | Conteúdo |
|---|---|---|---|---|
| 1 | `0x574c54` | 2 | 7 | **estruturas**; `tile.struct_id` indexa o slot (alta: nº de ids distintos == nº de registros em B&O, teste). Byte 0/1 = tipo/variante (p.ex. `03 17`), resto 0 nos maps |
| 2 | `0x574c60` | 5 | 0x39B / 0x4C7 | **estações** (Oxford, Slough, London no Example; vazio em maps e Tutorial). Nome char[32] em +26; +4 = id de objeto `c74`, +8 = tipo 0/1/2, +0xa/+0xe = x,y i32 |
| 3 | `0x574c68` | 3 | 0x4DC / 0x576 | **pessoas/personas**: id u32 +0, nome em +8, biografia em +0x27 |
| 4 | `0x574c6c` | 4 | 0x196C / 0x4E86 | **empresas**: id u32 +0, nome char[32] em +4; +0x2c u32 (10000, 40000, 20000, 80000...), +0x134/+0x138 (10000/12000; 23000/15244...), +0x148 ano (1870) |
| 5 | `0x574c70` | 7 | 0x4F | desconhecido (51 no Example) |
| 6 | `0x574c74` | 8 | 0x9F | **objetos de mapa** (387 no Tutorial, 13 em B&O, 0 em Britain.MAP); +7 flag; se 0, +8 u16 = estação (`c60`), senão x,y i16 em +0x22/+0x24; +0x9d = região do tile; +0x52 tipo (4->3 na conversão `<0x2338`). Função exata incerta (indústrias?) |
| 7 | `0x574c78` | 9 | 0x23 | desconhecido (99 no Example) |
| 8 | `0x574c58` | 10 | 0xE | desconhecido (72 no Example, 11 em B&O) |
| 9 | `0x574c7c` | 11 | 0x11 | desconhecido (150 no Tutorial, 331 no Example) |
| 10 | `0x574c80` | 12 | 5 | desconhecido (33 no Example) |
| 11 | `0x574c84` | 13 | 0x24 | **peças de trilho pré-construídas**; `tile.track_id` indexa o slot (alta: 184 ids == 184 registros no Example, 21 em B&O, teste) |
| 12 | `0x574c64` | 1 | 0x143 | **trens** (lista ligada): nome char[32] em +12; +0x3b u8 = nº de carros; +0x59 i32 = nº de passos da rota, +0x65 i32 = flag da rota (ver extras) |
| 13 | `0x574c5c` | 14 | 0x96 | **lugares** (cidades, regiões, mares): ver 5.4 |
| 14 | `0x574ca8` | 24 | 0x27A | **territórios**: nome em +4; +0x24 = flag, +0x2a = região |
| 15 | `0x574c88` | 15 | 7 | deslocamentos (i8 x, i8 y, 3 bytes) — 10 em B&O, 275 em Tutorial/Example; função desconhecida |

### 5.4 Lugares (0x96 bytes; alta nos campos listados)

| Offset | Tipo | Campo |
|---|---|---|
| +0 | u16 | slot próprio |
| +2 | u16 | contador/valor (380 London no Tutorial, 406 no Example: provavelmente população) — baixa |
| +4, +6 | u16 | x, y do centro (tiles) |
| +8, +10, +12, +14 | u16 | caixa x0, y0, x1, y1 |
| +22 | char[32] | nome |
| +0x36.. | | demanda/oferta, produção, atributos: não decodificado (`0x4a` em diante: u16 por carga limitado a 200 na conversão `<0x2334`; +0x11 tipo 4->3) |

Exemplos: Londres em (260,237) caixa (249,226)-(271,248); Baltimore em (72,53) caixa (65,46)-(79,60). O pool tem 103 lugares em Britain/Tutorial/Example e 20 em B&O.

### 5.5 Extras dos trens

Depois do 15º pool, para cada trem na ordem da lista ligada: `cars = trem[0x3b]` registros de **10 bytes** (14 na memória), e, se `trem[+0x59] = n > 0` e `trem[+0x65] != 0`: arrays `u16[n]`, `u16[n]`, `u8[n]`, `u8[n]`, `u8[n]` (rota; no formato `0x2348` o último vira `u16[n]`). Guardados crus em `TrainExtra`.

### 5.6 Regras de cenário / vitória (pool `0x574c9c`)

`u32 n`, depois `n` vezes (`rule_read` `0x457c90`, versão `< 0x2341`): 0x2C8 bytes fixos (ponteiros de processo sem valor, `+4` = nº de tokens `t`, flags), `u32 len + bytes` com o **texto da condição** (`GameYear<=1925 and CompanyBookValue>=70000000 and ...`), `i32[t]` e `i32[t]` (condição compilada: tokens e valores), e quatro textos `u32 len + bytes`: mensagem, dois extras (vazios nos maps), título. Os nomes (`GameYear`, `PlayerNetWorth`, ...) são interpretados pelo avaliador de expressões (não analisado).

## 6. .GAM vs .MAP

Ambos os `.GAM` derivam de **Britain.MAP** (mesmo `game_seed` 183274, 300x300), com o mesmo layout. Diferem em tiles/elevação (terraplenagem e trilhos), no estado (data) e em pools preenchidos: Tutorial tem 1435 estruturas, 387 objetos e nenhuma estação/trem; Example tem 10 estações, 9 trens, 5 empresas, 184 peças de trilho.

## 7. Perguntas abertas

1. ~~`var`, `flags2`, `flags6` e a tabela `ground`~~: resolvido na seção 3.2 (id de 10 bits, classes, overlays). Resta o bit 3 de `tile[0]` (desenha algo no modo de território, `0x511a90`) e a tabela `0x552cd4` das versões `0x233D..0x2341`.
2. Função dos pools 5, 7, 8, 9, 10, 15 e do pool 6 (objetos); campos de empresas e lugares além dos listados; o estado global além de data/velocidade.
3. Bloco `0x86` em 0x8793 e vista em 0x8819.
4. O estado do RNG **não** está no arquivo: como o original o semeia ao carregar (valor residual do processo), necessário para o teste de nível 2.
5. Campos novos dos registros `0x2348` (ver 1.1): significado desconhecido; é onde o diff campo a campo do oráculo deve começar.
6. Nível do mar e limite 510 de elevação.

## 8. Implementação

`crates/rt2-formats/src/map.rs`: `Map::parse` (terreno) e `Save::parse` (versões `0x233C` e `0x2348` via `Layout`; data/hora, `game_seed`, os 15 pools, extras e regras; visões `place_list`, `company_list`, `person_list`, `station_names`, `train_names`). Testes com `RT2_DIR` em `crates/rt2-formats/tests/original_data.rs`.

## 9. Conversão no load (`0x477a60`) de `0x233C` para `0x2348` (alta; verificado byte a byte)

O carregador lê cada bloco no tamanho da versão do arquivo e o expande na memória (sempre no layout `0x2348`); o gravador `0x477540` grava o layout da memória. Porte: `rt2_sim::convert` (`upgrade` = conversão dos registros; `SimState::load_passes` = passadas do load; `saver_refresh` = campos que o gravador recalcula), chamado por `World::from_save_seeded(save, data, rng)` para arquivos `< 0x2348`. Só as regras dos ramos `< 0x2341`..`< 0x2348` que valem para `0x233C` são portadas (`0x233D..0x2347` não existem na instalação).

**Verificação.** O original carrega o arquivo e grava com 0 dias (`advance_save.py <sb> <arq> 0 1234 <saida> --preseed 1234`: o RNG do jogo e o do CRT recebem 1234 *antes* da carga; o `.MAP` é copiado como `games/_advin.GAM`); o clone converte o mesmo arquivo com o mesmo RNG e grava (`rt2-advance <arq> 0 1234 <saida> --preseed 1234`). `rt2-savediff` (máscaras de 5.5): **0 diferenças nos 33 arquivos** (Tutorial, Example e os 31 `.MAP`), e o estado do RNG depois do load (`LOAD_RNG` no log) é igual em todos. Teste: `crates/rt2-sim/tests/convert.rs` (`RT2_DIR`, `RT2_CONVERT_DIR` com `index.txt` e `<nome>_d0.GAM`).

### 9.1 Blocos
- **Cabeçalho** (`< 0x2341`): lê `0x878F` bytes numa área zerada de `0x898A`; as 38 entradas de 9 bytes de `0x5700cd` (arquivo `0x8595`) são copiadas para `0x5702cb` (`0x8793`) e as 15 novas recebem (1, 100, 2); `0x5704b1..0x5704c6` zerados. Em todo load: byte 0 das entradas 49..52 = 1; `< 0x2340`: `0x570227..0x570229` = 0; e (fim do load) quatro entradas de 32 bytes em `0x570250` = `7F` + 9 zeros.
- **Estado** (`game_running`): três passos. `< 0x233F` (`0x29C8 -> 0x39CB`, buffer novo de malloc): cópia inteira, depois `[0xDF..+0xCF] -> 0xEB`, `[0x1CE..+0x2772] -> 0x1E6`, `[0x2729..+0x217] -> 0x3744`, zera `0x4C1..0x4C5`, `0xDD..0xE9`, `0x1DA..0x1E6`. `< 0x2341` (`-> 0x3A74`): `[0..0x111]`, 15 zeros, `[0x111..0x4C1] -> 0x120`, `0x9A` zeros, `[0x4C1..] -> 0x56A`. `< 0x2347`: `[0xAE..+0x3956] -> 0xDE` (desloca 0x30), `0x3A34 = [0x3A00]`, `0x3A26 &= 0xF7FFFFFF`, `0xDE = 0`. Os bytes do buffer intermediário que ninguém escreve (estado `0x2A31..0x381D` e `0x3A38..`) saem como lixo de heap: no oráculo (processo sob depurador, heap de depuração do Windows) o padrão `0xBAADF00D`, que o porte reproduz.
- **Tiles**: `< 0x233D` id antigo -> primeiro id da classe (`0x552b00`); `< 0x2343` `0x42a3e0` sorteia a variante (RNG). Os 5 bytes de execução zerados.

### 9.2 Registros das pools
O leitor de pool (`0x4f1bc0`) lê o tamanho antigo num array novo do tamanho novo: a cauda é lixo de heap (padrão `0xBAADF00D` com fase a partir do início do array, ou seja `(slot*tamanho + off) % 4`) até o conversor escrever.
| Pool | Conversão |
|---|---|
| estações `0x39B -> 0x4C7` (`< 0x233E`) | `+0x381..+0x390 -> +0x4AD`; `+0x61..+0x4AD` zerados; `< 0x2345`: u16 `+0x4BE` = 0; todo load: `+0x4AD` (u32) = região do tile (`+10`, `+0xE`) |
| pessoas `0x4DC -> 0x576` (`< 0x2341`) | `+0x48D..+0x4DC -> +0x527`, `+0x48D..+0x527` zerados |
| empresas `0x196C -> 0x4DEC` (`< 0x233E`) | `[0..0x3DF]` fica; 32 entradas de `0x49` em `+0x3DF` viram `0x1ED` (5 bytes copiados, resto zero); `[0xCFF..] -> 0x417F` |
| empresas `-> 0x4E86` (`< 0x2341`) | `[0x3D0..] -> 0x46A`, `0x9A` zeros |
| `c7c` `0x11 -> 0x15` (`< 0x2344`) | dois u16 zerados |
| trens `0x143 -> 0x14D` (`< 0x2346`) | `+0xFC = 0`, `+0xF3 = 1` (cauda = lixo); todo load: `+0xAB = 0` |
| lugares `0x96 -> 0xB4`, territórios `0x27A -> 0x314` (`< 0x2341`) | caudas zeradas |
| objetos `c74` (todo load) | `+0x42 = 0`; `+0x9D` = região da estação dona (`+7 == 0`, estação `+8`) ou do tile (`+0x22`, `+0x24`) |
| carros (extra) `10 -> 14` (`< 0x2347`) | byte 3 `&= 0xF7`, u16 `+10`/`+12` = 0; `< 0x233D`: nos 6 bytes de `+4`, `' '` -> `','` e `'!'` -> `'-'` |
| rota (extra, `< 0x2348`) | último array u8 -> u16 zerado; `train_route_refresh` `0x4d14b0` por trem com rota |
| regra (`0x457c90`, `< 0x2341`) | `0x2C8 -> 0x362`: dois blocos de `0x111` em `+0x64` viram `0x15E` (cauda zero), `+0x286..+0x2C8 -> +0x320`; o loader põe `+0x32E = 1` |

### 9.3 Passadas depois da leitura (todas as versões)
Na ordem: `0x4cb090` (trilhos, mapa todo), rios/estradas/chão (`0x4294d0`, `0x428fe0`, `0x42a7d0`; RNG). **Os sprites de trilho (camadas 4/5) já estão na lista do tile quando a passada do chão roda**: um tile com sobreposições recebe a mistura como nó em vez de trocar o id base (`terrain.rs` `blend`), por isso o porte injeta a presença dos trilhos (`Ai::track_overlay`) na grade antes das passadas. Depois: limites de vista `0x573d7e..` (todos 0 -> 9999, 9999, -1, -1); territórios `+0x2a` (oculto sem id válido -> 1, visível -> o próprio); anos `0x5700c5`/`0x5700c9` fora de 1800..2100 ou do lado errado de `0x5700c1` -> 0; bit 1 do byte 6 de cada tile = `station_at`; os 14 bytes do 1º carro do 1º trem -> `0x573f0e`; os **remendos fixos de seis cenários** (reconhecidos por tamanho do mapa, nº de jogadores/empresas, ano e regras: reescrevem condições de regras com textos do EXE `0x5598bc..0x559be0` via `rule_set_condition` `0x457a30`, ex.: #02 Hannibal, regra 7 e lugar 84); `< 0x2344`: por estação, `0x406f60` (desfaz os vínculos de captação em `pool_58`) e `station_rebuild_links` `0x4071b0` (refaz; o Example ganha 10 vínculos, 72 -> 82). Não salvos/sem efeito no arquivo: `0x4728f0` (UI), `0x42b310`, `0x471880`, janela `0xfd`. Nota: `station_networks_rebuild` `0x46a370` (também chamado pelo loader) não é chamado pelo porte; os 33 arquivos batem sem ele (o resultado já está gravado neles).

O invólucro do Load Game (`0x479710` -> `0x495d10` -> `0x495700`, também no New Game) ainda copia as opções de realismo do byte `0x574872` (bloco final `+0x32`) para `0x573d72..0x573d78` e redesenha as fronteiras de território (`0x42bf90`, `TerrainGrid::build_territory_borders`: camada/banco 1, peças de `0x42a640` para cada território vizinho de número maior e visível `+0x24`).

### 9.4 O gravador (`0x477540`)
Antes de gravar: para cada jogador da lista (`0x570775`/`0x570779`), `+0x54d = 0` se `+0x122 == 0` (senão um valor da sessão, mascarado); cabeçalho `0x567c88..0x567c9c` = (n, n, 1, humanos, n - humanos, n - 1); `0x567d69/6b` = tamanho do mapa; `0x570235 = 0`; `0x57022f` (campo do checksum) = 0; `0x570233 = (0x570234 != 0)`. `World::to_save` aplica isso (`convert::saver_refresh`).

### 9.5 RNG
A carga consome o RNG do jogo (variantes de terreno e passadas) e não o restaura; o CRT não é tocado. `World::from_save_seeded` recebe o estado do RNG no início da carga e deixa o mundo com o estado que a conversão deixou (igual ao `LOAD_RNG` do original). `World::from_save` usa 0. Arquivos `0x2348` não passam pelas passadas (o clone os carrega como estão; o original repetiria as passadas de terreno com RNG).
