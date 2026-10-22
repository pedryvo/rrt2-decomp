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
| `+2` | nibble baixo = classe de vegetação (`0x416860`); `0x10` = tem objeto tipo 3 (predio, `0x42e850`); `0x20` = tile já limpo (não cobra árvores, `0x463520`). Atenção: em `tile+3` (byte de árvores) o bit `0x20` é **estrada** (`0x428fe0`); `0x80` = o tile tem **ponte** (peça com `+0x11 != 0`) |
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

**Multiplicador de terreno** (`0x540aa0` +0, f32) por classe [D]: 0 e 1 = 1,25; 2 = 0,8; 3,4,5,6 = 1; 7 = 1,5; 8 = 2; 9,10,11 = 1; 12 = 1,5; 13 = 1,25; 14 = 1,2; 15 = 1,35; 16 = 1,3; 17 = 2; 18 (água) = 1. Nomes do editor (lng 700..713) para as 14 primeiras classes: beach, desert, wasteland, dirt, dry grass, grass, lush grass, swamp, snow, wheat field, cotton field, sugar cane, mountain, ocean. A tabela id→classe (`0x581090`) é montada em `0x471b10` e **decodificada**: 19 classes ocupam ids consecutivos (`start_id` 0, 32, 59, 86, 128, 167, 209, 251, 291, 317, 353, 389, 425, 454, 479, 504, 529, 554, 590; 613 ids), cada uma com `n0..n3` ids próprios + 19 fillers + 3 ids por par de transição; `classe(id)` = a classe cujo intervalo contém o id, e ids >= 613 são classe 0 (`rt2_formats::terrain::tables().class_of_id`). O algoritmo completo e os grupos estão em `docs/formats/sprite-catalog.md` 4.1 e `docs/formats/map-gam.md` 3.2. Arquivos `0x233C` guardam ids antigos: classe = `0x552b00[id]` (`rt2_formats::terrain::old_class_of_id`). Para o simulador: a classe de um tile é `class_of_id(u16 >> 6)`; água = 18. **Árvores**: `tile_tree_count`: classe de vegetação (`tile+2 & 0xf`) 0 → 0; 1,2,4,5,7,8,9 → `tile+3 & 0x1f`; demais → 1; custo 1000 cada, antes do multiplicador de terreno.

**Pontes** (`bridge_cost_table` `0x53ce60`, i32[tipo*2 + simples]) [D], por tile de rio, multiplicado por `m(0xe)`: tipo 1 madeira dupla 90000 / simples 60000; 2 pedra 300000 / 200000; 3 aço 225000 / 150000; 4 150000 / 100000 (tipo extra, só por opção); 0 e 5 = 0. Disponibilidade (`max_bridge_type` `0x469ef0`): opção do cenário (`+0x1bf`) se ≠ 0; senão ano < 1840 → 1, < 1880 → 2, ≥ 1880 → 3 (ou 3 se a flag `0x573d7a`) [S]. Vão sobre água aberta: 120000 x `m(0xe)` por tile em modo vão, grátis se o tile já é vão (`tile+3 & 0x40`). Túneis: **não existem** no código lido.

### 2.3 Terraplenagem (earthwork)
Ao fixar um caminho, `track_level_edge` (`0x427ff0`, `0x4274b0`, `0x4278f0`) nivela os vértices da aresta: para aresta ortogonal, os dois vértices recebem `(h1 + h2 + h3 + 1) / 3` (três vértices da aresta e do canto), limitado a **[-255, 510]**; diagonais por `0x4278f0`. Não nivela se um tile vizinho já tem trilho que usa o vértice (tabelas `0x53a0b0`/`0x53a12c`). O jogo registra os vértices alterados (`0x5650f8` índices, `0x5650fc` alturas antigas, contagem `0x565100`) e cobra em `0x4cc060`:
```
terraplenagem = 600 * ftol(effect_multiplier(0xa, empresa) ) * Σ |h_novo - h_antigo|      ; 0x5357ac = 600
```
(fator 600 é f32; o `ftol` é do produto `600 * m(0xa)`.) O total do caminho é `Σ custo dos tiles + terraplenagem`, lançado no razão categoria 3 (`0x4398d0(empresa, 3, total)`) [S]. A rampa resultante de cada peça fica em `+0x12` via `track_segment_grade` (`trains.md` 4.1). **Não há limite rígido de rampa** na construção: só o custo da terraplenagem e a penalidade do A* (`0x463b30`: |dh| > 8 → custo do passo multiplicado por `dh²/64`, com `0x5746ec`) [S].

### 2.4 Demolição e reembolsos [S, razão V]
A ferramenta de **escavadeira** (comando 10, `bulldoze_area` `0x428930(empresa, x, y, raio, só_custo, &custo, &motivo, mensagem, silencioso)`, a UI de pré-visualização chama com `só_custo = 1`) roda duas passadas sobre os tiles a distância (truncada) `<= raio` do centro: a passada 0 valida e soma o custo; a passada 1 executa. Falha em qualquer tile aborta tudo, com o motivo (texto LNG `0x553868[motivo]`, ids 1121..1128): 1 trilho que é saída principal de uma estação, 2 trilho de **outra empresa**, 3 trem sobre a peça, 4 estação, 5 traçado ilegal, 6 caixa insuficiente (`caixa < custo`; ignorado com `0x5748fc`/`0x574b78`), 7 predio de outra empresa, 8 predio dentro do raio de estação de outra empresa (com `0x574b78`/`0x5748f8` ligados as checagens 7 e 8 não valem).

