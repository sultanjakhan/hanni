import assert from 'node:assert/strict';
import {createHash,randomBytes,randomUUID} from 'node:crypto';
import {mkdtemp,readFile,mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import test from 'node:test';
async function loadRuntime(){
  if(process.env.HANNI_MINIFLARE_MODULE)return import(process.env.HANNI_MINIFLARE_MODULE);
  try{return await import('miniflare');}
  catch(error){if(error.code!=='ERR_MODULE_NOT_FOUND')throw error;return import(new URL('../../cloudflare-tooling/node_modules/miniflare/dist/src/index.js',import.meta.url).href);}
}
const {Miniflare,convertV4MiniflareOptions}=await loadRuntime();
const source=await readFile(new URL('../src/worker.mjs',import.meta.url),'utf8');
const devices=Object.fromEntries(['windows','mac','phone-a','phone-b'].map(id=>[id,randomBytes(32).toString('base64url')]));
const hash=value=>createHash('sha256').update(value).digest('hex');
const hashes=Object.fromEntries(Object.entries(devices).map(([id,token])=>[id,hash(token)]));
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
function runtime(bindings={},persistence,script=source,className='Relay'){
  return new Miniflare(convertV4MiniflareOptions({modules:true,script,compatibilityDate:'2026-09-01',
    durableObjects:{RELAY:{className,useSQLite:true}},bindings:{HANNI_DEVICE_TOKEN_HASHES:JSON.stringify(hashes),...bindings},
    ...(persistence?{resourcePersistencePath:persistence}:{})}));
}
function envelope(bytes=32){return {v:1,alg:'XChaCha20-Poly1305',key_id:'synthetic-key',nonce:randomBytes(24).toString('base64url'),ciphertext:randomBytes(bytes).toString('base64url')};}
function batch(client_seq,bytes=32){return {client_seq,batch_id:randomUUID(),envelope:envelope(bytes)};}
function request(mf,path,{method='GET',body,device='windows',headers={}}={}){
  return mf.dispatchFetch(`https://relay.test${path}`,{method,headers:{Authorization:`Bearer ${devices[device]}`,...(body===undefined?{}:{'Content-Type':'application/json'}),...headers},
    ...(body===undefined?{}:{body:JSON.stringify(body)})});
}
const append=(mf,b,device='windows')=>request(mf,'/v1/batches',{method:'POST',body:b,device});
async function ok(response,status=200){const value=await response.json();assert.equal(response.status,status,JSON.stringify(value));return value;}
async function error(response,status,code){const value=await response.json();assert.equal(response.status,status,JSON.stringify(value));assert.equal(value.error,code);return value;}
async function acquire(mf,{base=1,generation=0,device='windows',parts=[envelope()],id=randomUUID()}={}){
  const body={checkpoint_id:id,expected_generation:generation,base_seq:base,chunk_count:parts.length,total_bytes:parts.reduce((n,p)=>n+Buffer.byteLength(JSON.stringify(p)),0)};
  const lease=await ok(await request(mf,'/v1/checkpoints/lease',{method:'POST',body,device}),201);
  return {id,body,lease,parts,device};
}
async function put(mf,cp,index){return request(mf,`/v1/checkpoints/${cp.id}/chunks/${index}`,{method:'PUT',device:cp.device,body:{lease_epoch:cp.lease.lease_epoch,envelope:cp.parts[index]}});}
function manifest(cp){return {lease_epoch:cp.lease.lease_epoch,chunk_root_sha256:hash(JSON.stringify(cp.parts.map(p=>hash(JSON.stringify(p))))),envelope:envelope()};}
const finalize=(mf,cp,body)=>request(mf,`/v1/checkpoints/${cp.id}/finalize`,{method:'POST',device:cp.device,body});
async function publish(mf,options){const cp=await acquire(mf,options);for(let i=0;i<cp.parts.length;i++)await ok(await put(mf,cp,i),201);cp.manifest=manifest(cp);cp.ack=await ok(await finalize(mf,cp,cp.manifest),201);return cp;}
async function maintenance(mf){return ok(await request(mf,'/v1/maintenance',{method:'POST',body:{}}));}
async function readLease(mf,cp,device='mac'){return ok(await request(mf,`/v1/checkpoints/${cp.id}/read-lease`,{method:'POST',body:{},device}),201);}
function download(mf,cp,lease,index=null,device='mac'){
  return request(mf,`/v1/checkpoints/${cp.id}${index===null?'':`/chunks/${index}`}`,{device,headers:{'X-Hanni-Read-Lease':lease.read_lease_id}});
}

test('monotonic uploads retain exact last ACK after compaction and free the retained-count cap',async t=>{
  const mf=runtime({HANNI_MAX_RETAINED_BATCHES:'2'});t.after(()=>mf.dispose());
  const one=batch(1),two=batch(2),three=batch(3);
  await ok(await append(mf,one),201);const ack=await ok(await append(mf,two),201);
  await error(await append(mf,three),507,'relay_capacity_reached');
  const cp=await publish(mf,{base:2});await maintenance(mf);
  const duplicate=await ok(await append(mf,two));assert.equal(duplicate.seq,ack.seq);assert.equal(duplicate.envelope_sha256,ack.envelope_sha256);
  await error(await append(mf,{...two,envelope:envelope()}),409,'batch_payload_mismatch');
  await error(await append(mf,one),409,'device_state_stale');
  await error(await append(mf,batch(4)),409,'client_sequence_gap');
  assert.equal((await ok(await append(mf,three),201)).seq,3);
  const gap=await error(await request(mf,'/v1/batches?after=0'),409,'checkpoint_required');assert.equal(gap.checkpoint.checkpoint_id,cp.id);
  const tail=await ok(await request(mf,'/v1/batches?after=2'));assert.equal(tail.batches[0].client_seq,3);assert.equal(tail.next_cursor,3);
  const state=await ok(await request(mf,'/v1/device-state'));assert.equal(state.accepted_client_seq,3);
  const other=await ok(await request(mf,'/v1/device-state',{device:'mac'}));assert.equal(other.accepted_client_seq,0);assert.equal(other.last_ack,null);
});

test('incomplete, changed, reordered, wrong-root and wrong-key snapshots cannot compact data',async t=>{
  const mf=runtime();t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);
  const cp=await acquire(mf,{parts:[envelope(),envelope()]});const body=manifest(cp);
  await ok(await put(mf,cp,1),201);await error(await finalize(mf,cp,body),409,'checkpoint_incomplete');
  await ok(await put(mf,cp,1));const changed={...cp,parts:[cp.parts[0],envelope()]};await error(await put(mf,changed,1),409,'chunk_payload_mismatch');
  await ok(await put(mf,cp,0),201);
  const reversedRoot=hash(JSON.stringify(cp.parts.slice().reverse().map(p=>hash(JSON.stringify(p)))));
  await error(await finalize(mf,cp,{...body,chunk_root_sha256:reversedRoot}),400,'checkpoint_digest_mismatch');
  await error(await finalize(mf,cp,{...body,envelope:{...body.envelope,key_id:'other-key'}}),400,'checkpoint_key_mismatch');
  assert.equal((await ok(await request(mf,'/v1/batches'))).batches.length,1);
  const committed=await ok(await finalize(mf,cp,body),201);const retry=await ok(await finalize(mf,cp,body));assert.equal(retry.generation,committed.generation);assert.equal(retry.duplicate,true);
  await error(await finalize(mf,cp,{...body,envelope:envelope()}),409,'checkpoint_payload_mismatch');
});

