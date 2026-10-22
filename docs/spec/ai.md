# IA: oponentes de computador (turno mensal, construção, trens, finanças, ações)

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido no C exportado (`re/db/decomp`, regenerado com os nomes atuais) e no disassembly (`objdump -d -M intel`; o Ghidra perde os argumentos de `_ftol` e as larguras f32/f64, então toda constante abaixo foi lida do `.rdata` conforme a instrução x87 real). Confirmado no original rodando no Wine (`trace_save.py`, seção 11). Estado das provas: **[V]** = teste diferencial (`crates/rt2-oracle/tests/ai.rs`); **[D]** = observado no original (traço); **[S]** = lido no código. Endereços são VAs.

O manual e o guia (`docs/reference/manual-rules.md` 9.3 e Anexo H) só serviram de pista. A tabela de tendências dos tycoons do guia confere com o `.data` (seção 3); as divergências estão na seção 13.

Contexto: o turno da IA roda em `sim_month_end` (dia 28 às 23:59, passo 7, `world-tick.md` 3.3 e 3.5), depois da expiração de lotes e antes do passo mensal das empresas. Durante o turno, dois globais guardam a empresa corrente: `ai_cur_company_ptr` `0x565140` e `ai_cur_company_id` `0x565144` (zerados no fim de `ai_company_turn`). Várias rotinas usadas pela IA leem esses globais em vez de receber a empresa por argumento.

## 1. Estrutura do turno (resumo)

```
ai_monthly_turn()                                   ; 0x436d60, se 0x5748fc == 0 e 0x570229 == 0
  build_preview_suspend()
  para cada pessoa (C) com +0x122 == 0 (IA):  ai_person_turn(id)          ; 0x4a8800, seção 9
  para cada empresa (C) com chairman 0 ou IA: ai_company_turn(id)         ; 0x436b30, seção 2
  build_preview_resume()

ai_company_turn(id):                                ; 0x436b30
  se +0x16e == 0 (inativa): retorna (nada, nem finanças)
  +0x166 = 0                                        ; "expansão satisfeita" (ver 2.1)
  se ano ímpar e +0x422f != ano: ai_review_manager()                      ; 0x436880, seção 6 (RNG indireto)
  +0x4264 = (tipo do motor ai_pick_engine(1) == elétrico)                  ; RNG [80,120] por candidato
  se 0x5706b5 e 0x5706b6 e 0x5706b7 e +0x17e < T:                          ; prazo de expansão vencido
      r = rng(180, 360)
      +0x17e = T + atraso(r)                                               ; 2.2
      se rede principal vazia (+0x1aa < 1):
          ai_plan_first_line(0)                                            ; 0x435c20, seção 5.3
          se ainda vazia: zera os globais e RETORNA (pula trens, indústria e finanças)
          +0x154 = 0
      senão se ai_extend_network() == 0: +0x17e += 86400 (60 dias)         ; 0x435320, seção 5.2
      senão: +0x154 = 0
  se +0x154 == 0 ou +0x154 + 121000 < T: +0x154 = T; ai_manage_trains()   ; 0x4d9210, seção 7
  se mês >= 8 e (+0x424c == 0 ou +0x424c + 241920 < T):
      ai_industry_investment(); +0x424c = T                                       ; 0x436950, seção 8
  company_ai_finance(id)                                                   ; 0x45fbf0, seção 10
```
`T` = `game_time_minutes` (`0x5704f4`). 86400 min = 60 dias; 121000 min = 84 dias e 72 h (o código usa `0x1d880`); 241920 = 168 dias. [S, D]

Campos da empresa usados pela IA (complementa `finance.md` 1):

| Offset | Tipo | Conteúdo |
|---|---|---|
| `+0x27` | i32 | número de trens (`train_create` +1, `train_delete` -1) |
| `+0x154` | u32 | minuto da última gestão de trens (0 = forçar no próximo turno) |
| `+0x166` | i32 | 1 = a expansão falhou ou está satisfeita neste turno (libera recompra e quitação em 10) |
| `+0x16a` | i32 | ano da última revisão do dividendo pela IA |
| `+0x172` | i32 | número de estações da empresa |
| `+0x176` | u16 | estação principal (a primeira); `+0x17a` = rede (`station+0x57`) dessa estação; `+0x1aa` = número de estações nessa rede (recalculados em `station_networks_rebuild` `0x46a370`) |
| `+0x17e` | u32 | minuto em que vence o prazo da próxima expansão |
| `+0x182/+0x186/+0x18a` | i32 | células de trilho, simples, duplas (`terrain-track.md` 1.3) |
| `+0x422f` | u32 | ano da última revisão do gerente |
| `+0x4233..+0x423f` | i32 x4 | caixa (x0, y0, x1, y1) da rede da empresa |
| `+0x4243`, `+0x4245` | u16 | ano da última troca de motores velhos / da última replanificação de trens |
| `+0x4247` | u8 | já anunciou a fundação (mensagem LNG 166) |
| `+0x424c` | u32 | minuto da última compra de indústria |
| `+0x4264` | u8 | construir trilho eletrificado (o motor preferido é elétrico) |

Permissões de cenário (bytes `0x5706aa..0x5706c4`, todos 1 nos saves do jogo [D]): `b5`, `b6`, `b7` = IA pode expandir (as três são exigidas juntas; `b5` também libera a compra de trens), `b9` = IA pode comprar indústrias, `ac` = falência, `ae` = recompra de ações, `af` = emitir títulos, `b0` = quitar títulos, `b1` = mudar dividendo, `be` = pessoas IA fundam empresas, `c4` (com a pessoa `+0x555`), `aa` = investir em outras empresas. Os nomes das permissões vêm do uso; os rótulos da interface não foram localizados (Em aberto).

