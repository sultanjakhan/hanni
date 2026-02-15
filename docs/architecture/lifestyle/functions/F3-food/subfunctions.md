# F3 — Food (log, recipes, products): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F3: Food (log, recipes, products) |
| Файл(ы) | main.js:L1910-2117 |
| LOC | 208 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.1 | Роутинг Food по sub-tab | L1910-L1919 | 10 | `loadFood(subTab)` → header, делегирование loadFoodLog / loadRecipes / loadProducts | control | Trivial |
| F3.2 | Рендер Food Log со статистикой | L1921-L1953 | 33 | `loadFoodLog(el)` → invoke get_food_log + get_food_stats → карточки макросов + группировка по meal_type + delete | render + interaction | Medium |
| F3.3 | Модалка добавления еды | L1956-L1994 | 39 | Клик "+ Log Food" → модалка (meal, name, cal, protein, carbs, fat) → invoke log_food → loadFoodLog() | interaction | Medium |
| F3.4 | Рендер Recipes через DatabaseView | L1996-L2015 | 20 | `loadRecipes(el)` → invoke get_recipes → renderDatabaseView с fixedColumns (name, prep, calories, tags) | render | Low |
| F3.5 | Модалка добавления рецепта | L2017-L2052 | 36 | Клик "+ Add Recipe" → модалка (name, ingredients, instructions, prep, calories, tags) → invoke create_recipe → loadRecipes() | interaction | Medium |
| F3.6 | Рендер Products через DatabaseView | L2054-L2078 | 25 | `loadProducts(el)` → invoke get_products → renderDatabaseView с fixedColumns (name, location, qty, expiry) + expiry highlight | render | Low |
| F3.7 | Модалка добавления продукта | L2080-L2117 | 38 | Клик "+ Add Product" → модалка (name, location, qty, unit, expiry_date) → invoke add_product → loadProducts() | interaction | Medium |

## Data Flow

```
[loadFood(subTab)]
      │
      ├── 'Food Log' → loadFoodLog(pc)
      ├── 'Recipes' → loadRecipes(pc)
      ├── 'Products' → loadProducts(pc)
      └── default → loadFoodLog(pc)

[loadFoodLog]
      │
      ▼
  invoke('get_food_log', date=today) ──→ [Backend: B4]
  invoke('get_food_stats', days=1)   ──→ [Backend: B4]
      │
      ▼
  Статистика: Calories, Protein, Carbs, Fat
  + Группировка по meal_type: breakfast → lunch → dinner → snack
      │
      ├── Delete click → invoke('delete_food_entry') → loadFoodLog()
      └── "+ Log Food" → showAddFoodModal()

[showAddFoodModal]
      │
      ▼
  Модалка: meal, name, calories, protein, carbs, fat
      │
      ▼
  Save → invoke('log_food') ──→ [Backend: B4]
      │
      ▼
  loadFoodLog()

[loadRecipes]
      │
      ▼
  invoke('get_recipes') ──→ [Backend: B5]
      │
      ▼
  renderDatabaseView() ──→ таблица с колонками
      │
      └── "+ Add Recipe" → showAddRecipeModal()

[showAddRecipeModal]
      │
      ▼
  Модалка: name, ingredients, instructions, prep_time, calories, tags
      │
      ▼
  Save → invoke('create_recipe') → loadRecipes()

[loadProducts]
      │
      ▼
  invoke('get_products') ──→ [Backend: B6]
      │
      ▼
  renderDatabaseView() с highlight истекающих (< 3 дней)
      │
      └── "+ Add Product" → showAddProductModal()

[showAddProductModal]
      │
      ▼
  Модалка: name, location, qty, unit, expiry_date
      │
      ▼
  Save → invoke('add_product') → loadProducts()
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F3.3 / F3.5 / F3.7 | Модалки | Три модалки с одинаковым паттерном (overlay → form → validate → invoke → remove) — дублирование | Вынести общий `showModal(config)` хелпер | Low |
| F3.2 | Food Log | Статистика берётся за 1 день (`days: 1`), но возвращает `avg_*` — может путать при пустых данных | Уточнить: для одного дня показывать `total`, не `avg` | Low |
