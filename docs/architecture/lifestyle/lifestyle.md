# Module 12: Lifestyle

## Description

Six lifestyle sub-domains: Food (log, recipes, products with expiry), Money (transactions, budgets, savings, subscriptions, debts), Mindset (journal, mood, principles), Health/Fitness (workouts, health metrics, habits), Contacts (people, blocking), and Home (supplies inventory, shopping list). The largest module by function count.

## Overview

| Attribute        | Value                                                              |
|------------------|--------------------------------------------------------------------|
| Domain           | Food, Money, Mindset, Health/Fitness, Contacts, Home               |
| Total LOC        | ~2080                                                              |
| Backend          | `lib.rs` (L4520-4676, L5012-5612, L5742-5959)                    |
| Frontend         | `main.js` (L1699-2497, L4774-5058)                               |
| Backend functions | 18                                                                |
| Frontend functions | 7                                                               |
| Complexity       | Simple: 12, Medium: 6, Complex: 5 (frontend only)                 |

## Files

| File                              | Lines          | Role                                  |
|-----------------------------------|----------------|---------------------------------------|
| `desktop/src-tauri/src/lib.rs`    | L4520-4676     | Health/Fitness (workouts, health, habits) |
| `desktop/src-tauri/src/lib.rs`    | L5012-5612     | Food, Money, Mindset                   |
| `desktop/src-tauri/src/lib.rs`    | L5742-5959     | Home, Contacts                         |
| `desktop/src/main.js`             | L1699-2497     | Home, Mindset, Food, Money, Contacts UI |
| `desktop/src/main.js`             | L4774-5058     | Sports, Health UI                       |

## Dependencies

| Direction | Module    | Relationship                                        |
|-----------|-----------|-----------------------------------------------------|
| Uses      | core      | DB access for all 18 backend endpoints               |
| Used by   | chat      | Action execution creates food logs, transactions, etc. |
| Used by   | proactive | Provides lifestyle context for proactive messages    |
| Used by   | tracker   | Dashboard aggregates lifestyle data                  |

## Sub-domains

### Health/Fitness
Workouts with stats, daily health metric logging (weight, sleep, water, steps), habit tracking with daily check-in.

### Food
Food journal with calories/macros, recipe storage, product inventory with expiry date tracking and alerts.

### Money
Transaction log with stats, budgets, savings goals with progress, recurring subscriptions, debt tracking.

### Mindset
Journal entries (mood, energy, stress, gratitude, wins), mood history log, personal principles CRUD, aggregated mindset check.

### Home
Supplies inventory with toggle (in-stock/needed), shopping list management.

### Contacts
Contact management with favorites, blocking/unblocking, and contact block list.