test('append continues while a snapshot uploads; finalize preserves the newer tail',async t=>{
  const mf=runtime();t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);
  const cp=await acquire(mf);await ok(await append(mf,batch(1),'phone-a'),201);
  await ok(await put(mf,cp,0),201);await ok(await finalize(mf,cp,manifest(cp)),201);await maintenance(mf);
  const page=await ok(await request(mf,'/v1/batches?after=1'));assert.equal(page.batches.length,1);assert.equal(page.batches[0].sender_device_id,'phone-a');assert.equal(page.latest_seq,2);
});

test('expired uploader releases election; epoch fences late chunks/finalize without blocking append',async t=>{
  const mf=runtime({HANNI_LEASE_MS:'100'});t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);
  const old=await acquire(mf);await ok(await put(mf,old,0),201);
  await error(await request(mf,'/v1/checkpoints/lease',{method:'POST',device:'mac',body:{...old.body,checkpoint_id:randomUUID()}}),409,'checkpoint_lease_busy');
  await sleep(130);await ok(await append(mf,batch(2)),201);
  const next=await acquire(mf,{device:'mac',base:2});assert.ok(next.lease.lease_epoch>old.lease.lease_epoch);
  await error(await finalize(mf,old,manifest(old)),409,'checkpoint_lease_expired');
  await ok(await put(mf,next,0),201);await ok(await finalize(mf,next,manifest(next)),201);
});

