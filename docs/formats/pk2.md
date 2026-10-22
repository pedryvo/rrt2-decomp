# PK2 — contêiner de arquivos

Arquivos: `data/RT2.PK2` (2880 entradas), `RT2_06.PK2` (16), `RT2_09GO.PK2` (1), `RT2_11PL.PK2` (4). O jogo carrega todos via `.\data\*.pk2` (string em `0x55d500`).

## Layout (little-endian)

| Offset | Tipo | Campo |
|---|---|---|
| 0 | u32 | N = número de entradas |
| 4 + 12·i | u32 | hash do nome do arquivo |
| 8 + 12·i | u32 | offset do conteúdo, relativo ao fim do diretório (`4 + 12·N`) |
| 12 + 12·i | u32 | tamanho em bytes |

- O conteúdo **não** é comprimido. A última entrada termina exatamente no fim do arquivo nos quatro PK2, o que é verificado em `crates/rt2-formats/tests/original_data.rs`.
- A ordem das entradas nem sempre segue o hash: RT2_11PL e RT2_09GO estão ordenados, RT2 e RT2_06 não.

## Conteúdo observado

- Muitos blobs começam com o cabeçalho `08|04 00 00 00  03 00 00 00  bc 02 00 00 ...`, um formato próprio (imagem ou sprite?) ainda a identificar.
- Blobs como a entrada 8 em diante do RT2.PK2 parecem PCM de 16 bits sem cabeçalho.
- `RT2_09GO.PK2` tem uma entrada (hash `0x4d5b6f0b`); `setup.imb` (`0x0c9b172f`) está em RT2_11PL e RT2.

## Hash de nomes

Verificado em Python (`re/scripts/pk2_hash.py`): `hash("setup.imb") == 0x0c9b172f`, a entrada de RT2_11PL.PK2 (e de RT2.PK2).

```
fold(c)  = c - 0x20  se 'a' <= c <= 'z' ou c > 0xEF, senão c      ; FUN_004f0d70 (maiúsculas)
hash(s)  = h = 7
           para cada byte c de s (toda a string, extensão incluída):
               h = (fold(c) + 0x3039 + h * 0x41C64E6E) mod 2^32   ; LCG
```

- Fonte: laço em `0x4f4e60` (existe?), `0x4f5070` (posiciona o arquivo no conteúdo) e `0x4f48e0` (carrega/cacheia o item). Os três recalculam o hash inline.
- Sem separadores de caminho removidos: o jogo passa nomes simples (`setup.imb`), sem diretório. Maiúsculas/minúsculas não importam.
- O bit 0 do hash é a paridade de `fold(último char) + 1` (multiplicador par), o que explica a regularidade nos nibbles baixos; não há outro padrão.
- Segundo hash (`h2`): `s=0,u=0; para c do fim ao início de fold(c): s+=c; u=(u>>25)+c+u*32+s`. Só é testado quando o primeiro não acha; serve ao formato antigo `.PAK` (`rt2.PAK`, contagem u16, offsets absolutos, ver abaixo). Nenhuma entrada dos PK2 casa com ele (testado com `setup.imb`).

### Rotinas

| Endereço | Função |
|---|---|
| `0x4f43c0` | enumera `.\data\*.pk2` com FindFirstFile/FindNextFile (chamada de `0x4efc30`) e chama `0x4f4c20` por arquivo |
| `0x4f4c20` | abre um pack e lê o diretório: tipo pelo último char do nome (`'K'` = `.PAK`: contagem u16, offsets absolutos; senão PK2: contagem u32, offset relativo ao fim do diretório `4+12N`); guarda handle, contagem e tabela de 12 bytes/entrada nos arrays globais `0x57b904`/`0x57b954`/`0x57b92c`/`0x57b97c`; `0x57b9ac` = nº de packs |
| `0x4f4e60` | `pk2_has_file(nome)`: calcula hash1, busca em todos os packs; senão hash2; devolve 1/0 |
| `0x4f5070` | `pk2_seek(nome)`: acha o primeiro pack/entrada que casa e faz seek no conteúdo (`offset + 4 + 12N` para PK2) |
| `0x4f48e0` | `pk2_get_item(tipo, nome)`: cache de itens (11 bytes/slot em `0x57b9a4`, contagem de uso), cria o objeto por tipo 0/1/2 |
| `0x4f4600` | encerra: reporta "Pack Item leaked", fecha os packs |
| `0x465050` | inicialização; só testa a existência de `.\DATA\rt2.PK2` (não carrega) |

