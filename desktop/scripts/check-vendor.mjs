import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';

const installed = await readFile(new URL('../node_modules/dompurify/dist/purify.min.js', import.meta.url));
const vendored = await readFile(new URL('../src/vendor/purify.min.js', import.meta.url));
assert.deepEqual(vendored, installed, 'vendored DOMPurify differs from the locked npm package');
console.log(`DOMPurify vendor: ${createHash('sha256').update(vendored).digest('hex')}`);
