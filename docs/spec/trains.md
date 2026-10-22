# Trens: locomotivas, movimento, quebras e custos

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido no C exportado (`re/db/decomp/`) e no disassembly do rizin. O manual e o guia (`docs/reference/manual-rules.md`, seção 4) serviram só de pista; as divergências estão na seção 9. Estado das provas: **[V]** = função confirmada contra o original por teste diferencial (`crates/rt2-oracle/tests/trains.rs`, precisão x87 de 53 bits); **[S]** = lida no código, sem teste; **[D]** = conferida contra os dados (tabela do apêndice reproduzida a partir do `.data`).

## 1. Visão geral

```
sim_advance 0x4a5cb0 (jogo local, caminho linear)          ; time.md, seção 6
  para cada trem da lista 0x574c64: train_step(trem, dt, 1)          0x4d0ce0
      train_update_speed(trem, T)                                    0x4d0400   aceleração [V]
          train_target_speed(trem)                                   0x4d00c0   alvo [V]
              engine_max_speed(motor, toneladas, rampa, curva, idade) 0x4cfea0  curva de velocidade [V]
      dist = f32(vel * 0.000625 * dt); avança o vagão da frente e os demais
      train_breakdown_check(trem)  se vel > 0 e em movimento          0x4cf390   sorteio de quebra
          train_breakdown_chance(trem, células, 1)                   0x4cf030   probabilidade [V]
sim_month_end 0x4a5320 -> train_monthly_costs(trem) para cada trem   0x4d4390   combustível e manutenção
```
Em rede (jogadores > 1) os trens andam uma vez por dia com `dt = 1440` (`trains_advance` `0x4a58f0`, `time.md`).

### 1.1 Estruturas

- **Trem** (registro de 0x14d bytes, coleção `train_list` `0x574c64`). Campos usados aqui: `+0` id do vagão da frente; `+0x3d` id do trem; `+0x41` empresa; `+0x69` velocidade atual (i32, mph em ponto fixo 16.16); `+0x6d` minuto da última atualização de velocidade; `+0x71` minuto de partida agendada (0 = nenhuma); `+0x99` parado (1) ou andando (0); `+0x9c` já está nos trilhos; `+0xa0` células percorridas desde a última verificação de quebra (i16); `+0xa2` toneladas x células do mês (combustível); `+0xaf` peso total (t); `+0xb3` peso só dos vagões de carga; `+0xbb` bits (`2` = tem caboose, `8` = elétrica sem catenária); `+0xbd` ano de compra (u16); `+0xbf` distância por minuto (16.16); `+0xcf` areia, `+0xd3` óleo, `+0xd7` água (f32, 0..100); `+0xdb` acelerador (f32, 85 = normal); `+0xe1` células do mês; `+0xed` quebrado, `+0xf2` acidentado, `+0xbc` outro estado de parada (bytes); `+0xee` minuto em que o estado termina; `+0xf3` jogador/território do último trecho. Criação em `train_create` `0x4cdf80`: areia, óleo e água = 100, acelerador = 85, `+0xbd` = ano corrente, `+0x99 = 1` [S].
- **Veículo** (coleção `vehicle_list` `0x574c70`; cada trem é uma lista ligada pelo campo `+2`). `+8` tipo de veículo, `+9`/`+0xb` tile x/y, `+0x13` comprimento do trecho atual (16.16), `+0x17` posição dentro do trecho (16.16), `+0x23`/`+0x24` direção de entrada/saída do tile (0..7), `+0x2a` está nos trilhos, `+0x2d` quantidade de carga (0 = vazio).
- Coleções: `collection_get` (`0x4f18b0`) devolve `base(+0x28) + id * passo(+8)`; id 0 = nulo.

## 2. Dados das locomotivas e vagões

### 2.1 Onde ficam
As duas tabelas estão no **`.data` do executável**, não em PK2. Nenhum arquivo de dados as sobrescreve; o código só preenche campos derivados ao carregar o mapa [S]:

- `engine_table` `0x53cec0`: **66 registros de 0x3a bytes** (índice de motor 0..65). Nome: string `engine_name_ids` (`0x5534f4`, u16 por motor) no `.lng` (717 + índice na maioria; 2636..2638 e 3046..3050 para os acrescentados na Platinum).
- `vehicle_type_table` `0x53ddb8`: **175 registros de 0x39 bytes** (locomotiva, tênder, vagões). O tipo de veículo é o byte `+8` de cada veículo.
- Escritas em tempo de execução: `0x471b10` (carga do mundo) preenche os campos `+0x2b..+0x31` de cada motor e copia os anos para os veículos; `game_load_file` (`0x477a60`, `0x477c..`) troca o primeiro ano do motor 3 (TGVx) para 1955 se `net_min_peer_version() >= 0x2333` (jogo local) ou 1973 se menor [S].

### 2.2 Registro de locomotiva (`engine_table + e*0x3a`)

| Off | Tipo | Campo | Uso (endereço) |
|---|---|---|---|
| `+0x00` | u8 | habilitado (0 só no motor 6, "0-4-0 A3") | `engine_available` `0x4683a0` |
| `+0x01` | u16 | primeiro ano | idem (copiado para o veículo `+0x1c`) |
| `+0x03` | u16 | último ano | idem |
| `+0x05..07` | u8 x3 | região A (América), E (Europa), W (mundo) | editor de mapa, comandos `0x8330..0x8332` em `0x47ef50` copiam para `engine_allowed` `0x570667` |
| `+0x08` | i32 | classe de confiabilidade 0..10 | índice em `reliability_rate` `0x55c52c` |
| `+0x0c` | i32 | preço (US$) | compra `0x4cdf80`, UI `0x4e3f90` |
| `+0x10` | i32 | manutenção anual base (US$) | `0x4d4390` |
| `+0x14` | i32 | velocidade máxima (mph) | `0x4cfea0` |
| `+0x18` | i32 | potência (tração) | `0x4cfea0` |
| `+0x1c` | i32 | constante de aceleração (menor = mais rápido) | `0x4d0400`; UI: classe `(x+1)/5` limitada a 0..11 |
| `+0x20` | i32 | toneladas "livres" (descontadas do peso) | `0x4cfea0`, `0x4cf030` |
| `+0x24` | i32 | consumo de combustível base | `0x4d4390` |
| `+0x28` | u8 | tipo: 0 vapor, 1 diesel, 2 elétrica | modificadores, água, catenária |
| `+0x29`, `+0x2a` | u8 | sons (`+0x2a` indexa `0x55e7b0`, nomes de 0x12 bytes) | `0x4d0400`, `0x4d2360` |
| `+0x2b` | u8 | tem tênder (runtime) | `0x471b10` |
| `+0x2c`, `+0x2d` | u8 | tipo de veículo da locomotiva e do tênder (runtime) | idem |
| `+0x2e` | i32 | peso cheio locomotiva + tênder (runtime) | `0x4cfea0`, `0x4cf030` |
| `+0x32`, `+0x36` | u8, i32 | não usados nas funções lidas | Em aberto |

