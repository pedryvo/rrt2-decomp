<div align="center">

# Railroad Tycoon II (rrt2-decomp) — Player's Guide

**English** · [Português](player-guide.pt-BR.md) · [Español](player-guide.es.md) · [Deutsch](player-guide.de.md) · [中文](player-guide.zh-CN.md) · [Русский](player-guide.ru.md)

</div>

---

Welcome to **rrt2-decomp**, a modern, open-source reimplementation of the 1998 classic strategy game *Railroad Tycoon II Platinum* written from scratch in Rust. This guide covers everything you need to know: installing and setting up the game, learning the core mechanics, navigating shortcuts, understanding how this engine differs from the 1998 original, and taking advantage of optional modern enhancements.

> [!IMPORTANT]
> **rrt2-decomp** contains **no proprietary assets**. To play, you must provide a legitimate copy of *Railroad Tycoon II Platinum* (e.g., from [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum), Steam, or an original CD-ROM installation). The engine reads graphic archives, audio, maps, and text directly from your existing installation in read-only mode, following the same model as projects like OpenRCT2 and OpenTTD.

---

## Table of Contents

1. [Installation & Setup](#1-installation--setup)
   - [System Prerequisites](#system-prerequisites)
   - [Automatic Game Detection](#automatic-game-detection)
   - [Manual Path Configuration](#manual-path-configuration)
   - [Packages by Operating System](#packages-by-operating-system)
   - [Configuration and Save Directories](#configuration-and-save-directories)
2. [Getting Started](#2-getting-started)
   - [Game Modes: Campaigns, Scenarios & Sandbox](#game-modes-campaigns-scenarios--sandbox)
   - [Founding Your Railroad Company](#founding-your-railroad-company)
   - [Laying Track](#laying-track)
   - [Building Stations & Improvements](#building-stations--improvements)
   - [Purchasing Locomotives](#purchasing-locomotives)
   - [Creating Consists & Setting Routes](#creating-consists--setting-routes)
   - [Financial Management & Stock Trading](#financial-management--stock-trading)
3. [Controls & Keyboard Shortcuts](#3-controls--keyboard-shortcuts)
   - [Camera Navigation & Zoom](#camera-navigation--zoom)
   - [Simulation Speed & Pausing](#simulation-speed--pausing)
   - [Quick Access & Information Panels](#quick-access--information-panels)
   - [Financial Reports & Interactive Charts](#financial-reports--interactive-charts)
   - [Notification History](#notification-history)
   - [Gamepad & Steam Deck Controls](#gamepad--steam-deck-controls)
4. [Differences from the 1998 Original](#4-differences-from-the-1998-original)
   - [Bit-Exact Deterministic Simulation](#bit-exact-deterministic-simulation)
   - [Modern Displays, Widescreen & 4K](#modern-displays-widescreen--4k)
   - [Scalable Vector Typography & HiDPI](#scalable-vector-typography--hidpi)
   - [Modern TCP/IP Lockstep Multiplayer](#modern-tcpip-lockstep-multiplayer)
   - [Dynamic Six-Language Localization](#dynamic-six-language-localization)
5. [Optional Modern Features](#5-optional-modern-features)
   - [Rule-Based Original Bug Fixes (`Fix`)](#rule-based-original-bug-fixes-fix)
   - [Construction Undo (`Ctrl+Z`)](#construction-undo-ctrlz)
   - [Fleet Manager & Locomotive Upgrades](#fleet-manager--locomotive-upgrades)
   - [Advanced Map Overlays](#advanced-map-overlays)
   - [TOML Data Modding](#toml-data-modding)
   - [Accessibility Features](#accessibility-features)

---

## 1. Installation & Setup

### System Prerequisites

To run `rrt2`, ensure your system meets the following requirements:
- **Operating System**: Linux (x86_64), Windows 10/11 (x86_64), or macOS 11+ (Apple Silicon or Intel x86_64).
- **Graphics**: A graphics card or integrated GPU supporting Vulkan, DirectX 12, or Metal via `wgpu`.
- **Original Game Assets**: A valid installation of *Railroad Tycoon II Platinum*. Specifically, the engine requires:
  - `data/RT2.PK2`: Main sprite archives and terrain textures.
  - `RT2_PLAT.EXE`: Original game binary used to read vehicle statistics and balance tables.
  - `default2.lng`: Original multi-language text tables.
  - Optional directories: `maps/` (scenario maps), `games/` (pre-made saves), and `anim/` (`.SMK` cinematics; requires `ffmpeg` and `ffplay` installed on your system).

### Automatic Game Detection

On its very first launch, `rrt2` displays an automatic setup screen. The engine scans your drives in a deterministic order to locate existing installations:
- **Steam**: Steam library folders (`steamapps/common/Railroad Tycoon 2`), including Steam Flatpak and Proton prefixes (`compatdata/284840/pfx/drive_c`).
- **GOG**: Windows registry entries (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`), standard `GOG Games/Railroad Tycoon II` folders, and Heroic Games Launcher directories.
- **Lutris & Wine**: Configured Lutris prefixes (`games/*.yml`), default Wine prefixes (`~/.wine`), Bottles, and PlayOnLinux.
- **Standard Windows Drives**: `C:\Games\`, `C:\Program Files (x86)\`, across drive letters `A:` through `Z:`.

If a valid installation is discovered, it appears in a clickable list with its detection source (e.g., "Steam", "GOG", "Wine prefix"). Simply select it and click **Play**.

### Manual Path Configuration

If your game is installed in a custom directory, you have several quick ways to point `rrt2` to it:
1. **Interactive Setup Screen**: Click **Choose folder...** to open your operating system's native file picker (utilizing XDG Desktop Portal on Linux, ensuring full compatibility within Flatpak).
2. **Command Line Utility**:
   ```bash
   ./rrt2 set-path ~/Games/railroad-tycoon-2
   # Windows:
   rrt2.exe set-path "C:\Games\Railroad Tycoon II"
   ```
3. **Command Line Flag or Environment Variable**:
   ```bash
   ./rrt2 --game-dir /path/to/game
   # Or via environment variable:
   export RT2_DIR=/path/to/game
   ./rrt2
   ```

### Packages by Operating System

Official releases provide native packages for major platforms:

- **Linux**:
  - **AppImage**: Standalone executable containing all UI vector fonts and translations. Mark executable (`chmod +x rrt2-*.AppImage`) and run directly.
  - **Flatpak**: Available via `io.github.pedryvo.rrt2.flatpak`. Completely sandboxed; use the in-game folder picker to grant permission to your game assets directory.
  - **Tarball (`.tar.gz`)**: Unpack anywhere and run `./rrt2`. Only requires `glibc` and `libasound.so.2` (Vulkan and Wayland/X11 are dynamically loaded).
- **Windows**:
  - **Inno Setup Installer (`-setup.exe`)**: Installs without requiring administrator privileges, adds a Start Menu shortcut, registers an uninstaller in Windows Settings, and can remember your game data path.
  - **winget**: Install via the Windows Package Manager:
    ```cmd
    winget install pedryvo.rrt2
    ```
  - **Portable ZIP**: Extract to any folder and execute `rrt2.exe`.
- **macOS**:
  - **Disk Image (`.dmg`)**: Open the `.dmg` and drag `RRT2.app` to your `/Applications` folder. Supports Apple Silicon (`aarch64`) and Intel (`x86_64`).
  - *Gatekeeper notice*: Because development builds are not notarized, macOS may display a security warning. Right-click `RRT2.app` and choose **Open**, or run:
    ```bash
    xattr -dr com.apple.quarantine /Applications/RRT2.app
    ```

### Configuration and Save Directories

The engine never modifies or writes files inside your original game installation. User data is strictly separated:

| File / Purpose | Linux | Windows | macOS |
|---|---|---|---|
| Configuration (`config.toml`) | `~/.config/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |
| Save Games (`*.GAM`, `*.toml`, `*.png`) | `~/.local/share/rrt2/saves/` | `%APPDATA%\rrt2\saves\` | `~/Library/Application Support/rrt2/saves/` |
| Exported Reports (`*.csv`) | `~/.local/share/rrt2/reports/` | `%APPDATA%\rrt2\reports\` | `~/Library/Application Support/rrt2/reports/` |
| High Scores (`hiscore.dat`) | `~/.local/share/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |

> [!TIP]
> You can override these storage locations at runtime using the environment variables `RRT2_CONFIG`, `RRT2_SAVES`, and `RRT2_REPORTS`.

---

## 2. Getting Started

### Game Modes: Campaigns, Scenarios & Sandbox

From the **Single Player** menu, choose your path to railway dominance:
- **Campaign (Original)**: Embark on the 18 historical scenarios that defined railway history, from early American pioneers (such as Baltimore & Ohio) to European and global expansion. Each scenario allows selecting starting bonuses (e.g., lower interest rates, construction subsidies, or locomotive discounts).
- **Campaign (The Second Century)**: Experience the 18 advanced scenarios introduced in the expansion pack, featuring modern electric locomotives, high-speed passenger routes, complex industrial logistics, and post-war economic shifts.
- **Single Scenario**: Pick any `.MAP` or `.MP2` map from your installation. Read the mission briefing, examine victory conditions (Bronze, Silver, and Gold goals), configure player competition, and choose industrial/financial difficulty models.
- **Sandbox Mode**: Play without financial limits, debt pressure, or aggressive competitor takeovers. Build expansive networks at your own pace.
- **Tutorial & Example Scenarios**: Great for beginners. Load the built-in tutorial map to practice track laying and basic cargo transport without financial ruin.

### Founding Your Railroad Company

When starting a scenario without an assigned company (or when creating a new enterprise):
1. **Name & Branding**: Type a company name (up to 30 characters) and select your railway's logo and company color.
2. **Initial Capital & Shares**: Use the founding sliders to balance starting capital. The preview calculates:
   - Initial cash balance for operations.
   - Initial share count and opening stock price.
   - Your personal investment as chairman ("You buy"): the percentage of stock you personally own from day one.
3. **Chairman Standing**: Keep your company solvent. If your company goes bankrupt or your personal stock holdings drop dangerously low, rival chairmen or your board of directors can vote to remove you.

### Laying Track

Track is the backbone of your empire:
1. Select the **Lay Track** tool on the left toolbar.
2. Click and hold the **left mouse button** on your starting tile, then drag across the terrain towards your destination.
3. **Real-Time Cost & Route Preview**: While dragging, the pathfinding engine displays the proposed track layout, bridges over rivers, slope grades, and the exact construction cost. Red highlights indicate obstructed terrain (e.g., steep mountains or unpurchased rights of way).
4. Release the mouse button to purchase and build the track.
5. Press `Esc` while dragging to cancel without spending cash.

> [!TIP]
> Keep track grades under 2% whenever possible. Steep slopes (3%–6%) drastically reduce locomotive top speed and cause excessive fuel consumption.

### Building Stations & Improvements

Stations collect passengers, mail, and industrial freight within their catchment area:
1. Click **Build Station** on the toolbar.
2. **Station Sizes**:
   - **Small Depot**: Low cost ($50K base), 2-tile catchment radius. Ideal for low-volume rural industries.
   - **Medium Station**: Moderate cost ($100K base), 4-tile catchment radius. Perfect for growing towns.
   - **Large Terminal**: High cost ($200K base), 8-tile catchment radius. Essential for large metropolitan hubs.
3. **Catchment Visualizer**: Hovering over the map displays a **golden circle** illustrating the catchment radius, alongside a colored footprint ghost:
   - **Green**: Valid placement.
   - **Yellow**: Valid placement adjacent to existing track.
   - **Red**: Invalid placement (blocked terrain, water, or overlapping station boundaries).
4. **Architectural Styles**: Choose from Victorian, Colonial, Mission, Tudor, or Kyoto station styles, matching regional themes.
5. **Station Improvements**: Click an existing station to open its management window and purchase upgrades:
   - **Operational**: *Water Tower* (speeds up steam replenishment), *Sandhouse* (improves traction on slopes), *Roundhouse* (cuts engine maintenance by 50%).
   - **Commercial**: *Telegraph* (increases revenue speed), *Hotel* & *Restaurant* (boost passenger revenue), *Customs House* (international cargo bonus).
   - **Industrial**: *Grain Silo*, *Cold Storage*, *Goods Shed* (stores cargo surpluses and prevents spoilage).

### Purchasing Locomotives

Click on any station or open the Trains overview and select **Purchase Train**:
- **Engine Selection**: Browse available engines by era and type (Steam, Diesel, Electric).
- **Performance Matrix**: The engine detail window displays a speed-versus-grade matrix (0%, 2%, 4%, and 6% slopes) across consists of 1 to 6 cars carrying 30 to 50 tons each.
- **Operating Costs**: Review annual maintenance, fuel consumption rates, reliability ratings, and acceleration curves before committing company capital.
- **Electrification**: Remember that electric locomotives require electrified track before they can run!

### Creating Consists & Setting Routes

1. **Routing**: Open the train's route planner. Click on stations on the minimap or from the station dropdown to add stops.
2. **Car Assignment**: Assign freight cars for each leg of the journey:
   - **Fast Freight**: Passengers, Mail (high value, demands speed; cargo degrades in value over time).
   - **Bulk Freight**: Coal, Iron Ore, Grain, Cattle, Oil, Logs, Steel, Manufactured Goods, Automobiles.
3. **Load Policies**: Configure whether trains should depart immediately, wait until half-full, or wait for a full load (`Wait for Full Cargo`).

### Financial Management & Stock Trading

Success in *Railroad Tycoon II* requires astute financial acumen:
- **Corporate Bonds**: Issue bonds to raise capital for large track expansions. Your interest rate depends directly on your company's credit rating (AAA down to Junk F). Repay bonds as soon as profits stabilize to eliminate interest overhead.
- **Stock Market (`M`)**:
   - **Buy / Sell Shares**: Invest personal cash into your own company to increase control, or buy shares in competitors to reap dividend returns.
   - **Share Buybacks**: Use company treasury cash to repurchase company stock from the open market, boosting earnings per share (EPS) and driving up the share price.
   - **Stock Splits**: When share prices soar, shares automatically split 2-for-1, improving liquidity.
- **Dividends**: Declare annual dividends. High dividends satisfy shareholders and lift your stock price, but drain company cash.
- **Mergers & Acquisitions**: Acquire more than 50% of a competitor's stock to initiate a corporate merger, absorbing their tracks, stations, and rolling stock.

---

## 3. Controls & Keyboard Shortcuts

All keyboard shortcuts are fully rebindable in **Options (`F10`) > Keys**.

### Camera Navigation & Zoom

| Action | Default Key / Input | Notes |
|---|---|---|
| Pan Camera | `W`, `A`, `S`, `D` or Arrow Keys | Smooth directional movement |
| Mouse Drag Pan | Right Mouse Button (Hold & Drag) | Natural camera dragging |
| Edge Scrolling | Screen Borders (within 4px) | Configurable in Gameplay settings |
| Rotate Map Left | `Q` | 90-degree isometric counter-clockwise rotation |
| Rotate Map Right | `E` | 90-degree isometric clockwise rotation |
| Zoom In | Mouse Wheel Up / `+` / `Numpad +` | Continuous, smooth zoom |
| Zoom Out | Mouse Wheel Down / `-` / `Numpad -` | Continuous, smooth zoom |
| Center Camera | `Home` | Re-centers camera view |

### Simulation Speed & Pausing

| Action | Default Key | Notes |
|---|---|---|
| Toggle Pause | `Pause` or `Space` | Freezes simulation ticks |
| Decrease Speed | `-` or `Numpad -` | Slows down game time |
| Increase Speed | `+` or `Numpad +` | Speeds up game time |
| Toggle Fullscreen | `F11` | Borderless fullscreen / windowed |
| Adjust UI Scale | `Ctrl +` / `Ctrl -` | Scales interface in 25% steps (100%–300%) |
| Mute Audio | `Ctrl + M` | Toggles sound effects and music |
| Next Music Track | `Ctrl + N` | Skips to next soundtrack piece |

### Quick Access & Information Panels

| Screen | Key | Description |
|---|---|---|
| **Finances** | `F` | Profit & Loss statement, balance sheet, credit rating, debt |
| **Stock Market** | `M` | Live stock ticker, share prices, buy/sell trades |
| **Trains List** | `T` | Overview of all locomotives, routes, cargo status, age |
| **Stations List** | `G` | Station throughput, waiting cargo, improvements, demand |
| **Cities** | `C` | City size, growth rate, supply and demand listings |
| **Industries** | `I` | Factory production, raw material stockpiles, buy/sell rights |
| **Map Overview** | Left Toolbar (Globe icon) | Fullscreen thematic overlays |
| **Quick Save** | `F5` | Saves to fixed quicksave slot |
| **Quick Load** | `F9` | Restores previous quicksave |
| **Save Dialog** | `Ctrl + S` | Opens save manager with custom filename |
| **Load Dialog** | `Ctrl + L` | Opens save browser with metadata & screenshots |
| **Options Menu** | `F10` or `Esc` | Opens audio, display, gameplay, and key settings |

### Financial Reports & Interactive Charts

Accessible from the Company Book (`Reports` button or keys `1`, `2`, `3`):
- **Interactive Charts (Tab 1)**: Visualizes up to 12 metrics over time: Revenue, Expenses, Profit, Cash, Debt, Equity, Share Price, Book Value, Earnings per Share, and Dividend per Share. Features high-contrast Okabe-Ito color palettes, selectable curves for each competing company, and mouse scrubbing to inspect monthly figures.
- **Income by Category (Tab 2)**: Detailed breakdown across all 13 financial categories comparing Lifetime totals, Current Year, and the previous 5 fiscal years.
- **Rankings (Tab 3)**: Performance leaderboards sorting your most profitable trains and busiest stations. Clicking any entry jumps the camera directly to that train or station.
- **Export to CSV**: Click **Export CSV** in any report tab to dump raw financial data to `~/.local/share/rrt2/reports/*.csv` for analysis in spreadsheets or external tools.

### Notification History

Located in the in-game Activity Box on the HUD:
- Click **History** to open a comprehensive ledger of up to 500 game events (Scenario milestones, economic shifts, stock market events, city growth, and competitor actions).
- Filter notifications by category or click **Show on Map** to immediately pan the camera to the exact tile where the event occurred.
- Notification popups and newspaper dialogs can be enabled or suppressed individually in **Options > Notices**.

### Gamepad & Steam Deck Controls

When running on handhelds like the Steam Deck or with standard game controllers:
- **Left Analog Stick**: Smooth camera panning across the terrain.
- **Right Analog Stick**: Virtual cursor positioning with adaptive snapping.
- **D-Pad**: Rapid navigation through menus, dialogue buttons, and station lists.
- **Triggers (L2 / R2)**: Fast simulation speed adjustment and pause toggling.
- **Bumpers (L1 / R1)**: Map rotation (`Q` / `E`) and zoom.
- **Action Buttons (A / B / X / Y)**: Select, Cancel / Close, Open Track Tool, Open Station Tool.

---

## 4. Differences from the 1998 Original

While preserving the unmistakable gameplay, atmosphere, and charm of *Railroad Tycoon II Platinum*, `rrt2-decomp` modernizes the underlying technology:

```
┌─────────────────────────┬─────────────────────────────────┬─────────────────────────────────┐
│ Feature                 │ Original Game (1998–2001)       │ rrt2-decomp (Rust Re-engine)    │
├─────────────────────────┼─────────────────────────────────┼─────────────────────────────────┤
│ Screen Resolution       │ Fixed 800x600 or 1024x768       │ Native 1080p, 1440p, 4K & Ultra │
│ UI Scaling              │ None (tiny pixels on HiDPI)     │ Crisp vector fonts (100%–300%)  │
│ Simulation Fidelity     │ Original Win32 x86 binary       │ 100% bit-exact & deterministic  │
│ Graphics Backend        │ DirectDraw / Software GDI       │ Hardware-accelerated wgpu       │
│ Camera Controls         │ Fixed zoom steps, rigid scroll  │ Smooth continuous zoom & drag   │
│ Multiplayer Protocol    │ DirectPlay 4, IPX, Modem, COM   │ Modern TCP/IP lockstep network  │
│ Multiplayer Features    │ Host drop kills game; no join   │ Host migration & join-in-prog.  │
│ Operating Systems       │ Windows 95/98/XP                │ Native Linux, Windows & macOS   │
│ Languages               │ Single language per disc/exe    │ 6 built-in languages on the fly │
└─────────────────────────┴─────────────────────────────────┴─────────────────────────────────┘
```

### Bit-Exact Deterministic Simulation

The simulation core (`rt2-sim`) does not guess formulas. Every mathematical calculation—train braking dynamics, breakdown probabilities, city cargo generation, stock market valuation, interest rates, and rival AI decision trees—was reverse-engineered and validated bit-for-bit against the original executable via differential emulator testing (Unicorn). Saves loaded into the engine match original save game behavior down to the last integer.

### Modern Displays, Widescreen & 4K

Say goodbye to low-resolution stretching. The wgpu rendering pipeline renders the terrain mesh, rivers, tracks, and stations natively at your display's exact resolution. Original 2D sprites undergo a high-grade upscaling pass (xBRZ and smart filtering) that is cached to disk on first run, providing breathtaking clarity on 4K and ultrawide monitors.

### Scalable Vector Typography & HiDPI

Instead of pixelated bitmap fonts, `rrt2-decomp` bundles open-source SIL OFL vector typefaces meticulously calibrated to match the original typography:
- **Cinzel**: Elegant serif small-caps replicating Copperplate Gothic.
- **Libre Franklin**: Crisp sans-serif for controls and tabular data.
- **Dancing Script**: Cursive script for awards and historical letters.
- **Anton & Oswald**: Heavy condensed display titles.
- **Noto Sans SC**: High-quality bundled CJK Hanzi typeface for Simplified Chinese.
- **Cormorant SC**: Classic Cyrillic small-caps for Russian localization.

UI scaling dynamically adjusts in 25% increments (from 100% to 300%), ensuring effortless readability on both 4K monitors and compact handheld screens.

### Modern TCP/IP Lockstep Multiplayer

The original DirectPlay 4 networking, IPX protocols, and modem dialer have been replaced with a high-performance, cross-platform TCP/IP implementation:
- **Deterministic Command Lockstep**: Clients transmit compact command packets; all machines advance the simulation synchronously.
- **Join in Progress & Reconnect**: Players who drop connection can rejoin; the host streams a compressed game state snapshot.
- **Automatic Host Migration**: If the host disconnects, the lowest client ID seamlessly assumes hosting duties without terminating the session.
- **Graceful AI Handoff**: If a human player leaves the game, their railroad company is instantly placed under competitor AI stewardship to prevent network desyncs.

### Dynamic Six-Language Localization

Switch between **English**, **Português**, **Español**, **Deutsch**, **中文 (Simplified Chinese)**, and **Русский** on the fly via **Options > Language**. The entire interface, dialog tree, and font metrics refresh instantly without restarting the game.

---

## 5. Optional Modern Features

All quality-of-life additions are modular and can be toggled without disturbing the vanilla simulation baseline.

### Rule-Based Original Bug Fixes (`Fix`)

The 1998 executable possessed a few well-documented bugs (e.g., occasional rounding anomalies in founder stock issuance, duplicate station improvement charges, and integer overflow quirks in multi-decade cash accumulation). In `rrt2-decomp`, you can choose:
- **Strict Classic Mode (Default)**: Preserves every original quirk for 100% bug-for-bug historical fidelity and replay verification.
- **Modern Fix Mode**: Applies audited mathematical fixes to balance calculations, recorded neatly in save game headers to ensure deterministic replayability.

### Construction Undo (`Ctrl+Z`)

Accidentally built a costly bridge or misclicked a station depot? Press `Ctrl + Z` to undo your most recent track or building construction step. Provided that no train has already traversed the newly placed rail section, you receive an immediate, full cash refund.

### Fleet Manager & Locomotive Upgrades

Managing a hundred locomotives across a sprawling continental network no longer requires tedious manual clicking:
- **Consist Templates**: Save standard car configurations (e.g., "Heavy Express Passenger" or "Coal Shuttle") and apply them to newly purchased trains in a single click.
- **Mass Locomotive Replacement**: Select an aging locomotive model across your fleet and schedule an automated upgrade to a newer engine class as soon as each train visits a station with a roundhouse.

### Advanced Map Overlays

Expand beyond the classic resource map with powerful analytical layers:
- **Cargo Supply & Demand Heatmap**: Highlights regions producing surplus cargo and cities experiencing urgent shortages.
- **Track Grade & Elevation**: Color-codes slopes to identify bottleneck inclines.
- **Corridor Profitability**: Visualizes the net profit generated per section of track, helping you identify unprofitable branch lines.
- **Traffic Congestion**: Pinpoints track junctions and single-line bottlenecks suffering from train queuing delays.

### TOML Data Modding

Game data is no longer locked inside binary tables:
- Engine specifications, car types, cargo definitions, industrial production trees, and scenario rules can be exported to and loaded from human-readable **TOML** files.
- Community modders can introduce brand-new historical locomotives, create unique custom industries, and balance custom scenarios without recompiling the Rust codebase.

### Accessibility Features

- **Colorblind-Safe Palettes**: Financial charts and minimap radar displays employ the battle-tested Okabe-Ito colorblind palette, paired with distinct marker shapes (circles, squares, triangles, diamonds) and varied line patterns.
- **Independent Font Scaling**: Adjust reading font sizes independently of the overall HUD graphical scale.
- **Visual Audio Cues**: Optional on-screen subtitle badges and icon pulses alert players with hearing impairments to critical train breakdowns, robber attacks, or urgent financial warnings.

---

*Railroad Tycoon is a registered trademark of its respective owners. This open-source fan project is not affiliated with Take-Two Interactive, PopTop Software, or Gathering of Developers.*
