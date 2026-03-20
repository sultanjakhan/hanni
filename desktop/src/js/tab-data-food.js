// ── tab-data-food.js — Food tab (food log, recipes, products) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── Food ──
export async function loadFood(subTab) {
  const el = document.getElementById('food-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'food', {
    title: 'Food',
    subtitle: 'Питание и продукты',
    icon: '🍔',
    renderDash: async (paneEl) => {
      const today = new Date().toISOString().split('T')[0];
      const stats = await invoke('get_food_stats', { days: 1 }).catch(() => ({}));
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${stats.avg_calories || 0}</div><div class="uni-dash-label">Калории</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${stats.avg_protein || 0}g</div><div class="uni-dash-label">Белок</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${stats.avg_carbs || 0}g</div><div class="uni-dash-label">Углеводы</div></div>
          <div class="uni-dash-card purple"><div class="uni-dash-value">${stats.avg_fat || 0}g</div><div class="uni-dash-label">Жиры</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const activeInner = S._foodInner || 'log';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);">
          <button class="pill${activeInner === 'log' ? ' active' : ''}" data-inner="log">Дневник</button>
          <button class="pill${activeInner === 'recipes' ? ' active' : ''}" data-inner="recipes">Рецепты</button>
          <button class="pill${activeInner === 'products' ? ' active' : ''}" data-inner="products">Продукты</button>
        </div>
        <div id="food-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#food-inner-content');
      if (activeInner === 'recipes') await loadRecipes(innerEl);
      else if (activeInner === 'products') await loadProducts(innerEl);
      else await loadFoodLog(innerEl);
      paneEl.querySelectorAll('[data-inner]').forEach(btn => {
        btn.addEventListener('click', () => { S._foodInner = btn.dataset.inner; loadFood(); });
      });
    },
  });
}

async function loadFoodLog(el) {
  try {
    const today = new Date().toISOString().split('T')[0];
    const log = await invoke('get_food_log', { date: today }).catch(() => []);
    const mealLabels = { breakfast:'Завтрак', lunch:'Обед', dinner:'Ужин', snack:'Перекус' };

    const dbv = new DatabaseView(el, {
      tabId: 'food',
      recordTable: 'food_log',
      records: log,
      availableViews: ['table', 'list'],
      fixedColumns: [
        { key: 'name', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
        { key: 'meal_type', label: 'Приём', render: r => `<span class="badge badge-gray">${mealLabels[r.meal_type] || r.meal_type}</span>` },
        { key: 'calories', label: 'Калории', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.calories || 0} kcal</span>` },
        { key: 'protein', label: 'Белок', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.protein || '—'}g</span>` },
        { key: 'carbs', label: 'Углев.', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.carbs || '—'}g</span>` },
        { key: 'fat', label: 'Жиры', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.fat || '—'}g</span>` },
      ],
      idField: 'id',
      addButton: '+ Записать',
      onQuickAdd: async (name) => {
        await invoke('log_food', { mealType: 'other', name, calories: 0, protein: null, carbs: null, fat: null, notes: null });
        loadFoodLog(el);
      },
      reloadFn: () => loadFoodLog(el),
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddFoodModal(el) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Log Food</div>
    <div class="form-group"><label class="form-label">Meal</label>
      <select class="form-select" id="food-meal" style="width:100%;">
        <option value="breakfast">Breakfast</option><option value="lunch">Lunch</option>
        <option value="dinner">Dinner</option><option value="snack">Snack</option>
      </select></div>
    <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="food-name"></div>
    <div class="form-group"><label class="form-label">Calories</label><input class="form-input" id="food-cal" type="number"></div>
    <div class="form-group"><label class="form-label">Protein (g)</label><input class="form-input" id="food-protein" type="number"></div>
    <div class="form-group"><label class="form-label">Carbs (g)</label><input class="form-input" id="food-carbs" type="number"></div>
    <div class="form-group"><label class="form-label">Fat (g)</label><input class="form-input" id="food-fat" type="number"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="food-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('food-save')?.addEventListener('click', async () => {
    const name = document.getElementById('food-name')?.value?.trim();
    if (!name) return;
    try {
      await invoke('log_food', {
        date: null, mealType: document.getElementById('food-meal')?.value || 'snack', name,
        calories: parseInt(document.getElementById('food-cal')?.value)||null,
        protein: parseFloat(document.getElementById('food-protein')?.value)||null,
        carbs: parseFloat(document.getElementById('food-carbs')?.value)||null,
        fat: parseFloat(document.getElementById('food-fat')?.value)||null,
        notes: null,
      });
      overlay.remove();
      loadFoodLog(el);
    } catch (err) { alert('Error: ' + err); }
  });
}

