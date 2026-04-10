/**
 * Downloads panel component for Nkore Browser.
 *
 * Displays active and past downloads with progress bars, speed indicators,
 * and cancel / open-folder buttons.
 */

const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

/** @type {Map<string, DownloadInfo>} */
const downloadsMap = new Map();

// ── DOM helpers ───────────────────────────────────────────────────────────────
const listEl    = () => document.getElementById('downloads-list');
const badgeEl   = () => document.getElementById('btn-downloads')?.querySelector('.badge');

// ── File icon by extension ────────────────────────────────────────────────────
function fileIcon(filename) {
  const ext = (filename || '').split('.').pop()?.toLowerCase();
  const icons = {
    pdf: '📄', mp4: '🎬', mkv: '🎬', avi: '🎬', mov: '🎬',
    mp3: '🎵', wav: '🎵', flac: '🎵', ogg: '🎵',
    jpg: '🖼️', jpeg: '🖼️', png: '🖼️', gif: '🖼️', webp: '🖼️',
    zip: '📦', tar: '📦', gz: '📦', '7z': '📦', rar: '📦',
    exe: '⚙️', dmg: '⚙️', deb: '⚙️', apk: '⚙️',
    doc: '📝', docx: '📝', xls: '📊', xlsx: '📊',
  };
  return icons[ext] || '📁';
}

// ── Formatters ────────────────────────────────────────────────────────────────
function fmtBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  let v = bytes, i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
}

function fmtSpeed(bps) {
  if (!bps) return '';
  return `${fmtBytes(bps)}/s`;
}

function fmtEta(secs) {
  if (!secs) return '';
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h`;
}

// ── Render ────────────────────────────────────────────────────────────────────
function renderDownloads() {
  const list = listEl();
  if (!list) return;

  const items = Array.from(downloadsMap.values())
    .sort((a, b) => (b.created_at || '').localeCompare(a.created_at || ''));

  if (items.length === 0) {
    list.innerHTML = '<p class="panel-empty">No downloads yet.</p>';
    updateBadge(0);
    return;
  }

  const active = items.filter(d => d.status === 'downloading' || d.status === 'pending');
  updateBadge(active.length);

  list.innerHTML = items.map(d => {
    const pct = d.total_bytes > 0 ? Math.round((d.downloaded_bytes / d.total_bytes) * 100) : 0;
    const isActive = d.status === 'downloading' || d.status === 'pending';

    return `
    <div class="download-item" data-id="${d.id}">
      <div class="download-item-header">
        <span class="download-file-icon">${fileIcon(d.filename)}</span>
        <div class="download-info">
          <div class="download-filename" title="${escHtml(d.save_path)}">${escHtml(d.filename)}</div>
          <div class="download-url">${escHtml(d.url)}</div>
        </div>
        <span class="download-status-badge ${d.status}">${d.status}</span>
      </div>

      ${isActive ? `
      <div class="download-progress-bar">
        <div class="download-progress-fill" style="width:${pct}%"></div>
      </div>
      <div class="download-meta">
        <span>${fmtBytes(d.downloaded_bytes)}${d.total_bytes ? ` / ${fmtBytes(d.total_bytes)}` : ''} (${pct}%)</span>
        <span>${fmtSpeed(d.speed_bps)} ${d.eta_secs ? `· ${fmtEta(d.eta_secs)} remaining` : ''}</span>
      </div>` : ''}

      ${d.status === 'completed' ? `
      <div class="download-meta">
        <span>${fmtBytes(d.downloaded_bytes)}</span>
        <span>${d.save_path}</span>
      </div>` : ''}

      ${d.error ? `<div class="download-meta" style="color:var(--danger)">${escHtml(d.error)}</div>` : ''}

      <div class="download-actions">
        ${isActive
          ? `<button class="download-btn danger" data-action="cancel" data-id="${d.id}">Cancel</button>`
          : ''}
        ${d.status === 'completed'
          ? `<button class="download-btn" data-action="open-folder" data-path="${escHtml(d.save_path)}">Show in folder</button>`
          : ''}
      </div>
    </div>`;
  }).join('');

  // Bind action buttons
  list.querySelectorAll('[data-action]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const action = btn.dataset.action;
      if (action === 'cancel') {
        await invoke('cancel_download', { id: btn.dataset.id }).catch(console.error);
      } else if (action === 'open-folder') {
        // Use shell open on the parent folder
        const path = btn.dataset.path;
        if (path) {
          const parent = path.replace(/[/\\][^/\\]+$/, '');
          await window.__TAURI__.shell.open(parent).catch(console.error);
        }
      }
    });
  });
}

function updateBadge(count) {
  let badge = document.getElementById('btn-downloads')?.querySelector('.badge');
  const btn = document.getElementById('btn-downloads');
  if (!btn) return;

  if (count > 0) {
    if (!badge) {
      badge = document.createElement('span');
      badge.className = 'badge';
      btn.appendChild(badge);
    }
    badge.textContent = String(count);
  } else {
    badge?.remove();
  }
}

function escHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// ── Init ──────────────────────────────────────────────────────────────────────
export function initDownloads() {
  // Load persisted downloads
  invoke('get_downloads').then(list => {
    (list || []).forEach(d => downloadsMap.set(d.id, d));
    renderDownloads();
  }).catch(console.error);

  // Real-time progress events
  listen('download://progress', (event) => {
    const { id, downloaded, total, speedBps, etaSecs } = event.payload;
    const existing = downloadsMap.get(id) || {};
    downloadsMap.set(id, {
      ...existing,
      id,
      downloaded_bytes: downloaded,
      total_bytes: total,
      speed_bps: speedBps,
      eta_secs: etaSecs,
      status: 'downloading',
    });
    renderDownloads();
  });

  listen('download://status', (event) => {
    const { id, status, error } = event.payload;
    const existing = downloadsMap.get(id) || {};
    downloadsMap.set(id, { ...existing, id, status, error: error || null });
    renderDownloads();
  });

  listen('download://started', (event) => {
    const { id, totalBytes, mimeType } = event.payload;
    const existing = downloadsMap.get(id) || {};
    downloadsMap.set(id, {
      ...existing,
      id,
      total_bytes: totalBytes,
      mime_type: mimeType,
      status: 'downloading',
    });
    renderDownloads();
    // Auto-open panel
    document.getElementById('panel-downloads')?.classList.remove('hidden');
    document.getElementById('panel-backdrop')?.classList.remove('hidden');
  });

  // Clear finished button
  document.getElementById('btn-clear-downloads')?.addEventListener('click', async () => {
    await invoke('clear_finished_downloads').catch(console.error);
    for (const [id, d] of downloadsMap) {
      if (['completed', 'failed', 'cancelled'].includes(d.status)) {
        downloadsMap.delete(id);
      }
    }
    renderDownloads();
  });
}
