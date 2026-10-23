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
| Example save game (9 trens, 11 estações) | 30 | **igual** | 0 |  |
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
| Tutorial convertido pelo clone (0x233C -> 0x2348) | 0 | **igual** | 0 |  |
| Tutorial convertido pelo clone (0x233C -> 0x2348) | 7 | **igual** | 0 |  |
| Example convertido pelo clone (0x233C -> 0x2348) | 0 | **igual** | 0 |  |
| Example convertido pelo clone (0x233C -> 0x2348) | 7 | **igual** | 0 |  |
| #01 B&O.MAP carregado como save e convertido pelo clone | 0 | **igual** | 0 |  |
| #01 B&O.MAP carregado como save e convertido pelo clone | 7 | **igual** | 0 |  |
| Britain.MAP carregado como save e convertido pelo clone | 0 | **igual** | 0 |  |
| Britain.MAP carregado como save e convertido pelo clone | 7 | **igual** | 0 |  |
| #01 B&O iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #01 B&O iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #02 Hannibal iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #02 Hannibal iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #04 Silver iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #04 Silver iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #07 Birmingham iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #07 Birmingham iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #03 Bridges iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #03 Bridges iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #05 White iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #05 White iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #06 Cross iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #06 Cross iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #08 Orient iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #08 Orient iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #09 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #09 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #10 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #10 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #11 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #11 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #12 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #12 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #13 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #13 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #14 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #14 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #15 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #15 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #16 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #16 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #17 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #17 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #18 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #18 iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E01 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E01 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E02 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E02 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E03 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E03 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E04 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E04 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E05 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E05 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E06 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E06 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E07 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E07 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E08 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E08 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E09 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E09 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E10 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E10 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E11 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E11 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E12 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E12 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E13 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E13 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E14 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E14 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E15 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E15 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E16 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E16 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E17 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E17 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |
| #E18 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 0 | **igual** | 0 |  |
| #E18 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 7 | **igual** | 0 |  |

