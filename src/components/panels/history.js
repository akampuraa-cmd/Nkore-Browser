/**
 * History panel component for Nkore Browser.
 */

const { invoke } = window.__TAURI__.core;

let historyEntries = [];

const listEl        = () => document.getElementById('history-list');
const searchInputEl = () => document.getElementById('history-search');

function escHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function faviconForUrl(url) {
  try { return `${new URL(url).origin}/favicon.ico`; }
  catch { return ''; }
}

function relativeDate(dateStr) {
  try {
    const d = new Date(dateStr.endsWith('Z') ? dateStr : dateStr + 'Z');
    const now = new Date();
    const diff = now - d; // ms

    if (diff < 60_000)  return 'just now';
    if (diff < 3600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86400_000) return `${Math.floor(diff / 3600_000)}h ago`;
    if (diff < 604800_000) return `${Math.floor(diff / 86400_000)}d ago`;
    return d.toLocaleDateString();
  } catch {
    return '';
  }
}

function dateLabel(dateStr) {
  try {
    const d = new Date(dateStr.endsWith('Z') ? dateStr : dateStr + 'Z');
    const now = new Date();
    const diffDays = Math.floor((now - d) / 86400_000);
    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return 'Yesterday';
    if (diffDays < 7)  return `${diffDays} days ago`;
    return d.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
  } catch {
    return 'Older';
  }
}

function renderHistory(entries) {
  const list = listEl();
  if (!list) return;

  if (!entries || entries.length === 0) {
    list.innerHTML = '<p class="panel-empty">No browsing history.</p>';
    return;
  }

  // Group by date
  const groups = new Map();
  entries.forEach(h => {
    const label = dateLabel(h.visited_at);
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label).push(h);
  });

  let html = '';
  for (const [label, items] of groups) {
    html += `<div class="history-date-group">
      <div class="history-date-label">${escHtml(label)}</div>`;
    items.forEach(h => {
      html += `
      <div class="history-item" data-url="${escHtml(h.url)}">
        <img class="history-favicon"
             src="${escHtml(faviconForUrl(h.url))}"
             onerror="this.style.display='none'" alt="" />
        <div class="history-info">
          <div class="history-title">${escHtml(h.title || h.url)}</div>
          <div class="history-url">${escHtml(h.url)}</div>
        </div>
        <span class="history-time">${relativeDate(h.visited_at)}</span>
      </div>`;
    });
    html += '</div>';
  }

  list.innerHTML = html;

  list.querySelectorAll('.history-item').forEach(item => {
    item.addEventListener('click', () => {
      window.__nkoreNav?.navigate(item.dataset.url);
      document.getElementById('panel-history')?.classList.add('hidden');
      document.getElementById('panel-backdrop')?.classList.add('hidden');
    });
  });
}

async function loadHistory() {
  try {
    historyEntries = await invoke('get_history', { limit: 200 }) || [];
    renderHistory(historyEntries);
  } catch (e) {
    console.error('[Nkore] get_history error:', e);
  }
}

export function initHistory() {
  // Reload when panel opens
  document.getElementById('btn-history')?.addEventListener('click', loadHistory);
  loadHistory();

  // Search
  let debounce;
  searchInputEl()?.addEventListener('input', (e) => {
    clearTimeout(debounce);
    const q = e.target.value.trim();
    debounce = setTimeout(async () => {
      if (!q) {
        renderHistory(historyEntries);
        return;
      }
      try {
        const results = await invoke('search_history', { query: q });
        renderHistory(results);
      } catch (err) {
        console.error(err);
      }
    }, 200);
  });

  // Clear all
  document.getElementById('btn-clear-history')?.addEventListener('click', async () => {
    if (!confirm('Clear all browsing history?')) return;
    await invoke('clear_history').catch(console.error);
    historyEntries = [];
    renderHistory([]);
  });
}
