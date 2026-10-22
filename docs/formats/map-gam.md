# Formato MAP / GAM (mapas de cenário e saves) — primeira passada

Escopo: `maps/*.MAP` (31 arquivos; os `*.MP2` do Platinum parecem ser o mesmo formato, não analisados) e `games/*.GAM` (Tutorial, Example save game). Tudo little-endian, **sem compressão** (entropia ~3 bits/byte, regiões inteiras de zeros). Ferramenta: `re/scripts/mapgam.py` (numpy + Pillow). Tudo abaixo vem só dos dados; nenhum endereço do EXE foi consultado.

PNGs de prova (scratchpad, não versionados), pasta `/tmp/claude-1000/-home-pedryvo-Decomp-railroad/6d7cf073-1b4e-458b-98c6-4163984526ea/scratchpad/maps/`:
`t_britain.png` (Grã-Bretanha/Irlanda reconhecíveis), `t_bo.png` (Chesapeake e Delaware Bay), `elev_all.png` (os 31 mapas: Alpes/Itália, Índia, Japão, Austrália, África, América do Sul, Escandinávia, América do Norte), `bo_sheet.png` (cada coluna do registro de tile da B&O).

## 1. Cabeçalho (confiança: alta nos campos citados)

| Offset | Tipo | Conteúdo |
|---|---|---|
| 0x000 | u32 | `0x233C` (mágica/versão) |
| 0x004 | char[] NUL-pad | nome (nos `#NN…` é o nome 8.3 do arquivo, p.ex. `#01B&O~1`) |
| 0x131 | char[] | título do cenário (p.ex. `#01 B&0 @`, `#07 Birth of the Iron Horse`) — média |
| 0x150..0x23a | flags/u32 | opções do cenário (bytes 0/1, `1e`, `23` etc.), significado desconhecido |
| **0x231** | **u16 w, u16 h** | **largura e altura do mapa em tiles** (150x150 ... 500x500; 3 arquivos 300x300 idênticos em tamanho) — alta |
| 0x23b | char[] | nome de novo (nome curto do cenário) |
| 0x25a | char[] | descrição (texto livre, ~294 B) |
| 0x390 | ~28900 B | preview/minimapa; nos `#NN` parece imagem 170x170 de 1 byte com dither (valores 0..3 e 227+), nos demais maps é outra coisa (cor 16 bit?) — baixa |
| 0x7474 ... 0xB2D6 | ~16 KB | quase tudo zero, alguns blocos (palettes/ cor 16 bit, `0x8D57` zeros...) — desconhecido |

## 2. Mapa de seções do arquivo (alta para tiles/elevação; média para a cauda)

```
0x00000  cabeçalho (acima)
0x0B2D6  1 byte (0)
0x0B2D7  TILES: w*h registros de 10 bytes, linha a linha (y*w+x), norte em cima
         fim = 0xB2D7 + 10*w*h
fim      ELEVAÇÃO: (w+1)*(h+1) i16, linha a linha (cantos dos tiles), SEM gap
         fim_elev = fim + 2*(w+1)*(h+1)
fim_elev CAUDA (estado): u32s, tabelas, jogadores, empresas, lugares, eventos
```

O início fixo `0xB2D7` vale para os 31 maps e para os 2 GAM (é consistente porque o cabeçalho é de tamanho fixo). Offsets por arquivo: `mapgam.py info maps/*.MAP`. Exemplos: B&O (150x150) tiles 0xB2D7, elev 0x421BF, cauda 0x4D3E1 (37 697 B); Britain (300x300) tiles 0xB2D7, elev 0xE6E77, cauda 0x113249 (53 611 B).

## 3. Registro de tile (10 bytes) — confirmado visualmente na B&O e Britain

| Byte | Campo (nome provisório) | Observado | Conf. |
|---|---|---|---|
| 0 | `var` | 0/0x40/0x80/0xC0 + nibble baixo ocasional; ruído, parece variante/rotação de textura | baixa |
| 1 | `ground` | id do tipo de chão (0x00..0x62). **0x61 e 0x62 = água** (oceano/lago/baía; mascara Chesapeake, Delaware Bay e o mar da Grã-Bretanha). 0x00 contorna a costa (tiles de borda). Ex.: 0x2d, 0x36, 0x24, 0x12, 0x1b = vegetação/solo; 0x5b/0x5c/0x3f = rocha/montanha (altitude média 225–347) | alta (água), média (resto) |
| 2 | `flags2` | bit 6 (0x40) em quase todos os tiles de terra e ~1/3 da água; bits 0..3 pequenos valores | baixa |
| 3 | `trees` | 0..28, espalhado em pontos pela terra (nenhum em água) = densidade/quantidade de árvores | média |
| 4 | `struct_id` | 0 ou id único crescente (B&O: ~73 ids, agrupados em (55..77, 50..72) = Baltimore) = id de construção (cidade/indústria) numa tabela | média |
| 5 | — | sempre 0 | alta |
| 6 | `flags6` | bits 3 e 4 sempre ligados na água; bit 2 nunca na água; bit 7 / bit 5 variam. Provável: bitfield de borda/rio/costa | baixa |
| 7 | `track_id` | 0 ou id único; na B&O forma um traçado contínuo (78,47)→(73,52)→(60,53) = **trilho pré-construído** | média |
| 8 | — | sempre 0 | alta |
| 9 | `region` | 1..N (B&O: 1..5, até 31 no Wheel/…): territórios (blobs retangulares/arredondados em `bo_sheet.png`: Harpers Ferry, Baltimore, Washington, Filadélfia) | alta |

