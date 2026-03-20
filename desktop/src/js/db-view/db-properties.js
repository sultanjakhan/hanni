// ── db-view/db-properties.js — Property add/edit/delete (Notion-style) ──

import { invoke, PROPERTY_TYPE_DEFS, getTypeIcon, getTypeName } from '../state.js';
import { escapeHtml, confirmModal } from '../utils.js';

/** Show a compact popover to add a new property */
export function showAddPropertyPopover(tabId, anchorEl, reloadFn) {
  document.querySelectorAll('.prop-add-popover').forEach(p => p.remove());
  const pop = document.createElement('div');
  pop.className = 'prop-add-popover';
  const typesHtml = PROPERTY_TYPE_DEFS.map(t =>
    `<div class="prop-add-type-item" data-type="${t.id}"><span class="prop-add-type-icon">${t.icon}</span><span>${t.name}</span></div>`
  ).join('');
  pop.innerHTML = `<input class="prop-add-name-input" placeholder="Название свойства..." autocomplete="off"><div class="prop-add-type-list">${typesHtml}</div>`;
  const rect = anchorEl.getBoundingClientRect();
  pop.style.left = Math.min(rect.left, window.innerWidth - 200) + 'px';
  pop.style.top = (rect.bottom + 4) + 'px';
  document.body.appendChild(pop);
  const nameInput = pop.querySelector('.prop-add-name-input');
  nameInput.focus();
  pop.querySelectorAll('.prop-add-type-item').forEach(item => {
    item.addEventListener('click', async () => {
      const name = nameInput.value.trim() || 'Без названия';
      try { await invoke('create_property_definition', { tabId, name, propType: item.dataset.type, position: null, color: null, options: null, defaultValue: null }); pop.remove(); if (reloadFn) reloadFn(); } catch (err) { console.error('Property error:', err); }
    });
  });
  nameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); const name = nameInput.value.trim() || 'Без названия'; invoke('create_property_definition', { tabId, name, propType: 'text', position: null, color: null, options: null, defaultValue: null }).then(() => { pop.remove(); if (reloadFn) reloadFn(); }).catch(err => console.error('Property error:', err)); }
    if (e.key === 'Escape') pop.remove();
    e.stopPropagation();
  });
  setTimeout(() => { const close = (e) => { if (!pop.contains(e.target)) { pop.remove(); document.removeEventListener('mousedown', close); } }; document.addEventListener('mousedown', close); }, 10);
}

