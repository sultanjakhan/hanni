// background.js — service worker: relays API calls from content scripts to the
// local Hanni server. Extension context has host_permissions for 127.0.0.1,
// so no CORS changes are needed on the Rust side.
// Also opens the in-page panel from the context menu, the Alt+H command and
// the toolbar popup.

// Zero-config token: token.local.js (generated from api_token.txt, gitignored)
// wins over whatever was pasted by hand — the file is the source of truth.
try { importScripts('token.local.js'); } catch { /* file absent — manual token */ }

// Open the panel in the tab; if the page was loaded before the extension was
// installed/reloaded, the content script isn't there yet (sendMessage has no
// receiver) — inject it on demand and retry.
async function showPanel(tabId) {
  try {
    await chrome.tabs.sendMessage(tabId, { type: 'hanni-show-panel' });
  } catch {
    try {
      await chrome.scripting.insertCSS({ target: { tabId }, files: ['panel.css'] });
      await chrome.scripting.executeScript({ target: { tabId }, files: ['parser.js', 'content.js'] });
      await chrome.tabs.sendMessage(tabId, { type: 'hanni-show-panel' });
    } catch { /* chrome:// and similar pages — nothing to mark there */ }
  }
}

// Toolbar icon opens the persistent side panel (top-level: runs again on
// every service-worker restart, which is exactly what we want).
chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true }).catch(() => {});

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: 'hanni-mark',
    title: 'Отметить вакансию в Hanni',
    contexts: ['page', 'selection', 'link'],
  });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId === 'hanni-mark' && tab && tab.id != null) showPanel(tab.id);
});

chrome.commands.onCommand.addListener((command, tab) => {
  if (command === 'hanni-mark' && tab && tab.id != null) showPanel(tab.id);
});

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg && msg.type === 'hanni-mark-active-tab') {
    (async () => {
      const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
      if (tab && tab.id != null) await showPanel(tab.id);
      sendResponse({ ok: true });
    })();
    return true;
  }
  if (!msg || msg.type !== 'hanni-api') return;
  (async () => {
    const stored = await chrome.storage.sync.get(['port', 'token']);
    const port = stored.port || 8235;
    const token = ((self.HANNI_LOCAL_TOKEN || '') || stored.token || '').trim();

    async function call(p) {
      const res = await fetch(`http://127.0.0.1:${p}${msg.path}`, {
        method: msg.method || 'GET',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: msg.body ? JSON.stringify(msg.body) : undefined,
      });
      const data = await res.json().catch(() => null);
      return { ok: res.ok, status: res.status, data };
    }

    // Configured port first; on 404 (build without the route, e.g. old prod)
    // or no server, fall back to the other one — dev and prod share the DB.
    const fallbackPort = Number(port) === 8236 ? 8235 : 8236;
    try {
      const out = await call(port);
      if (out.status !== 404) { sendResponse(out); return; }
    } catch { /* server down on the configured port */ }
    try {
      sendResponse(await call(fallbackPort));
    } catch (e) {
      sendResponse({ ok: false, status: 0, error: String(e) });
    }
  })();
  return true; // keep the message channel open for the async response
});
