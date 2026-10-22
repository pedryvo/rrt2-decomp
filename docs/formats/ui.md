# Layouts de UI (`*.win`)

Implementação: `crates/rt2-formats/src/ui.rs`; ferramenta: `rt2-ui` (`rt2-ui filerq.win -o filerq.png [--dump]`). Teste contra os dados originais: `crates/rt2-formats/tests/ui_font.rs` (todo blob `0x7d0` dos 4 PK2, 46 janelas, parseia até o último byte).

## Quem é quem

- `.win` = **layout de janela** (o que o `pk2.md` chamava de "IMB, versão 2000": os 46 blobs que começam com `d0 07 00 00` são exatamente os `.win`).
- `.imb` = **banco de sprites LAD** (`lad.md`); não há outro formato `.imb` de layout. Um controle referencia seu banco por nome (`bpmisc.imb`, `setupb.imb`, ...).
- Carregador: `ui_window_load` (`0x507600`, `thiscall(janela, x, y, nome)`); controles em `ui_*_new` (`0x50c850` ...); ids de ação são tratados por funções `switch` por tela (ex.: o menu inicial em `0x40234c`, ids `0xc3f`..).

## Layout do arquivo (little-endian)

Fluxo de structs brutas separadas por marcadores de 4 bytes:

```
u32 0x7d0                      início da janela
u8[65]                         struct da janela (abaixo)
u32 0xbbe, u32 len, u8[len]    nome de banco de fundo (vazio em todos os arquivos)
u32 0x7d1                      início da lista de controles
repetir:
  u32 0x7d2                    início do controle (ou 0x7d4: fim da janela)
  u32 tipo                     1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200
  u32 0xbbf, len, bytes        string de runtime (sempre vazia nos arquivos)
  u32 0xbb8, len, bytes        string de runtime (sempre vazia)
  u32 0xbb9, len, bytes        banco de sprites (.imb) do controle, "" se não há
  u8[N(tipo)]                  struct do controle: base de 53 bytes + cauda por tipo
  [u32 tag, len, bytes]        string extra, só nos tipos 1 (0xbba), 2 (0xbbb), 8 (0xbc0), 0x100 (0xbc1)
  u32 0x7d3                    fim do controle
u32 0x7d4
```

Campos `[tag][len][bytes]` são lidos por `0x4f4220` (tag é só consumida, len inclui o NUL). O loader lê 65 + N bytes crus (`0x4f40e0`) e copia para o objeto; a ordem de desenho vem do byte z (lista ordenada em `0x5075a0`).

### Struct da janela (65 bytes)

| offset | tipo | campo |
|---|---|---|
| 9 | u32 | largura (1024 em telas cheias; 458..774 em diálogos) |
| 13 | u32 | altura (768 em telas cheias) |
| 33, 37, 41, 45 | u32 | área cliente (x, y, w, h) dentro da moldura; 0 em telas cheias |
| 49 | u32 | 1 = diálogo emoldurado (centrado na tela), 0 = camada de tela cheia |
| 53 | u32 | número da janela / base de ids (ex.: 4700 em `campaign.win`, 3204 em `setup.win`), 0 nos diálogos |
| resto | | zero em todos os arquivos (x, y de posição preenchidos pelo chamador em runtime) |

### Base do controle (53 bytes, `0x50eaf0`)

| offset | tipo | campo |
|---|---|---|
| 0 | u16 | **id de ação** (também chave de `ui_find_control` `0x507000`); ex.: botões do menu inicial 3135..3140 |
| 2 | u16 | tipo (repete o tipo do arquivo) |
| 4 | u8 | ordem de desenho (z, crescente) |
| 5 | u16 | id de grupo/vínculo (0 se nenhum) |
| 7, 9, 11, 13 | u16 | retângulo **x, y, w, h** em pixels originais, relativo à janela (a área 1024x768 é a tela de design) |
| 35 | u8 | número da fonte (`font.md`; 0 = 2) |
| 45..49 | u32 | lixo de ponteiro de runtime (`0x83..`) |
| demais | | flags não decodificadas (guardadas cruas em `Control::base`) |

