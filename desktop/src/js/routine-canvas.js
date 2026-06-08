// ── js/routine-canvas.js — Routine graph canvas: nodes, edges, interaction ──
// renderCanvas() draws a chain and wires drag, edge drawing, edit and delete.
// `refresh` is an async callback that reloads data and redraws after edits.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { openNodeModal } from './routine-node-modal.js';
import { openEdgeMenu } from './routine-edge-menu.js';

const CAT_ICONS = {
  health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡',
  practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽',
};

const TRIGGER_SUB = {
  sleep_end: 'срабатывает по концу сна — данные с часов',
  time: 'срабатывает в заданное время',
  manual: 'запускается вручную',
};
const TRIGGER_ICON = { sleep_end: '☀️', time: '🕐', manual: '▶️' };

export function renderCanvas(canvas, chain, refresh) {
  canvas.querySelectorAll('.rt-node, .rt-edge-menu').forEach(n => n.remove());
  for (const n of chain.nodes) canvas.appendChild(buildNode(n, chain));
  wireNodes(canvas, chain, refresh);
  renderEdges(canvas, chain, refresh, null);
  // On mobile, shrink the canvas to its node bounds. The fixed 1000×680 size
  // (≈3000×2040 at 3x DPI) is one big composited layer that blows the WebView
  // tile-memory budget and janks; desktop keeps the roomy fixed editing area.
  if (document.documentElement.classList.contains('mobile')) {
    let w = 0, h = 0;
    canvas.querySelectorAll('.rt-node').forEach(el => {
      w = Math.max(w, el.offsetLeft + el.offsetWidth);
      h = Math.max(h, el.offsetTop + el.offsetHeight);
    });
    canvas.style.width = (w + 60) + 'px';
    canvas.style.height = (h + 60) + 'px';
  }
}

function buildNode(n, chain) {
  const d = document.createElement('div');
  d.className = 'rt-node' + (n.is_start ? ' rt-start' : '');
  d.style.left = n.pos_x + 'px';
  d.style.top = n.pos_y + 'px';
  d.dataset.id = n.id;
  if (n.is_start) {
    let sub = TRIGGER_SUB[chain.trigger_type] || 'триггер запуска';
    if (chain.trigger_type === 'time' && chain.trigger_time) {
      const ts = String(chain.trigger_time).split(',').map(s => s.trim()).filter(Boolean).join(', ');
      sub = `срабатывает в ${escapeHtml(ts)}`;
    }
    d.innerHTML = `<span class="rt-node-icon">${TRIGGER_ICON[chain.trigger_type] || '⏰'}</span>
      <div class="rt-start-text">
        <span class="rt-node-title">${escapeHtml(n.title)}</span>
        <span class="rt-start-sub">${sub}</span>
      </div>
      <div class="rt-port out" data-port="${n.id}"></div>`;
  } else {
    // Required is the default → unlabelled; only optional carries a chip (+ dim).
    const isOpt = n.requirement === 'optional';
    if (isOpt) d.className += ' rt-opt';
    d.dataset.cat = n.category || 'other';
    const meta = isOpt
      ? `<div class="rt-node-meta"><span class="rt-badge optional" data-req="${n.id}"
           title="Опциональная: можно пропустить · клик — сделать обязательной">опц.</span></div>`
      : '';
    d.innerHTML = `
      <div class="rt-node-del" data-del="${n.id}" title="Удалить узел">✕</div>
      <div class="rt-node-top">
        <span class="rt-node-icon">${CAT_ICONS[n.category] || CAT_ICONS.other}</span>
        <span class="rt-node-title">${escapeHtml(n.title)}</span>
      </div>
      ${meta}
      <div class="rt-port in"></div>
      <div class="rt-port out" data-port="${n.id}"></div>`;
  }
  return d;
}

function wireNodes(canvas, chain, refresh) {
  const find = id => chain.nodes.find(n => n.id === id);
  canvas.querySelectorAll('.rt-node').forEach(el => {
    const n = find(parseInt(el.dataset.id));
    el.addEventListener('mousedown', (e) => {
      if (e.target.closest('[data-port],[data-del],[data-req]')) return;
      startDrag(canvas, chain, refresh, el, n, e);
    });
  });
  canvas.querySelectorAll('[data-port]').forEach(p =>
    p.addEventListener('mousedown', (e) => {
      e.stopPropagation();
      startEdge(canvas, chain, refresh, parseInt(p.dataset.port));
    }));
  canvas.querySelectorAll('[data-del]').forEach(b =>
    b.addEventListener('click', async (e) => {
      e.stopPropagation();
      await invoke('delete_routine_node', { id: parseInt(b.dataset.del) }).catch(() => {});
      refresh();
    }));
  canvas.querySelectorAll('[data-req]').forEach(b =>
    b.addEventListener('click', async (e) => {
      e.stopPropagation();
      const n = find(parseInt(b.dataset.req));
      const v = n.requirement === 'required' ? 'optional' : 'required';
      await invoke('update_routine_node', { id: n.id, requirement: v }).catch(() => {});
      refresh();
    }));
}

