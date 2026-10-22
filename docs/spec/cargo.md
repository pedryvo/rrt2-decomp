# Carga: produção, demanda, preço e entrega

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido com Ghidra via REA e com `objdump`. O manual (`docs/reference/manual-rules.md`, seções 5 a 7) só serviu de pista; onde o código diverge, a divergência está na seção 11. Endereços são VAs (base `0x400000`). Estado das provas: **[V]** = confirmado contra o original por teste diferencial (`crates/rt2-oracle/tests/cargo.rs`, seção 12); **[S]** = lido no código, sem teste. Todas as tabelas numéricas abaixo vêm do `.data` do executável (nenhuma do manual).

## 1. Visão geral

A carga é feita de **lotes** (`lot`, uma "carga" = o que cabe em um vagão; cada vagão carrega no máximo um lote, `car+0x2d`). Não há quantidade em toneladas.

```
indústria/casa --(produção periódica)--> pilha de lotes da própria indústria (lot.estação = 0)
estação          --(carregamento)-------> vagão   (o lote mais antigo do raio da estação)
trem chega       --(descarga)-----------> receita por lote (seção 4) + taxas de estação e de trilho (seção 6)
                                          lote entregue a indústria que o demanda -> estoque de entrada -> conversão em saída (seção 7.4)
```

Pontos centrais:
- **A carga espera na pilha da indústria**, não na estação (`industry+0x4d` = cabeça da lista, `lot+0xf` = próximo). A estação só contabiliza (`+0x61`) o que há nas pilhas das construções ligadas a ela (o raio de captação, lista de elos `0x574c58`).
- **A receita é calculada na descarga** por `cargo_delivery_revenue` (`0x4bf760`) **[V]**: preço-base da tabela de cargas x fator da demanda no destino x penalidade de tempo x fator de distância (aplicado duas vezes) x efeitos de gerente/evento x estado econômico x ajuste do jogador/computador, com redução progressiva acima de $80.000 por lote.
- O estado econômico (`0x570717`, 0 a 4) muda a cada seis meses (`0x46a010`, meses 2 e 8) e escala produção e receita (seção 9).

Mapa de funções:

| Endereço | Nome | Papel | Prova |
|---|---|---|---|
| `0x4bf760` | `cargo_delivery_revenue` | receita de um lote | [V] (ramo `commit=1`) |
| `0x46b480` | `effect_multiplier` | multiplicador de gerente/evento/território | [V] |
| `0x4d2c10` | `train_arrival_settlement` | descarga, taxas, partilha de trilho, lançamentos | [S] |
| `0x4d2360` | `train_estimate_revenue` | estimativa de receita da próxima parada | [S] |
| `0x414510` | `car_deliver_lot` | liga vagão -> receita | [S] |
| `0x471880` | `industry_rebuild_inputs` | lista de entradas e pesos de demanda do tipo | [V] |
| `0x40a8c0` | `industry_production_rate` | lotes por ano de um slot | [V] |
| `0x40aa20` | `industry_production_tick` | produção semanal | [S] |
| `0x40a770` | `industry_spawn_lot` | cria um lote | [S] |
| `0x40b250` | `industry_receive_cargo` | entrada de carga e conversão | [S] |
| `0x4bebf0` | `station_recompute_supply_demand` | oferta, demanda e níveis de preço da estação | [S] |
| `0x4bf260` | `station_load_lot` | escolhe o lote a carregar | [S] |
| `0x46a010` | `economy_state_step` | passo semestral do estado econômico | [S] |
| `0x4a5320` | `sim_month_end` | decaimento mensal dos contadores, expiração de lotes | [S] |

## 2. Estruturas (campos usados pela carga)

Todas as listas são objetos acessados por `list_record` (`0x4f18b0`): `registro(id) = lista[+0x28] + id * lista[+8]`, e `id == 0` devolve ponteiro nulo **[V]**. Globais das listas:

| Global | Lista | Tamanho do registro |
|---|---|---|
| `0x574c7c` | lotes | 0x15+ |
| `0x574c70` | vagões | |
| `0x574c64` | trens | 0x14d |
| `0x574c60` | estações | 0x4c7 |
| `0x574c6c` | empresas | 0x4e86 |
| `0x574c68` | pessoas (gerentes/jogadores) | 0x576 |
| `0x574c74` | indústrias e casas | 0x9f |
| `0x574c58` | elos estação-construção (raio de captação) | |
| `0x574c5c` | cidades | 180 |
| `0x574ca8` | territórios | |

**Lote** (`0x574c7c`): `+0` carga (i8), `+1` indústria de origem (u16), `+3` estação de origem (u16, definida no primeiro carregamento), `+5` vagão (u16), `+7` instante de criação (minutos), `+0xb` instante do último carregamento num vagão, `+0xf` próximo lote da pilha da indústria, `+0x11` estação onde foi largado (transferência), `+0x13` próximo da lista da estação.

**Estação**: `+0` id, `+0xa/+0xe` x e y (i32), `+0x39` empresa dona, `+0x3d` cabeça da lista de elos e `+0x41` quantidade deles, `+0x49` bitmask de melhorias (bit `i` = construção `STATION_BUILDINGS[i]` do LNG, id `1043 + i`: 5 restaurante pequeno, 6 restaurante grande, 7 saloon, 8 correio, 15 hotel grande, 16 hotel pequeno, 17 armazenamento líquido, 18 refrigerado, 19 silo, 20 armazém), `+0x61` u16[44] lotes esperando por carga, `+0x169` byte[44] (1 produzida por indústria, 2 por casa/estação ligada), `+0x195` byte[44] a estação demanda a carga, `+0x1c1` f32[44] peso de demanda, `+0x271` f32[44] contador de entregas (decai), `+0x321` byte[44] **nível de preço 0..9**, `+0x4ad` território, `+0x4be` lista de lotes largados.

**Indústria/casa**: `+0` id (u16), `+7` tipo, `+0x33` estações ligadas (i8), `+0x4d` pilha de lotes, `+0x53` u32[4] relógios dos slots de saída, `+0x6b` f32[8] estoque de cada entrada, `+0x8b` i16[4] lotes na pilha por saída, `+0x9d` território, `+0x9e` flags.

**Empresa**: `+0` id, `+0x23` pessoa, `+0x1ae` i16[efeito], `+0x4248` f32 multiplicador de receita (1.0 ao criar), `+0x4276` f32 "fator de computador" (1.0 para empresa sem humano, 0.0 com humano; `0x438a10`, `0x438900`, `0x4399d0` o suavizam por mês com 0,92). **Pessoa**: `+0x122` flag humano (`0x473590`), `+0x26b` i16[efeito]. **Território**: `+0x58` i16[efeito]. **Trem**: `+0x41` empresa, `+0xbb` flags (bit 0 vagão-restaurante, bit 2 recorde de velocidade de passageiros).

## 3. Tabela de cargas (`0x5404b1`, 46 registros de 0x21 bytes) **[V]**

A tabela vive no `.data` do executável (não em PK2). `cargo_table` **[V]** (lida da memória do original e usada pelo teste de receita).

| Offset | Tipo | Campo |
|---|---|---|
| `+0x00` | i32 | preço-base. **Na partida (`0x471bf5..0x471c0d`) vira `ftol(preço * 1.4)`** |
| `+0x04` | i32 | "dias de entrega" (prazo-base de tempo, dias) |
| `+0x08` | i32 | 0 no arquivo; na partida vira `dias * 1440` (`0x471eb0..0x471ed8`, em minutos, usado na vida útil dos lotes) |
| `+0x0c` | f32 | "ship": sensibilidade à distância `s` (o "Rot Factor" do manual) |
| `+0x10` | u16 | primeiro ano disponível (filtro de vagões da UI, `0x4e3eb0`) |
| `+0x14` | i16 | último ano em que **vilas** aceitam a carga (0 = nunca, 9999 = sempre) |
| `+0x16` | i16 | idem para **cidades** |
| `+0x18` | f32 | demanda por casa (acrescenta ao peso de demanda da estação) |
| `+0x1c` | i32 | bit da melhoria de estação que **metade** o tempo de espera (0 = nenhum) |
| `+0x20` | u8 | 1 nas cargas 35 a 41 (provável marca de carga do conjunto de expansão) |

