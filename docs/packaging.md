# Empacotamento

Como o `rrt2` é distribuído e como cada pacote é gerado. Nenhum pacote contém arquivos do jogo
original: na primeira execução o `rrt2` abre a tela de pasta do jogo (detecta Steam, GOG, Lutris,
Wine e deixa escolher a pasta; também há `rrt2 set-path`, `--game-dir` e `RT2_DIR`), e a
instalação é só lida.

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
| `rrt2-<v>-windows-x86_64.zip` | `build` (Windows) | `scripts/package-release.sh` (portátil) |
| `rrt2-<v>-windows-x86_64-setup.exe` | `build` (Windows) | `scripts/package-windows-installer.sh`, Inno Setup (seção 5) |
| `rrt2-<v>-winget-manifests.zip` | `build` (Windows, só tags `vX.Y.Z`) | `scripts/fill-winget.sh` (seção 6) |
| `rrt2-<v>-macos-{aarch64,x86_64}.tar.gz` | `build` (macOS) | `scripts/package-release.sh` |
| `rrt2-<v>-macos-{aarch64,x86_64}.dmg` | `build` (macOS) | `scripts/package-macos-app.sh`, `RRT2.app` (seção 7) |

Arquivos de integração com o desktop, compartilhados pelo AppImage e pelo Flatpak:
`packaging/linux/io.github.pedryvo.rrt2.desktop` e
`packaging/linux/io.github.pedryvo.rrt2.metainfo.xml` (AppStream, licença `GPL-3.0-or-later`,
metadados `CC0-1.0`). O ícone é o logo do projeto, `docs/assets/logo.svg` (arte própria, não do
jogo), instalado como `io.github.pedryvo.rrt2.svg`. O mesmo logo vira o `rrt2.ico` do Windows
(seção 5) e o `rrt2.icns` do macOS (seção 7). O job Linux valida os dois arquivos com
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
./rrt2-0.3.0-x86_64.AppImage                                      # a primeira execução pergunta a pasta
./rrt2-0.3.0-x86_64.AppImage set-path ~/Games/railroad-tycoon-2   # ou, sem a tela, uma vez
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
encaminha o ALSA para o PulseAudio/PipeWire). **Nenhum acesso a arquivos do host, e nenhum
`flatpak override` é preciso.** A tela de pasta do jogo abre o seletor nativo pelo
`org.freedesktop.portal.FileChooser` (crate `rfd` com o backend `xdg-portal`, sem GTK), que o
sandbox já alcança; o portal libera o acesso à pasta escolhida e devolve um caminho estável em
`/run/user/<uid>/doc/...`, que o `rrt2` grava no `config.toml`. Dentro do sandbox a detecção
automática só enxerga o que o sandbox enxerga (em geral nada), então a lista vem vazia e o
caminho é "Escolher pasta...".

```bash
flatpak install --user rrt2-0.3.0-x86_64.flatpak     # puxa o runtime do Flathub se faltar
flatpak run io.github.pedryvo.rrt2                   # ou pelo menu de aplicativos; escolha a pasta do jogo
```

Para revogar o acesso: `flatpak permission-reset io.github.pedryvo.rrt2` (ou o app Flatseal /
as configurações do portal do ambiente de desktop). Instalação do Steam: escolha a pasta do jogo
dentro de `steamapps/common`. Configuração e saves ficam em
`~/.var/app/io.github.pedryvo.rrt2/config/rrt2` e `~/.var/app/io.github.pedryvo.rrt2/data/rrt2`.
Quem prefere o terminal ainda pode usar `flatpak override --user --filesystem=<pasta>:ro` com
`set-path`.

Para compilar localmente (pesado, compila o projeto inteiro):

```bash
flatpak-builder --user --install-deps-from=flathub --force-clean --install \
    ~/.cache/rrt2/flatpak-build packaging/flatpak/io.github.pedryvo.rrt2.yml
```

Lacunas conhecidas:

- O runtime não traz os programas `ffmpeg`/`ffprobe`/`ffplay`, então o Flatpak não toca os vídeos
  (intro, vitória e derrota); o resto do jogo funciona. Resolver exige um módulo de ffmpeg mínimo
  (decodificador Smacker, `ffplay` com SDL2) no manifesto.
- A permissão do portal vale por pasta escolhida: se o jogador mover a pasta do jogo, a tela
  abre de novo e ele escolhe outra vez.
- A janela não define `app_id`, então alguns docks Wayland podem não associar a janela ao ícone
  (o `.desktop` declara `StartupWMClass=rrt2`).

## 5. Windows: instalador e ícone do executável

