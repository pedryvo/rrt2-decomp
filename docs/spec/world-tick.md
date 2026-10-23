# Ordem de atualização do mundo (world tick)

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido no C exportado (`re/db/decomp`) e no disassembly; confirmado com o original rodando no Wine (`advance_save.py` + breakpoints de contagem, seção 7). Endereços são VAs. Estado das provas: **[D]** = ordem observada no original (traço dinâmico); **[S]** = lida no código. O manual não descreve a ordem de atualização. Base de calendário, velocidade e laço principal: `time.md`. RNG: `rng.md`.

Objetivo: dar ao `rt2-sim` a sequência exata de chamadas da simulação para que uma partida (semente + save) siga o mesmo caminho do original (níveis 2/3 de fidelidade, `docs/PLAN.md` seção 11). **A ordem de chamada do RNG do jogo é estado**: qualquer troca de ordem entre entidades desalinha a sequência para sempre.

## 1. Iteração das listas de entidades

Todas as listas da simulação são pools com lista ligada (`docs/formats/map-gam.md` 5.3). Cabeçalho do pool em memória: `+0x08` tamanho do registro, `+0x14` contagem, `+0x24` u16 head, `+0x26` u16 tail, `+0x28` base dos registros, `+0x2c` `u16 next[]`, `+0x30` `u16 prev[]`, `+0x34` bitmap de livres.

| Função | Endereço | Uso |
|---|---|---|
| `list_record(list, id)` | `0x4f18b0` | `base + id * tam`; id 0 -> null |
| `pool_nth_id_list_order(list, n)` | `0x4f18d0` | id do n-ésimo na ordem da lista (anda por `next` a partir do head, ou por `prev` a partir do tail se `n > contagem/2`) |
| `pool_nth_list_order(list, n)` | `0x4f19e0` | o mesmo, devolvendo o registro |
| `pool_next_id(list, id)` | `0x4f1960` | `next[id]` |
| `<pool>_nth(n)` | `0x466910..0x4669e0` | wrappers por pool de `0x4f19e0` (`town_nth` `0x466910`, `territory_nth` `0x466920`, `train_nth` `0x466980`, `rule_nth` `0x466990`, `industry_nth` `0x4669a0`, `station_nth` `0x4669b0`, `person_nth` `0x4669c0`, `company_nth` `0x4669d0`) |

Três padrões aparecem nos passos da simulação, **todos na ordem da lista ligada** (não na ordem dos slots), exceto onde dito:

- **A (`next` antes da chamada):** `id = nth_id(list, 0); enquanto id: prox = next(id); f(id); id = prox`. Seguro contra a remoção do item corrente. Usado no caminho linear de `sim_advance` (animações e trens).
- **B (`next` depois da chamada):** `e = nth(list, 0)` e depois `e = record(next(e.+0 u16 slot próprio))`, `n` vezes com `n = contagem` relida a cada volta. Usado em `sim_week_end`.
- **C (`nth(i)` a cada volta):** `para i em 0..contagem-1: f(nth(list, i))`. Quadrático, mesma ordem. Usado em `sim_month_end` e `sim_year_end`.
- **Exceção, ordem de slot decrescente:** a expiração de lotes (`list_lots` `0x574c7c`, mensal) e a rotação do desgaste das peças de trilho (`track_piece_pool` `0x574c84`, anual) percorrem os slots de `+0x10 - 1` até 0 pelo bitmap.

A ordem da lista ligada é a ordem de inserção (alocação liga no fim, `0x4f1690`) com as remoções; ela é gravada no `.GAM` (arrays `next`/`prev`), então o clone deve **ler e manter** as listas `next/prev` do save, não iterar por slot.

## 2. `sim_advance(dt)` (`0x4a5cb0`) [S, D]

Pseudocódigo completo (jogo local, 1 jogador; os ramos de rede estão marcados):

```
sim_advance(dt):                                         ; fastcall ECX = minutos
  se sim_busy (0x57922c): retorna
  (rede) dt = min(dt, horizonte - T)
  ; 1. comandos agendados (cmd_queue 0x574c8c); em jogo local cmd_queue_next_time == T,
  ;    então todos os comandos pendentes rodam aqui, antes de qualquer avanço
  repete: p = cmd_queue_next_time() (0x4af2d0); p == 0 -> sai
          p > T: (rede) se dt <= p-T sai; dt -= p-T; sim_advance(p-T)
          senão: cmd_queue_run() (0x4af3d0)
  se dt == 0: retorna
  novo = data(T + dt)
  se novo.data > data:                                   ; cruza 23:59 -> 00:00
      r = 1439 - (hora*60 + min)                         ; minutos até 23:59
      sim_advance(r)                                     ; caminho linear até 23:59 (recursivo)
      fim_de_dia()                                       ; 23:59
      dt -= r + 1
      enquanto data != novo.data:                        ; dias inteiros
          sim_advance(1439); fim_de_dia(); dt -= 1440
  ; 2. caminho linear com o dt que sobrou (pode ser 0, ver 2.2)
  data/hora = data(T + dt);  T += dt
  para cada id em pool_78 (0x574c78), padrão A: anim_object_step(id)       ; 0x4283e0
  se jogadores == 1:
      nop (0x4cdd30)
      para cada trem em list_trains (0x574c64), padrão A: train_step(trem, dt, 1)   ; 0x4d0ce0
      nop (0x4cdd30)

fim_de_dia():                                            ; inline em 0x4a5e6a.. e 0x4a6010..
  net_time_sync() (0x49e0b0)                             ; local: sem efeito
  (rede) rng_seed(ftol(f64(T + game_seed) + pessoa0.+0x12a))
  se dia % 7 == 0:
      sim_week_end()                                     ; 0x4a5270
      se dia == 28: sim_month_end(); (ele avança a data e T)   ; 0x4a5320
      senão: data = data(T+1); T += 1
  senão: data = data(T+1); T += 1
  (rede) 0x46b150; trains_advance(1440) (0x4a58f0); 0x46b290
```

