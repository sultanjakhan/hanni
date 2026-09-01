import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';

const normalizeLineEndings = (buffer) => Buffer.from(
  buffer.toString('utf8').replaceAll('\r\n', '\n'),
  'utf8',
);

const installed = normalizeLineEndings(
  await readFile(new URL('../node_modules/dompurify/dist/purify.min.js', import.meta.url)),
);
const vendored = normalizeLineEndings(
  await readFile(new URL('../src/vendor/purify.min.js', import.meta.url)),
);
assert.deepEqual(vendored, installed, 'vendored DOMPurify differs from the locked npm package');
console.log(`DOMPurify vendor: ${createHash('sha256').update(vendored).digest('hex')}`);
