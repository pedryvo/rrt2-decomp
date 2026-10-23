# Comandos do jogador e da rede (`cmd_execute` `0x4b0210`)

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido no C exportado (`re/db/decomp`) e no disassembly (rizin); sem sessão REA. Endereços são VAs. Rótulos de prova: **[V]** = comparado com o original no emulador (`crates/rt2-oracle/tests/commands.rs`); **[S]** = lido no código; **[D]** = lido do `.data`/`.rdata`; **[?]** = hipótese. Textos do jogo citados vêm de `default2.lng` (id decimal).

Objetivo: a lista completa das ações do jogador (e da rede) que mudam o estado da simulação, com o pacote de cada uma, e a resposta, vinda do executável, para "o que o jogador pode construir e comprar". Guia de quem implementa o `rt2-sim`: a fila e os pacotes (seção 1), a tabela (seção 2) e as regras de elegibilidade (seção 3).

## 1. A fila de comandos

### 1.1 Pacote

Toda ação de jogo é um **pacote** `P` (bloco de bytes). O construtor é `cmd_packet_init` (`0x4af890`, `fastcall(ECX = buf, EDX = id, [esp+4] = tamanho)`) [V]: zera `tamanho` bytes e grava `+0 = id` (u32) e `+4 = tamanho` (u32, bytes do pacote inteiro). O **corpo começa em `+0xc`**; `+8` (u32) é o **minuto em que o comando vale** e só é preenchido pela rede (em jogo local fica 0 e `cmd_queue_next_time` ignora o campo). Os campos abaixo são offsets a partir do início do pacote, little-endian, sem alinhamento.

Quem enfileira é `cmd_send` (`0x4af6d0`, `fastcall(ECX = pacote)`, quase todos os 35 chamadores via thunk `0x4025f4`) [S]:

```
cmd_send(P):
    se jogadores < 2 (jogo local):
        se P.id == 0xb e 0x578b04 != 0:  cmd_execute(P); sim_advance(0); retorna      ; velocidade já, sem fila
        copia P (malloc(P.size)) para o fim de cmd_queue (0x574c8c);  sim_advance(0)
    senão (rede):
        cliente: manda ao host (0x4a0410(7, tempo, P, P.size)); host: copia para a lista de saída 0x574c94
```

`cmd_queue_run` (`0x4af3d0`, `time.md` 5.1) executa a fila em ordem de chegada com `cmd_execute`; em rede só os pacotes com `+8 <= game_time_minutes`. Em jogo local `sim_advance(0)` (chamado dentro de `cmd_send`) roda a fila antes de qualquer avanço de tempo, então **um comando local executa no mesmo instante de jogo em que o jogador o emitiu, antes do próximo passo da simulação** (`world-tick.md` 2). A IA **não** usa a fila, com uma exceção: `building_buy_request` (`0x43ea30`, chamada por `ai_buy_industry` `0x436950`) enfileira o `0x2d`; todo o resto da IA chama as primitivas direto (`ai.md`).

**Formato de rede** (`0x4af8c0`, buffer de 5000 bytes) [S]: por comando, 3 bytes `(id u8, len u16 = size - 9)` seguidos dos bytes de `+0xc` até o fim do pacote; o receptor reconstrói `+4 = len + 9` e carimbo `+8`. Por isso todo id cabe num byte.

### 1.2 Regras gerais de `cmd_execute`

- **Contador**: `0x57fee0` (nível de "UI ocupada") sobe no começo e desce no fim; o final chama `0x502200(1, 0)` (atualização de janelas). Nenhum efeito de simulação.
- **`cmd_execute` não valida permissões, caixa nem disponibilidade por ano.** Todas as restrições de cenário (seção 3.9), de dinheiro (`caixa >= preço`), de território e de ano/região vivem **na janela que monta o pacote**; o executor só valida **ids vivos** (bitmap de livres da lista, campo `+0x34` do cabeçalho do pool, e `id < +0x10`) nos comandos de trem, estação, indústria e empresa, e a geometria em `0x17`. Um peer ou um replay que mande um pacote "ilegal" executa como se fosse legal.
- Ids tratados: **1 e `0xa..0x2f`** (39 ids; `0xc` e `0xd` não têm emissor em nenhuma janela). Ids `2..9` existem na rede (emitidos pelo lobby, seção 2.3) mas `cmd_execute` os ignora (caso `default`); quem os consome é o código de rede (`0x4b75b0` etc.) [S].
- Texto de tela, sons e janelas que o executor mexe **só para a empresa local** (`P.empresa == DAT_00574414`) são omitidos da tabela, exceto quando mudam estado de simulação (marcados com **estado local** na tabela).
- Contas e categorias do razão: `finance.md` 1 (`company_account_move` `0x4398d0`: conta 0 = caixa; conta `k != 0` faz `conta[k] += x; caixa -= x`; `company_ledger_credit` `0x439970`: categoria `c`, `cat[c] += x`, caixa `+= x`).

## 2. Tabela dos comandos

Legenda: `+N` = offset no pacote; `u8/i8/u16/i16/u32/i32/f32`; "empresa" = id da empresa (byte com sinal no executor, sempre >= 0); "pessoa" = id da pessoa (jogador ou IA); `P.*` = campo do pacote; `T` = `game_time_minutes`. "Emissor" = quem monta e enfileira o pacote (função: janela). O tamanho (`size`) é o valor passado a `cmd_packet_init` pelo emissor [V, `commands.rs`].

### 2.1 Ações de jogo

