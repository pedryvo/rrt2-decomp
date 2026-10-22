# railroad — clone moderno de Railroad Tycoon II em Rust

## Regras
- Os arquivos originais do jogo ficam em `/home/pedryvo/Games/railroad-tycoon-2` e são SOMENTE LEITURA. Nunca modificar, mover ou gravar nada lá.
- Todo o trabalho vive nesta pasta.
- Assets extraídos do jogo vão para `assets-extracted/`, que fica no .gitignore e não é versionado.
- Commits seguem Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `build:`), com escopo quando fizer sentido, por exemplo `feat(pk2): ...`. Commits pequenos e atômicos ao longo do trabalho.
- Toolchain Rust fixado localmente por `rust-toolchain.toml` e `mise.toml`. Não mexer na configuração global do mise.
