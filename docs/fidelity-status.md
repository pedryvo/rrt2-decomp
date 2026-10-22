# Placar de fidelidade (nível 2: save como oráculo)

Compara o clone com o original rodando no Wine (`re/scripts/wine/advance_save.py`, RNG semeado com 1234, passos de 320 min) campo a campo com `rt2-savediff` (campos voláteis mascarados). O clone usa `rt2-advance <in.GAM> <dias> 1234 <out.GAM>` (`crates/rt2-tools/src/bin/rt2-advance.rs`) sobre a **mesma entrada convertida pelo original** (`base_*.GAM` = o save avançado 0 dias no original, que já é 0x2348), para que os campos crus sejam comparáveis. Teste automatizado: `crates/rt2-sim/tests/worldloop.rs` (variáveis `RT2_DIR` e `RT2_FIDELITY_DIR`). Spec do laço: `docs/spec/world-tick.md`.

## Resultado

<!-- fidelity:begin -->
<!-- Gerado por scripts/fidelity.py; nao editar a mao. -->
| Cenário | Dias | Resultado | Campos diferentes | Principais grupos |
|---|---|---|---|---|
| Tutorial (sem trens, sem estações) | 1 | **igual** | 0 |  |
| Tutorial (sem trens, sem estações) | 7 | **igual** | 0 |  |
| Tutorial (sem trens, sem estações) | 30 | **igual** | 0 |  |
| Example save game (9 trens, 11 estações) | 1 | **igual** | 0 |  |
| Example save game (9 trens, 11 estações) | 7 | diferente | 351 | `stations` (351) |
| Mapa Alaska.MP2 | 1 | **igual** | 0 |  |
| Mapa Alaska.MP2 | 7 | **igual** | 0 |  |
| Mapa Austria.MP2 | 1 | **igual** | 0 |  |
| Mapa Austria.MP2 | 7 | **igual** | 0 |  |
| Mapa Brazil.MP2 | 1 | **igual** | 0 |  |
| Mapa Brazil.MP2 | 7 | **igual** | 0 |  |
| Mapa Atlantis.MP2 | 1 | **igual** | 0 |  |
| Mapa Atlantis.MP2 | 7 | **igual** | 0 |  |

<!-- fidelity:state {"Alaska|1": {"equal": true}, "Alaska|7": {"equal": true}, "Atlantis|1": {"equal": true}, "Atlantis|7": {"equal": true}, "Austria|1": {"equal": true}, "Austria|7": {"equal": true}, "Brazil|1": {"equal": true}, "Brazil|7": {"equal": true}, "Example|1": {"equal": true}, "Example|7": {"equal": false}, "Tutorial|1": {"equal": true}, "Tutorial|30": {"equal": true}, "Tutorial|7": {"equal": true}} -->
<!-- fidelity:end -->

As saídas do original para o Example foram geradas com `trace_save.py` (sem breakpoints) a partir de `base_ex.GAM`, semente 1234 para o RNG do jogo **e** para o rand do CRT (que sorteia a fumaça gravada em `pool_78`; `rt2-advance` aplica `Command::SeedCrtRng`). `rt2-savediff` mascara os bits 4-15 das flags dos passos de rota, que o pathfinder do original preenche com memória de `malloc` não inicializada (`trains.md` 10.7). Testes: `example_one_day_is_equal` e `example_seven_days` em `crates/rt2-sim/tests/worldloop.rs` (precisam de `base_ex.GAM`, `orig_ex_1.GAM`, `orig_ex_7.GAM` em `RT2_FIDELITY_DIR`).

Caminhos raros do trem (chegada com entrega e receita, carga na estação, saída, custos mensais, passo anual; a quebra bate exceto pelos artefatos de som e vista da empresa vista na interface) são verificados por **snapshots de função** do original (`re/scripts/wine/snap_call.py`, `docs/dynamic-analysis.md` 6): o jogo é gravado logo antes e logo depois de uma chamada, e `crates/rt2-sim/tests/trainstep_snapshots.rs` roda o port sobre o primeiro save e compara com o segundo. Os 9 snapshots atuais (Example, até 120 dias) são idênticos, inclusive os estados finais dos dois geradores.

