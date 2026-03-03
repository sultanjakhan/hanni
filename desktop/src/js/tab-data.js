// ── js/tab-data.js — Data tab loaders: Home, Mindset, Food, Money, People, Memory, Work, Development, Hobbies, Sports, Health, About, Custom Pages ──

import { S, invoke, tabLoaders, TAB_REGISTRY, TAB_ICONS, MEDIA_TYPES, MEDIA_LABELS, STATUS_LABELS, MEMORY_CATEGORIES, COMMON_EMOJIS } from './state.js';
import { escapeHtml, renderMarkdown, renderPageHeader, setupPageHeaderControls, confirmModal, skeletonPage, skeletonGrid, skeletonList, skeletonSettings, initBlockEditor, blocksToPlainText, migrateTextToBlocks, loadTabBlockEditor } from './utils.js';
import { renderTabBar, closeTab } from './tabs.js';

// Helper: renderDatabaseView lives in tab-notes.js, access via tabLoaders
function renderDatabaseView(el, tabId, recordTable, records, options) {
  return tabLoaders.renderDatabaseView?.(el, tabId, recordTable, records, options);
}

// Helper: append block editor section to a page-content element
async function appendBlockEditor(pc, tabId, subTab) {
  const section = document.createElement('div');
  section.className = 'tab-block-section';
  section.innerHTML = '<div class="tab-block-section-header">Заметки</div>';
  pc.appendChild(section);
  return loadTabBlockEditor(tabId, subTab, section);
}

// Helper: show stub for empty/error states
function showStub(containerId, icon, label, desc) {
  const el = document.getElementById(containerId);
  if (el) el.innerHTML = `<div class="tab-stub">
    <div class="tab-stub-icon">${icon}</div>
    <div class="tab-stub-title">${label}</div>
    <div class="tab-stub-desc">${desc}</div>
    <span class="tab-stub-badge">Скоро</span>
  </div>`;
}

// ── Home ──
async function loadHome(subTab) {
  const el = document.getElementById('home-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('home') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Shopping List') await loadShoppingList(pc);
  else await loadSupplies(pc);
  appendBlockEditor(pc, 'home', subTab);
}

