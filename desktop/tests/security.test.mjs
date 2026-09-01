import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { JSDOM } from 'jsdom';
import createDOMPurify from 'dompurify';
import { sanitizeMarkdownHtml } from '../src/js/markdown-security.js';
import { parseRecipeSteps } from '../src/js/recipe-step-security.js';

function sanitizer() {
  const dom = new JSDOM('<!doctype html><body></body>');
  return { dom, purify: createDOMPurify(dom.window) };
}

test('markdown sanitizer removes executable and foreign content', () => {
  const { dom, purify } = sanitizer();
  const payloads = [
    '<script>window.pwned=1</script>',
    '<img src=x onerror="window.pwned=1">',
    '<svg><a onload="window.pwned=1"></a></svg>',
    '<math><mtext><img src=x onerror="window.pwned=1"></mtext></math>',
    '<iframe srcdoc="<script>window.pwned=1</script>"></iframe>',
    '<a href="javascript:window.pwned=1" onclick="window.pwned=1">x</a>',
    '<form><button formaction="javascript:window.pwned=1">x</button></form>',
  ];
  for (const payload of payloads) {
    const host = dom.window.document.createElement('div');
    host.innerHTML = sanitizeMarkdownHtml(payload, payload, purify);
    assert.equal(host.querySelector('script,img,svg,math,iframe,form,[onerror],[onload],[onclick],[formaction]'), null, payload);
    assert.notEqual(host.querySelector('a')?.getAttribute('href')?.startsWith('javascript:'), true, payload);
  }
});

test('markdown sanitizer preserves required presentation markup', () => {
  const { purify } = sanitizer();
  const safe = '<p><strong>ok</strong></p><pre><code class="hljs">x</code></pre>'
    + '<button class="code-copy-btn" type="button">copy</button>'
    + '<a class="md-link" href="#" data-href="https://example.com">link</a>';
  const out = sanitizeMarkdownHtml(safe, '', purify);
  assert.match(out, /code-copy-btn/);
  assert.match(out, /data-href="https:\/\/example.com"/);
  assert.match(out, /<strong>ok<\/strong>/);
});

test('missing sanitizer fails closed to escaped source', () => {
  const out = sanitizeMarkdownHtml('<p>parsed</p>', '<img src=x onerror=x>', undefined);
  assert.equal(out, '&lt;img src=x onerror=x&gt;');
});

test('recipe image helper never interpolates stored image data into markup', async () => {
  const dom = new JSDOM('<!doctype html><body></body>', { runScripts: 'outside-only' });
  const source = await readFile(new URL('../src/js/recipe-shared-image.js', import.meta.url), 'utf8');
  dom.window.eval(source);
  const image = dom.window.HanniRecipe.image;
  const jpeg = 'data:image/jpeg;base64,/9j/4P/Z';
  assert.equal(image.safeSrc(jpeg), jpeg);
  assert.equal(image.safeSrc('x" onerror="window.pwned=1'), '');
  assert.equal(image.safeSrc('data:image/svg+xml,<svg onload=alert(1)>'), '');
  assert.doesNotMatch(image.fieldHtml({ image: 'x" onerror="alert(1)' }), /src=/i);

  const overlay = dom.window.document.createElement('div');
  overlay.innerHTML = image.fieldHtml();
  const state = { image: 'x" onerror="window.pwned=1' };
  image.attach(overlay, state);
  assert.equal(state.image, '');
  assert.equal(overlay.querySelector('img').hasAttribute('src'), false);
  assert.equal(overlay.querySelector('[onerror]'), null);
});

test('structured recipe minutes are numeric and bounded before rendering', () => {
  const attack = JSON.stringify([{
    text: 'Boil',
    min: '</span><img src=x onerror="window.pwned=1">',
    ingredients: ['water'],
  }]);
  const [step] = parseRecipeSteps(attack);
  assert.equal(step.min, 0);
  assert.equal(typeof step.min, 'number');
  assert.equal(parseRecipeSteps('[{"text":"Wait","min":999999}]')[0].min, 1440);
});

