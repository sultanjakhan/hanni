import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const WORKER = 'hanni-personal-relay-v2';
const SAFE_ENV = { CI: 'true', WRANGLER_WRITE_LOGS: 'false', WRANGLER_LOG: 'none',
  WRANGLER_LOG_SANITIZE: 'true', WRANGLER_SEND_METRICS: 'false' };

export function validateHashes(value) {
  const pairs = value && typeof value === 'object' && !Array.isArray(value) ? Object.entries(value) : [];
  if (pairs.length !== 4 || pairs.some(([id, hash]) => !/^[A-Za-z0-9_-]{1,64}$/.test(id)
    || typeof hash !== 'string' || !/^[a-f0-9]{64}$/.test(hash)) || new Set(pairs.map(([, hash]) => hash)).size !== 4) {
    throw new Error('invalid_hash_mapping');
  }
  return JSON.stringify(value);
}

export function runWrangler(entry, args, input = '') {
  return new Promise((resolveResult, reject) => {
    const child = spawn(process.execPath, [entry, ...args], { windowsHide: true, shell: false,
      env: { ...process.env, ...SAFE_ENV }, stdio: ['pipe', 'pipe', 'pipe'] });
    let output = ''; let size = 0;
    const timer = setTimeout(() => { child.kill(); reject(new Error('wrangler_timeout')); }, 45000);
    child.stdout.on('data', bytes => {
      size += bytes.length;
      if (size > 1024 * 1024) { child.kill(); reject(new Error('wrangler_output_limit')); }
      else output += bytes.toString('utf8');
    });
    // Drain raw stderr without logging or retaining it.
    child.stderr.on('data', () => {});
    child.stdin.on('error', () => {});
    child.on('error', () => { clearTimeout(timer); reject(new Error('wrangler_start_failed')); });
    child.on('close', code => { clearTimeout(timer); resolveResult({ code, output }); });
    child.stdin.end(input);
  });
}

export async function publishHashes(entry, config, hashes, run = runWrangler) {
  const input = validateHashes(hashes);
  // Version pin documents and bounds the audited CLI behavior.
  const pkg = JSON.parse(await readFile(resolve(dirname(entry), '..', 'package.json'), 'utf8'));
  if (pkg.name !== 'wrangler' || pkg.version !== '4.129.0') throw new Error('wrangler_version_not_reviewed');
  const common = ['--config', config, '--name', WORKER];
  const check = await run(entry, ['versions', 'list', '--json', ...common]);
  if (check.code !== 0) throw new Error('existing_worker_required');
  const versions = JSON.parse(check.output);
  if (!Array.isArray(versions) || !versions.length || versions.some(v => typeof v.id !== 'string')) throw new Error('existing_worker_required');
  // Plain `secret put` can automatically CREATE a missing Worker in CI.
  // `versions secret put` only patches an existing latest version and does not
  // activate it. Thus a delete-after-check race still fails closed.
  const upload = await run(entry, ['versions', 'secret', 'put', 'HANNI_DEVICE_TOKEN_HASHES',
    '--tag', 'hanni-pairing-v1', '--message', 'Initial device authentication hashes', ...common], input);
  if (upload.code !== 0) throw new Error('hash_version_upload_failed');
  return 'hashes_version_created_activation_required';
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    if (process.argv.length !== 6 || process.argv[4] !== '--free-confirmed' || process.argv[5] !== '--candidate-verified') throw new Error('arguments_required');
    let input = ''; let length = 0;
    for await (const chunk of process.stdin) {
      length += chunk.length;
      if (length > 8192) throw new Error('input_limit');
      input += chunk.toString('utf8');
    }
    const result = await publishHashes(resolve(process.argv[2]), resolve(process.argv[3]), JSON.parse(input));
    process.stdout.write(result + '\n');
  } catch (_) {
    process.stdout.write('hash_upload_failed_no_sensitive_details\n');
    process.exitCode = 1;
  }
}
