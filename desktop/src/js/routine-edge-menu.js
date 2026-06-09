// ── js/routine-edge-menu.js — Context menu for a routine graph edge ──
// Change the edge's trigger type / value, or delete it.
import { invoke } from './state.js';
import { promptModal } from './prompt-modal.js';

export function openEdgeMenu(canvas, chain, refresh, edge, ev) {
  canvas.querySelectorAll('.rt-edge-menu').forEach(m => m.remove());
  const m = document.createElement('div');
  m.className = 'rt-edge-menu';
  m.style.left = (ev.clientX - canvas.getBoundingClientRect().left) + 'px';
  m.style.top = (ev.clientY - canvas.getBoundingClientRect().top) + 'px';
  m.innerHTML = `
    <div data-t="after_completion">→ после завершения</div>
    <div data-t="after_duration">⏱ через N минут…</div>
    <div data-t="manual">○ вручную</div>
    <div class="rt-em-sep"></div>
    <div class="rt-em-del" data-t="del">✕ удалить связь</div>`;
  m.querySelectorAll('[data-t]').forEach(it => it.addEventListener('click', async () => {
    const t = it.dataset.t;
    if (t === 'del') {
      await invoke('delete_routine_edge', { id: edge.id }).catch(() => {});
    } else if (t === 'after_duration') {
      const raw = await promptModal({
        title: 'Через сколько минут?', value: edge.trigger_value || 55, type: 'number',
      });
      const v = parseInt(raw);
      if (v > 0) await invoke('update_routine_edge', {
        id: edge.id, triggerType: 'after_duration', triggerValue: v,
      }).catch(() => {});
    } else {
      await invoke('update_routine_edge', { id: edge.id, triggerType: t, triggerValue: null }).catch(() => {});
    }
    refresh();
  }));
  canvas.appendChild(m);
  setTimeout(() => document.addEventListener('mousedown', function close(e) {
    if (!e.target.closest('.rt-edge-menu')) { m.remove(); document.removeEventListener('mousedown', close); }
  }), 0);
}
