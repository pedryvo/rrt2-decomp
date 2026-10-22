# Catálogo dos bancos de sprites (LAD)

Resultado de `rt2-sheet` (`crates/rt2-tools/src/bin/rt2-sheet.rs`, subcomandos `list`, `dump`, `stats`, `grid`, `sheet`) sobre os 4 PK2: **1987 bancos LAD** (RT2 1972, RT2_06 12, RT2_11PL 3; os 3 hashes repetidos entre RT2 e RT2_11PL contam uma vez), 30952 imagens. **1855 têm nome recuperado** (antes 116); os 132 restantes são telas, fotos e peças de interface. Formato do banco: `lad.md`. Versão legível por máquina: `re/names/sprite_catalog.tsv` (colunas `hash`, `name`, `category`, `notes`; linhas `#` são comentário; `name` = `-` quando o nome não foi achado). O teste `crates/rt2-formats/tests/sprite_catalog.rs` confere o TSV contra os PK2 (com `RT2_DIR`). Imagens do jogo não entram no repositório: as folhas de contato vivem fora dele.

## 1. Nomes recuperados

O nome do arquivo é montado com `sprintf`; o mesmo vale para a conta do hash (`pk2.md`). Formatos achados e a origem dos argumentos (todos confirmados: cada candidato gerado foi aceito só se o hash existe num PK2):

| Formato (endereço) | Argumentos | Bancos |
|---|---|---|
| `cr%s%s.imb` (`0x539d68`, `0x53a380`, funções `0x414150`, `0x414220`, `0x424e60`) | dígito `1..4`, nome de 5 letras de `vehicle_type_table` (`0x53ddb8`, +0) | 680 (170 nomes x 4) |
| `bl%s%s.imb` / `bn%s%s.imb` (`0x539ac4` / `0x539ab8`; `0x4093e0`, `0x424e60`) | dígito `1..4`, nome de 5 letras de `building_type_table` (`0x5457e8`, passo 0x154, 122 registros) | 863 |
| mesmo formato, estações | o nome `staNN` tem o 3o caractere trocado por uma letra de `"VCMTBK"` (`0x550474`) | 480 (so ocorrem V, C, M, T, K) |
| `%s%s.imb` (`0x53a368`) | nome da tabela de árvores `0x536450` (14 entradas, 13 bytes: `treoak_`, `trepine`, `scrub__`, `trepalm`, `trecact`, `trebrus`, `tremesq`, `trerfor`, `treswam`, `treprod`, `trerubb`, `trecoff`, `city__`), dígito `1..4` | 52 |
| `brlogo%s%s.imb`, `brplyr%s%s.imb`, `bigp%s%s.imb` (`0x4022c5`) | 2 dígitos `00..40` | 41 + 41 + 41 |
| `mousbw%02d.imb` (`0x56098c`) | `01..24` (faltam 20 e 21) | 22 |
| `ground.imb`, `tracks.imb`, `trackd.imb`, `road.imb`, `territor.imb`, `riveran.imb`/`rivernan.imb`, `groceana.imb`/`groceann.imb`, `grbeacha.imb`, `tem*.imb` (`0x471b10`) | literais | já conhecidos |

Os 1739 nomes novos foram acrescentados a `re/names/exe_filenames.txt` (só os que casam com um hash de PK2). Como `bn`/`bl` e `cr` são carregados por laço sobre os **4 slots de rotação de visão** (ver 2.3), o dígito é `rotação + 1`.

`bn` = versão **estática** (sem animação), `bl` = versão com animação (o jogo escolhe pelo campo `+0x70` das opções, `DAT_00574bc8`): o grupo 0 do `bl` carrega `N` imagens pequenas de animação (fumaça, rodas, luzes) com mapa de quadros (ex. `bl1steel.imb`: 1 imagem base + 95 quadros, mapa de 47 entradas); prédios sem animação só têm `bl`.

## 2. Geometria do mundo (alta, lida do código)

### 2.1 Projeção dos vértices (`0x419620`, rotação 0)
```
tela_x = (X - Y) * 64          tela_y = (X + Y) * 32 - 2 * elevação        (zoom máximo)
```
`X, Y` = coordenada do **vértice** (canto) na grade (a elevação `i16` de `map-gam.md` é por canto, 3 bytes por canto na memória; o primeiro `i16` é a altura). Em cada nível de zoom os dois valores são deslocados à direita por `s = 7 - zoom` (`0x421550`: `DAT_00580f38 = 7 - view.zoom`).

| Grandeza | Zoom máximo (`s = 0`) | Cada passo de zoom |
|---|---|---|
| Losango de um tile | **128 x 64 px** (cantos a `(±64, 32)` do centro) | metade (64x32, 32x16, ...) |
| Passo de tela de 1 tile em X ou Y | `(+-64, +32)` | idem, `>> s` |
| Degrau de elevação | **2 px por unidade** de elevação (`i16`, máx. 510 = 1020 px) | `2 >> s` |

