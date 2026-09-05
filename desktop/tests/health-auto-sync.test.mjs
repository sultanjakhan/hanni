import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

// Execute the actual scheduling module with isolated browser/IPC boundaries.
const source = readFileSync(new URL('../src/js/health-auto-sync.js', import.meta.url), 'utf8')
  .replace(/^import .*;$/gm, '').replace(/^export /gm, '');
function fixture(overrides = {}) {
  const calls = [], timers = new Map(), store = new Map(), listeners = new Map();
  let next = 1;
  const document = { visibilityState: 'visible', addEventListener: (key, callback) => listeners.set(key, callback) };
  const context = vm.createContext({
    IS_MOBILE: true, document, localDate: () => '2026-09-05',
    localStorage: { getItem: key => store.get(key), setItem: (key, value) => store.set(key, value) },
    setTimeout: (callback, delay) => { const id = next++; timers.set(id, { callback, delay }); return id; },
    clearTimeout: id => timers.delete(id), setInterval: () => next++,
    invoke: async (command) => {
      calls.push(command);
      if (overrides[command]) return overrides[command]();
      if (command === 'health_has_permissions') return true;
      if (command === 'health_background_status') return { available: true, granted: false };
      if (command === 'health_import_raw') return { more_pending: false, modified_records: 0 };
      if (command === 'import_health_connect_all') return { successful_types: ['steps'] };
      return true;
    },
  });
  vm.runInContext(source + '\n globalThis.api = {autoImportHealth,maybeRequestHealthBackground,startHealthPolling};', context);
  return { api: context.api, calls, timers, store, document, listeners };
}

test('foreground and background permission paths share one pending dialog and cooldown', async () => {
  let grantChecks = 0, release;
  const f = fixture({
    health_has_permissions: () => ++grantChecks > 1,
    health_request_permissions: () => new Promise(resolve => { release = resolve; }),
  });
  const first = f.api.autoImportHealth({ force: true });
  await Promise.resolve(); await Promise.resolve();
  const second = f.api.maybeRequestHealthBackground();
  await Promise.resolve(); await Promise.resolve();
  assert.equal(f.calls.filter(v => v === 'health_request_permissions').length, 1);
  release(false);
  await Promise.all([first, second]);
  await f.api.maybeRequestHealthBackground();
  assert.equal(f.calls.filter(v => v === 'health_request_permissions').length, 1);
  assert.equal(f.store.get('hc_permission_prompted_at'), f.store.get('hc_bg_asked'));
});

test('archive continues despite failed Calendar projection and drains healthy backlog promptly', async () => {
  const f = fixture({
    health_import_raw: () => ({ more_pending: true, modified_records: 2, retry_needed: false }),
    import_health_connect_all: () => { throw new Error('synthetic denied projection'); },
  });
  assert.equal(await f.api.autoImportHealth({ force: true }), true);
  assert.equal([...f.timers.values()][0].delay, 1000);
  [...f.timers.values()][0].callback();
  await Promise.resolve(); await Promise.resolve();
  assert.equal(f.calls.filter(v => v === 'health_import_raw').length, 2);
});

test('background visibility cancels foreground continuation and never starts another HC read', async () => {
  const f = fixture({ health_import_raw: () => ({ more_pending: true, retry_needed: false }) });
  f.api.startHealthPolling();
  await f.api.autoImportHealth({ force: true });
  assert.equal(f.timers.size, 1);
  f.document.visibilityState = 'hidden';
  f.listeners.get('visibilitychange')();
  assert.equal(f.timers.size, 0);
  const previous = f.calls.length;
  await f.api.autoImportHealth({ force: true });
  assert.equal(f.calls.length, previous);
});

test('actual import error gets delay instead of immediate retry loop', async () => {
  const f = fixture({ health_import_raw: () => ({ more_pending: true, retry_needed: true }) });
  await f.api.autoImportHealth({ force: true });
  assert.equal([...f.timers.values()][0].delay, 30_000);
});
