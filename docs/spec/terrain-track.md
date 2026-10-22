# Terreno, trilho e estações: construção, custo e captação

Fonte: `RT2_PLAT.EXE`, lido no C exportado (`re/db/decomp/`) e no disassembly do rizin. O manual (`docs/reference/manual-rules.md`, seções 2 e 3) serviu só de pista; as divergências estão na seção 9. Estado das provas: **[V]** = confirmado por teste diferencial (`crates/rt2-oracle/tests/track.rs`, x87 a 53 bits); **[S]** = lido no código, sem teste; **[D]** = tabela lida do `.data`. Nomes: canônicos de `re/notes/naming-conflicts.md` (`list_stations`, `list_industries`, `effect_multiplier`, `list_record`...).

## 1. Representação

### 1.1 Direções
Oito direções, índice `d` 0..7, sentido horário a partir do norte: `dx = {0,1,1,1,0,-1,-1,-1}` (`dir_dx` `0x537b48`), `dy = {-1,-1,0,1,1,1,0,-1}` (`dir_dy` `0x537b68`) [D]. Par = ortogonal, ímpar = diagonal; oposta = `(d+4)&7`. Um trecho de trilho é uma **aresta** do centro do tile para o vizinho `d`; uma peça guarda um byte com 1 bit por aresta.

### 1.2 Tile (15 bytes na memória, `tile_array` `0x574c24`, largura `0x55e7a4`)
Complementa `docs/formats/map-gam.md`. Campos usados pelo trilho:

| Off | Uso (endereço) |
|---|---|
| `+0` u16 | bits 6..15 = **id de terreno** (`>>6`, 10 bits); `terrain_class_of_id` `0x581090[id]` dá a **classe** 0..18 (18 = água, `== 0x12` em todo o código de trilho/estação); bit 0 de `+0` = **rio** (só ali há ponte, `0x460640`, `0x462a70`); bit 3 impede estação (`0x4087a0`) |
| `+2` | nibble baixo = classe de vegetação (`0x416860`); `0x10` = tem objeto tipo 3 (predio, `0x42e850`); `0x20` = tile já limpo/ocupado por trilho (não cobra árvores, `0x463520`); `0x80` = o tile tem **ponte** (peça com `+0x11 != 0`) |
| `+3` | `&0x1f` = árvores; `0x40` = tile faz parte de **vão sobre água** (spanning); `0x20` = marcado (estação) |
| `+4` u16 | `struct_id` (cadeia em `structure_pool` `0x574c54`, ordenada por tipo; `0x42e8f0(tile, 3)` = predio, `4` = estação) |
| `+6` | `0x02` = estação neste tile; `0x40` = redesenhar |
| `+7` u16 | `track_id` (0 = sem trilho) → `track_piece_ptr` `0x466800` = `list_record(0x574c84, id)` |
| `+9` | território/região (entra em `effect_multiplier` e em permissões) |

### 1.3 Peça de trilho (`track_piece_pool` `0x574c84`, 0x24 bytes)
`+0` máscara de arestas (bit `d`); `+1` segunda máscara (camada de previsão/duplicada, OR em `0x416420`); `+2` empresa dona (0 = nenhuma); `+3` u16 cabeça da lista de veículos sobre a peça; `+5` bits (`0x08` flag3, `0x04` previsão); `+0x11` **tipo de ponte** (0 = sem ponte); `+0x12` rampa em % do trecho (min(|`track_segment_grade`|,120), `0x4cc060`); `+0x13` u16 custo de previsão >> 3; `+0x15` u16 valor acumulado (soma de custo >> 3) [S]; `+0x17`/`+0x1b` desgaste (`trains.md`); `+0x23` bits: **`0x01` = via dupla** (0 = simples; contadores da empresa `+0x186` simples / `+0x18a` dupla, `0x416210`, `0x4162b0`), `0x02` previsão dupla, `0x04` previsão, `0x10` **eletrificada** (`+0x18e`), `0x20` fantasma de previsão, `0x40` peça é ponte de rio (`param_2 ∈ {2,4,7}`, `0x4164f0`).