Custo (passada 0), tudo truncado em `ftol` e somado em `custo` (i32), mostrado arredondado para baixo ao milhar (`custo/1000*1000`):
- **tile com trilho** (`tile+2 & 0x20`): validar a remoção (`track_bulldoze_tile` `0x4286a0(x, y, empresa, &motivo, só_custo)` com ponteiro de motivo = ensaio; a peça pode ocupar também o tile de ponte/vão ligado) e `+1000` por tile;
- **árvores** (tile sem o bit `0x20`): `+1000 * árvores` (`tile_tree_count` `0x416860`, só se `net_min_peer_version >= 0x2331`, sempre em jogo local);
- **predio** (casa ou indústria; um por tile coberto pelo pincel): `x = effect_multiplier(0x130, empresa, 0, território do predio) * custo_do_tipo * k`, com `custo_do_tipo` = u32 em `0x5457ef + tipo*0x154` (a mesma tabela de 3.1) e `k = 0,1` (`0x535178`) se o predio é **da empresa** (`predio+0x97 == empresa`), `k = 0,75` (`0x535228`) se é neutro (`+0x97 == 0`, tipo != estação); `custo += ftol(x)` **e** `capitalizado += ftol(x)`.

Pagamento (`só_custo == 0`, depois da passada 0): se `0x5748f8 == 0`, `conta 2 (terrenos) += capitalizado` (o valor do predio limpo vira terreno; o caixa cai); `categoria 5 (Trilho) += -(custo - capitalizado)` (árvores e trilho são despesa); e a **reputação pública** da empresa cai: `empresa+0x4229 = ftol(+0x4229 - (sqrt(custo - capitalizado) + 200))` (campo 0..10000, 5000 na criação em `0x43ddb0`, sorteado e limitado a `[0, 10000]` na avaliação anual `0x43c760` e também reduzido por quebras e acidentes de trem, trains.md 5.3; a escavadeira **não** o limita a 0 na hora). Passada 1: para cada tile, `DAT_00567684 = 0; 0x4286a0` (executa a remoção, `tile_track_remove` `0x4162b0` soma `peça+0x15 * 8` em `DAT_00567684`) e com o resultado `V`:
```
categoria 5 += -V ; caixa += -V          ; 0x439970
caixa      += +V                         ; 0x4398d0(conta 0)
conta 3    += -V ; caixa -= -V           ; 0x4398d0(conta 3)
caixa      += -V
=> caixa  não muda; a conta 3 (trilho) cai V; a categoria 5 recebe -V (despesa de baixa); o patrimônio cai V
```
`V` é o **valor de livro da peça** que o jogo carrega no campo `peça+0x15` (i16): cada construção soma `custo_do_tile >> 3` (`0x4cc060`, só o custo de `0x463520` do tile, **sem** a terraplenagem 2.3), então `V` = soma dos custos de construção das peças removidas, com perda de até 7 por peça. A terraplenagem e a diferença ficam para sempre na conta 3 (e na base da manutenção mensal, 2.5). Depois a passada remove o predio do tile (`0x408e70` -> `building_destroy` `0x40adb0(predio, 1)`), que **paga a venda de indústria** abaixo se o predio era da empresa.

**Tabela de reembolsos (o que volta em caixa)**:

| Ativo | Reembolso | Lançamento | Origem |
|---|---|---|---|
| Trilho/ponte demolido | **nenhum** (custa `1000`/tile mais o valor de livro `V` baixado em despesa, categoria 5) | acima | `0x428930`, `0x4162b0` |
| Estação (remoção por liquidação/mundo; jogador não pode) | **nenhum**; `predio+0x99` (preço + melhorias) baixado: conta 1 `-= valor`, categoria 5 `+= -valor` | 3.4 | `0x4bf4e0` |
| Melhoria de estação | não existe venda; some com a estação. A troca de tamanho baixa o custo cru do tamanho antigo em categoria 4 | 3.4 | `0x46aef0` |
| Trem (venda/aposentadoria/acidente) | **nenhum**: `0x439930(empresa, conta 4, -preço_do_trem, categoria 6)` baixa a conta 4 e a categoria 6 (Motor) **sem mexer em caixa nem patrimônio** (única rotina que não faz partida dobrada) | trains.md 6.1 | `train_delete` `0x4cdd50` |
| **Indústria comprada** (comando 0x2d com flag 0 = venda, 1 = compra; ou predio da empresa destruído) | **metade**: caixa `+= V - (V >> 1)` (`V = predio+0x99`, o preço de compra), conta 5 `-= V`, categoria 3 `+= -(V >> 1)` | abaixo (6.1) | `company_industry_trade` `0x43eb60` |

Árvores e prédios neutros limpos não rendem nada. A **compra** de um predio (indústria) paga `ftol(effect_multiplier(0xd, empresa, 0, território) * custo_do_tipo)` (`building_price` `0x40bae0`, mesma tabela `0x5457ef`), se `território.empresa+0x479+t*0x1ed & 2` (permissão de comprar) e `preço <= caixa`: conta 5 `+= preço`, caixa `-= preço`, `predio+0x99 = preço`, `predio+0x97 = empresa` (`0x43ea30` faz a checagem, `0x43eb60` a compra). Um predio já da empresa devolve `predio+0x99` sem recalcular.

### 2.5 Limites e verificações extras
- Água: `track_edge_valid` retorna 0 em tile de classe 18 salvo vão/ponte (`param_8`) [S].
- Cidades: o caminho evita estações; objetos tipo 3 (prédios) no tile impedem via `tile+2 & 0x10`/estação [S].
- Trilho de outra empresa: `0x4cc060` só reaproveita tile se a peça é da mesma empresa (senão limpa `+0x23 & 0xfd`); `track_company_may_build_at` (`0x4603a0`) exige trilho próprio até 2 tiles de distância quando `0x5706c1/c2` ligados [S].
- **Limite de células** (`CompanyTrackCellsRemaining`): `empresa+0x4260` é decrementado pelo nº de tiles novos; com `0x57022a` ligado e saldo < 0, a construção falha (`0x574b7c = 1`) [S].
- Manutenção mensal de trilho (`0x4399d0`): `m(0xc) * (-0.005 * razão[3])`, ou seja 0,5% do valor lançado no razão 3, por mês (se o produto for negativo vira 0) [S].

