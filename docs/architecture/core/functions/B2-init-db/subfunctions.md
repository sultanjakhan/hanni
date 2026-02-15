# B2 — Инициализация БД: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Инициализация БД (40+ таблиц) |
| Файл(ы) | lib.rs:L346-879 |
| LOC | 534 |
| Подфункций | 12 |
| Сложность (max) | High |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B2.1 | Таблица facts + FTS5 + триггеры | L346-374 | 29 | conn -> () | DB: CREATE TABLE facts, facts_fts, 3 триггера | persist | Low |
| B2.2 | Таблица conversations + FTS5 + триггеры | L376-399 | 24 | conn -> () | DB: CREATE TABLE conversations, conversations_fts, 3 триггера | persist | Low |
| B2.3 | Таблицы activities, notes + FTS5 | L401-440 | 40 | conn -> () | DB: CREATE TABLE activities, notes, notes_fts, 3 триггера | persist | Medium |
| B2.4 | Таблицы events, projects, tasks, learning | L442-492 | 51 | conn -> () | DB: CREATE TABLE events, projects, tasks, learning_items | persist | Medium |
| B2.5 | Таблицы hobbies, workouts, exercises | L494-534 | 41 | conn -> () | DB: CREATE TABLE hobbies, hobby_entries, workouts, exercises | persist | Medium |
| B2.6 | Таблицы health_log, habits, habit_checks | L536-561 | 26 | conn -> () | DB: CREATE TABLE health_log, habits, habit_checks | persist | Low |
| B2.7 | Таблицы media_items, user_lists, list_items | L563-601 | 39 | conn -> () | DB: CREATE TABLE media_items, user_lists, list_items | persist | Medium |
| B2.8 | Таблицы food (food_log, recipes, products) | L603-644 | 42 | conn -> () | DB: CREATE TABLE food_log, recipes, products | persist | Medium |
| B2.9 | Таблицы money (transactions, budgets, savings, subscriptions, debts) | L646-701 | 56 | conn -> () | DB: CREATE TABLE transactions, budgets, savings_goals, subscriptions, debts | persist | Medium |
| B2.10 | Таблицы mindset (journal, mood_log, principles) + blocklist | L703-744 | 42 | conn -> () | DB: CREATE TABLE journal_entries, mood_log, principles, blocklist | persist | Medium |
| B2.11 | Таблицы goals, settings, home, contacts | L746-799 | 54 | conn -> () | DB: CREATE TABLE tab_goals, app_settings, home_items, contacts, contact_blocks | persist | Medium |
| B2.12 | Таблицы page_meta, properties, views, snapshots, proactive, feedback | L801-878 | 78 | conn -> () | DB: CREATE TABLE page_meta, property_definitions, property_values, view_configs, activity_snapshots, proactive_history, message_feedback | persist | High |

## Data Flow

```
[Input: &rusqlite::Connection]
    │
    ▼
┌─ B2.1-B2.12 ──────────────────────────────┐
│  Один execute_batch() вызов                 │
│  Содержит все CREATE TABLE IF NOT EXISTS    │
│  + CREATE VIRTUAL TABLE (FTS5)              │
│  + CREATE TRIGGER IF NOT EXISTS             │
│                                             │
│  Порядок создания:                          │
│  facts -> conversations -> activities ->    │
│  notes -> events -> projects -> tasks ->    │
│  learning -> hobbies -> workouts ->         │
│  health -> media -> food -> money ->        │
│  mindset -> blocklist -> goals ->           │
│  settings -> home -> contacts ->            │
│  page_meta -> properties -> views ->        │
│  snapshots -> proactive -> feedback         │
└──────────────┬──────────────────────────────┘
               │
               ▼
        Result<(), String>
        (map_err на L877-878)

[Side effects: 40+ таблиц в SQLite]
    │
    ▼
[DB: ~/Library/Application Support/Hanni/hanni.db]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B2.* | Вся функция | 534 LOC в одном execute_batch() — невозможно определить какая таблица вызвала ошибку | Разбить на отдельные execute() для каждой таблицы с отдельной обработкой ошибок | Medium |
| B2.12 | Последний блок | 78 LOC, 7 таблиц — самый крупный блок | Можно декомпозировать дальше | Low |
| B2.* | Вся функция | Нет системы миграций — ALTER TABLE отсутствует, все IF NOT EXISTS | Добавить версионирование схемы (schema_version) | Medium |
