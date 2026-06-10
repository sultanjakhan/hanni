// background.js — service worker: relays API calls from content scripts to the
// local Hanni server. Extension context has host_permissions for 127.0.0.1,
// so no CORS changes are needed on the Rust side.

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (!msg || msg.type !== 'hanni-api') return;
  (async () => {
    const { port = 8235, token = '' } = await chrome.storage.sync.get(['port', 'token']);
    try {
      const res = await fetch(`http://127.0.0.1:${port}${msg.path}`, {
        method: msg.method || 'GET',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: msg.body ? JSON.stringify(msg.body) : undefined,
      });
      const data = await res.json().catch(() => null);
      sendResponse({ ok: res.ok, status: res.status, data });
    } catch (e) {
      // Server down (Hanni not running) or wrong port
      sendResponse({ ok: false, status: 0, error: String(e) });
    }
  })();
  return true; // keep the message channel open for the async response
});
