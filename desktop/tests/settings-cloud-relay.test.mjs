import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';

const source = await readFile(new URL('../src/js/settings-cloud-relay.js', import.meta.url), 'utf8');
const ui = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));
const config = JSON.stringify({ v: 1, endpoint: 'https://fixture.invalid', device_id: 'synthetic-device', key_id: 'synthetic-key',
  token: 'A'.repeat(43), key: 'B'.repeat(43), enabled: true, sleep_source_store_id: '00000000-0000-0000-0000-000000000001' });
const settle = async () => { await new Promise(setImmediate); await new Promise(setImmediate); };

async function mount(invoke, mobile = false) {
  const dom = new JSDOM('<!doctype html><main></main>', { url: 'https://fixture.invalid' });
  const main = dom.window.document.querySelector('main');
  main.innerHTML = ui.renderCloudRelaySection({ mobile });
  await ui.wireCloudRelayControls(main, invoke);
  return { dom, main, query: selector => main.querySelector(selector), submit() {
    main.querySelector('form').dispatchEvent(new dom.window.Event('submit', { bubbles: true, cancelable: true }));
  } };
}

test('write-only setup clears before IPC and never reflects an error containing credentials', async () => {
  let received;
  const fixture = await mount(async (command, args) => {
    if (command === 'cloud_relay_status') return { configured: false };
    assert.equal(command, 'cloud_relay_set_config');
    assert.equal(fixture.query('[data-relay-code]').value, '');
    received = args.config;
    throw new Error('<script>' + config + '</script>');
  });
  fixture.query('[data-relay-code]').value = config;
  fixture.submit();
  await settle();
  assert.equal(received, config);
  assert.equal(fixture.query('[data-relay-code]').value, '');
  assert.ok(!fixture.main.textContent.includes('synthetic-device'));
  assert.ok(!fixture.main.innerHTML.includes('fixture.invalid'));
  assert.equal(fixture.dom.window.localStorage.length, 0);
  assert.equal(fixture.dom.window.sessionStorage.length, 0);
  fixture.dom.window.close();
});

test('first pairing without source authority is rejected before native save', async () => {
  let saves = 0;
  const fixture = await mount(async command => {
    if (command === 'cloud_relay_set_config') saves++;
    return { configured: false };
  });
  const incomplete = JSON.parse(config);
  delete incomplete.sleep_source_store_id;
  fixture.query('[data-relay-code]').value = JSON.stringify(incomplete);
  fixture.submit();
  await settle();
  assert.equal(saves, 0);
  assert.equal(fixture.query('[data-relay-code]').value, '');
  assert.match(fixture.query('[data-relay-feedback]').textContent, /основным телефоном/);
  fixture.dom.window.close();
});

test('one pending setup cannot be submitted twice; accepted setup is not delivery success', async () => {
  let resolveSave;
  let saves = 0;
  let configured = false;
  const fixture = await mount(async command => {
    if (command === 'cloud_relay_status') return { configured, enabled: true };
    saves++;
    await new Promise(resolve => { resolveSave = resolve; });
    configured = true;
    return { configured: true };
  });
  fixture.query('[data-relay-code]').value = config;
  fixture.submit();
  fixture.submit();
  assert.equal(saves, 1);
  assert.equal(fixture.query('[data-relay-save]').disabled, true);
  resolveSave();
  await settle();
  assert.match(fixture.query('[data-relay-status]').textContent, /первый успешный обмен/);
  assert.equal(fixture.query('[data-relay-setup]').open, false);
  assert.equal(fixture.query('[data-relay-save]').disabled, false);
  fixture.dom.window.close();
});

