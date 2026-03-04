// ── db-view/db-properties.js — Property add/edit/delete modals ──

import { invoke, PROPERTY_TYPE_DEFS, getTypeIcon, getTypeName } from '../state.js';
import { escapeHtml, confirmModal } from '../utils.js';

/** Show the "Add property" modal for a tab */
export function showAddPropertyModal(tabId, reloadFn) {
  let selectedType = 'text';
  let optionsList = [];

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';

  function renderModal() {
    const needsOptions = ['select', 'multi_select'].includes(selectedType);
    const typeGrid = PROPERTY_TYPE_DEFS.map(t =>
      `<div class="prop-type-card${t.id === selectedType ? ' selected' : ''}" data-type="${t.id}">
        <div class="prop-type-icon">${t.icon}</div>
        <div class="prop-type-name">${t.name}</div>
      </div>`
    ).join('');

    const optionsHtml = needsOptions ? `
      <div class="prop-config-section">
        <div class="prop-section-label">Варианты</div>
        <div class="prop-options-container">
          <div class="prop-options-tags" id="dbv-prop-tags">
            ${optionsList.map((o, i) => `<span class="prop-option-tag">${escapeHtml(o)}<span class="prop-option-tag-remove" data-idx="${i}">&times;</span></span>`).join('')}
          </div>
          <div class="prop-option-add">
            <input id="dbv-prop-option-input" type="text" placeholder="Новый вариант..." autocomplete="off">
            <button id="dbv-prop-option-add-btn">+</button>
          </div>
        </div>
      </div>` : '';

    overlay.innerHTML = `<div class="modal modal-property">
      <div class="modal-title">Новое свойство</div>
      <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="dbv-prop-name" placeholder="Без названия" autocomplete="off"></div>
      <div class="form-group">
        <label class="form-label">Тип</label>
        <div class="prop-type-grid">${typeGrid}</div>
      </div>
      ${optionsHtml}
      <div class="modal-actions">
        <button class="btn-secondary" id="dbv-prop-cancel">Отмена</button>
        <button class="btn-primary" id="dbv-prop-save">Добавить</button>
      </div>
    </div>`;

    // Bind type selection
    overlay.querySelectorAll('.prop-type-card').forEach(card => {
      card.addEventListener('click', () => {
        selectedType = card.dataset.type;
        const nameVal = document.getElementById('dbv-prop-name')?.value || '';
        renderModal();
        const nameInput = document.getElementById('dbv-prop-name');
        if (nameInput) nameInput.value = nameVal;
      });
    });

    // Bind option tag removal
    overlay.querySelectorAll('.prop-option-tag-remove').forEach(btn => {
      btn.addEventListener('click', () => {
        const idx = parseInt(btn.dataset.idx);
        optionsList.splice(idx, 1);
        const nameVal = document.getElementById('dbv-prop-name')?.value || '';
        renderModal();
        document.getElementById('dbv-prop-name').value = nameVal;
      });
    });

    // Bind add option
    const addOptBtn = overlay.querySelector('#dbv-prop-option-add-btn');
    const addOptInput = overlay.querySelector('#dbv-prop-option-input');
    const addOption = () => {
      const val = addOptInput?.value?.trim();
      if (val && !optionsList.includes(val)) {
        optionsList.push(val);
        const nameVal = document.getElementById('dbv-prop-name')?.value || '';
        renderModal();
        document.getElementById('dbv-prop-name').value = nameVal;
        document.getElementById('dbv-prop-option-input')?.focus();
      }
    };
    addOptBtn?.addEventListener('click', addOption);
    addOptInput?.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); addOption(); } });

    // Bind cancel
    overlay.querySelector('#dbv-prop-cancel')?.addEventListener('click', () => overlay.remove());

    // Bind save
    overlay.querySelector('#dbv-prop-save')?.addEventListener('click', async () => {
      const name = document.getElementById('dbv-prop-name')?.value?.trim() || 'Без названия';
      let options = null;
      if (['select', 'multi_select'].includes(selectedType) && optionsList.length > 0) {
        options = JSON.stringify(optionsList);
      }
      try {
        await invoke('create_property_definition', { tabId, name, propType: selectedType, position: null, color: null, options, defaultValue: null });
        overlay.remove();
        if (reloadFn) reloadFn();
      } catch (err) { alert('Error: ' + err); }
    });
  }

  renderModal();
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  setTimeout(() => document.getElementById('dbv-prop-name')?.focus(), 50);
}

