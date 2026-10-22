# LNG — tabelas de texto

Arquivos: `default2.lng` (UI, 2807 ids únicos) e `credits2.lng` (texto corrido, sem ids). O jogo monta o nome via `%s.lng` (`0x55b7a0`).

## Sintaxe (texto, CRLF, ASCII nos arquivos em inglês)

- `<id> "<texto>"`, com espaços livres antes, entre e depois.
- Escapes: `\n` (o único usado nos arquivos originais), `\t` e `\<c>` → `<c>`.
- Linhas começando com `;` são comentários. Um comentário `;t<NOME>_0` marca o início de uma tabela indexada no id da próxima linha. Exemplo: `;tCARGO_TYPES_0` seguido de `812 "passenger"` faz `CARGO_TYPES[i]` = id `812 + i`.
- Linhas que não começam com dígito são ignoradas. Existe uma nota de desenvolvedor solta na linha 2860.
- **IDs repetidos:** as expansões (Second Century, Platinum) acrescentam seções no fim do arquivo, e **a última definição vence**. Exemplo: o id 13 termina como "Railroad Tycoon II - Platinum Edition".
- Placeholders no texto: `%1`, `%2`, … Também há nomes de arquivo, como `SH240%1.PCX`, o que indica imagens PCX referenciadas por nome.

Implementação: `crates/rt2-formats/src/lng.rs`.
