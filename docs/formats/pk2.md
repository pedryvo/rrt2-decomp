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
- `RT2_09GO.PK2` contém uma estrutura com nomes como `setup.imb`.

## Em aberto

- **Função de hash dos nomes:** é necessária para mapear nomes, como `setup.imb`, para entradas. Deve estar perto da rotina que abre `.\DATA\rt2.PK2` (`0x554300`) ou `.\data\*.pk2` (`0x55d500`).
- **Precedência:** quando o mesmo hash aparece em mais de um PK2, qual vence? Provavelmente o patch (`_06`, `_11PL`).