Opções de IA do cenário (`0x5704ac`, 4 bytes): byte 1 `0x5704ad` = **Track Expansion** (LNG 3401..3403), byte 2 `0x5704ae` = **Dividend Payout / Stock Buyback** (LNG 3404..3406); byte 0 é o conjunto industrial (`cargo.md`).

## 2. `ai_company_turn` (`0x436b30`) em detalhe [S, D]

### 2.1 Ordem
Exatamente a do pseudocódigo da seção 1. Observado no save Example (empresa 5 de IA, presidida pela pessoa 2, George Hudson): nos meses sem expansão vencida o turno faz só `ai_pick_engine` (4 sorteios), a gestão de trens a cada 84 dias e as finanças [D].

### 2.2 Atraso até a próxima expansão
```
r = rng_range(180, 360)
sem chairman:   atraso = r * 2880                                         ; minutos = 2r dias (360..720 dias)
com chairman:   atraso = ftol((300 - expand_rail) * (r*1440) * 0.005)     ; 0x535320
```
`expand_rail` = tendência "Expand Rail" da persona do chairman (seção 3, `+0x17`). Com 100 (média), `atraso = r` dias; com 200, `r/2` dias; com 20, `1,4 r` dias. Sem chairman, `2r` dias.

## 3. Personas (tendências dos tycoons) [S, D]

Tabela `ai_persona_table` `0x551e88`, **41 registros de 0x23 bytes** (índice 0..40), indexada pelo byte `pessoa+0x52f` (i8). Registro 0 é o jogador genérico (nome LNG 1539 vazio, todos 100).

| Off | Tipo | Campo (coluna da Tabela 3-1 do guia) | Usado em |
|---|---|---|---|
| `+0x00..+0x02` | u8 x3 | 0, índice, região (0 América, 1 Europa, 2 resto) | escolha de persona (`0x4a7990`, `0x47ef50`) |
| `+0x03` | i32 | **Starts Company** | `ai_person_turn` (9.1): `200 - x`; também `0x474280` |
| `+0x07` | i32 | **Margin Buying** | `ai_person_trade_stocks` (9.2) |
| `+0x0b` | i32 | **Short Selling** | 9.2 |
| `+0x0f` | i32 | **Investing in Others** | 9.2 e recompra de ações (10.4) |
| `+0x13` | i32 | **Issue Bonds** | caixa efetivo da expansão (5.2) |
| `+0x17` | i32 | **Expand Rail** | atraso de expansão (2.2) e orçamento (5.2) |
| `+0x1b` | i32 | **Industry Investment** | compra de indústria (8) |
| `+0x1f`, `+0x21` | u16 | nome e biografia (LNG) | |

Valores (todos do `.data`, conferem com o Anexo H do manual, inclusive "Jay Cooke", "Lord Strathconda"): 1 Bismarck (20,100,100,120,150,30,175), 2 Bleichroder (50,75,55,150,100,40,175), 3 Brunel (200,25,25,100,175,200,80), 4 Jay Cooke (80,115,100,150,200,100,100), 5 Crocker (200,100,10,200,100,200,135), 6 Donnersmarck (100,115,125,180,105,90,145), 7 Drew (140,180,180,150,85,100,35), 8 Forrest (125,85,75,100,140,175,40), 9 Fisk (170,185,145,165,100,100,160), 10 Fleming (100,100,100,120,200,100,20), 11 Fremont (180,70,70,60,170,135,100), 12 Gould (20,200,200,200,90,55,110), 13 Hansemann (45,200,100,155,100,65,150), 14 Harriman (170,185,190,160,120,180,185), 15 Hill (200,100,100,50,200,200,140), 16 Hincks (100,100,100,100,100,120,75), 17 Holliday (165,40,35,100,125,135,100), 18 Hopkins (100 x7), 19 Huntington (190,70,70,105,155,120,170), 20 Morgan (100,130,100,200,100,100,200), 21 Platner (140,100,100,100,115,165,100), 22 Rhodes (200,100,100,100,100,200,155), 23 Scott (130,100,100,100,145,170,100), 24 Stanford (100,100,40,185,120,100,140), 25 Stephen (100,100,100,100,100,100,50), 26 Strathconda (100,...,20), 27 Train (160,100,100,100,100,100,200), 28 Vanderbilt (100,...,190), 29 Meiji (140,100 x6), 30 Hudson (135,200,200,120,140,70,100), 31 Napoleon III (100,100,100,100,100,145,160), 32 Thomas (100 x7), 33 Durant (100,115,100,100,175,100,165), 34 Nehru (35,100,100,35,100,165,145), 35 Attlee (35,100,100,50,145,100,135), 36 Gentaro (25,30,30,100,100,180,100), 37 Mao (20,20,20,100,140,145,100), 38 Chiang (145,120,115,120,20,55,100), 39 Wheelwright (120,120,80,100,100,115,170), 40 Keith (130,100,100,155,100,120,175).

**Não há tabela de níveis de dificuldade da IA no código da IA.** A dificuldade entra só por: as opções Track Expansion e Dividend Payout (seção 1), as permissões de cenário, o modelo industrial (compra de indústria só no expert, 8) e o modificador de receita do computador (fora da IA, `cargo.md`). As personas são a única variação de comportamento por oponente.

