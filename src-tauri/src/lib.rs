//! Nkore Browser – Tauri application entry point.
//!
//! This file wires together all modules and registers every Tauri command that
//! the frontend browser chrome can invoke.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adblocker;
mod browser;
mod db;
mod downloads;

use browser::{resolve_url, BrowserState, TabInfo};
use db::{Bookmark, Database, DownloadRecord, HistoryEntry};
use downloads::{DownloadInfo, DownloadManager, DownloadStatus};

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Listener, Manager, State};

// ── Application state types ───────────────────────────────────────────────────

pub struct AppState {
    pub browser: Mutex<BrowserState>,
    pub downloads: Mutex<DownloadManager>,
    pub db: Mutex<Database>,
}

// ══════════════════════════════════════════════════════════════════════════════
//  TAB COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/// Creates a new browser tab and returns its info.
#[tauri::command]
async fn new_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    url: Option<String>,
) -> Result<TabInfo, String> {
    let resolved = resolve_url(url.as_deref().unwrap_or("https://duckduckgo.com"));
    let blocker_script = adblocker::build_blocker_script();

    let mut browser = state.browser.lock().map_err(|e| e.to_string())?;

    let tab_id = browser.next_tab_id();

    // Collect IDs first to avoid borrow conflicts
    let existing_ids: Vec<String> = browser.tab_order.clone();

    // Hide all currently visible content webviews
    for existing_id in &existing_ids {
        if let Some(wv) = app.get_webview(existing_id) {
            let _ = wv.hide();
        }
    }

    // Mark all tabs as inactive
    for info in browser.tabs.values_mut() {
        info.is_active = false;
    }

    let parsed_url: url::Url = resolved.parse().map_err(|e: url::ParseError| e.to_string())?;

    let webview = browser::create_tab_webview(&app, &browser, &tab_id, parsed_url, &blocker_script)
        .map_err(|e| e.to_string())?;
    let _ = webview.show();

    let tab_info = TabInfo {
        id: tab_id.clone(),
        url: resolved,
        title: "Loading…".to_string(),
        is_active: true,
        is_loading: true,
        can_go_back: false,
        can_go_forward: false,
        favicon_url: None,
    };

    browser.tabs.insert(tab_id.clone(), tab_info.clone());
    browser.tab_order.push(tab_id.clone());
    browser.active_tab_id = Some(tab_id);

    // Notify the chrome of the updated tab list
    let _ = app.emit("tabs_updated", browser.ordered_tabs());

    Ok(tab_info)
}

/// Closes a tab by ID.
#[tauri::command]
async fn close_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> Result<Option<TabInfo>, String> {
    let mut browser = state.browser.lock().map_err(|e| e.to_string())?;

    if !browser.tabs.contains_key(&tab_id) {
        return Err(format!("Tab '{}' not found", tab_id));
    }

    // Close the webview
    if let Some(wv) = app.get_webview(&tab_id) {
        let _ = wv.close();
    }

    browser.tabs.remove(&tab_id);
    browser.tab_order.retain(|id| id != &tab_id);

    // If we closed the active tab, switch to the last remaining one
    let new_active = if browser.active_tab_id.as_deref() == Some(&tab_id) {
        let new_id = browser.tab_order.last().cloned();
        browser.active_tab_id = new_id.clone();

        if let Some(ref nid) = new_id {
            if let Some(wv) = app.get_webview(nid) {
                let _ = wv.show();
            }
            if let Some(info) = browser.tabs.get_mut(nid) {
                info.is_active = true;
            }
        }
        browser.tabs.get(new_id.as_deref().unwrap_or("")).cloned()
    } else {
        None
    };

    let _ = app.emit("tabs_updated", browser.ordered_tabs());
    Ok(new_active)
}

/// Switches to the given tab.
#[tauri::command]
async fn switch_tab(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
) -> Result<TabInfo, String> {
    let mut browser = state.browser.lock().map_err(|e| e.to_string())?;

    if !browser.tabs.contains_key(&tab_id) {
        return Err(format!("Tab '{}' not found", tab_id));
    }

    // Hide all other tabs (collect IDs first to avoid borrow conflict)
    let other_ids: Vec<String> = browser.tab_order.iter()
        .filter(|id| *id != &tab_id)
        .cloned()
        .collect();

    for id in &other_ids {
        if let Some(wv) = app.get_webview(id) {
            let _ = wv.hide();
        }
        if let Some(info) = browser.tabs.get_mut(id) {
            info.is_active = false;
        }
    }

    // Show selected tab
    if let Some(wv) = app.get_webview(&tab_id) {
        let _ = wv.show();
    }
    if let Some(info) = browser.tabs.get_mut(&tab_id) {
        info.is_active = true;
    }
    browser.active_tab_id = Some(tab_id.clone());

    let tab_info = browser.tabs[&tab_id].clone();
    let _ = app.emit("tabs_updated", browser.ordered_tabs());
    Ok(tab_info)
}