<!-- fidelity:state {"#09 New Game|0": {"equal": true}, "#09 New Game|7": {"equal": true}, "#10 New Game|0": {"equal": true}, "#10 New Game|7": {"equal": true}, "#11 New Game|0": {"equal": true}, "#11 New Game|7": {"equal": true}, "#12 New Game|0": {"equal": true}, "#12 New Game|7": {"equal": true}, "#13 New Game|0": {"equal": true}, "#13 New Game|7": {"equal": true}, "#14 New Game|0": {"equal": true}, "#14 New Game|7": {"equal": true}, "#15 New Game|0": {"equal": true}, "#15 New Game|7": {"equal": true}, "#16 New Game|0": {"equal": true}, "#16 New Game|7": {"equal": true}, "#17 New Game|0": {"equal": true}, "#17 New Game|7": {"equal": true}, "#18 New Game|0": {"equal": true}, "#18 New Game|7": {"equal": true}, "#E01 New Game|0": {"equal": true}, "#E01 New Game|7": {"equal": true}, "#E02 New Game|0": {"equal": true}, "#E02 New Game|7": {"equal": true}, "#E03 New Game|0": {"equal": true}, "#E03 New Game|7": {"equal": true}, "#E04 New Game|0": {"equal": true}, "#E04 New Game|7": {"equal": true}, "#E05 New Game|0": {"equal": true}, "#E05 New Game|7": {"equal": true}, "#E06 New Game|0": {"equal": true}, "#E06 New Game|7": {"equal": true}, "#E07 New Game|0": {"equal": true}, "#E07 New Game|7": {"equal": true}, "#E08 New Game|0": {"equal": true}, "#E08 New Game|7": {"equal": true}, "#E09 New Game|0": {"equal": true}, "#E09 New Game|7": {"equal": true}, "#E10 New Game|0": {"equal": true}, "#E10 New Game|7": {"equal": true}, "#E11 New Game|0": {"equal": true}, "#E11 New Game|7": {"equal": true}, "#E12 New Game|0": {"equal": true}, "#E12 New Game|7": {"equal": true}, "#E13 New Game|0": {"equal": true}, "#E13 New Game|7": {"equal": true}, "#E14 New Game|0": {"equal": true}, "#E14 New Game|7": {"equal": true}, "#E15 New Game|0": {"equal": true}, "#E15 New Game|7": {"equal": true}, "#E16 New Game|0": {"equal": true}, "#E16 New Game|7": {"equal": true}, "#E17 New Game|0": {"equal": true}, "#E17 New Game|7": {"equal": true}, "#E18 New Game|0": {"equal": true}, "#E18 New Game|7": {"equal": true}, "Alaska|1": {"equal": true}, "Alaska|30": {"equal": true}, "Alaska|7": {"equal": true}, "Atlantis|1": {"equal": true}, "Atlantis|30": {"equal": true}, "Atlantis|7": {"equal": true}, "Austria|1": {"equal": true}, "Austria|30": {"equal": true}, "Austria|7": {"equal": true}, "B&O New Game|0": {"equal": true}, "B&O New Game|7": {"equal": true}, "B&O.MAP (carga)|0": {"equal": true}, "B&O.MAP (carga)|7": {"equal": true}, "Birmingham New Game|0": {"equal": true}, "Birmingham New Game|7": {"equal": true}, "Brazil|1": {"equal": true}, "Brazil|30": {"equal": true}, "Brazil|7": {"equal": true}, "Bridges New Game|0": {"equal": true}, "Bridges New Game|7": {"equal": true}, "Britain.MAP (carga)|0": {"equal": true}, "Britain.MAP (carga)|7": {"equal": true}, "Cross New Game|0": {"equal": true}, "Cross New Game|7": {"equal": true}, "Example (conversao)|0": {"equal": true}, "Example (conversao)|7": {"equal": true}, "Example|1": {"equal": true}, "Example|30": {"equal": true}, "Example|7": {"equal": true}, "Hannibal New Game|0": {"equal": true}, "Hannibal New Game|7": {"equal": true}, "Orient New Game|0": {"equal": true}, "Orient New Game|7": {"equal": true}, "Silver New Game|0": {"equal": true}, "Silver New Game|7": {"equal": true}, "Tutorial (conversao)|0": {"equal": true}, "Tutorial (conversao)|7": {"equal": true}, "Tutorial|1": {"equal": true}, "Tutorial|30": {"equal": true}, "Tutorial|7": {"equal": true}, "White New Game|0": {"equal": true}, "White New Game|7": {"equal": true}} -->
<!-- fidelity:end -->

As saídas do original para o Example foram geradas com `trace_save.py` (sem breakpoints) a partir de `base_ex.GAM`, semente 1234 para o RNG do jogo **e** para o rand do CRT (que sorteia a fumaça gravada em `pool_78`; `rt2-advance` aplica `Command::SeedCrtRng`). `rt2-savediff` mascara os bits 4-15 das flags dos passos de rota, que o pathfinder do original preenche com memória de `malloc` não inicializada (`trains.md` 10.7). Testes: `example_one_day_is_equal` e `example_seven_days` em `crates/rt2-sim/tests/worldloop.rs` (precisam de `base_ex.GAM`, `orig_ex_1.GAM`, `orig_ex_7.GAM` em `RT2_FIDELITY_DIR`).

Caminhos raros do trem (chegada com entrega e receita, carga na estação, saída, custos mensais, passo anual; a quebra bate exceto pelos artefatos de som e vista da empresa vista na interface) são verificados por **snapshots de função** do original (`re/scripts/wine/snap_call.py`, `docs/dynamic-analysis.md` 6): o jogo é gravado logo antes e logo depois de uma chamada, e `crates/rt2-sim/tests/trainstep_snapshots.rs` roda o port sobre o primeiro save e compara com o segundo. Os 9 snapshots atuais (Example, até 120 dias) são idênticos, inclusive os estados finais dos dois geradores.

