#!/usr/bin/env node
// Watches desktop/src/**/*.{js,css,html,mjs} and triggers WebView reload
// through the fixed, debug-only /auto/reload action on port 8236.

import { watch } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const SRC = join(HERE, '..', 'src');
const URL = 'http://127.0.0.1:8236/auto/reload';
const token = process.env.HANNI_DEV_RELOAD_TOKEN?.trim();

if (!token) {
  console.error('HANNI_DEV_RELOAD_TOKEN is required for debug auto-reload.');
  process.exit(1);
}
if (!/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(token)) {
  console.error('HANNI_DEV_RELOAD_TOKEN must be a canonical lowercase UUID.');
  process.exit(1);
}

const isWatchable = (name) =>
  /\.(js|mjs|css|html)$/i.test(name) &&
  !name.includes('vendor/') && !name.includes('vendor\\') &&
  !name.includes('node_modules');

let timer = null;
const pending = new Set();

async function reload() {
  const files = [...pending];
  pending.clear();
  try {
    const res = await fetch(URL, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
      redirect: 'error',
      signal: AbortSignal.timeout(2000),
    });
    if (res.status !== 204) {
      throw new Error(`reload endpoint returned HTTP ${res.status}`);
    }
    const ts = new Date().toLocaleTimeString();
    const sample = files.slice(0, 3).join(', ') + (files.length > 3 ? `, +${files.length - 3}` : '');
    console.log(`[${ts}] reload (${files.length}): ${sample}`);
  } catch (e) {
    console.error(`[reload] ${e.message} (is debug Hanni running on :8236 with the same reload token?)`);
  }
}

function schedule(file) {
  pending.add(file);
  clearTimeout(timer);
  timer = setTimeout(reload, 250); // debounce: bundle bursts of saves
}

console.log(`[watch] ${SRC}`);
console.log(`[watch] target: ${URL}`);
console.log(`[watch] press Ctrl+C to stop`);

watch(SRC, { recursive: true }, (_event, filename) => {
  if (!filename) return;
  if (!isWatchable(filename)) return;
  schedule(filename);
});