/// Navigates the active (or specified) tab to a URL or search query.
#[tauri::command]
async fn navigate(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    tab_id: Option<String>,
) -> Result<(), String> {
    let resolved = resolve_url(&input);
    let browser = state.browser.lock().map_err(|e| e.to_string())?;

    let target_id = tab_id
        .or_else(|| browser.active_tab_id.clone())
        .ok_or("No active tab")?;

    let webview = app.get_webview(&target_id).ok_or("Webview not found")?;
    let url: url::Url = resolved.parse().map_err(|e: url::ParseError| e.to_string())?;
    webview.navigate(url).map_err(|e| e.to_string())?;
    Ok(())
}

/// Go back in the active tab's history.
#[tauri::command]
async fn go_back(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let browser = state.browser.lock().map_err(|e| e.to_string())?;
    let id = browser.active_tab_id.as_ref().ok_or("No active tab")?.clone();
    drop(browser);
    let webview = app.get_webview(&id).ok_or("Webview not found")?;
    webview
        .eval("window.history.back();")
        .map_err(|e| e.to_string())
}

/// Go forward in the active tab's history.
#[tauri::command]
async fn go_forward(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let browser = state.browser.lock().map_err(|e| e.to_string())?;
    let id = browser.active_tab_id.as_ref().ok_or("No active tab")?.clone();
    drop(browser);
    let webview = app.get_webview(&id).ok_or("Webview not found")?;
    webview
        .eval("window.history.forward();")
        .map_err(|e| e.to_string())
}

/// Reload the active tab.
#[tauri::command]
async fn reload(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let browser = state.browser.lock().map_err(|e| e.to_string())?;
    let id = browser.active_tab_id.as_ref().ok_or("No active tab")?.clone();
    drop(browser);
    let webview = app.get_webview(&id).ok_or("Webview not found")?;
    webview
        .eval("window.location.reload();")
        .map_err(|e| e.to_string())
}

/// Returns the complete list of open tabs.
#[tauri::command]
async fn get_tabs(state: State<'_, AppState>) -> Result<Vec<TabInfo>, String> {
    let browser = state.browser.lock().map_err(|e| e.to_string())?;
    Ok(browser.ordered_tabs())
}

/// Called by the chrome when a content webview reports a URL/title change.
#[tauri::command]
async fn update_tab_info(
    app: AppHandle,
    state: State<'_, AppState>,
    tab_id: String,
    url: String,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
    favicon_url: Option<String>,
) -> Result<(), String> {
    {
        let mut browser = state.browser.lock().map_err(|e| e.to_string())?;
        if let Some(info) = browser.tabs.get_mut(&tab_id) {
            info.url = url.clone();
            info.title = title.clone();
            info.can_go_back = can_go_back;
            info.can_go_forward = can_go_forward;
            info.favicon_url = favicon_url;
            info.is_loading = false;
        }
        let _ = app.emit("tabs_updated", browser.ordered_tabs());
    }

    // Persist to history
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let _ = db.add_history(&url, &title);

    Ok(())
}

/// Scans the active tab for media elements and returns them.
#[tauri::command]
async fn scan_media(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let browser = state.browser.lock().map_err(|e| e.to_string())?;
    let id = browser.active_tab_id.as_ref().ok_or("No active tab")?.clone();
    drop(browser);

    let webview = app.get_webview(&id).ok_or("Webview not found")?;

    // Trigger the scan script, collect results, emit them as an event
    webview
        .eval(r#"
            (function() {
                var result = window.__nkoreScanMedia ? window.__nkoreScanMedia() : '[]';
                if (window.__TAURI__) {
                    window.__TAURI__.event.emit('media_scan_result', result);
                }
            })();
        "#)
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "status": "scan_requested", "tabId": id }))
}

// ══════════════════════════════════════════════════════════════════════════════
//  DOWNLOAD COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    save_path: Option<String>,
) -> Result<String, String> {
    let mut dm = state.downloads.lock().map_err(|e| e.to_string())?;
    let id = dm.start(app.clone(), url.clone(), save_path.clone());

    // Persist initial record to DB
    let record = DownloadRecord {
        id: id.clone(),
        url,
        filename: dm.downloads[&id].filename.clone(),
        save_path: dm.downloads[&id].save_path.clone(),
        file_size: 0,
        downloaded_bytes: 0,
        status: "downloading".to_string(),
        created_at: dm.downloads[&id].created_at.clone(),
        completed_at: None,
        mime_type: None,
    };

    if let Ok(db) = state.db.lock() {
        let _ = db.insert_download(&record);
    }

    Ok(id)
}

#[tauri::command]
async fn cancel_download(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let mut dm = state.downloads.lock().map_err(|e| e.to_string())?;
    Ok(dm.cancel(&id))
}

#[tauri::command]
async fn get_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadInfo>, String> {
    let dm = state.downloads.lock().map_err(|e| e.to_string())?;
    Ok(dm.get_all())
}

