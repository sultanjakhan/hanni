#!/usr/bin/env node
// Watches desktop/src/**/*.{js,css,html,mjs} and triggers WebView reload
// in the running Hanni dev (or prod) instance via /auto/eval.
// Port: HANNI_DEV_PORT env (default 8236).

import { watch } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const PORT = process.env.HANNI_DEV_PORT || '8236';
const HERE = dirname(fileURLToPath(import.meta.url));
const SRC = join(HERE, '..', 'src');
const TOKEN_PATH = join(homedir(), 'Library', 'Application Support', 'Hanni', 'api_token.txt');
const URL = `http://127.0.0.1:${PORT}/auto/eval`;

const token = (await readFile(TOKEN_PATH, 'utf8')).trim();
if (!token) { console.error('No API token at ' + TOKEN_PATH); process.exit(1); }

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
      headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
      body: JSON.stringify({ script: 'location.reload(); return "ok";' }),
    });
    const data = await res.json().catch(() => ({}));
    const ts = new Date().toLocaleTimeString();
    const sample = files.slice(0, 3).join(', ') + (files.length > 3 ? `, +${files.length - 3}` : '');
    console.log(`[${ts}] reload (${files.length}): ${sample}`);
    if (data && data.error) console.error('  api error:', data.error);
  } catch (e) {
    console.error(`[reload] ${e.message} (is dev running on :${PORT}?)`);
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
