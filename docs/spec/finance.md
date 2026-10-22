# Finanças: razão da empresa, preço da ação, títulos, dividendos, falência

Fonte: `RT2_PLAT.EXE` (sha256 `510b9e44...`), lido a partir de `re/db/decomp` e do desmontador (a decompilação do Ghidra perde argumentos de `_ftol` e larguras f32/f64; as constantes abaixo foram lidas do `.rdata`/`.data` conforme a instrução x87 real). O manual (`docs/reference/manual-rules.md`, seção 8) só serviu de pista; as divergências estão na seção 12. Estado das provas: **[V]** = confirmada por teste diferencial (`crates/rt2-oracle/tests/finance.rs`), **[S]** = lida no código, sem teste. Endereços são VAs (base `0x400000`). Todas as contas usam a precisão x87 de 53 bits (CW `0x27F`, ver `time.md`).

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
| `+0x13c` | i32 | gerente atual (índice na tabela de gerentes, stride 0x11; salário i32 em `0x54fdad + 0x11*i`) |
| `+0x148` | i32 | ano de fundação |
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
| `+0x46fe` | f64[13] | cópia usada só quando o ano vale 9001 (`0x2329`, marcador) |
| `+0x42ee` | 10 x 13 f64 | arquivo anual das categorias (`k-1`-ésimo ano atrás, 0x68 por ano) |
| `+0x4766` | f64[8] | contas: 0 caixa, 1 prédios, 2 terrenos/estações, 3 trilho, 4 indústrias compradas, 5 trens, 6 **dívida (negativa)**, 7 **patrimônio (negativo)** |
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
se d > 0 e (caixa_depois < -0.2*patrimônio  ou  máx <= 0.01):  d = 0; mensagem LNG 2274 "dividendo suspenso"
senão se máx < 0.75*d:   d = máx; mensagem LNG 2713 "conselho reduziu o dividendo para %1" (só empresa do jogador)
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
- **Lote**: `0x4a6e00` escolhe o tamanho de uma operação: arredonda para milhares, `~4% da posição + 1000`, reduzindo em 1000 até passar na verificação; mínimo 1000.

### 7.3 Margem e chamadas de margem (`0x4a7a40`, chamada de `0x46bb10` após cada negócio e no passo mensal) [S]
`poder = caixa + Σ posição_c * +0x427e_c * (0,5 se posição longa; 1,5 se vendida)`. Se `poder >= 0` o prazo `+0x544` é zerado. Senão, para o jogador humano: primeiro aviso (LNG 2270 aproximado) e prazo = `date_to_minutes(ano, mes, 28, 23, 59) + 0x9d7f` (28 dias menos 1 min depois do dia 28 às 23:59, ou seja, até 28 do mês seguinte); depois do prazo, ou imediatamente para a IA, o corretor vende (em lotes de 1000, `0x4a73d0`) ou recompra posições vendidas até `poder >= 0`, em duas passadas (a primeira poupando as ações da própria empresa do chairman), com as mensagens LNG 2271/2272. Depois, posições vendidas em empresas sem float livre (`0x43e340 < 0`) são cobertas à força (LNG 2273).

## 8. Fim de ano da empresa (`0x43d800`, de `sim_year_end`) [S]
1. Arquiva as 14 métricas, as categorias e as contas (desloca o arquivo, 10 anos), zera as categorias do ano, `+0x38`, `+0x192/+0x196/+0x19a`.
2. **Overhead** `+0x138`: média dos últimos 3 anos (contando o ano que acabou de ser zerado: divide por até 3) da receita `r`; `g = sqrt(max(1, r/100000))` comprimido (acima de 10, 15, 20 cada excesso pela metade); `+0x138 = max(1000, ftol(r*g*0.01))`.
3. Títulos com `vencimento == ano+1` são quitados (4.4).
4. **Desdobramento**: cotação com commit; se `> 100`: `n = ftol(+0x427e * 0.02)`; ações, carteiras e `+0x34` `*n`; `+0x30`, `+0x38`, `+0x3c` e as colunas de preço, patrimônio, receita, lucro, dividendo e `+0x38` do arquivo `/n`; mensagem LNG 169 ("split %2 for 1").
5. `0x43c760` (anual, 5.2).