## 3. Estações

### 3.1 Tipos e custo [D]
`building_type_table` `0x5457f8` (0x154 bytes por registro): `+0` tipo (0 = estação), `+1/+2` índices tamanho/estilo, pegada 10x10 em `+0xcd - 20*lin - 2*col` (u16), nome do sprite `+0x144` (`sta11..sta37`), custo u32 `+0x14b`. Estações: registros 0..7 pequenas (50000), 8..15 médias (100000), 16..23 grandes (200000), 8 estilos de arquitetura cada (só visual; a troca de tamanho soma 8 ou 16 ao índice, 3.4). O custo de um tipo `t` é o u32 em **`0x5457ef + t*0x154`** (`building_shape_table`, o mesmo endereço que `0x40bae0` e `0x428930` leem; é o que o código usa, e vale tanto para estações como para prédios e indústrias). Custo ao construir (`0x409a30`, `0x48eb30` em `0x40a45c`):
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

### 3.4 Melhorias de estação [D tabela, V compra/venda, S efeitos]

**Onde vivem.** Tabela no `.data` do EXE: `improvement_table` `0x5500c8`, 30 linhas de `0x1b` bytes (o índice da linha **é** o número do bit em `estação+0x49`). Layout de uma linha:

| Off | Tipo | Campo |
|---|---|---|
| `+0x00` | char[7] | chave interna ("STATNA", "WATER"...; em branco nas linhas sem uso). O código nunca a lê; o nome exibido vem do LNG (id `1043 + i`, `0x5537e0`) |
| `+0x07` | i32 | **custo base** (o código lê `0x5500cf + i*0x1b`) |
| `+0x0b` | i32 | primeiro ano |
| `+0x0f` | i32 | último ano (9999 = sem fim) |
| `+0x13` | i32 | 1 = comprável, 0 = desligada |
| `+0x17` | u32 | máscara de **exclusão**: a linha some da lista se a estação já tem o bit `i` **ou** qualquer bit desta máscara |

Descrições do LNG: `0x55381c[i]` (ids 1076..1098). Os bits `0x49` são `1 << linha`; a estação nasce com `1 << tamanho` (`0x4be840`: 1 pequena, 2 média, 4 grande).

| Bit | Linha (LNG 1043+i) | Custo | De..até | Exclui | Efeito e onde é aplicado |
|---|---|---|---|---|---|
| 0 | Small Station | 50 000 | 1800.. | 1,2 | raio de captação 2 (`0x54fd9c`, 3.3); tipo de predio 0..7 |
| 1 | Medium Station | 100 000 | 1800.. | 2 | raio 4; predio 8..15 |
| 2 | Large Station | 200 000 | 1800.. | - | raio 8; predio 16..23 |
| 3 | Telephone Poles | 80 000 | 1905.. | - | tempo de parada x0,5 (`0x4d1a4b`, `0x4d30fe`; const `0x535128`) |
| 4 | Telegraph Poles | 40 000 | 1837..1910 | 3 | tempo de parada x0,75 (const `0x535228`); só vale se não há telefone (testado depois dele) |
| 5 | Small Restaurant | 50 000 | 1800.. | 6 | passageiros +10% da receita, ao **dono da estação de destino** (`0x4bf760`, cargo.md 4.8) |
| 6 | Large Restaurant | 100 000 | 1850.. | - | passageiros +15% (substitui o 5) |
| 7 | Saloon | 16 000 | 1800.. | - | passageiros +5% |
| 8 | Post Office | 35 000 | 1840.. | - | correio: tempo de coleta contado pela metade (`cargo+0x1c`, cargo.md 7.6) |
| 9 | Customs House | 25 000 | 1800.. | - | atraso de alfândega 8 dias -> 4 dias por vagão (`0x4d29e0`, abaixo) |
| 10 | Sanding Tower | 30 000 | 1800.. | - | areia da locomotiva volta a 100 ao passar (`0x412ff0`, `& 0x400`) |
| 11 | Water Tower | 25 000 | 1800.. | - | água volta a 100 (`& 0x800`) |
| 12 | Turntable | 0 | - | - | **desligada** (flag 0); o bit 12 só existe derivado no editor (abaixo) |
| 13 | Second Track Line | 0 | - | - | desligada |
| 14 | Main Track Line | 0 | - | - | desligada |
| 15 | Large Hotel | 175 000 | 1878.. | - | passageiros +25% |
| 16 | Small Hotel | 65 000 | 1800.. | 15 | passageiros +15% |
| 17 | Liquid Storage | 50 000 | 1885.. | - | óleo, químicos: coleta pela metade |
| 18 | Refrigerated Storage | 50 000 | 1860.. | - | leite (e "produce"): coleta pela metade |
| 19 | Grain Silo | 35 000 | 1873.. | - | grão, café: coleta pela metade |
| 20 | Warehouse | 25 000 | 1800.. | - | bens, algodão, lã: coleta pela metade |
| 21 | Roundhouse | 120 000 | 1800.. | - | óleo volta a 100 (`& 0x200000`): a chance de quebra e a manutenção mensal dependem do óleo (trains.md 5.2, 6.2) |
| 22 | Engine Shop | 25 000 | 9999 (nunca) | - | desligada ("trens podem partir desta estação"; sem uso) |
| 23..29 | - | 0 | - | - | linhas vazias |

(A coluna "Exclui" lista os bits da máscara `+0x17`; por exemplo a máscara `6` de Small Station significa "some da lista se tiver Medium ou Large".) Os 19 compráveis são 0..11 e 15..21. Comprar o item "maior" quando o menor já existe (telefone sobre telégrafo, restaurante grande sobre pequeno, hotel grande sobre pequeno) **cobra o preço cheio e não devolve nada**; os dois bits ficam ligados e o código de receita escolhe o maior (`& 0x40` antes de `& 0x20`, `& 0x8000` antes de `& 0x10000`). O conjunto de expansão de indústrias (`0x5704ac != 0`) só deixa comprar as linhas 0..4, 10, 11 e 21.

