# Fluxo de engenharia reversa: dump, classificação e cobertura

## Regenerar

```sh
re/scripts/dump_program.sh          # Ghidra headless: importa, analisa, dumpa (~100 s)
python3 -I re/scripts/classify.py   # lê o dump e re/symbols.toml, escreve re/coverage.toml
```

- `dump_program.sh` roda `/opt/ghidra/support/analyzeHeadless` sobre `re/bin/RT2_PLAT.EXE` (somente leitura), aplica os nomes de `re/symbols.toml` e grava `re/db/program.json` (ignorado pelo git: é derivado do binário). O projeto Ghidra é temporário e fica fora do repositório (`RRT2_GHIDRA_DIR`, padrão: o scratchpad; `ANALYZE_HEADLESS` troca o executável). O script Java está em `re/scripts/ghidra/dump_program.java`.
- Por função, o JSON traz: endereço, tamanho, chamados, chamadores, APIs importadas (`DLL!nome`), strings referenciadas, endereços de dados e se o Function ID da Ghidra a identificou.
- O Ghidra enxerga 2945 funções (1108 são thunks da tabela de saltos do linker incremental); os "3082" do plano vinham de outra contagem. `classify.py` colapsa os thunks: chamadores e chamados são vistos através deles, e cada thunk herda a categoria do alvo.

## O que o `classify.py` faz

Regras na ordem: `lib` (Function ID, região CRT a partir da primeira função FID, Smacker, wrappers Miles pequenos, só APIs de CRT/kernel32), `platform` (user32, winmm, kernel32 de arquivo/registro/tempo, strings de arquivo), `render` (DDRAW/GDI32), `audio` (Miles, strings `.raw`/`.wav`), `ui` (strings `.win`/`.imb`/marcação de texto), `sim` (strings de simulação, só como candidata). A propagação é apenas para cima e por exclusividade (função chamada só por `lib`/`render`/`audio` herda; para `render` e `audio` só a partir de evidência de API). Quem só chama `lib` não vira `lib`. A classificação é uma pista de triagem, não uma prova: `why` registra a evidência.

## Como humanos e agentes editam `re/coverage.toml`

1. Nomeie a função em `re/symbols.toml` (vira `status = "named"` na próxima execução).
2. Para corrigir categoria ou subir o estado (`unknown` → `named` → `spec` → `verified`), edite a entrada em `re/coverage.toml` e adicione `manual = true`. Entradas manuais são preservadas integralmente a cada nova execução do `classify.py`.
3. `spec` exige a função descrita em `docs/spec/` ou `docs/formats/`; `verified` exige teste diferencial (`docs/oracle.md`).
4. Rode `classify.py` de novo para atualizar o resumo no topo do arquivo (contagens por categoria e estado, "sim spec coverage") e copie os números para o README e o `CHANGELOG.md`.

## Exportar o C decompilado (`re/db/decomp/`)

Para estudar o binário em paralelo sem disputar a sessão única do REA, exporte o C de todas as funções:

```sh
re/scripts/export_decomp.sh         # Ghidra headless: importa, analisa, aplica nomes, decompila (~140 s)
```

- `export_decomp.sh` roda `re/scripts/ghidra/export_decomp.java`: aplica de `re/symbols.toml` os nomes de funções e os rótulos de globais/tabelas/strings (com os comentários), decompila cada função não-thunk com `DecompInterface` (timeout de 60 s por função) e grava `re/db/decomp/<addr>_<nome>.c`. Cada arquivo começa com um cabeçalho: endereço, tamanho, categoria/estado de `re/coverage.toml`, chamadores e chamados (por nome).
- Também escreve `INDEX.tsv` (addr, name, category, status, size, file) e `FAILURES.tsv` (funções cuja decompilação falhou; os arquivos delas levam `DECOMPILE FAILED` no cabeçalho). Última execução: 1835 funções exportadas, 2 falhas, 1108 thunks ignorados (2945 no total).
- O diretório é limpo e regerado inteiro a cada execução (não é incremental; leva ~2,5 min). `re/db` é ignorado pelo git (derivado do binário).
- Rode `classify.py` antes se quiser categoria/estado atualizados no cabeçalho.

### Como os agentes usam

1. Explore com `rg`/`grep` sobre `re/db/decomp` (por nome, string, constante, API, `INDEX.tsv` para filtrar por categoria/estado). Exemplo: `rg -l 'rng_state' re/db/decomp`.
2. Confirme detalhes com o REA somente quando necessário (xrefs, disassembly, tipos, anotações).
3. Depois de adicionar nomes a `re/symbols.toml`, rode `re/scripts/export_decomp.sh` de novo para propagar os nomes (nas próprias funções e nas chamadas dos vizinhos).

## Trabalho em paralelo: `re/pending/`

Quando vários agentes trabalham ao mesmo tempo, nenhum deles edita `re/symbols.toml`, `re/coverage.toml`, `CHANGELOG.md` ou os READMEs. Cada um escreve seus resultados em `re/pending/` (ignorado pelo git):

- `<nome>.toml`: entradas `[[symbol]]` (formato de `re/symbols.toml`) e `[[coverage]]` com `addr`, `category`, `status` e `why`
- `<nome>.tsv`: classificação em massa, com cabeçalho `addr, category, confidence, why`
- `<nome>.changelog`: linhas para o `CHANGELOG.md`

Depois, quem coordena roda `python3 -I re/scripts/merge_pending.py`. O script aplica tudo e marca as entradas com `manual = true`, chama o `classify.py` para recalcular o resumo e move os arquivos para `re/pending/merged/`. Conflitos de nome de símbolo são reportados e nunca sobrescritos; o status só sobe (`unknown` < `named` < `spec` < `verified`).