### 8.1 Avaliação anual da diretoria (`0x43c760`) [S, resumo]
Calcula a reputação do chairman `+0x46a` a partir do crescimento do preço e do lucro por ação, e o fator `+0x4248` (atitude); paga bônus ao chairman (`+0x474`...); se o público (ações fora do chairman) > 50% e há 2 anos de resultado ruim, a primeira vez avisa (LNG 2277) e na segunda **demite o chairman** (LNG 2275/2276, `0x438900`). **Liquidação**: três anos seguidos de preço abaixo de 5/7/9, patrimônio negativo e prejuízo => a empresa some (`0x438170`, LNG 2279/2280), com aviso no segundo ano.

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

## 10. Fusão e presidência [S, resumo]
- **Fusão** (comandos `0x22`/`0x23`, propostas em `0x574ca0`, `0x48b4f0`/`0x4015a0`): o proponente oferece `preço` por ação (u16). Cada acionista IA com ações vota sim com probabilidade `((rand(-10,10)*0.01 + (rating5_proponente - 0,1)*0,33... + ((oferta/efeito(0x22))/ (média(preço fundamental, cotação)) - 1)*0,7 + 0,1) - 0,1)`, menos 0,45 se é chairman da alvo, mais ruído `rand(-20,20)*0,01`, comparada a `rand(0,100)` (valores exatos em aberto). Humanos votam por mensagem (LNG 164). Passa se os votos sim > metade das ações da alvo (`+0x2c / 2`); executa `0x43c3c0`. Se falha: `+0x14c = game_time_minutes` (um ano de espera, LNG 388).
- **Eleição de chairman** (`0x42ed10`/`0x42ee00`): `p = clamp(rand(-10,10)*0.01 + rep_desafiante*0.0125 + (0.2 - rep_chairman*0.016667), 0, 1)`; cada acionista IA que não é o chairman vota sim se `rand(0,100) < (rand(-20,20)*0.01 + p)*100`; vence com mais de metade das ações (LNG 26..28). Troca de chairman: `0x438900` (reputação do anterior atualizada por `0x4a8100`, `+0x134 = 10000`).
- Compra de indústria (comando `0x2d`, `0x43eb60`): preço `0x40bae0`, conta 4 `+= preço`; a venda devolve metade (a outra metade vai à categoria 3).

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

## Em aberto
- Teste do arquivo anual para anos acima de 9001 (`0x2329`, marcador) e do ramo do primeiro ano.
- Motivo de `max(NI, Rv)` em `0x43edf0` (a conta está provada, 6.1).
- Constantes finas da votação de fusão (4 f32/f64 em `0x4015a0`) e de `0x43c760` (reputação do chairman); critério exato da liquidação por 3 anos ruins.
- Limites da interface: emissão (preço e retorno mínimos, LNG 276/277), bancarrota (LNG 297/3412), dividendos (LNG 2370/2713) e tamanho de lote do jogador.
- Capital inicial (controles "Player Investment" e "Outside Investment"): criação da empresa não foi analisada.
- Papel de `0x5706d9` (=2), `0x5706d5` (5,0 = rating mínimo para bonds, confirmado) e do efeito `0x21` (liquidez, LNG 1651).
- Políticas de IA (9.1): limiares e ordem exatos de `0x45fbf0`.
- `0x43abc0` (retorno ponderado de 5 anos) e `0x4a7870/4a7880` (arquivo anual de patrimônio da pessoa).

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

O código sob teste é `rt2_sim::finance` (`crates/rt2-sim/src/finance.rs`); as tabelas `0x5504d0` e `0x550074` vêm do EXE (`FinanceTables::load`). Continuam **[S]** (não chamáveis isoladamente: dependem das listas de pessoas/empresas): o passo mensal `0x4399d0` (juros do caixa 5.4, manutenção, salários, dividendos 6), emissão e recompra de ações, emissão e quitação de títulos, desdobramento, margem e patrimônio pessoal; todos estão implementados em `rt2-sim` e marcados `[S]` nos comentários.

`cargo clippy -D warnings` no crate de testes passa; o `-D warnings` do workspace falha hoje em `crates/rt2-sim/src/fp.rs:209` (`int_plus_one`, fora do escopo desta spec).

> **Resolvido:** o teste do preço de negociação usa `rt2_sim::fp::pow` (a emulação fiel do `_CIpow` do EXE) e exige igualdade exata: 0 diferenças em 4000 casos.
