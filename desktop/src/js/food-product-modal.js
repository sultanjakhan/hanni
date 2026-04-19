// ── food-product-modal.js — Create/edit product modal ──
import { invoke } from './state.js';
import { CAT_LABELS, CAT_ORDER, invalidateCatalogCache } from './food-recipe-filters.js';

export function showProductModal(reloadFn, product) {
  const isEdit = !!product;
  const state = { cat: product?.category || 'other' };
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">${isEdit ? 'Редактировать продукт' : 'Новый продукт'}</div>
    <div class="form-group"><label class="form-label">Название <span class="req">*</span></label>
      <input class="form-input" id="pm-name" value="${esc(product?.name || '')}" placeholder="Название продукта"></div>
    <div class="form-group"><label class="form-label">Категория</label>
      <div class="add-chips pm-cats">${CAT_ORDER.map(c =>
        `<button type="button" class="rf-chip${c === state.cat ? ' active' : ''}" data-val="${c}">${CAT_LABELS[c]}</button>`
      ).join('')}</div></div>
    <div class="form-group"><label class="form-label">Теги</label>
      <input class="form-input" id="pm-tags" value="${esc(product?.tags || '')}" placeholder="птица, субпродукты"></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="pm-cancel">Отмена</button>
      ${isEdit ? '<button class="btn-secondary" id="pm-delete" style="color:var(--color-red)">Удалить</button>' : ''}
      <button class="btn-primary" id="pm-save">${isEdit ? 'Сохранить' : 'Создать'}</button>
    </div></div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#pm-cancel').onclick = () => overlay.remove();

  overlay.querySelectorAll('.pm-cats .rf-chip').forEach(btn => {
    btn.onclick = () => {
      state.cat = btn.dataset.val;
      overlay.querySelectorAll('.pm-cats .rf-chip').forEach(b => b.classList.toggle('active', b.dataset.val === state.cat));
    };
  });

  overlay.querySelector('#pm-save').onclick = async () => {
    const name = overlay.querySelector('#pm-name').value.trim();
    if (!name) { overlay.querySelector('#pm-name').classList.add('input-error'); return; }
    const tags = overlay.querySelector('#pm-tags').value.trim();
    try {
      if (isEdit) {
        const changes = {};
        if (name !== product.name) changes.name = name;
        if (state.cat !== product.category) changes.category = state.cat;
        if (tags !== (product.tags || '')) changes.tags = tags;
        if (Object.keys(changes).length) {
          await invoke('update_ingredient_in_catalog', { id: product.id, ...changes });
        }
      } else {
        await invoke('add_ingredient_to_catalog', { name, category: state.cat, tags });
      }
      invalidateCatalogCache();
      overlay.remove();
      if (reloadFn) await reloadFn();
    } catch (e) { alert('Ошибка: ' + e); }
  };

  if (isEdit) {
    overlay.querySelector('#pm-delete').onclick = async () => {
      try {
        const usage = await invoke('check_ingredient_usage', { ingredientName: product.name });
        if (usage.count > 0) {
          if (!confirm(`«${product.name}» используется в ${usage.count} рецептах:\n${usage.recipe_names.join(', ')}\n\nУдалить?`)) return;
        }
        await invoke('delete_ingredient_from_catalog', { id: product.id });
        invalidateCatalogCache();
        overlay.remove();
        if (reloadFn) await reloadFn();
      } catch (e) { alert('Ошибка: ' + e); }
    };
  }
}

function esc(s) { return (s || '').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }
