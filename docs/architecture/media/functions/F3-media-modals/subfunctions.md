# F3 — Add and Detail Media Modals: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F3: Add and detail media modals |
| Файл(ы) | main.js:L4681-4773 |
| LOC | 93 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

### showAddMediaModal (L4681-4727)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.1 | Создание модального окна добавления | L4681-4706 | 26 | `mediaType` → DOM overlay (title, status, year, rating, episodes?, notes) | render | Low |
| F3.2 | Обработчик закрытия | L4707-4708 | 2 | overlay click / cancel → `overlay.remove()` | interaction | Trivial |
| F3.3 | Обработчик сохранения | L4709-4727 | 19 | `#media-save` click → validate title → `invoke('add_media_item')` → `overlay.remove()` → `loadHobbies()` | interaction + bridge | Low |

### showMediaDetail (L4729-4771)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F3.4 | Создание модального окна деталей | L4729-4748 | 20 | `item, mediaType` → DOM overlay (title, year, status select, rating, progress?, notes) | render | Low |
| F3.5 | Обработчик обновления | L4751-4764 | 14 | `#md-save` click → `invoke('update_media_item')` → `overlay.remove()` → `loadHobbies()` | interaction + bridge | Low |
| F3.6 | Обработчик удаления | L4765-4770 | 6 | `#md-delete` click → `confirm()` → `invoke('delete_media_item')` → `overlay.remove()` → `loadHobbies()` | interaction + bridge | Trivial |

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F3.1/F3.4 | Модальные окна | Дублирование структуры модального окна (overlay, form, actions) — общий паттерн | Создать утилиту `createModal(title, fields[], onSave)` | Low |