Os bits 12 e 25..27 de `+0x49` são **derivados só do editor de mapas** (`0x4c0a50`, aparência do predio): 12 = tem Roundhouse ou Engine Shop (`& 0x600000`); 25 = isso e o tile-âncora tem trilho eletrificado; 26 = tem algum dos bits 5..9; 27 = tem hotel (`& 0x18000`); a mesma rotina normaliza o editor apagando o bit menor quando o maior existe (2 apaga 1 e 0, 1 apaga 0, 6 apaga 5, 15 apaga 16, 3 apaga 4). `0x4c0e60`/`0x4c0df0` ligam/desligam as linhas 0..25 sem custo no editor (`DAT_00566e14 == 2`). Nada disso roda no jogo.

**Disponibilidade** (`improvement_available` `0x468310`, `fastcall(ECX = linha)`) [V]:
```
se 0x5704ac != 0 e linha não está em {0..4, 10, 11, 0x15}: não
começou = ano > primeiro || (ano == primeiro && mês >= 9)         ; ano = u16 de game_date, mês = byte +2
vale    = (começou && ano <= último) || cenário(0x574bc8)+0x60 != 0   ; a opção do cenário ignora as datas
disponível = vale && flag(+0x13) != 0
```
Note o `mês >= 9`: um item de "1837" só aparece a partir de **setembro** de 1837 (para 1800 nunca importa). Telégrafo deixa de ser oferecido depois de 1910.

**Compra** (UI `0x407e80`, janela `buildbuy.win`, comando 0x18 -> `cmd_execute` -> `station_buy_improvements` `0x46aef0(ECX = estação, EDX = máscara)`) [V só para os bits que não são tamanho; o ramo de tamanho é S]:
```
preço_total = 0
para i em 0..30, se bit i da seleção:  preço_total = ftol( effect_multiplier(8, dono, 0, território_da_estação) * custo[i] + preço_total )
```
O multiplicador é o efeito 8 (Station Building) do **dono da estação** e do território dela (`estação+0x4ad`); o `ftol` trunca a cada bit (soma acumulada, não soma de truncados). A UI (`0x407540`, rotina de eventos da janela) só deixa confirmar se `preço_total <= caixa` (conta 0; `0x5748fc` ligado ignora a checagem) e mostra o preço arredondado ao milhar mais próximo (`(x + 500)/1000*1000`); o comando, a IA (`0x434850`) e a rede **não** repetem a checagem de caixa (a IA pode comprar com o caixa negativo). Só se compra em estação própria (a lista de `0x4010fa` só percorre estações da empresa). Lançamento:
```
razão: conta 1 (prédios) += preço_total - custo_antigo_do_tamanho ; caixa -= preço_total
       categoria 4 (Diversos) += -custo_antigo_do_tamanho   ; custo_antigo = custo base (sem multiplicador) da linha do tamanho antigo, 0 se não é troca de tamanho
predio.+0x99 (valor pago pelo prédio da estação) += preço_total - custo_antigo
estação.+0x49 |= seleção
```
**Troca de tamanho** (bits 1 ou 2; Medium cobra o preço cheio de uma Medium e assim por diante, como diz o manual): antes do lançamento o predio troca de tipo (`+6 += 8` para média, `+= 0x10` de pequena para grande, `+= 8` de média para grande), a pegada antiga sai dos tiles (`0x42e320`) e a nova entra (`0x409320`, `0x4071b0` refaz os elos, `0x4093e0`, `0x409770`); a UI antes testa `station_placement_valid` (`0x4087a0`) para a pegada maior no mesmo tile-âncora. O custo da troca é o preço cheio do novo tamanho; o valor **cru** do tamanho antigo é baixado em categoria 4 (nenhum reembolso). `estação+8` passa a 1 ou 2 e `+0x49` ganha o bit (o bit do tamanho antigo **não** é apagado; a exclusão da tabela é que o esconde).

**IA** (`ai_station_upgrades` `0x434850`, chamada por `0x434a40` e `0x435790` logo após a IA criar uma estação): sem checar caixa nem disponibilidade em alguns itens: tamanho >= 1: torre de água (bit 11) se ano < 1930 (`0x78a`) e ainda não tem; correio (bit 8); torre de areia (bit 10); tamanho 2: casa de máquinas (bit 21); saloon (bit 7) se `available(7)`; tamanho 2: restaurante grande (bit 6) se `available(6)`; tamanho >= 1 sem nenhum restaurante (`& 0x60`): restaurante pequeno (bit 5) se `available(5)`. Nunca compra hotéis nem armazéns.