### Precedência (resolvida)

`0x4f43c0` escolhe repetidamente o nome alfabético (strcmpi) **maior** abaixo do limite anterior (limite inicial `"ZZZZZZZZ"` em `0x55d4f4`, valor inicial `""` em `0x57bae4`), então os packs são carregados em ordem **alfabética decrescente**: `RT2_11PL`, `RT2_09GO`, `RT2_06`, `RT2` (índices 0 a 3). As buscas varrem os packs do índice 0 em diante e a primeira ocorrência vence, então **o patch de nome maior sobrescreve o menor**. Confirmado pelos dados: só 3 hashes se repetem (`0x0c9b172f` setup.imb, `0x3bf83d8f`, `0xb8be0a3f`), todos em RT2 e RT2_11PL, e o RT2_11PL vence. Hash1 é buscado em todos os packs antes de hash2.

### Outros caminhos de arquivo

- Não há fallback para arquivo solto na busca em pack: `0x4f5070`/`0x4f48e0` não checam "não achou"; os chamadores usam antes `0x4f4e60` para escolher variantes (ex.: `setupl.win`, `setupg.win`, `setup.win`, `set.win`).
- Arquivos soltos no disco são abertos direto por `0x51c53d` (open): animações `%sanim\*.SMK` (inclusive no CD), `music\track03.wav`, `hiscore.dat`, `rt2.log`, `rt2_tsc.CFG`, `language.log`, `*.lng`, DLLs.
- Procura de `\DATA\rt2.PAK` (formato antigo): diretório atual, `HKLM\Software\PopTop Software\Railroad Tycoon II` valor `AppPath`, `c:\program files\railroad tycoon...` e `A:\Games\RT2 Gold`: apenas para detectar o modo CD (`0x57b960`).

### Extensões de nomes no EXE

`.imb` (116 nomes casam; 12 sem match são modelos `%s`), `.raw` (123/123), `.win` (88 de 91; os 3 sem match são `buygold.win` e `setupl.win`, só testados por existência), `.dat` (`FLATC16.DAT` casa; `font%s.dat`, `shd16555.DAT`, `hiscore.dat` etc. não), além de `.smk`, `.pcx`, `.pal`, `.lng`, `.wav`, `.mp3`, `.cfg`, `.log`, `.dll` (fora dos packs). Lista completa em `re/names/exe_filenames.txt` (414 candidatos extraídos por regex de strings; muitos são lixo).

### Validação

Atualização: 1739 nomes de bancos de sprites foram recuperados dos formatos `cr%s%s.imb`, `bl%s%s.imb`, `bn%s%s.imb` e afins (ver `sprite-catalog.md`); `re/names/exe_filenames.txt` agora tem 2153 linhas e 2067 casam com hashes dos PK2. O texto abaixo descreve o primeiro levantamento.

Dos 414 candidatos, **328 casam** com hashes dos 4 PK2 (2898 hashes distintos). Os 86 restantes são modelos com `%s`/`%d`, DLLs, `.smk`/`.lng`/`.log` soltos e lixo. Todo nome concreto `.imb`, `.raw` e `.win` de pack casa, sem falsos negativos observados; o EXE referencia ~11% dos 2898 nomes (o resto vem de listas em dados e nomes montados com `%s`).

## Em aberto

- Nomes das ~2570 entradas restantes: dependem de nomes montados em tempo de execução (`%s%s.imb`, etc.) e de tabelas em dados.
- Formato `.PAK` (`0x4f4c20`, ramo `'K'`): só visto no código; não há `rt2.PAK` na pasta do jogo.
- Qual dos dois hashes o `.PAK` realmente usa não foi confirmado.
