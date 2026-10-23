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
| Example save game (9 trens, 11 estações) | 90 | **igual** | 0 |  |
| Example save game (9 trens, 11 estações) | 180 | **igual** | 0 |  |
| Example save game (9 trens, 11 estações) | 365 | **igual** | 0 |  |
| Example save game (9 trens, 11 estações) | 730 | **igual** | 0 |  |
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
| Mapa Lenin (English).MP2 (7 empresas, 2 trens, 94 estações) | 30 | **igual** | 0 |  |
| Mapa Lenin (English).MP2 (7 empresas, 2 trens, 94 estações) | 365 | **igual** | 0 |  |
| Mapa Lenin (English).MP2 (7 empresas, 2 trens, 94 estações) | 730 | **igual** | 0 |  |
| Mapa NewJapan.MP2 (2 empresas, 5 trens, 14 estações) | 30 | **igual** | 0 |  |
| Mapa NewJapan.MP2 (2 empresas, 5 trens, 14 estações) | 365 | **igual** | 0 |  |
| Mapa NewJapan.MP2 (2 empresas, 5 trens, 14 estações) | 730 | **igual** | 0 |  |
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
| #01 B&O iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 365 | **igual** | 0 |  |
| #01 B&O iniciado pela campanha (0x410de0), game_seed 4321, RNG 777 | 730 | **igual** | 0 |  |
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
| #E01 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 365 | **igual** | 0 |  |
| #E01 (expansão) iniciado pela campanha (0x410de0, conjunto 1), game_seed 4321, RNG 777 | 730 | **igual** | 0 |  |
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
| Example preparado: acidente de trem da IA (0x4ce730) e remoção do destroço (0x4cdd50) | 30 | **igual** | 0 |  |
| Example preparado: acidente de trem da IA (0x4ce730) e remoção do destroço (0x4cdd50) | 60 | **igual** | 0 |  |
| Example preparado: roubo de trem (0x4ceaa0, 0x42fa30), 0x573e02 = 3000 | 30 | **igual** | 0 |  |
| Example preparado: roubo de trem (0x4ceaa0, 0x42fa30), 0x573e02 = 3000 | 60 | **igual** | 0 |  |
| Example preparado: ladrões presos (0x4cf390, notícia, 0x573e02 / 3) | 60 | **igual** | 0 |  |
| Example preparado: 0x573e02 abaixo de 0x573d9e na virada do ano (sim_year_end 0x4a50f2) | 30 | **igual** | 0 |  |
| Example preparado: a diretoria demite o chairman da IA na revisão anual (0x438900) | 30 | **igual** | 0 |  |
| Example preparado: os credores liquidam a empresa da IA na revisão anual (0x438170) | 30 | **igual** | 0 |  |
| Example preparado: os credores liquidam a empresa da IA na revisão anual (0x438170) | 60 | **igual** | 0 |  |
| Example preparado: a pessoa da IA com $5 milhões compra ações (0x4a8af0) | 7 | **igual** | 0 |  |
| Example preparado: a pessoa da IA com $5 milhões compra ações (0x4a8af0) | 60 | **igual** | 0 |  |
| Example preparado: a IA troca a locomotiva de dois trens nos trilhos (0x4d4e90, 0x413550) | 16 | **igual** | 0 |  |
| Example preparado: a IA troca a locomotiva de dois trens nos trilhos (0x4d4e90, 0x413550) | 60 | **igual** | 0 |  |
| Example preparado: dois acidentes de trens do jogador (empresa da interface, 0x4ce730) | 30 | **igual** | 0 |  |
| Example preparado: dois acidentes de trens do jogador (empresa da interface, 0x4ce730) | 60 | **igual** | 0 |  |
| Example preparado: demolição de uma fazenda no fim do ano (0x4091f0, repintura dos campos) | 30 | diferente | 11 | `tiles` (11) |

