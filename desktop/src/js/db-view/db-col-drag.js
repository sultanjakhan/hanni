import { invoke } from '../state.js';
import { getColumnOrder, setColumnOrder } from './db-properties.js';

/** Enable mouse-based drag reordering on column headers (works in WKWebView) */
export function enableColumnDrag(tableEl, tabId, reloadFn, visProps) {
  const headers = [...tableEl.querySelectorAll('th.draggable-col')];
  if (headers.length < 2) return;

  let dragSrc = null, ghost = null, startX = 0;

  headers.forEach(th => {
    th.style.cursor = 'grab';
    th.addEventListener('mousedown', (e) => {
      if (e.button !== 0 || e.target.closest('.col-resize-handle')) return;
      e.preventDefault();
      dragSrc = th;
      startX = e.clientX;
      th.classList.add('col-dragging');

      // Ghost element
      ghost = th.cloneNode(true);
      ghost.className = 'col-drag-ghost';
      ghost.style.cssText = `position:fixed;top:${th.getBoundingClientRect().top}px;left:${e.clientX - 40}px;width:${th.offsetWidth}px;height:${th.offsetHeight}px;opacity:0.8;pointer-events:none;z-index:9999;background:var(--bg-secondary);border:1px solid var(--accent-blue);border-radius:4px;display:flex;align-items:center;padding:0 8px;font-size:12px;`;
      document.body.appendChild(ghost);

      const onMove = (ev) => {
        if (!ghost) return;
        ghost.style.left = (ev.clientX - 40) + 'px';
        headers.forEach(h => h.classList.remove('col-drag-over'));
        const target = headers.find(h => {
          if (h === dragSrc) return false;
          const r = h.getBoundingClientRect();
          return ev.clientX > r.left && ev.clientX < r.right;
        });
        if (target) target.classList.add('col-drag-over');
      };

      const onUp = async (ev) => {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        if (ghost) { ghost.remove(); ghost = null; }
        if (dragSrc) dragSrc.classList.remove('col-dragging');
        headers.forEach(h => h.classList.remove('col-drag-over'));

        const target = headers.find(h => {
          if (h === dragSrc) return false;
          const r = h.getBoundingClientRect();
          return ev.clientX > r.left && ev.clientX < r.right;
        });

        if (!target || !dragSrc) { dragSrc = null; return; }

        const order = headers.map(h => h.dataset.colId);
        const srcIdx = order.indexOf(dragSrc.dataset.colId);
        const tgtIdx = order.indexOf(target.dataset.colId);
        const srcId = order.splice(srcIdx, 1)[0];
        order.splice(tgtIdx, 0, srcId);

        setColumnOrder(tabId, order);
        if (visProps && visProps.length > 0) {
          const propOrder = order.filter(id => id.startsWith('prop_'));
          for (let i = 0; i < propOrder.length; i++) {
            const propId = parseInt(propOrder[i].replace('prop_', ''));
            await invoke('update_property_definition', { id: propId, name: null, propType: null, position: i, color: null, options: null, visible: null }).catch(() => {});
          }
        }
        dragSrc = null;
        if (reloadFn) reloadFn();
      };

      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });
  });
}