Criação `tile_track_add` (`0x4164f0`): se o tile não tem peça, zera 0x24 bytes, grava máscara, dona (`+2`), `bit 4` conforme `track_electric` e `tile+2 |= 0x20`; contadores da empresa `+0x182` (células), `+0x186` (simples), `+0x18e` (eletrificadas), globais `0x573d1d/21/29`, e por território (`+0x47c + terr*0x1ed`). Se já tem, só faz OR da máscara. Via dupla é aplicada depois por `tile_track_set_double` (`0x416210`), que **recusa** tile com `tile+3 & 0x40` (vão) ou ponte de madeira (`+0x11 == 1`) [S].

### 1.4 Formas de aresta válidas
Uma peça só pode ter conjuntos de arestas da tabela `edge_shape_table` `0x5365e8` (256 pares `(forma, aux)`, `'c'`=99 inválido) [D]: **50 máscaras válidas** para solo; em pontes (`0x5367e8`) as 8 de entroncamento em T são removidas (42). Válidas (decimal): 1,2,4,8,16,32,64,128 (aresta única), 9,17,18,19,25,33,34,35,36,38,49,50,51,66,68,70,72,76,85,98,100,102,132,136,137,140,144,145,152,153,170,196,200,204 (comuns) e, só em solo, os T 21,42,69,81,84,138,162,168. Regra prática: dois ramos formam reta (180°) ou curva suave (135°); nunca 90° ou 45° entre dois ramos isolados; derivações a 45° só em linha reta (ex.: 19 = 0,1,4). Cruzamentos: 85 (ortogonal) e 170 (diagonal). Isto responde "direções/curvas/diagonais permitidas"; a forma é também o índice de sprite [S]. Teste de uma aresta nova em `track_edge_valid` (`0x462b80`): dentro do mapa; máscara resultante nos dois tiles ∈ tabela; diagonal só se os dois tiles ortogonais vizinhos do canto não têm trilho; **água (classe 18) proíbe** a menos que seja ponte; estação no tile exige que a aresta use a direção da estação; tile com `+3 & 0x20` nos dois lados proíbe; permissão do território (`empresa+0x479+terr*0x1ed`: bit 1 = pode construir trilho, bit 0 = qualquer empresa, senão só se humana) [S].

## 2. Custo de construção de trilho (P0)

### 2.1 Entradas globais
Gravadas pelo chamador (`track_lay_route` `0x4cc060`, UI, ou `track_tile_cost_with_state` `0x463a20`): `track_single` `0x5534b0` (1 simples, 0 dupla), `track_bridge_type` `0x53ce18`, `track_electric` `0x566fcc`, `track_company` `0x566a50`, e as bases por tile (`0x460850` e `0x463a20`):
```
track_edge_base  0x566a64 = ftol(effect_multiplier(9, empresa, 0, território) * 2000)     ; "Track Building"
track_elec_base  0x566a60 = ftol(effect_multiplier(0xb, ...) * 6000)                       ; "Electric Track Building"
```
Ids dos efeitos de evento (lng `tMOD_FACTOR_2` 1624..: `id = índice_lng - 1619` até 0xc, `- 1618` depois): 8 Station Building, **9 Track Building**, 0xa Mountainous Track Building, **0xb Electric Track Building**, 0xc Track Maintenance, **0xe Bridge Building**. (O id 0xd existe sem nome na lista do editor.)

