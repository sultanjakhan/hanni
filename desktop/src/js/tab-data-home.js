// ── tab-data-home.js — Home tab (supplies, shopping list) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── Home ──
export async function loadHome(subTab) {
  const el = document.getElementById('home-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'home', {
    title: 'Home',
    subtitle: 'Дом и хозяйство',
    icon: '🏠',
    renderDash: async (paneEl) => {
      const items = await invoke('get_home_items', { category: null, neededOnly: false }).catch(() => []);
      const needed = items.filter(i => i.needed).length;
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${items.length}</div><div class="uni-dash-label">Предметов</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${needed}</div><div class="uni-dash-label">Нужно купить</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const items = await invoke('get_home_items', { category: null, neededOnly: false }).catch(() => []);
      const categories = { cleaning: 'Уборка', hygiene: 'Гигиена', household: 'Дом', electronics: 'Техника', tools: 'Инструменты', other: 'Другое' };
      const dbv = new DatabaseView(paneEl, {
        tabId: 'home', recordTable: 'home_items', records: items,
        availableViews: ['table', 'list'],
        fixedColumns: [
          { key: 'name', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
          { key: 'category', label: 'Категория', render: r => `<span class="badge badge-gray">${categories[r.category] || r.category}</span>` },
          { key: 'quantity', label: 'Кол-во', render: r => r.quantity != null ? `${r.quantity} ${r.unit || ''}` : '—' },
          { key: 'location', label: 'Место', render: r => r.location || '—' },
          { key: 'needed', label: 'Статус', render: r => r.needed ? '<span class="badge badge-red">Нужно</span>' : '<span class="badge badge-green">Есть</span>' },
        ],
        addButton: '+ Добавить',
        onAdd: () => { showHomeAddModal(); },
        onQuickAdd: async (name) => {
          await invoke('add_home_item', { name, category: 'other', quantity: null, unit: null, location: '', notes: null });
          loadHome();
        },
        reloadFn: () => loadHome(),
        onDelete: async (id) => { await invoke('delete_home_item', { id }); },
      });
      await dbv.render();
    },
  });
}

async function loadSupplies(el) {
  try {
    const items = await invoke('get_home_items', { category: null, neededOnly: false }).catch(() => []);
    const categories = { cleaning: 'Cleaning', hygiene: 'Hygiene', household: 'Household', electronics: 'Electronics', tools: 'Tools', other: 'Other' };
    el.innerHTML = `
      <div class="uni-section-header">Supplies <button class="btn-primary" id="home-add-btn" style="margin-left:auto;">+ Add Item</button></div>
      <div id="home-items-list">
        ${items.map(i => `<div class="focus-log-item" style="${i.needed ? 'border-left:2px solid var(--text-secondary);' : ''}">
          <span class="focus-log-title">${escapeHtml(i.name)}</span>
          <span class="badge badge-gray">${categories[i.category] || i.category}</span>
          ${i.quantity != null ? `<span style="color:var(--text-secondary);font-size:12px;">${i.quantity} ${i.unit||''}</span>` : ''}
          <span style="color:var(--text-faint);font-size:11px;">${i.location||''}</span>
          <button class="btn-secondary" style="padding:2px 8px;font-size:10px;margin-left:4px;" data-need="${i.id}">${i.needed ? 'In stock' : 'Need'}</button>
          <button class="memory-item-btn" data-hdel="${i.id}">&times;</button>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-need]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('toggle_home_item_needed', { id: parseInt(btn.dataset.need) }).catch(()=>{});
        loadSupplies(el);
      });
    });
    el.querySelectorAll('[data-hdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('delete_home_item', { id: parseInt(btn.dataset.hdel) }).catch(()=>{});
        loadSupplies(el);
      });
    });
    document.getElementById('home-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add Supply</div>
        <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="hi-name"></div>
        <div class="form-group"><label class="form-label">Category</label>
          <select class="form-select" id="hi-cat" style="width:100%;">
            ${Object.entries(categories).map(([k,v]) => `<option value="${k}">${v}</option>`).join('')}
          </select></div>
        <div class="form-group"><label class="form-label">Quantity</label><input class="form-input" id="hi-qty" type="number"></div>
        <div class="form-group"><label class="form-label">Unit</label><input class="form-input" id="hi-unit" placeholder="pcs, kg, L..."></div>
        <div class="form-group"><label class="form-label">Location</label>
          <select class="form-select" id="hi-loc" style="width:100%;">
            <option value="kitchen">Kitchen</option><option value="bathroom">Bathroom</option>
            <option value="bedroom">Bedroom</option><option value="living_room">Living Room</option>
            <option value="storage">Storage</option><option value="other">Other</option>
          </select></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="hi-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('hi-save')?.addEventListener('click', async () => {
        const name = document.getElementById('hi-name')?.value?.trim();
        if (!name) return;
        try {
          await invoke('add_home_item', {
            name, category: document.getElementById('hi-cat')?.value || 'other',
            quantity: parseFloat(document.getElementById('hi-qty')?.value) || null,
            unit: document.getElementById('hi-unit')?.value || null,
            location: document.getElementById('hi-loc')?.value || 'other',
            notes: null,
          });
          overlay.remove();
          loadSupplies(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadShoppingList(el) {
  try {
    const items = await invoke('get_home_items', { category: null, neededOnly: true }).catch(() => []);
    el.innerHTML = `
      <div class="uni-section-header">Shopping List</div>
      ${items.length > 0 ? `<div id="shopping-list">
        ${items.map(i => `<div class="habit-item">
          <div class="habit-check" data-bought="${i.id}"></div>
          <span class="habit-name">${escapeHtml(i.name)}</span>
          ${i.quantity != null ? `<span style="color:var(--text-secondary);font-size:12px;margin-left:auto;">${i.quantity} ${i.unit||''}</span>` : ''}
        </div>`).join('')}
      </div>` : '<div style="color:var(--text-faint);font-size:14px;padding:20px;text-align:center;">All stocked up!</div>'}`;
    el.querySelectorAll('[data-bought]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('toggle_home_item_needed', { id: parseInt(btn.dataset.bought) }).catch(()=>{});
        loadShoppingList(el);
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showHomeAddModal() {
  // Placeholder — the inline modal in loadSupplies is the main add flow
}
