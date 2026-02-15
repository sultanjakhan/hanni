# F6 — Sports (workouts, martial arts, stats): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F6: Sports (workouts, martial arts, stats) |
| Файл(ы) | main.js:L4774-4922 |
| LOC | 149 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F6.1 | Роутинг Sports по sub-tab | L4774-L4794 | 21 | `loadSports(subTab)` → header + делегирование: 'Martial Arts' → loadMartialArts, 'Stats' → loadSportsStats, default → invoke get_workouts + get_workout_stats → renderSports | control | Low |
| F6.2 | Рендер Martial Arts | L4796-L4822 | 27 | `loadMartialArts(el)` → invoke get_workouts → filter(type=martial_arts) → таблица (дата, название, время, калории) + кнопка "+ Тренировка" | render | Low |
| F6.3 | Рендер статистики | L4824-L4847 | 24 | `loadSportsStats(el)` → invoke get_workout_stats + get_workouts → dashboard stats (count, total_minutes, total_calories) + агрегация по типам | render + process | Low |
| F6.4 | Рендер основного списка тренировок | L4849-L4879 | 31 | `renderSports(el, workouts, stats)` → dashboard stats + workout cards (title, date, type badge, duration, calories) + кнопка "+ Тренировка" | render | Low |
| F6.5 | Модалка добавления тренировки | L4881-L4922 | 42 | `showAddWorkoutModal()` → модалка (type, title, duration, calories, notes) → invoke create_workout → loadSports() | interaction | Medium |

## Data Flow

```
[loadSports(subTab)]
      │
      ├── 'Martial Arts' → loadMartialArts(pc)
      │       │
      │       ▼
      │   invoke('get_workouts') ──→ [Backend: B1]
      │       │
      │       ▼
      │   filter(type === 'martial_arts') → таблица
      │       │
      │       └── "+ Тренировка" → showAddWorkoutModal() (preset martial_arts)
      │
      ├── 'Stats' → loadSportsStats(pc)
      │       │
      │       ▼
      │   invoke('get_workout_stats') ──→ [Backend: B1]
      │   invoke('get_workouts')      ──→ [Backend: B1]
      │       │
      │       ▼
      │   Агрегация byType{} → dashboard + breakdown
      │
      └── default → renderSports(pc, workouts, stats)
              │
              ▼
          invoke('get_workouts')      ──→ [Backend: B1]
          invoke('get_workout_stats') ──→ [Backend: B1]
              │
              ▼
          Dashboard stats + workout cards
              │
              └── "+ Тренировка" → showAddWorkoutModal()

[showAddWorkoutModal]
      │
      ▼
  Модалка: type (select), title, duration, calories, notes
      │
      ▼
  Save → invoke('create_workout') ──→ [Backend: B1]
      │
      ▼
  loadSports()
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F6.1 / F6.3 | Роутинг + Статистика | Двойной вызов get_workouts (в loadSports для renderSports и в loadSportsStats) — данные загружаются дважды | Кэшировать результат или передавать через параметр | Low |
| F6.2 | Martial Arts | Фильтрация `type=martial_arts` на фронте — лучше фильтровать на бекенде | Добавить параметр `workout_type` в `get_workouts` | Low |
