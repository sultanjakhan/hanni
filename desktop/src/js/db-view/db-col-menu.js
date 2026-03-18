import { invoke, PROPERTY_TYPE_DEFS, getTypeIcon, getTypeName } from '../state.js';
import { escapeHtml, confirmModal } from '../utils.js';

export function showColumnMenu(propDef, anchorRect, tabId, reloadFn, sortCallback) {
  document.querySelectorAll('.col-context-menu').forEach(m => m.remove());
  const menu = document.createElement('div');
  menu.className = 'col-context-menu';
  menu.innerHTML = `
    <div class="col-menu-section">
      <input class="col-menu-name-input" value="${escapeHtml(propDef.name)}" id="dbv-col-rename-input" autocomplete="off">
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="type"><span class="col-menu-icon">${getTypeIcon(propDef.type)}</span><span>${getTypeName(propDef.type)}</span></div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="sort-asc"><span class="col-menu-icon">↑</span><span>Сортировка А→Я</span></div>
      <div class="col-menu-item" data-action="sort-desc"><span class="col-menu-icon">↓</span><span>Сортировка Я→А</span></div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="wrap"><span class="col-menu-icon">↩</span><span>Перенос текста</span></div>
      <div class="col-menu-item" data-action="hide"><span class="col-menu-icon">◻</span><span>Скрыть</span></div>
      <div class="col-menu-item col-menu-item danger" data-action="delete"><span class="col-menu-icon">✕</span><span>Удалить</span></div>
    </div>`;

  menu.style.left = Math.min(anchorRect.left, window.innerWidth - 220) + 'px';
  menu.style.top = anchorRect.bottom + 4 + 'px';
  document.body.appendChild(menu);

  const renameInput = menu.querySelector('#dbv-col-rename-input');
  const doRename = async () => {
    const n = renameInput.value.trim();
    if (n && n !== propDef.name) await invoke('update_property_definition', { id: propDef.id, name: n }).catch(() => {});
  };
  renameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); doRename(); menu.remove(); if (reloadFn) reloadFn(); }
    if (e.key === 'Escape') menu.remove();
    e.stopPropagation();
  });
  renameInput.addEventListener('blur', doRename);

  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', async () => {
      const action = item.dataset.action;
      switch (action) {
        case 'type': {
          const types = PROPERTY_TYPE_DEFS.filter(t => !t.auto && t.id !== propDef.type);
          const sub = document.createElement('div');
          sub.className = 'col-context-menu col-type-submenu';
          sub.innerHTML = types.map(t => `<div class="col-menu-item" data-type-id="${t.id}"><span class="col-menu-icon">${t.icon}</span>${t.name}</div>`).join('');
          sub.style.left = (parseInt(menu.style.left) + 200) + 'px';
          sub.style.top = menu.style.top;
          document.body.appendChild(sub);
          sub.querySelectorAll('.col-menu-item').forEach(ti => {
            ti.addEventListener('click', async () => {
              await invoke('update_property_definition', { id: propDef.id, propType: ti.dataset.typeId }).catch(() => {});
              sub.remove(); menu.remove(); if (reloadFn) reloadFn();
            });
          });
          setTimeout(() => document.addEventListener('mousedown', (e) => { if (!sub.contains(e.target)) sub.remove(); }, { once: true }), 10);
          break;
        }
        case 'sort-asc': case 'sort-desc':
          if (sortCallback) sortCallback(`prop_${propDef.id}`, action === 'sort-asc' ? 'asc' : 'desc');
          menu.remove(); break;
        case 'wrap': {
          const col = document.querySelectorAll(`td[data-prop-id="${propDef.id}"]`);
          col.forEach(td => td.classList.toggle('cell-wrap'));
          menu.remove(); break;
        }
        case 'hide':
          await invoke('update_property_definition', { id: propDef.id, visible: false }).catch(() => {});
          menu.remove(); if (reloadFn) reloadFn(); break;
        case 'delete':
          if (await confirmModal(`Удалить свойство "${propDef.name}"?`)) {
            await invoke('delete_property_definition', { id: propDef.id }).catch(() => {});
            if (reloadFn) reloadFn();
          }
          menu.remove(); break;
      }
    });
  });

  const closeMenu = (e) => {
    if (!menu.contains(e.target)) { doRename(); menu.remove(); document.removeEventListener('mousedown', closeMenu); }
  };
  setTimeout(() => document.addEventListener('mousedown', closeMenu), 10);
}