test('renewal is fenced and a stale expected generation cannot replace a checkpoint',async t=>{
  const mf=runtime();t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);
  const cp=await acquire(mf);const stale=cp.lease.lease_epoch;
  cp.lease=await ok(await request(mf,'/v1/checkpoints/lease',{method:'POST',body:cp.body}),201);assert.ok(cp.lease.lease_epoch>stale);
  await error(await request(mf,`/v1/checkpoints/${cp.id}/chunks/0`,{method:'PUT',body:{lease_epoch:stale,envelope:cp.parts[0]}}),409,'checkpoint_lease_expired');
  await ok(await put(mf,cp,0),201);await ok(await finalize(mf,cp,manifest(cp)),201);
  await error(await request(mf,'/v1/checkpoints/lease',{method:'POST',body:{...cp.body,checkpoint_id:randomUUID()}}),409,'checkpoint_generation_changed');
});

test('concurrent identical finalization produces one generation and two matching ACKs',async t=>{
  const mf=runtime();t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);const cp=await acquire(mf);await ok(await put(mf,cp,0),201);
  const body=manifest(cp);const replies=await Promise.all([finalize(mf,cp,body),finalize(mf,cp,body)]);
  assert.deepEqual(replies.map(r=>r.status).sort(),[200,201]);const acks=await Promise.all(replies.map(r=>r.json()));
  assert.equal(acks[0].generation,1);assert.equal(acks[1].generation,1);assert.equal(acks[0].envelope_sha256,acks[1].envelope_sha256);
});

test('read lease pins an immutable replaced snapshot through GC, then expires',async t=>{
  const mf=runtime({HANNI_READ_LEASE_MS:'300',HANNI_GRACE_MS:'10'});t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);
  const first=await publish(mf);const lease=await readLease(mf,first);
  const same=await readLease(mf,first);assert.equal(same.read_lease_id,lease.read_lease_id);
  await ok(await append(mf,batch(2)),201);const second=await publish(mf,{base:2,generation:1});
  await sleep(20);await maintenance(mf);
  const oldPart=await ok(await download(mf,first,lease,0));assert.deepEqual(oldPart.envelope,first.parts[0]);
  await error(await download(mf,first,lease,0,'phone-a'),409,'read_lease_expired');
  await sleep(310);await maintenance(mf);
  await error(await download(mf,first,lease,0),409,'read_lease_expired');
  const latest=await ok(await request(mf,'/v1/checkpoints/latest'));assert.equal(latest.checkpoint_id,second.id);
  const latestLease=await readLease(mf,second);await ok(await download(mf,second,latestLease));
});

test('a full log still has checkpoint recovery reserve and can resume after GC',async t=>{
  const mf=runtime({HANNI_MAX_STORAGE_BYTES:'1000'});t.after(()=>mf.dispose());const one=batch(1);
  await ok(await append(mf,one),201);await error(await append(mf,batch(2)),507,'relay_capacity_reached');
  await publish(mf);await maintenance(mf);assert.equal((await ok(await append(mf,batch(2)),201)).seq,2);
});

