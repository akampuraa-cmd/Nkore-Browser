/**
 * Tab bar component for Nkore Browser.
 *
 * Manages the visual tab strip and communicates with the Rust backend via
 * Tauri IPC to create, close, and switch between browser tabs.
 */

const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

/** @type {Array<TabInfo>} */
let tabs = [];
let activeTabId = null;

// ── DOM elements ──────────────────────────────────────────────────────────────
const tabsList    = () => document.getElementById('tabs-list');
const newTabBtn   = () => document.getElementById('btn-new-tab');
const loadingBar  = () => document.getElementById('loading-bar');

// ── Favicon helper ────────────────────────────────────────────────────────────
function faviconUrl(tab) {
  if (tab.favicon_url) return tab.favicon_url;
  try {
    const origin = new URL(tab.url).origin;
    return `${origin}/favicon.ico`;
  } catch {
    return null;
  }
}

// ── Render ────────────────────────────────────────────────────────────────────
function renderTabs() {
  const list = tabsList();
  if (!list) return;
  list.innerHTML = '';

  tabs.forEach(tab => {
    const el = document.createElement('div');
    el.className = 'tab' + (tab.is_active ? ' active' : '');
    el.dataset.tabId = tab.id;
    el.title = tab.title || tab.url;

    // Favicon / spinner
    if (tab.is_loading) {
      const spinner = document.createElement('div');
      spinner.className = 'tab-loading';
      el.appendChild(spinner);
    } else {
      const fav = document.createElement('img');
      fav.className = 'tab-favicon';
      const src = faviconUrl(tab);
      fav.src = src || '';
      fav.onerror = () => { fav.style.display = 'none'; };
      el.appendChild(fav);
    }

    // Title
    const title = document.createElement('span');
    title.className = 'tab-title';
    title.textContent = tab.title || tab.url || 'New Tab';
    el.appendChild(title);

    // Close button
    const closeBtn = document.createElement('button');
    closeBtn.className = 'tab-close';
    closeBtn.innerHTML = '✕';
    closeBtn.title = 'Close tab';
    closeBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      closeTab(tab.id);
    });
    el.appendChild(closeBtn);

    // Click to switch
    el.addEventListener('click', () => switchTab(tab.id));

    list.appendChild(el);
  });
}

// ── IPC Actions ───────────────────────────────────────────────────────────────

export async function newTab(url) {
  try {
    showLoadingBar();
    const tab = await invoke('new_tab', { url: url || null });
    // tabs array will be updated via the tabs_updated event
    return tab;
  } catch (err) {
    console.error('[Nkore] new_tab error:', err);
  }
}

export async function closeTab(tabId) {
  try {
    await invoke('close_tab', { tabId });
  } catch (err) {
    console.error('[Nkore] close_tab error:', err);
  }
}

export async function switchTab(tabId) {
  if (tabId === activeTabId) return;
  try {
    await invoke('switch_tab', { tabId });
  } catch (err) {
    console.error('[Nkore] switch_tab error:', err);
  }
}

export function closeActiveTab() {
  if (activeTabId) closeTab(activeTabId);
}

export function getActiveTab() {
  return tabs.find(t => t.is_active) || null;
}

// ── Loading bar ───────────────────────────────────────────────────────────────
function showLoadingBar() {
  loadingBar()?.classList.remove('hidden');
}
function hideLoadingBar() {
  loadingBar()?.classList.add('hidden');
}

// ── Init ──────────────────────────────────────────────────────────────────────
export function initTabs() {
  // New-tab button
  newTabBtn()?.addEventListener('click', () => newTab());

  // Listen for backend tab updates
  listen('tabs_updated', (event) => {
    tabs = event.payload || [];
    const active = tabs.find(t => t.is_active);
    activeTabId = active?.id || null;
    renderTabs();

    if (active) {
      hideLoadingBar();
      // Notify navigation bar
      window.__nkoreNav?.onTabChanged(active);
    }
  });

  // Load initial tab list
  invoke('get_tabs').then(list => {
    tabs = list || [];
    renderTabs();
    const active = tabs.find(t => t.is_active);
    if (active) {
      activeTabId = active.id;
      window.__nkoreNav?.onTabChanged(active);
    }
  }).catch(console.error);

  // Expose to other modules
  window.__nkoreTabs = { newTab, closeActiveTab, getActiveTab, switchTab };
}
