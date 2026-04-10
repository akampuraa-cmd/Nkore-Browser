/**
 * Navigation component for Nkore Browser.
 *
 * Manages the address/search bar, navigation buttons (back, forward, reload,
 * home), bookmark toggle, and address-bar autocomplete.
 */

const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

let currentTab = null;
let isBookmarked = false;
let autocompleteResults = [];
let selectedAutocompleteIndex = -1;

// ── DOM references ────────────────────────────────────────────────────────────
const addressInput    = () => document.getElementById('address-input');
const securityIcon    = () => document.getElementById('security-icon');
const btnBack         = () => document.getElementById('btn-back');
const btnForward      = () => document.getElementById('btn-forward');
const btnReload       = () => document.getElementById('btn-reload');
const btnBookmark     = () => document.getElementById('btn-bookmark');
const autocompleteEl  = () => document.getElementById('autocomplete-dropdown');

// ── Navigation actions ────────────────────────────────────────────────────────

export async function navigate(input) {
  if (!input?.trim()) return;
  try {
    document.getElementById('loading-bar')?.classList.remove('hidden');
    await invoke('navigate', { input: input.trim() });
  } catch (err) {
    console.error('[Nkore] navigate error:', err);
  }
}

export async function goBack() {
  try { await invoke('go_back'); } catch (e) { console.error(e); }
}

export async function goForward() {
  try { await invoke('go_forward'); } catch (e) { console.error(e); }
}

export async function reload() {
  try {
    document.getElementById('loading-bar')?.classList.remove('hidden');
    await invoke('reload');
  } catch (e) { console.error(e); }
}

export async function toggleBookmark() {
  if (!currentTab) return;
  try {
    if (isBookmarked) {
      await invoke('remove_bookmark', { url: currentTab.url });
      isBookmarked = false;
    } else {
      await invoke('add_bookmark', { url: currentTab.url, title: currentTab.title || currentTab.url });
      isBookmarked = true;
    }
    updateBookmarkButton();
  } catch (e) { console.error(e); }
}

// ── Tab change callback (called by tabs.js) ───────────────────────────────────
export function onTabChanged(tab) {
  currentTab = tab;
  updateAddressBar(tab.url);
  updateNavButtons(tab);
  checkBookmarkStatus(tab.url);
  updateSecurityIcon(tab.url);
  // Update window title
  document.title = `${tab.title || tab.url} – Nkore`;
}

// ── Address bar ───────────────────────────────────────────────────────────────
function updateAddressBar(url) {
  const input = addressInput();
  if (!input) return;
  if (document.activeElement === input) return; // don't overwrite while typing
  input.value = url || '';
}

function updateNavButtons(tab) {
  const back    = btnBack();
  const forward = btnForward();
  if (back)    back.disabled    = !tab.can_go_back;
  if (forward) forward.disabled = !tab.can_go_forward;
}

function updateBookmarkButton() {
  const btn = btnBookmark();
  if (!btn) return;
  btn.classList.toggle('active', isBookmarked);
  btn.title = isBookmarked ? 'Remove bookmark' : 'Bookmark this page';
}

async function checkBookmarkStatus(url) {
  if (!url) return;
  try {
    isBookmarked = await invoke('is_bookmarked', { url });
    updateBookmarkButton();
  } catch (_) {}
}

function updateSecurityIcon(url) {
  const icon = securityIcon();
  if (!icon) return;
  if (!url) { icon.className = 'security-icon insecure'; return; }
  if (url.startsWith('https://')) {
    icon.className = 'security-icon';
    icon.title = 'Connection is secure (HTTPS)';
  } else if (url.startsWith('http://')) {
    icon.className = 'security-icon warning';
    icon.title = 'Connection is not secure (HTTP)';
  } else {
    icon.className = 'security-icon insecure';
    icon.title = '';
  }
}

// ── Autocomplete ──────────────────────────────────────────────────────────────
async function updateAutocomplete(query) {
  const dropdown = autocompleteEl();
  if (!dropdown) return;

  if (!query.trim()) {
    dropdown.classList.add('hidden');
    return;
  }

  try {
    // Get history suggestions
    const historyResults = await invoke('search_history', { query });
    const resolvedUrl = await invoke('resolve_input_url', { input: query });

    autocompleteResults = [];

    // First item: direct navigation / search
    if (resolvedUrl !== query) {
      autocompleteResults.push({
        type: 'navigate',
        display: resolvedUrl,
        subtitle: resolvedUrl.startsWith('https://duckduckgo.com') ? 'Search DuckDuckGo' : 'Navigate to URL',
        url: resolvedUrl,
      });
    }

    // History items
    historyResults.slice(0, 6).forEach(h => {
      autocompleteResults.push({
        type: 'history',
        display: h.title || h.url,
        subtitle: h.url,
        url: h.url,
      });
    });

    renderAutocomplete();
  } catch (e) {
    console.error('[Nkore] autocomplete error:', e);
  }
}

