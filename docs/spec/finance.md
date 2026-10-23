# Finanças: razão da empresa, preço da ação, títulos, dividendos, falência

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido a partir de `re/db/decomp` e do desmontador (a decompilação do Ghidra perde argumentos de `_ftol` e larguras f32/f64; as constantes abaixo foram lidas do `.rdata`/`.data` conforme a instrução x87 real). O manual (`docs/reference/manual-rules.md`, seção 8) só serviu de pista; as divergências estão na seção 12. Estado das provas: **[V]** = confirmada por teste diferencial (`crates/rt2-oracle/tests/finance.rs` e `company.rs`), **[S]** = lida no código, sem teste. Endereços são VAs (base `0x400000`). Todas as contas usam a precisão x87 de 53 bits (CW `0x27F`, ver `time.md`).

## 1. Registro da empresa e do jogador (campos usados pelas finanças) [S]

Lista `list_companies` (`0x574c6c`, 0x4e86 bytes por registro). O contêiner de conjunto de jogadores/empresas (`0x46a7e0` = quantidade, `0x46a7f0(i)` = i-ésima empresa; índice `0x43a9e0(empresa)` = posição da empresa nesse conjunto, usado para indexar as carteiras das pessoas, máximo 36).

| Offset | Tipo | Conteúdo |
|---|---|---|
| `+0x00` | i32 | id da empresa |
| `+0x04` | char[] | nome |
| `+0x23` | i32 | id da pessoa que é chairman (0 = sem chairman) |
| `+0x2c` | i32 | ações em circulação |
| `+0x30` | f32 | dividendo anual por ação |
| `+0x34` | f32 | pressão de negociação acumulada, em ações (`trade_pressure`; x0,9 por mês, 5.1) |
| `+0x38` | f32 | dividendos pagos no ano, por ação (zerado no fim do ano) |
| `+0x3c` | f32 | preço inicial da ação |
| `+0x40` | i32 | quantidade de títulos (bonds), máximo 20 |
| `+0x44` | 20 x 12 B | títulos: `{i32 principal, i32 ano de vencimento, f32 taxa}` |
| `+0x134` | i32 | salário anual do chairman (10 000 ao trocar de chairman, `0x438900`) |
| `+0x138` | i32 | overhead anual (5.5) |
| `+0x13c` | i32 | gerente atual (índice na tabela de gerentes, stride 0x11; salário i32 em `0x54fdad + 0x11*i`; -1 = nenhum) |
| `+0x140` / `+0x144` | i32 | os dois candidatos a gerente (regerados em anos ímpares, 13.6) |
| `+0x2b` | u8 | cor/logotipo (0..40) |
| `+0x148` | i32 | ano de fundação |
| `+0x46a` / `+0x474` | i32 | desempenho do chairman (13.1) / último bônus pago (13.3) |
| `+0x46e` / `+0x470` / `+0x472` | i16 | ano do último bônus / ano em que o chairman atual assumiu / ano do último aviso de demissão (13.3, 15.4, 13.7) |
| `+0x479 + t*0x1ed` | blocos por território (`t` = 0..31) | `+0` byte de direitos, `+1` i16 posição da empresa no território (13.4); `+0x4229` i32 posição geral; `+0x4248` f32 multiplicador de receita (13.5) |
| `+0x14c` | i32 | minuto da última tentativa de fusão fracassada |
| `+0x158` | i32 | ano da última falência (usado como idade nas penalidades de preço e rating) |
| `+0x160/+0x164` | data, hora | momento da última emissão de ações |
| `+0x16e` | i32 | empresa ativa (0 = inexistente; as rotinas devolvem 0) |
| `+0x1ae` | i16[effect] | efeitos da empresa (ver `cargo.md`) |
| `+0x46a` | i32 | reputação/desempenho do chairman |
| `+0x4250` | i32 | carimbo (`game_time_minutes`) do cache do preço fundamental; **zerado a cada lançamento contábil** |
| `+0x4254` | f64 | preço fundamental em cache |
| `+0x425c` | i32 | minuto de criação da empresa |
| `+0x4276` | f32 | atitude dos investidores (5.1) |
| `+0x427a` | f32 | preço "sem dividendo" (3.1) |
| `+0x427e` / `+0x4282` | f32 | último preço negociado / preço do fim do último mês |
| `+0x4286` | f64[13] | categorias de receita/despesa do ano corrente |
| `+0x46fe` | f64[13] | acumulador de **vida inteira** das 13 categorias (nunca zerado; é o que as rotinas de arquivo leem quando o "ano" vale 9001 = `0x2329`; LNG 2368 "Lifetime"; seção 16.3) |
| `+0x42ee` | 10 x 13 f64 | arquivo anual das categorias (`k-1`-ésimo ano atrás, 0x68 por ano) |
| `+0x4766` | f64[8] | contas: 0 caixa, 1 prédios, 2 terrenos/estações, 3 trilho, 4 **trens** (`train_create` `0x4cdf80` lança aqui), 5 **indústrias compradas** (`0x43eb60` lança aqui; `scenarios.md` `CompanyIndustryInvestments` lê a conta 5), 6 **dívida (negativa)**, 7 **patrimônio (negativo)** |
| `+0x47a6` | 10 x 8 f64 | arquivo anual das contas |
| `+0x49b6` | 11 x 14 f64 | arquivo anual das métricas (`k` anos atrás em `0x49b6 + 0x70*k`, `k = 1..10`; `k = 0` é a métrica viva) |

Categorias (13 f64 em `+0x4286 + 8*i`, rótulos LNG 961..970): 0 Passageiros, 1 Carga, 2 Estação, 3 Investimento em indústrias, 4 Diversos (receitas); 5 Trilho, 6 Motor, 7 Vagão, 8 Combustível, 9 Juros, 10 Overhead, 11 Salários, 12 Diversos (taxas de emissão e corretagem) (despesas, valores negativos). `receita = Σ cat[0..4]`, `lucro = Σ cat[0..12]`.

Lançamento **[V]** (`company_ledger_credit` `0x439970(cat, x)`): `cat[i] += x; ano[i] += x` (`ano` = f64[13] em `+0x46fe`, a cópia usada no marcador 9001); `caixa += x; conta7 -= x` e zera o carimbo `+0x4250`. `0x4398d0(conta, x)` **[V]**: conta 0 faz caixa `+= x`, `conta7 -= x`; conta `k != 0` faz `conta[k] += x; caixa -= x`. Partida dobrada: Σ contas 0..6 + conta7 = 0.

Pessoa (lista `list_persons` `0x574c68`, 0x576 B): `+0x04` ativa, `+0x122` humana, `+0x12a` f64 caixa, `+0x1d3` i32[36] ações por empresa (negativo = posição vendida), `+0x263` empresa de que é chairman, `+0x26b` efeitos, `+0x52b` reputação, `+0x544` prazo da chamada de margem (minuto, 0 = nenhuma). Arquivo anual de patrimônio em `+0x123`.

Métricas por empresa (`0x43ad60(i)`, as 14 do arquivo): 0 preço (`0x43b410(0,0)`), 1 patrimônio por ação (`-conta7/ações`), 2 receita por ação (`0x43a440(14)/ações`), 3 lucro por ação (`0x43a440(13)/ações`), 4 dividendo (`+0x30`), 5 retorno ponderado de 5 anos (`0x43abc0`), 6 rating de crédito (`0x43b650`), 7 `+0x38`, 8..13 contadores (`+0x192`, `+0x196`, `(+0x186 + 2*+0x18a)*10`, `+0x19a/+0x196`, `+0x46a`, `+0x134`).

## 2. Anualização do ano corrente (`0x43a440(i)`) [V, i = 0..15; fora o ano 9001]

```
raw = (i < 13) ? cat[i] : (i == 13) ? Σ cat[0..12] : (i == 14) ? Σ cat[0..4] : Σ cat[5..12]
se mes == 12 e dia == 28: devolve raw
f = (dia + 28*mes - 28) * 0.002976190476      ; guardado como f32; < 0.05 -> 0.05f; > 0.95 -> 0.95f
se ano <= fundação: devolve raw / f
se i in {3, 13, 14}: raw += f * ano_anterior.cat[3]           ; (arquivo k=1)
devolve f*raw + (1 - f) * ano_anterior(i)                       ; i == 13/14 usam as somas
```
Constantes: `0x535580` = 1/336, `0x535470` = 0,05, `0x535570` = 0,95. O somatório arquivado é `0x43a740(k-1, i)`.

## 3. Preço da ação

### 3.1 Preço fundamental (`0x43aea0`, cache por minuto) [V]

Entradas por ano `i = 0..4` (ano atual e quatro anteriores; só anos `>= fundação`): peso `w_i = i == 0 ? 5*mes - 5 : (5-i)*12`; `E = Σ w_i*lucro_ps_i`, `R = Σ w_i*receita_ps_i`, `W = Σ w_i` (o ano 0 usa as métricas vivas 3 e 2, os demais o arquivo). Dividendos, `i = 1..9` com `ano-i >= fundação`: `v_i = 10 - i`, `F = Σ v_i*div_i`, `V = Σ v_i`, e `C = Σ v_i` dos anos com `ano-i-1 >= fundação` (o original compara o dividendo do ano com ele mesmo, então a condição "dividendo não caiu" é sempre verdadeira; bug fiel).

```
crit[0] = (E/W)*10              ; lucro   (só se W != 0, senão 0)
crit[1] =  R/W                  ; receita
d = f32(F/V); d = min(d, f32((2*div_ano_ant + d) * 0.3333333433))   ; só se V != 0
crit[2] = ((C/V)*0.5 + 0.6) * d * 30                                   ; dividendo
crit[3] = patrimônio_ps (métrica 1 viva)                              ; valor contábil
m = max(1.0f, crit[0], crit[1], crit[3]); crit[2] = min(crit[2], m*1.13)
crit[k] = max(crit[k], 0)
peso = {40, 10, 20, 30};  se ano - fund < 5: peso[j] *= T[ano-fund][j]
        T (0x550074) = {0,0,0,100} {25,25,35,100} {50,50,65,100} {75,75,85,100} {100,100,100,100}
peso[argmax crit] *= 1.25;  peso[argmin crit] *= 0.8                 ; primeiro índice em empate; argmax antes
X = Σ crit*peso, Wt = Σ peso;  Y, Wy = os mesmos sem o índice 2
+0x427a = f32(max(0.5, (Y/Wy) * (econ[estado] + 1) * 0.5))
P = efeito(0x1e, empresa) * ((X/Wt) * econ[estado])         ; econ = 0x5504d0 {0.8,0.9,1.0,1.1,1.2}
se ano - +0x158 < 10: P = ((ano - +0x158)*0.025 + 0.75) * P
P = max(P, 1.0)  -> +0x4254, carimbo +0x4250 = game_time_minutes
```
`efeito(0x1e)` é a linha "Stock Prices" dos eventos (`0x46b480`, ver `cargo.md`). Os pesos 40/10/20/30 e os multiplicadores 10/1/30/1 do manual batem; o código acrescenta o ajuste por idade da empresa, a penalidade pós-falência de 10 anos, o piso de 1,00, o fator do estado econômico e o realce +25% do melhor/-20% do pior critério.

### 3.2 Preço de negociação (`stock_trade_price` `0x43b410(n, commit)`, thiscall) [V]