### 2.1 O que roda em cada passo linear
1. **Animações do mapa** (`pool_78`, registros de 0x23 bytes, `anim_object_step` `0x4283e0`) [S]: só registros com estado `+0 in {1,2}`. `t = max(0, (T - +0x0f) * (1/2160))` (`0x5352f8` = 4,6296e-4); `quadro = ftol(t * +0x1f)`; se `quadro >= nº de quadros` da sequência `+2` (tabela `0x567a1c`), o objeto é apagado (`0x466d20`: limpa a marca no tile `(+3,+5)` e libera o slot); senão `+1 = quadro`, `+7 = ftol(+0x0b + t*+0x13)`, `+8 = ftol(+0x0c + t*+0x17)`, `+9 (u16) = ftol(+0x0d + t*+0x1b)` (interpolação linear de posição). Determinístico, sem RNG. Exemplo: 3291 chamadas em 39 passos no save Example (cerca de 84 objetos).
2. **Trens** (jogo local): `train_step(trem, dt, 1)` para cada trem, na ordem da lista (`trains.md` 4.5). Carga e descarga **não** são um passo à parte: acontecem dentro do `train_step`, quando o trem chega à estação (`0x4d2c10`, `cargo.md` 6; receita `0x4bf760`). Quebras e acidentes também (`train_breakdown_check` `0x4cf390`, `trains.md` 5.1).
3. **Não há** produção de carga, demanda de estação nem decisões de IA no passo linear: produção e demanda são semanais (seção 3.2), IA é mensal (3.3).

### 2.2 Consequências para o clone
- **Um minuto por dia sem movimento:** o `+1` minuto de 23:59 para 00:00 é aplicado fora do caminho linear; os trens não andam nesse minuto. Em 7 dias, o relógio anda 10 080 min e a soma dos `dt` passados a `train_step` é 10 073.
- **Número de passos lineares:** cada chamada de `sim_advance(dt)` com `dt < 1440` faz 1 passo linear, ou 2 se cruzar a meia-noite (um até 23:59 e o resto). Com o passo fixo do oráculo (320 min) e começando em 19:03: 7 dias = 32 chamadas externas + 7 recursivas = **39 passos** [D] (`train_step` = 39 x 9 trens = 351 no save Example [D]).
- **Passo linear com `dt = 0`:** se `dt` termina exatamente às 00:00 (`dt == r + 1`), o caminho linear roda com `dt = 0` (não há novo teste de zero): animações e `train_step(trem, 0, 1)` são chamados. O efeito em `train_step` (divisão por `dt`) não foi verificado (Em aberto).
- **Comandos do jogador** rodam no início de cada `sim_advance` (e de cada recursão), antes do avanço; em jogo local todos de uma vez.

## 3. Ganchos periódicos

### 3.1 Por dia (23:59)
Jogo local: nada além da troca de data (o RNG **não** é re-semeado). Em rede: `net_time_sync`, re-semeadura do RNG e `trains_advance(1440)` (`time.md` 7.2).

### 3.2 Semana: `sim_week_end` (`0x4a5270`), dias 7, 14, 21, 28 às 23:59 [S, D]
1. Para cada registro de `list_industries` (`0x574c74`, padrão B) com tipo `+7 != 0`: `industry_recount_piles(e, 0)` (`0x40a610`) e `industry_production_tick(e, 0)` (`0x40aa20`) — produção periódica, `cargo.md` 7.3.
2. Para cada estação (`list_stations` `0x574c60`, padrão B): `station_recompute_supply_demand(e)` (`0x4bebf0`, `cargo.md` 8).
Sem RNG.

### 3.3 Mês: `sim_month_end` (`0x4a5320`), dia 28 às 23:59, depois do passo semanal [S, D]
Em ordem (mês e ano ainda os do mês que termina):

| # | Chamada | O quê | Spec |
|---|---|---|---|
| 1 | se `0x5794f0`: zera e `station_networks_rebuild` `0x46a370` | reagrupa estações em redes conectadas (`0x460370`), recalcula contagens por rede e dispara gatilho 4 | **não especificada** |
| 2 | `event_fire_trigger(4)` se `net_min_peer_version() > 0x232c` (local: sempre) | regras de cenário de verificação periódica | `time.md` 7.4; regras: **não especificadas** |
| 3 | `event_fire_trigger(3)` | regras "fim de mês" | idem |
| 4 | se mês in {2, 8}: `economy_state_step` `0x46a010` | passo semestral do estado econômico; RNG `[-5,5]` e `[0,10]` (2 chamadas) [D] | `cargo.md` 9 |
| 5 | se mês <= 10: `rng_range(0, 10000)` (`0x4a53bf`), e se `< ((50 - |ano-1860|) * (54300/L)) / 70`: `town_bonus_offer` `0x431660` | oferta de bônus de cidade (seção 3.6) | aqui |
| 6 | se `0x570227 == 0`: expiração de lotes (slots decrescentes) | `cargo.md` 7.6 | `cargo.md` |
| 7 | `ai_monthly_turn` `0x436d60` | turno da IA (seção 3.5) | **não especificada** |
| 8 | para cada empresa (C): `company_month_step` `0x4399d0` | juros, dividendos trimestrais, salários, cotação | `finance.md` 5 |
| 9 | para cada pessoa (C): `person_month_step` `0x4a7590` | `caixa += ftol((prime(0) - 0.04) * caixa / 12)` (`0x535530` = 0,04, `0x535540` = 1/12; `prime` = `0x46bac0`) | aqui; prime em `finance.md` 4.1 |
| 10 | para cada trem (C): `train_monthly_costs` `0x4d4390` | manutenção, combustível | `trains.md` 6.2 |
| 11 | para cada estação (C): 44 contadores `+0x271` `*= 0.94`; `station_recompute_supply_demand` | decaimento de entregas | `cargo.md` 8 |
| 12 | `margin_check_all` `0x46bb10` | chamadas de margem dos jogadores (repete até estabilizar) | `finance.md` 7.3 |
| 13 | relatório: `report_due_flag = 1` conforme opção (`time.md` 7.4 item 7) | | `time.md` |
| 14 | mês 12: `sim_year_end()` e depois **`event_fire_trigger(2)`**; senão data + 1 min, `T += 1`, `event_fire_trigger(2)` | | |