| id | nome | size | corpo (a partir de `+0xc`) | executor e efeito | emissor (UI) |
|---|---|---|---|---|---|
| `0x0a` | `cmd_bulldoze` | `0x13` | `+0xc` empresa u8; `+0xd` x i16; `+0xf` y i16; `+0x11` raio i16 | `bulldoze_area(empresa, x, y, raio, só_custo = 0, 0, 0, mensagem = 1, 0)` `0x428930` (`terrain-track.md` 2.4: duas passadas, 1000 por tile de trilho/árvore, prédios 0,1x próprio, razão 3/2/5). Se a empresa é a local e `DAT_00574414 == P.empresa` toca `bulld61m.raw` e `0x45cd60` (atualiza o mapa) quando `bulldoze_area` devolve != 0. | `0x48eb30` (clique, ferramenta 8, escavadeira) e `0x48d900` (pré-visualização arrastando, ferramenta 8); `+0x11` = `raio_tabela[DAT_005534c4]` com `{0, 2, 5, 11, 20}` em `0x53656c` [D] |
| `0x0b` | `cmd_set_speed` | `0xe` | `+0xc` i8 delta ou `0x7f` (absoluto); `+0xd` i8 valor absoluto | `v = (P+0xc == 0x7f) ? i8(P+0xd) : i8(P+0xc) + velocidade (0x57050e)`; `v = min(max(v, 0), 10)`; se mudou: som `GAMES61m.raw`; se a velocidade atual != 0 guarda `_game_speed_resume`; grava `0x57050e = v` e mostra texto (LNG 991..1001 `Paused,1..10`). `time.md` 3. | teclado (`0x4531f0`, atalhos de velocidade: delta `±1` ou `0xff`), controle deslizante da janela principal (`0x47beb6`, absoluto `10 - posição`), `settings.win` (`0x4b1c30`, absoluto), acidente de trem (`0x4ce9ab`: `train_crash` força `0x7f, 4` se a velocidade > 4), rede (`0x49e046` no início do jogo em rede, `0x4a7b9c` depois do aviso do corretor se a velocidade > 4) |
| `0x0c` | `cmd_account_move` | (sem emissor) | `+0xc` empresa i8; `+0xd` valor i32 | `company_account_move(empresa, conta 0, (f64) valor)` `0x4398d0` (caixa e patrimônio) e atualiza a UI. **Sem emissor** em nenhuma janela: comando de depuração/rede. | nenhum [?] |
| `0x0d` | `cmd_person_cash` | (sem emissor) | `+0xc` pessoa i8; `+0xd` valor i32 | `person_cash_add(pessoa, valor)` `0x4a7570` (`pessoa+0x12a += valor`). Sem emissor. | nenhum [?] |
| `0x0e` | `cmd_stock_trade` | `0x10` | `+0xc` pessoa u8; `+0xd` empresa u8; `+0xe` i8 (> 0 compra, <= 0 venda); `+0xf` u8 bandeira: **margem** na compra, **venda a descoberto** na venda (`finance.md` 18) | valida `empresa < +0x10` da lista e não livre e `empresa+0x16e != 0`. `n = person_trade_lot(pessoa, índice_da_empresa, compra)` `0x4a6e00` (lote padrão, múltiplos de 1000); compra: `person_buy_stock(pessoa, idx, n, P+0xf)` `0x4a7300`; venda: `person_sell_stock(pessoa, idx, -n, P+0xf)` `0x4a73d0`. Corretagem 3% (`finance.md` 8). | `stockdtl.win`: `0x4c56e0` (comprar, `+0xe = 1`, `0x4c58c0`) e `0x4c57d0` (vender, `+0xe = 0xff`); sons `stock61m`/`STOCS61m` |
| `0x0f` | `cmd_resign_chairman` | `0xe` | `+0xc` pessoa i8; `+0xd` empresa u8 | `company_resign_chairman(empresa(+0xd), pessoa(+0xc))` `0x43bf80`: se a pessoa é a chairman, limpa o cargo. | `compndtl.win` `0x4496e0` (botão de renunciar, depois da confirmação LNG 362 `0x16a`); permissão `0x5706bc` |
| `0x10` | `cmd_set_manager` | `0xf` | `+0xc` empresa u8; `+0xd` gerente i16 (índice 0..39 da tabela `0x54fdad`) | `company_set_manager(empresa, gerente, cobrar = 1)` `0x438780` (`finance.md` 14.3: taxa = `salário*efeito(0x10)/2` arredondada a 1000, categoria 11) | `compndtl.win`, diálogo de gerentes (`0x448600..0x448d36`): `+0xd` = índice do gerente escolhido na lista da janela |
| `0x11` | `cmd_stock_issue` | `0x11` | `+0xc` empresa u8; `+0xd` n i32 (`> 0` emitir, `<= 0` recomprar `-n`) | `n > 0`: `company_issue_stock(empresa, n, 0)` `0x439060` (preço `trade_price(-0,7n)`, taxa 5% na categoria 12); `n <= 0`: `company_buy_back_stock(empresa, -n)` `0x438f70` (preço `trade_price(0,7n)`, corretagem 3%). `finance.md` 8, 9 | emitir: `0x447730` (lote `max(1000, (ações/20000)*1000)` = 5%; exige permissão `0x5706ad`, preço >= 5 e >= 1 ano desde a última emissão `+0x160`); recomprar: `0x447bb0` (permissão `0x5706ae`, envia `-n`) |
| `0x12` | `cmd_set_dividend` | `0x11` | `+0xc` empresa u8; `+0xd` dividendo f32 | `company_set_dividend(empresa, f32)` `0x43bfd0` (`empresa+0x30`) | `compndtl.win` botão `0xa8f` (`0x4496e0`, valor do deslizante `DAT_00565164`); permissão `0x5706b1` |
| `0x13` | `cmd_bankruptcy` | `0xd` | `+0xc` empresa u8 | `company_declare_bankruptcy(empresa)` `0x43bc30` (`finance.md` 15) | `compndtl.win` `0x4496e0` (confirmação LNG 366 `0x16e`); permissão `0x5706ac` |
| `0x14` | `cmd_bond_issue` | `0x12` | `+0xc` empresa u8; `+0xd` principal i32; `+0x11` prazo i8 (anos) | `company_issue_bond(empresa, principal, prazo, taxa = 1)` `0x439320` (rating >= 5,0, no máximo 20 títulos). **A UI só envia 500 000 a 30 anos** (`+0xd = 500000`, `+0x11 = 0x1e`). | `0x448250` (permissão `0x5706af`) |
| `0x15` | `cmd_bond_repay` | `0xf` | `+0xc` empresa u8; `+0xd` título i16 | `company_repay_bond(empresa, título, multa = 1)` `0x4391b0` (2% de antecipação) | `0x4480f0` (permissão `0x5706b0`) |
| `0x16` | `cmd_lay_track` | `0x1a` | `+0xc` empresa u8; `+0xd` x1 i16; `+0xf` y1 i16; `+0x11` x2 i16; `+0x13` y2 i16; `+0x15` d1 i16; `+0x17` d2 i16; `+0x19` bandeiras u8: bit 0 pré-visualização (sempre 0 no pacote), bit 1 via **simples**, bit 2 **eletrificada**, bits 3..5 **tipo de ponte** (0..7), bit 6 `DAT_005794f4` | `build_preview_suspend` `0x46b150`; `DAT_005794f4 = bit 6`; `track_lay_route(empresa, x1, y1, x2, y2, d1, d2, bit0, bit1, bit2, bits3..5)` `0x4cc060` (pathfinding, terraplenagem, razão 3; `terrain-track.md` 2); depois zera `DAT_005794f4`, `0x57b3b8`, `0x57b3e0`, redesenha a área `[x1-3, y1-3, x2+3, y2+3]` e `build_preview_resume` `0x46b290`. | `track_lay_command` `0x4cbf60`: `0x48d900` (arrasto, ferramenta 4), `0x495ea0` (ferramenta de trilho, confirmação), `0x495d50`; permissão `0x5706b6`; direitos de território `& 2` (LNG 2266, `0x495ea0`) |
| `0x17` | `cmd_build` | `0x13` | `+0xc` empresa u8; `+0xd` x i16; `+0xf` y i16; `+0x11` forma u8 (índice 0..121 de `building_shape_table`); `+0x12` estilo i8 (-1 = herdar da cidade) | `build_preview_suspend`; `ok = station_placement_valid(x, y, forma, empresa, early = 1, 0, 1, 0, 0, 0)` `0x4087a0` (seção 3.3); se `ok`: `id = building_create(x, y, forma, 0, empresa, 0, estilo)` `0x409010` -> `0x409a30` (estação: razão conta 1, trilho sob a estação conta 3; ver 3.3); `build_preview_resume`. **Estado local** (só `P.empresa == DAT_00574414`, forma de tipo 0): abre a estação na tela `2` e **apaga o bit do tamanho** `estação+0x49 &= ~(1 << tamanho)` (seção 3.3). | `0x48eb30` (clique; ferramentas `0xe` estação e `7` editor): estação pela barra de ferramentas (botão `0xd0`, `bpaddstn.win`); `7` só nos editores |
| `0x18` | `cmd_buy_improvements` | `0x12` | `+0xc` estação u16; `+0xe` máscara u32 (bits = linhas de `improvement_table` `0x5500c8`) | `station_buy_improvements(estação, máscara)` `0x46aef0` (`terrain-track.md` 3.4: preço acumulado com `ftol`, razão conta 1, categoria 4, `estação+0x49 \|= máscara`) | `statndtl.win` `0x4c2050` botão `0x25a` (abre `buildbuy.win` por `0x407e80`; só envia se a janela confirma `preço <= caixa`); permissão `0x5706b8` |
| `0x19` | `cmd_buy_train` | `0x12` | `+0xc` empresa u8; `+0xd` estação i16 (0 = nenhuma); `+0xf` motor u8 (0..65); `+0x10` trem-modelo u16 (cópia de paradas, 0 = nenhum) | `id = train_buy(motor, estação, empresa)` `0x4cdf00` (exige >= 2 estações no mundo; `train_create` `0x4cdf80`: preço do motor, razão conta 4 e categoria 6) ; `0x4d3f40(trem(id), +0x10)` copia as paradas do trem-modelo (ou cria 2 paradas padrão). **Estado local**: abre a tela do trem novo (`0x40de20(3, 0)`, `DAT_00566c9c = id`, `DAT_005742c8 = 1`). | `trainbuy.win` `0x4d9d20` (dentro de `0x4019ba`), botão da barra "trens" (`0x47b35a`); permissão `0x5706b5` |
| `0x1a` | `cmd_train_goto_stop` | `0xf` | `+0xc` trem u16; `+0xe` parada u8 | se o trem existe: `train_set_stop(trem, parada, refresh = 1)` `0x4d1630`. **Estado local**: toca `nexts61m.raw` se o trem é o selecionado. | `traindtl.win` `0x4dea00` (ids `0x797c..0x79df` = linhas de paradas, `P+0xe = id - 0x7c`; `0x4deda8`) |
| `0x1b` | `cmd_train_stop_wait` | `0x10` | `+0xc` trem u16; `+0xe` parada u8; `+0xf` modo i8 (0 não espera, 1 meia carga, 2 carga cheia; LNG 1114..1116) | se `parada < trem+0x3b`: `train_set_stop_wait(trem, parada, modo)` `0x4d5830`: `parada.+3 = (parada.+3 & 0x3f) \| modo << 6`. | `traindtl.win` `0x4dea00` (`0x4df444`) |
| `0x1c` | `cmd_train_stop_cargo` | `0x15` | `+0xc` trem u16; `+0xe` parada u8 (`0xff` = todas); `+0xf..+0x14` 6 x u8 tipo de carga por vagão (`0xff` = vazio) | se `parada == 0xff` ou `< trem+0x3b`: `n = nº de bytes != 0xff`; grava `parada.+2 = n` e `parada.+4..+9 = cargas` (em todas as paradas se `0xff`); se o trem está trabalhando numa estação (`0x4d4750`) e a parada atual mudou: `trem.+0x71 = 0` e `train_arrive(trem)` `0x4d29e0`. **Estado local**: atualiza a janela. | `traindtl.win` `0x4dea00` (vários), `0x4dc9b0`, `0x4da5d0`, `0x4dc0d0` (editor de vagões), `0x4dd6c0` |
| `0x1d` | `cmd_train_insert_stop` | `0x15` | `+0xc` trem u16; `+0xe` posição u8; `+0xf` estação u16 (0 = ponto de passagem); `+0x11` u16: bit 0 = ponto de passagem, bits 1..15 = x; `+0x13` y u16 | `train_insert_stop(trem, pos, null, estação, bit0, x, y)` `0x4d11f0` (insere o registro de 0xe bytes; `0x4d1130` recalcula) | `traindtl.win` `0x4dd6c0` (adicionar parada: usa a estação selecionada `0x55c698`/`0x55c6a0` ou o ponto clicado, sentinela `0x3039` = ponto de passagem) |
| `0x1e` | `cmd_train_delete_stop` | `0xf` | `+0xc` trem u16; `+0xe` parada u8 | se `parada < trem+0x3b`: `train_delete_stop(trem, parada)` `0x4d5710` | `traindtl.win` `0x4dea00` (`0x4df4dc`) |
| `0x1f` | `cmd_train_sell` | `0xe` | `+0xc` trem u16 | se o trem existe: `train_delete(id)` `0x4cdd50` (**sem reembolso**; `terrain-track.md`) | `traindtl.win` `0x4dea00` (`0x4df351`) e `trainlst.win` `0x4e1a50` (`0x4e1aa8`, ids `9000..10999`); a janela só deixa se `0x4d5e70`/`0x4d5f70` aprovam (permissão `0x5706b4`) |
| `0x20` | `cmd_replace_engine` | `0xf` | `+0xc` trem u16; `+0xe` motor u8 | se o trem existe: `train_replace_engine(trem, motor, 0)` `0x4d4e90` (sem movimento de caixa além do razão do `train_create`); **estado local**: recarrega a tela `0x4dc0d0` | `trainbuy.win` `0x4d9d20` (modo "substituir", `0x4da162`) |
| `0x21` | `cmd_train_class` | `0xf` | `+0xc` trem u16; `+0xe` classe i8 (0 Halted, 1 Slow, 2 Normal, 3 Express) | se o trem existe: `train_set_class(trem, classe)` `0x4d59b0` | lista de trens da janela principal, ids `7000..8999` (`0x47b7c9`: `classe = (atual + 1) & 3`) |
| `0x22` | `cmd_merger_propose` | `0x10` | `+0xc` empresa compradora u8; `+0xd` empresa alvo u8; `+0xe` oferta u16 (US$ por ação) | executor `0x48b2f0`: se as duas estão ativas: `merger_propose(compradora, alvo, oferta)` `0x48b4f0` (`finance.md` 15); se o jogador local tem ações do alvo abre o diálogo de voto e envia o `0x23` | `compndtl.win` `0x48b170` (botão de fusão; permissão **`0x5706b4`**, não `0x5706bd`: ver seção 4); exige >= 2 empresas e 1 ano desde a última tentativa fracassada (`empresa+0x14c`, `0x761ff` min) |
| `0x23` | `cmd_merger_vote` | `0x12` | `+0xc` proposta u32; `+0x10` pessoa u8; `+0x11` voto u8 (1 = sim) | executor `0x48b4b0`: grava o voto da pessoa na tabela de votos (`0x4a75e0`: `+10+i = 1`, `+0x2a+i = voto`) e apura (`0x4015a0`) | `0x48b2f0` (diálogo sim/não LNG 164) |
| `0x24` | `cmd_election_start` | `0xe` | `+0xc` empresa u8; `+0xd` desafiante (pessoa) u8 | executor `0x42eb40`: `chairman_election_start(empresa, desafiante)` `0x42ed10`; abre diálogo de voto para o jogador local acionista (envia o `0x25`) | `compndtl.win` `0x42eb00` (via `0x4496e0`); permissões `0x5706bb`/`0x5706bc` (`0x443ab0`) |
| `0x25` | `cmd_election_vote` | `0x12` | `+0xc` eleição u32; `+0x10` pessoa u8; `+0x11` voto u8 | executor `0x42ecd0`: grava o voto (`+8+i`, `+0x28+i`) e apura `0x42ee00` | `0x42eb40` (diálogo LNG 165) |
| `0x26` | `cmd_found_company` | `0x35` | `+0xc` pessoa i8; `+0xd` nome char[31]; `+0x2c` logo/cor u8; `+0x2d` capital i32 (soma de dois valores da janela); `+0x31` investimento do fundador i32 (o executor sobrescreve com `ftol` do custo das ações) | `cmd_found_company` `0x4bdcd0`: se a pessoa já era chairman de outra empresa, `0x438900`; `pessoa+0x555 += 1`; `0x4383f0(pessoa, logo, nome, capital)` cria a empresa; ações do fundador `= min(((ftol(..) + 500) / 1000) * 1000, ações da empresa)`; `pessoa.+0x1d3[idx] = ações`; `pessoa.+0x12a -= P+0x31`; `event_fire_trigger(5)` (empresa criada). `finance.md` | `0x4bd950` (diálogo de fundação, `company_found_precheck`; permissão `0x5706be`; ids de pacote `0x26` ao criar, `0x27` ao editar) |
| `0x27` | `cmd_set_company_info` | `0x2d` | `+0xc` empresa u8; `+0xd` nome char[31]; `+0x2c` logo/cor u8 | executor `0x4bddb0`: `empresa+0x2b = P+0x2c`; copia o nome para `empresa+4`; **estado local**: redesenha | `0x4bd950` (editar a empresa existente) |
| `0x28` | `cmd_rename_station` | `0x2d` | `+0xc` estação u16; `+0xe` nome char[31] (30 + NUL) | se a estação existe (bitmap da lista `0x574c60`): copia o nome para `estação+0x1a` [V] | `statndtl.win` `0x4c2050` (caixa de texto `0x259`, `_strncpy(.., 0x1e)`) |
| `0x29` | `cmd_train_throttle` | `0xf` | `+0xc` trem u16; `+0xe` acelerador u8 | `train_set_throttle(trem, (f32) P+0xe)` `0x4d6080`: `trem+0xdb = v`; se `v <= 0x537814` `trem+0xa8 = 0`; senão `+0xa8 = classe (+0xa7)` (+4 em trilho próprio). **Estado local**: som `0x96, 0x30c`. | `traindtl.win` `0x4dea00` (`0x4def45`, deslizante); permissão `0x5706b2` (`0x4de210`) |
| `0x2a` | `cmd_buy_territory_rights` | `0x13` | `+0xc` empresa u8; `+0xd` território u8; `+0xe` máscara u8 (bit 0 trens, 1 trilho, 2 prédios, 3 estações); `+0xf` preço i32 | executor `0x43e8e0`: para cada bit da máscara `territory_set_rights(território, bit, 1, empresa, 0)` `0x4c8540`; `empresa+0x4250 = 0`; `conta 2 += preço`, `caixa -= preço` (`+0x4776`/`+0x4766`) | `0x441410` (consultoria de território, botões `0x11f9..0x11fc` -> bit `0..3`), `0x43e760` valida o caixa; ver 3.8 |
| `0x2b` | `cmd_seed_rng` | `0x10` | `+0xc` semente u32 | `0x578af8 = 0`; `rng_seed(semente)` [V] | rede: `0x49de20` (host, `crt_rand_range(1, 100000)`) |
| `0x2c` | `cmd_train_mark` | `0xe` | `+0xc` trem u16 | se o trem existe: `trem+0x6d = T`; `trem+0x9d = 1` [V] | `traindtl.win` (depois de editar paradas/vagões: `0x4da611`, `0x4dc1aa`, `0x4df675`, `0x4df83d`) |
| `0x2d` | `cmd_trade_industry` | `0x12` | `+0xc` empresa u8; `+0xd` indústria u32 (id da lista `0x574c74`); `+0x11` compra u8 (1 compra, 0 venda) | se `id < +0x10` e não livre: `company_industry_trade(empresa, id, compra)` `0x43eb60` (seção 3.7) | `0x43ea30` (compra, `building_buy_request`), `0x43eb20` (venda) a partir de `0x4e2030` (diálogo da indústria, botões `0x81`/`0x82`) e da IA (`0x436950`) |
| `0x2e` | `cmd_chairman_takeover` | `0xe` | `+0xc` pessoa u8; `+0xd` empresa u8 | `company_chairman_takeover(empresa(+0xd), pessoa(+0xc))` `0x43c050` (`finance.md` 15.4) | `compndtl.win` `0x4496e0` (`0x44a0b0`, "assumir a presidência", depois de um diálogo de confirmação; `+0xc = 0x566ca4` pessoa local, `+0xd` empresa mostrada); permissão `0x5706bb` |
| `0x2f` | `cmd_train_stop_sell` | `0x10` | `+0xc` trem u16; `+0xe` parada u8; `+0xf` modo u8 (0..3; LNG 3305..3308) | se `parada < trem+0x3b`: `train_set_stop_sell(trem, parada, modo)` `0x4d58e0`: `parada.+3 = (modo & 3) << 4 \| (parada.+3 & 0xcf)`; com `0x570229` (sandbox) o modo vira 3 | `traindtl.win` `0x4dea00` (ids `0x8020..0x8083`, `0x4deae3`) |

