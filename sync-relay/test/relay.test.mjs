import assert from 'node:assert/strict';
import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { mkdtemp, mkdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

// Tooling is isolated beside this review artifact, never installed in canonical.
async function loadRuntime() {
  if (process.env.HANNI_MINIFLARE_MODULE) return import(process.env.HANNI_MINIFLARE_MODULE);
  try { return await import('miniflare'); }
  catch (error) {
    if (error.code !== 'ERR_MODULE_NOT_FOUND') throw error;
    return import(new URL('../../cloudflare-tooling/node_modules/miniflare/dist/src/index.js', import.meta.url).href);
  }
}
const { Miniflare, convertV4MiniflareOptions } = await loadRuntime();
const scriptPath = fileURLToPath(new URL('../src/worker.mjs', import.meta.url));
const script = await readFile(scriptPath, 'utf8');
const devices = Object.fromEntries(['windows', 'mac', 'phone-a', 'phone-b'].map(id => [id, randomBytes(32).toString('base64url')]));
const hashes = Object.fromEntries(Object.entries(devices).map(([id, token]) => [id, hash(token)]));

function hash(value) { return createHash('sha256').update(value).digest('hex'); }
function batch(cipherBytes = 32, client_seq = 1) {
  return { client_seq, batch_id: randomUUID(), envelope: {
    v: 1, alg: 'XChaCha20-Poly1305', key_id: 'synthetic-key-v1',
    nonce: randomBytes(24).toString('base64url'),
    // Random synthetic opaque bytes test transport, not the client AEAD implementation.
    ciphertext: randomBytes(cipherBytes).toString('base64url'),
  } };
}
function runtime(bindings = {}, persistence) {
  return new Miniflare(convertV4MiniflareOptions({
    modules: true, script, compatibilityDate: '2026-09-01',
    durableObjects: { RELAY: { className: 'Relay', useSQLite: true } },
    bindings: { HANNI_DEVICE_TOKEN_HASHES: JSON.stringify(hashes), ...bindings },
    ...(persistence ? { resourcePersistencePath: persistence } : {}),
  }));
}
async function request(mf, path, { device = 'windows', method = 'GET', body, headers = {} } = {}) {
  return mf.dispatchFetch(`https://relay.test${path}`, {
    method, headers: { Authorization: `Bearer ${devices[device]}`, ...(body !== undefined ? { 'Content-Type': 'application/json' } : {}), ...headers },
    ...(body !== undefined ? { body: typeof body === 'string' ? body : JSON.stringify(body) } : {}),
  });
}
async function append(mf, value, device = 'windows') {
  return request(mf, '/v1/batches', { method: 'POST', body: value, device });
}
async function error(response, status, code) {
  assert.equal(response.status, status);
  assert.deepEqual(await response.json(), { error: code });
}
function inbox(socket) {
  const frames = [];
  const waiters = [];
  socket.addEventListener('message', event => {
    const next = waiters.shift();
    if (next) next(event.data); else frames.push(event.data);
  });
  return () => frames.length ? Promise.resolve(frames.shift()) : new Promise((resolve, reject) => {
    const receive = value => { clearTimeout(timer); resolve(value); };
    const timer = setTimeout(() => {
      const index = waiters.indexOf(receive);
      if (index !== -1) waiters.splice(index, 1);
      reject(new Error('WebSocket notification timeout'));
    }, 5000);
    waiters.push(receive);
  });
}

test('authentication fails closed; URL tokens and plaintext fields are rejected', async () => {
  const mf = runtime();
  try {
    await error(await mf.dispatchFetch('https://relay.test/v1/batches'), 401, 'unauthorized');
    await error(await request(mf, '/v1/batches', { headers: { Authorization: `Bearer ${randomBytes(32).toString('base64url')}` } }), 401, 'unauthorized');
    await error(await request(mf, '/v1/batches?token=synthetic'), 400, 'invalid_query');
    await error(await append(mf, { ...batch(), rows: [{ synthetic: true }] }), 400, 'invalid_batch');
    // Miniflare's dispatchFetch upgrade bridge can emit ECONNRESET for a rejected
    // upgrade. Test rejection over HTTP; successful authenticated upgrade is below.
    await error(await request(mf, '/v1/stream?token=synthetic'), 400, 'invalid_query');
    assert.equal((await (await request(mf, '/v1/batches')).json()).latest_seq, 0);
  } finally { await mf.dispose(); }
  const unconfigured = runtime({ HANNI_DEVICE_TOKEN_HASHES: '{}' });
  try { await error(await request(unconfigured, '/v1/batches'), 503, 'relay_not_configured'); }
  finally { await unconfigured.dispose(); }
});

test('commit ACK, identical retry, changed-payload conflict and canonical field order', async () => {
  const mf = runtime();
  try {
    const value = batch();
    const first = await append(mf, value, 'phone-a');
    assert.equal(first.status, 201);
    const ack = await first.json();
    assert.deepEqual(ack, { seq: 1, duplicate: false, client_seq: 1, sender_device_id: 'phone-a', batch_id: value.batch_id, envelope_sha256: hash(JSON.stringify(value.envelope)) });
    const shuffled = { ...value, envelope: Object.fromEntries(Object.entries(value.envelope).reverse()) };
    const retry = await append(mf, shuffled, 'phone-a');
    assert.equal(retry.status, 200);
    assert.deepEqual(await retry.json(), { ...ack, duplicate: true });
    await error(await append(mf, { ...value, envelope: { ...value.envelope, nonce: randomBytes(24).toString('base64url') } }, 'phone-a'), 409, 'batch_payload_mismatch');
    const page = await (await request(mf, '/v1/batches')).json();
    assert.equal(page.latest_seq, 1);
    assert.equal(page.batches.length, 1);
    assert.deepEqual(page.batches[0].envelope, value.envelope);
    assert.equal(page.batches[0].envelope_sha256, ack.envelope_sha256);
    assert.equal(page.next_cursor, 1);
  } finally { await mf.dispose(); }
});

test('concurrent duplicate and distinct batches get one durable sequence each', async () => {
  const mf = runtime();
  try {
    const same = batch();
    const repeated = await Promise.all(Array.from({ length: 8 }, () => append(mf, same)));
    assert.equal(repeated.filter(response => response.status === 201).length, 1);
    assert.equal(repeated.filter(response => response.status === 200).length, 7);
    for (const response of repeated) assert.equal((await response.json()).seq, 1);
    const sequences = [];
    // Each device uploads in order, while the four devices race independently.
    for (let round = 0; round < 3; round++) {
      const parallel = await Promise.all(Object.keys(devices).map(device =>
        append(mf, batch(32, round + (device === 'windows' ? 2 : 1)), device)));
      for (const response of parallel) { assert.equal(response.status, 201); sequences.push((await response.json()).seq); }
    }
    assert.deepEqual(sequences.sort((a, b) => a - b), Array.from({ length: 12 }, (_, i) => i + 2));
    assert.equal((await (await request(mf, '/v1/batches?limit=32')).json()).batches.length, 13);
  } finally { await mf.dispose(); }
});

test('cursor advances only over returned records, including sender records', async () => {
  const mf = runtime();
  try {
    for (let i = 0; i < 5; i++) assert.equal((await append(mf, batch(32, Math.floor(i / 2) + 1), i % 2 ? 'mac' : 'windows')).status, 201);
    const first = await (await request(mf, '/v1/batches?after=0&limit=2')).json();
    assert.deepEqual(first.batches.map(value => value.seq), [1, 2]);
    assert.equal(first.batches[0].sender_device_id, 'windows');
    assert.equal(first.next_cursor, 2);
    assert.equal(first.has_more, true);
    const second = await (await request(mf, '/v1/batches?after=2&limit=32')).json();
    assert.deepEqual(second.batches.map(value => value.seq), [3, 4, 5]);
    assert.equal(second.next_cursor, 5);
    assert.equal(second.has_more, false);
    assert.deepEqual(await (await request(mf, '/v1/batches?after=5')).json(), { batches: [], next_cursor: 5, latest_seq: 5, has_more: false });
    await error(await request(mf, '/v1/batches?after=6'), 409, 'cursor_ahead');
    await error(await request(mf, '/v1/batches?after=-1'), 400, 'invalid_cursor');
    await error(await request(mf, '/v1/batches?after=0&after=1'), 400, 'invalid_cursor');
  } finally { await mf.dispose(); }
});

test('body, encoding, nonce, tag and algorithm validation do not write records', async () => {
  const mf = runtime();
  try {
    const value = batch();
    for (const envelope of [
      { ...value.envelope, nonce: randomBytes(12).toString('base64url') },
      { ...value.envelope, nonce: `${value.envelope.nonce}=` },
      { ...value.envelope, ciphertext: randomBytes(15).toString('base64url') },
      { ...value.envelope, alg: 'A256GCM' },
      { ...value.envelope, key_id: '../synthetic' },
      { ...value.envelope, plaintext: 'synthetic' },
    ]) await error(await append(mf, { ...value, envelope }), 400, 'invalid_envelope');
    await error(await request(mf, '/v1/batches', { method: 'POST', body: '{' }), 400, 'invalid_json');
    await error(await request(mf, '/v1/batches', { method: 'POST', body: 'x'.repeat(97 * 1024) }), 413, 'batch_too_large');
    await error(await request(mf, '/v1/batches', { method: 'POST', body: value, headers: { 'Content-Type': 'text/plain' } }), 415, 'json_required');
    assert.equal((await (await request(mf, '/v1/batches')).json()).latest_seq, 0);
  } finally { await mf.dispose(); }
});

test('SQLite survives runtime shutdown: old ACK can be replayed and all data pulled', async () => {
  const root = fileURLToPath(new URL('../test-state/', import.meta.url));
  await mkdir(root, { recursive: true });
  const state = await mkdtemp(`${root}durable-`);
  const value = batch();
  let mf = runtime({}, state);
  let original;
  try {
    const response = await append(mf, value, 'phone-b');
    assert.equal(response.status, 201);
    original = await response.json();
  } finally { await mf.dispose(); }
  mf = runtime({}, state);
  try {
    const retry = await append(mf, value, 'phone-b');
    assert.equal(retry.status, 200);
    assert.deepEqual(await retry.json(), { ...original, duplicate: true });
    const page = await (await request(mf, '/v1/batches')).json();
    assert.equal(page.latest_seq, 1);
    assert.deepEqual(page.batches[0].envelope, value.envelope);
  } finally { await mf.dispose(); }
});

test('capacity exhaustion rejects new writes without losing old data or retry ACKs', async () => {
  const mf = runtime({ HANNI_MAX_STORAGE_BYTES: '1200' });
  try {
    const value = batch();
    assert.equal((await append(mf, value)).status, 201);
    await error(await append(mf, batch(32, 2)), 507, 'relay_capacity_reached');
    assert.equal((await append(mf, value)).status, 200);
    const page = await (await request(mf, '/v1/batches')).json();
    assert.equal(page.batches.length, 1);
    assert.deepEqual(page.batches[0].envelope, value.envelope);
    assert.equal(page.latest_seq, 1);
  } finally { await mf.dispose(); }
});

test('large encrypted batches respect response byte bound without skipping a cursor', async () => {
  const mf = runtime();
  try {
    for (let i = 0; i < 8; i++) assert.equal((await append(mf, batch(64 * 1024, i + 1))).status, 201);
    const response = await request(mf, '/v1/batches?limit=32');
    const raw = await response.text();
    assert.ok(Buffer.byteLength(raw) <= 512 * 1024);
    const page = JSON.parse(raw);
    assert.ok(page.batches.length > 0 && page.batches.length < 8);
    assert.equal(page.next_cursor, page.batches.at(-1).seq);
    assert.equal(page.has_more, true);
    const remainder = await (await request(mf, `/v1/batches?after=${page.next_cursor}&limit=32`)).json();
    assert.equal(remainder.batches[0].seq, page.next_cursor + 1);
    assert.equal(remainder.next_cursor, 8);
  } finally { await mf.dispose(); }
});

test('authenticated hibernation WebSocket announces only a committed cursor; ping works', async () => {
  const mf = runtime();
  let socket;
  try {
    const upgrade = await request(mf, '/v1/stream', { device: 'mac', headers: { Upgrade: 'websocket' } });
    assert.equal(upgrade.status, 101);
    socket = upgrade.webSocket;
    assert.ok(socket);
    const next = inbox(socket);
    socket.accept();
    assert.deepEqual(JSON.parse(await next()), { type: 'ready', latest_seq: 0 });
    const value = batch();
    const ack = await append(mf, value, 'phone-a');
    assert.equal(ack.status, 201);
    assert.deepEqual(JSON.parse(await next()), { type: 'changed', latest_seq: 1 });
    const page = await (await request(mf, '/v1/batches', { device: 'mac' })).json();
    assert.deepEqual(page.batches[0].envelope, value.envelope);
    socket.send('ping');
    assert.equal(await next(), 'pong');
  } finally {
    socket?.close(1000, 'test_complete');
    await mf.dispose();
  }
});

test('WebSocket reconnect does not wait for another append to release a closing slot', async () => {
  const mf = runtime();
  const sockets = [];
  const open = async () => {
    const response = await request(mf, '/v1/stream', { device: 'windows', headers: { Upgrade: 'websocket' } });
    assert.equal(response.status, 101);
    const socket = response.webSocket;
    assert.ok(socket);
    const next = inbox(socket);
    sockets.push(socket);
    socket.accept();
    assert.deepEqual(JSON.parse(await next()), { type: 'ready', latest_seq: 0 });
    return socket;
  };
  try {
    const first = await open();
    await open();
    first.close(1000, 'reconnect');
    await open();
    assert.equal((await (await request(mf, '/v1/batches')).json()).latest_seq, 0);
  } finally {
    for (const socket of sockets) { try { socket.close(1000, 'test_complete'); } catch {} }
    await mf.dispose();
  }
});