// ── drag a node (persist on drop) or, if it didn't move, open its detail modal ──
function startDrag(canvas, chain, refresh, el, n, e) {
  const sx = e.clientX, sy = e.clientY, ox = n.pos_x, oy = n.pos_y;
  let moved = false;
  const move = (ev) => {
    if (Math.abs(ev.clientX - sx) > 3 || Math.abs(ev.clientY - sy) > 3) moved = true;
    n.pos_x = Math.max(0, ox + ev.clientX - sx);
    n.pos_y = Math.max(0, oy + ev.clientY - sy);
    el.style.left = n.pos_x + 'px';
    el.style.top = n.pos_y + 'px';
    renderEdges(canvas, chain, refresh, null);
  };
  const up = async () => {
    document.removeEventListener('mousemove', move);
    document.removeEventListener('mouseup', up);
    if (!moved) {                                  // a click, not a drag
      openNodeModal(n, chain, refresh);
      return;
    }
    await invoke('update_routine_node', { id: n.id, posX: n.pos_x, posY: n.pos_y }).catch(() => {});
  };
  document.addEventListener('mousemove', move);
  document.addEventListener('mouseup', up);
}

// ── draw an edge from an out-port to a target node ──
function startEdge(canvas, chain, refresh, fromId) {
  const rect = () => canvas.getBoundingClientRect();
  let dx = 0, dy = 0;
  const move = (ev) => {
    dx = ev.clientX - rect().left;
    dy = ev.clientY - rect().top;
    canvas.querySelectorAll('.rt-node').forEach(x => x.classList.remove('rt-droptarget'));
    const t = document.elementFromPoint(ev.clientX, ev.clientY)?.closest('.rt-node');
    if (t && parseInt(t.dataset.id) !== fromId && !t.classList.contains('rt-start')) {
      t.classList.add('rt-droptarget');
    }
    renderEdges(canvas, chain, refresh, { fromId, x: dx, y: dy });
  };
  const up = async (ev) => {
    document.removeEventListener('mousemove', move);
    document.removeEventListener('mouseup', up);
    canvas.querySelectorAll('.rt-node').forEach(x => x.classList.remove('rt-droptarget'));
    const t = document.elementFromPoint(ev.clientX, ev.clientY)?.closest('.rt-node');
    const toId = t ? parseInt(t.dataset.id) : null;
    if (toId && toId !== fromId && !t.classList.contains('rt-start')
        && !chain.edges.some(e => e.from_node_id === fromId && e.to_node_id === toId)) {
      await invoke('create_routine_edge', {
        chainId: chain.id, fromNodeId: fromId, toNodeId: toId,
      }).catch(() => {});
    }
    refresh();
  };
  document.addEventListener('mousemove', move);
  document.addEventListener('mouseup', up);
}

function nodeBox(canvas, id) {
  const e = canvas.querySelector(`.rt-node[data-id="${id}"]`);
  return e ? { x: e.offsetLeft, y: e.offsetTop, w: e.offsetWidth, h: e.offsetHeight } : null;
}

function edgeColor(t) {
  if (t === 'after_duration') return 'var(--accent-orange, #c98b3c)';
  if (t === 'manual') return 'var(--accent-purple)';
  return 'var(--text-secondary)';
}

function svgPath(d) {
  const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
  p.setAttribute('d', d);
  return p;
}

function renderEdges(canvas, chain, refresh, draft) {
  const svg = canvas.querySelector('#rt-edges');
  svg.innerHTML = `<defs>
    <marker id="rt-ar" markerWidth="9" markerHeight="9" refX="7" refY="3" orient="auto">
      <path d="M0,0 L7,3 L0,6 Z" fill="var(--text-secondary)"/></marker></defs>`;
  chain.edges.forEach((e) => {
    const a = nodeBox(canvas, e.from_node_id);
    const b = nodeBox(canvas, e.to_node_id);
    if (!a || !b) return;
    const ax = a.x + a.w, ay = a.y + a.h / 2, bx = b.x, by = b.y + b.h / 2;
    const mx = (ax + bx) / 2;
    const d = `M${ax},${ay} C${mx},${ay} ${mx},${by} ${bx},${by}`;
    const wire = svgPath(d);
    wire.setAttribute('class', 'rt-wire');
    wire.setAttribute('fill', 'none');
    wire.setAttribute('stroke', edgeColor(e.trigger_type));
    wire.setAttribute('stroke-width', '1.8');
    if (e.trigger_type === 'after_duration') wire.setAttribute('stroke-dasharray', '5,4');
    if (e.trigger_type === 'manual') wire.setAttribute('stroke-dasharray', '2,3');
    wire.setAttribute('marker-end', 'url(#rt-ar)');
    const hit = svgPath(d);
    hit.setAttribute('class', 'rt-hit');
    hit.addEventListener('click', (ev) => openEdgeMenu(canvas, chain, refresh, e, ev));
    svg.append(wire, hit);
  });
  if (draft) {
    const a = nodeBox(canvas, draft.fromId);
    if (a) {
      const ln = svgPath(`M${a.x + a.w},${a.y + a.h / 2} L${draft.x},${draft.y}`);
      ln.setAttribute('fill', 'none');
      ln.setAttribute('stroke', 'var(--accent-blue)');
      ln.setAttribute('stroke-width', '2');
      ln.setAttribute('stroke-dasharray', '4,4');
      svg.append(ln);
    }
  }
}
