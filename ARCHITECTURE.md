# Nkore Browser – System Architecture

## Overview

Nkore is a privacy-focused desktop web browser built with **Tauri v2** (Rust backend + WebView frontend). It targets Windows, macOS, and Linux using each platform's native WebView engine:

| Platform | WebView Engine |
|----------|---------------|
| Windows  | WebView2 (Chromium-based) |
| macOS    | WKWebView (WebKit) |
| Linux    | WebKitGTK |

---

## Technology Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Runtime framework | **Tauri v2** | Native desktop wrapper, IPC bridge, window management |
| Backend language | **Rust 2021** | Performance, memory safety, system-level access |
| Frontend framework | **Vanilla JS + Vite** | Browser chrome UI, no heavy framework overhead |
| Persistence | **SQLite (rusqlite)** | Bookmarks, history, download records |
| HTTP client | **reqwest** | File downloads with streaming progress |
| Build/package | **Tauri CLI + Vite** | Cross-platform bundling and distribution |

---

## Architecture Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                      OS Window (Tauri v2)                     │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │             Chrome Webview (72 px height)             │   │
│  │  ┌────────┐ ┌──────────────────────────┐ ┌────────┐  │   │
│  │  │ Nav    │ │     Address / Search Bar  │ │ Tools  │  │   │
│  │  │ Btns   │ │  (DuckDuckGo default)    │ │ Btns   │  │   │
│  │  └────────┘ └──────────────────────────┘ └────────┘  │   │
│  │  [Tab 1] [Tab 2] [Tab 3] [+]                          │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │           Content Webview (active tab)                │   │
│  │                                                       │   │
│  │  External web page rendered here (WebKitGTK/WKWebView │   │
│  │  /WebView2 depending on platform)                     │   │
│  │                                                       │   │
│  │  ← JavaScript injection by Nkore:                     │   │
│  │    • Ad / tracker blocker (fetch + XHR override)      │   │
│  │    • Media sniffer (MutationObserver + DOM scan)      │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────── Side Panels (overlay) ─────────────┐  │
│  │  Downloads │ Bookmarks │ History │ Media Sniffer        │  │
│  └─────────────────────────────────────────────────────── ┘  │
└──────────────────────────────────────────────────────────────┘
           │  Tauri IPC  │
           ▼             ▼
┌──────────────────────────────────────────────────────────────┐
│                   Rust Backend (src-tauri)                    │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │  browser.rs  │  │ downloads.rs │  │ adblocker.rs │       │
│  │  Tab / webview│  │ reqwest DL   │  │ JS injection │       │
│  │  management  │  │ + progress   │  │ scripts      │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│  ┌──────────────┐                                            │
│  │    db.rs     │                                            │
│  │  SQLite via  │                                            │
│  │  rusqlite    │                                            │
│  └──────────────┘                                            │
└──────────────────────────────────────────────────────────────┘
```

---

## Module Descriptions

### Backend (Rust)

#### `src-tauri/src/main.rs`
Entry point. Delegates to `lib.rs::run()`.

#### `src-tauri/src/lib.rs`
- Registers all Tauri plugins (dialog, fs, http, notification, shell).
- Sets up the `AppState` (browser state, download manager, database).
- Registers all `#[tauri::command]` handlers.
- Calls `browser::setup_browser_window()` in the `setup` hook.
- Listens for `download://progress` and `download://status` events from spawned download tasks, updating state and re-broadcasting to the chrome.

#### `src-tauri/src/browser.rs`
- Defines `BrowserState` (ordered tab list, active tab ID, chrome height).
- `setup_browser_window()` – creates the main OS window with:
  - A chrome webview loaded from the Vite dev server (dev) or `dist/` (release).
  - An initial content tab pointing to `https://duckduckgo.com`.
- `resize_tabs()` – called on window resize to keep content webviews filling the available area.
- `resolve_url()` – turns user input into a navigable URL (domain detection or DuckDuckGo search fallback).

#### `src-tauri/src/downloads.rs`
- `DownloadManager` – owns a map of in-progress downloads and a shared `reqwest::Client`.
- `start()` – spawns a `tokio` task that streams the download, emitting `download://started`, `download://progress`, and `download://completed` events.
- `cancel()` – sends a `Cancel` signal via an mpsc channel to the running task.
- Implements rolling-average speed calculation and ETA estimation.

#### `src-tauri/src/adblocker.rs`
- `build_blocker_script()` – returns a JavaScript string injected into **every** content webview via `WebviewBuilder::initialization_script()`.
  - Overrides `window.fetch`, `XMLHttpRequest.open/send`, and `WebSocket` to silently drop requests to known ad/tracker domains.
  - Injects CSS to collapse common ad placeholder elements.
  - Sets up a `MutationObserver` to continuously collect media URLs into `window.__nkoreMedia`.
  - Exposes `window.__nkoreScanMedia()` for on-demand media scanning.