## 4. Escolha de motor (`ai_pick_engine` `0x4d86f0`, fastcall ECX = modo) [V]

Todos os chamadores usam modo 1 (`0x436b95`, `0x436401`, `0x436509`, `0x4a4d52`, `0x4d9125`, `0x4d9643`); o modo 0 nunca é usado.
```
melhor = -1; nota_max = -1
para passada em 0, 1, 2:
    se melhor != -1: continua                            ; passadas seguintes só se nada achado
    para e em 0..65 (engine_table 0x53cec0, ordem do índice):
        se tipo(e) == 2 (elétrico) e passada == 0: pula   ; sem RNG
        ok = engine_available(e, ai_cur_company_id)      ; 0x4683a0 (trains.md 2.4)
             ou (passada == 2 e primeiro_ano(loco) < ano <= último_ano(loco))   ; ignora junho e engine_allowed
        se !ok: pula
        atributo = modo ? vel_max(+0x14) : vel_max * potência(+0x18)
        nota = ftol(rng_range(80, 120) * 0.01 * atributo)  ; 1 sorteio POR CANDIDATO
        se nota > nota_max: nota_max = nota; melhor = e  ; estrito: empate fica com o primeiro
devolve melhor < 1 ? 0 : melhor
```
O motor `-1` (nenhum) vira 0 (Stephenson Rocket). Teste `ai_pick_engine_matches_original`: 1500 casos (datas 1800..2059, `engine_allowed` aleatório, efeitos de cenário que forçam/proíbem), índice e estado do RNG idênticos [V]. No Example (dez/1875..mar/1876), 4 candidatos: 4 sorteios por chamada [D].

## 5. Rotas e construção

### 5.1 Valor de uma cidade (`ai_town_value` `0x4313f0`, thiscall(lugar, empresa)) [V com empresa 0; S]
```
se lugar.+0x10 != 0 (não é cidade): 0
v = ftol((f32 +0x12 - f32 +0x3a) * 60000.0f)
se +0x48 >= 8: v *= 2;  senão se +0x48 >= 4: v = ftol(v * 1.7)
se empresa != 0:
    se (empresa.+0x479 + território(tile do centro)*0x1ed) & 3 != 3: 0      ; sem permissão de trilho+estação
    n = estações (de qualquer dono) dentro da caixa do lugar e a distância euclidiana < 9 do centro
    se alguma dessas n estações é usável pela empresa (station_usable_by 0x4c01b0): 0
    se n: v = v / (2n + 1)
    v = ftol((lugar.+0x36 + 500000) * v * 2e-6)                              ; +0x36 = bônus oferecido (world-tick.md 3.6)
```
`station_usable_by(st, c)` (`0x4c01b0`) = a estação é de `c` ou está na rede principal de `c` (`st.+0x57 == c.+0x17a`). Os campos `+0x12`/`+0x3a` (f32) e `+0x48` (u8) do lugar ainda não têm nome (Em aberto: provavelmente população/demanda e classe de tamanho). Teste `ai_town_value_company0_matches_original` (5000 casos) [V].

### 5.2 Expansão da rede (`ai_extend_network` `0x435320`) [S, D]
```
caixa' = ftol(caixa)  (conta 0)
com chairman: caixa' = ftol((issue_bonds + 200) * caixa * (1/300))           ; 0x535430
se caixa' < 400000: devolve 0
limite de estações (+0x172) por Track Expansion (0x5704ad): 0 -> > 15 devolve 0; 1 -> > 20; 2 -> > 24; outro valor: sem limite
escolhidas = []; viáveis = 0
para passada em 0..9:
    resultado[p] = 0
    se viáveis >= 2: continua                                  ; sem RNG
    melhor = 0; cidade = 0
    para cada lugar i (ordem da lista, town_nth) com +0x10 == 0 e não em escolhidas:
        w = ai_town_value(lugar, empresa)
        s = ftol(rng_range(80, 120) * 0.01 * w)                ; 1 sorteio por lugar elegível
        se +0x1aa <= 0: pula
        d = min sobre as estações k = 1..+0x1aa da rede principal (0x43bbc0) de int_dist(lugar.centro, estação.xy)   ; 0x4f0cd0
        se d > 60: se d > 120: pula;  s >>= 2;  se d > 90: s >>= 2
        nota = ftol(30.0 / (max(d, 8) + 13) * s)
        se nota > melhor: melhor = nota; cidade = lugar.id; s_melhor = s
    orçamento = ftol(7.0 / (+0x172 + 7) * ftol((caixa' + 2000000) * melhor * 4e-7))
    com chairman: orçamento = ftol((expand_rail + 900) * orçamento * 0.001)
    Track Expansion 1: orçamento = ftol(orçamento * 1.12);  2: ftol(orçamento * 1.25)
    se cidade != 0 e orçamento > 300000:
        resultado[p] = ai_connect_town(cidade, 1 (simulação), &custo[p])
        se custo[p]*1.15 + 150000 > caixa' ou custo[p] > orçamento: resultado[p] = 0
        se resultado[p]: viáveis++
        nota_p[p] = s_melhor
    escolhidas[p] = cidade                                     ; inclusive 0
k = argmax_p (resultado[p] != 0) de f32(nota_p[p] / custo[p])  ; estrito, começa em 0
se nenhum: +0x166 = 1; devolve 0
ai_connect_town(escolhidas[k], 0 (real), NULL)                 ; o resultado não é testado
se caixa' > 1000000: +0x166 = 1
devolve 1
```
Consumo de RNG: soma, nas passadas com menos de 2 viáveis, do número de cidades ainda não escolhidas. Confirmado no Example com o prazo forçado (`+0x17e = 0`): 68 cidades elegíveis, 10 passadas = 68+67+...+59 = **635 sorteios**, com a simulação de `ai_connect_town` na 3ª passada [D].

