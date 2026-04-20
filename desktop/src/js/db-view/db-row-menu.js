// ── db-view/db-row-menu.js — Row context menu (right-click) ──

import { confirmModal } from '../utils.js';

/** Bind right-click context menu on table rows */
export function bindRowContextMenu(container, ctx) {
  const { records, idField, onDelete, onDuplicate, reloadFn } = ctx;
  container.querySelectorAll('.data-table-row').forEach(row => {
    row.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const rid = parseInt(row.dataset.id);
      const record = records.find(r => r[idField] === rid);
      if (!record) return;
      showRowMenu(e, rid, record, ctx);
    });
  });
}

function showRowMenu(e, rid, record, ctx) {
  document.querySelectorAll('.row-context-menu').forEach(m => m.remove());
  const { onDelete, onDuplicate, onFreeze, reloadFn } = ctx;

  const items = [];
  items.push({ id: 'copy', label: 'Копировать ID', icon: '⎘' });
  if (onDuplicate) items.push({ id: 'duplicate', label: 'Дубликат', icon: '⧉' });
  if (onFreeze) items.push({ id: 'freeze', label: 'Заморозить', icon: '❄' });
  if (onDelete) items.push({ id: 'delete', label: 'Удалить', icon: '✕', danger: true });

  const menu = document.createElement('div');
  menu.className = 'inline-dropdown row-context-menu';
  menu.innerHTML = items.map(i =>
    `<div class="inline-dd-option${i.danger ? ' danger' : ''}" data-action="${i.id}"><span style="margin-right:6px;">${i.icon}</span>${i.label}</div>`
  ).join('');
  menu.style.left = e.clientX + 'px';
  menu.style.top = e.clientY + 'px';
  document.body.appendChild(menu);

  menu.querySelectorAll('.inline-dd-option').forEach(item => {
    item.addEventListener('click', async () => {
      const action = item.dataset.action;
      menu.remove();
      if (action === 'copy') {
        try { await navigator.clipboard.writeText(String(rid)); } catch {}
      } else if (action === 'duplicate') {
        if (onDuplicate) { await onDuplicate(record); if (reloadFn) reloadFn(); }
      } else if (action === 'freeze') {
        if (onFreeze) { await onFreeze(rid, record); if (reloadFn) reloadFn(); }
      } else if (action === 'delete') {
        if (onDelete && await confirmModal('Удалить запись?')) {
          await onDelete(rid);
          surgicalRowRemove(container, rid);
        }
      }
    });
  });

  setTimeout(() => document.addEventListener('mousedown', (ev) => {
    if (!menu.contains(ev.target)) menu.remove();
  }, { once: true }), 10);
}

/** Remove a single row from DOM without full re-render */
function surgicalRowRemove(container, rid) {
  const tr = container.querySelector(`tr.data-table-row[data-id="${rid}"]`);
  if (tr) tr.remove();
  const remaining = container.querySelectorAll('.data-table-row');
  const footer = container.querySelector('.dbv-table-footer span');
  if (footer) footer.textContent = `Записей: ${remaining.length}`;
  if (remaining.length === 0) {
    const tbody = container.querySelector('tbody');
    const colCount = container.querySelector('thead tr')?.children.length || 1;
    const addRow = tbody?.querySelector('.dbv-add-row');
    const emptyRow = document.createElement('tr');
    emptyRow.innerHTML = `<td colspan="${colCount}" style="text-align:center;color:var(--text-faint);padding:24px;">Пока пусто</td>`;
    if (addRow) addRow.before(emptyRow); else if (tbody) tbody.appendChild(emptyRow);
  }
}

export { surgicalRowRemove };
