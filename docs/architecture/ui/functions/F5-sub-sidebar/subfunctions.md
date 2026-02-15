# F5 — Sub-sidebar rendering: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F5: Sub-sidebar rendering |
| Файл(ы) | main.js:L483-530 |
| LOC | 48 |
| Подфункций | 4 |
| Сложность (max) | Low |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F5.1 | Guard: скрытие если нет subTabs | L483-491 | 9 | activeTab → если нет reg.subTabs, скрыть sidebar; иначе показать и очистить | control | Trivial |
| F5.2 | Рендер sub-tab элементов | L495-509 | 15 | subTabs[] + activeSubTab → для каждого sub: createElement .sub-sidebar-item + icon/dot + click handler → saveTabs + renderSubSidebar + loadSubTabContent | render | Low |
| F5.3 | Рендер нижней секции (Settings gear + version) | L510-528 | 19 | — → DOM: gear icon "Настройки" + click handler (openTab/switchTab settings) + version label | render | Low |
| F5.4 | Загрузка виджета целей | L529-530 | 2 | — → loadGoalsWidget() (делегация в F1) | bridge | Trivial |

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F5.2 | Sub-tab items | При клике на sub-tab вызывается полный re-render (renderSubSidebar) — рекурсивный вызов | Обновлять только active-класс без полного re-render | Low |
