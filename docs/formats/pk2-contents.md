# Conteúdo dos PK2 — análise dos blobs

Base: blobs desempacotados dos 4 PK2 (RT2: 2880, RT2_06: 16, RT2_09GO: 1, RT2_11PL: 4 = 2901). Nomes desconhecidos (só hash). Scripts: `re/scripts/lad.py` (decodificador de sprites, gera PNG), `re/scripts/pk2_classify.py` (classificação). Tudo little-endian.

## Tabela de clusters

Classificação feita por estrutura (não só pelos primeiros bytes); assinatura = primeiros bytes mais comuns.

| Cluster | Qtde | Tamanho (B) | Assinatura inicial | Exemplo | Confiança |
|---|---|---|---|---|---|
| **LAD** (sprite/animação indexada, RLE) | 1910 completos + 80 parciais (RT2 1975, RT2_06 12, RT2_11PL 3) | 760 – 2,49 M | `N 03000000 hx ...` (N=1: 957, 8: 453, 4: 351, ...); `N 01000000 ...` (cru) | RT2/0000_e0de26ff | alta |
| **Banco de paletas 16 bit** | 714 | 551 / 1063 / 1575 / 2087 / 16423 / 17959 | `n ffffffff...` (n=1,2,3,4,32,35) | RT2/2244_9beb8d9d, 2167_965ea30d | média-alta |
| **Áudio PCM 16 bit** (sem cabeçalho) | ~123 (RT2 121 + RT2_06 2) | 2,8 K – 429 K | sem assinatura (amostras s16) | RT2/0011_05c06750 | média |
| **IMB** (layout de UI, versão 2000) | 46 (RT2 43, RT2_06 2, RT2_09GO 1) | 191 – 21964 | `d0070000` | RT2/0015_fa6c503b, RT2_09GO/0000 | média |
| Paleta RGB 8 bit | 9 | 768 | `ffffff00 ffffff00 ...` | RT2/1272_bf5a004d | alta |
| LUT 32768 (RGB555 → índice?) | 9 | 32768 | — | RT2/1273_708078a5 | média |
| Tabela de rampa (`u32 len` + dados) | 9 | 7556 / 15108 | `801d0000`, `003b0000` | RT2/1271, 1270 | baixa |
| Tabela hash→tipo de paleta | 1 | 8560 | `c9020000 00000000 ...` | RT2/2073_95966de7 | média |

Os 9 blobs 768 / 32768 / 7556 formam trios consecutivos no RT2 (1272-1296: paleta, LUT, tabela). Contagem total = 2901, tudo classificado.

Distribuição por índice no RT2 (aprox.): 0000-0007 LAD; 0008-0013 áudio; 0014+ LAD/IMB; 0194+ sprites grandes; 0279-0314, 0608-0636, 1396-1461 áudio; 1270-1296 paletas/LUT; 2073 tabela; 2079+ LAD tiles; 2167-2879 bancos de paletas.

## LAD — sprite indexado com RLE (confiança alta)

> Atualização: layout final, regra de paleta (`PALT%04d.PAL`, XRGB1555) e endereços em `docs/formats/lad.md`; as ressalvas e itens "em aberto" abaixo sobre paleta, `hx`, `nlayers` e os 80 parciais estão resolvidos lá.

Estrutura (confirmada: 1910 arquivos são consumidos exatamente até o último byte, sem sobra):

```
u32 N                       // nº de registros (frames/direções: 1, 4 ou 8 na maioria)
N x registro:
  60 bytes de cabeçalho: u32 tipo (3 = RLE, 1 = cru), u32 hx (700, 413, 465, 28..., id/parâmetro ainda desconhecido),
                         14 x u32 em pares (0,1)/(0,0xFFFFFFFF)  -> slots de camada presentes/ausentes?
  u32 tag                   // 0x82xxxxxx, parece lixo de ponteiro de memória (varia entre arquivos)
  u32 nlayers               // costuma ser 2; nem sempre bate com o nº real de camadas
  camadas (até falhar o cabeçalho; ex.: 2 = imagem + sombra, até 198 em sprites grandes):
    [u32 k, u8 cnt, cnt bytes 0..cnt-1]   // opcional (formas "01 00 00 00 01 00" ou "11 00 00 00 12 00 01..11")
    u8 flag (0x2a, 0x66, 0x02, ...)       // valor variável, semântica desconhecida
    u32 0, u32 ltype (3 = RLE, 2 = cru)
    i32 x, i32 y, i32 largura, i32 altura   // NÃO é x0,y0,x1,y1: largura/altura batem com nº de linhas
    u32 tag2, u32 size, dados[size]
```

Bounding box = (x, y, largura, altura) com origem relativa ao hotspot (valores negativos comuns). Evidência: ponte 106x97 decodifica exatamente 98 linhas; base 834x542 decodifica 542 linhas.

RLE (por linha), confirmado visualmente:
- `[len][tokens...]`; `len` inclui o próprio byte e o terminador `00`. Se `len & 0x80`, o comprimento ocupa 2 bytes (big-endian, 15 bits) — necessário em linhas longas (sprites > 255 B/linha).
- token `>= 0xC0`: pula `(b - 0xC0)` pixels transparentes (0..63; vários seguidos somam-se: `ff ff ff c5` = 63*3+5).
- token `1..0xBF`: `n` literais (índices de paleta de 8 bit) seguem.
- `00` encerra a linha; skip final à direita é opcional.
- Camada tipo 2: `w*h` bytes crus (ex.: tile 32x32 de máscara com índices 0/1/4, RT2/2079).

