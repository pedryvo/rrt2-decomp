# Crescimento do mundo: cidades, prédios, indústrias e eventos

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido no C exportado (`re/db/decomp`), em disassembly (`rizin`) e confirmado por teste diferencial no oráculo (`crates/rt2-oracle/tests/growth.rs`, seção 9). O manual (`docs/reference/manual-rules.md`, seção 6) serviu só de pista; onde o código diverge, a divergência está na seção 8. Endereços são VAs (base `0x400000`). Estado das provas: **[V]** = confirmado contra o original (bit a bit, inclusive o estado do RNG); **[S]** = lido no código, sem teste. Tabelas numéricas vêm do `.data` do executável (o clone as lê em tempo de execução: `GrowthTables::load`, `crates/rt2-sim/src/growth.rs`); aqui aparecem só os valores necessários para entender as regras.

Produção, demanda, preço e a tabela de tipos de indústria (`0x541ae0`) estão em `docs/spec/cargo.md`; o estado econômico (`0x46a010`) em `cargo.md` seção 9; o relógio e a ordem dos passos periódicos em `docs/spec/time.md`; o RNG em `docs/spec/rng.md`; tiles e estruturas em `docs/spec/terrain-track.md`; finanças em `docs/spec/finance.md`. A ordem exata dos ganchos do tick fica em `docs/spec/world-tick.md` (outro autor); aqui só se diz qual gancho chama cada função.

## 1. Visão geral

O mundo "cresce" por três mecanismos, todos disparados por passos periódicos:

1. **Anual (23:59 de 28/dez, `sim_year_end` `0x4a4d00` -> `town_year_step` `0x430a40(0)`)**: cada **lugar** (cidade ou região, `list_towns` `0x574c5c`) compara o número de prédios que tem com um alvo e, ao acaso, cria ou demole prédios. Os prédios criados são casas (tipo `0x12`) e **indústrias de qualquer tipo** (é assim que surgem indústrias novas: não existe fundação de indústria separada). Pouco antes, no mesmo fim de ano, cada prédio recalcula sua **lucratividade** (`0x40b450`), que decide quem é demolido.
2. **Mensal (`sim_month_end` `0x4a5320`)**: com chance pequena e dependente do ano, uma cidade sem estação por perto **oferece um bônus** (US$ 100 mil a 500 mil) à primeira ferrovia que a ligue a outra cidade (`0x431660`, pago por `0x46a370`).
3. **Compra e venda de indústrias** pelas empresas (comando `0x2d`, `0x43eb60`; a IA decide em `0x436950`).

**Não há desastres** (enchentes, incêndios, greves): nenhum gancho periódico do mundo os sorteia. Os únicos eventos gerados pelo código base são: o estado econômico semestral (cargo.md 9), a oferta de bônus de conexão, o anúncio do primeiro prédio de cada tipo e as manchetes históricas de fim de ano que avisam que as cidades deixaram de aceitar certas cargas (seção 6). Os eventos de cenário (`event_fire_trigger`) são outra spec.

### 1.1 Mapa de funções

| Endereço | Nome (símbolo) | Papel | Prova |
|---|---|---|---|
| `0x430a40` | `town_year_step` | passo anual: pesos de escassez, `town_update` por lugar, decaimento dos contadores | [S] |
| `0x430540` | `town_update` | crescimento/encolhimento de um lugar (seção 3) | [V] |
| `0x430400` | `town_building_stats` | contagem de casas, outros prédios e tamanho; taxa de produção do lugar | [V] |
| `0x42ffc0` | `town_add_building` | escolhe o tipo do prédio novo (seção 4.1) | [V] |
| `0x42fcb0` | `town_place_building` | procura o terreno e cria o prédio (4.2) | [V] |
| `0x42fb10` | `site_classes` | classe de floresta e de declive de um terreno (4.3) | [V] |
| `0x40ba00` | `building_shape_pick` | forma (rotação/variante) do prédio novo (4.2) | [V] |
| `0x4302c0` | `town_remove_worst_building` | escolhe o prédio a demolir (3.6) | [V] |
| `0x40adb0` | `building_destroy` | demolição (remove da lista do lugar, da carga e do mapa) | [S] |
| `0x409010` / `0x409a30` | `building_create` / `building_construct` | aloca e inicializa o prédio (4.4) | [S] |
| `0x40bb40` | `building_init_slot_clocks` | relógios de produção iniciais (4.4) | [V] |
| `0x430c20` | `place_at` | lugar dono de um tile | [S] |
| `0x430e70` | `towns_recompute_extent` | caixa e nº de tiles de terra de cada lugar | [S] |
| `0x4317f0` | `town_reset` | zera contadores e arma o período sem demolição | [S] |
| `0x42fa30` | `nearest_town_id` | cidade mais próxima (mensagem do primeiro prédio) | [S] |
| `0x40b450` | `industry_year_step` | lucratividade anual do prédio (5.1) | [V] (sem dono) |
| `0x40b7c0` | `industry_year_reset` | zera os contadores anuais do prédio | [S] |
| `0x4e2330` | `slot_valid` | o slot de saída vale no ano? | [V] |
| `0x40bae0` | `building_price` | preço de compra (5.3) | [S] |
| `0x43ea30` / `0x43eb20` | `building_buy_request` / `building_sell_request` | pedidos de compra/venda (enfileiram o comando `0x2d`) | [S] |
| `0x43eb60` | `company_industry_trade` | efetiva a compra ou venda (5.3) | [S] |
| `0x436950` | `ai_industry_investment` | a IA escolhe uma indústria para comprar (5.4) | [S] |
| `0x431660` | `town_bonus_offer` | oferta mensal de bônus de conexão (6.2) | [V] |
| `0x46a370` | `station_networks_rebuild` | reagrupa redes e paga o bônus (6.2) | [S] |

Ganchos que chamam estas funções (detalhe em `world-tick.md`):

- `sim_year_end` (`0x4a4d00`): `0x40b450` para cada prédio (antes), `0x430a40(0)` (se `0x570237 == 0`), `sim_week_end`, `0x40b7c0` para cada prédio (depois); define `0x5705d1` (fator de crescimento, 3.2).
- `sim_month_end` (`0x4a5320`): sorteio e `0x431660` (6.2); `0x46a370` quando `0x5794f0 != 0` (a flag é armada por `0x4caaa0`/`0x4cc060`, mudanças de trilho/estação; `0x46a370` também roda direto nessas mudanças).
- Criação do mundo `0x474280` (mapa novo): `0x430e70`, `0x430a40(1)`, `sim_week_end`. Carga de jogo `0x479a30`: `0x430e70` (sem `0x430a40(1)`). `game_set_date` `0x46b650`: `0x4317f0` por lugar e `0x40b820` por prédio. Cheat `Viagra` (`0x46c8a0`): liga `0x574c14` e chama `0x430a40(1)` na hora.
- Entregas: `cargo_delivery_revenue` (`0x4bf760`, ramo de confirmação) alimenta os contadores que servem de "serviço" ao crescimento (3.5).

## 2. Estruturas

### 2.1 Lugar (`list_towns` `0x574c5c`, 180 = `0xb4` bytes)

| Off | Tipo | Campo |
|---|---|---|
| `+0` | u16 | id |
| `+2` | u16 | primeiro prédio da lista do lugar (lista encadeada: o prédio guarda o próximo em `+4`; prédios novos entram na **cabeça**, `0x430d80`) |
| `+4,+6` | i16 | centro (x, y) em tiles |
| `+8,+0xa,+0xc,+0xe` | i16 | caixa x0, y0, x1, y1 (recalculada em `0x430e70`) |
| `+0x10` | u8 | tipo: 0 cidade, 1 região, 2 rótulo (LNG 1146..1148); só 0 e 1 crescem |
| `+0x11` | u8 | copiado para `+0x52` de prédios sem tipo (uso na estação) |
| `+0x12` | f32 | soma das taxas anuais dos slots primários válidos dos prédios (`0x430400`) |
| `+0x16` | char[] | nome |
| `+0x2c` | u8 | **classe de tamanho** 0..4 (Village, Town, City, Metropolis, Megalopolis; LNG 1152..1156): raio, alvo e bônus |
| `+0x2d` | u8 | máscara de bits de serviço (zerada se nenhuma estação "livre" existe; `0x42f910`); não afeta o crescimento |
| `+0x2e` | i32 | **índice de crescimento** em % (50..300; 100 = neutro) |
| `+0x32` | i32 | tiles de terra do lugar (exclui água e neve; `0x430e70`); só regiões o usam |
| `+0x36` | i32 | bônus de conexão pendente (0 = nenhum) |
| `+0x3a` | f32 | como `+0x12`, só para prédios com estação ligada |
| `+0x3e` | u16 | lotes **entregues ao** lugar neste ano (decai 0,8 por ano) |
| `+0x40` | u16 | lotes **despachados do** lugar neste ano (decai 0,8 por ano) |
| `+0x42` | u32 | minuto antes do qual **não há demolição**; `game_set_date` grava `agora + 0x1d8800` (4 anos, 3.7) |
| `+0x46` | u16 | **intensidade** em % (80..120 cidade; 95..105 região) |
| `+0x48` | u8 | nº de casas (máx. 255) |
| `+0x49` | i8 | **tendência** da região, -2..6 (fora disso volta a 0) |
| `+0x4a` | u16[53] | **peso de cada tipo de prédio** no lugar (0 = não nasce; o editor usa 0..200) |

### 2.2 Prédio (`list_industries` `0x574c74`, `0x9f` bytes; é um pool misto de casas, indústrias e estações)

O pool contém todo prédio do mapa: casas, indústrias e os prédios de estação do jogador (tipo 0). Campos relevantes (ver também cargo.md seção 2):