/** Show column context menu for a property header */
export function showColumnMenu(propDef, anchorRect, tabId, reloadFn, sortCallback) {
  document.querySelectorAll('.col-context-menu').forEach(m => m.remove());
  const isWrapped = propDef._wrap;
  const menu = document.createElement('div');
  menu.className = 'col-context-menu';
  menu.innerHTML = `
    <div class="col-menu-section"><input class="col-menu-name-input" value="${escapeHtml(propDef.name)}" id="dbv-col-rename-input" autocomplete="off"></div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="type"><span class="col-menu-icon">${getTypeIcon(propDef.type)}</span><span>Тип: ${getTypeName(propDef.type)}</span></div>
      <div class="col-menu-item" data-action="change-type"><span class="col-menu-icon">⇄</span><span>Изменить тип</span></div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="sort-asc"><span class="col-menu-icon">↑</span><span>Сортировка А→Я</span></div>
      <div class="col-menu-item" data-action="sort-desc"><span class="col-menu-icon">↓</span><span>Сортировка Я→А</span></div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="duplicate"><span class="col-menu-icon">⧉</span><span>Дубликат</span></div>
      <div class="col-menu-item" data-action="wrap"><span class="col-menu-icon">${isWrapped ? '☑' : '☐'}</span><span>Перенос текста</span></div>
      <div class="col-menu-item" data-action="hide"><span class="col-menu-icon">◻</span><span>Скрыть</span></div>
      <div class="col-menu-item danger" data-action="delete"><span class="col-menu-icon">✕</span><span>Удалить</span></div>
    </div>`;
  menu.style.left = Math.min(anchorRect.left, window.innerWidth - 220) + 'px';
  menu.style.top = anchorRect.bottom + 4 + 'px';
  document.body.appendChild(menu);

  const renameInput = menu.querySelector('#dbv-col-rename-input');
  const doRename = async () => {
    const newName = renameInput.value.trim();
    if (newName && newName !== propDef.name) {
      try { await invoke('update_property_definition', { id: propDef.id, name: newName, propType: null, position: null, color: null, options: null, visible: null }); if (reloadFn) reloadFn(); } catch {}
    }
  };
  renameInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); doRename(); menu.remove(); } if (e.key === 'Escape') menu.remove(); e.stopPropagation(); });
  renameInput.addEventListener('blur', doRename);

  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', async () => {
      const action = item.dataset.action;
      if (action === 'sort-asc' || action === 'sort-desc') {
        if (sortCallback) sortCallback(`prop_${propDef.id}`, action === 'sort-asc' ? 'asc' : 'desc');
        menu.remove();
      } else if (action === 'change-type') {
        menu.remove();
        showTypeChanger(propDef, anchorRect, tabId, reloadFn);
      } else if (action === 'duplicate') {
        try { await invoke('create_property_definition', { tabId, name: propDef.name + ' (копия)', propType: propDef.type, position: null, color: null, options: propDef.options, defaultValue: null }); if (reloadFn) reloadFn(); } catch {}
        menu.remove();
      } else if (action === 'wrap') {
        toggleWrap(tabId, propDef.id);
        if (reloadFn) reloadFn();
        menu.remove();
      } else if (action === 'hide') {
        try { await invoke('update_property_definition', { id: propDef.id, name: null, propType: null, position: null, color: null, options: null, visible: false }); if (reloadFn) reloadFn(); } catch {}
        menu.remove();
      } else if (action === 'delete') {
        if (await confirmModal(`Удалить свойство "${propDef.name}"?`)) { try { await invoke('delete_property_definition', { id: propDef.id }); if (reloadFn) reloadFn(); } catch {} }
        menu.remove();
      }
    });
  });

  const closeMenu = (e) => { if (!menu.contains(e.target)) { doRename(); menu.remove(); document.removeEventListener('mousedown', closeMenu); } };
  setTimeout(() => document.addEventListener('mousedown', closeMenu), 10);
}

function showTypeChanger(propDef, anchorRect, tabId, reloadFn) {
  const menu = document.createElement('div');
  menu.className = 'col-context-menu';
  menu.innerHTML = PROPERTY_TYPE_DEFS.map(t =>
    `<div class="col-menu-item${t.id === propDef.type ? ' active' : ''}" data-type="${t.id}"><span class="col-menu-icon">${t.icon}</span><span>${t.name}</span></div>`
  ).join('');
  menu.style.left = (anchorRect.left + 220) + 'px';
  menu.style.top = anchorRect.bottom + 4 + 'px';
  document.body.appendChild(menu);
  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', async () => {
      const newType = item.dataset.type;
      if (newType !== propDef.type) {
        try { await invoke('update_property_definition', { id: propDef.id, name: null, propType: newType, position: null, color: null, options: null, visible: null }); if (reloadFn) reloadFn(); } catch {}
      }
      menu.remove();
    });
  });
  setTimeout(() => { const close = (e) => { if (!menu.contains(e.target)) { menu.remove(); document.removeEventListener('mousedown', close); } }; document.addEventListener('mousedown', close); }, 10);
}

function toggleWrap(tabId, propId) {
  if (!window._dbvWrap) window._dbvWrap = {};
  if (!window._dbvWrap[tabId]) window._dbvWrap[tabId] = {};
  window._dbvWrap[tabId][propId] = !window._dbvWrap[tabId][propId];
}

/** Check if column should wrap */
export function isColumnWrapped(tabId, propId) {
  return window._dbvWrap?.[tabId]?.[propId] || false;
}