Rotações de visão 0..3 (campo `+0x18` da visão, `DAT_00581b9c`): `0x42b120` troca a ordem de varredura dos tiles e `0x419620` gira a grade de vértices em passos de 90 graus (com `W`, `H` = dimensões do mapa). O quadrante do tile (`tile[0] >> 4 & 3`) soma-se à rotação para escolher a orientação da textura (4 giros de 90 graus aplicados por UV, tabela `0x538000`, não por registros separados).

### 2.2 Texturas de chão e de trilho (bancos quadrados)
`ground`, `tracks`, `trackd`, `road`, `territor`, `riveran`/`rivernan`, `groceana`/`groceann`, `grbeacha` e `tem*` são **atlas de texturas 64x64 com mipmaps**, não sprites isométricos: cada registro é uma textura quadrada; os grupos são níveis de mip (`ground`: 64, 16, 8, 4, 2 (alguns registros só 64, 16, 8, 4); `tracks`/`trackd`/`road`: 64, 32, 16, 8, 4, 2; `territor`: 64, 16, 8, 4) e o grupo usado é o índice `s` do zoom (`0x511a90`: `registro + 0xc + s*8`). O `x, y` de cada imagem é a posição dentro de um atlas (passo 66 px = 64 + 1 px de borda de cada lado), não um hotspot. A textura é mapeada no losango do tile; as cores do chão usam a **paleta de 32 níveis de sombreamento** (`PALT0058`, `0289`, etc. têm 32 paletas: o nível 0..31 vem da inclinação dos 4 cantos e da altura, `0x511a90`); renderizando com a paleta 22 o chão sai com cores naturais. O índice ciano das texturas de trilho/estrada/costa é a **chave de transparência/água** (o trilho é um overlay).

### 2.3 Sprites (veículos, prédios, árvores)
- `x, y, w, h` da imagem = caixa relativa ao ponto de ancoragem do objeto no mapa (tile do objeto, no chão): casa 110x67 em `(-58,-47)`; vagão 20x87 pequeno; locomotiva 16x44 em `(-8,-29)`. Tudo é desenhado em **uma escala só** (zoom máximo, tile de 128x64); não há mips nos sprites de objeto.
- **4 bancos por objeto, um por rotação de visão** (`cr1..cr4`, `bl1..bl4`, `tre*1..4`, `electtw1..4`): `0x424e60` marca num vetor de 4 as rotações usadas pelas visões abertas (`lista 0x574c50`, campo `+0x18`) e só carrega essas (a rotação `r` usa o dígito `r + 1`). Cada banco tem as mesmas direções já giradas.
- Todo registro tem **2 grupos**: grupo 0 = imagem principal (paleta do registro, flags `0x2a`/`0x2b`); grupo 1 = camada esparsa de poucos pixels com índices reservados (0, 128..135, 192+), provável máscara de sombra/luz (semântica em aberto).
- Paletas: veículos 1 paleta (`PALT` n = 1); prédios 3 ou 4; árvores 32 (sombreamento por luz, como o chão).

## 3. Resumo por categoria

