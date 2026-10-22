# Fontes bitmap (`font%s.dat`)

Implementação: `crates/rt2-formats/src/font.rs`; ferramenta: `rt2-font` (`rt2-font all -o fontes.png -s`). Teste contra os dados originais: `crates/rt2-formats/tests/ui_font.rs`.

## Resumo

- **Não existe arquivo `font*.dat`.** `font%s.dat` (`0x55d56c`, montado em `pk2_get_item_type2` `0x4f4bc0`) é só um *nome*: `pk2_get_item(2, nome)` calcula o hash e o construtor de fonte `0x4f24c0` compara esse hash (hash1 ou hash2) com o de `font0.dat` .. `font10.dat`; o índice que casa (1..10) escolhe uma das 10 definições embutidas no executável. Nenhum PK2 contém essas entradas (verificado em teste).
- Cada fonte é um banco LAD (`.imb`, ver `lad.md`) carregado por `pk2_get_item_type0`: o **registro `i` é o caractere `0x20 + i`** (224 registros, 0x20..0xFF; `htra46` só 96).
- Cada registro tem 2 grupos de 1 imagem. **Grupo 0 = glifo**, **grupo 1 = sombra** (desenhada só quando o estilo pede, deslocada 1 px para baixo/esquerda; ver abaixo). `kind = 3`, `palette_id = 0`; as cores não vêm da paleta: o texto é desenhado com uma cor única (tabela de 14 cores, abaixo).

## Definições (`switch` em `0x4f24c0`)

Altura e baseline: a linha de base de um glifo fica na linha `topo + altura - 1`, e a origem `(x, y)` da imagem é relativa a esse ponto (y negativo = acima). Passo de linha = `altura + entrelinha`.

| N | banco | altura (+8) | espaç. (+0xc) | entrelinha (+0x10) | sublinhado (+0x18 / +0x1c) | observações |
|---|---|---|---|---|---|---|
| 1 | copp6.imb | 6 | 1 | 4 | 1 / 1 | |
| 2 | copp8.imb | 8 | 2 | 5 | 1 / 1 | fonte padrão dos controles de UI |
| 3 | copp14.imb | 14 | 3 | 8 | 2 / 2 | |
| 4 | copp24.imb | 24 | 4 | 11 | 2 / 3 | |
| 5 | copp10.imb | 10 | 2 | 6 | 1 / 1 | |
| 6 | copp12.imb | 12 | 2 | 7 | 1 / 1 | |
| 7 | scri11.imb | 11 | 1 | 8 | 1 / 1 | cursiva; larguras da tabela `0x55d358` |
| 8 | fran13.imb | 13 | 2 | 8 | 1 / 1 | |
| 9 | haet46.imb | 46 (59 no codepage 5) | 3 | 15 | 3 / 4 | título; dobra para maiúsculas |
| 10 | htra46.imb | 46 | 3 | 15 | 3 / 4 | título "transparente"; 96 glifos; dobra para maiúsculas |

Larguras de avanço medidas (A, a, 0, espaço, W): N1 8,7,7,4,11; N2 11,9,10,5,15; N3 19,17,17,9,26; N4 33,29,27,14,42; N5 13,12,12,6,16; N6 16,13,13,7,21; N7 10,9,9,5,17; N8 14,12,13,12,19; N9 31,31,27,18,43.

## Métricas

