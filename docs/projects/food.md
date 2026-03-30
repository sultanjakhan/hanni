# Food

## Purpose
Nutrition management — daily food logging by meal type, recipe collection, and pantry inventory with expiry tracking.

## DB Tables

| Table | Purpose |
|-------|---------|
| `food_log` | Daily food entries (meal_type, name, calories, protein, carbs, fat, date) |
| `recipes` | Recipe collection (title, ingredients, instructions, prep_time, servings) |
| `products` | Pantry inventory (name, category, quantity, expiry_date, location) |

## Views
- **Food Log** — daily meal tracking with macronutrient totals
- **Recipes** — recipe cards with ingredients and instructions
- **Products** — pantry inventory with expiry alerts

## Key Commands (Tauri)
- `get_food_log` / `log_food` / `update_food_entry` / `delete_food_entry` — food log CRUD
- `get_food_stats` — daily/weekly nutrition stats
- `get_recipes` / `create_recipe` / `update_recipe` / `delete_recipe` — recipe CRUD
- `get_products` / `add_product` / `update_product` / `delete_product` — pantry CRUD

## Relations
- **Schedule** — "healthy eating" routines tracked via Schedule
- **Home** — complements household inventory (Home = non-food, Food = food)

## Notable
- Meal types: breakfast, lunch, dinner, snack
- Macronutrient tracking: calories, protein, carbs, fat
- Product expiry date tracking with alerts
- Recipe ingredients linked to products for pantry awareness