test('quota and total reserve errors retain existing readable committed data',async t=>{
  const mf=runtime({HANNI_MAX_TOTAL_STORAGE_BYTES:'160000'});t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);
  const huge=envelope(60000);const body={checkpoint_id:randomUUID(),expected_generation:0,base_seq:1,chunk_count:1,total_bytes:Buffer.byteLength(JSON.stringify(huge))};
  await error(await request(mf,'/v1/checkpoints/lease',{method:'POST',body}),507,'relay_total_capacity_reached');
  assert.equal((await ok(await request(mf,'/v1/batches'))).batches.length,1);
  const small=runtime({HANNI_MAX_REQUESTS_PER_DAY:'2'});t.after(()=>small.dispose());await ok(await request(small,'/v1/device-state'));await ok(await request(small,'/v1/device-state'));
  const rejected=await request(small,'/v1/device-state');assert.ok(Number(rejected.headers.get('Retry-After'))>0);await error(rejected,429,'daily_request_limit');
});

test('checkpoint bytes quota rejects the part without publishing or losing prior log',async t=>{
  const mf=runtime({HANNI_MAX_CHECKPOINT_BYTES_PER_DAY:'1'});t.after(()=>mf.dispose());await ok(await append(mf,batch(1)),201);const cp=await acquire(mf);
  await error(await put(mf,cp,0),429,'daily_checkpoint_limit');await error(await finalize(mf,cp,manifest(cp)),409,'checkpoint_incomplete');
  assert.equal((await ok(await request(mf,'/v1/batches'))).next_cursor,1);
});

test('restart preserves staging, finalized manifest, cursor floor and last-ACK dedup',async()=>{
  const parent=fileURLToPath(new URL('../test-state/',import.meta.url));await mkdir(parent,{recursive:true});const disk=await mkdtemp(`${parent}restart-`);
  let mf=runtime({},disk);const sent=batch(1);await ok(await append(mf,sent),201);const cp=await acquire(mf);await ok(await put(mf,cp,0),201);await mf.dispose();
  mf=runtime({},disk);cp.manifest=manifest(cp);await ok(await finalize(mf,cp,cp.manifest),201);await mf.dispose();
  mf=runtime({},disk);try{
    await error(await request(mf,'/v1/batches?after=0'),409,'checkpoint_required');await maintenance(mf);
    assert.equal((await ok(await append(mf,sent))).duplicate,true);
    assert.equal((await ok(await finalize(mf,cp,cp.manifest))).duplicate,true);
    const lease=await readLease(mf,cp);assert.deepEqual((await ok(await download(mf,cp,lease,0))).envelope,cp.parts[0]);
  }finally{await mf.dispose();}
});

test('strict authorization/body/cursor validation covers the checkpoint endpoints',async t=>{
  const mf=runtime();t.after(()=>mf.dispose());
  await error(await request(mf,'/v1/checkpoints/lease',{method:'POST',body:{},headers:{Authorization:''}}),401,'unauthorized');
  await error(await request(mf,'/v1/device-state?token=x'),400,'invalid_query');
  await error(await append(mf,{...batch(1),plaintext:'forbidden'}),400,'invalid_batch');
  await error(await append(mf,batch(0)),400,'invalid_batch');
  await error(await request(mf,'/v1/batches?after=1'),409,'cursor_ahead');
  await ok(await append(mf,batch(1)),201);const cp=await acquire(mf);
  await error(await request(mf,`/v1/checkpoints/${cp.id}/chunks/0`,{method:'PUT',device:'mac',body:{lease_epoch:cp.lease.lease_epoch,envelope:cp.parts[0]}}),403,'checkpoint_owner_required');
  await error(await request(mf,`/v1/checkpoints/${cp.id}`),409,'read_lease_required');
});

test('GC is bounded, resumes after restart and never reuses the global sequence',async()=>{
  const parent=fileURLToPath(new URL('../test-state/',import.meta.url));await mkdir(parent,{recursive:true});const disk=await mkdtemp(`${parent}gc-`);
  let mf=runtime({},disk);for(let n=1;n<=205;n++)await ok(await append(mf,batch(n)),201);
  await publish(mf,{base:205});assert.equal((await maintenance(mf)).removed_rows,100);await mf.dispose();
  mf=runtime({},disk);try{
    assert.equal((await maintenance(mf)).removed_rows,100);assert.equal((await maintenance(mf)).removed_rows,5);
    assert.equal((await ok(await append(mf,batch(206)),201)).seq,206);
    const page=await ok(await request(mf,'/v1/batches?after=205'));assert.equal(page.batches.length,1);assert.equal(page.next_cursor,206);
  }finally{await mf.dispose();}
});

