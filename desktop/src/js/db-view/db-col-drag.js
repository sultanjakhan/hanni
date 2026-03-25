import { invoke } from '../state.js';
import { setColumnOrder } from './db-properties.js';

/** Enable mouse-based drag reordering on column headers (works in WKWebView) */
export function enableColumnDrag(tableEl, tabId, reloadFn, visProps) {
  const headerRow = tableEl.querySelector('thead tr');
  if (!headerRow) return;
  const getHeaders = () => [...headerRow.querySelectorAll('th.draggable-col')];
  if (getHeaders().length < 2) return;

  let dragSrc = null, ghost = null;

  const onMove = (ev) => {
    if (!ghost || !dragSrc) return;
    ghost.style.left = (ev.clientX - 40) + 'px';

    const headers = getHeaders();
    const target = headers.find(h => {
      if (h === dragSrc) return false;
      const r = h.getBoundingClientRect();
      return ev.clientX > r.left && ev.clientX < r.right;
    });
    if (!target) return;

    // Swap columns in DOM immediately
    const srcIdx = headers.indexOf(dragSrc);
    const tgtIdx = headers.indexOf(target);
    if (srcIdx < 0 || tgtIdx < 0) return;

    // Swap header cells
    if (srcIdx < tgtIdx) headerRow.insertBefore(dragSrc, target.nextSibling);
    else headerRow.insertBefore(dragSrc, target);

    // Swap body cells in each row
    tableEl.querySelectorAll('tbody tr').forEach(row => {
      const cells = [...row.children];
      const offset = row.querySelector('.col-check') ? 1 : 0;
      const srcCell = cells[srcIdx + offset];
      const tgtCell = cells[tgtIdx + offset];
      if (!srcCell || !tgtCell) return;
      if (srcIdx < tgtIdx) row.insertBefore(srcCell, tgtCell.nextSibling);
      else row.insertBefore(srcCell, tgtCell);
    });
  };

  const onUp = async (ev) => {
    document.removeEventListener('mousemove', onMove);
    document.removeEventListener('mouseup', onUp);
    if (ghost) { ghost.remove(); ghost = null; }
    if (dragSrc) dragSrc.classList.remove('col-dragging');

    if (!dragSrc) return;
    const headers = getHeaders();
    const order = headers.map(h => h.dataset.colId);

    setColumnOrder(tabId, order);
    if (visProps && visProps.length > 0) {
      const propOrder = order.filter(id => id.startsWith('prop_'));
      for (let i = 0; i < propOrder.length; i++) {
        const propId = parseInt(propOrder[i].replace('prop_', ''));
        await invoke('update_property_definition', { id: propId, name: null, propType: null, position: i, color: null, options: null, visible: null }).catch(() => {});
      }
    }
    dragSrc = null;
  };

  getHeaders().forEach(th => {
    th.style.cursor = 'grab';
    th.addEventListener('mousedown', (e) => {
      if (e.button !== 0 || e.target.closest('.col-resize-handle')) return;
      e.preventDefault();
      dragSrc = th;
      th.classList.add('col-dragging');

      ghost = th.cloneNode(true);
      ghost.className = 'col-drag-ghost';
      ghost.style.cssText = `position:fixed;top:${th.getBoundingClientRect().top}px;left:${e.clientX - 40}px;width:${th.offsetWidth}px;height:${th.offsetHeight}px;opacity:0.85;pointer-events:none;z-index:9999;background:var(--bg-secondary);border:1px solid var(--accent-blue);border-radius:4px;display:flex;align-items:center;padding:0 8px;font-size:12px;box-shadow:0 4px 12px rgba(0,0,0,.15);`;
      document.body.appendChild(ghost);

      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });
  });
}