Os nomes vêm da tabela `CARGO_TYPES` do LNG (id `812 + i`, `i` = 0..32). As linhas 33 a 45 não têm nome em `default2.lng`; as linhas 44 e 45 (preço 0, anos 1846 e 1839) coincidem com as linhas "Dining" e "Caboose" do guia, e a linha 32 (LNG "dining") tem preço e prazo iguais aos de passageiros.

Valores no arquivo e em tempo de execução (ordem das linhas = id da carga; coluna "escala de espera" = `+0x08`):

| id | LNG id | name | raw price | runtime price (x1.4) | days | wait scale | ship s | first | towns<= | cities<= | per house | storage bit |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 812 | passenger | 56000 | 78400 | 500 | 720000 | 0.45 | 1800 | 9999 | 9999 | 0.35 | - |
| 1 | 813 | mail | 65000 | 91000 | 300 | 432000 | 0.54 | 1800 | 9999 | 9999 | 0.18 | 8 |
| 2 | 814 | lumber | 35000 | 49000 | 1300 | 1872000 | 0.25 | 1800 | 9999 | 9999 | 0.12 | - |
| 3 | 815 | log | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1800 | 0 | 0 | 0.00 | - |
| 4 | 816 | milk | 51000 | 71400 | 300 | 432000 | 0.32 | 1800 | 1899 | 1899 | 0.00 | 18 |
| 5 | 817 | cattle | 37000 | 51800 | 800 | 1152000 | 0.32 | 1800 | 0 | 0 | 0.00 | - |
| 6 | 818 | steel | 35000 | 49000 | 1300 | 1872000 | 0.25 | 1856 | 0 | 0 | 0.00 | - |
| 7 | 819 | goods | 42000 | 58799 | 800 | 1152000 | 0.33 | 1800 | 9999 | 9999 | 0.20 | 20 |
| 8 | 820 | auto | 49000 | 68600 | 800 | 1152000 | 0.32 | 1900 | 0 | 9999 | 0.20 | - |
| 9 | 821 | cement | 38000 | 53200 | 800 | 1152000 | 0.25 | 1884 | 0 | 9999 | 0.12 | - |
| 10 | 822 | food | 49000 | 68600 | 600 | 864000 | 0.36 | 1800 | 9999 | 9999 | 0.20 | - |
| 11 | 823 | cotton | 36000 | 50400 | 1100 | 1584000 | 0.29 | 1800 | 0 | 0 | 0.00 | 20 |
| 12 | 824 | wool | 36000 | 50400 | 1100 | 1584000 | 0.29 | 1800 | 0 | 0 | 0.00 | 20 |
| 13 | 825 | paper | 46000 | 64399 | 800 | 1152000 | 0.32 | 1800 | 0 | 9999 | 0.10 | - |
| 14 | 826 | oil | 37000 | 51800 | 1100 | 1584000 | 0.25 | 1860 | 0 | 1909 | 0.12 | 17 |
| 15 | 827 | chemical | 37000 | 51800 | 800 | 1152000 | 0.32 | 1882 | 0 | 0 | 0.00 | 17 |
| 16 | 828 | fertilizer | 35000 | 49000 | 1300 | 1872000 | 0.25 | 1905 | 0 | 0 | 0.00 | - |
| 17 | 829 | waste | 48000 | 67200 | 800 | 1152000 | 0.34 | 1950 | 0 | 0 | 0.00 | - |
| 18 | 830 | coal | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1800 | 0 | 1909 | 0.15 | - |
| 19 | 831 | grain | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1800 | 0 | 0 | 0.00 | 19 |
| 20 | 832 | aluminum | 35000 | 49000 | 1300 | 1872000 | 0.25 | 1910 | 0 | 0 | 0.00 | - |
| 21 | 833 | coffee | 37000 | 51800 | 800 | 1152000 | 0.32 | 1800 | 1878 | 1878 | 0.12 | 19 |
| 22 | 834 | tire | 35000 | 49000 | 1300 | 1872000 | 0.25 | 1900 | 0 | 0 | 0.00 | - |
| 23 | 835 | sugar | 37000 | 51800 | 800 | 1152000 | 0.32 | 1800 | 0 | 0 | 0.00 | 19 |
| 24 | 836 | uranium | 43000 | 60199 | 800 | 1152000 | 0.36 | 1950 | 0 | 0 | 0.00 | - |
| 25 | 837 | pulpwood | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1800 | 0 | 0 | 0.00 | - |
| 26 | 838 | gravel | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1884 | 0 | 0 | 0.00 | - |
| 27 | 839 | iron | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1800 | 0 | 0 | 0.00 | - |
| 28 | 840 | produce | 42000 | 58799 | 600 | 864000 | 0.36 | 1800 | 1878 | 1878 | 0.12 | 18 |
| 29 | 841 | bauxite | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1910 | 0 | 0 | 0.00 | - |
| 30 | 842 | diesel | 39000 | 54600 | 1100 | 1584000 | 0.29 | 1890 | 0 | 0 | 0.00 | - |
| 31 | 843 | rubber | 28000 | 39200 | 1600 | 2304000 | 0.21 | 1900 | 0 | 0 | 0.00 | - |
| 32 | 844 | dining | 56000 | 78400 | 500 | 720000 | 0.45 | 1900 | 0 | 0 | 0.00 | - |
| 33 | - | (unnamed) | 38000 | 53200 | 1200 | 1728000 | 0.27 | 1900 | 0 | 0 | 0.00 | - |
| 34 | - | (unnamed) | 38000 | 53200 | 1200 | 1728000 | 0.27 | 1900 | 0 | 0 | 0.00 | - |
| 35 | - | (unnamed) | 40000 | 56000 | 1100 | 1584000 | 0.31 | 1800 | 9999 | 9999 | 0.11 | - |
| 36 | - | (unnamed) | 50000 | 70000 | 300 | 432000 | 0.38 | 1900 | 0 | 0 | 0.00 | - |
| 37 | - | (unnamed) | 50000 | 70000 | 300 | 432000 | 0.38 | 1900 | 0 | 0 | 0.00 | - |
| 38 | - | (unnamed) | 50000 | 70000 | 400 | 576000 | 0.38 | 1900 | 0 | 0 | 0.00 | - |
| 39 | - | (unnamed) | 50000 | 70000 | 400 | 576000 | 0.45 | 1900 | 0 | 0 | 0.00 | - |
| 40 | - | (unnamed) | 50000 | 70000 | 400 | 576000 | 0.45 | 1900 | 0 | 0 | 0.00 | - |
| 41 | - | (unnamed) | 50000 | 70000 | 300 | 432000 | 0.38 | 1900 | 0 | 0 | 0.00 | - |
| 42 | - | (unnamed) | 50000 | 70000 | 300 | 432000 | 0.38 | 1900 | 0 | 0 | 0.00 | - |
| 43 | - | (unnamed) | 1000 | 1400 | 1600 | 2304000 | 0.21 | 1900 | 0 | 0 | 0.00 | - |
| 44 | - | (unnamed) | 0 | 0 | 0 | 0 | 0.00 | 1846 | 0 | 0 | 0.00 | - |
| 45 | - | (unnamed) | 0 | 0 | 0 | 0 | 0.00 | 1839 | 0 | 0 | 0.00 | - |

