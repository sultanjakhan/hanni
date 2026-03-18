// ── db-view/db-cell-editors.js — Inline cell editing (type-aware) ──

import { invoke } from '../state.js';
import { escapeHtml } from '../utils.js';

/** Format a property value for display in a table cell */
export function formatPropValue(val, prop) {
  if (!val && val !== 0) return '<span class="text-faint">\u2014</span>';
  switch (prop.type) {
    case 'checkbox': return val === 'true' ? '\u2713' : '\u2014';
    case 'select': return `<span class="badge badge-blue">${escapeHtml(val)}</span>`;
    case 'status': {
      const colors = { 'Готово': 'green', 'В работе': 'blue', 'Не начато': 'gray' };
      const c = colors[val] || 'gray';
      return `<span class="badge badge-${c}">${escapeHtml(val)}</span>`;
    }
    case 'multi_select': {
      try {
        const items = JSON.parse(val);
        return items.map(i => `<span class="badge badge-purple">${escapeHtml(i)}</span>`).join(' ');
      } catch { return escapeHtml(val); }
    }
    case 'url': return `<a href="${escapeHtml(val)}" target="_blank" style="color:var(--accent-blue);text-decoration:none;">${escapeHtml(val.substring(0, 30))}</a>`;
    case 'number': return escapeHtml(val);
    case 'date': return escapeHtml(val);
    default: return escapeHtml(val);
  }
}

/** Start inline editing on a cell click */
export function startInlineEdit(cell, recordTable, reloadFn) {
  if (cell.querySelector('.inline-editor')) return;
  const recordId = parseInt(cell.dataset.recordId);
  const propId = parseInt(cell.dataset.propId);
  const propType = cell.dataset.propType;
  let options = [];
  try { options = JSON.parse(cell.dataset.propOptions || '[]'); } catch {}

  const currentVal = cell.textContent.trim();
  const originalHtml = cell.innerHTML;

  let editorHtml = '';
  switch (propType) {
    case 'status': {
      const statusOpts = options.length > 0 ? options : ['Не начато', 'В работе', 'Готово'];
      editorHtml = `<select class="inline-editor inline-select">
        <option value="">\u2014</option>
        ${statusOpts.map(o => `<option value="${escapeHtml(o)}"${o === currentVal ? ' selected' : ''}>${escapeHtml(o)}</option>`).join('')}
      </select>`;
      break;
    }
    case 'select':
      editorHtml = `<select class="inline-editor inline-select">
        <option value="">\u2014</option>
        ${options.map(o => `<option value="${escapeHtml(o)}"${o === currentVal ? ' selected' : ''}>${escapeHtml(o)}</option>`).join('')}
      </select>`;
      break;
    case 'multi_select': {
      let selected = [];
      try { selected = JSON.parse(cell.dataset.currentValue || '[]'); } catch {}
      editorHtml = `<div class="inline-editor inline-multi-select">
        ${options.map(o => `<label class="inline-ms-option"><input type="checkbox" value="${escapeHtml(o)}"${selected.includes(o) ? ' checked' : ''}> ${escapeHtml(o)}</label>`).join('')}
        <button class="btn-primary inline-ms-done" style="font-size:11px;padding:2px 8px;margin-top:4px;">OK</button>
      </div>`;
      break;
    }
    case 'checkbox': {
      const newVal = currentVal === '\u2713' ? 'false' : 'true';
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: newVal })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
      return;
    }
    case 'date':
      editorHtml = `<input type="date" class="inline-editor inline-date" value="${currentVal === '\u2014' ? '' : currentVal}">`;
      break;
    case 'number':
      editorHtml = `<input type="number" class="inline-editor inline-number" value="${currentVal === '\u2014' ? '' : currentVal}">`;
      break;
    default:
      editorHtml = `<input type="text" class="inline-editor inline-text" value="${currentVal === '\u2014' ? '' : escapeHtml(currentVal)}">`;
  }

  cell.innerHTML = editorHtml;
  const editor = cell.querySelector('.inline-editor');
  if (editor.tagName === 'INPUT' || editor.tagName === 'SELECT') {
    editor.focus();
    const saveAndClose = () => {
      const val = editor.value || null;
      cell.innerHTML = originalHtml;
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: val })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
    };
    editor.addEventListener('blur', saveAndClose);
    editor.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); editor.blur(); }
      if (e.key === 'Escape') { cell.innerHTML = originalHtml; }
    });
  } else if (propType === 'multi_select') {
    cell.querySelector('.inline-ms-done')?.addEventListener('click', () => {
      const checked = [...cell.querySelectorAll('input:checked')].map(cb => cb.value);
      const val = checked.length > 0 ? JSON.stringify(checked) : null;
      cell.innerHTML = originalHtml;
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: val })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
    });
  }
}