| Off | Tipo | Campo |
|---|---|---|
| `+0` | u16 | id |
| `+2` | u16 | lugar dono (`0x430c20` na criação) |
| `+4` | u16 | próximo prédio do mesmo lugar |
| `+6` | u8 | forma (índice em `building_shape_table` `0x5457ef`) |
| `+7` | u8 | **tipo** (0 = prédio de estação do jogador; 1..52 = tabela `0x535c38`) |
| `+8` | u16 | id da estação (só prédios de estação) |
| `+0x26..+0x2c` | i16 | caixa do prédio (x0, y0, x1, y1) |
| `+0x2e` | u8 | 0 = prédio "real"; != 0 = decorativo (não conta, não produz, não demole por crescimento) |
| `+0x33` | i8 | nº de estações ligadas |
| `+0x34` | u16 | lotes recebidos neste ano |
| `+0x36` | i16 | lucro anual / 1000 |
| `+0x3c` | u16 | ano de construção |
| `+0x4b` | u16 | **lucratividade** (rating) 0..250, 100 = neutro |
| `+0x4f` | u8 | 1 = demolido |
| `+0x53` | u32[4] | relógios de produção dos 4 slots |
| `+0x63` / `+0x67` | i8[4] | lotes **entregues a estação que os demanda** / **produzidos**, por slot de saída, neste ano |
| `+0x93` | f32 | lucratividade suavizada (inicial 0,5) |
| `+0x97` | u16 | empresa dona (0 = ninguém) |
| `+0x99` | i32 | preço pago pelo dono |
| `+0x9d` | i8 | território |

Um prédio novo nasce com `+0x4b = 0` (o registro vem zerado; **[S]**: a alocação `0x4f1690` não foi lida) e `+0x93 = 0,5`: até o primeiro fim de ano ele é o candidato preferido à demolição.

### 2.3 Tabela de tipos de prédio (`building_type_table` `0x535c38`, 53 registros de `0x27` bytes) [V]

| Off | Campo |
|---|---|
| `+0` u8 | id de gráfico/ícone (não usado pela simulação) |
| `+1` u8 | 1 = comprável (lido só pela IA em `0x436950`; tipos 0, 18, 38, 39, 43, 44, 47, 48 têm 0) |
| `+2` i32 | peso base em % (100 em todos) |
| `+6` i32 | não lido pelas rotinas de crescimento (50, 70, 80, 100, 110, 120, 45, 60, 20 em alguns tipos) |
| `+0x0a` u16 | **tamanho** em "unidades de prédio" (o que conta para o alvo de indústrias) |
| `+0x0c` i32 | **primeiro ano** em que o tipo pode ser fundado |
| `+0x10..+0x13` i8 | faixa de classe de floresta `[lo, hi]` e de declive `[lo, hi]` aceitas (4.3) |
| `+0x14` u8[19] | tipo aceito na classe de terreno `c` (0..18) |

Valores (extraídos do `.data`): tamanho 2 na maioria; 3 em Aluminum Mill, Coffee/Cotton/Grain/Sugar/Produce/Rubber; 4 em Nuclear Plant e Steel Mill; 1 em Housing; 0 em Station. Classes de terreno vedadas: 8 (neve), 9, 10, 11 (campos de trigo, algodão, cana) e 18 (água) para quase todos; as 1 e 2 (deserto, terra devastada) e 12 (montanha) também para Cattle Yard, Dairy Farm, Grain Silo, Rubber Farm e Sugar Farm; Port aceita água; Station aceita tudo. Como a classe 8 é vedada para todo tipo que passa pelo filtro, o ramo "neve" de `0x42fcb0` (41% de chance) é **inalcançável** com a tabela original.

Primeiro ano (coluna `+0x0c`) e faixas (floresta / declive) por tipo:

| tipo | nome | tam. | 1º ano | floresta | declive | tipo | nome | tam. | 1º ano | floresta | declive |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Aluminum Mill | 3 | 1910 | 0..2 | 0..1 | 20 | Landfill | 2 | 1950 | 0..2 | 0..0 |
| 2 | Auto Plant | 2 | 1900 | 0..2 | 0..1 | 21 | Logging Camp | 2 | 1800 | 1..2 | 0..2 |
| 3 | Bakery | 2 | 1800 | 0..2 | 0..1 | 22 | Lumber Mill | 2 | 1800 | 0..2 | 0..1 |
| 4 | Bauxite Mine | 2 | 1910 | 0..2 | 0..1 | 23 | Meat Packing Plant | 2 | 1800 | 0..2 | 0..1 |
| 5 | Cannery | 2 | 1856 | 0..2 | 0..1 | 24 | Nuclear Plant | 4 | 1950 | 0..2 | 0..1 |
| 6 | Cattle Yard | 2 | 1800 | 0..0 | 0..1 | 25 | Oil Refinery | 2 | 1890 | 0..2 | 0..1 |
| 7 | Cement Plant | 2 | 1884 | 0..2 | 0..1 | 26 | Oil Well | 2 | 1860 | 0..2 | 0..0 |
| 8 | Chemical Plant | 2 | 1905 | 0..2 | 0..1 | 27 | Paper Mill | 2 | 1800 | 0..2 | 0..1 |
| 9 | Coal Mine | 2 | 1800 | 0..2 | 1..2 | 28 | Port | 2 | 1800 | 0..2 | 0..1 |
| 10 | Coffee Farm | 3 | 1800 | 0..2 | 0..2 | 29 | Produce Orchard | 3 | 1800 | 0..1 | 0..1 |
| 11 | Cotton Farm | 3 | 1800 | 0..1 | 0..1 | 30 | Rubber Farm | 3 | 1900 | 0..2 | 0..1 |
| 12 | Dairy Farm | 2 | 1800 | 0..1 | 0..1 | 31 | Sheep Farm | 2 | 1800 | 0..2 | 0..1 |
| 13 | Dairy Processor | 2 | 1881 | 0..2 | 0..1 | 32 | Steel Mill | 4 | 1856 | 0..2 | 0..1 |
| 14 | Electric Plant | 2 | 1890 | 0..2 | 0..1 | 33 | Sugar Farm | 3 | 1800 | 0..2 | 0..1 |
| 15 | Fertilizer Plant | 2 | 1905 | 0..2 | 0..1 | 34 | Textile Mill | 2 | 1800 | 0..2 | 0..1 |
| 16 | Grain Silo | 3 | 1800 | 0..1 | 0..0 | 35 | Tire Factory | 2 | 1900 | 0..2 | 0..1 |
| 17 | Gravel Pit | 2 | 1884 | 0..2 | 0..1 | 36 | Tool and Die Factory | 2 | 1800 | 0..2 | 0..1 |
| 18 | Housing | 1 | 1800 | 0..2 | 0..1 | 37 | Uranium Mine | 2 | 1950 | 0..2 | 0..1 |
| 19 | Iron Ore Mine | 2 | 1800 | 0..2 | 0..2 | 38..52 | (sem nome, expansão) | 2 | 1800 | 0..2 | 0..1 |

(tipo 0, Station: tamanho 0, 1800, floresta 0..2, declive 0..1.) Faixa `0..2` de floresta junto com `0..2` de declive significa "sem restrição" (`unconstrained`).

### 2.4 Tabela de formas (`building_shape_table` `0x5457ef`, `0x7a` registros de `0x154` bytes) [V]

`+0` i32 **preço de compra base**, `+9` u8 **tipo** do prédio, `+0xa` u8 classe de tamanho e `+0xb` i8 direção 0..7 (prédios de estação), `+0xc,+0xd` largura e altura (metade = centro), `+0xe,+0xf` deslocamento do canto, `+0x10..+0xd7` máscara de pegada 10x10 de u16 (percorrida de trás para frente: linha `r`, coluna `c` em `+0xd6 - 20r - 2c`). As formas 0..23 são os prédios de estação (3 tamanhos x 8 rotações: US$ 50 mil, 100 mil, 200 mil); 25..32 são casas (tipo 0x12, US$ 100 mil); o resto são indústrias (por exemplo forma 57 = Steel Mill, US$ 800.000; forma 60 = Aluminum Mill, US$ 500.000).

### 2.5 Globais

| Endereço | Tipo | Significado |
|---|---|---|
| `0x5705d1` | f32 | **fator de crescimento** `g` (3.2) |
| `0x570223` | i32 | nível econômico inicial em % (padrão 100, `0x472980`); editor "Annual Growth Rate" (LNG 2293) |
| `0x57022b` | f32 | taxa anual composta (padrão `0x3ba3d70a` = 0,005); o editor grava `(controle - 50) * 0,001` |
| `0x570713` | i32 | nº de prédios "industriais" do mundo (tipo != 0, `+0x2e == 0`); `+1` em `0x409a30`, `-1` em `0x40adb0` |
| `0x57070f` | i32 | lotes entregues na janela atual (+1 por entrega confirmada; decai 0,8 por ano) |
| `0x5706dd` | u32 | minuto do último passo anual (`dt` = agora - isto) |
| `0x570508` | u32 | data de início do jogo (ano = 16 bits baixos) |
| `0x5702cb` | 53 x 9 B | por tipo: `+0` u8 tipo **desativado** pelo cenário; `+1` i32 **peso de escassez** em %; `+5` i32 (padrão 2, não lido) |
| `0x570603` | u8[53] | "este tipo já foi visto" (1 se o ano inicial já passou ou se já foi construído um; `game_set_date`, `0x409a30`) |
| `0x5704ac` | u8 | conjunto de indústrias (0 padrão, 1 expansão: **sem casas**) |
| `0x573d78` | u8 | modelo industrial (0 básico, 1 avançado, 2 expert) |
| `0x574c14` | i32 | cheat Viagra (dobra o alvo de casas) |
| `0x570717` | i32 | estado econômico 0..4 (cargo.md 9) |

Tabelas de crescimento (`.data`):

