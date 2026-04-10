/**
 * Nkore Browser – Main entry point
 *
 * Bootstraps all UI components and wires them to the Tauri backend.
 */

import '/src/style.css';
import { initTabs }        from '/src/components/tabs.js';
import { initNavigation }  from '/src/components/navigation.js';
import { initDownloads }   from '/src/components/panels/downloads.js';
import { initBookmarks }   from '/src/components/panels/bookmarks.js';
import { initHistory }     from '/src/components/panels/history.js';
import { initMediaSniffer } from '/src/components/panels/media-sniffer.js';

// ── Wait for Tauri to be ready ───────────────────────────────────────────────
async function main() {
  // Tauri injects `window.__TAURI__` synchronously, but we wait for
  // DOMContentLoaded to ensure all elements are available.
  await domReady();

  // Initialise all UI modules
  initTabs();
  initNavigation();
  initDownloads();
  initBookmarks();
  initHistory();
  initMediaSniffer();

  // ── Panel toggle buttons ─────────────────────────────────────────────────
  const panels = {
    downloads:     document.getElementById('panel-downloads'),
    bookmarks:     document.getElementById('panel-bookmarks'),
    history:       document.getElementById('panel-history'),
    'media-sniffer': document.getElementById('panel-media-sniffer'),
  };
  const backdrop = document.getElementById('panel-backdrop');

  function togglePanel(name) {
    const panel = panels[name];
    if (!panel) return;

    const isOpen = !panel.classList.contains('hidden');
    // Close all panels first
    Object.values(panels).forEach(p => p.classList.add('hidden'));
    Object.querySelectorAll && Object.keys(panels).forEach(n => {
      document.getElementById(`btn-${n}`)?.classList.remove('active');
    });
    backdrop.classList.add('hidden');

    if (!isOpen) {
      panel.classList.remove('hidden');
      backdrop.classList.remove('hidden');
      document.getElementById(`btn-${name}`)?.classList.add('active');
    }
  }

  document.getElementById('btn-downloads')?.addEventListener('click', () => togglePanel('downloads'));
  document.getElementById('btn-bookmarks')?.addEventListener('click', () => togglePanel('bookmarks'));
  document.getElementById('btn-history')?.addEventListener('click', () => togglePanel('history'));
  document.getElementById('btn-media-sniffer')?.addEventListener('click', () => togglePanel('media-sniffer'));

  backdrop.addEventListener('click', () => {
    Object.values(panels).forEach(p => p.classList.add('hidden'));
    backdrop.classList.add('hidden');
    document.querySelectorAll('.tool-btn').forEach(b => b.classList.remove('active'));
  });

  // Close-panel buttons (✕)
  document.querySelectorAll('.panel-close-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const name = btn.dataset.panel;
      if (panels[name]) {
        panels[name].classList.add('hidden');
        backdrop.classList.add('hidden');
        document.getElementById(`btn-${name}`)?.classList.remove('active');
      }
    });
  });

  // ── Global keyboard shortcuts ────────────────────────────────────────────
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey || e.metaKey) {
      switch (e.key) {
        case 't': e.preventDefault(); window.__nkoreTabs?.newTab(); break;
        case 'w': e.preventDefault(); window.__nkoreTabs?.closeActiveTab(); break;
        case 'l': e.preventDefault(); document.getElementById('address-input')?.select(); break;
        case 'r': e.preventDefault(); window.__nkoreNav?.reload(); break;
        case 'd': e.preventDefault(); window.__nkoreNav?.toggleBookmark(); break;
      }
    }
    if (e.key === 'F5') { e.preventDefault(); window.__nkoreNav?.reload(); }
    if (e.key === 'Escape') {
      Object.values(panels).forEach(p => p.classList.add('hidden'));
      backdrop.classList.add('hidden');
      document.querySelectorAll('.tool-btn').forEach(b => b.classList.remove('active'));
      document.getElementById('autocomplete-dropdown')?.classList.add('hidden');
    }
  });
}

function domReady() {
  return new Promise(resolve => {
    if (document.readyState !== 'loading') resolve();
    else document.addEventListener('DOMContentLoaded', resolve);
  });
}

main().catch(console.error);