| Categoria (`category` no TSV) | Bancos | Registros por banco | Layout |
|---|---|---|---|
| `terrain-ground` | 1 (`ground.imb`) | 221 | 221 texturas 64x64 + mips; ver 4.1 |
| `terrain-ocean` | 2 (`groceana` 64 / `groceann` 4) | 64 / 4 | água; `groceana` = 16 quadros de animação x 4 giros (registro = variante + `4 * (contador & 15)`, `0x511a90`); `groceann` = 4 giros estáticos (modo de detalhe baixo, opção `+0x38 == 0`) |
| `terrain-beach` | 1 (`grbeacha`) | 256 | transições de praia, só em detalhe alto |
| `terrain-river` | 2 (`riveran` 90 / `rivernan` 18) | 90 / 18 | rios 64x64, 6 mips |
| `terrain-road` | 1 | 15 | estrada 64x64 |
| `terrain-territory` | 1 (`territor.imb`) | 5 | bordas de território |
| `terrain-editor` | 5 (`tem*`) | 2 | 128x128, pré-visualização do editor |
| `track-single` / `track-double` | 1 / 1 | 65 / 69 | overlays 64x64 de trilho; 4.2 |
| `track-catenary` | 4 (`electtw1..4`) | 12 | postes da catenária, 1 por rotação |
| `bridge` | 4 (`bridge01..04`) | 96 | vãos de ponte, 2 camadas (a de trás e a de frente do trem) |
| `bridge-end`, `bridge-arch` | 4 + 4 | 8 / 4 | rampas e arcos de pedra, 8 / 4 direções |
| `tree` | 44 (11 espécies x 4 rotações) | 1 a 8 | variantes de copa; grupo 1 = sombra |
| `scrub` | 4 (`scrub__1..4`) | 32 | arbustos 29x13 |
| `vehicle-engine` | 264 (66 motores) | 8 | 8 direções |
| `vehicle-tender` | 140 | 8 | 8 direções |
| `vehicle-wagon` | 276 (69 tipos) | 4 (6 tipos têm 8) | 4 eixos |
| `vehicle-unnamed` | 4 (RT2/0552..0555) | 8 | veículo pequeno 13x32 sem nome |
| `station` | 480 (24 projetos x 5 estilos x 4 rotações) | 1 | prédio da estação; 2 grupos |
| `industry` | 252 | 1 (5 em `port_`) | indústrias e prédios especiais |
| `housing` | 35 | 1 | casas `housa..housh` |
| `city-building` | 96 | 1 | `th*` (casas), `com*`, `ret*` |
| `effect` | 2 (`SMOKE`, `smoke2`) | 9 | fumaça/vapor, 13 grupos |
| `map-marker` | 4 (`city__1..4`) | 24 | marcadores de cidade |
| `cursor` | 23 | 1 / 24 | `mousbw01..24`, `mousco` |
| `portrait`, `logo` | 82, 41 | 1 | `brplyr`/`bigp` (retratos), `brlogo` (logos de ferrovia) |
| `font` | 10 | 96 a 224 | ver `font.md` |
| `ui`, `ui-icons`, `ui-cab`, `ui-picture`, `picture-landscape`, `scenery-vignette` | 83, 14, 23, 23, 13, 43 | 1 a 139 | telas, ícones, cabine da locomotiva (`td_*`, `tds_*`...), fotos |

Total 1987.

## 4. Terreno e trilho

### 4.1 `ground.imb` (221 registros): tabela id -> registro (decodificada)

O registro do chão é `0x5804ac[id]` (id de terreno de 10 bits, `map-gam.md` 3.2); a classe é `0x581090[id]`. Ambas são construídas em `0x471b10` a partir de 19 descritores de classe (`0x540aa0`, passo 0xc0: `+0` multiplicador de custo f32, `+0x64..0x70` tamanhos `n0..n3` dos quatro grupos próprios, `+0x74 + 4j` = nº de peças de transição com a classe `j`). Reimplementação em `rt2_formats::terrain::Tables` (conferida id a id com as tabelas despejadas do original):

- Cada classe `c` ocupa ids consecutivos, na ordem das classes: **`n0` variantes comuns, `n1` raras (1 vez em 40), `n2` peças de mistura (canto, lado), `n3` peças de margem da praia (classe 0) ou estágios de cultura (classes 9, 10, 11, 17)** (`S0..S4` = início de cada grupo; `S4` = fim dos ids próprios), depois **19 "fillers"** (`S4 + j`: tile da classe `c` que mostra o chão da classe `j`, registro = primeiro registro da classe `j`), depois as peças de **transição** com cada classe `j` com par definido (3 por par: canto, lado, cheio), em ordem de `j`.
- `n0..n3` por classe 0..18: `(3,0,2,8) (6,0,2,0) (6,0,2,0) (6,3,2,0) (6,3,2,0) (6,3,2,0) (6,3,2,0) (6,4,2,0) (5,0,2,0) (5,0,0,12) (5,0,0,12) (5,0,0,12) (8,0,2,0) (4,0,2,0) (4,0,2,0) (4,0,2,0) (4,0,2,0) (5,0,0,12) (4,0,0,0)`. Pares com transição (3 peças): (3,4) (3,5) (3,6) (3,7) (4,5) (4,6) (5,6) (5,7) (6,7). Total de ids por classe = `n0+n1+n2+n3 + 19 + 3*(nº de pares com a classe)`; ids totais 613; primeiros ids das classes 0..18: 0 32 59 86 128 167 209 251 291 317 353 389 425 454 479 504 529 554 590.
- Registros: ids próprios da classe `c` -> `primeiro_registro[c] + (id - S0)`; os pares `(a, b)`, `a < b`, têm 3 registros logo depois dos registros próprios de `a`, na ordem dos `b`; os ids de transição de `a` contra `b` **e** de `b` contra `a` usam os mesmos 3 registros (o código troca giro/tipo quando `b < a`). A água (classe 18) tem 4 ids próprios = registros 221..224 (= as 4 variantes do oceano) e seu "registro" 221 serve de filler.