<!-- fidelity:state {"#09 New Game|0": {"equal": true}, "#09 New Game|7": {"equal": true}, "#10 New Game|0": {"equal": true}, "#10 New Game|7": {"equal": true}, "#11 New Game|0": {"equal": true}, "#11 New Game|7": {"equal": true}, "#12 New Game|0": {"equal": true}, "#12 New Game|7": {"equal": true}, "#13 New Game|0": {"equal": true}, "#13 New Game|7": {"equal": true}, "#14 New Game|0": {"equal": true}, "#14 New Game|7": {"equal": true}, "#15 New Game|0": {"equal": true}, "#15 New Game|7": {"equal": true}, "#16 New Game|0": {"equal": true}, "#16 New Game|7": {"equal": true}, "#17 New Game|0": {"equal": true}, "#17 New Game|7": {"equal": true}, "#18 New Game|0": {"equal": true}, "#18 New Game|7": {"equal": true}, "#E01 New Game|0": {"equal": true}, "#E01 New Game|365": {"equal": true}, "#E01 New Game|7": {"equal": true}, "#E01 New Game|730": {"equal": true}, "#E02 New Game|0": {"equal": true}, "#E02 New Game|7": {"equal": true}, "#E03 New Game|0": {"equal": true}, "#E03 New Game|7": {"equal": true}, "#E04 New Game|0": {"equal": true}, "#E04 New Game|7": {"equal": true}, "#E05 New Game|0": {"equal": true}, "#E05 New Game|7": {"equal": true}, "#E06 New Game|0": {"equal": true}, "#E06 New Game|7": {"equal": true}, "#E07 New Game|0": {"equal": true}, "#E07 New Game|7": {"equal": true}, "#E08 New Game|0": {"equal": true}, "#E08 New Game|7": {"equal": true}, "#E09 New Game|0": {"equal": true}, "#E09 New Game|7": {"equal": true}, "#E10 New Game|0": {"equal": true}, "#E10 New Game|7": {"equal": true}, "#E11 New Game|0": {"equal": true}, "#E11 New Game|7": {"equal": true}, "#E12 New Game|0": {"equal": true}, "#E12 New Game|7": {"equal": true}, "#E13 New Game|0": {"equal": true}, "#E13 New Game|7": {"equal": true}, "#E14 New Game|0": {"equal": true}, "#E14 New Game|7": {"equal": true}, "#E15 New Game|0": {"equal": true}, "#E15 New Game|7": {"equal": true}, "#E16 New Game|0": {"equal": true}, "#E16 New Game|7": {"equal": true}, "#E17 New Game|0": {"equal": true}, "#E17 New Game|7": {"equal": true}, "#E18 New Game|0": {"equal": true}, "#E18 New Game|7": {"equal": true}, "Alaska|1": {"equal": true}, "Alaska|30": {"equal": true}, "Alaska|7": {"equal": true}, "Arrest|60": {"equal": true}, "Atlantis|1": {"equal": true}, "Atlantis|30": {"equal": true}, "Atlantis|7": {"equal": true}, "Austria|1": {"equal": true}, "Austria|30": {"equal": true}, "Austria|7": {"equal": true}, "B&O New Game|0": {"equal": true}, "B&O New Game|365": {"equal": true}, "B&O New Game|7": {"equal": true}, "B&O New Game|730": {"equal": true}, "B&O.MAP (carga)|0": {"equal": true}, "B&O.MAP (carga)|7": {"equal": true}, "Birmingham New Game|0": {"equal": true}, "Birmingham New Game|7": {"equal": true}, "Brazil|1": {"equal": true}, "Brazil|30": {"equal": true}, "Brazil|7": {"equal": true}, "Bridges New Game|0": {"equal": true}, "Bridges New Game|7": {"equal": true}, "Britain.MAP (carga)|0": {"equal": true}, "Britain.MAP (carga)|7": {"equal": true}, "CrashHuman|30": {"equal": true}, "CrashHuman|60": {"equal": true}, "Crash|30": {"equal": true}, "Crash|60": {"equal": true}, "Cross New Game|0": {"equal": true}, "Cross New Game|7": {"equal": true}, "Engine|16": {"equal": true}, "Engine|60": {"equal": true}, "Example (conversao)|0": {"equal": true}, "Example (conversao)|7": {"equal": true}, "Example|1": {"equal": true}, "Example|180": {"equal": true}, "Example|30": {"equal": true}, "Example|365": {"equal": true}, "Example|7": {"equal": true}, "Example|730": {"equal": true}, "Example|90": {"equal": true}, "FieldDemolition|30": {"equal": false}, "Fire|30": {"equal": true}, "Hannibal New Game|0": {"equal": true}, "Hannibal New Game|7": {"equal": true}, "Lenin|30": {"equal": true}, "Lenin|365": {"equal": true}, "Lenin|730": {"equal": true}, "Liquidate|30": {"equal": true}, "Liquidate|60": {"equal": true}, "NewJapan|30": {"equal": true}, "NewJapan|365": {"equal": true}, "NewJapan|730": {"equal": true}, "Orient New Game|0": {"equal": true}, "Orient New Game|7": {"equal": true}, "Silver New Game|0": {"equal": true}, "Silver New Game|7": {"equal": true}, "Stocks|60": {"equal": true}, "Stocks|7": {"equal": true}, "TheftYear|30": {"equal": true}, "Theft|30": {"equal": true}, "Theft|60": {"equal": true}, "Tutorial (conversao)|0": {"equal": true}, "Tutorial (conversao)|7": {"equal": true}, "Tutorial|1": {"equal": true}, "Tutorial|30": {"equal": true}, "Tutorial|7": {"equal": true}, "White New Game|0": {"equal": true}, "White New Game|7": {"equal": true}} -->
<!-- fidelity:end -->