### 2.2 Custo de um tile (`track_tile_cost` `0x463520`) [V]
Chamada para cada nó do caminho com `(x, y, d_in, d_out, modo, vão)`; `d = -1` = sem aresta; `modo` = nibble alto do byte 0 do nó (0 solo; 2, 4, 7 = tile de rio: ponte; 1, 3, 5, 6 = aproximação); `vão` = 2 bits altos do byte 1 (1,2,3 = água adiante/no tile). Devolve `(custo, custo2)`; só `custo` é cobrado, `custo2` é soma informativa (testada só ≠ 0 em `0x4cc060`).
```
m(id) = effect_multiplier(id, empresa, 0, tile.região)         ; f32 (floor 0.01)
se vão != 0:   custo = (tile+3 & 0x40) ? 0 : ftol(m(0xe) * 120000);  return        ; custo2 intocado
single = track_single
se modo == 0:
    se tile+2 & 0x80:  single = (track_id == 0) ? 0 : peça.+0x23 & 1             ; herda da ponte existente
senão:
    se track_single == 0 e tipo_ponte == 1: single = 1                           ; madeira só simples
    se modo ∈ {2,4,7}:                                                           ; PONTE neste tile
        custo2 = 0
        se tile+2 & 0x80:
            se peça.dona != empresa:                       custo = custo2 = 0; return
            se !(peça.dupla==0 e single==0) e peça.+0x11 == tipo_ponte: custo = custo2 = 0; return   ; já tem
            custo2 = ftol(m(0xe) * bridge_cost_table[peça.+0x11*2 + peça.dupla])
        custo = ftol(m(0xe) * bridge_cost_table[tipo_ponte*2 + single]); return
    B = 0                                                                        ; modos 1,3,5,6: sem custo de aresta
senão (modo 0): B = track_edge_base
aresta(d, acc2_in):                       ; d != -1
    presente = (peça ? peça.máscara : 0) & (1<<d)
    se presente: v = (peça && peça.dupla) || single ? 0 : B,  f = (v==B e !=0) ? 0.5 : 0
    senão:       v = single ? B : ftol(B * 1.5)
    diagonal (d ímpar): a1 = ftol(v*1.4), a2 = ftol(v*f*1.4)   senão a1 = v, a2 = ftol(v*f)
eletrificação (só após a 1ª aresta, se track_electric e !(peça && peça.+0x23 & 0x10)):
    e = (single && !(peça && peça.dupla)) ? E : ftol(E * 1.5)                   ; E = track_elec_base
    se a aresta A já existia na peça: acc1 = ftol(e*1.3 + acc1); acc2 = ftol(e*0.3 + acc2)
    senão: acc1 += e
2ª aresta: mesma regra de aresta, acumulando em acc1/acc2 (diag: acc1 = ftol(v*1.4 + acc1))
se !(tile+2 & 0x20): acc1 += 1000 * tile_tree_count(tile)                        ; 0x416860
t = ftol(acc1 * terrain_class_table[classe].f32_0)                               ; 0x540aa0 + classe*0xc0
custo = ftol(t * econ_track_cost[estado_econômico 0x570717])                     ; 0x5504a8 = {0.8,0.9,1.0,1.1,1.2}
custo2 = acc2                                                                    ; sem multiplicadores de terreno
```
Consequências com modificadores neutros, solo plano, estado 2: **via simples ortogonal = 2 x 2000 = 4000 por tile**; dupla = 2 x 3000 = 6000; eletrificada simples 4000 + 6000 = 10000; dupla eletrificada 6000 + 9000 = 15000 (bate com a tabela 5-4 do guia). Diagonal: 1,4 x por aresta (5600 simples). Reforçar uma aresta existente para dupla cobra `B` por aresta (e `custo2 = B/2`).

**Multiplicador de terreno** (`0x540aa0` +0, f32) por classe [D]: 0 e 1 = 1,25; 2 = 0,8; 3,4,5,6 = 1; 7 = 1,5; 8 = 2; 9,10,11 = 1; 12 = 1,5; 13 = 1,25; 14 = 1,2; 15 = 1,35; 16 = 1,3; 17 = 2; 18 (água) = 1. Nomes do editor (lng 700..713) para as 14 primeiras classes: beach, desert, wasteland, dirt, dry grass, grass, lush grass, swamp, snow, wheat field, cotton field, sugar cane, mountain, ocean. A tabela id→classe (`0x581090`) é montada em `0x471b10` (não decodificada; Em aberto). **Árvores**: `tile_tree_count`: classe de vegetação (`tile+2 & 0xf`) 0 → 0; 1,2,4,5,7,8,9 → `tile+3 & 0x1f`; demais → 1; custo 1000 cada, antes do multiplicador de terreno.

