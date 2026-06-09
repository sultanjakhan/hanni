// ── js/calendar-routine.js — Routine graph editor (Calendar → Рутина) ──
// The tab is a pure constructor: a toolbar (chain chips + add) and a graph
// canvas. The "now" player (running a routine) lives in the "+" widget.
import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { renderCanvas } from './routine-canvas.js';
import { openAddTaskModal } from './routine-add-modal.js';
import { promptModal } from './prompt-modal.js';

let chains = [];
let schedById = {};   // schedule id → row; gives canvas nodes their step type

// Multi-time "meal" chains (e.g. Еда: breakfast/lunch/dinner) are played per-slot
// in the "+" widget as Завтрак/Обед/Ужин — they don't get a single chip here.
const isMealChain = (c) => c.trigger_type === 'time' &&
  String(c.trigger_time || '').split(',').map(s => s.trim()).filter(Boolean).length > 1;
const TRIGGER_ICON = { sleep_end: '☀️', time: '🕐', manual: '▶️' };

export async function renderCalendarRoutine(el) {
  await loadChains();
  if (chains.length === 0) {
    el.innerHTML = '<div class="rt-empty">Нет цепочек рутины<br><button class="rt-btn-add" id="rt-add-chain-empty">＋ Цепочка</button></div>';
    el.querySelector('#rt-add-chain-empty').addEventListener('click', () => addChain(el));
    return;
  }
  el.innerHTML = `
    <div class="rt-main">
      <div class="rt-toolbar">
        <div class="rt-chains-bar" id="rt-chains"></div>
        <button class="rt-btn-add" id="rt-add-chain">＋ Цепочка</button>
        <span class="rt-toolbar-sep"></span>
        <button class="rt-btn-add" id="rt-add-task">＋ Задача</button>
        <span class="rt-toolbar-info" tabindex="0" role="button"
          title="Жесты и типы связей">ⓘ подсказка</span>
      </div>
      <div class="rt-canvas-wrap">
        <div class="rt-canvas" id="rt-canvas"><svg class="rt-edges" id="rt-edges"></svg></div>
      </div>
    </div>`;
  el.querySelector('#rt-add-task').addEventListener('click', () => {
    // Reload before drawing — `chains` is a cache and doesn't have the new node.
    openAddTaskModal(S._routineChainId, async () => { await loadChains(); draw(el); }, freeSpawnPos());
  });
  el.querySelector('#rt-add-chain').addEventListener('click', () => addChain(el));
  const info = el.querySelector('.rt-toolbar-info');
  info.addEventListener('click', () => toggleLegend(el));
  info.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleLegend(el); }
  });
  // Mouse wheel scrolls the wide graph horizontally.
  const wrap = el.querySelector('.rt-canvas-wrap');
  wrap.addEventListener('wheel', (e) => {
    if (e.deltaY !== 0 && !e.shiftKey) { wrap.scrollLeft += e.deltaY; e.preventDefault(); }
  }, { passive: false });
  renderChainChips(el);
  draw(el);
}

async function loadChains() {
  chains = await invoke('get_routine_chains').catch(() => []);
  const scheds = await invoke('get_schedules', { category: null }).catch(() => []);
  schedById = Object.fromEntries(scheds.map(s => [String(s.id), s]));
  const visible = chains.filter(c => !isMealChain(c));
  if (!S._routineChainId || !visible.some(c => c.id === S._routineChainId)) {
    S._routineChainId = visible[0]?.id || null;
  }
}

// Legend popover under the ⓘ — edge types and gestures. The old title-only
// tooltip was unreachable on touch and unexplained the wire colors entirely.
function toggleLegend(el) {
  const ex = el.querySelector('.rt-legend');
  if (ex) { ex.remove(); return; }
  const lg = document.createElement('div');
  lg.className = 'rt-legend';
  lg.innerHTML = `
    <div class="rt-leg-title">Связи</div>
    <div class="rt-leg-row"><span class="rt-leg-line"></span>после завершения</div>
    <div class="rt-leg-row"><span class="rt-leg-line rt-leg-line--dur"></span>через N минут</div>
    <div class="rt-leg-row"><span class="rt-leg-line rt-leg-line--man"></span>вручную</div>
    <div class="rt-leg-title">Шаги</div>
    <div class="rt-leg-row"><span class="rt-leg-ic tr">▶</span>таймер</div>
    <div class="rt-leg-row"><span class="rt-leg-ic ck">✓</span>быстрая отметка</div>
    <div class="rt-leg-row"><span class="rt-leg-ic">📓</span>дневник</div>
    <div class="rt-leg-hint">Тяни узел — расставить · клик по узлу — карточка ·
      тяни от ○ порта — связь · клик по связи — тип / удалить</div>`;
  el.querySelector('.rt-toolbar').appendChild(lg);
  setTimeout(() => document.addEventListener('pointerdown', function close(e) {
    if (!e.target.closest('.rt-legend, .rt-toolbar-info')) {
      lg.remove();
      document.removeEventListener('pointerdown', close);
    }
  }), 0);
}

// Create a new chain (with its start node) and switch to it.
async function addChain(el) {
  const title = ((await promptModal({ title: 'Название цепочки', placeholder: 'Утро' })) || '').trim();
  if (!title) return;
  const id = await invoke('create_routine_chain', { title }).catch(() => null);
  if (!id) return;
  S._routineChainId = id;
  await renderCalendarRoutine(el);
}

function chain() { return chains.find(c => c.id === S._routineChainId); }

// First spot on a diagonal cascade not already occupied by a node — new nodes
// used to all land on (60,60) and stack into an unreadable pile.
function freeSpawnPos() {
  const nodes = chain()?.nodes || [];
  let x = 60, y = 60;
  while (nodes.some(n => Math.abs(n.pos_x - x) < 40 && Math.abs(n.pos_y - y) < 40)) {
    x += 36; y += 36;
    if (y > 600) { x = x - 540 + 36; y = 60; } // wrap before the canvas bottom
  }
  return { x, y };
}

function renderChainChips(el) {
  const box = el.querySelector('#rt-chains');
  box.innerHTML = chains.filter(c => !isMealChain(c)).map(c =>
    `<button class="rt-chain-chip${c.id === S._routineChainId ? ' active' : ''}" data-cid="${c.id}">
       ${TRIGGER_ICON[c.trigger_type] || '▶️'} ${escapeHtml(c.title)}
     </button>`).join('');
  box.querySelectorAll('[data-cid]').forEach(btn => {
    btn.addEventListener('click', () => {
      S._routineChainId = parseInt(btn.dataset.cid);
      renderChainChips(el);
      draw(el);
    });
  });
}

// Render the active chain's graph; reload data + redraw after structural edits.
function draw(el) {
  const c = chain();
  if (!c) return;
  renderCanvas(el.querySelector('#rt-canvas'), c, async () => {
    await loadChains();
    draw(el);
  }, schedById);
}