#[tauri::command]
async fn clear_finished_downloads(state: State<'_, AppState>) -> Result<(), String> {
    let mut dm = state.downloads.lock().map_err(|e| e.to_string())?;
    dm.clear_finished();
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
//  BOOKMARK COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn add_bookmark(
    state: State<'_, AppState>,
    url: String,
    title: String,
) -> Result<Bookmark, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_bookmark(&url, &title).map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_bookmark(state: State<'_, AppState>, url: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.remove_bookmark(&url)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_bookmarks(state: State<'_, AppState>) -> Result<Vec<Bookmark>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_bookmarks().map_err(|e| e.to_string())
}

#[tauri::command]
async fn is_bookmarked(state: State<'_, AppState>, url: String) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.is_bookmarked(&url).map_err(|e| e.to_string())
}

// ══════════════════════════════════════════════════════════════════════════════
//  HISTORY COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn get_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<HistoryEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_history(limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_history(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<HistoryEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.search_history(&query).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.clear_history().map(|_| ()).map_err(|e| e.to_string())
}

// ══════════════════════════════════════════════════════════════════════════════
//  UTILITY COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/// Returns the URL that would be navigated to for the given user input
/// (for address-bar autocomplete previews).
#[tauri::command]
fn resolve_input_url(input: String) -> String {
    resolve_url(&input)
}

// ══════════════════════════════════════════════════════════════════════════════
//  MAIN / SETUP
// ══════════════════════════════════════════════════════════════════════════════

pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // ── Data directory ───────────────────────────────────────────────
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            std::fs::create_dir_all(&data_dir).ok();

            // ── Database ─────────────────────────────────────────────────────
            let db_path = data_dir.join("nkore.db");
            let database = Database::open(&db_path)
                .expect("Failed to open/create Nkore database");

            // ── Download directory ───────────────────────────────────────────
            let download_dir = dirs::download_dir()
                .unwrap_or_else(|| data_dir.join("downloads"));
            std::fs::create_dir_all(&download_dir).ok();

            let download_manager = DownloadManager::new(download_dir);

            // ── Browser state ────────────────────────────────────────────────
            let mut browser_state = BrowserState::new();

            let blocker_script = adblocker::build_blocker_script();

            browser::setup_browser_window(app.handle(), &mut browser_state, &blocker_script)
                .expect("Failed to create browser window");

            // ── Register shared state ────────────────────────────────────────
            app.manage(AppState {
                browser: Mutex::new(browser_state),
                downloads: Mutex::new(download_manager),
                db: Mutex::new(database),
            });

            // ── Listen for download events emitted by the download tasks ─────
            let app_handle = app.handle().clone();
            app.listen("download://status", move |event| {
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let id = payload["id"].as_str().unwrap_or("").to_string();
                    let status = payload["status"].as_str().unwrap_or("").to_string();
                    let error = payload["error"].as_str().map(|s| s.to_string());

                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Ok(mut dm) = state.downloads.lock() {
                            let ds = match status.as_str() {
                                "completed" => DownloadStatus::Completed,
                                "failed" => DownloadStatus::Failed,
                                "cancelled" => DownloadStatus::Cancelled,
                                _ => DownloadStatus::Downloading,
                            };
                            dm.set_status(&id, ds, error.clone());
                        }
                        if let Ok(db) = state.db.lock() {
                            let _ = db.complete_download(&id, &status);
                        }
                    }
                    // Re-broadcast for the chrome UI
                    let _ = app_handle.emit("download://status", event.payload());
                }
            });

            let app_handle2 = app.handle().clone();
            app.listen("download://progress", move |event| {
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let id = payload["id"].as_str().unwrap_or("").to_string();
                    let downloaded = payload["downloaded"].as_u64().unwrap_or(0);
                    let total = payload["total"].as_u64().unwrap_or(0);
                    let speed = payload["speedBps"].as_u64().unwrap_or(0);

                    if let Some(state) = app_handle2.try_state::<AppState>() {
                        if let Ok(mut dm) = state.downloads.lock() {
                            dm.update_progress(&id, downloaded, total, speed);
                        }
                        if let Ok(db) = state.db.lock() {
                            let _ = db.update_download_progress(&id, downloaded as i64, total as i64, "downloading");
                        }
                    }
                    let _ = app_handle2.emit("download://progress", event.payload());
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Tabs
            new_tab,
            close_tab,
            switch_tab,
            navigate,
            go_back,
            go_forward,
            reload,
            get_tabs,
            update_tab_info,
            scan_media,
            // Downloads
            start_download,
            cancel_download,
            get_downloads,
            clear_finished_downloads,
            // Bookmarks
            add_bookmark,
            remove_bookmark,
            get_bookmarks,
            is_bookmarked,
            // History
            get_history,
            search_history,
            clear_history,
            // Utility
            resolve_input_url,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Nkore Browser");
}