Classes de confiabilidade (`0x55c52c`, f32, quebras por célula): `1/400, 1/600, 1/875, 1/1100, 1/1400, 1/1800, 1/2500, 1/4000, 1/6000, 1/12000, 1/24000` (Atrocious .. Near Perfect, iguais à tabela 4-1 do guia).

### 2.3 Registro de veículo (`vehicle_type_table + t*0x39`)
`+0x00` nome curto (ex.: `sroc_`, `troc_`, `fcoal`); `+0x06` u16 comprimento (usado como meia distância de engate em `0x415040`); `+0x18` u16 peso vazio (t); `+0x1a` u16 peso cheio (t); `+0x1c`/`+0x1e` u16 anos de disponibilidade; `+0x2e` u8 espécie (0 locomotiva, 1 tênder, 2 vagão); `+0x2f` i8 índice do motor; `+0x30` u8 carga (`0x7f` = nenhuma, `0x2c` restaurante, `0x2d` caboose; 0..43 = tipo de carga, nome curto no `.lng` 776 + índice). Peso de um veículo (`vehicle_weight` `0x414460`): cheio se `+0x2d != 0` ou se o byte global `0x570236` estiver ligado, senão vazio. O peso do trem `+0xaf` é a soma de todos os veículos, locomotiva e tênder incluídos (`train_recalc_weight` `0x4cfc40`).

Tabelas completas: anexos A e B no fim.

### 2.4 Disponibilidade (`engine_available` `0x4683a0`) [S]
```
v = modifier_raw(e < 0x3e ? 0x7f + e : 0xf3 + e)       ; 0x46b5d0: >0 força, <0 proíbe
se v > 0: disponível;  se v < 0: indisponível
senão: +0 != 0 e engine_allowed[e] (0x570667, todos 1 por padrão; o editor copia A, E ou W)
       e (ano > primeiro ou (ano == primeiro e mês > 5)) e ano <= último     ; anos do veículo (+0x1c/+0x1e)
```
Ou seja, um modelo aparece em **junho** do primeiro ano e some depois do último.

### 2.5 Modificadores (`modifier_factor` `0x46b480`) [S; caminho global V]
Fator multiplicativo por id: `f32(1 + pct * 0.01)` com `pct = max(i16, -99)` lido de `0x5707c6 + 2*id` (global do cenário), multiplicado pelos equivalentes da empresa (`+0x1ae`), do jogador/gerente (`+0x26b`) e de `0x574ca8` (`+0x58`); resultado mínimo 0.01. Ids usados pelos trens (`t` = tipo 0/1/2):

| Id | Efeito |
|---|---|
| `0x11`, `0x12+t` | combustível |
| `0x15`, `0x1b+t` | manutenção (o `0x1b+t` também entra no preço) |
| `0x1a` | preço de compra |
| `0x19` | custo mensal por vagão |
| `0x21` | roubos (seção 6.4) |
| `0x23`, `0x24+t` | velocidade |
| `0x27` | confiabilidade (divide a chance de quebra) |
| `0x2a`, `0x2b+t` | potência |
| `0x2e`, `0x2f+t` | aceleração (divide a constante de tempo) |

## 3. Unidades e posição

- **Velocidade**: mph em ponto fixo 16.16 (`0x10000` = 1 mph). O mínimo devolvido pela curva é 1 mph.
- **Tempo**: minutos de jogo (`time.md`).
- **Posição**: cada veículo guarda o tile, as direções de entrada/saída e a posição `+0x17` dentro do trecho, em unidades 16.16. Comprimento do trecho (`track_segment_length` `0x4ca4e0`): reta ortogonal **64**, reta diagonal **90**, curvas 80, 69 ou 91 (conforme a tabela `0x55c3d8`) [S]. Em certos tiles do percurso (bit 1 da rota do trem) o comprimento é dividido por 2.
- **Distância por minuto** = `velocidade(16.16) * 0.000625` (`0x537800`). A 60 mph o trem anda 2457 unidades/min, cerca de 0,84 tile reto por dia.

## 4. Movimento (P0)

### 4.1 Rampa de um trecho (`track_segment_grade` `0x4284e0`) [V]
Alturas por vértice (`vertex_heights` `0x574c28`, i16 a cada 3 bytes, largura `0x5609ac`); os quatro cantos do tile estão nos deslocamentos `tile_corner_offsets` `0x580fd8` (i32[4]). Direção `d` (0..7): ímpar = canto `((d>>1)+1)&3`; par = meio da aresta, média aritmética (`>>1`) dos cantos `d>>1` e `((d>>1)+1)&3`.
```
rampa = (h(saída) - h(entrada)) * 0.005
se entrada e saída ímpares (diagonal): rampa *= 0.7092
senão se uma delas ímpar:             rampa *= 0.85
```
Uma unidade de altura num tile reto vale 0,5% de rampa (0.005 = fração). A rampa do trem (`0x4d00c0`, `0x4cf030`, `train_grade` `0x4cfd40`) é a média ponderada pelo peso dos **dois primeiros** veículos nos trilhos: `acc = f32(rampa_i * peso_i + acc)`, `g = f32(acc / soma_pesos)`; 0 se `+0x9c == 0`.