Confirmado visualmente (PNG no scratchpad, paleta RGB `1272_bf5a004d`):
- `RT2/0000_e0de26ff` rec 0 camada 0: ponte ferroviária isométrica (tabuleiro, trilhos, pilar) — `scratchpad/png/sheet2.png`.
- `RT2/0194_49d0f71f`: base/estação sci-fi 834x542 com cúpula, torre e painéis (usa linhas > 255 B) — `scratchpad/png/b0194.png`.
- Tile 32x32 `RT2/2079`: máscara losango (diamante isométrico).

Ressalvas: cores com ruído (pontos vermelhos/magenta) porque a paleta correta de cada sprite não é conhecida (provavelmente depende da tabela de paletas/estação e de índices de remapeamento tipo cor de jogador). 80 arquivos ainda não fecham no parser (variações do cabeçalho de registro ou `size` menor que os dados reais, ex.: `RT2/0145`, `0193`, `0025`).

## Bancos de paleta 16 bit (confiança média-alta)

`u32 n` + 35 bytes de tabela de slots (cada byte = índice da paleta ou `0xFF`; para n=32 é `00..1f ffffff`, para n=35 `00..22`) + `n` x 512 bytes (256 x u16 little-endian). Tamanho = 39 + 512·n em 100% dos 714 casos. Formato de pixel provável RGB565 (rampas verdes de terreno coerentes, `0x7c1f` magenta de chave no fim); índice 0 é especial (`0x0196`, `0x0044`). Rampas escuras (`0x3def`→`0x0800`) sugerem tabelas de iluminação/estação, por isso 4 paletas (n=4) e 3 (n=3) por blob. Distribuição: n=3: 343, n=1: 169, n=4: 165, n=32: 25, n=2: 8, n=35: 3.

Tabela `RT2/2073_95966de7`: `u32 713` + 713 x `(u32 0, u32 hash, u32 tipo)`; contagem por `tipo` casa exatamente com o nº de blobs por tamanho (97→343, 1→169, 29→165, 3→25, 129→8, 31→3). O `hash` não aparece no diretório PK2 (outra função de hash ou outro espaço de nomes). Provável mapa nome→tipo de paleta.

## Paleta RGB / LUT (confiança alta / média)

768 B = 256 x RGB 8 bit; começa com as 10 cores estáticas do Windows e termina com as 10 finais (índices 0xF6-0xFF = `fffbf0`, `a0a0a4`, `808080`, `ff0000`...). Usada para renderizar os sprites acima. LUT de 32768 B: tamanho = 32³, provavelmente RGB555→índice (não testado). Tabela de 7556 B (`u32 7552` + dados, primeiros bytes crescentes `28 41 56 69 80 99`) e 15108 B: rampa/fade de 59 níveis? (hipótese fraca).

## Áudio (confiança média)

> Atualização: formato resolvido em `docs/formats/sound.md`. São 123 efeitos PCM cru, mono, 16 bit a 11025 Hz (`clock82m.raw` é 8 bit a 22050 Hz), descritos pela tabela de 0x12 bytes em `0x55e7b0`; os "39 de ruído" são sons de banda larga e a hipótese de 22050 Hz/16 kHz abaixo está errada.

Heurística: 87 de 126 blobs têm autocorrelação lag-1 > 0,5 como s16le mono (ex.: `0011` 0,98; `0288` 0,99; `0290` 0,99; `0631` 0,97), mas os 39 restantes têm autocorrelação baixa/negativa e energia alta em HF (ruído tipo vapor/vento, ou outro codec): não resolvido. Nenhum cabeçalho (`RIFF` aparece só por acaso em `2148`/`2228`). Sem metadado de taxa: palpite 22050 Hz mono 16 bit (conversão `ffmpeg -f s16le -ar 22050 -ac 1` dá durações de 0,6-2 s e nível médio -10..-22 dB, plausível). RT2_06 `0001` = 320000 B = 10,0 s a 16 kHz exatos (alternativa: 16 kHz). Amplitudes: desvio 200-17000.

## IMB (confiança média)

`u32 0x7D0` (versão 2000) + campos u32 + strings com prefixo de comprimento `u32 len` (inclui `\0`), p.ex. `0d000000 "bpaddtr2.imb\0"`. Strings mais frequentes: `setupb.imb`, `setupp.imb`, `setup.imb`, `trainbuy.imb`, `filltext.imb`, `odbottom.imb`, `trndbut.imb`, `mainbut.imb`... São definições de tela/painel de UI referenciando bancos de imagem `.imb` (que não existem soltos; provavelmente são os LAD do PK2). `RT2_09GO/0000` e `RT2_06/0014` têm 21964 B, igual ao maior do RT2 (`setup.imb`).

## Arquivos soltos (rápido)

- `data2/Miles/*.m3d`, `*.flt`: DLLs PE (`MZ`) do Miles Sound System; sem relação com PK2.
- `maps/*.MAP`, `games/*.GAM`: cabeçalho `3c23 0000` (0x233C) + nome ASCII; contêm strings e muitos `ffffffff`, mas nenhum padrão LAD (`01 00 2a 00 00 00 00 03` ausente). Outro formato de serialização.
- `anim/*.smk` (Smacker), `*.mp2/*.mp3`: formatos padrão, não vêm do PK2.

## Em aberto

1. Semântica de `hx`, `tag`/`tag2`, `flag`, pares de slots e `nlayers` no LAD; origem exata (hotspot) e ordem de camadas (imagem x sombra).
2. 80 LAD parciais (variações de cabeçalho/`size`).
3. Qual paleta cada sprite usa (tabela `2073`, hash dos nomes, ou campo do cabeçalho) e como funcionam os 35 slots e a LUT 32768.
4. Áudio: taxa de amostragem, estéreo/mono e os 39 blobs de baixa correlação (talvez outro codec).
5. Função de hash dos nomes (ver `pk2.md`) para ligar `.imb` e demais nomes a blobs.