| Endereço | Tipo | Valores | Uso |
|---|---|---|---|
| `0x53a464` | f32[10] | 4, 2, 7, 4, 12, 6, 17, 8, 28, 12 (índice `classe*2 + pass`) | alvo base: pass 0 = casas (4, 7, 12, 17, 28), pass 1 = tamanho de indústrias (2, 4, 6, 8, 12) |
| `0x53a48c` | f32[9] | 0,5, 0,75, 1, 1,5, 2, 4, 7, 10, 13 (tendência -2..6) | alvo de regiões |
| `0x5504e4` | f32[5] | 0,90, 0,95, 1,00, 1,05, 1,10 | multiplicador de cidade por estado econômico |
| `0x5504f8` | f32[5] | 0,96, 0,98, 1,00, 1,02, 1,04 | multiplicador de região por estado econômico |
| `0x5503f4` | i32[5] | 4, 6, 7, 9, 11 | raio máximo de colocação por classe de tamanho |
| `0x550408` | i32[5] | 16, 36, 49, 81, 121 | raio² de captação do lugar por classe |
| `0x53a4b0` | i32[5] | 100000, 200000, 300000, 400000, 500000 | bônus de conexão por classe |

## 3. Cidades e regiões

### 3.1 Quando e com que passo

`town_year_step(initial)` (`0x430a40`) roda **uma vez por ano** em `sim_year_end` (e uma vez na criação do mundo com `initial = 1`). Ignora tudo se `0x5748f8 != 0`, ou se `0x573d7a != 0` (modo editor) e não é a chamada inicial. Com `dt = agora - 0x5706dd` (o passo anual vale 483 840 minutos; o primeiro depende da data de início) faz, em ordem:

```
0x5706dd = agora
para t em 0..52: escassez[t] = 100                                   ; 0x5702cc + 9t
se não initial:
    n[t] = nº de prédios do tipo t em list_industries (todos, inclusive casas e estações)
    escassez[t] = (n[t] < 3) ? 500 / (n[t] + 1) : 100                ; 500, 250, 166, 100
para cada lugar L na ordem da lista ligada (head -> next):
    se L.+0x2d != 0 e nenhuma estação com +0x45 == 0 existe: L.+0x2d = 0
    town_update(L, dt, initial)                                       ; seção 3.3
    L.+0x3e = ftol(L.+0x3e * 0,8 + 0,5);  L.+0x40 = ftol(L.+0x40 * 0,8 + 0,5)
0x57070f = ftol(0x57070f * 0,8 + 0,5)
```

Os lugares são percorridos na ordem da lista ligada (`pool_nth_list_order`), não por id.

### 3.2 Fator de crescimento global `g` (`0x5705d1`)

Definido em dois lugares [S]:

- `game_set_date` (`0x46b650`): `g = nível% * 0,01` (1,0 por padrão).
- Todo fim de ano (`sim_year_end`, antes de `0x430a40`): `g = pow(1 + taxa, ano - ano_inicial + 1) * nível% * 0,01`, com `ano` o ano em curso (28/dez, antes de virar), `taxa = f32 0x57022b` (0,005) e `pow` = `_CIpow` (`fp::pow`). Com os padrões: 1,005^(n+1): ~1,16 após 30 anos.

### 3.3 `town_update(L, dt, initial)` (`0x430540`) [V]

```
se L.tipo ∉ {0, 1}: devolve
se initial: dt = 10000
se initial ou L.intensidade ∉ [80,120]:                              ; [R1]
    L.intensidade = (tipo == 0) ? rng(80,120) : rng(95,105)
para pass em (1, 0):                      ; 1 = indústrias (tamanho), 0 = casas
    se conjunto 1 e pass == 0: continua                                ; sem casas
    ; alvo (int), depois guardado como f32
    se tipo == 0 (cidade):
        alvo = ftol( ((g * K[2*classe + pass]) * eco_cidade[estado]) * (intensidade * 0,01) )
        se Viagra e pass == 0: alvo = 2 * alvo
    senão (região):
        se pass == 0: continua                                        ; só indústrias
        se tendência ∉ [-2,6]: tendência = 0
        alvo = ftol( ((tr[tend] * ((g+1) * 0,5)) * eco_regiao[estado] * (intensidade * 0,01) * terra) * 0,01 )
    ; laço de recontagem (um passo por demolição; ver abaixo)
    removeu = falso
    repete:
        (H, O, S) = town_building_stats(L)                  ; casas, outros prédios, soma dos tamanhos
        cur  = (pass == 0) ? H : S ;  n_ind = (pass == 0) ? H : O
        x1 = (0x570713 != 0) ? (H + n_ind) / 0x570713 : 0              ; parcela de prédios do lugar
        x2 = (0x57070f != 0) ? (L.+0x3e + L.+0x40) / (2 * 0x57070f) : x1  ; parcela das entregas
        se região: x2 *= 1,5
        razão = (x1 <= 0) ? 1,0 : min(x2 / x1, 16,0)
        L.indice = clamp(ftol(razão * 20 + 80), 50, 300)
        anos = ano - ano_inicial
        se anos < 12: L.indice = ((L.indice - 100) * anos + 1200) / 15       ; divisão inteira, sem clamp
        N = ftol( (L.indice * 0,01) * f32(alvo) )
        se tipo == 0 e N <= 1: N = 1
        se initial e N > 5: N = N - 1
        p = f32( ((N - cur) / max(cur, 1)) * f32(dt) * 1,18102797e-7 )       ; ~ (N-cur)/cur / 17,5 por ano
        se initial:
            se p >= 0: sai do laço
            initial = falso                                           ; a partir daqui não é mais inicial
        se p >= 0 ou agora < L.+0x42: sai do laço
        se rng(0,1000000) >= -p * 1e6: sai do laço                   ; [R2] demolição com probabilidade |p|
        town_remove_worst_building(L, casa = (pass == 0));  removeu = verdadeiro
        volta a "repete"
    ; demolição espontânea ("churn")
    a = f32(dt * 1,24007937e-8)                                       ; 0,006 por prédio por ano
    se não initial e agora >= L.+0x42:
        n = (pass == 0) ? H : O                                       ; do último town_building_stats
        se n > 4 e rng(0,1000000) < n * a * 1e6:                      ; [R3]
            town_remove_worst_building(L, casa = (pass == 0));  continua (próximo pass)
    se removeu ou p <= 0: continua
    ; crescimento: um prédio por iteração, com probabilidade p
    repete:
        se não initial e p * 1e6 <= rng(0,1000000): sai                ; [R4]
        se não town_add_building(L, casa = (pass == 0)): sai
        (H, O, S) = town_building_stats(L);  cur = (pass == 0) ? H : S
        p = f32( ((N - cur) / max(cur,1)) * f32(dt) * 4,1335980e-7 )  ; ~ (N-cur)/cur * 0,2 por ano
        se p <= 0: sai
```

Pontos importantes:

- **Dois alvos por lugar**: casas (pass 0, cresce o nº de casas) e "tamanho" de indústrias (pass 1, a soma dos tamanhos `+0x0a` de todos os prédios que não são casas, **estações do jogador inclusas**). O passo 1 roda antes do 0.
- **O alvo é multiplicado pelo índice `L.+0x2e`** (50%..300%). O índice mede o quanto o lugar recebe serviço relativo ao resto do mundo: `x1` é a fatia dos prédios do mundo que o lugar tem (casas + outros / `0x570713`) e `x2` a fatia dos lotes entregues e despachados no ano (3.5). Lugar com serviço proporcional ao tamanho: razão 1 -> índice 100; sem entregas: razão 0 -> 80; razão 16 ou mais -> 300 (três vezes o alvo, o "três vezes o tamanho original" do manual).
- **Rampa dos primeiros 12 anos**: com `anos < 12` o índice vira `((idx - 100) * anos + 1200) / 15`; no ano 0 todo lugar tem índice 80 e a rampa vai até `0,8 * idx` (+-) no ano 11; no ano 12 volta a valer o índice cheio (degrau; mantido por fidelidade). Durante a rampa não há clamp (o valor pode passar de 300 ou cair abaixo de 50 antes da divisão).
- **Probabilidades por ano** (`dt = 483 840`): crescimento inicial `p = (N-cur)/cur / 17,5`; depois do primeiro prédio criado a regra repete com `p = (N-cur)/cur * 0,2`; encolhimento `|p|` com a primeira fórmula (aplicada a cada recontagem até o alvo ou o sorteio falhar); demolição espontânea `0,6%` por prédio (só com mais de 4 do tipo e fora do período protegido).
- **Chamada inicial** (`initial = 1`, mundo novo e cheat Viagra): `dt = 10000`, a intensidade é sempre re-sorteada, o alvo perde 1 se passa de 5, e o laço de crescimento **não sorteia**: cria prédios até `cur >= N` ou `town_add_building` falhar.
- A demolição só ocorre depois de `L.+0x42` (`game_set_date` arma `agora + 1 935 360` minutos = 4 anos). Crescimento não tem essa trava.

Exemplo (valores padrão, estado normal, 1870, `g = 1,0`): cidade classe 2 (City), intensidade 100: alvo de casas 12, alvo de tamanho 6; com índice 100, `N = 12` e `6`. Região com tendência 2 e 3000 tiles de terra: `alvo = 2,0 * 1,0 * 1,0 * 1,0 * 3000 * 0,01 = 60`.

### 3.4 Contagem de prédios (`town_building_stats` `0x430400`) [V]

Percorre a lista do lugar (`L.+2`, `prédio.+4`). Para cada prédio: tipo `0x12` (conjunto 0) -> `H += 1`; senão `O += 1` e `S += tamanho[tipo]`. Também soma a taxa (`f32` do slot, em f32) de cada slot de saída com modo primário (`+4 != 0`), saída `!= 0x7f` e `ano_de..ano_até` válido ao ano: em `L.+0x12`; e em `L.+0x3a` se o prédio tem estação ligada (`+0x33 != 0`). `L.+0x48 = min(H, 255)`.

### 3.5 O que alimenta o crescimento

