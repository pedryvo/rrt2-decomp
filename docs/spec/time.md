# Tempo, calendário e laço principal

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido com Ghidra via REA. O manual (`docs/reference/manual-rules.md`, seção 1) só serviu de pista; onde o código diverge, a divergência está em "Divergências do manual". Endereços são VAs (base `0x400000`). Estado das provas: **[V]** = função confirmada contra o original por teste diferencial (`crates/rt2-oracle/tests/sim_time_rng.rs`); **[S]** = lida no código, sem teste.

## 1. Visão geral

```
entry CRT 0x51c155 -> game_start 0x49eff0 [S]
    crt_srand(GetTickCount())                     ; 0x49eff0, ver rng.md
    game_main_loop 0x464f60 [S]                   ; laço infinito
        message_pump 0x49ef70                     ; PeekMessage/Translate/Dispatch
        despacho de eventos de UI (0x502f90, 0x501c10, 0x502f00)
        -> 0x4066b0 -> game_idle_tick 0x404e30    ; porteiro: roda o jogo no máximo a cada ~100 ms
              -> game_frame_update 0x405540       ; por quadro
                    -> sim_advance 0x4a5cb0       ; avança N minutos de jogo
                          -> sim_week_end 0x4a5270   ; dias 7, 14, 21, 28
                          -> sim_month_end 0x4a5320  ; 23:59 do dia 28
                          -> sim_year_end 0x4a4d00   ; 23:59 de 28/dez (dentro do mês)
```

`game_idle_tick` também é chamado de dentro de muitas rotinas de UI (`0x404e30` tem 18 chamadores), então o jogo continua avançando enquanto janelas modais esperam.

## 2. Calendário

Constantes: **12 meses por ano, 28 dias por mês, 24 h, 60 min**. O ano tem 336 dias. Não há tabela de dias por mês nem anos bissextos: a aritmética usa só `28` (`0x1c`), `12`, `24`, `60` (`0x4a4950`, `0x4a4a30`).

Unidade de tempo de jogo: **o minuto**. Estado global:

| Endereço | Tipo | Nome | Conteúdo |
|---|---|---|---|
| `0x5704f4` | i32 | `game_time_minutes` | minutos absolutos desde o ano 0: sempre igual a `date_to_minutes` da data atual (`0x46b650` grava `0x4a4990(data, hora)` aqui) |
| `0x5704f8` | u32 | `game_date` | `+0` ano u16 absoluto (1860 = `0x744`), `+2` mês u8 1..12, `+3` dia u8 1..28 |
| `0x5704fc` | u16 | `game_time_of_day` | `+0` hora 0..23, `+1` minuto 0..59 |

```
minutos(ano, mes, dia, hora, min) = ((((ano*12 + mes)*28 + dia - 29)*24 + hora)*60) + min     ; 0x4a4950, 0x4a4990 [V]
```
Em 1/jan 00:00 de um ano `a`, o resultado é `a * 483840` (`483840 = 12*28*1440 = 0x76200`). Cabe em i32 até o ano 4437.

```
data(t):                                                                                        ; 0x4a4a30 [V], t >= 0
    ano  = t / 483840;   r = t % 483840
    mes  = r / 40320 + 1                 ; 40320 = 28*1440 = 0x9d80
    dia  = (r % 40320) / 1440 + 1        ; 1440 = 0x5a0
    hora = (r % 1440) / 60;  min = r % 60
```
Os testes diferenciais cobrem 20 000 datas aleatórias de 1804 a 2203 (empacotar, desempacotar, ida e volta) mais 20 000 contagens de minutos aleatórias até 1,5e9.

Todas as funções de data acima ficam em `0x4a4950..0x4a4a30`; não há limites de ano no cálculo (o ano é u16). O ano inicial e final vêm dos dados do mapa/cenário, não achados no código de tempo (ver "Em aberto").

## 3. Velocidade de jogo

- `game_speed` (`0x57050e`) é um índice **0..10**. 0 = pausa. Novo jogo começa em **3** (`0x4732ba`, em `0x472980`, que também grava `game_speed_resume` = 3 em `0x5707c2`).
- Tabela `time_speed_table` (`0x536508`, f32[11]): `0, 0.3, 0.6, 1.0, 1.7, 2.1, 3.0, 5.0, 8.0, 12.0, 18.0` [V: lida da memória do original].
- Escala `time_speed_scale` (`0x537578`, f64) = `3.2` [V].
- Minutos de jogo por quadro (`0x46c650`, thiscall ECX = ms) [V]:

```
dt_minutos = ftol( tabela[game_speed] * ms * 3.2 )          ; truncamento (CRT _ftol, 0x51b524)
```
Relação com o tempo real no multiplicador 1.0 (velocidade 3): 3,2 minutos de jogo por ms, ou seja, **um dia = 450 ms**, um mês (28 dias) = 12,6 s, um ano = 151,2 s. Na velocidade 10 (x18) um ano leva 8,4 s.
- Comando de mudar velocidade (`cmd_execute` `0x4b0210`, caso em `0x4b0d91`): se o byte `+0xc` vale `0x7f` a nova velocidade é absoluta (`+0xd`), senão é `atual + delta` (byte com sinal). O resultado é limitado a `[0, 10]` (`0x4b0da9..0x4b0dba`). Se a velocidade antiga era diferente de 0, ela é guardada em `game_speed_resume` (`0x4b0de2`). A tecla "pausa" e as teclas +/- chegam por esse comando (a UI só enfileira); o texto do painel usa os ids de string em `0x553788 + 2*velocidade` (ver "Em aberto").
- **Precisão do x87:** `dt_minutos` depende da precisão da FPU. Com precisão de 53 bits (`CW = 0x27f`, estado típico do CRT MSVC) o oráculo reproduz exatamente `trunc(f32 * ms * 3.2)` em `f64`; com 24 bits (`CW = 0x7f`) 24 de 16 511 combinações (velocidade 0..10 x ms 0..1500) diferem em 1 minuto (por exemplo velocidade 5, 75 ms: 503 contra 504). O estado real da FPU no original não foi determinado (ver "Em aberto"). Teste: `speed_ticks_depend_on_x87_precision`.

## 4. Porteiro de quadros (`game_idle_tick`, `0x404e30`) [S]

```
game_idle_tick():
    cmd_queue_run()                       ; 0x4af3d0 (ver 5.1)
    se 0x564ed8 != 0: retorna             ; (flag de reentrada/bloqueio)
    limiar = (0x579418 != 0) ? 0 : 100    ; ms
    seguir se  (game_running(0x5704e8) && agora - 0x574bec > limiar)
            || modo(0x566e14) in {0xb, 0xc} || forcar(0x574be0)
       e 0x574c04 == 0
    ritmo = game_running && (agora - 0x574be8 > limiar) || forcar      ; 0x574be8 = ultimo quadro com avanço de simulação
    elapsed = agora - 0x574bec            ; ms desde o último quadro
    elapsed = min(elapsed, 1000);  se !game_running: elapsed = min(elapsed, 100)
    0x574bec = agora;  0x5704f0 += elapsed
    game_frame_update(elapsed, param_2 = !ritmo)      ; param_2 = 1 -> quadro só de UI, sem avançar
```
`agora` = `clock_ms` (`0x4efea0`: `GetTickCount() - base`, base gravada na primeira chamada em `0x57b570`). Portanto a simulação avança no máximo uma vez por mais de 100 ms reais (cerca de 10 Hz) e cada avanço usa o tempo real decorrido (até 1000 ms). Em jogo local isso torna o resultado **dependente da taxa de quadros** (ver "Divergências / fidelidade").

## 5. Quadro (`game_frame_update`, `0x405540`) [S]

Ordem do que roda por quadro (parâmetros: `ms`, `param_2`):

1. Guarda de reentrada `0x564ef0`; incrementa a profundidade `0x57fee0`.
2. **Minutos a avançar `dt`**: `dt = 0` se modo `0xb`/`0xc`, ou `sim_busy (0x57922c) != 0`, ou (`opcoes+0x50 == 0` e jogadores < 2 e `0x57fee4 != 0`, UI modal); senão `dt = time_game_minutes_for_ms(ms)`. Em rede (jogadores > 1) `dt` ainda é limitado pelo horizonte de tempo (`time_horizon`, `0x4afee0`) e reajustado quando a razão `(horizonte - T) / dt` passa de 3 ou 10 (`0x405540`, trechos com `3.0` e `10.0`).
3. Contador de pulsos de UI de 100 ms (`0x564eec`, `0x564ee0`, `0x57b3e4`) e contador de períodos de **600 minutos** (`0x5744bc`; número de fronteiras `(T+dt)/600 - T/600`): quando algum muda, os temporizadores das janelas (`0x5025e0`) rodam duas vezes, primeiro com os valores à frente e depois com os valores já confirmados. São relógios de UI, não regras da simulação.
4. Se `dt != 0` e `param_2 == 0`: **`sim_advance(dt)`** (`0x4a5cb0`).
5. Tarefas de UI e ambiente (`0x405240`, `0x405390`, `0x406080`, e a cada ~1 s no modo 1 um bloco que abre a janela de acordo com `0x5743f4`).
6. Acumula o tempo gasto em `0x564edc`.