**Pontes** (`bridge_cost_table` `0x53ce60`, i32[tipo*2 + simples]) [D], por tile de rio, multiplicado por `m(0xe)`: tipo 1 madeira dupla 90000 / simples 60000; 2 pedra 300000 / 200000; 3 aço 225000 / 150000; 4 150000 / 100000 (tipo extra, só por opção); 0 e 5 = 0. Disponibilidade (`max_bridge_type` `0x469ef0`): opção do cenário (`+0x1bf`) se ≠ 0; senão ano < 1840 → 1, < 1880 → 2, ≥ 1880 → 3 (ou 3 se a flag `0x573d7a`) [S]. Vão sobre água aberta: 120000 x `m(0xe)` por tile em modo vão, grátis se o tile já é vão (`tile+3 & 0x40`). Túneis: **não existem** no código lido.

### 2.3 Terraplenagem (earthwork)
Ao fixar um caminho, `track_level_edge` (`0x427ff0`, `0x4274b0`, `0x4278f0`) nivela os vértices da aresta: para aresta ortogonal, os dois vértices recebem `(h1 + h2 + h3 + 1) / 3` (três vértices da aresta e do canto), limitado a **[-255, 510]**; diagonais por `0x4278f0`. Não nivela se um tile vizinho já tem trilho que usa o vértice (tabelas `0x53a0b0`/`0x53a12c`). O jogo registra os vértices alterados (`0x5650f8` índices, `0x5650fc` alturas antigas, contagem `0x565100`) e cobra em `0x4cc060`:
```
terraplenagem = 600 * ftol(effect_multiplier(0xa, empresa) ) * Σ |h_novo - h_antigo|      ; 0x5357ac = 600
```
(fator 600 é f32; o `ftol` é do produto `600 * m(0xa)`.) O total do caminho é `Σ custo dos tiles + terraplenagem`, lançado no razão categoria 3 (`0x4398d0(empresa, 3, total)`) [S]. A rampa resultante de cada peça fica em `+0x12` via `track_segment_grade` (`trains.md` 4.1). **Não há limite rígido de rampa** na construção: só o custo da terraplenagem e a penalidade do A* (`0x463b30`: |dh| > 8 → custo do passo multiplicado por `dh²/64`, com `0x5746ec`) [S].

### 2.4 Demolição
`track_demolish_edges` (`0x4ca920`, `0x4162b0`): remove as arestas pedidas; recusa se há estação neste tile (motivo 1), trem sobre a peça (3) ou dono diferente (2, salvo `0x5748fc`). Não há lançamento no razão: **sem reembolso** nesse caminho [S]; custos de limpeza/demolição de prédios não lidos (Em aberto). Contadores da empresa e globais são decrementados.

### 2.5 Limites e verificações extras
- Água: `track_edge_valid` retorna 0 em tile de classe 18 salvo vão/ponte (`param_8`) [S].
- Cidades: o caminho evita estações; objetos tipo 3 (prédios) no tile impedem via `tile+2 & 0x10`/estação [S].
- Trilho de outra empresa: `0x4cc060` só reaproveita tile se a peça é da mesma empresa (senão limpa `+0x23 & 0xfd`); `track_company_may_build_at` (`0x4603a0`) exige trilho próprio até 2 tiles de distância quando `0x5706c1/c2` ligados [S].
- **Limite de células** (`CompanyTrackCellsRemaining`): `empresa+0x4260` é decrementado pelo nº de tiles novos; com `0x57022a` ligado e saldo < 0, a construção falha (`0x574b7c = 1`) [S].
- Manutenção mensal de trilho (`0x4399d0`): `m(0xc) * (-0.005 * razão[3])`, ou seja 0,5% do valor lançado no razão 3, por mês (se o produto for negativo vira 0) [S].

## 3. Estações