Verificações extras: contagens de passos do Tutorial 7 dias batem com `world-tick.md` seção 6 (39 `sim_advance`, 387 recontagens e 387 produções, 0 RNG); em 30 dias o estado final do RNG é `0x051bc859`, igual ao original, e o passo econômico, o sorteio de bônus de cidade e os juros das pessoas reproduzem o original. Round-trip do salvamento: `Save::to_bytes` regrava byte a byte os saves 0x2348 do original (checksum `0x4ef4d0` incluso) e os dois `.GAM` 0x233C.

## Diferenças restantes

1. ~~Tutorial 7/30 dias, 3 campos~~ **resolvido**: a linha 28 (Port) e as taxas das casas da `industry_type_table` são reescritas na carga por `industry_load_fixups` (`0x4719e0`), que copia o Port do cabeçalho do save (`cargo.md` 7.1.1). Implementado em `rt2_sim::industry::apply_load_fixups`; Tutorial 7 e 30 dias dão 0 campos diferentes.
2. ~~Example, 1 dia (1198 campos)~~ **resolvido**: `train_step` (`0x4d0ce0`) e `anim_object_step` (`0x4283e0`) portados função a função (`crates/rt2-sim/src/trainstep/`, spec em `trains.md` seção 10): movimento pela rota, look-ahead e reservas de via (`pool_80`, donos `+7/+9` das peças), pathfinder em modo trem (`0x460850`, fila de baldes), parada na estação, fumaça (`0x412900`/`0x412a90`, rand do CRT) e as cadeias de estruturas dos tiles.
3. ~~Example, 7 dias (351 campos em `stations`)~~ **resolvido**: `station_recompute_supply_demand` (`0x4bebf0`, `crates/rt2-sim/src/trainstep/supply.rs`) no passo semanal e mensal e a parte das estações de `industry_spawn_lot` (`0x468af0`, `+0x4b5`).

## Ganchos que faltam (por impacto)

O clone os conta em runtime (`rt2-advance` imprime `MISSING`, `World::hook_log`). Pedidos de especificação em `world-tick.md` seção 9.

2. `company_month_step`/`company_annual_review`/`company_year_end` sobre o registro cru (`finance.md`; falta mapear o registro para `finance::Company`); `ai_monthly_turn` (RNG, `world-tick.md` 3.5, não especificado).
3. Fim de ano: `town_year_step` (regras em `growth.rs`/`world-growth.md`, faltam as implementações dos traits `GrowthWorld`/`PickWorld` sobre os pools do save), `industry_year_step`/`industry_year_reset`, `ai_pick_engine`, `history_snapshot`, contadores `0x573d05/09/0d`, desgaste das peças de trilho.
4. `event_fire_trigger` (regras de cenário, pode consumir RNG): 3 chamadas por mês no Tutorial, sem efeito visível ali.
5. Remoção de lote vencido (`0x466a60`), `margin_check_all`, `person_year_step`, mensagens (`economy_headline`, `town_bonus_message`). (`train_monthly_costs`, `train_year_step` e a rotação do desgaste das peças de trilho agora estão implementados.)
6. Dentro de `train_step` (contados como ganchos ausentes; nenhum é atingido em 30 dias do Example): acidente e roubo (`0x4ce730`, `0x4ceaa0`, `0x42fa30`), vagão genérico, troca de composição na estação, lote largado para transferência, lote que volta à pilha, `industry_receive_cargo` (`0x40b250`), mensagens e sons da empresa vista na interface (`0x46abf0`, `0x4d0870`, que gravam a alça de som `+0xab` e o bit `0x10` de `+0xbb` do trem e a vista em `estado+0x2e/+0x32`), `0x4d14b0`, `train_delete`, colocação de trem novo, ramos da flag `0x570229` (`trains.md` 10.8).

Suposições fora do save, anotadas em `crates/rt2-sim/src/tick.rs`: sem cheat Overtime, jogo local, conjunto de indústrias 0 (`0x5704ac`), `0x5748f8 == 0`; arquivos 0x233C não são suportados nos ganchos que leem o estado (offsets deslocados).
