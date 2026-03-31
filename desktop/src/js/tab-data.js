// ── js/tab-data.js — Data tab loaders: Home, Mindset, Food, Money, People, Memory, Work, Development, Hobbies, Sports, Health, About, Custom Pages ──

import { S, invoke, tabLoaders, TAB_REGISTRY, TAB_ICONS, MEDIA_TYPES, MEDIA_LABELS, STATUS_LABELS, MEMORY_CATEGORIES } from './state.js';
import { showEmojiPicker } from './emoji-picker.js';
import { escapeHtml, renderMarkdown, renderPageHeader, setupPageHeaderControls, confirmModal, skeletonPage, skeletonGrid, skeletonList, skeletonSettings, initBlockEditor, blocksToPlainText, migrateTextToBlocks, loadTabBlockEditor } from './utils.js';
import { renderTabBar, closeTab } from './tabs.js';
import { DatabaseView } from './db-view/db-view.js';
import { formatRecurrence } from './db-view/db-recurrence-editor.js';

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
    renderTable: async (paneEl) => {
      const items = await invoke('get_home_items', { category: null, neededOnly: false }).catch(() => []);
      const categories = { cleaning: 'Уборка', hygiene: 'Гигиена', household: 'Дом', electronics: 'Техника', tools: 'Инструменты', other: 'Другое' };
      const homeCatColors = { cleaning: 'blue', hygiene: 'pink', household: 'orange', electronics: 'purple', tools: 'yellow', other: 'gray' };
      const catOptions = Object.entries(categories).map(([k, v]) => ({ value: k, label: v, color: homeCatColors[k] || 'gray' }));
      const dbv = new DatabaseView(paneEl, {
        tabId: 'home', recordTable: 'home_items', records: items,
        fixedColumns: [
          { key: 'name', label: 'Название', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
          { key: 'category', label: 'Категория', editable: true, editType: 'select', editOptions: catOptions, render: r => `<span class="badge badge-${homeCatColors[r.category] || 'gray'}">${categories[r.category] || r.category}</span>` },
          { key: 'quantity', label: 'Кол-во', editable: true, editType: 'number', render: r => r.quantity != null ? `${r.quantity} ${r.unit || ''}` : '—' },
          { key: 'location', label: 'Место', editable: true, editType: 'text', render: r => r.location || '—' },
          { key: 'needed', label: 'Статус', render: r => r.needed ? '<span class="badge badge-red">Нужно</span>' : '<span class="badge badge-green">Есть</span>' },
        ],
        addButton: '+ Добавить',
        onAdd: () => { showHomeAddModal(); },
        onQuickAdd: async () => {
          await invoke('add_home_item', { name: '', category: '', location: '' });
          loadHome();
        },
        onCellEdit: async (recordId, key, value, skipReload) => {
          const params = { id: recordId, name: null, category: null, quantity: null, location: null, notes: null, needed: null };
          if (key === 'quantity') params.quantity = parseFloat(value) || null;
          else params[key] = value;
          await invoke('update_home_item', params);
          if (!skipReload) loadHome();
        },
        onDelete: async (id) => { await invoke('delete_home_item', { id }); },
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
    renderTable: async (paneEl) => {
      await loadJournal(paneEl);
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

// ── Food ──
async function loadFood(subTab) {
  const el = document.getElementById('food-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'food', {
    title: 'Food',
    subtitle: 'Питание и продукты',
    icon: '🍔',
    renderTable: async (paneEl) => {
      await loadFoodLog(paneEl);
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
        { key: 'name', label: 'Название', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
        { key: 'meal_type', label: 'Приём', editable: true, editType: 'select', editOptions: [
          { value: 'breakfast', label: 'Завтрак', color: 'yellow' }, { value: 'lunch', label: 'Обед', color: 'green' },
          { value: 'dinner', label: 'Ужин', color: 'purple' }, { value: 'snack', label: 'Перекус', color: 'orange' },
        ], render: r => {
          const mc = { breakfast: 'yellow', lunch: 'green', dinner: 'purple', snack: 'orange' };
          return `<span class="badge badge-${mc[r.meal_type] || 'gray'}">${mealLabels[r.meal_type] || r.meal_type}</span>`;
        }},
        { key: 'calories', label: 'Калории', editable: true, editType: 'number', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.calories || 0} kcal</span>` },
        { key: 'protein', label: 'Белок', editable: true, editType: 'number', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.protein || '—'}g</span>` },
        { key: 'carbs', label: 'Углев.', editable: true, editType: 'number', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.carbs || '—'}g</span>` },
        { key: 'fat', label: 'Жиры', editable: true, editType: 'number', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.fat || '—'}g</span>` },
      ],
      idField: 'id',
      addButton: '+ Записать',
      onAdd: () => showAddFoodModal(el),
      onQuickAdd: async () => {
        await invoke('log_food', { mealType: '', name: '' });
        loadFoodLog(el);
      },
      onCellEdit: async (recordId, key, value, skipReload) => {
        const params = { id: recordId, name: null, mealType: null, calories: null, protein: null, carbs: null, fat: null };
        if (['calories'].includes(key)) params[key] = parseInt(value) || null;
        else if (['protein', 'carbs', 'fat'].includes(key)) params[key] = parseFloat(value) || null;
        else if (key === 'meal_type') params.mealType = value;
        else params[key] = value;
        await invoke('update_food_entry', params);
        if (!skipReload) loadFoodLog(el);
      },
      onDelete: async (id) => { await invoke('delete_food_entry', { id }); },
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

// ── Money ──
async function loadMoney(subTab) {
  const el = document.getElementById('money-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'money', {
    title: 'Money',
    subtitle: 'Финансы и бюджет',
    icon: '💰',
    renderTable: async (paneEl) => {
      await loadTransactions(paneEl);
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
      { key: 'description', label: 'Description', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.description || r.category)}</span>` },
      { key: 'category', label: 'Category', editable: true, editType: 'text', render: r => `<span class="badge badge-gray">${escapeHtml(r.category)}</span>` },
      { key: 'tx_type', label: 'Type', editable: true, editType: 'select', editOptions: [
        { value: 'expense', label: 'Expense', color: 'purple' }, { value: 'income', label: 'Income', color: 'green' },
      ], render: r => `<span class="badge ${r.tx_type === 'income' ? 'badge-green' : 'badge-purple'}">${r.tx_type === 'income' ? 'Income' : 'Expense'}</span>` },
      { key: 'amount', label: 'Amount', editable: true, editType: 'number', render: r => {
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
      onQuickAdd: async () => {
        await invoke('add_transaction', { transactionType: '', amount: 0, category: '' });
        loadTransactions(el);
      },
      onCellEdit: async (recordId, key, value, skipReload) => {
        const params = { id: recordId, amount: null, category: null, description: null, txType: null };
        if (key === 'amount') params.amount = parseFloat(value) || null;
        else if (key === 'tx_type') params.txType = value;
        else params[key] = value;
        await invoke('update_transaction', params);
        if (!skipReload) loadTransactions(el);
      },
      onDelete: async (id) => { await invoke('delete_transaction', { id }); },
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

// ── People Tab ──
async function loadPeople(subTab) {
  const el = document.getElementById('people-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'people', {
    title: 'People',
    subtitle: 'Контакты и связи',
    icon: '👥',
    renderTable: async (paneEl) => {
      try {
        const items = await invoke('get_contacts', {});
        const contacts = Array.isArray(items) ? items : [];

        paneEl.innerHTML = '<div id="people-dbv"></div>';

        const dbvEl = paneEl.querySelector('#people-dbv');
        const dbv = new DatabaseView(dbvEl, {
          tabId: 'people',
          recordTable: 'contacts',
          records: contacts,
          fixedColumns: [
            { key: 'name', label: 'Имя', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.name)}${r.favorite ? ' ★' : ''}</span>` },
            { key: 'category', label: 'Категория', editable: true, editType: 'text', render: r => `<span class="badge badge-gray">${r.category || r.relationship || '—'}</span>` },
            { key: 'phone', label: 'Телефон', editable: true, editType: 'phone', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.phone || '—'}</span>` },
            { key: 'email', label: 'Email', editable: true, editType: 'text', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.email || '—'}</span>` },
            { key: 'status', label: 'Статус', render: r => r.blocked ? '<span class="badge badge-red">Blocked</span>' : '<span class="badge badge-green">OK</span>' },
            { key: 'actions', label: '', render: r => `
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleContactFav(${r.id})">${r.favorite ? '★' : '☆'}</button>
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleContactBlock(${r.id})">${r.blocked ? '🔓' : '🚫'}</button>
            ` },
          ],
          idField: 'id',
          availableViews: ['table', 'list'],
          defaultView: 'table',
          addButton: '+ Добавить',
          onAdd: () => showAddContactModal(),
          onQuickAdd: async () => {
            await invoke('add_contact', { name: '' });
            loadPeople();
          },
          onCellEdit: async (recordId, key, value, skipReload) => {
            const params = { id: recordId, name: null, phone: null, email: null, category: null, relationship: null, notes: null, birthday: null, favorite: null, blocked: null };
            params[key] = value;
            await invoke('update_contact', params);
            if (!skipReload) loadPeople();
          },
          onDelete: async (id) => { await invoke('delete_contact', { id }); },
          reloadFn: () => loadPeople(),
        });
        await dbv.render();
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

// ── Jobs ──
async function loadJobs() {
  const el = document.getElementById('jobs-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'jobs', {
    title: 'Jobs',
    subtitle: 'Поиск работы',
    icon: '💼',
    renderTable: async (paneEl) => {
      const vacancies = await invoke('get_job_vacancies', { stage: null }).catch(() => []);
      renderVacanciesTable(paneEl, vacancies);
    },
    renderStore: async (paneEl) => {
      const { renderJobsMemory } = await import('./jobs-memory.js');
      await renderJobsMemory(paneEl);
    },
  });
}

function renderVacanciesTable(el, vacancies) {
  const stageLabels = { found: 'Найдена', saved: 'Сохранена', applied: 'Отклик', responded: 'Ответ', interview: 'Интервью', offer: 'Оффер', accepted: 'Принято', rejected: 'Отказ', ignored: 'Пропущена' };
  const stageColors = { found: 'badge-gray', saved: 'badge-blue', applied: 'badge-yellow', responded: 'badge-purple', interview: 'badge-green', offer: 'badge-green', accepted: 'badge-green', rejected: 'badge-red', ignored: 'badge-gray' };
  const stageColorMap = { found: 'gray', saved: 'blue', applied: 'yellow', responded: 'purple', interview: 'green', offer: 'green', accepted: 'green', rejected: 'red', ignored: 'gray' };
  const stageOptions = Object.entries(stageLabels).map(([k, v]) => ({ value: k, label: v, color: stageColorMap[k] || 'gray' }));

  const dbv = new DatabaseView(el, {
    tabId: 'jobs', recordTable: 'job_vacancies', records: vacancies,
    fixedColumns: [
      { key: 'position', label: 'Позиция', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.company && r.position ? `${r.company} — ${r.position}` : r.position || r.company || '')}</span>` },
      { key: 'stage', label: 'Этап', editable: true, editType: 'select', editOptions: stageOptions, render: r => `<span class="badge ${stageColors[r.stage] || 'badge-gray'}">${stageLabels[r.stage] || r.stage}</span>` },
      { key: 'url', label: 'Ссылка', editable: true, editType: 'text', render: r => r.url ? `<a href="${r.url}" target="_blank" class="text-link">Открыть</a>` : '—' },
      { key: 'contact', label: 'Контакт', editable: true, editType: 'text', render: r => escapeHtml(r.contact || '') || '—' },
      { key: 'applied_at', label: 'Подача', editable: true, editType: 'date', render: r => r.applied_at ? r.applied_at.slice(0, 10) : '—' },
      { key: 'source', label: 'Источник', editable: true, editType: 'text', render: r => r.source ? `<span class="badge badge-blue">${escapeHtml(r.source)}</span>` : '—' },
    ],
    availableViews: ['table', 'kanban', 'list'],
    defaultView: 'table',
    addButton: '+ Вакансия',
    onQuickAdd: async () => {
      await invoke('add_job_vacancy', { company: '', position: '', url: '', stage: 'found', contact: null, source: null });
      loadJobs();
    },
    onCellEdit: async (recordId, key, value, skipReload) => {
      const params = { id: recordId, company: null, position: null, url: null, stage: null, contact: null, appliedAt: null, source: null };
      params[key === 'applied_at' ? 'appliedAt' : key] = value;
      await invoke('update_job_vacancy', params);
      if (!skipReload) loadJobs();
    },
    onDelete: async (id) => { await invoke('delete_job_vacancy', { id }); },
    reloadFn: () => loadJobs(),
    kanban: {
      groupByField: 'stage',
      columns: [
        { key: 'found', label: 'Найдена', icon: '🔍' },
        { key: 'applied', label: 'Отклик', icon: '📤' },
        { key: 'interview', label: 'Интервью', icon: '🎤' },
        { key: 'offer', label: 'Оффер', icon: '🎉' },
        { key: 'rejected', label: 'Отказ', icon: '❌' },
      ],
    },
    onDrop: async (recordId, field, newValue) => {
      await invoke('update_job_vacancy', { id: parseInt(recordId), stage: newValue, company: null, position: null, url: null, contact: null, appliedAt: null, source: null });
      loadJobs();
    },
  });
  dbv.render();
}

// ── Projects tab (custom pages) ──
async function loadProjects() {
  const el = document.getElementById('projects-content');
  if (!el) return;
  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'projects', {
    title: 'Projects',
    subtitle: 'Пользовательские проекты',
    icon: '📁',
    renderTable: async (paneEl) => {
      const pages = await invoke('get_custom_pages').catch(() => []);
      if (!pages.length) { paneEl.innerHTML = '<div class="uni-empty">Нет проектов</div>'; return; }
      paneEl.innerHTML = '<div class="uni-empty">Выберите проект в боковой панели</div>';
    },
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
    { key: 'title', label: 'Title', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
    { key: 'type', label: 'Type', editable: true, editType: 'select', editOptions: [
      { value: 'course', label: 'Курс', color: 'blue' }, { value: 'book', label: 'Книга', color: 'green' },
      { value: 'skill', label: 'Навык', color: 'orange' }, { value: 'article', label: 'Статья', color: 'purple' },
    ], render: r => {
      const devTypeColors = { course: 'blue', book: 'green', skill: 'orange', article: 'purple' };
      return `<span class="badge badge-${devTypeColors[r.type] || 'purple'}">${filterLabels[r.type] || r.type}</span>`;
    }},
    { key: 'status', label: 'Status', editable: true, editType: 'select', editOptions: [
      { value: 'planned', label: 'Запланировано', color: 'gray' }, { value: 'in_progress', label: 'В процессе', color: 'blue' }, { value: 'completed', label: 'Завершено', color: 'green' },
    ], render: r => `<span class="badge ${statusColors[r.status] || 'badge-gray'}">${statusLabels[r.status] || r.status}</span>` },
    { key: 'progress', label: 'Progress', editable: true, editType: 'number', render: r => `<div class="dev-progress" style="width:80px;display:inline-block;"><div class="dev-progress-bar" style="width:${r.progress || 0}%"></div></div> <span style="font-size:11px;color:var(--text-faint);">${r.progress || 0}%</span>` },
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
    onQuickAdd: async () => {
      await invoke('create_learning_item', { itemType: '', title: '', description: '', url: '' });
      loadDevelopment();
    },
    onCellEdit: async (recordId, key, value, skipReload) => {
      const params = { id: recordId, title: null, itemType: null, status: null, progress: null, url: null };
      if (key === 'type') params.itemType = value;
      else if (key === 'progress') params.progress = parseInt(value) || null;
      else params[key] = value;
      await invoke('update_learning_item', params);
      if (!skipReload) loadDevelopment();
    },
    onDelete: async (id) => { await invoke('delete_learning_item', { id }); },
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
      { key: 'title', label: 'Title', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
      { key: 'status', label: 'Status', editable: true, editType: 'select', editOptions: [
        { value: 'planned', label: 'Planned', color: 'gray' }, { value: 'in_progress', label: 'In Progress', color: 'blue' },
        { value: 'completed', label: 'Completed', color: 'green' }, { value: 'on_hold', label: 'On Hold', color: 'yellow' }, { value: 'dropped', label: 'Dropped', color: 'red' },
      ], render: r => `<span class="badge ${r.status === 'completed' ? 'badge-green' : r.status === 'in_progress' ? 'badge-blue' : r.status === 'on_hold' ? 'badge-yellow' : r.status === 'dropped' ? 'badge-red' : 'badge-gray'}">${STATUS_LABELS[r.status] || r.status}</span>` },
      { key: 'rating', label: 'Rating', editable: true, editType: 'number', render: r => {
        const stars = r.rating ? '\u2605'.repeat(Math.round(r.rating / 2)) + '\u2606'.repeat(5 - Math.round(r.rating / 2)) : '\u2014';
        return `<span style="color:var(--text-secondary);font-size:12px;">${stars}</span>`;
      }},
      ...(hasEp ? [{ key: 'progress', label: 'Progress', editable: true, editType: 'number', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.total_episodes ? `${r.progress || 0}/${r.total_episodes}` : ''}</span>` }] : []),
      { key: 'year', label: 'Year', editable: true, editType: 'number', render: r => `<span style="color:var(--text-muted);font-size:12px;">${r.year || '\u2014'}</span>` },
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
      onQuickAdd: async () => {
        await invoke('add_media_item', { mediaType, title: '' });
        loadMediaList(el, mediaType);
      },
      onRowClick: (record) => showMediaDetail(record, mediaType),
      onCellEdit: async (recordId, key, value, skipReload) => {
        const params = { id: recordId, status: null, rating: null, progress: null, notes: null, title: null, description: null, coverUrl: null, totalEpisodes: null };
        if (key === 'rating') params.rating = parseInt(value) || null;
        else if (key === 'progress') params.progress = parseInt(value) || null;
        else if (key === 'year') { /* year not in update_media_item, skip */ }
        else params[key] = value;
        await invoke('update_media_item', params);
        if (!skipReload) loadMediaList(el, mediaType);
      },
      onDelete: async (id) => { await invoke('delete_media_item', { id }); },
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
    renderBody: async (paneEl) => {
      const { loadBodyInline } = await import('./tab-body.js');
      await loadBodyInline(paneEl);
    },
    renderTable: async (paneEl) => {
      const workouts = await invoke('get_workouts', { dateRange: null }).catch(() => []);
      const stats = await invoke('get_workout_stats').catch(() => ({ count: 0, total_minutes: 0, total_calories: 0 }));
      renderSports(paneEl, workouts || [], stats);
    },
  });
}