**Efeitos fora da receita** (cada um com o endereço do código que o aplica):
- **Tempo de parada (3, 4)**: `train_station_work` (`0x4d18b0`, carga) soma `2880 min` por vagão carregado; `train_arrival_settlement` (`0x4d2c10`) soma `2880 * (vagões entregues + 1)`; em ambos o total vira `ftol(t * 0,5)` com o bit 3 ou `ftol(t * 0,75)` com o bit 4 (testados nessa ordem) e depois `ftol(t / max(effect(0x29, empresa do trem, 0, território), 0,1))`; `+0x71 = +0x6d + t` (trains.md 10.5).
- **Alfândega (9)** (`train_arrive` `0x4d29e0`): para cada vagão com carga que descarrega (`0x414510` com `commit = 1` devolve a indústria de destino), atrasa **11 520 min (8 dias)**, ou **5 760 min (4 dias)** se a estação de descarga tem o bit 9; só se a região (`território+0x2a`) da indústria de origem (`objeto+0x9d`) difere da do território da estação, ou se a indústria de destino é do tipo `0x1c` (Port). A opção `0x570239` desliga o teste de região (não o do Port). Os atrasos somam: `+0x71 = +0x6d + soma`, `0x4d3c80(trem, 1, 0, 9)` põe o trem no estado 9 e a liquidação `0x4d2c10` roda quando o tempo passa. Sem vagões sujeitos, liquida na hora.
- **Serviço de locomotiva (10, 11, 21)**: em `route_step` (`0x412ff0`), quando um veículo do trem entra no tile-âncora da estação (`tile+6 & 2`) cujo `+0x49` tem o bit, o trem recebe areia (`+0xcf`), água (`+0xd7`) ou óleo (`+0xd3`) = 100,0. Sem areia (<= 1): a rampa dobra (`0x4d00c0`, trains.md 4.3); sem água (vapor, <= 1): velocidade x0,5; sem óleo (<= 1): chance de quebra x3 e manutenção mensal x3 (trains.md 5.2, 6.2). O LNG ("sem areia, as rampas íngremes são duas vezes mais difíceis") confere com o código; o manual diz "50% mais lento".
- **Armazenamento (8, 17..20)**: `cargo+0x1c` guarda o **bit** da melhoria de cada carga (cargo.md 3); com o bit, `p1 = p1 >> 1` no tempo de espera do lote (cargo.md 7.6).
- Bônus de passageiros: `B = 0,15 R` (bit 6) ou `0,10 R` (bit 5) mais `0,25 R` (bit 15) ou `0,15 R` (bit 16) mais `0,05 R` (bit 7), tudo ao **dono da estação de destino** (cargo.md 4.8). O texto do `default2.lng` final (ids 1081..1083, 1091, 1092: 10/15/5/25/15%) **confere com o código**; só o manual e o guia dizem 20/30/10/40/20%.
- Manutenção: **nenhuma melhoria tem manutenção própria**; o valor pago (`predio+0x99`) entra na conta 1 e **não é depreciado**.

**Preço, efeito 8, e outros ids de efeito vistos aqui**: `0x8` = custo de estação e de melhoria; `0x29` = tempo de parada; `0x3c` = receita de estação; `0xd` = **preço de compra de indústria** (resolve o id sem nome do editor; ver 6.1); `0x130` = custo de limpeza (2.4).

**Venda/remoção de estação** (`station_remove` `0x4bf4e0`, chamada por `building_destroy` `0x40adb0(predio, 1)`; só por liquidação de empresa e por funções de mundo; o jogador **não pode** vender nem demolir estações: mensagem 1124) [V no razão]: se o predio não é "fantasma" (`predio+0x2e == 0`) e a estação tem dono: `empresa+0x172 -= 1`; razão: `conta 1 -= predio+0x99` **e** `categoria 5 (Trilho) += -predio+0x99`; o caixa **não muda** (o valor pago, melhorias incluídas, é baixado como despesa; **nenhum reembolso**). Depois: tira os elos (`0x406f60`), refaz redes (`0x46a370`), apaga a marca de estação do tile, remove as paradas dessa estação de todas as rotas de trem (`0x4d5710`), e se a estação era a "principal" da empresa (`empresa+0x176`), escolhe outra estação dela.

## 4. Edição de terreno por jogadores
O modo de ferramenta `0x574cb0` (`0x48eb30`) tem os casos 1 (subir/descer), 9/0x12 (pincel), 10/0xb (balde), 0x10 e 0x13 com sons `raise61m`, `brush61m`, `buckt61m`: pertencem ao editor de mapas (manual: só no editor). Nenhum deles lança custo no razão (a lista de chamadores de `0x4398d0` não os inclui) [S]. O único corte/aterro pago é a terraplenagem do trilho (2.3). Limpar árvores é parte do custo do tile (2.2).

## 5. Sinais, bloqueio, prioridade e via dupla [V no teste de conflito, S no resto]

**Não existem sinais de via, blocos nem "caixas" de sinalização** no código: nenhum tipo de peça, texto LNG ou global os descreve (o "semáforo" do manual é o indicador de espera de carga da parada, `+3 & 0xc0` das paradas, trains.md 10.1). O que faz o papel de bloco é a **reserva por tile**: cada peça de trilho tem dois "donos" de via (`peça+7`, e `peça+9` só em via dupla) e uma lista de reservas (`pool_80`, trains.md 10.1). Um trem só entra num tile (`0x414dd0`, `route_step`) se o conflito de `tile_track_conflict` (`0x415ff0` -> `track_piece_conflict` `0x416050`) deixa; ele reserva do passo da cabeça (+1 se passou da metade do tile) até o fim do último vagão. Resumo do modelo:

- **Um bloco = um tile** (trecho de uma peça). Em **via simples** (`peça+0x23 & 1 == 0`) só um trem por vez, nos dois sentidos e no mesmo sentido (o seguidor espera o líder sair do tile). Em **via dupla** há duas faixas: o lado vem de `0x536c4c[direção_de_entrada]` = `{2,1,1,1,1,2,2,2}` (guardado nos bits `0x06` do passo da rota), então os sentidos opostos usam faixas diferentes (e **nunca conflitam** numa reta). Numa peça dupla com mais de duas arestas (junção/cruzamento), só há conflito se as faixas se cruzam (comparação de arcos abaixo).
- **Não há detecção de colisão física.** Não existe "trem bateu no outro": os acidentes são sorteios (trains.md 5). O que impede dois trens de ocuparem o mesmo tile é o conflito acima; o trem barrado espera onde está (estado 1 "esperando via", `reserve(trem, 1, 1, 1)`), solta as reservas dele, e a cabeça tenta de novo a cada passo.
- **Nada no código resolve impasses** (inferido; não observado em jogo): dois trens de mesma prioridade frente a frente em via simples se barram para sempre (nenhum vence a regra abaixo); só o jogador (mudar a classe) ou um evento (quebra, prioridade 20) desfaz.

