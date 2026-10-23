# Cenários: regras, condições, efeitos e vitória

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido no C exportado (`re/db/decomp/`) e no disassembly do rizin; as tabelas saem do `.data` do executável e as regras de 128 arquivos do jogo (`maps/*.MAP`, `maps/*.MP2`, `games/*.GAM`). O manual do editor (`editor3.txt`, `EDITOR2.TXT`) e `docs/reference/manual-rules.md` serviram só de pista (nomes dos efeitos, ordem de avaliação); as divergências estão na seção 8. Testes: `crates/rt2-sim/tests/scenarios.rs` (dados reais, sem o exe) e `crates/rt2-sim/tests/scenario_engine.rs` (motor em mundo simulado) e `crates/rt2-oracle/tests/scenarios.rs` (diferencial contra o exe emulado); o código (tabela de 118 tokens, tokenizador, compilador, redutor, agregação de classe, regras, efeitos, gatilhos) está em `crates/rt2-sim/src/scenario/`.

## 1. Resumo do modelo

Um **cenário** é uma lista de **regras** (pool `0x574c9c`, serializado no fim do `.MAP`/`.MP2`/`.GAM`, `docs/formats/map-gam.md` 5.6). Cada regra tem um **gatilho** (quando é avaliada: fim/início de ano ou mês, mudança de trilho/estação, empresa criada, briefing), uma **condição** (texto como `GameYear>=1850 and CompanyCash(IsHighestInClass)`, compilada em dois vetores de `i32`), uma **mensagem** (diálogo, jornal ou escolha) e **dois conjuntos de efeitos** (um por botão da escolha; 350 bytes cada). Ao avaliar, a regra pode ser repetida para cada jogador, empresa e território; a cada combinação verdadeira ela mostra a mensagem e aplica os efeitos, que mexem nos **multiplicadores de efeito** (`effect_multiplier` `0x46b480`, `cargo.md` 5), no caixa, na disponibilidade de motores/gerentes, nos direitos de território, em variáveis livres e no **fim de jogo** (bronze/prata/ouro/derrota).

Não existe tabela de metas separada: as metas de medalha dos 18 cenários da campanha e dos avulsos são regras comuns cujos efeitos são `Game Over - Win 1/2/3/Loss` (efeitos 1 a 4). A progressão da campanha é o registro da melhor medalha por cenário (seção 5).

Mapa de funções (todas `sim`, exceto onde indicado; nomes em `re/pending/scenarios.toml`):

| Endereço | Nome | Papel |
|---|---|---|
| `0x457fe0` | `event_fire_trigger(k)` | percorre o pool e avalia as regras de gatilho `k` (2.2) |
| `0x455f30` | `rule_evaluate` | laços jogador/empresa/território, condição, mensagem, efeitos, regras de desfazer (2.3, 2.4, 4) |
| `0x454270` | `rule_reduce` | redutor por passes de precedência (3.2) |
| `0x454ae0` | `rule_function_value` | valor de uma função da linguagem, com parâmetro de classe (3.3) |
| `0x453e70` | `rule_split_words` | divisor de palavras (3.1) |
| `0x454060` | `rule_compile` | palavras para (ids, valores) (3.1) |
| `0x4ef3f0` | `parse_int` | número decimal/hex do compilador |
| `0x457a30` | `rule_set_condition` | compila, deriva as flags de escopo e faz uma avaliação de teste (`execute = 0`); usada pelo editor |
| `0x457c90` / `0x457b00` | `rule_read` / `rule_write` | leitura e gravação no arquivo |
| `0x457ee0` | `rule_copy` | cópia profunda de uma regra (editor) |
| `0x455e70` | `rule_show_message` | mostra diálogo/jornal/escolha, devolve o botão |
| `0x458210` | `rule_collect_status` | dispara as regras de gatilho 7 acumulando o texto no buffer `0x5665e0` (janela de briefing/status) |
| `0x458050` | `rule_warn_scopes` | (editor, `ui`) avisa sobre efeitos que exigem escopo que a regra não tem |
| `0x45c150` | `effect_money` | byte do controle deslizante para dólares (4.3) |
| `0x4e9280` | `game_over_screen` | tela de fim de jogo, medalha da campanha, placar (seção 5) |
| `0x46b9a0` | `stations_connected` | "conectado a" (3.3) |
| `0x4c0240` | `stations_visited` | contagem de estações visitadas |

## 2. Modelo de regra

### 2.1 Registro de 0x362 bytes e arquivo

Memória e arquivo (`rule_read` `0x457c90`, `rule_write` `0x457b00`): `0x362` bytes crus (versão `>= 0x2341`) ou `0x2C8` (anterior; o carregador converte para `0x362`: copia os `0x2C8` iniciais, move os efeitos de blocos de `0x111` para blocos de `0x15e` zerando o resto, e o miolo final `0x286..0x2C8` vai para `0x320..0x362`), seguidos de: `u32 n` + condição (`n` bytes com o `\0`), `i32[tokens]` (ids), `i32[tokens]` (valores), e quatro textos `u32 n` + bytes (mensagem, rótulo/legenda 1, rótulo 2, comentário). Os primeiros campos de ponteiro do registro são sobrescritos na leitura. O número de tokens `t` está em `+4` (`map-gam.md` 5.6).

Campos (offsets do formato `0x362`; os dos arquivos `.MAP` de `0x2C8` são iguais abaixo de `0x64` e `0x9A` menores a partir de `0x320`). Contagens de valores na coluna "dados" vêm dos 3790 registros de 128 arquivos.

| Offset | Tipo | Significado | Dados / prova |
|---|---|---|---|
| `+0x00` | ptr | texto da condição (`rule_set_condition`) | |
| `+0x04` | i32 | nº de tokens | igual ao tamanho dos vetores em 100% |
| `+0x08` / `+0x0c` | ptr | ids dos tokens / valores (`i32[t]`) | |
| `+0x10` | i32 | **modo de data**: != 0 só nas regras de desfazer geradas em tempo de execução (4.4); a condição é então "empresa == `+0x20`, pessoa == `+0x1c`, data >= (`+0x14` ano, `+0x18` mês)" e não há avaliação nem RNG | 0 nos 3790 |
| `+0x14` / `+0x18` | i32 | ano e mês (1..12) do modo de data | |
| `+0x1c` / `+0x20` | i32 | pessoa e empresa do modo de data | |
| `+0x24` | i32 | **testar contra cada empresa** (laço do meio, 2.3) | 2401 |
| `+0x28` | i32 | **testar contra cada jogador** (laço de fora) | 677 |
| `+0x2c` / `+0x30` | i32 | a condição usa funções de empresa / jogador (derivado por `0x457a30`, que também liga `+0x24`/`+0x28`) | 1837 / 371 |
| `+0x320` | i32 | a condição usa funções de território (derivado; liga `+0x324`) | 1059 |
| `+0x34` | i32 | **gatilho** 0..7 (2.2) | 0:391 1:529 2:1340 3:505 4:272 5:79 6:303 7:371 |
| `+0x38` / `+0x3c` | i32 | no laço de empresas, avaliar empresas **de IA** / **humanas** (`0x43c030`: pessoa da empresa com `+0x122 != 0` é humana) | 1983 / 3675 com 1 |
| `+0x40` / `+0x44` | i32 | no laço de jogadores, avaliar jogadores de IA / humanos (`pessoa+0x122`) | 3245 / 3729 |
| `+0x48` | i32 | **uma vez só** | 3023 |
| `+0x4c` | i32 | já disparou (setado em `0x456a1f`); só importa com `+0x48` | 0 em 3784 |
| `+0x50` / `+0x54` / `+0x58` / `+0x5c` | ptr | texto principal; rótulo do 1º botão (ou sub-título do jornal); rótulo do 2º botão; comentário/título (`Gold`, `Victory Conditions`...) | |
| `+0x60` | i32 | tipo da mensagem: 0 diálogo (`0x469320`), 1 jornal (`0x4a42f0`), 2 escolha (`0x4510a0`) | 2757 / 818 / 215 |
| `+0x64 + k*0x15e + e` | i8 | **efeito `e` (0..349) do conjunto `k` (0 = botão 1/único, 1 = botão 2)**; 0 = sem efeito; `0x9c` (-100) = "valor do gatilho" nas linhas que permitem | 5191 bytes não nulos, todos dentro de [min,max] da linha |
| `+0x324` | i32 | **testar contra cada território** | 1326 |
| `+0x328` | i16 | restringe o laço a um território (id; <= 0 = todos) | 0 em 2962; ids pequenos e 27 nos demais |
| `+0x32a` | i32 | gatilho 7: mostrar o texto mesmo com condição falsa (linha de status fixa) | 342 |
| `+0x32e` | i32 | mostrar a mensagem só na primeira combinação verdadeira da avaliação (`param_5` em `0x455f30`) | 743 |
| `+0x34f` | u8 | **avaliação local** (só a empresa do jogador local, `[0x574414]`): usa o `rand` do CRT, não aplica efeitos | 87 |
| `+0x350` | u8 | idem para o jogador local (`[0x566ca4]`) | 14 |
| `+0x351` | u8 | 0 qualquer partida, 1 só uma partida local, 2 só rede (ignorado em campanha, `[0x570234]`) | 0:2778 1:870 2:142 |
| `+0x352 + 4k` | i32 | **permanente** (1) ou temporário (0) para o conjunto `k` | 3654 / 3774 com 1 |
| `+0x35a + 4k` | i32 | duração em meses do efeito temporário do conjunto `k` | 1, 4, 6, 12, 24...60 |

Valores da regra de desfazer (4.4) e de uma regra nova do editor: `+0x38..+0x44 = 1`, `+0x48 = 1`, `+0x352..+0x35e = 1`.

### 2.2 Gatilhos (`+0x34`) e onde disparam