- **Índice do glifo** (`0x4f1f60`): `ch - 0x20`; fora de `[0, nº de registros)` vira 0. Fontes 9 e 10 (`+0x30 = 1`) dobram antes: lower case e acentuadas viram a maiúscula base (Ù Ú Û Ü→U, Ò..Ö→O, Ì..Ï→I, È..Ë→E, À..Å→A, Ý ý ÿ→Y, Ñ→N, Ç→C); no codepage 5 (polonês) há mais pares preservados.
- **Avanço** (`0x4f2970`): `largura_do_box_do_glifo + min(x, 0) + espaçamento`, com o box do grupo 0. A fonte 7 usa a tabela de bytes com sinal em `0x55d358` (índice `ch-0x20`, 96 entradas) mais o espaçamento. O espaço (0x20) tem a largura do `1` (`0x31`); `[ ] { } |`, 0x1e e controles (<0x20) têm largura 0 (são markup). Existe um modo de dígitos tabulares (`[N1`...): `$ , . - espaço` e `0-9` ocupam a largura máxima dos caracteres `0123456789.$-, ` (`this+0x2c`).
- **Sem kerning.** Texto = soma de avanços.
- **Linhas**: `\n` (10) quebra; o desenhador (`0x4f3570`) também quebra por largura no último espaço/hífen, trunca (acrescenta a string de `0x476910(6)`, provavelmente reticências, não confirmado) quando a altura acaba e alinha vertical (centro/fim) calculando a altura em modo de medição (`+0x20 = 1`).
- **Markup** (consumido por `0x4f3570`/`0x4f2a50`/`0x4f2ba0`): `[...]` comando inline (cor, estilo; `[N<d>` altera o modo de dígitos tabulares), `{` empilha o estilo e `}` desempilha. O estilo vive em 8 níveis de 32 bytes em `0x57b580`: cor (índice na tabela de cores, convertida por `0x4f9880` para o formato de tela), flag de sombra (+0x14), sublinhado (+0x18: 1 sempre, 2 só na última linha). Detalhes dos comandos `[...]` ainda não decodificados.
- **Cores do texto** (`0x55d438`, XRGB1555): `7fff 7ff0 6318 0001 7ff0 6000 7e0c 39c4 4a46 03e0 7fe0 7c00 14a2 2d84` (branco, amarelo claro, cinza, quase preto, ...).
- **Sublinhado**: retângulo de `avanço x espessura` na linha `altura + offset + k` (`0x4f6d00`).
- Com `DAT_0057d260 != 0` as fontes 1 e 7 desenham o glifo duas vezes com 1 px de deslocamento (provável negrito sintético; não confirmado).

## Codificação do glifo (RLE da `0x4fff00`)

Mesmo enquadramento do LAD (`[len][tokens]`, len de 1 ou 2 bytes), mas com mais tokens do que o decodificador simples de sprites (`lad.rs`):

| token | significado |
|---|---|
| `00` | fim da linha |
| `01..7f` | `n` índices de paleta seguem (não usado nos glifos) |
| `80..9f` | corrida de `n = t & 7` pixels (0: próximo byte) que **escurecem o destino** pela tabela `(t >> 3) & 3` |
| `a0..bf` | corrida de `n = t & 0x1f` pixels (0: próximo byte) de **alfa**: `0` = sólido; `a` = `dst = (a·cor + (255−a)·dst) >> 8` |
| `c1..ff` | pula `t & 0x3f` pixels; `c0` encerra a imagem |

Tabelas de escurecimento (`0x470b50`, canal de 5 bits): 0: `max(c−8, c/2)`, 1: `max(c−4, c/2)`, 2: `(0x78+3c)/8`, 3: `(0x5a+3c)/8` (puxam para cinza), 4: outra rampa. O grupo 1 (sombra) usa `0x81` (nível 0) para escurecer 1 px atrás de cada pixel do glifo.

Atenção: o `lad.rs` trata `0x80..0xbf` como literais; para sprites de mundo isso ainda não foi checado contra o desenhador `0x4ff900` (provavelmente o mesmo conjunto de tokens; ver "Em aberto").

## Em aberto

- Comandos do markup `[...]` e as 5 tabelas de escurecimento completas; modo `[N..]` tabular.
- `fran13.imb` (fonte 8) mostra avanços largos e um `Q` elevado: pode ser desenho da fonte ou métrica a refinar.
- Semântica de `+0x04` (valores 0/1/2 por fonte) e `+0x1c`: provavelmente flags de estilo (sombra); não usados pelos parâmetros acima.
- Verificar se os sprites de mundo (`0x4ff900`) também usam os tokens `0x80..0xbf` (sombras projetadas, alfa).
