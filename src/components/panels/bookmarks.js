/**
 * Bookmarks panel component for Nkore Browser.
 */

const { invoke } = window.__TAURI__.core;

let bookmarks = [];

const listEl = () => document.getElementById('bookmarks-list');

function faviconForUrl(url) {
  try { return `${new URL(url).origin}/favicon.ico`; }
  catch { return ''; }
}

function escHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function renderBookmarks() {
  const list = listEl();
  if (!list) return;

  if (bookmarks.length === 0) {
    list.innerHTML = '<p class="panel-empty">No bookmarks yet.<br>Press <kbd>Ctrl+D</kbd> to add the current page.</p>';
    return;
  }

  list.innerHTML = bookmarks.map(b => `
    <div class="bookmark-item" data-url="${escHtml(b.url)}">
      <img class="bookmark-favicon"
           src="${escHtml(faviconForUrl(b.url))}"
           onerror="this.style.display='none'"
           alt="" />
      <div class="bookmark-info">
        <div class="bookmark-title">${escHtml(b.title || b.url)}</div>
        <div class="bookmark-url">${escHtml(b.url)}</div>
      </div>
      <button class="bookmark-remove-btn" data-url="${escHtml(b.url)}" title="Remove bookmark">
        <svg viewBox="0 0 24 24"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>
      </button>
    </div>
  `).join('');

  // Navigate on click
  list.querySelectorAll('.bookmark-item').forEach(item => {
    item.addEventListener('click', (e) => {
      if (e.target.closest('.bookmark-remove-btn')) return;
      const url = item.dataset.url;
      window.__nkoreNav?.navigate(url);
      document.getElementById('panel-bookmarks')?.classList.add('hidden');
      document.getElementById('panel-backdrop')?.classList.add('hidden');
    });
  });

  // Remove on button click
  list.querySelectorAll('.bookmark-remove-btn').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const url = btn.dataset.url;
      await invoke('remove_bookmark', { url }).catch(console.error);
      bookmarks = bookmarks.filter(b => b.url !== url);
      renderBookmarks();
    });
  });
}

async function loadBookmarks() {
  try {
    bookmarks = await invoke('get_bookmarks') || [];
    renderBookmarks();
  } catch (e) {
    console.error('[Nkore] get_bookmarks error:', e);
  }
}

export function initBookmarks() {
  // Load bookmarks when the panel becomes visible
  document.getElementById('btn-bookmarks')?.addEventListener('click', loadBookmarks);
  // Initial load
  loadBookmarks();
}