`n` = ações compradas (>0) ou vendidas (<0) neste negócio, 0 para só cotar.
```
Pf = 0x43aea0();  se f32(+0x3c) < Pf e ano - fund <= 5:
    m = min(50, (game_time - +0x425c) / 40320)               ; meses
    Pf = ((50-m)*(+0x3c) + (50+m)*Pf) * 0.01                  ; mistura com o preço inicial
x   = clamp(n / ações, -0.2, +0.2)                            ; f32 0.2 em 0x5355fc
press = (n != 0) ? (1 - x)*(+0x34) + n : +0x34               ; se commit, +0x34 = press (f32)
r = (S + 1.4 * press * (S*5e-5)^0.33 * (100/liq)) / S         ; S = ações, liq = 0x5706cd (100, "Stock Liquidity")
r: se r < 0.6: r = 0.6 + (r-0.6)/2;  se r < 0.45: 0.45 + (r-0.45)/2;  se r < 0.3: r = 0.3
   senão: r > 1.7: 1.7 + (r-1.7)/2; r > 2.5: 2.5 + (r-2.5)/2; r > 4: 4 + (r-4)/2; r > 6: r = 6
preço = ftol(r*Pf + 0.5)            ; se commit, +0x427e = (f32) preço
```
O modificador de negociação do manual (mínimo 30% do fundamental) é `r` (piso 0,3, teto 6; o manual diz "sem teto"). A pressão decai (1-x) a cada negócio e x0,9 por mês (5.1), o que reproduz "o efeito diminui com o tempo". `pow` é o `0x51bab0` da CRT.

### 3.3 Cadência de atualização [S]

Não há "preço armazenado" que seja atualizado em passo próprio: o preço é função do estado, recalculada sob demanda e guardada por minuto de jogo em `+0x4254` (qualquer lançamento zera o carimbo). O que o jogo grava:
- cada negócio com `commit = 1` grava a última cotação em `+0x427e` e a pressão em `+0x34`;
- passo mensal da empresa (`0x4399d0`, 5.1): `+0x34 *= 0.9`, cotação com commit e `+0x4282 = +0x427e`;
- mudança do estado econômico (`0x469fb0`/`0x46a010`): cotação com commit de **todas** as empresas (`0x43b410(0,1)`);
- fim do ano (`0x43d800`, 8): cotação com commit, arquivo anual e possível desdobramento.
O patrimônio dos jogadores usa a **última cotação gravada** (`+0x427e`) nas chamadas de margem (7.3) e a cotação viva (`0x43b410(0,0)`) no patrimônio exibido (7.1).

### 3.4 Emitir e recomprar ações [S]
- **Emitir** (`0x439060(n, flag, preço)`, comando `0x11` com valor > 0): se `flag == 0` (jogador): `preço = ftol(0x43b410(ftol(-0.7*n), 1))`; taxa de subscrição `n*preço*0.05` na categoria 12; grava data e hora em `+0x160`. Depois `ações += n` e caixa `+= n*preço` (conta 0, patrimônio recebe o contrário). O jogo de rede/IA chama com `flag != 0` e preço já calculado.
- **Recomprar** (`0x438f70(n)`, comando `0x11` com valor <= 0, n = -valor): `preço = ftol(0x43b410(ftol(0.7*n), 1))`; corretagem `0.03 (0x5706c9) * n * preço` na categoria 12; `ações -= n`; caixa `-= n*preço + corretagem`.
- Restrições de interface (mensagens LNG 276..278): preço mínimo e retorno ponderado de 5 anos mínimo, um ano entre emissões (`+0x160`). Os valores exatos ficam em aberto (UI).

## 4. Rating de crédito e juros

### 4.1 Taxa prime (`0x46bac0(id_empresa)`) [V]
`prime = ftol((efeito_pct(0x20, empresa) * 0.01 + prime_rate + 0.001) * 100) * 0.01`, com `prime_rate` = f32 em `0x5706d1` = `0,09 - estado*0,01` (`0x469fb0`/`0x46a010`; inicial `0,0x3da3d70a` = 0,08 no estado 2). O `efeito_pct(0x20)` é aditivo (soma global + empresa + pessoa + território, `0x46b5d0`). O resultado é sempre um percentual inteiro: 9, 8, 7, 6 ou 5% (estado 0..4) mais o efeito.

### 4.2 Rating (`0x43b650(X)`, X = dívida adicional hipotética; métrica 6 usa X = 0) [V]
Devolve um inteiro 0..10 (como f64): LNG 1020..1030 = F, D, C, CC, CCC, B, BB, BBB, A, AA, AAA.
```
A    = Σ_{i=1..4, ano-i >= fund} (5-i) * (lucro_i - juros_i) / 10     ; lucro_i = Σ cat13 do ano, juros_i = cat[9] (negativa)
taxa = Σ principal*taxa / Σ principal dos títulos (0 sem títulos)
est  = (conta6 + X) * taxa                       ; conta6 é negativa: o X soma ao contrário (bug fiel)
se caixa < 0: est += caixa * (prime + 0.03)
cob  = (est == 0) ? 10 : A / (-est)
s1   = cob >= 5: 10; cob > 2: 5 + cob; cob > 1: 3 + 2*cob; cob > 0: 1 + 4*cob; senão 0
se ano - fund < 5: s1 += (5 - idade) * 0.1 * (10 - s1)
T    = ftol(Σ conta[0..5]);  razão = (T > 0) ? (-conta6 + X) / T : 1
s2   = razão <= 0.1: 10; < 0.25: 10 - 4r; < 0.5: 9 - 8(r-0.25); < 0.75: 7 - 12(r-0.5); < 1: 4 - 16(r-0.75); senão 0
R    = ((s1 + s2)*0.5 + 0.2*s1*s2) / 3 * econ[estado]
E    = -conta7;  se E < 1e6: R -= 10 - E*1e-5
idade<5: a = max(idade,1); R -= (5-a)*0.5; R = min(R, a+5)
ba   = ano - +0x158;  se ba < 15: R = ba*R*0.0666
rating = clamp(ftol(R + 0.5) + efeito_int(0x1f), 0, 10)
```
Para obter títulos é preciso rating >= `0x5706d5` = 5,0 (= "B"), ignorado se `0x5748fc != 0`. A falência zera quase o rating durante 15 anos (fator `ba/15`).

### 4.3 Taxa de um novo título (`0x4392e0(X)`) [V]
`taxa = prime(empresa) + (10 - rating(X)) * 0.01`: prime mais 1% por nível abaixo de AAA (bate com o manual; o manual não diz que o rating inclui o novo principal).

### 4.4 Emitir e quitar títulos [S]
- **Emitir** (`0x439320(principal, prazo, taxa_flag)`, comando `0x14`): recusa se já há 20 títulos ou rating(0) < 5,0. Taxa = 4.3 com `X = principal`. Se a flag: taxa de subscrição `0,02 * principal` (categoria 12). Caixa `+= principal`, `conta6 -= principal`; grava `{principal, ano + prazo, taxa}`. A IA usa principal 500 000 e prazo 30; o jogador escolhe pela interface (a granularidade de $500 000 é do manual; o código aceita o valor do comando).
- **Quitar** (`0x4391b0(i, multa)`, comando `0x15`): `conta6 += principal`, caixa `-= principal`; se `multa`: `0,02 * principal` na categoria 12 (LNG 288 "prepayment fee"); remove o título (desloca a lista).
- **Vencimento**: no fim do ano (8), todo título com `vencimento == ano + 1` é quitado sem multa.
- **Juros**: mensais sobre o caixa (5.1) e trimestrais sobre os títulos (`taxa_média * conta6 * 0,25` na categoria 9, nos meses 3, 6, 9 e 12), sempre pela taxa média ponderada (não pela taxa de cada título).

## 5. Passo mensal da empresa (`0x4399d0`, por empresa ativa, dia 28 às 23:59; `time.md` 7.4) [S]

Não corre se `0x5748f8 != 0`. Ordem:
1. **Atitude dos investidores** `+0x4276`: se há chairman humano: `0`; senão `mood = mood*0.92 + ((1 - H/(ações+1)) + 1)*0.5*0.08` com `H` = ações detidas por jogadores humanos.
2. **Manutenção de trilho** (cat 5): `gasto = conta3 * -0.005` (0 se o valor do trilho for negativo), vezes `efeito(0xc)`; vai ao caixa e à categoria 5.
3. **Overhead** (cat 10): `-(+0x138)/12 * efeito(0xf)`.
4. **Juros do caixa** (cat 9): caixa `>= 0`: `(prime - 0.04) * caixa / 12`; caixa `< 0`: `(prime + (10 - rating)*0.01 + 0.02) * caixa / 12`.
5. **Se `mes % 3 == 0`** (trimestre): juros dos títulos (4.4) e **dividendos** (seção 6).
6. **Salários** (cat 11): `-(+0x134)/12` (e o chairman recebe `ftol(+0x134/12)` no caixa pessoal) e `-salário_gerente/12 * efeito(0x10)`.
7. `+0x34 *= 0.9`; `0x43b410(0, 1)`; `+0x4282 = +0x427e`.

## 6. Dividendos (resolve a pendência de `time.md`) [S]

Dividendos são pagos **trimestralmente: no passo mensal de março, junho, setembro e dezembro** (dia 28 às 23:59, dentro de `0x4399d0`, `mes % 3 == 0`), antes dos salários.
```
d = +0x30 (anual, por ação);  pago = ações * d / 4
caixa_depois = caixa - pago
máx = 0x43edf0()           ; dividendo anual máximo suportável (6.1)
se d > 0 e (caixa_depois < -0.2*patrimônio  ou  máx <= 0.01):  d = 0; +0x30 = 0; mensagem LNG 2274 "dividendo suspenso"
senão se máx < 0.75*d:   d = máx; +0x30 = f32(máx); mensagem LNG 2713 "conselho reduziu o dividendo para %1" (só empresa do jogador)
se d > 0:
    caixa -= ações*d/4;  conta7 += ações*d/4                 ; não passa pela demonstração de resultados
    +0x38 += d/4
    para cada pessoa com ações da empresa (positivas ou negativas): caixa_pessoa += ftol(ações_pessoa * d / 4)
```
Ações fora das carteiras das pessoas (público) recebem o dividendo do caixa da empresa mas o dinheiro não vai a ninguém. O controle do jogador (comando `0x12`, `0x43bfd0`) só grava `+0x30` sem validar; a recusa a aumentos acontece na interface (LNG 2370, "refuses to let you raise the dividend", usando 6.1).

### 6.1 Dividendo máximo suportável (`0x43edf0`) [V]
```
cashps = f32(caixa / (ações + 1));  n = min(3, ano - fund)
NI = max(0, max_i lucro_i), Rv = max(0, max_i receita_i)   (i de (mes == 12 ? 0 : 1) a n, ano i atrás;
      i = 0 usa as somas brutas do ano corrente cat[0..12] e cat[0..4], sem anualizar; i >= 1 usa o arquivo k = i-1)
X = max(NI, Rv);  y = min(NI, 0.5*X)
se n < 2: y = max(y, ações * +0x3c * 0.05)
y = f32(min(y / (ações + 1) * 1.25, cashps))
ref = (n == 0) ? +0x3c : preço do fim do ano anterior (arquivo, métrica 0)
máx = max(0, min(ref * 0.35, y))
```
Lido no desmontador e provado: os dois máximos começam em 0 (`0x535340`), `X = max(NI, Rv)` só limita `y` a metade de `X` (o motivo de `max(NI, Rv)` continua desconhecido, mas a conta está provada).

## 7. Jogador: ações, patrimônio e margem

### 7.1 Patrimônio pessoal [S]
`0x4a7620`/`0x4a76b0(2)` = `caixa (+0x12a) + Σ_c ações_c * 0x43b410(0,0)_c` (soma sobre as 36 posições; vendidas contam como negativo).