### 5.3 Primeira linha (`ai_plan_first_line` `0x435c20`, fastcall(simulação)) [S]
Usada quando a rede da empresa está vazia e, com empresa 0 e `simulação = 1`, por `ai_can_found_company` (9.1).
```
se versão >= 0x2333 e empresa != 0 e caixa < 350000 e atitude (+0x4276) < 0.7: devolve 0
n = min(lugares, 128)
para i em 0..n-1 (ordem da lista):
    se lugar.+0x10 == 0 e nenhuma estação a < 9 do centro (0x431280):
        v[i] = ftol(rng_range(70, 130) * 0.01 * (ai_town_value(lugar, empresa) >> 10))   ; 1 sorteio
    senão v[i] = 0
M[i][j] (i < j, v > 0 nos dois, |dx|+|dy| < 100): d = int_dist; se d > 12: (d < 25: d = d/4 + 15); se d < 75: M = v_i*v_j / (d*d)   ; inteiros
usadas[128] = 0; ativo[20] = ok[20] = 0
repete:
    cont = 0
    para slot em 0..19:
        se !ativo[slot]:
            se cont < 3:
                ativo[slot] = 1
                (a, b) = argmax M[a][b] com !usadas[a], !usadas[b], b > a (estrito, primeiro em ordem)
                se M <= 0: termina tudo (sem construir)
                ok[slot] = ai_build_line(a, b, 1, &custo[slot])          ; 5.5, simulação
                se ok: cont++; par[slot] = (a, b)
                se rng_range(1, 10) < 5: usadas[a] = 1 senão usadas[b] = 1  ; 1 sorteio por par tentado
            senão ativo[slot] = 0
        senão se ok[slot]: cont++
    k = argmax (ativo e ok) de f32(M[par] / custo)
    se k == -1: sai
    r = ai_build_line(a_k, b_k, simulação, NULL)
    se r: sai
    ok[k] = 0
até cont == 0
se simulação == 0 (empresa real):
    se +0x4247 == 0: +0x4247 = 1; se há chairman: (se r) company_name_from_towns(a, b) (0x43e040, RNG); notícia LNG 166 "%1 has just started the %2."
    senão se r: notícia LNG 240 "%1 has connected %2 to %3."
devolve r
```
`company_name_from_towns` (`0x43e040`): a cidade maior (classe `+0x2c`*500 + `+0x46`) vem primeiro; `rng_range(0, 100) > 34` -> "%1 & %2" se couber em 31 caracteres; senão nome direcional (LNG 2281..2284 Eastern/Western/Southern/Northern pela posição relativa ao centro do mapa) e, se ainda não servir, `rng_range(0, 100) < 40` -> "%1 Line" (2285) senão "%1 Central" (2286). 1 ou 2 sorteios.

### 5.4 Ligar uma cidade à rede (`ai_connect_town` `0x435060`, fastcall(lugar, simulação, &custo)) [S]
```
(tamanho, tamanho') = ai_station_size(lugar)                 ; 0x434ef0, abaixo
st = primeira estação a < 9 do centro (de qualquer dono), se houver
até 3 tentativas:
    alvo = tile com trilho da própria empresa, dentro da caixa da rede (+0x4233..+0x423f), que minimiza
           dx² + dy² até o centro do lugar (tiles com estação: d -= d >> 4), fora das caixas ±5 das tentativas falhas
    sem alvo: devolve 0
    sem st: r = ai_build_town_station(lugar, alvo, tamanho, tamanho, simulação, &custo)     ; 0x434a40
    com st: r = ai_connect_station(st.xy, alvo, simulação, &custo)                       ; 0x434940
    se r: devolve 1
    exclui caixa (alvo ± 5)
devolve 0
```
`ai_station_size` (`0x434ef0`): tamanho pela classe do lugar `+0x2c`: 0 -> pequena (0), 1..3 -> média (1), >= 4 -> grande (2); conta indústrias a menos de 15 (Manhattan) do centro com peso 3 (2 se tipo `0x12`) por raio de captação `{2,4,8}` (`station_radius` `0x54fd9c`); pequena vira média se `cont_média >= cont_pequena + 4`; média vira grande se `cont_grande >= cont_média + 7`.

`ai_connect_station` (`0x434940`): simulação = `track_pathfind` (modo de planejamento, `terrain-track.md` 2) devolvendo o custo; real = `track_lay_route(empresa, ..., elétrico = +0x4264, ponte = (max_bridge_type() >= 3) ? 3 : 1)`.

`ai_build_town_station` (`0x434a40`): duas orientações (eixo maior de `|dx|`,`|dy|` até o alvo, depois o outro); `ai_find_station_site` (`0x430fd0`, usa `rng_hash_range`, não o RNG do jogo) acha o lugar; a pegada da estação é marcada temporariamente (`tile+2 |= 0x10`) para o caminho não atravessá-la; simulação: custo do caminho + preço do prédio da estação; real: `track_lay_route`, `building_create` da estação, melhorias `ai_station_upgrades` (`0x434850`: correio/serviços conforme tamanho e ano < 1930) e notícia LNG 239 "%1 has connected to %2.".

