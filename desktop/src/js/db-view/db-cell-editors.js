// ── db-view/db-cell-editors.js — Inline cell editing (Notion-style, no popups) ──

import { invoke } from '../state.js';
import { showSelectDropdown, showMultiSelectDropdown } from './db-dropdowns.js';
import { showRecurrenceEditor } from './db-recurrence-editor.js';
import { getNextCell, getPrevCell, getCellBelow } from './db-cell-nav.js';
import { renderTimeEditor, renderProgressEditor, renderRatingEditor } from './db-type-editors.js';
import { getType } from './db-type-registry.js';
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

  const colKey = cell.dataset.editKey;
  const storageKey = `fixedOpts_${colKey}`;
  const saveCellVal = (val) => {
    cell.dispatchEvent(new CustomEvent('fixed-cell-save', {
      bubbles: true,
      detail: { recordId: cell.dataset.recordId, key: colKey, value: val },
    }));
  };

  // Load catalog from localStorage (source of truth after first edit) or keep defaults
  const loadStoredCatalog = (baseOpts) => {
    const raw = localStorage.getItem(storageKey);
    if (raw === null) return; // first use — keep defaults
    try {
      const stored = JSON.parse(raw);
      if (!Array.isArray(stored) || stored.length === 0) return;
      const isObj = typeof baseOpts[0] === 'object';
      baseOpts.length = 0;
      stored.forEach(v => baseOpts.push(isObj ? { value: v, label: v } : v));
    } catch {}
  };

  if (editType === 'select') {
    const mapped = options.map(o => typeof o === 'object' ? o : { value: o, label: o });
    loadStoredCatalog(mapped);
    const onOptsChange = (vals) => { try { localStorage.setItem(storageKey, JSON.stringify(vals)); } catch {} };
    showSelectDropdown(cell, mapped, rawVal, saveCellVal, null, onOptsChange);
    return;
  }
  if (editType === 'multi_select') {
    const optList = options.map(o => typeof o === 'object' ? o.value : o);
    const labelMap = {};
    options.forEach(o => { if (typeof o === 'object') labelMap[o.value] = o.label; });
    loadStoredCatalog(optList);
    const onOptsChange = (vals) => { try { localStorage.setItem(storageKey, JSON.stringify(vals)); } catch {} };
    showMultiSelectDropdown(cell, optList, rawVal, saveCellVal, null, labelMap, onOptsChange);
    return;
  }
  if (editType === 'recurrence') {
    showRecurrenceEditor(cell, rawVal, saveCellVal);
    return;
  }

  const inputType = editType === 'number' ? 'number' : editType === 'date' ? 'date' : editType === 'phone' ? 'tel' : 'text';
  const cleanVal = rawVal === '\u2014' ? '' : rawVal;
  const editor = overlayEditor(cell, inputType, cleanVal);
  if (inputType === 'number') { editor.step = 'any'; }

  const saveAndClose = () => {
    const val = editor.value || '';
    const navTarget = cell._navTarget;
    delete cell._navTarget;
    removeEditor(cell);
    const changed = val !== cleanVal;
    if (changed) {
      cell.dataset.rawValue = val;
      cell.dispatchEvent(new CustomEvent('fixed-cell-save', {
        bubbles: true,
        detail: { recordId: cell.dataset.recordId, key: cell.dataset.editKey, value: val },
      }));
    }
    if (navTarget) setTimeout(() => navTarget.click(), 10);
  };
  editor.addEventListener('blur', saveAndClose);
  if (inputType === 'date') editor.addEventListener('change', saveAndClose);
  editor.addEventListener('keydown', (e) => editorKeydown(e, editor, cell, () => removeEditor(cell)));
}

/** Start inline editing on a custom property cell */
export function startInlineEdit(cell, recordTable, reloadFn) {
  if (cell.querySelector('.inline-editor') || cell.querySelector('.inline-dropdown')) return;
  const propType = cell.dataset.propType;
  if (getType(propType).auto) return;
  const recordId = parseInt(cell.dataset.recordId);
  const propId = parseInt(cell.dataset.propId);
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
      showSelectDropdown(cell, options.map(o => ({ value: o, label: o })), rawVal, save, propId);
      return;
    case 'multi_select':
      showMultiSelectDropdown(cell, options, rawVal, save, propId);
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
    case 'recurrence':
      showRecurrenceEditor(cell, rawVal, (val) => { save(val); if (reloadFn) reloadFn(); });
      return;
    default: {
      const inputType = propType === 'date' ? 'date' : propType === 'number' ? 'number' : 'text';
      const origVal = rawVal || '';
      const editor = overlayEditor(cell, inputType, origVal);
      const done = () => {
        const val = editor.value || '';
        const navTarget = cell._navTarget;
        delete cell._navTarget;
        const typeDef = getType(propType);
        if (val && typeDef.validate && !typeDef.validate(val)) {
          cell.classList.add('cell-invalid');
          editor.focus();
          setTimeout(() => cell.classList.remove('cell-invalid'), 1200);
          return;
        }
        removeEditor(cell);
        if (val !== origVal) {
          cell.dataset.rawValue = val;
          save(val || null);
        }
        if (navTarget) setTimeout(() => navTarget.click(), 10);
      };
      editor.addEventListener('blur', done);
      if (propType === 'date') editor.addEventListener('change', done);
      editor.addEventListener('keydown', (e) => editorKeydown(e, editor, cell, () => removeEditor(cell)));
    }
  }
}
