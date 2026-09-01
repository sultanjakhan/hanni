import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { JSDOM } from 'jsdom';

const prototypeRoot = new URL('../../docs/prototypes/issue-34-now/', import.meta.url);

async function prototypeSources() {
  const [html, css, js, readme] = await Promise.all([
    readFile(new URL('index.html', prototypeRoot), 'utf8'),
    readFile(new URL('prototype.css', prototypeRoot), 'utf8'),
    readFile(new URL('prototype.js', prototypeRoot), 'utf8'),
    readFile(new URL('README.md', prototypeRoot), 'utf8'),
  ]);
  return { html, css, js, readme };
}

function createPrototype(html, js) {
  const dom = new JSDOM(html, {
    runScripts: 'outside-only',
    url: 'http://127.0.0.1:4173/',
    pretendToBeVisual: true,
  });

  let nextTimerId = 1;
  const timeouts = new Map();
  const intervals = new Map();
  dom.window.setTimeout = (callback) => {
    const id = nextTimerId++;
    timeouts.set(id, callback);
    return id;
  };
  dom.window.clearTimeout = (id) => timeouts.delete(id);
  dom.window.setInterval = (callback) => {
    const id = nextTimerId++;
    intervals.set(id, callback);
    return id;
  };
  dom.window.clearInterval = (id) => intervals.delete(id);

  dom.window.eval(js);

  return {
    dom,
    flushTimers() {
      let guard = 0;
      while (timeouts.size > 0) {
        assert.ok(guard++ < 30, 'prototype timers must settle');
        const pending = [...timeouts.entries()];
        timeouts.clear();
        for (const [, callback] of pending) callback();
      }
    },
  };
}

function selectScenario(window, value) {
  const select = window.document.getElementById('scenario-select');
  select.value = value;
  select.dispatchEvent(new window.Event('change', { bubbles: true }));
}

function themeVariables(css, selector) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const block = css.match(new RegExp(`${escapedSelector}\\s*\\{([\\s\\S]*?)\\n\\}`));
  assert.ok(block, `${selector} variables must exist`);
  return new Map([...block[1].matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})\s*;/gi)]
    .map(([, name, value]) => [name, value]));
}