Famílias de ground.imb por classe (registros próprios): 0 praia 0-12 (0-2 areia, 3-4 mistura, 5-12 margem animada), 1 deserto 13-20, 2 terra devastada 21-28, 3 terra 29-39, 4 grama seca 52-62, 5 grama 69-79, 6 grama viçosa 86-96, 7 pântano 100-111, 8 neve 112-118, 9 trigo 119-135, 10 algodão 136-152, 11 cana 153-169, 12 montanha 170-179, 13..16 rochas escura / marrom / cinza / bege (180-203), 17 rocha com costa (204-220); 40-51, 63-68, 80-85 e 97-99 são as transições entre os solos 3-7. Classes 13..17 só aparecem em mapas `.MP2`. Nomes do editor (lng 700..713) para as classes 0..12: beach, desert, wasteland, dirt, dry grass, grass, lush grass, swamp, snow, wheat field, cotton field, sugar cane, mountain.

Cada tile desenha: a textura base (`record_of_id`, giros `tile[0] >> 4 & 3`) e os overlays (`map-gam.md` 3.2): transições de solo, margem da praia sobre a água (animada com `grbeacha.imb`), rios (`riveran.imb`, 90 = 5 quadros x 18 peças), estradas (`road.imb`, 15). O renderizador usa a paleta 22 (sombreamento natural). Os registros 3-4 / 19-20 / ... ("ruído de água") são as peças de mistura `n2` de cada classe, índice de cor 0 = transparente.

### 4.2 Trilho (`tracks`, `trackd`)
Registro do trilho = `shape_record[forma * 4 + giro]`, com `(forma, giro) = edge_shape_table[máscara]` (`0x5365e8`) e `shape_record` em `0x536594` (21 formas x 4; 99 = inválido); a máscara tem 1 bit por aresta (bit `d`, direção `d` de `terrain-track.md` 1.1). Os registros 14-18, 33-36, 47-49 e 60-62 pertencem às formas 14..18, que nenhuma máscara de aresta usa (provável pontes, estações, fim de linha). `trackd` (via dupla) tem a mesma tabela e 4 registros a mais. Registros por máscara (decimal; arestas listadas como dígitos de direção `0..7`):

| Máscara | Arestas | Registro | Máscara | Arestas | Registro |
|---|---|---|---|---|---|
| 1 | 0 | 0 | 70 | 126 | 25 |
| 2 | 1 | 9 | 72 | 36 | 24 |
| 4 | 2 | 52 | 76 | 236 | 26 |
| 8 | 3 | 57 | 81 | 046 | 37 |
| 9 | 03 | 2 | 84 | 246 | 50 |
| 16 | 4 | 39 | 85 | 0246 | 6 |
| 17 | 04 | 1 | 98 | 156 | 46 |
| 18 | 14 | 41 | 100 | 256 | 55 |
| 19 | 014 | 43 | 102 | 1256 | 28 |
| 21 | 024 | 63 | 128 | 7 | 29 |
| 25 | 034 | 4 | 132 | 27 | 54 |
| 32 | 5 | 44 | 136 | 37 | 30 |
| 33 | 05 | 3 | 137 | 037 | 32 |
| 34 | 15 | 10 | 138 | 137 | 51 |
| 35 | 015 | 11 | 140 | 237 | 58 |
| 36 | 25 | 53 | 144 | 47 | 40 |
| 38 | 125 | 12 | 145 | 047 | 42 |
| 42 | 135 | 38 | 152 | 347 | 59 |
| 49 | 045 | 5 | 153 | 0347 | 8 |
| 50 | 145 | 45 | 162 | 157 | 64 |
| 51 | 0145 | 7 | 168 | 357 | 20 |
| 64 | 6 | 21 | 170 | 1357 | 13 |
| 66 | 16 | 23 | 196 | 267 | 56 |
| 68 | 26 | 22 | 200 | 367 | 31 |
| 69 | 026 | 19 | 204 | 2367 | 27 |

Registros não usados por máscara: 14-18, 33-36, 47-49, 60-62.

## 5. Pontes, árvores e efeitos

- `bridge01..04`: um banco por tipo de ponte (`track_bridge_type` 1..4 de `terrain-track.md`), 96 registros cada, 2 camadas (grupo 0 = estrutura, grupo 1 = camada de frente do trem). Os registros cobrem 6 linhas de 16 (8 direções x dois tipos de peça por linha); o mapeamento exato peça/direção -> registro não foi decodificado. Trilho e overlays: **os nós de trilho vivem na mesma lista de overlays do tile** (banco 4 = `tracks`, 5 = `trackd`, camada 4/5; byte de sombra -8 em trilho de outro dono, `0x4cb090`), com o registro `shape_record[forma*4+giro]`.
- `ends1..4` (8 direções) e `arch1..4` (4 direções): rampas e arcos da ponte de pedra, um banco por rotação (a ordem dos registros gira junto).
- Árvores (`treoak_`, `trepine`, `trepalm`, `trecact`, `trebrus`, `tremesq`, `trerfor`, `treswam`, `treprod`, `trerubb`, `trecoff`, `scrub__`): registros = variantes de copa (carvalho/pinheiro/floresta 8, palmeira/café/borracha 3, deserto 6, arbusto 4, mesquite 4, pântano 6, produção 1, scrub 32); a classe de vegetação do tile (`tile+2 & 0xf`, `terrain-track.md` 2.2) e a contagem `tile+3 & 0x1f` decidem a espécie e a densidade. 32 paletas por banco (luz). A associação espécie -> classe de vegetação não foi lida (aberta). O estilo de estação (`V C M T B K`) é o byte `+0x52` do registro do objeto (`0x4093e0`: `"VCMTBK"[registro[0x52]]`); `B` não tem arte.
- `SMOKE.imb` (1x1, 156 imagens) e `smoke2.imb` (4x25, 313): 9 registros, 13 grupos de quadros animados (nuvens de fumaça e vapor).

