# railroad — clone moderno de Railroad Tycoon II em Rust

## Regras
- Os arquivos originais do jogo ficam em `/home/pedryvo/Games/railroad-tycoon-2` e são SOMENTE LEITURA. Nunca modificar, mover ou gravar nada lá.
- Todo o trabalho vive nesta pasta.
- Assets extraídos do jogo vão para `assets-extracted/`, que fica no .gitignore e não é versionado.
- Commits seguem Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `build:`), com escopo quando fizer sentido, por exemplo `feat(pk2): ...`. Commits pequenos e atômicos ao longo do trabalho.
- Toolchain Rust fixado localmente por `rust-toolchain.toml` e `mise.toml`. Não mexer na configuração global do mise.
- READMEs públicos em seis idiomas, que devem ficar sincronizados: `README.md` (inglês, padrão), `README.pt-BR.md`, `README.es.md`, `README.de.md`, `README.zh-CN.md`, `README.ru.md`. Ao concluir um marco (formato decodificado, spec escrita, crate novo), atualize a tabela de progresso, as contagens (`scripts/update-readme-counts.py` cobre os seis) e o roteiro, no mesmo commit ou num `docs(readme): ...` logo em seguida.
- O repositório é público (github.com/pedryvo/rrt2-decomp): nunca versionar assets ou trechos binários do jogo original. Fazer push depois dos commits.
- Toda mudança visível ao usuário ou descoberta de engenharia reversa ganha uma linha em `CHANGELOG.md` (`- Feature|Improved|Change|Fix|Docs|RE: texto`), checada no CI por `scripts/check-changelog.sh`.
- Fidelidade: a mecânica deve ser idêntica ao original. O executável é a referência; o manual é só pista (docs/PLAN.md, seção 11).
- Antes de push: `scripts/verify-head.sh` valida o HEAD num worktree isolado (rode sem pipe, ele já imprime só o resumo e termina com `verify-head: OK`; um `| tail` mascara a falha) (o trabalho não commitado de agentes paralelos não interfere). Contagens do README: `python3 -I scripts/update-readme-counts.py`. Resultados de agentes paralelos: `re/pending/` + `re/scripts/merge_pending.py [arquivos]`. Placar de fidelidade (nível 2, exige Wine): `python3 -I scripts/fidelity.py` (docs/dynamic-analysis.md 5.7).
- Traduções do jogo em `assets/lang/<locale>.toml` (formato em `docs/i18n.md`), publicadas por decisão do usuário (2026-10-22). Texto novo de interface passa pela mesma tabela de strings; valide com `scripts/check-translations.py`.
- Recursos da máquina: compile e teste sempre com `nice -n 15` (ex.: `nice -n 15 cargo nextest run -p <crate>`); o `.cargo/config.toml` limita a 4 jobs e o mold a 2 threads, e o nextest a 2 threads. Rode testes só dos crates que mudou; a suíte completa fica para o `scripts/verify-head.sh` do coordenador.
- Arquivos grandes de trabalho (sandboxes e prefixos do Wine, extrações de PK2, saves gerados, screenshots em lote) vão em `~/.cache/rrt2/`, não no `/tmp`: o `/tmp` é um disco em RAM de ~6,7 GB, compartilhado pela sessão, e já encheu uma vez.
- Ferramentas de interface (construção, diálogos, seleção no mapa) precisam de teste que passe pelo caminho real de entrada: eventos sintéticos de mouse e teclado roteados como numa janela, não só chamadas diretas aos comandos. Em 2026-10-23 o arrastar de trilho entre estações passava nos testes de comando, mas não funcionava para o usuário.
