# Money

## Purpose
Personal finance management — expense/income tracking, budgets, savings goals, subscriptions, and debt tracking.

## DB Tables

| Table | Purpose |
|-------|---------|
| `transactions` | Income/expense entries (amount, category, type, date, description) |
| `budgets` | Monthly budget allocations per category |
| `savings_goals` | Savings targets with progress tracking |
| `subscriptions` | Recurring subscriptions (name, amount, frequency, next_date) |
| `debts` | Debt records (creditor, amount, interest_rate, due_date) |

## Views
- **Transactions** — expense/income list with filters and stats
- **Budgets** — monthly budget allocation and spending progress
- **Savings** — savings goals with progress bars
- **Subscriptions** — recurring payments overview
- **Debts** — debt tracking with payoff progress

## Key Commands (Tauri)
- `get_transactions` / `add_transaction` / `update_transaction` / `delete_transaction` — transaction CRUD
- `get_transaction_stats` — spending analytics
- `get_budgets` / `create_budget` / `update_budget` / `delete_budget` — budget CRUD
- `get_savings_goals` / `create_savings_goal` / `update_savings_goal` / `delete_savings_goal` — savings CRUD
- `get_subscriptions` / `add_subscription` / `update_subscription` / `delete_subscription` — subscription CRUD
- `get_debts` / `add_debt` / `update_debt` / `delete_debt` — debt CRUD

## Relations
- Standalone — most complex tab by number of sub-views and tables

## Notable
- 5 DB tables — the most data-rich tab in Hanni
- Transaction categories for spending breakdown
- Budget vs actual spending comparison
- Subscription tracking with renewal date alerts
- Debt interest rate and payoff calculation