## 6. Estações

`bl{rot+1}st{estilo}{NN}.imb` (só `bl`): `NN` = `sta10..sta17` (pequenas), `sta20..sta27` (médias), `sta30..sta37` (grandes), 8 projetos por tamanho (os registros 0..23 de `building_type_table`, `terrain-track.md` 3.1); `estilo` = V, C, M, T ou K (6 letras na tabela, B não tem arte). 4 paletas por banco. 1 registro, 2 grupos (prédio, camada esparsa). O formato de cada registro é de um prédio de 1 a 3 tiles (pequena 59-113 x 49-73, grande até 184x131).

## 7. Indústrias e prédios

Dois bancos por prédio e rotação (`bn` estático, `bl` com animação, ver 1). Classe = tipo da indústria em `industry_type_table` (`cargo.md` 7.1, LNG `920 + classe`); o registro de `building_type_table` (`0x5457e8`, passo 0x154, +0x10 = classe, +0..4 = nome do sprite) liga o nome do banco à classe. Tipos repetidos (4 `grain`, 4 `cottn`, 4 `coffe`, 4 `prodc`, 4 `rubbr`, 4 `sugar`, 4 `port_`) são estágios visuais do mesmo prédio (`port_` tem 5 registros: partes do porto).

| Classe | Nome | Banco(s) (`bn`/`bl` + 1..4 + nome) | Tipos de prédio (índice em `0x5457e8`) |
|---|---|---|---|
| 1 | Aluminum Mill | `alum_` | 60 |
| 2 | Auto Plant | `auto_` | 37 |
| 3 | Bakery | `baker` | 56 |
| 4 | Bauxite Mine | `baux_` | 83 |
| 5 | Cannery | `caner` | 55 |
| 6 | Cattle Yard | `cattl` | 61 |
| 7 | Cement Plant | `cemnt` | 54 |
| 8 | Chemical Plant | `chem_` | 53 |
| 9 | Coal Mine | `cmine` | 52 |
| 10 | Coffee Farm | `coffe` | 62, 63, 64, 65 |
| 11 | Cotton Farm | `cottn` | 48, 49, 50, 51 |
| 12 | Dairy Farm | `dfarm` | 45 |
| 13 | Dairy Processor | `dproc` | 44 |
| 14 | Electric Plant | `elect` | 66 |
| 15 | Fertilizer Plant | `fertl` | 43 |
| 16 | Grain Silo | `grain` | 33, 34, 35, 36 |
| 17 | Gravel Pit | `gravl` | 47 |
| 18 | Housing | `housa`, `housb`, `housc`, `housd`, `house`, `housf`, `housg`, `housh` | 25, 26, 27, 28, 29, 30, 31, 32 |
| 19 | Iron Ore Mine | `imine` | 88 |
| 20 | Landfill | `landf` | 67 |
| 21 | Logging Camp | `loggi` | 68 |
| 22 | Lumber Mill | `lumbr` | 24 |
| 23 | Meat Packing Plant | `meatp` | 46 |
| 24 | Nuclear Plant | `nuker` | 42 |
| 25 | Oil Refinery | `oilrf` | 41 |
| 26 | Oil Well | `oilwl` | 69 |
| 27 | Paper Mill | `paper` | 40 |
| 28 | Port | `port_` | 84, 85, 86, 87 |
| 29 | Produce Orchard | `prodc` | 70, 71, 72, 73 |
| 30 | Rubber Farm | `rubbr` | 74, 75, 76, 77 |
| 31 | Sheep Farm | `sheep` | 39 |
| 32 | Steel Mill | `steel` | 57 |
| 33 | Sugar Farm | `sugar` | 79, 80, 81, 82 |
| 34 | Textile Mill | `textl` | 38 |
| 35 | Tire Factory | `tiref` | 58 |
| 36 | Tool and Die Factory | `toold` | 59 |
| 37 | Uranium Mine | `uran_` | 78 |
| 38 | Geothermal/Chemical Dump (geoc_) | `geoc_` | 89 |
| 39 | Town house (th*) | `th1ab`, `th1ac`, `th2ca`, `th2cb`, `th3ab`, `th3ac`, `th4ba`, `th4bc` | 90, 91, 92, 93, 94, 95, 96, 97 |
| 40 | Commercial (com*) | `com1a`, `com1b`, `com2b`, `com2c`, `com3a`, `com3b`, `com4a`, `com4c` | 98, 99, 100, 101, 102, 103, 104, 105 |
| 41 | Retail (ret*) | `ret1a`, `ret1b`, `ret2b`, `ret2c`, `ret3a`, `ret3b`, `ret4a`, `ret4c` | 106, 107, 108, 109, 110, 111, 112, 113 |
| 42 | Distillery (distl) | `distl` | 114 |
| 43 | Barracks (barrk) | `barrk` | 115 |
| 44 | Depot (depot) | `depot` | 116 |
| 45 | Munitions Factory (munit) | `munit` | 117 |
| 46 | Weapons Factory (weapf) | `weapf` | 118 |
| 47 | Airport (airpt) | `airpt` | 119 |
| 48 | Airport part (airpn/airps) | `airpn`, `airps` | 120, 121 |