function renderSports(el, workouts, stats) {
  const typeLabels = { gym: 'Зал', cardio: 'Кардио', yoga: 'Йога', swimming: 'Плавание', martial_arts: 'Единоборства', other: 'Другое' };

  const dbv = new DatabaseView(el, {
    tabId: 'sports',
    recordTable: 'workouts',
    records: workouts,
    fixedColumns: [
      { key: 'date', label: 'Дата', render: r => `<span style="font-size:12px;color:var(--text-muted);font-variant-numeric:tabular-nums;">${r.date || '—'}</span>` },
      { key: 'title', label: 'Название', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.title || typeLabels[r.type] || r.type)}</span>` },
      { key: 'type', label: 'Тип', editable: true, editType: 'select', editOptions: [
        { value: 'gym', label: 'Зал', color: 'blue' }, { value: 'cardio', label: 'Кардио', color: 'red' }, { value: 'yoga', label: 'Йога', color: 'green' },
        { value: 'swimming', label: 'Плавание', color: 'purple' }, { value: 'martial_arts', label: 'Единоборства', color: 'orange' }, { value: 'other', label: 'Другое', color: 'gray' },
      ], render: r => {
        const sportColors = { gym: 'blue', cardio: 'red', yoga: 'green', swimming: 'purple', martial_arts: 'orange', other: 'gray' };
        return `<span class="badge badge-${sportColors[r.type] || 'purple'}">${typeLabels[r.type] || r.type}</span>`;
      }},
      { key: 'duration_minutes', label: 'Время', editable: true, editType: 'number', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.duration_minutes || 0} мин</span>` },
      { key: 'calories', label: 'Калории', editable: true, editType: 'number', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.calories || '—'}</span>` },
    ],
    idField: 'id',
    availableViews: ['table', 'list'],
    defaultView: 'table',
    addButton: '+ Тренировка',
    onAdd: () => showAddWorkoutModal(),
    onQuickAdd: async () => {
      await invoke('create_workout', { workoutType: '', title: '', durationMinutes: 0, notes: '' });
      loadSports();
    },
    onCellEdit: async (recordId, key, value, skipReload) => {
      const params = { id: recordId, title: null, workoutType: null, durationMinutes: null, calories: null };
      if (key === 'type') params.workoutType = value;
      else if (key === 'duration_minutes') params.durationMinutes = parseInt(value) || null;
      else if (key === 'calories') params.calories = parseInt(value) || null;
      else params[key] = value;
      await invoke('update_workout', params);
      if (!skipReload) loadSports();
    },
    onDelete: async (id) => { await invoke('delete_workout', { id }); },
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
    renderBody: async (paneEl) => {
      const { loadBodyInline } = await import('./tab-body.js');
      await loadBodyInline(paneEl);
    },
    renderTable: async (paneEl) => {
      try {
        const habits = await invoke('get_habits_today').catch(() => []);
        renderHealth(paneEl, null, habits);
      } catch (e) {
        paneEl.innerHTML = '<div class="uni-empty">Не удалось загрузить данные здоровья</div>';
      }
    },
  });
}

function renderHealth(el, today, habits) {
  el.innerHTML = `<div id="habits-dbv"></div>`;

  // Habits as DatabaseView
  const dbvEl = el.querySelector('#habits-dbv');
  const dbv = new DatabaseView(dbvEl, {
    tabId: 'health',
    recordTable: 'habits',
    records: habits,
    fixedColumns: [
      { key: 'done', label: '', render: r => `<div class="habit-check${r.completed ? ' checked' : ''}" style="cursor:pointer;" data-hid="${r.id}">${r.completed ? '&#10003;' : ''}</div>` },
      { key: 'name', label: 'Привычка', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
      { key: 'frequency', label: 'Частота', editable: true, editType: 'select', editOptions: [
        { value: 'daily', label: 'Ежедневно', color: 'blue' }, { value: 'weekly', label: 'Еженедельно', color: 'green' },
        { value: 'weekdays', label: 'Будни', color: 'yellow' }, { value: 'weekends', label: 'Выходные', color: 'purple' },
      ], render: r => {
        const freqColors = { daily: 'blue', weekly: 'green', weekdays: 'yellow', weekends: 'purple' };
        return `<span class="badge badge-${freqColors[r.frequency] || 'gray'}">${r.frequency || 'daily'}</span>`;
      }},
      { key: 'streak', label: 'Серия', render: r => r.streak > 0 ? `<span class="badge badge-green">${r.streak} дн.</span>` : '<span style="color:var(--text-faint);font-size:12px;">—</span>' },
    ],
    idField: 'id',
    addButton: '+ Привычка',
    onAdd: () => {
      const name = prompt('Название привычки:');
      if (name) invoke('create_habit', { name, icon: '', frequency: 'daily' }).then(() => loadHealth()).catch(e => alert(e));
    },
    onQuickAdd: async () => {
      await invoke('create_habit', { name: '', icon: '', frequency: '' });
      loadHealth();
    },
    onCellEdit: async (recordId, key, value, skipReload) => {
      const params = { id: recordId, name: null, frequency: null, icon: null };
      params[key] = value;
      await invoke('update_habit', params);
      if (!skipReload) loadHealth();
    },
    onDelete: async (id) => { await invoke('delete_habit', { id }); },
    reloadFn: () => loadHealth(),
  });
  dbv.render();

  // Delegate habit check clicks
  el.addEventListener('click', async (e) => {
    const check = e.target.closest('[data-hid]');
    if (!check) return;
    await invoke('check_habit', { habitId: parseInt(check.dataset.hid), date: null }).catch(() => {});
    loadHealth();
  });
}

// ── Custom Pages ──
async function loadCustomPage(tabId, subTab) {
  const reg = TAB_REGISTRY[tabId];
  if (!reg?.custom || !reg.pageId) return;
  const el = document.getElementById(`${tabId}-content`);
  if (!el) return;

  // Projects use unified layout with DatabaseView
  if (reg.pageType === 'project') {
    return loadCustomProject(tabId, subTab, el, reg);
  }

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
    `;

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
    document.getElementById('cp-icon-btn')?.addEventListener('click', (e) => {
      e.stopPropagation();
      showEmojiPicker(e.target, (emoji) => {
        document.getElementById('cp-icon-btn').textContent = emoji;
        autoSaveMeta('icon', emoji);
      });
    });

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

// ── Custom Project (unified layout) ──
async function loadCustomProject(tabId, subTab, el, reg) {
  const projectId = String(reg.pageId);
  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, tabId, {
    title: reg.label || 'Новый проект',
    subtitle: '',
    icon: reg.icon || '📁',
    renderTable: async (paneEl) => {
      const records = await invoke('get_project_records', { projectId }).catch(() => []);
      const dbv = new DatabaseView(paneEl, {
        tabId, recordTable: 'project_records', records,
        fixedColumns: [
          { key: 'name', label: 'Название', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.name || '')}</span>` },
        ],
        addButton: '+ Добавить',
        onAdd: async () => {
          await invoke('create_project_record', { projectId, name: '' });
          loadCustomPage(tabId, subTab);
        },
        onQuickAdd: async () => {
          await invoke('create_project_record', { projectId, name: '' });
          loadCustomPage(tabId, subTab);
        },
        onCellEdit: async (recordId, key, value, skipReload) => {
          await invoke('update_project_record', { id: recordId, name: value });
          if (!skipReload) loadCustomPage(tabId, subTab);
        },
        onDelete: async (id) => { await invoke('delete_project_record', { id }); },
        reloadFn: () => loadCustomPage(tabId, subTab),
      });
      S.dbViews[`project_${projectId}`] = dbv;
      await dbv.render();
    },
  });
}

// ── Schedule tab ──

const SCHEDULE_CATEGORIES = { health: 'Здоровье', sport: 'Спорт', hygiene: 'Гигиена', home: 'Дом', practice: 'Практика', challenge: 'Челлендж', growth: 'Развитие', work: 'Работа', other: 'Другое' };
const SCH_CAT_COLORS = { health: 'blue', sport: 'green', hygiene: 'pink', practice: 'purple', challenge: 'red', growth: 'yellow', work: 'orange', home: 'orange', other: 'gray' };
const SCHEDULE_FREQ = { daily: 'Ежедневно', weekly: 'Еженедельно', custom: 'По дням' };
const DAYS_SHORT = ['Пн','Вт','Ср','Чт','Пт','Сб','Вс'];

function buildRecurrenceJson(r) {
  if (!r.frequency) return '';
  const obj = { every: 1, unit: 'day' };
  if (r.frequency === 'daily') { obj.unit = 'day'; }
  else if (r.frequency === 'weekly') { obj.unit = 'week'; }
  else if (r.frequency === 'custom' && r.frequency_days) { obj.unit = 'week'; obj.days = r.frequency_days.split(',').map(Number); }
  if (r.time_of_day) obj.time = r.time_of_day;
  return JSON.stringify(obj);
}

async function loadSchedule(subTab) {
  const el = document.getElementById('schedule-content');
  if (!el) return;
  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'schedule', {
    title: 'Schedule',
    subtitle: 'Расписание и повторяющиеся события',
    icon: '📅',
    renderTracking: async (paneEl) => {
      if (!S._schedTrackMode) S._schedTrackMode = 'week';
      const mode = S._schedTrackMode;
      const numDays = mode === 'month' ? 30 : 7;

      const schedules = await invoke('get_schedules', { category: null }).catch(() => []);
      const active = schedules.filter(s => s.is_active);
      const days = [];
      for (let i = numDays - 1; i >= 0; i--) {
        const d = new Date(); d.setDate(d.getDate() - i);
        days.push(d.toISOString().slice(0, 10));
      }
      const compMap = {};
      for (const date of days) {
        const comps = await invoke('get_schedule_completions', { date }).catch(() => []);
        for (const c of comps) { if (c.completed) { if (!compMap[c.schedule_id]) compMap[c.schedule_id] = {}; compMap[c.schedule_id][date] = c.completed_at || true; } }
      }
      const dayFmt = mode === 'month' ? { day: 'numeric' } : { weekday: 'short', day: 'numeric' };
      const dayLabels = days.map(d => { const dt = new Date(d + 'T12:00:00'); return dt.toLocaleDateString('ru', dayFmt); });
      const thWidth = 'min-width:50px;';
      const headerCols = dayLabels.map(l => `<th style="text-align:center;font-size:${mode === 'month' ? '10' : '12'}px;${thWidth}">${l}</th>`).join('');
      const rows = active.map(s => {
        const cells = days.map(d => {
          const val = compMap[s.id]?.[d];
          if (!val) return '<td style="text-align:center;"><span style="color:var(--text-faint);">·</span></td>';
          let timeStr = '';
          if (typeof val === 'string') { try { const dt = new Date(val); timeStr = `${String(dt.getHours()).padStart(2,'0')}:${String(dt.getMinutes()).padStart(2,'0')}`; } catch {} }
          return `<td style="text-align:center;"><span style="color:var(--color-green);font-size:11px;font-weight:500;">${timeStr || '✓'}</span></td>`;
        }).join('');
        const streak = days.reduce((acc, d) => compMap[s.id]?.[d] ? acc + 1 : 0, 0);
        return `<tr class="data-table-row" data-id="${s.id}"><td class="col-check"><input type="checkbox"></td><td style="font-size:13px;white-space:nowrap;">${escapeHtml(s.title)}</td>${cells}<td style="text-align:center;font-size:12px;color:var(--text-muted);">${streak > 0 ? streak + '🔥' : ''}</td></tr>`;
      }).join('');

      // Toggle buttons (neutral style matching toolbar)
      const mkBtn = (m, label) => {
        const isActive = mode === m;
        const style = isActive
          ? 'background:var(--bg-hover);color:var(--text-primary);border-color:var(--border-subtle);'
          : 'color:var(--text-muted);';
        return `<button class="dbv-quick-filter-btn" style="${style}" data-track-mode="${m}">${label}</button>`;
      };
      const toolbar = `<div style="display:flex;align-items:center;justify-content:flex-end;margin-bottom:8px;">
        <div class="dbv-quick-filter-group">${mkBtn('week', 'Н')}${mkBtn('month', 'М')}</div>
      </div>`;

      paneEl.innerHTML = active.length === 0
        ? '<div style="text-align:center;color:var(--text-faint);padding:40px;">Нет активных расписаний</div>'
        : `${toolbar}<div class="database-view" style="overflow-x:auto;"><table class="data-table" style="font-size:13px;">
            <thead><tr><th class="col-check-header"><input type="checkbox"></th><th style="min-width:150px;">Практика</th>${headerCols}<th style="text-align:center;font-size:12px;">Streak</th></tr></thead>
            <tbody>${rows}</tbody>
          </table></div>`;
      paneEl.querySelectorAll('[data-track-mode]').forEach(btn => {
        btn.addEventListener('click', () => {
          S._schedTrackMode = btn.dataset.trackMode;
          loadSchedule('tracking');
        });
      });
      const { bindCheckboxes, renderBulkBar } = await import('./db-view/db-select.js');
      const trackCtx = {
        tabId: 'schedule_tracking', idField: 'id', records: active,
        onDelete: async (id) => { await invoke('delete_schedule', { id }); },
        reloadFn: () => loadSchedule('tracking'),
      };
      bindCheckboxes(paneEl, 'schedule_tracking', active, 'id', trackCtx);
      renderBulkBar(paneEl, 'schedule_tracking', trackCtx);
    },
    renderTable: async (paneEl) => {
      const schedules = await invoke('get_schedules', { category: null }).catch(() => []);
      const today = new Date().toISOString().slice(0, 10);
      const completions = await invoke('get_schedule_completions', { date: today }).catch(() => []);
      const completedIds = new Set(completions.filter(c => c.completed).map(c => c.schedule_id));

      const reloadTable = async () => {
        const fresh = await invoke('get_schedules', { category: null }).catch(() => []);
        const comp = await invoke('get_schedule_completions', { date: today }).catch(() => []);
        const cIds = new Set(comp.filter(c => c.completed).map(c => c.schedule_id));
        dbv.schema.records = fresh;
        dbv.schema.fixedColumns[0].render = r => `<div class="habit-check${cIds.has(r.id) ? ' checked' : ''}" data-schid="${r.id}" style="cursor:pointer;">${cIds.has(r.id) ? '&#10003;' : ''}</div>`;
        await dbv.render();
      };
      const dbv = new DatabaseView(paneEl, {
        tabId: 'schedule', recordTable: 'schedules', records: schedules,
        availableViews: ['table', 'list'],
        fixedColumns: [
          { key: 'done', label: '✓', render: r => `<div class="habit-check${completedIds.has(r.id) ? ' checked' : ''}" data-schid="${r.id}" style="cursor:pointer;">${completedIds.has(r.id) ? '&#10003;' : ''}</div>` },
          { key: 'title', label: 'Название', editable: true, editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
          { key: 'category', label: 'Категория', editable: true, editType: 'select', editOptions: Object.entries(SCHEDULE_CATEGORIES).map(([k, v]) => ({ value: k, label: v, color: SCH_CAT_COLORS[k] || 'gray' })), render: r => {
            if (!r.category) return '';
            let cats = [r.category];
            try { const p = JSON.parse(r.category); if (Array.isArray(p)) cats = p; } catch {}
            return cats.map(c => `<span class="badge badge-${SCH_CAT_COLORS[c] || 'gray'} cell-badge-removable">${SCHEDULE_CATEGORIES[c] || c}<span class="cell-badge-x" data-remove-cat="${c}">✕</span></span>`).join(' ');
          }},
          { key: 'recurrence', label: 'Повторение', editable: true, editType: 'recurrence', render: r => {
            const json = r.recurrence || buildRecurrenceJson(r);
            if (!json) return '<span class="text-faint">—</span>';
            return `<span class="cell-recurrence">${escapeHtml(formatRecurrence(json))}</span>`;
          }},
          { key: 'details', label: 'Детали', editable: true, editType: 'text', render: r => {
            if (!r.details) return '<span class="text-faint">—</span>';
            return `<span style="font-size:12px;color:var(--text-secondary);">${escapeHtml(r.details)}</span>`;
          }},
        ],
        idField: 'id',
        addButton: '+ Расписание',
        onAdd: () => showScheduleModal(),
        onQuickAdd: async () => {
          await invoke('create_schedule', { title: '', category: '', frequency: '', frequencyDays: null, timeOfDay: null, details: null });
          reloadTable();
        },
        onCellEdit: async (recordId, key, value, skipReload) => {
          if (key === 'recurrence') {
            try {
              const r = JSON.parse(value);
              const params = { id: recordId };
              if (r.unit === 'day' && (r.every || 1) === 1) { params.frequency = 'daily'; }
              else if (r.unit === 'day') { params.frequency = `every_${r.every}d`; }
              else if (r.unit === 'week' && r.days?.length > 0) { params.frequency = 'custom'; params.frequencyDays = r.days.join(','); }
              else if (r.unit === 'week') { params.frequency = 'weekly'; }
              else if (r.unit === 'hour') { params.frequency = `every_${r.every}h`; }
              else if (r.unit === 'month' && (r.every || 1) === 1) { params.frequency = 'monthly'; }
              else if (r.unit === 'month') { params.frequency = `every_${r.every}m`; }
              if (r.time) params.timeOfDay = r.time;
              await invoke('update_schedule', params);
            } catch {}
          } else {
            const keyMap = { title: 'title', category: 'category', details: 'details' };
            const params = { id: recordId };
            const paramKey = keyMap[key];
            if (paramKey) params[paramKey] = value;
            await invoke('update_schedule', params);
          }
          if (!skipReload) reloadTable();
        },
        reloadFn: reloadTable,
        onDelete: async (id) => { await invoke('delete_schedule', { id }); },
      });
      await dbv.render();
      // Templates button next to add-row
      const addRow = paneEl.querySelector('.dbv-add-row');
      if (addRow) {
        const tplCell = addRow.querySelector('td');
        if (tplCell) {
          const tplBtn = document.createElement('span');
          tplBtn.style.cssText = 'margin-left:12px;cursor:pointer;color:var(--text-muted);font-size:13px;';
          tplBtn.textContent = '📋 Шаблоны';
          tplBtn.addEventListener('click', async (e) => {
            e.stopPropagation();
            const { showScheduleTemplatesModal } = await import('./schedule-templates.js');
            await showScheduleTemplatesModal(reloadTable);
          });
          tplCell.querySelector('.dbv-add-row-label').appendChild(tplBtn);
        }
      }
      paneEl.addEventListener('click', async (e) => {
        const chk = e.target.closest('[data-schid]');
        if (chk) {
          await invoke('toggle_schedule_completion', { scheduleId: parseInt(chk.dataset.schid), date: today });
          reloadTable();
          return;
        }
        const tog = e.target.closest('[data-toggle-id]');
        if (tog) {
          const id = parseInt(tog.dataset.toggleId);
          const isOn = tog.classList.contains('on');
          await invoke('update_schedule', { id, isActive: !isOn });
          reloadTable();
        }
      });
    },
  });
}

function showScheduleModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Новое расписание</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="sch-title"></div>
    <div class="form-group"><label class="form-label">Категория</label>
      <select class="form-select" id="sch-cat" style="width:100%;">
        ${Object.entries(SCHEDULE_CATEGORIES).map(([k,v]) => `<option value="${k}">${v}</option>`).join('')}
      </select></div>
    <div class="form-group"><label class="form-label">Частота</label>
      <select class="form-select" id="sch-freq" style="width:100%;">
        <option value="daily">Ежедневно</option><option value="weekly">Еженедельно</option><option value="custom">По дням</option>
      </select></div>
    <div class="form-group" id="sch-days-group" style="display:none;"><label class="form-label">Дни (1=Пн ... 7=Вс)</label>
      <div style="display:flex;gap:4px;">${DAYS_SHORT.map((d,i) => `<label style="display:flex;align-items:center;gap:2px;font-size:12px;"><input type="checkbox" value="${i+1}" class="sch-day-cb">${d}</label>`).join('')}</div>
    </div>
    <div class="form-group"><label class="form-label">Время</label><input class="form-input" id="sch-time" type="time"></div>
    <div class="form-group"><label class="form-label">Детали</label><input class="form-input" id="sch-details" placeholder="Дозировка, длительность..."></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="sch-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('sch-freq')?.addEventListener('change', (e) => {
    document.getElementById('sch-days-group').style.display = e.target.value === 'custom' ? '' : 'none';
  });
  document.getElementById('sch-save')?.addEventListener('click', async () => {
    const title = document.getElementById('sch-title')?.value?.trim();
    if (!title) return;
    const freqDays = [...overlay.querySelectorAll('.sch-day-cb:checked')].map(cb => cb.value).join(',') || null;
    try {
      await invoke('create_schedule', {
        title,
        category: document.getElementById('sch-cat')?.value || 'other',
        frequency: document.getElementById('sch-freq')?.value || 'daily',
        frequencyDays: freqDays,
        timeOfDay: document.getElementById('sch-time')?.value || null,
        details: document.getElementById('sch-details')?.value || null,
      });
      overlay.remove();
      loadSchedule();
    } catch (err) { alert('Ошибка: ' + err); }
  });
}

// ── Dan Koe Protocol tab ──

const DK_PRACTICES = [
  {
    key: 'contemplation', label: 'Contemplation', icon: '🧘',
    what: 'Тихое созерцание — 10-15 минут без отвлечений',
    why: 'Тренирует осознанность, снижает реактивность ума, помогает замечать автоматические мысли вместо того, чтобы следовать за ними.',
    how: [
      'Сядь в тихое место, закрой глаза',
      'Не пытайся "очистить разум" — просто наблюдай мысли как облака',
      'Когда замечаешь, что увлёкся мыслью — мягко возвращайся к наблюдению',
      'Начни с 5 минут, постепенно увеличивай до 15-20',
    ],
  },
  {
    key: 'pattern_interrupt', label: 'Pattern Interrupt', icon: '⚡',
    what: 'Прерывание автоматических паттернов поведения',
    why: 'Большинство действий выполняются на автопилоте. Прерывая паттерны, ты возвращаешь контроль над вниманием и решениями.',
    how: [
      'Замечай моменты "на автопилоте": рука тянется к телефону, открываешь соцсети, ешь от скуки',
      'Спроси себя: "Зачем я это делаю прямо сейчас?"',
      'Сделай паузу на 10 секунд перед импульсивным действием',
      'Замени автоматическое действие осознанным выбором',
    ],
    examples: 'Потянулся к Instagram → паузу → "мне скучно, я могу почитать/погулять". Хочу сладкое → "голоден или стресс?"',
  },
  {
    key: 'vision', label: 'Vision', icon: '🔭',
    what: 'Вопросы о видении будущего — 5-10 минут рефлексии',
    why: 'Без ясного видения будущего ты реагируешь на обстоятельства вместо того, чтобы создавать жизнь по своему дизайну.',
    how: [
      'Задай себе один из вопросов ниже и запиши ответ',
      'Не фильтруй — пиши первое, что приходит',
      'Перечитывай свои ответы раз в неделю',
    ],
    questions: [
      'Какой будет моя идеальная жизнь через 3 года?',
      'Что бы я делал, если бы деньги не были проблемой?',
      'Какой навык изменит мою жизнь больше всего?',
      'От чего мне нужно отказаться, чтобы расти?',
      'Что я откладываю и почему?',
      'Каким человеком я хочу стать?',
    ],
  },
  {
    key: 'integration', label: 'Integration', icon: '🔗',
    what: 'Одно конкретное действие, основанное на практиках выше',
    why: 'Инсайты без действий — просто развлечение. Integration превращает осознанность в реальные изменения.',
    how: [
      'Выбери одну вещь из сегодняшних практик, которую можешь применить прямо сейчас',
      'Сделай это маленьким и конкретным: "напишу 200 слов", а не "начну писать книгу"',
      'Запиши, что сделал — это усиливает привычку',
    ],
    examples: 'Vision → "хочу быть здоровее" → Integration → "сегодня пойду на 30-мин прогулку вместо скролла"',
  },
];

async function loadDanKoe(subTab) {
  const el = document.getElementById('dankoe-content');
  if (!el) return;
  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  const today = new Date().toISOString().slice(0, 10);
  const schedules = await invoke('get_schedules', { category: 'practice' }).catch(() => []);
  const completions = await invoke('get_schedule_completions', { date: today }).catch(() => []);
  const completedIds = new Set(completions.filter(c => c.completed).map(c => c.schedule_id));
  const schByLabel = {};
  for (const s of schedules) schByLabel[s.title.toLowerCase()] = s;

  await renderUnifiedLayout(el, 'dankoe', {
    title: 'Dan Koe Protocol',
    subtitle: 'Ежедневная практика осознанности',
    icon: '🧠',
    renderTable: async (paneEl) => {
      paneEl.innerHTML = DK_PRACTICES.map(p => {
        const sch = schByLabel[p.label.toLowerCase()];
        const done = sch && completedIds.has(sch.id);
        const statusHtml = sch
          ? `<div style="margin-top:12px;padding:6px 10px;border-radius:var(--radius-1);background:${done ? 'var(--color-green-bg)' : 'var(--bg-hover)'};font-size:12px;color:${done ? 'var(--color-green)' : 'var(--text-muted)'};">${done ? '✓ Выполнено сегодня' : '○ Не выполнено сегодня'}</div>`
          : '<div style="margin-top:12px;font-size:12px;color:var(--text-faint);">⚠ Нет в Schedule — добавьте через Шаблоны</div>';
        return `
        <div style="margin-bottom:24px;padding:16px;border-radius:var(--radius-2);border:1px solid var(--border-subtle);">
          <div style="display:flex;align-items:center;gap:8px;margin-bottom:10px;">
            <span style="font-size:20px;">${p.icon}</span>
            <span style="font-size:16px;font-weight:600;color:var(--text-primary);">${p.label}</span>
          </div>
          <div style="font-size:13px;color:var(--text-secondary);margin-bottom:8px;">${p.what}</div>
          <div style="font-size:12px;color:var(--text-muted);margin-bottom:12px;padding:8px 12px;background:var(--bg-hover);border-radius:var(--radius-1);">
            <b>Зачем:</b> ${p.why}
          </div>
          <div style="font-size:13px;color:var(--text-primary);font-weight:500;margin-bottom:6px;">Как делать:</div>
          <ul style="margin:0 0 8px 16px;padding:0;font-size:13px;color:var(--text-secondary);line-height:1.7;">
            ${p.how.map(h => `<li>${h}</li>`).join('')}
          </ul>
          ${p.questions ? `
            <div style="font-size:13px;color:var(--text-primary);font-weight:500;margin-bottom:6px;">Вопросы для рефлексии:</div>
            <ul style="margin:0 0 8px 16px;padding:0;font-size:13px;color:var(--text-muted);line-height:1.7;font-style:italic;">
              ${p.questions.map(q => `<li>${q}</li>`).join('')}
            </ul>` : ''}
          ${p.examples ? `<div style="font-size:12px;color:var(--text-faint);margin-top:8px;">💡 <i>${p.examples}</i></div>` : ''}
          ${statusHtml}
        </div>`;
      }).join('');
    },
  });
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
  loadJobs,
  loadProjects,
  loadDevelopment,
  loadHobbies,
  loadSports,
  loadHealth,
  loadCustomPage,
  loadSchedule,
  loadDanKoe,
  panelItem,
};

// Register in tabLoaders for tabs.js switchTab()
Object.assign(tabLoaders, {
  loadHome, loadMindset, loadFood, loadMoney, loadPeople,
  loadJobs, loadProjects, loadDevelopment,
  loadHobbies, loadSports, loadHealth,
  loadCustomPage, loadSchedule, loadDanKoe,
  loadMemoryTab, loadAbout,
});