As saídas do original para o Example foram geradas com `trace_save.py` (sem breakpoints) a partir de `base_ex.GAM`, semente 1234 para o RNG do jogo **e** para o rand do CRT (que sorteia a fumaça gravada em `pool_78`; `rt2-advance` aplica `Command::SeedCrtRng`). `rt2-savediff` mascara os bits 4-15 das flags dos passos de rota, que o pathfinder do original preenche com memória de `malloc` não inicializada (`trains.md` 10.7). Testes: `example_one_day_is_equal` e `example_seven_days` em `crates/rt2-sim/tests/worldloop.rs` (precisam de `base_ex.GAM`, `orig_ex_1.GAM`, `orig_ex_7.GAM` em `RT2_FIDELITY_DIR`).

Caminhos raros do trem (chegada com entrega e receita, carga na estação, saída, custos mensais, passo anual; a quebra bate exceto pelos artefatos de som e vista da empresa vista na interface) são verificados por **snapshots de função** do original (`re/scripts/wine/snap_call.py`, `docs/dynamic-analysis.md` 6): o jogo é gravado logo antes e logo depois de uma chamada, e `crates/rt2-sim/tests/trainstep_snapshots.rs` roda o port sobre o primeiro save e compara com o segundo. Os 9 snapshots atuais (Example, até 120 dias) são idênticos, inclusive os estados finais dos dois geradores.

Verificações extras: contagens de passos do Tutorial 7 dias batem com `world-tick.md` seção 6 (39 `sim_advance`, 387 recontagens e 387 produções, 0 RNG); em 30 dias o estado final do RNG é `0x051bc859`, igual ao original, e o passo econômico, o sorteio de bônus de cidade e os juros das pessoas reproduzem o original. Round-trip do salvamento: `Save::to_bytes` regrava byte a byte os saves 0x2348 do original (checksum `0x4ef4d0` incluso) e os dois `.GAM` 0x233C.

## Longo prazo (90 dias a 2 anos)

Casos longos no placar: Example 90/180/365/730 dias, `Lenin (English).MP2` (7 empresas, 2 trens, 94 estações, regras de escolha e de gatilho 7) e `NewJapan.MP2` (2 empresas, 5 trens) em 30/365/730, #01 B&O (campanha) e #E01 (Second Century) iniciados por New Game em 365/730. Bisseção: os dois lados avançados dia a dia (dias **pares**: 1440/320 = 4,5 passos, um N ímpar termina com meio passo e sai da trajetória dos pares), o primeiro campo diferente e o rastreio do original (`trace_save.py` com uma variante que pula os primeiros dias sem breakpoints, `snap_call.py` na função suspeita). Divergências encontradas, na ordem em que apareceram, e o que foi feito:

1. **Example, 90 dias: alça de som e vista da interface** (`trains[*]+0xab`, bit `0x10` de `+0xbb`; estado `+0x2e..+0x35`). A quebra de um trem da empresa da interface (`0x4ce4c0`) toca um som em laço (`0x4d0870`, alça do sistema de áudio) e a caixa de mensagem (`0x46aa90` -> `0x466230`) abre a vista do trem (`0x570516`/`0x57051a`). Estado de interface: `rt2-savediff` ganhou máscaras de bits (`savefields::BIT_MASKS`) que escondem só esses bits, sem esconder o resto do bloco.
2. **Example, 163 dias: vagão novo fora dos trilhos.** Na troca de composição numa parada (`0x4d1bb0`), `vehicle_init` (`0x414220`) põe o vagão acrescentado atrás de outro nos trilhos na hora (`0x413550`, ramo de vagão), o que estende o intervalo reservado `+0x97`; o port deixava para o próximo movimento. Corrigido em `build/train.rs`.
3. **Example, 1º fim de ano (dia 356): `+0x572` de cada território** (u16 logo depois do vetor `+0x51a[44]`) é zerado por `company_year_end` (`0x43d800`); faltava no port.
4. **NewJapan, 1995-09-01: manchete da economia.** Na mudança de estado, `0x46a010` posta a manchete como notícia (`news_post` `0x4a42f0`: um `crt_rand_range(0, 10000)`, `0x5707bd = 1`, `0x5707be += 1`) fora do editor e com os bytes do cabeçalho `0x570238`/`0x57023a` zerados; o clone só emitia a mensagem.
5. **NewJapan, fim de 1995: luz dos tiles** (`tile+6 & 0x20`) em volta dos lotes nivelados. O original roda a passada de luz (`0x417c40`) ao pintar o mapa: uma vez depois da carga (o Load Game pinta; a união do carregador é limpa) e no gravador (a prévia), sobre a união acumulada desde então (`0x417770`). O clone faz o mesmo (`World::from_save_seeded`, `World::to_save`); antes só rodava no fim do New Game.
6. **NewJapan, fim de 1995: desdobramento da empresa da interface** vira notícia (`0x43d800`, opção `0x5706ab`, mesma escrita de 4).
7. **Lenin, 1º fim de ano: texto de status das regras de gatilho 7** no registro histórico (`+0x56`, `rule_collect_status` `0x458210`): portado (o motor anexa ao buffer `[0x5665e0]` antes do teste de fim de jogo, separador LNG 2 `"\n\n"`); `(TRIGGER_TF)` é LNG 3317/3318 `True`/`False` (era `true`/`false`); as regras com o jogador/empresa locais (`+0x34f`/`+0x350`) usam a primeira pessoa humana (eram 0, nunca batiam).
8. **Lenin, fev/1919: gerente da IA** (`company+0x13c/+0x140/+0x144`). `ai_review_manager` (`0x436880`) e `ai_manager_value` (`0x436210`, 22 efeitos pela tabela de saltos do asm, constantes do `.data`) portados em `ai/engine.rs`, com `company_set_manager(g, 0)` e os novos candidatos (`0x438450`) no turno da IA.
9. **#E01, 1933-01-01: `GameYear>1932` falso.** As funções de cenário liam a data do bloco de estado cru, que o clone só atualizava ao gravar; `set_clock` agora escreve minutos, data e hora no estado (todas as leituras cruas veem a data viva). No mesmo caso, `(TRIGGER_MONEY)` sem separadores: `format_money` (`0x475da0`) / `format_number` (`0x475980`) portados (`$2,531,907`; `K`/`M` acima do limite de dígitos).
10. **Example, fim de 1877 (dia 695): prédio de tipo 0 criado no jogo** fica com lucratividade 0 no clone; `industry_year_reset` (`0x40b7c0`) recalcula todo prédio ativo (`0x40a610`: `+0x4b = ftol(+0x93*100 + 50)` = 100), o port pulava o tipo 0.
11. **Example, 1878-01-17 (dia 707): trem quebrado desacelera diferente.** Com o trem parado por quebra `0x4d00c0` não escreve a saída `restricted`, e `0x4d0400` lê o que está na pilha: medido no original, `0x02f3f10b` (o `esi` de `train_step` salvo pelo prólogo de `0x412900`, a fumaça do trem anterior), verdadeiro. O clone usava o valor da chamada anterior; agora usa verdadeiro (`tau >> 4`).
12. **Example, dia 510: bytes velhos de vagões.** `vehicle_init` não inicializa `+0x13` (comprimento do trecho) nem `+0x38` (quadro da biela), e `pool_free` não limpa o registro: um vagão novo num slot reaproveitado começa com os bytes do vagão liberado ali. O clone guarda os registros liberados (`Globals::freed_cars`).
13. **Example, dia 529: som da caixa registradora.** Numa entrega da empresa da interface, `0x46abf0` toca `0x47d100` (`clamp(v/25000, 1, 10)` vezes, `v` = receita menos tarifas) e sorteia a variante com `crt_rand_range(0, 2)`, que desloca a fumaça. O valor vem da cópia da mensagem em `0x574430`, gravada só quando a caixa de mensagem a aceita, e a caixa descarta mensagens de prioridade menor por 9 s de relógio de parede depois de uma quebra: **o próprio original dava resultados diferentes conforme a velocidade da máquina** (um breakpoint a mais no gdb mudava a fumaça do Example em 366 dias). Agora `advance_save.py`/`trace_save.py`/`snap_call.py` mantêm a janela aberta (`[0x574d20] = 0` na entrada de `0x46aa90`) e o clone sorteia em toda entrega com `v > 0` (suposição: opção de som `[0x574bc8]+0x224 != 2`). A resposta automática das escolhas modais passou a ser opcional (`--skip-choices`, só no Lenin).