### 5.5 Linha entre duas cidades (`ai_build_line` `0x435790`) [S]
Lugares de estação nas duas cidades voltados um para o outro (`ai_station_size`, `ai_find_station_site`); falha se os dois sítios ficam a menos de 8 tiles; marca as pegadas; simulação: `track_pathfind` + preço das duas estações; real: `track_lay_route` (elétrico `+0x4264`, ponte `max_bridge_type()`), duas estações com `building_create` e `ai_station_upgrades`.

**Pathfinding e custo:** a IA não tem custo próprio: usa `track_pathfind` (`0x460850`, A*, custo por tile `track_tile_cost` `0x463520`, `terrain-track.md` 2.2) e cobra pelo `track_lay_route`. As preferências do guia ("evita montanhas") vêm só desse custo.

## 6. Gerente (`ai_review_manager` `0x436880`, `ai_manager_value` `0x436210`) [S]
Uma vez por ano ímpar (no primeiro turno do ano). Gerentes: tabela `0x54fdad`, 40 registros de 0x11 bytes (`+0` salário i32, `+4` 4 pares `(efeito u8, pct i8)`); disponíveis se `0x573e09[i] in {0, empresa}` e `0x570638[i] != 0`.
```
melhor = argmax_i (ai_manager_value(i) - salário_i)    ; estrito, começa em -9999999
se há gerente atual g e (melhor_nota - ai_manager_value(g)) * 0.8 < salário_melhor * 0.4: mantém
senão se 0x573e09[melhor] != empresa: company_set_manager(empresa, melhor, 0)
```
`ai_manager_value(i)`: 0 se a empresa tem menos de 1 ano; senão, para cada um dos 4 pares, `p = f32(pct * 0.01)` e uma base do ano anterior (`cat` = categoria anual, `finance.md` 1; `trens` = `max(+0x27, 1)`; `n_t` = trens do tipo `t`, `0x437980`):

| Efeito | Valor |
|---|---|
| `0x3d` | `(cat1 + cat0) * p` |
| `0x3e`, `0x3c`, `0x0c`, `0x19`, `0x15`, `0x11` | `cat0`, `cat2`, `cat5`, `cat7`, `cat6`, `cat8` vezes `p` |
| `0x3f`; `0x50`, `0x59` | `cat1*0.4*p`; `cat1*0.03*p` |
| `0x0f`; `0x29`; `0x2e`; `0x23`; `0x27` | `cat14 * {0.01, 0.08, 0.05, 0.3, 0.03} * p` |
| `0x12`, `0x14` | `cat8 * n_vapor / trens * p`, `cat8 * n_elétr / trens * p` |
| `0x24`, `0x2b`; `0x31` | `cat14 * n_vapor * {0.3, 0.18} / trens * p`; `cat14 * n_elétr * 0.05 / trens * p` |
| `0x1f` | `ftol(conta6 * (-pct) * 0.004)` |
| `0x08`, `0x09`, `0x0a`, `0x0e` | `X = idade < 3 ? 300000 : ftol((conta3(ano-1) - conta3(ano-3)) * 1.4)`; `ftol(X*p*{0.3, 0.7, 0.2, 0.1})` |
| `0x1a..0x1d` | `f = idade < 5 ? 2.0 : max(trens/15, 1.0)`; `e = ai_pick_engine(1)`; `(frac, casa) = (1, 1)` para 0x1a, `(n_t/trens, tipo(e) == t)` para 0x1b..0x1d (t = vapor, diesel, elétrico); `e2 = ai_pick_engine(1)`; `ftol(preço(e2) * (casa*0.7 + frac*0.3) * f * p)` |
| outros | 0 |

Cada termo é `ftol`; positivo vira `(v >> 2) + (v >> 1)` (0,75), negativo `ftol(v * 1.25)`; soma dos 4. **RNG:** cada par 0x1a..0x1d de cada gerente avaliado chama `ai_pick_engine` duas vezes. `company_set_manager` (`0x438780`) regera (`person_generate`, RNG) os candidatos `+0x140/+0x144` de qualquer empresa que apontavam para o gerente contratado.

## 7. Trens (`ai_manage_trains` `0x4d9210`) [S, D]
```
laço:
  para cada estação (C) usável pela empresa:
      para c em 0..43: st.+0x3fd[c] = 0; pot = f32((i16 st.+0x61[c] * 1.5 + f32 st.+0xb9[c]) * 0.5); st.+0x34d[c] = pot
      se st é da empresa: própria += pot;  senão se st.+0x4b1 > 0: alheia += pot * (0.7 / st.+0x4b1)
  para cada trem (C) da empresa com >= 2 paradas (+0x3b):
      areia = óleo = água = 100, acelerador = 85; se +0xe9 == 0: +0xe9 = 2.0
      w = 1.0 / +0xe9
      para cada parada sem a flag 8: para cada vagão de carga c < 44 da parada: st.+0x3fd[c] += w; servido += w
  se própria <= 0: retorna
  razão (0x57951c) = max(servido / própria, 0.5)
  X = min(própria * 1.15, própria + alheia)
  A = min((X + 2) * 0.2, (+0x186 + 2*+0x18a) / 5)                 ; células simples + 2x duplas
  alvo = max(ftol(A * 220 / (min(ano, 2100) - 1650) + 0.5), 2, +0x172)
  se 0x5706b5 e +0x27 < alvo e ai_buy_train(): volta ao laço
  se +0x4245 != ano:
      primeiro trem da empresa (C) com ano - +0xdf > 6: +0xdf = ano; ai_replan_train(trem); volta ao laço
      se nenhum: +0x4245 = ano
  se +0x4243 != ano: +0x4243 = ano; para cada trem da empresa com ano - ano_de_compra(+0xa9) > 25 e caixa >= -100000:
      train_replace_engine(trem, ai_pick_engine(1), 0)               ; 0x4d4e90
  razão = 0
```
`ai_buy_train` (`0x4d90f0`): `a = ai_pick_stop(NULL)`, `b = ai_pick_stop(a)`; se ambas existem: `train_buy(ai_pick_engine(1), estação 0, empresa)` (`0x4cdf00` -> `train_create`), apaga a rota inicial, `+0x9d = 1`, `+0xa7 = 3` se as duas paradas são estações a mais de 80 tiles, senão 2; parada `b` recebe a flag `0x40000000`; insere as duas (`0x4d11f0`). `ai_replan_train` (`0x4d8f40`): o mesmo para um trem existente, exigindo caminho (`track_route_exists` `0x460370`) da frente do trem até a primeira parada.