Observações:
- O preço de partida usa `ftol(raw * 1.4)` em x87 de 53 bits: bens, produce, papel e urânio caem em `58799`, `58799`, `64399`, `60199` (e não 58800 etc.), pois `raw * 1.4` fica um ulp abaixo do inteiro. Com precisão de 24 bits o resultado pode diferir; o estado real da FPU no original continua em aberto (seção 13).
- O `raw` de passageiros (56000) e de correio (65000) difere do manual (60.000 e 70.000), assim como leite, comida e perigosos; ver seção 11.

## 4. Receita de um lote entregue (`0x4bf760`) **[V]**

`cargo_delivery_revenue`: `thiscall`, `ECX` = estação de destino `D`; argumentos `(lote, &receita, &flag, &indústria, commit)`, `ret 0x14`. `commit == 0` aplica os efeitos colaterais (seção 4.8); `commit == 1` só calcula (é o que a estimativa usa). Todas as contas são x87; os `ftol` truncam em direção a zero (CRT `0x51b524`). O teste diferencial reproduz o resultado bit a bit com precisão de 53 bits (`CW = 0x27f`).

Notação: `S` = estação de origem do lote (`lot+3`), `D` = destino, `C` = empresa do trem, `c` = carga, `year` = ano do jogo (u16), `human = company_is_human(C)` (pessoa da empresa com flag `+0x122`; `0x43c030` **[V]**), `model` = modelo industrial (`0x573d78`, 0 básico, 1 avançado, 2 expert), `state` = estado econômico.

### 4.1 Distância e fator de distância `X`

```
d   = ftol( sqrt((Dx-Sx)^2 + (Dy-Sy)^2) )                     ; 0x4f0ca0 [V], coordenadas das estações (+0xa/+0xe)
yrs = min(year, 2000) - 1800
r   = (d * 1.4) / (yrs * 0.5 + 15.0)                           ; os trens ficam mais rápidos: a distância "de referência" cresce
s   = cargo[c].ship
X   = (f32)( r * s + (1.0 - s) )                               ; 0x4bf860..0x4bf8a3
```

### 4.2 Preço-base conforme a demanda (R0)

```
se lot.origem == D.id e human:  R = 0                         ; entregar no próprio ponto de origem não paga (0x4bf8a7)
senão se D.demanda[c] != 0 e D.nº_elos > 0:                    ; flag +0x195+c
    off = rng_hash_range(lot_id, 0, n-1)                       ; 0x4f06f0, ponto de partida do giro
    para k em 0..n-1:  I = indústria do elo ((off+k) mod n)
        se I.tipo == 0x12: casas += 1;  se I.tipo == 0x27: casas += 2
        se I.id != lot.indústria_origem ou lot.indústria_origem == 0:
            para cada entrada j (0..7) de industry_type_table[conjunto][I.tipo].inputs: se inputs[j] == c e I.estoque[j] < melhor:
                melhor = I.estoque[j] (início 9999.0);  alvo = I.id           ; consumidor com MENOS estoque
    se há alvo:               R = ftol( P * preço[c] );  indústria_out = alvo;  (commit=0: industry_receive_cargo(alvo, c))
    senão se (casas >= 8 e ano <= cargo.cidades) ou (casas >= 4 e ano <= cargo.vilas):
                              R = ftol( P * preço[c] )                        ; demanda de cidade/vila; indústria_out = 0
    onde P = (model >= 2) ? tabela_nível[D.nível[c]] : 1.0                    ; 0x550524: 0.5 0.6 ... 1.4 (nível 0..9)
senão (carga não demandada):
    R = ftol( preço[c] * tabela_sem_demanda[human*3 + model] )               ; 0x55050c
    se model >= 2 e d < 16: R = (R * d) >> 4
se R != 0 e d < 20: R = ((d + 1) * R) / 21                                   ; divisão com sinal
```

`tabela_sem_demanda` (f32): **computador** 0.8 / 0.55 / 0.4 e **humano** 0.8 / 0.5 / 0.2 para os modelos básico / avançado / expert. Trechos com `x1.07`/`x0.93` por versão de rede (`0x4bfae5..0x4bfb2e`) são código morto: o valor é sobrescrito logo depois.

### 4.3 Penalidade de tempo `t`

```
p1 = (lot.carregado - lot.criado) / 1440 (u32);  se cargo[c].armazenamento != 0 e S.melhorias tem esse bit: p1 = p1 >> 1 (aritmético)
p2 = (agora - lot.carregado) / 1440 (u32)                       ; dias no vagão
E  = max(0, (i32)(p1 + p2))                                      ; dias totais: espera + viagem
A  = ftol( cargo[c].dias * X )                                   ; prazo-base escalado pela distância
yv = year>2000 ? 1960 : year<1850 ? 3*year-3700 : year<=1920 ? year : (year-1920)/2+1920   ; 0x4bfc76..0x4bfcb1
A2 = max(1, ftol( ((1960 - yv) * 0.005 + 1.0) * A ))             ; prazo maior nos anos antigos
E2 = human ? E : ftol( E - C.fator_computador * E * 0.25 )       ; 0x4bfcff..0x4bfd1c
q  = max(0, (A2 - E2) / A2) em f64
t  = q * 0.7 + 0.3                                               ; nunca abaixo de 0,3: o atraso não zera a receita
R  = ftol( ftol(R * t * X) * X )                                 ; X entra duas vezes no dinheiro
```

### 4.4 Multiplicadores

```
R = ftol( effect_multiplier(0x3d, C.id, 0, D.território) * R )               ; todas as cargas
R = ftol( effect_multiplier(c < 32 ? 0x3e + c : 0xf1 + c, C.id, 0, D.território) * R )   ; esta carga
se c == 0:  se trem.flags & 1 (vagão-restaurante): R = ftol(R * 1.2)
            se trem.flags & 4 (recorde de velocidade): R = ftol(R * 1.15)
se human:   R = ftol(R * tabela_humano[slider_humano])                       ; 0x550448: 0.75 0.80 .. 1.25 (padrão 5 = 1.0)
senão:      m = C.fator_computador
            R = ftol( (tabela_humano[lo] * (1-m) + tabela_computador[hi] * m) * R )   ; 0x55041c: 1.55 1.60 1.65 1.70 1.75 1.80 2.0 2.2 2.4 2.6 2.8
R = ftol( R * tabela_estado_receita[state] )                                 ; 0x5504bc: 0.8 0.9 1.0 1.1 1.2
R = ftol( R * C.multiplicador_receita )                                      ; empresa +0x4248
```

Os sliders (`0x574874`, bytes `lo` humano e `hi` computador, 0..10, padrão 5 e 5) são as opções "Revenue Modifier" do jogo (LNG `2603`, `2604`).

### 4.5 Redução progressiva

```
se R > 80000:  R -= (R - 80000) >> 2
  se R > 120000: R -= (R - 120000) >> 2
    se R > 180000: R -= (R - 180000) >> 2
      se R > 250000: R -= (R - 250000) >> 2
```
Cada degrau só é avaliado se o anterior disparou. Saídas: `*receita = R`, `*flag = 1` (o ramo `t <= 0`, que daria 0, nunca ocorre porque `t >= 0,3`), `*indústria = alvo` (0 se não houve).

### 4.6 Ordem das etapas

Preço da demanda (4.2) -> escala de distância curta `(d+1)/21` -> tempo e `X` duas vezes (4.3) -> efeitos `0x3d`, `0x3e+c` -> bônus de trem -> ajuste humano/computador -> estado econômico -> multiplicador da empresa -> redução progressiva (4.5). Cada seta é um `ftol`; a ordem e os truncamentos intermediários importam: o teste só reproduz o original bit a bit com todos eles.

