import assert from 'node:assert/strict';
import { readFile, readdir, stat } from 'node:fs/promises';
import test from 'node:test';
import { JSDOM } from 'jsdom';
import createDOMPurify from 'dompurify';
import { sanitizeMarkdownHtml } from '../src/js/markdown-security.js';
import { parseRecipeSteps } from '../src/js/recipe-step-security.js';

function sanitizer() {
  const dom = new JSDOM('<!doctype html><body></body>');
  return { dom, purify: createDOMPurify(dom.window) };
}

function cspDirectives(policy) {
  return new Map(policy.split(';').map(part => {
    const [name, ...values] = part.trim().split(/\s+/);
    return [name, values];
  }).filter(([name]) => name));
}

async function firstPartyJavaScript(root) {
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    if (entry.name === 'vendor' || entry.name === 'assets') continue;
    const url = new URL(entry.name + (entry.isDirectory() ? '/' : ''), root);
    if (entry.isDirectory()) files.push(...await firstPartyJavaScript(url));
    else if (entry.name.endsWith('.js') && !entry.name.endsWith('.min.js')) files.push(url);
  }
  return files;
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

test('native entrypoints enforce external scripts under the same CSP exception', async () => {
  const [indexHtml, focusHtml, tauriConfigText, themeBoot, focusJavaScript] = await Promise.all([
    readFile(new URL('../src/index.html', import.meta.url), 'utf8'),
    readFile(new URL('../src/focus-overlay.html', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
    readFile(new URL('../src/js/theme-boot.js', import.meta.url), 'utf8'),
    readFile(new URL('../src/focus-overlay.js', import.meta.url), 'utf8'),
  ]);
  const indexDom = new JSDOM(indexHtml);
  const focusDom = new JSDOM(focusHtml);
  const indexPolicy = indexDom.window.document
    .querySelector('meta[http-equiv="Content-Security-Policy"]')
    ?.getAttribute('content');
  const configPolicy = JSON.parse(tauriConfigText).app.security.csp;

  for (const [source, policy] of [['index meta', indexPolicy], ['Tauri config', configPolicy]]) {
    assert.ok(policy, `${source} CSP is present`);
    const scriptSources = cspDirectives(policy).get('script-src');
    assert.deepEqual(scriptSources, ["'self'", "'wasm-unsafe-eval'"], `${source} script-src`);
    assert.equal(scriptSources.includes("'unsafe-inline'"), false, source);
    assert.equal(scriptSources.includes("'unsafe-eval'"), false, source);
  }

  for (const [name, dom] of [['index.html', indexDom], ['focus-overlay.html', focusDom]]) {
    for (const script of dom.window.document.querySelectorAll('script')) {
      assert.ok(script.hasAttribute('src'), `${name} has an inline executable script`);
    }
    for (const element of dom.window.document.querySelectorAll('*')) {
      for (const attribute of element.getAttributeNames()) {
        assert.equal(/^on/i.test(attribute), false, `${name} has ${attribute}`);
      }
      for (const attribute of ['href', 'src', 'action', 'formaction', 'xlink:href']) {
        assert.equal(
          element.getAttribute(attribute)?.trim().toLowerCase().startsWith('javascript:') ?? false,
          false,
          `${name} has a javascript: ${attribute}`
        );
      }
    }
  }

  assert.match(indexHtml, /<script src="js\/theme-boot\.js"><\/script>/);
  assert.ok(indexHtml.indexOf('js/theme-boot.js') < indexHtml.indexOf('styles.css'));
  assert.match(focusHtml, /<script src="focus-overlay\.js"><\/script>/);
  assert.match(themeBoot, /localStorage\.getItem\('hanni_theme'\)/);
  assert.match(themeBoot, /setAttribute\(\s*'data-theme'/);
  assert.match(focusJavaScript, /listen\('focus-state'/);
  assert.match(focusJavaScript, /invoke\('stop_activity'\)/);
  assert.match(focusJavaScript, /invoke\('get_current_activity'\)/);
});

test('first-party generated markup contains no CSP-blocked event attributes', async () => {
  const sources = [
    ...await firstPartyJavaScript(new URL('../src/', import.meta.url)),
    ...await firstPartyJavaScript(new URL('../src-tauri/src/share_assets/', import.meta.url)),
  ];
  const htmlEventAttribute = /<[a-z][a-z0-9:-]*\b[^>]*\son[a-z][a-z0-9-]*\s*=/i;
  const setEventAttribute = /setAttribute\(\s*['"]on[a-z][a-z0-9-]*['"]/i;
  const javascriptUrl = /<[a-z][a-z0-9:-]*\b[^>]*\s(?:href|src|action|formaction|xlink:href)\s*=\s*['"]?\s*javascript:/i;
  const inlineExecutableScript = /<script(?:\s[^>]*)?>[\s\S]*?<\/script>/i;

  for (const url of sources) {
    const source = await readFile(url, 'utf8');
    assert.doesNotMatch(source, htmlEventAttribute, url.pathname);
    assert.doesNotMatch(source, setEventAttribute, url.pathname);
    assert.doesNotMatch(source, javascriptUrl, url.pathname);
    assert.doesNotMatch(source, inlineExecutableScript, url.pathname);
  }
});

test('guest share page externalizes context and bootstrap under response CSP', async () => {
  const [guestHtml, guestJavaScript, shareServer, utilities, chat, capability, macos] = await Promise.all([
    readFile(new URL('../src-tauri/src/share_assets/guest.html', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/share_assets/guest.js', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/share_server.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src/js/utils.js', import.meta.url), 'utf8'),
    readFile(new URL('../src/js/chat.js', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/capabilities/default.json', import.meta.url), 'utf8'),
    readFile(new URL('../src-tauri/src/macos.rs', import.meta.url), 'utf8'),
  ]);
  const dom = new JSDOM(guestHtml);
  const body = dom.window.document.body;

  for (const script of dom.window.document.querySelectorAll('script')) {
    assert.ok(script.hasAttribute('src'), 'guest.html has an inline executable script');
  }
  for (const element of dom.window.document.querySelectorAll('*')) {
    for (const attribute of element.getAttributeNames()) {
      assert.equal(/^on/i.test(attribute), false, `guest.html has ${attribute}`);
    }
    for (const attribute of ['href', 'src', 'action', 'formaction', 'xlink:href']) {
      assert.equal(
        element.getAttribute(attribute)?.trim().toLowerCase().startsWith('javascript:') ?? false,
        false,
        `guest.html has a javascript: ${attribute}`
      );
    }
  }
  for (const attribute of [
    'data-share-token',
    'data-share-tab',
    'data-share-scope',
    'data-share-permissions',
    'data-share-label',
  ]) assert.equal(body.hasAttribute(attribute), true, attribute);
  assert.doesNotMatch(guestHtml, /window\.__SHARE__|renderShell\?\./);
  assert.match(guestJavaScript, /document\.body\.dataset/);
  assert.match(guestJavaScript, /DOMContentLoaded/);
  assert.match(shareServer, /header::CONTENT_SECURITY_POLICY/);
  assert.match(shareServer, /script-src 'self'/);
  assert.doesNotMatch(
    shareServer.match(/const SHARE_CSP: &str = "([^"]+)"/)?.[1] || '',
    /script-src[^;]*unsafe-inline/
  );

  assert.match(chat, /data-open-url="http:\/\/127\.0\.0\.1:18789\/"/);
  assert.match(utilities, /invoke\('open_url', \{ url \}\)/);
  assert.equal(JSON.parse(capability).permissions.includes('opener:default'), true);
  assert.match(macos, /use tauri_plugin_opener::OpenerExt/);
  assert.match(macos, /validate_external_url\(&url\)\?/);
  assert.match(macos, /app\.opener\(\)[\s\S]*?\.open_url\(url\.clone\(\), None::<&str>\)/);
});

test('wasm CSP exception is justified by the bundled Draco decoder', async () => {
  const [bodyViewer, bodyModel, wrapper, wasm, architecture] = await Promise.all([
    readFile(new URL('../src/js/body-viewer.js', import.meta.url), 'utf8'),
    readFile(new URL('../src/assets/body.glb', import.meta.url)),
    readFile(new URL('../src/assets/draco/draco_wasm_wrapper.js', import.meta.url), 'utf8'),
    readFile(new URL('../src/assets/draco/draco_decoder.wasm', import.meta.url)),
    readFile(new URL('../../docs/architecture/quick-reference.md', import.meta.url), 'utf8'),
  ]);
  const wasmInfo = await stat(new URL('../src/assets/draco/draco_decoder.wasm', import.meta.url));

  assert.match(bodyViewer, /new window\.THREE_DRACOLoader\(\)/);
  assert.match(bodyViewer, /setDecoderPath\('\.\/assets\/draco\/'\)/);
  assert.equal(bodyModel.includes(Buffer.from('KHR_draco_mesh_compression')), true);
  assert.match(wrapper, /WebAssembly\.instantiate(?:Streaming)?/);
  assert.equal(wasm.subarray(0, 4).toString('hex'), '0061736d');
  assert.ok(wasmInfo.size > 0);
  assert.match(architecture, /wasm-unsafe-eval[\s\S]*Draco/i);
});
