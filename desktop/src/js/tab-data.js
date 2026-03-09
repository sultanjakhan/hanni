// ── js/tab-data.js — Data tab loaders: Home, Mindset, Food, Money, People, Memory, Work, Development, Hobbies, Sports, Health, About, Custom Pages ──

import { S, invoke, tabLoaders, TAB_REGISTRY, TAB_ICONS, MEDIA_TYPES, MEDIA_LABELS, STATUS_LABELS, MEMORY_CATEGORIES, COMMON_EMOJIS } from './state.js';
import { escapeHtml, renderMarkdown, renderPageHeader, setupPageHeaderControls, confirmModal, skeletonPage, skeletonGrid, skeletonList, skeletonSettings, initBlockEditor, blocksToPlainText, migrateTextToBlocks, loadTabBlockEditor } from './utils.js';
import { renderTabBar, closeTab } from './tabs.js';
import { DatabaseView } from './db-view/db-view.js';

// Legacy helper: backward-compat wrapper (still used by Recipes, Products)
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
        fixedColumns: [
          { key: 'name', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
          { key: 'category', label: 'Категория', render: r => `<span class="badge badge-gray">${categories[r.category] || r.category}</span>` },
          { key: 'quantity', label: 'Кол-во', render: r => r.quantity != null ? `${r.quantity} ${r.unit || ''}` : '—' },
          { key: 'location', label: 'Место', render: r => r.location || '—' },
          { key: 'needed', label: 'Статус', render: r => r.needed ? '<span class="badge badge-red">Нужно</span>' : '<span class="badge badge-green">Есть</span>' },
        ],
        addButton: '+ Добавить',
        onAdd: () => { showHomeAddModal(); },
        reloadFn: () => loadHome(),
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

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'mindset', {
    title: 'Mindset',
    subtitle: 'Дневник, настроение, принципы',
    icon: '🧠',
    renderDash: async (paneEl) => {
      const today = await invoke('get_journal_entry', { date: null }).catch(() => null);
      const history = await invoke('get_mood_history', { days: 7 }).catch(() => []);
      const avgMood = history.length > 0 ? (history.reduce((s, m) => s + (m.mood || 3), 0) / history.length).toFixed(1) : '—';
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${today ? today.mood + '/5' : '—'}</div><div class="uni-dash-label">Настроение сегодня</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${avgMood}</div><div class="uni-dash-label">Ср. за неделю</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${today?.energy || '—'}</div><div class="uni-dash-label">Энергия</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      // Internal sub-navigation for Journal / Mood / Principles
      const activeInner = S._mindsetInner || 'journal';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);">
          <button class="pill${activeInner === 'journal' ? ' active' : ''}" data-inner="journal">Дневник</button>
          <button class="pill${activeInner === 'mood' ? ' active' : ''}" data-inner="mood">Настроение</button>
          <button class="pill${activeInner === 'principles' ? ' active' : ''}" data-inner="principles">Принципы</button>
        </div>
        <div id="mindset-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#mindset-inner-content');
      if (activeInner === 'mood') await loadMoodLog(innerEl);
      else if (activeInner === 'principles') await loadPrinciples(innerEl);
      else await loadJournal(innerEl);
      paneEl.querySelectorAll('[data-inner]').forEach(btn => {
        btn.addEventListener('click', () => { S._mindsetInner = btn.dataset.inner; loadMindset(); });
      });
    },
  });
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
    const dbv = new DatabaseView(el, {
      tabId: 'mindset',
      recordTable: 'principles',
      records: principles,
      fixedColumns: [
        { key: 'active', label: '', render: r => `<div class="habit-check${r.active ? ' checked' : ''}" style="cursor:pointer;" data-pid="${r.id}">${r.active ? '&#10003;' : ''}</div>` },
        { key: 'title', label: 'Принцип', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
        { key: 'category', label: 'Категория', render: r => `<span class="badge badge-gray">${r.category || '—'}</span>` },
        { key: 'actions', label: '', render: r => `<button class="btn-secondary" style="padding:4px 8px;font-size:11px;color:var(--color-red);" data-pdel="${r.id}">✕</button>` },
      ],
      idField: 'id',
      addButton: '+ Принцип',
      onAdd: () => {
        const title = prompt('Принцип:');
        if (title) invoke('create_principle', { title, description: '', category: 'discipline' }).then(() => loadPrinciples(el)).catch(e => alert(e));
      },
      reloadFn: () => loadPrinciples(el),
    });
    await dbv.render();

    // Delegate delete clicks
    el.addEventListener('click', async (e) => {
      const del = e.target.closest('[data-pdel]');
      if (del) {
        if (await confirmModal('Удалить?')) { await invoke('delete_principle', { id: parseInt(del.dataset.pdel) }).catch(()=>{}); loadPrinciples(el); }
      }
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

// ── Food ──
async function loadFood(subTab) {
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
      onAdd: () => showAddFoodModal(el),
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
      fixedColumns, idField: 'id',
      addButton: '+ Рецепт',
      onAdd: () => showAddRecipeModal(el),
      reloadFn: () => loadRecipes(el),
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
      fixedColumns, idField: 'id',
      addButton: '+ Продукт',
      onAdd: () => showAddProductModal(el),
      reloadFn: () => loadProducts(el),
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

// ── Money ──
async function loadMoney(subTab) {
  const el = document.getElementById('money-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'money', {
    title: 'Money',
    subtitle: 'Финансы и бюджет',
    icon: '💰',
    renderDash: async (paneEl) => {
      const stats = await invoke('get_transaction_stats', { days: 30 }).catch(() => ({}));
      const balance = (stats.total_income || 0) - (stats.total_expenses || 0);
      const subs = await invoke('get_subscriptions').catch(() => []);
      const monthly = subs.filter(s => s.active).reduce((sum, s) => sum + (s.period === 'yearly' ? s.amount / 12 : s.amount), 0);
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card ${balance >= 0 ? 'green' : 'red'}"><div class="uni-dash-value">${balance}</div><div class="uni-dash-label">Баланс (30д)</div></div>
          <div class="uni-dash-card purple"><div class="uni-dash-value">${stats.total_expenses || 0}</div><div class="uni-dash-label">Расходы</div></div>
          <div class="uni-dash-card blue"><div class="uni-dash-value">${stats.total_income || 0}</div><div class="uni-dash-label">Доходы</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${Math.round(monthly)}</div><div class="uni-dash-label">Подписки/мес</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const activeInner = S._moneyInner || 'transactions';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);">
          <button class="pill${activeInner === 'transactions' ? ' active' : ''}" data-inner="transactions">Транзакции</button>
          <button class="pill${activeInner === 'budgets' ? ' active' : ''}" data-inner="budgets">Бюджет</button>
          <button class="pill${activeInner === 'savings' ? ' active' : ''}" data-inner="savings">Накопления</button>
          <button class="pill${activeInner === 'subscriptions' ? ' active' : ''}" data-inner="subscriptions">Подписки</button>
          <button class="pill${activeInner === 'debts' ? ' active' : ''}" data-inner="debts">Долги</button>
        </div>
        <div id="money-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#money-inner-content');
      if (activeInner === 'budgets') await loadBudgets(innerEl);
      else if (activeInner === 'savings') await loadSavings(innerEl);
      else if (activeInner === 'subscriptions') await loadSubscriptions(innerEl);
      else if (activeInner === 'debts') await loadDebts(innerEl);
      else await loadTransactions(innerEl);
      paneEl.querySelectorAll('[data-inner]').forEach(btn => {
        btn.addEventListener('click', () => { S._moneyInner = btn.dataset.inner; loadMoney(); });
      });
    },
  });
}

async function loadTransactions(el) {
  try {
    const txFilter = S.moneyTxFilter;
    const txType = txFilter === 'all' ? null : txFilter;
    const items = await invoke('get_transactions', { txType, category: null, days: 30 }).catch(() => []);
    const stats = await invoke('get_transaction_stats', { days: 30 }).catch(() => ({}));

    // Type filter chips (expense/income/all)
    const typeFilterBar = `<div class="dev-filters" style="margin-bottom:var(--space-2);">
      ${['all','expense','income'].map(f =>
        `<button class="pill${S.moneyTxFilter === f ? ' active' : ''}" data-txfilter="${f}">${f === 'all' ? 'All' : f === 'expense' ? 'Expenses' : 'Income'}</button>`
      ).join('')}
    </div>`;

    // Stats summary
    const statsHtml = `<div class="dashboard-stats" style="margin-bottom:var(--space-3);">
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_expenses || 0}</div><div class="dashboard-stat-label">Expenses (30d)</div></div>
      <div class="dashboard-stat"><div class="dashboard-stat-value">${stats.total_income || 0}</div><div class="dashboard-stat-label">Income (30d)</div></div>
      <div class="dashboard-stat"><div class="dashboard-stat-value">${(stats.total_income || 0) - (stats.total_expenses || 0)}</div><div class="dashboard-stat-label">Balance</div></div>
    </div>`;

    const fixedColumns = [
      { key: 'date', label: 'Date', render: r => `<span style="font-size:12px;color:var(--text-muted);font-variant-numeric:tabular-nums;">${r.date}</span>` },
      { key: 'description', label: 'Description', render: r => `<span class="data-table-title">${escapeHtml(r.description || r.category)}</span>` },
      { key: 'category', label: 'Category', render: r => `<span class="badge badge-gray">${escapeHtml(r.category)}</span>` },
      { key: 'tx_type', label: 'Type', render: r => `<span class="badge ${r.tx_type === 'income' ? 'badge-green' : 'badge-purple'}">${r.tx_type === 'income' ? 'Income' : 'Expense'}</span>` },
      { key: 'amount', label: 'Amount', render: r => {
        const isIncome = r.tx_type === 'income';
        return `<span style="color:${isIncome ? 'var(--color-green)' : 'var(--text-secondary)'};font-variant-numeric:tabular-nums;font-weight:500;">${isIncome ? '+' : '-'} ${r.amount} ${r.currency || 'KZT'}</span>`;
      }},
    ];

    el.innerHTML = typeFilterBar + statsHtml + '<div id="tx-dbv"></div>';
    const dbvEl = document.getElementById('tx-dbv');

    const dbv = new DatabaseView(dbvEl, {
      tabId: 'money',
      recordTable: 'transactions',
      records: items,
      fixedColumns,
      idField: 'id',
      availableViews: ['table', 'list'],
      defaultView: 'table',
      addButton: '+ Add',
      onAdd: () => showAddTransactionModal(el),
      reloadFn: () => loadTransactions(el),
    });
    S.dbViews.transactions = dbv;
    await dbv.render();

    el.querySelectorAll('[data-txfilter]').forEach(btn => {
      btn.addEventListener('click', () => { S.moneyTxFilter = btn.dataset.txfilter; loadTransactions(el); });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddTransactionModal(parentEl) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Add Transaction</div>
    <div class="form-group"><label class="form-label">Type</label>
      <select class="form-select" id="tx-type" style="width:100%;">
        <option value="expense">Expense</option><option value="income">Income</option>
      </select></div>
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
        date: null, txType: document.getElementById('tx-type')?.value || 'expense', amount,
        currency: document.getElementById('tx-currency')?.value || 'KZT',
        category: document.getElementById('tx-category')?.value || 'other',
        description: document.getElementById('tx-desc')?.value || '',
        recurring: false, recurringPeriod: null,
      });
      overlay.remove();
      loadTransactions(parentEl);
    } catch (err) { alert('Error: ' + err); }
  });
}