Classes 38 a 48 não têm nome no `.lng`; os nomes acima vêm do próprio nome do sprite. Prédios `housa..housh` (classe 18) são casas de cidade; `th1ab..th4bc` (classe 39), `com*` (40) e `ret*` (41) são as variantes de casas, comércio e varejo (tipos 90 a 113 de `building_type_table`).

## 8. Veículos

`cr{rot+1}{nome}.imb` (`nome` = 5 primeiras letras de `vehicle_type_table`, `0x53ddb8`, 175 registros; `docs/spec/trains.md` 2.3). Prefixo de 1o caractere: `s` locomotiva a vapor, `t` tênder, `d` diesel, `e` elétrica, `f` vagão.

**Quadros:** não há quadros de animação: 1 imagem por direção. Locomotivas e tênderes têm **8 registros** (8 direções em tela) e vagões **4** (4 eixos, o sentido oposto reaproveita o mesmo desenho); 6 tipos de vagão (cabine/restaurante) têm 8. O registro `r` é a direção em tela em sentido horário a partir de "para cima": `r0` vertical (cima), `r1` cima-direita (sentido de `d0` de `terrain-track.md` 1.1 em rotação 0, N = `(+64,-32)` na tela), `r2` horizontal (direita), `r3` baixo-direita, `r4` baixo, `r5` baixo-esquerda, `r6` esquerda, `r7` cima-esquerda; direção de mapa `d` -> `r = (d + 1) & 7` na rotação 0 (confirmado nas imagens; a frente x traseira do desenho e a conversão para as outras rotações ainda não foram conferidas).

Há 1 paleta por banco (cor fixa por desenho; a cor de jogador não foi encontrada na paleta). Grupo 1 de cada registro = camada esparsa (ver 2.3).

### 8.1 Motor -> banco
`engine_table` (66 motores, `trains.md` anexo A) -> tipo de veículo em `vehicle_type_table` (+0x2f = motor, +0x2e = 0 locomotiva / 1 tênder):