### 2.2 Comandos de rede e de sistema

| id | nome | size | corpo | efeito | emissor |
|---|---|---|---|---|---|
| `0x01` | `cmd_player_left` | `0x10` | `+0xc` id de rede do jogador u32 | `net_player_left(id)` `0x49d970`: remove o registro do jogador da tabela de rede (registros de `0x147` bytes em `0x577e2f`), mostra LNG 2319 ("Player %1 has exited the game") e entrega as pessoas humanas dele (`pessoa+0x122 = 0`) à IA (`+0x52f = 0`, `+0x548 = 3`, `+0x267 = 0`). Com menos de 3 jogadores roda direto (sem pacote). | `0x49ce50`, `0x49d6f0` (desconexão, tempo esgotado) |

### 2.3 Ids de lobby (`2..9`, não executados por `cmd_execute`)

`cmd_execute` não tem caso para eles; são mensagens do lobby multijogador (`0x4b75b0`, janela de rede) tratadas pelo código de rede [S, conteúdo de campo [?]]:

| id | size | emissor | uso provável |
|---|---|---|---|
| `2` | `0xd` | `0x4b75b0` (botões `0xc8a..0xc8e`) | escolha de vaga/cor (`+0xc = id - 0x76`) |
| `3` | `0xd` | `0x4b75b0` | idem (outra tecla) |
| `4` | `0x38` | `0x4b75b0` (`0x40234c` cópia) | texto de chat (nome 0x1e + mensagem) |
| `5` | `0xc2` | `0x4b9f80` | sincronização da lista de jogadores (`0x574840`, 0xb8 bytes) |
| `6` | `0xd` | `0x4b75b0` | pronto/cancelar |
| `7` | `0x10` | `0x4b73a0`, `0x4b75b0`, `0x4b9df0` (botões `0xcaa..0xcc4`) | opções de jogo (`+0x10`, `+0x11` = `id + 0x56`/`0x3e`) |
| `8` | `0x10` | `0x4b74f0` (`horse61m.raw`) | confirma jogador/pessoa |
| `9` | `0x15` | `net_game_start` `0x4b69b0` | parâmetros de partida (`0x49d4f0()`, bit 0/1 de bandeiras, `0x45d8f0`) |