### 3.1 Tipos e custo [D]
`building_type_table` `0x5457f8` (0x154 bytes por registro): `+0` tipo (0 = estação), `+1/+2` índices tamanho/estilo, pegada 10x10 em `+0xcd - 20*lin - 2*col` (u16), nome do sprite `+0x144` (`sta11..sta37`), custo u32 `+0x14b`. Estações: registros 0..6 pequenas (50000), 7..14 médias (100000), 15..22 grandes (200000); 7/8/8 estilos de arquitetura (só visual). Custo ao construir (`0x409a30`, `0x48eb30` em `0x40a45c`):
```
custo = ftol(effect_multiplier(8, empresa, 0, região do tile) * custo_do_tipo)        ; lançado no razão 1
```
mais o trilho sob a estação (0x463a20 com simples, sem eletrificação, ponte tipo 1) no razão 3 [S]. A UI recusa se o caixa não cobre (`0x43a060`) [S]. Os custos 50k/100k/200k batem com o manual.

### 3.2 Colocação (`station_placement_valid` `0x4087a0`) [S]
Para cada célula da pegada com flags `& 0x3c3`: dentro do mapa; terreno sem rio (`tile[0]&1`), sem `tile+3 & 0x20` se a célula exige; célula em **terra** precisa de `flags & 0x2c1`, em **água** de `flags & 0x102`; sem outro predio (objeto tipo 3) exceto o permitido; estação nova (tipo 0): `tile+6 ... puVar1[1] & 0x20` impede. Depois, para a célula-âncora (flag `0x10`), as duas arestas vizinhas (`dir±2`) devem existir ou poder ser construídas (`track_edge_valid` x3), o território deve permitir (`empresa+0x479 & 2`), e o tile da âncora não pode ser de outra estação (`0x4bf1d0`). A estação fica **ao lado do trilho** (aresta perpendicular), orientação `dir`.

### 3.3 Captação [V]
`station_catchment_test` (`0x407030`): prédio `p` pertence à estação `s` se
```
raio = station_radius[tamanho]      ; {2, 4, 8}  0x54fd9c
dist(a, b) = ftol( sqrt((bx-ax)² + (by-ay)²) )   ; 0x4f0ca0, truncado
rápido:  dist(estação.xy, p.centro(+0x1a,+0x1c)) <= raio + 11
min = min sobre as células (lin, col) da pegada de p com flag & 0x43 de dist(estação.xy, (p.+0x1e - col, p.+0x20 - lin))
elo se min <= raio
```
A estação nunca liga outra estação; o prédio da própria estação é aceito. Como a distância é truncada, o tile está dentro se a distância euclidiana é `< raio + 1`: pequena (raio 2) cobre o quadrado 5x5 inteiro (corner 2,83 → 2); média (4) disco de diâmetro 9 com cantos cortados ((4,3) = 5 fora); grande (8) disco de diâmetro 17. `station_rebuild_links` (`0x4071b0`) percorre `list_industries` e cria elos em `list_links` (`0x574c58`), incrementando `+0x33` do prédio e `+0x41` da estação; a oferta/demanda sai de `station_recompute_supply_demand` (`cargo.md`, seção 8). A "primeira que chega leva a carga" do manual corresponde ao carregamento do lote mais antigo (`cargo.md` 7.6).

### 3.4 Melhorias de estação
Não decodificadas: a tabela de melhorias (preço, ano, efeito) e o ramo de compra (`0x4bf4e0`, ledger) não foram lidos. Efeitos de receita já especificados em `cargo.md`. Em aberto.

## 4. Edição de terreno por jogadores
O modo de ferramenta `0x574cb0` (`0x48eb30`) tem os casos 1 (subir/descer), 9/0x12 (pincel), 10/0xb (balde), 0x10 e 0x13 com sons `raise61m`, `brush61m`, `buckt61m`: pertencem ao editor de mapas (manual: só no editor). Nenhum deles lança custo no razão (a lista de chamadores de `0x4398d0` não os inclui) [S]. O único corte/aterro pago é a terraplenagem do trilho (2.3). Limpar árvores é parte do custo do tile (2.2).

## 5. Sinais, via dupla e junções
Não há sinais de linha. Via dupla é o bit `+0x23 & 1` (seção 1.3) com custo 1,5x; o efeito no movimento (faixas, prioridade) está em `trains.md` (Em aberto lá: bloqueio entre trens, `0x4d3c80`). Junções são as formas T/Y da tabela 1.4.

