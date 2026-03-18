import { invoke } from '../state.js';

/** Enable drag-and-drop reordering on column headers */
export function enableColumnDrag(tableEl, tabId, reloadFn) {
  const headers = [...tableEl.querySelectorAll('th.prop-header')];
  if (headers.length < 2) return;

  let dragSrcId = null;

  headers.forEach(th => {
    th.draggable = true;
    th.addEventListener('dragstart', (e) => {
      dragSrcId = th.dataset.propId;
      th.classList.add('col-dragging');
      e.dataTransfer.effectAllowed = 'move';
    });
    th.addEventListener('dragend', () => {
      th.classList.remove('col-dragging');
      headers.forEach(h => h.classList.remove('col-drag-over'));
    });
    th.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      headers.forEach(h => h.classList.remove('col-drag-over'));
      if (th.dataset.propId !== dragSrcId) th.classList.add('col-drag-over');
    });
    th.addEventListener('drop', async (e) => {
      e.preventDefault();
      const targetId = th.dataset.propId;
      if (!dragSrcId || dragSrcId === targetId) return;
      // Reorder: put dragSrc before target
      const order = headers.map(h => parseInt(h.dataset.propId));
      const srcIdx = order.indexOf(parseInt(dragSrcId));
      const tgtIdx = order.indexOf(parseInt(targetId));
      order.splice(srcIdx, 1);
      order.splice(tgtIdx, 0, parseInt(dragSrcId));
      // Persist new positions
      for (let i = 0; i < order.length; i++) {
        await invoke('update_property_definition', { id: order[i], position: i }).catch(() => {});
      }
      dragSrcId = null;
      if (reloadFn) reloadFn();
    });
  });
}