**Regra de conflito** (`track_piece_conflict`, `thiscall(tile, _, trem, a, b)` com `a` a direção de entrada e `b` a de saída do trem que quer entrar; devolve 1 = barrado) [V, `tests/stations.rs`]:
```
p = prioridade(trem) - 1                       ; prioridade = trem+0xa8 (0..255), como inteiro com sinal
para faixa em {0, 1}:                          ; a faixa 1 só se a peça é dupla
    h = dono da faixa (peça+7 / +9);  se h == 0 ou h == trem: continua
    se a peça é dupla:
        d = (saída_de_h - b) & 7                       ; saída_de_h = peça+0xd/+0xe
        se 2 <= d <= 6:                                ; saídas não vizinhas
            se ((1<<a | 1<<b) & máscara) == máscara: continua    ; peça só tem as arestas a e b
            lo = entrada_de_h*2 ;  hi = saída_de_h*2 + 1          ; entrada_de_h = peça+0xb/+0xc
            dentro(x) = lo < hi ? (lo < x && x < hi) : (hi < x && x < lo)
            se dentro(a*2) == dentro(b*2 + 1): continua           ; caminhos não se cruzam
    ; conflito de fato:
    se p < 1  ou  prioridade(h) > p:  devolve 1      ; barrado
    reserve(h, 1, 1, 1)                                ; o dono h PARA e solta a peça (0x4d3c80)
devolve 0
```
Ou seja o trem que quer entrar só vence se a sua prioridade for **estritamente maior** que a de cada dono em conflito e pelo menos 2; ao vencer ele **para o dono** (a peça passa a ser dele no passo seguinte).

**Prioridade `+0xa8`** (recalculada em cada tile novo, `train_enter_tile` `0x4d5370`, e quando a classe muda, `0x4d59b0`): `+0xa8 = 0` se o acelerador `+0xdb <= 65` (trem "parado/lento demais" nunca ganha); senão `+0xa8 = classe` (`+0xa7`), mais `4` se o tile sob a locomotiva é de trilho **da própria empresa**; `20` se o trem quebrou/acidentou (`0x4ce4c0`, `0x4ce730`, `0x4ceaa0`: ninguém o empurra, mas ele também não se move). A classe (`train_set_class` `0x4d59b0`, comando 0x21; o clique no ícone da lista envia `(classe + 1) & 3`) tem 4 valores, texto LNG em `0x553858` (ids 1109..1112):

| `+0xa7` | Classe | Efeito |
|---|---|---|
| 0 | Halted | o trem **não parte** (`0x4d0a70` exige `+0xa7 != 0`) e um trem em marcha é parado no lugar (`reserve(trem, 1, 0, 5)`, trains.md 10.3) até mudar a classe |
| 1 | Slow | prioridade 1 (< 2): nunca vence um conflito; cede a todos |
| 2 | Normal | prioridade 2 (6 em trilho próprio); padrão de um trem novo (`0x4cdf80`) |
| 3 | Express | prioridade 3 (7 em trilho próprio): passa qualquer trem de classe menor; **dois Express** (ou dois Normal) de mesma prioridade se travam |

A IA (`0x4d8f40`, `0x4d90f0`) põe a classe 3 em trens cujas duas primeiras paradas ficam a mais de 80 de distância (`0x4f0cd0`) e 2 nos demais. O bônus de +4 em trilho próprio faz um Normal em casa (6) ganhar de um Express visitante (3); trens de mesma prioridade se barram mutuamente. O manual descreve a ordem Express > Normal > Slow certo (3.6) mas **não** o bônus de trilho próprio, o limite de acelerador (65) nem o impasse de iguais.

**Caminhos de código que verificam ocupação** (todos já em trains.md 10.4/10.5): `route_step` (`0x412ff0`) testa o passo seguinte com `0x415ff0` e, barrado, chama `reserve(trem, 1, 1, 1)`; `vehicle_advance_head` (`0x415040`) falha com `reserve(trem, 1, 1, 7)`; `train_depart_check` (`0x4d0a70`) usa `route_blocked` (`0x4d3e30`); o intervalo reservado é mantido por `0x4d0bc0` -> `0x4d3950` (reservas `0x4d36a0`/`0x4d3770`).

**Via dupla** (bit `+0x23 & 1`): custo 1,5x (2.2); no movimento é o que dá duas faixas (acima). Não há "passagem" automática em desvio: o jogador precisa de via dupla ou de um trecho de desvio por onde o pathfinder (custo ortogonal 2, diagonal 3; 6/9 em trilho alheio, trains.md 10.7) encaminhe o trem. **Junções** são as formas T/Y da tabela 1.4 (mais de duas arestas na máscara da peça).

## 6. Propriedade e taxas de uso [S]
Dono da peça = `peça+2` (id da empresa; 0 = sem dono: trilho pré-existente do mapa). Troca de dono na reconstrução: `0x4cc060` mantém a peça se `dona == empresa`, senão remove bits de previsão.

**Não existe taxa por uso de trilho alheio.** A única coisa que o dono do trilho recebe é uma **fatia da receita** do trem que passa; não há cobrança por tile nem por vagão, e a estação alheia cobra uma **taxa fixa por vagão**. Regras (acumulação em `train_enter_tile` `0x4d5370`, liquidação em `train_arrival_settlement` `0x4d2c10`, cargo.md 6):