function relativeLuminance(hex) {
  const channels = hex.match(/[0-9a-f]{2}/gi).map((value) => Number.parseInt(value, 16) / 255);
  const [red, green, blue] = channels.map((value) => (
    value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  ));
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function contrastRatio(first, second) {
  const values = [relativeLuminance(first), relativeLuminance(second)].sort((a, b) => b - a);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

test('prototype is self-contained and has no executable inline surface', async () => {
  const { html, js } = await prototypeSources();
  const dom = new JSDOM(html);
  const { document } = dom.window;

  assert.equal(document.querySelector('script[src="prototype.js"]')?.textContent.trim(), '');
  assert.ok(document.querySelector('link[rel="stylesheet"][href="prototype.css"]'));
  assert.equal(document.querySelector('script:not([src])'), null);
  assert.equal(document.querySelector('[style]'), null);

  for (const element of document.querySelectorAll('*')) {
    for (const attribute of element.getAttributeNames()) {
      assert.doesNotMatch(attribute, /^on/i, `${element.tagName} must not use ${attribute}`);
    }
  }

  for (const element of document.querySelectorAll('[src], [href]')) {
    const value = element.getAttribute('src') || element.getAttribute('href') || '';
    assert.doesNotMatch(value, /^(?:https?:)?\/\//i, `${value} must stay local`);
    assert.doesNotMatch(value, /^javascript:/i);
  }

  assert.doesNotMatch(js, /__TAURI__|\binvoke\s*\(|\bfetch\s*\(|XMLHttpRequest|WebSocket|localStorage|indexedDB/);
  assert.match(html, /Экспериментальный прототип · только синтетические данные/);
});

test('prototype exposes the authorized state, action and accessibility contract', async () => {
  const { html, js } = await prototypeSources();
  const dom = new JSDOM(html);
  const { document } = dom.window;

  for (const state of [
    'loading',
    'recommendation',
    'clarified',
    'dismissed',
    'active',
    'paused',
    'finishPending',
    'empty',
    'error',
  ]) {
    assert.ok(document.querySelector(`#scenario-select option[value="${state}"]`), state);
    assert.match(js, new RegExp(`(?:['"]|\\b)${state}(?:['"]|\\b)`));
  }

  for (const id of [
    'start-action',
    'pause-action',
    'finish-action',
    'dismiss-action',
    'restore-action',
    'clarify-action',
    'retry-action',
  ]) {
    assert.equal(document.getElementById(id)?.getAttribute('type'), 'button', id);
  }

  const clarify = document.getElementById('clarify-action');
  assert.equal(clarify.getAttribute('aria-controls'), 'clarification-panel');
  assert.equal(clarify.getAttribute('aria-expanded'), 'false');
  assert.equal(document.getElementById('now-live').getAttribute('aria-live'), 'polite');
  assert.equal(document.getElementById('error-callout').getAttribute('role'), 'alert');
  assert.equal(document.getElementById('now-card').getAttribute('aria-busy'), 'false');
  assert.equal(document.querySelector('.rail-link[aria-current="page"]').getAttribute('aria-label'), 'Calendar, текущий раздел');
  assert.equal(document.getElementById('theme-toggle').getAttribute('aria-label'), 'Включить тёмную тему');

  assert.match(html + js, /На сейчас нет подходящей задачи\./);
  assert.match(html + js, /Рекомендация скрыта до обновления\./);
  assert.match(html + js, /Не удалось обновить «Сейчас»\./);
});

test('prototype CSS covers Hanni themes, target widths and reduced motion', async () => {
  const { css } = await prototypeSources();

  assert.match(css, /\[data-theme="dark"\]/);
  assert.match(css, /@media\s*\(max-width:\s*800px\)/);
  assert.match(css, /@media\s*\(max-width:\s*680px\)/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
  assert.match(css, /\.now-main-row\s*\{[^}]*justify-content:\s*space-between/s);
  assert.match(css, /\.now-actions \.button--primary\s*\{[^}]*width:\s*100%/s);
  assert.match(css, /:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--accent-blue\)/s);
});

test('visible text and action color pairs meet WCAG AA contrast', async () => {
  const { css } = await prototypeSources();
  const themes = [themeVariables(css, ':root'), themeVariables(css, '[data-theme="dark"]')];
  const pairs = [
    ['text-secondary', 'bg-page'],
    ['text-muted', 'bg-page'],
    ['text-muted', 'bg-sidebar'],
    ['accent-blue', 'accent-blue-contrast'],
    ['accent-blue', 'bg-accent-soft'],
    ['accent-green', 'accent-green-soft'],
    ['accent-yellow', 'accent-yellow-soft'],
    ['accent-red', 'accent-red-soft'],
  ];

  for (const variables of themes) {
    for (const [foreground, background] of pairs) {
      const ratio = contrastRatio(variables.get(foreground), variables.get(background));
      assert.ok(ratio >= 4.5, `${foreground}/${background} contrast ${ratio.toFixed(3)} must be >= 4.5`);
    }
  }
});

test('clarify and dismiss flows restore keyboard focus', async () => {
  const { html, js } = await prototypeSources();
  const { dom, flushTimers } = createPrototype(html, js);
  const { document, KeyboardEvent } = dom.window;

  const card = document.getElementById('now-card');
  const clarify = document.getElementById('clarify-action');
  const panel = document.getElementById('clarification-panel');

  clarify.click();
  assert.equal(card.dataset.state, 'clarified');
  assert.equal(panel.hidden, false);
  assert.equal(clarify.getAttribute('aria-expanded'), 'true');
  assert.equal(document.activeElement, panel);

  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  assert.equal(card.dataset.state, 'recommendation');
  assert.equal(panel.hidden, true);
  assert.equal(document.activeElement, clarify);

  document.getElementById('dismiss-action').click();
  assert.equal(card.dataset.state, 'dismissed');
  assert.equal(document.activeElement, document.getElementById('restore-action'));

  document.getElementById('restore-action').click();
  assert.equal(card.dataset.state, 'recommendation');
  assert.equal(document.activeElement, document.getElementById('start-action'));
  flushTimers();
});

test('actions are single-flight and retry preserves the failed operation context', async () => {
  const { html, js } = await prototypeSources();
  const { dom, flushTimers } = createPrototype(html, js);
  const { document } = dom.window;
  const card = document.getElementById('now-card');

  const start = document.getElementById('start-action');
  start.click();
  assert.equal(card.getAttribute('aria-busy'), 'true');
  assert.equal(start.disabled, true);
  start.click();
  flushTimers();
  assert.equal(card.dataset.state, 'active');

  document.getElementById('simulate-failure').checked = true;
  document.getElementById('pause-action').click();
  assert.equal(card.getAttribute('aria-busy'), 'true');
  flushTimers();

  assert.equal(card.dataset.state, 'error');
  assert.equal(document.getElementById('error-callout').hidden, false);
  assert.match(document.getElementById('error-context').textContent, /Активная задача не была изменена/);
  assert.match(document.getElementById('error-context').textContent, /Подготовить вопросы к интервью/);
  assert.equal(document.activeElement, document.getElementById('retry-action'));

  document.getElementById('retry-action').click();
  flushTimers();
  assert.equal(card.dataset.state, 'paused');
  assert.equal(document.getElementById('start-action').textContent, 'Продолжить');

  document.getElementById('start-action').click();
  flushTimers();
  assert.equal(card.dataset.state, 'active');

  document.getElementById('finish-action').click();
  assert.equal(card.dataset.state, 'finishPending');
  assert.equal(card.getAttribute('aria-busy'), 'true');
  assert.equal(document.getElementById('finish-action').textContent, 'Завершаем…');
  flushTimers();
  assert.equal(card.dataset.state, 'empty');
  assert.equal(document.getElementById('now-title').textContent, 'На сейчас нет подходящей задачи.');
  assert.equal(document.activeElement, card);
});

test('paused recommendation keeps resume semantics through clarify and dismiss', async () => {
  const { html, js } = await prototypeSources();
  const { dom, flushTimers } = createPrototype(html, js);
  const { document } = dom.window;
  const card = document.getElementById('now-card');
  const start = document.getElementById('start-action');

  selectScenario(dom.window, 'paused');
  document.getElementById('clarify-action').click();
  assert.equal(card.dataset.state, 'clarified');
  assert.equal(start.textContent, 'Продолжить');
  assert.equal(document.getElementById('now-status-label').textContent, 'Приостановлено');

  start.click();
  flushTimers();
  assert.equal(card.dataset.state, 'active');

  selectScenario(dom.window, 'paused');
  document.getElementById('clarify-action').click();
  document.getElementById('dismiss-action').click();
  assert.equal(card.dataset.state, 'dismissed');
  document.getElementById('restore-action').click();
  assert.equal(card.dataset.state, 'paused');
  assert.equal(start.textContent, 'Продолжить');
  assert.equal(document.activeElement, start);
});

test('scenario and theme controls expose static review states without persistence', async () => {
  const { html, js } = await prototypeSources();
  const { dom, flushTimers } = createPrototype(html, js);
  const { document } = dom.window;
  const card = document.getElementById('now-card');

  selectScenario(dom.window, 'loading');
  assert.equal(card.dataset.state, 'loading');
  assert.equal(card.getAttribute('aria-busy'), 'true');
  assert.equal(document.getElementById('loading-state').hidden, false);

  selectScenario(dom.window, 'error');
  assert.equal(card.dataset.state, 'error');
  assert.equal(document.getElementById('retry-action').hidden, false);

  selectScenario(dom.window, 'empty');
  assert.equal(card.dataset.state, 'empty');
  assert.equal(document.getElementById('now-title').textContent, 'На сейчас нет подходящей задачи.');
  assert.equal(document.getElementById('now-meta').textContent, 'План не изменён.');

  const theme = document.getElementById('theme-toggle');
  theme.click();
  assert.equal(document.documentElement.dataset.theme, 'dark');
  assert.equal(theme.getAttribute('aria-pressed'), 'true');
  assert.equal(theme.getAttribute('aria-label'), 'Включить светлую тему');
  theme.click();
  assert.equal(document.documentElement.dataset.theme, 'light');
  assert.equal(theme.getAttribute('aria-pressed'), 'false');
  assert.equal(theme.getAttribute('aria-label'), 'Включить тёмную тему');
  flushTimers();
});

test('README records evidence, local serving and prototype limitations', async () => {
  const { readme } = await prototypeSources();

  assert.match(readme, /Status:|Статус:/i);
  assert.match(readme, /EXPERIMENT/);
  assert.match(readme, /hanni-tasks\/issues\/34/);
  assert.match(readme, /b7439fd4555b20da303cd66646c7b8deae7d77ae/);
  assert.match(readme, /py -m http\.server 4173 --directory docs\/prototypes\/issue-34-now/);
  assert.match(readme, /не production-код/i);
  assert.match(readme, /не native\/live evidence/i);
  assert.match(readme, /desktop\/src/);
});
