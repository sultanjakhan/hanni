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

  // GET share_links/{token}/{coll} — paginated, walks every page so callers
  // get the full collection in one promise.
  async function list(coll, opts) {
    opts = opts || {};
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
    return out;
  }

  async function get(coll, docId) {
    const url = `${base}/${tokenPath}/${encodeURIComponent(coll)}/${encodeURIComponent(String(docId))}`
      + `?key=${encodeURIComponent(fs.api_key)}`;
    try {
      const j = await fetchJson(url);
      return parseDoc(j);
    } catch (e) {
      if (e.status === 404) return null;
      throw e;
    }
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.firestore = { list, get };
})();
