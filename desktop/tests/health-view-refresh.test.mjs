import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
const source = await readFile(new URL('../src/js/health-view-refresh.js', import.meta.url), 'utf8');
let serial = 0;
const moduleUrl = text => 'data:text/javascript;base64,' + Buffer.from(text).toString('base64');
const settle = () => new Promise(resolve => setTimeout(resolve, 20));

async function setup({ mobile = false, status = () => ({ projection: { projection_revision: '0' } }) } = {}) {
  const dom = new JSDOM('<div id="view-calendar" class="view active"><div id="calendar-inner-content">before</div></div><div id="view-health" class="view"><main><div class="sleep-list"></div></main></div>', { url:'https://fixture.invalid', pretendToBeVisual:true });
  const w = dom.window, doc = w.document;
  let visibility = 'visible', tick, calls = [], listener, events = 0;
  const state = { activeTab:'calendar',activeSubTab:{},_unifiedPane:{health:'sleep'} };
  Object.defineProperty(doc,'visibilityState',{get:()=>visibility});
  w.setInterval = fn => { tick = fn; return 1; };
  globalThis.window=w; globalThis.document=doc; globalThis.CustomEvent=w.CustomEvent; globalThis.MutationObserver=w.MutationObserver;
  globalThis.localStorage=w.localStorage;
  globalThis.healthUiFixture = { S:state, IS_MOBILE:mobile, listen:async(name,fn)=>{assert.equal(name,'cloud-relay-updated');listener=fn;return()=>{};},invoke:async cmd=>{calls.push(cmd);return status();} };
  const actual = source.replace("import { S, listen, invoke, IS_MOBILE } from './state.js';",'const { S, listen, invoke, IS_MOBILE } = globalThis.healthUiFixture;');
  const url = moduleUrl(actual+'\n// instance '+(++serial)), ui=await import(url);
  w.addEventListener('hanni:calendar-refresh',()=>events++);
  return {dom,w,doc,state,ui,url,calls, get events(){return events;}, emit:payload=>listener({payload}), tick:()=>tick?.(),hide:value=>{visibility=value?'hidden':'visible';doc.dispatchEvent(new w.Event('visibilitychange'));}, target:doc.querySelector('#calendar-inner-content')};
}

test('idle desktop relay ticks do not redraw; real invalidations coalesce', async()=>{
  const x=await setup(); try {
    x.ui.startHealthViewRefresh();x.emit({ok:true});x.emit({ok:false});await settle();assert.equal(x.events,0);
    x.emit({ok:true,views_changed:true});x.emit({views_changed:true});await settle();assert.equal(x.events,1);assert.deepEqual(x.calls,[]);
  } finally{x.dom.window.close();}
});

test('hidden pane, editor and pointer defer one update until the interaction ends',async()=>{
  const x=await setup();try{
    x.ui.startHealthViewRefresh();x.hide(true);x.emit({views_changed:true});await settle();assert.equal(x.events,0);
    const editor=x.doc.createElement('dialog');editor.setAttribute('open','');editor.innerHTML='<input value="draft">';x.doc.body.append(editor);
    x.hide(false);editor.querySelector('input').focus();await settle();assert.equal(x.events,0);
    editor.remove();x.doc.dispatchEvent(new x.w.Event('pointerdown'));await settle();assert.equal(x.events,0);
    x.doc.dispatchEvent(new x.w.Event('pointerup'));await settle();assert.equal(x.events,1);
  }finally{x.dom.window.close();}
});

test('read completed after editing began cannot replace DOM; superseded reads cannot commit',async()=>{
  const x=await setup();try{
    x.ui.startHealthViewRefresh();const old=x.ui.beginHealthViewRead(x.target);const latest=x.ui.beginHealthViewRead(x.target);
    assert.equal(old(),false);assert.equal(latest(),true);
    assert.equal(x.ui.mayCommitHealthView(x.target,'rows-v1'),true);
    assert.equal(x.ui.mayCommitHealthView(x.target,'rows-v1',true),false);
    const input=x.doc.createElement('input');x.target.append(input);input.focus();
    assert.equal(x.ui.mayCommitHealthView(x.target,'rows-v2',true),false);assert.ok(x.target.contains(input));
    input.remove();await settle();assert.equal(x.events,1);
  }finally{x.dom.window.close();}
});