- Só duas coisas entram na fórmula além do ano e da economia: o **número de prédios** (que o próprio crescimento produz) e a **fatia de entregas** do lugar. Cada lote entregue de forma confirmada (`0x4bf760`, ramo `commit`) soma 1 em `estação_origem.+0x4b7`, `estação_destino.+0x4b9`, **`+0x40` do lugar da estação de origem**, **`+0x3e` do lugar da estação de destino** e em `0x57070f`. Os dois contadores decaem 20% ao ano. Logo uma cidade cresce quando recebe e despacha carga (passageiros, correio, mercadorias) em proporção maior que sua fatia de prédios; o motor não olha número de estações, população nem tipo de carga.
- A classe `+0x2c`, a intensidade, a economia (cargo.md 9, +-10% cidade / +-4% região) e o nível global `g` (1,005 por ano) escalam o alvo sem depender do jogador.
- Prédios de estação do jogador (`tipo 0`) contam em `O` mas têm tamanho 0 (não aumentam `S`).

### 3.6 Encolhimento (`town_remove_worst_building` `0x4302c0`) [V]

```
melhor = 999999,0f ; escolha = nenhuma
para cada prédio B da lista do lugar, na ordem:
    pula se B.+8 != 0 (estação) ou B.+0x97 != 0 (de empresa) ou conjunto 1
    pula se (B.tipo == 0x12) != casa                       ; casas só no pass 0, o resto no pass 1
    v = rng(0x58, 0x70) * B.+0x4b                          ; [R5] um sorteio por prédio elegível
    se (f32) v < melhor: melhor = v ; escolha = B
se escolha: building_destroy(escolha, 1)
```
O prédio de **menor lucratividade** (com ruído de +-10%) da classe pedida é demolido; prédios de empresas e prédios de estação são imunes, e prédios novos (rating 0) são os primeiros da fila. Não há falência de indústria por outra via: **uma indústria só fecha quando o lugar encolhe** (laço de encolhimento ou demolição espontânea, 3.3), e a escolhida é a menos lucrativa; uma indústria sem nenhum serviço continua viva enquanto o alvo do lugar a comportar.

`building_destroy` (`0x40adb0`) [S]: decrementa `0x570713` (se real), tira o prédio da lista do lugar (`0x430da0`), percorre a pilha de lotes (`+0x4d`, próximo `+0xf`): cada lote perde a indústria (`+1 = 0`) e o que não está num vagão (`+5 == 0`) é liberado [V: placar Flood], destrói a estação ligada, devolve os tiles (`0x42e320`), limpa a máscara de pegada e marca `+0x4f = 1` (4.5).

### 3.7 Reinício (`town_reset` `0x4317f0`) [S]

`+0x36 = 0` (sem bônus), `+0x3e = +0x40 = 0`, `+0x2e = 100`, `0x57070f = 0`, `+0x42 = agora + 0x1d8800` (1 935 360 minutos = 4 anos de 483 840 minutos).

### 3.8 Extensão dos lugares (`0x430c20`, `0x430e70`) [S]

`place_at(x, y, qualquer)` devolve o lugar dono do tile: a **cidade** (tipo 0) mais próxima (distância² ao centro) cuja caixa contém o tile (ignorada se `qualquer`) e cuja distância² não passa de `0x550408[classe]`; se nenhuma, a **região** (tipo 1) mais próxima cuja caixa contém o tile (cidades têm preferência, independente da ordem na lista). `0x430e70` (criação/carga) recalcula por varredura de todos os tiles com `qualquer = 1`: caixa = extensão dos tiles do lugar; `+0x32` = tiles cuja classe de terreno não é 18 (água) nem 8 (neve); lugar sem tiles recebe o mapa inteiro.

## 4. Prédios novos (onde e quais)

### 4.1 Escolha do tipo (`town_add_building(L, _, casa)` `0x42ffc0`) [V]

```
excluidos = {0}                                       ; o prédio de estação nunca nasce
se casa:
    se conjunto 0: devolve town_place_building(L, 0x12)        ; sem sorteio de tipo
senão: se conjunto 0: excluidos += {0x12}
cont[t] = nº de prédios do tipo t no lugar (primários — slot 0 com modo != 0 — contam 0)
para t em 0..52:
    se L.+0x4a[t] == 0 ou ano < primeiro_ano[t] ou desativado[t]: excluidos += {t}
    peso[t] = excluido ? 0 : ftol( (base[t] * 0,01) * (escassez[t] * 0,01) * L.+0x4a[t] )   ; base = 100
cands = tipos não excluídos (em ordem crescente); se vazio: devolve falso
orçamento = 3000 ; limite = 0x400 ; k = 0
repete:
    i = rng(0, |cands| - 1) ; t = cands[i]                                     ; [R6]
    se (região ou cont[t] <= k) e rng(0, limite) < peso[t]:                    ; [R7]
        se town_place_building(L, t): devolve verdadeiro
        remove t de cands ; se vazio: devolve falso
    orçamento -= 1
    se orçamento == 0 e limite > 8: limite >>= 2 ; orçamento = 500
    k = (3000 - orçamento) >> 10
    se orçamento == 0: devolve falso
```

- O sorteio é ponderado por `peso[t] / (limite + 1)`, e o limite cai de 1024 para 256, 64, 16 e 4 a cada 3000/500/500/500/500 tentativas: tipos de peso baixo ainda saem, só demoram mais. No máximo 5000 iterações.
- **Cidades** só aceitam um tipo que ainda não existe no lugar (`cont[t] <= k`, com `k = 0` nas primeiras 1024 iterações, `1` até 2048 e `2` depois); tipos **primários** (minas, fazendas: o slot 0 produz por taxa) ignoram a contagem. **Regiões** não têm essa restrição.
- **Escassez**: tipos raros no mundo (menos de 3 prédios) têm o peso multiplicado por 5, 2,5 ou 1,66 (`500/(n+1)` %), o que garante que cada tipo apareça em algum lugar; é recalculado a cada passo anual (a chamada inicial usa 100).
- **Ano**: só o `primeiro_ano` da tabela 2.3 vale (não há ano final: indústrias nunca "saem de moda"). O peso por lugar vem do mapa (`+0x4a`, 0..200).
- O tipo 0x12 (casa) nunca entra na escolha ponderada no conjunto 0; no conjunto 1 não há casas.

### 4.2 Escolha do terreno (`town_place_building(L, tipo)` `0x42fcb0`) [V]

```
tentativas = 300 (0x2904 = 10500 para Port 0x1c; 900 para 0x27, 0x28, 0x29); x3 se região
forma = building_shape_pick(tipo, sorteia = verdadeiro)                       ; [R8]
raio = (tipo == 0x12) ? 1 : 0
enquanto tentativas != 0:
    se tentativas & 7 == 7: raio = min(raio + 1, raio_max[classe] + 1)        ; 1º aumento na tentativa 295
    se cidade:
        a = rng(0,raio); b = rng(0,raio); dx = ftol(sqrt(a*b))               ; [R9] 4 sorteios (os de faixa 0 não avançam o RNG)
        c = rng(0,raio); d = rng(0,raio); dy = ftol(sqrt(c*d))
        se rng(0,1) == 0: dx = -dx ; x = cx + dx
        se rng(0,1) == 0: dy = -dy ; y = cy + dy
    senão (região): x = rng(x0, x1) ; y = rng(y0, y1)
    ok = place_at(x, y) == L.id
         e ( sem_restrição[tipo]
             ou ( floresta ∈ [lo,hi] e declive ∈ [lo,hi]       ; site_classes(x, y)
                  e aceito[tipo][classe_do_tile(x, y)]
                  e (classe != 8 ou rng(0,100) < 41) ) )        ; ramo inalcançável com a tabela original
    se ok:
        se tipo == 0x1c (Port): para k em 0..3: se building_can_place(x, y, 0x54 + k): criar(x, y, 0x54 + k); devolve verdadeiro
        senão se building_can_place(x, y, forma, cedo = tentativas > 300): criar(x, y, forma); devolve verdadeiro
    tentativas -= 1
devolve falso
```
- A distribuição de `dx` é `sqrt(a*b)`, com `a, b` uniformes em `[0, raio]`: mais denso perto do centro; o raio cresce de ~0/1 a `raio_max + 1` (5, 7, 8, 10, 12) ao longo das tentativas.
- `building_can_place` (`0x4087a0`) valida a pegada 10x10: regras completas em 4.6.
- `building_shape_pick` (`0x40ba00`) [V]: tipo `> 0x30` vira `0x12`; para os tipos 10, 11, 0x10, 0x12, 0x1c, 0x1d, 0x1e, 0x21 e 0x27..0x29 sorteia `n = rng(1, 24)`, senão `n = 1`; devolve a `n`-ésima forma (a partir do índice 0, circular em `0x7a`) cujo tipo bate.

### 4.3 Classes de floresta e declive (`site_classes` `0x42fb10`) [V]

Amostra 9 pontos em cruz em torno de `(x, y)`: o centro (peso 32), os 4 vizinhos a distância 1 (peso 24) e a distância 2 (peso 16); só os que estão no mapa contam. **Floresta**: soma de `(tile+3 & 0x1f) * peso` nos pontos com `(tile+6 & 4) != 0` e `(tile+2 & 0xf) ∈ {1, 2}`; `pct = soma * 100 / soma_pesos`; classe 0 se `pct < 4`, 1 se `4..20`, 2 se `> 20`. **Declive**: `|h(x,y) - h(x+1,y+1)|` (cantos da elevação, i16) `* 100`; classe 0 se `<= 180` (diferença <= 1), 1 se `<= 500` (2..5), 2 acima.

### 4.4 Criação (`building_create` `0x409010` -> `building_construct` `0x409a30`) [S]