## 3. O que o jogador pode construir e comprar

**Resumo (a resposta, do executável).** No **jogo** o jogador pode: assentar **trilho** (`0x16`: simples ou dupla, eletrificado só com motor elétrico disponível, pontes de tipo 1..3 conforme o ano), construir **estações** (`0x17`, 3 tamanhos x 8 orientações = formas 0..23, 50 000 / 100 000 / 200 000 vezes o efeito 8, sem filtro de ano), comprar **melhorias de estação** (`0x18`, 19 itens, com ano e exclusões), **trens** (`0x19`, só locomotivas disponíveis no ano/região; não há compra de vagões) e **substituir o motor** (`0x20`), **comprar e vender indústrias que já existem** (`0x2d`, 41 tipos, só no modelo industrial Expert, preço = efeito `0xd` x preço base da forma), **direitos de território** (`0x2a`, 4 direitos), ações, títulos (US$ 500 000 a 30 anos), gerentes, presidência, fusões e empresas novas. **Não** existem compra de vagões, sinais, túneis, nem construção de indústrias ou casas pelo jogador: as 98 formas `24..121` (indústrias, casas, comércio, aeroporto) só são colocadas pelo **editor** (ferramenta 7), pelo crescimento do mundo e por cenários. Todo o resto é restrição de interface (seção 3.9) e caixa.