Verificações extras: contagens de passos do Tutorial 7 dias batem com `world-tick.md` seção 6 (39 `sim_advance`, 387 recontagens e 387 produções, 0 RNG); em 30 dias o estado final do RNG é `0x051bc859`, igual ao original, e o passo econômico, o sorteio de bônus de cidade e os juros das pessoas reproduzem o original. Round-trip do salvamento: `Save::to_bytes` regrava byte a byte os saves 0x2348 do original (checksum `0x4ef4d0` incluso) e os dois `.GAM` 0x233C.

## Diferenças restantes

1. ~~Tutorial 7/30 dias, 3 campos~~ **resolvido**: a linha 28 (Port) e as taxas das casas da `industry_type_table` são reescritas na carga por `industry_load_fixups` (`0x4719e0`), que copia o Port do cabeçalho do save (`cargo.md` 7.1.1). Implementado em `rt2_sim::industry::apply_load_fixups`; Tutorial 7 e 30 dias dão 0 campos diferentes.
2. ~~Example, 1 dia (1198 campos)~~ **resolvido**: `train_step` (`0x4d0ce0`) e `anim_object_step` (`0x4283e0`) portados função a função (`crates/rt2-sim/src/trainstep/`, spec em `trains.md` seção 10): movimento pela rota, look-ahead e reservas de via (`pool_80`, donos `+7/+9` das peças), pathfinder em modo trem (`0x460850`, fila de baldes), parada na estação, fumaça (`0x412900`/`0x412a90`, rand do CRT) e as cadeias de estruturas dos tiles.
3. ~~Example, 7 dias (351 campos em `stations`)~~ **resolvido**: `station_recompute_supply_demand` (`0x4bebf0`, `crates/rt2-sim/src/trainstep/supply.rs`) no passo semanal e mensal e a parte das estações de `industry_spawn_lot` (`0x468af0`, `+0x4b5`).
4. ~~Example, 30 dias (4684 campos)~~ **reduzido a 7**: os ganchos mensais e anuais estão ligados ao estado cru do save, o registro anual `history_snapshot` (`0x4e9770`, `world-tick.md` 3.4.1) e a marcação do terreno de `building_create`/`building_destroy` (`world-growth.md` 4.5: estruturas dos tiles, nivelamento da elevação, flags de terreno, elos de estação, `0x4beae0`) seguem o original, e `building_can_place` está provado no emulador (`world-growth.md` 4.6). Alaska, Austria, Atlantis e Brazil (mapas `.MP2`, 30 dias) e Tutorial (30 dias) batem em 0 campos. No Example restam só a empresa de IA 5 (`companies[5]`, 5 campos) e seu chairman (`persons[2]`, 2 campos): o turno da IA. O consumo do RNG do ano inteiro (revisões das diretorias, gerentes candidatos, `ai_pick_engine`, crescimento das cidades com 2 prédios criados e 2 demolidos) acompanha o original. Fora do alcance do placar: o Example em 400 dias (dois fins de ano) diverge em muitos grupos por causa da IA e dos trens, e não isola o crescimento.

## Ganchos ligados em 30 dias

