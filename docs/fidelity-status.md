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
| Example save game (9 trens, 11 estações) | 7 | **igual** | 0 |  |
| Example save game (9 trens, 11 estações) | 30 | diferente | 1495 | `state` (1099), `structures` (268), `stations` (65) |
| Mapa Alaska.MP2 | 1 | **igual** | 0 |  |
| Mapa Alaska.MP2 | 7 | **igual** | 0 |  |
| Mapa Alaska.MP2 | 30 | **igual** | 0 |  |
| Mapa Austria.MP2 | 1 | **igual** | 0 |  |
| Mapa Austria.MP2 | 7 | **igual** | 0 |  |
| Mapa Austria.MP2 | 30 | **igual** | 0 |  |
| Mapa Brazil.MP2 | 1 | **igual** | 0 |  |
| Mapa Brazil.MP2 | 7 | **igual** | 0 |  |
| Mapa Brazil.MP2 | 30 | **igual** | 0 |  |
| Mapa Atlantis.MP2 | 1 | **igual** | 0 |  |
| Mapa Atlantis.MP2 | 7 | **igual** | 0 |  |
| Mapa Atlantis.MP2 | 30 | **igual** | 0 |  |

<!-- fidelity:state {"Alaska|1": {"equal": true}, "Alaska|30": {"equal": true}, "Alaska|7": {"equal": true}, "Atlantis|1": {"equal": true}, "Atlantis|30": {"equal": true}, "Atlantis|7": {"equal": true}, "Austria|1": {"equal": true}, "Austria|30": {"equal": true}, "Austria|7": {"equal": true}, "Brazil|1": {"equal": true}, "Brazil|30": {"equal": true}, "Brazil|7": {"equal": true}, "Example|1": {"equal": true}, "Example|30": {"equal": false}, "Example|7": {"equal": true}, "Tutorial|1": {"equal": true}, "Tutorial|30": {"equal": true}, "Tutorial|7": {"equal": true}} -->
<!-- fidelity:end -->

As saídas do original para o Example foram geradas com `trace_save.py` (sem breakpoints) a partir de `base_ex.GAM`, semente 1234 para o RNG do jogo **e** para o rand do CRT (que sorteia a fumaça gravada em `pool_78`; `rt2-advance` aplica `Command::SeedCrtRng`). `rt2-savediff` mascara os bits 4-15 das flags dos passos de rota, que o pathfinder do original preenche com memória de `malloc` não inicializada (`trains.md` 10.7). Testes: `example_one_day_is_equal` e `example_seven_days` em `crates/rt2-sim/tests/worldloop.rs` (precisam de `base_ex.GAM`, `orig_ex_1.GAM`, `orig_ex_7.GAM` em `RT2_FIDELITY_DIR`).

Caminhos raros do trem (chegada com entrega e receita, carga na estação, saída, custos mensais, passo anual; a quebra bate exceto pelos artefatos de som e vista da empresa vista na interface) são verificados por **snapshots de função** do original (`re/scripts/wine/snap_call.py`, `docs/dynamic-analysis.md` 6): o jogo é gravado logo antes e logo depois de uma chamada, e `crates/rt2-sim/tests/trainstep_snapshots.rs` roda o port sobre o primeiro save e compara com o segundo. Os 9 snapshots atuais (Example, até 120 dias) são idênticos, inclusive os estados finais dos dois geradores.

Verificações extras: contagens de passos do Tutorial 7 dias batem com `world-tick.md` seção 6 (39 `sim_advance`, 387 recontagens e 387 produções, 0 RNG); em 30 dias o estado final do RNG é `0x051bc859`, igual ao original, e o passo econômico, o sorteio de bônus de cidade e os juros das pessoas reproduzem o original. Round-trip do salvamento: `Save::to_bytes` regrava byte a byte os saves 0x2348 do original (checksum `0x4ef4d0` incluso) e os dois `.GAM` 0x233C.

## Diferenças restantes

1. ~~Tutorial 7/30 dias, 3 campos~~ **resolvido**: a linha 28 (Port) e as taxas das casas da `industry_type_table` são reescritas na carga por `industry_load_fixups` (`0x4719e0`), que copia o Port do cabeçalho do save (`cargo.md` 7.1.1). Implementado em `rt2_sim::industry::apply_load_fixups`; Tutorial 7 e 30 dias dão 0 campos diferentes.
2. ~~Example, 1 dia (1198 campos)~~ **resolvido**: `train_step` (`0x4d0ce0`) e `anim_object_step` (`0x4283e0`) portados função a função (`crates/rt2-sim/src/trainstep/`, spec em `trains.md` seção 10): movimento pela rota, look-ahead e reservas de via (`pool_80`, donos `+7/+9` das peças), pathfinder em modo trem (`0x460850`, fila de baldes), parada na estação, fumaça (`0x412900`/`0x412a90`, rand do CRT) e as cadeias de estruturas dos tiles.
3. ~~Example, 7 dias (351 campos em `stations`)~~ **resolvido**: `station_recompute_supply_demand` (`0x4bebf0`, `crates/rt2-sim/src/trainstep/supply.rs`) no passo semanal e mensal e a parte das estações de `industry_spawn_lot` (`0x468af0`, `+0x4b5`).
4. ~~Example, 30 dias (4684 campos)~~ **reduzido a 1495** (os ganchos mensais e anuais foram ligados ao estado cru do save; ver abaixo). Alaska, Austria, Atlantis e Brazil (mapas `.MP2`, 30 dias) e Tutorial (30 dias) batem em 0 campos. No Example restam só grupos que as specs deixam em aberto: `state` (1099: o registro anual `history_snapshot` `0x4e9770`, 0x570a80..0x57381c), `structures` (268) e `tiles`/`elevation` (marcação do terreno dos 2 prédios criados e das 2 demolições do crescimento anual) e `stations` (65: o efeito completo de `0x4beae0` no reinício anual e os elos de captação dos prédios novos), mais a empresa de IA 5 e seu chairman (turno da IA). O consumo do RNG do ano inteiro (revisões das diretorias, gerentes candidatos, `ai_pick_engine`, crescimento das cidades com 2 prédios criados e 2 demolidos) acompanha o original: o `places` e o `objects` do crescimento batem campo a campo.

