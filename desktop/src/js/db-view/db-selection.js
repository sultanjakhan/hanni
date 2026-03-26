// ── db-view/db-selection.js — Cell range selection (Notion/Excel style) ──

const sel = { anchor: null, extent: null, active: false };

function cellCoords(cell) {
  const row = cell.closest('tr');
  if (!row) return null;
  return { row: Array.from(row.parentElement.children).indexOf(row), col: Array.from(row.children).indexOf(cell) };
}

function getRange() {
  if (!sel.anchor || !sel.extent) return null;
  return {
    minRow: Math.min(sel.anchor.row, sel.extent.row),
    maxRow: Math.max(sel.anchor.row, sel.extent.row),
    minCol: Math.min(sel.anchor.col, sel.extent.col),
    maxCol: Math.max(sel.anchor.col, sel.extent.col),
  };
}

function applyClasses(table) {
  table.querySelectorAll('.cell-range-selected, .cell-range-top, .cell-range-bottom, .cell-range-left, .cell-range-right')
    .forEach(c => c.classList.remove('cell-range-selected', 'cell-range-top', 'cell-range-bottom', 'cell-range-left', 'cell-range-right'));
  table.querySelectorAll('th.col-range-selected').forEach(th => th.classList.remove('col-range-selected'));

  const r = getRange();
  if (!r) return;

  const rows = table.querySelectorAll('tbody tr.data-table-row');
  for (let ri = r.minRow; ri <= r.maxRow && ri < rows.length; ri++) {
    const cells = rows[ri].children;
    for (let ci = r.minCol; ci <= r.maxCol && ci < cells.length; ci++) {
      const td = cells[ci];
      td.classList.add('cell-range-selected');
      if (ri === r.minRow) td.classList.add('cell-range-top');
      if (ri === r.maxRow) td.classList.add('cell-range-bottom');
      if (ci === r.minCol) td.classList.add('cell-range-left');
      if (ci === r.maxCol) td.classList.add('cell-range-right');
    }
  }
  // Highlight selected column headers
  const ths = table.querySelectorAll('thead th');
  for (let ci = r.minCol; ci <= r.maxCol && ci < ths.length; ci++) {
    ths[ci].classList.add('col-range-selected');
  }
}

export function setAnchor(cell, table) {
  const coords = cellCoords(cell);
  if (!coords) return;
  sel.anchor = sel.extent = coords;
  sel.active = true;
  applyClasses(table);
}

export function extendTo(cell, table) {
  if (!sel.anchor) { setAnchor(cell, table); return; }
  const coords = cellCoords(cell);
  if (!coords) return;
  sel.extent = coords;
  applyClasses(table);
}

export function selectColumn(colIndex, table) {
  const rows = table.querySelectorAll('tbody tr.data-table-row');
  if (rows.length === 0) return;
  sel.anchor = { row: 0, col: colIndex };
  sel.extent = { row: rows.length - 1, col: colIndex };
  sel.active = true;
  applyClasses(table);
}

export function clearSelection(table) {
  if (!sel.active) return;
  sel.anchor = sel.extent = null;
  sel.active = false;
  if (table) applyClasses(table);
}

export function getSelectedCells(table) {
  return table ? Array.from(table.querySelectorAll('.cell-range-selected')) : [];
}

export function isSelectionActive() { return sel.active; }

export function buildTSV(table) {
  const r = getRange();
  if (!r) return '';
  const rows = table.querySelectorAll('tbody tr.data-table-row');
  const lines = [];
  for (let ri = r.minRow; ri <= r.maxRow && ri < rows.length; ri++) {
    const cells = rows[ri].children;
    const vals = [];
    for (let ci = r.minCol; ci <= r.maxCol && ci < cells.length; ci++) {
      vals.push(cells[ci].dataset.rawValue || cells[ci].textContent.trim());
    }
    lines.push(vals.join('\t'));
  }
  return lines.join('\n');
}