test('secret provisioning paths stay encrypted and redacted at UI boundaries', async () => {
  const [store, meta, google, share, lan] = await Promise.all([
    readFile(new URL('../src-tauri/src/secret_store.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/commands_meta.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/google_auth.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/sync_share.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/lan_sync.rs', import.meta.url), 'utf8'),
  ]);

  assert.match(store, /CryptProtectData/);
  assert.match(store, /CryptUnprotectData/);
  assert.match(meta, /sensitive settings cannot be read through the generic settings API/);
  assert.doesNotMatch(google, /missing id_token:\s*\{\}\",\s*google_resp/);
  assert.doesNotMatch(google, /missing localId:\s*\{\}\",\s*fb_resp/);
  assert.match(share, /Ok\(redact_config\(cfg\)\)/);

  const getConfig = lan.slice(
    lan.indexOf('pub fn lan_sync_get_config'),
    lan.indexOf('pub fn lan_sync_set_config')
  );
  assert.match(getConfig, /"key_set"/);
  assert.doesNotMatch(getConfig, /"key"\s*:/);
});

test('automation surface is fixed-action debug-only and logs metadata only', async () => {
  const [meta, lib, securityUi, reloadTool, reloadShell] = await Promise.all([
    readFile(new URL('../src-tauri/src/commands_meta.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src/js/settings-security.js', import.meta.url), 'utf8'),
    readFile(new URL('../tools/auto-reload.mjs', import.meta.url), 'utf8'),
    readFile(new URL('../tools/auto-reload.sh', import.meta.url), 'utf8'),
  ]);

  assert.doesNotMatch(meta, /route\(\s*["']\/auto\/eval/);
  assert.doesNotMatch(meta, /AutoEvalCallbacks|auto_eval_callback|EvalReq/);
  assert.doesNotMatch(lib, /AutoEvalCallbacks|auto_eval_callback/);

  const reloadHandler = meta.slice(
    meta.indexOf('async fn auto_reload'),
    meta.indexOf('async fn google_oauth_callback')
  );
  assert.match(meta, /#\[cfg\(debug_assertions\)\][\s\S]*?async fn auto_reload/);
  assert.match(meta, /HANNI_DEV_RELOAD_TOKEN/);
  assert.match(meta, /must differ from the API and Jobs credentials/);
  assert.match(reloadHandler, /window\.reload\(\)/);
  assert.doesNotMatch(reloadHandler, /Json\s*<|script|\.eval\(/);
  assert.match(meta, /route\("\/auto\/reload", post\(auto_reload\)\)/);

  const logRow = meta.slice(
    meta.indexOf('pub struct AutomationLogRow'),
    meta.indexOf('fn get_or_create_token')
  );
  assert.doesNotMatch(logRow, /script_preview|pub script\s*:/);
  assert.doesNotMatch(securityUi, /script_preview|\/auto\/eval/);
  assert.match(securityUi, /тела запросов и скрипты не сохраняются/);

  assert.match(reloadTool, /http:\/\/127\.0\.0\.1:8236\/auto\/reload/);
  assert.match(reloadTool, /process\.env\.HANNI_DEV_RELOAD_TOKEN/);
  assert.match(reloadTool, /Authorization: `Bearer \$\{token\}`/);
  assert.doesNotMatch(reloadTool, /readFile|homedir|api_token\.txt|HANNI_DEV_PORT|process\.argv|8235/);
  const reloadOutputCalls = [...reloadTool.matchAll(/console\.(?:log|error)\([^;]*\);/g)]
    .map(match => match[0])
    .join('\n');
  assert.doesNotMatch(reloadOutputCalls, /\$\{token\}|\+\s*token/);
  assert.doesNotMatch(reloadShell, /\/auto\/eval|HANNI_DEV_PORT|8235|\$\{1|\$1/);

  const scrub = lib.indexOf('db::migrate_automation_log(&conn)');
  const oldBackups = lib.indexOf('secret_store::migrate_backup_databases(&data_dir)');
  const newBackup = lib.indexOf('backup_db()');
  assert.ok(scrub > 0 && scrub < oldBackups && oldBackups < newBackup);
});