### 3.1 Modo jogo e modo editor

`ui_set_editor_mode(p)` (`0x495700`) grava `0x5748f8 = p` (editor de **mapa**) e `0x5748fc = 0x573d7a | p` (**qualquer** editor: editor de mapa ou de cenário); a barra de ferramentas da janela principal (`mainwin.win`, botões `0xc9..0xd6`, `0xdd`) tem layouts diferentes. **No jogo** (`0x5748fc == 0`) a barra mostra, da esquerda para a direita (x em `0x495700`): trilho `0xce` (0x55), **estação `0xd0`** (0x82), **escavadeira `0xcf`** (0xaf), `0xdd` (0xdc, ferramenta `0x11`: modos da visão geral `bpovervw.win`, sem comando) e as abas de tela `0xd4..0xd6` (0x110, 0x13d, 0x16a). No editor entram `0xc9` (altura, ferramentas 1..3), `0xca`/`0xcb` (pincel e balde), `0xcc` (ferramentas 5/6), `0xcd` (ferramenta `0x12`) e `0xd1` (adicionar prédio, ferramentas 7, `0xd`, `0xf`, `0x10`, `0x13`).

A ferramenta ativa é `DAT_00574cb0` (`tool_select` `0x48f520(ferramenta, ?, p3, p4, p5, p6)`; `p3..p6` ficam em `DAT_005534c4`, `DAT_005534c8`, `DAT_00574428`, `DAT_00566abc`); a paleta correspondente (`0x492e40`) carrega a janela `bp*.win` e o clique no mapa é `map_tool_click` `0x48eb30` (a pré-visualização é `map_tool_hover` `0x48d900`):

| ferramenta | uso | janela de paleta | comando |
|---|---|---|---|
| 0 | seleção (clique em estação, trem, indústria, cidade) | nenhuma | nenhum: abre a estação (`prédio.+8 != 0`; a própria empresa seleciona, segundo clique em 2,5 s abre a tela), o trem (só o próprio; LNG 533 nos outros), a cidade (`0x433eb0`) ou o diálogo da indústria (`0x4e3620`, seção 3.6) |
| 1, 2, 3 | altura do terreno (subir/descer/nivelar) | `bpraisea.win` | **só editor** (sem comando, sem razão) |
| **4** | **assentar trilho** | `bpaddtra.win` / `bpaddtr2.win` | **`0x16`** |
| 5, 6 | pintar terreno / apagar árvores | `bppaintt.win`, `bpaddtre.win` | só editor |
| **7** | adicionar prédio/indústria (qualquer forma `24..121`) | `bpaddbui.win` | **`0x17`, só editor** |
| **8** | **escavadeira** | `bpbulldz.win` | **`0xa`** |
| 9, `0xc`, 10, `0xb`, `0x10`, `0x12`, `0x13`, `0xf`, `0xd` | pincel, balde, rio, cidade, etc. | `bpterrit.win`, `bpfillte.win`, ... | só editor |
| **`0xe`** | **construir estação** | `bpaddstn.win` | **`0x17`** |
| `0x11` | visão geral / território | `bpovervw.win` | nenhum (a consultoria de território abre por aqui, `0x441410`) |

### 3.2 Trilho (`0x16`)

Opções da paleta de trilho (ferramenta 4, `0x495ea0`/`0x48d900`): via **simples ou dupla** (`DAT_0055a6bc`, `track_single`), **eletrificação** (`DAT_00577d3c`), **tipo de ponte** `0..3` (`DAT_0055a6c0`). A eletrificação só é oferecida se há **algum motor elétrico disponível** para a empresa (`electric_track_available` `0x469f30`: existe motor de tipo 2 com `engine_available` `0x4683a0`; ou a opção de cenário `+0x64`; ou editor [S]). O tipo de ponte máximo é `max_bridge_type` `0x469ef0`: opção `cenário+0x1bf` se != 0; senão ano `< 1840` -> 1, `< 1880` -> 2, senão 3 (ou 3 no editor). Custos por tile e por ponte: `terrain-track.md` 2.2 e 2.3 (via simples ortogonal 4000, dupla 6000, eletrificada +6000/+9000, pontes 60000..300000, vão 120000, fator econômico `0x5504a8`). Condições na janela: permissão `0x5706b6` ("Laying track"; senão LNG 23 e o botão `0xce` fica cinza), empresa existente (LNG 337), **direitos de trilho** do território (`empresa+0x479+t*0x1ed & 2`, LNG 2266), limite de células (`empresa+0x4260`, LNG 3320), e `0x5706c1`/`0x5706c2` (trilho só conectado ao próprio: `track_company_may_build_at` `0x4603a0`). **Não há túneis, sinais nem trilhos comprados à parte**: pontes, via dupla e eletrificação são opções do mesmo comando.

### 3.3 Estações e prédios (`0x17`)

**O que o jogador constrói no jogo: só estações** (ferramenta `0xe`). Os prédios de indústria, as casas e todo o resto são do mundo (`world-growth.md`) e só entram pelo editor (ferramenta 7) ou pela IA/cidades.

Nomes: `symbols.toml` usa `building_type_table` para `0x535c38` (53 tipos de `0x27` bytes, o "tipo" de uma indústria) e também para `0x5457f8` (que é só a coluna `+9` da forma 0 de `building_shape_table`, `0x5457ef`, 122 formas de `0x154` bytes, o "tipo de prédio" de `0x17`). Aqui, "forma" = índice em `building_shape_table`, "tipo" = `forma.+9` = índice em `building_type_table`.

*Estação (ferramenta `0xe`, `bpaddstn.win`)*: a forma é `tamanho * 8 + orientação`, ou seja **24 formas**, índices `0..23` de `building_shape_table` (`0x5457ef`, 122 registros de `0x154` bytes, `+9` tipo) [D]:

| formas | tamanho | tipo | preço base `+0` |
|---|---|---|---|
| 0..7 | pequena (raio 2) | 0 (Station) | 50 000 |
| 8..15 | média (raio 4) | 0 | 100 000 |
| 16..23 | grande (raio 8) | 0 | 200 000 |

