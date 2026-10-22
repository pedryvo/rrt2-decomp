# railroad — clone moderno de Railroad Tycoon II em Rust

## Regras
- Os arquivos originais do jogo ficam em `/home/pedryvo/Games/railroad-tycoon-2` e são SOMENTE LEITURA. Nunca modificar, mover ou gravar nada lá.
- Todo o trabalho vive nesta pasta.
- Assets extraídos do jogo vão para `assets-extracted/`, que fica no .gitignore e não é versionado.
- Commits seguem Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `build:`), com escopo quando fizer sentido, por exemplo `feat(pk2): ...`. Commits pequenos e atômicos ao longo do trabalho.
- Toolchain Rust fixado localmente por `rust-toolchain.toml` e `mise.toml`. Não mexer na configuração global do mise.
- O `README.md` (inglês, padrão) e o `README.pt-BR.md` são a vitrine pública do projeto e devem ficar sincronizados: ao concluir um marco (formato decodificado, spec escrita, crate novo), atualize a tabela de progresso, a contagem de funções nomeadas e o roteiro, no mesmo commit ou num `docs(readme): ...` logo em seguida.
- O repositório é público (github.com/pedryvo/rrt2-decomp): nunca versionar assets ou trechos binários do jogo original. Fazer push depois dos commits.
- Toda mudança visível ao usuário ou descoberta de engenharia reversa ganha uma linha em `CHANGELOG.md` (`- Feature|Improved|Change|Fix|Docs|RE: texto`), checada no CI por `scripts/check-changelog.sh`.
- Fidelidade: a mecânica deve ser idêntica ao original. O executável é a referência; o manual é só pista (docs/PLAN.md, seção 11).
