// ── js/calendar-routine.js — Routine graph editor (Calendar → Рутина) ──
// The tab is a pure constructor: a toolbar (chain chips + add) and a graph
// canvas. The "now" player (running a routine) lives in the "+" widget.
import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { renderCanvas } from './routine-canvas.js';
import { openAddTaskModal } from './routine-add-modal.js';

let chains = [];

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
        <span class="rt-toolbar-info" tabindex="0"
          title="Тяни узлы, чтобы расставить · тяни от круглого порта справа — рисуешь стрелку · клик по стрелке — сменить тип или удалить">ⓘ подсказка</span>
      </div>
      <div class="rt-canvas-wrap">
        <div class="rt-canvas" id="rt-canvas"><svg class="rt-edges" id="rt-edges"></svg></div>
      </div>
    </div>`;
  el.querySelector('#rt-add-task').addEventListener('click', () => {
    openAddTaskModal(S._routineChainId, () => draw(el));
  });
  el.querySelector('#rt-add-chain').addEventListener('click', () => addChain(el));
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
  const visible = chains.filter(c => !isMealChain(c));
  if (!S._routineChainId || !visible.some(c => c.id === S._routineChainId)) {
    S._routineChainId = visible[0]?.id || null;
  }
}

// Create a new chain (with its start node) and switch to it.
async function addChain(el) {
  const title = (prompt('Название цепочки') || '').trim();
  if (!title) return;
  const id = await invoke('create_routine_chain', { title }).catch(() => null);
  if (!id) return;
  S._routineChainId = id;
  await renderCalendarRoutine(el);
}

function chain() { return chains.find(c => c.id === S._routineChainId); }

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
  });
}
