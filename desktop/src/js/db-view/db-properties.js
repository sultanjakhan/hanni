// ── db-view/db-properties.js — Column context menus (Notion-style) ──

import { invoke, PROPERTY_TYPE_DEFS, getTypeIcon, getTypeName } from '../state.js';
import { escapeHtml, confirmModal } from '../utils.js';
import { getHiddenFixedCols, setHiddenFixedCols, getDeletedFixedCols, addDeletedFixedCol, getFixedColName, setFixedColName, clearColumnHighlight, isColumnWrapped, toggleWrap } from './db-col-state.js';
import { showAddPropertyPopover } from './db-add-property.js';

export { showAddPropertyPopover } from './db-add-property.js';
export { getHiddenFixedCols, getDeletedFixedCols, addDeletedFixedCol, getFixedColName, getColumnOrder, setColumnOrder, isColumnWrapped, highlightColumn, clearColumnHighlight, buildUnifiedColumns } from './db-col-state.js';

/** Show column menu for a custom property */
export function showColumnMenu(propDef, anchorRect, tabId, reloadFn, sortCallback, filterCallback) {
  const colId = `prop_${propDef.id}`;
  const wrapped = isColumnWrapped(tabId, colId);
  const menu = openMenu(anchorRect, buildMenuHTML({
    name: propDef.name, typeIcon: getTypeIcon(propDef.type), typeName: getTypeName(propDef.type),
    canChangeType: true, wrapped, canHide: true, canInsert: true, canDelete: true,
  }));

  wireRename(menu, async (newName) => {
    if (newName !== propDef.name) { try { await invoke('update_property_definition', { id: propDef.id, name: newName, propType: null, position: null, color: null, options: null, visible: null }); if (reloadFn) reloadFn(); } catch {} }
  });

  wireActions(menu, {
    'change-type': () => { menu.remove(); showTypeChanger(propDef, anchorRect, tabId, reloadFn); },
    'sort-asc': () => { if (sortCallback) sortCallback(`prop_${propDef.id}`, 'asc'); menu.remove(); },
    'sort-desc': () => { if (sortCallback) sortCallback(`prop_${propDef.id}`, 'desc'); menu.remove(); },
    'filter': () => { menu.remove(); if (filterCallback) filterCallback(`prop_${propDef.id}`); },
    'wrap': () => { toggleWrap(tabId, colId); if (reloadFn) reloadFn(); menu.remove(); },
    'hide': () => { invoke('update_property_definition', { id: propDef.id, name: null, propType: null, position: null, color: null, options: null, visible: false }).then(() => { if (reloadFn) reloadFn(); }).catch(() => {}); menu.remove(); },
    'insert-left': () => { menu.remove(); showAddPropertyPopover(tabId, menu, reloadFn); },
    'insert-right': () => { menu.remove(); showAddPropertyPopover(tabId, menu, reloadFn); },
    'delete': async () => { menu.remove(); if (await confirmModal(`Удалить свойство "${propDef.name}"?`)) { try { await invoke('delete_property_definition', { id: propDef.id }); if (reloadFn) reloadFn(); } catch {} } },
  });
  wireClose(menu);
}

/** Show column menu for a fixed column */
export function showFixedColumnMenu(colKey, colLabel, anchorRect, tabId, reloadFn, sortCallback, filterCallback) {
  const displayName = getFixedColName(tabId, colKey, colLabel || colKey);
  const wrapped = isColumnWrapped(tabId, colKey);
  const menu = openMenu(anchorRect, buildMenuHTML({
    name: displayName, typeIcon: getTypeIcon('text'), typeName: 'Текст',
    canChangeType: false, wrapped, canHide: true, canInsert: false, canDelete: true,
  }));

  wireRename(menu, (newName) => {
    if (newName && newName !== (colLabel || colKey)) { setFixedColName(tabId, colKey, newName); if (reloadFn) reloadFn(); }
    else if (!newName || newName === (colLabel || colKey)) { setFixedColName(tabId, colKey, null); }
  });

  wireActions(menu, {
    'sort-asc': () => { if (sortCallback) sortCallback(colKey, 'asc'); menu.remove(); },
    'sort-desc': () => { if (sortCallback) sortCallback(colKey, 'desc'); menu.remove(); },
    'filter': () => { menu.remove(); if (filterCallback) filterCallback(colKey); },
    'wrap': () => { toggleWrap(tabId, colKey); if (reloadFn) reloadFn(); menu.remove(); },
    'hide': async () => { menu.remove(); const hidden = getHiddenFixedCols(tabId); if (!hidden.includes(colKey)) { hidden.push(colKey); await setHiddenFixedCols(tabId, hidden); } if (reloadFn) reloadFn(); },
    'delete': async () => { menu.remove(); if (await confirmModal(`Удалить столбец "${displayName}"? Данные будут потеряны.`)) { await addDeletedFixedCol(tabId, colKey); if (reloadFn) reloadFn(); } },
  });
  wireClose(menu);
}