### 7.1 Escolha de parada e composição (`ai_pick_stop` `0x4d87e0`, fastcall(&saída, anterior, modo)) [S]
```
repete:
  para cada estação (C), exceto a anterior, usável pela empresa (sem anterior: só estações próprias; pula +0x45 != 0):
      d = anterior ? int_dist(anterior, st) : 0
      se anterior e a 1ª carga da anterior não é passageiro/correio e d > 40: pula
      nota = Σ_c [pot = st.+0x34d[c] > 0 e u = (razão - st.+0x3fd[c]/pot) * pot > 0]:
             d == 0: u;  c in {0,1}: u * (d + 60) * 0.0125;  senão: u / ((d + 20) * 0.025)
      se st não é da empresa: nota *= (modo & 3) == 1 ? 0.4 : 0
      s = rng_range(75, 125) * nota * 0.01                     ; 1 sorteio por estação elegível
      se s > melhor: com anterior exige track_route_exists(st, anterior) e os limites do caminho (0x566a44/0x566a40); melhor = s
  se achou: sai
  se razão >= 5.0: devolve 0
  razão += 1.0                                                  ; repete o laço inteiro (e os sorteios)
vagões = ano < 1835 ? 2 : ano < 1850 ? 3 : 4
valor por carga (com anterior: c in {0,1}: u*(d+20)*0.025, senão u/((d+20)*0.025)); as 4 melhores em ordem decrescente
n_0 = vagões; para i em 1..: 2 rodadas: para j < i com n_i < n_j: se max(v_j/n_j, v_i/(n_i+2)) < max(v_i/(n_i+1), v_j/(n_j+1)): n_i++, n_j--
se n_1 == vagões: n_1 = 2;  senão se n_0 + n_1 == vagões == 4: n_0--
saída: estação, +2 = número de vagões, +4.. = cargas repetidas n_k vezes
```

## 8. Compra de indústria (`ai_industry_investment` `0x436950`) [S]
Só com >= 2 estações, modelo industrial expert (`0x573d78 == 2`), permissão `0x5706b9` e caixa >= 200000.
```
limiar = sem chairman ? 0.16 : (300 - industry_investment) * 0.0008
para cada registro de list_industries (padrão B):
    exige tipo comprável (0x535c39 + tipo*0x27 != 0), +0x4b > 107, caixa > preço (prédio, +0x14b), +0x97 == 0 (sem dono)
    r = f32(i16 +0x36 * 1000 / preço)
    se ligada a estação (+0x33): r *= (dona da estação == empresa) ? 1.1 : 0.7
    se r > limiar: limiar = r; escolhida = id
se escolhida: building_buy_request(empresa, id)               ; 0x43ea30: enfileira o comando 0x2d
```
Sem RNG.

## 9. Pessoas de IA (`ai_person_turn` `0x4a8800`) [S, D]

### 9.1 Fundar empresa
Só se a pessoa está ativa, `0x5706be`, (`0x5706c4` ou `+0x555 <= 0`), não é chairman (`+0x263 == 0`), `T - minuto_inicial > +0x267` e caixa > 80000:
```
f = f32(terra * (1/30000) + 2.0)            ; terra = tiles não-água (0x573e04, territories_recount_land 0x4c8970)
y = min(ano - ano_inicial(0x570508), 30)
q = f32(n_empresas / ((60 - y) * f * (1/60)) * (200 - starts_company) * 0.01)
s = rng_hash_range(+0x548, 80, 120) * q * 0.01      ; hash, NÃO consome o RNG do jogo
se s > 1.0 e 0x5706aa e 0x573d74: +0x54c = 1 (só investe); +0x267 += 483840 (1 ano)
senão se ai_can_found_company() == 0: +0x54c = 1; +0x267 += 2419200 (5 anos)
senão:
    c = ftol(caixa); se c > 3e6: c = (c - 3e6)/2 + 3e6;  C = (c/1000)*1000
    extra = min(max(1500000 - C, 0), 9C)
    nova = company_create(pessoa, -1, NULL, C + (extra/1000)*1000)     ; 0x4383f0 -> 0x438a10 (RNG)
    +0x54c = 0; event_fire_trigger(5); ai_company_turn(nova)           ; turno completo imediato
    ações = min(ftol((caixa + 8000) / preço_inicial * 0.001) * 1000, ações da empresa); caixa -= ações * preço_inicial
se chairman ou +0x54c: ai_person_trade_stocks(pessoa)
```
`ai_can_found_company` (`0x4361c0`) = `ai_plan_first_line(1)` com empresa 0 (consome RNG: seção 5.3). `company_init` (`0x438a10`) sorteia a cor/logotipo entre as livres (`rng_range(0, livres-1)`) e chama `person_generate` 3 vezes (gerente e dois candidatos).