### 4.2 Curva de velocidade (`engine_max_speed` `0x4cfea0`) [V]
Fastcall: ECX = motor, EDX = toneladas (peso total do trem), pilha = rampa (f32), curva (0/1), idade (anos), empresa, jogador. Devolve mph 16.16.
```
idade = min(idade, 50);  af = f32(idade * 0.005)
se g > 0: g = f32((4g + 1) * g)
w = curva ? ftol(toneladas * 1.4) : toneladas
mult = g > 0 ? (20g + 1)^2.3 : max(g, -0.03) * 10 + 1          ; descida alivia até 0.7
e = ftol(mult * w) - (motor.+0x2e + motor.+0x20);  e = max(e, 0)
e = ftol((af + 1) * e)                                          ; idade pesa mais
P = f32(mod(0x2a) * mod(0x2b+t) * potência * 0.92);  P = max(P, 1.0)
s = ftol(0.98^(e / P) * (vmax << 16))
s = ftol(s * (1 - af))                                          ; -0,5% por ano, até 25%
se curva: s = ftol(s * 0.6)
s = ftol(mod(0x23) * mod(0x24+t) * s)
devolve max(s, 0x10000)
```
`ftol` = truncamento do CRT. Constantes: `0x535320` 0.005, `0x5375b4` 4, `0x535388` 20, `0x5377c8` 2.3, `0x5377b8/0x5377b4` -0.03, `0x535714` 10, `0x5354a0` 1.4, `0x535560` 0.92, `0x5377f0` 0.98, `0x535158` 0.6. [D] Com modificadores neutros, idade 0 e `toneladas = (locomotiva + tênder) + carga`, a função reproduz **todas** as colunas 0@0%, 150@0%, 300@0%, 150@4%, 300@4% da tabela A-1 do guia (teste `engine_speed_reproduces_guide_table` e o anexo A): "150 t" é carga além da locomotiva e "4%" é `g = 0.04`.

### 4.3 Velocidade alvo (`train_target_speed` `0x4d00c0`) [V]
```
se acidentado, quebrado ou +0xbc: alvo = 0
curva = algum veículo nos trilhos com (entrada - saída) & 3 != 0     ; a varredura para no primeiro
g = rampa do trem (4.1);  se areia <= 1: g = f32(2g)                  ; sem areia a rampa dobra
s = engine_max_speed(motor, +0xaf, g, curva, ano - +0xbd, empresa, +0xf3)
s = ftol(acelerador * (1/85) * s)                                     ; 85 = velocidade nominal
classe de estrutura sob os veículos (lista 0x574c84, campo +0x11, prioridade em 0x55c558):
    classe 3: s = ftol(s * 0.7);  classe 1: s = ftol(s * 0.45);  ambas marcam "restrito"
se vapor e água <= 1: s = ftol(s * 0.5)
se 0x574c08 != 0: s *= 2
"restrito" (saída por ponteiro) = curva ou classe 1/3
```

### 4.4 Passo de aceleração (`train_update_speed` `0x4d0400`) [V]
Chamado a cada `train_step` com `agora = game_time_minutes`. Caminho com o trem andando (`+0x99 == 0`):
```
tau = motor.aceleração * 1440                            ; minutos
se vel > alvo: tau >>= (restrito ? 4 : 1)                ; frear é 2x (16x em curva) mais rápido
se quebrado ou acidentado: tau >>= 1
tau -= tau >> 2                                          ; x0.75
m = mod(0x2e) * mod(0x2f+t);  se m < 0.1: m = 0.1
tau = ftol(tau / m)
dt = (+0x6d != 0) ? agora - +0x6d : 0;  dt = clamp(dt, 0, tau)
div = alvo < 0 ? tau / 2 : tau
vel += ftol(dt / div * (alvo - vel))                     ; aproximação exponencial por passos
+0x6d = agora
```
A velocidade fecha a fração `dt/tau` da diferença até o alvo a cada passo; o resultado depende do tamanho do passo (`time.md`, seção 4). Exemplos: Rocket (50) tem `tau = 54000` min (37,5 dias), Shinkansen (5) 5400 min.
Com o trem parado (`+0x99 != 0`): velocidade = 0, roda a lógica de estação (carga, espera); na partida (`0x4d0a70` devolve 1) o passo começa em `dt = agora - partida` e toca o apito.

### 4.5 Avanço (`train_step` `0x4d0ce0`) [S]
```
se acidentado e agora >= +0xee: apaga o trem (train_delete 0x4cdd50)
se ainda fora dos trilhos: monta/posiciona (0x4d1bb0) e calcula o bit 8 (elétrica sem catenária)
se flag: train_update_speed(agora)
se andando e +0xa7 != 0, ou parado e pronto para partir:
    tmp = f32(vel * 0.000625 * dt)
    +0xbf = ftol(tmp / dt)                                ; distância por minuto
    real = vehicle_advance_head(frente, ftol(tmp))        ; 0x415040: percorre tiles, pode parar antes
    +0xc3 = agora - dt
    para cada veículo: vehicle_advance(v, ftol(real))     ; 0x4147e0
    0x4d0bc0(trem)
    se vel > 0 e andando: train_breakdown_check(trem)
```
Ao entrar num tile novo (`train_enter_tile` `0x4d5370`): `+0xa0 += 1`, `+0xe1 += 1`, desgaste do trecho (`+0x17/+0x1b` da peça de trilho `+= peso`), contagem de trechos de outras empresas. No passo de rota (`0x412ff0`), se o trem consome: `+0xa2 += peso`, óleo `-= 0.5`, água `-= 1.2`, areia `-= |rampa| * 140` [S].

## 5. Quebras e acidentes

### 5.1 Verificação (`train_breakdown_check` `0x4cf390`) [S]
Roda depois do avanço quando `+0xa0 >= 10` células e o trem não está parado por quebra/acidente; `0x5748fc != 0` desliga.
```
humano = 0x43c030(empresa)                            ; jogador da empresa com +0x122 != 0
p = f32(train_breakdown_chance(trem, +0xa0, 1));  c = f32(+0xa0)
se !humano: p *= 0.5; c *= 0.5;  (e p = 0 se a flag 0x570229 estiver ligada)
+0xa0 = 0
se p > rng_range(0, 1000000) * 1e-6:                  ; RNG do jogo
    lim = 20 (14 a partir de 1970, 17 de 1920 a 1969; só com versão >= 0x2334)
    se rng_range(0, 100) < lim e 0x570229 == 0: train_crash   0x4ce730
    senão:                                    train_breakdown 0x4ce4c0
senão: teste de roubo (6.4)
```
`rng_range(0, 100)` tem 101 valores: a chance de acidente é 20/101 (≈19,8%).