test('source metadata is requested only by the explicit phone button', async () => {
  const calls = [];
  const fixture = await mount(async command => {
    calls.push(command);
    if (command === 'cloud_relay_status') return { configured: false };
    return { supported: true, source_store_id: '00000000-0000-0000-0000-000000000001' };
  }, true);
  assert.deepEqual(calls, ['cloud_relay_status']);
  fixture.query('[data-relay-source]').click();
  await settle();
  assert.deepEqual(calls, ['cloud_relay_status', 'cloud_relay_pairing_source']);
  assert.equal(fixture.query('[data-relay-source-result]').hidden, false);
  assert.equal(fixture.query('[data-relay-source-code]').value.length, 36);
  fixture.dom.window.close();
});

test('aggregate status hides IDs, unknown errors and health freshness payloads', async () => {
  const fixture = await mount(async () => ({ configured: true, enabled: true, last_ok: 'synthetic time',
    error_code: 'unsafe diagnostic value', freshness: [{ type: 'sensitive sample' }],
    device_receipts: [{ device_id: 'private id' }] }));
  assert.match(fixture.query('[data-relay-status]').textContent, /повторит попытку/);
  assert.ok(!fixture.main.textContent.includes('unsafe diagnostic'));
  assert.ok(!fixture.main.textContent.includes('sensitive sample'));
  assert.ok(!fixture.main.textContent.includes('private id'));
  fixture.dom.window.close();
});

test('diagnostics distinguish source history gaps, cloud receive and pending projection', async () => {
  const fixture = await mount(async () => ({ configured: true, enabled: true, initializing: true,
    pending_keys: 2, incomplete_parts: 3, unresolved_deletions: 1, device_receipts: [{ device_id: 'never-render-id' }],
    source_import: [{ type: 'SleepSessionRecord', phase: 'changes', status: 'caught_up_with_deletion_gap',
      history_coverage: 'limited_unknown_grant_start', deletion_gap: true, more_pending: true,
      last_success_at: '2026-09-05T00:00:00Z' }],
    freshness: [{ type: 'health_records', record_updated_at: '2026-09-04 01:00:00', received_at: '2026-09-05T00:00:00Z' }],
    projection: { status: 'projection_partial', records: 0, pending_records: 7, errors: 2,
      retry_needed: true, next_retry_epoch: 1788566400, last_projected_epoch: 1788480000 } }));
  const text = fixture.query('[data-relay-diagnostics]').textContent;
  assert.match(text, /SleepSessionRecord/);
  assert.match(text, /пробел в истории удалений/);
  assert.match(text, /ограничена разрешением/);
  assert.match(text, /Архив Health Connect/);
  assert.match(text, /Ожидают обработки: 7/);
  assert.match(text, /Ошибок: 2/);
  assert.match(text, /подтверждений устройств: 1/);
  assert.ok(!text.includes('never-render-id'));
  fixture.dom.window.close();
});

test('raw freshness beyond eight entries and every individual receipt remain visible without IDs', async () => {
  const fixture = await mount(async () => ({ configured: true, enabled: true, applied_seq: 20,
    device_receipts: [{ device_id: 'never-render-1', applied_seq: 20, received_at: '2026-09-05T00:00:00Z' },
      { device_id: 'never-render-2', applied_seq: 12, received_at: '2026-09-04T00:00:00Z' }],
    freshness: [...Array.from({ length: 8 }, () => ({ type: 'ignored' })),
      { type: 'raw:SleepSessionRecord', record_updated_at: '2026-09-04T00:00:00Z', received_at: '2026-09-05T00:00:00Z' },
      { type: 'raw:StepsRecord' }, { type: 'health_log:exercise' }] }));
  const text = fixture.query('[data-relay-diagnostics]').textContent;
  assert.match(text, /Health Connect — SleepSessionRecord/);
  assert.match(text, /Health Connect — StepsRecord/);
  assert.match(text, /Тренировки/);
  assert.match(text, /Устройство 1: подтверждено сохранение пакетов до 20/);
  assert.match(text, /Устройство 2: подтверждено сохранение пакетов до 12/);
  assert.doesNotMatch(text, /локального уровня/);
  assert.doesNotMatch(text, /ещё ожидается/);
  assert.ok(!text.includes('never-render'));
  fixture.dom.window.close();
});
