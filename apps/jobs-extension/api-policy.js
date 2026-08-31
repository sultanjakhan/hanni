// api-policy.js — fail-closed policy for the only local API calls this
// extension is allowed to make. Kept independent so it can be unit-tested.
(function expose(root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  root.HanniApiPolicy = api;
})(typeof self === 'object' ? self : globalThis, function createPolicy() {
  const ALLOWED_PORTS = new Set([8235, 8236]);

  function validateApiMessage(msg, port) {
    const numericPort = Number(port);
    if (!Number.isInteger(numericPort) || !ALLOWED_PORTS.has(numericPort)) return null;
    if (!msg || msg.type !== 'hanni-api' || typeof msg.path !== 'string') return null;
    if (!msg.path.startsWith('/') || msg.path.startsWith('//')) return null;

    const origin = `http://127.0.0.1:${numericPort}`;
    let url;
    try { url = new URL(msg.path, `${origin}/`); } catch { return null; }
    if (url.origin !== origin || url.pathname !== '/api/vacancy' || url.hash) return null;

    const method = String(msg.method || 'GET').toUpperCase();
    if (method === 'GET') {
      const entries = [...url.searchParams.entries()];
      if (entries.length !== 1 || entries[0][0] !== 'url' || msg.body != null) return null;
      if (!entries[0][1] || entries[0][1].length > 4096) return null;
      return { method, url: url.href, body: null };
    }
    if (method === 'POST') {
      if (url.search || !msg.body || typeof msg.body !== 'object' || Array.isArray(msg.body)) return null;
      return { method, url: url.href, body: msg.body };
    }
    return null;
  }

  return { ALLOWED_PORTS, validateApiMessage };
});
