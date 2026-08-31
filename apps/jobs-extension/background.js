// background.js — service worker: relays API calls from content scripts to the
// local Hanni server. Extension context has host_permissions for 127.0.0.1,
// so no CORS changes are needed on the Rust side.
// Also opens the in-page panel from the context menu, the Alt+H command and
// the toolbar popup.

// A Jobs-only token is loaded from a gitignored file when present. The master
// automation token must never enter the extension.
importScripts('api-policy.js');
try { importScripts('token.local.js'); } catch { /* file absent — manual token */ }

const storageReady = (async () => {
  // Chrome Sync used to contain the master token. Remove it from every synced
  // profile and deliberately do not migrate it into another store.
  const legacy = await chrome.storage.sync.get(['port']);
  if (legacy.port === 8235 || legacy.port === 8236) {
    await chrome.storage.local.set({ port: legacy.port });
  }
  await chrome.storage.sync.remove(['port', 'token']);
  await chrome.storage.local.remove(['token']);
  await chrome.storage.session.setAccessLevel({ accessLevel: 'TRUSTED_CONTEXTS' });
  await chrome.storage.local.setAccessLevel({ accessLevel: 'TRUSTED_CONTEXTS' });
})().catch(() => {});

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

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
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
    await storageReady;
    if (sender.id !== chrome.runtime.id) {
      sendResponse({ ok: false, status: 403, error: 'Forbidden sender' });
      return;
    }
    const [{ port = 8235 }, { jobToken = '' }] = await Promise.all([
      chrome.storage.local.get(['port']),
      chrome.storage.session.get(['jobToken']),
    ]);
    const token = ((self.HANNI_LOCAL_JOB_TOKEN || '') || jobToken || '').trim();
    if (!token) {
      sendResponse({ ok: false, status: 401, error: 'Jobs token is not configured' });
      return;
    }

    async function call(p) {
      const request = self.HanniApiPolicy.validateApiMessage(msg, p);
      if (!request) return { ok: false, status: 403, error: 'API request is not allowed' };
      const res = await fetch(request.url, {
        method: request.method,
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: request.body ? JSON.stringify(request.body) : undefined,
      });
      const data = await res.json().catch(() => null);
      return { ok: res.ok, status: res.status, data };
    }

    // Configured port first; on 404 (build without the route, e.g. old prod)
    // or no server, fall back to the other one — dev and prod share the DB.
    const configuredPort = self.HanniApiPolicy.ALLOWED_PORTS.has(Number(port)) ? Number(port) : 8235;
    const fallbackPort = configuredPort === 8236 ? 8235 : 8236;
    try {
      const out = await call(configuredPort);
      if (out.status === 403) { sendResponse(out); return; }
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