Observação: na primeira posição (0xB2D6) há um byte 0 antes do primeiro registro; com a fase certa as colunas 5 e 8 são zero em 100% dos tiles.

## 4. Elevação (alta)

`i16` com dimensão `(w+1)*(h+1)` (altura nos cantos; verificado por suavidade: menor diferença média entre vizinhos com stride `w+1`). Valores pequenos negativos (65535 = -1, -2) aparecem no fundo do mar (Britain, Silésia…): **ler como i16**. O nível do mar varia por mapa (B&O ~21–50, Britain 0; máx típico 120–510). Para o renderizador: altura do tile = média dos 4 cantos; água = `ground in (0x61,0x62)`.

## 5. Cauda: estado do jogo (média/baixa)

Começa com u32 (B&O: `100, 7, 0, 100, 18, 200, 73, 2, 0` em 0x4D3E1) e uma tabela de registros de 6 bytes (`03 17 00 00 00 00` ...) ~85 entradas, depois blocos de strings:

- **Jogadores/figuras**: nome (`Isambard Kingdom Brunel`) + bio, stride `0x4BD` (1213 B).
- **Empresas** (`Great Western`, `Liverpool & Manchester`, ...): stride `0x196C` (6508 B).
- **Lugares nomeados** (cidades, mares, regiões de indústria): stride `0x96` (150 B), com saltos de `0x12C` (registro vazio). Em Britain são ~90 (Dover, Brighton, … `Atlantic Ocean`, `Iron,LTcoal`).
- **Territórios**: 3 registros stride `0x27A` (634 B) (`Britain`, `Ireland`, `France`).
- **Regras de vitória e eventos**: texto + expressões (`GameYear=1855 and PlayerNetWorth(IsHighestInClass)`, `TerritoryId=2 and GameYearMonth=184509`, `CompanyCash * 4<=CompanyDebt`, ...). Uma linguagem de condições interpretada em tempo de execução.
- **Cidade pré-colocada (B&O)**: `Baltimore` em 0x4D653, precedida por u32 `(0x46,0x35),(0x46,0x36)` = x=70,y=53 e y=54 (coincide com o aglomerado `struct_id`).

## 6. .GAM vs .MAP

Ambos os `.GAM` são derivados de **Britain.MAP** (300x300; 2086 cantos de elevação e 191 tiles de `ground` diferentes, contra 90 000 de qualquer outro mapa 300x300), e têm o **mesmo layout** até `fim_elev` (0x113249). Diferenças: tiles e elevação alterados (terraplenagem/trilhos: colunas 2,3,4,5,6 e `track_id` mudam; Example save: 184 `track_id`≠0 contra 0 no MAP) e **cauda muito maior** (Tutorial +60 KB, Example +110 KB): lista de cidades vivas (stride `0x39B`=923 B, `Oxford`, `Slough`, `London`, ...), empresas e trens (`Nottingham Northern`, `Slough Bullet`), seguidas dos mesmos blocos do cenário (lugares, territórios, vitória, eventos). Os dois GAM não têm campo de data ainda localizado.

## 7. Perguntas abertas

1. Significado exato de `var`, `flags2`, `flags6`, `ground` (tabela id→terreno/textura/custo); relação com os 7 tipos de terreno do manual.
2. Preview (0x390) e bloco 0x7474..0xB2D6: formato, tamanho exato, se é cache regenerável.
3. Estrutura dos registros de cidade/indústria/estação/trem/trilho (cauda). `struct_id`/`track_id` indexam quais tabelas? Dimensão e layout de cada registro (0x39B, 0x196C, 0x96, 0x27A...).
4. Onde está a data do jogo, o dinheiro, o cargo/oferta-demanda por cidade, o RNG seed.
5. Por que há 1 byte antes dos tiles e como o tile 0 tem `region=0`.
6. O nível do mar e o limite de 510 em alguns mapas (clamp?).
7. `.MP2`: mesmo formato? (mesma mágica; comprimento maior).

## 8. O que precisa do executável

- Rotina de carga do mapa: procurar xrefs de `*.MAP` / `.GAM`, e leituras de `0x233C`; ela revela os `fread` na ordem das seções e os tamanhos (`10*w*h`, `2*(w+1)*(h+1)`), de onde sai o struct de tile com nomes.
- Tabela id→tipo de chão (`ground` 0..0x62): em `.data`, indexada por `byte` do tile; custos de construção/vegetação.
- Structs de cidade, indústria, estação, trem, empresa e jogador (strides 0x39B, 0x196C, 0x4BD, 0x96, 0x27A) e o ponteiro raiz do mundo.
- Avaliador das expressões de eventos/vitória (nomes `GameYear`, `PlayerNetWorth`, ...).
- A rotina de gravação do `.GAM` (para saber o que muda em relação ao `.MAP` e o formato da data/estado).