async function loadBudgets(el) {
  try {
    const budgets = await invoke('get_budgets').catch(() => []);
    const dbv = new DatabaseView(el, {
      tabId: 'money',
      recordTable: 'budgets',
      records: budgets,
      fixedColumns: [
        { key: 'category', label: 'Категория', render: r => `<span class="data-table-title">${escapeHtml(r.category)}</span>` },
        { key: 'amount', label: 'Бюджет', render: r => `<span style="font-variant-numeric:tabular-nums;font-size:12px;">${r.amount}</span>` },
        { key: 'spent', label: 'Потрачено', render: r => {
          const pct = r.amount > 0 ? Math.min(100, Math.round((r.spent||0) / r.amount * 100)) : 0;
          const warn = pct > 80;
          return `<span style="color:${warn?'var(--color-yellow)':'var(--text-secondary)'};font-size:12px;">${r.spent||0}</span>`;
        }},
        { key: 'progress', label: 'Прогресс', render: r => {
          const pct = r.amount > 0 ? Math.min(100, Math.round((r.spent||0) / r.amount * 100)) : 0;
          const warn = pct > 80;
          return `<div class="dev-progress" style="width:80px;display:inline-block;"><div class="dev-progress-bar" style="width:${pct}%;background:${warn?'var(--color-yellow)':'var(--accent-blue)'}"></div></div> <span style="font-size:11px;color:var(--text-faint);">${pct}%</span>`;
        }},
        { key: 'period', label: 'Период', render: r => `<span class="badge badge-gray">${r.period || 'monthly'}</span>` },
      ],
      idField: 'id',
      addButton: '+ Бюджет',
      onAdd: () => {
        const cat = prompt('Категория:');
        const amt = prompt('Сумма:');
        if (cat && amt) invoke('create_budget', { category: cat, amount: parseFloat(amt), period: 'monthly' }).then(() => loadBudgets(el)).catch(e => alert(e));
      },
      reloadFn: () => loadBudgets(el),
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadSavings(el) {
  try {
    const goals = await invoke('get_savings_goals').catch(() => []);
    const dbv = new DatabaseView(el, {
      tabId: 'money',
      recordTable: 'savings_goals',
      records: goals,
      fixedColumns: [
        { key: 'name', label: 'Цель', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
        { key: 'current_amount', label: 'Накоплено', render: r => `<span style="font-variant-numeric:tabular-nums;font-size:12px;">${r.current_amount || 0}</span>` },
        { key: 'target_amount', label: 'Цель', render: r => `<span style="font-variant-numeric:tabular-nums;font-size:12px;">${r.target_amount || 0}</span>` },
        { key: 'progress', label: 'Прогресс', render: r => {
          const pct = r.target_amount > 0 ? Math.min(100, Math.round(r.current_amount / r.target_amount * 100)) : 0;
          return `<div class="dev-progress" style="width:80px;display:inline-block;"><div class="dev-progress-bar" style="width:${pct}%"></div></div> <span style="font-size:11px;color:var(--text-faint);">${pct}%</span>`;
        }},
        { key: 'deadline', label: 'Дедлайн', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.deadline || '—'}</span>` },
        { key: 'actions', label: '', render: r => `<button class="btn-secondary" style="padding:4px 8px;font-size:11px;" data-sadd="${r.id}">+ Пополнить</button>` },
      ],
      idField: 'id',
      addButton: '+ Цель',
      onAdd: () => {
        const name = prompt('Название цели:');
        const target = prompt('Целевая сумма:');
        if (name && target) invoke('create_savings_goal', { name, targetAmount: parseFloat(target), currentAmount: 0, deadline: null, color: '#9B9B9B' }).then(() => loadSavings(el)).catch(e => alert(e));
      },
      reloadFn: () => loadSavings(el),
    });
    await dbv.render();

    // Add funds buttons (delegated)
    el.addEventListener('click', async (e) => {
      const btn = e.target.closest('[data-sadd]');
      if (!btn) return;
      const amount = prompt('Сумма пополнения:');
      if (amount) {
        const goal = goals.find(g => g.id === parseInt(btn.dataset.sadd));
        if (goal) {
          await invoke('update_savings_goal', { id: goal.id, currentAmount: (goal.current_amount||0) + parseFloat(amount), name: null, targetAmount: null, deadline: null }).catch(e => alert(e));
          loadSavings(el);
        }
      }
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadSubscriptions(el) {
  try {
    const subs = await invoke('get_subscriptions').catch(() => []);
    const dbv = new DatabaseView(el, {
      tabId: 'money',
      recordTable: 'subscriptions',
      records: subs,
      fixedColumns: [
        { key: 'name', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
        { key: 'amount', label: 'Сумма', render: r => `<span style="font-variant-numeric:tabular-nums;font-size:12px;">${r.amount} ${r.currency || 'KZT'}</span>` },
        { key: 'period', label: 'Период', render: r => `<span class="badge badge-gray">${r.period === 'yearly' ? 'Годовая' : 'Месячная'}</span>` },
        { key: 'active', label: 'Статус', render: r => r.active ? '<span class="badge badge-green">Активна</span>' : '<span class="badge badge-gray">Пауза</span>' },
        { key: 'next_payment', label: 'Следующий платеж', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.next_payment || '—'}</span>` },
      ],
      idField: 'id',
      addButton: '+ Добавить',
      onAdd: () => showAddSubscriptionModal(el),
      reloadFn: () => loadSubscriptions(el),
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddSubscriptionModal(parentEl) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить подписку</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="sub-name"></div>
    <div class="form-group"><label class="form-label">Сумма</label><input class="form-input" id="sub-amount" type="number"></div>
    <div class="form-group"><label class="form-label">Период</label>
      <select class="form-select" id="sub-period" style="width:100%;"><option value="monthly">Месячная</option><option value="yearly">Годовая</option></select></div>
    <div class="form-group"><label class="form-label">Категория</label><input class="form-input" id="sub-cat" placeholder="entertainment, tools..."></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="sub-save">Сохранить</button>
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
      loadSubscriptions(parentEl);
    } catch (err) { alert('Error: ' + err); }
  });
}

async function loadDebts(el) {
  try {
    const debts = await invoke('get_debts').catch(() => []);
    const dbv = new DatabaseView(el, {
      tabId: 'money',
      recordTable: 'debts',
      records: debts,
      fixedColumns: [
        { key: 'name', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
        { key: 'type', label: 'Тип', render: r => `<span class="badge ${r.type === 'owe' ? 'badge-purple' : 'badge-green'}">${r.type === 'owe' ? 'Я должен' : 'Мне должны'}</span>` },
        { key: 'remaining', label: 'Остаток', render: r => `<span style="font-variant-numeric:tabular-nums;font-size:12px;">${r.remaining || 0}</span>` },
        { key: 'amount', label: 'Сумма', render: r => `<span style="font-variant-numeric:tabular-nums;font-size:12px;color:var(--text-muted);">${r.amount || 0}</span>` },
        { key: 'progress', label: 'Прогресс', render: r => {
          const pct = r.amount > 0 ? Math.min(100, Math.round((r.amount - r.remaining) / r.amount * 100)) : 0;
          return `<div class="dev-progress" style="width:80px;display:inline-block;"><div class="dev-progress-bar" style="width:${pct}%"></div></div> <span style="font-size:11px;color:var(--text-faint);">${pct}%</span>`;
        }},
      ],
      idField: 'id',
      addButton: '+ Долг',
      onAdd: () => {
        const name = prompt('Название:');
        const type = prompt('Тип (owe/owed):') || 'owe';
        const amount = prompt('Сумма:');
        if (name && amount) invoke('add_debt', { name, debtType: type, amount: parseFloat(amount), remaining: parseFloat(amount), interestRate: null, dueDate: null, description: '' }).then(() => loadDebts(el)).catch(e => alert(e));
      },
      reloadFn: () => loadDebts(el),
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

// ── People Tab ──
async function loadPeople(subTab) {
  const el = document.getElementById('people-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'people', {
    title: 'People',
    subtitle: 'Контакты и связи',
    icon: '👥',
    renderDash: async (paneEl) => {
      const items = await invoke('get_contacts', {}).catch(() => []);
      const contacts = Array.isArray(items) ? items : [];
      const favs = contacts.filter(c => c.favorite).length;
      const blocked = contacts.filter(c => c.blocked).length;
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${contacts.length}</div><div class="uni-dash-label">Контактов</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${favs}</div><div class="uni-dash-label">Избранных</div></div>
          <div class="uni-dash-card purple"><div class="uni-dash-value">${blocked}</div><div class="uni-dash-label">Заблокировано</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const activeInner = S._peopleInner || 'all';
      const filter = activeInner === 'blocked' ? { blocked: true } : {};
      try {
        const items = await invoke('get_contacts', filter);
        let contacts = Array.isArray(items) ? items : [];
        if (activeInner === 'favorites') contacts = contacts.filter(c => c.favorite);

        paneEl.innerHTML = `
          <div class="dev-filters" style="margin-bottom:var(--space-3);">
            <button class="pill${activeInner === 'all' ? ' active' : ''}" data-inner="all">Все</button>
            <button class="pill${activeInner === 'favorites' ? ' active' : ''}" data-inner="favorites">Избранные</button>
            <button class="pill${activeInner === 'blocked' ? ' active' : ''}" data-inner="blocked">Заблокированные</button>
          </div>
          <div id="people-dbv"></div>`;

        const dbvEl = paneEl.querySelector('#people-dbv');
        const dbv = new DatabaseView(dbvEl, {
          tabId: 'people',
          recordTable: 'contacts',
          records: contacts,
          fixedColumns: [
            { key: 'name', label: 'Имя', render: r => `<span class="data-table-title">${escapeHtml(r.name)}${r.favorite ? ' ★' : ''}</span>` },
            { key: 'category', label: 'Категория', render: r => `<span class="badge badge-gray">${r.category || r.relationship || '—'}</span>` },
            { key: 'phone', label: 'Телефон', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.phone || '—'}</span>` },
            { key: 'email', label: 'Email', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.email || '—'}</span>` },
            { key: 'status', label: 'Статус', render: r => r.blocked ? '<span class="badge badge-red">Blocked</span>' : '<span class="badge badge-green">OK</span>' },
            { key: 'actions', label: '', render: r => `
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleContactFav(${r.id})">${r.favorite ? '★' : '☆'}</button>
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleContactBlock(${r.id})">${r.blocked ? '🔓' : '🚫'}</button>
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;color:var(--color-red);" onclick="deleteContact(${r.id})">✕</button>
            ` },
          ],
          idField: 'id',
          availableViews: ['table', 'list'],
          defaultView: 'table',
          addButton: '+ Добавить',
          onAdd: () => showAddContactModal(),
          reloadFn: () => loadPeople(),
        });
        await dbv.render();

        paneEl.querySelectorAll('[data-inner]').forEach(btn => {
          btn.addEventListener('click', () => { S._peopleInner = btn.dataset.inner; loadPeople(); });
        });
      } catch (e) {
        paneEl.innerHTML = `<div class="uni-empty">Ошибка: ${e}</div>`;
      }
    },
  });
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

  // Use unified layout
  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'work', {
    title: 'Work',
    subtitle: 'Проекты и задачи',
    icon: '💼',
    renderDash: async (paneEl) => {
      // Dashboard with stats
      const projects = await invoke('get_projects').catch(() => []);
      const totalTasks = projects.reduce((sum, p) => sum + (p.task_count || 0), 0);
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${projects.length}</div><div class="uni-dash-label">Проекты</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${totalTasks}</div><div class="uni-dash-label">Всего задач</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      // Simple tasks table (all tasks across projects)
      try {
        const projects = await invoke('get_projects').catch(() => []);
        let allTasks = [];
        for (const p of projects) {
          const tasks = await invoke('get_tasks', { projectId: p.id }).catch(() => []);
          allTasks.push(...tasks.map(t => ({ ...t, projectName: p.name, projectColor: p.color })));
        }
        renderWorkTasks(paneEl, allTasks, projects);
      } catch (e) {
        paneEl.innerHTML = '<div class="uni-empty">Не удалось загрузить задачи</div>';
      }
    },
  });
}

// ── Projects tab ──
async function loadProjects() {
  const el = document.getElementById('projects-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'projects', {
    title: 'Projects',
    subtitle: 'Проекты и их задачи',
    icon: '📁',
    renderDash: async (paneEl) => {
      const projects = await invoke('get_projects').catch(() => []);
      const totalTasks = projects.reduce((sum, p) => sum + (p.task_count || 0), 0);
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${projects.length}</div><div class="uni-dash-label">Проекты</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${totalTasks}</div><div class="uni-dash-label">Всего задач</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const projects = await invoke('get_projects').catch(() => []);
      await renderProjectsView(paneEl, projects || []);
    },
  });
}

async function renderProjectsView(el, projects) {
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
    item.addEventListener('click', () => { S.currentProjectId = p.id; loadProjects(); });
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
      loadProjects();
    });
    taskList.appendChild(item);
  }

  document.getElementById('new-project-btn')?.addEventListener('click', () => {
    const name = prompt('Название проекта:');
    if (name) invoke('create_project', { name, description: '', color: '#9B9B9B' }).then(() => loadProjects()).catch(e => alert(e));
  });

  document.getElementById('new-task-btn')?.addEventListener('click', () => {
    const title = prompt('Задача:');
    if (title) invoke('create_task', { projectId: S.currentProjectId, title, description: '', priority: 'normal', dueDate: null }).then(() => loadProjects()).catch(e => alert(e));
  });
}

// ── Work: simple tasks table ──
function renderWorkTasks(el, tasks, projects) {
  const statusLabels = { todo: 'To Do', in_progress: 'В работе', done: 'Готово' };
  const statusColors = { todo: 'badge-gray', in_progress: 'badge-blue', done: 'badge-green' };
  const priorityColors = { high: 'badge-red', normal: 'badge-gray', low: 'badge-gray' };

  const dbv = new DatabaseView(el, {
    tabId: 'work',
    recordTable: 'tasks',
    records: tasks,
    fixedColumns: [
      { key: 'done', label: '', render: r => `<div class="work-task-check${r.status === 'done' ? ' done' : ''}" data-tid="${r.id}" style="cursor:pointer;"></div>` },
      { key: 'title', label: 'Задача', render: r => `<span class="data-table-title" style="${r.status === 'done' ? 'text-decoration:line-through;opacity:0.5;' : ''}">${escapeHtml(r.title)}</span>` },
      { key: 'projectName', label: 'Проект', render: r => `<span style="color:${r.projectColor || 'var(--text-secondary)'};font-size:12px;">${escapeHtml(r.projectName || '')}</span>` },
      { key: 'priority', label: 'Приоритет', render: r => `<span class="badge ${priorityColors[r.priority] || 'badge-gray'}">${r.priority || 'normal'}</span>` },
      { key: 'status', label: 'Статус', render: r => `<span class="badge ${statusColors[r.status] || 'badge-gray'}">${statusLabels[r.status] || r.status}</span>` },
    ],
    idField: 'id',
    availableViews: ['table', 'kanban', 'list'],
    defaultView: 'table',
    addButton: '+ Задача',
    onAdd: () => showAddTaskModal(projects),
    reloadFn: () => loadWork(),
    kanban: {
      groupByField: 'status',
      columns: [
        { key: 'todo', label: 'To Do', icon: '📋' },
        { key: 'in_progress', label: 'В работе', icon: '▶' },
        { key: 'done', label: 'Готово', icon: '✅' },
      ],
    },
    onDrop: async (recordId, field, newValue) => {
      await invoke('update_task_status', { id: parseInt(recordId), status: newValue }).catch(() => {});
      loadWork();
    },
  });
  dbv.render();

  // Delegate click for task checkboxes
  el.addEventListener('click', async (e) => {
    const check = e.target.closest('[data-tid]');
    if (!check) return;
    const id = parseInt(check.dataset.tid);
    const task = tasks.find(t => t.id === id);
    const newStatus = task?.status === 'done' ? 'todo' : 'done';
    await invoke('update_task_status', { id, status: newStatus }).catch(() => {});
    loadWork();
  });
}

function showAddTaskModal(projects) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новая задача</div>
    <div class="form-group"><label class="form-label">Задача</label><input class="form-input" id="wt-title" placeholder="Название задачи"></div>
    <div class="form-group"><label class="form-label">Проект</label>
      <select class="form-input" id="wt-project">
        ${projects.map(p => `<option value="${p.id}">${escapeHtml(p.name)}</option>`).join('')}
        ${projects.length === 0 ? '<option value="">Нет проектов</option>' : ''}
      </select>
    </div>
    <div class="form-group"><label class="form-label">Приоритет</label>
      <select class="form-input" id="wt-priority">
        <option value="normal">Normal</option>
        <option value="high">High</option>
        <option value="low">Low</option>
      </select>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" id="wt-cancel">Отмена</button>
      <button class="btn-primary" id="wt-save">Создать</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('wt-cancel')?.addEventListener('click', () => overlay.remove());
  document.getElementById('wt-save')?.addEventListener('click', async () => {
    const title = document.getElementById('wt-title')?.value?.trim();
    if (!title) return;
    const projectId = parseInt(document.getElementById('wt-project')?.value);
    const priority = document.getElementById('wt-priority')?.value || 'normal';
    if (!projectId) { alert('Сначала создайте проект'); return; }
    await invoke('create_task', { projectId, title, description: '', priority, dueDate: null }).catch(e => alert(e));
    overlay.remove();
    loadWork();
  });
}

// ── Development ──
async function loadDevelopment() {
  const el = document.getElementById('development-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'development', {
    title: 'Development',
    subtitle: 'Обучение и навыки',
    icon: '🚀',
    renderDash: async (paneEl) => {
      const items = await invoke('get_learning_items', { typeFilter: null }).catch(() => []);
      const inProgress = items.filter(i => i.status === 'in_progress').length;
      const completed = items.filter(i => i.status === 'completed').length;
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${items.length}</div><div class="uni-dash-label">Всего</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${inProgress}</div><div class="uni-dash-label">В процессе</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${completed}</div><div class="uni-dash-label">Завершено</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      try {
        const items = await invoke('get_learning_items', { typeFilter: S.devFilter === 'all' ? null : S.devFilter }).catch(() => []);
        renderDevelopment(paneEl, items || []);
      } catch (e) {
        paneEl.innerHTML = '<div class="uni-empty">Не удалось загрузить</div>';
      }
    },
  });
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

  const dbv = new DatabaseView(dbvEl, {
    tabId: 'development',
    recordTable: 'learning_items',
    records: items,
    fixedColumns,
    idField: 'id',
    availableViews: ['table', 'kanban', 'list'],
    defaultView: 'table',
    addButton: '+ Добавить',
    onAdd: () => showAddLearningModal(),
    reloadFn: () => loadDevelopment(),
    kanban: {
      groupByField: 'status',
      columns: [
        { key: 'planned', label: 'Запланировано', icon: '\ud83d\udccb' },
        { key: 'in_progress', label: 'В процессе', icon: '\u25b6' },
        { key: 'completed', label: 'Завершено', icon: '\u2705' },
      ],
    },
    onDrop: async (recordId, field, newValue) => {
      try {
        await invoke('update_learning_item_status', { id: parseInt(recordId), status: newValue });
        loadDevelopment();
      } catch (err) { console.error('kanban drop:', err); }
    },
  });
  S.dbViews.development = dbv;
  dbv.render();

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

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'hobbies', {
    title: 'Hobbies',
    subtitle: 'Медиа-коллекции',
    icon: '🎮',
    renderDash: async (paneEl) => {
      await loadHobbiesOverview(paneEl);
    },
    renderTable: async (paneEl) => {
      const activeMedia = S._hobbiesMedia || 'music';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);flex-wrap:wrap;">
          ${MEDIA_TYPES.map(t => `<button class="pill${activeMedia === t ? ' active' : ''}" data-media="${t}">${MEDIA_LABELS[t]}</button>`).join('')}
        </div>
        <div id="hobbies-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#hobbies-inner-content');
      await loadMediaList(innerEl, activeMedia);
      paneEl.querySelectorAll('[data-media]').forEach(btn => {
        btn.addEventListener('click', () => { S._hobbiesMedia = btn.dataset.media; loadHobbies(); });
      });
    },
  });
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
    const hasEp = ['anime','series','cartoon','manga','podcast'].includes(mediaType);

    // Status filter bar (domain filter — above DatabaseView)
    const filterBar = `<div class="dev-filters">
      ${['all','planned','in_progress','completed','on_hold','dropped'].map(s =>
        `<button class="pill${S.mediaStatusFilter === s ? ' active' : ''}" data-filter="${s}">${s === 'all' ? 'All' : STATUS_LABELS[s]}</button>`
      ).join('')}
    </div>`;

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

    el.innerHTML = filterBar + '<div id="media-dbv"></div>';
    const dbvEl = document.getElementById('media-dbv');

    const dbv = new DatabaseView(dbvEl, {
      tabId: 'hobbies',
      recordTable: 'media_items',
      records: items,
      fixedColumns,
      idField: 'id',
      availableViews: ['table', 'kanban', 'gallery'],
      defaultView: 'table',
      addButton: '+ Add',
      onAdd: () => showAddMediaModal(mediaType),
      onRowClick: (record) => showMediaDetail(record, mediaType),
      reloadFn: () => loadMediaList(el, mediaType),
      kanban: {
        groupByField: 'status',
        columns: [
          { key: 'planned', label: 'Planned', icon: '\ud83d\udccb' },
          { key: 'in_progress', label: 'In Progress', icon: '\u25b6' },
          { key: 'completed', label: 'Completed', icon: '\u2705' },
          { key: 'on_hold', label: 'On Hold', icon: '\u23f8' },
          { key: 'dropped', label: 'Dropped', icon: '\u274c' },
        ],
      },
      gallery: {
        minCardWidth: 200,
        renderCard: (r) => {
          const stars = r.rating ? '\u2605'.repeat(Math.round(r.rating / 2)) + '\u2606'.repeat(5 - Math.round(r.rating / 2)) : '';
          return `<div class="dbv-gallery-card-title">${escapeHtml(r.title)}</div>
            <div class="dbv-gallery-card-badges">
              <span class="badge ${r.status === 'completed' ? 'badge-green' : r.status === 'in_progress' ? 'badge-blue' : 'badge-gray'}">${STATUS_LABELS[r.status] || r.status}</span>
              ${r.year ? `<span style="font-size:11px;color:var(--text-muted);">${r.year}</span>` : ''}
            </div>
            ${stars ? `<div style="font-size:12px;color:var(--text-secondary);margin-top:auto;">${stars}</div>` : ''}`;
        },
      },
      onDrop: async (recordId, field, newValue) => {
        try {
          await invoke('update_media_item', {
            id: parseInt(recordId), status: newValue,
            rating: null, progress: null, notes: null, title: null, description: null, coverUrl: null, totalEpisodes: null,
          });
          loadMediaList(el, mediaType);
        } catch (err) { console.error('kanban drop:', err); }
      },
    });
    S.dbViews[`hobbies_${mediaType}`] = dbv;
    await dbv.render();

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

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'sports', {
    title: 'Sports',
    subtitle: 'Тренировки и физическая активность',
    icon: '💪',
    renderDash: async (paneEl) => {
      const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${stats.count || 0}</div><div class="uni-dash-label">Тренировок</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${stats.total_minutes || 0}м</div><div class="uni-dash-label">Общее время</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${stats.total_calories || 0}</div><div class="uni-dash-label">Калории</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const activeInner = S._sportsInner || 'workouts';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);">
          <button class="pill${activeInner === 'workouts' ? ' active' : ''}" data-inner="workouts">Тренировки</button>
          <button class="pill${activeInner === 'martial_arts' ? ' active' : ''}" data-inner="martial_arts">Единоборства</button>
          <button class="pill${activeInner === 'stats' ? ' active' : ''}" data-inner="stats">Статистика</button>
        </div>
        <div id="sports-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#sports-inner-content');
      if (activeInner === 'martial_arts') await loadMartialArts(innerEl);
      else if (activeInner === 'stats') await loadSportsStats(innerEl);
      else {
        const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
        const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
        renderSports(innerEl, workouts || [], stats);
      }
      paneEl.querySelectorAll('[data-inner]').forEach(btn => {
        btn.addEventListener('click', () => { S._sportsInner = btn.dataset.inner; loadSports(); });
      });
    },
  });
}