1. **Contagem por trecho de percurso.** A cada tile novo que a **cabeça** do trem pisa (`0x4d5370` só roda para a cabeça, via `route_step`, e só se o tile mudou), o trem soma 1 em `+0x7d` (total de tiles do trecho) e conta o dono da peça sob ele (`peça+2`): se é a empresa do trem (`+0x41`) soma 1 em `+0x7f` (próprio); se é outra, procura o dono nas até 4 entradas `+0x82` (u16 empresas) e soma 1 na parte `+0x8a` (i16) dele, criando a entrada (parte 1) se há lugar (`+0x81 < 4`); **a partir do quinto dono estranho o tile conta como próprio** (`+0x7f`), e o trilho sem dono (`peça+2 == 0`, id 0) ocupa uma entrada como "empresa 0". Todos os tiles contam igual (reta, diagonal, ponte, estação).
2. **Zeragem.** O trio `+0x7d/+0x7f/+0x81` volta a 0 a cada troca da parada atual (`train_set_stop` `0x4d1630`, também na criação do trem e em `0x4d5560`), e o tile atual é recontado em seguida: a "milhagem" é por **trecho entre duas paradas**, não acumulada. (A ordem exata entre a troca da parada, feita pelo planejador de rota, e a liquidação em `train_arrive` não foi traçada em jogo; a liquidação usa os contadores como estão quando roda.)
3. **Partilha na chegada** (depois de descarregar): `bruto = Σ receita dos vagões`; `taxas = $2000 por vagão entregue` na estação da parada anterior (`trem+0x9e`) **e** `$2000` na atual; cada estação **com dono** recebe a sua taxa em categoria 2 (Station) (mesmo se o dono é a empresa do trem), e só é **descontada** do trem a taxa da estação cujo dono **não** é a empresa do trem (inclui estação sem dono: a taxa some). `líquido = max(0, bruto - taxas descontadas)`. Para cada dono estranho `j` listado e ativo (`empresa+0x16e != 0`; empresa extinta ou id 0: **a parte se perde**), o razão de `j` recebe, em categoria 4 (Diversos), `líquido * (f32)(parte_j / total)` (f32, **sem** `ftol`: centavos fracionários). A empresa do trem fica com `líquido * (f32)(próprio / total)` (distribuído em passageiros/correio/carga como em cargo.md 6, item 6). As taxas saem do bruto **antes** da partilha, então o dono do trilho paga parte da taxa de estação do trem proporcional à fatia dele.
4. **Quem é o "dono do trem" para as taxas**: `trem+0x41` (a empresa que comprou o trem); a taxa e a partilha são computadas para cada trem separadamente.

Consequências: só a **receita de entregas** (não a manutenção) é repartida; um trem que roda 100% em trilho próprio paga só as taxas das estações alheias; o jogador não consegue cobrar pedágio por tile, só construir a estação onde os outros descarregam. A manutenção mensal do trilho de cada empresa é de 0,5% da conta 3 dela (2.5), independente do uso; os campos `peça+0x17/+0x1b` (peso total que cruzou a peça, `+= trem+0xaf` por entrada da locomotiva; `+0x1b` passa para `+0x1f` no fim do ano, `0x4a4d00`) **não são lidos por nenhuma regra de economia** que tenhamos achado: servem à interface (mapa de uso). O campo `peça+0x15` é o valor de livro usado na demolição (2.4).

## 7. Verificação diferencial
`crates/rt2-oracle/tests/track.rs` (CW 0x27f):

| Teste | Função | Casos | Resultado |
|---|---|---|---|
| `track_tile_cost_matches_original` | `0x463520` | 30 000 (tile, peça, globais, modos 0..7, vão, arestas) | custo e custo2 idênticos |
| `station_catchment_matches_original` | `0x407030` | 20 000 (tamanhos 0..2, prédios 24..63) | retorno e distância idênticos |

`track_tables_match_emulated_memory` compara as tabelas que `rt2_sim::track::TrackTables` lê do EXE (pontes, multiplicadores de terreno, estado econômico, raios, pegadas) com a memória emulada. A implementação testada é `rt2_sim::track::{track_tile_cost, station_catchment}`. Limite: nos testes a empresa é 0 e a região do tile 0 (sem listas de empresas/territórios); `effect_multiplier` com empresa/território é coberto por `tests/cargo.rs`. Tile com `+2 & 0x80` sem peça trava o original (registro 0): nunca ocorre no jogo.

`crates/rt2-oracle/tests/stations.rs` (CW 0x27f; a referência em Rust está no próprio arquivo, escrita a partir desta spec; chamadas só de repintura/religação — `0x406f60`, `0x42e410`, `0x46a370` — viram `ret` na cópia emulada e `0x4d3c80` vira um gravador de chamadas):

| Teste | Função | Casos | Resultado |
|---|---|---|---|
| `improvement_table_matches_emulated_memory_and_spec` | tabela `0x5500c8` (30 linhas) lida do EXE e da memória emulada; custos, anos, máscaras e a lista de linhas compráveis de 3.4 | 30 linhas | idênticas |
| `improvement_availability_matches_original` | `0x468310` (ano, mês >= 9, conjunto de expansão, opção de cenário `+0x60`) | 6 000 | idêntico |
| `improvement_purchase_matches_original` | `0x46aef0` (ramo sem troca de tamanho): preço acumulado com `ftol` por bit, efeito 8 com empresa e território, razão (conta 1, categoria 4, carimbo), `predio+0x99`, máscara `+0x49` | 1 500 | idêntico |
| `station_removal_writes_off_value_matches_original` | `0x4bf4e0`: conta 1 e categoria 5 `-= predio+0x99`, caixa intacto, contador de estações, predio "fantasma", estação sem dono | 800 | idêntico |
| `industry_purchase_and_sale_match_original` | `0x43eb60` (compra e venda) e `0x40bae0` (preço com efeito `0xd`; predio já da empresa): venda devolve `V - (V >> 1)` | 1 500 | idêntico |
| `tile_conflict_priority_matches_original` | `0x416050` (via simples/dupla, arcos, prioridades 0..20): bloqueio e quais donos são parados | 20 000 | idêntico |

Não cobertos por teste (S): troca de tamanho de `0x46aef0` (mexe em tiles e elos), `0x428930` (escavadeira), turnaround/alfândega (`0x4d18b0`, `0x4d29e0`) e a partilha de trilho (`0x4d5370`, `0x4d2c10`).

