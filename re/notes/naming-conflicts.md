# Conflitos de nomes a resolver

Gerado na mesclagem de 2026-10-22. O nome mantido em `re/symbols.toml` é o canônico. As specs que usam o alias devem passar a usar o nome canônico.

| Endereço | Canônico (mantido) | Alias | Origem do alias | Observação |
|---|---|---|---|---|
| `0x574c58` | `list_links` | `pool_58` | map-gam.md | |
| `0x574c5c` | `list_towns` | `place_pool` | map-gam.md | O pool inclui cidades, regiões e mares, então `list_places` seria mais preciso |
| `0x574c60` | `list_stations` | `station_pool` | map-gam.md | |
| `0x574c64` | `list_trains` | `train_pool`, `train_list` | map-gam.md, trains.md | |
| `0x574c68` | `list_persons` | `person_pool` | map-gam.md | |
| `0x574c6c` | `list_companies` | `company_pool` | map-gam.md | |
| `0x574c70` | `list_cars` | `pool_70`, `vehicle_list` | map-gam.md, trains.md | |
| `0x574c74` | `list_industries` | `map_object_pool` | map-gam.md | **Verificar**: indústrias ou objetos de mapa em geral? |
| `0x574c7c` | `list_lots` | `pool_7c` | map-gam.md | |
| `0x574ca8` | `list_territories` | `territory_pool` | map-gam.md | |
| `0x46b480` | `effect_multiplier` | `modifier_factor` | trains.md | |
| `0x4d4200` | `train_ledger_add` | `train_charge` | trains.md | |
| `0x4f18b0` | `list_record` | `collection_get` | trains.md | |
| `0x5707c6` | `effect_global` | `modifier_pct` | trains.md | |

Nota `0x574c74` (alias não substituído, em aberto): cargo.md (tabela de listas) e time.md (7.3) descrevem o pool como "indústrias e casas" e aplicam a produção periódica (`0x40aa20`) e a recontagem de pilhas a cada registro com `+7 != 0`, o que favorece `list_industries`; map-gam.md (linha 128) lê os registros como "objetos de mapa" (387 no Tutorial, 13 no B&O, 0 no Britain.MAP), com `+8` u16 = estação quando `+7 == 0`, o que sugere um pool misto; a função exata continua incerta.

Aliases substituídos nas specs em 2026-10-22 (exceto 0x574c74).