Correção a `time.md` 7.4 item 8: em dezembro o gatilho 2 (início de mês) **também** dispara, depois de `sim_year_end` (`0x4a560c..0x4a5611`), já em 1/jan 00:00 [D].

### 3.4 Ano: `sim_year_end` (`0x4a4d00`), 28/dez 23:59, dentro do passo mensal [S, D]

| # | Chamada | O quê | Spec |
|---|---|---|---|
| 1 | para i < `company_set_count` (`0x46a7e0` = `[0x570729]`): `company_metric(company_set_at(i), 11)` (`0x43ad60`; `company_set_at` `0x46a7f0(i)` = `list_record(empresas, u16 0x57072d[i])`) | média `m` da métrica 11 (`+0x19a/+0x196`), soma com `ftol` acumulado | `finance.md` 3 |
| 2 | `e = ai_pick_engine(1)` (`0x4d86f0`, **RNG** `[80,120]` por motor candidato) | `0x573e08 = clamp(ftol((engine[e].vel_max * 0.5 + m) * 0.5), 4, 200)` (`engine_table+0x14`; `0x535128` = 0,5) | aqui |
| 3 | para cada indústria (C): `industry_year_step` `0x40b450` | razão entregas/produção (`+0x63`/`+0x67`), lucratividade por tipo; sem RNG | **não especificada** |
| 4 | para cada empresa (C): `company_annual_review` `0x43c760` | avaliação da diretoria, bônus, demissão, liquidação; **RNG** (seção 4) | `finance.md` 8.1 (resumo) |
| 5 | para cada pessoa (C): `person_year_nop` `0x4a7870` | vazio (`ret`) | aqui |
| 6 | `event_fire_trigger(1)` | regras "fim de ano" | |
| 7 | `history_snapshot(&0x570a82)` `0x4e9770` | refaz do zero o registro anual do jogador e as classificações de empresas e pessoas (3.4.1) | 3.4.1 [V] |
| 8 | `0x5705d1 = f32(pow(1 + f32[0x57022b], ano - [0x570508] + 1) * i32[0x570223] * 0.01)` | meta composta anual (papel incerto; `0x570508` = ano inicial) | Em aberto |
| 9 | para cada empresa (C): `company_year_end` `0x43d800` | arquivo anual, overhead, títulos, desdobramento | `finance.md` 8 |
| 10 | para cada pessoa (C): `person_year_step` `0x4a7880` | desloca o arquivo de patrimônio (`+0x133` -> `+0x143`, 0x90 B) e grava caixa e valor das ações (`0x43b410`) | aqui |
| 11 | para cada trem (C): `train_year_step` `0x4d4620` | desloca estatísticas anuais (`+0xfd..+0x11d`) e zera o ano | aqui |
| 12 | para cada território (C): `territory_year_step` `0x4c8520` | `+0x52 (u16) = 0` | aqui |
| 13 | se `0x570237 == 0`: `town_year_step(0)` `0x430a40` | crescimento de cidades e construções; **RNG pesado** (seção 4) | **não especificada** |
| 14 | `sim_week_end()` | segundo passo semanal no mesmo minuto | 3.2 |
| 15 | para cada indústria (C): `industry_year_reset` `0x40b7c0` | se ativa: zera `+0x34`, `+0x63`, `+0x67`, recontagem de pilhas, `0x4beae0` na estação ligada (`+8`) | `0x4beae0(estação)` zera 8 bytes `+0x59..+0x60` (as receitas do ano como origem `+0x59` e destino `+0x5d`, `cargo.md` 7) e os três `u16` `+0x4b5`, `+0x4b7`, `+0x4b9` (`finance.md` 13.4) [V no Example: `stations` em 30 dias iguais]; o resto **não especificada** |
| 16 | peças de trilho (slots decrescentes): `+0x1f = +0x1b; +0x1b = 0` | desgaste do ano anterior | `terrain-track.md` |
| 17 | data -> 1/jan 00:00, `T += 1`; zera `0x573d05/09/0d`; se `0x573e02 < 0x573d9e`: `0x573e02 = ftol(...)` | | `time.md` 7.5 |
| 18 | `industry_rebuild_inputs(i)` `0x471880`, i = 0..52 | tipos de indústria | `cargo.md` |
| 19 | mensagens históricas (1877, 1879, 1900, 1910, 1960) | LNG 2630, 2851, 419, 417, 420 | `time.md` 7.5 |
| 20 | `event_fire_trigger(0)` | regras "início de ano" | |
| — | (de volta em `sim_month_end`) `event_fire_trigger(2)` | | |

### 3.4.1 `history_snapshot` (`0x4e9770`) [V no Example, 30 dias]

Roda no item 7 de `sim_year_end`, **depois** das revisões anuais e **antes** de `company_year_end`, `person_year_step` e do deslocamento dos arquivos: lê as categorias do ano corrente (`+0x4286`), os arquivos das empresas (`+0x42ee`, `+0x49b6`) e das pessoas (`+0x133`) ainda não deslocados. Não tem RNG. Grava no estado global, em `0x570a82` (offset de estado `0x59a`, 0x3283 bytes até `0x57381c`, logo antes dos contadores anuais `0x573d05`), um registro **inteiro novo** (zera `0xca0` dwords + 3 bytes, depois preenche). Consome: a janela de histórico/briefing (`0x444e10`, não analisada).

Campos (offsets no registro; `co` = empresa local, `p` = pessoa local; f64 não alinhados):