### 5.2 Probabilidade (`train_breakdown_chance` `0x4cf030`) [V]
Thiscall: trem, células, `checa_idade`. Devolve f32 em ST0.
```
idade = ano - +0xbd
se checa_idade e idade < 1: devolve 0                  ; locomotiva nova não quebra no primeiro ano
x = f32(células * reliability_rate[motor.+0x08])
se acelerador > 85: x = ((acel - 85) * 3 * (1/15) + 1) * x      ; 100% = x4 (versão >= 0x232d)
senão:              x = x * acel / 85
x = f32(x);  x = f32(x / max(mod(0x27), 0.01))
mult = g > 0 ? (20g + 1)^2.3 : max(g, -0.03) * 10 + 1          ; rampa do trem, sem o (4g+1)
e = max(ftol(mult * +0xaf) - (motor.+0x2e + motor.+0x20), 0)
y = (e + 150) * x * 0.005                                       ; 150 t extras dobram a chance
se caboose (+0xbb & 2): y *= 0.75
y *= (idade + 30) / 30                                          ; +33% aos 10 anos, +66% aos 20
se idade <= 0: y *= 0.3;  senão se idade <= 1: y *= 0.6
y *= óleo < 1 ? 3 : (200 - óleo) / 100                          ; meio tanque = x1.5
y = f32(y * 0.7)
(versão >= 0x2334) ano >= 1970: y = f32(y*0.7);  1920..1969: y = f32(y*0.85)
se jogadores <= 1 e nowreck (0x574b74): devolve y * 0.0001
```
O parâmetro `/nowreck` (`game_init_paths` `0x465050`) ou a trapaça `nowreck` (`0x46c8a0`) **não zeram** a chance: multiplicam por 1e-4.

### 5.3 Consequências [S]
| | Quebra `0x4ce4c0` | Acidente `0x4ce730` |
|---|---|---|
| Estado | `+0xed = 1` | `+0xf2 = 1` |
| Duração | `rng_range(60, 120)` dias (`* 1440` min) | `rng_range(30, 45)` dias, depois o trem é **apagado** (`0x4d0ce0` -> `0x4cdd50`) |
| Penalidade | `2000 / (n^0.66 + 4)` | `8000 / (n^0.66 + 4)` |

`n` = número de trens da empresa (`empresa+0x27`). A penalidade sai de `empresa+0x4229` (mínimo 0) e do valor por território `empresa+0x47a + terr*0x1ed` (i16, mínimo -10000; território do tile da locomotiva, ou 1 fora dos trilhos). Os dois também marcam `+0xa8 = 20` e põem o minuto da locomotiva `+0x3d` em `agora - 500`. Acidente com velocidade de jogo > 4 rebaixa a velocidade para 4. Ao fim da quebra, `0x4d0400` limpa `+0xed`/`+0xbc` e chama `0x4d59b0`.

### 5.4 O rand do CRT nos trens
`0x4d0400`, `0x4d2360` e `0x4d29e0` chamam `crt_rand_range(0, 3)` só para o som: nos dois primeiros o resultado é descartado (o `EAX` é sobrescrito logo depois, `0x4d07c4`/`0x4d07ec`, `0x4d253a`/`0x4d256f`); em `0x4d29e0` ele escolhe a variante do som (`+ 0x27`, índice em `0x55e7b0`). **Não afeta a simulação**, mas consome o estado do CRT. As quebras usam o RNG do jogo (`rng.md`, gerador A).

## 6. Custos

### 6.1 Compra (`train_create` `0x4cdf80`) [S]
`preço = ftol(mod(0x1a) * mod(0x1b+t) * motor.preço)`, gravado em `+8` e lançado no razão 4 da empresa (`0x4398d0`). `empresa+0x27 += 1`.

### 6.2 Mensal (`train_monthly_costs` `0x4d4390`, chamado por `sim_month_end`) [S]
```
idade = ano - +0xbd
se a empresa não é humana: areia = óleo = água = 100, acelerador = 85
unid = +0xa2 + +0xe1 * 200                          ; toneladas x células + 200 por célula
fa = f32((idade + 100) * 0.01)
combustível = ftol(mod(0x12+t) * mod(0x11) * fa*fa * (-unid) * nível_comb[0x567d6d[t]] * motor.+0x24 * 0.005 * 0.35)
manut = ftol(mod(0x1b+t) * mod(0x15) * nível_man[0x567d6d[t]] * motor.+0x10 * (óleo <= 1 ? 3 : 1)
             * f32(((idade*4 + 100) * 0.01)^2) * (-1/12))
vagões = ftol(mod(0x19) * (-200 * nº de veículos com carga != 0x7f))
train_charge(3, combustível); train_charge(1, manut); train_charge(2, vagões)     ; 0x4d4200
+0xa2 = +0xe1 = 0
```
`nível_comb` = `0x552ae8` `{1.15, 1.0, 0.85}`, `nível_man` = `0x552af4` `{1.05, 1.0, 0.95}`, indexados por `0x567d6d[tipo]` (estado de jogo; 1 = neutro). `train_charge` (`0x4d4200`) soma o valor nos acumuladores do trem (`+0xfd + 4*cat`, `+0x13d + 4*cat`, total `+0xb7`) e no razão da empresa (categorias 1/2/3 -> razões 6/7/8).

A manutenção cresce com `(1 + 0.04*idade)^2` (x1,96 aos 10 anos, x3,24 aos 20) e triplica sem óleo; o combustível cresce com `(1 + 0.01*idade)^2` e é proporcional ao peso transportado por célula.

### 6.3 Estimativas da janela de compra (`0x4e3f90`) [S, D]
- Preço = `mod(0x1a) * mod(0x1b+t) * preço`; manutenção anual = `mod(0x15) * mod(0x1b+t) * nível_man * manut`.
- Combustível anual = `motor.+0x24 * nível_comb * mod(0x11) * mod(0x12+t) * (v * 0.7 + 40) * 0.5`, com `v = engine_fuel_speed(motor, empresa, acelerador)` (`0x4d6030`) `= ftol((engine_max_speed(motor, +0x2e + 120, 0.0002, 0, 0, empresa, 0) >> 16) * acel/85 * 1.8)`.
- Com modificadores neutros esses números **são** as colunas Cost, Maint. e Fuel da tabela A-1 do guia (anexo A, última coluna).

### 6.4 Roubo de trem (`0x4cf390`, segundo ramo) [S]
`c = células * 0.0005 * pct(0x573e02) * 0.01`; sai se `c <= 1e-5`. Se há uma cidade/estação próxima (`0x4021e4`), `k = dist(0x4f0cd0) - 2*nível - 3`; se `k < 1` não há roubo, senão `c *= 1 + 0.1k`. `c /= (mod(0x21) + 1) * 0.5`; caboose: `c *= 0.4`. Se `c > rng_range(0, 1000000) * 1e-6` e o trem leva passageiros ou correio (carga 0 ou 1), há roubo (`0x4ceaa0`) ou, com `rng_range(0,100) < (mod(0x21)+1) * 0.1 * 100` (x1.3 com caboose), os ladrões são presos (mensagens 0x8b2/0x8b3).

