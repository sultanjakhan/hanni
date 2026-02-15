# F7 — Health (metrics, habits): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F7: Health (metrics, habits) |
| Файл(ы) | main.js:L4925-5022 |
| LOC | 98 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F7.1 | Загрузка данных Health | L4925-L4937 | 13 | `loadHealth()` → invoke get_health_today + get_habits_today → делегирование renderHealth | control + bridge | Trivial |
| F7.2 | Рендер метрик здоровья | L4939-L4976 | 38 | `renderHealth(el, today, habits)` → helper metricClass + HTML-рендер 4 метрик (sleep, water, mood, weight) с цветовой индикацией (good/warning/bad) | render | Medium |
| F7.3 | Рендер списка привычек с toggle | L4977-L4998 | 22 | Цикл по habits → создание DOM-элементов habit-item + обработчик check_habit click → invoke check_habit → loadHealth() | render + interaction | Low |
| F7.4 | Обработчики клика по метрикам и кнопки "Записать" | L5000-L5017 | 18 | Клик на health-metric → prompt(type) → invoke log_health; Клик "+ Записать" → prompt(type, value) → invoke log_health | interaction | Low |
| F7.5 | Кнопка добавления привычки | L5019-L5022 | 4 | Клик "+ Добавить" → prompt(name) → invoke create_habit → loadHealth() | interaction | Trivial |

## Data Flow

```
[loadHealth]
      │
      ▼
  invoke('get_health_today')  ──→ [Backend: B2]
  invoke('get_habits_today')  ──→ [Backend: B3]
      │
      ▼
  renderHealth(pc, today, habits)

[renderHealth]
      │
      ├── metricClass(type, val) → CSS class (good/warning/bad)
      │
      ├── Рендер 4 метрик:
      │   sleep (>7 good, >5 warning, <5 bad)
      │   water (>8 good, >4 warning, <4 bad)
      │   mood  (>4 good, >3 warning, <3 bad)
      │   weight (нет порогов)
      │
      ├── Рендер привычек:
      │   for each habit → habit-item DOM
      │       └── check click → invoke('check_habit') ──→ [Backend: B3]
      │                              → loadHealth()
      │
      ├── Клик по метрике:
      │   prompt(label) → invoke('log_health') ──→ [Backend: B2]
      │                       → loadHealth()
      │
      ├── "+ Записать":
      │   prompt(type, value) → invoke('log_health') → loadHealth()
      │
      └── "+ Добавить привычку":
          prompt(name) → invoke('create_habit') ──→ [Backend: B3]
                              → loadHealth()
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F7.4 | Обработчики метрик | Используют `prompt()` для ввода — плохой UX, нет валидации, нет выбора типа из списка | Заменить на модалку с числовым input и select для типа | Low |
| F7.2 | Рендер метрик | Пороговые значения (7/5 для сна, 8/4 для воды) захардкожены — нельзя настроить | Вынести пороги в настройки или константы | Low |