async function loadSupplies(el) {
  try {
    const items = await invoke('get_home_items', { category: null, neededOnly: false }).catch(() => []);
    const categories = { cleaning: 'Cleaning', hygiene: 'Hygiene', household: 'Household', electronics: 'Electronics', tools: 'Tools', other: 'Other' };
    el.innerHTML = `
      <div class="module-header"><h2>Supplies</h2><button class="btn-primary" id="home-add-btn">+ Add Item</button></div>
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
      <div class="module-header"><h2>Shopping List</h2></div>
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

// ── Mindset ──
async function loadMindset(subTab) {
  const el = document.getElementById('mindset-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('mindset') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Journal') await loadJournal(pc);
  else if (subTab === 'Mood') await loadMoodLog(pc);
  else if (subTab === 'Principles') await loadPrinciples(pc);
  else await loadJournal(pc);
  appendBlockEditor(pc, 'mindset', subTab);
}

async function loadJournal(el) {
  try {
    const today = await invoke('get_journal_entry', { date: null }).catch(() => null);
    const entries = await invoke('get_journal_entries', { days: 7 }).catch(() => []);
    const mood = today?.mood || 3, energy = today?.energy || 3, stress = today?.stress || 3;
    el.innerHTML = `
      <div class="module-header"><h2>Journal</h2></div>
      <div class="settings-section">
        <div class="settings-section-title">Today</div>
        <div class="settings-row"><span class="settings-label">Mood (1-5)</span><input class="form-input" id="j-mood" type="number" min="1" max="5" value="${mood}" style="width:60px;"></div>
        <div class="settings-row"><span class="settings-label">Energy (1-5)</span><input class="form-input" id="j-energy" type="number" min="1" max="5" value="${energy}" style="width:60px;"></div>
        <div class="settings-row"><span class="settings-label">Stress (1-5)</span><input class="form-input" id="j-stress" type="number" min="1" max="5" value="${stress}" style="width:60px;"></div>
        <div class="form-group"><label class="form-label">Gratitude</label><textarea class="form-textarea" id="j-gratitude" rows="2">${escapeHtml(today?.gratitude||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Wins</label><textarea class="form-textarea" id="j-wins" rows="2">${escapeHtml(today?.wins||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Struggles</label><textarea class="form-textarea" id="j-struggles" rows="2">${escapeHtml(today?.struggles||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Reflection</label><textarea class="form-textarea" id="j-reflection" rows="3">${escapeHtml(today?.reflection||'')}</textarea></div>
        <button class="btn-primary" id="j-save" style="margin-top:8px;">Save</button>
      </div>
      ${entries.length > 0 ? `<div class="module-card-title" style="margin-top:16px;">Recent Entries</div>
        ${entries.map(e => `<div class="focus-log-item">
          <span class="focus-log-time">${e.date}</span>
          <span class="focus-log-title">Mood:${e.mood} Energy:${e.energy} Stress:${e.stress}</span>
        </div>`).join('')}` : ''}`;
    document.getElementById('j-save')?.addEventListener('click', async () => {
      try {
        await invoke('save_journal_entry', {
          mood: parseInt(document.getElementById('j-mood')?.value)||3,
          energy: parseInt(document.getElementById('j-energy')?.value)||3,
          stress: parseInt(document.getElementById('j-stress')?.value)||3,
          gratitude: document.getElementById('j-gratitude')?.value||null,
          reflection: document.getElementById('j-reflection')?.value||null,
          wins: document.getElementById('j-wins')?.value||null,
          struggles: document.getElementById('j-struggles')?.value||null,
        });
        loadJournal(el);
      } catch (err) { alert('Error: ' + err); }
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadMoodLog(el) {
  try {
    const history = await invoke('get_mood_history', { days: 14 }).catch(() => []);
    const moods = ['😤','😕','😐','🙂','😊'];
    el.innerHTML = `
      <div class="module-header"><h2>Mood Log</h2></div>
      <div style="display:flex;gap:12px;justify-content:center;margin:20px 0;">
        ${moods.map((m,i) => `<button class="mood-btn" data-mood="${i+1}" style="font-size:32px;background:none;border:none;cursor:pointer;opacity:0.5;transition:opacity 0.1s;" title="Mood ${i+1}">${m}</button>`).join('')}
      </div>
      <input class="form-input" id="mood-note" placeholder="Note (optional)..." style="max-width:400px;margin:0 auto 16px;display:block;">
      <div class="module-card-title">Recent</div>
      <div id="mood-history">
        ${history.map(m => `<div class="focus-log-item">
          <span class="focus-log-time">${m.date} ${m.time||''}</span>
          <span style="font-size:18px;">${moods[(m.mood||3)-1]}</span>
          <span class="focus-log-title">${escapeHtml(m.note||'')}</span>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('.mood-btn').forEach(btn => {
      btn.addEventListener('mouseenter', () => btn.style.opacity = '1');
      btn.addEventListener('mouseleave', () => btn.style.opacity = '0.5');
      btn.addEventListener('click', async () => {
        try {
          await invoke('log_mood', { mood: parseInt(btn.dataset.mood), note: document.getElementById('mood-note')?.value||null, trigger: null });
          loadMoodLog(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadPrinciples(el) {
  try {
    const principles = await invoke('get_principles').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Principles</h2><button class="btn-primary" id="add-principle-btn">+ Add</button></div>
      <div id="principles-list">
        ${principles.map(p => `<div class="habit-item">
          <div class="habit-check${p.active ? ' checked' : ''}" data-id="${p.id}">${p.active ? '&#10003;' : ''}</div>
          <span class="habit-name">${escapeHtml(p.title)}</span>
          <span style="color:var(--text-faint);font-size:11px;">${p.category||''}</span>
          <button class="memory-item-btn" data-del="${p.id}" style="margin-left:auto;">&times;</button>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-del]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (await confirmModal('Удалить?')) { await invoke('delete_principle', { id: parseInt(btn.dataset.del) }).catch(()=>{}); loadPrinciples(el); }
      });
    });
    document.getElementById('add-principle-btn')?.addEventListener('click', () => {
      const title = prompt('Principle:');
      if (title) invoke('create_principle', { title, description: '', category: 'discipline' }).then(() => loadPrinciples(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

// ── Food ──
async function loadFood(subTab) {
  const el = document.getElementById('food-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('food') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Food Log') await loadFoodLog(pc);
  else if (subTab === 'Recipes') await loadRecipes(pc);
  else if (subTab === 'Products') await loadProducts(pc);
  else await loadFoodLog(pc);
  appendBlockEditor(pc, 'food', subTab);
}

async function loadFoodLog(el) {
  try {
    const today = new Date().toISOString().split('T')[0];
    const log = await invoke('get_food_log', { date: today }).catch(() => []);
    const stats = await invoke('get_food_stats', { days: 1 }).catch(() => ({}));
    const mealLabels = { breakfast:'Breakfast', lunch:'Lunch', dinner:'Dinner', snack:'Snack' };
    el.innerHTML = `
      <div class="module-header"><h2>Food Log</h2><button class="btn-primary" id="food-add-btn">+ Log Food</button></div>
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_calories||0}</div><div class="dashboard-stat-label">Calories</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_protein||0}g</div><div class="dashboard-stat-label">Protein</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_carbs||0}g</div><div class="dashboard-stat-label">Carbs</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.avg_fat||0}g</div><div class="dashboard-stat-label">Fat</div></div>
      </div>
      <div id="food-log-list">
        ${['breakfast','lunch','dinner','snack'].map(meal => {
          const items = log.filter(l => l.meal_type === meal);
          return items.length > 0 ? `<div class="module-card-title">${mealLabels[meal]}</div>
            ${items.map(i => `<div class="focus-log-item">
              <span class="focus-log-title">${escapeHtml(i.name)}</span>
              <span class="focus-log-duration">${i.calories||0} kcal</span>
              <button class="memory-item-btn" data-fdel="${i.id}">&times;</button>
            </div>`).join('')}` : '';
        }).join('')}
      </div>`;
    el.querySelectorAll('[data-fdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('delete_food_entry', { id: parseInt(btn.dataset.fdel) }).catch(()=>{});
        loadFoodLog(el);
      });
    });
    document.getElementById('food-add-btn')?.addEventListener('click', () => showAddFoodModal(el));
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
    await renderDatabaseView(dbvEl, 'food', 'recipes', recipes, {
      fixedColumns, idField: 'id',
      addButton: '+ Add Recipe',
      onAdd: () => showAddRecipeModal(el),
      reloadFn: () => loadRecipes(el),
      _tabId: 'food', _recordTable: 'recipes',
    });
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
    await renderDatabaseView(dbvEl, 'food', 'products', products, {
      fixedColumns, idField: 'id',
      addButton: '+ Add Product',
      onAdd: () => showAddProductModal(el),
      reloadFn: () => loadProducts(el),
      _tabId: 'food', _recordTable: 'products',
    });
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

// ── Money ──
async function loadMoney(subTab) {
  const el = document.getElementById('money-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('money') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Expenses' || subTab === 'Income') await loadTransactions(pc, subTab === 'Income' ? 'income' : 'expense');
  else if (subTab === 'Budget') await loadBudgets(pc);
  else if (subTab === 'Savings') await loadSavings(pc);
  else if (subTab === 'Subscriptions') await loadSubscriptions(pc);
  else if (subTab === 'Debts') await loadDebts(pc);
  else await loadTransactions(pc, 'expense');
  appendBlockEditor(pc, 'money', subTab);
}

async function loadTransactions(el, txType) {
  try {
    const items = await invoke('get_transactions', { txType, category: null, days: 30 }).catch(() => []);
    const stats = await invoke('get_transaction_stats', { days: 30 }).catch(() => ({}));
    const isExpense = txType === 'expense';
    el.innerHTML = `
      <div class="module-header"><h2>${isExpense ? 'Expenses' : 'Income'}</h2><button class="btn-primary" id="tx-add-btn">+ Add</button></div>
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${isExpense ? (stats.total_expenses||0) : (stats.total_income||0)}</div><div class="dashboard-stat-label">Total (30d)</div></div>
      </div>
      <div id="tx-list">
        ${items.map(t => `<div class="focus-log-item">
          <span class="focus-log-time">${t.date}</span>
          <span class="focus-log-title">${escapeHtml(t.description||t.category)}</span>
          <span class="badge badge-gray">${t.category}</span>
          <span class="focus-log-duration" style="color:${isExpense?'var(--text-muted)':'var(--text-primary)'}">${isExpense?'-':'+'} ${t.amount} ${t.currency||'KZT'}</span>
          <button class="memory-item-btn" data-txdel="${t.id}">&times;</button>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('[data-txdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        await invoke('delete_transaction', { id: parseInt(btn.dataset.txdel) }).catch(()=>{});
        loadTransactions(el, txType);
      });
    });
    document.getElementById('tx-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add ${isExpense ? 'Expense' : 'Income'}</div>
        <div class="form-group"><label class="form-label">Amount</label><input class="form-input" id="tx-amount" type="number" step="0.01"></div>
        <div class="form-group"><label class="form-label">Category</label><input class="form-input" id="tx-category" placeholder="food, transport, salary..."></div>
        <div class="form-group"><label class="form-label">Description</label><input class="form-input" id="tx-desc"></div>
        <div class="form-group"><label class="form-label">Currency</label>
          <select class="form-select" id="tx-currency" style="width:100%;">
            <option value="KZT">KZT</option><option value="USD">USD</option><option value="RUB">RUB</option>
          </select></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="tx-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('tx-save')?.addEventListener('click', async () => {
        const amount = parseFloat(document.getElementById('tx-amount')?.value);
        if (!amount) return;
        try {
          await invoke('add_transaction', {
            date: null, txType, amount,
            currency: document.getElementById('tx-currency')?.value||'KZT',
            category: document.getElementById('tx-category')?.value||'other',
            description: document.getElementById('tx-desc')?.value||'',
            recurring: false, recurringPeriod: null,
          });
          overlay.remove();
          loadTransactions(el, txType);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadBudgets(el) {
  try {
    const budgets = await invoke('get_budgets').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Budgets</h2><button class="btn-primary" id="budget-add-btn">+ Add Budget</button></div>
      <div id="budgets-list">
        ${budgets.map(b => {
          const pct = b.amount > 0 ? Math.min(100, Math.round((b.spent||0) / b.amount * 100)) : 0;
          const warn = pct > 80;
          return `<div class="settings-section" style="margin-bottom:8px;">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="color:var(--text-primary);font-size:14px;">${escapeHtml(b.category)}</span>
              <span style="color:${warn?'var(--color-yellow)':'var(--text-secondary)'};font-size:12px;">${b.spent||0} / ${b.amount} (${b.period})</span>
            </div>
            <div class="dev-progress" style="margin-top:6px;"><div class="dev-progress-bar" style="width:${pct}%;background:${warn?'var(--color-yellow)':'var(--accent-blue)'}"></div></div>
          </div>`;
        }).join('')}
      </div>`;
    document.getElementById('budget-add-btn')?.addEventListener('click', () => {
      const cat = prompt('Category:');
      const amt = prompt('Amount:');
      if (cat && amt) invoke('create_budget', { category: cat, amount: parseFloat(amt), period: 'monthly' }).then(() => loadBudgets(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadSavings(el) {
  try {
    const goals = await invoke('get_savings_goals').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Savings Goals</h2><button class="btn-primary" id="savings-add-btn">+ Add Goal</button></div>
      <div id="savings-list">
        ${goals.map(g => {
          const pct = g.target_amount > 0 ? Math.min(100, Math.round(g.current_amount / g.target_amount * 100)) : 0;
          return `<div class="settings-section" style="margin-bottom:8px;">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="color:var(--text-primary);font-size:14px;">${escapeHtml(g.name)}</span>
              <span style="color:var(--text-secondary);font-size:12px;">${g.current_amount} / ${g.target_amount}</span>
            </div>
            <div class="dev-progress" style="margin-top:6px;"><div class="dev-progress-bar" style="width:${pct}%"></div></div>
            ${g.deadline ? `<div style="font-size:11px;color:var(--text-faint);margin-top:4px;">Deadline: ${g.deadline}</div>` : ''}
            <button class="btn-secondary" style="margin-top:6px;font-size:11px;padding:4px 10px;" data-sadd="${g.id}">+ Add funds</button>
          </div>`;
        }).join('')}
      </div>`;
    el.querySelectorAll('[data-sadd]').forEach(btn => {
      btn.addEventListener('click', async () => {
        const amount = prompt('Amount to add:');
        if (amount) {
          const goal = goals.find(g => g.id === parseInt(btn.dataset.sadd));
          if (goal) {
            await invoke('update_savings_goal', { id: goal.id, currentAmount: (goal.current_amount||0) + parseFloat(amount), name: null, targetAmount: null, deadline: null }).catch(e => alert(e));
            loadSavings(el);
          }
        }
      });
    });
    document.getElementById('savings-add-btn')?.addEventListener('click', () => {
      const name = prompt('Goal name:');
      const target = prompt('Target amount:');
      if (name && target) invoke('create_savings_goal', { name, targetAmount: parseFloat(target), currentAmount: 0, deadline: null, color: '#9B9B9B' }).then(() => loadSavings(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadSubscriptions(el) {
  try {
    const subs = await invoke('get_subscriptions').catch(() => []);
    const monthly = subs.filter(s => s.active).reduce((sum, s) => sum + (s.period === 'yearly' ? s.amount/12 : s.amount), 0);
    el.innerHTML = `
      <div class="module-header"><h2>Subscriptions</h2><button class="btn-primary" id="sub-add-btn">+ Add</button></div>
      <div class="dashboard-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${Math.round(monthly)}</div><div class="dashboard-stat-label">/month</div></div>
      </div>
      <div id="subs-list">
        ${subs.map(s => `<div class="focus-log-item">
          <span class="focus-log-title">${escapeHtml(s.name)}</span>
          <span class="badge ${s.active?'badge-green':'badge-gray'}">${s.active?'Active':'Paused'}</span>
          <span class="focus-log-duration">${s.amount} ${s.currency||'KZT'}/${s.period}</span>
          ${s.next_payment ? `<span style="color:var(--text-faint);font-size:11px;">Next: ${s.next_payment}</span>` : ''}
        </div>`).join('')}
      </div>`;
    document.getElementById('sub-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal">
        <div class="modal-title">Add Subscription</div>
        <div class="form-group"><label class="form-label">Name</label><input class="form-input" id="sub-name"></div>
        <div class="form-group"><label class="form-label">Amount</label><input class="form-input" id="sub-amount" type="number"></div>
        <div class="form-group"><label class="form-label">Period</label>
          <select class="form-select" id="sub-period" style="width:100%;"><option value="monthly">Monthly</option><option value="yearly">Yearly</option></select></div>
        <div class="form-group"><label class="form-label">Category</label><input class="form-input" id="sub-cat" placeholder="entertainment, tools..."></div>
        <div class="modal-actions">
          <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
          <button class="btn-primary" id="sub-save">Save</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
      document.getElementById('sub-save')?.addEventListener('click', async () => {
        const name = document.getElementById('sub-name')?.value?.trim();
        if (!name) return;
        try {
          await invoke('add_subscription', {
            name, amount: parseFloat(document.getElementById('sub-amount')?.value)||0,
            currency: 'KZT', period: document.getElementById('sub-period')?.value||'monthly',
            nextPayment: null, category: document.getElementById('sub-cat')?.value||'other', active: true,
          });
          overlay.remove();
          loadSubscriptions(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadDebts(el) {
  try {
    const debts = await invoke('get_debts').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Debts</h2><button class="btn-primary" id="debt-add-btn">+ Add</button></div>
      <div id="debts-list">
        ${debts.map(d => {
          const pct = d.amount > 0 ? Math.min(100, Math.round((d.amount - d.remaining) / d.amount * 100)) : 0;
          return `<div class="settings-section" style="margin-bottom:8px;">
            <div style="display:flex;justify-content:space-between;align-items:center;">
              <span style="color:var(--text-primary);font-size:14px;">${escapeHtml(d.name)}</span>
              <span class="badge ${d.type==='owe'?'badge-purple':'badge-green'}">${d.type==='owe'?'I owe':'Owed to me'}</span>
            </div>
            <div style="color:var(--text-secondary);font-size:12px;margin-top:4px;">Remaining: ${d.remaining} / ${d.amount}</div>
            <div class="dev-progress" style="margin-top:4px;"><div class="dev-progress-bar" style="width:${pct}%"></div></div>
          </div>`;
        }).join('')}
      </div>`;
    document.getElementById('debt-add-btn')?.addEventListener('click', () => {
      const name = prompt('Name:');
      const type = prompt('Type (owe/owed):') || 'owe';
      const amount = prompt('Amount:');
      if (name && amount) invoke('add_debt', { name, debtType: type, amount: parseFloat(amount), remaining: parseFloat(amount), interestRate: null, dueDate: null, description: '' }).then(() => loadDebts(el)).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

// ── People Tab ──
async function loadPeople(subTab) {
  const el = document.getElementById('people-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('people') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  const filter = subTab === 'Blocked' ? { blocked: true } : subTab === 'Favorites' ? {} : {};
  try {
    const items = await invoke('get_contacts', filter);
    let contacts = Array.isArray(items) ? items : [];
    if (subTab === 'Favorites') contacts = contacts.filter(c => c.favorite);
    // Load blocks for each contact
    for (const c of contacts) {
      try { c._blocks = await invoke('get_contact_blocks', { contactId: c.id }); } catch { c._blocks = []; }
    }
    pc.innerHTML = `
      <div class="module-header">
        <h2>${subTab === 'Blocked' ? 'Blocked' : subTab === 'Favorites' ? 'Favorites' : 'All Contacts'}</h2>
        <button class="btn-primary" id="add-contact-btn">+ Add</button>
      </div>
      <div class="contacts-list">
        ${contacts.length === 0 ? '<div class="tab-stub"><div class="tab-stub-icon">👤</div>No contacts yet</div>' :
          contacts.map(c => `
            <div class="contact-item${c.blocked ? ' blocked' : ''}${c.favorite ? ' favorite' : ''}">
              <div class="contact-avatar">${c.name.charAt(0).toUpperCase()}</div>
              <div class="contact-info">
                <div class="contact-name">${c.name}${c.favorite ? ' ★' : ''}</div>
                <div class="contact-detail">${c.relationship || c.category || ''}${c.phone ? ' · ' + c.phone : ''}${c.email ? ' · ' + c.email : ''}</div>
                ${c.blocked ? '<span class="badge badge-red">Blocked</span>' : ''}
                ${c.block_reason ? '<div class="contact-detail" style="color:var(--text-muted)">' + c.block_reason + '</div>' : ''}
                ${c.notes ? '<div class="contact-detail">' + c.notes + '</div>' : ''}
                ${c._blocks && c._blocks.length > 0 ? `
                  <div class="contact-blocks-list">
                    ${c._blocks.map(b => `
                      <div class="contact-block-item">
                        <span class="contact-block-type">${b.block_type === 'app' ? 'App' : 'Site'}</span>
                        <span class="contact-block-value">${b.value}</span>
                        ${b.reason ? '<span class="contact-block-reason">' + b.reason + '</span>' : ''}
                        <button class="contact-block-del" onclick="deleteContactBlock(${b.id})">✕</button>
                      </div>
                    `).join('')}
                  </div>` : ''}
              </div>
              <div class="contact-actions">
                <button class="btn-secondary" onclick="showContactBlockModal(${c.id}, '${c.name.replace(/'/g, "\\'")}')" title="Block sites/apps">🔗</button>
                <button class="btn-secondary" onclick="toggleContactFav(${c.id})" title="${c.favorite ? 'Unfavorite' : 'Favorite'}">${c.favorite ? '★' : '☆'}</button>
                <button class="btn-secondary" onclick="toggleContactBlock(${c.id})" title="${c.blocked ? 'Unblock' : 'Block'}">${c.blocked ? '🔓' : '🚫'}</button>
                <button class="btn-danger" onclick="deleteContact(${c.id})" style="padding:8px 12px">✕</button>
              </div>
            </div>
          `).join('')}
      </div>`;
    document.getElementById('add-contact-btn')?.addEventListener('click', showAddContactModal);
  } catch (e) {
    pc.innerHTML = `<div class="tab-stub"><div class="tab-stub-icon">⚠️</div>${e}</div>`;
  }
  appendBlockEditor(pc, 'people', subTab);
}

// Window handlers for People (called from inline onclick)
window.toggleContactFav = async (id) => {
  await invoke('toggle_contact_favorite', { id });
  loadPeople(S.activeSubTab.people || 'All');
};
window.toggleContactBlock = async (id) => {
  await invoke('toggle_contact_blocked', { id });
  loadPeople(S.activeSubTab.people || 'All');
};
window.deleteContact = async (id) => {
  if (await confirmModal('Удалить контакт?')) {
    await invoke('delete_contact', { id });
    loadPeople(S.activeSubTab.people || 'All');
  }
};
window.deleteContactBlock = async (id) => {
  await invoke('delete_contact_block', { id });
  loadPeople(S.activeSubTab.people || 'All');
};
window.showContactBlockModal = (contactId, contactName) => {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <div class="modal-title">Block site/app for ${contactName}</div>
      <div class="form-group"><label class="form-label">Type</label>
        <select class="form-select" id="cb-type" style="width:100%">
          <option value="site">Site</option><option value="app">App</option>
        </select>
      </div>
      <div class="form-group"><label class="form-label">Value *</label><input class="form-input" id="cb-value" placeholder="e.g. instagram.com or Instagram"></div>
      <div class="form-group"><label class="form-label">Reason</label><input class="form-input" id="cb-reason" placeholder="Why block?"></div>
      <div class="modal-actions">
        <button class="btn-secondary" id="cb-cancel">Cancel</button>
        <button class="btn-primary" id="cb-save">Add</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#cb-cancel').onclick = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#cb-save').onclick = async () => {
    const value = document.getElementById('cb-value').value.trim();
    if (!value) return;
    await invoke('add_contact_block', {
      contactId,
      blockType: document.getElementById('cb-type').value,
      value,
      reason: document.getElementById('cb-reason').value.trim() || null,
    });
    overlay.remove();
    loadPeople(S.activeSubTab.people || 'All');
  };
};

function showAddContactModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <div class="modal-title">Add Contact</div>
      <div class="form-group"><label class="form-label">Name *</label><input class="form-input" id="mc-name" placeholder="Name"></div>
      <div class="form-group"><label class="form-label">Phone</label><input class="form-input" id="mc-phone" placeholder="Phone"></div>
      <div class="form-group"><label class="form-label">Email</label><input class="form-input" id="mc-email" placeholder="Email"></div>
      <div class="form-group"><label class="form-label">Category</label>
        <select class="form-select" id="mc-category" style="width:100%">
          <option value="friend">Friend</option><option value="family">Family</option><option value="work">Work</option>
          <option value="spammer">Spammer</option><option value="other">Other</option>
        </select>
      </div>
      <div class="form-group"><label class="form-label">Relationship</label><input class="form-input" id="mc-rel" placeholder="e.g. College friend"></div>
      <div class="form-group"><label class="form-label">Notes</label><textarea class="form-textarea" id="mc-notes" placeholder="Notes"></textarea></div>
      <div class="form-group" style="display:flex;align-items:center;gap:8px">
        <input type="checkbox" id="mc-blocked"><label for="mc-blocked" style="color:var(--text-secondary);font-size:14px">Block this contact</label>
      </div>
      <div class="form-group"><label class="form-label">Block reason</label><input class="form-input" id="mc-reason" placeholder="Why blocked?"></div>
      <div class="modal-actions">
        <button class="btn-secondary" id="mc-cancel">Cancel</button>
        <button class="btn-primary" id="mc-save">Save</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#mc-cancel').onclick = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#mc-save').onclick = async () => {
    const name = document.getElementById('mc-name').value.trim();
    if (!name) return;
    await invoke('add_contact', {
      name,
      phone: document.getElementById('mc-phone').value.trim() || null,
      email: document.getElementById('mc-email').value.trim() || null,
      category: document.getElementById('mc-category').value,
      relationship: document.getElementById('mc-rel').value.trim() || null,
      notes: document.getElementById('mc-notes').value.trim() || null,
      blocked: document.getElementById('mc-blocked').checked,
      blockReason: document.getElementById('mc-reason').value.trim() || null,
    });
    overlay.remove();
    loadPeople(S.activeSubTab.people || 'All');
  };
}

// ── Memory Tab ──
async function loadMemoryTab(subTab) {
  const el = document.getElementById('memory-content');
  if (!el) return;
  if (subTab === 'Search') loadMemorySearch(el);
  else loadAllFacts(el);
}

async function loadAllFacts(el) {
  try {
    const memories = await invoke('get_all_memories', { search: null }).catch(() => []);
    el.innerHTML = `
      <div class="memory-header">
        <div class="module-header" style="margin:0;flex:1;"><h2>Память</h2></div>
        <button class="btn-primary" id="mem-tab-add-btn">+ Добавить</button>
      </div>
      <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;">${memories.length} фактов</div>
      <div class="memory-browser" id="memory-all-list">
        ${memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
          <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
          <div class="memory-item-actions">
            <button class="memory-item-btn memory-edit-btn" data-medit="${m.id}" title="Редактировать">&#9998;</button>
            <button class="memory-item-btn" data-mdel="${m.id}" title="Удалить">&times;</button>
          </div>
        </div>`).join('')}
      </div>`;

    // Delete handlers
    el.querySelectorAll('[data-mdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (await confirmModal()) { await invoke('delete_memory', { id: parseInt(btn.dataset.mdel) }).catch(e => console.error('delete_memory error:', e)); loadAllFacts(el); }
      });
    });

    // Edit handlers
    el.querySelectorAll('[data-medit]').forEach(btn => {
      btn.addEventListener('click', () => {
        const id = parseInt(btn.dataset.medit);
        const m = memories.find(x => x.id === id);
        if (!m) return;
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.innerHTML = `<div class="modal modal-compact">
          <div class="modal-title">Редактировать факт</div>
          <div class="form-group"><label class="form-label">Категория</label>
            <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
          </div>
          <div class="form-group"><label class="form-label">Ключ</label>
            <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
          </div>
          <div class="form-group"><label class="form-label">Значение</label>
            <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
          </div>
          <div class="modal-actions">
            <button class="btn-secondary mem-cancel">Отмена</button>
            <button class="btn-primary mem-save">Сохранить</button>
          </div>
        </div>`;
        document.body.appendChild(overlay);
        overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
        overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
        overlay.querySelector('.mem-save').onclick = async () => {
          const cat = overlay.querySelector('.memory-edit-cat').value;
          const key = overlay.querySelector('.memory-edit-key').value.trim();
          const val = overlay.querySelector('.memory-edit-val').value.trim();
          if (!key || !val) return;
          try {
            await invoke('update_memory', { id, category: cat, key, value: val });
          } catch (err) { console.error('Memory edit error:', err); }
          overlay.remove();
          loadAllFacts(el);
        };
      });
    });

    // Add button
    document.getElementById('mem-tab-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Новый факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Добавить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-add-cat').value;
        const key = overlay.querySelector('.memory-add-key').value.trim();
        const val = overlay.querySelector('.memory-add-val').value.trim();
        if (!key || key.length < 2 || !val || val.length < 2) return;
        try { await invoke('memory_remember', { category: cat, key, value: val }); } catch (err) { console.error('Memory add error:', err); }
        overlay.remove();
        loadAllFacts(el);
      };
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

function renderMemoryList(memories, el) {
  const list = document.getElementById('settings-mem-list');
  if (!list) return;
  const countEl = document.getElementById('settings-mem-count');
  if (countEl) countEl.textContent = `${memories.length} фактов`;
  list.innerHTML = memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
    <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
    <span class="memory-item-key">${escapeHtml(m.key)}</span>
    <span class="memory-item-value">${escapeHtml(m.value)}</span>
    <div class="memory-item-actions">
      <button class="memory-item-btn memory-edit-btn" data-edit="${m.id}" title="Редактировать">&#9998;</button>
      <button class="memory-item-btn" data-del="${m.id}" title="Удалить">&times;</button>
    </div>
  </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';

  list.querySelectorAll('[data-del]').forEach(btn => {
    btn.addEventListener('click', async () => {
      if (await confirmModal()) { await invoke('delete_memory', { id: parseInt(btn.dataset.del) }).catch(e => console.error('delete_memory error:', e)); loadMemoryInSettings(el); }
    });
  });

  list.querySelectorAll('[data-edit]').forEach(btn => {
    btn.addEventListener('click', () => {
      const id = parseInt(btn.dataset.edit);
      const m = memories.find(x => x.id === id);
      if (!m) return;
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Редактировать факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Сохранить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-edit-cat').value;
        const key = overlay.querySelector('.memory-edit-key').value.trim();
        const val = overlay.querySelector('.memory-edit-val').value.trim();
        if (!key || !val) return;
        try {
          await invoke('update_memory', { id, category: cat, key, value: val });
        } catch (err) { console.error('Failed to update memory:', err); }
        overlay.remove();
        loadMemoryInSettings(el);
      };
    });
  });
}

async function loadMemoryInSettings(el) {
  el.innerHTML = skeletonPage();
  try {
    const memories = await invoke('get_all_memories', { search: null }).catch(() => []);
    el.innerHTML = `
      <div class="memory-header">
        <div class="memory-search-box" style="flex:1;">
          <input class="form-input" id="settings-mem-search" placeholder="Поиск по памяти..." autocomplete="off">
        </div>
        <button class="btn-primary" id="mem-add-btn">+ Добавить</button>
      </div>
      <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;" id="settings-mem-count">${memories.length} фактов</div>
      <div class="memory-browser" id="settings-mem-list"></div>`;

    renderMemoryList(memories, el);

    document.getElementById('mem-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Новый факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Добавить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-add-cat').value;
        const key = overlay.querySelector('.memory-add-key').value.trim();
        const val = overlay.querySelector('.memory-add-val').value.trim();
        if (!key || !val) return;
        try {
          await invoke('memory_remember', { category: cat, key, value: val });
        } catch (err) { console.error('Failed to add memory:', err); }
        overlay.remove();
        loadMemoryInSettings(el);
      };
    });

    let searchTimeout;
    document.getElementById('settings-mem-search')?.addEventListener('input', async (e) => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(async () => {
        const q = e.target.value;
        const results = await invoke('get_all_memories', { search: q || null }).catch(() => []);
        renderMemoryList(results, el);
      }, 300);
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

async function loadMemorySearch(el) {
  el.innerHTML = `
    <div class="module-header"><h2>Поиск по памяти</h2></div>
    <div class="memory-search-box" style="margin-bottom:16px;">
      <input class="form-input" id="mem-search-input" placeholder="Поиск..." autocomplete="off">
    </div>
    <div class="memory-browser" id="mem-search-results"></div>`;
  let memSearchTimeout;
  document.getElementById('mem-search-input')?.addEventListener('input', async (e) => {
    clearTimeout(memSearchTimeout);
    memSearchTimeout = setTimeout(async () => {
      const q = e.target.value;
      if (!q || q.length < 2) return;
      try {
        const results = await invoke('get_all_memories', { search: q });
        const list = document.getElementById('mem-search-results');
        if (list) list.innerHTML = results.map(m => `<div class="memory-item">
          <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
        </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';
      } catch (_) {}
    }, 300);
  });
}

// ── Integrations page helper ──
function panelItem(item) {
  return `<div class="panel-item">
    <span class="panel-dot ${item.status}"></span>
    <div class="panel-item-info">
      <div class="panel-item-name">${item.name}</div>
      <div class="panel-item-detail">${item.detail}</div>
    </div>
  </div>`;
}

// ── About (Settings page) ──
async function loadAbout(el) {
  try {
    const info = await invoke('get_model_info').catch(() => ({}));
    el.innerHTML = `
      <div class="about-wrapper">
        <div class="about-card">
          <div class="about-header">
            <div class="about-logo">🤖</div>
            <div class="about-name">Hanni</div>
            <span class="about-version">v${S.APP_VERSION}</span>
          </div>
          <hr class="about-divider">
          <div class="about-info-list">
            <div class="about-info-row"><span class="about-info-label">Модель</span><span class="about-info-value">${info.model_name||'?'}</span></div>
            <div class="about-info-row"><span class="about-info-label">MLX сервер</span><span class="about-info-value ${info.server_online?'online':'offline'}">${info.server_online?'Онлайн':'Офлайн'}</span></div>
            <div class="about-info-row"><span class="about-info-label">HTTP API</span><span class="about-info-value" id="about-api-status">Проверяю...</span></div>
          </div>
          <hr class="about-divider">
          <div class="about-actions">
            <button class="settings-btn" id="about-check-update">Проверить обновления</button>
          </div>
        </div>
      </div>`;
    document.getElementById('about-check-update')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.textContent = 'Проверяю...'; btn.disabled = true;
      try { const r = await invoke('check_update'); btn.textContent = r; }
      catch (err) { btn.textContent = 'Ошибка'; }
      setTimeout(() => { btn.textContent = 'Проверить обновления'; btn.disabled = false; }, 4000);
    });
    try {
      const resp = await fetch('http://127.0.0.1:8235/api/status');
      const apiEl = document.getElementById('about-api-status');
      if (apiEl) { apiEl.textContent = resp.ok ? 'Активен' : 'Недоступен'; apiEl.className = 'about-info-value ' + (resp.ok ? 'online' : 'offline'); }
    } catch (_) {
      const apiEl = document.getElementById('about-api-status');
      if (apiEl) { apiEl.textContent = 'Недоступен'; apiEl.className = 'about-info-value offline'; }
    }
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

// ── Work ──
async function loadWork() {
  const el = document.getElementById('work-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('work') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  try {
    const projects = await invoke('get_projects').catch(() => []);
    await renderWork(pc, projects || []);
  } catch (e) {
    pc.innerHTML = '<div class="tab-stub"><div class="tab-stub-icon">💼</div><div class="tab-stub-title">Работа</div><div class="tab-stub-desc">Проекты и задачи</div><span class="tab-stub-badge">Скоро</span></div>';
  }
  appendBlockEditor(pc, 'work', 'default');
}

async function renderWork(el, projects) {
  if (!S.currentProjectId && projects.length > 0) S.currentProjectId = projects[0].id;
  const tasks = S.currentProjectId ? await invoke('get_tasks', { projectId: S.currentProjectId }).catch(() => []) : [];

  el.innerHTML = `<div class="work-layout">
    <div class="work-projects">
      <div class="work-projects-header">
        <button class="btn-primary" id="new-project-btn" style="width:100%;">+ Проект</button>
      </div>
      <div class="work-projects-list" id="work-projects-list"></div>
    </div>
    <div class="work-tasks">
      <div class="work-tasks-header">
        <h2 style="font-size:16px;color:var(--text-primary);">${S.currentProjectId ? escapeHtml(projects.find(p => p.id === S.currentProjectId)?.name || '') : 'Выберите проект'}</h2>
        ${S.currentProjectId ? '<button class="btn-primary" id="new-task-btn">+ Задача</button>' : ''}
      </div>
      <div id="work-tasks-list"></div>
    </div>
  </div>`;

  const projectList = document.getElementById('work-projects-list');
  for (const p of projects) {
    const item = document.createElement('div');
    item.className = 'work-project-item' + (p.id === S.currentProjectId ? ' active' : '');
    const taskCount = (p.task_count || 0);
    item.innerHTML = `<span class="work-project-dot" style="background:${p.color || 'var(--accent-blue)'}"></span>
      <span class="work-project-name">${escapeHtml(p.name)}</span>
      <span class="work-project-count">${taskCount}</span>`;
    item.addEventListener('click', () => { S.currentProjectId = p.id; loadWork(); });
    projectList.appendChild(item);
  }

  const taskList = document.getElementById('work-tasks-list');
  for (const t of (tasks || [])) {
    const item = document.createElement('div');
    item.className = 'work-task-item';
    const isDone = t.status === 'done';
    const priorityClass = `priority-${t.priority || 'normal'}`;
    item.innerHTML = `
      <div class="work-task-check${isDone ? ' done' : ''}" data-id="${t.id}"></div>
      <span class="work-task-title${isDone ? ' done' : ''}">${escapeHtml(t.title)}</span>
      <span class="work-task-priority ${priorityClass}">${t.priority || 'normal'}</span>`;
    item.querySelector('.work-task-check').addEventListener('click', async () => {
      const newStatus = isDone ? 'todo' : 'done';
      await invoke('update_task_status', { id: t.id, status: newStatus }).catch(() => {});
      loadWork();
    });
    taskList.appendChild(item);
  }

  document.getElementById('new-project-btn')?.addEventListener('click', () => {
    const name = prompt('Название проекта:');
    if (name) invoke('create_project', { name, description: '', color: '#9B9B9B' }).then(() => loadWork()).catch(e => alert(e));
  });

  document.getElementById('new-task-btn')?.addEventListener('click', () => {
    const title = prompt('Задача:');
    if (title) invoke('create_task', { projectId: S.currentProjectId, title, description: '', priority: 'normal', dueDate: null }).then(() => loadWork()).catch(e => alert(e));
  });
}

// ── Development ──
async function loadDevelopment() {
  const el = document.getElementById('development-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('development') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  try {
    const items = await invoke('get_learning_items', { typeFilter: S.devFilter === 'all' ? null : S.devFilter }).catch(() => []);
    renderDevelopment(pc, items || []);
  } catch (e) {
    pc.innerHTML = '<div class="tab-stub"><div class="tab-stub-icon">🚀</div><div class="tab-stub-title">Развитие</div><div class="tab-stub-desc">Обучение и саморазвитие</div><span class="tab-stub-badge">Скоро</span></div>';
  }
  appendBlockEditor(pc, 'development', 'default');
}

function renderDevelopment(el, items) {
  const filters = ['all', 'course', 'book', 'skill', 'article'];
  const filterLabels = { all: 'Все', course: 'Курсы', book: 'Книги', skill: 'Навыки', article: 'Статьи' };
  const statusLabels = { planned: 'Запланировано', in_progress: 'В процессе', completed: 'Завершено' };
  const statusColors = { planned: 'badge-gray', in_progress: 'badge-blue', completed: 'badge-green' };

  const filterBar = `<div class="dev-filters">
    ${filters.map(f => `<button class="pill${S.devFilter === f ? ' active' : ''}" data-filter="${f}">${filterLabels[f]}</button>`).join('')}
  </div>`;

  const fixedColumns = [
    { key: 'title', label: 'Title', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
    { key: 'type', label: 'Type', render: r => `<span class="badge badge-purple">${filterLabels[r.type] || r.type}</span>` },
    { key: 'status', label: 'Status', render: r => `<span class="badge ${statusColors[r.status] || 'badge-gray'}">${statusLabels[r.status] || r.status}</span>` },
    { key: 'progress', label: 'Progress', render: r => `<div class="dev-progress" style="width:80px;display:inline-block;"><div class="dev-progress-bar" style="width:${r.progress || 0}%"></div></div> <span style="font-size:11px;color:var(--text-faint);">${r.progress || 0}%</span>` },
  ];

  el.innerHTML = filterBar + '<div id="dev-dbv"></div>';
  const dbvEl = document.getElementById('dev-dbv');

  renderDatabaseView(dbvEl, 'development', 'learning_items', items, {
    fixedColumns,
    idField: 'id',
    addButton: '+ Добавить',
    onAdd: () => showAddLearningModal(),
    reloadFn: () => loadDevelopment(),
    _tabId: 'development',
    _recordTable: 'learning_items',
  });

  el.querySelectorAll('.dev-filters .pill').forEach(btn => {
    btn.addEventListener('click', () => { S.devFilter = btn.dataset.filter; loadDevelopment(); });
  });
}

function showAddLearningModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить</div>
    <div class="form-group"><label class="form-label">Тип</label>
      <select class="form-select" id="learn-type" style="width:100%;">
        <option value="course">Курс</option><option value="book">Книга</option>
        <option value="skill">Навык</option><option value="article">Статья</option>
      </select></div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="learn-title"></div>
    <div class="form-group"><label class="form-label">Описание</label><textarea class="form-textarea" id="learn-desc"></textarea></div>
    <div class="form-group"><label class="form-label">URL</label><input class="form-input" id="learn-url"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="learn-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('learn-save')?.addEventListener('click', async () => {
    const title = document.getElementById('learn-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_learning_item', {
        itemType: document.getElementById('learn-type')?.value || 'course',
        title,
        description: document.getElementById('learn-desc')?.value || '',
        url: document.getElementById('learn-url')?.value || '',
      });
      overlay.remove();
      loadDevelopment();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Hobbies (Media Collections) ──
async function loadHobbies(subTab) {
  const el = document.getElementById('hobbies-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('hobbies') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (!subTab || subTab === 'Overview') {
    await loadHobbiesOverview(pc);
  } else {
    const mediaType = Object.entries(MEDIA_LABELS).find(([k,v]) => v === subTab)?.[0];
    if (mediaType) await loadMediaList(pc, mediaType);
  }
  appendBlockEditor(pc, 'hobbies', subTab || 'Overview');
}

async function loadHobbiesOverview(el) {
  try {
    const stats = await invoke('get_media_stats', { mediaType: null }).catch(() => ({}));
    const lists = await invoke('get_user_lists').catch(() => []);
    el.innerHTML = `
      <div class="module-header"><h2>Collections</h2></div>
      <div class="dashboard-stats">
        ${MEDIA_TYPES.map(t => `<div class="dashboard-stat"><div class="dashboard-stat-value">${stats[t] || 0}</div><div class="dashboard-stat-label">${MEDIA_LABELS[t]}</div></div>`).join('')}
      </div>
      ${lists.length > 0 ? `<div class="module-card-title" style="margin-top:16px;">Lists</div>
        <div class="hobby-grid">${lists.map(l => `<div class="hobby-card" data-list="${l.id}">
          <div class="hobby-card-name">${escapeHtml(l.name)}</div>
          <div class="hobby-card-label">${l.item_count || 0} items</div>
        </div>`).join('')}</div>` : ''}
      <div style="margin-top:16px;">
        <button class="btn-primary" id="create-list-btn">+ New List</button>
      </div>`;
    document.getElementById('create-list-btn')?.addEventListener('click', () => {
      const name = prompt('List name:');
      if (name) invoke('create_user_list', { name, description: '', color: '#9B9B9B' }).then(() => loadHobbies('Overview')).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadMediaList(el, mediaType) {
  try {
    const items = await invoke('get_media_items', { mediaType, status: S.mediaStatusFilter === 'all' ? null : S.mediaStatusFilter, hidden: false });
    const label = MEDIA_LABELS[mediaType];
    const hasEp = ['anime','series','cartoon','manga','podcast'].includes(mediaType);

    // Status filter bar
    const filterBar = `<div class="dev-filters">
      ${['all','planned','in_progress','completed','on_hold','dropped'].map(s =>
        `<button class="pill${S.mediaStatusFilter === s ? ' active' : ''}" data-filter="${s}">${s === 'all' ? 'All' : STATUS_LABELS[s]}</button>`
      ).join('')}
    </div>`;

    // Fixed columns from existing schema
    const fixedColumns = [
      { key: 'title', label: 'Title', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
      { key: 'status', label: 'Status', render: r => `<span class="badge ${r.status === 'completed' ? 'badge-green' : r.status === 'in_progress' ? 'badge-blue' : 'badge-gray'}">${STATUS_LABELS[r.status] || r.status}</span>` },
      { key: 'rating', label: 'Rating', render: r => {
        const stars = r.rating ? '\u2605'.repeat(Math.round(r.rating / 2)) + '\u2606'.repeat(5 - Math.round(r.rating / 2)) : '\u2014';
        return `<span style="color:var(--text-secondary);font-size:12px;">${stars}</span>`;
      }},
      ...(hasEp ? [{ key: 'progress', label: 'Progress', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.total_episodes ? `${r.progress || 0}/${r.total_episodes}` : ''}</span>` }] : []),
      { key: 'year', label: 'Year', render: r => `<span style="color:var(--text-muted);font-size:12px;">${r.year || '\u2014'}</span>` },
    ];

    // Render filter bar + database view container
    el.innerHTML = filterBar + '<div id="media-dbv"></div>';
    const dbvEl = document.getElementById('media-dbv');

    await renderDatabaseView(dbvEl, 'hobbies', 'media_items', items, {
      fixedColumns,
      idField: 'id',
      addButton: '+ Add',
      onAdd: () => showAddMediaModal(mediaType),
      onRowClick: (record) => showMediaDetail(record, mediaType),
      reloadFn: () => loadMediaList(el, mediaType),
      _tabId: 'hobbies',
      _recordTable: 'media_items',
    });

    el.querySelectorAll('.dev-filters .pill').forEach(btn => {
      btn.addEventListener('click', () => { S.mediaStatusFilter = btn.dataset.filter; loadMediaList(el, mediaType); });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddMediaModal(mediaType) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  const hasEpisodes = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Add ${MEDIA_LABELS[mediaType]}</div>
    <div class="form-row"><input class="form-input" id="media-title" placeholder="Title"></div>
    <div class="form-row">
      <select class="form-select" id="media-status">
        <option value="planned">Planned</option><option value="in_progress">In Progress</option>
        <option value="completed">Completed</option><option value="on_hold">On Hold</option>
      </select>
      <input class="form-input" id="media-year" type="number" placeholder="Year" style="max-width:80px;">
      <input class="form-input" id="media-rating" type="number" min="0" max="10" placeholder="Rating" style="max-width:80px;">
    </div>
    ${hasEpisodes ? `<div class="form-row">
      <input class="form-input" id="media-progress" type="number" min="0" placeholder="Episode" style="max-width:80px;">
      <span class="form-hint">/</span>
      <input class="form-input" id="media-total" type="number" min="0" placeholder="Total" style="max-width:80px;">
    </div>` : ''}
    <textarea class="form-textarea" id="media-notes" placeholder="Notes" rows="2"></textarea>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="media-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('media-save')?.addEventListener('click', async () => {
    const title = document.getElementById('media-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('add_media_item', {
        mediaType, title,
        originalTitle: null, year: parseInt(document.getElementById('media-year')?.value) || null,
        description: null, coverUrl: null,
        status: document.getElementById('media-status')?.value || 'planned',
        rating: parseInt(document.getElementById('media-rating')?.value) || null,
        progress: hasEpisodes ? (parseInt(document.getElementById('media-progress')?.value) || null) : null,
        totalEpisodes: hasEpisodes ? (parseInt(document.getElementById('media-total')?.value) || null) : null,
        notes: document.getElementById('media-notes')?.value || null,
      });
      overlay.remove();
      loadHobbies(MEDIA_LABELS[mediaType]);
    } catch (err) { alert('Error: ' + err); }
  });
}

function showMediaDetail(item, mediaType) {
  const hasEpisodes = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">${escapeHtml(item.title)}</div>
    ${item.year ? `<div style="color:var(--text-secondary);font-size:12px;margin-bottom:8px;">${item.year}</div>` : ''}
    <div class="form-group"><label class="form-label">Status</label>
      <select class="form-select" id="md-status" style="width:100%;">
        ${Object.entries(STATUS_LABELS).map(([k,v]) => `<option value="${k}"${item.status===k?' selected':''}>${v}</option>`).join('')}
      </select></div>
    <div class="form-group"><label class="form-label">Rating (0-10)</label><input class="form-input" id="md-rating" type="number" min="0" max="10" value="${item.rating||''}"></div>
    ${hasEpisodes ? `<div class="form-group"><label class="form-label">Progress</label><input class="form-input" id="md-progress" type="number" value="${item.progress||0}"></div>` : ''}
    <div class="form-group"><label class="form-label">Notes</label><textarea class="form-textarea" id="md-notes">${escapeHtml(item.notes||'')}</textarea></div>
    <div class="modal-actions">
      <button class="btn-danger" id="md-delete">Delete</button>
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="md-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('md-save')?.addEventListener('click', async () => {
    try {
      await invoke('update_media_item', {
        id: item.id,
        status: document.getElementById('md-status')?.value || null,
        rating: parseInt(document.getElementById('md-rating')?.value) || null,
        progress: hasEpisodes ? (parseInt(document.getElementById('md-progress')?.value) || null) : null,
        notes: document.getElementById('md-notes')?.value || null,
        title: null, description: null, coverUrl: null, totalEpisodes: null,
      });
      overlay.remove();
      loadHobbies(MEDIA_LABELS[mediaType]);
    } catch (err) { alert('Error: ' + err); }
  });
  document.getElementById('md-delete')?.addEventListener('click', async () => {
    if (!(await confirmModal('Удалить?'))) return;
    await invoke('delete_media_item', { id: item.id }).catch(e => alert(e));
    overlay.remove();
    loadHobbies(MEDIA_LABELS[mediaType]);
  });
}

// ── Sports ──
async function loadSports(subTab) {
  const el = document.getElementById('sports-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('sports') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  if (subTab === 'Martial Arts') {
    await loadMartialArts(pc);
  } else if (subTab === 'Stats') {
    await loadSportsStats(pc);
  } else {
    try {
      const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
      const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
      renderSports(pc, workouts || [], stats);
    } catch (e) {
      showStub('sports-content', '💪', 'Спорт', 'Тренировки и физическая активность');
    }
  }
  appendBlockEditor(pc, 'sports', subTab || 'Workouts');
}

async function loadMartialArts(el) {
  try {
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const ma = (workouts || []).filter(w => w.type === 'martial_arts');
    el.innerHTML = `
      <div class="module-header"><h2>Единоборства</h2><button class="btn-primary" id="new-ma-btn">+ Тренировка</button></div>
      <table class="data-table">
        <thead><tr><th>Дата</th><th>Название</th><th>Время</th><th>Калории</th></tr></thead>
        <tbody id="ma-tbody"></tbody>
      </table>`;
    const tbody = document.getElementById('ma-tbody');
    if (ma.length === 0) {
      tbody.innerHTML = '<tr><td colspan="4" style="text-align:center;color:var(--text-faint);padding:24px;">Нет тренировок</td></tr>';
    } else {
      for (const w of ma) {
        const row = document.createElement('tr');
        row.className = 'data-table-row';
        row.innerHTML = `<td>${w.date || '\u2014'}</td><td class="data-table-title">${escapeHtml(w.title || 'Единоборства')}</td><td>${w.duration_minutes || 0} мин</td><td>${w.calories || '\u2014'}</td>`;
        tbody.appendChild(row);
      }
    }
    document.getElementById('new-ma-btn')?.addEventListener('click', () => {
      showAddWorkoutModal();
      setTimeout(() => { const sel = document.getElementById('workout-type'); if (sel) sel.value = 'martial_arts'; }, 50);
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);">Error: ${e}</div>`; }
}

async function loadSportsStats(el) {
  try {
    const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const typeLabels = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };
    const byType = {};
    for (const w of (workouts || [])) {
      byType[w.type] = (byType[w.type] || 0) + 1;
    }
    el.innerHTML = `
      <div class="module-header"><h2>Статистика</h2></div>
      <div class="sports-stats">
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.count || 0}</div><div class="dashboard-stat-label">Тренировок</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_minutes || 0}м</div><div class="dashboard-stat-label">Общее время</div></div>
        <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_calories || 0}</div><div class="dashboard-stat-label">Калории</div></div>
      </div>
      <div style="margin-top:16px;">
        <h3 style="font-size:14px;color:var(--text-muted);margin-bottom:8px;">По типам</h3>
        ${Object.entries(byType).map(([t, c]) => `<div style="display:flex;justify-content:space-between;padding:4px 0;font-size:14px;color:var(--text-secondary);border-bottom:1px solid var(--bg-hover);">
          <span>${typeLabels[t] || t}</span><span style="color:var(--text-muted);">${c}</span>
        </div>`).join('') || '<div style="color:var(--text-faint);font-size:14px;">No data yet</div>'}
      </div>`;
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);">Error: ${e}</div>`; }
}

function renderSports(el, workouts, stats) {
  const typeLabels = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };

  el.innerHTML = `
    <div class="module-header"><h2>Спорт</h2><button class="btn-primary" id="new-workout-btn">+ Тренировка</button></div>
    <div class="sports-stats">
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.count || 0}</div><div class="dashboard-stat-label">Тренировок</div></div>
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_minutes || 0}м</div><div class="dashboard-stat-label">Общее время</div></div>
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_calories || 0}</div><div class="dashboard-stat-label">Калории</div></div>
    </div>
    <div id="workouts-list"></div>`;

  const list = document.getElementById('workouts-list');
  for (const w of workouts) {
    const card = document.createElement('div');
    card.className = 'workout-card';
    card.innerHTML = `
      <div class="workout-card-header">
        <span class="workout-card-title">${escapeHtml(w.title || typeLabels[w.type] || w.type)}</span>
        <span class="workout-card-date">${w.date || ''}</span>
      </div>
      <div class="workout-card-meta">
        <span class="badge badge-purple">${typeLabels[w.type] || w.type}</span>
        <span>${w.duration_minutes || 0} мин</span>
        ${w.calories ? `<span>${w.calories} ккал</span>` : ''}
      </div>`;
    list.appendChild(card);
  }

  document.getElementById('new-workout-btn')?.addEventListener('click', () => showAddWorkoutModal());
}

function showAddWorkoutModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новая тренировка</div>
    <div class="form-row">
      <select class="form-select" id="workout-type">
        <option value="gym">Зал</option><option value="cardio">Кардио</option>
        <option value="yoga">Йога</option><option value="swimming">Плавание</option>
        <option value="martial_arts">Единоборства</option><option value="other">Другое</option>
      </select>
      <input class="form-input" id="workout-title" placeholder="Название">
    </div>
    <div class="form-row">
      <input class="form-input" id="workout-duration" type="number" value="60" placeholder="Минуты" style="max-width:100px;">
      <span class="form-hint">мин</span>
      <input class="form-input" id="workout-calories" type="number" placeholder="Калории" style="max-width:100px;">
      <span class="form-hint">ккал</span>
    </div>
    <textarea class="form-textarea" id="workout-notes" placeholder="Заметки (необязательно)" rows="2"></textarea>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="workout-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('workout-save')?.addEventListener('click', async () => {
    const title = document.getElementById('workout-title')?.value?.trim();
    try {
      await invoke('create_workout', {
        workoutType: document.getElementById('workout-type')?.value || 'other',
        title: title || '',
        durationMinutes: parseInt(document.getElementById('workout-duration')?.value || '60'),
        calories: parseInt(document.getElementById('workout-calories')?.value || '0') || null,
        notes: document.getElementById('workout-notes')?.value || '',
      });
      overlay.remove();
      loadSports();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Health ──
async function loadHealth() {
  const el = document.getElementById('health-content');
  if (!el) return;
  el.innerHTML = renderPageHeader('health') + '<div class="page-content"></div>';
  const pc = el.querySelector('.page-content');
  try {
    const today = await invoke('get_health_today').catch(() => ({}));
    const habits = await invoke('get_habits_today').catch(() => []);
    renderHealth(pc, today, habits);
  } catch (e) {
    pc.innerHTML = '<div class="tab-stub"><div class="tab-stub-icon">❤️</div><div class="tab-stub-title">Здоровье</div><div class="tab-stub-desc">Метрики здоровья и привычки</div><span class="tab-stub-badge">Скоро</span></div>';
  }
  appendBlockEditor(pc, 'health', 'default');
}

function renderHealth(el, today, habits) {
  const sleep = today.sleep || null;
  const water = today.water || null;
  const mood = today.mood || null;
  const weight = today.weight || null;

  function metricClass(type, val) {
    if (val === null) return '';
    if (type === 'sleep') return val >= 7 ? 'good' : val >= 5 ? 'warning' : 'bad';
    if (type === 'water') return val >= 8 ? 'good' : val >= 4 ? 'warning' : 'bad';
    if (type === 'mood') return val >= 4 ? 'good' : val >= 3 ? 'warning' : 'bad';
    return '';
  }

  el.innerHTML = `
    <div class="module-header"><h2>Здоровье</h2><button class="btn-primary" id="health-log-btn">+ Записать</button></div>
    <div class="health-metrics">
      <div class="health-metric ${metricClass('sleep', sleep)}" data-type="sleep">
        <div class="health-metric-icon">&#x1F634;</div>
        <div class="health-metric-value">${sleep !== null ? sleep + 'ч' : '\u2014'}</div>
        <div class="health-metric-label">Сон</div>
      </div>
      <div class="health-metric ${metricClass('water', water)}" data-type="water">
        <div class="health-metric-icon">&#x1F4A7;</div>
        <div class="health-metric-value">${water !== null ? water : '\u2014'}</div>
        <div class="health-metric-label">Вода (стаканов)</div>
      </div>
      <div class="health-metric ${metricClass('mood', mood)}" data-type="mood">
        <div class="health-metric-icon">${mood >= 4 ? '&#x1F60A;' : mood >= 3 ? '&#x1F610;' : mood ? '&#x1F641;' : '&#x1F636;'}</div>
        <div class="health-metric-value">${mood !== null ? mood + '/5' : '\u2014'}</div>
        <div class="health-metric-label">Настроение</div>
      </div>
      <div class="health-metric" data-type="weight">
        <div class="health-metric-icon">&#x2696;</div>
        <div class="health-metric-value">${weight !== null ? weight + 'кг' : '\u2014'}</div>
        <div class="health-metric-label">Вес</div>
      </div>
    </div>
    <div class="habits-section">
      <div class="module-card-title" style="display:flex;justify-content:space-between;align-items:center;">
        Привычки
        <button class="btn-secondary" id="add-habit-btn" style="padding:4px 10px;font-size:11px;">+ Добавить</button>
      </div>
      <div id="habits-list"></div>
    </div>`;

  const habitsList = document.getElementById('habits-list');
  for (const h of habits) {
    const item = document.createElement('div');
    item.className = 'habit-item';
    item.innerHTML = `
      <div class="habit-check${h.completed ? ' checked' : ''}" data-id="${h.id}">${h.completed ? '&#10003;' : ''}</div>
      <span class="habit-name">${escapeHtml(h.name)}</span>
      ${h.streak > 0 ? `<span class="habit-streak">${h.streak} дн.</span>` : ''}`;
    item.querySelector('.habit-check').addEventListener('click', async () => {
      await invoke('check_habit', { habitId: h.id, date: null }).catch(() => {});
      loadHealth();
    });
    habitsList.appendChild(item);
  }

  // Click on metric to log
  el.querySelectorAll('.health-metric').forEach(m => {
    m.addEventListener('click', () => {
      const type = m.dataset.type;
      const labels = { sleep: 'Сон (часы)', water: 'Вода (стаканы)', mood: 'Настроение (1-5)', weight: 'Вес (кг)' };
      const val = prompt(labels[type] + ':');
      if (val) {
        invoke('log_health', { healthType: type, value: parseFloat(val), notes: null }).then(() => loadHealth()).catch(e => alert(e));
      }
    });
  });

  document.getElementById('health-log-btn')?.addEventListener('click', () => {
    const type = prompt('Тип (sleep/water/mood/weight):');
    if (!type) return;
    const val = prompt('Значение:');
    if (val) invoke('log_health', { healthType: type, value: parseFloat(val), notes: null }).then(() => loadHealth()).catch(e => alert(e));
  });

  document.getElementById('add-habit-btn')?.addEventListener('click', () => {
    const name = prompt('Название привычки:');
    if (name) invoke('create_habit', { name, icon: '', frequency: 'daily' }).then(() => loadHealth()).catch(e => alert(e));
  });
}

// ── Custom Pages ──
async function loadCustomPage(tabId) {
  const reg = TAB_REGISTRY[tabId];
  if (!reg?.custom || !reg.pageId) return;
  const el = document.getElementById(`${tabId}-content`);
  if (!el) return;

  try {
    const page = await invoke('get_custom_page', { id: reg.pageId });

    el.innerHTML = `
      <div class="custom-page-header">
        <div class="custom-page-icon-row">
          <button class="custom-page-icon-btn" id="cp-icon-btn" title="Сменить иконку">${escapeHtml(page.icon || '📄')}</button>
          <button class="btn-danger btn-small custom-page-delete-btn" id="cp-delete-btn">Удалить</button>
        </div>
        <input class="page-title-input" id="cp-title" value="${escapeHtml(page.title || '')}" placeholder="Без названия">
        <input class="page-description-input" id="cp-desc" value="${escapeHtml(page.description || '')}" placeholder="Добавить описание...">
      </div>
      <div class="custom-page-content">
        <div id="cp-editor" class="block-editor-container"></div>
      </div>
      <div class="custom-page-emoji-picker hidden" id="cp-emoji-picker">
        ${COMMON_EMOJIS.map(e => `<button class="emoji-pick-btn">${e}</button>`).join('')}
      </div>`;

    // Auto-save helper for metadata fields
    const autoSaveMeta = (field, value) => {
      clearTimeout(S.customPageAutoSave);
      S.customPageAutoSave = setTimeout(async () => {
        const args = { id: reg.pageId };
        args[field] = value;
        await invoke('update_custom_page', args).catch(() => {});
        if (field === 'title') { reg.label = value || 'Без названия'; renderTabBar(); }
        if (field === 'icon') { reg.icon = value; renderTabBar(); }
      }, 500);
    };

    // Auto-save for Editor.js content
    const autoSaveContent = async () => {
      clearTimeout(S.customPageAutoSave);
      S.customPageAutoSave = setTimeout(async () => {
        if (!S.currentCpEditor) return;
        try {
          const output = await S.currentCpEditor.save();
          const contentBlocks = JSON.stringify(output);
          const content = blocksToPlainText(output);
          await invoke('update_custom_page', { id: reg.pageId, content, contentBlocks }).catch(() => {});
        } catch (e) { console.error('cp editor save error:', e); }
      }, 500);
    };

    document.getElementById('cp-title')?.addEventListener('input', (e) => autoSaveMeta('title', e.target.value));
    document.getElementById('cp-desc')?.addEventListener('input', (e) => autoSaveMeta('description', e.target.value));

    // Initialize Editor.js for custom page
    let editorData = null;
    if (page.content_blocks) {
      try { editorData = JSON.parse(page.content_blocks); } catch (e) { console.error('parse cp content_blocks:', e); }
    }
    if (!editorData && page.content) {
      editorData = migrateTextToBlocks(page.content);
    }

    // Destroy previous editor instance
    if (S.currentCpEditor) {
      try { S.currentCpEditor.destroy(); } catch (e) {}
      S.currentCpEditor = null;
    }
    S.currentCpEditor = initBlockEditor('cp-editor', editorData, () => autoSaveContent());

    // Emoji picker
    const emojiPicker = document.getElementById('cp-emoji-picker');
    document.getElementById('cp-icon-btn')?.addEventListener('click', (e) => {
      e.stopPropagation();
      emojiPicker?.classList.toggle('hidden');
    });
    document.querySelectorAll('.emoji-pick-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const emoji = btn.textContent;
        document.getElementById('cp-icon-btn').textContent = emoji;
        emojiPicker?.classList.add('hidden');
        autoSaveMeta('icon', emoji);
      });
    });
    // Close emoji picker on outside click
    const closeEmojiPicker = (e) => {
      if (!emojiPicker?.contains(e.target) && e.target.id !== 'cp-icon-btn') {
        emojiPicker?.classList.add('hidden');
      }
    };
    document.addEventListener('click', closeEmojiPicker);

    // Delete page
    document.getElementById('cp-delete-btn')?.addEventListener('click', async () => {
      if (!(await confirmModal('Удалить страницу?'))) return;
      if (S.currentCpEditor) {
        try { S.currentCpEditor.destroy(); } catch (e) {}
        S.currentCpEditor = null;
      }
      await invoke('delete_custom_page', { id: reg.pageId }).catch(() => {});
      closeTab(tabId);
      delete TAB_REGISTRY[tabId];
      const viewDiv = document.getElementById(`view-${tabId}`);
      if (viewDiv) viewDiv.remove();
    });

  } catch (e) {
    el.innerHTML = `<div class="empty-state"><div class="empty-state-icon">📄</div><div class="empty-state-text">Страница не найдена</div></div>`;
  }
}

export {
  loadHome,
  loadMindset,
  loadFood,
  loadMoney,
  loadPeople,
  loadMemoryTab,
  loadMemoryInSettings,
  renderMemoryList,
  loadAbout,
  loadWork,
  loadDevelopment,
  loadHobbies,
  loadSports,
  loadHealth,
  loadCustomPage,
  panelItem,
};
