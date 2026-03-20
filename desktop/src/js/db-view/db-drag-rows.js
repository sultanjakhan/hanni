// ── db-view/db-drag-rows.js — Drag-and-drop row reordering ──

/** Enable drag-and-drop reordering on table rows */
export function enableRowDrag(container, records, idField, onReorder) {
  let dragRow = null;
  const rows = container.querySelectorAll('.data-table-row');
  if (rows.length < 2) return;

  rows.forEach(row => {
    row.draggable = true;
    row.addEventListener('dragstart', (e) => {
      dragRow = row;
      row.classList.add('row-dragging');
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', row.dataset.id);
    });
    row.addEventListener('dragend', () => {
      if (dragRow) dragRow.classList.remove('row-dragging');
      dragRow = null;
      container.querySelectorAll('.row-drag-over').forEach(r => r.classList.remove('row-drag-over'));
    });
    row.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      if (row !== dragRow) {
        container.querySelectorAll('.row-drag-over').forEach(r => r.classList.remove('row-drag-over'));
        row.classList.add('row-drag-over');
      }
    });
    row.addEventListener('dragleave', () => {
      row.classList.remove('row-drag-over');
    });
    row.addEventListener('drop', (e) => {
      e.preventDefault();
      row.classList.remove('row-drag-over');
      if (!dragRow || dragRow === row) return;

      const fromId = parseInt(dragRow.dataset.id);
      const toId = parseInt(row.dataset.id);
      const fromIdx = records.findIndex(r => r[idField] === fromId);
      const toIdx = records.findIndex(r => r[idField] === toId);
      if (fromIdx < 0 || toIdx < 0) return;

      // Reorder in-memory
      const [moved] = records.splice(fromIdx, 1);
      records.splice(toIdx, 0, moved);

      // Visual reorder (instant, no reload)
      const tbody = container.querySelector('tbody');
      if (tbody) {
        tbody.removeChild(dragRow);
        if (fromIdx < toIdx) row.after(dragRow);
        else row.before(dragRow);
      }

      if (onReorder) onReorder(records.map(r => r[idField]));
    });
  });
}