### 4.7 Sensibilidade à precisão do x87

Com precisão de 53 bits (`CW = 0x27f`, o padrão do CRT MSVC) 8000 cenários aleatórios batem bit a bit. Com 24 bits (`CW = 0x7f`) cerca de **10%** dos cenários diferem em alguns dólares (teste `revenue_depends_on_x87_precision`). O clone precisa fixar a precisão (ver seção 13).

### 4.8 Efeitos colaterais do ramo `commit == 0` **[S]**

Só quando a receita é de fato paga (`param_5 == 0`):
- `S.+0x4b7 += 1` (lotes que saíram), `D.+0x4b9 += 1` (entregues); contadores da cidade da estação (`+0x53` -> cidade `+0x40`/`+0x3e`); `0x57070f += 1`.
- Ramo "indústria alvo": `industry_receive_cargo(alvo, c)` (+1 no estoque da entrada, conversão, seção 7.4).
- Se o lote voltou à própria estação de origem (`lot.origem == D.id`): se `agora - lot.criado < cargo.+8 / 2` e a indústria de origem existe, o lote **volta à pilha** (`industry.+0x8b[slot] += 1`, campos do lote zerados, `0x468af0`), senão é liberado (`0x466a60`) (`lot_return_or_free`, `0x4bf450`).
- Passageiros (`c == 0`) e a melhoria de estação do **destino** `D`. Com a versão de rede mínima >= 9009 (sempre em jogo local: `net_min_peer_version` devolve 9012, `0x49d920`): `B = 0; se D.melh & 0x40: B = ftol(R*0.15) senão se & 0x20: B = ftol(R*0.10); se & 0x8000: B = ftol(R*0.25 + B) senão se & 0x10000: B = ftol(R*0.15 + B); se & 0x80: B = ftol(R*0.05 + B)` (restaurante grande/pequeno, hotel grande/pequeno, saloon). Com versão < 9009 os valores são 0,30 / 0,20 / 0,40 / 0,20 / 0,10 (os do manual e do LNG).
- `Srev = ftol( effect_multiplier(0x3c, D.dono, 0, D.território) + B )` (note: o multiplicador é **somado** a `B`, não multiplicado: efeito de `fiadd`, quase certamente um bug do original; replicar). Se `Srev != 0` e `D.dono != 0`: `company_ledger_credit(D.dono, 2, Srev)` (categoria "Station"). `D.+0x5d += R + Srev`; `D.+0x271[c] += 1.0` (contador de entregas, seção 8); `S.+0x59 += R`.

## 5. Efeitos de gerente, evento e território (`0x46b480`) **[V]**

`effect_multiplier(e, empresa, pessoa, território)`: `fastcall`, `ECX` = id do efeito, `EDX` = empresa, pilha `[pessoa, território]`, `ret 8`, resultado em ST0 (f32 em precisão estendida).

```
f(n)  = max(n, -99) * 0.01 + 1.0                                         ; n é i16 em porcentagem
v     = (f32) f(efeito_global[e])                                         ; 0x5707c6 + 2e
se empresa != 0:  v = (f32)( f(empresa.i16[0x1ae + 2e]) * v );  pessoa = (pessoa == 0) ? empresa.+0x23 : pessoa
se pessoa  != 0:  v = (f32)( f(pessoa.i16[0x26b + 2e]) * v )
se território != 0: v = (f32)( f(território.i16[0x58 + 2e]) * v )
devolve  (v < 0.01) ? 0.01f : v
```
Ids de efeito usados pela carga: `0x3c` receita de estação, `0x3d` receita de todas as cargas, `0x3e + c` receita da carga `c` (`c < 32`; `0xf1 + c` para `c >= 32`), `0x5e` produção de todas, `0x5f + c` produção da carga `c`. Os bônus de gerente do manual ("Steel Revenue +10%") correspondem a `pessoa.i16[...]` e os eventos a `efeito_global`/território; quem grava nesses vetores não foi analisado (o efeito instantâneo "Economic Status" chama `0x469fb0`, que grava `economy_state`).

## 6. Descarga do trem e partilha de receita (`0x4d2c10`) **[S]**

`train_arrival_settlement(trem)`: depois de descarregar, para cada vagão com lote cujo destino é a parada atual (`0x4d2720`):
1. `car_deliver_lot` -> `cargo_delivery_revenue(commit=0)` devolve `R` e o tipo de vagão (byte `0x53dde8 + tipo*0x39` = carga que o vagão transporta: 0 passageiros, 1 correio, outra = carga).
2. Acumula `R` em três totais: passageiros (carga 0), correio (carga 1) e demais. Cada vagão com lote soma **$2000 de taxa de estação na parada anterior (`trem+0x9e`) e $2000 na parada atual**.
3. Créditos de taxa: para cada uma das duas estações com dono (`+0x39 != 0`): `company_ledger_credit(dono, 2 "Station", taxa)` (mesmo se o dono for a empresa do trem). A taxa só é **descontada** da receita da empresa do trem se o dono for outra empresa.
4. `líquido = max(0, bruto - taxas descontadas)`.
5. Partilha de trilho: o trem acumula quilometragem por dono de trilho (`trem+0x7d` total, `+0x7f` parte própria, `+0x81` quantidade, `+0x82` u16 empresas[4], `+0x8a` i16 partes[4]). Cada empresa listada e ativa (`+0x16e != 0`) recebe `ledger(empresa, 4, líquido * (f32)(parte / total))`.
6. A parte da própria empresa é `líquido_próprio = (f32)(líquido * (f32)(própria / total))`; os lançamentos são `ftol(parcela / bruto * líquido_próprio)` para passageiros (`train_ledger_add(0x65)` -> categoria 0 "Passengers"), correio (`0x66`) e carga (`0x67`) (ambos categoria 1 "Freight"); a ligação de cada total a `0x65/0x66/0x67` foi inferida da ordem de chamada.
7. Estatísticas (`0x43bde0`) e atualização de `trem+0xaf`, `+0xb3`.

`train_estimate_revenue` (`0x4d2360`) repete o cálculo com `commit = 1` para cada vagão e guarda o total em `trem+0x2b`; só recalcula se passaram mais de `0x7080` minutos (20 dias) do cálculo anterior.

## 7. Oferta

### 7.1 Tipos de indústria (`industry_type_table`, `0x541ae0`) **[V]**

`[2 conjuntos][53 tipos][0x90 bytes]`; o conjunto vem do byte 0 de `0x5704ac` (0 = RT2 padrão, 1 = conjunto de expansão com cargas 33..45). Cada tipo tem **4 slots de saída de 0x20 bytes**:

| Offset no slot | Campo |
|---|---|
| `+0x00`, `+0x02` | ano inicial e final (i16) em que o slot vale |
| `+0x04` | modo: **1 = primário** (produz por taxa, ex.: mina), **0 = conversor** (produz a partir de insumos) |
| `+0x05` | carga de saída (`0x7f` = slot vazio) |
| `+0x06` | f32 taxa: lotes por ano (primário) ou lotes por lote de insumo (conversor) |
| `+0x0a`, `+0x0b`, `+0x0c`, `+0x10`, `+0x11` | entrada 1: carga, unidades exigidas (conversor), limiar do bônus f32 (primário), índice no estoque (calculado), peso de demanda f32 |
| `+0x15`, `+0x16`, `+0x17`, `+0x1b`, `+0x1c` | entrada 2 (mesma ordem) |

