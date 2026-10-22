# Placar de fidelidade (nível 2: save como oráculo)

Compara o clone com o original rodando no Wine (`re/scripts/wine/advance_save.py`, RNG semeado com 1234, passos de 320 min) campo a campo com `rt2-savediff` (campos voláteis mascarados). O clone usa `rt2-advance <in.GAM> <dias> 1234 <out.GAM>` (`crates/rt2-tools/src/bin/rt2-advance.rs`) sobre a **mesma entrada convertida pelo original** (`base_*.GAM` = o save avançado 0 dias no original, que já é 0x2348), para que os campos crus sejam comparáveis. Teste automatizado: `crates/rt2-sim/tests/worldloop.rs` (variáveis `RT2_DIR` e `RT2_FIDELITY_DIR`). Spec do laço: `docs/spec/world-tick.md`.

## Resultado

| Cenário | Dias | Resultado | Campos diferentes |
|---|---|---|---|
| Tutorial (sem trens, sem estações) | 1 | **igual** | 0 |
| Tutorial | 7 | quase igual | 3 (`objects[88,104,236].raw[0x8c..0x90]`) |
| Tutorial | 30 | quase igual | 3 (os mesmos) |
| Example save game | 1 | diferente | 1198 (o original muda 1200 contra a base; o clone só reproduz os 2 campos de relógio) |

Verificações extras: contagens de passos do Tutorial 7 dias batem com `world-tick.md` seção 6 (39 `sim_advance`, 387 recontagens e 387 produções, 0 RNG); em 30 dias o estado final do RNG é `0x051bc859`, igual ao original, e o passo econômico, o sorteio de bônus de cidade e os juros das pessoas reproduzem o original. Round-trip do salvamento: `Save::to_bytes` regrava byte a byte os saves 0x2348 do original (checksum `0x4ef4d0` incluso) e os dois `.GAM` 0x233C.

## Diferenças restantes

1. **Tutorial 7/30 dias, 3 campos**: os três "Port" (tipo 28: indústrias 88, 104 e 236) têm um lote de passageiros (carga 0). No original a recontagem das pilhas (`0x40a610`) o conta no slot 2 (`+0x8f = 1`); no clone, com a tabela de tipos do `.data` do exe, a carga 0 não casa com nenhum slot do Port. Medido com o jogo rodando: a linha 28 do conjunto 0 de `industry_type_table` (`0x541ae0`) **em memória difere do `.data` do exe** (slots com saídas 22, 2, passageiro modo 1, 20; não existe no exe nem em arquivo do jogo sem compressão). Algo a reescreve na carga; a origem é desconhecida. Lacuna de spec (`cargo.md` 7.1), não de código do laço.
2. **Example, 1 dia (1198 campos)**, por seção: `trains` 92, `track_pieces` 27, `pool_80` 6 (movimento do trem, `train_step`); `pool_78` 728 (animações `anim_object_step` e objetos criados pelos trens); `pool_70` 185, `structures` 153, `tiles` 7 (provavelmente efeitos de `train_step` em tiles; atribuição não verificada). Todos explicados por ganchos ausentes abaixo.

## Ganchos que faltam (por impacto)

O clone os conta em runtime (`rt2-advance` imprime `MISSING`, `World::hook_log`). Pedidos de especificação em `world-tick.md` seção 9.

1. `train_step` `0x4d0ce0` (`trains.md` 4.5): bloqueia qualquer save com trens (Example 1 dia: 54 chamadas).
2. `anim_object_step` `0x4283e0` (`world-tick.md` 2.1): spec existe; falta apagar o objeto (`0x466d20`) e a marca no tile. 594 chamadas no Example.
3. `station_recompute_supply_demand` `0x4bebf0` (`cargo.md` 8) e a notificação das estações em `industry_spawn_lot`: bloqueia qualquer save com estações a partir da primeira semana.
4. `company_month_step`/`company_annual_review`/`company_year_end` sobre o registro cru (`finance.md`; falta mapear o registro para `finance::Company`); `ai_monthly_turn` (RNG, `world-tick.md` 3.5, não especificado).
5. Fim de ano: `town_year_step` (regras em `growth.rs`/`world-growth.md`, faltam as implementações dos traits `GrowthWorld`/`PickWorld` sobre os pools do save), `industry_year_step`/`industry_year_reset`, `ai_pick_engine`, `history_snapshot`, contadores `0x573d05/09/0d`, desgaste das peças de trilho.
6. `event_fire_trigger` (regras de cenário, pode consumir RNG): 3 chamadas por mês no Tutorial, sem efeito visível ali.
7. Remoção de lote vencido (`0x466a60`), `margin_check_all`, `train_monthly_costs`, `person_year_step`, `train_year_step`, mensagens (`economy_headline`, `town_bonus_message`).

Suposições fora do save, anotadas em `crates/rt2-sim/src/tick.rs`: sem cheat Overtime, jogo local, conjunto de indústrias 0 (`0x5704ac`), `0x5748f8 == 0`; arquivos 0x233C não são suportados nos ganchos que leem o estado (offsets deslocados).
