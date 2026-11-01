<div align="center">

# Railroad Tycoon II (rrt2-decomp) — Spielerhandbuch

[English](player-guide.md) · [Português](player-guide.pt-BR.md) · [Español](player-guide.es.md) · **Deutsch** · [中文](player-guide.zh-CN.md) · [Русский](player-guide.ru.md)

</div>

---

Willkommen bei **rrt2-decomp**, einer modernen Open-Source-Neuimplementierung des Strategieklassikers *Railroad Tycoon II Platinum* aus dem Jahr 1998, von Grund auf neu in Rust geschrieben. Dieses Handbuch führt Sie durch alle Facetten des Spiels: von der Installation und Ersteinrichtung über die wirtschaftlichen und betrieblichen Grundlagen bis hin zu Tastaturkürzeln, technischen Unterschieden zum Original von 1998 und modernen Komfortfunktionen.

> [!IMPORTANT]
> **rrt2-decomp** enthält **keine geschützten Original-Spieldateien**. Zum Spielen benötigen Sie eine rechtmäßige Kopie von *Railroad Tycoon II Platinum* (beispielsweise von [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum), Steam oder von der Original-CD). Die Engine liest Grafikarchive, Musik, Karten und Texte im reinen Lesezugriff direkt aus Ihrer bestehenden Installation ein – genau wie verwandte Projekte wie OpenRCT2 oder OpenTTD.

---

## Inhaltsverzeichnis