Depois dos 4 slots: `+0x80` `inputs[8]` e `+0x88` `weights[8]` (carga e peso de demanda de cada entrada), preenchidos por `industry_rebuild_inputs` (`0x471880`, chamada para os 53 tipos na partida e a cada fim de ano): para cada slot válido no ano, atribui índices de entrada sem repetir cargas (`idx`) e `weights[idx] = (u8) ftol(weights[idx] + peso)`. O estoque da indústria (`+0x6b`) é indexado por esse `idx`.

Tabela de slots do conjunto 0 (extraída do `.data`; tipo = índice da classe `BUILDING_CLASS_NAMES`, LNG `920 + tipo`; "x n" = unidades de insumo consumidas por conversão; "w" = peso de demanda; entradas de bônus mostram o estoque mínimo que ativa o bônus):

| type | class (LNG 920+n) | slot | years | mode | output | rate/yr | inputs (need, units) | bonus inputs (threshold) |
|---|---|---|---|---|---|---|---|---|
| 1 | Aluminum Mill | 0 | 1800-9999 | 0 | aluminum | 1 | bauxite x1 (w=5) | - |
| 2 | Auto Plant | 0 | 1800-9999 | 0 | auto | 3 | steel x2 (w=5), tire x1 (w=0) | - |
| 3 | Bakery | 0 | 1800-9999 | 0 | food | 1 | grain x1 (w=2) | - |
| 3 | Bakery | 1 | 1800-9999 | 0 | food | 1 | sugar x1 (w=2) | - |
| 4 | Bauxite Mine | 0 | 1800-9999 | 1 | bauxite | 2 | - | - |
| 5 | Cannery | 0 | 1800-1930 | 0 | food | 2 | coffee x1 (w=3), steel x1 (w=0) | - |
| 5 | Cannery | 1 | 1800-1930 | 0 | food | 2 | produce x1 (w=3), steel x1 (w=0) | - |
| 5 | Cannery | 2 | 1910-9999 | 0 | food | 2 | coffee x1 (w=3), aluminum x1 (w=0) | - |
| 5 | Cannery | 3 | 1910-9999 | 0 | food | 2 | produce x1 (w=3), aluminum x1 (w=0) | - |
| 6 | Cattle Yard | 0 | 1800-9999 | 1 | cattle | 3 | - | grain >= 0.5 (w=1) |
| 7 | Cement Plant | 0 | 1800-9999 | 0 | cement | 1 | gravel x1 (w=5) | - |
| 8 | Chemical Plant | 0 | 1800-9999 | 1 | chemical | 2 | - | - |
| 9 | Coal Mine | 0 | 1800-9999 | 1 | coal | 2 | - | - |
| 10 | Coffee Farm | 0 | 1800-1904 | 1 | coffee | 2 | - | - |
| 10 | Coffee Farm | 1 | 1905-9999 | 1 | coffee | 2 | - | fertilizer >= 0.5 (w=1) |
| 11 | Cotton Farm | 0 | 1800-1904 | 1 | cotton | 2 | - | - |
| 11 | Cotton Farm | 1 | 1905-9999 | 1 | cotton | 2 | - | fertilizer >= 0.5 (w=1) |
| 12 | Dairy Farm | 0 | 1800-9999 | 1 | milk | 2 | - | grain >= 0.5 (w=2) |
| 13 | Dairy Processor | 0 | 1800-9999 | 0 | food | 1 | milk x1 (w=4) | - |
| 14 | Electric Plant | 0 | 1960-9999 | 0 | waste | 1 | coal x3 (w=4) | - |
| 14 | Electric Plant | 1 | 1960-9999 | 0 | waste | 1 | diesel x3 (w=4) | - |
| 15 | Fertilizer Plant | 0 | 1800-9999 | 0 | fertilizer | 1 | chemical x1 (w=6) | - |
| 16 | Grain Silo | 0 | 1800-1904 | 1 | grain | 2 | - | - |
| 16 | Grain Silo | 1 | 1905-9999 | 1 | grain | 2 | - | fertilizer >= 0.5 (w=1) |
| 17 | Gravel Pit | 0 | 1800-9999 | 1 | gravel | 2 | - | - |
| 18 | Housing | 0 | 1800-1919 | 1 | passenger | 0.5 | - | - |
| 18 | Housing | 1 | 1800-9999 | 1 | mail | 0.2 | - | - |
| 18 | Housing | 2 | 1920-1957 | 1 | passenger | 0.4 | - | - |
| 18 | Housing | 3 | 1958-9999 | 1 | passenger | 0.2 | - | - |
| 19 | Iron Ore Mine | 0 | 1800-9999 | 1 | iron | 2 | - | - |
| 21 | Logging Camp | 0 | 1800-9999 | 1 | log | 2 | - | - |
| 21 | Logging Camp | 1 | 1800-9999 | 1 | pulpwood | 1 | - | - |
| 22 | Lumber Mill | 0 | 1800-9999 | 0 | lumber | 1 | log x1 (w=3) | - |
| 23 | Meat Packing Plant | 0 | 1800-9999 | 0 | food | 1 | cattle x1 (w=5) | - |
| 24 | Nuclear Plant | 0 | 1800-9999 | 0 | waste | 2 | uranium x1 (w=2) | - |
| 25 | Oil Refinery | 0 | 1800-9999 | 0 | diesel | 1 | oil x1 (w=5) | - |
| 26 | Oil Well | 0 | 1800-9999 | 1 | oil | 3 | - | - |
| 27 | Paper Mill | 0 | 1800-9999 | 0 | paper | 1 | pulpwood x1 (w=4) | - |
| 28 | Port | 0 | 1800-9999 | 0 | goods | 1 | cotton x1 (w=3) | - |
| 28 | Port | 1 | 1800-9999 | 0 | goods | 1 | wool x1 (w=3) | - |
| 29 | Produce Orchard | 0 | 1800-1904 | 1 | produce | 2 | - | - |
| 29 | Produce Orchard | 1 | 1905-9999 | 1 | produce | 2 | - | fertilizer >= 0.25 (w=0.5) |
| 30 | Rubber Farm | 0 | 1800-1904 | 1 | rubber | 2 | - | - |
| 30 | Rubber Farm | 1 | 1905-9999 | 1 | rubber | 2 | - | fertilizer >= 0.25 (w=0.5) |
| 31 | Sheep Farm | 0 | 1800-9999 | 1 | wool | 2 | - | grain >= 0.5 (w=1) |
| 32 | Steel Mill | 0 | 1800-9999 | 0 | steel | 2 | coal x1 (w=4), iron x1 (w=4) | - |
| 33 | Sugar Farm | 0 | 1800-1904 | 1 | sugar | 3 | - | - |
| 33 | Sugar Farm | 1 | 1905-9999 | 1 | sugar | 3 | - | fertilizer >= 0.25 (w=0.5) |
| 34 | Textile Mill | 0 | 1800-9999 | 0 | goods | 1 | cotton x1 (w=3) | - |
| 34 | Textile Mill | 1 | 1800-9999 | 0 | goods | 1 | wool x1 (w=3) | - |
| 35 | Tire Factory | 0 | 1800-9999 | 0 | tire | 1 | rubber x1 (w=4) | - |
| 36 | Tool and Die Factory | 0 | 1800-1876 | 0 | goods | 1 | iron x1 (w=3) | - |
| 36 | Tool and Die Factory | 1 | 1800-9999 | 0 | goods | 1 | steel x1 (w=3) | - |
| 36 | Tool and Die Factory | 2 | 1800-9999 | 0 | goods | 1 | aluminum x1 (w=3) | - |
| 37 | Uranium Mine | 0 | 1800-9999 | 1 | uranium | 2 | - | - |
| 38 | (unnamed) | 0 | 1800-9999 | 0 | waste | 1 | chemical x1 (w=6) | - |
| 39 | (unnamed) | 0 | 1800-1919 | 1 | passenger | 1 | - | - |
| 39 | (unnamed) | 1 | 1800-9999 | 1 | mail | 0.4 | - | - |
| 39 | (unnamed) | 2 | 1920-1957 | 1 | passenger | 0.8 | - | - |
| 39 | (unnamed) | 3 | 1958-9999 | 1 | passenger | 0.4 | - | - |
| 40 | (unnamed) | 0 | 1800-9999 | 0 | #38 | 1 | #36 x1 (w=0) | - |
| 40 | (unnamed) | 1 | 1800-9999 | 1 | #40 | 1 | - | - |
| 41 | (unnamed) | 0 | 1800-9999 | 0 | #38 | 1 | #36 x1 (w=0) | - |
| 42 | (unnamed) | 0 | 1800-9999 | 0 | #35 | 1 | grain x1 (w=2.5) | - |
| 42 | (unnamed) | 1 | 1800-9999 | 0 | #35 | 1 | sugar x1 (w=2.5) | - |
| 42 | (unnamed) | 2 | 1800-9999 | 0 | #35 | 1 | produce x1 (w=2.5) | - |
| 43 | (unnamed) | 0 | 1800-9999 | 1 | dining | 1 | - | - |
| 45 | (unnamed) | 0 | 1905-9999 | 0 | #33 | 2 | steel x1 (w=3), chemical x1 (w=0) | - |
| 45 | (unnamed) | 1 | 1910-9999 | 0 | #33 | 2 | aluminum x1 (w=3), chemical x1 (w=0) | - |
| 46 | (unnamed) | 0 | 1905-9999 | 0 | #34 | 4 | steel x3 (w=4), rubber x1 (w=0) | - |
| 46 | (unnamed) | 1 | 1910-9999 | 0 | #34 | 3 | aluminum x2 (w=2), rubber x1 (w=0) | - |
| 47 | (unnamed) | 0 | 1800-9999 | 0 | #38 | 1 | #39 x1 (w=9999) | - |
| 47 | (unnamed) | 1 | 1800-9999 | 0 | #41 | 1 | #40 x1 (w=9999) | - |
| 49 | (unnamed) | 0 | 1800-9999 | 1 | uranium | 2 | - | - |
| 50 | (unnamed) | 0 | 1800-9999 | 1 | uranium | 2 | - | - |
| 51 | (unnamed) | 0 | 1800-9999 | 1 | uranium | 2 | - | - |
| 52 | (unnamed) | 0 | 1800-9999 | 1 | uranium | 2 | - | - |