## 8. Cobertura de funções
Nomeadas em `re/pending/track.toml` e `re/pending/stations.toml` (melhorias, vendas, escavadeira, prioridades: `0x468310` [V], `0x46aef0` [V/S], `0x4bf4e0` [V], `0x43eb60` [V], `0x40bae0` [V], `0x416050` [V], `0x428930` [S], `0x4286a0` [S], `0x434850` [S], `0x4d59b0` [S], `0x4d5560` [S], `0x407e80` [S, UI]). Lidas e descritas: `0x463520` [V], `0x407030` [V]; [S]: `0x463a20`, `0x4cc060`, `0x4164f0`, `0x416210`, `0x4162b0`, `0x4ca920`, `0x462b80`, `0x4087a0`, `0x409a30`, `0x4071b0`, `0x42e8f0`, `0x42e850`, `0x469ef0`, `0x416860`. Só nomeadas: `0x460850` (A* com listas abertas, hash de nós; também roteia trens com `param_7 == 0`), `0x463b30`, `0x427ff0`, `0x460640`, `0x460520`.

## 9. Divergências do manual
- **Custo base**: o manual dá 4000/6000 por "quadrado"; no código é `2000` por meia aresta (2 arestas por tile) x1,5 para dupla; eletrificação 6000/9000 por tile. Confere em solo plano; aresta diagonal custa 1,4x (não citado).
- **Terreno**: areia (praia e deserto) +25%, pântano e montanha +50%, neve x2 conferem; o código também tem `wasteland` x0,8 (barato) e classes 13..17 (1,25; 1,2; 1,35; 1,3; 2,0) que o manual não cita. Árvores +1000 conferem.
- **Pontes**: preços por tile de rio conferem (pedra 200k/300k, aço 150k/225k, madeira 60k); a madeira dupla (90k) existe nos dados mas é forçada a simples; ano de disponibilidade no código: madeira desde o início, pedra 1840, aço 1880 (o manual diz 1830/1840/1880).
- **Vão sobre água**: 120000 por tile (o manual de TSC diz 120k por seção; o guia 150k).
- **Rampa**: sem limite rígido; o manual sugere custo alto em declives, que aqui sai da terraplenagem 600 x Σ|dh|.
- **Túneis**: inexistentes, como no manual.
- **Estações**: custos 50k/100k/200k conferem; captação pequena 5x5 e discos de diâmetro 9 e 17 conferem (raios 2/4/8 com distância truncada).
- **Melhorias de estação (tabela 5-1 do guia)**: custos e anos **conferem** em todos os itens (telefone 80k/1905, telégrafo 40k/1837, alfândega 25k, saloon 16k, restaurantes 50k/100k/1850, hotéis 65k/175k/1878, armazém 25k, correio 35k/1840, refrigerado 50k/1860, silo 35k/1873, líquidos 50k/1885, água 25k, areia 30k, casa de máquinas 120k). Divergências: (1) bônus de passageiros 10/15/5/15/25% no código e no LNG final, o manual e o guia dizem 20/30/10/20/40%; (2) o telégrafo **deixa de ser vendido depois de 1910** (não citado); (3) item com data aparece só a partir de **setembro** do ano (`mês >= 9`); (4) sem areia o código **dobra a rampa** (g -> 2g), o guia diz "50% mais lento"/"metade da velocidade"; (5) sem água, vapor x0,5 (o guia: "muito mais lento"); (6) trocar para o item maior não dá crédito pelo menor, e não há venda de melhorias; (7) há linhas desligadas (Turntable, Second/Main Track Line, Engine Shop 25k) que o manual não cita; (8) alfândega: 8 dias por vagão sujeito, 4 com a melhoria ("reduz 50%" confere).
- **Taxa de estação**: o manual diz $1.000 por carga; código $2.000 por vagão em cada ponta (cargo.md 6); a estação sem dono cobra e **ninguém recebe**.
- **Uso de trilho alheio**: `docs/reference/manual-rules.md` não traz a regra; no código não há cobrança por uso, só a partilha da receita por tiles do trecho (6), com limite de 4 donos estranhos.
- **Sinais e prioridades**: o manual (Express/Normal/Slow/Halted) confere; não cita o +4 em trilho próprio, o acelerador > 65 (prioridade 0) nem que iguais se travam (5).
- **Reembolsos**: o manual não diz; código: nenhum reembolso por trilho, estação, melhoria ou trem; indústria vendida devolve metade (2.4).

## Em aberto
- Nomes das classes 13..17 (rochas escura/marrom/cinza/bege e rocha com costa pelo aspecto; sem nome no `.lng`).
- Troca de tamanho de estação (`0x46aef0`, bits 1 e 2): o valor de livro e o tipo do predio estão descritos, mas o ramo não tem teste diferencial; a validação de espaço na UI (`0x4087a0` para a pegada maior) foi lida só em linhas gerais.
- Escavadeira (`0x428930`): descrita de leitura e do asm do custo; sem teste diferencial. `DAT_00567684` só é zerado e usado ali. O que a reputação `empresa+0x4229` controla fora da avaliação anual (`finance.md` 8.1) não foi traçado.
- Momento exato da troca da parada (e da zeragem da milhagem de trilho) em relação a `train_arrive`/`0x4d2c10`; só o efeito (contadores por trecho, até 4 donos estranhos) está fechado.
- Impasse de trens de mesma prioridade (5): inferido do código, não observado em jogo.
- `0x439930` baixa a conta e a categoria sem mexer em caixa nem patrimônio (venda/acidente de trem): a identidade de partida dobrada de `finance.md` parece quebrar; não investigado.
- Significado dos bytes de forma `aux` de `0x5365e8` e custos do A* (`0x463b30`) só em linhas gerais.
- Resolvidos aqui: melhorias (3.4), taxa/partilha de trilho e de estação (6), sinais/prioridades (5), reembolsos (2.4), id de efeito `0xd` (preço de compra de indústria), `custo2` (só marca que "algo foi construído" em `0x4cc060` ao lado de `custo`; não vai ao razão), `0x4cdd50` (apaga o trem, sem reembolso), opção de cenário `+0x1bf` (pontes disponíveis: 0 = por ano, 1..5 = limite fixo; menu `0x4658f0`, ids `0xa582..`).