- **Sem filtro de ano, região ou cenário**: os três tamanhos são sempre oferecidos (a paleta `0x4941c0` modo 1 lista as três linhas e mostra o preço; as linhas de `improvement_table` 0..2 valem desde 1800). A orientação (0..7) vem do trilho vizinho (`0x468920(&forma, x, y)` e `0x408550`); o estilo de arquitetura é um de 7 (`0x7d8..0x7de`, `DAT_005534c8`; 0 = estilo da cidade mais próxima).
- **Preço**: `custo = ftol(effect_multiplier(8, empresa, 0, território_do_tile) * preço_base[forma])` (`0x46b480`; efeito 8 "Station Building"), lançado em **conta 1** (`0x409a30`: `company_account_move(empresa, 1, custo)`; `estação.+0x99 = custo`), mais o trilho sob a estação (`track_tile_cost_with_state` `0x463a20`) na **conta 3** (`terrain-track.md` 3.1).
- **Condições na janela** (`0x48eb30` casos `7`/`0xe`; `0x47b1b5` no botão): permissão `0x5706b7` ("Building a station"; senão LNG 23); empresa existente (LNG 340); caixa `> 0` (LNG 341, ignorado no editor); `station_placement_valid` `0x4087a0` (senão LNG 535 `0x217`: "You can't put a station here! (It must be on clear ground, next to existing track.)"); caixa `>=` custo (`company_account(0)`, LNG 394 `0x18a`; **ignorado se `0x5748fc != 0`**).
- **Validação no executor** (`0x4087a0`, `terrain-track.md` 3.2): para cada célula da pegada 10x10 com flags `& 0x3c3`: dentro do mapa; terreno sem rio/ponte; célula em terra `flags & 0x2c1`, em água `flags & 0x102`; sem outro prédio (objeto tipo 3) a menos que seja o da própria estação; para a célula-âncora (`0x10`): as duas arestas vizinhas do trilho (`dir ± 2`) devem existir ou poder ser construídas (`0x4629f0` x3, `0x4603a0`); **direitos de trilho** `empresa+0x479+território*0x1ed & 2` (é o bit 1 = trilho, não o bit 3 = estações: ver seção 4); e o tile não pode ser de outra estação (`0x4bf1d0`). Falha => `ok = 0`, nada acontece (o emissor local já avisou por LNG 535).
- **Estado local** (só para a empresa que enviou, `P.empresa == DAT_00574414`): depois de criar a estação, `0x40de20(2, 1)` abre a estação, `DAT_00574420 = DAT_005743f8`, e `estação+0x49 &= ~(1 << estação.+8)`: **a estação do jogador local nasce sem o bit do próprio tamanho** nas melhorias (em rede isso difere entre máquinas) [S].

*Prédios/indústrias (ferramenta 7, só editor)*: a lista `0xa29` de `bpaddbui.win` (`0x4941c0` modo 2, `0x494f40`) tem os **98** registros `24..121` ordenados por nome (`strcmpi`), mais as entradas especiais `** Special - Add City/Region/Label **` (LNG 536..538, só com `0x573d7a == 0`), `** Special - Lay River **` (539), `** Special - Reserve Cells **` (540, só com `0x573d7a == 0`) e `** Special - Lay Road **` (3321). **Sem filtro de ano**, de região, de conjunto industrial nem de cenário: aparecem todos. Preço: o mesmo `preço_base[forma]` abaixo; **sem lançamento no razão** para formas de tipo != 0 (`0x409a30` só cobra `conta 1` quando o tipo é 0) e sem checagem de caixa (`0x5748fc != 0`). No editor o dono vira 0 (`param_6 = 0` se `0x5748fc != 0`). Lista (forma -> tipo; nome LNG `920 + tipo` (`3010 + tipo - 38` nos tipos de expansão, tabela `0x5536e8`); preço base `+0`; primeiro ano do tipo `+0xc` em `0x535c38`) [D]:

| formas | tipo | nome | preço | 1º ano | | formas | tipo | nome | preço | 1º ano |
|---|---|---|---|---|---|---|---|---|---|---|
| 24 | 22 | Lumber Mill | 200 000 | 1800 | | 66 | 14 | Electric Plant | 400 000 | 1890 |
| 25..32 | 18 | Housing (8 modelos) | 100 000 | 1800 | | 67 | 20 | Landfill | 300 000 | 1950 |
| 33..36 | 16 | Grain Silo (4) | 250 000 | 1800 | | 68 | 21 | Logging Camp | 150 000 | 1800 |
| 37 | 2 | Auto Plant | 600 000 | 1900 | | 69 | 26 | Oil Well | 200 000 | 1860 |
| 38 | 34 | Textile Mill | 400 000 | 1800 | | 70..73 | 29 | Produce Orchard (4) | 200 000 | 1800 |
| 39 | 31 | Sheep Farm | 200 000 | 1800 | | 74..77 | 30 | Rubber Farm (4) | 150 000 | 1900 |
| 40 | 27 | Paper Mill | 300 000 | 1800 | | 78 | 37 | Uranium Mine | 600 000 | 1950 |
| 41 | 25 | Oil Refinery | 500 000 | 1890 | | 79..82 | 33 | Sugar Farm (4) | 200 000 | 1800 |
| 42 | 24 | Nuclear Plant | 1 000 000 | 1950 | | 83 | 4 | Bauxite Mine | 150 000 | 1910 |
| 43 | 15 | Fertilizer Plant | 300 000 | 1905 | | 84..87 | 28 | Port (4) | 300 000 | 1800 |
| 44 | 13 | Dairy Processor | 300 000 | 1881 | | 88 | 19 | Iron Ore Mine | 200 000 | 1800 |
| 45 | 12 | Dairy Farm | 200 000 | 1800 | | 89 | 38 | Geocore Plant | 2 000 000 | 1800 |
| 46 | 23 | Meat Packing Plant | 400 000 | 1800 | | 90..97 | 39 | Townhouse (8) | 120 000 | 1800 |
| 47 | 17 | Gravel Pit | 200 000 | 1884 | | 98..105 | 40 | Commercial (8) | 300 000 | 1800 |
| 48..51 | 11 | Cotton Farm (4) | 200 000 | 1800 | | 106..113 | 41 | Retail (8) | 150 000 | 1800 |
| 52 | 9 | Coal Mine | 300 000 | 1800 | | 114 | 42 | Distillery | 400 000 | 1800 |
| 53 | 8 | Chemical Plant | 300 000 | 1905 | | 115 | 43 | Barracks | 250 000 | 1800 |
| 54 | 7 | Cement Plant | 400 000 | 1884 | | 116 | 44 | Military Depot | 200 000 | 1800 |
| 55 | 5 | Cannery | 250 000 | 1856 | | 117 | 45 | Munitions Factory | 700 000 | 1800 |
| 56 | 3 | Bakery | 300 000 | 1800 | | 118 | 46 | Weapons Factory | 500 000 | 1800 |
| 57 | 32 | Steel Mill | 800 000 | 1856 | | 119 | 47 | Airport Terminal | 10 000 000 | 1800 |
| 58 | 35 | Tire Factory | 300 000 | 1900 | | 120..121 | 48 | Airport Tarmac (2) | 10 000 000 | 1800 |
| 59 | 36 | Tool and Die Factory | 200 000 | 1800 | | | | | | |
| 60 | 1 | Aluminum Mill | 500 000 | 1910 | | | | | | |
| 61 | 6 | Cattle Yard | 150 000 | 1800 | | | | | | |
| 62..65 | 10 | Coffee Farm (4) | 150 000 | 1800 | | | | | | |

(Os tipos 49..52 não têm forma; são reservas da expansão. As formas `0..23` são as estações; as 98 restantes são as do editor. Os tipos `38..48` pertencem ao conjunto industrial de expansão `0x5704ac = 1`, `world-growth.md` 2.5.) O primeiro ano **não** filtra o editor; ele só decide quando o mundo funda a indústria (`world-growth.md` 2.3).

### 3.4 Melhorias de estação (`0x18`)

Lista, ano, exclusões e custo: `terrain-track.md` 3.4 (19 itens compráveis: linhas `0..11` e `15..21` de `improvement_table` `0x5500c8`; as linhas 12..14 e 22 estão desligadas). Elegibilidade (`improvement_available` `0x468310`): `ano > primeiro || (ano == primeiro && mês >= 9)`, `ano <= último`, flag `+0x13`, bits excluídos; o conjunto de expansão `0x5704ac != 0` restringe às linhas `{0..4, 10, 11, 0x15}`. A janela `buildbuy.win` (`0x407e80`, abre por `0x4c2050` botão `0x25a`) exige permissão `0x5706b8` ("Building a station support building"), estação aberta pela empresa local (o clique do mapa só seleciona/abre estações com `estação.+0x39 == empresa local`), e `preço_total <= caixa` (ignorado se `0x5748fc != 0`). O jogador não pode vender nem demolir estações (LNG 1124).

