# Module 12: Lifestyle — Function Index

## Backend (lib.rs)

### Health/Fitness

| ID  | Function Group        | Type     | Key Functions                                           | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------|----------|---------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B1  | CRUD workouts         | endpoint | `create_workout`, `get_workouts`, `get_workout_stats`   | lib.rs:L4520-4564 | 45  | Simple     | —        | —       |
| B2  | Health logging        | endpoint | `log_health`, `get_health_today`                        | lib.rs:L4566-4611 | 46  | Simple     | —        | —       |
| B3  | CRUD habits           | endpoint | `create_habit`, `check_habit`, `get_habits_today`       | lib.rs:L4613-4676 | 64  | Medium     | 3        | ✅      |

### Food

| ID  | Function Group              | Type     | Key Functions                                           | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|----------|---------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B4  | CRUD food journal           | endpoint | `log_food`, `get_food_log`, `delete_food`, `get_food_stats` | lib.rs:L5012-5068 | 57  | Simple     | —        | —       |
| B5  | CRUD recipes                | endpoint | `create_recipe`, `get_recipes`, `delete_recipe`         | lib.rs:L5070-5121 | 52  | Simple     | —        | —       |
| B6  | CRUD products (expiry)      | endpoint | `add_product`, `get_products`, `update_product`, `delete_product`, `get_expiring` | lib.rs:L5123-5204 | 82  | Medium     | 5        | ✅      |

### Money

| ID  | Function Group              | Type     | Key Functions                                           | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|----------|---------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B7  | CRUD transactions           | endpoint | `add_transaction`, `get_transactions`, `delete_transaction`, `get_transaction_stats` | lib.rs:L5204-5279 | 76  | Medium     | 4        | ✅      |
| B8  | CRUD budgets                | endpoint | `create_budget`, `get_budgets`, `delete_budget`         | lib.rs:L5281-5321 | 41  | Simple     | —        | —       |
| B9  | CRUD savings goals          | endpoint | `create_savings_goal`, `get_savings_goals`, `update_savings_goal`, `delete_savings_goal` | lib.rs:L5322-5373 | 52  | Simple     | —        | —       |
| B10 | CRUD subscriptions          | endpoint | `add_subscription`, `get_subscriptions`, `update_subscription`, `delete_subscription` | lib.rs:L5374-5417 | 44  | Simple     | —        | —       |
| B11 | CRUD debts                  | endpoint | `add_debt`, `get_debts`, `update_debt`, `delete_debt`   | lib.rs:L5418-5468 | 51  | Simple     | —        | —       |

### Mindset

| ID  | Function Group              | Type     | Key Functions                                           | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|----------|---------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B12 | Journal (diary)             | endpoint | `save_journal_entry`, `get_journal_entries`              | lib.rs:L5468-5524 | 57  | Medium     | 3        | ✅      |
| B13 | Mood logging                | endpoint | `log_mood`, `get_mood_history`                           | lib.rs:L5525-5555 | 31  | Simple     | —        | —       |
| B14 | CRUD principles             | endpoint | `create_principle`, `get_principles`, `update_principle`, `delete_principle` | lib.rs:L5555-5596 | 42  | Simple     | —        | —       |
| B15 | Mindset check (aggregate)   | endpoint | `get_mindset_check`                                      | lib.rs:L5597-5612 | 16  | Simple     | —        | —       |

### Home

| ID  | Function Group              | Type     | Key Functions                                           | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|----------|---------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B16 | CRUD home inventory         | endpoint | `add_home_item`, `get_home_items`, `update_home_item`, `delete_home_item`, `toggle_home_item` | lib.rs:L5742-5801 | 60  | Simple     | —        | —       |

### Contacts

| ID  | Function Group              | Type     | Key Functions                                           | Location          | LOC | Complexity | Subfuncs | Phase 3 |
|-----|-----------------------------|----------|---------------------------------------------------------|-------------------|-----|------------|----------|---------|
| B17 | CRUD contacts               | endpoint | `add_contact`, `get_contacts`, `update_contact`, `delete_contact`, `toggle_blocked`, `toggle_favorite` | lib.rs:L5798-5906 | 109 | Medium     | 5        | ✅      |
| B18 | Contact blocking            | endpoint | `add_contact_block`, `get_contact_blocks`, `delete_contact_block`, `toggle_contact_block` | lib.rs:L5907-5959 | 53  | Simple     | —        | —       |

## Frontend (main.js)

| ID | Function Group                            | Type | Key Functions                                                        | Location           | LOC | Complexity | Subfuncs | Phase 3 |
|----|------------------------------------------|------|----------------------------------------------------------------------|--------------------|-----|------------|----------|---------|
| F1 | Home supplies + shopping list            | view | `loadHome`, `loadSupplies`, `loadShoppingList`                       | main.js:L1699-1801 | 103 | Medium     | 5        | ✅      |
| F2 | Mindset (journal, mood, principles)      | view | `loadMindset`, `loadJournal`, `loadMoodLog`, `loadPrinciples`        | main.js:L1802-1909 | 108 | Medium     | 4        | ✅      |
| F3 | Food (log, recipes, products)            | view | `loadFood` + sub-loaders + modals                                    | main.js:L1910-2119 | 210 | Complex    | 7        | ✅      |
| F4 | Money (all sub-tabs)                     | view | `loadMoney`, `loadTransactions`, `loadBudgets`, `loadSavings`, `loadSubscriptions`, `loadDebts` | main.js:L2120-2339 | 220 | Complex    | 7        | ✅      |
| F5 | Contacts                                 | view | `loadPeople`, `showAddContactModal`                                  | main.js:L2340-2497 | 158 | Complex    | 5        | ✅      |
| F6 | Sports (workouts, martial arts, stats)   | view | `loadSports` + sub-loaders + `showAddWorkoutModal`                   | main.js:L4774-4924 | 151 | Complex    | 5        | ✅      |
| F7 | Health (metrics, habits)                 | view | `loadHealth`, `renderHealth`                                         | main.js:L4925-5058 | 134 | Complex    | 5        | ✅      |

## Summary

- **Total functions**: 25 (18 backend + 7 frontend)
- **Simple**: 12 (B1, B2, B4, B5, B8, B9, B10, B11, B13, B14, B15, B18)
- **Medium**: 6 (B3, B6, B7, B12, B17, F1, F2)
- **Complex**: 5 (F3, F4, F5, F6, F7) — all frontend
- **Phase 3 decomposed**: 12 functions → 53 subfunctions total