### 7.2 Compra e venda [S]
- **Compra** (`0x4a7300(pessoa, c, n, forçada)`): se não forçada exige `0x4a6f60`; `0x43b410(n, 1)`; custo `= ftol(cotação_após(0,0) * n)` e débito `ftol(custo * 1.03)` (corretagem 3% do `0x5706c9`); `ações_pessoa += n`; depois `0x46bb10` (chamadas de margem).
- **Venda** (`0x4a73d0(pessoa, c, n, forçada)`): limita `n` às ações detidas (exceto forçada/venda a descoberto); `preço = ftol(0x43b410(-n, 1))` (cotação **durante** a venda), receita `ftol(0.97 * preço*n)`.
- **Permissões** (`0x4a6f60`/`0x4a7190`): comprar ações de outra empresa exige `0x573d74` (modelo avançado/expert) ou ser o chairman; margem exige `0x573d72` e `0x5706c0`; venda a descoberto exige `0x573d73` e `0x5706bf`, só de empresa em que não é chairman, até o limite `0x43e340` (metade, ou um quarto com a flag, do float livre) e com cotação >= `0x535784`. A compra exige float livre `0x43e580 = ações - Σ posições longas das pessoas ativas` >= n.
- **Lote** (`0x4a6e00(pessoa, índice, compra)`, [V] por snapshots): pessoa de IA: 1000. Humano: `n = (ftol(|posição| * 0.04 + 1000) / 1000) * 1000` (compra usa a posição com sinal); compra: `ok1 = person_can_buy(índice, margem = 0, 1000)`, depois `n -= 1000` e `person_can_buy(índice, margem = !ok1, n)` enquanto `n > 1000` e recusado; venda: `person_can_sell(índice, curto = (posição < 1000), 0, n)` pelo mesmo laço; resultado `max(1000, ((n + 500) / 1000) * 1000)`.

### 7.3 Margem e chamadas de margem (`0x4a7a40`, chamada de `0x46bb10` após cada negócio e no passo mensal) [S]
`poder = caixa + Σ posição_c * +0x427e_c * (0,5 se posição longa; 1,5 se vendida)`. Se `poder >= 0` o prazo `+0x544` é zerado. Senão, para o jogador humano: primeiro aviso (LNG 2270 aproximado) e prazo = `date_to_minutes(ano, mes, 28, 23, 59) + 0x9d7f` (28 dias menos 1 min depois do dia 28 às 23:59, ou seja, até 28 do mês seguinte); depois do prazo, ou imediatamente para a IA, o corretor vende (em lotes de 1000, `0x4a73d0`) ou recompra posições vendidas até `poder >= 0`, em duas passadas (a primeira poupando as ações da própria empresa do chairman), com as mensagens LNG 2271/2272. Depois, posições vendidas em empresas sem float livre (`0x43e340 < 0`) são cobertas à força (LNG 2273).