### 5.1 Fila de comandos

`cmd_queue_run` (`0x4af3d0`) executa os comandos pendentes da lista `0x574c8c` com `cmd_execute` (`0x4b0210`), e é chamada no início de cada porteiro e dentro de `sim_advance`. Em jogo local todos os comandos são executados de imediato (`cmd_queue_next_time` devolve `game_time_minutes`, `0x4af2d0`); em rede só os com tempo `<= T` (campo `+8`). Mudar velocidade, semear o RNG e as demais ações do jogador passam por aqui. Se `0x578b00 == 0` (partida em rede ainda não iniciada) chama `net_game_start` (`0x4b69b0`) em vez disso.

## 6. `sim_advance` (`0x4a5cb0`) [S]

```
sim_advance(dt):
    se sim_busy (0x57922c) != 0: retorna
    se jogadores > 1 e horizonte <= T + dt: dt = horizonte - T
    repete:                                  ; fila de comandos agendados
        prox = cmd_queue_next_time()
        se prox == 0: sai
        se T < prox:   (rede) se dt <= prox - T: sai
                       dt -= prox - T;  sim_advance(prox - T)
        senão: cmd_queue_run()
    se dt != 0:
        novo = data(minutos(data_atual, hora_atual) + dt)
        se novo > data_atual (algum campo ano/mes/dia maior):       ; cruza meia-noite
            sim_advance(1439 - hora*60 - minuto)                    ; até 23:59, caminho linear
            fim_de_dia():                                           ; (0x4a5e..0x4a6200)
                net_time_sync()                                     ; 0x49e0b0, só rede
                se jogadores > 1: rng_seed( ftol( (f64)(T + game_seed) + f64 do jogador 0 +0x12a ) )
                se dia % 7 == 0:
                    sim_week_end()
                    se dia == 28: sim_month_end()                   ; avança a data sozinho
                    senão: data = data(minutos + 1);  T += 1
                senão: data = data(minutos + 1);  T += 1
                se jogadores > 1: trains_advance(1440)
            dt -= minutos_ate_2359 + 1
            enquanto data_atual != novo.data:                       ; dias inteiros
                sim_advance(1439); fim_de_dia(); dt -= 1440
        ; resto do dia: caminho linear
        data/hora = data(minutos(atual) + dt);  T += dt
        para cada e em lista 0x574c78: 0x4283e0(e)                  ; atualização por avanço
        se jogadores == 1: para cada trem em lista 0x574c64: 0x4d0ce0(trem, dt, 1)    ; movimento
```

Pontos:
- **Dia, semana, mês, ano:** cada transição acontece às **23:59** do dia que termina e avança a data em 1 minuto (`+1` minuto leva a 00:00 do dia seguinte). A semana termina nos dias 7, 14, 21 e 28; o mês termina no dia 28 (depois do passo semanal); o ano termina em 28 de dezembro dentro de `sim_month_end`.
- Em jogo local não há reaplicação de semente por dia; em rede (jogadores > 1) o RNG é re-semeado todo dia (ver `rng.md`) e os trens andam em passos fixos de um dia.
- Em jogo local os trens andam no caminho linear com `dt` variável (até 1439 min por chamada), logo o passo de movimento depende de `ms` e da velocidade escolhida.
- `0x4a5940` (+`0x401db6`) é uma cópia solta do passo de fim de dia sem chamadores; Ghidra não a cria como função. Ignorar.
- A comparação "novo > atual" compara campo a campo (`ano <`, `mes <`, `dia <`, em OU); basta qualquer campo maior para entrar no ramo (`0x4a5ea.. 0x4a5ffc`). Para avanços menores que um dia em datas consecutivas o resultado é o esperado; não foi testado fora disso.

## 7. O que roda por período

