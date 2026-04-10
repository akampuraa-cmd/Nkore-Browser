/**
 * Media Sniffer panel for Nkore Browser.
 *
 * Scans the active tab for video, audio, and image media, then presents
 * them in a filterable list with per-item download buttons.
 *
 * How it works:
 *   1. The user clicks "Scan Page" (or opens the panel).
 *   2. We invoke `scan_media` on the backend, which calls eval() on the active
 *      content webview to run `window.__nkoreScanMedia()`.
 *   3. The content webview emits a "media_scan_result" Tauri event back with
 *      the collected media URLs.
 *   4. We render the results with download buttons.
 */

const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

let mediaItems  = [];
let activeFilter = 'all';

const listEl = () => document.getElementById('media-list');

function escHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function filteredItems() {
  if (activeFilter === 'all') return mediaItems;
  return mediaItems.filter(m => m.type === activeFilter || m.type === 'source');
}

// ── Determine media type icon / badge ─────────────────────────────────────────
function typeIcon(type) {
  switch (type) {
    case 'video': return '🎬';
    case 'audio': return '🎵';
    case 'img':   return '🖼️';
    case 'source': return '🎬';
    default:       return '📁';
  }
}

function typeBadge(type) {
  const label = type === 'source' ? 'video' : (type || 'file');
  const cls = ['video','source'].includes(type) ? 'video' : (type === 'audio' ? 'audio' : 'img');
  return `<span class="media-type-badge ${cls}">${label}</span>`;
}

// ── Render ─────────────────────────────────────────────────────────────────────
function renderMedia() {
  const list = listEl();
  if (!list) return;

  const items = filteredItems();

  if (items.length === 0) {
    list.innerHTML = '<p class="panel-empty">No media found.<br>Try clicking "Scan Page" after the page has loaded.</p>';
    return;
  }

  list.innerHTML = items.map((m, idx) => {
    const isImage = m.type === 'img';
    const thumb   = isImage
      ? `<img class="media-thumbnail" src="${escHtml(m.url)}" alt="" loading="lazy" onerror="this.style.display='none'" />`
      : `<div class="media-thumbnail-placeholder">${typeIcon(m.type)}</div>`;

    const filename = (m.url || '').split('/').pop()?.split('?')[0] || `media_${idx + 1}`;
    const sizeInfo = (m.width && m.height) ? `${m.width}×${m.height}` : '';

    return `
    <div class="media-item" data-idx="${idx}">
      ${thumb}
      <div class="media-info">
        ${typeBadge(m.type)}
        <div class="media-title" title="${escHtml(m.title || filename)}">${escHtml(m.title || filename)}</div>
        <div class="media-url" title="${escHtml(m.url)}">${escHtml(m.url)}</div>
        ${sizeInfo ? `<div class="media-size">${sizeInfo}</div>` : ''}
      </div>
      <button class="media-download-btn" data-url="${escHtml(m.url)}" data-filename="${escHtml(filename)}" title="Download">
        <svg viewBox="0 0 24 24"><path d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"/></svg>
        Download
      </button>
    </div>`;
  }).join('');

  // Bind download buttons
  list.querySelectorAll('.media-download-btn').forEach(btn => {
    btn.addEventListener('click', async () => {
      const url = btn.dataset.url;
      const filename = btn.dataset.filename;
      if (!url) return;
      try {
        btn.textContent = 'Starting…';
        btn.disabled = true;
        await invoke('start_download', { url, savePath: null });
        btn.innerHTML = `<svg viewBox="0 0 24 24"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></svg> Added`;

        // Auto-open downloads panel
        document.getElementById('panel-downloads')?.classList.remove('hidden');
        document.getElementById('panel-backdrop')?.classList.remove('hidden');
        document.getElementById('panel-media-sniffer')?.classList.add('hidden');
      } catch (e) {
        console.error('[Nkore] download error:', e);
        btn.textContent = 'Error';
        btn.disabled = false;
      }
    });
  });
}

// ── Scan ──────────────────────────────────────────────────────────────────────
async function scanMedia() {
  const list = listEl();
  if (list) list.innerHTML = '<p class="panel-empty">Scanning page for media…</p>';

  try {
    // Ask backend to eval the scanner in the content webview
    await invoke('scan_media');
    // Result will arrive via 'media_scan_result' event (see listener below)
    // Give it a moment; if no event arrives in 2s, show empty state
    setTimeout(() => {
      if (mediaItems.length === 0 && list) {
        list.innerHTML = '<p class="panel-empty">No media detected on this page.</p>';
      }
    }, 2500);
  } catch (e) {
    console.error('[Nkore] scan_media error:', e);
    if (list) list.innerHTML = `<p class="panel-empty" style="color:var(--danger)">Scan failed: ${escHtml(String(e))}</p>`;
  }
}

// ── Init ──────────────────────────────────────────────────────────────────────
export function initMediaSniffer() {
  // Scan when the panel opens
  document.getElementById('btn-media-sniffer')?.addEventListener('click', scanMedia);
  document.getElementById('btn-refresh-media')?.addEventListener('click', () => {
    mediaItems = [];
    scanMedia();
  });

  // Filter buttons
  document.querySelectorAll('.media-filter-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.media-filter-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      activeFilter = btn.dataset.filter || 'all';
      renderMedia();
    });
  });

  // Listen for scan results emitted by the content webview via the
  // adblocker/init script (which uses window.__nkoreScanMedia()).
  // The Rust backend relays this after eval() completes.
  listen('media_scan_result', (event) => {
    const payload = event.payload;
    let items = payload;
    if (typeof payload === 'string') {
      try { items = JSON.parse(payload); } catch { items = []; }
    }
    if (!Array.isArray(items)) items = [];

    // Deduplicate by URL
    const seen = new Set(mediaItems.map(m => m.url));
    items.forEach(item => {
      if (item.url && !seen.has(item.url)) {
        seen.add(item.url);
        mediaItems.push(item);
      }
    });

    renderMedia();
  });
}
