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