## 7. Envelhecimento
Não há obsolescência forçada: a idade (`ano - +0xbd`) só entra nas fórmulas: velocidade -0,5% por ano e peso efetivo +0,5% por ano até 50 anos (4.2); chance de quebra `(idade + 30)/30`, com 0 no primeiro ano e x0.6 no segundo (5.2); manutenção `(1 + 0.04 idade)^2` e combustível `(1 + 0.01 idade)^2` (6.2).

## 8. Verificação diferencial
`crates/rt2-oracle/tests/trains.rs` monta trens e veículos na memória emulada (coleção falsa em `0x574c70`, tiles e alturas aleatórios, tabelas do `.data` com os campos de `0x471b10` preenchidos, modificadores globais aleatórios) e compara com referências em Rust:

| Teste | Função | Casos | Resultado |
|---|---|---|---|
| `track_grade_matches_original` | `0x4284e0` | 20 000 | ST0 idêntico bit a bit |
| `engine_speed_curve_matches_original` | `0x4cfea0` | 30 000 | idêntico (inclui `pow` do CRT vs. `f64::powf`) |
| `engine_speed_reproduces_guide_table` | `0x4cfea0` | 4 motores x 5 colunas | igual à tabela A-1 |
| `train_accel_step_matches_original` | `0x4d0400` (+ `0x4d00c0`) | 10 000 | velocidade e `+0x6d` idênticos |
| `breakdown_chance_matches_original` | `0x4cf030` | 10 000 | ST0 idêntico bit a bit |

Os testes usam a palavra de controle 0x27f (53 bits); com 24 bits podem surgir diferenças de arredondamento como em `time.md`, seção 3.

## 9. Divergências do manual e do guia
- **Preços**: a tabela A-1 do guia bate com o `.data` em anos, regiões, preço, manutenção e velocidades para os modelos originais, mas diverge em GP9 (EXE 235 000, guia 165 000), SD45 (280 000 vs. 360 000), E60CP (310 000 vs. 260 000), Eurostar (1 600 000 e manutenção 65 000 vs. 1 500 000 e 180 000), TGV-PSE (EXE 750 000/72 000, primeiro ano 1973) e TGV-Prototype (850 000, 1955-9999 no EXE contra 750 000, 1955-1983). O combustível do guia também não bate para Thalys, Mag-Lev, Brenner, DB 18 201, TGVs e Eurostar. A tabela 4-1 do manual (anexo B) erra muitas células (combustível "330" para os primeiros modelos, velocidades como Rocket 26 mph contra 15 no EXE). **Vale o EXE** (anexo A).
- **Peso**: o guia diz "50 t livres, +200 t dobra a chance"; no código a chance é linear em `e + 150`, com `e` = toneladas além de locomotiva + tênder + `+0x20`: 150 t extras dobram.
- **Rampa**: o guia diz "a cada 2% o peso quadruplica"; no código o peso é multiplicado por `(20g+1)^2.3`: na velocidade, com o ajuste `(4g+1)g`, x2,3 a 2% e x4,5 a 4%; na chance de quebra, sem o ajuste, x2,2 e x3,9.
- **Idade**: confere com o guia (33% aos 10, 66% aos 20); o "60% a mais que uma de 5 anos" do manual não. A locomotiva não quebra no primeiro ano e tem 60% da chance no segundo.
- **Acelerador**: confere (x4 no máximo, proporcional abaixo de 85).
- **Óleo, caboose, patch 1.53**: conferem (x3 vazio, x1,5 meio; -25%; -15% em 1920 e de novo em 1970, aqui x0.85 e x0.7; acidente 20 -> 17 -> 14 em 101).
- **Nowreck**: não elimina acidentes, divide a chance por 10 000.
- **Areia**: confere ("2% vira 4%", a rampa dobra quando a areia chega a 1).
- **Água**: "muito reduzida" = metade da velocidade alvo para vapor.
- **Envelhecimento**: confere (-0,5% de velocidade por ano, 10% aos 20).

## Em aberto
- Número máximo de vagões por trem (a UI de composição não foi lida; o teste usa até 8 veículos).
- Sinais, via dupla e bloqueio entre trens: `vehicle_advance_head` (`0x415040`) para o trem em `0x4d3c80(trem,1,1,7)` quando `0x414dd0` falha; a regra de ocupação não foi lida.
- Classes de estrutura 1 e 3 em `0x4d00c0` (lista `0x574c84`, campo `+0x11`, provavelmente pontes/estações) e o significado de `+0xbc`.
- Flags globais `0x574c08` (dobra a velocidade alvo), `0x570229` (sem quebras para IA, sem acidentes; carga 0x11) e `0x5748fc` (desliga quebras): origem não identificada.
- `0x567d6d[tipo]` (nível de preço de combustível por tipo) e quem o muda.
- Reabastecimento de areia, óleo e água nas estações; regra de `+0xa7`/`+0xa8` (som/estado do acelerador).
- Detalhes de `vehicle_advance_head`/`vehicle_advance` (troca de tile, meia distância de engate `+0x06`, tiles com comprimento pela metade).
- Campos `+0x32` e `+0x36` do registro de motor; nomes dos tipos de carga 44/45 (restaurante/caboose pelo nome do veículo).
- `0x43c030`: confirmar que `jogador+0x122` significa "humano".
- Precisão real do x87 no jogo (testado só em 53 bits).

## Anexo A. `engine_table` (`0x53cec0`), valores do `.data`
"Conf." = classe de confiabilidade; "Motor+tênder t" = `+0x2e` calculado como em `0x471b10`; "Comb./ano" = estimativa da janela de compra (6.3) com modificadores neutros. Anos do motor 3 (TGVx) antes do ajuste de `0x477a60`.