### 3.5 Trens (`0x19`, `0x20`, `0x1f`)

Janela `trainbuy.win` (`0x4d9d20`) [S]: lista os **motores `0..65`** para os quais `engine_available(e, empresa)` `0x4683a0` é verdadeiro (`trains.md` 2.4: modificador `0x7f+e` positivo força, negativo proíbe; senão motor habilitado, `engine_allowed[e]` (região do mapa A/E/W), e `primeiro ano < ano <= último`, o modelo aparece em **junho** do primeiro ano). O filtro é ignorado se `0x573d7a != 0` (editor), `cenário+0x5c != 0` ou `0x574c0c != 0` (trapaça). Ordenação: crescente pelo **primeiro ano do veículo** (`vehicle_type_table+0x1c` do tipo da locomotiva, `0x53ddd4`). Pré-condições: caixa `> 0` (senão LNG 354 `0x162`, ignorado no editor), >= 2 estações (LNG 352 `0x160`, também checado em `train_buy`), pelo menos um motor disponível (LNG 355 `0x163`), permissão `0x5706b5`. Preço: `engine_table+0xc` com os modificadores de preço (`ui_train_purchase_info` `0x4e3f90`, `trains.md` 2.2 e 2.5), razão conta 4. **Não há compra de vagões**: o trem leva os vagões que as paradas pedem (`0x1c`), sem custo separado além do `train_create`; motor trocado em trem existente: `0x20` (mesma lista). Vender: `0x1f`, sem reembolso, só com permissão `0x5706b4` (`0x4d5e70`/`0x4d5f70`); só se abre o detalhe de trem da empresa local (clicar no de outra empresa dá LNG 533 `0x215`).

### 3.6 Indústrias existentes (`0x2d`)

O jogador **compra e vende** indústrias que o mundo já tem; não as constrói. A janela da indústria (`0x4e3620`, aberta com a ferramenta 0 ao clicar no prédio) só mostra os botões Comprar/Vender (`0x81`/`0x82` em `0x4e2030`) quando **todas** valem [S]:

1. modelo industrial **Expert**: `0x573d78 == 2` (a regra do manual "só no modelo Expert");
2. permissão `0x5706b9` ("Buying an industrial building");
3. o tipo é comprável: `building_type_table[tipo].+1 == 1` (`0x535c39 + tipo*0x27`) [D]. São **41 tipos**: `1..17`, `19..37`, `40`, `41`, `42`, `45`, `46`; **não** são compráveis `0` (Station), `18` (Housing), `38` Geocore Plant, `39` Townhouse, `43` Barracks, `44` Military Depot, `47` Airport Terminal, `48` Airport Tarmac (e `49..52` não têm forma);
4. o diálogo foi aberto pelo clique da ferramenta 0 num prédio de indústria (`prédio.+8 == 0`; `0x48eb30` chama `0x4e3620(-1, -1, id, param_4 = 0, ..., param_6 = 1)`), e há empresa local (`0x574414 != 0`); o caminho do menu de contexto (`0x466f10`, `param_4 = 1`) e a lista (`0x405ec0`, `param_6 = 0`) abrem o mesmo diálogo **sem** Comprar/Vender;
5. dono: se `indústria.+0x97 == empresa local`, só **Vender**; se `+0x97 == 0`, só **Comprar**; de outro dono, nenhum.

No clique de comprar (`building_buy_request` `0x43ea30`): `preço = building_price(id, empresa)` `0x40bae0` (`ftol(effect_multiplier(0xd, empresa, 0, território) * preço_base[forma])`, ou `+0x99` se já tem dono); exige `empresa+0x479+território*0x1ed & 2` (**o bit 1, o direito de trilho, e não o bit 2 "prédios"** [V]; LNG 2288 `0x8f0` se falta) e `preço <= caixa(conta 0)` (LNG 2289 `0x8f1`); então enfileira `0x2d` com compra = 1. Efeito do executor `0x43eb60` e vendas: `world-growth.md` 5.3 (compra: `conta 5 += preço`, caixa `-= preço`, `+0x97 = empresa`, `+0x99 = preço`; venda devolve o preço e perde metade: categoria 3 `-= preço/2`). Não há filtro de ano: o jogador compra qualquer indústria **que já exista**.

### 3.7 Direitos de território (`0x2a`)

Janela de consultoria de território (`0x441410`; botões `0x11f9..0x11fc` = bits `0..3`: trens, trilho, prédios, estações; LNG 2459..2462) [S]. `territory_rights_request(empresa, território, bit)` `0x43e760`:

```
preço = territory_rights_price(empresa, t, bit)             ; 0x440fb0, fastcall(ECX = empresa, EDX = t, [esp+4] = bit), ret 4 [V]
g = company_goodwill(empresa, t)                            ; 0x43eca0, thiscall(ECX = empresa, [t]) 0..10000: com t != 0 é a média
                                                            ;   (goodwill geral + posição da empresa no território +0x47a+t*0x1ed) / 2
f = (15000 - g) * 0.0001                                    ; 0.5 .. 1.5   (0x535718)
y = (f <= 1) ? f*f : 1 + 3*(f*f - 1)                         ; 0x535030 = 3.0, 0x5350c8 = 1.0
preço = (ftol(base * y) / 1000) * 1000                      ; base = 1000 * i16 em território(t)+0x3b+2*bit (um preço por direito, "Rights cost")
máscara = (território.+0x43 == 0) ? 0xf : 1 << bit           ; "Allow separate rights purchases" desligado compra tudo [V]
se a pessoa da empresa é humana (+0x122) e caixa < preço: LNG 2287 (só mostra se empresa == local) e retorna 0
senão cmd 0x2a(empresa, t, máscara, preço)
```

O executor aplica `territory_set_rights` `0x4c8540` (também aos territórios que "remapeiam" para `t`: `território+0x2a == t`), debita `conta 2` e `caixa`. Efeito dos bits: bit 0 libera os trens na área (`0x43df20`: `empresa+0x422e`); bit 1 é o **direito de trilho** que o código exige em **três** lugares (assentar trilho, posicionar estação, comprar prédio); os bits 2 e 3 só são lidos pelas funções de regra de cenário (`CompanyTerritoryHasBuildingBuyAccess`/`...StationAccess`, `scenarios.md`) e pelo texto da janela.

### 3.8 Finanças e empresa

Ações e títulos (`0xe`, `0x11`, `0x12`, `0x13`, `0x14`, `0x15`), gerente (`0x10`), presidência (`0xf`, `0x24`, `0x2e`), fusão (`0x22`) e fundação (`0x26`/`0x27`): regras e fórmulas em `finance.md` (seções 8..15); cada janela conserva a permissão de cenário da seção 3.9. Fixos que vêm da UI e não do executor: **títulos só de US$ 500 000 a 30 anos** (`0x448250`); emissão de ações com lote de 5% do total (mínimo 1000) e uma por ano.

### 3.9 Permissões de cenário (`0x5706a9 + i`, `finance.md` 13.9) e onde são lidas

O jogo checa cada permissão **só na janela** (ou na IA), nunca em `cmd_execute`. Valor 1 = permitido; negado mostra LNG 23 ("That action is not allowed in this scenario.") ou desabilita o botão. Mapa `endereço -> quem lê` (por referência de dado no executável):

