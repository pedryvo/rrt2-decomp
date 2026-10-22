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