| Off | Tipo | Conteúdo |
|---|---|---|
| `+0x00` | u32 | 1 (registro válido) |
| `+0x04` | u32 | ano (`game_date & 0xffff`) |
| `+0x08` | u32 | `min(9, ano - [0x570508] + 1)`: anos de histórico |
| `+0x0c` | u32 | `[0x574414]`, id da empresa local (0 = nenhuma) |
| `+0x10` / `+0x18` | f64 | `co` total do ano das categorias 0..4 (`0x43a660(14)`, receita) / das 13 categorias (`0x43a660(13)`, resultado) |
| `+0x20` | u32 | `0x43c730(co)`: degrau 0..12 = `clamp((+0x46a + 0x3a) / 9, 0, 12)` (divisão inteira com sinal) |
| `+0x24` | f64 | `(double) co+0x134` (salário do chairman, métrica 13) |
| `+0x2c` | f64 | `(double) co+0x474` (último bônus pago) |
| `+0x34` | u32 | 4 (constante) |
| `+0x38` | u8 | `[0x570717]` (estado econômico) |
| `+0x39` | f64 | métrica 9 = `(double) co+0x196` |
| `+0x41` | f64 | `ftol(0x43a660(1) + 0x43a660(0)) / max(métrica 9, 1,0)` (categorias 0 e 1 do ano; `0x5350c8` = 1,0) |
| `+0x49` | i16 | métrica 11 = `co+0x19a / max(co+0x196, 1)` (inteiro) |
| `+0x4b` | f64 | se `co+0x148 == ano`: `co+0x134 - 10000`; senão `co+0x134 - arquivo[1][13]` (`0x43a160(co, ano-1, 13)` = `co+0x49b6 + (13 + 14)*8`, a métrica 13 de um ano atrás) |
| `+0x53`, `+0x54` | u8 | `0xff`; em ano **ímpar**, `co+0x140` e `co+0x144` (bytes baixos dos dois candidatos a gerente) |
| `+0x55` | u8 | `0x4a80c0(p, co)`: 1 se a pessoa local tem **todas** as ações (`co+0x2c == p+0x1d3[índice da empresa no conjunto]`) |
| `+0x56` | char[~4 KB] | texto das regras de gatilho 7 (`rule_collect_status` `0x458210`; vazio sem regras 7) |
| `+0x105b` | u32 | número de empresas do conjunto (`0x46a7e0`) |
| `+0x105f` | u32[2][0x24] | ids das empresas ordenadas: fileira 0 por `ftol(0x43a660(14))` decrescente, fileira 1 por `ftol(0x43a660(13))` |
| `+0x117f` | f64[2][0x24][10] | para cada posição e cada um dos últimos 10 anos (`ano - j`): se `ano - j < co+0x148` o marcador `12343,0` (`0x40c81b8000000000`); senão o total da categoria 14 (fileira 0) ou 13 (fileira 1) daquele ano (`0x43a7d0`: ano corrente = vivo, anterior = `co+0x42ee + (ano - y - 1)*0x68`) |
| `+0x27ff` | u32 | número de pessoas do conjunto de jogadores (`0x46a890` = `[0x570775]`) |
| `+0x2803` | u32[32] | ids das pessoas por patrimônio líquido decrescente (`ftol(0x4a7620)` = caixa + ações * cotação) |
| `+0x2883` | f64[n][10] | patrimônio (caixa + valor das ações, `0x4a7760(p, ano - j, 2)`) por pessoa e ano: o ano corrente é vivo; os anteriores vêm do arquivo `p+0x123 + (ano - y)*16` (caixa, ações); `j = 9` lê `+0x1b3`, ainda dentro do arquivo de 9 entradas |

Ordenação (as duas): seleção repetida do maior entre os ainda não usados, na ordem do conjunto; o primeiro candidato é aceito sempre, os seguintes só se **estritamente** maiores (empate fica com o primeiro do conjunto). As fileiras de empresas usam o espaçamento `0x24` mesmo com poucas empresas; o resto fica zero.

Pseudocódigo:
```
rec = zeros(0x3283)
rec.u32[0]=1; rec.year=ano; rec.n=min(9, ano-inicio+1); rec.company=[0x574414]
rec.b53=rec.b54=0xff
se [0x574414] != 0:                       ; campos de co, tabela acima
rec.b38 = estado_econômico; rec.u32[0x34]=4
para k em 0..2: para rank em 0..n: ordena (14 ou 13); grava id; para j em 0..10: histórico
pessoas: idem, chave ftol(patrimônio); histórico por 0x4a7760
rule_collect_status(rec+0x56)              ; só texto, sem efeitos
```
O clone: `rt2_sim::tick::history` (`crates/rt2-sim/src/tick/history.rs`), ligado ao item 7. Verificado: o Example em 30 dias (a virada de 1875 para 1876) deixa de ter diferenças em `state` (de 1099 campos para 0).