test('Android polls only visible unedited health panes and compares decimal revisions without rounding',async()=>{
  let rev='9007199254740992';const x=await setup({mobile:true,status:()=>({projection:{projection_revision:rev},last_ok:Math.random()})});try{
    x.ui.startHealthViewRefresh();await settle();assert.equal(x.events,1);await x.tick();await settle();assert.equal(x.events,1);
    rev='9007199254740993';await x.tick();await settle();assert.equal(x.events,2);
    x.hide(true);await x.tick();assert.equal(x.calls.length,3);
    x.state.activeTab='notes';x.hide(false);await x.tick();assert.equal(x.calls.length,3);
    assert.ok(x.calls.every(cmd=>cmd==='cloud_relay_status'));
  }finally{x.dom.window.close();}
});

test('Sleep receives quiet refresh; another health pane is left untouched',async()=>{
  const x=await setup();try{
    x.state.activeTab='health';x.doc.querySelector('#view-calendar').classList.remove('active');x.doc.querySelector('#view-health').classList.add('active');
    let refreshed=0;const pane=x.doc.querySelector('main');x.ui.registerSleepHealthView(pane,()=>refreshed++);x.ui.startHealthViewRefresh();
    x.emit({views_changed:true});await settle();assert.equal(refreshed,1);assert.equal(x.events,0);
    x.state._unifiedPane.health='body';x.emit({views_changed:true});await settle();assert.equal(refreshed,1);
  }finally{x.dom.window.close();}
});

test('actual Sleep renderer preserves unchanged nodes and never starts HC on a quiet refresh',async()=>{
  const x=await setup();try{
    x.state.activeTab='health';x.doc.querySelector('#view-calendar').classList.remove('active');x.doc.querySelector('#view-health').classList.add('active');
    let records=[],imports=0;
    globalThis.sleepFixture={invoke:async cmd=>cmd==='get_sleep_sessions'?records:cmd==='get_sleep_stats'?{avg_duration_minutes:0,total_sessions:records.length}:true, IS_MOBILE:true,escapeHtml:String,autoImportHealth:async()=>{imports++;return true;}};
    let text=await readFile(new URL('../src/js/tab-sleep.js',import.meta.url),'utf8');
    text=text.replace("import { invoke, IS_MOBILE } from './state.js';",'const {invoke,IS_MOBILE}=globalThis.sleepFixture;')
      .replace("import { escapeHtml } from './utils.js';",'const {escapeHtml}=globalThis.sleepFixture;')
      .replace("import { autoImportHealth } from './health-auto-sync.js';",'const {autoImportHealth}=globalThis.sleepFixture;')
      .replace("'./health-view-refresh.js'",JSON.stringify(x.url));
    const {renderSleepPane}=await import(moduleUrl(text));const pane=x.doc.querySelector('main');
    await renderSleepPane(pane,{autoImport:false});const button=pane.querySelector('button');
    await renderSleepPane(pane,{quiet:true,autoImport:false});assert.equal(pane.querySelector('button'),button);
    records=[{date:'2026-09-05',start_time:'01:00',end_time:'09:00',duration_minutes:480,source:'health_connect_raw:synthetic',stages:[]}];
    await renderSleepPane(pane,{quiet:true,autoImport:false});assert.equal(pane.querySelectorAll('.sleep-session').length,1);assert.equal(imports,0);
  }finally{x.dom.window.close();}
});