// ── Internal helpers ──

function buildMenuHTML({ name, typeIcon, typeName, canChangeType, wrapped, canHide, canInsert, canDelete }) {
  const typeClass = canChangeType ? '' : ' disabled';
  return `
    <div class="col-menu-section">
      <input class="col-menu-name-input" value="${escapeHtml(name)}" autocomplete="off">
      <div class="col-menu-item${typeClass}" data-action="change-type"><span class="col-menu-icon">${typeIcon}</span>Тип: ${typeName}</div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="sort-asc"><span class="col-menu-icon">↑</span>Сортировка А→Я</div>
      <div class="col-menu-item" data-action="sort-desc"><span class="col-menu-icon">↓</span>Сортировка Я→А</div>
      <div class="col-menu-item" data-action="filter"><span class="col-menu-icon">⫧</span>Фильтр</div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item toggle" data-action="wrap"><span class="col-menu-icon">↩</span>Перенос текста<span class="col-menu-toggle${wrapped ? ' on' : ''}"></span></div>
      ${canHide ? '<div class="col-menu-item" data-action="hide"><span class="col-menu-icon">◻</span>Скрыть</div>' : ''}
    </div>
    ${canInsert ? `<div class="col-menu-section">
      <div class="col-menu-item" data-action="insert-left"><span class="col-menu-icon">←</span>Вставить слева</div>
      <div class="col-menu-item" data-action="insert-right"><span class="col-menu-icon">→</span>Вставить справа</div>
    </div>` : ''}
    ${canDelete ? '<div class="col-menu-section"><div class="col-menu-item danger" data-action="delete"><span class="col-menu-icon">✕</span>Удалить</div></div>' : ''}`;
}

function openMenu(anchorRect, html) {
  document.querySelectorAll('.col-context-menu').forEach(m => m.remove());
  const menu = document.createElement('div');
  menu.className = 'col-context-menu';
  menu.innerHTML = html;
  document.body.appendChild(menu);
  const mH = menu.offsetHeight, mW = menu.offsetWidth;
  const top = anchorRect.bottom + 4 + mH > window.innerHeight ? Math.max(4, anchorRect.top - mH - 4) : anchorRect.bottom + 4;
  menu.style.left = Math.min(anchorRect.left, window.innerWidth - mW - 8) + 'px';
  menu.style.top = top + 'px';
  return menu;
}

function wireRename(menu, callback) {
  const input = menu.querySelector('.col-menu-name-input');
  if (!input) return;
  const doIt = () => callback(input.value.trim());
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); doIt(); menu.remove(); } if (e.key === 'Escape') menu.remove(); e.stopPropagation(); });
  input.addEventListener('blur', doIt);
}

function wireActions(menu, handlers) {
  menu.querySelectorAll('.col-menu-item:not(.disabled)').forEach(item => {
    item.addEventListener('click', () => { const fn = handlers[item.dataset.action]; if (fn) fn(); });
  });
}

function wireClose(menu) {
  const close = (e) => {
    if (!menu.contains(e.target)) { menu.remove(); clearColumnHighlight(document.querySelector('.data-table')); document.removeEventListener('mousedown', close); }
  };
  setTimeout(() => document.addEventListener('mousedown', close), 10);
}

function showTypeChanger(propDef, anchorRect, tabId, reloadFn) {
  const menu = document.createElement('div');
  menu.className = 'col-context-menu';
  menu.innerHTML = PROPERTY_TYPE_DEFS.map(t =>
    `<div class="col-menu-item${t.id === propDef.type ? ' active' : ''}" data-type="${t.id}"><span class="col-menu-icon">${t.icon}</span><span>${t.name}</span></div>`
  ).join('');
  document.body.appendChild(menu);
  const mW = menu.offsetWidth, mH = menu.offsetHeight;
  let left = anchorRect.left + 220;
  if (left + mW > window.innerWidth) left = Math.max(4, anchorRect.left - mW - 4);
  let top = anchorRect.bottom + 4;
  if (top + mH > window.innerHeight) top = Math.max(4, anchorRect.top - mH - 4);
  menu.style.left = left + 'px';
  menu.style.top = top + 'px';
  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', async () => {
      if (item.dataset.type !== propDef.type) {
        if (!await confirmModal('Смена типа может привести к потере данных. Продолжить?')) { menu.remove(); return; }
        try { await invoke('update_property_definition', { id: propDef.id, name: null, propType: item.dataset.type, position: null, color: null, options: null, visible: null }); if (reloadFn) reloadFn(); } catch {}
      }
      menu.remove();
    });
  });
  setTimeout(() => { const close = (e) => { if (!menu.contains(e.target)) { menu.remove(); document.removeEventListener('mousedown', close); } }; document.addEventListener('mousedown', close); }, 10);
}
