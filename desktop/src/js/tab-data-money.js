// ── tab-data-money.js — Money tab (transactions, budgets, savings, subscriptions, debts) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── Money ──
export async function loadMoney(subTab) {
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

    const typeFilterBar = `<div class="dev-filters" style="margin-bottom:var(--space-2);">
      ${['all','expense','income'].map(f =>
        `<button class="pill${S.moneyTxFilter === f ? ' active' : ''}" data-txfilter="${f}">${f === 'all' ? 'All' : f === 'expense' ? 'Expenses' : 'Income'}</button>`
      ).join('')}
    </div>`;

    const statsHtml = `<div class="uni-dash-grid" style="margin-bottom:var(--space-3);">
      <div class="uni-dash-card purple"><div class="uni-dash-value">${stats.total_expenses || 0}</div><div class="uni-dash-label">Expenses (30d)</div></div>
      <div class="uni-dash-card blue"><div class="uni-dash-value">${stats.total_income || 0}</div><div class="uni-dash-label">Income (30d)</div></div>
      <div class="uni-dash-card ${((stats.total_income || 0) - (stats.total_expenses || 0)) >= 0 ? 'green' : 'red'}"><div class="uni-dash-value">${(stats.total_income || 0) - (stats.total_expenses || 0)}</div><div class="uni-dash-label">Balance</div></div>
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
      onQuickAdd: async (description) => {
        await invoke('add_transaction', { date: null, txType: 'expense', amount: 0, currency: 'KZT', category: 'other', description, recurring: false, recurringPeriod: null });
        loadTransactions(el);
      },
      reloadFn: () => loadTransactions(el),
      onDelete: async (id) => { await invoke('delete_transaction', { id }); },
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
      availableViews: ['table', 'list'],
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
      onQuickAdd: async (category) => {
        await invoke('create_budget', { category, amount: 0, period: 'monthly' });
        loadBudgets(el);
      },
      reloadFn: () => loadBudgets(el),
      onDelete: async (id) => { await invoke('delete_budget', { id }); },
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
      availableViews: ['table', 'list'],
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
      onQuickAdd: async (name) => {
        await invoke('create_savings_goal', { name, targetAmount: 0, currentAmount: 0, deadline: null, color: '#9B9B9B' });
        loadSavings(el);
      },
      reloadFn: () => loadSavings(el),
      onDelete: async (id) => { await invoke('delete_savings_goal', { id }); },
    });
    await dbv.render();

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
      availableViews: ['table', 'list'],
      fixedColumns: [
        { key: 'name', label: 'Название', render: r => `<span class="data-table-title">${escapeHtml(r.name)}</span>` },
        { key: 'amount', label: 'Сумма', render: r => `<span style="font-variant-numeric:tabular-nums;font-size:12px;">${r.amount} ${r.currency || 'KZT'}</span>` },
        { key: 'period', label: 'Период', render: r => `<span class="badge badge-gray">${r.period === 'yearly' ? 'Годовая' : 'Месячная'}</span>` },
        { key: 'active', label: 'Статус', render: r => r.active ? '<span class="badge badge-green">Активна</span>' : '<span class="badge badge-gray">Пауза</span>' },
        { key: 'next_payment', label: 'Следующий платеж', render: r => `<span style="font-size:12px;color:var(--text-muted);">${r.next_payment || '—'}</span>` },
      ],
      idField: 'id',
      addButton: '+ Добавить',
      onQuickAdd: async (name) => {
        await invoke('add_subscription', { name, amount: 0, currency: 'KZT', period: 'monthly', nextPayment: null, category: 'other', active: true });
        loadSubscriptions(el);
      },
      reloadFn: () => loadSubscriptions(el),
      onDelete: async (id) => { await invoke('delete_subscription', { id }); },
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
      availableViews: ['table', 'list'],
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
      onQuickAdd: async (name) => {
        await invoke('add_debt', { name, debtType: 'owe', amount: 0, remaining: 0, interestRate: null, dueDate: null, description: '' });
        loadDebts(el);
      },
      reloadFn: () => loadDebts(el),
      onDelete: async (id) => { await invoke('delete_debt', { id }); },
    });
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}
