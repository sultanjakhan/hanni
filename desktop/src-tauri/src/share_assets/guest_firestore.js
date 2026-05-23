// guest_firestore.js — Stage C-1 Firestore REST client.
//
// Loaded before guest.js. Registers window.HanniGuest.firestore = { list, get }
// when the landing page provided cloud config (window.__SHARE__.firestore),
// otherwise leaves the property unset so view-modules fall back to /api via
// the local axum tunnel.
//
// Auth model: rules grant `allow read` on share_links/{token}/** by URL token
// alone (the token is the secret). REST requires the project's Web API key as
// a query param.

(function () {
  const ctx = window.__SHARE__ || {};
  const fs = ctx.firestore;
  if (!fs || !fs.project_id || !fs.api_key) return;

  const base = `https://firestore.googleapis.com/v1/projects/${encodeURIComponent(fs.project_id)}/databases/(default)/documents`;
  const tokenPath = `share_links/${encodeURIComponent(ctx.token)}`;

  // Firestore REST returns every value tagged: { stringValue: "..." }, etc.
  // Unwrap into plain JS so view-modules don't have to know about the wire
  // format. Numbers come back as strings for integerValue — coerce them.
  function parseValue(v) {
    if (!v || typeof v !== 'object') return null;
    if ('stringValue'   in v) return v.stringValue;
    if ('integerValue'  in v) return Number(v.integerValue);
    if ('doubleValue'   in v) return v.doubleValue;
    if ('booleanValue'  in v) return v.booleanValue;
    if ('nullValue'     in v) return null;
    if ('timestampValue'in v) return v.timestampValue;
    if ('arrayValue'    in v) return (v.arrayValue.values || []).map(parseValue);
    if ('mapValue'      in v) return parseFields(v.mapValue.fields || {});
    return null;
  }
  function parseFields(fields) {
    const out = {};
    for (const k in fields) out[k] = parseValue(fields[k]);
    return out;
  }
  function parseDoc(doc) {
    if (!doc || !doc.fields) return null;
    return parseFields(doc.fields);
  }

  async function fetchJson(url) {
    const r = await fetch(url, { credentials: 'omit' });
    if (!r.ok) {
      const txt = await r.text().catch(() => '');
      const err = new Error(`Firestore ${r.status}: ${txt.slice(0, 200)}`);
      err.status = r.status;
      throw err;
    }
    return r.json();
  }

  // localStorage cache to keep us under the 50k/day Firestore free quota.
  // list TTL 60s (might change often), get TTL 5min. invalidate() drops keys
  // when the guest writes, so the next read fetches fresh.
  const CACHE_PFX = `hg:fs:${ctx.token}:`;
  const TTL_LIST = 60 * 1000, TTL_GET = 5 * 60 * 1000;
  function cacheGet(key, maxAge) {
    try {
      const raw = localStorage.getItem(CACHE_PFX + key);
      if (!raw) return null;
      const o = JSON.parse(raw);
      if (Date.now() - o.t > maxAge) return null;
      return o.v;
    } catch { return null; }
  }
  function cacheSet(key, v) {
    try { localStorage.setItem(CACHE_PFX + key, JSON.stringify({ t: Date.now(), v })); } catch {}
  }
  function invalidate(coll) {
    const sub = CACHE_PFX + (coll || '');
    try { for (const k of Object.keys(localStorage)) if (k.startsWith(sub)) localStorage.removeItem(k); } catch {}
  }

  // GET share_links/{token}/{coll} — paginated, walks every page so callers
  // get the full collection in one promise.
  async function list(coll, opts) {
    opts = opts || {};
    const key = coll;
    if (!opts.fresh) { const c = cacheGet(key, TTL_LIST); if (c) return c; }
    const pageSize = Math.min(opts.pageSize || 300, 300);
    let pageToken = '';
    const out = [];
    for (let i = 0; i < 20; i++) {  // hard cap: 20 pages × 300 = 6000 docs
      const url = `${base}/${tokenPath}/${encodeURIComponent(coll)}`
        + `?key=${encodeURIComponent(fs.api_key)}`
        + `&pageSize=${pageSize}`
        + (pageToken ? `&pageToken=${encodeURIComponent(pageToken)}` : '');
      const j = await fetchJson(url);
      for (const d of (j.documents || [])) {
        const p = parseDoc(d);
        if (p) out.push(p);
      }
      if (!j.nextPageToken) break;
      pageToken = j.nextPageToken;
    }
    cacheSet(key, out);
    return out;
  }

  async function get(coll, docId) {
    const key = `${coll}:${docId}`;
    const c = cacheGet(key, TTL_GET);
    if (c) return c;
    const url = `${base}/${tokenPath}/${encodeURIComponent(coll)}/${encodeURIComponent(String(docId))}`
      + `?key=${encodeURIComponent(fs.api_key)}`;
    try {
      const j = await fetchJson(url);
      const v = parseDoc(j);
      if (v) cacheSet(key, v);
      return v;
    } catch (e) {
      if (e.status === 404) return null;
      throw e;
    }
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.firestore = { list, get, invalidate };
})();
