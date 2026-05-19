// ── js/calendar-routine.js — Routine graph editor (Calendar → Рутина) ──
// The tab is a pure constructor: a toolbar (chain chips + add) and a graph
// canvas. The "now" player (running a routine) lives in the "+" widget.
import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { renderCanvas } from './routine-canvas.js';
import { openAddTaskModal } from './routine-add-modal.js';

let chains = [];

export async function renderCalendarRoutine(el) {
  await loadChains();
  if (chains.length === 0) {
    el.innerHTML = '<div class="rt-empty">Нет цепочек рутины</div>';
    return;
  }
  el.innerHTML = `
    <div class="rt-main">
      <div class="rt-toolbar">
        <div class="rt-chains-bar" id="rt-chains"></div>
        <button class="btn-secondary" id="rt-add-task">+ Задача</button>
        <span class="rt-toolbar-hint">Тяни узлы · от порта рисуй стрелку · клик по стрелке — тип/удаление</span>
      </div>
      <div class="rt-canvas-wrap">
        <div class="rt-canvas" id="rt-canvas"><svg class="rt-edges" id="rt-edges"></svg></div>
      </div>
    </div>`;
  el.querySelector('#rt-add-task').addEventListener('click', () => {
    openAddTaskModal(S._routineChainId, () => draw(el));
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
  if (!S._routineChainId || !chains.some(c => c.id === S._routineChainId)) {
    S._routineChainId = chains[0]?.id || null;
  }
}

function chain() { return chains.find(c => c.id === S._routineChainId); }

function renderChainChips(el) {
  const box = el.querySelector('#rt-chains');
  box.innerHTML = chains.map(c =>
    `<button class="rt-chain-chip${c.id === S._routineChainId ? ' active' : ''}" data-cid="${c.id}">
       ◆ ${escapeHtml(c.title)}
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
  });
}
