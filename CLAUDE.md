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
- Antes de push: `scripts/verify-head.sh` valida o HEAD num worktree isolado (o trabalho não commitado de agentes paralelos não interfere). Contagens do README: `python3 -I scripts/update-readme-counts.py`. Resultados de agentes paralelos: `re/pending/` + `re/scripts/merge_pending.py [arquivos]`. Placar de fidelidade (nível 2, exige Wine): `python3 -I scripts/fidelity.py` (docs/dynamic-analysis.md 5.7).
- Traduções do jogo em `assets/lang/<locale>.toml` (formato em `docs/i18n.md`), publicadas por decisão do usuário (2026-10-22). Texto novo de interface passa pela mesma tabela de strings; valide com `scripts/check-translations.py`.