function renderAutocomplete() {
  const dropdown = autocompleteEl();
  if (!dropdown) return;

  if (autocompleteResults.length === 0) {
    dropdown.classList.add('hidden');
    return;
  }

  dropdown.innerHTML = '';
  autocompleteResults.forEach((item, idx) => {
    const el = document.createElement('div');
    el.className = 'autocomplete-item' + (idx === selectedAutocompleteIndex ? ' selected' : '');

    const iconEl = document.createElement('div');
    iconEl.className = 'autocomplete-icon';
    iconEl.innerHTML = item.type === 'history'
      ? '<svg viewBox="0 0 24 24"><path d="M13 3c-4.97 0-9 4.03-9 9H1l3.89 3.89.07.14L9 12H6c0-3.87 3.13-7 7-7s7 3.13 7 7-3.13 7-7 7c-1.93 0-3.68-.79-4.94-2.06l-1.42 1.42C8.27 19.99 10.51 21 13 21c4.97 0 9-4.03 9-9s-4.03-9-9-9zm-1 5v5l4.28 2.54.72-1.21-3.5-2.08V8H12z"/></svg>'
      : '<svg viewBox="0 0 24 24"><path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zm4.24 16L12 15.45 7.77 18l1.12-4.81-3.73-3.23 4.92-.42L12 5l1.92 4.53 4.92.42-3.73 3.23L16.23 18z"/></svg>';

    const textEl = document.createElement('div');
    textEl.className = 'autocomplete-text';
    textEl.innerHTML = `<div class="autocomplete-main">${escapeHtml(item.display)}</div>
                        <div class="autocomplete-sub">${escapeHtml(item.subtitle)}</div>`;

    el.appendChild(iconEl);
    el.appendChild(textEl);
    el.addEventListener('click', () => {
      navigate(item.url);
      hideAutocomplete();
    });

    dropdown.appendChild(el);
  });

  dropdown.classList.remove('hidden');
  selectedAutocompleteIndex = -1;
}

function hideAutocomplete() {
  autocompleteEl()?.classList.add('hidden');
  selectedAutocompleteIndex = -1;
}

function navigateAutocompleteSelection(direction) {
  const len = autocompleteResults.length;
  if (len === 0) return;
  selectedAutocompleteIndex = (selectedAutocompleteIndex + direction + len) % len;
  renderAutocomplete();

  const selected = autocompleteResults[selectedAutocompleteIndex];
  if (selected) {
    const input = addressInput();
    if (input) input.value = selected.url;
  }
}

function escapeHtml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ── Init ──────────────────────────────────────────────────────────────────────
export function initNavigation() {
  const input = addressInput();

  if (input) {
    // Select all on focus
    input.addEventListener('focus', () => input.select());

    // Navigate on Enter
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        const val = input.value.trim();
        if (selectedAutocompleteIndex >= 0 && autocompleteResults[selectedAutocompleteIndex]) {
          navigate(autocompleteResults[selectedAutocompleteIndex].url);
        } else {
          navigate(val);
        }
        hideAutocomplete();
        input.blur();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        navigateAutocompleteSelection(1);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        navigateAutocompleteSelection(-1);
      } else if (e.key === 'Escape') {
        hideAutocomplete();
        input.blur();
      }
    });

    // Autocomplete on input
    let debounceTimer;
    input.addEventListener('input', () => {
      clearTimeout(debounceTimer);
      debounceTimer = setTimeout(() => updateAutocomplete(input.value), 150);
    });

    input.addEventListener('blur', () => {
      // Delay to allow click events on autocomplete items to fire first
      setTimeout(hideAutocomplete, 150);
    });
  }

  // Navigation buttons
  btnBack()?.addEventListener('click', goBack);
  btnForward()?.addEventListener('click', goForward);
  btnReload()?.addEventListener('click', reload);
  document.getElementById('btn-home')?.addEventListener('click', () => navigate('https://duckduckgo.com'));
  btnBookmark()?.addEventListener('click', toggleBookmark);

  // Alt+Left / Alt+Right
  document.addEventListener('keydown', (e) => {
    if (e.altKey && e.key === 'ArrowLeft')  { e.preventDefault(); goBack(); }
    if (e.altKey && e.key === 'ArrowRight') { e.preventDefault(); goForward(); }
  });

  // Expose to other modules
  window.__nkoreNav = { onTabChanged, navigate, reload, toggleBookmark };
}