`event_fire_trigger(k)` (`0x457fe0`) não faz nada com `[0x5748fc] != 0` (editor) ou sem pool. Chamadores (por `re/db/decomp`; rótulos do editor em `editor3.txt`, "Frequency"):

| k | Rótulo do editor | Dispara em |
|---|---|---|
| 0 | início do ano (depois da virada) | fim de `sim_year_end` `0x4a4d00` (linha 216) e na carga/início de partida `0x479b20` |
| 1 | fim do ano | meio de `sim_year_end` (antes dos arquivos anuais; linha 81) |
| 2 | início do mês | `sim_month_end` `0x4a5320`, depois de avançar a data (duas vezes no código: mês 12 e demais) e em `0x479b20` |
| 3 | fim do mês | começo de `sim_month_end` |
| 4 | trilho/estação posto ou removido | `station_networks_rebuild` `0x46a370` (fim; só roda se o flag `[0x5794f0]` de mudança estava ligado) e `sim_month_end` (logo antes do 3, se `net_min_peer_version() > 0x232c`) |
| 5 | empresa criada | depois da criação de empresa por jogador (`0x4bdcd0`, ao comprar o controle) e pela IA (`0x4a8800`) |
| 6 | briefing (início da partida e botão de briefing) | `0x479b20` (início/carga) e `0x48c370` (botão `0x1fb`, com `[0x5665f0] = 1`); **não aplica efeitos** (4.2) |
| 7 | linhas de status do briefing (`Status: ...`) | `0x458210`, chamada pelas janelas de briefing/status (`0x4e9770`, `0x444e10`); acumula os textos no buffer `[0x5665e0]`; **não aplica efeitos** |

Em jogo local, ao carregar/iniciar (`0x479b20`) a ordem é `fire(6)`, `fire(0)`, `fire(2)`. Cada chamada percorre as regras na ordem do pool (inserção; regras de desfazer entram no fim) e, **depois de cada regra avaliada cujo gatilho coincide**, volta imediatamente se o estado de fim de jogo `[0x573d70]` mudou (nenhuma regra seguinte é avaliada).

### 2.3 Laços de avaliação (`rule_evaluate` `0x455f30`)

Parâmetros: `execute` (0 = só validar, `rule_set_condition`; 1 = disparo normal). Pseudocódigo (o decompilador reaproveita variáveis; a ordem abaixo vem do disassembly):

```
se [0x570234] == 0 (não campanha) e ((+0x351 == 1 e partida em rede) ou (+0x351 == 2 e partida local)): return 1 (pula)
_DAT_005665dc = 0                                           ; "último valor T2T" para (COMPANY_T2T_LOADS)
n_pessoas  = +0x28 ? 0x46a890() : 1                           ; jogadores do conjunto indexado [0x570775]
n_empresas = +0x24 ? 0x46a7e0() : 1                         ; empresas do conjunto indexado [0x570729]
n_terr     = +0x324 ? contagem do pool 0x574ca8 : 1
para p em 0..n_pessoas:                                      ; jogadores, ordem do conjunto 0x570779
  pessoa = +0x28 ? 0x46a8a0(p).id : 0
  se +0x28 e filtro: (+0x350 == 0: pessoa.+0x122 ? +0x44 : +0x40 ; senão pessoa == [0x566ca4]) falha -> próxima
  para c em 0..n_empresas:                                   ; empresas, ordem do conjunto 0x57072d
    empresa = +0x24 ? 0x46a7f0(c).id : 0
    se +0x24: (+0x34f == 0: 0x43c030(empresa) ? +0x3c : +0x38 ; senão empresa == [0x574414]) falha -> próxima
    se +0x28 e empresa.+0x23 != pessoa -> próxima
    para t em 0..n_terr:                                     ; territórios, ordem de lista do pool (0x4f18d0)
      terr = +0x324 ? id : 0 ;  se +0x328 > 0 e +0x328 != terr -> próximo
      condição = (+0x10 != 0) ? datado(empresa == +0x20, pessoa == +0x1c, data >= (+0x14,+0x18))
                              : rule_reduce(tokens, valores, empresa, pessoa, terr)        ; 3.2
      se avaliação deu erro: devolve 0 (nada mais é avaliado)
      se (condição != 0 ou (+0x32a e gatilho == 7)) e execute:
          se +0x48 e +0x4c e gatilho não in {6,7}: continua      ; "uma vez só" já disparou: a condição JÁ foi avaliada (o RNG já foi gasto)
          mensagem (2.4); +0x4c = 1; efeitos (4.2); regra de desfazer (4.4)
fim: anexa ao pool as regras de desfazer criadas (depois de todos os laços)
devolve 1
```

O laço de empresas usa o **conjunto indexado** (`0x57072d`, as empresas ativas) e o de jogadores o conjunto `0x570779`; os parâmetros de classe (3.3) percorrem o **pool inteiro** na ordem de lista (`0x4f18d0`). `rule_set_condition` (editor) avalia uma vez com `execute = 0` (e sorteia o RNG se houver `Random1To10000`).

### 2.4 Mensagens

Condições: texto principal (`+0x50`) com mais de 1 caractere e (`param_5 == 0` ou `+0x32e == 0`). O texto só aparece se `pessoa == [0x566ca4]`, ou `empresa == [0x574414]`, ou pessoa e empresa são ambas 0, ou o gatilho é 7 (o manual diz: "o texto não aparece se o evento só afeta outro jogador ou empresa que você não controla").

Substituições nos três textos (`+0x50`, `+0x54`, `+0x58`; troca de substring pela função `0x4f03e0`): `(PLAYER)` nome da pessoa (`+8`, ou texto LNG `0x2f` se 0), `(COMPANY)`/`(GESELLSCHAFT)` nome da empresa (`+4`, LNG `0x30` se 0), `(TERRITORY)` nome do território (`+4`, LNG `0x89b` se 0), `(COMPANY_LOADS)` `empresa+0x19e` (cargas, vida), `(COMPANY_T2T_LOADS)` `_DAT_005665dc` (último valor das funções de matriz de cargas `0x5b/0x5c/0x69/0x6a..0x6d`), `(NUMBER_OF_STATIONS_VISITED)` `0x4c0240()`, `(TRIGGER)` o valor da condição como número, `(TRIGGER_MONEY)` como dinheiro, `(TRIGGER_TF)` "true"/"false" (LNG `0xcf5`/`0xcf6`). (`EDITOR2.TXT` lista os cinco primeiros do TSC.)

Se alguma das flags de fim de jogo `+0x65..+0x68` (efeitos 1..4 do conjunto 0) está ligada, o texto vai para o buffer do quadro de fim de jogo `0x574b90` (só se `[0x573d70] == 0`) em vez de abrir um diálogo. Caso contrário `rule_show_message` (`0x455e70`) usa `+0x60`: diálogo, jornal (título `+0x50`, sub-título `+0x54`) ou escolha (botões `+0x54`/`+0x58`; o retorno `1` = segundo botão seleciona o conjunto de efeitos 1; em rede (>= 2 jogadores) a escolha é sempre 0 sem janela). Com o buffer `[0x5665e0]` ativo (gatilho 7) o texto é anexado a ele, com o separador `"\n\n"` (LNG 2).

## 3. Linguagem de condição

### 3.1 Tokenização (`0x453e70`, `0x454060`, `0x4ef3f0`) [V]

- **Palavras**: separadas por espaço, tab, CR, LF; além disso uma palavra termina antes e depois de `( ) + - * /`, e entre um caractere de comparação (`> < =`) e qualquer outro. Consequências: `a>=3`, `CompanyCash(IsHighestInClass)` e `1-2` se dividem; `>=` é uma palavra; **`!=` vira `!` e `=`** (palavra `!` inexistente: o compilador rejeita; nenhum arquivo do jogo usa `!=`); `> =` com espaço vira `>` e `=` (erro de forma em tempo de avaliação: um caso em `Dominican Republic.MP2`).
- **Compilação** (`rule_compile`): cada palavra é comparada sem distinção de maiúsculas (`strcmpi`) com o nome e o alias de cada entrada da tabela (3.5, primeiro acerto vale; `constant` também casa e vale 0), senão vira número (`0x4ef3f0`: decimal, `0x` + hex, qualquer `-` nega, `i32` com wrap; qualquer outro caractere rejeita) e produz o token `0x5a` (`CONSTANT`) com o valor; senão erro (LNG `0x7a`/`0x7b`). Número negativo escrito `-5` é `-` seguido de `5` (sinal unário no redutor).
- **Validado**: nos 3790 registros, `compile(condição)` reproduz exatamente os dois vetores gravados (`tokens_a`, `tokens_b`) e todos os ids são < 118 (103 ids distintos usados).

### 3.2 Redutor por passes (`rule_reduce` `0x454270`) [V; diferencial]

`rule_reduce(this, n, tokens, valores, empresa, pessoa, território, msg, &saída)` reduz o vetor (token, valor) no lugar, em **8 passes** (0 a 7); em cada passe percorre da esquerda para a direita os tokens cuja `prec` na tabela (`+8` da entrada) é igual ao passe. O resultado é o valor do único token que sobra.

| Passe | Tokens | Ação |
|---|---|---|
| 0 | classe (`IsHighestInClass`, `IsLowestInClass`) | padrão `função ( classe )` (3 tokens após a função), senão erro LNG `0x7c`; avalia com a classe (3.3) |
| 1 | funções | se a função tem parâmetro numérico (`P`): padrão `f ( constante )`, senão erro `0x898`; avalia (`rule_function_value`) |
| 2 | `(` | acha o `)` casado (erro `0x7e` se faltar), vazio é erro `0x80`; reduz o interior recursivamente |
| 3 | constante | só passa pelo avaliador (devolve o próprio valor) |
| 4 | `*` `/` | binário; `/` com divisor 0 usa 1; divisão com sinal de 32 bits |
| 5 | `+` `-` | binário se o token anterior é constante; senão **sinal unário** sobre a constante seguinte |
| 6 | `>= <= > < = !=` | 1 ou 0 |
| 7 | `AND` `OR` | 1 se ambos (`AND`) / algum (`OR`) é != 0 |