- Contains a curated blocklist of ~90 ad network and tracker domains (a representative subset of EasyList / EasyPrivacy).

#### `src-tauri/src/db.rs`
- `Database` wrapper around a `rusqlite::Connection`.
- Schema migrations for `bookmarks`, `history`, and `downloads` tables.
- Provides typed CRUD methods used by command handlers in `lib.rs`.

---

### Frontend (JavaScript)

All frontend code is compiled by **Vite** and runs inside the chrome webview.

#### `index.html`
Static HTML skeleton for the browser chrome (address bar, tab bar, panel overlays).

#### `src/style.css`
Complete dark-theme stylesheet. Uses CSS custom properties for consistent theming.

#### `src/main.js`
Bootstrap: imports all component modules, wires panel-toggle buttons and global keyboard shortcuts.

#### `src/components/tabs.js`
- Renders the tab strip.
- Calls `new_tab`, `close_tab`, `switch_tab` Tauri commands.
- Listens for `tabs_updated` events from the backend to keep the UI in sync.

#### `src/components/navigation.js`
- Manages the address bar and navigation buttons.
- Address bar autocomplete powered by `search_history` + `resolve_input_url`.
- Keeps the security icon, bookmark button, and window title in sync with the active tab.

#### `src/components/panels/downloads.js`
- Renders download items with progress bars, speed, and ETA.
- Listens for `download://progress`, `download://status`, `download://started` events.
- Provides Cancel and "Show in folder" buttons.
- Displays an active-download badge on the toolbar button.

#### `src/components/panels/bookmarks.js`
- Lists bookmarks from the backend.
- Each item navigates on click; has a remove button.

#### `src/components/panels/history.js`
- Displays browsing history grouped by day.
- Includes a search field and a "Clear All" button.

#### `src/components/panels/media-sniffer.js`
- Triggers `scan_media` command on the backend.
- Listens for `media_scan_result` events with the collected media list.
- Renders filterable (All / Video / Audio / Images) list with thumbnails.
- Each item has a "Download" button that calls `start_download`.

---

## Data Flow: New Tab Creation

```
User clicks [+]
      │
      ▼
tabs.js::newTab()
      │ invoke('new_tab', { url })
      ▼
lib.rs::new_tab()
  ├─ Locks BrowserState
  ├─ Hides all existing tab webviews
  ├─ WebviewBuilder::new(tab_id, url)
  │    .initialization_script(adblocker_js)
  │    .build_on_window(&window)
  ├─ Updates BrowserState
  └─ app.emit('tabs_updated', tabs)
              │
              ▼
         tabs.js listens
         Rerenders tab strip
         Calls nav.onTabChanged()
```

## Data Flow: Media Scan

```
User clicks [Media Sniffer] or [Scan Page]
      │
      ▼
media-sniffer.js::scanMedia()
      │ invoke('scan_media')
      ▼
lib.rs::scan_media()
      │ webview.eval("window.__nkoreScanMedia && ...")
      │
      │   (Content webview runs the injected scanner)
      │   window.__nkoreMedia is populated
      │   The init script emits back... however, since direct
      │   eval return values aren't available, the content page
      │   dispatches a message via a custom protocol or the
      │   front-end polls via a subsequent eval call
      ▼
'media_scan_result' event arrives in chrome webview
      │
      ▼
media-sniffer.js::renderMedia()
```

---

## Privacy & Security Features

| Feature | Implementation |
|---------|---------------|
| **Ad Blocking** | JavaScript init-script overrides `fetch`/`XHR`/`WebSocket` to drop requests to ~90 ad/tracker domains; CSS hides common ad elements |
| **Tracker Blocking** | Same blocklist covers analytics and tracking pixels |
| **HTTPS visual indicator** | Security icon in address bar shows lock (HTTPS), warning (HTTP), or neutral |
| **No telemetry** | No analytics, crash reporting, or phone-home functionality |
| **Default search: DuckDuckGo** | All searches go to `https://duckduckgo.com/?q=…` |
| **Local-only persistence** | All history, bookmarks, and download records stored locally in SQLite |

---

## Database Schema

```sql
CREATE TABLE bookmarks (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    url        TEXT    NOT NULL UNIQUE,
    title      TEXT    NOT NULL DEFAULT '',
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    url         TEXT    NOT NULL,
    title       TEXT    NOT NULL DEFAULT '',
    visited_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    visit_count INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE downloads (
    id               TEXT    PRIMARY KEY,
    url              TEXT    NOT NULL,
    filename         TEXT    NOT NULL,
    save_path        TEXT    NOT NULL,
    file_size        INTEGER NOT NULL DEFAULT 0,
    downloaded_bytes INTEGER NOT NULL DEFAULT 0,
    status           TEXT    NOT NULL DEFAULT 'downloading',
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    completed_at     TEXT,
    mime_type        TEXT
);
```

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
| `Escape` | Close panels / autocomplete |