### Tipos de controle (confiança: classe pelo constructor e por onde o banco é usado)

| tipo | classe | struct | extra | notas |
|---|---|---|---|---|
| 0x1 | botão (`0x50c850`) | 0x52 | 0xbba | cauda: `+0` registro do sprite "solto", `+4` registro "apertado" (u16), `+6` tecla de atalho (ASCII), `+14..22` retângulo do rótulo (x,y,w,h) dentro do botão; ex.: `setup.win` S/M/E/H/C/Q |
| 0x2 | rótulo de texto (`0x509960`) | 0x4d | 0xbbb | usa a fonte do controle; o texto vem do código/LNG em runtime |
| 0x4 | imagem (`0x50afe0`) | 0x3f | | `+0` (u16) = registro do banco; o fundo de cada diálogo é uma imagem de 1 sprite grande (ver `filerq`) |
| 0x8 | lista com barra (`0x5047f0`) | 0x91 | 0xbc0 | banco de preenchimento de linhas (`filltext.imb`); banco principal `*lb.imb` |
| 0x10 | área sensível (`0x508480`) | 0x35 | | só a base |
| 0x20 | (`0x5088f0`) | 0x45 | | finalidade não identificada (`mainwin`, `traindtl`) |
| 0x40 | superfície de vídeo (`0x503790`) | 0x45 | | só em `smackwi*.win` (Smacker) |
| 0x80 | barra/valor (`0x50a890`) | 0x4f | | faixas de rolagem (`campaign.win`, 500 por padrão) |
| 0x100 | desenhado pelo dono (`0x507cf0`) | 0x61 | 0xbc1 | banco `buildbuy.imb` em `eventset.win` |
| 0x200 | slider (`0x50b8d0`) | 0x55 | | banco `horsb_*.imb` (`settings.win`) |

Caudas além das do botão e da imagem ficam cruas (`Control::tail`).

## Ajustes do loader após ler

`ui_window_load` ainda ajusta por nome de janela (tabela de ponteiros `0x55fe24`..`0x55fed8`: lista de `.win` conhecidos, com ids `<= 0x2b` e registros extras de rotas `0x7c`/`0x7d` via `0x507400`), mas isso é lógica de tela, não do formato. O texto dos rótulos/botões e a lista de itens são preenchidos pelo código de cada tela; **os arquivos não contêm ids de string do LNG**, só ids de ação (os valores 3135.. do `setup.win` coincidem numericamente com strings do LNG por acaso).

## Prova visual (geometria)

`rt2-ui filerq.win -o filerq.png` (janela de 562x572 centrada em 1024x768): a imagem de moldura (`filerq.imb`, registro 0) cai exatamente na janela, a lista (id 401, 431x321 em (95,70), banco `filerqlb.imb`, rosa) ocupa o painel interno, o rótulo (405) a faixa verde sobre o campo de nome e os dois botões (406/407, `filerq.imb` registros do "up") os botões inferiores. Arquivo gerado fora do repositório (arte do jogo; não versionar). `credits.win` (3 controles) mostra a tela cheia com o rótulo de rodapé.

## Cobertura

46 de 46 janelas do PK2 (todas as `.win` nomeadas no EXE; `buygold.win` e `setupl.win` só são testadas por existência) parseiam exatamente até o marcador `0x7d4`, 1000+ controles. Campos decodificados: tamanho, área cliente, tipo, id, z, grupo, retângulo, fonte, banco, extra, botão (frames, atalho, rótulo) e imagem (registro).

## Em aberto

- Caudas dos tipos 0x2, 0x8, 0x20, 0x80, 0x100, 0x200 e as flags da base (alinhamento do texto, cores, estados).
- Qual frame do banco cada controle usa em cada estado (hover, desabilitado) além de "cima"/"baixo" do botão.
- Os controles de tipo 0x20 e 0x80 (barras) e como o conteúdo das listas é preenchido (código de cada tela).
- Ordem/ajustes por nome em `ui_window_load` (tabela de `0x55fe24`) e a relação das janelas `setupl/setupg/setup/set.win` (o mesmo blob de 21964 bytes em três nomes).
