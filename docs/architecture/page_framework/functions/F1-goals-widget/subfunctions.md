# F1 — Goals widget: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Goals widget |
| Файл(ы) | main.js:L532-601 |
| LOC | 70 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Скрытие sidebar-секции + guard | L532-540 | 9 | renderSubSidebar() → скрывает sub-sidebar-goals, выход если chat/settings | control | Trivial |
| F1.2 | Очистка и загрузка целей | L542-546 | 5 | activeTab → invoke('get_goals') | bridge | Trivial |
| F1.3 | Рендер виджета целей | L547-563 | 17 | goals[] → DOM: .goals-inline с прогресс-барами и кнопкой "+ Goal" | render | Low |
| F1.4 | Обработчик кнопки "+ Goal" | L562-563 | 2 | click #add-goal-btn → showAddGoalModal() | interaction | Trivial |
| F1.5 | Модал добавления цели | L566-601 | 36 | showAddGoalModal() → modal overlay с формой → invoke('create_goal') → reload | interaction | Medium |

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F1.5 | Модал добавления | Пустой catch блок (L563) в loadGoalsWidget — ошибки молча игнорируются | Добавить логирование ошибок | Low |
