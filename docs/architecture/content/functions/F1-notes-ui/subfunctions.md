# F1 — Notes list + editor: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Notes (list + editor) |
| Файл(ы) | main.js:L3162-3287 |
| LOC | 126 |
| Подфункций | 7 |
| Сложность (max) | Medium |

## Подфункции

### loadNotes (L3162-3205)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Загрузка и рендер каркаса заметок | L3162-3181 | 20 | `subTab?` → fetch notes → `el.innerHTML` (layout: список + редактор) | bridge + render | Low |
| F1.2 | Обработчик создания новой заметки | L3185-3191 | 7 | `#new-note-btn` click → `invoke('create_note')` → reload | interaction + bridge | Trivial |
| F1.3 | Обработчик поиска с debounce | L3193-3201 | 9 | `#notes-search` input → debounce 300ms → `invoke('get_notes', { search })` → renderNotesList() | interaction + bridge | Trivial |

### renderNotesList (L3207-3223)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.4 | Рендер списка заметок | L3207-3223 | 17 | `notes[]` → DOM elements в `#notes-list-items` (title, preview, date, pinned icon, click → openNote) | render | Low |

### openNote (L3225-3284)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.5 | Рендер редактора заметки | L3225-3244 | 20 | `id` → fetch note → `#notes-editor-panel` innerHTML (title input, tags, textarea, action buttons) | bridge + render | Low |
| F1.6 | Авто-сохранение при вводе | L3246-3258 | 13 | `input` events на title/body/tags → debounce 1000ms → `invoke('update_note')` | interaction + bridge | Trivial |
| F1.7 | Обработчики действий (pin/archive/delete) | L3260-3281 | 22 | `#note-pin-btn` / `#note-archive-btn` / `#note-delete-btn` click → `invoke('update_note'/'delete_note')` → reload | interaction + bridge | Low |

## Data Flow

```
[Input: subTab?]
    │
    ▼
┌─ F1.1 Загрузка каркаса ──────┐
│  invoke('get_notes', {filter}) │ ──→ [Backend: get_notes]
│  el.innerHTML = layout          │ ──→ [DOM]
│  renderNotesList(notes)         │
└──────┬─────────────────────────┘
       │
       ├──────────────────────────────────┐
       ▼                                  ▼
┌─ F1.2 Новая заметка ─┐  ┌─ F1.3 Поиск ──────────────┐
│  invoke('create_note') │  │  debounce 300ms             │
│  → loadNotes()         │  │  invoke('get_notes',{search})│
└────────────────────────┘  │  → renderNotesList()         │
                            └──────────────────────────────┘
       │
       ▼
┌─ F1.4 Список заметок ────────┐
│  for note → DOM item           │
│  click → openNote(id)          │
└──────┬────────────────────────┘
       │ click
       ▼
┌─ F1.5 Редактор заметки ──────┐
│  invoke('get_note', {id})      │ ──→ [Backend: get_note]
│  title input + textarea + tags │ ──→ [DOM]
└──────┬────────────────────────┘
       │
       ├──────────────────────────────────┐
       ▼                                  ▼
┌─ F1.6 Авто-сохранение ──┐  ┌─ F1.7 Действия ──────────┐
│  debounce 1000ms          │  │  pin/unpin                 │
│  invoke('update_note')    │  │  archive/unarchive         │
│  ──→ [Backend]            │  │  delete (confirm)          │
└───────────────────────────┘  │  → loadNotes()             │
                               └─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F1.7 | Действия pin/archive/delete | При pin/archive передаются текущие title/content/tags из объекта note (замыкание), а не актуальные из формы — если пользователь редактировал и нажал pin до авто-сохранения, изменения теряются | Считывать актуальные значения из DOM inputs при каждом действии | Medium |