**Ícone e metadados no `rrt2.exe`.** O `build.rs` do `rt2-game` chama o crate `winresource`
(build-dependency só em `cfg(windows)`, sem features padrão) quando o host e o alvo são Windows:
embute `packaging/windows/rrt2.ico` e o recurso de versão (`FileVersion`/`ProductVersion` vêm da
versão do workspace; `ProductName` RRT2, descrição, nome original e licença). Em outros hosts o
passo não existe, então compilar para Windows a partir do Linux gera um `.exe` sem ícone, que
funciona do mesmo jeito. O `.ico` (7 tamanhos, de 16 a 256 px, PNG dentro do ICO, ~64 KB) é
versionado por ser arte do projeto; para regenerar depois de mudar o logo:

```bash
d=$(mktemp -d); for s in 16 24 32 48 64 128 256; do rsvg-convert -w $s -h $s docs/assets/logo.svg -o $d/$s.png; done
python3 -I -c "from PIL import Image; import sys; d=sys.argv[1]; ss=[16,24,32,48,64,128]; Image.open(f'{d}/256.png').save('packaging/windows/rrt2.ico', sizes=[(s,s) for s in ss+[256]], append_images=[Image.open(f'{d}/{s}.png') for s in ss])" $d
```

**Instalador.** `packaging/windows/rrt2.iss` (Inno Setup 6). O job Windows roda
`scripts/package-windows-installer.sh <rótulo> dist` depois do `package-release.sh`, sobre a
mesma pasta que vira o zip; o script acha o `ISCC.exe` (vem na imagem `windows-latest`; senão
instala pelo Chocolatey) e passa `AppVersion` (rótulo, ex. `0.3.0` ou `nightly-<commit>`),
`VersionNum` (versão `X.Y.Z` do `Cargo.toml`, para o recurso de versão do setup), `StageDir` e
`OutputDir`. Resultado: `rrt2-<rótulo>-windows-x86_64-setup.exe`. O zip portátil continua.

- **Por usuário, sem admin, por padrão** (`PrivilegesRequired=lowest`): instala em
  `%LOCALAPPDATA%\Programs\RRT2`. A caixa de diálogo do início oferece "instalar para todos os
  usuários" (pede UAC, vai para `Program Files\RRT2`); na linha de comando, `/ALLUSERS` ou
  `/CURRENTUSER`.
- Mostra a licença (o `LICENSE`, GPL-3.0); atalho no menu Iniciar sempre e na área de trabalho
  como tarefa opcional (desmarcada); desinstalador registrado em "Aplicativos instalados", com o
  ícone do `rrt2.exe`.
- Página opcional "Pasta do jogo original": vem preenchida com o primeiro local comum que tiver
  o jogo (GOG, GOG Galaxy, Steam, CD), valida `RT2_PLAT.EXE` e `data\RT2.PK2` e, no fim, roda
  `rrt2 set-path "<pasta>"` como o usuário que abriu o instalador (não a conta elevada). Em
  branco, o jogador usa `rrt2 set-path` depois; nesse caso a caixa "Executar RRT2" do fim fica
  oculta, porque sem pasta o jogo só mostra um erro.
- Idiomas do assistente: inglês, português do Brasil, espanhol, alemão e russo (os arquivos
  oficiais do Inno Setup; chinês simplificado não vem com ele, então cai no inglês).
- Desinstalar remove só os arquivos instalados: `config.toml`, saves e recordes (em `%APPDATA%`
  e `%LOCALAPPDATA%`, seção 1) ficam, e a pasta do jogo original nunca é tocada.
- Instalação silenciosa: `rrt2-<v>-windows-x86_64-setup.exe /VERYSILENT /CURRENTUSER` (usa o
  palpite da pasta do jogo, se houver).
- O `AppId` (`{C5E014EA-B253-4B7C-A586-DFDF3BDBFCEE}`) é fixo para sempre: é por ele que uma
  versão nova atualiza a anterior.

Sem assinatura de código, o Windows mostra o aviso do SmartScreen na primeira execução do setup
(seção 8).

## 6. winget

Modelos em `packaging/winget/` (schema 1.6.0, identificador `pedryvo.RRT2`): versão,
instalador (tipo `inno`, escopos `user` e `machine` com `/CURRENTUSER` e `/ALLUSERS`,
`ProductCode` = `AppId` + `_is1`) e locale padrão `en-US`. Os marcadores `__VERSION__`,
`__INSTALLER_SHA256__` e `__RELEASE_DATE__` são preenchidos por
`scripts/fill-winget.sh <X.Y.Z> <setup.exe> [saída] [data]`. Nas tags `vX.Y.Z` o job Windows faz
isso com o instalador recém-gerado e anexa `rrt2-<v>-winget-manifests.zip` à release; a URL do
instalador aponta para o asset da própria release, então o hash confere.

**Nada é enviado ao winget automaticamente.** Para publicar (decisão do usuário):

1. Baixe o zip de manifestos da release e confira (`winget validate --manifest <pasta>`; para
   testar, `winget settings --enable LocalManifestFiles` e `winget install --manifest <pasta>`).
2. Faça um fork de `microsoft/winget-pkgs`, copie os três arquivos para
   `manifests/p/pedryvo/RRT2/<versão>/` e abra o PR (ou use `wingetcreate submit <pasta>`).
3. Versões seguintes: `wingetcreate update pedryvo.RRT2 --version <v> --urls <url do setup>`.

## 7. macOS: `RRT2.app` num `.dmg`

`scripts/package-macos-app.sh <plataforma> <rótulo> [target-dir] [saída]` roda nos dois jobs
macOS (aarch64 e x86_64, este compilado de forma cruzada no runner Apple Silicon), depois do
tarball, que continua. Monta:

- `RRT2.app/Contents/Info.plist` a partir de `packaging/macos/Info.plist` (identificador
  `io.github.pedryvo.rrt2`, versão `X.Y.Z` do workspace, rótulo no `CFBundleGetInfoString`,
  macOS mínimo 11.0 no aarch64 e 10.13 no x86_64), validado com `plutil -lint`;
- `Contents/MacOS/rrt2` e `Contents/Resources/rrt2.icns`, gerado do logo no runner
  (`rsvg-convert`, instalado pelo Homebrew se faltar, e `iconutil`, tamanhos de 16 a 1024 px);
- assinatura **ad-hoc** (`codesign --sign -`): gratuita e sem identidade da Apple. Não é
  assinatura de desenvolvedor nem notarização; serve só para o Apple Silicon não acusar o app
  baixado de "danificado";
- `rrt2-<rótulo>-<plataforma>.dmg` (UDZO, `hdiutil`) com o app, um atalho para `/Applications`,
  `INSTALL.txt`, `README.md`, `CHANGELOG.md`, `LICENSE` e as licenças das fontes.

**Primeira execução (Gatekeeper).** Como o app não é notarizado, o macOS bloqueia a primeira
abertura ("não é possível verificar o desenvolvedor"). Para abrir:

- macOS 14 ou anterior: no Finder, botão direito (ou Control-clique) em `RRT2.app` → Abrir →
  Abrir. Depois disso ele abre normalmente.
- macOS 15 ou mais novo: tente abrir uma vez, depois Ajustes do Sistema → Privacidade e
  Segurança → "Abrir Mesmo Assim".
- Ou, no Terminal: `xattr -dr com.apple.quarantine /Applications/RRT2.app`.

Ainda não há tela de configuração, então informe a pasta do jogo uma vez pelo Terminal:

```bash
/Applications/RRT2.app/Contents/MacOS/rrt2 set-path "/caminho/do/Railroad Tycoon II"
```

Configuração e saves em `~/Library/Application Support/rrt2/`.

## 8. Assinatura de código

**Nenhum pacote é assinado hoje, e implementar assinatura é decisão do usuário** (todas as opções
abaixo envolvem conta, identidade verificada ou custo). O que acontece sem ela e quais são as
saídas:

**Windows.** O setup e o `rrt2.exe` sem assinatura disparam o Microsoft Defender SmartScreen
("O Windows protegeu o computador"): o jogador clica em "Mais informações" → "Executar assim
mesmo". O aviso some com o tempo conforme o arquivo ganha reputação, mas cada versão nova
recomeça do zero. Opções:

- **Certificado comercial de assinatura de código** (OV ou EV, de uma autoridade como DigiCert,
  Sectigo ou SSL.com): pago por ano, e desde 2023 a chave precisa ficar em hardware (token ou HSM
  na nuvem). Com certificado OV a reputação no SmartScreen ainda se acumula aos poucos; o EV
  deixou de dar reputação imediata em 2024.
- **Azure Trusted Signing** (Microsoft): serviço de assinatura gerenciado, cobrado por mês, com
  validação de identidade; integra no GitHub Actions (`azure/trusted-signing-action`). A
  disponibilidade para pessoas físicas depende de país.
- **SignPath Foundation**: assinatura gratuita para projetos open source aprovados. O certificado
  é emitido em nome da fundação, a assinatura roda na infraestrutura deles a partir dos artefatos
  do CI (há integração com GitHub Actions), e o projeto precisa cumprir as regras (licença OSI,
  build reproduzível no CI, política de assinatura publicada).

Em qualquer caso, assinar o `rrt2.exe` antes de empacotar e o setup depois (o Inno Setup aceita
`SignTool=` no `[Setup]`), com carimbo de tempo.

**macOS.** Para abrir sem aviso é preciso assinar com um certificado **Developer ID Application**
e **notarizar** (`xcrun notarytool submit`, depois `xcrun stapler staple` no `.dmg`), o que exige
o **Apple Developer Program** (anuidade paga, US$ 99). Sem isso, a assinatura ad-hoc atual e as
instruções da seção 7 são o caminho.