### 9.2 Bolsa (`ai_person_trade_stocks` `0x4a8af0`) [S]
Laço guloso, um lote de 1000 ações por volta, até a melhor oportunidade valer < 1.15:
```
m = caixa > 100000 ? caixa * 1e-5 : 1.0;   L = valor das posições / patrimônio
para cada empresa ativa:
    justo = +0x427a (x1.1 se é a empresa da pessoa); B = é chairman de outra empresa e esta não é a sua
    compra (person_can_buy): a = justo / preço(+1000); B: a = (a-1)*investing*0.01 + 1
        se caixa < preço*1000: a = (a-1)*margin*0.005 + 1; se L > 1.25: a = (a-1)/L + 1
        se a > 0.95: a *= m
    venda (person_can_sell): v = preço(-1000) / justo; B: v = (v-1)*investing*0.005 + 1
        sem posição: v = (v-1)*short*0.005 + 1;  senão se v > 0.92: v = max(L' , 0.5)*(v - 0.9) + 0.9 com L' = L (<= 1.1) ou (L-1.1)*3+1.1
        se !B: v = posição < 1001 ? 0 : (v-1)*0.6 + 1
    melhor de todas as compras/vendas
se melhor >= 1.15: compra/vende 1000 (forçado) e repete; notícia LNG 2324 se envolveu a empresa do jogador
```
Sem RNG do jogo.

## 10. Finanças da empresa de IA (`company_ai_finance` `0x45fbf0`) [S]
Completa `finance.md` 9.1 (lá, o resumo; aqui os valores):
1. **Refinanciamento simulado:** para `k = 1..títulos`, numa cópia da empresa: quita os `k` de maior taxa e emite `k` de 500 000 a 30 anos; ganho = `Σ juros quitados - Σ juros novos - 8000k`; guarda o melhor `k` com ganho > 0. Restaura. Se `0x5706b0` e `0x5706af`: quita os `k` de maior taxa (a reemissão vem do passo 2).
2. **Emissão:** se `0x5706af`: enquanto caixa < -250000 e a emissão (500 000, 30 anos) for aceita: emite.
3. **Quitação:** se `0x5706b0` e `+0x166`: enquanto caixa > 600000 e há título com taxa >= 0.06 (o de maior taxa): quita.
4. **Recompra** (só com `+0x166`): `w = 1.0`, ou `(investing + 300) * 0.0025` com chairman; Dividend Payout 1: `w *= 1.6`; 2: sem recompra. Enquanto `0x5706ae`, caixa > `w*800000`, caixa > `preço * w * 1000 * 1.2` e float livre >= 1000: recompra 1000.
5. **Falência:** se nada foi feito em 1..4, `0x5706ac`, caixa < -300000, idade > 2 anos, nenhum dos últimos 3 anos com lucro > -20000 e última falência há mais de 4 anos: `company_declare_bankruptcy` e notícia LNG 182.
6. **Dividendo (dezembro):** se `0x5706b1`, mês 12, `ano > +0x16a` e idade >= 2: `+0x16a = ano`; `P = (3*lucro(ano) + 2*lucro(ano-1) + lucro(ano-2)) / 6`; float livre <= 1000: `D = (max(caixa,0)/3 + P/3) / ações`, senão `D = P*0.1/ações`; `D < 0.1`: dividendo 0; senão `ftol((antigo + D + 0.1) * 0.5 * 10) * 0.1`; teto `company_max_dividend` (`finance.md` 6.1).
7. **Política de dividendos, a cada turno:** Dividend Payout 1 -> `+0x30 *= 0.66`; 2 -> `+0x30 = 0`.

## 11. Ordem das chamadas do RNG no turno da IA

Local = endereço de retorno da chamada a `rng_range` (`0x4f0660`). Tudo dentro de `sim_month_end`, passo 7, na ordem abaixo; repetições conforme o estado.

| # | Local (retorno) | Função | Faixa | Quantas |
|---|---|---|---|---|
| P1 | `0x435cf0` | `ai_plan_first_line(1)` via `ai_can_found_company`, por pessoa IA que tenta fundar | `[70,130]` | 1 por lugar elegível (<= 128) |
| P2 | `0x435f9f` | idem | `[1,10]` | 1 por par tentado |
| P3 | `0x438c7d`, `person_generate` (`0x438553`...) | `company_init` da nova empresa | `[0, livres-1]`, ... | se fundou |
| P4 | regras (`0x454db0`) | `event_fire_trigger(5)` | variável | se fundou |
| P5 | todo o turno da empresa nova (C1..C9) | `ai_company_turn(nova)` | | se fundou |
| C1 | `0x4d878a` (x2 por par) | `ai_manager_value` -> `ai_pick_engine` | `[80,120]` | ano ímpar, 1º turno do ano: 2 x candidatos por par de efeito 0x1a..0x1d de cada gerente |
| C1' | `person_generate` | `company_set_manager` | | se contratou |
| C2 | `0x4d878a` | `ai_pick_engine(1)` em `0x436b95` | `[80,120]` | **sempre** (empresa ativa): 1 por motor candidato |
| C3 | `0x436c0b` | prazo de expansão | `[180,360]` | 1 se o prazo venceu |
| C4a | `0x435cf0`, `0x435f9f` | `ai_plan_first_line(0)` | como P1/P2 | rede vazia |
| C4b | `0x435464` | `ai_extend_network` | `[80,120]` | 1 por cidade elegível não escolhida, por passada com < 2 viáveis |
| C4c | `0x43e0d9`, `0x43e244` | `company_name_from_towns` | `[0,100]` | 1 ou 2, primeira linha com chairman |
| C4d | `0x4259e0`..., `0x40bb40` (`0x40bc34`), `building_construct` | construção real (trilho, estações) | | conforme os tiles (`terrain-track.md`, `world-tick.md` 4) |
| C5 | `0x4d8993` | `ai_pick_stop` (compra/replanejamento) | `[75,125]` | 1 por estação elegível, por iteração da razão (até 5) |
| C6 | `0x4d878a` | `ai_pick_engine(1)` em `0x4d9125` / `0x4d9643` | `[80,120]` | por trem comprado / por trem > 25 anos (1x por ano) |
| C7 | (nenhum) | `ai_industry_investment`, `company_ai_finance` | | |

