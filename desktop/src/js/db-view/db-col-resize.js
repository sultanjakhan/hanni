// ── db-view/db-col-resize.js — Column resize via drag on header border ──

const MIN_COL_WIDTH = 60;

/**
 * Wire column resize handles on table headers.
 * Saves widths to localStorage keyed by tabId.
 */
export function wireColumnResize(el, tabId) {
  const table = el.querySelector('.data-table');
  if (!table) return;

  // Apply saved widths
  const saved = loadWidths(tabId);
  const headers = table.querySelectorAll('thead th');
  headers.forEach((th, i) => {
    if (saved[i] != null) th.style.width = saved[i] + 'px';
    th.style.position = 'relative';
  });

  headers.forEach((th, i) => {
    // Skip drag/check/add-prop columns
    if (th.classList.contains('col-drag') || th.classList.contains('col-check') || th.classList.contains('col-check-header') || th.classList.contains('add-prop-col')) return;

    const handle = document.createElement('div');
    handle.className = 'col-resize-handle';
    th.appendChild(handle);

    let startX = 0, startW = 0;

    handle.addEventListener('mousedown', (e) => {
      e.preventDefault();
      e.stopPropagation();
      startX = e.clientX;
      startW = th.offsetWidth;
      handle.classList.add('active');

      const onMove = (me) => {
        const w = Math.max(MIN_COL_WIDTH, startW + me.clientX - startX);
        th.style.width = w + 'px';
        th.style.minWidth = w + 'px';
        // Sync body cells
        const rows = table.querySelectorAll('tbody tr');
        rows.forEach(row => {
          const td = row.children[i];
          if (td) { td.style.width = w + 'px'; td.style.minWidth = w + 'px'; }
        });
      };

      const onUp = () => {
        handle.classList.remove('active');
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        saveWidths(tabId, table);
      };

      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });

    // Double-click to auto-fit
    handle.addEventListener('dblclick', (e) => {
      e.preventDefault();
      e.stopPropagation();
      const rows = table.querySelectorAll('tbody tr');
      let maxW = MIN_COL_WIDTH;
      rows.forEach(row => {
        const td = row.children[i];
        if (td) { td.style.width = ''; td.style.minWidth = ''; maxW = Math.max(maxW, td.scrollWidth + 16); }
      });
      th.style.width = maxW + 'px';
      th.style.minWidth = maxW + 'px';
      saveWidths(tabId, table);
    });
  });
}

function saveWidths(tabId, table) {
  const widths = {};
  table.querySelectorAll('thead th').forEach((th, i) => {
    if (th.offsetWidth) widths[i] = th.offsetWidth;
  });
  localStorage.setItem(`dbv_colwidths_${tabId}`, JSON.stringify(widths));
}

function loadWidths(tabId) {
  try { return JSON.parse(localStorage.getItem(`dbv_colwidths_${tabId}`) || '{}'); } catch { return {}; }
}
