# Nkore Browser

A privacy-focused, dark-themed desktop web browser built with [Tauri v2](https://tauri.app/) (Rust + WebView).

> **Platforms:** Windows · macOS · Linux  
> **Default search engine:** DuckDuckGo  
> **Theme:** Dark, minimalist

---

## Features

| Feature | Description |
|---------|-------------|
| 🌐 **Multi-tab browsing** | Full multi-tab support with keyboard shortcuts |
| 🔒 **Ad & Tracker Blocker** | Built-in, zero-config ad blocking via JS injection |
| 📥 **Download Manager** | In-app download panel with progress, speed, and ETA |
| 🔖 **Bookmarks** | One-click bookmarking with a dedicated panel |
| 🕐 **History** | Searchable browsing history with clear functionality |
| 🎬 **Media Sniffer** | Detects all video/audio/image media on the current page and lets you download any item |
| 🌑 **Dark Theme** | Sleek, modern dark UI by default |

---

## Prerequisites

Install the required tools for your platform before building:

### All Platforms
- [Node.js ≥ 18](https://nodejs.org/) (includes npm)
- [Rust + Cargo](https://rustup.rs/) (stable channel)
- [Tauri prerequisites](https://tauri.app/start/prerequisites/)

### Linux (Debian/Ubuntu)
```bash
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

### macOS
```bash
xcode-select --install
```

### Windows
- Install **Microsoft Edge WebView2 Runtime** (usually pre-installed on Windows 11)
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload

---

## Getting Started

### 1. Clone & install dependencies

```bash
git clone https://github.com/akampuraa-cmd/Nkore-Browser.git
cd Nkore-Browser
npm install
```

### 2. Run in development mode

```bash
npm run tauri dev
```

This starts the Vite dev server on port 5173 and launches Nkore with hot-reload for the browser chrome UI.

### 3. Build for production

```bash
npm run tauri build
```

Produces a platform-specific installer / binary in `src-tauri/target/release/bundle/`.

---

## Project Structure

```
Nkore-Browser/
├── index.html                 # Browser chrome HTML entry
├── vite.config.js             # Vite configuration
├── package.json
├── src/
│   ├── main.js                # Frontend entry point
│   ├── style.css              # Dark theme styles
│   └── components/
│       ├── tabs.js            # Tab bar
│       ├── navigation.js      # Address bar & nav buttons
│       └── panels/
│           ├── downloads.js   # Download manager panel
│           ├── bookmarks.js   # Bookmarks panel
│           ├── history.js     # History panel
│           └── media-sniffer.js # Media detection panel
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── src/
│       ├── main.rs            # Rust entry point
│       ├── lib.rs             # Commands, setup, event wiring
│       ├── browser.rs         # Tab / webview management
│       ├── downloads.rs       # Download manager
│       ├── adblocker.rs       # Ad/tracker blocking scripts
│       └── db.rs              # SQLite persistence
├── ARCHITECTURE.md            # Full system architecture doc
└── README.md
```

---

## Packaging & Distribution

After `npm run tauri build`, find the artifacts in:

| Platform | Location | Format |
|----------|----------|--------|
| Windows  | `src-tauri/target/release/bundle/msi/` | `.msi` installer |
| Windows  | `src-tauri/target/release/bundle/nsis/` | `.exe` installer |
| macOS    | `src-tauri/target/release/bundle/macos/` | `.app` bundle |
| macOS    | `src-tauri/target/release/bundle/dmg/` | `.dmg` disk image |
| Linux    | `src-tauri/target/release/bundle/deb/` | `.deb` package |
| Linux    | `src-tauri/target/release/bundle/appimage/` | `.AppImage` |
| Linux    | `src-tauri/target/release/bundle/rpm/` | `.rpm` package |

### Code Signing (optional, recommended for distribution)

Set the following environment variables before building:

**macOS:**
```bash
export APPLE_CERTIFICATE="<base64-encoded P12>"
export APPLE_CERTIFICATE_PASSWORD="<password>"
export APPLE_ID="<your-apple-id>"
export APPLE_ID_PASSWORD="<app-specific-password>"
export APPLE_TEAM_ID="<team-id>"
npm run tauri build
```

**Windows:**
```bash
set TAURI_SIGNING_PRIVATE_KEY=<path-to-key.pem>
set TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<password>
npm run tauri build
```

---

## Architecture

See [ARCHITECTURE.md](./ARCHITECTURE.md) for:
- Full system architecture diagram
- Module descriptions
- Data flow examples
- Database schema
- Privacy & security feature matrix

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close current tab |
| `Ctrl+L` | Focus address bar |
| `Ctrl+R` / `F5` | Reload page |
| `Ctrl+D` | Toggle bookmark |
| `Alt+←` | Go back |
| `Alt+→` | Go forward |
| `Escape` | Close open panels |

---

## License

MIT
