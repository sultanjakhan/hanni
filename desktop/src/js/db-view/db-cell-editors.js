// ── db-view/db-cell-editors.js — Inline cell editing (Notion-style, no popups) ──

import { invoke } from '../state.js';
import { showSelectDropdown, showMultiSelectDropdown } from './db-dropdowns.js';
import { getNextCell, getPrevCell, getCellBelow } from './db-cell-nav.js';
import { renderTimeEditor, renderProgressEditor, renderRatingEditor } from './db-type-editors.js';
export { formatPropValue } from './db-cell-format.js';

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
    const changed = val !== cleanVal;
    if (changed) {
      cell.dataset.rawValue = val;
      const display = cell.querySelector('.data-table-title, span');
      if (display) display.textContent = val || '\u2014';
      cell.dispatchEvent(new CustomEvent('fixed-cell-save', {
        bubbles: true,
        detail: { recordId: cell.dataset.recordId, key: cell.dataset.editKey, value: val, skipReload: true },
      }));
    }
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
    case 'time':
      renderTimeEditor(cell, rawVal, (val) => { save(val); if (reloadFn) reloadFn(); });
      return;
    case 'progress':
      renderProgressEditor(cell, rawVal, (val) => { save(val); if (reloadFn) reloadFn(); });
      return;
    case 'rating':
      renderRatingEditor(cell, rawVal, (val) => { save(val); if (reloadFn) reloadFn(); });
      return;
    default: {
      const inputType = propType === 'date' ? 'date' : propType === 'number' ? 'number' : 'text';
      const origVal = rawVal || '';
      const editor = overlayEditor(cell, inputType, origVal);
      const done = () => {
        const val = editor.value || '';
        const navTarget = cell._navTarget;
        delete cell._navTarget;
        removeEditor(cell);
        if (val !== origVal) {
          cell.dataset.rawValue = val;
          save(val || null, true);
        }
        if (navTarget) setTimeout(() => navTarget.click(), 10);
      };
      editor.addEventListener('blur', done);
      if (propType === 'date') editor.addEventListener('change', done);
      editor.addEventListener('keydown', (e) => editorKeydown(e, editor, cell, () => removeEditor(cell)));
    }
  }
}