test('actual raw import notifies projected changes even when no HC record was modified',async()=>{
  const x=await setup();try{
    x.ui.startHealthViewRefresh();globalThis.rawUiFixture={invoke:async()=>({modified_records:0,projection:{records:2},more_pending:false}),IS_MOBILE:true,localDate:()=> '2026-09-05'};
    let text=await readFile(new URL('../src/js/health-auto-sync.js',import.meta.url),'utf8');
    text=text.replace("import { invoke, IS_MOBILE } from './state.js';",'const {invoke,IS_MOBILE}=globalThis.rawUiFixture;')
      .replace("import { localDate } from './utils.js';",'const {localDate}=globalThis.rawUiFixture;')
      .replace("'./health-view-refresh.js'",JSON.stringify(x.url));
    const mod=await import(moduleUrl(text+'\nexport {importRawHealth};'));
    assert.equal(await mod.importRawHealth(),true);await settle();assert.equal(x.events,1);
  }finally{x.dom.window.close();}
});

test('quiet Sleep DB failure keeps previous rows and coalesces one delayed retry',async()=>{
  const x=await setup();try{
    x.ui.startHealthViewRefresh();x.state.activeTab='health';x.doc.querySelector('#view-calendar').classList.remove('active');x.doc.querySelector('#view-health').classList.add('active');
    let failing=false;const timers=[];const realTimeout=x.w.setTimeout.bind(x.w);
    x.w.setTimeout=(fn,delay)=>delay===15000?(timers.push(fn),timers.length):realTimeout(fn,delay);
    globalThis.sleepFixture={invoke:async cmd=>{
      if(failing)throw Error('synthetic DB unavailable');
      return cmd==='get_sleep_sessions'?[{date:'2026-09-05',start_time:'01:00',end_time:'09:00',duration_minutes:480,source:'health_connect_raw:synthetic',stages:[]}]:{avg_duration_minutes:480,total_sessions:1};
    },IS_MOBILE:false,escapeHtml:String,autoImportHealth:async()=>false};
    let text=await readFile(new URL('../src/js/tab-sleep.js',import.meta.url),'utf8');
    text=text.replace("import { invoke, IS_MOBILE } from './state.js';",'const {invoke,IS_MOBILE}=globalThis.sleepFixture;')
      .replace("import { escapeHtml } from './utils.js';",'const {escapeHtml}=globalThis.sleepFixture;')
      .replace("import { autoImportHealth } from './health-auto-sync.js';",'const {autoImportHealth}=globalThis.sleepFixture;')
      .replace("'./health-view-refresh.js'",JSON.stringify(x.url));
    const {renderSleepPane}=await import(moduleUrl(text));const pane=x.doc.querySelector('main');
    await renderSleepPane(pane,{autoImport:false});await settle();const row=pane.querySelector('.sleep-session');
    failing=true;await renderSleepPane(pane,{quiet:true,autoImport:false});await renderSleepPane(pane,{quiet:true,autoImport:false});
    assert.equal(pane.querySelector('.sleep-session'),row);assert.equal(timers.length,1);
    failing=false;timers[0]();await settle();assert.equal(pane.querySelector('.sleep-session'),row);
  }finally{x.dom.window.close();}
});

test('actual Calendar period renderer rejects quiet failed reads before replacing existing records',async()=>{
  const x=await setup();try{
    globalThis.periodFixture={S:x.state,invoke:async()=>{throw Error('synthetic DB unavailable');},tabLoaders:{},escapeHtml:String};
    let text=await readFile(new URL('../src/js/calendar-task-list-period.js',import.meta.url),'utf8');
    text=text.replace("import { S, invoke, tabLoaders } from './state.js';",'const {S,invoke,tabLoaders}=globalThis.periodFixture;')
      .replace("import { escapeHtml } from './utils.js';",'const {escapeHtml}=globalThis.periodFixture;')
      .replace("'./health-view-refresh.js'",JSON.stringify(x.url));
    const {renderPeriodMode}=await import(moduleUrl(text));const row=x.target.firstChild;
    await assert.rejects(renderPeriodMode(x.target,{start:'2026-09-01',end:'2026-09-07',view:'week',quiet:true}),/synthetic DB unavailable/);
    assert.equal(x.target.firstChild,row);assert.equal(x.target.textContent,'before');
  }finally{x.dom.window.close();}
});