| Motor | Nome | Locomotiva (`cr{1-4}` + ...) | Tênder |
|---|---|---|---|
| 0 | Stephenson Rocket | `sroc_` | `troc_` |
| 1 | 2-4-0 John Bull | `sbul_` | `tbul_` |
| 2 | 0-4-0 Dewitt Clinton | `sdew_` | `tdew_` |
| 3 | TGVx | `etgvx` | - |
| 4 | 4-2-0 Prussian | `spru_` | `tpru_` |
| 5 | 3-Truck Shay | `sshay` | `tshay` |
| 6 | 0-4-0 A3 | `s040_` | `t040_` |
| 7 | 4-4-0 American-C | `s440_` | `t440_` |
| 8 | USRA 0-6-0 | `s060_` | `t060_` |
| 9 | USRA 0-8-0 | `s080_` | `t080_` |
| 10 | 2-6-2 Prairie | `s262_` | `t262_` |
| 11 | 4-6-0 Ten-Wheeler | `s460_` | `t460_` |
| 12 | 2-8-0 Consolidation | `s280_` | `t280_` |
| 13 | 4-4-2 Atlantic | `s442_` | `t442_` |
| 14 | 4-8-0 Mastodon | `s480_` | `t480_` |
| 15 | 4-6-2 Pacific | `s462_` | `t462_` |
| 16 | 4-6-4 Hudson | `s464_` | `t464_` |
| 17 | 2-6-0 Camelback | `s260c` | `t260c` |
| 18 | 2-6-0 Mogul | `s260_` | `t260_` |
| 19 | 2-8-2 Mikado | `s282_` | `t282_` |
| 20 | 4-4-0 D16sb | `s440b` | `t440b` |
| 21 | 4-4-4-4 T-1 | `s4444` | `t4444` |
| 22 | 4-6-4 J3A Streamliner | `s464b` | `t464b` |
| 23 | 4-8-4 Daylight | `s484_` | `t484_` |
| 24 | 4-8-8-4 Big Boy | `s4884` | `t4884` |
| 25 | 4-6-0 Class B12 | `sb12_` | `tb12_` |
| 26 | 2-10-0 Class 13 H | `sc13h` | `tc13h` |
| 27 | 0-10-0 Class G10 | `sc57_` | `tc57_` |
| 28 | Class A4 Mallard | `small` | `tmall` |
| 29 | Trevithick-1 | `strev` | `ttrev` |
| 30 | Dash-9 | `d449_` | - |
| 31 | F3A+B | `df3a_` | `df3b_` |
| 32 | F9 | `df7a_` | - |
| 33 | GP18 | `dgp18` | - |
| 34 | GP9 | `dgp9_` | - |
| 35 | SDP40 | `dsd40` | - |
| 36 | SD45 | `dsd45` | - |
| 37 | Alco PA-1 | `dpa1_` | - |
| 38 | Class 232 | `d232_` | - |
| 39 | AMD-103 | `damd1` | - |
| 40 | FP45 | `dfp45` | - |
| 41 | V200 | `dv200` | - |
| 42 | Class 1020 | `e1020` | - |
| 43 | Class 1045 | `e1170` | - |
| 44 | E656 FS | `e656a` | `e656b` |
| 45 | E 69 | `egen_` | - |
| 46 | Ee 3/3 | `essbe` | - |
| 47 | Ae 8/14 | `eae8a` | `eae8b` |
| 48 | Be 4/6 II | `ebe46` | - |
| 49 | Class E111 | `ec111` | - |
| 50 | Class E18 | `ee18_` | - |
| 51 | Penn. E44 | `ee44_` | - |
| 52 | E60CP | `ee60c` | - |
| 53 | GG1 | `egg1_` | - |
| 54 | Shinkansen Bullet | `eshin` | - |
| 55 | Thalys Bullet | `ethal` | - |
| 56 | 2-4-0 Vulcan | `svulc` | - |
| 57 | 4-4-0 8-Wheeler | `s440x` | `t440x` |
| 58 | 1-3 BoBo | `ebno3` | - |
| 59 | Class 55 Deltic | `dc55_` | - |
| 60 | Mag-Lev TBX-1 | `emag_` | - |
| 61 | 4-2-2 Iron Duke | `s422_` | `t422_` |
| 62 | Eurostar | `euro_` | - |
| 63 | DB 18 201 | `sdbag` | `tdbag` |
| 64 | Brenner | `e412_` | - |
| 65 | TGV | `etgv_` | - |

### 8.2 Vagões por carga
Vários bancos por carga são **eras** (anos de disponibilidade do tipo; o sufixo numérico cresce com a era, p. ex. `fpas0..fpas4`). Cargas 33 a 45 são as do conjunto de expansão (`fmet*`, `ftroo`, `fmun_`, `fweap`, `falco`, `fwool` repetido; 44 = restaurante `fdin*`, 45 = caboose `fcab1`).