/** Show column context menu for a property header */
export function showColumnMenu(propDef, anchorRect, tabId, reloadFn, sortCallback) {
  // Remove any existing menu
  document.querySelectorAll('.col-context-menu').forEach(m => m.remove());

  const menu = document.createElement('div');
  menu.className = 'col-context-menu';

  menu.innerHTML = `
    <div class="col-menu-section">
      <input class="col-menu-name-input" value="${escapeHtml(propDef.name)}" id="dbv-col-rename-input" autocomplete="off">
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="type">
        <span class="col-menu-icon">${getTypeIcon(propDef.type)}</span>
        <span>${getTypeName(propDef.type)}</span>
      </div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="sort-asc">
        <span class="col-menu-icon">\u2191</span>
        <span>Сортировка А\u2192Я</span>
      </div>
      <div class="col-menu-item" data-action="sort-desc">
        <span class="col-menu-icon">\u2193</span>
        <span>Сортировка Я\u2192А</span>
      </div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="hide">
        <span class="col-menu-icon">\u25fb</span>
        <span>Скрыть</span>
      </div>
      <div class="col-menu-item col-menu-item danger" data-action="delete">
        <span class="col-menu-icon">\u2715</span>
        <span>Удалить</span>
      </div>
    </div>
  `;

  menu.style.left = Math.min(anchorRect.left, window.innerWidth - 220) + 'px';
  menu.style.top = anchorRect.bottom + 4 + 'px';
  document.body.appendChild(menu);

  // Rename on Enter/blur
  const renameInput = menu.querySelector('#dbv-col-rename-input');
  const doRename = async () => {
    const newName = renameInput.value.trim();
    if (newName && newName !== propDef.name) {
      try {
        await invoke('update_property_definition', { id: propDef.id, name: newName, propType: null, position: null, color: null, options: null, visible: null });
        if (reloadFn) reloadFn();
      } catch {}
    }
  };
  renameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); doRename(); menu.remove(); }
    if (e.key === 'Escape') { menu.remove(); }
    e.stopPropagation();
  });
  renameInput.addEventListener('blur', () => { doRename(); });

  // Menu item clicks
  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', async () => {
      const action = item.dataset.action;
      switch (action) {
        case 'sort-asc':
        case 'sort-desc': {
          const dir = action === 'sort-asc' ? 'asc' : 'desc';
          if (sortCallback) sortCallback(`prop_${propDef.id}`, dir);
          menu.remove();
          break;
        }
        case 'hide':
          try {
            await invoke('update_property_definition', { id: propDef.id, name: null, propType: null, position: null, color: null, options: null, visible: false });
            if (reloadFn) reloadFn();
          } catch {}
          menu.remove();
          break;
        case 'delete':
          if (await confirmModal(`Удалить свойство "${propDef.name}"?`)) {
            try {
              await invoke('delete_property_definition', { id: propDef.id });
              if (reloadFn) reloadFn();
            } catch {}
          }
          menu.remove();
          break;
      }
    });
  });

  // Close on outside click
  const closeMenu = (e) => {
    if (!menu.contains(e.target)) {
      doRename();
      menu.remove();
      document.removeEventListener('mousedown', closeMenu);
    }
  };
  setTimeout(() => document.addEventListener('mousedown', closeMenu), 10);
}