Quem produz e quem demanda cada carga (conjunto 0):

| carga | produzida por (tipos 0..37, set 0) | demandada por |
|---|---|---|
| passenger | (unnamed), Housing | - |
| mail | (unnamed), Housing | - |
| lumber | Lumber Mill | - |
| log | Logging Camp | Lumber Mill |
| milk | Dairy Farm | Dairy Processor |
| cattle | Cattle Yard | Meat Packing Plant |
| steel | Steel Mill | (unnamed), Auto Plant, Cannery, Tool and Die Factory |
| goods | Port, Textile Mill, Tool and Die Factory | - |
| auto | Auto Plant | - |
| cement | Cement Plant | - |
| food | Bakery, Cannery, Dairy Processor, Meat Packing Plant | - |
| cotton | Cotton Farm | Port, Textile Mill |
| wool | Sheep Farm | Port, Textile Mill |
| paper | Paper Mill | - |
| oil | Oil Well | Oil Refinery |
| chemical | Chemical Plant | (unnamed), Fertilizer Plant |
| fertilizer | Fertilizer Plant | Coffee Farm, Cotton Farm, Grain Silo, Produce Orchard, Rubber Farm, Sugar Farm |
| waste | (unnamed), Electric Plant, Nuclear Plant | - |
| coal | Coal Mine | Electric Plant, Steel Mill |
| grain | Grain Silo | (unnamed), Bakery, Cattle Yard, Dairy Farm, Sheep Farm |
| aluminum | Aluminum Mill | (unnamed), Cannery, Tool and Die Factory |
| coffee | Coffee Farm | Cannery |
| tire | Tire Factory | Auto Plant |
| sugar | Sugar Farm | (unnamed), Bakery |
| uranium | (unnamed), Uranium Mine | Nuclear Plant |
| pulpwood | Logging Camp | Paper Mill |
| gravel | Gravel Pit | Cement Plant |
| iron | Iron Ore Mine | Steel Mill, Tool and Die Factory |
| produce | Produce Orchard | (unnamed), Cannery |
| bauxite | Bauxite Mine | Aluminum Mill |
| diesel | Oil Refinery | Electric Plant |
| rubber | Rubber Farm | (unnamed), Tire Factory |
| dining | (unnamed) | - |

Notas:
- Os pesos da **segunda** entrada de vários conversores são 0 no arquivo (pneus no Auto Plant, aço/alumínio na Cannery etc.): a estação é marcada como demandante da carga, mas contribui com peso 0 ao nível de preço (seção 8); provável falha de dados do original.
- Os tipos 38 a 52 não têm nome no LNG. O tipo 39 é uma casa grande (conta como 2 casas, o dobro da produção do tipo 18); os tipos 40 a 47 usam cargas 33 a 41 do conjunto de expansão; 49 a 52 são cópias de produção de urânio.
- Os valores do manual para "Qty. annually" (2, 3, 0.5/0.2 etc.) coincidem com a coluna "taxa".

### 7.2 Taxa de produção por slot (`0x40a8c0`) **[V]**

```
slot válido se saída != 0x7f, ano_inicial <= year <= ano_final e taxa != 0
alt  = 1 se (year fora do intervalo do slot 0) e (year dentro do intervalo do slot 1), senão 0
modo 0 (conversor): se !(modelo básico ou sandbox) ou slot != alt: devolve 0       ; só produz "de graça" no modelo básico
                    base = (tipo == 0x20 ? 3.0 : tipo == 0x0e ? 1.0 : 2.0)           ; siderúrgica 3, usina elétrica 1, demais 2
modo 1 (primário):  base = taxa
bônus = 1.0;  se entrada1 existe e limiar1 <= estoque[idx1]: bônus = limiar1 + 1.0   ; ex.: fertilizante 0,5 -> x1,5
              se entrada2 existe e limiar2 <= estoque[idx2]: bônus += limiar2
devolve bônus * base
```

### 7.3 Produção periódica (`0x40aa20`) **[S; período V]**

Chamada na virada de semana (dias 7, 14, 21 e 28) para toda indústria/casa da lista (ver `time.md`, 7.3) e de novo no fim do ano. Para cada slot `k` (0..3) enquanto `industry_production_rate(k) > 0,01`:

```
prod   = industry_production_rate(k)                                   ; f32
m      = (f32)effect(0x5f + saída, 0, 0, I.território) * effect(0x5e, 0, 0, I.território) * tabela_estado_produção[state]
         ; 0x550494: 0.84 0.92 1.0 1.08 1.16; piso 0,01
período = ftol( 483840.0 / prod / m )                                  ; 483840 = minutos em um ano
         ; ano >= 1970: ftol(período * 0.73);  >= 1950: * 0.82;  >= 1930: * 0.91     (a produção acelera com o tempo)
         ; cheat "Overtime" (0x574c10): período >>= 1
relógio[k] = min(relógio[k], agora);  q = min(1, (agora - relógio[k]) / período)
se q == 0: próximo slot
relógio[k] += q * período
se entrada1 existe e limiar1 <= estoque[idx1]: estoque[idx1] -= limiar1     ; o fertilizante é consumido (0,5 ou 0,25 por lote)
se entrada2 existe e limiar2 <= estoque[idx2]: estoque[idx2] -= limiar2     ; (nos conversores o limiar é 0: nada muda)
cria um lote (industry_spawn_lot) e repete
```
O cálculo de `m` e `período` (até o `>>= 1` do Overtime) é verificado por `tests/cargo.rs::production_period_matches_original` (fragmento `0x40abca..0x40acb2`, 3 989 casos): `m = (e(0x5e) * e(0x5f+saída)) * tabela[estado]` com piso `0.01f32`, `prod` guardado como `f32`.