| # | Nome (lng) | +0 | Anos | A/E/W | Conf. | Preço | Manut./ano | mph | Potência | Acel. | Livre t | Comb. | Tipo | Motor+tênder t | Comb./ano (calc.) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | Stephenson Rocket | 1 | 1829-1851 | AEW | 4 | 16000 | 6000 | 15 | 4 | 50 | 10 | 321 | vapor | 60 | 7992 |
| 1 | 2-4-0 John Bull | 1 | 1831-1853 | AEW | 4 | 23000 | 7000 | 25 | 4 | 43 | 10 | 330 | vapor | 60 | 9256 |
| 2 | 0-4-0 Dewitt Clinton | 1 | 1833-1855 | AEW | 3 | 18000 | 5000 | 20 | 7 | 50 | 15 | 250 | vapor | 60 | 7187 |
| 3 | TGVx | 1 | 1973-9999 | -EW | 4 | 850000 | 80000 | 155 | 7 | 27 | 80 | 680 | elétrica | 30 | 71672 |
| 4 | 4-2-0 Prussian | 1 | 1837-1857 | AEW | 4 | 35000 | 8000 | 30 | 5 | 49 | 20 | 333 | vapor | 60 | 10622 |
| 5 | 3-Truck Shay | 1 | 1882-1930 | AEW | 7 | 43000 | 17000 | 15 | 50 | 41 | 50 | 634 | vapor | 60 | 18227 |
| 6 | 0-4-0 A3 | 0 | 1865-1910 | A-W | 7 | 67000 | 8000 | 30 | 13 | 46 | 45 | 448 | vapor | 60 | 16172 |
| 7 | 4-4-0 American-C | 1 | 1848-1885 | AEW | 6 | 46000 | 5000 | 42 | 6 | 40 | 20 | 330 | vapor | 60 | 12606 |
| 8 | USRA 0-6-0 | 1 | 1918-1942 | AE- | 7 | 90000 | 13000 | 40 | 15 | 42 | 110 | 674 | vapor | 60 | 29993 |
| 9 | USRA 0-8-0 | 1 | 1930-1948 | AEW | 6 | 98000 | 18000 | 45 | 23 | 37 | 50 | 697 | vapor | 60 | 32236 |
| 10 | 2-6-2 Prairie | 1 | 1912-1953 | AEW | 7 | 85000 | 11000 | 60 | 10 | 37 | 60 | 650 | vapor | 60 | 34157 |
| 11 | 4-6-0 Ten-Wheeler | 1 | 1892-1911 | AEW | 4 | 66000 | 11000 | 50 | 9 | 37 | 45 | 477 | vapor | 60 | 21727 |
| 12 | 2-8-0 Consolidation | 1 | 1877-1905 | AEW | 7 | 51000 | 8000 | 40 | 10 | 36 | 45 | 480 | vapor | 60 | 19512 |
| 13 | 4-4-2 Atlantic | 1 | 1902-1929 | AEW | 6 | 93000 | 18000 | 80 | 6 | 20 | 20 | 797 | vapor | 60 | 43556 |
| 14 | 4-8-0 Mastodon | 1 | 1890-1911 | AEW | 3 | 60000 | 13000 | 45 | 12 | 45 | 30 | 525 | vapor | 60 | 22995 |
| 15 | 4-6-2 Pacific | 1 | 1908-1945 | AEW | 5 | 119000 | 21000 | 95 | 8 | 28 | 20 | 970 | vapor | 60 | 62516 |
| 16 | 4-6-4 Hudson | 1 | 1937-1953 | AEW | 5 | 210000 | 25000 | 90 | 16 | 36 | 30 | 775 | vapor | 60 | 54017 |
| 17 | 2-6-0 Camelback | 1 | 1905-1932 | AEW | 8 | 75000 | 9000 | 30 | 30 | 42 | 30 | 400 | vapor | 60 | 15000 |
| 18 | 2-6-0 Mogul | 1 | 1895-1915 | AEW | 5 | 83000 | 12000 | 50 | 10 | 29 | 30 | 556 | vapor | 60 | 25131 |
| 19 | 2-8-2 Mikado | 1 | 1919-1950 | AEW | 7 | 133000 | 32000 | 55 | 33 | 28 | 30 | 985 | vapor | 60 | 51072 |
| 20 | 4-4-0 D16sb | 1 | 1914-1934 | AEW | 8 | 65000 | 9000 | 45 | 12 | 43 | 30 | 480 | vapor | 60 | 21024 |
| 21 | 4-4-4-4 T-1 | 1 | 1945-9999 | A-W | 7 | 284000 | 28000 | 85 | 16 | 20 | 40 | 1157 | vapor | 60 | 78213 |
| 22 | 4-6-4 J3A Streamliner | 1 | 1938-1955 | AEW | 3 | 255000 | 28000 | 103 | 12 | 25 | 72 | 1418 | vapor | 60 | 112234 |
| 23 | 4-8-4 Daylight | 1 | 1937-1952 | AEW | 4 | 230000 | 30000 | 80 | 22 | 33 | 30 | 966 | vapor | 60 | 63611 |
| 24 | 4-8-8-4 Big Boy | 1 | 1941-1955 | A-- | 4 | 375000 | 75000 | 68 | 49 | 24 | 120 | 1420 | vapor | 60 | 88040 |
| 25 | 4-6-0 Class B12 | 1 | 1923-1939 | AEW | 6 | 146000 | 14000 | 71 | 8 | 19 | 30 | 593 | vapor | 60 | 32407 |
| 26 | 2-10-0 Class 13 H | 1 | 1917-1940 | -EW | 7 | 102000 | 36000 | 40 | 30 | 28 | 30 | 1074 | vapor | 60 | 46289 |
| 27 | 0-10-0 Class G10 | 1 | 1910-1939 | -EW | 5 | 98000 | 38000 | 50 | 10 | 27 | 130 | 981 | vapor | 60 | 50521 |
| 28 | Class A4 Mallard | 1 | 1935-1955 | -EW | 4 | 200000 | 19000 | 126 | 6 | 26 | 65 | 656 | vapor | 60 | 55136 |
| 29 | Trevithick-1 | 1 | 1800-1835 | -E- | 1 | 10000 | 6000 | 10 | 4 | 50 | 30 | 321 | vapor | 60 | 7543 |
| 30 | Dash-9 | 1 | 1993-9999 | AEW | 7 | 478000 | 18000 | 70 | 40 | 28 | 30 | 1116 | diesel | 30 | 68410 |
| 31 | F3A+B | 1 | 1945-1968 | AEW | 6 | 265000 | 16000 | 85 | 17 | 22 | 100 | 728 | diesel | 60 | 52015 |
| 32 | F9 | 1 | 1949-1970 | AEW | 6 | 337000 | 18000 | 110 | 18 | 28 | 25 | 788 | diesel | 30 | 63749 |
| 33 | GP18 | 1 | 1958-1975 | AEW | 8 | 245000 | 15000 | 83 | 33 | 26 | 35 | 799 | diesel | 30 | 55131 |
| 34 | GP9 | 1 | 1954-1980 | AEW | 7 | 235000 | 11000 | 71 | 25 | 29 | 35 | 788 | diesel | 30 | 48028 |
| 35 | SDP40 | 1 | 1973-1989 | AEW | 4 | 292000 | 18000 | 103 | 19 | 25 | 30 | 797 | diesel | 30 | 61966 |
| 36 | SD45 | 1 | 1972-1994 | AEW | 5 | 280000 | 21000 | 65 | 30 | 29 | 30 | 623 | diesel | 30 | 36009 |
| 37 | Alco PA-1 | 1 | 1946-1990 | AEW | 5 | 210000 | 16000 | 90 | 18 | 29 | 30 | 750 | diesel | 30 | 52800 |
| 38 | Class 232 | 1 | 1997-9999 | AEW | 8 | 492000 | 35000 | 75 | 34 | 18 | 30 | 1133 | diesel | 30 | 72625 |
| 39 | AMD-103 | 1 | 1993-9999 | A-- | 6 | 425000 | 25000 | 105 | 14 | 15 | 30 | 1143 | diesel | 30 | 88068 |
| 40 | FP45 | 1 | 1968-9999 | AEW | 6 | 366000 | 14000 | 106 | 8 | 26 | 60 | 865 | diesel | 30 | 65740 |
| 41 | V200 | 1 | 1959-1985 | -EW | 4 | 160000 | 19000 | 87 | 22 | 24 | 30 | 773 | diesel | 30 | 53878 |
| 42 | Class 1020 | 1 | 1941-1981 | -EW | 8 | 119000 | 18000 | 56 | 29 | 22 | 30 | 410 | elétrica | 30 | 21545 |
| 43 | Class 1045 | 1 | 1927-1950 | AEW | 6 | 95000 | 6000 | 40 | 32 | 22 | 30 | 270 | elétrica | 30 | 11636 |
| 44 | E656 FS | 1 | 1975-1995 | -EW | 6 | 226000 | 17000 | 93 | 14 | 30 | 30 | 536 | elétrica | 60 | 37734 |
| 45 | E 69 | 1 | 1955-1970 | -EW | 4 | 86000 | 8000 | 31 | 65 | 19 | 30 | 330 | elétrica | 30 | 12837 |
| 46 | Ee 3/3 | 1 | 1923-1948 | AEW | 7 | 47000 | 7000 | 31 | 40 | 46 | 30 | 311 | elétrica | 30 | 11880 |
| 47 | Ae 8/14 | 1 | 1939-1955 | -EW | 6 | 210000 | 24000 | 68 | 50 | 22 | 30 | 786 | elétrica | 60 | 47906 |
| 48 | Be 4/6 II | 1 | 1920-1950 | -EW | 8 | 61000 | 11000 | 35 | 18 | 21 | 60 | 368 | elétrica | 30 | 14701 |
| 49 | Class E111 | 1 | 1974-9999 | AEW | 7 | 390000 | 17000 | 85 | 38 | 30 | 30 | 540 | elétrica | 30 | 38016 |
| 50 | Class E18 | 1 | 1936-1966 | -EW | 7 | 97000 | 16000 | 93 | 8 | 21 | 30 | 422 | elétrica | 30 | 27493 |
| 51 | Penn. E44 | 1 | 1960-1985 | AEW | 5 | 370000 | 22000 | 70 | 28 | 23 | 30 | 623 | elétrica | 30 | 37971 |
| 52 | E60CP | 1 | 1973-1998 | A-- | 5 | 310000 | 19000 | 85 | 21 | 18 | 30 | 557 | elétrica | 30 | 38043 |
| 53 | GG1 | 1 | 1935-1970 | A-W | 9 | 285000 | 19000 | 100 | 20 | 27 | 30 | 557 | elétrica | 30 | 42721 |
| 54 | Shinkansen Bullet | 1 | 1966-2002 | -EW | 6 | 650000 | 66000 | 130 | 8 | 5 | 30 | 800 | elétrica | 30 | 66680 |
| 55 | Thalys Bullet | 1 | 1994-9999 | AEW | 5 | 1000000 | 78000 | 186 | 8 | 7 | 70 | 1010 | elétrica | 30 | 122361 |
| 56 | 2-4-0 Vulcan | 1 | 1872-1900 | AEW | 4 | 32000 | 4000 | 30 | 7 | 44 | 30 | 190 | vapor | 30 | 6393 |
| 57 | 4-4-0 8-Wheeler | 1 | 1868-1903 | AEW | 6 | 59000 | 5000 | 48 | 7 | 38 | 35 | 420 | vapor | 60 | 17808 |
| 58 | 1-3 BoBo | 1 | 1895-1935 | AEW | 3 | 85000 | 6000 | 55 | 12 | 40 | 30 | 680 | elétrica | 30 | 33116 |
| 59 | Class 55 Deltic | 1 | 1961-1981 | -EW | 8 | 480000 | 15000 | 100 | 19 | 35 | 30 | 680 | diesel | 30 | 52155 |
| 60 | Mag-Lev TBX-1 | 1 | 2008-9999 | AEW | 6 | 2500000 | 200000 | 280 | 7 | 3 | 70 | 1800 | elétrica | 30 | 305639 |
| 61 | 4-2-2 Iron Duke | 1 | 1855-1895 | AEW | 4 | 78000 | 9000 | 54 | 7 | 41 | 30 | 680 | vapor | 60 | 30736 |
| 62 | Eurostar | 1 | 1994-9999 | -EW | 8 | 1600000 | 65000 | 175 | 12 | 9 | 30 | 850 | elétrica | 30 | 96135 |
| 63 | DB 18 201 | 1 | 1998-9999 | AEW | 6 | 400000 | 75000 | 120 | 18 | 15 | 70 | 1600 | vapor | 60 | 144560 |
| 64 | Brenner | 1 | 1999-9999 | AEW | 7 | 1250000 | 170000 | 140 | 30 | 28 | 30 | 680 | elétrica | 30 | 69292 |
| 65 | TGV | 1 | 1973-9999 | -EW | 5 | 750000 | 72000 | 155 | 7 | 19 | 30 | 880 | elétrica | 30 | 81664 |