### 7.1 Por quadro / por avanço de minutos (`sim_advance`, caminho linear)
- Lista `0x574c78` (registro de 0x23 bytes): `0x4283e0` por entidade.
- Jogo local: lista `0x574c64` (registro de 0x14d bytes, provavelmente trens): `0x4d0ce0(e, dt, 1)`; entre os chamados estão `0x4cf390` e `0x4d0400` (probabilidades com o RNG do jogo e com o rand do CRT, ver `rng.md`).
- Comandos pendentes (5.1).

### 7.2 Por dia (23:59)
Só rede: `net_time_sync` (`0x49e0b0`, só no dia 1 ou se `0x57b400+0x2c != 0`), re-semeadura do RNG, `trains_advance` (`0x4a58f0`, 1440 min). Em jogo local não há trabalho próprio por dia.

### 7.3 Por semana (dias 7, 14, 21, 28) `sim_week_end` (`0x4a5270`)
Para cada registro da lista `0x574c74` (0x9f bytes, indústrias e casas) com o byte `+7 != 0`: `0x40a610(e,0)` (recontagem das pilhas) e `0x40aa20(e,0)` (produção periódica, `cargo.md` 7.3); para cada registro da lista `0x574c60` (0x4c7 bytes, estações): `0x4bebf0`. O mesmo passo roda de novo dentro de `sim_year_end`.

### 7.4 Por mês `sim_month_end` (`0x4a5320`), dia 28 às 23:59, com `mes` e `ano` ainda os do mês que termina
Em ordem:
1. Se `0x5794f0 != 0`: zera e chama `0x46a370`.
2. Se `net_min_peer_version() > 9004` (em jogo local sempre, devolve 9012): dispara gatilhos de evento tipo **4**; depois sempre tipo **3** (`event_fire_trigger`, `0x457fe0`: percorre a lista de eventos `0x574c9c` e dispara os de gatilho igual; os valores 0..3 coincidem com "início de ano / fim de ano / início de mês / fim de mês" por onde são chamados: 3 aqui no início do passo mensal, 2 no fim, 1 e 0 dentro do passo anual).
3. Meses 2 e 8: `0x46a010` = `economy_state_step`, passo semestral do **estado econômico** (`0x570717`, 0 a 4; passeio aleatório com momento, `cargo.md` seção 9).
4. Se `mes <= 10` e `rng_range(0, 10000) < ((50 - |ano - 1860|) * (54300 / L)) / 70` com `L = 0x55e7a4` (largura do mapa em tiles): chama `0x431660` (escolhe um item aleatório da lista `0x574c5c`, registro de 180 bytes, e emite mensagem). A probabilidade é nula para anos a mais de 50 de 1860 (`0x744`).
5. Se `0x570227 == 0`: remove entradas velhas da lista `0x574c7c` (bitmap de ocupação; registros com `u16 +3 == 0` e idade `T - u32(+7) > 0x34bbf` = 216 000 min = 150 dias, com limiar menor pela metade ou um quarto conforme campos do tipo; ver "Em aberto").
6. `0x436d60`; depois para cada item das listas `0x574c6c` (empresas, registro `0x4e86`, `0x4399d0`), `0x574c68` (pessoas, `0x576` bytes, `0x4a7590`), `0x574c64` (trens, `0x4d4390`) e `0x574c60` (**estações**, `0x4c7` bytes): os 44 contadores de entrega f32 a partir de `+0x271` são multiplicados por **0,94** e depois `0x4bebf0(e)` recalcula oferta, demanda e níveis de preço (`cargo.md` seção 8). `0x46bb10`.
7. Relatório periódico (`report_due_flag` `0x5665d4 = 1`), conforme a opção de frequência `F = opcoes[+0x23c]` (`0x4b187c` a grava):
   - `F == 1`: meses 3, 6, 9 e 12 de todo ano;
   - `F == 2`: dezembro de anos ímpares; `F == 3`: dezembro de anos pares; `F == 4`: dezembro de anos múltiplos de 5;
   - qualquer outro valor: nunca. O consumidor está em `0x453333` (zera a flag em `0x453389`).
8. Se `mes == 12`: **`sim_year_end`** e retorna (ele próprio avança a data). Senão avança a data em 1 minuto (1 do mês seguinte, 00:00), `T += 1` e dispara os gatilhos tipo **2** (início de mês).

