// ── food-product-popup.js — Product detail popup (view/edit/delete) ──
import { invoke } from './state.js';
import { CAT_LABELS, CAT_ORDER, invalidateCatalogCache } from './food-recipe-filters.js';

export function showProductPopup(anchorEl, product, catalog, onUpdate) {
  closeAllPopups();
  const rect = anchorEl.getBoundingClientRect();
  const popup = document.createElement('div');
  popup.className = 'product-popup';
  popup.style.top = `${rect.bottom + 4}px`;
  popup.style.left = `${rect.left}px`;
  popup.innerHTML = `
    <div class="pp-path">${CAT_LABELS[product.category] || product.category} → ${product.name}</div>
    <div class="pp-field"><label class="form-label">Название</label>
      <input class="form-input pp-name" value="${esc(product.name)}"></div>
    <div class="pp-field"><label class="form-label">Категория</label>
      <div class="pp-cat-chips">${CAT_ORDER.map(c =>
        `<button class="rf-chip pp-cat-chip${c === product.category ? ' active' : ''}" data-val="${c}">${CAT_LABELS[c]}</button>`
      ).join('')}</div></div>
    <div class="pp-field"><label class="form-label">Теги</label>
      <input class="form-input pp-tags" value="${esc(product.tags || '')}" placeholder="птица, субпродукты"></div>
    <div class="pp-actions">
      <button class="btn-secondary pp-cancel">Закрыть</button>
      <button class="btn-secondary pp-delete" style="color:var(--color-red)">Удалить</button>
      <button class="btn-primary pp-save">Сохранить</button>
    </div>`;
  document.body.appendChild(popup);

  let selectedCat = product.category;
  popup.querySelectorAll('.pp-cat-chip').forEach(btn => {
    btn.onclick = () => {
      selectedCat = btn.dataset.val;
      popup.querySelectorAll('.pp-cat-chip').forEach(b => b.classList.toggle('active', b.dataset.val === selectedCat));
    };
  });

  popup.querySelector('.pp-cancel').onclick = () => popup.remove();
  popup.querySelector('.pp-save').onclick = async () => {
    const newName = popup.querySelector('.pp-name').value.trim();
    if (!newName) return;
    const newTags = popup.querySelector('.pp-tags').value.trim();
    const changes = {};
    if (newName !== product.name) changes.name = newName;
    if (selectedCat !== product.category) changes.category = selectedCat;
    if (newTags !== (product.tags || '')) changes.tags = newTags;
    if (Object.keys(changes).length) {
      try {
        await invoke('update_ingredient_in_catalog', { id: product.id, ...changes });
        invalidateCatalogCache();
        const idx = catalog.findIndex(c => c.id === product.id);
        if (idx >= 0) { if (changes.name) catalog[idx].name = changes.name; if (changes.category) catalog[idx].category = changes.category; if (changes.tags !== undefined) catalog[idx].tags = changes.tags; }
        if (onUpdate) onUpdate();
      } catch (e) { alert('Ошибка: ' + e); }
    }
    popup.remove();
  };

  popup.querySelector('.pp-delete').onclick = async () => {
    try {
      const usage = await invoke('check_ingredient_usage', { ingredientName: product.name });
      if (usage.count > 0) {
        if (!confirm(`«${product.name}» используется в ${usage.count} рецептах:\n${usage.recipe_names.join(', ')}\n\nУдалить?`)) return;
      }
      await invoke('delete_ingredient_from_catalog', { id: product.id });
      invalidateCatalogCache();
      const idx = catalog.findIndex(c => c.id === product.id);
      if (idx >= 0) catalog.splice(idx, 1);
      if (onUpdate) onUpdate();
      popup.remove();
    } catch (e) { alert('Ошибка: ' + e); }
  };

  const onClickOutside = (e) => {
    if (!popup.contains(e.target)) { popup.remove(); document.removeEventListener('mousedown', onClickOutside); }
  };
  setTimeout(() => document.addEventListener('mousedown', onClickOutside), 10);
}

function closeAllPopups() {
  document.querySelectorAll('.product-popup').forEach(p => p.remove());
}
function esc(s) { return s.replace(/"/g, '&quot;').replace(/</g, '&lt;'); }
