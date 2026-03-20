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
      } catch (err) { console.error('Property error:', err); }
    });
  }

  renderModal();
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  setTimeout(() => document.getElementById('dbv-prop-name')?.focus(), 50);
}

// Re-export column menu from its own module
export { showColumnMenu } from './db-col-menu.js';