`ai_person_turn` só consome o RNG do jogo pelos itens P1..P5 (o teste de fundar usa `rng_hash_range`); `ai_person_trade_stocks` não usa RNG. **Ordem global:** primeiro todas as pessoas IA (P), na ordem da lista; depois todas as empresas (C), na ordem da lista.

## 12. Confirmação dinâmica [D]
`trace_save.py` com a lista de funções da IA e `rng_range` registrado (cópia instrumentada no scratchpad que também despeja as permissões e os campos das empresas):
- **Example, 120 dias** (10/dez/1875 -> 8/abr/1876, semente 1234): 4 turnos mensais (dez, jan, fev, mar). Em cada um: `ai_person_turn(2)` -> `ai_person_trade_stocks(2)` (pessoa 2 é chairman da empresa 5; nenhuma ordem), `ai_company_turn(2)` (inativa, retorna sem nada), `ai_company_turn(5)` -> `ai_pick_engine` (**4 sorteios** em `0x4d878a`) -> `company_ai_finance(5)`; `ai_manage_trains` só em dezembro (84 dias desde `+0x154`), sem compra (trens >= alvo), sem RNG. A empresa 4 (chairman humano) não tem turno. O prazo `+0x17e` da empresa 5 vence só em 1877, então não houve expansão. RNG da IA: 16 nos turnos mensais + 4 do `ai_pick_engine(1)` anual. Permissões `0x5706a8..0x5706c7` todas 1; `0x5704ac..af` = 0 (Track Expansion 0: limite de 15 estações; a empresa tem 5).
- **Example, 30 dias com `+0x17e = 0` forçado:** `ai_pick_engine` (4) -> `rng(180,360)` -> `ai_extend_network` com **635** sorteios em `0x435464` (68 cidades, 10 passadas, exclusão confirmada) e uma simulação `ai_connect_town` -> `ai_build_town_station` -> `ai_find_station_site` na 3ª passada; nenhuma opção viável -> `+0x166 = 1`, `+0x17e = T + atraso + 60 dias` (atraso compatível com Hudson, Expand Rail 70), e as finanças passam a quitar títulos (caixa 1 199 028 -> 706 523).

## 13. Divergências e notas de fidelidade
- Manual/guia: "100 é a média, 200 o máximo, abaixo de 100 raramente usada" é aproximação: as tendências entram linearmente (`(300 - x)`, `(x + 200)`, `(200 - x)`, `(x + 900)`, `(x + 300)`, `(x-1)*x*0.005`), sem limiar em 100.
- Guia: "IA expande em áreas planas e evita montanhas" — não há regra própria: é o custo do A* de trilho (`terrain-track.md`).
- Manual: dificuldade muda o "modificador do computador"; o código da IA não lê nível de dificuldade (só as opções Track Expansion e Dividend Payout).
- `0x5704ae == 1` multiplica o dividendo por 0,66 **a cada turno mensal** (não só na revisão de dezembro): o dividendo decai ao longo do ano. É o código; o clone deve reproduzir.
- `ai_extend_network` não confere o resultado da construção real e ainda devolve 1.
- `ai_pick_engine` devolve 0 (Stephenson Rocket) quando não há motor.

## Em aberto
- Nomes de interface das permissões `0x5706aa..0x5706c4` (seção 1); papel de `+0x555` da pessoa e de `0x5706c4`.
- Campos do lugar `+0x12`, `+0x3a` (f32) e `+0x48` (u8) usados em 5.1; campos da estação `+0x61`, `+0xb9`, `+0x34d`, `+0x3fd`, `+0x4b1` (oferta/demanda e serviço) e do trem `+0xe9`, `+0xdf`, `+0xa7`, `+0x9d`.
- Semântica de `0x566a40`/`0x566a44` (saídas de `track_pathfind` usadas no filtro de 7.1).
- `ai_find_station_site` (`0x430fd0`), `ai_station_upgrades` (`0x434850`), `train_replace_engine` (`0x4d4e90`), `building_buy_request` (`0x43ea30`) e `company_create` (`0x4383f0`/`0x438a10`) só descritos em linhas gerais.
- Números de RNG da construção real (C4d) não foram observados (a expansão forçada não achou opção viável).
- `0x48cc00` (3 sorteios) aparece na árvore de chamadas só através de rotinas de interface; não é chamado pela IA diretamente.
- O teste de `ai_town_value` cobre só empresa 0 (sem listas de estações e territórios).
