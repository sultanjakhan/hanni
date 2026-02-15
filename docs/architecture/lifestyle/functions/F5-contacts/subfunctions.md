# F5 — Contacts: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F5: Contacts |
| Файл(ы) | main.js:L2340-2495 |
| LOC | 156 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F5.1 | Загрузка и рендер списка контактов | L2340-L2395 | 56 | `loadPeople(subTab)` → invoke get_contacts + get_contact_blocks → фильтрация (All/Blocked/Favorites) → HTML-рендер карточек контактов с аватаром, деталями, блокировками, кнопками действий | render | Medium |
| F5.2 | Глобальные обработчики действий контактов | L2397-L2414 | 18 | window.toggleContactFav / toggleContactBlock / deleteContact / deleteContactBlock → invoke соответствующих backend-команд → loadPeople() | interaction | Low |
| F5.3 | Модалка блокировки сайта/приложения для контакта | L2415-L2448 | 34 | `showContactBlockModal(contactId, contactName)` → модалка (type: site/app, value, reason) → invoke add_contact_block → loadPeople() | interaction | Medium |
| F5.4 | Модалка добавления контакта | L2450-L2495 | 46 | `showAddContactModal()` → модалка (name, phone, email, category, relationship, notes, blocked, block_reason) → invoke add_contact → loadPeople() | interaction | Medium |

## Data Flow

```
[loadPeople(subTab)]
      │
      ▼
  invoke('get_contacts', filter) ──→ [Backend: B17]
      │
      ▼
  Фильтрация на фронте:
      ├── 'Blocked' → {blocked: true}
      ├── 'Favorites' → .filter(c => c.favorite)
      └── 'All' → все
      │
      ▼
  Для каждого контакта:
    invoke('get_contact_blocks', contactId) ──→ [Backend: B18]
      │
      ▼
  Рендер карточек:
    Avatar (первая буква) + name + relationship + phone + email
    + blocked badge + block_reason + contact blocks list
    + action buttons: [block-site] [fav] [block] [delete]

[Action buttons]
      │
      ├── toggleContactFav → invoke('toggle_contact_favorite') → loadPeople()
      ├── toggleContactBlock → invoke('toggle_contact_blocked') → loadPeople()
      ├── deleteContact → confirm → invoke('delete_contact') → loadPeople()
      ├── deleteContactBlock → invoke('delete_contact_block') → loadPeople()
      └── showContactBlockModal → модалка → invoke('add_contact_block') → loadPeople()

[showAddContactModal]
      │
      ▼
  Модалка: name*, phone, email, category, relationship, notes, blocked, block_reason
      │
      ▼
  Save → invoke('add_contact') ──→ [Backend: B17]
      │
      ▼
  loadPeople()
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F5.1 | Загрузка контактов | Последовательный `for await` для get_contact_blocks каждого контакта — N+1 проблема | Один бекенд-вызов, возвращающий контакты с их блоками (JOIN) | Medium |
| F5.2 | Глобальные обработчики | Используют `window.` и `onclick` в HTML-строках — не соответствует паттерну addEventListener в остальном коде | Перейти на data-атрибуты + addEventListener для единообразия | Low |