## 6. Propriedade e taxas de uso
Dono da peça = `peça.+2`. Troca de dono na reconstrução: `0x4cc060` mantém a peça se `dona == empresa`, senão remove bits de previsão. A partilha de receita por quilometragem em trilho alheio (`cargo.md` 4.8 item 5) é acumulada em `train_enter_tile` (`0x4d5370`, `trains.md` 4); a regra de quanto vale cada trecho (campos `+0x15/+0x17` da peça) **não foi fechada**: Em aberto.

## 7. Verificação diferencial
`crates/rt2-oracle/tests/track.rs` (CW 0x27f):

| Teste | Função | Casos | Resultado |
|---|---|---|---|
| `track_tile_cost_matches_original` | `0x463520` | 30 000 (tile, peça, globais, modos 0..7, vão, arestas) | custo e custo2 idênticos |
| `station_catchment_matches_original` | `0x407030` | 20 000 (tamanhos 0..2, prédios 24..63) | retorno e distância idênticos |

Limite: nos testes a empresa é 0 e a região do tile 0 (sem listas de empresas/territórios); `effect_multiplier` com empresa/território é coberto por `tests/cargo.rs`. Tile com `+2 & 0x80` sem peça trava o original (registro 0): nunca ocorre no jogo.

## 8. Cobertura de funções
Nomeadas em `re/pending/track.toml`. Lidas e descritas: `0x463520` [V], `0x407030` [V]; [S]: `0x463a20`, `0x4cc060`, `0x4164f0`, `0x416210`, `0x4162b0`, `0x4ca920`, `0x462b80`, `0x4087a0`, `0x409a30`, `0x4071b0`, `0x42e8f0`, `0x42e850`, `0x469ef0`, `0x416860`. Só nomeadas: `0x460850` (A* com listas abertas, hash de nós; também roteia trens com `param_7 == 0`), `0x463b30`, `0x427ff0`, `0x460640`, `0x460520`.

## 9. Divergências do manual
- **Custo base**: o manual dá 4000/6000 por "quadrado"; no código é `2000` por meia aresta (2 arestas por tile) x1,5 para dupla; eletrificação 6000/9000 por tile. Confere em solo plano; aresta diagonal custa 1,4x (não citado).
- **Terreno**: areia (praia e deserto) +25%, pântano e montanha +50%, neve x2 conferem; o código também tem `wasteland` x0,8 (barato) e classes 13..17 (1,25; 1,2; 1,35; 1,3; 2,0) que o manual não cita. Árvores +1000 conferem.
- **Pontes**: preços por tile de rio conferem (pedra 200k/300k, aço 150k/225k, madeira 60k); a madeira dupla (90k) existe nos dados mas é forçada a simples; ano de disponibilidade no código: madeira desde o início, pedra 1840, aço 1880 (o manual diz 1830/1840/1880).
- **Vão sobre água**: 120000 por tile (o manual de TSC diz 120k por seção; o guia 150k).
- **Rampa**: sem limite rígido; o manual sugere custo alto em declives, que aqui sai da terraplenagem 600 x Σ|dh|.
- **Túneis**: inexistentes, como no manual.
- **Estações**: custos 50k/100k/200k conferem; captação pequena 5x5 e discos de diâmetro 9 e 17 conferem (raios 2/4/8 com distância truncada).
- **Reembolso na demolição**: nenhum visto; o manual não diz.

## Em aberto
- Tabela id de terreno → classe (`0x581090`, montada em `0x471b10`) e nomes das classes 13..17.
- Melhorias de estação (custos, anos, efeitos) e seu ramo de compra.
- Custo/refundo de demolição de prédios e `0x4cdd50`; função exata de `custo2` além de teste ≠ 0.
- Id 0xd de efeito (sem nome no editor) e uso da opção `+0x1bf` (ponte máxima).
- Regra de taxa por uso de trilho alheio (campos `+0x15/+0x17` da peça), manutenção anual por peça.
- Sinais/bloqueio de via simples (ver `trains.md`), significado dos bytes de forma `aux` de `0x5365e8`.
- `0x460850`: custos do A* (`0x463b30`) só descritos em linhas gerais.