| Carga | Nome | Vagões (nome do banco, anos) |
|---|---|---|
| 0 | passenger | `fpas0` (1800-1845), `fpas1` (1846-1897), `fpas2` (1898-1930), `fpas3` (1931-1960), `fpas4` (1961-) |
| 1 | mail | `fmal1` (1800-1897), `fmal2` (1898-1925), `fmal3` (1926-) |
| 2 | lumber | `flumb` (1895-), `flum2` (1800-1894) |
| 3 | log | `flogs` (1893-), `flog2` (1800-1892) |
| 4 | milk | `fmlk1` (1800-1909), `fmlk3` (1910-) |
| 5 | cattle | `fcatt` (1800-) |
| 6 | steel | `fstel` (1898-), `fste2` (1800-1897) |
| 7 | goods | `fgud1` (1800-1940), `fgud3` (1941-) |
| 8 | auto | `fauto` (1800-) |
| 9 | cement | `fcemt` (1800-) |
| 10 | food | `ffod1` (1800-1900), `ffod3` (1901-) |
| 11 | cotton | `fcott` (1902-), `fcot2` (1800-1903) |
| 12 | wool | `fwool` (1909-), `fwoo2` (1800-1908) |
| 13 | paper | `fpapr` (1917-), `fpap2` (1800-1916) |
| 14 | oil | `foil1` (1800-1900), `foil3` (1901-) |
| 15 | chemical | `fchem` (1800-) |
| 16 | fertilizer | `ffert` (1800-) |
| 17 | waste | `fhazd` (1800-) |
| 18 | coal | `fcoal` (1909-), `fcoa2` (1800-1908) |
| 19 | grain | `fgrai` (1890-), `fgrn2` (1800-1889) |
| 20 | aluminum | `falum` (1800-) |
| 21 | coffee | `fcoff` (1911-), `fcof2` (1800-1912) |
| 22 | tire | `ftire` (1800-) |
| 23 | sugar | `fsugr` (1916-), `fsug2` (1800-1915) |
| 24 | uranium | `furan` (1800-) |
| 25 | pulpwood | `fpulp` (1908-), `fpul2` (1800-1907) |
| 26 | gravel | `fgrav` (1907-), `fgrv2` (1800-1906) |
| 27 | iron | `fore_` (1896-), `fore2` (1800-1895) |
| 28 | produce | `fprod` (1902-), `fpro2` (1800-1901) |
| 29 | bauxite | `fbaux` (1800-) |
| 30 | diesel | `fdies` (1912-), `fdie2` (1800-1911) |
| 31 | rubber | `frubb` (1800-) |
| 32 | dining | `ftroo` (1800-) |
| 33 | (unnamed) | `fmun_` (1800-) |
| 34 | (unnamed) | `fweap` (1800-) |
| 35 | (unnamed) | `falco` (1800-) |
| 36 | (unnamed) | `fmetg` (1800-) |
| 37 | (unnamed) | `fmetr` (1800-) |
| 38 | (unnamed) | `fmety` (1800-) |
| 39 | (unnamed) | `fmetb` (1800-) |
| 40 | (unnamed) | `fmetb` (1800-) |
| 41 | (unnamed) | `fmetg` (1800-) |
| 42 | (unnamed) | `fmety` (1800-) |
| 43 | (unnamed) | `fwool` (1800-) |
| 44 | dining car | `fdin1` (1846-1892), `fdin2` (1893-1920), `fdin3` (1921-), `fdin3` (9999-) |
| 45 | caboose | `fcab1` (1800-) |

## 9. Interface e imagens

- Fontes: `scri11`, `copp6/8/10/12/14/24`, `fran13`, `haet46`, `htra46` (`font.md`).
- Cursores: `mousbw01..24` (preto e branco) e `mousco.imb` (24 coloridos).
- Telas e janelas: `setupb`, `trainbuy`, `trainlst`, `stockdtl`, `helpd`, `eventset`, `campaign*`, `paper*`, `playrdtl`, `mw*`, `side*` (vistas laterais de trem para a lista, 139 registros em 5 escalas), `tbbuillb`/`tbmaplb`/`tbeffelb`/`horsb_*`/`trainbdl`/`statnsym`/`speed` (ícones).
- Cabine: `td_steam`, `td_diese`, `td_elect` (fundos 1024x728 por tipo de motor) e `tds_*`/`tdd_*`/`tde_*` (`oil`, `thro`, `thdi`, `sand`: manômetros e alavancas de vapor/diesel/elétrica), além de 23 bancos sem nome (RT2/2123..).
- Retratos: `brplyr00..40` (ovais, 156x179), `bigp00..40` (grandes), logos `brlogo00..40`.
- Imagens de fundo: 13 paisagens 1024x569 (RT2/1462..1474), 43 objetos de cenário em 1 registro (RT2/1479..1517), 20 fotos 218x134 (RT2/0693..0714); `RT2_11PL` 0, 1, 3 = telas do patch.

## 10. Em aberto

1. ~~Tabela id -> registro de ground.imb~~: resolvido (4.1, `map-gam.md` 3.2).
2. Como o jogo escolhe a paleta (n = 1, 3, 4, 32) de cada banco: níveis de sombreamento do chão e das árvores (0..31) vêm do relevo; os bancos de prédio com 3 ou 4 paletas (estação, indústria) podem ser estação do ano ou iluminação.
3. Grupo 1 de cada registro: **não é sombra** (conferido em `treoak_1` e `bl1housa`: pixels esparsos ao longo da borda esquerda do desenho, índices 0 e 8..25, 128..135, 192+; parece um realce de borda). As sombras escuras no chão ao lado das árvores (vistas no original rodando) não vêm dos bancos; o viewer desenha uma elipse procedural.
4. Mapeamento peça/direção -> registro das pontes e das formas 14..18 de trilho.
5. Qual banco de rotação o jogo usa para cada visão, e se a ordem dos registros gira com ela (comparar `ends1` com `ends2`).
6. Nome dos 132 bancos de interface restantes e dos 4 veículos `RT2/0552..0555`; semântica de `kind` e dos pares do cabeçalho.
7. Tokens `0x80..0x9f` do RLE nos sprites de interface (aparecem como linhas quebradas nos quadros de janela).