Aloca um registro em `list_industries` e o inicializa: `+7 = tipo da forma`; se tipo != 0, `0x570713 += 1`; `+2 =` o lugar passado pelo crescimento (ou `place_at(x, y)` se nenhum foi passado); relógios `+0x53` por `0x40bb40` (abaixo); `+0x93 = 0,5`; `+0x47 = 1,0`; `+0x3c = ano`; contadores zerados; cria casas/indústrias com `+0x2e = 0`; o prédio entra na **cabeça** da lista do lugar (`0x430d80`); marca o terreno e refaz o relevo (4.5; campos de fazenda, `0x409120`/`0x429fc0`/`0x42a3e0`/`0x42a7d0`, com RNG por tile: 4.5.1) e liga as estações ao alcance. Se `0x570603[tipo] == 0` (primeiro prédio do tipo desde o início do jogo) dispara a manchete "A new type of building has been built for the first time near X" (LNG 94; 95 sem cidade), onde X é a cidade mais próxima por `0x42fa30` (distância inteira), e marca o tipo como visto.

`building_init_slot_clocks` (`0x40bb40`) [V]: para cada slot `k` (4, mesmo vazios):

```
relógio[k] = agora ; se ano < slot.ano_de: relógio[k] = slot.ano_de * 483840
se o prédio é real (+0x2e == 0):
    r = rng(0, 100)                                                         ; [R10]
    m = 1,1 se a carga de saída é 1; 1,25 se é 0; senão 1,42
    taxa' = (taxa > 0,1) ? taxa : 1,0
    relógio[k] = ftol( f32(relógio[k]) - ((r * (483840 / taxa')) * 0,01f) * m )
```
ou seja, o primeiro lote do prédio sai até um período antes do normal, o que espalha as primeiras produções (cargo.md 7.3: `relógio = min(relógio, agora)`).

### 4.5 Marcação do terreno de `building_create` e `building_destroy` [V no Example, 30 dias]

Tudo isto mexe no mapa (`tiles`, `elevation`) e no pool de estruturas `0x574c54`; sem RNG para os tipos de pegada simples (casas e a maioria dos prédios: células só com `& 0xc3`). Funções em `crates/rt2-sim/src/tilemark.rs`; prova: o Example em 30 dias (2 casas criadas, 2 prédios de 2 tiles demolidos) bate em `tiles`, `elevation`, `structures` e `stations` com o original (1099 + 268 + 65 + 33 campos antes).

**Pegada.** `building_shape_table` (`0x5457ef`, `0x154` bytes por forma): `+9` tipo do prédio, `+10`/`+11` (`0x5457f9`/`fa`) orientação e deslocamento de trilho (estações), `+12`/`+13` (`0x5457fb`/`fc`) largura e altura, `+14`/`+15` (`0x5457fd`/`fe`) deslocamentos da célula de região; a grade `10x10` de palavras de flags termina em `0x5458c5` e é lida **de trás para a frente**: a célula `(lin, col)` está em `0x5458c5 + 0x154*forma - 20*lin - 2*col` e cai no tile `(cx - col, cy - lin)`. Bits: `0x1`/`0x2` solo, `0x40` campo (textura), `0x80`, `0x100`/`0x200` água/estrada, `0x2c1`/`0x102` terra/água (4.6); `& 0xc3` faz parte do prédio, `& 0xc1` do nivelamento, `& 0x43` da captação (`terrain-track.md` 3.3).

**Geometria do registro** (`0x409a30` + `0x409770`), com `(x, y)` a âncora passada:
```
+0x1a,+0x1c = (x, y)            +0x1e = x + (largura >> 1)      +0x20 = y + (altura >> 1)
+0x22 = +0x1e - b(0x5457fd)     +0x24 = +0x20 - b(0x5457fe)     +0x9d = região do tile (+0x22,+0x24)
+0x26,+0x28,+0x2a,+0x2c = caixa (xmin, ymin, xmax, ymax) das células & 0xc3   (inicia 9999,9999,0,0)
cantos: para k em 0..4, entre as células & 3 (lin externo, col interno; primeiro máximo vence):
    chave k=0: tx*1001 + ty*1000 ; k=1: ty*1001 + (L-tx)*1000 ;
    k=2: (L-tx)*1001 + (A-ty)*1000 ; k=3: (A-ty)*1001 + tx*1000          (L, A = largura e altura do mapa)
    +0x0a+2k = tx ; +0x12+2k = ty                (0xffff se nenhuma célula)
```
**Criação** (na ordem do original; `id` = slot do prédio):
1. `0x409770` fim: para cada célula `& 0xc3` dentro do mapa (`lin` externo, `col` interno) `tile_struct_add(tile, 3, id)` (`0x42e850`: cadeia do tile ordenada por tipo decrescente, o novo vai depois do último de tipo `>=`; aloca no pool de estruturas, **a ordem de alocação decide os ids que os marcadores dos trens recebem depois**; liga `tile+2 |= 0x10`).
2. `0x409580` (nivelamento): para as células `& 0xc1` dentro do mapa, os 4 cantos do tile (deslocamentos `0x580fd8` = `0, 1, L+2, L+1` em índices de `(L+1)*y + x`; sem repetição); `m = ((n >> 1) + soma) / n` (divisão inteira com sinal) e cada canto vira `(h + m + 1) >> 1`, limitado a `-255..510` (`0x426000`).
3. Células `& 0x40` (campos): `0x409120` guarda o tile em `pool_88` e as texturas de campo são pintadas (`0x429fc0` tipos 10/0x1d/0x1e, `0x42a3e0` tipos 0xb/0x10/0x21, depois `0x42a7d0` no retângulo dos campos), com RNG por tile: 4.5.1. `0x409580` também acumula `0x417770(caixa +- 1)` no retângulo de redesenho (passo 7).
4. `0x4093e0`: sprites do prédio (somente runtime).
5. Estações ligadas (`0x4072c0`, só prédios com tipo != 0): para cada estação da lista com `+0x45 == 0`, se `station_catchment_test` (`0x407030`: distância da estação à âncora `<= raio + 11` e menor distância às células `& 0x43` `<= raio`; raio por tamanho em `0x54fd9c`) cria um elo em `pool_58` `{u16 prédio, u16 estação, u16 distância, i32 próximo do prédio (+0x2f), i32 próximo da estação (+0x3d)}`, cabeça nova nos dois, `prédio+0x33 += 1`, `estação+0x41 += 1`. Depois `0x40a610(prédio, 1)` (pilhas).
6. `0x424c10(x-4, y-4, x+4, y+4)` com a âncora `(+0x1a, +0x1c)` (flags do terreno, abaixo).
7. Fim: o prédio vai para a cabeça da lista do lugar (`0x430d80`), a manchete do primeiro do tipo (4.4) e, no redesenho adiado (`0x417c40` -> `0x417880`), `tile+6 & 1` = "tile plano" (os 4 cantos com a mesma elevação) é recalculado para o retângulo `caixa +- 1` (`0x417770`). Um tile plano tem sempre o bit; o carregador também o calcula (conferido em todos os tiles do Example: 55395 planos, 34605 não).

**Flags do terreno** (`0x424c10(x0, y0, x1, y1)`, região cortada ao mapa; duas passadas sobre toda a região):
```
passada 1, por tile:  c = classe(tile+0 >> 6), n = tile+2 & 0xf
    inconstruível = c == 18 ou (c == 0 e n != 4) ou c in {9, 10, 11} ou track_id (u16 em +7) != 0
                    ou tile+0 & 1 (rio) ou (há prédio vivo no tile (0x408fd0) e n < 10)
    tile+6 bit 4 (0x10) = inconstruível
    se árvores (tile+3 & 0x1f) == 0 e n in {1, 2, 4..9}: tile+2 &= 0xf0
    tile+6 bit 2 (0x04) = !(bit 4) e (tile+2 & 0xf) != 0
passada 2, por tile:  tile+6 bit 3 (0x08) = !(bit 2) ou x < 1 ou vizinho oeste bit 4 ou y < 2 (sic) ou vizinho norte bit 4
                      ou x >= L-1 ou vizinho leste bit 4 ou y >= A-1 ou vizinho sul bit 4
```
**Demolição** (`0x40adb0(prédio, 1)`, depois do que 3.6 descreve: lista do lugar, lotes sem estação, elos): `tile_struct_remove(tile, 3, id)` (`0x42e320`: desliga e libera o primeiro registro `(3, id)`; `tile+2 & 0x10` passa a valer se ainda há tipo 3) para todos os tiles da caixa `+0x26..+0x2c` (**`x` externo, `y` interno**); `0x4091f0` devolve os tiles guardados em `+0x50` (`flags2` nibble, árvores, terreno) e libera `pool_88`; para tipos 0xb/0x10/0x21 repinta as transições (`0x42a7d0(xmin - 1, ymin - 1, larg + 3, alt + 3)` sobre a caixa `+0x26..+0x2c`, com RNG, 4.5.1); `0x424c10(caixa +- 1)`; `+0x4f = 1`. A elevação **não** volta ao que era (só as prévias restauram a lista `0x564fc8`). A repintura parte das listas de sobreposição do tile **como estavam** (as transições de camada 0 que o carregador pôs ao redor dos campos, `tile+6 & 0x80`): `0x42a7d0` remove essas transições dos tiles devolvidos e chama `0x4258e0`, que limpa `tile+2 & 0x40` e `tile+6 & 0x80` onde nada sobra [V: placar FieldDemolition; o clone precisa montar a camada de chão antes de devolver os tiles]. Prédios de estação (`+8 != 0`) também chamam `station_remove` (`terrain-track.md`). O contador `0x570713` e a venda ao dono (`0x43eb60`) estão em 3.6/5.3.

#### 4.5.1 Campos, sobreposições e luz [V]

Portado em `crates/rt2-sim/src/fields.rs` e `tilemark.rs`; verificado pelo placar (`B&O`, `Hannibal`, `Silver` e `Birmingham New Game`: 0 campos em 0 e 7 dias, com 19, 175, 51 e 76 prédios de campo) e por traços do original (`ADV_TRACE`/`ADV_HOOKPY`, `docs/dynamic-analysis.md` 5.8).