Eventos raros (saves preparados, `docs/dynamic-analysis.md` 5.9; casos `Crash`, `CrashHuman`, `Theft`, `Arrest`, `TheftYear`, `Fire`, `Liquidate`, `Stocks`, `Engine`, `FieldDemolition`): acidentes e remoção do destroço (`0x4ce730`, `0x4cdd50`), roubo com a busca de cidade (`0x4ceaa0`, `0x42fa30`), prisão e a recuperação anual de `0x573e02`, demissão do chairman e liquidação na revisão anual (`0x438900`, `0x438170` com `f = 0`), a bolsa da IA (`0x4a8af0`) e a troca de locomotiva no trilho (`0x4d4e90`). Ao trocar as sementes apareceram duas divergências antigas do fim de ano, corrigidas: `0x430da0` zera `+2/+4` do prédio demolido (o slot reaproveitado herda os bytes) e o crescimento das cidades (`0x430540`) lê `0x570713` vivo. Ainda não exercitados: fusões (só pelo comando `0x22`; as propostas `0x574ca0` não ficam no save; cobertas pelos snapshots de comando), o aviso de recorde de velocidade (`0x46abf0`: só enfileira um diálogo da interface em `0x574c1c`, fora do `sim_advance`), a inundação (`0x4c8a30`, efeito 233: falta portar o modo 3 de `0x490490`, que apaga trilho, trens e prédios), a demolição de estações no confisco e na liquidação com `f != 0` (`0x40adb0` em prédio de estação) e os eventos históricos de setembro (`0x406080`, janela modal). `FieldDemolition` diverge: depois de demolir uma fazenda (tipo 0x10) o clone deixa sobreposições de chão em 11 tiles (`tile+2 & 0x40`, `tile+6 & 0x80`) que o original limpa (`0x4091f0` -> `0x42a7d0`).

## Diferenças restantes

1. ~~Tutorial 7/30 dias, 3 campos~~ **resolvido**: a linha 28 (Port) e as taxas das casas da `industry_type_table` são reescritas na carga por `industry_load_fixups` (`0x4719e0`), que copia o Port do cabeçalho do save (`cargo.md` 7.1.1). Implementado em `rt2_sim::industry::apply_load_fixups`; Tutorial 7 e 30 dias dão 0 campos diferentes.
2. ~~Example, 1 dia (1198 campos)~~ **resolvido**: `train_step` (`0x4d0ce0`) e `anim_object_step` (`0x4283e0`) portados função a função (`crates/rt2-sim/src/trainstep/`, spec em `trains.md` seção 10): movimento pela rota, look-ahead e reservas de via (`pool_80`, donos `+7/+9` das peças), pathfinder em modo trem (`0x460850`, fila de baldes), parada na estação, fumaça (`0x412900`/`0x412a90`, rand do CRT) e as cadeias de estruturas dos tiles.
3. ~~Example, 7 dias (351 campos em `stations`)~~ **resolvido**: `station_recompute_supply_demand` (`0x4bebf0`, `crates/rt2-sim/src/trainstep/supply.rs`) no passo semanal e mensal e a parte das estações de `industry_spawn_lot` (`0x468af0`, `+0x4b5`).
4. ~~Example, 30 dias (4684 campos)~~ **reduzido a 7**: os ganchos mensais e anuais estão ligados ao estado cru do save, o registro anual `history_snapshot` (`0x4e9770`, `world-tick.md` 3.4.1) e a marcação do terreno de `building_create`/`building_destroy` (`world-growth.md` 4.5: estruturas dos tiles, nivelamento da elevação, flags de terreno, elos de estação, `0x4beae0`) seguem o original, e `building_can_place` está provado no emulador (`world-growth.md` 4.6). Alaska, Austria, Atlantis e Brazil (mapas `.MP2`, 30 dias) e Tutorial (30 dias) batem em 0 campos. No Example restam só a empresa de IA 5 (`companies[5]`, 5 campos) e seu chairman (`persons[2]`, 2 campos): o turno da IA. O consumo do RNG do ano inteiro (revisões das diretorias, gerentes candidatos, `ai_pick_engine`, crescimento das cidades com 2 prédios criados e 2 demolidos) acompanha o original. Casos longos: seção "Longo prazo".

## Ganchos ligados em 30 dias