## 8. Fim de ano da empresa (`0x43d800`, de `sim_year_end`) [S]
1. Arquiva as 14 métricas, as categorias e as contas (desloca o arquivo, 10 anos), zera as categorias do ano, `+0x38`, `+0x192/+0x196/+0x19a`.
2. **Overhead** `+0x138` (detalhado e corrigido em 16.1: divisão inteira por `100000`; a "média" é `(0 + R_Y + R_{Y-1}) / 3`, porque o ano zerado conta no divisor): `g = sqrt(max(1, r/100000))` comprimido (acima de 10, 15, 20 cada excesso pela metade); `+0x138 = max(1000, ftol(r*g*0.01))`.
3. Títulos com `vencimento == ano+1` são quitados (4.4).
4. **Desdobramento**: cotação com commit; se `> 100`: `n = ftol(+0x427e * 0.02)`; ações, carteiras e `+0x34` `*n`; `+0x30`, `+0x38`, `+0x3c` e as colunas de preço, patrimônio, receita, lucro, dividendo e `+0x38` do arquivo `/n`; depois **`+0x4250 = 0` e uma segunda cotação com commit** (`0x43b410(0, 1)`, já com as ações novas: refaz o cache `+0x4254`, o preço sem dividendo `+0x427a` e `+0x427e` antes do arquivo de patrimônio das pessoas, passo 10 de `sim_year_end`; provado no Example 30 dias, empresa 5, desdobramento 2 por 1 em 1876); mensagem LNG 169 ("split %2 for 1"). `0x43b410` devolve 0,0 (`0x53500c`) sem tocar no cache nem em `+0x427e` quando `+0x16e == 0` (empresa inexistente; a empresa 1 de #03 no início do cenário).
5. A avaliação anual da diretoria `0x43c760` **não** é chamada daqui: `sim_year_end` a roda antes de `company_year_end` (passo 4 de `world-tick.md` 3.4, seção 13).

### 8.1 Avaliação anual da diretoria (`0x43c760`) [V]
Ver a seção 13 (desempenho e salário do chairman, bônus, posição nos territórios, multiplicador de receita, candidatos a gerente, aviso e demissão, liquidação) e a seção 16 (sobrecarga, métricas arquivadas, ano 9001).

## 9. Falência (`bankruptcy` `0x43bc30`, comando `0x13`) [V: contas, ações, títulos, cache; S: restrições]
```
cada título: principal = principal/2 (trunca);  H = (Σ principais originais)/2
conta6 += H;  caixa = (caixa - H) + H (sem efeito, salvo arredondamento);  conta7 -= H      ; metade da dívida some e vira patrimônio
x = caixa;  se caixa <= 0: x = caixa*0.5            ; caixa positivo é confiscado inteiro; caixa negativo é cancelado pela metade
caixa -= x;  conta7 += x
ações = ftol(ações * 1.5 * 0.001) * 1000          ; +50% (as constantes são 1,5 e 0,001, nesta ordem) (versão de rede >= 0x2332, sempre em jogo local)
0x43b410(0, 0) (só recotação: o carimbo/cache do preço fundamental é recalculado com o ano de falência ANTIGO)
+0x158 = ano atual                                 ; penalidades: preço (10 anos, 3.1) e rating (15 anos, 4.2)
```
"Ações novas" não vão para ninguém. Restrições (LNG 297, 3412: "não pode declarar de novo", "perdeu dinheiro 2 anos seguidos") ficam na interface e não foram localizadas; a IA só declara com `0x43bc30` sob condições de 9.1.

### 9.1 IA (`0x45fbf0`, por empresa de IA) [S, resumo]
Políticas (`0x5704ac+2`: 0 dividendos altos e recompra agressiva, 1 moderados, 2 nenhum; LNG 3404..3409) e permissões (`0x5706ac..b1`): a IA recompra ações e quita títulos de maior taxa quando tem caixa (> 600 000 ou 800 000*limite), emite títulos de 500 000/30 anos quando o caixa fica abaixo de -250 000 e simula 1..N emissões escolhendo o melhor ganho líquido (-8 000 por título), declara falência se caixa < -300 000, 3+ anos de prejuízo (> -20 000) e a última falência há > 4 anos, e uma vez por ano (dezembro) redefine o dividendo a partir da média ponderada dos lucros (3,2,1) com teto `0x43edf0`. Os valores e limiares exatos estão em "Em aberto".

## 10. Fusão e presidência [V]
- **Fusão** (comandos `0x22`/`0x23`, propostas em `0x574ca0`, `0x48b4f0`/`0x4015a0` = `0x48b5d0`), **eleição de chairman** (comandos `0x24`/`0x25`, propostas em `0x574ca4`, `0x42ed10`/`0x42ee00`) e **troca de chairman** (`0x438900`): fórmulas e constantes exatas na seção 15 (testadas em `company.rs`). Resumo: o proponente oferece um preço por ação (u16); cada acionista de IA vota por sorteio com a probabilidade de 15.1; o float livre vota em bloco pela fração `C`; passa com mais de metade das ações do alvo; falha grava `+0x14c = game_time_minutes` (um ano de espera, LNG 388).
- Compra de indústria (comando `0x2d`, `0x43eb60`): preço `0x40bae0`, conta 5 (indústrias compradas) `+= preço`; a venda devolve metade (a outra metade vai à categoria 3).

## 11. Estado econômico
A variável (`0x570717`, 4 bytes, 0..4) e o passo semestral estão em `cargo.md` seção 9. O que o passo faz nas finanças: `prime_rate = 0,09 - estado*0,01` (4.1), recotação de todas as empresas, e os multiplicadores `0x5504d0` (preço, 3.1) e `0x43b650` (rating). Mudanças de estado são aplicadas por `0x469fb0` (também usada por cenários/eventos).

## 12. Divergências do manual (o código vale)
- **Pesos do preço**: o manual (40/30/20/10 e 10x/1x/1x/30x) confere, mas o código acrescenta: ajuste +25%/-20% do melhor/pior critério, pesos por idade da empresa (só valor contábil no primeiro ano), teto do critério de dividendos em 1,13x o maior dos outros, multiplicador do estado econômico por valor de tabela (0,8 a 1,2; o manual diz +/-10% por nível, o código usa +/-10% e +/-20%), piso de 1,00 e penalidade pós-falência.
- **Modificador de negociação**: o manual diz piso 30% e sem teto; o código tem piso 0,3 e teto 6,0 com compressão por faixas e depende do número de ações e da "liquidez" (100).
- **Corretagem**: o manual não menciona; o código cobra 3% em compras e vendas de ações de jogadores e na recompra da empresa, e 5% de taxa de subscrição na emissão.
- **Dividendos**: "trimestrais" confere (meses 3, 6, 9, 12), mas são pagos no passo mensal e o teto do conselho é 35% do preço de referência, 1,25x lucro por ação e o caixa por ação (6.1).
- **Rating**: o manual dá as faixas da razão dívida/ativos (tabela 5-5); o código usa funções lineares por partes de duas variáveis (cobertura de juros e dívida/ativos), a média ponderada de 4 anos, multiplicador econômico, penalidade por patrimônio < 1 milhão, idade e falência (15 anos), e o nível mínimo para bonds é "B" (5).
- **Bonds**: a taxa é prime + 1% por nível abaixo de AAA (confere), mas o prime é inteiro (5..9%) e o novo principal entra no cálculo do rating. A multa de quitação é 2%; a taxa de emissão é 2%; o vencimento é pago automaticamente no fim do ano anterior ao ano de vencimento; máximo de 20 títulos.
- **Falência**: "metade da dívida some" confere; o código também cancela metade do caixa negativo, confisca o caixa positivo, emite +50% de ações (arredondado a milhares) e mantém o rating baixo por 15 anos (o manual diz "dez").
- **Juros de caixa**: o manual diz "cerca de 50% do prime"; o código paga `prime - 4%`.

## 13. Revisão anual da empresa (`company_annual_review` `0x43c760`) [V]

Chamada por `sim_year_end` uma vez por empresa (passo 4 de `world-tick.md` 3.4), em 28/dez 23:59 e **antes** de `company_year_end` (`0x43d800`): as categorias do ano ainda estão cheias e o arquivo `k = 0` ainda é o ano anterior. Não faz nada se `+0x16e == 0` (empresa inexistente) ou `0x5748f8 != 0`. As entradas vivas vêm de `0x43ad60`: preço `P` = métrica 0 e lucro por ação `E` = métrica 3 (seção 1). Teste `annual_review_matches_original` (`tests/company.rs`, 8000 casos, igualdade exata de todos os campos escritos, das duas pessoas e do estado do RNG).

Notação: `ano` = `game_date & 0xffff`, `idade = ano - fundação (+0x148)`, `ftol` trunca em direção a zero, `/` inteiro é a divisão com sinal do C. Tudo em f64 de 53 bits; os pontos onde o original guarda em f32 estão marcados.

### 13.1 Desempenho do chairman (`+0x46a`, `+0x52b` da pessoa)
```
ant = (fundação == ano) ? f32(+0x3c) : arquivo[1].métrica0 ;  se ant <= 0.01: ant = 0.01
g1  = f32(P/ant - 1)  ; g1 > 1: g1 = sqrt(g1) ; g1 < -1: g1 = -sqrt(-g1) ; g1 = clamp(g1, -2, 2)   [f32]
a = E ;  b = (fundação == ano) ? 1.0 : arquivo[1].métrica3 ;  (a == 0: a = 0.01 ; b == 0: b = 0.01)
g2 = a>0,b>0 : q = a/(b+1) - 1          a<0,b<0 : q = b/(a-1) - 1          a>0,b<0 : q = a/(1-b) - 1
     a<0,b>0 : -1   (sem compressão)     ;  q > 1: sqrt(q) ; q < -1: -sqrt(-q) ; clamp(q, -2, 2)
h2 = 0.5 * g2
mix = (g1 >= h2) ? 6*g1 + 4*h2 : 4*g1 + 6*h2                      ; o maior dos dois pesa 6
v = (mix * 0.05) * 100 ;  perf = (v < 0 ? -(sqrt(-v)*7.07) : v > 0 ? sqrt(v)*7.07 : 0) * 1.8   ; |perf| <= 113.8
perf = f32(perf) ;  se 0.5 <= P < 10 e perf > 0: perf = f32(perf * P * 0.1)                   ; ações baratas rendem menos
w = idade < 3 ? idade*0.15*0.45 + 0.225 : 0.45                    ; 0.225, 0.2925, 0.36, depois 0.45
+0x46a = ftol((1 - w)*+0x46a + w*perf)                            ; desempenho do chairman na empresa (|valor| <= 114)
se há chairman: pessoa.+0x52b = ftol(pessoa.+0x52b*0.55 + perf*0.45)   ; reputação pessoal (a troca de chairman a limita a >= -99, 15.4)
```
Constantes: `0x535470` 0,05; `0x5351a8` 100; `0x5356f0` 7,07; `0x5356e0` 1,8; `0x5356d0` 0,15; `0x535108` 0,45; `0x5356c0` 0,225; `0x535118` 0,55.

### 13.2 Salário do chairman (`+0x134`)
```
alvo = ftol((sqrt(Σ cat[0..4]) + 200) * (3*(+0x46a + 40)) * 0.1)     ; receita do ano (cat 0..4) e a reputação nova
modelo financeiro = 0x574872 & 0x38:  0 (básico): s = ftol(+0x134 * 1.5) ; 8 (avançado): s = ftol(+0x134 * 1.25) ; senão s = +0x134
+0x134 = max(1000, ((ftol(0.65*s + 0.35*alvo) + 500) / 1000) * 1000)   ; arredonda ao milhar
```
O fator 1,5 / 1,25 antes da mistura muda o ponto de equilíbrio do salário: converge para 14 x `alvo` no modelo básico, 1,87 x `alvo` no avançado e 1 x `alvo` no expert (a cada ano `s <- 0,65*k*s + 0,35*alvo`). Salário inicial 10 000 (`0x438a10`, `0x438900`).

### 13.3 Bônus (`+0x474`, categoria 11)
```
se ano - (i16 +0x46e) > 1 e há chairman:                ; no máximo um bônus a cada 2 anos
    r = rng_range(20, 110)                              ; o único consumo de RNG condicional
    se r < +0x46a:                                      ; só com reputação > 20
        b = max(5000, (+0x46a * +0x134) / 100 / 1000 + 1000)       ; divisões inteiras em sequência
        +0x46e = ano ; pessoa.caixa += b ; lança -b na categoria 11 (cópia de vida inclusa) ; caixa -= b ; patrimônio += b
        +0x474 = b
    senão +0x474 = 0
senão +0x474 = 0
```
O bônus passa pelo razão (zera o carimbo do preço, `+0x4250`) e entra no lucro das contas seguintes da mesma chamada.

### 13.4 Posição nos territórios (`+0x47a + t*0x1ed` i16, `+0x4229`)
Base de referência `base = 5000`; se `0x573e08 != 0` (velocidade de referência u8, `world-tick.md` 3.4 passo 2): `vel = métrica 11 (+0x19a / max(+0x196,1), inteiro)`, `base = ftol((min(vel/ref, 2.0) - 1)*2000 + 5000)` (3000..7000).

Contadores das estações da empresa (`+0x39 == id`, lista `0x574c60`, `+0x4ad` = território i32 0..31) acumulados em f32 por território `t`: `B[t] += u16 +0x4b5` (lotes que as indústrias geraram para a estação no ano; `industry_spawn_lot` `0x40a770` o incrementa) e `A[t] += u16 +0x4b9 + u16 +0x4b7` (lotes recebidos e despachados com entrega confirmada; `0x4bf760` incrementa `+0x4b7` na estação de origem e `+0x4b9` na de destino). `0x4beae0` zera os três (e as receitas do ano `+0x59`/`+0x5d`, 8 bytes seguidos) no `industry_year_reset` (`world-tick.md` 3.4 passo 15), depois da revisão. Para `t = 0..31`, em ordem: `TA = ftol(TA + A[t])`, `TB = ftol(TB + B[t])` e, se `B[t] > 3`: `s = ftol((A[t]/B[t] - 1)*4000 + base)`, `pos[t] = ftol(0.3*s + 0.7*pos[t])`.
```
d = max(TB, 1) ;  rnd = rng_range(-200, 200)                             ; 1 sorteio
g = rnd + ftol((TA/d - 1)*4000 + base)
tmp = ftol(0.7*+0x4229 + 0.3*g)
para t = 0..31:  pos[t] = clamp(i16(pos[t] + rng_range(-200, 200)), -10000, 10000)     ; 32 sorteios, soma em 16 bits
+0x4229 = clamp(tmp, 0, 10000)
```
Valor inicial `+0x4229 = 5000` e `pos[t] = 5000` (ou `2*territ.+0x4e - 5000`; `0x43ddb0`, seção 17). O uso é a penalidade de quebra/acidente de trens (`trains.md` 5.3). **RNG por empresa: 33 sorteios fixos + 1 do bônus + os dos candidatos (13.6)**; os "68 sorteios em 2 de 5 empresas" de `world-tick.md` eram 2 x 34 (sem anos ímpares).

### 13.5 Multiplicador de receita (`+0x4248`, f32; usado em `cargo.md` 4 e 4.4)
```
R = Σ cat[0..4] ;  L = Σ cat[0..12]                      ; do ano corrente, já com o bônus de 13.3 (L inclui -b)
idade > 0: R += 0.7 * Σ arquivo[0].cat[0..4] ;  L += 0.7 * Σ arquivo[0].cat[0..12]
idade > 1: R += 0.4 * Σ arquivo[1].cat[0..4] ;  L += 0.4 * Σ arquivo[1].cat[0..12]
R < 1: R = 1
m = 1 - (L/R - 0.2) / 3 ;  m_f32 = clamp(m, 0.8, 1.2)      ; teto/piso gravados como 0x3f4ccccd / 0x3f99999a
idade < 5: m_f32 = f32((idade * m_f32 + (5 - idade)) * 0.2)     ; mistura com 1.0 nos primeiros 5 anos
chairman humano: m_f32 = f32((m_f32 + 1) * 0.5)
```
É um elástico: quanto maior a margem de lucro (L/R), menor a receita por entrega no ano seguinte (0,8 com margem de lucro de 80%, 1,2 com prejuízo de 40% da receita); com chairman humano o efeito é metade.

### 13.6 Candidatos a gerente (anos ímpares, `ano & 1`)
`+0x140` e `+0x144` são regerados: os dois antigos recebem `uso[c]++` (para não saírem de novo), `+0x140 = manager_pick_candidate(-1)`, depois `+0x144 = manager_pick_candidate(-1)` (já com o novo `+0x140`), e os contadores voltam. Os sorteios vêm da seção 14.1 (2 ou mais cada).

### 13.7 Aviso e demissão do chairman [V]
Só se `0x573d7a == 0` (não é editor) **e** a restrição `0x5706ba` ("Being fired as chairman", LNG 1284, tabela 13.9) está ligada; a demissão ainda exige `0x573d75` (modelo financeiro >= avançado) e chairman presente. `S` = ações (`+0x2c`), `held` = soma das posições longas (> 0) das pessoas **ativas** (`pessoa.+4 != 0`) na empresa (posição `idx` da empresa no conjunto `0x570729/0x57072d`).
```
se S - held > S/2:                                          ; mais de 50% de ações fora das pessoas
    n = 0 ; t = -15
    repita: y = ano - n ; sai se y < fundação ou y < (i16 +0x470)        ; só anos sob o chairman atual
            R = (n == 0) ? +0x46a : arquivo[n].métrica12 ; sai se R >= t   ; reputação abaixo de -15, -10, -5
            E' = (n == 0) ? E : arquivo[n].métrica3 ;       sai se E' >= 1.0   ; lucro por ação < $1
            n += 1 ; t += 5 ; sai se t >= 0                                    ; no máximo 3 anos
    se n >= 2:
        pct = min(100, f32(-(+0x46a) * 1.75))
        votos = ftol(pct * 0.01 * (S - held))              ; o público vota "sim" na fração pct
        se pct > 50:  votos += Σ posição longa de toda pessoa IA (+0x122 == 0) que NÃO é o chairman da empresa
        se votos > S * 0.5:
            se (i16 +0x472) < ano - 1:  aviso: se o chairman é a pessoa local (0x566ca4) e 0x570237 == 0: caixa LNG 2277 ; +0x472 = ano
            senão: demissão: LNG 2275/2276 e company_set_chairman(0) = 0x438900(empresa, 0)
```
Ou seja, **um aviso num ano, demissão no seguinte** (mesmo para chairmans de IA, sem mensagem) se a empresa seguir ruim. `0x438900` está na seção 15.4.

### 13.8 Liquidação pelos credores [V]
Mesmo portão de 13.7 (editor desligado, restrição `0x5706ba`), mas sem o requisito `0x573d75`. Conta anos consecutivos `n = 0, 1, 2` (limite de preço `thr = 5, 7, 9`):
```
repita: y = ano - n ; sai se y < fundação
        preço(y) = (n == 0) ? P : arquivo[n].métrica0 ;  sai se preço >= thr
        patrimônio(y) = (n == 0) ? -conta7 : Σ conta[0..5] + conta6 do arquivo de contas k = n-1 ;  sai se >= 0
        lucro(y) = Σ cat[0..12] (n == 0: do ano; senão arquivo k = n-1) ;  sai se >= -10000
        caixa(y) = conta0 (n == 0) ou arquivo k = n-1 ;  sai se >= 0
        n += 1 ; thr += 2 ; sai se thr >= 11
n == 2: se a empresa é a local (0x574414) e 0x570237 == 0: caixa de aviso LNG 2278 (um ano para melhorar)
n == 3: 0x438170(empresa, f) com f = 1 se (versão de rede >= 0x232d, no máximo 1 jogador e byte 0x574bc8+0x240 != 0) senão 0 ; LNG 2279/2280
```
Três anos seguidos com preço < $5/7/9, patrimônio negativo, prejuízo > $10 000 e caixa negativo. `0x438170` (seção 15.5) apaga os trens, solta indústrias e estações (as estações ficam sem dono, LNG 2280), zera a coluna da empresa nas carteiras, libera o gerente e põe `+0x16e = 0`.

### 13.9 Restrições de cenário e modelo financeiro
Os bytes `0x5706a9 + i` (`i = 0..21`) são as "General Restrictions" do editor (LNG `1267 + i`, tabela `INTERFACE_ALLOWED_DESC`); 1 = permitido:

| `i` | Byte | Restrição | | `i` | Byte | Restrição |
|---|---|---|---|---|---|---|
| 0 | `a9` | Stock selling | | 11 | `b4` | Retiring a train |
| 1 | `aa` | Stock buying | | 12 | `b5` | Buying a train |
| 2 | `ab` | Accessing the stock market screen | | 13 | `b6` | Laying track |
| 3 | `ac` | Declaring bankruptcy | | 14 | `b7` | Building a station |
| 4 | `ad` | Issuing stock | | 15 | `b8` | Building a station support building |
| 5 | `ae` | Buying back stock | | 16 | `b9` | Buying an industrial building |
| 6 | `af` | Issuing bonds | | 17 | `ba` | **Being fired as chairman** |
| 7 | `b0` | Repaying bonds | | 18 | `bb` | Assuming chairmanship of another company |
| 8 | `b1` | Changing the dividend rate | | 19 | `bc` | Resigning as chairman |
| 9 | `b2` | Setting the throttle | | 20 | `bd` | Merging companies |
| 10 | `b3` | Replacing a train | | 21 | `be` | Starting a company |

Os nomes de `ai.md` 1 (`b5/b6/b7`, `b9`, `ac`, `ae`, `af`, `b0`, `b1`, `be`) conferem; `0x5706ba` é o portão da seção 13.7/13.8 e `0x5706bc` o da mensagem LNG 2863 ("this scenario does not allow you to resign"). O modelo financeiro (`0x574872 & 0x38`: 0 básico, 8 avançado, 0x10 expert; `0x474280`/`0x495700`) liga `0x573d74/75` (>= avançado: comprar ações de outras empresas, demissão, votações) e `0x573d72/73` (expert: margem e venda a descoberto).

### 13.10 O que a revisão não faz
Não há aquisições na revisão: fusões (`0x22`) e disputas de presidência (`0x24`) nascem só de comandos do jogador (as rotinas da IA, `0x4a8800`/`0x4a8af0`/`0x436b30`, não chamam `0x48b4f0` nem `0x42ed10`); a IA só compra e vende ações. As votações estão na seção 15.

## 14. Pessoas, gerentes e candidatos

### 14.1 `0x438450` não gera pessoas: escolhe um gerente (`manager_pick_candidate`) [V]
O nome `person_generate` de `symbols.toml` é impróprio: a função devolve o **índice (0..39) de um gerente** da tabela fixa `0x54fdad`, nunca cria registro de pessoa. `thiscall(empresa, forçado)`, `ret 4`; `forçado = -1` sorteia. Teste `manager_pick_matches_original` (6000 casos: índice, RNG e cursor de rede iguais).
```
y = ano - (mês != 12)                         ; ano de referência (dezembro: o ano que acaba)
R = (fundação <= y) ? Σ cat[0..4] de y (y == ano: categorias do ano; senão arquivo k = 0) : 0      ; as 3 voltas do original usam o MESMO y
B = R * 1.4 * 0.01 * rng_range(25, 180) * 0.01           ; salário-alvo = 0,35% a 2,52% da receita
se B < 8000: B = rng_range(1000, 12000)                    ; sem histórico: alvo aleatório
c = (0x5748f8 != 0) ? (0x565154 = (0x565154 + 1) % 40) : rng_range(0, 39) ;  se forçado != -1: c = forçado
tent = 4000 ; sal_ok = disp_ok = verdadeiro ; lo = 0.8f ; hi = 1.3f
repita:
    rejeita se uso[c] != 0 (0x573e09[c] = empresa que emprega c) ou c == +0x140 ou c == +0x144
             ou (sal_ok e não (lo < salário[c]/B < hi)) ou (disp_ok e disponível[c] == 0)     ; 0x570638[c], da época/mapa
    senão devolve c
    se (tent & 15) == 15: lo *= 0.96 ; hi *= 1.04 ; se tent <= 1000: sal_ok = falso ; se tent <= 500: disp_ok = falso
    próximo: modo rede: cursor = (cursor+1) % 40 ; senão tent > 60: c = rng_range(0, 39) ; senão c = (c+1) % 40
    tent -= 1
```
Consumo de RNG: 2 (ou 1) para `B`, 1 para o `c` inicial (mesmo se forçado, exceto modo rede) e 1 por tentativa recusada enquanto `tent > 60`. Chamadores: `company_init` (3 vezes), `company_set_manager` (reposição) e 13.6.

### 14.2 Tabela de gerentes (`0x54fdad`, 40 registros de 0x11 bytes)
`+0` salário anual i32, `+4` quatro pares `(efeito u8, pct i8)` (efeito 0 = vazio). Nome = LNG `1456 + 2i`, biografia `1457 + 2i` (`MANAGER_NAME`). Salários (US$/ano) pelo índice: 0 Ames Oakes 8000, 1 F. Billings 3000, 2 E. Debs 85000, 3 J. W. Garrett 21000, 4 R. Gerwig 37000, 5 D. Gooch 44000, 6 C. M. Hays 77000, 7 J. Hopkins 41000, 8 T. Judah 29000, 9 C. Mayer 89000, 10 O. Murray 87000, 11 G. Nagelmachers 46000, 12 W. Palmer 39000, 13 G. Stephenson 43000, 14 W. Strong 11000, 15 T. Swann 6000, 16 R. Trevithick 14000, 17 G. Twitchell 4000, 18 W. Van Horne 26000, 19 G. Westinghouse 35000, 20 R. White 12000, 21 D. Willard 75000, 22 A. Robinson 19000, 23 P. Randolph 29000, 24 A. Carnegie 52000, 25 R. Diesel 3000, 26 B. Holladay 26000, 27 B. Masterson 38000, 28 H. Booth 13000, 29 A. Chapelon 36000, 30 T. Crampton 4000, 31 R. Garbe 6000, 32 H. Garratt 14000, 33 H. Giffard 20000, 34 A. MacNab 10000, 35 G. Pullman 94000, 36 W. Schmidt 22000, 37 E. Siemens 6000, 38 R. Stevens 73000, 39 J. Wootten 4000. Os efeitos (`+4`) valem para `ai_manager_value` (`ai.md` 6) e, ao contratar, somam `pct` a `empresa.+0x1ae + 2*efeito` (i16, limite +-127; `0x438710`, que subtrai ao demitir).

### 14.3 Contratar um gerente (`company_set_manager` `0x438780(g, cobrar)`) [S]
Ignora se `uso[g] != 0`. Taxa `f = (((ftol(efeito(0x10, empresa) * salário[g])) / 2) / 1000) * 1000`. Remove os efeitos do gerente atual e libera `uso[atual] = 0`; grava `+0x13c = g`, aplica os efeitos de `g`, `uso[g] = id da empresa`; se `cobrar`: `-f` na categoria 11, no caixa (`conta0`) e `+f` no patrimônio. Depois regera (14.1, com RNG) `+0x140/+0x144` de todas as empresas que apontavam para `g`. O salário mensal é `-salário[g]/12 * efeito(0x10)` (seção 5, item 6).

### 14.4 Tycoons (pessoas) [S]
- **Origem.** Os 41 tycoons vêm da tabela `ai_persona_table` `0x551e88` (`ai.md` 3; registro 0x23, índice 0 = jogador genérico). Nome = LNG `1539 + 2p`, biografia `1540 + 2p` (campos `+0x1f`/`+0x21`, `PLAYER_NAME`); o retrato vem do byte `+1`. No início de uma partida/cenário (`0x473590`) cada pessoa ativa recebe `+0x52f = persona`, nome e biografia copiados do LNG (`_strncpy` 30 e 250 bytes). Uma persona é escolhida por `0x4a7990(dica, passo, incluir0)`: sorteio `rng_range(0, 40)` (ou `(passo + 41 + dica) % 41`), avançando de `passo` em `passo` módulo 41 até uma persona que nenhuma pessoa ativa usa (99 tentativas, depois 0).
- **Registro novo** (`0x4a6b00`, `0x4a6a60` insere na lista `0x574c68`): `+0 id`, `+4 = 1` (ativa), `+0x122 = (id == 1)` (humano), `+0x12a` caixa = **100 000** (`0x40f86a00` no dword alto), ações zeradas, reputação `+0x52b = 0`, `+0x263 = 0` (sem empresa), `+0x544/+0x548/+0x551/+0x555 = 0`, `+0x267 = rng_range(60, 450) * 1440` (minutos até a pessoa de IA poder fundar empresa). Há um sorteio extra `rng_range(0, 40)` que marca como usada (`byte +0 da persona = 1`) a primeira persona livre a partir do sorteado sem atribuí-la à pessoa: sobra de código (consome RNG e esgota o conjunto; com as 41 usadas o laço não termina). Arquivo de patrimônio `+0x133` zerado.
- **Início do mundo** (`0x474280`): cada pessoa recebe `+0x548 = rng_range(-32767, 32767)` (semente do `rng_hash_range` da IA), `+0x551 = 0` e `+0x267 = clamp(ftol((rng_range(-80,80) + 60 + (200 - StartsCompany_persona)*1.5) * 1440), 72000, 576000) + ((4 - modelo_financeiro) - modelo_industrial) * 80` (50 a 400 dias).
- **Ciclo de vida.** Não há envelhecimento, morte nem aposentadoria no executável: uma pessoa só some por `0x4a6a00` (regra de cenário "derrota", exclusão no editor ou ajuste da lista no início da partida), que tira a chairmanship (`0x438900(empresa, 0)`), zera as 36 posições e põe `+4 = 0`. A reputação `+0x52b` muda só em 13.1 (a cada ano, 55/45 com o desempenho) e na troca de chairman (15.4).
- **Dinheiro pessoal** (`+0x12a`, f64): salário do chairman `ftol(+0x134/12)` por mês (seção 5), dividendos trimestrais (seção 6), bônus anual (13.3), compras e vendas de ações (7.2), taxa de 3%, e a fundação de empresa (seção 17). O arquivo de patrimônio (`0x4a7880`, em `sim_year_end`, passo 10) desloca 9 anos de `{caixa, valor das ações = Σ posição * cotação viva}` (16 bytes por ano, `+0x133`).
- **Efeito sobre a IA.** `ai_person_turn` (`0x4a8800`) usa os 7 números da persona (Starts Company etc., `ai.md` 3) e a reputação via 15.1/15.3.

## 15. Votações e troca de chairman [V]

Os dois registros de proposta vivem em `0x574ca0` (fusão: `+4` u16 proponente, `+6` u16 alvo, `+8` u16 preço por ação, `+10 + i` "pessoa i já respondeu", `+0x2a + i` voto) e `0x574ca4` (presidência: `+4` u16 empresa, `+6` u16 desafiante, `+8 + i` respondeu, `+0x28 + i` voto). `i` = posição da pessoa na lista. Os votos humanos chegam por comando (`0x23`, `0x25`) e a avaliação só corre quando **todas** as pessoas responderam (`+10+i != 0` / `+8+i != 0`); os votos de IA são sorteados na própria avaliação (nenhuma resposta pendente, nenhum RNG antes disso). Numa partida em rede de versão >= `0x232d`, uma empresa inativa aborta a votação.

### 15.1 Fusão (`0x48b5d0`, chamada por `0x4015a0`; propostas criadas em `0x48b4f0`) [V]
`0x48b4f0` marca de saída: o chairman do proponente (`pessoa.+0x263 == proponente`) e todo acionista do alvo com posição > 0 votam: o chairman do proponente `sim`, a IA `não` (provisório), humano pendente. A avaliação, com `S` = ações do alvo, `F` = float livre do alvo (`0x43e580` = `S - Σ posições longas das pessoas ativas`):
```
P_n = cotação do alvo (0x43b410(0,0), f32) ; P_f = preço fundamental (0x43aea0)
v   = f32((P_f + P_n) * 0.5) ;  v < 1: v = 1                    ; preço de referência
ef  = efeito(0x22, proponente)                                   ; efeito multiplicativo 0x22 do proponente (1.0 sem efeitos)
A   = f32(((oferta / ef) / v - 1) * 0.7 + 0.1)                   ; oferta = u16 do registro
B   = f32((r5 - 0.1) / 3 + A)                                    ; r5 = métrica 5 do proponente (retorno ponderado de 5 anos, 16.2)
C   = f32(rng_range(-10, 10) * 0.01 + B)                         ; 1 sorteio
para cada pessoa p (em ordem), IA (+0x122 == 0) com posição > 0 no alvo:
    q = f32(C - 0.1) ;  se p é chairman do alvo: q = f32(q - 0.45)
    q = f32(rng_range(-20, 20) * 0.01 + q) ;  sim se q * 100 > rng_range(0, 100)          ; 2 sorteios por IA
sim = Σ posição de quem votou sim (humanos com o voto guardado, IA com o sorteado; só posições > 0)
público = (ftol(F * clamp(C, 0, 1)) / 1000) * 1000                ; o float livre vota na fração C
passa se sim + público > S / 2                                    ; S/2 com sinal truncado
```
Constantes: `0x5350f8` 0,7; `0x535178` 0,1; `0x535020` 1/3; `0x5350b8` 0,01; `0x535108` 0,45; `0x535070` 100,0f; `0x535124` 0,5f.
Efeito: aprova => `0x43c3c0(proponente, alvo, oferta)` e mensagem LNG 24; reprova => `proponente.+0x14c = game_time_minutes` (um ano de espera, LNG 388) e LNG 25. A proposta é apagada (`0x466cc0`) nos dois casos. Teste `merger_vote_matches_original` (6000 casos: votos, chamada de fusão e argumentos, `+0x14c`, RNG).

### 15.2 Execução da fusão (`0x43c3c0(proponente, id_alvo, oferta)`) [S]
Reatribui ao proponente: trilho do mapa (`0x574c24`, dono `+2`), trens (`+0x41`, soma em `+0x27`), estações (`+0x39`, soma em `+0x172`), indústrias (`+0x97`), títulos (até 20, `+0x40/+0x44`) e as 8 contas; soma os contadores de trilho `+0x182/+0x186/+0x18a/+0x18e`. Cada acionista do alvo recebe `posição * oferta` em dinheiro e a posição some; o proponente paga `S * oferta` (caixa e patrimônio) e o alvo é liquidado com `0x438170(alvo, 0)`. Cotação do proponente recalculada (`0x43b410(0,0)`); LNG 168 ("%1 takes over %2!").

### 15.3 Disputa de presidência (`0x42ed10` cria, `0x42ee00` avalia) [V]
`0x42ed10(empresa, desafiante)`: o desafiante vota `sim` em suas ações; outros acionistas IA começam em `não`; humanos pendentes. Avaliação:
```
p0 = f32((0.2 - rep_empresa(+0x46a) * 0.016667) + rep_desafiante(+0x52b) * 0.0125)
p  = f32(rng_range(-10, 10) * 0.01 + p0) ;  p > 1: 1 ; p < 0: 0                       ; 1 sorteio
público = (ftol(F * p) / 1000) * 1000
para cada pessoa IA com posição > 0:  se é o chairman atual da empresa: não
        senão: q = f32(rng_range(-20, 20) * 0.01 + p) ;  sim se q * 100 > rng_range(0, 100)         ; 2 sorteios
passa se sim + público > S / 2
```
Constantes: `0x535350` 0,0125; `0x535360` 1/60 (0,016667); `0x5351c8` 0,2. Passa: `0x43c050(empresa, desafiante)` (LNG 26/27): se o desafiante já preside outra empresa, ela fica sem chairman (`0x438900(outra, 0)`), depois `0x438900(empresa, desafiante)`. Falha: LNG 28 e `desafiante.+0x527 = game_time_minutes` (carimbo da última tentativa fracassada). Apaga a proposta (`0x466cf0`). Teste `chairmanship_vote_matches_original` (6000 casos).

### 15.4 Troca de chairman (`0x438900(empresa, novo)`) [S]
Se `novo != atual`: o chairman antigo (se houver) recebe `+0x52b += 0x4a8100` e mínimo -99, onde `0x4a8100(pessoa)` = `(min(10, max(0, ano - empresa.+0x470)) - 10) * 7` (penalidade de 0 a -70 por mandato curto; 0 sem empresa) e `+0x263 = 0`. Grava `+0x23 = novo`; atitude dos investidores `+0x4276`: `*0.8 + 0.2` (sem humano) ou 0 (humano); `+0x46a = novo.+0x52b` (0 sem chairman); **`+0x134 = 10000`** e `+0x470 = ano`; o novo recebe `+0x263 = empresa`.

### 15.5 Liquidação (`0x438170(empresa, f)`) [S]
Apaga os trens da empresa (`0x4cdd50`), tira o dono das indústrias, solta as estações (`f = 0`: sem dono; `f != 0`: `0x40adb0(prédio, 1)` por estação, efeito não analisado), tira a coluna da empresa das 36 posições de todas as pessoas (deslocando as seguintes), libera o gerente (`uso[+0x13c] = 0`, `+0x13c = -1`), tira o chairman, zera `+0x16e` e retira a empresa do conjunto `0x570729` (`0x46a830`).

## 16. Fim de ano: sobrecarga e métricas arquivadas (complemento da seção 8)

### 16.1 Sobrecarga anual `+0x138` (`0x43d800`, depois de arquivar e zerar as categorias) [V]
O arquivo já foi deslocado quando a sobrecarga é calculada, e a data ainda não avançou, então o ano que acabou está em `arquivo k = 0` mas o laço o enxerga como "ano - 1":
```
acum = 0 ; n = 0
para i = 0..2:  y = ano - i ; se y >= fundação:
        soma = (y == ano) ? Σ cat[0..4] (as categorias do ano acabaram de ser zeradas: 0) : Σ arquivo[k = i-1].cat[0..4]
        acum = ftol(acum + soma) ;  n += 1
media = acum / n                                         ; inteiro com sinal, n >= 1 (a volta i = 0 sempre conta)
q = media / 100000                                       ; inteiro
g = sqrt(max(1.0, q)) ;  g > 10: g = (g - 10)*0.5 + 10 ;  g > 15: g = (g - 15)*0.5 + 15 ;  g > 20: g = (g - 20)*0.5 + 20
+0x138 = max(1000, ftol(media * g * 0.01))
```
Constantes: `0x535714` 10,0f; `0x535710` 15,0f; `0x535388` 20,0f; `0x535124` 0,5f; `0x5350b8` 0,01. Correções à seção 8.2: `r/100000` é divisão **inteira**, e a "média dos últimos 3 anos" é `(0 + R_Y + R_{Y-1}) / 3` para empresas com 2+ anos (o ano corrente zerado conta no divisor e o arquivo k = 0 é o ano que acabou, lido como "ano - 1"; o ano `Y-2` fica de fora); empresa fundada neste ano: só a volta 0 conta e a sobrecarga cai para o piso 1000. A sobrecarga é cobrada por mês (seção 5, item 3): `-(+0x138)/12 * efeito(0xf)`.

### 16.2 As 14 métricas arquivadas (`0x43ad60(i)`, arquivo `+0x49b6 + 0x70*k`, `k = 1..10`)
Gravadas em `k = 1` por `company_year_end` antes de zerar categorias e contadores (o arquivo desloca 9 anos):

| `i` | Valor | Observação |
|---|---|---|
| 0..4 | preço, patrimônio/ação, receita/ação, lucro/ação, dividendo `+0x30` | seção 1 |
| 5 | retorno ponderado de 5 anos (`0x43abc0`) | abaixo |
| 6 | rating de crédito (`0x43b650(0)`) | seção 4.2 |
| 7 | dividendos pagos no ano por ação (`+0x38`) | zerado no fim do ano |
| 8 | cargas transportadas no ano (`+0x192`) | `scenarios.md` `CompanyLoadsHauledThisYear` |
| 9 | amostras de velocidade no ano (`+0x196`) | contagem do `+0x19a` |
| 10 | `(+0x186 + 2*+0x18a) * 10` | trilho simples + 2 x duplo, em células x 10 |
| 11 | velocidade média do ano: `+0x19a / max(+0x196, 1)` (inteiro) | alimenta `0x573e08` e a posição nos territórios (13.4) |
| 12 | `+0x46a`, reputação do chairman | alimenta o aviso de demissão (13.7) |
| 13 | `+0x134`, salário do chairman | |
| outro | -1.0 | |
`+0x192/+0x196/+0x19a` e os contadores anuais por território (`+0x47e`, `+0x51a[44]`, `+0x60e[44]`) são zerados no fim do ano; `+0x182..+0x18e` (trilho) e `+0x19e` (cargas, vida) não.

`0x43abc0` (retorno ponderado de 5 anos), `f_0 = mês/12`, `f_i = 1` para `i >= 1` (0 se a empresa não existe):
```
para i = 0..4 com ano - i >= fundação:
    p0 = (ano - i - 1 < fundação) ? f32(+0x3c) : arquivo[i+1].métrica0          ; preço no início do ano i
    pe = (i == 0) ? P : arquivo[i].métrica0                                       ; preço no fim
    d  = (i == 0) ? +0x38 : arquivo[i].métrica7                                   ; dividendos pagos
    ret_i = p0 > 0 ? (d + pe - p0) / p0 : 1.0 ;  w_i = (5 - i) * f_i
r5 = Σ (5 - i)*ret_i / Σ w_i
```
O retorno do ano corrente é anualizado dividindo pela fração do ano (`0x535540` = 1/12). É a métrica 5 que entra na votação de fusão (15.1) e no "retorno ponderado de 5 anos mínimo" da emissão de ações (3.4).

### 16.3 O "ano 9001" (`0x2329`)
Resolvido: `+0x46fe` (f64[13]) é o **acumulador de vida inteira** das 13 categorias (`company_ledger_credit` soma em `cat[i]` e em `+0x46fe`; só `company_init` o zera; `company_year_end` não o desloca). As rotinas de arquivo (`0x43a0a0`, `0x43a7d0`, `0x43a6d0`, `0x445be0`...) tratam o "ano" `0x2329` (9001) como o pseudo-ano que seleciona esse bloco (`0x43a6d0(emp, i)` lê só ele; o painel de relatórios mostra LNG 2368 "Lifetime" no lugar do ano). Em `0x43c760`, `0x43d800`, `0x438450`, `0x43b650` e `0x43edf0` os testes `y == 0x2329` (com `y = ano - i`) nunca disparam em jogo (anos reais); as condições de `scenarios.md` `CompanyRevenueLifetime`/`CompanyNetIncomeLifetime` usam `0x43a6d0`.

## 17. Fundação de empresa e capital inicial

### 17.1 Pré-condições do jogador (`0x4bd950`, botão "Start Company") [S]
Recusas, na ordem: restrição `0x5706bc == 0` (resignar proibido) e o jogador já é chairman (LNG 2863); 36 ou mais empresas no conjunto (LNG 144); a menos de **672 dias** (`0xec400` minutos = 24 meses de 28 dias, "two years") da última fundação do jogador (`pessoa.+0x551`, ignorado com `0x5748f8 != 0`; LNG 2708); poder de compra `W < 100 000` (`W` = `caixa + min(valor_das_ações, 0)`, 17.2; LNG 145, ou LNG 2317 no modelo expert). `pessoa.+0x555` conta as fundações.

### 17.2 Os controles "Player Investment" e "Outside Investment" (`0x4bc990`, `0x4bc7d0`, rótulos LNG 541/542) [V]
Cada controle é um slider de `n` posições; o valor é `mínimo + posição * (máximo - mínimo) / (n - 1)` (`ftol`). Posição inicial `(n - 1)/2`. Teste `founding_sliders_match_original` (3000 casos por controle: limites, `n` e posições enviadas aos widgets).
**Investimento próprio `P`** (`0x4bc990`). `W = caixa (+0x12a) + s` se `s <= 0`, senão `W = caixa`, com `s = 0x4a6d80(pessoa)` = `Σ posição * cotação (+0x427e)`, contando `0,5x` de cada posição positiva e `1,5x` de cada negativa (venda a descoberto) com sinal (`s` negativo reduz o poder de compra; ações longas **não** o aumentam); `W = ftol(min(W, 1e8))`.
```
W < 1 000 000:  passo 10 000, mínimo 100 000       ;  W < 10 000 000:  passo 100 000, mínimo 100 000
senão: passo = W > 99 999 999 ? 10 000 000 : 1 000 000 ; mínimo = passo
máximo = W - W % passo ;  n = max(1, (máximo - mínimo)/passo + 1)
```
**Investimento externo `O`** (`0x4bc7d0`), dado `P`. `m` = confiança dos investidores `0x4a8150(pessoa)` = IA: 10,0; humano: `clamp((reputação +0x52b + 100 + (min(10, max(0, ano - empresa.+0x470)) - 10)*7) * 0.1, 0.5, 15)` (a penalidade só com `+0x263 != 0`; teste `credit_multiplier_matches_original`).
```
C = (m*P >= 1e9) ? 1e9 : ftol(m*P) ;  C = max(C, 200 000 - P)
C <= 50 000: C = 50 000, passo 1000  |  C >= 4 000 000: C = 4 000 000, passo 100 000  |  C < 100 000: 1000  |  C < 1 000 000: 10 000  |  senão 100 000
mínimo = arredonda_para_cima_ao_passo(max(0, min(C/2, 500 000 - P))) ;  máximo = C - C % passo ; máximo = max(máximo, mínimo + passo)
n = max(3, (máximo - mínimo)/passo + 1)
```
Ao recalcular depois de mexer em `P`, a posição de `O` é mantida: `clamp((O_antigo - mínimo)/((máximo - mínimo)/(n-1)), 0, n-1)` (ou o meio, se `O = -1` ou reset). Quem põe mais dinheiro próprio e tem boa reputação consegue mais capital externo (`m*P`), como diz o guia.

### 17.3 O comando `0x26` (`0x4bdcd0`) [S]
Dados: pessoa, logotipo/cor (`+0x2c`), nome (31 caracteres), capital total `K = P + O` (`+0x2d`), investimento próprio `P` (`+0x31`). Se a pessoa já presidia uma empresa, `0x438900(antiga, 0)`; `+0x555++`; cria a empresa (`0x4383f0` -> `0x438a10`, 17.4) com `K`; a pessoa compra `n_p = min(ações, max(0, (ftol(P / preço_inicial) + 500)/1000 * 1000))` ações, paga `ftol(n_p * preço_inicial)` (grava de volta em `+0x31`) e `n_p` vai à sua posição; evento de cenário 5 ("empresa criada"). O comando `0x27` só renomeia/recolore a empresa.

### 17.4 Registro novo (`company_init` `0x438a10(id, chairman, cor, nome, capital)`) [V]
```
K = (capital == -1) ? 500 000 : capital
preço0 = f32(K / 10000)  (inteiro) ;  enquanto preço0 >= 100: preço0 *= 0.5            ; 5 a 99,99 (ou 50 para K = 1e6)
ações  = ftol(K / preço0 * 0.001f) * 1000                                              ; múltiplo de 1000
caixa  = ftol(preço0) * ações = conta0 += ; conta7 -=        ; "ações vendidas ao preço inteiro": arredonda K para baixo
+0x2c = ações ; +0x3c = preço0 ; +0x427e = +0x4282 = preço0 (depois recotado com commit)
```
Exemplos: `K = 500 000` -> preço 50, 10 000 ações, caixa 500 000; `1 000 000` -> 50, 20 000, 1 000 000; `1 500 000` -> 75, 20 000, 1 500 000; `4 000 000` -> 50, 80 000, 4 000 000; `123 456` -> 12, 10 000, 120 000 (o resto de K se perde). Demais valores iniciais: dividendo `+0x30 = 0,5` por ação/ano; `+0x138 = 6000` (sobrecarga); `+0x134 = 10 000` (salário do chairman); `+0x148 = ano` (fundação) e `+0x470 = ano`; `+0x425c = game_time_minutes`; `+0x17e = game_time_minutes - 1`; atitude dos investidores `+0x4276` = 1,0 (0 se chairman humano); multiplicador de receita `+0x4248 = 1.0`; `+0x14c`, `+0x158`, `+0x46a`, `+0x46e` = 0; `+0x15c = 15` (papel não identificado); `+0x160/+0x164` = data do ano 1800 (sem espera para emitir ações); `+0x13c` = gerente (1º sorteio de 14.1 e `0x438780` sem taxa), `+0x140/+0x144` = 2 candidatos (2º e 3º sorteios); conjunto de empresas `0x46a810(id)` (acrescenta o id a `0x57072d`); territórios `0x43ddb0`: posição `+0x47a` = 5000 se `territorio.+0x4e == 0`, senão `2*(+0x4e) - 5000`, direitos `+0x479 |= 0xf` (ou bit a bit de `territorio.+0x26..+0x29`), `+0x4229 = 5000`. **Cor** (`+0x2b`): com `cor = -1`, `rng_range(0, livres - 1)` e a `(r+1)`-ésima cor livre em ordem crescente de 0..40, onde "usada" é toda empresa ativa com `+0x478 == 0` (a própria empresa nova conta: ativa, cor 0, então a cor 0 nunca sai desse sorteio). **Nome** (se `nome == NULL`): `TEMP_COMPANY_NAMES` (LNG, ids u16 em `0x5539c0`), índice `0x5707b9 % 40`, com `0x5707b9++`. Teste `company_init_matches_original` (4000 casos: ações, preço, caixa, equity, campos iniciais, cor, gerente e candidatos, conjunto, contador de nomes, RNG).

### 17.5 Fundação por pessoa de IA (`ai_person_turn` `0x4a8800`) [S]
As condições e a fórmula do capital estão em `ai.md` 9.1 (conferidas no desmontador: `v = ftol(caixa)`, acima de 3 000 000 só metade do excedente conta; `P = (v/1000)*1000`; `O = min(max(1 500 000 - P, 0), 9*P)`; `K = P + (O/1000)*1000`; ações da pessoa = `min(ações, ftol((caixa + 8000)/preço0 * 0.001) * 1000)` pagas a `preço0`). O jogador não passa por essa fórmula: o limite dele são os sliders (17.2). Tanto `K` da IA quanto o `K` do jogador entram em `company_init` (17.4).

## Em aberto
- Motivo de `max(NI, Rv)` em `0x43edf0` (a conta está provada, 6.1).
- (Resolvido, seção 18) Limites da interface: emissão, recompra, títulos, dividendos, bancarrota, fusão, presidência e lote do jogador.
- Papel de `0x5706d9` (=2), `0x5706d5` (5,0 = rating mínimo para bonds, confirmado) e do efeito `0x21` (liquidez, LNG 1651); nome do efeito `0x22` (preço de fusão).
- Efeito de `f != 0` em `0x438170` (`0x40adb0`), papel de `+0x15c` (=15) na empresa nova e execução da fusão `0x43c3c0` (15.2) e de `0x43ddb0` (17.4): lidas, sem teste.
- Fórmula do poder de compra `0x4a6d80` e a precondição `0x4bd950` (17.1) foram lidas; os sliders (17.2) estão testados.
- O sorteio de persona sem atribuição em `0x4a6b00` (14.4) é leitura de código, sem teste.
- Resolvidos nesta revisão: constantes da votação de fusão e da revisão anual, critério da liquidação, capital inicial, ano 9001 (lifetime), `0x43abc0`, arquivo de patrimônio da pessoa, políticas de IA (`ai.md` 10).

## Verificação (`crates/rt2-oracle/tests/finance.rs`)

Id de empresa 0 em todos os casos (efeitos só globais), `CW = 0x27F`, empresa montada em memória emulada (campos da seção 1).

| Teste | Função | Casos | Resultado |
|---|---|---|---|
| `prime_rate_matches_original` | `0x46bac0` (4.1) | 600 (estado, efeito -4..6, f32 arbitrário) | bit a bit |
| `trade_price_matches_original` | `0x43b410` (3.2), com o preço fundamental servido do cache | 4000 (n = 0, +-milhares, até +-todas as ações; commit sim/não; liquidez 40..250) | preço inteiro, `+0x34` e `+0x427e` iguais; tolerância de 1 dólar em no máximo 4 casos pelo `pow` da CRT (Rust usa `powf`) |
| `fundamental_price_matches_original` | `0x43aea0` (3.1) e, por dentro, `0x43a440`, `0x43ad60` (métricas 1..4) | 3000 (idade 0..14, dia 28/dez em 1/7 dos casos, histórico de 10 anos) | bit a bit, inclusive `+0x427a` e o cache `+0x4254/+0x4250` |
| `credit_rating_and_bond_rate_match_original` | `0x43b650` (4.2) e `0x4392e0` (4.3) | 4000 (0..6 x 500 000 de dívida extra, 0..5 títulos) | bit a bit; a escala de rating exercitada tem >= 7 notas distintas |
| `ledger_entries_match_original` | `0x439970`, `0x4398d0` (seção 1) | 1500 (categoria/conta aleatória, valores inteiros e fracionários) | bit a bit: categorias, `+0x46fe`, contas e carimbo |
| `annualized_totals_match_original` | `0x43a440` (seção 2) | 3000 (i = 0..15, 12/28 em 1/7) | bit a bit |
| `max_dividend_matches_original` | `0x43edf0` (6.1) | 4000 (idade 0..14, dezembro em 1/4, caixa positivo em 70%) | bit a bit; mais de 500 tetos positivos |
| `bankruptcy_matches_original` | `0x43bc30` (seção 9) | 1500 (caixa +/-, principais ímpares) | bit a bit: contas, ações, títulos, ano, cache do preço |

### Verificação da revisão anual e das pessoas (`crates/rt2-oracle/tests/company.rs`)
`CW = 0x27F`; as funções que chamam UI ou outros subsistemas rodam com os callees trocados por stubs de código de máquina que contam chamadas e argumentos. Referências em Rust escritas no próprio teste (só o `GameRng` vem de `rt2-sim`).

| Teste | Função | Casos | Resultado |
|---|---|---|---|
| `manager_pick_matches_original` | `0x438450` (14.1) | 6000 (mês 12 ou não, forçado, modo rede, uso/disponibilidade, salários da tabela do EXE) | índice, RNG e cursor iguais |
| `credit_multiplier_matches_original` | `0x4a8150` (17.2) | 3000 | bit a bit |
| `founding_sliders_match_original` | `0x4bc990`, `0x4bc7d0` (17.2) | 3000 por controle (caixa em todas as faixas, posições vendidas) | limites, `n` e posições enviadas aos widgets iguais |
| `annual_review_matches_original` | `0x43c760` (13) | 8000 (1/3 em estado de crise; 125 bônus, 88 demissões, 20 liquidações, 129 avisos) | todos os campos, as duas pessoas, stubs e RNG iguais |
| `merger_vote_matches_original` | `0x48b5d0` (15.1) | 6000 (4200 aprovadas, 337 reprovadas, 1237 esperando humano) | votos, chamada de fusão, `+0x14c`, RNG |
| `chairmanship_vote_matches_original` | `0x42ee00` (15.3) | 6000 (1718 aprovadas, 2784 reprovadas) | votos, troca, carimbo, RNG |
| `company_init_matches_original` | `0x438a10` (17.4) | 4000 | ações, preço, caixa, campos iniciais, cor, gerente, candidatos, RNG |

O código sob teste é `rt2_sim::finance` (`crates/rt2-sim/src/finance.rs`); as tabelas `0x5504d0` e `0x550074` vêm do EXE (`FinanceTables::load`). Continuam **[S]** (não chamáveis isoladamente: dependem das listas de pessoas/empresas): o passo mensal `0x4399d0` (juros do caixa 5.4, manutenção, salários, dividendos 6), emissão e recompra de ações, emissão e quitação de títulos, desdobramento, margem e patrimônio pessoal; todos estão implementados em `rt2-sim` e marcados `[S]` nos comentários.

`cargo clippy -D warnings` no crate de testes passa; o `-D warnings` do workspace falha hoje em `crates/rt2-sim/src/fp.rs:209` (`int_plus_one`, fora do escopo desta spec).

> **Resolvido:** o teste do preço de negociação usa `rt2_sim::fp::pow` (a emulação fiel do `_CIpow` do EXE) e exige igualdade exata: 0 diferenças em 4000 casos.


## 18. Comandos financeiros do jogador e o que as janelas decidem [V: snapshots do original]

`crates/rt2-sim/src/fincmd.rs` (tipos, pacotes, números das janelas) e `tick/fincmd.rs` (executores) portam o grupo financeiro de `cmd_execute` (`commands.md` 2.1). O executor só valida ids vivos; as janelas (`check_fin`) aplicam as regras abaixo, lidas do desmontador. Teste: `crates/rt2-sim/tests/fincmd_snapshots.rs` (25 snapshots do original chamando `cmd_execute` com um pacote sobre o save Example; `docs/dynamic-analysis.md` 6), `fincmd_flow.rs` (fluxo de ponta a ponta).

**Pacote `0x0e`.** `+0xf` não é "forçada": na compra é a bandeira de **margem** (`0x4a6f60` `param_2`) e o `forçada` de `0x4a7300` é sempre 0; na venda é a bandeira de venda **a descoberto** (`0x4a73d0` `param_3`: sem limites de posição). O lote vem de `0x4a6e00` (7.2).

**Compra** (`0x4a7300`): `0x43b410(n, commit)`; `custo = ftol(cotação(0,0) * n)`; débito `ftol((0.03f + 1.0) * custo)`; `0x46bb10`. **Venda** (`0x4a73d0`): se `n > posição` e não forçada `n = posição`; `preço = ftol(0x43b410(-n, commit))`; crédito `ftol((1.0 - 0.03f) * (preço * n))`; `0x46bb10`.

**`person_can_buy`** (`0x4a6f60(pessoa, c, margem, n)`; `n = 0` = lote): `margem` exige `0x573d72` e `0x5706c0`; exige `0x573d74` ou ser o chairman; `posição <= -n` aceita; senão `0x5706aa`, `n <= float livre` e `cotação*n <= caixa` (e `<= caixa + soma ponderada`), ou, com margem, `cotação*n <= caixa + soma ponderada`. **`person_can_sell`** (`0x4a7190(pessoa, c, curto, cobrir, n)`): `0x5706a9`; `n <= posição`; ou venda a descoberto (`0x573d73` e `0x5706bf`, não chairman, `n < 0x43e340(cobrir)`, cotação `>= 5.0`, `cotação*n <= caixa + soma ponderada`). Soma ponderada: `Σ posição * última cotação (+0x427e) * (0,5 se > 0; 1,5 se < 0)`, em ordem `f*0.5 + f + soma`. `0x43e340(f) = ((ações - Σ longas) >> (f ? 2 : 1)) + Σ curtas` (pessoas ativas).

**Corretor** (`0x46bb10` repete `0x4a7a40(pessoa)` para o conjunto de jogadores até uma passada sem venda; exige `0x573d72`): poder = caixa + soma ponderada; `>= 0`: prazo `+0x544 = 0`. Senão, se a pessoa tem posição: humano sem prazo vencido: aviso LNG 2270 (nome do mês seguinte `mês % 12`), velocidade limitada a 4, prazo `data(ano, mês, 28, 23, 59) + 0x9d7f`; IA ou prazo vencido: duas passadas (a primeira sem a empresa que a pessoa preside), vende 1000 por vez enquanto poder < 0 (LNG 2271) e recompra posições vendidas 1000 por vez com a mesma compra do 7.2 (LNG 2272); prazo zerado. Por fim, posições vendidas em empresas com `0x43e340 < 0` são recompradas à força (LNG 2273).

**Emitir ações** (`0x447730`): `lote = max(1000, ações/20000*1000)`; recusas na ordem: `0x5706ad` (LNG 23), preço `ftol(0x43b410(ftol(-0.7*lote)))` < 5 fora do editor (LNG 276), última emissão (`+0x160` data, `+0x164` hora) há <= `0x761ff` min (LNG 278); confirma LNG 117..11b e envia `0x11` com `+lote`. Executor `0x439060`: preço `ftol(0x43b410(ftol(-0.7 n), commit))`, taxa `-(n*preço)*0.05` na categoria 12, `+0x160/+0x164 = data e hora`, ações `+= n`, caixa `+= n*preço`.

**Recomprar** (`0x447bb0`): `0x5706ae`; `lote0 = ações/20000*1000`; `p = ftol(0x43b410(ftol(0.7*lote0), 0))`; `máx = ftol(caixa / (p * 1.2))`, se `máx < lote0` `lote = máx/1000*1000`; `lote >= 1000`; `lote <= float livre` (0: LNG 284); `lote < ações` senão LNG 2705; `(0.03f + 1)*p*lote <= caixa` (LNG 360). Executor `0x438f70`: `p = 0x43b410(ftol(0.7 n), commit)`, `prod = n*p`, `taxa = -(0.03f * prod)` na categoria 12, `caixa = (taxa + caixa) + (-prod)`, `patrimônio = (patrimônio - taxa) - (-prod)`; tudo em `f64`.

**Títulos** (`0x448250`): `0x5706af`; menos de 20 títulos (LNG 289); `ftol(rating) >= 5` fora do editor (LNG 290 com as notas `0x5537bc`); o diálogo mostra a taxa `0x4392e0`, 500 000 e 2%; envia `0x14` com 500 000 e 30 anos. Quitar (`0x4480f0`): `0x5706b0`; `caixa >= principal * 1.02` (LNG 361); LNG 288 com a multa de 2%.

**Dividendo** (`0xcb`, `0x449300`): botão `+` (`0x693`): `teto = 0x43edf0` (ou o dividendo atual se menor); recusa LNG 2370 se `teto <= valor + 0.0100...`; senão `valor = ftol(valor*100 + passo + 0.5) * 0.01` limitado ao teto; botão `-` (`0x694`): `ftol(valor*100 - passo + 0.5)*0.01`, mínimo 0; `passo = max(5, ftol((valor*100/6 + 2.5) * 0.2) * 5)` centésimos. OK envia `0x12` (`0x5706b1`). `rt2_sim::fincmd::dividend_click`.

**Falência** (`0xa94`): `0x5706ac`; `ano - +0x158 < 5`: LNG 297; `fundação <= ano - 2` e lucro(ano-2) <= 0 e lucro(ano-1) <= 0 senão LNG 3412 (`0xd54`); confirma LNG 366; envia `0x13`.

**Gerente** (`0x10`): executor `0x438780` (14.3) com taxa `ftol(efeito(0x10) * salário)/2000*1000`; recusa se `uso[g] != 0`; a janela recusa se a taxa passa do caixa (LNG 293).

**Fusão** (`0x48b170`): `0x5706b4`; menos de 2 empresas: LNG 387; `+0x14c != 0` e `<= 0x761ff` min: LNG 388; diálogo (alvo LNG 529, preço LNG 530 iniciando em `ftol(cotação) + 1`, total = ações do alvo x preço; LNG 386 se maior que o caixa da compradora); envia `0x22`. Proposta (`0x48b4f0`): votam o chairman da compradora (sim) e todo acionista do alvo com posição > 0 (IA: não provisório; humano que não é chairman da compradora: pendente). Apuração (`0x4015a0`, 15.1): mensagem LNG 24/25 e duas colunas LNG 63/64 (nome e posição de cada acionista, "Other Investors" LNG 19 = `ftol(F*clamp(C))/1000*1000` de um lado e o resto do float do outro, totais). Aprovada: `0x43c3c0` (`news_post` LNG 168; trilho, trens (+1 em `+0x27` cada), estações (`+0x172`), títulos (até 20), indústrias, as 8 contas, contadores de trilho `+0x182..+0x18e`; cada acionista recebe `posição*oferta` e perde a posição; a compradora paga `ações*oferta`; `0x438170(alvo, 0)`; cotação). Reprovada: `+0x14c = agora`.

**Presidência.** Botões do livro (`0x443ab0`; pessoa local): `Resign` (LNG 327, `0x5706bc`, é o chairman); com `0x573d75` e `0x5706bb`: `Assume` (LNG 329, `0x469ed0 > 0.5` = acionista de mais de metade, ou editor; envia `0x2e` depois de LNG 363) e `Call election` (LNG 331, `>= 0.1`; recusa LNG 364 se `+0x527` da pessoa foi há menos de `0x76200` min; confirma LNG 365; envia `0x24`). `0x43c050`: `news_post` LNG 27, a pessoa deixa a outra empresa e `0x438900`. `0x438900(empresa, novo)`: se `novo != atual`: o antigo recebe `+0x52b += 0x4a8100` (mínimo -99), `+0x263 = 0`, e, se é a pessoa local, `0x467fd0(0)`; humor `*0.8 + 0.2` (sem chairman ou IA) ou 0; o novo recebe `+0x263 = empresa` (e `0x467fd0(empresa)` se local); `+0x46a = +0x52b` do novo; salário 10 000; `+0x470 = ano`. `0x467fd0(id)` grava a empresa local (cópia salva no cabeçalho `0x570574`, `header[0x8a3c]`; 0 também zera `0x570512`).

**`news_post`** (`0x4a42f0`): cada aviso de fusão/presidência acrescenta um item à lista de notícias (fora do save) com um sorteio `crt_rand_range(0, 10000)`, `0x5707be += 1` (`state[0x2d6]`) e `0x5707bd = 1` (`state[0x2d5]`): o único estado são o contador, a bandeira e o sorteio do CRT.

**Liquidação** (`0x438170(empresa, 0)`): solta o trilho que sobrou, libera o gerente (`uso`, `+0x13c = -1`), tira a coluna de cada pessoa deslocando as seguintes (`0x1cf + n*4 = 0`), tira o chairman, zera `+0x16e`, tira do conjunto `0x570729`; se era a empresa local, `0x467fd0(0)`.

**Não portado**: `0x4d14b0` (reavaliação da rota de cada trem do comprador depois da fusão, não alterou nenhum campo no snapshot); `net_min_peer_version` (jogo local: as votações exigem as duas empresas ativas); faixa do controle de preço do diálogo de fusão (`0x48abc0`, widget `0x118` com argumentos 4 e 6; o clone usa de 1 a 3 vezes a cotação).