**Tipo -> pintura** (o `switch` de `0x409a30`): 10, 0x1d, 0x1e usam `0x429fc0` com a classe de vegetação na global `0x53a0a4` = 12, 10, 11 (restaurada depois); 0xb, 0x10, 0x21 usam `0x42a3e0` com a classe de terreno forçada 10, 9, 11. Para cada célula `& 0x40` (`lin` externo, `col` interno), no tile `(cx - col, cy - lin)`, **sem teste de limites**:

1. `0x409120(prédio, -col, -lin)`: registro de 7 bytes em `pool_88` (`0x574c88`, `pool_alloc`): `{i8 dx, i8 dy, u8 (tile+3 << 4) | (tile+2 & 0xf), u16 (id << 2) | giros, u16 próximo}` com `próximo = +0x50` antigo; `+0x50 = novo`. Só os 4 bits baixos das árvores cabem (a restauração `0x4091f0` faz `tile+3 = (tile+3 & 0xe0) | (b >> 4)`).
2. `0x429fc0(x, y, raio 0, força 1, flags 1)` (tipos 10/0x1d/0x1e): se o tile está no mapa, a classe do terreno não é 18 (água) nem 0 (praia; salvo classe de vegetação 4) e não é estrada: `rng(0, 100)` comparado com a tabela de chances local `[0, 60, 60, 60, 70, 75, 80, 92, 60, 60, 0, 0, 0, 0]` (0 para 10..12: sempre passa); nibble `tile+2 & 0xf` := classe (num tile sem vegetação as árvores zeram); depois `rng(0, 8)` nas árvores (`tile+3 & 0x1f`) até sair de 3..6. Sempre termina com `0x424c10(x-1, y-1, x+1, y+1)`.
3. `0x42a3e0(x, y, 1, 1, classe, 0, _, forçada 1, 0, 0)` (tipos 0xb/0x10/0x21): faixa de variantes da classe forçada (`0x540aa4`/`0x540b04`, ou o grupo raro `0x540aa8`/`0x540b08` quando `(y * 0x41c64e6d - x * 0x39b19300) % 40 == 1`, aritmética de 32 bits com sinal); se o id do tile está fora, `id = rng(lo, hi)`; os giros (`tile & 0x30`) zeram; `0x417770(x-1, y-1, x-1, y-1)`. A caixa `(xmin, ymin, xmax, ymax)` desses tiles é acumulada.
4. Fim (tipos 0xb/0x10/0x21): `0x42a7d0(xmin - 1, ymin - 1, xmax - xmin + 3, ymax - ymin + 3)` (posição e **tamanho**, cortado ao mapa). Seu final: `0x42bdf0` (só sprites), `0x424c10` e `0x42bf90` (fronteiras de território) em `(x0 - 1, y0 - 1, x0 + w + 1, y0 + h + 1)` a partir do canto **cortado**, e `0x417770(x0, y0, x1, y1)` cortado.

**Listas de sobreposição** (o `u32` em `tile+10`, não salvas): `0x42a7d0` depende só da **presença** de nós (se a mistura troca o id base ou vira nó; se `0x4258e0` roda depois de limpar a camada 0). O clone guarda por tile se há nós de camada 0 (`SimState::ground_layer`: o que as passadas do carregador deixaram em `load_passes`, ou recalculado delas para um save já 0x2348) e deriva as outras camadas do tile: rio (`tile & 1`), estrada (`tile+3 & 0x20`), trilho desenhado (`Ai::track_overlay`, camadas 4/5, inclusive as peças que o trilho do vizinho desenha no tile) e fronteiras (reconstruídas por `0x42bf90` antes). **`tile+6 & 0x80` não serve de indicador**: nos `.MAP` ele pode vir ligado sem nós (conferido no original: `#02`, tiles `(71,204)`/`(71,205)` com `0x88` e lista vazia), e `0x4258e0` só o recalcula quando uma lista muda (`0x4259e0` não chama `0x4258e0` se nada foi removido, nem no ramo que sorteia variante nova).

**Trilho desenhado em `building_can_place`**: o teste `0x415f80` (nó de camada 4/5 no tile) vê também as peças desenhadas a partir do trilho vizinho; com o trilho pré-construído de `#04 Silver` a aproximação `track_id != 0` aceitava um sítio que o original recusa. O crescimento usa a máscara completa (`fields::TrackMask`, `tilemark::PlaceView`), calculada uma vez por `town_year_step`.

**Redesenho e luz** (`0x417770`/`0x417c40`): `0x417770` só acumula a união dos retângulos (sem cortar; `0x425850` corta no uso). `0x417c40` roda no desenho do mapa (`0x421550`) e em `0x469670`; no início de campanha ele foi visto **uma vez**, no fim de `0x479b20`, depois de `0x430a40(1)`; o `advance_save.py` não desenha quadros, então nenhum outro redesenho acontece nos casos do placar. Ele refaz `tile+6 & 1` (plano, `0x417880`) e a luz: para cada tile da união `0x416bf0` (luz `tile+0xe`, 0..31, `atan` das inclinações: `terrain-track.md`, `build::tiles::tile_shade_from`) e depois `0x416d40` (luz dos 4 vértices = média dos até 4 tiles em volta, `>> 2` no interior, soma/contagem na borda; **`tile+6 & 0x20` = a luz do tile é igual à dos 4 vértices**). Depois da carga a união vale `(0, 0, W - 2, H - 2)` (passada do chão do carregador); o clone a acumula (`SimState::dirty`) e roda a passada de luz no fim de `World::new_game` (`SimState::redraw_map`), calculando a luz dos tiles fora da união pela elevação atual.

**Memória do pool**: `0x409a30` não escreve `+0x38..+0x3b` do registro; num slot criado pelo crescimento do pool (`slot >=` o tamanho na carga) ficam os bytes de preenchimento do heap de depuração do Wine, `[0x0d, 0xf0, 0xad, 0xba][(slot * 0x9f + o) % 4]` (os slots livres da carga vêm zerados). Um slot reaproveitado depois de uma demolição guardaria os bytes do prédio antigo (não modelado).

### 4.6 `building_can_place` (`0x4087a0`) [V]

Assinatura `fastcall(x, y, forma, empresa, cedo?, permitido, estacao, &a, &b, estrada)`; o crescimento chama com `empresa = 0`, `param_5 = 0`, `permitido = 0`, `estacao = 1`, `estrada = (tentativas > 300)` (o argumento que 4.2 chama `cedo`). Origem da pegada: `(x + (largura >> 1), y + (altura >> 1))`. Para cada célula `c` com `c & 0x3c3 != 0`, em `lin` externo / `col` interno, no tile `(cx - col, cy - lin)`, **falha** se:
1. o tile está fora do mapa;
2. `c & 0x200`, o tile não é estrada (`tile+3 & 0x20 == 0`) e `estrada != 0`;
3. há prédio vivo no tile (cadeia tipo 3 + registro em `list_industries`) e `permitido == 0` (com `permitido != 0` o prédio tem de ser esse);
4. `tile+0 & 8` (com `param_5 == 0`) ou `tile+0 & 1` (rio);
5. estrada (`tile+3 & 0x20`) e `c & 1`;
6. terreno de classe 18 (água) sem `c & 0x102`, ou outra classe sem `c & 0x2c1`;
7. forma de estação (tipo 0): `tile+2 & 0x20`; outra forma: `0x415f80(tile)` = o tile tem ponte (`tile+2 & 0x80`), vão (`tile+3 & 0x40`) ou uma camada de trilho (4 ou 5) na lista de overlays (`tile+10`).
Depois, só para estações (`tipo 0` e `estacao != 0`), as arestas ao lado da âncora (`terrain-track.md` 3.2): não portadas aqui. O clone modela a camada de trilho como `track_id != 0` (a lista de overlays é derivada do desenho); `crates/rt2-oracle/tests/placement.rs` roda `0x4087a0` no emulador sobre um mapa aleatório (rios, bit 3, estradas, pontes, trilho com overlays, prédios nas cadeias) com todas as formas de prédios, 78 400 casos, e compara com `tilemark::can_place` (aceita 5 331). **Divergência do clone antigo**: ignorava as regras 2, 5 e 7 (estrada, ponte/vão/trilho) e o argumento `estrada`.

## 5. Indústrias

### 5.1 Lucratividade anual (`industry_year_step` `0x40b450`) [V para prédios sem dono]

Roda **uma vez por ano, para todo prédio** (inclusive casas), antes de `0x430a40`. Pula tipo 0 e prédios com `+0x4f`. Sem RNG.

```
D = Σ +0x63[k] (entregues a estação demandante) ; P = Σ +0x67[k] (produzidos)   ; i8, k = 0..3
razão = (P == 0) ? (D == 0 ? 1,0 : 1,5) : clamp(f32(D / P), 0, 2)
ativ  = f32(+0x34 + D)                               ; lotes recebidos + entregues
cap = 0
para cada slot k válido no ano (slot_valid, 5.2):
    se modo 0 (conversor):  cap += (saída == 0x7f) ? 2,5 : (saída == 0x11) ? 3,0 : 4,0
    senão (primário):       cap += taxa * 1,1 ; se há entrada 1: cap += 0,6              ; f32 a cada soma
se cap <= 0,5: cap = 0,5 ; senão se cap > 3: cap = (cap - 3) * 0,5 + 3
u    = clamp(ativ / cap, 0, 2)
novo = (u + razão) * 0,5                                                               ; 0..2
suav = (novo <= +0x93) ? +0x93 * 0,7 + novo * 0,3 : +0x93 * 0,55 + novo * 0,45        ; f32 -> +0x93
rating (+0x4b) = ftol(suav * 100 + 50)                                                  ; 50..250
se dono é empresa de computador: rating = (rating < 100) ? (rating + 100) / 2 : rating + 5
lucro = ftol( preço_da_forma * ((rating - 100) * 0,01) * 0,25 )
+0x36 = lucro / 1000
se dono: lançamento 0x439970(empresa, categoria 3, lucro)       ; "Investimento em indústrias", finance.md
```
O suavizador sobe mais depressa (0,45) do que desce (0,3): "média móvel de alguns anos" do manual. `rating = 100` (suav 1,0) é o ponto sem lucro nem prejuízo; o lucro anual de um dono é `25% do preço * (rating - 100)%`: **37,5% do preço** no máximo (rating 250), -12,5% no mínimo (rating 50). O texto "Gushing Cash" do jogo (retorno de 20% ou mais) corresponde a `rating >= 180`; o mapa índice -> rótulo (LNG 2466..2471) não foi localizado no código.