### 7.5 Por ano `sim_year_end` (`0x4a4d00`)
Roda 23:59 de 28/dez após todo o passo mensal: loops sobre o conjunto de jogadores (`0x46a7e0`/`0x46a7f0`/`0x43ad60`), sobre listas `0x574c74`, `0x574c6c`, `0x574c68`, `0x574c64`, `0x574ca8`, `sim_week_end` de novo, e `0x430a40(0)` (se `0x570237 == 0`); dispara os gatilhos tipo **1** (fim de ano) no meio; avança a data para 1/jan 00:00 (`T += 1`); zera `0x573d05/09/0d`; limita `0x573e08` (valor em 4..200 a partir de `0x4d86f0`, que usa o RNG do jogo); roda `0x471880(i)` para i em 0..52; mensagens históricas em 1877 (`0xa46`), 1879 (`0xb23`), 1900 (`0x1a3`), 1910 (`0x1a1`) e 1960 (`0x1a4`), cada uma condicionada a flags do cenário; por último gatilhos tipo **0** (início de ano).

## 8. Divergências do manual (o código vale)
- Manual: velocidade de jogo recomendada "3" a "4", teclas +/- e PAUSE. Código: 11 níveis (0..10) com multiplicadores 0, 0.3, 0.6, 1, 1.7, 2.1, 3, 5, 8, 12, 18; padrão 3.
- Manual: período 1804 a 2020. O código de calendário não tem limite de ano (u16 absoluto); os limites não aparecem em `0x4a49xx..0x4a5xxx`.
- Manual: dividendos trimestrais (março, junho, setembro, dezembro). Não localizado onde o código paga dividendos; achamos apenas que os pulsos de relatório com `F == 1` coincidem com esses meses (7.4, item 7).
- Manual (SG): estado econômico muda a cada 6 a 18 meses. Código: passo de 6 meses (meses 2 e 8, `0x46a010`) com mudança de no máximo um nível, probabilidade `|momento|/11`; a duração resulta desse sorteio (`cargo.md` seção 9).
- Manual: "1 ano de jogo percorre o que 1 dia real" é uma metáfora: no multiplicador 1.0 um ano de jogo dura 151,2 s reais.

## 9. Fidelidade e implicações para o clone
- A simulação local depende do `ms` real do quadro (seção 4): o clone fiel precisa fixar a regra `dt = ftol(tabela * ms * 3.2)` com `ms` de passo fixo (por exemplo 100 ms, que o porteiro usa como limiar) e declarar o resultado como "determinístico, mas não idêntico ao de uma execução do original com outro ritmo de quadros".
- Pausa é velocidade 0 (tabela 0); com `dt = 0` nada de `sim_advance` roda, mas as filas de comandos e a UI continuam.

## Em aberto
- Estado da FPU no original (precisão do x87): o oráculo roda a 24 bits por padrão do Unicorn (CW 0), o teste fixa 53 bits (`0x27f`); falta confirmar com `fldcw`/DirectDraw no processo real (Wine + gdb).
- Origem dos limites de ano (início e fim do jogo) e valor de `game_time_minutes` ao carregar um `.GAM` (resolvido: o arquivo guarda minutos, data e hora no bloco de estado em 0x8865/0x8869/0x886D, ver docs/formats/map-gam.md 5.1) (`0x46b650` é chamado de `0x453970`, `0x472980`, `0x473590`, `0x474280`, `0x479a30`).
- Identidade das listas de entidades (`0x574c5c`, `0x574c60`, `0x574c64`, `0x574c68`, `0x574c6c`, `0x574c74`, `0x574c78`, `0x574c7c`, `0x574c84`, `0x574ca8`): tamanhos dos registros vêm dos construtores em `0x472bb0..0x472f37`; os papéis (empresa, trem, estação, cidade, indústria) serão fixados nas specs de terreno, trens e carga.
- `0x431660` (sorteio mensal): papel exato; `0x436d60`, `0x4399d0`, `0x4a7590`, `0x4d4390`, `0x46bb10`, `0x4283e0`: conteúdo dos passos por entidade (`0x46a010` e `0x4bebf0` estão em `cargo.md`).
- Condição da limpeza de `0x574c7c` no passo mensal (7.4, item 5).
- Valores de `0x566e14` (modo de jogo: 1 = partida em andamento; 3, 0xb e 0xc têm tratamento próprio) e de `0x574bc8+0x50`/`0x57fee4` (pausa por UI).
- Ids de string do painel de velocidade (`0x553788`) e o mapeamento tecla -> comando.
- A razão `3.0`/`10.0` que corrige `dt` em rede (`0x405540`) e `net_time_sync` (`0x49e0b0`): fora do escopo da primeira passada (multijogador).
