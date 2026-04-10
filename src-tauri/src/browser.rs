//! Browser tab and webview management for Nkore.
//!
//! Each tab corresponds to a Tauri Webview positioned inside the main Window,
//! below the chrome bar.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowBuilder};

pub const CHROME_HEIGHT: f64 = 72.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub is_active: bool,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub favicon_url: Option<String>,
}

pub struct BrowserState {
    pub tab_order: Vec<String>,
    pub tabs: HashMap<String, TabInfo>,
    pub active_tab_id: Option<String>,
    pub tab_counter: u32,
    pub chrome_height: f64,
}

impl BrowserState {
    pub fn new() -> Self {
        Self {
            tab_order: Vec::new(),
            tabs: HashMap::new(),
            active_tab_id: None,
            tab_counter: 0,
            chrome_height: CHROME_HEIGHT,
        }
    }

    pub fn next_tab_id(&mut self) -> String {
        let id = format!("tab_{}", self.tab_counter);
        self.tab_counter += 1;
        id
    }

    pub fn ordered_tabs(&self) -> Vec<TabInfo> {
        self.tab_order
            .iter()
            .filter_map(|id| self.tabs.get(id))
            .cloned()
            .collect()
    }
}

/// Resolves user input to a navigable URL, defaulting to DuckDuckGo search.
pub fn resolve_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "https://duckduckgo.com".to_string();
    }
    if let Ok(url) = url::Url::parse(trimmed) {
        if matches!(url.scheme(), "http" | "https" | "file") {
            return trimmed.to_string();
        }
    }
    if !trimmed.contains(' ') && trimmed.contains('.') {
        return format!("https://{}", trimmed);
    }
    let query = urlencoding_encode(trimmed);
    format!("https://duckduckgo.com/?q={}", query)
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Creates the main browser window with chrome + first content tab.
/// Requires Tauri `unstable` feature for multi-webview support.
pub fn setup_browser_window(
    app: &AppHandle,
    state: &mut BrowserState,
    blocker_script: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let window = WindowBuilder::new(app, "main")
        .title("Nkore")
        .inner_size(1400.0, 900.0)
        .min_inner_size(800.0, 600.0)
        .decorations(true)
        .build()?;

    let win_size = window.inner_size()?;
    let w = win_size.width as f64;
    let h = win_size.height as f64;
    let chrome_h = state.chrome_height;

    #[cfg(debug_assertions)]
    let chrome_url = WebviewUrl::External("http://localhost:5173".parse()?);
    #[cfg(not(debug_assertions))]
    let chrome_url = WebviewUrl::App("index.html".into());

    let chrome_builder = WebviewBuilder::new("chrome", chrome_url)
        .transparent(false)
        .focused(true);

    window.add_child(
        chrome_builder,
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(w, chrome_h),
    )?;

    let tab_id = state.next_tab_id();
    let new_tab_url: url::Url = "https://duckduckgo.com".parse()?;

    let tab_builder = WebviewBuilder::new(&tab_id, WebviewUrl::External(new_tab_url))
        .initialization_script(blocker_script);

    window.add_child(
        tab_builder,
        LogicalPosition::new(0.0, chrome_h),
        LogicalSize::new(w, h - chrome_h),
    )?;

    let tab_info = TabInfo {
        id: tab_id.clone(),
        url: "https://duckduckgo.com".to_string(),
        title: "DuckDuckGo".to_string(),
        is_active: true,
        is_loading: true,
        can_go_back: false,
        can_go_forward: false,
        favicon_url: None,
    };

    state.tabs.insert(tab_id.clone(), tab_info);
    state.tab_order.push(tab_id.clone());
    state.active_tab_id = Some(tab_id);

    Ok(())
}

/// Creates a new content webview for a tab inside the main window.
pub fn create_tab_webview(
    app: &AppHandle,
    state: &BrowserState,
    tab_id: &str,
    url: url::Url,
    blocker_script: &str,
) -> Result<tauri::Webview<tauri::Wry>, Box<dyn std::error::Error>> {
    let window = app.get_window("main").ok_or("Main window not found")?;
    let win_size = window.inner_size()?;
    let w = win_size.width as f64;
    let h = win_size.height as f64;
    let chrome_h = state.chrome_height;

    let builder = WebviewBuilder::new(tab_id, WebviewUrl::External(url))
        .initialization_script(blocker_script);

    let webview = window.add_child(
        builder,
        LogicalPosition::new(0.0, chrome_h),
        LogicalSize::new(w, h - chrome_h),
    )?;

    Ok(webview)
}

/// Resizes all webviews when the window is resized.
pub fn resize_tabs(app: &AppHandle, state: &BrowserState) {
    let window = match app.get_window("main") {
        Some(w) => w,
        None => return,
    };
    let size = match window.inner_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let w = size.width as f64;
    let h = size.height as f64;
    let chrome_h = state.chrome_height;

    if let Some(chrome) = app.get_webview("chrome") {
        let _ = chrome.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: w,
            height: chrome_h,
        }));
    }

    for tab_id in &state.tab_order {
        if let Some(webview) = app.get_webview(tab_id) {
            let _ = webview.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: w,
                height: h - chrome_h,
            }));
        }
    }
}
