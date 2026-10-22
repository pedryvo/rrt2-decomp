# LAD — bancos de sprites (`.imb`) e paletas (`PALT%04d.PAL`)

Implementação: `crates/rt2-formats/src/lad.rs` e `palette.rs`; ferramenta: `rt2-sprite`. Little-endian. Os 1990 blobs LAD dos PK2 (RT2 1975, RT2_06 12, RT2_11PL 3) são consumidos exatamente até o último byte, com 30952 imagens decodificáveis (teste `lad_blobs_parse_exactly_and_decode`). Os "80 parciais" do parser heurístico eram só o formato mal interpretado.

Catálogo de todos os bancos (nome, categoria, camadas, geometria do tile): `sprite-catalog.md`.

## Origem (RT2_PLAT.EXE)

| Endereço | Função |
|---|---|
| `0x4f48e0` | `pk2_get_item(tipo, nome)`; tipo 0 = banco de sprites LAD, 1 = paleta, 2 = fonte. Thunks `0x4f4ba0` / `0x4f4bb0` / `0x4f4bc0` |
| `0x4f8b80` | `lad_load`: lê o banco (tipo 0) |
| `0x4f0f80` | `lad_get_image(registro, grupo, quadro)` |
| `0x4f8ea0` / `0x4f8ed0` | construtor / leitor da paleta (tipo 1) |
| `0x55e5e0` | `"PALT%04d.PAL"` |
| `0x4f24c0` | construtor de fonte (tipo 2): carrega `copp6.imb` ... `htra46.imb` por tipo 0 |

## Layout do banco

```
u32 N                      // o loader usa só os 16 bits baixos
N x registro (0x44 bytes na memória):
  u32 kind                 // 3 (11172 regs) ou 1 (842)
  u32 palette_id           // número do banco PALT%04d.PAL (0x4f8d1a); ver abaixo
  u32[13]                  // pares 0/1 ou 0/-1; em memória o loader escreve o ponteiro da paleta em +8
  u32 tag                  // lixo de ponteiro (0x82xxxxxx), sem uso
  u32 G                    // nº de grupos
  G x grupo:
    u32 k, u16 cnt         // k = tamanho do mapa de quadros
    u8[k]                  // só se k >= 2: quadro f mostra imagem mapa[f % k]
    cnt x imagem:
      u8 flags             // bit 1 (0x02): usa a paleta do registro (0x2a e 0x66 são os comuns)
      u32 0
      u32 encoding         // 3 = RLE por linha, 2 = bytes crus w*h
      i32 x, y, w, h       // bounding box relativa ao hotspot
      u32 tag2             // lixo
      u32 size, u8[size]
```

Antes (`re/scripts/lad.py`) o "nlayers" era tratado como nº de camadas e `[u32 k, u8 cnt, ...]` como opcional; o formato real é grupos de imagens com mapa de quadros (`k`, `cnt` u16). Imagens de 1x1 com dado `01 c0` (linha degenerada) existem; decodificam como transparentes.

RLE por linha: `[len][tokens]`; `len` conta o próprio byte e o `00` final (2 bytes big-endian de 15 bits se o bit 7 estiver ligado); token `>= 0xC0` pula `token-0xC0` px, token `1..0xBF` é seguido de `n` índices, `00` encerra a linha. Linha `01` = vazia (1 byte).

## Regra da paleta (confiança alta)

Para cada registro, o loader (`0x4f8d05..0x4f8d3d`) testa o bit 1 do `flags` da primeira imagem do primeiro grupo; se ligado, monta `sprintf("PALT%04d.PAL", registro.palette_id)` (o campo que o parser antigo chamava `hx`) e pede o item tipo 1. O hash é o hash PK2 normal do nome: `PALT0000.PAL`...`PALT0712.PAL` são exatamente os blobs 2167..2879 do RT2.PK2 (em ordem; a tabela `2073` lista o tipo de cada um), e ids acima disso estão nos PK2 de patch. Todo registro com flag de paleta de todos os PK2 resolve. Validado visualmente: ponte `RT2/0000` (PALT0700) e base sci-fi `RT2/0194` (PALT0424) saem com cores corretas.

### Banco de paleta

```
u32 n
u8[35]                 // tabela de slots: índice da paleta ou 0xff (loader: cópia para +0xf)
n x u16[256]           // XRGB1555: R = bits 10-14, G = 5-9, B = 0-4
```

Os arquivos guardam sempre 1555 (o parser anterior assumindo RGB565 estava errado); `palette_load` converte para o formato de tela (`DAT_0055d228`: 0 = 565, 1 = 555, 6 = LUT de 8 bit via `DAT_0057b3f8`, que é o papel das LUTs de 32 KB). Sem seleção em tempo de desenho conhecida, o renderizador usa a paleta 0 do banco (cores corretas).

## Em aberto

- Qual das `n` paletas do banco o jogo escolhe em cada situação (estação do ano, iluminação, cor do jogador?) e o papel exato dos 35 slots (o loader zera a entrada 0 de alguns slots na conversão).
- Semântica de `kind` (1 vs 3), dos pares 0/1 do cabeçalho e dos valores de `flags` além do bit 1.
- Semântica do `x, y` (hotspot) e como os grupos se combinam (imagem + sombra?).
- `palette_id` > 712 (bancos nos PK2 de patch) não foram renderizados individualmente.

## Nota: tokens extras no RLE

O desenhador de texto (`0x4fff00`, ver `font.md`) decodifica `0x80..0x9f` (escurecer o destino) e `0xa0..0xbf` (corrida de alfa) em vez de "literais"; `lad.rs::decode` ainda os trata como literais, o que só importa para imagens que usem esses tokens (fontes: use `font::decode_text_image`). Resta confirmar se os sprites de mundo usam o mesmo conjunto.
