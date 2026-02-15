# F1 — Вкладка памяти (все факты): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Вкладка памяти (все факты) |
| Файл(ы) | main.js:L2505-2608 |
| LOC | 104 |
| Подфункций | 5 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Загрузка фактов и рендер списка | L2505-2524 | 20 | el → invoke('get_all_memories') → HTML list | render | Low |
| F1.2 | Обработчики удаления фактов | L2526-2531 | 6 | click delete → invoke('delete_memory') → reload | interaction | Trivial |
| F1.3 | Обработчики редактирования (модальное окно) | L2533-2572 | 40 | click edit → modal form → invoke('update_memory') → reload | interaction | Medium |
| F1.4 | Обработчик добавления нового факта (модальное окно) | L2574-2606 | 33 | click "+ Добавить" → modal form → invoke('memory_remember') → reload | interaction | Medium |
| F1.5 | Обработка ошибок загрузки | L2607-2608 | 2 | error → "Ошибка: ..." | error | Trivial |

## Data Flow

```
[Input: el (DOM container)]
    │
    ▼
┌─ F1.1 Load + Render ───────┐
│  invoke('get_all_memories')  │ ──→ [Tauri: get_all_memories]
│  build HTML with memory items│ ──→ [DOM: memory-all-list]
└──────┬──────────────────────┘
       │ DOM rendered
       ▼
┌─ F1.2 Delete Handlers ─────┐
│  forEach [data-mdel] btn     │
│  confirm() → delete_memory   │ ──→ [Tauri: delete_memory]
│  → reload loadAllFacts(el)   │
└──────┬──────────────────────┘
       │
       ▼
┌─ F1.3 Edit Handlers ───────┐
│  forEach [data-medit] btn    │
│  create modal overlay        │ ──→ [DOM: modal-overlay]
│  populate: category/key/val  │
│  save → update_memory        │ ──→ [Tauri: update_memory]
│  → reload loadAllFacts(el)   │
└──────┬──────────────────────┘
       │
       ▼
┌─ F1.4 Add Handler ─────────┐
│  #mem-tab-add-btn click      │
│  create modal overlay        │ ──→ [DOM: modal-overlay]
│  save → memory_remember      │ ──→ [Tauri: memory_remember]
│  → reload loadAllFacts(el)   │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F1.3 | Edit modal | 40 LOC — создание модального окна inline, дублирует код с F1.4 и F2 (renderMemoryList) | Вынести создание модалки в переиспользуемую функцию createMemoryModal() | Medium |
| F1.4 | Add modal | Дублирование HTML-шаблона модалки с F1.3 | Объединить в один шаблон с параметром (edit vs add) | Medium |
