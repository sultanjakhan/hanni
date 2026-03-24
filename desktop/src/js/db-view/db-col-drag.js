import { invoke } from '../state.js';
import { getColumnOrder, setColumnOrder } from './db-properties.js';

/** Enable drag-and-drop reordering on all column headers (fixed + custom) */
export function enableColumnDrag(tableEl, tabId, reloadFn, visProps) {
  const headers = [...tableEl.querySelectorAll('th.draggable-col')];
  if (headers.length < 2) return;

  let dragSrcId = null;

  headers.forEach(th => {
    th.draggable = true;
    th.addEventListener('dragstart', (e) => {
      dragSrcId = th.dataset.colId;
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
      if (th.dataset.colId !== dragSrcId) th.classList.add('col-drag-over');
    });
    th.addEventListener('drop', async (e) => {
      e.preventDefault();
      const targetId = th.dataset.colId;
      if (!dragSrcId || dragSrcId === targetId) return;

      // Build new order from current DOM
      const order = headers.map(h => h.dataset.colId);
      const srcIdx = order.indexOf(dragSrcId);
      const tgtIdx = order.indexOf(targetId);
      order.splice(srcIdx, 1);
      order.splice(tgtIdx, 0, dragSrcId);

      // Persist unified order to localStorage
      setColumnOrder(tabId, order);

      // Also update DB positions for custom props so they stay consistent
      if (visProps && visProps.length > 0) {
        const propOrder = order.filter(id => id.startsWith('prop_'));
        for (let i = 0; i < propOrder.length; i++) {
          const propId = parseInt(propOrder[i].replace('prop_', ''));
          await invoke('update_property_definition', { id: propId, position: i }).catch(() => {});
        }
      }

      dragSrcId = null;
      if (reloadFn) reloadFn();
    });
  });
}