| byte | restrição (LNG `1267+i`) | quem lê |
|---|---|---|
| `a9` | Stock selling | `person_can_sell` `0x4a7190`, `0x482e80` |
| `aa` | Stock buying | `person_can_buy` `0x4a6f60`, `0x482e80`, IA `0x4a8800` |
| `ab` | Accessing the stock market screen | `0x453970` (abre `stockdtl`), `0x4496e0`, `0x43d800` |
| `ac` | Declaring bankruptcy | `0x4496e0`, IA `0x45fbf0` |
| `ad` | Issuing stock | `0x447730` |
| `ae` | Buying back stock | `0x447bb0`, IA `0x45fbf0` |
| `af` | Issuing bonds | `0x448250`, IA `0x45fbf0` |
| `b0` | Repaying bonds | `0x4480f0`, IA `0x45fbf0` |
| `b1` | Changing the dividend rate | `0x4496e0`, IA `0x45fbf0` |
| `b2` | Setting the throttle | `0x4de210` (controle do acelerador) |
| `b3` | Replacing a train | **não referenciado** por nenhuma função |
| `b4` | Retiring a train | `0x4d5e70`, `0x4d5f70`, **e `0x48b170` (fusão)** |
| `b5` | Buying a train | botão de trens (`0x47b35a`), `train_crash` `0x4ce730` (`0x4ce9c3`), IA `0x436b30` e `ai_manage_trains` `0x4d9210` (`0x4d953a`) |
| `b6` | Laying track | botão `0xce` (`0x47af75`), IA `0x436b30` |
| `b7` | Building a station | botão `0xd0` (`0x47b1b5`), IA `0x436b30` |
| `b8` | Building a station support building | `0x4c2050`, `0x4c1080` (melhorias) |
| `b9` | Buying an industrial building | `0x4e3620`, IA `0x436950` |
| `ba` | Being fired as chairman | `0x43c760` |
| `bb` | Assuming chairmanship of another company | `0x443ab0` |
| `bc` | Resigning as chairman | `0x443ab0`, `0x4bd950` |
| `bd` | Merging companies | **não referenciado** (a fusão lê `b4`) |
| `be` | Starting a company | `0x498cf0`, IA `0x4a8800` |
| `bf` | Short selling stock | `0x4a7190` |
| `c0` | Buying stock on margin | `0x4a6f60` |
| `c1` | Unconnected to any track laying | `0x4603a0` |
| `c2` | (sem rótulo) | `0x4603a0`, `0x48d900` |
| `c3` | (sem rótulo) | botão da escavadeira (`0x47b0b5`; no ramo negado o código acinzenta o controle `0xce`, o do trilho: provável cópia e cola do original) |
| `c4` | (sem rótulo) | `0x498cf0`, IA `0x4a8800` |

## 4. Divergências do manual e esquisitices do original

- **A fusão checa a permissão errada**: `merger_request` (`0x48b170`, `mov al, [0x5706b4]`) lê "Retiring a train" (`b4`); a permissão "Merging companies" (`bd`) não é lida por ninguém [S, confirmado no disassembly].
- **Direitos de território**: o byte `empresa+0x479+t*0x1ed` tem os bits 0 = trens, 1 = trilho, 2 = prédios, 3 = estações (ordem dos botões `0x11f9..0x11fc` e de `0x454ae0`), mas o código de jogo usa o **bit 1 (trilho)** como direito para assentar trilho, posicionar estação (`0x4087a0`) e comprar prédio (`0x43ea30`); os bits 2 e 3 só afetam as regras de cenário e o texto. Vale o código; o manual descreve quatro direitos independentes.
- **Prédios nunca são construídos pelo jogador** no jogo: a ferramenta 7 é do editor. Quem "constrói indústrias" no manual compra as que o mundo gera.
- **Comando executa sem checar caixa**: uma queda de caixa entre a janela e o executor (rede) deixa o saldo negativo; não há nova checagem.
- **`0x17` do jogador local apaga o bit do tamanho** em `estação+0x49` (estado local, não vai para os outros peers).
- Ids `0xc`/`0xd` existem no executor e não têm emissor (comandos de depuração).

## 5. Testes de oracle

`crates/rt2-oracle/tests/commands.rs` (18 testes, pulam se o executável falta; só dependem de `rt2-oracle`) [V]:

- `packet_ids_and_sizes_match_spec`: varre o `.text` atrás de todas as chamadas a `cmd_packet_init` (thunk `0x401b8b`) e confere os 45 pares `(id, size)` da seção 2 (nenhum para `0xc`/`0xd`); `packet_init_zeroes_and_stamps_the_header`: o construtor zera o corpo e grava `+0 = id`, `+4 = size`.
- `shape_table_matches_spec`, `buyable_industry_types_match_spec`: as 122 formas (tipo `+9`, preço `+0`, estações `+0xa`/`+0xb`), o id LNG de cada tipo e a lista de 41 tipos compráveis (flag `+1`).
- `building_buy_request_validation_and_packet` (400 casos), `building_request_by_the_local_company_...`, `building_sell_request_packet`: o direito é o **bit 1** do byte (bits 0, 2, 3 não contam), `preço <= caixa`, e o pacote `0x2d` byte a byte.
- `territory_rights_price_matches_formula` (300 casos, `0x440fb0` + `0x43eca0` reais) e `territory_rights_request_validation_and_packet` (`0x43e760`: máscara `0xf` ou `1 << bit`, recusa só com chairman humano e caixa menor que o preço, pacote `0x2a`).
- `cmd_execute` (`0x4b0210`) para `0x12`, `0x1b`, `0x28`, `0x29`, `0x2b`, `0x2c`, `0x2f`: offsets dos campos, ids de lista inválidos/livres ignorados, contador de UI balanceado.
- `max_bridge_type_matches_spec` (`0x469ef0`), `engine_available_matches_spec` (`0x4683a0`, 2000 casos) e `electric_track_is_offered_only_with_an_available_electric_engine` (`0x469f30`).

Os emissores são testados com `cmd_send` (`0x4af6d0`) substituído por um gravador que copia o pacote; as janelas de mensagem e de texto são cortadas. Os executores com efeito de tela ou de rede (`0x0a`, `0x0b`, `0x16`..`0x1a`, `0x1c`..`0x27`, `0x2a`, `0x2d`, `0x2e`) seguem **[S]**: a camada de construção do `rt2-sim` (`build/cmd.rs`) cobre `0x16`..`0x21`, `0x2a`, `0x2c`, `0x2d` e `0x2f` (`0x1a`, `0x1b`, `0x1c`, `0x21`, `0x2c`, `0x2f` **[V]** por snapshot do original em `tests/build_snapshots.rs`, casos `goto8_*`, `wait8*`, `cargo8*`, `class*`, `sell8`, `mark8`), e `0x2d` já tem teste em `stations.rs` (`company_industry_trade`).

## 6. Em aberto

- Corpo exato dos ids de lobby `2..9` (seção 2.3) e do `0x26` (`+0xc..+0x34`): só os tamanhos e emissores foram lidos.
- Significado de `param_6`/`param_4` de `ui_industry_dialog` (`0x4e3620`; "clique no mapa" e "remoto") e de `0x573d7a` no modo de jogo normal (aqui tratado como "editor de cenário").
- O que `0x5706c2`/`0x5706c3`/`0x5706c4` permitem exatamente (usados em `0x4603a0`, `0x48d900`, botão da escavadeira, `0x498cf0`).
- Se a ferramenta `0x11` (botão `0xdd`) emite algum comando além de abrir telas.
- Origem de `+0x10` de `0x4d3f40` (as duas paradas padrão do trem novo: `pool_nth(estações)` e `0x439720`) e do campo "forçada" (`+0xf`) do `0x0e`.
- Janelas `0xc`/`0xd`: se algum mod/depurador as usa.