`company_month_step` (juros, manutenção, overhead, salários, dividendos trimestrais pagos aos acionistas), `event_fire_trigger` 0..4 (regras de cenário com efeitos de nível, variáveis, direitos de território, estado econômico e caixa; `tick/events.rs`), `margin_check_all`, `economy_state_step` (cotação de todas as empresas na mudança de estado), expiração de lotes (`lot_free` `0x466a60`), no fim do ano `company_annual_review` (porte de `review_ref`, provado no oráculo), `company_year_end` (arquivo de métricas/categorias/contas, sobrecarga, títulos, desdobramento), `person_year_step`, `industry_year_step`/`industry_year_reset`, velocidade de referência `0x573e08` com `ai_pick_engine`, fator de crescimento `0x5705d1`, zeragem dos contadores e `town_year_step` (`GrowthWorld`/`PickWorld`/`TownEnv`/`SiteEnv` sobre os pools do save; `tick/town.rs`). Descobertas contra o original: o passo mensal multiplica por 1/12 (não divide por 12); a métrica 5 do arquivo anual enxerga o preço novo na linha 1 (as linhas deslocam antes da gravação); `building_can_place` rejeita tile com `tile+0 & 8`; negar o último direito de um território (`0x4c8540`) zera um byte de resumo da empresa (`+0x422d/+0x422e` nos territórios 4 e 5; regra inferida).

## Ganchos que ainda faltam (por impacto)

O clone os conta em runtime (`rt2-advance` imprime `MISSING`, `World::hook_log`). Pedidos de especificação em `world-tick.md` seção 9.

1. ~~`history_snapshot` (`0x4e9770`)~~ **feito** (`world-tick.md` 3.4.1): falta só o texto das regras de gatilho 7 (`rule_collect_status`, sem regra 7 nos saves testados) e as globais da interface `0x574414`/`0x566ca4`, aproximadas pela primeira pessoa humana.
2. Turno da IA (`ai_monthly_turn` `0x436d60`): só os 4 sorteios de `ai_pick_engine` por empresa de IA ativa são reproduzidos (mantêm o RNG alinhado no Example); `company_ai_finance` (dividendo da empresa 5, `+0x154`, `+0x168`, caixa do chairman) e o resto estão no módulo `ai/` de outro agente.
3. ~~Marcação do terreno de `building_create`/`building_destroy`, `building_can_place` e texturas de campo~~ **feito** (`world-growth.md` 4.5, 4.5.1 e 4.6): `pool_88`, `0x429fc0`, `0x42a3e0`, `0x42a7d0` com as listas de sobreposição do tile, o trilho desenhado no teste de colocação e a luz do primeiro redesenho. Os New Game de campanha (#01 B&O, #02, #04, #07) batem em 0 campos; a repintura na demolição está portada mas sem caso no placar.
4. ~~`0x4beae0`~~ **feito** (zera `+0x59..+0x60` e os `u16` `+0x4b5/7/9`); elos de captação dos prédios novos (`0x4072c0`) feitos; resta `station_networks_rebuild` (`0x46a370`).
5. `company_set_chairman(0)` e `company_liquidate` na revisão (demissão/liquidação, contadas), chamadas de margem com saldo negativo, `0x573e02`, mensagens (`economy_headline`, `town_bonus_message`, desdobramento, manchetes históricas, janelas das regras), a inundação de território (`0x4c8a30`, efeito 233) e funções de cenário não especificadas (`PlayerStockValue`).
6. Dentro de `train_step` (contados como ganchos ausentes; nenhum é atingido em 30 dias do Example): acidente e roubo (`0x4ce730`, `0x4ceaa0`, `0x42fa30`), vagão genérico, troca de composição na estação, lote largado para transferência, lote que volta à pilha, `industry_receive_cargo` (`0x40b250`), mensagens e sons da empresa vista na interface (`0x46abf0`, `0x4d0870`, que gravam a alça de som `+0xab` e o bit `0x10` de `+0xbb` do trem e a vista em `estado+0x2e/+0x32`), `0x4d14b0`, `train_delete`, colocação de trem novo, ramos da flag `0x570229` (`trains.md` 10.8).

Suposições fora do save, anotadas em `crates/rt2-sim/src/tick.rs`: sem cheat Overtime, jogo local, conjunto de indústrias 0 (`0x5704ac`), `0x5748f8 == 0`; arquivos 0x233C não são suportados nos ganchos que leem o estado (offsets deslocados).