async function loadMartialArts(el) {
  try {
    const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
    const ma = (workouts || []).filter(w => w.type === 'martial_arts');
    const dbv = new DatabaseView(el, {
      tabId: 'sports',
      recordTable: 'workouts',
      records: ma,
      fixedColumns: [
        { key: 'date', label: 'Дата', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.date || '—'}</span>` },
        { key: 'title', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.title || 'Единоборства')}</span>` },
        { key: 'duration_minutes', label: 'Время', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.duration_minutes || 0} мин</span>` },
        { key: 'calories', label: 'Калории', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.calories || '—'}</span>` },
      ],
      idField: 'id',
      addButton: '+ Тренировка',
      onAdd: () => {
        showAddWorkoutModal();
        setTimeout(() => { const sel = document.getElementById('workout-type'); if (sel) sel.value = 'martial_arts'; }, 50);
      },
      reloadFn: () => loadSports(),
    });
    await dbv.render();
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

  const dbv = new DatabaseView(el, {
    tabId: 'sports',
    recordTable: 'workouts',
    records: workouts,
    fixedColumns: [
      { key: 'date', label: 'Дата', render: r => `<span style="font-size:12px;color:var(--text-muted);font-variant-numeric:tabular-nums;">${r.date || '—'}</span>` },
      { key: 'title', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.title || typeLabels[r.type] || r.type)}</span>` },
      { key: 'type', label: 'Тип', render: r => `<span class="badge badge-purple">${typeLabels[r.type] || r.type}</span>` },
      { key: 'duration_minutes', label: 'Время', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.duration_minutes || 0} мин</span>` },
      { key: 'calories', label: 'Калории', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.calories || '—'}</span>` },
    ],
    idField: 'id',
    availableViews: ['table', 'list'],
    defaultView: 'table',
    addButton: '+ Тренировка',
    onAdd: () => showAddWorkoutModal(),
    reloadFn: () => loadSports(),
  });
  dbv.render();
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

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'health', {
    title: 'Health',
    subtitle: 'Здоровье и привычки',
    icon: '❤️',
    renderDash: async (paneEl) => {
      const today = await invoke('get_health_today').catch(() => ({}));
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${today.sleep ? today.sleep + 'ч' : '—'}</div><div class="uni-dash-label">Сон</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${today.water || '—'}</div><div class="uni-dash-label">Вода (стаканов)</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${today.mood ? today.mood + '/5' : '—'}</div><div class="uni-dash-label">Настроение</div></div>
          <div class="uni-dash-card purple"><div class="uni-dash-value">${today.weight ? today.weight + 'кг' : '—'}</div><div class="uni-dash-label">Вес</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      try {
        const today = await invoke('get_health_today').catch(() => ({}));
        const habits = await invoke('get_habits_today').catch(() => []);
        renderHealth(paneEl, today, habits);
      } catch (e) {
        paneEl.innerHTML = '<div class="uni-empty">Не удалось загрузить данные здоровья</div>';
      }
    },
  });
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

  // Health metrics (clickable cards) + habits table via DatabaseView
  el.innerHTML = `
    <div class="health-metrics" style="margin-bottom:var(--space-4);">
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
    <div id="habits-dbv"></div>`;

  // Habits as DatabaseView
  const dbvEl = el.querySelector('#habits-dbv');
  const dbv = new DatabaseView(dbvEl, {
    tabId: 'health',
    recordTable: 'habits',
    records: habits,
    fixedColumns: [
      { key: 'done', label: '', render: r => `<div class="habit-check${r.completed ? ' checked' : ''}" style="cursor:pointer;" data-hid="${r.id}">${r.completed ? '&#10003;' : ''}</div>` },
      { key: 'name', label: 'Привычка', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
      { key: 'frequency', label: 'Частота', render: r => `<span class="badge badge-gray">${r.frequency || 'daily'}</span>` },
      { key: 'streak', label: 'Серия', render: r => r.streak > 0 ? `<span class="badge badge-green">${r.streak} дн.</span>` : '<span style="color:var(--text-faint);font-size:12px;">—</span>' },
    ],
    idField: 'id',
    addButton: '+ Привычка',
    onAdd: () => {
      const name = prompt('Название привычки:');
      if (name) invoke('create_habit', { name, icon: '', frequency: 'daily' }).then(() => loadHealth()).catch(e => alert(e));
    },
    reloadFn: () => loadHealth(),
  });
  dbv.render();

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

  // Delegate habit check clicks
  el.addEventListener('click', async (e) => {
    const check = e.target.closest('[data-hid]');
    if (!check) return;
    await invoke('check_habit', { habitId: parseInt(check.dataset.hid), date: null }).catch(() => {});
    loadHealth();
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
  loadProjects,
  loadDevelopment,
  loadHobbies,
  loadSports,
  loadHealth,
  loadCustomPage,
  panelItem,
};