Ou seja, a produção anual é `prod * m` lotes e todos os lotes devidos são criados a cada chamada (a cada semana, 48 vezes por ano de 336 dias). Em modelo avançado ou expert um conversor só produz quando recebe insumo (7.4). `industry_spawn_lot` (`0x40a770`) cria o lote (`criado = agora`) na pilha, a menos que o slot já tenha 50 lotes esperando (fora do sandbox), e recalcula as estações ligadas (`0x4bebf0`, `+0x4b5 += 1`).

### 7.4 Entrada de carga e conversão (`0x40b250`) **[S]**

Ao entregar `c` a uma indústria: `estoque[idx de c] += 1`, `+0x34 += 1`; para cada slot de modo 0 válido no ano, enquanto o estoque cobre as unidades exigidas de entrada 1 e entrada 2: subtrai-as e cria `ceil(taxa)` lotes de saída (`industry_spawn_lot`), repetindo; no fim recalcula as estações ligadas. Ex.: aciaria (carvão x1 + ferro x1 -> 2 aço), fábrica de automóveis (aço x2 + pneu x1 -> 3 autos).

### 7.5 Casas

Casas (tipo `0x12`) e casas grandes (tipo `0x27`) são "indústrias" de modo 1 com saídas de passageiros e correio e taxas por ano: passageiros 0,5 (até 1919), 0,4 (1920 a 1957), 0,2 (a partir de 1958) e correio 0,2 por casa; o tipo grande dobra tudo. Passam pelo mesmo `m` (estado econômico, eventos).

### 7.6 Carregamento e vida útil dos lotes

- **Carregar** (`station_load_lot`, `0x4bf260`): entre as pilhas das construções do raio da estação (lotes com estação 0) e a lista de lotes largados na estação (`+0x4be`), escolhe o lote da carga pedida com **menor `lot.criado`** (o mais antigo; empate fica com o primeiro encontrado). Grava `lot.origem = estação` (se 0), `lot.vagão`, `lot.carregado = agora`.
- **Transferência** (`station_store_lot`, `0x4beba0`): o lote largado fica na lista da estação com `lot.vagão = 0`.
- **Expiração** (`sim_month_end`, `0x4a5320`, a menos que seja sandbox): lote sem estação (`lot.origem == 0`) com idade `> 216000` min (150 dias) é liberado se não tem indústria de origem; com indústria de origem só é liberado se a idade também for `>= cargo.+8 / 2` (`/ 4` se a indústria não tem estação ligada, `+0x33 == 0`), e a contagem da pilha é decrementada.

## 8. Demanda da estação (`0x4bebf0`) **[S]**

`station_recompute_supply_demand(estação)` (chamada a cada mudança no raio e todo mês), ignorada se `estação.+0x45 != 0`. Zera `+0x61`, `+0x169`, `+0x195`, `+0x1c1`, depois percorre os elos:
- Elo para outra estação (tipo 0): soma os lotes largados lá a `+0x61` e marca `+0x169 = 2`.
- Elo para indústria/casa: soma as pilhas a `+0x61`, marca `+0x169 = 1` nas cargas dos slots válidos (`0x4e2330`) e soma a taxa dos slots primários válidos em `+0xb9` (oferta anual); para cada entrada `j` do tipo (cargas habilitadas em `0x5705d5`): `+0x195[c] = 1`, `+0x1c1[c] += weights[j] / indústria.+0x33` (divisão inteira).
- Casas: `H += 1` (tipo `0x12`) ou `+= 2` (`0x27`) e `w += 1,0 ou 2,0 / indústria.+0x33`. Com `H >= 8` (**cidade**, `+0x51 = 1`) ou `4 <= H < 8` (**vila**, `+0x52 = 1`), para cada carga habilitada cujo ano limite (`+0x16` cidade, `+0x14` vila) ainda não passou: `+0x195[c] = 1`, `+0x1c1[c] += w * demanda_por_casa[c]`.

**Nível de preço** (byte `+0x321 + c`, 0 a 9), para cada uma das 44 cargas, com `d = +0x1c1[c]` e `s = +0x271[c]` (contador de entregas):

```
idade = max(0, year - construção.+0x3c)                       ; construção = registro apontado por estação.+4
n = (s <= 0) ? 9 : ftol(d * 4.0 / s)                          ; guardado em um byte (módulo 256)
se idade < 5: n = ftol( ((5-idade)*d + (idade+2)*n + 3.5) / 7 )
n = ftol(min(n, d + 4.0));  n = min(n, 9)
```
Cada entrega confirmada soma 1,0 a `s` (4.8) e **todo mês `s` é multiplicado por 0,94** (`0x4a5320`, constante `0x5376d8`). O nível, portanto, cai quando a estação recebe muita carga e se recupera pelo decaimento; sem entregas (`s <= 0`) o nível é 9. O preço só usa o nível no modelo expert (`P`, 4.2). Peso `d` pequeno limita o nível: `min(n, d + 4)`.

## 9. Estado econômico (`0x46a010`, `0x570717`) **[S]**

Estado `0..4` (depressão, recessão, normal, prosperidade, boom; LNG `1215..1219`), inicial 2 (`0x46b650`). `economy_state_step` roda no passo mensal dos **meses 2 e 8** (a cada seis meses) e só se o mês mudou desde a última execução (`0x57071f`). Não corre com `0x570237 != 0`.

```
meses = (ano*12 + mês) - (ano_ant*12 + mês_ant)
deriva = ftol(deriva * 0.8) + meses * (estado - 2)                  ; 0x570725
mom    = clamp( ftol( (rng_range(-5,5) + 2) - deriva * 0.1 + mom ), -5, 5 )   ; 0x57071b
se rng_range(0, 10) < |mom|:  estado += (mom <= 0) ? -1 : +1           ; clamp 0..4
se mudou: manchete (ids em 0x5538dc), prime_rate = 0.09 - estado * 0.01, e cada empresa recalcula (0x43b410)
```
O sorteio usa o RNG do jogo (`rng.md`, gerador A) na ordem: primeiro `rng_range(-5, 5)`, depois `rng_range(0, 10)`.

Tabelas indexadas por estado (f32[5], `.data`):

| Endereço | Valores | Uso |
|---|---|---|
| `0x550494` | 0,84 0,92 1,00 1,08 1,16 | multiplicador de **produção** (7.3) [S] |
| `0x5504bc` | 0,8 0,9 1,0 1,1 1,2 | multiplicador de **receita** de carga (4.4) [V] |
| `0x5504a8` | 0,8 0,9 1,0 1,1 1,2 | custo de construção de trilho (`0x463520`, ver spec de trilhos) |
| `0x5504d0` | 0,8 0,9 1,0 1,1 1,2 | valorização da empresa (`0x43aea0`, `0x43b650`, ver spec de finanças) |
| `0x5504e4`, `0x5504f8` | 0,9 .. 1,1 e 0,96 .. 1,04 | crescimento de cidades (`0x430540`) |

A taxa de juros primária é `0,09 - estado * 0,01` (`0x5706d1`).

## 10. Modelos industriais e opções