`company_month_step` (juros, manutenção, overhead, salários, dividendos trimestrais pagos aos acionistas), `event_fire_trigger` 0..4 (regras de cenário com efeitos de nível, variáveis, direitos de território, estado econômico e caixa; `tick/events.rs`), `margin_check_all`, `economy_state_step` (cotação de todas as empresas na mudança de estado), expiração de lotes (`lot_free` `0x466a60`), no fim do ano `company_annual_review` (porte de `review_ref`, provado no oráculo), `company_year_end` (arquivo de métricas/categorias/contas, sobrecarga, títulos, desdobramento), `person_year_step`, `industry_year_step`/`industry_year_reset`, velocidade de referência `0x573e08` com `ai_pick_engine`, fator de crescimento `0x5705d1`, zeragem dos contadores e `town_year_step` (`GrowthWorld`/`PickWorld`/`TownEnv`/`SiteEnv` sobre os pools do save; `tick/town.rs`). Descobertas contra o original: o passo mensal multiplica por 1/12 (não divide por 12); a métrica 5 do arquivo anual enxerga o preço novo na linha 1 (as linhas deslocam antes da gravação); `building_can_place` rejeita tile com `tile+0 & 8`; negar o último direito de um território (`0x4c8540`) zera um byte de resumo da empresa (`+0x422d/+0x422e` nos territórios 4 e 5; regra inferida).

## Ganchos que ainda faltam (por impacto)

O clone os conta em runtime (`rt2-advance` imprime `MISSING`, `World::hook_log`). Pedidos de especificação em `world-tick.md` seção 9.

1. ~~`history_snapshot` (`0x4e9770`)~~ **feito** (`world-tick.md` 3.4.1): o texto das regras de gatilho 7 (`rule_collect_status`) também (Lenin, "Longo prazo" 7); restam as globais da interface `0x574414`/`0x566ca4`, aproximadas pela primeira pessoa humana.
2. Turno da IA (`ai_monthly_turn` `0x436d60`): a revisão do gerente nos anos ímpares (`0x436880`/`0x436210`, "Longo prazo" 8) e os sorteios de `ai_pick_engine` são reproduzidos; `company_ai_finance` (dividendo da empresa 5, `+0x154`, `+0x168`, caixa do chairman) e o resto estão no módulo `ai/` de outro agente.
3. ~~Marcação do terreno de `building_create`/`building_destroy`, `building_can_place` e texturas de campo~~ **feito** (`world-growth.md` 4.5, 4.5.1 e 4.6): `pool_88`, `0x429fc0`, `0x42a3e0`, `0x42a7d0` com as listas de sobreposição do tile, o trilho desenhado no teste de colocação e a luz do primeiro redesenho. Os New Game de campanha (#01 B&O, #02, #04, #07) batem em 0 campos; a repintura na demolição está portada mas sem caso no placar.
4. ~~`0x4beae0`~~ **feito** (zera `+0x59..+0x60` e os `u16` `+0x4b5/7/9`); elos de captação dos prédios novos (`0x4072c0`) feitos; resta `station_networks_rebuild` (`0x46a370`).
5. `company_set_chairman(0)` e `company_liquidate` na revisão (demissão/liquidação, contadas), chamadas de margem com saldo negativo, `0x573e02`, mensagens (`economy_headline`, `town_bonus_message`, desdobramento, manchetes históricas, janelas das regras), a inundação de território (`0x4c8a30`, efeito 233) e funções de cenário não especificadas (`PlayerStockValue`).
6. Dentro de `train_step` (contados como ganchos ausentes; nenhum é atingido em 30 dias do Example): acidente e roubo (`0x4ce730`, `0x4ceaa0`, `0x42fa30`), vagão genérico, troca de composição na estação, lote largado para transferência, lote que volta à pilha, `industry_receive_cargo` (`0x40b250`), mensagens e sons da empresa vista na interface (`0x46abf0`, `0x4d0870`, que gravam a alça de som `+0xab` e o bit `0x10` de `+0xbb` do trem e a vista em `estado+0x2e/+0x32`), `0x4d14b0`, `train_delete`, colocação de trem novo, ramos da flag `0x570229` (`trains.md` 10.8).

Suposições fora do save, anotadas em `crates/rt2-sim/src/tick.rs`: sem cheat Overtime, jogo local, conjunto de indústrias 0 (`0x5704ac`), `0x5748f8 == 0`; arquivos 0x233C não são suportados nos ganchos que leem o estado (offsets deslocados).
