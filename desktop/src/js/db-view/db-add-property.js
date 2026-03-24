// ── db-view/db-add-property.js — Add property popover ──

import { invoke, PROPERTY_TYPE_DEFS } from '../state.js';

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
      try {
        await invoke('create_property_definition', { tabId, name, propType: item.dataset.type, position: null, color: null, options: null, defaultValue: null });
        pop.remove();
        if (reloadFn) reloadFn();
      } catch (err) { console.error('Property error:', err); }
    });
  });
  nameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      invoke('create_property_definition', { tabId, name: nameInput.value.trim() || 'Без названия', propType: 'text', position: null, color: null, options: null, defaultValue: null })
        .then(() => { pop.remove(); if (reloadFn) reloadFn(); }).catch(() => {});
    }
    if (e.key === 'Escape') pop.remove();
    e.stopPropagation();
  });
  setTimeout(() => {
    const close = (e) => { if (!pop.contains(e.target)) { pop.remove(); document.removeEventListener('mousedown', close); } };
    document.addEventListener('mousedown', close);
  }, 10);
}