## Ganchos ligados em 30 dias

`company_month_step` (juros, manutenção, overhead, salários, dividendos trimestrais pagos aos acionistas), `event_fire_trigger` 0..4 (regras de cenário com efeitos de nível, variáveis, direitos de território, estado econômico e caixa; `tick/events.rs`), `margin_check_all`, `economy_state_step` (cotação de todas as empresas na mudança de estado), expiração de lotes (`lot_free` `0x466a60`), no fim do ano `company_annual_review` (porte de `review_ref`, provado no oráculo), `company_year_end` (arquivo de métricas/categorias/contas, sobrecarga, títulos, desdobramento), `person_year_step`, `industry_year_step`/`industry_year_reset`, velocidade de referência `0x573e08` com `ai_pick_engine`, fator de crescimento `0x5705d1`, zeragem dos contadores e `town_year_step` (`GrowthWorld`/`PickWorld`/`TownEnv`/`SiteEnv` sobre os pools do save; `tick/town.rs`). Descobertas contra o original: o passo mensal multiplica por 1/12 (não divide por 12); a métrica 5 do arquivo anual enxerga o preço novo na linha 1 (as linhas deslocam antes da gravação); `building_can_place` rejeita tile com `tile+0 & 8`; negar o último direito de um território (`0x4c8540`) zera um byte de resumo da empresa (`+0x422d/+0x422e` nos territórios 4 e 5; regra inferida).

## Ganchos que ainda faltam (por impacto)

O clone os conta em runtime (`rt2-advance` imprime `MISSING`, `World::hook_log`). Pedidos de especificação em `world-tick.md` seção 9.

1. `history_snapshot` (`0x4e9770`): grava ~3 KB de histórico no fim de cada ano (1099 campos no Example 30 dias). **Não especificado.**
2. Turno da IA (`ai_monthly_turn` `0x436d60`): só os 4 sorteios de `ai_pick_engine` por empresa de IA ativa são reproduzidos (mantêm o RNG alinhado no Example); `company_ai_finance` (dividendo da empresa 5, `+0x154`, `+0x168`, caixa do chairman) e o resto estão no módulo `ai/` de outro agente.
3. Marcação do terreno de `building_create`/`building_destroy` (cadeias `structures`, bits dos tiles, relevo, sorteios de textura de fazenda: 1 `rng` por tile da pegada de alguns tipos) e `building_can_place` completo (`0x4087a0`): o clone usa uma leitura mínima (limites, rio, `tile+0 & 8`, prédio existente, bits `0x2c1`/`0x102`), que reproduziu o Example mas não está provada.
4. `0x4beae0` (reinício anual das estações), elos de captação dos prédios novos (`0x4071b0`), `station_networks_rebuild` (`0x46a370`).
5. `company_set_chairman(0)` e `company_liquidate` na revisão (demissão/liquidação, contadas), chamadas de margem com saldo negativo, `0x573e02`, mensagens (`economy_headline`, `town_bonus_message`, desdobramento, manchetes históricas, janelas das regras), regras de desfazer (`scenario_undo_rule`: efeitos temporários, a regra de desfazer não é anexada ao pool salvo) e funções de cenário não especificadas (`CompanyGoodwill`, `PlayerStockValue`, `TerritoryCompletelyFlooded`).
6. Dentro de `train_step` (contados como ganchos ausentes; nenhum é atingido em 30 dias do Example): acidente e roubo (`0x4ce730`, `0x4ceaa0`, `0x42fa30`), vagão genérico, troca de composição na estação, lote largado para transferência, lote que volta à pilha, `industry_receive_cargo` (`0x40b250`), mensagens e sons da empresa vista na interface (`0x46abf0`, `0x4d0870`, que gravam a alça de som `+0xab` e o bit `0x10` de `+0xbb` do trem e a vista em `estado+0x2e/+0x32`), `0x4d14b0`, `train_delete`, colocação de trem novo, ramos da flag `0x570229` (`trains.md` 10.8).

Suposições fora do save, anotadas em `crates/rt2-sim/src/tick.rs`: sem cheat Overtime, jogo local, conjunto de indústrias 0 (`0x5704ac`), `0x5748f8 == 0`; arquivos 0x233C não são suportados nos ganchos que leem o estado (offsets deslocados).
