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

Pendências: (1) os bytes de tile de `0x2348` não passam pelo remapeamento de terreno (`<0x233d` e `<0x2342` usam as tabelas `0x552b00`/`0x552cd4`), então os ids de `ground` dos `.MP2` podem não ser os mesmos dos `.MAP` (o teste `is_water` usa os ids de `0x233C`); (2) os campos novos dos registros de `0x2348` (estação +0x39B.., empresa +0x4DEC.., pessoa +0x4DC.., trem +0x143.., lugar +0x96.., território +0x27A..) ficam crus; (3) a versão intermediária `0x2341..0x2347` não é aceita (nenhum arquivo do jogo a usa).

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
| 0 | `var` | 0/0x40/0x80/0xC0 + nibble baixo ocasional; ruído, parece variante/rotação de textura | baixa |
| 1 | `ground` | id do tipo de chão (0x00..0x62). **0x61 e 0x62 = água** (oceano/lago/baía; mascara Chesapeake, Delaware Bay e o mar da Grã-Bretanha). 0x00 contorna a costa (tiles de borda). Ex.: 0x2d, 0x36, 0x24, 0x12, 0x1b = vegetação/solo; 0x5b/0x5c/0x3f = rocha/montanha (altitude média 225–347) | alta (água), média (resto) |
| 2 | `flags2` | bit 6 (0x40) em quase todos os tiles de terra e ~1/3 da água; bits 0..3 pequenos valores | baixa |
| 3 | `trees` | 0..28, espalhado em pontos pela terra (nenhum em água) = densidade/quantidade de árvores | média |
| 4 | `struct_id` | 0 ou id único crescente (B&O: ~73 ids, agrupados em (55..77, 50..72) = Baltimore) = índice no pool de estruturas `0x574c54` (confirmado: nº de ids distintos == nº de registros) | alta |
| 5 | — | sempre 0 | alta |
| 6 | `flags6` | bits 3 e 4 sempre ligados na água; bit 2 nunca na água; bit 7 / bit 5 variam. Provável: bitfield de borda/rio/costa | baixa |
| 7 | `track_id` | 0 ou id único; na B&O forma um traçado contínuo (78,47)→(73,52)→(60,53) = índice no pool de peças de trilho `0x574c84` = **trilho pré-construído** (confirmado: ids distintos == registros) | alta |
| 8 | — | sempre 0 | alta |
| 9 | `region` | 1..N (B&O: 1..5, até 31 no Wheel/…): territórios (blobs retangulares/arredondados em `bo_sheet.png`: Harpers Ferry, Baltimore, Washington, Filadélfia) | alta |

Observação: na primeira posição (0xB2D6) há um byte 0 antes do primeiro registro; com a fase certa as colunas 5 e 8 são zero em 100% dos tiles.

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

1. `var`, `flags2`, `flags6` e a tabela `ground` (id -> terreno): o loader só os copia (os 10 bytes vão direto para a memória), mas para versões `< 0x233d` remapeia os 10 bits altos de `tile[0..2]` (`ushort >> 6`, tabela `0x552b00`/`0x552cd4` -> `0x540aa4`): o u16 em `tile[0]` guarda **`ground` (10 bits, `>> 6`) e 6 bits baixos de flags**, o que sugere que `var`+`ground` formam um u16 (id de terreno em 10 bits). Falta confirmar o consumo.
2. Função dos pools 5, 7, 8, 9, 10, 15 e do pool 6 (objetos); campos de empresas e lugares além dos listados; o estado global além de data/velocidade.
3. Bloco `0x86` em 0x8793 e vista em 0x8819.
4. O estado do RNG **não** está no arquivo: como o original o semeia ao carregar (valor residual do processo), necessário para o teste de nível 2.
5. Campos novos dos registros `0x2348` (ver 1.1): significado desconhecido; é onde o diff campo a campo do oráculo deve começar.
6. Nível do mar e limite 510 de elevação.

## 8. Implementação

`crates/rt2-formats/src/map.rs`: `Map::parse` (terreno) e `Save::parse` (versões `0x233C` e `0x2348` via `Layout`; data/hora, `game_seed`, os 15 pools, extras e regras; visões `place_list`, `company_list`, `person_list`, `station_names`, `train_names`). Testes com `RT2_DIR` em `crates/rt2-formats/tests/original_data.rs`.
