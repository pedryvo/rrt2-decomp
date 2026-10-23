# Empacotamento

Como o `rrt2` é distribuído e como cada pacote é gerado. Nenhum pacote contém arquivos do jogo
original: o jogador aponta o `rrt2` para a própria instalação (`rrt2 set-path`, `--game-dir` ou
`RT2_DIR`), que é só lida.

## 1. O que o binário precisa em tempo de execução

- **Nada além dele mesmo.** As fontes (`assets/fonts`, `include_bytes!` em `rt2-ui`) e as
  traduções (`assets/lang/*.toml`, embutidas pelo `build.rs` do `rt2-game`) vão dentro do
  executável. Não existe pasta de dados do projeto a localizar.
- Traduções extras ou corrigidas, sem recompilar: `RRT2_LANG_DIR=<pasta>` ou uma pasta `lang/` ao
  lado do executável (`locales.rs`, docs/i18n.md). Dentro do AppImage e do Flatpak use a variável.
- Configuração em `dirs::config_dir()/rrt2/config.toml` (ou `RRT2_CONFIG`), saves em
  `dirs::data_dir()/rrt2/saves` (ou `RRT2_SAVES`), recordes em `dirs::data_dir()/rrt2/hiscore.dat`.
- Bibliotecas: só `libasound.so.2` e a glibc são ligadas; Vulkan, Wayland, X11 e xkbcommon são
  abertos com `dlopen` pelo wgpu e pelo winit.
- Vídeos (`anim/*.smk`): os programas `ffmpeg`, `ffprobe` e `ffplay` do sistema. Sem eles o clipe
  é ignorado e o jogo segue normalmente.

## 2. Artefatos de cada release

O workflow `.github/workflows/release.yml` roda em cada push na `main` (builds de
desenvolvimento, só como artefatos do run, rótulo `nightly-<commit>`) e em cada tag `v*` (publica a
release com todos os arquivos abaixo).

| Arquivo | Job | Como é gerado |
|---|---|---|
| `rrt2-<v>-linux-x86_64.tar.gz` | `build` (Linux) | `scripts/package-release.sh` |
| `rrt2-<v>-x86_64.AppImage` | `build` (Linux) | `scripts/package-appimage.sh`, mesmo binário do tarball |
| `rrt2-<v>-x86_64.flatpak` | `flatpak` | `packaging/flatpak/io.github.pedryvo.rrt2.yml`, compilado do código no SDK |
| `rrt2-<v>-windows-x86_64.zip` | `build` (Windows) | `scripts/package-release.sh` |
| `rrt2-<v>-macos-{aarch64,x86_64}.tar.gz` | `build` (macOS) | `scripts/package-release.sh` |

Arquivos de integração com o desktop, compartilhados pelo AppImage e pelo Flatpak:
`packaging/linux/io.github.pedryvo.rrt2.desktop` e
`packaging/linux/io.github.pedryvo.rrt2.metainfo.xml` (AppStream, licença `GPL-3.0-or-later`,
metadados `CC0-1.0`). O ícone é o logo do projeto, `docs/assets/logo.svg` (arte própria, não do
jogo), instalado como `io.github.pedryvo.rrt2.svg`. O job Linux valida os dois arquivos com
`desktop-file-validate` e `appstreamcli validate --no-net`.

## 3. AppImage

`scripts/package-appimage.sh <rótulo> [target-dir] [out-dir]` monta um AppDir com `usr/bin/rrt2`,
o `.desktop`, o metainfo, o ícone, os docs e as licenças OFL das fontes, e chama o
`appimagetool` 1.9.1 com o runtime tipo 2 `20251108`, ambos fixados por versão e sha256 e guardados
em `~/.cache/rrt2/appimage-tools`. `APPIMAGE_EXTRACT_AND_RUN=1` dispensa FUSE no CI. Não precisa
de rede depois do primeiro download nem compila nada: dá para rodar localmente sobre um binário já
compilado.

O `AppRun` só executa `usr/bin/rrt2` com os argumentos recebidos, então todos os subcomandos
funcionam:

```bash
chmod +x rrt2-0.3.0-x86_64.AppImage
./rrt2-0.3.0-x86_64.AppImage set-path ~/Games/railroad-tycoon-2   # uma vez
./rrt2-0.3.0-x86_64.AppImage
```

Nada é empacotado além do binário: a glibc e a `libasound` vêm do sistema. O binário é compilado no
`ubuntu-latest` do GitHub (Ubuntu 24.04), então exige glibc 2.39 ou mais nova. Os vídeos usam o
`ffmpeg` do sistema, como no tarball. Sem FUSE no sistema, rode com `--appimage-extract-and-run`.

## 4. Flatpak

Manifesto: `packaging/flatpak/io.github.pedryvo.rrt2.yml`. Runtime `org.freedesktop.Platform`
26.08, SDK com a extensão `rust-stable`. O módulo copia o checkout (sem `target/`, `.git/`, `re/`),
apaga a cópia do `.cargo/config.toml` (que pede clang e mold, ausentes no SDK) e roda
`cargo build --release --locked -p rt2-game`. O cargo baixa as dependências durante o build
(`--share=network` nos `build-args`); isso serve para o nosso bundle, mas uma submissão ao Flathub
exigiria vendorizar as crates (`flatpak-cargo-generator`). No CI, o job `flatpak` usa a imagem
`ghcr.io/flathub-infra/flatpak-github-actions:freedesktop-26.08` e a action
`flatpak/flatpak-github-actions/flatpak-builder@v6.8`, e grava o commit em `RRT2_GIT_COMMIT` no
manifesto antes do build (`rrt2 --version`).

Permissões (`finish-args`): `wayland`, `fallback-x11`, `ipc`, `dri` e `pulseaudio` (o runtime
encaminha o ALSA para o PulseAudio/PipeWire). **Nenhum acesso a arquivos do host por padrão.** O
`rrt2` ainda não tem seletor de pasta na interface, então o portal de arquivos não se aplica; o
mínimo é liberar, só leitura, a pasta do jogo que o jogador escolher:

```bash
flatpak install --user rrt2-0.3.0-x86_64.flatpak     # puxa o runtime do Flathub se faltar
flatpak override --user --filesystem="$HOME/Games/railroad-tycoon-2":ro io.github.pedryvo.rrt2
flatpak run io.github.pedryvo.rrt2 set-path "$HOME/Games/railroad-tycoon-2"
flatpak run io.github.pedryvo.rrt2                   # ou pelo menu de aplicativos
```

A pasta aparece no mesmo caminho dentro do sandbox, então o `set-path` grava o caminho de verdade.
Para revogar: `flatpak override --user --reset io.github.pedryvo.rrt2`. Instalação do Steam:
use o caminho da pasta do jogo dentro de `steamapps/common`. Configuração e saves ficam em
`~/.var/app/io.github.pedryvo.rrt2/config/rrt2` e `~/.var/app/io.github.pedryvo.rrt2/data/rrt2`.

Para compilar localmente (pesado, compila o projeto inteiro):

```bash
flatpak-builder --user --install-deps-from=flathub --force-clean --install \
    ~/.cache/rrt2/flatpak-build packaging/flatpak/io.github.pedryvo.rrt2.yml
```

Lacunas conhecidas:

- O runtime não traz os programas `ffmpeg`/`ffprobe`/`ffplay`, então o Flatpak não toca os vídeos
  (intro, vitória e derrota); o resto do jogo funciona. Resolver exige um módulo de ffmpeg mínimo
  (decodificador Smacker, `ffplay` com SDL2) no manifesto.
- Sem seletor de pasta, a primeira execução pelo menu sem `set-path` só mostra o erro no terminal.
  Um seletor via `org.freedesktop.portal.FileChooser` (modo pasta) dispensaria o `override`.
- A janela não define `app_id`, então alguns docks Wayland podem não associar a janela ao ícone
  (o `.desktop` declara `StartupWMClass=rrt2`).