test('v1 SQLite schema fails closed and is never silently migrated or erased',async()=>{
  const parent=fileURLToPath(new URL('../test-state/',import.meta.url));await mkdir(parent,{recursive:true});const disk=await mkdtemp(`${parent}legacy-`);
  const legacy=`import {DurableObject} from 'cloudflare:workers';
    export class Relay extends DurableObject {
      constructor(ctx,env){super(ctx,env);this.sql=ctx.storage.sql;this.sql.exec('CREATE TABLE IF NOT EXISTS batches(seq INTEGER PRIMARY KEY,sentinel TEXT)');this.sql.exec("INSERT OR IGNORE INTO batches VALUES(1,'synthetic-preserved')");}
      fetch(){return Response.json({count:this.sql.exec('SELECT COUNT(*) AS n FROM batches').one().n});}
    }
    export default {fetch(request,env){return env.RELAY.get(env.RELAY.idFromName('hanni-personal-relay-v2')).fetch(request);}};`;
  let mf=runtime({},disk,legacy);assert.equal((await ok(await request(mf,'/v1/device-state'))).count,1);await mf.dispose();
  mf=runtime({},disk);await error(await request(mf,'/v1/device-state'),503,'protocol_migration_required');await mf.dispose();
  mf=runtime({},disk,legacy);try{assert.equal((await ok(await request(mf,'/v1/device-state'))).count,1);}finally{await mf.dispose();}
});

test('page byte bounds remain valid for large encrypted batches',async t=>{
  const mf=runtime();t.after(()=>mf.dispose());for(let n=1;n<=9;n++)await ok(await append(mf,batch(n,60000)),201);
  const response=await request(mf,'/v1/batches?after=0&limit=32');const text=await response.text();assert.equal(response.status,200);assert.ok(Buffer.byteLength(text)<512*1024);
  const page=JSON.parse(text);assert.ok(page.batches.length<9);assert.equal(page.next_cursor,page.batches.at(-1).seq);assert.equal(page.has_more,true);
  const tail=await ok(await request(mf,`/v1/batches?after=${page.next_cursor}&limit=32`));assert.equal(tail.next_cursor,9);assert.equal(tail.has_more,false);
});

test('native authenticated WebSocket hints follow commits and fixed ping remains supported',async t=>{
  const mf=runtime();t.after(()=>mf.dispose());
  const response=await request(mf,'/v1/stream',{headers:{Upgrade:'websocket'}});assert.equal(response.status,101);
  const socket=response.webSocket;const messages=[];const waiters=[];
  socket.addEventListener('message',event=>{if(waiters.length)waiters.shift()(event.data);else messages.push(event.data);});socket.accept();
  const next=()=>messages.length?Promise.resolve(messages.shift()):new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error('websocket timeout')),2000);waiters.push(value=>{clearTimeout(timer);resolve(value);});});
  assert.equal(JSON.parse(await next()).type,'ready');socket.send('ping');assert.equal(await next(),'pong');
  await ok(await append(mf,batch(1)),201);assert.equal(JSON.parse(await next()).latest_seq,1);
  const cp=await publish(mf);assert.equal(JSON.parse(await next()).latest_seq,1);
  const recovery=await error(await request(mf,'/v1/batches?after=0'),409,'checkpoint_required');assert.equal(recovery.checkpoint.checkpoint_id,cp.id);socket.close(1000,'');
});