Faltando operando (token binário sem constante de cada lado, exceto o unário `+`/`-`): erro `0x7f`; `)` solto: erro `0x7d`; sobra de tokens depois do passe 7: erro `0x89a`. Em todos os erros a avaliação devolve 0 e a mensagem de erro é formatada num buffer (LNG `0x7a..0x7f`, `0x80`, `0x81`, `0x898..0x89a`, `0xb2b`). Observações:

- **Todas as funções são avaliadas antes de qualquer operador**, na ordem do texto: `AND`/`OR` não fazem curto-circuito e **todo `Random1To10000` sorteia** (3.4).
- Precedência incomum que o código impõe: `*` `/` antes de `+` `-` antes das comparações antes de `AND`/`OR`, sem associatividade especial (esquerda para direita dentro do passe); o unário não funciona depois de `*` ou `/` (`3 * -2` é erro `0x7f`; `-2 * 3` vale -6).
- Comparação com o resultado de outra comparação é permitida (booleanos 0/1 são inteiros): `TerritoryConnectedToTerritory(5)=FALSE` aparece em dados reais.
- Seis regras de mapas de usuário são mal formadas e **falham no original** com o mesmo código (confirmado no exe): `#08Ori~1.MAP` regra 26 (falta `and`, `0x89a`), `Do the Hop.MP2` 12 (`0x89a`), `Dominican Republic.MP2` 28 (`> =`, `0x7f`), `Durango.MP2` 37 (`+` sem operando e `=` mal escrito, `0x898`), `Florida 1900.MP2` 6 (parêntese aberto, `0x7e`), `Wooden Limbs & Iron Muscles.MP2` 5 (`f(13)(IsHighestInClass)`, `0x7c`).

### 3.3 Avaliador de funções (`rule_function_value` `0x454ae0`)

`rule_function_value(&saída, msg, id, parâmetro, empresa, pessoa, território, classe, local)`: `classe = -1` sem classe. Sem classe devolve o valor da função `id` para o contexto `(empresa, pessoa, território)` (zero = ausente); funções cujo dado exige uma entidade ausente devolvem 0 (por exemplo, `CompanyCash` com empresa 0). `id = 0x5a` devolve o `parâmetro` (o literal). A tabela completa está em 3.5.

**Classe** (`IsHighestInClass` `0x4a` / `IsLowestInClass` `0x4b`): a função precisa ter escopo (`C`, `J` ou `T`), senão erro `0x81`. A dimensão varrida é a primeira disponível na ordem **empresa > jogador > território**, entre as que a função tem e o contexto fornece (empresa != 0 e função de empresa; senão pessoa != 0 e função de jogador; senão território != 0 e função de território); sem nenhuma, erro `0xb2b`. As demais dimensões ficam fixas no contexto. Percorre **todas** as entidades do pool da dimensão em ordem de lista; para a entidade própria guarda o valor `v_próprio`, para as outras acumula `acc` = máximo (`Highest`) ou mínimo (`Lowest`) com o sentinela `0x3b8d6c1f` como "vazio". Resultado: 1 se não há outras (`acc` vazio); senão `Highest`: `acc < v_próprio`, `Lowest`: `v_próprio < acc` (**estrito**: empate dá 0). Exemplo: `CompanyCash(IsHighestInClass)` é verdadeiro só para a única empresa de maior caixa. Validado por diferencial com `CompanyId`/`PlayerId`/`TerritoryId` em pools montados na memória emulada.

**Modo local** (`local != 0`: gatilho 6 ou 7, `+0x34f` ou `+0x350`): `Random1To10000` usa `crt_rand_range(1,10000)` em vez do RNG do jogo.

Estruturas lidas (campos de `docs/spec/cargo.md` e `finance.md`):
- empresa: `+0x23` pessoa, `+0x148` ano de fundação, `+0x16e` ativa, `+0x182..+0x18e` células de trilho, `+0x192`/`+0x19e` cargas (ano/vida), `+0x196/+0x19a` e `+0x1a2/+0x1a6` soma e contagem para a velocidade média, `+0x1ae` efeitos, `+0x4219..` variáveis, `+0x4260` trilho restante, `+0x4766` contas, blocos por território em `+0x479 + t*0x1ed` (`t` = id do território; 32 blocos): byte de direitos (bit 0 trens, bit 1 trilho, bit 2 comprar prédios, bit 3 estações), i16 `+3` trilho, `+5` cargas no ano, `+7` cargas vida, matriz `M` i16[32] em `+9` (cargas entre territórios), vetores i16[44] por tipo de carga em `+0x4c2-0x479`, `+0x51a-0x479`, `+0x5b6-0x479`, `+0x60e-0x479`;
- pessoa: `+4` ativa, `+0x122` humana, `+0x12a` caixa f64, `+0x1d3` ações, `+0x263` empresa presidida, `+0x26b` efeitos, `+0x534..` variáveis;
- território: `+4` nome, `+0x44..+0x47` flags "direitos à venda", `+0x50` u16 trilho, `+0x52` u16 cargas no ano, `+0x54` u32 cargas vida, `+0x58` efeitos, `+0x2b..` variáveis.

### 3.4 RNG (ordem para reproduzir a sequência)

`Random1To10000` chama `rng_range(1, 10000)` (`0x4f0660`, **uma** saída de 15 bits, `rng.md` 2.2) por ocorrência no texto, **a cada avaliação da condição**. Consumo, em ordem:
1. regras na ordem do pool, só as de gatilho `k`; avaliação de uma regra: para cada (jogador, empresa, território) da ordem de 2.3, **uma avaliação completa da condição**, com as ocorrências de `Random1To10000` sorteando da esquerda para a direita no texto (passe 1, 3.2), sem curto-circuito;
2. uma regra "uma vez só" já disparada continua avaliando (e sorteando) nas combinações seguintes: só o efeito/mensagem é pulado;
3. regras de modo de data (desfazer) não avaliam: sem sorteio;
4. gatilhos 6 e 7, `+0x34f`, `+0x350`: a função usa o `rand()` do CRT (estado `0x5609f0`, `crt_rand_range`: `lo + rand % (hi-lo+1)`), **não** o RNG do jogo;
5. a mensagem e os efeitos do conjunto de efeitos não sorteiam (o que as funções chamadas por alguns efeitos, `0x438780` gerentes, `0x4c8a30` inundação, `0x4cdd50` aposentar trem, `0x469fb0` estado econômico, sortearem não foi lido: Em aberto).
Confirmado: o diferencial compara o estado do RNG do jogo e do CRT depois de 6000 expressões bem formadas e 4000 malformadas.

### 3.5 Tabela de tokens (118 entradas, `0x53b330`, 17 bytes cada)

Entrada: `+0` ponteiro do nome, `+4` u32 `kind`, `+8` u8 `prec` (passe), `+9` `P` (tem parâmetro numérico `(n)`), `+10` `C` (função de empresa), `+11` `J` (de jogador), `+12` `T` (de território), `+13` ponteiro do alias (vazio na maioria; só os ids 65, 66, 91 têm). `kind`: 0 função/constante, 1 comparador, 2 `(`, 3 `)`, 4 operador binário, 5 classe. `0x457a30` liga "testar contra cada empresa/jogador/território" (`+0x24`/`+0x28`/`+0x324`) quando a condição tem alguma função `C`/`J`/`T`; o editor também permite ligar sem funções. 101 funções (ids 0..73 e 91..117), 2 classes, 6 comparadores, 2 parênteses, `AND`/`OR`, `+ - * /` e a constante. O manual cita "mais de 70" (texto) e 84 entradas (Apêndice A): o código tem 101; o texto do editor mostra poucas das 27 funções acrescentadas pelo TSC (`EDITOR2.TXT` lista 16).

Descrição por id (a coluna "lê" é o que o ramo de `0x454ae0` calcula; `ftol` = truncamento do f64):

