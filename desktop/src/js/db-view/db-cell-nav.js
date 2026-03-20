// ── db-view/db-cell-nav.js — Cell navigation helpers ──

/** Get next editable cell (right, wraps to next row) */
export function getNextCell(cell) {
  let next = cell.nextElementSibling;
  while (next && !next.classList.contains('cell-editable') && !next.classList.contains('cell-fixed-edit')) {
    next = next.nextElementSibling;
  }
  if (next) return next;
  const row = cell.closest('tr');
  const nextRow = row?.nextElementSibling;
  if (nextRow?.classList.contains('data-table-row')) {
    return nextRow.querySelector('.cell-editable, .cell-fixed-edit');
  }
  return null;
}

/** Get previous editable cell (left, wraps to prev row) */
export function getPrevCell(cell) {
  let prev = cell.previousElementSibling;
  while (prev && !prev.classList.contains('cell-editable') && !prev.classList.contains('cell-fixed-edit')) {
    prev = prev.previousElementSibling;
  }
  if (prev) return prev;
  const row = cell.closest('tr');
  const prevRow = row?.previousElementSibling;
  if (prevRow?.classList.contains('data-table-row')) {
    const cells = prevRow.querySelectorAll('.cell-editable, .cell-fixed-edit');
    return cells[cells.length - 1] || null;
  }
  return null;
}

/** Get cell directly below */
export function getCellBelow(cell) {
  const row = cell.closest('tr');
  const idx = Array.from(row.children).indexOf(cell);
  const nextRow = row?.nextElementSibling;
  if (nextRow?.classList.contains('data-table-row')) {
    return nextRow.children[idx] || null;
  }
  return null;
}

/** Get cell directly above */
export function getCellAbove(cell) {
  const row = cell.closest('tr');
  const idx = Array.from(row.children).indexOf(cell);
  const prevRow = row?.previousElementSibling;
  if (prevRow?.classList.contains('data-table-row')) {
    return prevRow.children[idx] || null;
  }
  return null;
}

/** Clear cell focus from all cells */
export function clearCellFocus(el) {
  el.querySelectorAll('.cell-focused').forEach(c => c.classList.remove('cell-focused'));
}
