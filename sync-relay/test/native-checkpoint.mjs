// Run the actual Rust client against workerd/SQLite. Fixed synthetic credentials
// are scoped to this loopback runtime and must never be used for deployment.
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';

const runtime = process.env.HANNI_MINIFLARE_MODULE
  ? await import(process.env.HANNI_MINIFLARE_MODULE) : await import('miniflare');
const { Miniflare, convertV4MiniflareOptions } = runtime;
const hashes = Object.fromEntries([['device-a',3],['device-b',5]].map(([id,byte]) =>
  [id,createHash('sha256').update(Buffer.alloc(32,byte).toString('base64url')).digest('hex')]));
const mf = new Miniflare(convertV4MiniflareOptions({
  host: '127.0.0.1', port: 0, modules: true,
  script: await readFile(new URL('../src/worker.mjs',import.meta.url),'utf8'),
  compatibilityDate: '2026-09-01',
  durableObjects: { RELAY: { className: 'Relay', useSQLite: true } },
  bindings: { HANNI_DEVICE_TOKEN_HASHES: JSON.stringify(hashes) },
}));
// The production Worker requires HTTPS. Miniflare dispatch supplies that URL
// while this loopback-only adapter lets the native test avoid a fake trusted CA.
const proxy=createServer(async (request,response) => {
  try {
    const chunks=[];let size=0;
    for await (const chunk of request) {
      size+=chunk.length;
      if (size>96*1024) { response.writeHead(413).end();return; }
      chunks.push(chunk);
    }
    const headers={...request.headers};delete headers.host;delete headers.connection;delete headers['transfer-encoding'];
    const result=await mf.dispatchFetch(`https://relay.test${request.url}`,{
      method:request.method,headers,...(size ? {body:Buffer.concat(chunks)} : {}),
    });
    response.writeHead(result.status,Object.fromEntries(result.headers));
    response.end(Buffer.from(await result.arrayBuffer()));
  } catch { response.writeHead(502).end(); }
});
proxy.requestTimeout=30000;
try {
  if (!process.env.HANNI_NATIVE_TEST_EXE) throw new Error('Set the compiled native test executable');
  await mf.ready;
  await new Promise(resolve=>proxy.listen(0,'127.0.0.1',resolve));
  const url = `http://127.0.0.1:${proxy.address().port}`;
  const exit = await new Promise((resolve,reject) => {
    const child = spawn(process.env.HANNI_NATIVE_TEST_EXE,
      ['--ignored','--exact','cloud_relay::checkpoint::tests::real_workerd_checkpoint_roundtrip','--test-threads=1'],
      { windowsHide: true, stdio: 'inherit', env: { ...process.env,HANNI_RELAY_TEST_URL:url } });
    const timeout = setTimeout(() => { child.kill(); reject(new Error('Native client test timed out')); },120000);
    child.once('error', error => { clearTimeout(timeout); reject(error); });
    child.once('exit', code => { clearTimeout(timeout); resolve(code ?? 1); });
  });
  process.exitCode=exit;
} finally { proxy.closeAllConnections();await new Promise(resolve=>proxy.close(resolve));await mf.dispose(); }