async function loadRecipes(el) {
  try {
    const recipes = await invoke('get_recipes', { search: null, tags: null }).catch(() => []);
    const fixedColumns = [
      { key: 'name', label: 'Name', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
      { key: 'prep_time', label: 'Prep', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.prep_time||0}+${r.cook_time||0} min</span>` },
      { key: 'calories', label: 'Calories', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.calories||'\u2014'}</span>` },
      { key: 'tags', label: 'Tags', render: r => r.tags ? r.tags.split(',').map(t => `<span class="badge badge-gray">${t.trim()}</span>`).join(' ') : '' },
    ];
    el.innerHTML = '<div id="recipes-dbv"></div>';
    const dbvEl = document.getElementById('recipes-dbv');
    const dbv = new DatabaseView(dbvEl, {
      tabId: 'food', recordTable: 'recipes', records: recipes,
      availableViews: ['table', 'list'],
      fixedColumns, idField: 'id',
      addButton: '+ Рецепт',
      onQuickAdd: async (name) => {
        await invoke('create_recipe', { name, description: null, ingredients: '', instructions: '', prepTime: null, cookTime: null, servings: null, calories: null, tags: null });
        loadRecipes(el);
      },
      reloadFn: () => loadRecipes(el),
      onDelete: async (id) => { await invoke('delete_recipe', { id }); },
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddRecipeModal(el) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Add Recipe</div>
    <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="rcp-name"></div>
    <div class="form-group"><label class="form-label">Ingredients</label><textarea class="form-textarea" id="rcp-ing" rows="3"></textarea></div>
    <div class="form-group"><label class="form-label">Instructions</label><textarea class="form-textarea" id="rcp-inst" rows="3"></textarea></div>
    <div class="form-group"><label class="form-label">Prep time (min)</label><input class="form-input" id="rcp-prep" type="number"></div>
    <div class="form-group"><label class="form-label">Calories</label><input class="form-input" id="rcp-cal" type="number"></div>
    <div class="form-group"><label class="form-label">Tags (comma-separated)</label><input class="form-input" id="rcp-tags"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="rcp-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('rcp-save')?.addEventListener('click', async () => {
    const name = document.getElementById('rcp-name')?.value?.trim();
    if (!name) return;
    try {
      await invoke('create_recipe', {
        name, description: null,
        ingredients: document.getElementById('rcp-ing')?.value||'',
        instructions: document.getElementById('rcp-inst')?.value||'',
        prepTime: parseInt(document.getElementById('rcp-prep')?.value)||null,
        cookTime: null, servings: null,
        calories: parseInt(document.getElementById('rcp-cal')?.value)||null,
        tags: document.getElementById('rcp-tags')?.value||null,
      });
      overlay.remove();
      loadRecipes(el);
    } catch (err) { alert('Error: ' + err); }
  });
}

async function loadProducts(el) {
  try {
    const products = await invoke('get_products', { location: null, expiringSoon: false }).catch(() => []);
    const fixedColumns = [
      { key: 'name', label: 'Name', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
      { key: 'location', label: 'Location', render: r => `<span class="badge badge-gray">${r.location||''}</span>` },
      { key: 'quantity', label: 'Qty', render: r => r.quantity ? `<span style="font-size:12px;color:var(--text-secondary);">${r.quantity} ${r.unit||''}</span>` : '' },
      { key: 'expiry_date', label: 'Expiry', render: r => {
        if (!r.expiry_date) return '';
        const exp = new Date(r.expiry_date);
        const isExpiring = (exp - Date.now()) < 3 * 86400000;
        return `<span style="color:${isExpiring?'var(--color-red)':'var(--text-secondary)'};font-size:12px;">${r.expiry_date}</span>`;
      }},
    ];
    el.innerHTML = '<div id="products-dbv"></div>';
    const dbvEl = document.getElementById('products-dbv');
    const dbv = new DatabaseView(dbvEl, {
      tabId: 'food', recordTable: 'products', records: products,
      availableViews: ['table', 'list'],
      fixedColumns, idField: 'id',
      addButton: '+ Продукт',
      onQuickAdd: async (name) => {
        await invoke('add_product', { name, location: '', quantity: null, unit: null, expiryDate: null, notes: null });
        loadProducts(el);
      },
      reloadFn: () => loadProducts(el),
      onDelete: async (id) => { await invoke('delete_product', { id }); },
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddProductModal(el) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Add Product</div>
    <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="prod-name"></div>
    <div class="form-group"><label class="form-label">Location</label>
      <select class="form-select" id="prod-loc" style="width:100%;">
        <option value="fridge">Fridge</option><option value="freezer">Freezer</option>
        <option value="pantry">Pantry</option><option value="other">Other</option>
      </select></div>
    <div class="form-group"><label class="form-label">Quantity</label><input class="form-input" id="prod-qty" type="number"></div>
    <div class="form-group"><label class="form-label">Unit</label><input class="form-input" id="prod-unit" placeholder="pcs, kg, L..."></div>
    <div class="form-group"><label class="form-label">Expiry Date</label><input class="form-input" id="prod-exp" type="date"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="prod-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('prod-save')?.addEventListener('click', async () => {
    const name = document.getElementById('prod-name')?.value?.trim();
    if (!name) return;
    try {
      await invoke('add_product', {
        name, category: null,
        quantity: parseFloat(document.getElementById('prod-qty')?.value)||null,
        unit: document.getElementById('prod-unit')?.value||null,
        expiryDate: document.getElementById('prod-exp')?.value||null,
        location: document.getElementById('prod-loc')?.value||'fridge',
        barcode: null, notes: null,
      });
      overlay.remove();
      loadProducts(el);
    } catch (err) { alert('Error: ' + err); }
  });
}
