// ── food-product-modal.js — Create/edit product modal ──
import { invoke } from './state.js';
import { CAT_LABELS, CAT_ORDER, invalidateCatalogCache, loadCatalog } from './food-recipe-filters.js';
import { HIERARCHICAL_CATS } from './food-product-views.js';

export function showProductModal(reloadFn, product, defaults = {}) {
  const isEdit = !!product;
  const state = { cat: product?.category || defaults.category || 'other' };
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
    <div class="form-group" id="pm-parent-wrap" style="display:none">
      <label class="form-label">Разновидность чего? <span style="color:var(--text-secondary);font-size:11px">(необязательно)</span></label>
      <select class="form-input" id="pm-parent">
        <option value="">— нет (самостоятельный продукт) —</option>
      </select></div>
    <div class="form-group"><label class="form-label">Подгруппа</label>
      <input class="form-input" id="pm-subgroup" value="${esc(product?.subgroup || defaults.subgroup || '')}" placeholder="например: говядина, курица, кисломолочные" list="pm-subgroup-list">
      <datalist id="pm-subgroup-list"></datalist></div>
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

  const dl = overlay.querySelector('#pm-subgroup-list');
  async function refreshSubgroups() {
    try {
      const rows = await invoke('list_catalog_subgroups', { category: state.cat });
      dl.innerHTML = rows.filter(r => r.name).map(r => `<option value="${esc(r.name)}">`).join('');
    } catch {}
  }
  refreshSubgroups();

  async function refreshParents() {
    const wrap = overlay.querySelector('#pm-parent-wrap');
    const sel = overlay.querySelector('#pm-parent');
    if (!HIERARCHICAL_CATS.has(state.cat)) {
      wrap.style.display = 'none';
      sel.value = '';
      return;
    }
    wrap.style.display = '';
    let cat;
    try { cat = await loadCatalog(); } catch { cat = []; }
    const candidates = cat.filter(p =>
      p.category === state.cat
      && !p.parent_id
      && (!product || p.id !== product.id)
    ).sort((a, b) => a.name.localeCompare(b.name, 'ru'));
    const currentParentId = product?.parent_id ?? '';
    sel.innerHTML = '<option value="">— нет (самостоятельный продукт) —</option>'
      + candidates.map(c =>
          `<option value="${c.id}"${c.id === currentParentId ? ' selected' : ''}>${esc(c.name)}</option>`
        ).join('');
  }
  refreshParents();

  overlay.querySelectorAll('.pm-cats .rf-chip').forEach(btn => {
    btn.onclick = () => {
      state.cat = btn.dataset.val;
      overlay.querySelectorAll('.pm-cats .rf-chip').forEach(b => b.classList.toggle('active', b.dataset.val === state.cat));
      refreshSubgroups();
      refreshParents();
    };
  });

  overlay.querySelector('#pm-save').onclick = async () => {
    const name = overlay.querySelector('#pm-name').value.trim();
    if (!name) { overlay.querySelector('#pm-name').classList.add('input-error'); return; }
    const tags = overlay.querySelector('#pm-tags').value.trim();
    const subgroup = overlay.querySelector('#pm-subgroup').value.trim();
    const parentRaw = overlay.querySelector('#pm-parent').value;
    const parentId = (HIERARCHICAL_CATS.has(state.cat) && parentRaw) ? Number(parentRaw) : null;
    try {
      if (isEdit) {
        const changes = {};
        if (name !== product.name) changes.name = name;
        if (state.cat !== product.category) changes.category = state.cat;
        if (tags !== (product.tags || '')) changes.tags = tags;
        if (subgroup !== (product.subgroup || '')) changes.subgroup = subgroup;
        const oldParent = product.parent_id ?? null;
        if (parentId !== oldParent) {
          if (parentId === null) changes.clearParent = true;
          else changes.parentId = parentId;
        }
        if (Object.keys(changes).length) {
          await invoke('update_ingredient_in_catalog', { id: product.id, ...changes });
        }
      } else {
        const args = { name, category: state.cat, tags, subgroup };
        if (parentId !== null) args.parentId = parentId;
        await invoke('add_ingredient_to_catalog', args);
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