`industry_year_reset` (`0x40b7c0`) [S], depois de `sim_week_end` do mesmo fim de ano: `+0x34 = 0`, zera `+0x63` e `+0x67`, recontagem das pilhas (`0x40a610`), recálculo da estação ligada.

### 5.2 Slot válido (`slot_valid` `0x4e2330`) [V]

Ignora o slot se saída e entrada 1 são `0x7f`; se o ano está fora de `[de, até]`; se a carga de saída está desabilitada (`0x5705d5[carga]`); se a entrada 1 ou 2 está desabilitada e não é uma entrada de **bônus opcional** (limiar `> 0,01` com versão de rede `> 0x2332`, sempre verdadeiro em jogo local) e o modo sandbox (`0x570227`) está desligado.

### 5.3 Compra e venda (`0x43ea30`, `0x43eb60`, `0x40bae0`) [S]

```
preço(B, empresa) = (B.+0x97 != 0) ? B.+0x99 : ftol( effect_multiplier(0xd, empresa, 0, B.territorio) * preço_base_da_forma[B.forma] )
comprar(empresa, B):                              ; comando 0x2d, flag = 1
    exige direito de compra de prédios no território: empresa.+(0x479 + território*0x1ed) & 2   (senão mensagem LNG 2288)
    exige caixa >= preço (senão LNG 2289)
    conta 5 += preço ; caixa -= preço ; B.+0x99 = preço ; B.+0x97 = empresa
vender(empresa, B):                               ; comando 0x2d, flag = 0; só se B.+0x97 == empresa
    conta 5 -= preço ; caixa += preço                       ; devolve o preço
    categoria 3 -= preço / 2 ; caixa -= preço / 2 ; patrimônio += preço / 2     ; perde metade
    B.+0x97 = 0
```
- A venda devolve **metade do preço** (LNG 2338: "You will only receive half the purchase price if you sell"). O dono recebe, a cada fim de ano, o lucro da seção 5.1 em `categoria 3`.
- O código de compra **não verifica o modelo industrial**; a restrição ao modelo Expert (SG) vem da UI (não localizada) e a IA só compra com `0x573d78 == 2` (5.4). O pré-requisito no código é o **direito de compra de prédios** do território (bit 1 do byte de direitos); o preço do direito é do editor de territórios (LNG 2461).
- `effect_multiplier(0xd, ...)` é o efeito de gerente/evento/território de compra de indústria (cargo.md 5). Os preços base são os da tabela 2.4 (ex.: Steel Mill US$ 800.000; Cotton Farm US$ 200.000, que batem com o manual).
- A conta contábil é o deslocamento `+0x478e` = conta 5 pelo índice de `0x4398d0` (`conta k` em `+0x4766 + 8k`). (conta 5 = indústrias, conforme `finance.md`, corrigido em 2026-10-22).

### 5.4 Compra pela IA (`0x436950`, chamada por `0x436b30` a partir de agosto, no máximo a cada 168 dias de jogo) [S]

```
exige: nº de estações da empresa (+0x172) > 1 ; modelo industrial == 2 (expert) ; 0x5706b9 != 0 ; caixa (0x43a060) >= 200000
limiar = (empresa sem pessoa) ? 0,16 : (300 - T) * 0,0008     ; T = tendência "Industry Investment" do tycoon (tabela 0x551ea3, passo 0x23, índice pessoa.+0x52f): 0,16 com T = 100; 0,08 com 200
para cada prédio B (ordem da lista): tipo comprável (tabela 2.3 +1) e B.+0x4b > 107 e preço(B) < caixa e sem dono:
    retorno = B.+0x36 * 1000 / preço_base_da_forma
    se B tem estação ligada: retorno *= (dona da 1ª estação ligada == empresa) ? 1,1 : 0,7
    se retorno > limiar: limiar = retorno ; melhor = B
se melhor: building_buy_request(empresa, melhor)
```

### 5.5 Produção ao longo do tempo

Além de cargo.md 7.2/7.3 (taxa por slot, estado econômico, aceleração por ano, fertilizante), o que muda a produção de uma indústria ao longo do tempo é: (a) os **intervalos de ano dos slots** (a tabela de tipos troca de saída/entrada por ano), (b) o fator `m` do estado econômico, (c) **quem a cidade deixa viver** (3.6): a taxa de um prédio não depende de sua lucratividade ou de seus clientes, só a sobrevivência. Não há "aumento de produção por serviço" além do bônus de fertilizante.

## 6. Eventos do mundo

### 6.1 Estado econômico

`economy_state_step` (`0x46a010`, meses 2 e 8) está em `cargo.md` seção 9. Os efeitos de crescimento: multiplicador de cidade 0,90..1,10 e de região 0,96..1,04 (2.5), que entram no alvo da seção 3.3.

### 6.2 Oferta de bônus de conexão (`0x4a5320` -> `0x431660`; pagamento `0x46a370`)

Todo mês 1..10 (mês < 11) `sim_month_end` sorteia `rng(0, 10000)` e, se `< ((50 - |ano - 1860|) * (54300 / L)) / 70` com `L` a largura do mapa (`0x55e7a4`), chama `town_bonus_offer` [V]:

```
repete até 500 vezes:
    i = rng(0, n_lugares - 1)  ; L = lugar_na_ordem_da_lista(i)               ; [R11]
    se L.tipo != 0 ou L.+0x36 > 0: tenta de novo
    se existe estação com (x,y) na caixa de L e distância ao centro < 9,0: tenta de novo
    L.+0x36 = bônus[L.classe]  (100 mil .. 500 mil)  ; mensagem LNG 226 ; devolve
```
A chance é 0 em 1910 (50 anos de distância de 1860) e negativa depois: **depois de ~1910 não há mais ofertas**. Para um mapa de 400 de largura: `50 * 135 / 70 = 96` em 10000 em 1860, ~0,96% por mês.

Pagamento [S] (`0x46a370`, chamada logo após criar estação (`0x4be840`) ou remover estação (`0x4bf4e0`), após construir trilho (`0x4cc060`, `0x4caaa0`), ao criar/carregar o mundo (`0x472980`, `0x477a60`) e, no passo mensal, quando a flag `0x5794f0` está armada): reagrupa as estações em redes conexas (`+0x57`); para cada estação `S` de uma rede com mais de uma estação, cujo prédio pertence a um lugar com bônus pendente, procura em sua rede outra estação cujo prédio pertença a **outro lugar do tipo cidade**; se achar, a empresa dona de `S` (`+0x39`) recebe o bônus (`0x439970(empresa, 4, bônus)`, categoria 4 "Diversos"), a mensagem LNG 253 aparece para a empresa humana local e `+0x36 = 0`.

### 6.3 Anúncios

- **Primeiro prédio de cada tipo** (4.4): manchete LNG 94/95.
- **Fim de ano** (`sim_year_end`, depois de avançar para 1/jan; só se `0x570237 == 0`), por ano `A` e com a flag `0x570603[tipo]` do tipo citado: 1877 (tipo 36, Tool and Die): "Tool and Die Factories have stopped accepting iron" (LNG 2630); 1879 (tipo 29 ou 10): "Towns and Cities have stopped accepting raw produce and coffee" (2851); 1900 (tipo 12): "...stopped accepting unprocessed milk" (419); 1910: "...unprocessed oil and coal" (417, sem condição); 1960 (tipo 14): "Electric plants must now dispose of their hazardous waste properly" (420). Os anos coincidem com os limites de aceitação de cargas de `cargo.md` (campos `+0x14/+0x16` da tabela de cargas); o aviso é só texto.
- **Não existem** inundações, incêndios, greves, quebras de safra ou outros desastres sorteados no código base; os rótulos "Flood", "Burned" do LNG são nomes do editor.

## 7. RNG, na ordem

Todos os sorteios usam o gerador A (`rng_range`, rng.md). A ordem dentro de cada gancho:

**Mensal** (`sim_month_end`): (1) `0x46a010` nos meses 2 e 8: `rng(-5,5)`, depois `rng(0,10)` (cargo.md 9); (2) mês <= 10: `rng(0,10000)`; (3) se o sorteio acerta, `0x431660`: até 500 x `rng(0, n_lugares-1)`.

**Anual** (`sim_year_end`), em ordem: o passo das empresas/jogadores (`0x43ad60`), `0x4d86f0` (um `rng(0x50,0x78)` por locomotiva candidata; não é crescimento mas consome RNG), `0x40b450` por prédio (**sem RNG**), `0x43c760` por empresa (RNG de finanças), ..., `0x430a40(0)`:

```
para cada lugar L (ordem da lista):
    [R1]  rng(80,120) ou rng(95,105)             se a intensidade está fora da faixa
    para pass em (1, 0):
        enquanto p < 0 e agora >= L.+0x42:
            [R2]  rng(0, 1000000)
            se removeu:  para cada prédio elegível, na ordem da lista: [R5] rng(0x58, 0x70)
        [R3]  rng(0, 1000000)                    se n > 4 e agora >= L.+0x42 e não initial
              (se removeu: [R5] x elegíveis)
        enquanto cresce:
            [R4]  rng(0, 1000000)                (não initial)
            town_add_building:
                [R6] rng(0, |cands|-1)  e [R7] rng(0, limite) (se elegível) ... até achar
                town_place_building(tipo):
                    [R8]  rng(1,24)              (formas variáveis)
                    por tentativa: [R9] 4 x rng(0,raio) (sem consumo se raio 0), 2 x rng(0,1)  | cidade
                                   ou rng(x0,x1), rng(y0,y1)                                   | região
                    (rng(0,100) em terreno de classe 8: inalcançável)
                    se cria: 4087a0 sem RNG; building_construct: [R10] 4 x rng(0,100), depois os
                    sorteios de textura dos campos (4.4, não decodificados)
```
Os `rng(0, 0)` não avançam o estado. `sim_week_end` e `0x40b7c0` não sorteiam (a produção, `0x40aa20`, tampouco).

## 8. Divergências do manual (o código vale)

- **Tamanho e crescimento**: o manual diz que a cidade bem servida chega a "cerca de 3 vezes o tamanho original em 10 a 15 anos" e a mal servida encolhe "10 a 30% em uma ou duas décadas". No código o alvo é `classe x g x economia x intensidade` multiplicado por um **índice 50..300%** que depende da fatia de entregas (3.3); o teto é 3x (índice 300). O crescimento é gradual: `(N-cur)/cur / 17,5` por ano para o primeiro prédio e `0,2` para os seguintes. O "fator aleatório de ~30% do tamanho inicial" do manual é a **intensidade** `80..120%` (cidades) e `95..105%` (regiões).
- **Quem cresce**: "serviço" no manual (estação, entregas); no código só contam os **lotes entregues/despachados** (3.5), não a presença de estação.
- **Tipos novos por ano**: os anos do manual (Paper Mill 1800, Gravel Pit 1884, Electric Plant 1890, Fertilizer Plant 1905, Rubber Farm 1900, Tire Factory 1900, Nuclear/Uranium 1950, Dairy Processor ~1880) batem com o `primeiro_ano` da tabela 2.3. Mas o código também tem 1856 (Cannery, Steel Mill), 1860 (Oil Well), 1881 (Dairy Processor), 1884 (Cement Plant), 1890 (Oil Refinery), 1900 (Auto Plant), 1905 (Chemical Plant), 1910 (Aluminum Mill, Bauxite Mine) e 1950 (Landfill) — o manual não os lista.
- **Compra de indústria**: SG diz só no modelo Expert e M diz Advanced; no código a compra exige o **direito de compra do território** (não o modelo) e só a IA exige o Expert. A venda devolve metade do preço: bate com o texto do jogo.
- **Retorno**: "Gushing Cash ~ 20% ou mais do preço" (SG); o código dá `25% x (rating - 100)%`, com teto de 37,5%.
- **Desastres**: o manual do editor não promete nenhum e o código não tem.
- **Bônus de conexão**: não consta do manual consultado; no código é o único evento aleatório mensal do mundo (6.2).

## 9. Verificação (`crates/rt2-oracle/tests/growth.rs`)

| Teste | O que cobre | Casos |
|---|---|---|
| `town_growth_step_matches_original` | `0x430540` inteiro com `0x430400`, `0x42ffc0` e `0x4302c0` trocados por stubs (contagens, logs de criação/demolição): intensidade, índice, tendência, contagens finais, log de chamadas e **estado do RNG** | 6000 |
| `town_add_building_matches_original` | `0x42ffc0` com `0x42fcb0` stub (tabela de sucesso): pesos, escassez, anos, desativados, lista de prédios, log de tipos tentados, RNG | 2500 |
| `town_place_building_matches_original` | `0x42fcb0` com `place_at`, `site_classes`, `building_can_place` e `building_create` stubs: posições, raios, tentativas, forma, classe de terreno real, logs e RNG | 3000 |
| `pick_worst_building_matches_original` | `0x4302c0` com `building_destroy` stub: prédio escolhido, RNG | 5000 |
| `shape_pick_matches_original` | `0x40ba00` | ~4000 |
| `town_building_stats_matches_original` | `0x430400`: contagens, `+0x12`, `+0x3a`, `+0x48` (f32 bit a bit) | 4000 |
| `site_classes_match_original` | `0x42fb10` sobre tiles/elevações aleatórios | 3000 |
| `industry_rating_matches_original` | `0x40b450` (prédio sem dono; os dois conjuntos; `slot_valid` real): `+0x93`, `+0x4b`, `+0x36` | 8000 |
| `initial_slot_clocks_match_original` | `0x40bb40`: 4 relógios e RNG | 4000 |
| `town_bounty_offer_matches_original` | `0x431660` com as rotinas de mensagem neutralizadas: lugar escolhido, bônus, RNG | 3000 |
| `can_place_matches_original` (`crates/rt2-oracle/tests/placement.rs`) | `0x4087a0` inteiro (formas de prédios) sobre um mapa aleatório com rios, bit 3, estradas, pontes, trilho com overlays e prédios nas cadeias; as duas versões de `estrada` | 78 400 |
| `example_thirty_days_residual_is_the_documented_gap` (`crates/rt2-sim/tests/worldloop.rs`) | `building_construct`/`building_destroy` inteiros no Example (30 dias): `tiles`, `elevation`, `structures`, `stations`, `pool_58` iguais ao original | 2 criações, 2 demolições |

Os stubs são código de máquina escrito sobre o corpo do callee na imagem emulada (`Asm` no teste); o lado Rust roda o mesmo cenário com ambientes equivalentes (`TownEnv`, `SiteEnv`). Não testados no emulador: o pagamento do bônus (`0x46a370`), a compra/venda (`0x43eb60`, depende de listas de empresa) e o laço anual `0x430a40` (chama a UI); `building_construct` e a demolição são provadas só pelo Example (4.5).

## 10. Fidelidade e implicações para o clone

- Implementar `town_growth_step` com a ordem exata (pass 1 depois 0), os `f32` em cada armazenamento e `ftol` truncando. O RNG é parte do estado: cada `rng_range(a, b)` com `a == b` **não** avança.
- A lista de prédios do lugar é uma pilha (prédio novo na cabeça): a ordem importa para os sorteios de `rng(0x58,0x70)` da demolição, que ocorrem **por prédio elegível, na ordem da lista**.
- Preservar o degrau do índice no 12º ano, a ausência de trava nas primeiras iterações do crescimento, e o período protegido de 4 anos contra demolição.
- Passos do tick: `town_year_step` roda **depois** de `industry_year_step` de todos os prédios e **antes** de `industry_year_reset`; a produção semanal do mesmo fim de ano roda entre os dois.
- O `rng(0,100)` dos relógios e os sorteios de textura de campos consomem RNG na criação de cada prédio, na ordem de 4.5.1.

> **Clone (`rt2-sim::growth`)**: `town_growth_step`, `town_add_building`, `town_place_building`, `pick_worst_building`, `shape_pick`, `site_classes`, `town_building_stats`, `industry_rating`, `slot_valid`, `initial_slot_clocks`, `town_bounty_offer`, `bounty_draw_threshold`, `growth_factor`, `type_scarcity_percent`, `decay_counter`. O mundo concreto (listas de prédios e tiles) ainda não existe no clone: as regras recebem ambientes (`TownEnv`, `SiteEnv`) para serem ligadas a ele.

## Em aberto

- Rótulo de lucratividade da UI (LNG 2466..2471 "Poor" .. "Gushing Cash"; 2475..2482 "Atrocious" .. "Good"): a função que mapeia `+0x4b`/`+0x36` aos textos não foi localizada.
- A parte de estações de `building_can_place` (arestas de trilho ao lado da âncora, `0x4629f0`, `0x4603a0`) é de `terrain-track.md`.
- Repintura na demolição (`0x4091f0` + `0x42a7d0`) [V: placar FieldDemolition]; o reaproveitamento de slot do pool de prédios guardaria bytes do registro antigo em `+0x38` (4.5.1).
- Outras mudanças de elevação fora da construção de prédios (trilho, terraplenagem: `0x426000` com `param_3 != 0`) ainda não acumulam o retângulo de redesenho no clone; só importa para a luz (`tile+6 & 0x20`) num redesenho.
- `+0x4b` inicial de um prédio novo (a alocação `0x4f1690` zera o registro?) e `+0x4a` (pesos por tipo) no formato do `.MAP`: o decodificador de `map-gam.md` ainda não os expõe (o registro de arquivo tem 0x96 bytes, a memória 0xb4).
- Significado de `0x573d7a` (modo editor), `0x570237` (desliga crescimento, economia e finanças) e `0x5748f8` (trava).
- Quem define `0x570223` e `0x57022b` ao carregar um mapa (bloco de estado do `.MAP`/`.GAM`): o editor (`0x47fea0`) mostra "Annual Growth Rate" e o nível de economia após 30 anos.
- `0x46a370`: condição `4c01b0` (rede/empresa) e o caso da mensagem; `0x4c0190` (tamanho da rede). Pagamento sem teste.
- Quem decide a visibilidade do botão "Buy this building" na UI (modelo Expert?).
- `finance.md`: trocar os rótulos das contas 4 (trens) e 5 (indústrias).
- Campo `+0x06` (i32) da tabela `0x535c38` e `+0` (id de gráfico).

## 11. Construção pelo jogador e pelo editor (complemento de 4.4 e 5.3)
- `building_create` (`0x409010` -> `0x409a30`) para estações está portado e verificado em `crate::build::station` (terrain-track.md 10.3); as extensões da pegada (`forma +12..+15`) são de tempo de execução (`0x471b10`) e agora vêm de `tilemark::shape_extents` (vale também para o crescimento das cidades).
- Construir prédio ou indústria (`0x17` com forma 24..121) é **só do editor** (ferramenta 7, docs/spec/commands.md 3.1); a primitiva existe (mesma função; as texturas de campo de 4.5.1 estão na criação pelo crescimento) com `todo_hook` para os elos `0x4072c0` e a produção inicial `0x40a610`.
- Compra (`0x43eb60` flag 1) e venda (flag 0: só o dono, devolve `+0x99` e perde a metade na categoria 3) verificadas por snapshot (`buyind5`, `sellind5`); as condições da janela (modelo Expert, permissão `0x5706b9`, tipo comprável, direito de trilho, caixa) estão em `Ai::check`.