## Anexo B. Vagões (`vehicle_type_table`, espécie 2)
Locomotivas (espécie 0) e tênderes (espécie 1) ocupam os demais registros; o peso deles entra em `+0x2e` do motor. Nomes curtos de carga só existem para os índices 0..32 (`.lng` 776..808); 33..43 (vagões `fmet*`) ficam como `(?)`.

| Tipo | Nome | Carga (idx, lng 776+idx) | Vazio t | Cheio t | Anos |
|---|---|---|---|---|---|
| 88 | fdin1 | 44 (restaurante) | 10 | 15 | 1846-1892 |
| 89 | fdin2 | 44 (restaurante) | 20 | 30 | 1893-1920 |
| 90 | fdin3 | 44 (restaurante) | 25 | 40 | 1921-9999 |
| 91 | fdin3 | 44 (restaurante) | 25 | 40 | 9999-9999 |
| 92 | falum | 20 alm | 15 | 45 | 1800-9999 |
| 93 | FAUTO | 8 aut | 30 | 45 | 1800-9999 |
| 94 | FBAUX | 29 bax | 25 | 65 | 1800-9999 |
| 95 | FCATT | 5 ctl | 15 | 30 | 1800-9999 |
| 96 | FCEMT | 9 cmt | 15 | 30 | 1800-9999 |
| 97 | fchem | 15 chm | 25 | 40 | 1800-9999 |
| 98 | fcoal | 18 coa | 25 | 65 | 1909-9999 |
| 99 | fcoff | 21 cof | 20 | 50 | 1911-9999 |
| 100 | fcott | 11 cot | 20 | 55 | 1902-9999 |
| 101 | fdies | 30 dsl | 20 | 45 | 1912-9999 |
| 102 | ffert | 16 frt | 20 | 35 | 1800-9999 |
| 103 | FFOD1 | 10 fod | 10 | 25 | 1800-1900 |
| 104 | ffod3 | 10 fod | 20 | 45 | 1901-9999 |
| 105 | fgud1 | 7 gds | 10 | 25 | 1800-1940 |
| 106 | fgud3 | 7 gds | 15 | 50 | 1941-9999 |
| 107 | fgrai | 19 grn | 20 | 40 | 1890-9999 |
| 108 | FGRAV | 26 gvl | 25 | 65 | 1907-9999 |
| 109 | fhazd | 17 wst | 20 | 45 | 1800-9999 |
| 110 | FORE_ | 27 irn | 15 | 45 | 1896-9999 |
| 111 | flogs | 3 log | 10 | 45 | 1893-9999 |
| 112 | flumb | 2 lmb | 15 | 40 | 1895-9999 |
| 113 | FMAL1 | 1 mal | 10 | 15 | 1800-1897 |
| 114 | FMAL2 | 1 mal | 20 | 30 | 1898-1925 |
| 115 | fmal3 | 1 mal | 25 | 40 | 1926-9999 |
| 116 | FMLK1 | 4 mlk | 15 | 30 | 1800-1909 |
| 117 | fmlk3 | 4 mlk | 20 | 45 | 1910-9999 |
| 118 | FOIL1 | 14 oil | 15 | 30 | 1800-1900 |
| 119 | foil3 | 14 oil | 20 | 45 | 1901-9999 |
| 120 | fpapr | 13 ppr | 20 | 50 | 1917-9999 |
| 121 | FPAS0 | 0 pas | 5 | 10 | 1800-1845 |
| 122 | FPAS1 | 0 pas | 10 | 15 | 1846-1897 |
| 123 | FPAS2 | 0 pas | 20 | 30 | 1898-1930 |
| 124 | fpas3 | 0 pas | 25 | 40 | 1931-1960 |
| 125 | fpas4 | 0 pas | 25 | 40 | 1961-9999 |
| 126 | FPROD | 28 prd | 15 | 40 | 1902-9999 |
| 127 | FPULP | 25 plp | 20 | 55 | 1908-9999 |
| 128 | FRUBB | 31 rbr | 15 | 35 | 1800-9999 |
| 129 | FSTEL | 6 stl | 10 | 45 | 1898-9999 |
| 130 | FSUGR | 23 sgr | 25 | 55 | 1916-9999 |
| 131 | ftire | 22 tir | 25 | 50 | 1800-9999 |
| 132 | FURAN | 24 urn | 15 | 30 | 1800-9999 |
| 133 | fwool | 12 wol | 35 | 55 | 1909-9999 |
| 138 | fcab1 | 45 (caboose) | 20 | 20 | 1800-9999 |
| 139 | fCOA2 | 18 coa | 15 | 35 | 1800-1908 |
| 140 | fCOF2 | 21 cof | 15 | 35 | 1800-1912 |
| 141 | fCOT2 | 11 cot | 15 | 35 | 1800-1903 |
| 142 | fDIE2 | 30 dsl | 15 | 30 | 1800-1911 |
| 143 | fGRN2 | 19 grn | 10 | 25 | 1800-1889 |
| 144 | fGRV2 | 26 gvl | 15 | 35 | 1800-1906 |
| 145 | fLOG2 | 3 log | 5 | 25 | 1800-1892 |
| 146 | fLUM2 | 2 lmb | 10 | 25 | 1800-1894 |
| 147 | fORE2 | 27 irn | 10 | 25 | 1800-1895 |
| 148 | fPAP2 | 13 ppr | 10 | 25 | 1800-1916 |
| 149 | fPRO2 | 28 prd | 10 | 25 | 1800-1901 |
| 150 | fPUL2 | 25 plp | 10 | 25 | 1800-1907 |
| 151 | fSTE2 | 6 stl | 10 | 25 | 1800-1897 |
| 152 | fSUG2 | 23 sgr | 15 | 30 | 1800-1915 |
| 153 | fWOO2 | 12 wol | 15 | 35 | 1800-1908 |
| 158 | ftroo | 32 dng | 25 | 40 | 1800-9999 |
| 159 | fmun_ | 33 (?) | 15 | 50 | 1800-9999 |
| 160 | fweap | 34 (?) | 15 | 35 | 1800-9999 |
| 161 | falco | 35 (?) | 15 | 50 | 1800-9999 |
| 162 | fmety | 38 (?) | 20 | 35 | 1800-9999 |
| 163 | fmetg | 36 (?) | 20 | 35 | 1800-9999 |
| 164 | fmetr | 37 (?) | 20 | 35 | 1800-9999 |
| 165 | fmetb | 40 (?) | 20 | 35 | 1800-9999 |
| 166 | fmetg | 41 (?) | 20 | 35 | 1800-9999 |
| 167 | fwool | 43 (?) | 35 | 55 | 1800-9999 |
| 172 | fmetb | 39 (?) | 20 | 35 | 1800-9999 |
| 173 | fmety | 42 (?) | 20 | 35 | 1800-9999 |