1. [Installation & Einrichtung](#1-installation--einrichtung)
   - [Systemvoraussetzungen & Spieldateien](#systemvoraussetzungen--spieldateien)
   - [Automatische Erkennung von Installationen](#automatische-erkennung-von-installationen)
   - [Manuelle Pfadkonfiguration](#manuelle-pfadkonfiguration)
   - [Verteilungspakete nach Betriebssystem](#verteilungspakete-nach-betriebssystem)
   - [Konfigurations- und Speicherpfade](#konfigurations--und-speicherpfade)
2. [Erste Schritte](#2-erste-schritte)
   - [Spielmodi: Kampagnen, Einzelszenarien & Sandkasten](#spielmodi-kampagnen-einzelszenarien--sandkasten)
   - [Gründung Ihrer Eisenbahngesellschaft](#gründung-ihrer-eisenbahngesellschaft)
   - [Gleisbau & Streckenplanung](#gleisbau--streckenplanung)
   - [Bahnhöfe & Stationserweiterungen](#bahnhöfe--stationserweiterungen)
   - [Kauf von Lokomotiven](#kauf-von-lokomotiven)
   - [Zugzusammenstellung & Fahrplanrouten](#zugzusammenstellung--fahrplanrouten)
   - [Finanzmanagement & Aktienhandel](#finanzmanagement--aktienhandel)
3. [Steuerung & Tastenkürzel](#3-steuerung--tastenkürzel)
   - [Kamerasteuerung & Stufenloser Zoom](#kamerasteuerung--stufenloser-zoom)
   - [Spielgeschwindigkeit & Pause](#spielgeschwindigkeit--pause)
   - [Informationsübersichten & Schnellzugriff](#informationsübersichten--schnellzugriff)
   - [Finanzberichte & Interaktive Diagramme](#finanzberichte--interaktive-diagramme)
   - [Ereignisverlauf & Benachrichtigungen](#ereignisverlauf--benachrichtigungen)
   - [Gamepad- & Steam-Deck-Unterstützung](#gamepad--und-steam-deck-unterstützung)
4. [Unterschiede zum Original von 1998](#4-unterschiede-zum-original-von-1998)
   - [Bitgenaue, Deterministische Simulation](#bitgenaue-deterministische-simulation)
   - [Moderne Bildschirme, Breitbild & 4K](#moderne-bildschirme-breitbild--4k)
   - [Skalierbare Vektortypografie & HiDPI](#skalierbare-vektortypografie--hidpi)
   - [Moderner TCP/IP-Lockstep-Mehrspielermodus](#moderner-tcpip-lockstep-mehrspielermodus)
   - [Dynamische Lokalisierung in 6 Sprachen](#dynamische-lokalisierung-in-6-sprachen)
5. [Optionale Moderne Komfortfunktionen](#5-optionale-moderne-komfortfunktionen)
   - [Regelbasierte Fehlerkorrekturen des Originals (`Fix`)](#regelbasierte-fehlerkorrekturen-des-originals-fix)
   - [Bauschritte Rückgängig Machen (`Ctrl+Z`)](#bauschritte-rückgängig-machen-ctrlz)
   - [Flottenmanager & Massen-Lokomotiventausch](#flottenmanager--massen-lokomotiventausch)
   - [Erweiterte Karten-Overlays](#erweiterte-karten-overlays)
   - [Modding über TOML-Dateien](#modding-über-toml-dateien)
   - [Barrierefreiheit & Zugänglichkeit](#barrierefreiheit--zugänglichkeit)

---

## 1. Installation & Einrichtung

### Systemvoraussetzungen & Spieldateien

Um `rrt2` auszuführen, benötigt Ihr Computer:
- **Betriebssystem**: Linux (x86_64), Windows 10/11 (x86_64) oder macOS 11+ (Apple Silicon oder Intel x86_64).
- **Grafikkarte**: Unterstützung für Vulkan, DirectX 12 oder Metal über die Grafikbibliothek `wgpu`.
- **Dateien des Originalspiels**: Eine gültige Installation von *Railroad Tycoon II Platinum*. Erforderlich sind:
  - `data/RT2.PK2`: Hauptarchiv für Geländetexturen, Sprites und Grafikelemente.
  - `RT2_PLAT.EXE`: Original-Binärdatei, aus der Fahrzeugdaten und Wirtschaftsmatrizen ausgelesen werden.
  - `default2.lng`: Originale sprachübergreifende Texttabelle.
  - Optionale Verzeichnisse: `maps/` (Szenariokarten), `games/` (vorgefertigte Spielstände) sowie `anim/` (`.SMK`-Zwischensequenzen; erfordert installiertes `ffmpeg` und `ffplay`).

### Automatische Erkennung von Installationen

Beim ersten Start öffnet `rrt2` selbstständig den **Einrichtungsbildschirm**. Das System durchsucht vorhandene Datenträger in deterministischer Reihenfolge:
- **Steam**: Steam-Bibliotheksordner (`steamapps/common/Railroad Tycoon 2`), einschließlich Steam Flatpak und Proton-Präfixe (`compatdata/284840/pfx/drive_c`).
- **GOG**: Windows-Registrierungseinträge (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`), Standardordner unter `GOG Games/Railroad Tycoon II` sowie Bibliotheken des Heroic Games Launchers.
- **Lutris & Wine**: Konfigurierte Lutris-Spieleordner (`games/*.yml`), Wine-Standardverzeichnisse (`~/.wine`), Bottles und PlayOnLinux.
- **Typische Windows-Pfade**: `C:\Games\`, `C:\Program Files (x86)\` über alle Laufwerksbuchstaben von `A:` bis `Z:`.

Wird eine gültige Installation gefunden, erscheint sie in einer interaktiven Auswahlliste mit Herkunftsangabe (z. B. „Steam“, „GOG“, „Lutris“). Wählen Sie den Eintrag aus und klicken Sie auf **Spielen**.

### Manuelle Pfadkonfiguration

Befindet sich Ihre Installation in einem benutzerdefinierten Verzeichnis:
1. **Über die Benutzeroberfläche**: Klicken Sie auf **Ordner wählen...**, um den systemeigenen Dateidialog aufzurufen (unter Linux via XDG Desktop Portal, vollständig kompatibel mit Flatpak).
2. **Über die Befehlszeile**:
   ```bash
   ./rrt2 set-path ~/Games/railroad-tycoon-2
   # Unter Windows:
   rrt2.exe set-path "C:\Games\Railroad Tycoon II"
   ```
3. **Über Startparameter oder Umgebungsvariable**:
   ```bash
   ./rrt2 --game-dir /pfad/zum/spiel
   # Oder per Umgebungsvariable:
   export RT2_DIR=/pfad/zum/spiel
   ./rrt2
   ```

### Verteilungspakete nach Betriebssystem

- **Linux**:
  - **AppImage**: Eigenständige Binärdatei mit integrierten Vektorschriftarten und Übersetzungen. Ausführbar machen (`chmod +x rrt2-*.AppImage`) und direkt starten.
  - **Flatpak**: Erhältlich unter `io.github.pedryvo.rrt2.flatpak`. Läuft isoliert; erteilen Sie über den Ordnerauswahldialog den Lesezugriff auf Ihre Spieldateien.
  - **Tarball (`.tar.gz`)**: Archiv entpacken und `./rrt2` ausführen. Setzt lediglich `glibc` und `libasound.so.2` voraus (Vulkan und Wayland/X11 werden dynamisch geladen).
- **Windows**:
  - **Inno Setup-Installationsprogramm (`-setup.exe`)**: Benötigt keine Administratorrechte, richtet eine Startmenü-Verknüpfung ein, bietet einen sauberen Deinstallierer und speichert den Pfad zu den Spieldaten.
  - **winget**: Installation direkt über den Windows-Paketmanager:
    ```cmd
    winget install pedryvo.rrt2
    ```
  - **Portables ZIP**: In einen beliebigen Ordner entpacken und `rrt2.exe` starten.
- **macOS**:
  - **Disk-Image (`.dmg`)**: `.dmg` öffnen und `RRT2.app` in den Ordner `/Applications` ziehen. Unterstützt Apple Silicon (`aarch64`) und Intel (`x86_64`).
  - *Gatekeeper-Hinweis*: Bei nicht kommerziell signierten Entwicklerversionen klicken Sie mit der rechten Maustaste auf `RRT2.app` und wählen **Öffnen**, oder führen Sie im Terminal folgenden Befehl aus:
    ```bash
    xattr -dr com.apple.quarantine /Applications/RRT2.app
    ```

### Konfigurations- und Speicherpfade

Die Engine verändert niemals Dateien in Ihrem Original-Spieleordner. Alle Einstellungen und Spielstände verbleiben sicher in Ihren Benutzerverzeichnissen:

| Zweck | Linux | Windows | macOS |
|---|---|---|---|
| Konfiguration (`config.toml`) | `~/.config/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |
| Spielstände (`*.GAM`, `*.toml`, `*.png`) | `~/.local/share/rrt2/saves/` | `%APPDATA%\rrt2\saves\` | `~/Library/Application Support/rrt2/saves/` |
| Exportierte Berichte (`*.csv`) | `~/.local/share/rrt2/reports/` | `%APPDATA%\rrt2\reports\` | `~/Library/Application Support/rrt2/reports/` |
| Bestenlisten (`hiscore.dat`) | `~/.local/share/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |

> [!TIP]
> Die Speicherpfade lassen sich zur Laufzeit über die Umgebungsvariablen `RRT2_CONFIG`, `RRT2_SAVES` und `RRT2_REPORTS` umleiten.

---

## 2. Erste Schritte

### Spielmodi: Kampagnen, Einzelszenarien & Sandkasten

Wählen Sie im Menü **Einzelspieler (Single Player)** Ihren Weg zum Eisenbahn-Tycoon:
- **Klassische Kampagne (Original)**: Meistern Sie die 18 historischen Szenarien, die die Entstehung des Schienenverkehrs in Nordamerika und Europa nachzeichnen (beginnend mit der Pionierstrecke der *Baltimore & Ohio*). Zu Beginn jedes Szenarios wählen Sie einen Startbonus (Zinsnachlässe, Bausubventionen oder Rabatte beim Lokomotivkauf).
- **Kampagne The Second Century**: 18 anspruchsvolle Szenarien des 20. und 21. Jahrhunderts. Erleben Sie die Umstellung auf moderne Elektrolokomotiven, Hochgeschwindigkeits-Passagierverkehr und komplexe Industrielogistik.
- **Einzelszenarien**: Laden Sie eine beliebige `.MAP`- oder `.MP2`-Karte. Informieren Sie sich im Briefing über Ihre Siegbedingungen (Bronze, Silber, Gold) und stellen Sie KI-Konkurrenz sowie Wirtschaftsmodelle ein.
- **Sandkastenmodus**: Bauen Sie frei und unbeschwert ohne Zeitlimits, Konkursgefahr oder feindliche Übernahmen durch Mitbewerber.
- **Tutorial & Beispielszenarien**: Perfekt für Neueinsteiger. Erlernen Sie Gleisverlegung und einfache Frachtketten ohne finanzielles Risiko.

### Gründung Ihrer Eisenbahngesellschaft

Wenn Sie ein Szenario ohne bestehende Bahngesellschaft starten:
1. **Unternehmensname & Wappen**: Geben Sie einen Namen ein (bis zu 30 Zeichen) und wählen Sie Logo sowie Firmenfarbe.
2. **Startkapital & Aktienemission**: Über die Schieberegler bestimmen Sie:
   - Verfügbares Anfangskapital der Gesellschaft.
   - Anzahl emittierter Aktien und Ausgabepreis.
   - Ihren persönlichen Anteil als Präsident („You buy“): Die Anzahl der Aktien, die Sie von Beginn an privat besitzen.
3. **Vertrauen des Aufsichtsrats**: Halten Sie das Unternehmen profitabel! Dauerhafte Verluste oder ein zu geringer privater Aktienanteil können dazu führen, dass Aktionäre Sie per Misstrauensvotum absetzen.

### Gleisbau & Streckenplanung

Das Streckennetz bildet das Rückgrat Ihres Eisenbahnunternehmens:
1. Wählen Sie das Werkzeug **Gleis verlegen (Lay Track)** in der linken Leiste.
2. Halten Sie die **linke Maustaste** auf dem Startfeld gedrückt und ziehen Sie die Linie quer über das Gelände zum Zielort.
3. **Echtzeit-Kosten- und Streckenvorschau**: Während des Ziehens visualisiert der Streckenplaner Kurvenradien, Brückenbauten über Flüsse, Steigungsgrade und die exakten Gesamtkosten. Rote Markierungen signalisieren unpassierbares Terrain oder fehlende Durchfahrtsrechte.
4. Lassen Sie die Maustaste los, um die Strecke kostenpflichtig zu errichten.
5. Drücken Sie während des Ziehens `Esc`, um den Bauvorgang kostenfrei abzubrechen.

> [!TIP]
> Halten Sie Steigungen nach Möglichkeit unter 2%. Steile Steigungen von 3% bis 6% bremsen Dampflokomotiven drastisch aus und treiben den Treibstoffverbrauch massiv in die Höhe.

### Bahnhöfe & Stationserweiterungen

Bahnhöfe schöpfen Passagiere, Post und Industriegüter in ihrem Umkreis ab:
1. Klicken Sie auf **Bahnhof bauen (Build Station)**.
2. **Bahnhofsgrößen**:
   - **Kleines Depot (Small)**: Günstig ($50.000), 2 Kacheln Erfassungsradius. Ideal für abgelegene Rohstoffbetriebe.
   - **Mittlerer Bahnhof (Medium)**: Mittlere Kosten ($100.000), 4 Kacheln Erfassungsradius. Perfekt für wachsende Kleinstädte.
   - **Großer Kopfbahnhof (Large)**: Hohe Kosten ($200.000), 8 Kacheln Erfassungsradius. Unverzichtbar für Ballungszentren.
3. **Goldener Einzugskreis & Platzierungsanzeige**: Beim Bewegen des Cursors markiert ein **goldener Kreis** das Einzugsgebiet; die Gebäudevorschau zeigt die Gültigkeit an:
   - **Grün**: Gültiger, freier Bauplatz.
   - **Gelb**: Gültiger Bauplatz mit direktem Gleisanschluss.
   - **Rot**: Ungültiger Bauplatz (unebenes Gelände, Gewässer oder Überschneidung mit anderen Stationen).
4. **Baustile**: Wählen Sie zwischen viktorianischem, kolonialem, Missions-, Tudor- oder Kyoto-Stil passend zur Region.
5. **Stationserweiterungen**: Klicken Sie auf einen fertigen Bahnhof, um wertvolle Zusatzausstattungen zu erwerben:
   - **Betriebsbauten**: *Wasserturm* (Water Tower) und *Sandspeicher* (Sandhouse) für kurze Wartungsaufenthalte von Dampfzügen; *Ringlokschuppen* (Roundhouse), der die Wartungskosten der Züge halbiert.
   - **Gewerbebauten**: *Telegraf* (beschleunigt Abrechnungen), *Hotel* & *Restaurant* (erhebliche Bonuszahlungen durch Passagiere), *Zollamt* (Customs House) für grenzüberschreitenden Güterverkehr.
   - **Lagerhäuser**: *Getreidesilo*, *Kühlhaus* und *Güterschuppen* zur verlustfreien Zwischenlagerung von Frachtüberschüssen.

### Kauf von Lokomotiven

Wählen Sie in einem Bahnhof oder im Zugmenü **Lokomotive kaufen (Purchase Train)**:
- **Lokomotivenkatalog**: Historische Modelle gegliedert nach Epochen und Antriebsarten (Dampf, Diesel, Elektro).
- **Leistungsmatrix**: Prüfen Sie die Geschwindigkeitsmatrix nach Steigungsgraden (0%, 2%, 4% und 6%) bei Zuglasten von 1 bis 6 Waggons mit 30 bis 50 Tonnen.
- **Betriebskosten**: Beachten Sie jährliche Wartungsgebühren, Treibstoffverbrauch, Zuverlässigkeitsprofile und Beschleunigungswerte.
- **Elektrifizierung**: Elektrolokomotiven können nur auf Strecken eingesetzt werden, die zuvor mit einer Oberleitung ausgestattet wurden!

### Zugzusammenstellung & Fahrplanrouten

1. **Routenplanung**: Fügen Sie im Zugfenster Bahnhöfe per Klick auf die Übersichtskarte oder aus der Liste hinzu.
2. **Waggonarten**:
   - **Expressfracht**: Passagiere und Post (hohe Rendite, stark zeit- und geschwindigkeitsabhängig).
   - **Schwer- und Schüttgüter**: Kohle, Eisenerz, Getreide, Vieh, Holz, Öl, Stahl, Fertigwaren, Automobile.
3. **Abfertigungsregeln**: Bestimmen Sie, ob Züge sofort abfahren, eine Teilladung abwarten oder erst bei vollständiger Beladung das Gleis verlassen (`Wait for Full Cargo`).

### Finanzmanagement & Aktienhandel

Ein wahrer Tycoon beherrscht das Kapital:
- **Unternehmensanleihen (Bonds)**: Begeben Sie Anleihen zur Finanzierung umfangreicher Netzerweiterungen. Der Zinssatz richtet sich nach der Bonität Ihrer Gesellschaft (von AAA bis Junk F). Tilgen Sie Kredite zügig bei stabilem Cashflow.
- **Aktienmarkt (`M`)**:
  - **Aktienkauf & -verkauf**: Investieren Sie Ihr Privatvermögen in die eigene Bahngesellschaft zur Festigung Ihrer Kontrolle oder spekulieren Sie auf Dividenden von Konkurrenten.
  - **Aktienrückkauf (Share Buybacks)**: Setzen Sie Unternehmensgewinne ein, um eigene Aktien vom Markt zurückzukaufen, was den Gewinn je Aktie (EPS) sowie den Börsenkurs steigert.
  - **Aktiensplits (Stock Splits)**: Steigt der Börsenwert stark an, erfolgt ein automatischer 2-für-1-Aktiensplit zur Erhöhung der Marktliquidität.
- **Dividenden**: Schütten Sie jährliche Dividenden aus, um Aktionäre zufriedenzustellen und den Kurswert zu stützen – ohne die laufende Liquidität zu gefährden.
- **Fusionen & Übernahmen**: Sobald Sie mehr als 50% der Anteile eines Mitbewerbers kontrollieren, können Sie eine Fusion erzwingen und dessen gesamtes Streckennetz übernehmen.

---

## 3. Steuerung & Tastenkürzel

Alle Tastaturbelegungen können unter **Optionen (`F10`) > Tasten** frei konfiguriert werden.

### Kamerasteuerung & Stufenloser Zoom

| Aktion | Standardtastatur / Eingabe | Hinweise |
|---|---|---|
| Kamera bewegen | `W`, `A`, `S`, `D` oder Pfeiltasten | Flüssige Bewegung |
| Maus-Verschiebung | Rechte Maustaste gedrückt halten | Natürliches Verschieben |
| Rand-Scrollen | Cursor innerhalb von 4 px am Bildschirmrand | Einstellbar unter Gameplay |
| Karte nach links drehen | `Q` | 90°-Drehung gegen den Uhrzeigersinn |
| Karte nach rechts drehen | `E` | 90°-Drehung im Uhrzeigersinn |
| Hereinzoomen | Mausrad aufwärts / `+` | Stufenloser, flüssiger Zoom |
| Herauszoomen | Mausrad abwärts / `-` | Stufenloser, flüssiger Zoom |
| Kamera zentrieren | `Home` (Pos 1) | Setzt den Kartenausschnitt zurück |

### Spielgeschwindigkeit & Pause

| Aktion | Tastaturkürzel | Hinweise |
|---|---|---|
| Pause umschalten | `Pause` oder `Leertaste` | Hält die Simulation an |
| Geschwindigkeit verringern | `-` oder `Num -` | Verlangsamt den Zeitablauf |
| Geschwindigkeit erhöhen | `+` oder `Num +` | Beschleunigt den Zeitablauf |
| Vollbildmodus | `F11` | Wechselt zwischen Fenster und randlosem Vollbild |
| UI-Skalierung anpassen | `Ctrl +` / `Ctrl -` | In 25%-Schritten (100% bis 300%) |
| Ton stummschalten | `Ctrl + M` | Schaltet Musik und Soundeffekte stumm |
| Nächster Musiktitel | `Ctrl + N` | Springt zum nächsten Musikstück |

### Informationsübersichten & Schnellzugriff

| Übersicht | Taste | Inhalt |
|---|---|---|
| **Finanzen** | `F` | Gewinn- und Verlustrechnung, Bilanz, Kreditwürdigkeit, Schulden |
| **Aktienmarkt** | `M` | Live-Kurse, Handelsoptionen, Firmenbewertungen |
| **Zugliste** | `T` | Alle Lokomotiven, Betriebsdaten, Fahrstrecken, Alter |
| **Bahnhofsliste** | `G` | Frachtaufkommen, installierte Bauten, lokale Nachfrage |
| **Städte** | `C` | Einwohnerzahl, Wachstumsrate, benötigte Güter |
| **Industrien** | `I` | Fabrikproduktion, Lagerbestände, Kaufoptionen für Betriebe |
| **Kartenübersicht** | Linke Leiste (Globus-Symbol) | Thematische Vollbild-Overlays |
| **Schnellspeichern** | `F5` | Speichert in festen Schnellspeicherplatz |
| **Schnellladen** | `F9` | Lädt den letzten Schnellspeicherstand |
| **Speichern-Menü** | `Ctrl + S` | Speicherverwaltung mit Namenseingabe |
| **Laden-Menü** | `Ctrl + L` | Spielstand-Browser mit Vorschaubildern |
| **Optionen** | `F10` oder `Esc` | Grafikeinstellungen, Audio, Steuerung und Gameplay |

### Finanzberichte & Interaktive Diagramme

Erreichbar über das Buch der Gesellschaft (Schaltfläche **Reports** oder Tasten `1`, `2`, `3`):
- **Interaktive Diagramme (Reiter 1)**: Visualisiert bis zu 12 Kennzahlen im Zeitverlauf: Umsatz, Kosten, Gewinn, Barmittel, Verbindlichkeiten, Eigenkapital, Aktienkurs, Buchwert, Gewinn je Aktie und Dividende. Nutzt die barrierefreie Okabe-Ito-Farbpalette mit geometrischen Markern und Zeitschieber.
- **Ergebnis nach Kategorien (Reiter 2)**: Detaillierte Aufschlüsselung der 13 Ertrags- und Aufwandsklassen im Vergleich zwischen Gesamtlaufzeit, aktuellem Jahr und den letzten fünf Wirtschaftsjahren.
- **Ranglisten (Reiter 3)**: Bestenlisten der profitabelsten Lokomotiven und am stärksten frequentierten Bahnhöfe. Ein Klick auf einen Eintrag springt sofort zur Position auf der Karte.
- **CSV-Export**: Die Schaltfläche **Export CSV** speichert die angezeigten Daten tabellarisch unter `~/.local/share/rrt2/reports/*.csv` zur Weiterverarbeitung in Tabellenkalkulationen.

### Ereignisverlauf & Benachrichtigungen

Zu finden in der Aktivitätsbox (Activity Box) in der unteren Kontrollleiste:
- Klicken Sie auf **History**, um ein Protokoll von bis zu 500 Meldungen einzusehen (Szenarioereignisse, Wirtschaftsberichte, Börsenmeldungen, Städtewachstum und Aktivitäten der Konkurrenz).
- Filtern Sie nach Kategorien oder nutzen Sie **Show on Map**, um die Kamera direkt zur betroffenen Kachel zu bewegen.
- Pop-up-Meldungen und Zeitungsberichte können unter **Optionen > Benachrichtigungen** gezielt gefiltert werden.

### Gamepad- & Steam-Deck-Unterstützung

Für ein erstklassiges Handheld-Erlebnis auf dem Steam Deck und mit Standard-Gamepads:
- **Linker Analogstick**: Flüssige Navigation der Kamera über die Landkarte.
- **Rechter Analogstick**: Präziser virtueller Mauszeiger mit Zielhilfen.
- **Schultertrigger (L2 / R2)**: Direkte Zeitsteuerung und Schnellpause.
- **Bumper (L1 / R1)**: Kartendrehung (`Q` / `E`) und Zoom.
- **Aktionstasten (A / B / X / Y)**: Bestätigen, Abbrechen / Schließen, Gleisbau, Bahnhofsbau.

---

## 4. Unterschiede zum Original von 1998

Das Projekt bewahrt den unverkennbaren Charakter und Spielspaß von *Railroad Tycoon II Platinum*, modernisiert jedoch das technische Fundament grundlegend:

```
┌─────────────────────────┬─────────────────────────────────┬─────────────────────────────────┐
│ Eigenschaft             │ Originalspiel (1998–2001)       │ rrt2-decomp (Rust-Engine)       │
├─────────────────────────┼─────────────────────────────────┼─────────────────────────────────┤
│ Bildschirmauflösung     │ Fest auf 800x600 oder 1024x768  │ Nativ in 1080p, 1440p, 4K & UW  │
│ UI-Skalierung           │ Keine (winzige Pixel bei HiDPI) │ Vektorschriftarten (100%–300%)  │
│ Simulationsgenauigkeit  │ Geschlossenes x86-Original      │ 100% bitgenau und deterministisch│
│ Grafik-Backend          │ DirectDraw / Software GDI       │ Hardwarebeschleunigtes wgpu     │
│ Kamerasteuerung         │ Starre Zoomstufen, hakelig      │ Stufenloser Zoom & sanftes Pan  │
│ Mehrspieler-Protokoll   │ DirectPlay 4, IPX, Modem, COM   │ Zeitgemäßes TCP/IP-Lockstep-Netz│
│ Netzwerkstabilität      │ Host-Absturz beendete das Spiel │ Host-Migration & Reconnect      │
│ Plattformen             │ Nur Windows 95/98/XP            │ Nativ für Linux, Windows & macOS│
│ Sprachen                │ Eine Sprache je CD/Medium       │ 6 Sprachen im laufenden Spiel   │
└─────────────────────────┴─────────────────────────────────┴─────────────────────────────────┘
```

### Bitgenaue, Deterministische Simulation

Der Simulationskern (`rt2-sim`) verzichtet auf Näherungsformeln. Sämtliche Berechnungen – Zugbremskurven, Ausfallwahrscheinlichkeiten, Frachtaufkommen in Metropolen, Aktienkursformeln und KI-Verhaltensbäume – wurden durch Reverse Engineering des Originalcodes entschlüsselt und in Emulatoren (Unicorn) verifiziert. Geladene Spielstände verhalten sich bis auf die letzte Nachkommastelle identisch zum Original.

### Moderne Bildschirme, Breitbild & 4K

Verzerrte oder gestreckte Grafiken gehören der Vergangenheit an. Die Geländestruktur, Flüsse und Gleisanlagen werden von `wgpu` direkt in der nativen Zielauflösung Ihres Monitors gerendert. Originale 2D-Sprites durchlaufen ein hochwertiges Upscaling (xBRZ und intelligente Filterung), das beim Erststart lokal zwischengespeichert wird und für gestochen scharfe Details auf 4K- und Ultrawide-Bildschirmen sorgt.

### Skalierbare Vektortypografie & HiDPI

Anstelle verpixelter Bitmap-Schriften nutzt `rrt2-decomp` hochwertige Open-Source-Schriftarten unter der SIL OFL-Lizenz, die dem Schriftbild des Originals exakt nachempfunden sind:
- **Cinzel**: Klassische Kapitälchen mit Serifenschliff als Ersatz für Copperplate Gothic.
- **Libre Franklin**: Klare serifenlose Schrift für Tabellen, Zahlen und Schaltflächen.
- **Dancing Script**: Elegante Schreibschrift für Auszeichnungen und historische Urkunden.
- **Anton & Oswald**: Markante Plakatschriften für Titelzeilen.
- **Noto Sans SC**: Integrierte CJK-Schriftart für vereinfachtes Chinesisch.
- **Cormorant SC**: Russische Kapitälchen mit voller kyrillischer Glyphenabdeckung.

Die Benutzeroberfläche lässt sich flexibel von 100% bis 300% skalieren.

### Moderner TCP/IP-Lockstep-Mehrspielermodus

Veraltete Schnittstellen wie DirectPlay 4, IPX-Netzwerke oder Modem-Verbindungen wurden durch eine schlanke TCP/IP-Architektur ersetzt:
- **Host-gesteuertes Lockstep**: Geringe Bandbreitennutzung durch reinen Befehlsaustausch im Minutentakt der Simulation.
- **Beitritt im laufenden Spiel & Wiederverbindung**: Getrennte Spieler können nahtlos zurückkehren, indem der Server einen komprimierten Spielstand übermittelt.
- **Automatische Host-Migration**: Verlässt der Spielleiter die Partie, übernimmt automatisch der Spieler mit der niedrigsten ID die Serverfunktion.
- **Unterbrechungsfreie KI-Übernahme**: Scheidet ein Mitspieler dauerhaft aus, steuert die KI dessen Eisenbahnunternehmen weiter, um die Netzbalance zu wahren.

### Dynamische Lokalisierung in 6 Sprachen

Wechseln Sie jederzeit zwischen **English**, **Português**, **Español**, **Deutsch**, **中文 (Vereinfachtes Chinesisch)** und **Русский** unter **Optionen > Sprache**. Texte, Benutzeroberfläche und Schriftmetriken passen sich ohne Neustart sofort an.

---

## 5. Optionale Moderne Komfortfunktionen

Alle Modernisierungen sind modular aufgebaut und können nach Wunsch aktiviert werden, ohne die ursprüngliche Spielbalance zu verändern.

### Regelbasierte Fehlerkorrekturen des Originals (`Fix`)

Das Originalspiel von 1998 wies bekannte Ungereimtheiten auf (etwa Rundungsfehler bei Gründeraktien, doppelte Abrechnungen bestimmter Bahnhofsverbesserungen oder Speicherüberläufe bei Barmitteln nach mehreren Jahrzehnten). In `rrt2-decomp` haben Sie die Wahl:
- **Klassischer Modus (Standard)**: Bewahrt alle Eigenheiten und historischen Besonderheiten für absolute Originaltreue und Replay-Validität.
- **Modus mit Fehlerkorrekturen (`Fix`)**: Aktiviert geprüfte mathematische Korrekturen, die transparent im Spielstand vermerkt werden.

### Bauschritte Rückgängig Machen (`Ctrl+Z`)

Haben Sie versehentlich eine teure Hängebrücke gebaut oder einen Bahnhof falsch platziert? Mit `Ctrl + Z` machen Sie die letzte Gleis- oder Stationsbauaktion sofort rückgängig. Sofern noch kein Zug den neu errichteten Abschnitt befahren hat, erhalten Sie die vollen Baukosten erstattet.

### Flottenmanager & Massen-Lokomotiventausch

Die Instandhaltung und Modernisierung hunderter Züge erfordert kein zeitraubendes Durchklicken einzelner Einheiten mehr:
- **Zugvorlagen (Consist Templates)**: Speichern Sie bevorzugte Waggonkombinationen (z. B. „Schwerer Kohlependelzug“ oder „Transkontinentaler Luxusexpress“) und weisen Sie diese neu gekauften Zügen mit einem Klick zu.
- **Flottenweiter Lokomotiventausch**: Bestimmen Sie veraltete Lokomotivklassen in Ihrem Bestand und beauftragen Sie einen automatischen Umstieg, sobald die Züge das nächste Mal einen Bahnhof mit Betriebswerk anfahren.

### Erweiterte Karten-Overlays

Nutzen Sie neben den Standardansichten moderne Analyse-Overlays:
- **Wärmebildkarte für Angebot & Nachfrage**: Zeigt auf einen Blick Gebiete mit Rohstoffüberschüssen und Städte mit akutem Güterbedarf.
- **Steigungs- und Höhenprofil**: Farbige Hervorhebung anspruchsvoller Geländepassagen zur Vermeidung von Geschwindigkeitsengpässen.
- **Streckenrentabilität**: Identifiziert rentable Hauptstrecken und unrentable Nebenlinien.
- **Verkehrsdichte & Engpässe**: Hebt überlastete Weichen und eingleisige Nadelöhre hervor.

### Modding über TOML-Dateien

Spieldaten sind nicht länger in Binärtabellen verschlüsselt:
- Lokomotivdaten, Waggonarten, Industriekreisläufe und Szenarioregeln können in menschenlesbare **TOML**-Dateien exportiert und daraus geladen werden.
- Modder können eigene historische Zugmodelle, neue Industriegüter und individuelle Szenarien erstellen, ohne den Quelltext neu kompilieren zu müssen.

### Barrierefreiheit & Zugänglichkeit

- **Farbenblind-Modus**: Diagramme und Radaransichten setzen auf die bewährte Okabe-Ito-Farbpalette, kombiniert mit eindeutigen geometrischen Symbolen und Liniensignaturen.
- **Unabhängige Schriftgrößen**: Passen Sie Fließtextgrößen unabhängig von der Skalierung der restlichen Benutzeroberfläche an.
- **Visuelle Audiountertitel**: Optische Warnsymbole und Einblendungen informieren bei Zugpannen, Sabotageakten oder kritischen Finanzwarnungen.

---

*Railroad Tycoon ist ein eingetragenes Warenzeichen der jeweiligen Rechteinhaber. Dieses freie Fanprojekt steht in keiner Verbindung zu Take-Two Interactive, PopTop Software oder Gathering of Developers.*
