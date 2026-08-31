const assert = require('node:assert/strict');
const { validateApiMessage } = require('./api-policy.js');

const lookup = {
  type: 'hanni-api',
  path: '/api/vacancy?url=https%3A%2F%2Fexample.com%2Fjob%2F1',
  method: 'GET',
};
const save = {
  type: 'hanni-api',
  path: '/api/vacancy',
  method: 'POST',
  body: { url: 'https://example.com/job/1', stage: 'applied' },
};

assert.equal(validateApiMessage(lookup, 8235).method, 'GET');
assert.equal(validateApiMessage(save, 8236).method, 'POST');

for (const request of [
  { ...lookup, path: '/auto/eval?url=x' },
  { ...lookup, path: '/api/chat?url=x' },
  { ...lookup, path: 'http://attacker.invalid/api/vacancy?url=x' },
  { ...lookup, path: '//attacker.invalid/api/vacancy?url=x' },
  { ...lookup, path: '/api/vacancy?url=x&extra=y' },
  { ...lookup, method: 'DELETE' },
  { ...save, path: '/api/vacancy?extra=y' },
  { ...save, body: ['not', 'an', 'object'] },
]) {
  assert.equal(validateApiMessage(request, 8235), null, JSON.stringify(request));
}

assert.equal(validateApiMessage(lookup, 80), null);
assert.equal(validateApiMessage(lookup, '8235x'), null);
console.log('jobs extension API policy: ok');
