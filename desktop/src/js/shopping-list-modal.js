// shopping-list-modal.js — Two UIs over the same shopping_list:
//   showShoppingPicker(onPick)  — multi-select for the Закупка template
//   showShoppingManager()        — standalone CRUD modal
//
// Both modals share rendering so the list looks identical wherever it
// appears. Picker returns the selected items and (optionally) marks them
// bought_at when the event is saved upstream.

import { escapeHtml } from './utils.js';
import { invoke } from './state.js';
import {
  listShoppingItems, addShoppingItem, deleteShoppingItem, markBought,
} from './shopping-list.js';

// "2 шт" / "500 г" → { quantity, unit } for the fridge product.
function parseQty(qtyStr) {
  const m = String(qtyStr || '').trim().match(/^([\d.,]+)\s*(.*)$/);
  if (m) return { quantity: parseFloat(m[1].replace(',', '.')) || 1, unit: (m[2] || '').trim() || 'шт' };
  return { quantity: 1, unit: 'шт' };
}

function rowHtml(item, mode, selected) {
  const checked = selected ? ' checked' : '';
  const qty = item.qty ? `<span class="sl-qty">${escapeHtml(item.qty)}</span>` : '';
  const note = item.note ? `<span class="sl-note">${escapeHtml(item.note)}</span>` : '';
  const picker = mode === 'picker'
    ? `<label class="sl-check"><input type="checkbox" data-id="${item.id}"${checked}></label>`
    : '';
  const buy = mode === 'manage'
    ? `<button class="sl-buy" data-buy="${item.id}" title="Куплено → в холодильник" style="font-size:12px;color:var(--color-green);background:none;border:none;cursor:pointer;white-space:nowrap">✓ в холодильник</button>`
    : '';
  const del = mode === 'manage'
    ? `<button class="sl-del" data-del="${item.id}" title="Удалить">×</button>`
    : '';
  return `<div class="sl-row" data-row="${item.id}">
    ${picker}
    <div class="sl-main">
      <div class="sl-name">${escapeHtml(item.name)}</div>
      ${(qty || note) ? `<div class="sl-meta">${qty}${note}</div>` : ''}
    </div>
    ${buy}${del}
  </div>`;
}

async function renderList(container, mode, preselectedIds = new Set()) {
  const items = await listShoppingItems(false);
  if (!items.length) {
    container.innerHTML = '<div class="sl-empty">Список покупок пуст. Добавьте товар сверху.</div>';
    return items;
  }
  container.innerHTML = items.map(i => rowHtml(i, mode, preselectedIds.has(i.id))).join('');
  if (mode === 'manage') {
    container.querySelectorAll('[data-buy]').forEach(b => {
      b.addEventListener('click', async () => {
        const id = Number(b.dataset.buy);
        const item = items.find(i => i.id === id);
        if (item) {
          const { quantity, unit } = parseQty(item.qty);
          try {
            await invoke('add_product', { name: item.name, category: null, quantity, unit,
              expiryDate: null, location: 'fridge', notes: '', catalogId: null });
          } catch (e) { console.warn('[hanni] shopping→fridge failed', e); }
        }
        await markBought([id]); // bought items drop off the active list
        await renderList(container, mode, preselectedIds);
      });
    });
    container.querySelectorAll('[data-del]').forEach(b => {
      b.addEventListener('click', async () => {
        const id = Number(b.dataset.del);
        await deleteShoppingItem(id);
        await renderList(container, mode, preselectedIds);
      });
    });
  }
  return items;
}

function modalShell(title, mode) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  const okLabel = mode === 'picker' ? 'Добавить в событие' : 'Готово';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">${escapeHtml(title)}</div>
    <div class="sl-add-row">
      <input class="form-input" id="sl-add-name" placeholder="Что купить (Помидоры)">
      <input class="form-input sl-add-qty" id="sl-add-qty" placeholder="2 шт">
      <button class="btn-secondary" id="sl-add-btn">+ Добавить</button>
    </div>
    <div class="sl-list" id="sl-list"><div class="muted">Загрузка…</div></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="sl-cancel">${mode === 'picker' ? 'Отмена' : 'Закрыть'}</button>
      ${mode === 'picker' ? `<button class="btn-primary" id="sl-ok">${escapeHtml(okLabel)}</button>` : ''}
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  return overlay;
}

async function wireAdd(overlay, listEl, mode, preselectedIds) {
  overlay.querySelector('#sl-add-btn').addEventListener('click', async () => {
    const name = overlay.querySelector('#sl-add-name').value.trim();
    if (!name) return;
    const qty = overlay.querySelector('#sl-add-qty').value.trim();
    await addShoppingItem(name, qty, '');
    overlay.querySelector('#sl-add-name').value = '';
    overlay.querySelector('#sl-add-qty').value = '';
    await renderList(listEl, mode, preselectedIds);
    overlay.querySelector('#sl-add-name').focus();
  });
  overlay.querySelector('#sl-add-name').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') overlay.querySelector('#sl-add-btn').click();
  });
}

export async function showShoppingPicker(onPick) {
  const overlay = modalShell('🛒 В магазин', 'picker');
  const listEl = overlay.querySelector('#sl-list');
  const items = await renderList(listEl, 'picker');
  await wireAdd(overlay, listEl, 'picker');
  overlay.querySelector('#sl-cancel').addEventListener('click', () => overlay.remove());
  overlay.querySelector('#sl-ok').addEventListener('click', async () => {
    const ids = Array.from(overlay.querySelectorAll('input[type="checkbox"]:checked'))
      .map(c => Number(c.dataset.id));
    const picked = items.filter(i => ids.includes(i.id));
    overlay.remove();
    onPick?.(picked);
  });
}

export async function showShoppingManager() {
  const overlay = modalShell('🛒 Список покупок', 'manage');
  const listEl = overlay.querySelector('#sl-list');
  await renderList(listEl, 'manage');
  await wireAdd(overlay, listEl, 'manage');
  overlay.querySelector('#sl-cancel').addEventListener('click', () => overlay.remove());
}
