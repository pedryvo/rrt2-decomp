; Inno Setup 6 script for the Windows installer of rrt2 (docs/packaging.md, section 5).
; The release workflow compiles it after scripts/package-release.sh staged the zip contents:
;   iscc /DAppVersion=<label> /DVersionNum=<X.Y.Z> /DStageDir=<staged dir> /DOutputDir=<dist>
;        packaging\windows\rrt2.iss
; Only rrt2.exe, the docs and the licences are installed. No original game file is ever
; packaged: the player points rrt2 at their own install (optional page below, or
; `rrt2 set-path`). Installs per user by default (no admin); the wizard and the
; /ALLUSERS command-line switch allow an all-users install in Program Files.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif
#ifndef VersionNum
  #define VersionNum "0.0.0"
#endif
#ifndef StageDir
  #error StageDir must point at the staged package (scripts/package-release.sh output folder)
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

#define AppName "RRT2"
#define AppExe "rrt2.exe"
#define AppUrl "https://github.com/pedryvo/rrt2-decomp"

[Setup]
; Fixed forever: Windows uses it to find the installed copy for upgrades and uninstall.
AppId={{C5E014EA-B253-4B7C-A586-DFDF3BDBFCEE}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Pedro Ivo Rocha
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
AppComments=Open-source clone of Railroad Tycoon II (needs your own copy of the original game)
VersionInfoVersion={#VersionNum}
VersionInfoProductVersion={#VersionNum}
VersionInfoProductTextVersion={#AppVersion}
VersionInfoDescription={#AppName} setup
; Per-user by default ({autopf} = %LOCALAPPDATA%\Programs); the dialog offers "all users".
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
UsePreviousAppDir=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile={#StageDir}\LICENSE
SetupIconFile=rrt2.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#OutputDir}
OutputBaseFilename=rrt2-{#AppVersion}-windows-x86_64-setup
CloseApplications=yes
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
english.GameDirCaption=Original game folder
english.GameDirDescription=Where is Railroad Tycoon II installed?
english.GameDirSub=RRT2 needs your own copy of Railroad Tycoon II (GOG, Steam or CD) and only reads it. Choose the folder that holds RT2_PLAT.EXE and data\RT2.PK2, or leave it empty to set it later with "rrt2 set-path".
english.GameDirInvalid=This folder does not look like a Railroad Tycoon II install (RT2_PLAT.EXE and data\RT2.PK2 are needed). Choose another folder or leave the field empty.
english.GameDirSaveFailed=Could not save the game folder. Run "rrt2 set-path <folder>" later.
brazilianportuguese.GameDirCaption=Pasta do jogo original
brazilianportuguese.GameDirDescription=Onde o Railroad Tycoon II está instalado?
brazilianportuguese.GameDirSub=O RRT2 precisa da sua própria cópia do Railroad Tycoon II (GOG, Steam ou CD) e só a lê. Escolha a pasta que contém RT2_PLAT.EXE e data\RT2.PK2, ou deixe em branco para definir depois com "rrt2 set-path".
brazilianportuguese.GameDirInvalid=Esta pasta não parece uma instalação do Railroad Tycoon II (são necessários RT2_PLAT.EXE e data\RT2.PK2). Escolha outra pasta ou deixe o campo em branco.
brazilianportuguese.GameDirSaveFailed=Não foi possível gravar a pasta do jogo. Rode "rrt2 set-path <pasta>" depois.
spanish.GameDirCaption=Carpeta del juego original
spanish.GameDirDescription=¿Dónde está instalado Railroad Tycoon II?
spanish.GameDirSub=RRT2 necesita tu propia copia de Railroad Tycoon II (GOG, Steam o CD) y solo la lee. Elige la carpeta que contiene RT2_PLAT.EXE y data\RT2.PK2, o déjala vacía para indicarla más tarde con "rrt2 set-path".
spanish.GameDirInvalid=Esta carpeta no parece una instalación de Railroad Tycoon II (se necesitan RT2_PLAT.EXE y data\RT2.PK2). Elige otra carpeta o deja el campo vacío.
spanish.GameDirSaveFailed=No se pudo guardar la carpeta del juego. Ejecuta "rrt2 set-path <carpeta>" más tarde.
german.GameDirCaption=Ordner des Originalspiels
german.GameDirDescription=Wo ist Railroad Tycoon II installiert?
german.GameDirSub=RRT2 braucht deine eigene Kopie von Railroad Tycoon II (GOG, Steam oder CD) und liest sie nur. Wähle den Ordner mit RT2_PLAT.EXE und data\RT2.PK2 oder lass das Feld leer und setze ihn später mit "rrt2 set-path".
german.GameDirInvalid=Dieser Ordner sieht nicht wie eine Railroad-Tycoon-II-Installation aus (RT2_PLAT.EXE und data\RT2.PK2 werden benötigt). Wähle einen anderen Ordner oder lass das Feld leer.
german.GameDirSaveFailed=Der Spielordner konnte nicht gespeichert werden. Führe später "rrt2 set-path <Ordner>" aus.
russian.GameDirCaption=Папка оригинальной игры
russian.GameDirDescription=Где установлена Railroad Tycoon II?
russian.GameDirSub=RRT2 нужна ваша собственная копия Railroad Tycoon II (GOG, Steam или CD), которую он только читает. Выберите папку с RT2_PLAT.EXE и data\RT2.PK2 или оставьте поле пустым и укажите её позже командой "rrt2 set-path".
russian.GameDirInvalid=Эта папка не похожа на установку Railroad Tycoon II (нужны RT2_PLAT.EXE и data\RT2.PK2). Выберите другую папку или оставьте поле пустым.
russian.GameDirSaveFailed=Не удалось сохранить папку игры. Позже выполните "rrt2 set-path <папка>".

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#StageDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\INSTALL.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Railroad Tycoon II clone"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent; Check: GameDirChosen

; Config and saves live in the user's AppData (dirs::config_dir / dirs::data_dir) and are kept
; on uninstall, like the original game folder, which is never touched.

[Code]
var
  GameDirPage: TInputDirWizardPage;

function IsGameDir(const Dir: String): Boolean;
begin
  Result := FileExists(AddBackslash(Dir) + 'RT2_PLAT.EXE') and
            FileExists(AddBackslash(Dir) + 'data\RT2.PK2');
end;

{ First common install location that holds the game, or '' (the player can browse). }
function GuessGameDir(): String;
var
  Candidates: array of String;
  I: Integer;
begin
  Result := '';
  SetArrayLength(Candidates, 6);
  Candidates[0] := ExpandConstant('{sd}') + '\GOG Games\Railroad Tycoon II Platinum';
  Candidates[1] := ExpandConstant('{commonpf32}') + '\GOG Galaxy\Games\Railroad Tycoon II Platinum';
  Candidates[2] := ExpandConstant('{commonpf32}') + '\GOG.com\Railroad Tycoon II Platinum';
  Candidates[3] := ExpandConstant('{commonpf32}') + '\Steam\steamapps\common\Railroad Tycoon II';
  Candidates[4] := ExpandConstant('{commonpf32}') + '\Steam\steamapps\common\Railroad Tycoon II Platinum';
  Candidates[5] := ExpandConstant('{commonpf32}') + '\PopTop Software\Railroad Tycoon II';
  for I := 0 to GetArrayLength(Candidates) - 1 do
    if IsGameDir(Candidates[I]) then
    begin
      Result := Candidates[I];
      Exit;
    end;
end;

{ Offer to start the game only when it knows where the original is (no setup screen yet). }
function GameDirChosen(): Boolean;
begin
  Result := Trim(GameDirPage.Values[0]) <> '';
end;

procedure InitializeWizard();
begin
  GameDirPage := CreateInputDirPage(wpSelectTasks, CustomMessage('GameDirCaption'),
    CustomMessage('GameDirDescription'), CustomMessage('GameDirSub'), False, '');
  GameDirPage.Add('');
  GameDirPage.Values[0] := GuessGameDir();
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = GameDirPage.ID) and (Trim(GameDirPage.Values[0]) <> '') and
     not IsGameDir(Trim(GameDirPage.Values[0])) then
  begin
    MsgBox(CustomMessage('GameDirInvalid'), mbError, MB_OK);
    Result := False;
  end;
end;

{ Remembers the folder in the user's config.toml through `rrt2 set-path`, run as the user who
  started the setup (not the elevated account of an all-users install). }
procedure CurStepChanged(CurStep: TSetupStep);
var
  Dir: String;
  Code: Integer;
begin
  if CurStep <> ssPostInstall then
    Exit;
  Dir := Trim(GameDirPage.Values[0]);
  if Dir = '' then
    Exit;
  if not ExecAsOriginalUser(ExpandConstant('{app}\{#AppExe}'), 'set-path "' + Dir + '"', '',
                            SW_HIDE, ewWaitUntilTerminated, Code) or (Code <> 0) then
    SuppressibleMsgBox(CustomMessage('GameDirSaveFailed'), mbInformation, MB_OK, IDOK);
end;