| id | hex | nome | kind | prec | flags | lê |
|---|---|---|---|---|---|---|
| 0 | `0x00` | `Random1To10000` | 0 | 1 | - | `rng_range(1,10000)` (`crt_rand_range(1,10000)` se a avaliação é local: gatilho 6/7 ou `+0x34f`/`+0x350`) |
| 1 | `0x01` | `True` | 0 | 1 | - | 1 (caso 1) |
| 2 | `0x02` | `False` | 0 | 1 | - | 0 (ramo padrão) |
| 3 | `0x03` | `GameNumPlayers` | 0 | 1 | - | `0x46a890()` = `[0x570775]`, nº de jogadores do conjunto indexado |
| 4 | `0x04` | `GameNumCompanies` | 0 | 1 | - | `0x46a7e0()` = `[0x570729]`, nº de empresas do conjunto indexado |
| 5 | `0x05` | `GameExistsCompanyId` | 0 | 1 | P | 1 se `n` é um slot ocupado do pool de empresas (`0x574c6c`) e `empresa(n)+0x16e != 0`; senão 0 |
| 6 | `0x06` | `GameExistsPlayerId` | 0 | 1 | P | 1 se `n` é um slot ocupado do pool de pessoas (`0x574c68`) e `pessoa(n)+4 != 0` (ativa); senão 0 |
| 7 | `0x07` | `GameYearsElapsedSinceStart` | 0 | 1 | - | `ano(game_date) - ano([0x570508])`, +1 se mês > 11 e dia > 27 |
| 8 | `0x08` | `GameStartYear` | 0 | 1 | - | `[0x570508] & 0xffff` (ano inicial, `game_start_date`) |
| 9 | `0x09` | `GameYear` | 0 | 1 | - | ano de `game_date` (u16) |
| 10 | `0x0a` | `GameMonth` | 0 | 1 | - | mês de `game_date` (+2, 1..12) |
| 11 | `0x0b` | `GameYearMonth` | 0 | 1 | - | `mês + ano*100` |
| 12 | `0x0c` | `GameEconomicState` | 0 | 1 | - | `[0x570717]` (`economy_state`, 0..4) |
| 13 | `0x0d` | `GameLoadsHauledThisYear` | 0 | 1 | - | `[0x573d05]` (cargas transportadas no ano, global) |
| 14 | `0x0e` | `GameLoadsHauledLifetime` | 0 | 1 | - | `[0x573d11]` (cargas, vida) |
| 15 | `0x0f` | `GameTrackCells` | 0 | 1 | - | `[0x573d1d]` (células de trilho) |
| 16 | `0x10` | `GameSingleTrackCells` | 0 | 1 | - | `[0x573d21]` (trilho simples) |
| 17 | `0x11` | `GameDoubleTrackCells` | 0 | 1 | - | `[0x573d25]` (trilho duplo) |
| 18 | `0x12` | `GameElectricTrackCells` | 0 | 1 | - | `[0x573d29]` (trilho elétrico) |
| 19 | `0x13` | `GameVariable1` | 0 | 1 | - | `[0x573d8e]` (`0x573d42 + 4*19`), variável global 1 (efeitos `0xf1`/`0xf5`) |
| 20 | `0x14` | `GameVariable2` | 0 | 1 | - | `[0x573d92]`, variável global 2 |
| 21 | `0x15` | `GameVariable3` | 0 | 1 | - | `[0x573d96]`, variável global 3 |
| 22 | `0x16` | `GameVariable4` | 0 | 1 | - | `[0x573d9a]`, variável global 4 |
| 23 | `0x17` | `CompanyCash` | 0 | 1 | C | `ftol(conta 0 do razão)` = caixa (`0x43a060(emp,0)`) |
| 24 | `0x18` | `CompanyDebt` | 0 | 1 | C | `ftol(conta 6)` = dívida **como guardada (negativa)** |
| 25 | `0x19` | `CompanyTotalAssets` | 0 | 1 | C | `ftol(conta 8)` = soma das contas 0..5 (ativos) |
| 26 | `0x1a` | `CompanyBookValue` | 0 | 1 | C | `ftol(conta 9)` = `-conta 7` (patrimônio líquido) |
| 27 | `0x1b` | `CompanyRevenueThisYear` | 0 | 1 | C | `ftol(0x43a660(emp,0xe))`: receita do ano corrente (categorias 0..4) |
| 28 | `0x1c` | `CompanyRevenueLifetime` | 0 | 1 | C | `ftol(0x43a6d0(emp,0xe))`: receita "vida" (cópia `+0x46fe`, marcador 9001) |
| 29 | `0x1d` | `CompanyNetIncomeThisYear` | 0 | 1 | C | `ftol(0x43a660(emp,0xd))`: resultado do ano (categorias 0..12) |
| 30 | `0x1e` | `CompanyNetIncomeLifetime` | 0 | 1 | C | `ftol(0x43a6d0(emp,0xd))`: resultado vida |
| 31 | `0x1f` | `CompanyLoadsHauledThisYear` | 0 | 1 | C | `empresa+0x192` (cargas no ano) |
| 32 | `0x20` | `CompanyLoadsHauledLifetime` | 0 | 1 | C | `empresa+0x19e` (cargas, vida) |
| 33 | `0x21` | `CompanyTrackCells` | 0 | 1 | C | `empresa+0x182` (células de trilho) |
| 34 | `0x22` | `CompanySingleTrackCells` | 0 | 1 | C | `empresa+0x186` (simples) |
| 35 | `0x23` | `CompanyDoubleTrackCells` | 0 | 1 | C | `empresa+0x18a` (duplo) |
| 36 | `0x24` | `CompanyElectricTrackCells` | 0 | 1 | C | `empresa+0x18e` (elétrico) |
| 37 | `0x25` | `CompanyGoodwill` | 0 | 1 | C | `0x43eca0(emp, 0) / 100` [V: #17]: sobre os blocos dos territórios `1..n`, `v = vida/N + (trilho >> 4) + ano` (i16 `+7`, `+3`, `+5` do bloco; `N = max(1, ftol((fundação - ano) * 1,5))`, ou seja 1); com `Σv > 0` (só `v > 0`), `g = ftol((Σ posição*v / Σv + empresa.+0x4229) * 0,5)`, senão `g = +0x4229`; com território (ignorado se há um só), `g = (posição do território + g) / 2`; resultado `ftol(effect_multiplier(0x28) * max(g, 0))` em `0..10000` |
| 38 | `0x26` | `CompanyId` | 0 | 1 | C | id da empresa do contexto |
| 39 | `0x27` | `CompanyVariable1` | 0 | 1 | C | `empresa+0x417d+4*39` = `+0x4219`, variável 1 da empresa (efeitos `0xf9`/`0xfd`) |
| 40 | `0x28` | `CompanyVariable2` | 0 | 1 | C | `empresa+0x421d`, variável 2 |
| 41 | `0x29` | `CompanyVariable3` | 0 | 1 | C | `empresa+0x4221`, variável 3 |
| 42 | `0x2a` | `CompanyVariable4` | 0 | 1 | C | `empresa+0x4225`, variável 4 |
| 43 | `0x2b` | `CompanyTerritoryConnectedToTerritory` | 0 | 1 | PCT | `0x46b9a0`: 1 se alguma rede de estações (`estação+0x57 < 0x100`) da empresa tem estação no território do contexto (`estação+0x4ad`) e outra no território `n`; exige `n` válido no pool `0x574ca8` |
| 44 | `0x2c` | `CompanyTerritoryConnectedToCity` | 0 | 1 | PCT | idem com a cidade `n` (`estação+0x53`, pool de cidades `0x574c5c`) |
| 45 | `0x2d` | `CompanyTerritoryGoodwill` | 0 | 1 | CT | `0x43eca0(emp, território)/100`: a conta de `CompanyGoodwill` com a média final com a posição do território (exige território != 0) |
| 46 | `0x2e` | `CompanyTerritoryHasTrainAccess` | 0 | 1 | CT | bit 0 de `empresa+0x479+território*0x1ed` (acesso a trens) |
| 47 | `0x2f` | `CompanyTerritoryHasTrackAccess` | 0 | 1 | CT | bit 1 (acesso a construir trilho) |
| 48 | `0x30` | `CompanyTerritoryHasStationAccess` | 0 | 1 | CT | bit 3 (acesso a construir estação) |
| 49 | `0x31` | `CompanyTerritoryHasBuildingBuyAccess` | 0 | 1 | CT | bit 2 (acesso a comprar prédios) |
| 50 | `0x32` | `CompanyTerritoryHasAnyAccess` | 0 | 1 | CT | 1 se o byte de flags do território é != 0 |
| 51 | `0x33` | `CompanyTerritoryTrackCells` | 0 | 1 | CT | i16 em `bloco+3` (`+0x47c`), trilho da empresa no território |
| 52 | `0x34` | `CompanyTerritoryLoadsHauledThisYear` | 0 | 1 | CT | i16 em `bloco+5` (`+0x47e`), cargas entregues ao território no ano |
| 53 | `0x35` | `CompanyTerritoryLoadsHauledLifetime` | 0 | 1 | CT | i16 em `bloco+7` (`+0x480`), cargas, vida |
| 54 | `0x36` | `PlayerCash` | 0 | 1 | J | `ftol(pessoa+0x12a)` (caixa f64) |
| 55 | `0x37` | `PlayerStockValue` | 0 | 1 | J | `ftol(0x4a6d00(pessoa,0))`: ações x preço |
| 56 | `0x38` | `PlayerNetWorth` | 0 | 1 | J | `ftol(0x4a7620(pessoa))`: caixa + ações x cotação |
| 57 | `0x39` | `PlayerControlsAnyCompany` | 0 | 1 | J | 1 se `pessoa+0x263 != 0` |
| 58 | `0x3a` | `PlayerControlledCompanyId` | 0 | 1 | J | `pessoa+0x263` (empresa que preside) |
| 59 | `0x3b` | `PlayerId` | 0 | 1 | J | **devolve o id da empresa do contexto** (o ramo `0x454dbc` é compartilhado com `CompanyId`; o nome promete o id do jogador; [V] no disassembly) |
| 60 | `0x3c` | `PlayerVariable1` | 0 | 1 | J | `pessoa+0x534` (variável 1 do jogador; efeitos `0x101`/`0x105`) |
| 61 | `0x3d` | `PlayerVariable2` | 0 | 1 | J | `pessoa+0x538` |
| 62 | `0x3e` | `PlayerVariable3` | 0 | 1 | J | `pessoa+0x53c` |
| 63 | `0x3f` | `PlayerVariable4` | 0 | 1 | J | `pessoa+0x540` |
| 64 | `0x40` | `TerritoryTrackCells` | 0 | 1 | T | u16 `território+0x50`, trilho |
| 65 | `0x41` | `TerritoryLoadsHauledToThisYear` (alias `TerritoryLoadsHauledThisYear`) | 0 | 1 | T | u16 `território+0x52`, cargas entregues no ano |
| 66 | `0x42` | `TerritoryLoadsHauledToLifetime` (alias `TerritoryLoadsHauledLifetime`) | 0 | 1 | T | u32 `território+0x54`, cargas, vida |
| 67 | `0x43` | `TerritoryConnectedToTerritory` | 0 | 1 | PT | `0x46b9a0` sem empresa: alguma rede com estações no território do contexto e no território `n` |
| 68 | `0x44` | `TerritoryConnectedToCity` | 0 | 1 | PT | idem com a cidade `n` |
| 69 | `0x45` | `TerritoryId` | 0 | 1 | T | id do território do contexto |
| 70 | `0x46` | `TerritoryVariable1` | 0 | 1 | T | `território+0x2b` (variável 1; efeitos `0x109`/`0x10d`) |
| 71 | `0x47` | `TerritoryVariable2` | 0 | 1 | T | `território+0x2f` |
| 72 | `0x48` | `TerritoryVariable3` | 0 | 1 | T | `território+0x33` |
| 73 | `0x49` | `TerritoryVariable4` | 0 | 1 | T | `território+0x37` |
| 74 | `0x4a` | `IsHighestInClass` | 5 | 0 | - | parâmetro de classe (ver 2.4) |
| 75 | `0x4b` | `IsLowestInClass` | 5 | 0 | - | parâmetro de classe (ver 2.4) |
| 76 | `0x4c` | `>=` | 1 | 6 | - | `>=` |
| 77 | `0x4d` | `<=` | 1 | 6 | - | `<=` |
| 78 | `0x4e` | `>` | 1 | 6 | - | `>` |
| 79 | `0x4f` | `<` | 1 | 6 | - | `<` |
| 80 | `0x50` | `=` | 1 | 6 | - | `=` |
| 81 | `0x51` | `!=` | 1 | 6 | - | `!=` (o divisor de palavras o parte em `!` e `=`: **inalcançável** pelo texto, 2.1) |
| 82 | `0x52` | `(` | 2 | 2 | - | `(` |
| 83 | `0x53` | `)` | 3 | 2 | - | `)` |
| 84 | `0x54` | `AND` | 4 | 7 | - | `AND`/`and` (1 se ambos != 0) |
| 85 | `0x55` | `OR` | 4 | 7 | - | `OR`/`or` |
| 86 | `0x56` | `+` | 4 | 5 | - | `+` (também sinal unário) |
| 87 | `0x57` | `-` | 4 | 5 | - | `-` (também sinal unário) |
| 88 | `0x58` | `*` | 4 | 4 | - | `*` |
| 89 | `0x59` | `/` | 4 | 4 | - | `/` (divisor 0 vira 1) |
| 90 | `0x5a` | `constant` | 0 | 3 | - | literal numérico (o valor vai no vetor de valores) |
| 91 | `0x5b` | `CompanyTerritoryLoadsHauledToLifetimeFromTerritory` (alias `CompanyTerritoryLoadsHauledLifetimeFromTerritory`) | 0 | 1 | PCT | `M[terr][n]`, vida: cargas para o território do contexto vindas de `n`; `M[a][b]` = i16 em `empresa+0x482 + a*0x1ed + b*2` |
| 92 | `0x5c` | `CompanyTerritoryLoadsHauledLifetimeBetweenTerritory` | 0 | 1 | PCT | `M[terr][n] + M[n][terr]`, vida, nos dois sentidos |
| 93 | `0x5d` | `CampaignChoice1To3` | 0 | 1 | - | `campanha ? campaign_choice+1 : 0` (`[0x566d2a]`) |
| 94 | `0x5e` | `CampaignDifficulty1To3` | 0 | 1 | - | `campanha ? campaign_difficulty+1 : 0` (`[0x566d00]`) |
| 95 | `0x5f` | `StationVisited` | 0 | 1 | P | byte `estação(n)+0x4bb` (visitada), `n` válido no pool de estações |
| 96 | `0x60` | `NumberOfStationsVisited` | 0 | 1 | - | `0x4c0240`: estações do jogador (dono +0x45 == 0) com `+0x4bb != 0` |
| 97 | `0x61` | `CompanyIndustryProfitsThisYear` | 0 | 1 | C | **igual a 101** (mesmo ramo): se `empresa+0x148` (ano de fundação) < ano, `ftol(0x43a7d0(emp, ano-1, 3))`, senão 0 |
| 98 | `0x62` | `CompanyIndustryInvestments` | 0 | 1 | C | `ftol(conta 5)` (investimento em indústrias; conta 5 = indústrias, `finance.md`) |
| 99 | `0x63` | `GameIsMultiPlayer` | 0 | 1 | - | `[0x573f1c] != 0` (partida em rede) |
| 100 | `0x64` | `GameIsNotMultiPlayer` | 0 | 1 | - | `[0x573f1c] == 0` |
| 101 | `0x65` | `CompanyIndustryProfitsLastYear` | 0 | 1 | C | ver 97: lucro de indústrias (categoria 3) do **ano anterior** |
| 102 | `0x66` | `CompanyIndustryProfitsLifetime` | 0 | 1 | C | `ftol(0x43a6d0(emp,3))`: categoria 3 na cópia "vida" |
| 103 | `0x67` | `CompanyTerritoryLoadsHauledFromThisYear` | 0 | 1 | PCT | **sempre 0** (índice sem ramo no `switch` de `0x454ae0`, vai ao padrão; nenhum arquivo do jogo o usa) |
| 104 | `0x68` | `CompanyTerritoryLoadsHauledFromLifetime` | 0 | 1 | PCT | **sempre 0** (idem 103) |
| 105 | `0x69` | `CompanyTerritoryLoadsHauledFromLifetimeToTerritory` | 0 | 1 | PCT | `M[n][terr]`, vida: cargas do território do contexto para `n` (ver 2.4) |
| 106 | `0x6a` | `CompanyTerritoryTypeLoadsHauledToThisYear` | 0 | 1 | PCT | `empresa+0x51a + n*2 + terr*0x1ed`, `n < 44` (tipo de carga): para o território, no ano |
| 107 | `0x6b` | `CompanyTerritoryTypeLoadsHauledToLifetime` | 0 | 1 | PCT | `+0x4c2`: para o território, vida |
| 108 | `0x6c` | `CompanyTerritoryTypeLoadsHauledFromThisYear` | 0 | 1 | PCT | `+0x60e`: do território, no ano |
| 109 | `0x6d` | `CompanyTerritoryTypeLoadsHauledFromLifetime` | 0 | 1 | PCT | `+0x5b6`: do território, vida |
| 110 | `0x6e` | `CompanyTypeLoadsHauledThisYear` | 0 | 1 | PC | soma sobre os 32 blocos de `+0x51a + n*2`: cargas do tipo `n` no ano |
| 111 | `0x6f` | `CompanyTypeLoadsHauledLifetime` | 0 | 1 | PC | soma de `+0x4c2 + n*2`: vida |
| 112 | `0x70` | `CompanyAverageSpeedThisYear` | 0 | 1 | C | `0x476140(empresa+0x19a / max(1,empresa+0x196))`: velocidade média no ano (`0x476140` é identidade) |
| 113 | `0x71` | `CompanyAverageSpeedLifetime` | 0 | 1 | C | `empresa+0x1a6 / max(1,empresa+0x1a2)`: vida |
| 114 | `0x72` | `GameTrackCellsVisited` | 0 | 1 | - | nº de peças de trilho do pool `0x574c84` com `(+0x23 & 0x80)` |
| 115 | `0x73` | `TerritoryCompletelyFlooded` | 0 | 1 | T | 1 se todos os tiles do território são água (id de terreno `0x12`, tabela `0x581090`) |
| 116 | `0x74` | `CompanyTrackCellsRemaining` | 0 | 1 | C | `empresa+0x4260` (células de trilho que a empresa ainda pode construir; efeitos `0x129`/`0x12a`) |
| 117 | `0x75` | `CompanyHasAnyAccessTerritoryParameter` | 0 | 1 | P | 1 se o byte de flags de `empresa+0x479+n*0x1ed` != 0 (empresa do contexto, território `n`) |

Notas da tabela: ids 5 e 6 usam o bitmap de livres do pool (`+0x34`); `GameYearsElapsedSinceStart` conta o ano que começa em 28/dez como completo; as funções de empresa/jogador/território sem a entidade no contexto devolvem 0 (`CompanyCash` fora de um laço de empresas vale 0). Valores `ftol` são truncados em 32 bits. Os ids 103 e 104 existem na tabela (e em `EDITOR2.TXT`) mas **não têm ramo** em `0x454ae0` e valem sempre 0.

## 4. Efeitos

### 4.1 Tabela de efeitos (350 linhas de 16 bytes, `0x550550`)

Linha `e` (0..349): `+0` u16 id do controle do editor (igual a `e` na maior parte; o id agrupa páginas), `+2` u8 "visível no editor", `+3` / `+4` / `+5` u8 **exige** testar contra cada empresa / jogador / território (usados por `rule_warn_scopes` `0x458050`, que avisa com os textos LNG `0x89c`/`0x89d`/`0x89e`), `+6..+10` u8 flags de escopo do editor (não decodificadas), `+0xb` u8 **instantâneo** (não é desfeito por regra temporária, 4.4), `+0xc` i8 mínimo, `+0xd` i8 máximo do controle (`-1..1` liga/desliga, `-99..100` percentual, `-100..100` com `-100` = "valor do gatilho", `-4..4`, `-10..10`...), `+0xe` u16 id LNG do nome (0 nas linhas sem nome próprio). Os valores gravados nas regras dos 128 arquivos ficam todos dentro de `[mín, máx]` da linha (5191 bytes não nulos, teste `shipped_effect_values_are_in_range`).

Grupos (ids em decimal; hex entre parênteses):

| Ids | Efeito |
|---|---|
| 0 | sem nome; cai no ramo padrão |
| 1..4 | `Game Over - Win 1/2/3`, `Game Over - Loss` (instantâneos; seção 5) |
| 5, 6, 7 | `Company Cash`, `Player Cash`, `Economy Status` (instantâneos) |
| 8..49 | multiplicadores de custo, desempenho e finanças (nomes abaixo); `effect_multiplier(e, ...)` lê `e` direto (`cargo.md` 5, `finance.md`, `terrain-track.md`) |
| 50..54 | direitos de território: todos / trilho / trem / estação / prédio |
| 55..59 | `Territory Confiscate` todos / trilho / trens / estações / prédios (instantâneos) |
| 60, 61 | `Station Revenue` (`0x3c`), `All Passenger & Cargo Revenue` (`0x3d`) |
| 62..93 (`0x3e..0x5d`) | receita da carga `c = e - 62`, `c < 32` (a linha não tem nome LNG; o nome é o da carga) |
| 94 (`0x5e`) | `All Passenger & Cargo Production` |
| 95..126 (`0x5f..0x7e`) | produção da carga `c = e - 95`, `c < 32` |
| 127..188 (`0x7f..0xbc`) | disponibilidade de locomotiva `e - 127` (-1 nunca, 0 normal, +1 imediata; ao aplicar liga `[0x574784]`) |
| 189..192 (`0xbd..0xc0`) | `Deactivate company`, `Deactivate player`, `Make human control this player`, `Set player type` |
| 193..232 (`0xc1..0xe8`) | disponibilidade do gerente `e - 193` (com empresa chama `0x438780(empresa, e-0xc1, 0)`) |
| 233 (`0xe9`) | `Territory Flood Amount` (instantâneo) |
| 234 (`0xea`) | sem nome, sem ramo especial (vira acréscimo de nível); **Em aberto** |
| 235 (`0xeb`) | `Retire Train ID#` |
| 236..240 (`0xec..0xf0`) | `Set Territory Rights Buyable` todos / trilho / trens / estações / prédios (instantâneos) |
| 241..272 (`0xf1..0x110`) | variáveis: `Set`/`Add To` x `GameVariable`/`CompanyVariable`/`PlayerVariable`/`TerritoryVariable` 1..4, nessa ordem de oito (instantâneos) |
| 273..284 (`0x111..0x11c`) | receita das cargas 32..43 (`0xf1 + c` do `cargo.md` = `0x111 + c - 32`) |
| 285..296 (`0x11d..0x128`) | produção das cargas 32..43 (por simetria da tabela; [S]) |
| 297, 298 (`0x129`, `0x12a`) | `Add to track allowed`, `Set track allowed` (instantâneos) |
| 299..303 (`0x12b..0x12f`) | `Territory Destroy` todos / trilho / trens / estações / prédios (instantâneos) |
| 304 (`0x130`) | `Bulldoze building` (sem ramo especial; acréscimo de nível) |
| 305..308 (`0x131..0x134`) | disponibilidade de mais quatro locomotivas (liga `[0x574784]`) |
| 309..349 (`0x135..0x15d`) | 41 linhas instantâneas sem nome, `-99..100`; **Em aberto** |

Nomes das linhas 5..61, 189..192, 233..272, 297..304 (`escala` = intervalo do controle; `inst` = instantâneo). Os nomes vêm das strings LNG que a própria tabela referencia (`default2.lng`):

| e | hex | nome | escala | inst |
|---|---|---|---|---|
| 0 | `0x0` | (sem nome) | -99..100 | sim |
| 1 | `0x1` | Game Over - Win 1 | -1..1 | sim |
| 2 | `0x2` | Game Over - Win 2 | -1..1 | sim |
| 3 | `0x3` | Game Over - Win 3 | -1..1 | sim |
| 4 | `0x4` | Game Over - Loss | -1..1 | sim |
| 5 | `0x5` | Company Cash | -127..127 | sim |
| 6 | `0x6` | Player Cash | -127..127 | sim |
| 7 | `0x7` | Economy Status | -4..4 | sim |
| 8 | `0x8` | Station Building | -99..100 | não |
| 9 | `0x9` | Track Building | -99..100 | não |
| 10 | `0xa` | Mountainous Track Building | -99..100 | não |
| 11 | `0xb` | Electric Track Building | -99..100 | não |
| 12 | `0xc` | Track Maintenance | -99..100 | não |
| 13 | `0xd` | Building Buy Cost | -99..100 | não |
| 14 | `0xe` | Bridge Building | -99..100 | não |
| 15 | `0xf` | Company Overhead | -99..100 | não |
| 16 | `0x10` | Manager Salaries | -99..100 | não |
| 17 | `0x11` | Fuel Costs | -99..100 | não |
| 18 | `0x12` | Fuel Costs - Steam | -99..100 | não |
| 19 | `0x13` | Fuel Costs - Diesel | -99..100 | não |
| 20 | `0x14` | Fuel Costs - Electric | -99..100 | não |
| 21 | `0x15` | Engine Maintenance | -99..100 | não |
| 22 | `0x16` | Engine Maintenance - Steam | -99..100 | não |
| 23 | `0x17` | Engine Maintenance - Diesel | -99..100 | não |
| 24 | `0x18` | Engine Maintenance - Electric | -99..100 | não |
| 25 | `0x19` | Car Maintenance | -99..100 | não |
| 26 | `0x1a` | Engine Purchase | -99..100 | não |
| 27 | `0x1b` | Steam Engine Purchase | -99..100 | não |
| 28 | `0x1c` | Diesel Engine Purchase | -99..100 | não |
| 29 | `0x1d` | Electric Engine Purchase | -99..100 | não |
| 30 | `0x1e` | Stock Prices | -99..100 | não |
| 31 | `0x1f` | Credit Rating | -10..10 | não |
| 32 | `0x20` | Prime Rate | -5..5 | não |
| 33 | `0x21` | Security | -99..100 | não |
| 34 | `0x22` | Merger Premium | -99..100 | não |
| 35 | `0x23` | Train Speed | -99..100 | não |
| 36 | `0x24` | Train Speed - Steam | -99..100 | não |
| 37 | `0x25` | Train Speed - Diesel | -99..100 | não |
| 38 | `0x26` | Train Speed - Electric | -99..100 | não |
| 39 | `0x27` | Train Safety | -99..100 | não |
| 40 | `0x28` | Goodwill | -99..100 | não |
| 41 | `0x29` | Station Turnaround | -99..100 | não |
| 42 | `0x2a` | Traction | -99..100 | não |
| 43 | `0x2b` | Traction - Steam | -99..100 | não |
| 44 | `0x2c` | Traction - Diesel | -99..100 | não |
| 45 | `0x2d` | Traction - Electric | -99..100 | não |
| 46 | `0x2e` | Acceleration | -99..100 | não |
| 47 | `0x2f` | Acceleration - Steam | -99..100 | não |
| 48 | `0x30` | Acceleration - Diesel | -99..100 | não |
| 49 | `0x31` | Acceleration - Electric | -99..100 | não |
| 50 | `0x32` | Territory Allow All | -1..1 | não |
| 51 | `0x33` | Territory Allow Track Build | -1..1 | não |
| 52 | `0x34` | Territory Allow Train Run | -1..1 | não |
| 53 | `0x35` | Territory Allow Station Build | -1..1 | não |
| 54 | `0x36` | Territory Allow Building Buy | -1..1 | não |
| 55 | `0x37` | Territory Confiscate All | -1..1 | sim |
| 56 | `0x38` | Territory Confiscate Track | -1..1 | sim |
| 57 | `0x39` | Territory Confiscate Trains | -1..1 | sim |
| 58 | `0x3a` | Territory Confiscate Stations | -1..1 | sim |
| 59 | `0x3b` | Territory Confiscate Buildings | -1..1 | sim |
| 60 | `0x3c` | Station Revenue | -99..100 | não |
| 61 | `0x3d` | All Passenger & Cargo Revenue | -99..100 | não |
| 189 | `0xbd` | Deactivate company | -1..1 | não |
| 190 | `0xbe` | Deactivate player | -1..1 | não |
| 191 | `0xbf` | Make human control this player | -1..1 | não |
| 192 | `0xc0` | Set player type | -40..40 | não |
| 233 | `0xe9` | Territory Flood Amount | -100..100 | sim |
| 234 | `0xea` | (sem nome) | -1..1 | não |
| 235 | `0xeb` | Retire Train ID# | -127..127 | não |
| 236 | `0xec` | Set Territory Rights Buyable - All | -1..1 | sim |
| 237 | `0xed` | Set Territory Rights Buyable - Track | -1..1 | sim |
| 238 | `0xee` | Set Territory Rights Buyable - Trains | -1..1 | sim |
| 239 | `0xef` | Set Territory Rights Buyable - Stations | -1..1 | sim |
| 240 | `0xf0` | Set Territory Rights Buyable - Buildings | -1..1 | sim |
| 241 | `0xf1` | Set GameVariable1 | -100..100 | sim |
| 242 | `0xf2` | Set GameVariable2 | -100..100 | sim |
| 243 | `0xf3` | Set GameVariable3 | -100..100 | sim |
| 244 | `0xf4` | Set GameVariable4 | -100..100 | sim |
| 245 | `0xf5` | Add To GameVariable1 | -100..100 | sim |
| 246 | `0xf6` | Add To GameVariable2 | -100..100 | sim |
| 247 | `0xf7` | Add To GameVariable3 | -100..100 | sim |
| 248 | `0xf8` | Add To GameVariable4 | -100..100 | sim |
| 249 | `0xf9` | Set CompanyVariable1 | -100..100 | sim |
| 250 | `0xfa` | Set CompanyVariable2 | -100..100 | sim |
| 251 | `0xfb` | Set CompanyVariable3 | -100..100 | sim |
| 252 | `0xfc` | Set CompanyVariable4 | -100..100 | sim |
| 253 | `0xfd` | Add To CompanyVariable1 | -100..100 | sim |
| 254 | `0xfe` | Add To CompanyVariable2 | -100..100 | sim |
| 255 | `0xff` | Add To CompanyVariable3 | -100..100 | sim |
| 256 | `0x100` | Add To CompanyVariable4 | -100..100 | sim |
| 257 | `0x101` | Set PlayerVariable1 | -100..100 | sim |
| 258 | `0x102` | Set PlayerVariable2 | -100..100 | sim |
| 259 | `0x103` | Set PlayerVariable3 | -100..100 | sim |
| 260 | `0x104` | Set PlayerVariable4 | -100..100 | sim |
| 261 | `0x105` | Add To PlayerVariable1 | -100..100 | sim |
| 262 | `0x106` | Add To PlayerVariable2 | -100..100 | sim |
| 263 | `0x107` | Add To PlayerVariable3 | -100..100 | sim |
| 264 | `0x108` | Add To PlayerVariable4 | -100..100 | sim |
| 265 | `0x109` | Set TerritoryVariable1 | -100..100 | sim |
| 266 | `0x10a` | Set TerritoryVariable2 | -100..100 | sim |
| 267 | `0x10b` | Set TerritoryVariable3 | -100..100 | sim |
| 268 | `0x10c` | Set TerritoryVariable4 | -100..100 | sim |
| 269 | `0x10d` | Add To TerritoryVariable1 | -100..100 | sim |
| 270 | `0x10e` | Add To TerritoryVariable2 | -100..100 | sim |
| 271 | `0x10f` | Add To TerritoryVariable3 | -100..100 | sim |
| 272 | `0x110` | Add To TerritoryVariable4 | -100..100 | sim |
| 297 | `0x129` | Add to track allowed | -100..100 | sim |
| 298 | `0x12a` | Set track allowed | -100..100 | sim |
| 299 | `0x12b` | Territory Destroy All | -1..1 | sim |
| 300 | `0x12c` | Territory Destroy Track | -1..1 | sim |
| 301 | `0x12d` | Territory Destroy Trains | -1..1 | sim |
| 302 | `0x12e` | Territory Destroy Stations | -1..1 | sim |
| 303 | `0x12f` | Territory Destroy Buildings | -1..1 | sim |
| 304 | `0x130` | Bulldoze building | -99..100 | não |

### 4.2 Aplicação (`rule_evaluate`, depois da mensagem)

Os efeitos de uma combinação (conjunto `k` = 0, ou o escolhido no diálogo de escolha) **não são aplicados** se `[0x5665f0] != 0` (briefing aberto pelo botão), `+0x34f`, `+0x350` ou gatilho 6/7 (avaliação local: só mensagem). Para `e` de 0 a 349, com `b = i8 efeito[k][e]` (`b == 0`: nada):

| `e` | Ação |
|---|---|
| 1..4 | fim de jogo (seção 5) |
| 5 | exige empresa: `company_ledger_credit(empresa, 4, effect_money(b))` (`0x439970`, categoria 4 "Diversos" das receitas) |
| 6 | exige pessoa: `0x4a7570(pessoa, effect_money(b))` soma ao caixa da pessoa |
| 7 | `n = clamp([0x570717] + b, 0, 4)`; `0x469fb0(n)` fixa o estado econômico |
| 50 | exige território: `0x4c8540(território, m, b >= 0, empresa, 0)` para m = 2, 1, 4, 8 (liga/desliga todos os direitos da empresa no território) |
| 51 / 52 / 53 / 54 | o mesmo só com a máscara 2 (trilho) / 1 (trens) / 8 (estações) / 4 (prédios) |
| 55 | `0x437d40(empresa, território, 0)`, `0x437db0(empresa, território)`, `0x437f50(empresa, território, 0)` e `0x4380a0(empresa, território)` (confisco de tudo) |
| 56 / 57 / 58 / 59 | só `0x437d40` (trilho) / `0x437db0` (trens) / `0x437f50` (estações) / `0x4380a0` (prédios) |
| 189 | exige empresa: `0x438170(empresa, 1)` desativa a empresa e a retira das contagens dos laços |
| 190 | pessoa de IA (`+0x122 == 0`): `0x4a6a00(pessoa)` elimina o jogador e o retira dos laços |
| 191 | `0x467bd0(pessoa)`: um humano assume o jogador |
| 192 | `b > 0`, pessoa: troca nome (`+8`, 30 bytes) e descrição (`+0x27`, 250) por textos LNG da tabela de personas `0x551e89` (35 bytes por entrada: LNG do nome em `+0x1e`, da descrição em `+0x20`, byte `+0` copiado para `pessoa+0x132`) e grava o índice em `pessoa+0x52f` |
| 233 | `v = (b == 0x9c) ? valor_do_gatilho : b`; `v > 0`: `0x4c8a30(território, v)` inunda |
| 235 | `b > 0` e o trem `b` existe no pool `0x574c64`: `0x4cdd50(b)` aposenta o trem |
| 236 | `território.+0x44..+0x47 = (b >= 0)` (todos "à venda") |
| 237 / 238 / 239 / 240 | idem só `+0x45` / `+0x44` / `+0x46` / `+0x47` |
| 241..244 | `[0x573d8e + 4*(e-241)] = v` (variável global; `v` como em 233) |
| 245..248 | `+= v` na mesma variável |
| 249..252 / 253..256 | empresa: `empresa+0x4219 + 4*i` `= v` / `+= v` (exige empresa) |
| 257..260 / 261..264 | pessoa: `pessoa+0x534 + 4*i` `= v` / `+= v` (exige pessoa) |
| 265..268 / 269..272 | território: `território+0x2b + 4*i` `= v` / `+= v` (exige território) |
| 297 / 298 | `empresa+0x4260 += / =` `(b == 0x9c ? gatilho : effect_money(b)/1000)` (exige empresa) |
| 299 | confisco com flag 1 ("destruir"): `0x437d40(..., 1)`, `0x437db0`, `0x437f50(..., 1)`, `0x4380a0` |
| 300 / 301 / 302 / 303 | só `0x437d40(.., 1)` / `0x437db0` / `0x437f50(.., 1)` / `0x4380a0` |
| outros (0, 8..49, 60..188, 193..232, 234, 273..296, 304..349) | **nível**: `pessoa != 0`: `pessoa.i16[0x26b + 2e] += b`; senão `empresa != 0`: `empresa.i16[0x1ae + 2e] += b`; senão `território != 0`: `território.i16[0x58 + 2e] += b`; senão global `i16[0x5707c6 + 2e] += b`. Depois: se `+0x352 + 4k == 0` (temporário) marca a regra de desfazer; se `e` em `0x7f..0xbc` ou `0x131..0x134` liga `[0x574784]`; se `e` em `0xc1..0xe8` e empresa != 0, `0x438780(empresa, e - 0xc1, 0)` |

Notas: `b == 0x9c` (-100) só existe nas linhas com mínimo -100 (variáveis, inundação, trilho permitido) e significa "o valor numérico que a condição produziu" (`*saída` de `rule_reduce`, o mesmo que `(TRIGGER)`); os efeitos 1..7, 50..59, 189..192, 233..240, 297..303 ignoram o limite de escopo "pessoa > empresa > território > global" do ramo de nível porque usam as entidades do contexto diretamente. Os acréscimos de nível são **cumulativos**: uma regra de condição permanentemente verdadeira e sem "uma vez só" soma `b` a cada avaliação (o editor avisa isso no texto do tutorial).

Efeitos de nível lidos por: `effect_multiplier` `0x46b480` (`cargo.md` 5): `f(n) = max(n, -99) * 0.01 + 1`, compostos global x empresa x pessoa x território, piso 0,01. O consumo de disponibilidade de locomotivas e gerentes (ids 127..188, 193..232, 305..308) **não foi rastreado** (Em aberto).

### 4.3 Dinheiro (`0x45c150`)

Controle deslizante `b` (i8) dos efeitos de dinheiro: `v = |b|`, sinal `+` se `b >= 1`, `-` se `b <= 0`. `v <= 10`: `v * 1000`; `11..59`: `(v - 9) * 10000` (ex.: 14 = US$ 50 000, 19 = US$ 100 000, como nos títulos "Choice 1: +$50k"/"+$100k" do cenário 1); `60..104`: `(v - 54) * 100000`; acima: `(v - 99) * 1000000`. Teste diferencial para todo `b` em -127..127.

### 4.4 Efeitos temporários e regra de desfazer

Se, ao aplicar o conjunto `k`, algum efeito de nível rodou com `+0x352 + 4k == 0` (temporário), a avaliação cria (depois dos laços) uma **regra de desfazer** de 0x362 bytes anexada ao pool: gatilho 2, modo de data (`+0x10 = 1`) com `(ano, mês) = undo_date(ano, mês, +0x35a + 4k)` onde `t = meses + ano*12 - 1 + mês`, `ano = t / 12`, `mês = t % 12 + 1`, `+0x1c` pessoa, `+0x20` empresa e `+0x24` (empresa != 0), `+0x28` (pessoa != 0) do contexto, filtros `+0x38..+0x44 = 1`, "uma vez só" (`+0x48 = 1`), permanente, textos vazios, e efeitos `-b` nas linhas não instantâneas (`+0xb == 0`) e 0 nas instantâneas. Ao chegar a data (gatilho 2 = início de mês) a regra aplica `-b` pelo mesmo ramo (somando de volta). Peculiaridades: (i) só os efeitos do ramo de nível marcam o desfazer; um conjunto temporário só com direitos de território (50..54) nunca é desfeito; (ii) quando é desfeito, a negação também vale para os direitos de território do mesmo conjunto (`-1` = negar); (iii) a regra de desfazer fica no pool (e é salva) mesmo depois de disparar (`+0x4c = 1`).

## 5. Vitória, derrota e campanha

**Estado de fim de jogo** `[0x573d70]` (u8): 0 em jogo, 1/2/3 vitória bronze/prata/ouro, 4 derrota. Zerado em `0x472980` (novo mundo) junto com `[0x573d71]` ("tela já mostrada").

Efeitos 1..4 (`rule_evaluate`), só se `[0x573d70] == 0`: a pessoa-alvo é a do contexto, senão a presidente da empresa do contexto (`empresa+0x23`), senão o único jogador do conjunto, senão (rede com um jogador) o jogador de `0x46b3c0`.
- **Derrota (4)**: se o alvo é o jogador local (`[0x566ca4]`) ou 0: `[0x573d70] = 4`, texto da regra para o buffer `[0x574b90]`; senão `0x4a6a00(alvo)` elimina esse jogador (sem fim de jogo).
- **Vitória (1..3)**: se o alvo é o jogador local: `[0x573d70] = e` e o texto da regra; senão `[0x573d70] = 4` com a mensagem LNG `0xa83` (outro jogador venceu: o local perde).
Depois de qualquer mudança de `[0x573d70]`, `event_fire_trigger` encerra a varredura (2.2).

**Quando é verificado**: a condição é uma regra comum, avaliada nos gatilhos dela (nos cenários da campanha, em geral 1 e 3, "fim de ano"/"fim de mês", e 2 para o prazo `GameYear>=...` da derrota). O laço principal da interface (`0x453970`) testa a cada quadro: se `[0x573d70] != 0` e `[0x573d71] == 0` (e o estado da UI não é `0xb`/`0xc`) chama `game_over_screen` (`0x4e9280`) e retorna.

**Medalhas na campanha** (`[0x570234] != 0`): `game_over_screen` grava em `[0x566d04 + índice]` (18 bytes, índice do cenário `[0x566d16]`): se o valor atual é 0 ou 4, recebe o estado; senão, se o estado novo é != 4, fica o maior (`max`, a derrota nunca apaga uma medalha). Mostra o quadro de vitória/derrota (`0x4e91c0`) com o texto `[0x574b90]` fornecido pela regra (vazio se a regra não tinha mensagem). Fora da campanha, em partida local de um jogador e com vitória, abre o registro de placar (`0x4701f0` com `estado - 1`).
- **Progressão**: se não é o último cenário do conjunto ou o resultado é derrota (4), a UI volta ao estado `0xc` (`0x40de20(0xc, 0)`, escolha do próximo cenário). O último cenário é o índice `0x11` nos conjuntos 0 e 1 (`[0x566d3d]`; 18 cenários), `2` no conjunto 2 e `5` no conjunto 3; ganhar o último toca o final (`0x4bc1d0`: `0x52` nos conjuntos 1/3, `0x1f` nos demais), conta as medalhas bronze/prata/ouro dos 18 bytes e abre o placar da campanha (`0x4701f0(1, ..., dificuldade [0x566d00], n_bronze, n_prata, n_ouro)`).
- **Escolhas e dificuldade da campanha**: `CampaignChoice1To3` (`[0x566d2a] + 1`) e `CampaignDifficulty1To3` (`[0x566d00] + 1`), válidas só com `[0x570234] != 0`, alimentam regras de gatilho 2 sem mensagem (`GameYear>=1830 and CampaignChoice1To3=1` com efeito `+$50k`, etc., cenário `#01B&O`). A tabela de cenários da campanha é `0x552430` (24 bytes por cenário: ponteiro do nome-base do arquivo (`#01B&o~1`...), ids LNG de título/briefing/textos, número da região e id LNG do cenário; 18 por conjunto, 4 conjuntos); `0x410de0` carrega o cenário escolhido: semeia o jogo com `crt_rand_range(1,32000) + clock_ms()`, grava `[0x566d2a]` de `[0x564fe8 + índice/6]` e `[0x566d16] = índice`.
- As metas concretas (bronze/prata/ouro) são regras com os efeitos 1/2/3 e a derrota o efeito 4, por exemplo no `#01B&O`: ouro = `CompanyTerritoryConnectedToTerritory(4) and TerritoryConnectedToTerritory(3) and TerritoryConnectedToTerritory(5) and TerritoryId=2 and CompanyLoadsHauledLifetime>=12` (gatilho 3, uma vez só, efeito 3 e `Set GameVariable1 = 100`), prata/bronze com `GameYear=1840` (gatilho 1), derrota `GameYear>=1841` (gatilho 2, efeito 4). O manual descreve as metas dos 18 cenários e 12 avulsos; elas agora saem direto do `.MAP`/`.MP2` e não de tabela do exe.

## 6. Dados reais (128 arquivos, 3790 regras)

`cargo test -p rt2-formats --test scenarios` com `RT2_DIR` (128 arquivos entre `maps/` e `games/`: `.MAP`, `.MP2` e `.GAM`):
- todos os 3790 registros têm parte fixa de `0x2C8` (599, `.MAP`) ou `0x362` (3191, `.MP2`/`.GAM`), `+4` igual ao nº de tokens e `compile(condição)` igual aos dois vetores gravados;
- **ids de token desconhecidos: 0**; 103 dos 118 ids aparecem; não aparecem: 3 `GameNumPlayers`, 6 `GameExistsPlayerId`, 13, 16, 17, 34, 35, 48, 49, 55 `PlayerStockValue`, 65, 81 `!=`, 101 `CompanyIndustryProfitsLastYear`, 103, 104;
- gatilhos: 0:391, 1:529, 2:1340, 3:505, 4:272, 5:79, 6:303, 7:371;
- 6 regras mal formadas (3.2); as outras 3784 reduzem sem erro de forma;
- efeitos: 261 ids de efeito distintos aparecem (máximo 308), todos dentro da faixa da tabela; `Game Over` 1/2/3/4: 131/132/178/132 usos (os dois conjuntos somados).

## 7. Testes

- `crates/rt2-sim/tests/scenarios.rs` (precisa de `RT2_DIR`): compilação x tokens gravados, ids, flags de escopo derivadas, forma das condições, efeitos; separação de palavras e números.
- `crates/rt2-oracle/tests/scenarios.rs` (executável emulado): `token_table_matches_executable` (118 entradas), `effect_table_shape`, `shipped_effect_values_are_in_range` (precisa de `RT2_DIR`), `reducer_matches_original_on_well_formed_expressions` (6000 expressões aleatórias, valor, sucesso, estados do RNG do jogo e do CRT), `reducer_matches_original_on_malformed_token_streams` (4000), `class_aggregation_matches_original` (600 casos nas três dimensões, pools montados na memória emulada), `class_parameter_error_paths_match_original`, `every_shipped_condition_reduces_like_the_original` (3649 condições reais com o avaliador de funções trocado por um stub que devolve 7, 6 falham nos dois, 141 regras de classe puladas), `effect_money_scale_matches_original`.

## 8. Divergências do manual (o código vale)

- Divisão por zero: `editor3.txt` diz "if right side is 0, always evaluates to 1"; o código usa divisor 1, então `x / 0 = x` (3.2).
- `!=` está listado como comparador, mas o divisor de palavras o parte em `!` e `=`: inutilizável pelo texto (3.1). `> =` também falha.
- "Test Against Multiple ... it will stop after the first true evaluation" (uma vez só): o código **continua avaliando** (e sorteando o RNG) e só pula mensagem e efeitos (2.3).
- Ordem dos laços: jogadores (fora), empresas, territórios (dentro) (2.3); o manual só diz "ordem das listas".
- Funções: o manual cita "over 70" e 84 entradas (Apêndice A); o exe tem 101 funções e 350 linhas de efeitos (manual: "approximately 250", 167 entradas). Funções do TSC (`EDITOR2.TXT`) `CompanyTerritoryLoadsHauledFromThisYear/Lifetime` existem na tabela mas **valem 0** no código; `CompanyIndustryProfitsThisYear` devolve o lucro do ano anterior (igual a `...LastYear`); `PlayerId` devolve o id da empresa do contexto; `CompanyDebt` é negativo; `CompanyTerritoryGoodwill` passa o território a `0x43eca0` (média com a posição dele), não é o mesmo valor de `CompanyGoodwill` (o argumento de pilha não aparecia na decompilação).
- Rótulos de vitória: o manual diz bronze/prata/ouro (E3) ou último/segundo/primeiro lugar (AG); o código guarda 1/2/3 como medalha da campanha e no placar usa `estado - 1`; vitória de outro jogador vira derrota local.
- Efeitos de nível são cumulativos a cada avaliação verdadeira (4.2); `Variable` com -100 usa o valor do gatilho.

## 9. Em aberto

1. Flags `+2` e `+6..+10` da tabela de efeitos (provavelmente "visível" e escopos permitidos no editor) e as 41 linhas 309..349, a linha 234 sem ramo.
2. Diferença entre `Territory Confiscate` (55..59, flag 0) e `Territory Destroy` (299..303, flag 1): semântica de `0x437d40/0x437db0/0x437f50/0x4380a0`, e de `0x4c8540` (máscaras), `0x4c8a30` (inundação), `0x438780`, `0x438170`, `0x467bd0`, `0x4cdd50`.
3. Quem lê os níveis de disponibilidade (127..188, 193..232, 305..308) e `[0x574784]`.
4. Nomes finais dos campos `+0x32a`, `+0x32e`, `+0x34f/+0x350` (a avaliação "local" não aplica efeitos: provavelmente para não dessincronizar a rede) e de `[0x5665f0]`, `[0x5748fc]`.
5. Mensagens: janelas (`0x469320`, `0x4a42f0`, `0x4510a0`) e a ordem de exibição em rede (escolha sempre 0 com 2 ou mais jogadores).
6. A função `0x4c0240` e a leitura de `empresa+0x16e` ("ativa") em `GameExistsCompanyId` (resolvido: `CompanyTerritoryGoodwill` passa o território a `0x43eca0`, seção 3.5).
7. Progressão da campanha: seleção do próximo cenário (UI `0xc`), tabela `0x552430` campo a campo, conjuntos 2 e 3 (3 e 6 cenários), `[0x564fe8]`.
8. Avaliação de teste do editor (`0x457a30`, `execute = 0`) sorteia o RNG; só é relevante se o editor for portado.
9. Uso dos 15 ids de função que nenhum arquivo do jogo usa (lista na seção 6): só implementados por leitura do código.
