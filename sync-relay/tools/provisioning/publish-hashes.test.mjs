import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { publishHashes, validateHashes } from './publish-hashes.mjs';

const entry = process.env.HANNI_WRANGLER_ENTRY || fileURLToPath(new URL('../../node_modules/wrangler/wrangler-dist/cli.js', import.meta.url));
const config = 'synthetic-config.jsonc';
const hashes = Object.fromEntries([1,2,3,4].map(n => ['synthetic-' + n, n.toString(16).repeat(64)]));

test('read-only existence check precedes hashes-only stdin version upload; no deployment command', async () => {
  const calls = [];
  const result = await publishHashes(entry, config, hashes, async (_, args, input = '') => {
    calls.push({ args, input });
    return calls.length === 1 ? { code: 0, output: '[{"id":"synthetic-version"}]' }
      : { code: 0, output: 'unsafe fixture diagnostic with account id' };
  });
  assert.equal(result, 'hashes_version_created_activation_required');
  assert.deepEqual(calls[0].args.slice(0, 3), ['versions', 'list', '--json']);
  assert.deepEqual(calls[1].args.slice(0, 4), ['versions', 'secret', 'put', 'HANNI_DEVICE_TOKEN_HASHES']);
  assert.equal(calls[0].input, '');
  assert.deepEqual(JSON.parse(calls[1].input), hashes);
  assert.ok(calls.every(c => !c.args.includes('deploy') && !c.args.includes('login')));
  assert.ok(calls.every(c => !c.args.some(a => a.includes('1111111111'))));
});

test('missing Worker or empty version list causes no write', async () => {
  for (const response of [{ code: 1, output: 'unsafe error' }, { code: 0, output: '[]' }]) {
    let calls = 0;
    await assert.rejects(publishHashes(entry, config, hashes, async () => { calls++; return response; }), /existing_worker_required/);
    assert.equal(calls, 1);
  }
});

test('delete-after-check does not fall back to draft Worker creation', async () => {
  let calls = 0;
  await assert.rejects(publishHashes(entry, config, hashes, async () => {
    calls++;
    return calls === 1 ? { code: 0, output: '[{"id":"synthetic"}]' } : { code: 1, output: 'worker deleted' };
  }), /hash_version_upload_failed/);
  assert.equal(calls, 2);
});

test('malformed map never reaches Wrangler and exception contains no input', async () => {
  let calls = 0;
  await assert.rejects(publishHashes(entry, config, { token: 'synthetic-private-token' }, async () => { calls++; }), /invalid_hash_mapping/);
  assert.equal(calls, 0);
  assert.throws(() => validateHashes({ ...hashes, 'synthetic-2': hashes['synthetic-1'] }), /invalid_hash_mapping/);
});

test('CLI malformed input emits only fixed safe status, without invoking actual Wrangler', async () => {
  const child = spawn(process.execPath, [fileURLToPath(new URL('./publish-hashes.mjs', import.meta.url)), entry, config, '--free-confirmed', '--candidate-verified'],
    { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  let out = '', err = '';
  child.stdout.on('data', b => { out += b; }); child.stderr.on('data', b => { err += b; });
  child.stdin.end('{"token":"synthetic-private-token"}');
  const code = await new Promise(resolve => child.on('close', resolve));
  assert.equal(code, 1);
  assert.equal(out.trim(), 'hash_upload_failed_no_sensitive_details');
  assert.equal(err, '');
});