| Opção | Efeito no código |
|---|---|
| Modelo industrial (`0x573d78` = opção `& 7`) | 0 básico: toda estação aceita qualquer carga (x0,8 sem demanda) e conversores produzem sem insumo (`0x573d77 = 1`, 7.2); 1 avançado: não demandada x0,5 (humano) e conversor exige insumo; 2 expert: não demandada x0,2 (humano), nível de preço 0,5..1,4 (`0x573d76 = 1`) e, a menos de 16 de distância, a receita não demandada cai com `d/16` |
| Sliders de receita (`0x574874`) | tabelas 0,75..1,25 (humano) e 1,55..2,8 (computador) (4.4) |
| Sandbox (`0x570227`) | sem níveis de preço; produção pelo ramo próprio de `0x40aa20` |
| Cheats `Overtime`, `Viagra` (`0x574c10`, `0x574c14`) | período de produção pela metade; crescimento de cidades dobrado |

## 11. Divergências do manual (o código vale)

- **Preços-base**: o código tem passageiros 56.000, correio 65.000, leite 51.000, comida 49.000, "waste" (perigosos) 48.000 (valores de `+0x00` antes do x1,4); o manual (SG Tabela 2-1: 60.000, 70.000...) e a Tabela A-2 do Apêndice (60.000, 70.000, 55.000, 58.000, 46.000) diferem nesses cinco; as demais cargas coincidem com a Tabela A-2 (por exemplo madeira 35.000, carvão 28.000, tronco 28.000), e **não** com a Tabela 2-1 do guia (30.000 etc.). O preço efetivo de partida é 1,4x o valor do arquivo.
- **Rot Factor**: não há "rot factor" no código; o campo equivalente é o f32 `ship` (0,21 a 0,54), com a mesma ordem de grandeza (passageiros 8 -> 0,45; correio 10 -> 0,54; carvão 2 -> 0,21).
- **Prazos**: "dias de entrega" do guia (500, 300, 1600...) vs. o código (500, 300, 1600/1300/800/1100/600...): a tabela A-2 bate com o código (madeira 1300), a Tabela 2-1 do guia não (1600). O prazo **não faz a carga sumir**: ele escala a penalidade de tempo (4.3, mínimo 30% do preço) e define a vida útil do lote não coletado (7.6).
- **Distância e tempo**: o manual diz que a fórmula "não é documentada". Está em 4.1 a 4.3.
- **Carga sem demanda**: o SG diz 80/50/30% e o AG 80/50/20% (Básico/Avançado/Expert). Código: humano 80/50/20 (bate com o AG), computador 80/55/40 (não documentado).
- **Estado econômico**: bate com o manual (+/-10% por nível na receita, +/-8% na produção, boom +16%; juros 9% a 5%). A duração "6 a 18 meses" vem do passo semestral com chance `|mom|/11`.
- **Taxa de estação alheia**: manual $1.000 por carga em cada ponta; código **$2.000 por vagão carregado em cada ponta** (6), cobrada da parada anterior e da atual, e também creditada a si mesmo quando a estação é própria.
- **Bônus de passageiros**: manual e LNG (restaurante +20/+30, saloon +10, hotel +20/+40%); o código em versão >= 9009 (a única em jogo local) paga 10/15, 5 e 15/25% e o **entrega ao dono da estação**, não ao trem (4.8). Vagão-restaurante +20% e recorde de velocidade +15% batem com o manual.
- **Perda por atraso de coleta**: o manual fala em melhorias que "reduzem a penalidade de atraso pela metade"; o código reduz **pela metade o tempo de espera contado** (`p1 >> 1`) quando a estação tem a melhoria da carga (3, bit em `cargo+0x1c`).
- **Leite**: guia 1889, apêndice 1900; código 1899 (vilas e cidades). Petróleo e carvão: 1909 (apêndice 1910, guia 1899). Café e produce: 1878 (bate).

## 12. Verificação (`crates/rt2-oracle/tests/cargo.rs`)

| Teste | O que cobre | Casos |
|---|---|---|
| `effect_multiplier_matches_original` | `0x46b480` com os quatro níveis (global, empresa, pessoa, território), argumentos opcionais e piso 0,01 | 6000 |
| `industry_input_rebuild_matches_original` | `0x471880`: `inputs` e `weights` dos 53 tipos nos dois conjuntos em 15 anos | 1590 registros |
| `production_rate_matches_original` | `0x40a8c0`: slot, ano, modelo básico/sandbox, bônus de insumo | 16000 (4 slots x 4000) |
| `revenue_matches_original` | `0x4bf760` com `commit=1`: listas, empresas, pessoas, territórios, estações e indústrias montadas na memória emulada; ramos de demanda por indústria, por vila/cidade e sem demanda | 8000 cenários, bit a bit |
| `revenue_depends_on_x87_precision` | mostra que 53 vs 24 bits muda cerca de 10% dos resultados | 3000 |

A preparação é a do jogo: `price = ftol(price * 1.4)` e `+8 = dias * 1440` na tabela de cargas (a rotina `0x471b10` não roda no oráculo), `industry_rebuild_inputs` para o ano do caso, e listas montadas com o cabeçalho `(+8 stride, +0x28 base)` que `list_record` espera.

## 13. Fidelidade e implicações para o clone

- Pinar o x87 em 53 bits ao reproduzir a receita (4.7); registrar a precisão como parte das regras do replay.
- Reproduzir os `ftol` intermediários, o fator `X` aplicado duas vezes, a soma (e não produto) do efeito `0x3c`, a redução progressiva e a taxa de $2000 por ponta.
- O arredondamento `ftol(raw * 1.4)` da tabela de cargas deve ser feito em tempo de carga, não digitado (bens 58799, papel 64399, urânio 60199 a 53 bits).
- A seleção de consumidor usa `rng_hash_range(lote, 0, n-1)` (sem estado): determinístico.

## Em aberto

- Precisão do x87 no processo real (ver `time.md`); decide se `58799` ou `58800`, etc.
- Ramo `commit == 0` de `cargo_delivery_revenue`, `train_arrival_settlement`, `industry_receive_cargo`, `industry_production_tick` e `station_recompute_supply_demand` estão **[S]**: falta teste diferencial (exigem alocar e liberar lotes, ver `0x4f17e0`/`0x4f1690`).
- Como o trem acumula a partilha de trilho (`trem+0x7d..0x8a`, em `0x4d0ce0`/movimento) e o que significa `0x4d2590` (vagão elegível para descarga).
- Raio de captação (criação dos elos `0x574c58` na construção da estação), crescimento de cidades e criação de indústrias (spec `economy-world`).
- Fórmula de `0x4399d0` (atualização mensal de `empresa.+0x4276`) e `0x43b410` (efeito da mudança do estado econômico nas empresas).
- Significado de `0x570229` (regra de lixo/perigosos: só os tipos `0x14` e `0x18` contam em `0x4bebf0`), de `0x570237` e do byte `+0x20` da tabela de cargas.
- Nomes e função das cargas 33 a 45 e do conjunto 1 de indústrias (expansão).
- O crédito de $2000 à própria empresa quando ela é dona da estação (6, item 3) parece dupla contagem; mantido por fidelidade.
- Efeitos `0x29` (usado por `0x4d2c10`) e demais ids de efeito não usados pela carga; compra de indústria (modelo expert), fora desta spec.
- Ligação exata dos totais de passageiros, correio e carga aos lançamentos `0x65/0x66/0x67` (6, item 6).

> **Resolvido (2026-10-22):** a precisão x87 da thread principal, medida com o jogo rodando no Wine, é **53 bits** (control word `0x27F`) em todas as amostras: frame gate, `sim_advance` e receita. Ver `docs/dynamic-analysis.md`. Os testes do oracle devem fixar `0x27F`.