Suposições fora do save: `[0x574414]` e `[0x566ca4]` (empresa e pessoa da interface) não estão no arquivo; o clone usa a primeira pessoa humana (`+0x122`) e a empresa em que ela é chairman (`+0x263`), que dá `4` no Example (igual ao original). Sem pessoa humana o registro fica sem a parte da empresa. Texto de regras 7: o original copia a string vazia `0x5667f0` e roda as regras do gatilho 7 em modo de coleta (`0x455f30` com o buffer `[0x5665e0]`: cada texto é anexado, precedido do LNG 2 `"\n\n"` quando o buffer já tem mais de 2 caracteres, antes do teste de fim de jogo; a varredura para se o estado de fim de jogo muda); o clone faz o mesmo com `event_fire_trigger(7)` e o buffer `SimState::status_buf` (Lenin.MP2 e #E01, `docs/fidelity-status.md`, "Longo prazo").

### 3.5 Turno mensal da IA (`ai_monthly_turn` `0x436d60`) [S, D]
Se `0x5748fc == 0` e `0x570229 == 0`:
```
build_preview_suspend()  (0x46b150)
para cada pessoa (C) com +0x122 == 0 (IA):        ai_person_turn(pessoa.+0)   ; 0x4a8800
para cada empresa (C) cujo presidente +0x23 é 0 ou é IA:  ai_company_turn(empresa.+0)  ; 0x436b30
build_preview_resume()    (0x46b290)
```
Os `clock_ms` só medem tempo (perfil). `build_preview_suspend`/`resume` só tiram e repõem a prévia da ferramenta de construção da interface (seção 9, item 9). `ai_company_turn` chama `company_ai_finance` (`0x45fbf0`, `finance.md` 9.1), constrói/compra trens (`ai_pick_engine`), ferrovias e indústrias, com RNG.

### 3.6 Oferta de bônus de cidade (`town_bonus_offer` `0x431660`) [S]
Até 500 tentativas: `i = rng_range(0, n_lugares - 1)`; lugar `town_nth(i)` elegível se `+0x10 == 0` e `+0x36 < 1` e nenhuma estação a menos de `[0x53539c]` (distância euclidiana `0x4f0ca0`) dentro da caixa `(+8,+0xa)-(+0xc,+0xe)` do lugar. No primeiro elegível: `+0x36 = bonus_table[+0x2c]` (`0x53a4b0`, i32 por classe de tamanho) e mensagem LNG 226 ("%1 has offered a bonus of %2 to the first railroad to connect it to another city!"). Cada tentativa consome 1 RNG.

### 3.7 Gatilhos de evento (`event_fire_trigger` `0x457fe0`)
Percorre as regras do cenário (`rule_pool` `0x574c9c`) e avalia as de gatilho igual (`0x455f30` -> `0x454270` -> `0x454ae0`; `0x454ae0` tem um `rng_range` em `0x454db0` para condições aleatórias). Códigos e chamadores (os nomes 0..3 vêm da posição; 4..6 são hipóteses pelo contexto):

| Código | Onde dispara |
|---|---|
| 0 | fim de `sim_year_end` (1/jan 00:00) |
| 1 | meio de `sim_year_end` |
| 2 | fim de `sim_month_end` (1º do mês, 00:00) |
| 3 | início de `sim_month_end` |
| 4 | início de `sim_month_end` e fim de `station_networks_rebuild` (`0x46a7d0`): verificação periódica |
| 5 | turno da IA (`0x4a8a55`) e comando de jogador `0x4bdcd0` (após mudar o caixa da pessoa): ação financeira |
| 6 | carga/início de partida (`0x479d33`, `0x47abcf`, `0x48c689`) |

## 4. Chamadas do RNG do jogo (`rng_range` `0x4f0660`) num tick normal

Ordem dentro de um tick (jogo local), com o local da chamada (endereço de retorno) e a faixa. `[0,1000000]` consome **duas** saídas (faixa > 0x7fff, `rng.md` 2.2). **M** = mundo/eventos, **IA** = decisão de empresa/pessoa, **T** = trens.

| Ordem | Local (retorno) | Função | Faixa | Tipo | Quando |
|---|---|---|---|---|---|
| passo linear, por trem | `0x4cf45d` (e os demais sorteios de `0x4cf390`) | `train_breakdown_check` | `[0,1000000]`, depois `[0,100]` se quebrar; roubo | T | após o avanço, se o trem passou de 10 células |
| passo linear, por trem | `0x425a9a` | `tile_reroll_variant` `0x4259e0` (via `0x4cb090`/`train_enter_tile`) | `[0, n-1]` | M | só quando o trem limpa sobreposições de um tile |
| passo linear, por trem | `0x4ce4c0`, `0x4ce730` | quebra/acidente | `[60,120]`, `[30,45]` | T | consequência |
| mês, 2 | `0x454db0` | `0x454ae0` | variável | M | regra com condição aleatória |
| mês, 4 (meses 2 e 8) | `0x46a0a7`, `0x46a0fc` | `economy_state_step` | `[-5,5]`, `[0,10]` | M | sempre 2 chamadas [D] |
| mês, 5 (meses 1..10) | `0x4a53c4` | `sim_month_end` | `[0,10000]` | M | sempre 1 chamada [D] |
| mês, 5 | `0x43168d` | `town_bonus_offer` | `[0, n_lugares-1]` | M | 1 a 500 |
| mês, 7 | vários (`0x436c06`, `0x435c20`, `0x435320`, `0x4d878a`, `0x48cc00`...) | `ai_company_turn` e filhos | variáveis | IA | depende do estado; 4 chamadas `ai_pick_engine` [80,120] observadas |
| ano, 2 | `0x4d878a` | `ai_pick_engine(1)` | `[80,120]` por motor candidato | M (usa a rotina da IA) | sempre |
| ano, 4 | `0x43cc56`, `0x43cef7`, `0x43cf59` (x32) | `company_annual_review` | `[20,110]`, `[-200,200]` x33 | IA | por empresa, condicionado |
| ano, 4 | `0x438553`, `0x438593`, `0x4385cf`, `0x4386dc` (laço) | `manager_pick_candidate` `0x438450` (de `0x43d235`/`0x43d244`) | `[25,180]`, `[1000,12000]`, `[0,39]`... | IA | gera candidatos (pessoas) |
| ano, 13 | `0x430878`, `0x430916`, `0x43097e` | `town_update` `0x430540` | `[0,1000000]` | M | por lugar |
| ano, 13 | `0x430330` | `0x4302c0` | `[88,112]` | M | |
| ano, 13 | `0x42fd71..0x42fddc` (6 locais) | `0x42fcb0` | `[0,1..3]` | M | posicionamento de construções |
| ano, 13 | `0x40ba37`, `0x40bc34` | `0x40ba00`, `0x40bb40` (via `building_construct`) | `[1,24]`, `[0,100]` | M | construção nova |

Observado no original (save Example, semente 1234, 30 dias de 10/dez/1875 a 1/jan/1876 + 7 dias): **467 chamadas** de `rng_range` de primeiro nível (715 contando as 248 internas das 124 faixas `[0,1000000]`): 4 nos trens, 4 no turno da IA de dezembro, 4 em `ai_pick_engine` anual, 68 na avaliação das diretorias (2 de 5 empresas), 62 em `0x438450`, 120 em `town_update`, 3 em `0x4302c0`, 192 em `0x42fcb0` e 10 em construções. No Tutorial (sem trens nem estações), 30 dias a partir de 4/fev/1870: **3 chamadas** (2 de `economy_state_step` e 1 do sorteio de bônus), igual ao achado de `dynamic-analysis.md` 5.3.

O `rand()` do CRT (gerador C) é chamado ~90 vezes por dia no Example (sons dos trens, `trains.md` 5.4); não afeta a simulação, mas mostra que o clone não deve compartilhar estado entre os dois geradores.

## 5. Uma passagem de 28/dez a 1/jan observada [D]
Save Example, `sim_advance` em passos de 320 min (seq. = ordem no traço):
```
28/12 23:59  sim_week_end (indústrias 391, estações 11)
             sim_month_end
               event_fire_trigger(4), event_fire_trigger(3)   (3 avaliações 0x454ae0)
               ai_monthly_turn: build_preview_suspend, ai_person_turn(2), ai_company_turn(2), ai_company_turn(5)
                                -> ai_pick_engine (4 RNG), company_ai_finance(5), build_preview_resume
               company_month_step x5, person_month_step x4, train_monthly_costs x9, (estações x11), margin_check_all
               sim_year_end
                 ai_pick_engine(1) (4 RNG); industry_year_step x401; company_annual_review x5 (RNG nas 2 últimas)
                 event_fire_trigger(1) (15 avaliações 0x454ae0); history_snapshot
                 company_year_end x5; person_year_step x4; train_year_step x9; territory_year_step x3
                 town_year_step: build_preview_suspend, 103 x town_update (RNG), build_preview_resume
                 sim_week_end; industry_year_reset x403; industry_rebuild_inputs x53
  1/1  00:00     event_fire_trigger(0)
               event_fire_trigger(2)
```
Bate com a leitura estática das seções 3.3 e 3.4 item a item.

## 6. Contagens por função (7 dias)
`games/Tutorial.GAM` (1870-02-04 19:03), semente 1234, passo 320: `sim_advance` 39, `sim_week_end` 1, `industry_recount_piles` 387, `industry_production_tick` 387, RNG 0.
`Example save game.GAM` (1875-12-10 21:05), semente 1234, passo 320: `sim_advance` 39, `anim_object_step` 3291, `train_step` 351, `train_update_speed` 351, `train_breakdown_check` 193, `sim_week_end` 1, indústrias 391+391, `station_recompute_supply_demand` 11, `rng_range` 1 (+2 internas, de `0x4cf45d`), `crt_rand` 530.

## 7. Como reproduzir (análise dinâmica)
`re/scripts/wine/trace_save.py` (com `trace_gdb.py` e `trace_funcs.json`) é a variante de `advance_save.py` com breakpoints Python que contam ou registram chamadas sem parar (`gdb.Breakpoint.stop` devolvendo `False`). Cada linha do registro: `seq função ret ECX EDX [esp+4] data hora estado_do_RNG`; no fim, `COUNT função n`. Mesma sequência de carga e semente de `docs/dynamic-analysis.md` seção 5.
```
TR_LOG=/caminho/trace.log re/scripts/wine/trace_save.py $SB "$SB/games/Example save game.GAM" 30 1234 /tmp/x.GAM --no-warmup
```
30 dias do Example com ~3800 breakpoints atingidos levam ~90 s. `TR_FUNCS` troca a lista de funções. Diferente de `advance_save.py`, não executa `pkill -x winedbg.exe` global; mesmo assim uma instância de `advance_save.py` em outro sandbox mata o `winedbg` desta (rodar uma de cada vez).

## 8. Divergências e fidelidade
- Manual: não descreve a ordem. Nada a anotar.
- `time.md` 7.4 item 8 dizia que, em dezembro, `sim_month_end` retorna após `sim_year_end`; o código e o traço mostram `event_fire_trigger(2)` depois dele (corrigido aqui).
- `time.md` 7.5 cita "loops sobre o conjunto de jogadores (`0x46a7e0`/`0x46a7f0`/`0x43ad60`)": são empresas (índice de empresas ativas `0x57072d`), não jogadores.
- O clone precisa: (1) iterar as listas na ordem ligada do save; (2) respeitar os padrões A/B/C (remoção durante a iteração); (3) o minuto de virada sem movimento; (4) o mesmo `dt` por passo (`time.md` 9); (5) as chamadas de RNG na ordem da seção 4, inclusive as de IA, que dependem de todo o estado das empresas.

## 9. Próximos alvos de RE (não especificados, por prioridade)
1. `town_year_step` `0x430a40` + `town_update` `0x430540`, `0x4302c0`, `0x42fcb0`, `0x40ba00`, `0x40bb40`, `building_create`/`building_construct` (`0x409010`/`0x409a30`): maior consumidor de RNG por ano; crescimento de cidades e construção de indústrias.
2. Turno da IA: `ai_monthly_turn` `0x436d60`, `ai_person_turn` `0x4a8800`, `ai_company_turn` `0x436b30` e filhos (`0x436880`, `0x436210`, `0x435c20`, `0x435320`, `0x436950`, `0x4d9210`, `0x48cc00`), `ai_pick_engine` `0x4d86f0`.
3. `company_annual_review` `0x43c760` (detalhar além do resumo de `finance.md` 8.1) e `0x438450` (geração de pessoas/candidatos).
4. Regras de cenário: `event_fire_trigger` `0x457fe0`, `0x455f30`, `0x454270`, `0x454ae0` (condições e efeitos, códigos 4..6).
5. `train_step` interno: `0x4d1bb0`, `0x413550`, `0x4d0a70`, `0x4d0bc0`, `0x4d3c80`, `0x4cb090`, `tile_reroll_variant` `0x4259e0` (RNG em tile), roubo em `0x4cf390`.
6. ~~`industry_year_step`, `industry_year_reset` e `0x4beae0`~~ (feitos: `world-growth.md` 5, 3.4 item 15).
7. `station_networks_rebuild` `0x46a370` (e quem liga `0x5794f0`).
8. ~~`history_snapshot` `0x4e9770`~~ especificada (3.4.1); falta o papel do par `0x5705d1`/`0x573e08` e o consumidor do registro (`0x444e10`).
9. `build_preview_suspend`/`build_preview_resume` `0x46b150`/`0x46b290`: apagam e refazem a indústria-prévia da ferramenta de construção (`0x574cf0`, um registro de `list_industries`) em volta de cada turno de IA/cidades. Sem jogador interagindo não há prévia; com prévia, o slot e a posição na lista mudam (confirmar se a prévia recriada volta ao fim da lista).

## 10. Carga de arquivos antigos e início de New Game

**Carga** (`game_load_file` `0x477a60`): a conversão `0x233C -> 0x2348` e as passadas do load estão em `docs/formats/map-gam.md` seção 9 (porte `rt2_sim::convert`, `World::from_save_seeded`; 0 diferenças nos 33 arquivos da instalação contra o original).

**New Game de um cenário** (`0x479b20(nome)`) [S, D parcial]. Entrada pela campanha: `0x410de0` (zera o bloco de opções `0x574840`, preenche com `0x4b3ac0` (nome, checksum, contagens de jogadores do cabeçalho, ano inicial `0x5700c1`) e `0x4b45b0(1)` (vagas de jogadores, personagens por `rng_hash(game_seed)` em `0x4b4360`), opções `& 0xd2 | 0x12`, `game_seed = crt_rand_range(1,32000) + clock_ms()`, `[0x566d2a] = 0x564fe8[índice/6]`, `rng_seed(crt_rand_range(1,32000) + clock_ms())`, chama `0x479b20`, depois `[0x566d16] = índice`, `0x570234 = 1`, `[0x566d1a] = 1`). Pela tela New Scenario (um jogador também passa pelo comando de rede 6 de `net_game_start` `0x4b69b0`): `rng_seed(game_seed)`, `0x479b20`, `0x570234 = 0`. Em `0x479b20`, em ordem:
1. `0x495700(0)` (UI); guarda o bloco `0x566d00` (0x86) e o bloco de opções `0x574840` (0xB6); carrega `maps\<nome>.MP2` ou `.MAP` com `0x477a60` (conversão e passadas, com RNG); **restaura o bloco de opções** (o `game_seed` e as opções vêm da tela de configuração, não do mapa).
2. `0x495d10`: opções de realismo -> `0x573d72..78` e fronteiras de território (`0x42bf90`).
3. `0x473590` (um jogador): para cada jogador da lista (de trás para a frente), a vaga `i` do bloco de opções (`+0x36+4i`: personagem, índice humano ou -1, índice de IA, flags bit 0 = em uso) define `+0x122` (humano), `+0x126` (0 / -1), `+0x132` e `+0x52f` (personagem, tabela `0x551e89`, 0x23 bytes), nomes do LNG (`+8`, `+0x27`), `+0x555 = 0`; vaga livre -> `0x4a6a00` (remove o jogador); todos: `+0x534..+0x544 = 0`. Empresas: `0x43ddb0` e `+0x4219..+0x4229 = 0`; territórios: `+0x2b..+0x3b`, `+0x52..+0x58` e `+0x58..+0x314` zerados; estações `+0x4bb = 0`.
4. Restaura o bloco `0x566d00`.
5. `0x474280` (início do mundo): velocidade >= 3; opções de realismo de novo e `0x573e02 = 0x573d9e`; se o ano do bloco de opções (`+0x22`) != 0, `game_set_date` (`0x46b650`) com esse ano e mês/dia/hora de `0x570508`; `0x430e70` (extensão e tiles de terra de cada lugar, `place_at(x,y,1)`); `0x4c8970` (tiles de terra por território, total em `0x573e04`); `0x430a40(1)` (crescimento inicial das cidades, world-growth.md 3.3); `sim_week_end`; empresas `+0x4248 = 1.0`; cada pessoa (ordem dos slots): `+0x548 = rng(-0x7fff, 0x7fff)`, `+0x267 = ftol(((rng(-80,80) + 60) + (200 - 0x551e8b[pers]) * f64[0x535390]) * f64[0x5375d8])` limitado a `[0x11940, 0x8ca00]` mais `(4 - (op>>3 & 7) - (op & 7)) * 80`, `+0x551 = 0`; `0x5706e1..+0x2c` zerado; quatro entradas de 0x18 bytes em `0x573db7`: bit 0 limpo, bit 1 = nome (em `-0x15`) com mais de 1 caractere.
6. `0x4b34c0` (só rede); nome do arquivo em `0x567b5b`; `0x573f1c = (jogadores de rede > 1)`; gatilhos de cenário 6, 0 e 2 (`event_fire_trigger`; as regras de gatilho 2 da campanha usam `CampaignChoice1To3 = [0x566d2a] + 1` e `CampaignDifficulty1To3 = [0x566d00] + 1`); `0x573d2d[e] = engine_available(e, 0)`.

`game_set_date` (`0x46b650`): data/hora/minutos, `0x5705d1 = 0x570223 * f64[0x5350b8]`, datas de início e da economia (`0x570508`, `0x5704fe`, `0x57071f` e as horas), `0x5706d1 = 0.08`, estado econômico 2, `0x5706dd = agora`; por empresa `0x43dca0` (datas, cotação `0x43b410(0,1)`, `+0x17e = agora + rng(30,90)*1440`, contadores zerados), por pessoa `+0x267 = rng(60,450)*1440`, por trem `0x4d4660` (desloca o histórico, datas), por prédio `0x40b820` (prédio vivo, na ordem da lista: zera `+0x63`/`+0x67`, relógios `0x40bb40` com 4 `rng(0, 100)` salvo decorativo `+0x2e`, `0x40a610(b, 0)`, `+0x3c = ano`, e `0x4beb00` na estação `+8`; portado em `SimState::buildings_set_date`: eram os 4 x 13 sorteios que tiravam o #01 B&O de fase antes do primeiro `town_update`), por lugar `0x4317f0`, peças de trilho `+0x1b/+0x1f = 0`, `0x573d2d`, e `0x570603[t] = (ano >= ano inicial do tipo t)`.

Porte: `World::new_game(map, &NewGameOptions, data, rng)` (`crates/rt2-sim/src/tick/newgame.rs`); `NewGameOptions` traz o bloco de opções e o bloco de campanha como a tela de configuração os deixa (a montagem de `0x4b3ac0`/`0x4b45b0`/`0x4b4360` não está portada), o nome e o estado do CRT. **Verificação** (`advance_save.py --new-game 0:0:4321:777 --preseed 1234`, que chama `0x410de0` no processo do original trocando os sorteios de `game_seed` e do RNG; `docs/dynamic-analysis.md` 5.8): no #01 B&O o estado do RNG depois do início é igual ao do original, e cabeçalho, estado (menos o contador de indústrias), empresas, pessoas, regras e trilhos batem; os prédios criados por `0x430a40(1)` também batem desde que `0x40b820` (acima), os campos de `building_construct` e a passada de luz do primeiro redesenho (`0x417c40`, no fim do início) estão portados (`world-growth.md` 4.5.1): `B&O`, `Hannibal` (#02), `Silver` (#04) e `Birmingham` (#07) `New Game` dão 0 campos diferentes em 0 e 7 dias. Também portados (placar: as 18 da campanha original e as 18 da expansão `#ENNcamp.MP2`, conjunto 1, dão 0 campos diferentes em 0 e 7 dias):
- **Nomes do LNG** (`0x473590`): para cada vaga em uso, `_strncpy(+8, LNG[u16 0x551ea7 + 0x23*persona], 0x1e)`, `+0x26 = 0`, `_strncpy(+0x27, LNG[u16 0x551ea9 + 0x23*persona], 0xfa)`, `+0x121 = 0` (`strncpy` preenche com zeros). Os mapas da campanha trazem pessoas "Unassigned N" (#03) que só ganham nome e biografia aqui; `NewGameOptions::lng` recebe o `default2.lng`.
- **Vaga livre** -> `player_remove` (`0x4a6a00`): só pessoa ativa; `0x438900(empresa, 0)` se é chairman, zera as 36 posições (`+0x1d3`), `+4 = 0` e `0x46a940` tira o id da lista de jogadores (`0x570775` contagem, `0x570779` u16, deslocando as seguintes; a última entrada fica). O mesmo porte serve ao efeito de cenário 190.
- **`0x43ddb0` por empresa** (antes de zerar `+0x4219..+0x4229`): bloco de cada território (ordem da lista, `+0x479 + id*0x1ed`): posição `+0x47a` = 5000 ou `2*território.+0x4e - 5000`; direitos `+0x479` = `0xf` no editor, senão `0xf` se o território não tem direitos por tipo (`+0x43 == 0`) e `+0x26 != 0`, ou bit `i` por `+0x26 + i`; `+0x47e..` (0x1e8 bytes) zerados; `+0x4229 = 5000`; `+0x422d`/`+0x422e` = 1 se todos os territórios têm o direito 1/0 (no #09 o mapa traz 0 nas empresas 2 a 4).
- **Efeitos das regras dos gatilhos 6/0/2 no início**: o efeito de gerente (`0xc1..0xe8`, `0x438780(empresa, g, 0)`: tira os efeitos do gerente atual e libera `0x573e09[atual]`, `+0x13c = g`, aplica os efeitos de `g` (`0x438710`, limite +-127), `0x573e09[g] = empresa`, regera os candidatos `+0x140/+0x144` que apontavam para `g`) troca o gerente da empresa pré-colocada em #06 e #08; o efeito 235 (`0x4cdd50`) aposenta os trens pré-colocados de #05 (2 de 3: conta 4 e categoria 6 sem caixa, `+0x27 -= 1`, carros, reservas).
- **Passadas de todo load num mapa 0x2348** (`rt2_sim::convert::every_load`, `map-gam.md` 9): o início aplica ao `.MP2` as mesmas correções do loader (`+0x42 = 0` e região `+0x9D` dos objetos, região `+0x4AD` das estações, `+0xAB = 0` dos trens, entradas 49..52 e `0x570250` do cabeçalho); `#E01CAMP` traz `+0x42` velhos.
- **Produção no sandbox** (`0x40aa20` com `0x570227`, `cargo.md` 7.3): sem relógios; os 7 bits baixos de `+0x9e` são um estoque de lotes: por slot com taxa > 0,01, um lote por unidade inteira abaixo da taxa f32 enquanto houver estoque (cada um tira 1); a volta pelos 4 slots repete enquanto o estoque não zera nem volta ao valor da entrada (`#E03`, `#E06`).
- **`CompanyGoodwill`/`CompanyTerritoryGoodwill`** (`0x43eca0`, `scenarios.md` 3.5) portados (regras de `#17`).
- **`0x43dca0` numa empresa inexistente** (`+0x16e == 0`, a empresa 1 de #03): `0x43b410` devolve 0,0, então `+0x3c = 0` e o cache e `+0x427e` ficam como no mapa (`finance.md` 8).
- **Efeitos 50..54** passam por `territory_set_rights` (`0x4c8540`, `build::rights`, com `0x43df20` recalculando `+0x422d/+0x422e`) em vez da regra inferida antiga.

Também portados para a expansão (as 18 `#ENNcamp.MP2` dão 0 campos diferentes em 0 e 7 dias; placar 98 de 98):
- **Ano do motor 3** (`#E07`): todo `game_load_file` reescreve o primeiro ano do motor 3 e do seu veículo para 1955 (`trains.md` 2.4), então `0x573d2d[3] = 1` num início de 1960.
- **Jornal** (`#E13`): a mensagem de jornal de uma regra passa por `news_post` (`0x4a42f0`: sorteio do CRT, `0x5707bd`, `0x5707be`; `scenarios.md` 2.4).
- **Confisco de território** (`#E09`, `#E15`: confisco dos trens da empresa, que são apagados) e `TerritoryCompletelyFlooded` (`scenarios.md` 3.5 e 4.2.1).
- **Regra de desfazer** (`#E08`): a regra de um efeito temporário entra no pool salvo, com as durações 1 e na ordem da pilha (`scenarios.md` 4.4).
- **Conjunto industrial 1** (`#E08`, `#E11`, `0x5704ac = 1`): a tabela de tipos é a do conjunto 1, sem as correções de carga do conjunto 0 (`cargo.md` 7.1.1); era a causa dos lotes a mais. E o passo do pool de prédios cujo bloco chega ao tamanho de bloco grande do Wine (`malloc` de cerca de `0x7f000` bytes) vem zerado, não `0xBAADF00D` (`+0x38..+0x3b` dos prédios criados nesse passo; limiar entre 519 930 e 523 110 bytes, medido no `#E08`).

## Em aberto
- Passo linear com `dt = 0` (2.2): efeito em `train_step`.
- Papel de `0x5705d1` (meta composta) e de `0x573e08` (velocidade de referência 4..200).
- Códigos de gatilho 4, 5, 6 (seção 3.7): nomes inferidos pelos chamadores.
- Ordem das chamadas de RNG dentro de `ai_company_turn` e `town_update`: depende de funções ainda não especificadas (seção 9).

> **Nota (coordenador):** o byte divergente no offset 34552 (`0x86F8`) cai dentro do checksum do arquivo em `+0x86F7`, que já é mascarado pelo `rt2-savediff`. As duas rodadas são equivalentes.
