// Hanni Sync Worker — Cloudflare D1 relay for cr-sqlite changesets
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const auth = request.headers.get("Authorization");
    if (auth !== `Bearer ${env.DEVICE_TOKEN}`) {
      return json({ error: "Unauthorized" }, 401);
    }

    if (url.pathname === "/sync/push" && request.method === "POST") {
      return handlePush(request, env);
    }
    if (url.pathname === "/sync/pull" && request.method === "GET") {
      return handlePull(url, env);
    }
    return json({ error: "Not found" }, 404);
  },
};

async function handlePush(request, env) {
  const { device_id, changes, db_version } = await request.json();
  if (!changes || !changes.length) {
    return json({ ok: true, stored: 0 });
  }

  const stmt = env.DB.prepare(
    `INSERT INTO sync_changes
      (tbl, pk, cid, val, col_version, db_version, site_id, cl, seq)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
  );

  const batch = changes.map((c) =>
    stmt.bind(c.table, c.pk, c.cid,
      c.val !== null && c.val !== undefined ? String(c.val) : null,
      c.col_version, c.db_version, c.site_id, c.cl, c.seq)
  );

  await env.DB.batch(batch);
  return json({ ok: true, stored: changes.length });
}

async function handlePull(url, env) {
  const since = parseInt(url.searchParams.get("since") || "0", 10);
  const rows = await env.DB.prepare(
    `SELECT tbl, pk, cid, val, col_version, db_version, site_id, cl, seq
     FROM sync_changes WHERE id > ? ORDER BY id ASC LIMIT 5000`
  ).bind(since).all();

  const changes = (rows.results || []).map((r) => ({
    table: r.tbl,
    pk: r.pk,
    cid: r.cid,
    val: r.val,
    col_version: r.col_version,
    db_version: r.db_version,
    site_id: r.site_id,
    cl: r.cl,
    seq: r.seq,
  }));

  // server_version = max id seen
  const maxId = rows.results?.length
    ? Math.max(...rows.results.map((_, i) => since + i + 1))
    : since;

  return json({ changes, server_version: maxId });
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
