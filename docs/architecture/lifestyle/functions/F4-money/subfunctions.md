# F4 — Money (all sub-tabs): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F4: Money (all sub-tabs) |
| Файл(ы) | main.js:L2120-2337 |
| LOC | 218 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F4.1 | Роутинг Money по sub-tab | L2120-L2131 | 12 | `loadMoney(subTab)` → header, делегирование loadTransactions / loadBudgets / loadSavings / loadSubscriptions / loadDebts | control | Trivial |
| F4.2 | Рендер транзакций + модалка добавления | L2133-L2194 | 62 | `loadTransactions(el, txType)` → invoke get_transactions + get_transaction_stats → список + статистика + модалка добавления + delete | render + interaction | High |
| F4.3 | Рендер бюджетов + добавление | L2196-L2220 | 25 | `loadBudgets(el)` → invoke get_budgets → прогресс-бары (spent/amount) + предупреждение при >80% + prompt-добавление | render + interaction | Low |
| F4.4 | Рендер целей накоплений + управление | L2222-L2259 | 38 | `loadSavings(el)` → invoke get_savings_goals → прогресс-бары + "+ Add funds" → invoke update_savings_goal + prompt-добавление | render + interaction | Medium |
| F4.5 | Рендер подписок + модалка добавления | L2261-L2310 | 50 | `loadSubscriptions(el)` → invoke get_subscriptions → monthly total + список + модалка (name, amount, period, category) | render + interaction | Medium |
| F4.6 | Рендер долгов + добавление | L2312-L2337 | 26 | `loadDebts(el)` → invoke get_debts → прогресс-бары (paid/total) + prompt-добавление (name, type, amount) | render + interaction | Low |

## Data Flow

```
[loadMoney(subTab)]
      │
      ├── 'Expenses' → loadTransactions(pc, 'expense')
      ├── 'Income'   → loadTransactions(pc, 'income')
      ├── 'Budget'   → loadBudgets(pc)
      ├── 'Savings'  → loadSavings(pc)
      ├── 'Subscriptions' → loadSubscriptions(pc)
      ├── 'Debts'    → loadDebts(pc)
      └── default    → loadTransactions(pc, 'expense')

[loadTransactions(el, txType)]
      │
      ▼
  invoke('get_transactions')      ──→ [Backend: B7]
  invoke('get_transaction_stats') ──→ [Backend: B7]
      │
      ▼
  Статистика: Total (30d)
  + Список транзакций: date, description, category, amount
      │
      ├── Delete click → invoke('delete_transaction') → loadTransactions()
      └── "+ Add" click → модалка (amount, category, desc, currency)
              │
              ▼
          invoke('add_transaction') → loadTransactions()

[loadBudgets]
      │
      ▼
  invoke('get_budgets') ──→ [Backend: B8]
      │
      ▼
  Список: category + spent/amount + progress bar (warn >80%)
      │
      └── "+ Add Budget" → prompt(category, amount) → invoke('create_budget')

[loadSavings]
      │
      ▼
  invoke('get_savings_goals') ──→ [Backend: B9]
      │
      ▼
  Список: name + current/target + progress bar + deadline
      │
      ├── "+ Add funds" → prompt → invoke('update_savings_goal')
      └── "+ Add Goal" → prompt(name, target) → invoke('create_savings_goal')

[loadSubscriptions]
      │
      ▼
  invoke('get_subscriptions') ──→ [Backend: B10]
      │
      ▼
  Monthly total (calculated) + список: name, active/paused, amount/period, next_payment
      │
      └── "+ Add" → модалка (name, amount, period, category) → invoke('add_subscription')

[loadDebts]
      │
      ▼
  invoke('get_debts') ──→ [Backend: B11]
      │
      ▼
  Список: name, type (owe/owed), remaining/amount + progress bar
      │
      └── "+ Add" → prompt(name, type, amount) → invoke('add_debt')
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F4.2 | Транзакции | 62 LOC — модалка встроена прямо в функцию, а не вынесена отдельно (в отличие от F3) | Вынести `showAddTransactionModal()` для единообразия | Low |
| F4.3 / F4.6 | Бюджеты / Долги | Используют `prompt()` вместо модалок — менее удобный UX | Заменить на модалки (как в подписках) | Low |
| F4.4 | Savings | `update_savings_goal` вызывается с `name: null, targetAmount: null` — рискует затереть данные, если бекенд не обрабатывает null | Проверить, что бекенд правильно обрабатывает partial update | Medium |