test('no-op maintenance does not burn the daily write budget; multiple checkpoint generations reclaim space',async t=>{
  const probe=source+`\nexport class ProbeRelay extends Relay {async fetch(request){
    if(new URL(request.url).pathname==='/__probe')return Response.json({meta:this.state(),bytes:this.sql.databaseSize});
    return super.fetch(request);
  }}`;
  const mf=runtime({HANNI_GRACE_MS:'1',HANNI_MAX_TOTAL_STORAGE_BYTES:'600000'},undefined,probe,'ProbeRelay');t.after(()=>mf.dispose());
  const ns=await mf.getDurableObjectNamespace('RELAY');const stub=ns.get(ns.idFromName('hanni-personal-relay-v2'));
  const inspect=async()=>{const r=await stub.fetch('https://relay.test/__probe');return r.json();};
  const before=await inspect();for(let i=0;i<20;i++)await maintenance(mf);const after=await inspect();assert.equal(after.meta.daily_write_units-before.meta.daily_write_units,40);
  for(let n=1;n<=8;n++){
    await ok(await append(mf,batch(n)),201);await publish(mf,{base:n,generation:n-1,parts:[envelope(60000)]});await sleep(3);await maintenance(mf);
  }
  const end=await inspect();assert.equal(end.meta.generation,8);assert.ok(end.meta.log_bytes>=0);assert.ok(end.meta.retained_count>=0);
});

test('durable alarms resume prefix GC after restart without client maintenance',async()=>{
  const probe=source+`\nexport class AlarmProbeRelay extends Relay {async fetch(request){
    if(new URL(request.url).pathname==='/__probe')return Response.json({meta:this.state(),alarm:await this.ctx.storage.getAlarm()});
    return super.fetch(request);
  }}`;
  const parent=fileURLToPath(new URL('../test-state/',import.meta.url));await mkdir(parent,{recursive:true});const disk=await mkdtemp(`${parent}alarm-`);
  let mf=runtime({},disk,probe,'AlarmProbeRelay');for(let n=1;n<=105;n++)await ok(await append(mf,batch(n)),201);
  const cp=await publish(mf,{base:105});await mf.dispose();
  mf=runtime({},disk,probe,'AlarmProbeRelay');try{
    const ns=await mf.getDurableObjectNamespace('RELAY');const stub=ns.get(ns.idFromName('hanni-personal-relay-v2'));
    const inspect=async()=>ok(await stub.fetch('https://relay.test/__probe'));
    const before=await inspect();assert.ok(before.alarm!==null);
    const deadline=Date.now()+8000;let current=before;
    while(current.meta.retained_count && Date.now()<deadline){await sleep(100);current=await inspect();}
    assert.equal(current.meta.retained_count,0);assert.equal(current.meta.latest_seq,105);assert.equal(current.meta.daily_gc_rows,105);
    assert.equal(current.alarm,null);assert.equal((await ok(await request(mf,'/v1/checkpoints/latest'))).checkpoint_id,cp.id);
  }finally{await mf.dispose();}
});

test('an abandoned upload expires and is collected by its scheduled alarm',async t=>{
  const probe=source+`\nexport class StageProbeRelay extends Relay {async fetch(request){
    if(new URL(request.url).pathname==='/__probe')return Response.json({checkpoints:this.sql.exec('SELECT COUNT(*) AS n FROM checkpoints').one().n,chunks:this.sql.exec('SELECT COUNT(*) AS n FROM checkpoint_chunks').one().n,meta:this.state()});
    return super.fetch(request);
  }}`;
  const mf=runtime({HANNI_STAGING_MS:'100'},undefined,probe,'StageProbeRelay');t.after(()=>mf.dispose());
  await ok(await append(mf,batch(1)),201);const cp=await acquire(mf);await ok(await put(mf,cp,0),201);
  const ns=await mf.getDurableObjectNamespace('RELAY');const stub=ns.get(ns.idFromName('hanni-personal-relay-v2'));
  const inspect=async()=>ok(await stub.fetch('https://relay.test/__probe'));
  const deadline=Date.now()+8000;let state=await inspect();
  while(state.checkpoints && Date.now()<deadline){await sleep(100);state=await inspect();}
  assert.equal(state.checkpoints,0);assert.equal(state.chunks,0);assert.equal(state.meta.compacted_through,0);assert.equal(state.meta.retained_count,1);
  await error(await finalize(mf,cp,manifest(cp)),404,'checkpoint_missing');
  assert.equal((await ok(await request(mf,'/v1/batches'))).batches.length,1);
});
