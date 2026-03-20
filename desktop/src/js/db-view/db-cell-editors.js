// ── db-view/db-cell-editors.js — Inline cell editing (Notion-style, no popups) ──

import { invoke } from '../state.js';
import { escapeHtml } from '../utils.js';
import { showSelectDropdown, showMultiSelectDropdown } from './db-dropdowns.js';
import { getNextCell, getPrevCell, getCellBelow } from './db-cell-nav.js';

const BADGE_COLORS = ['blue', 'green', 'yellow', 'red', 'purple', 'orange', 'pink', 'gray'];

function badgeColor(val, prop) {
  let opts = []; try { opts = JSON.parse(prop.options || '[]'); } catch {}
  const idx = opts.indexOf(val);
  return BADGE_COLORS[idx >= 0 ? idx % BADGE_COLORS.length : 0];
}

export function formatPropValue(val, prop) {
  if (!val && val !== 0) return '<span class="text-faint">\u2014</span>';
  if (prop.type === 'checkbox') return `<span class="cell-check-round${val === 'true' ? ' checked' : ''}"></span>`;
  if (prop.type === 'select') return `<span class="badge badge-${badgeColor(val, prop)}">${escapeHtml(val)}</span>`;
  if (prop.type === 'multi_select') {
    try { return JSON.parse(val).map(i => `<span class="badge badge-${badgeColor(i, prop)}">${escapeHtml(i)}</span>`).join(' '); }
    catch { return escapeHtml(val); }
  }
  if (prop.type === 'url') return `<a href="${escapeHtml(val)}" target="_blank" class="cell-link">${escapeHtml(val.length > 30 ? val.substring(0, 30) + '...' : val)}</a>`;
  return escapeHtml(val);
}

function overlayEditor(cell, type, value) {
  cell.style.position = 'relative';
  Array.from(cell.children).forEach(c => c.style.visibility = 'hidden');
  const input = document.createElement('input');
  input.type = type;
  input.className = 'inline-editor';
  input.value = value;
  cell.appendChild(input);
  input.focus();
  if (type === 'text') input.select();
  return input;
}

function removeEditor(cell) {
  const editor = cell.querySelector('.inline-editor');
  if (editor) editor.remove();
  Array.from(cell.children).forEach(c => c.style.visibility = '');
  cell.style.position = '';
}

function editorKeydown(e, editor, cell, closeFn) {
  if (e.key === 'Tab') {
    e.preventDefault();
    const next = e.shiftKey ? getPrevCell(cell) : getNextCell(cell);
    cell._navTarget = next;
    editor.blur();
    return;
  }
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    const below = getCellBelow(cell);
    cell._navTarget = below;
    editor.blur();
    return;
  }
  if (e.key === 'Escape') { if (closeFn) closeFn(); }
  e.stopPropagation();
}

/** Start inline editing for a fixed column cell */
export function startFixedCellEdit(cell, reloadFn) {
  if (cell.querySelector('.inline-editor')) return;
  const editType = cell.dataset.editType || 'text';
  let options = [];
  try { options = JSON.parse(cell.dataset.editOptions || '[]'); } catch {}

  const rawVal = cell.dataset.rawValue || cell.textContent.trim();

  if (editType === 'select') {
    showSelectDropdown(cell, options.map(o => typeof o === 'object' ? o : { value: o, label: o }), rawVal, (val) => {
      cell.dispatchEvent(new CustomEvent('fixed-cell-save', {
        bubbles: true,
        detail: { recordId: cell.dataset.recordId, key: cell.dataset.editKey, value: val },
      }));
    });
    return;
  }

  const cleanVal = rawVal === '\u2014' ? '' : rawVal;
  const editor = overlayEditor(cell, 'text', cleanVal);

  const saveAndClose = () => {
    const val = editor.value || '';
    const navTarget = cell._navTarget;
    delete cell._navTarget;
    removeEditor(cell);
    cell.dataset.rawValue = val;
    const display = cell.querySelector('.data-table-title, span');
    if (display) display.textContent = val || '\u2014';
    cell.dispatchEvent(new CustomEvent('fixed-cell-save', {
      bubbles: true,
      detail: { recordId: cell.dataset.recordId, key: cell.dataset.editKey, value: val, skipReload: !!navTarget },
    }));
    if (navTarget) setTimeout(() => navTarget.click(), 10);
  };
  editor.addEventListener('blur', saveAndClose);
  editor.addEventListener('keydown', (e) => editorKeydown(e, editor, cell, () => removeEditor(cell)));
}

/** Start inline editing on a custom property cell */
export function startInlineEdit(cell, recordTable, reloadFn) {
  if (cell.querySelector('.inline-editor') || cell.querySelector('.inline-dropdown')) return;
  const recordId = parseInt(cell.dataset.recordId);
  const propId = parseInt(cell.dataset.propId);
  const propType = cell.dataset.propType;
  const rawVal = cell.dataset.rawValue || '';
  let options = [];
  try { options = JSON.parse(cell.dataset.propOptions || '[]'); } catch {}

  const save = (val, skipReload) => {
    invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: val })
      .then(() => { if (reloadFn && !skipReload) reloadFn(); }).catch(() => {});
  };

  switch (propType) {
    case 'checkbox':
      save(rawVal === 'true' ? 'false' : 'true');
      return;
    case 'select':
      showSelectDropdown(cell, options.map(o => ({ value: o, label: o })), rawVal, save);
      return;
    case 'multi_select':
      showMultiSelectDropdown(cell, options, rawVal, save);
      return;
    default: {
      const inputType = propType === 'date' ? 'date' : propType === 'number' ? 'number' : 'text';
      const editor = overlayEditor(cell, inputType, rawVal || '');
      const done = () => {
        const navTarget = cell._navTarget;
        delete cell._navTarget;
        removeEditor(cell);
        cell.dataset.rawValue = editor.value || '';
        save(editor.value || null, !!navTarget);
        if (navTarget) setTimeout(() => navTarget.click(), 10);
      };
      editor.addEventListener('blur', done);
      if (propType === 'date') editor.addEventListener('change', done);
      editor.addEventListener('keydown', (e) => editorKeydown(e, editor, cell, () => removeEditor(cell)));
    }
  }
}
