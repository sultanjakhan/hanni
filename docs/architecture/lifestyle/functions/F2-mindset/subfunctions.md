# F2 — Mindset (journal, mood, principles): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F2: Mindset (journal, mood, principles) |
| Файл(ы) | main.js:L1802-1907 |
| LOC | 106 |
| Подфункций | 4 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.1 | Роутинг Mindset по sub-tab | L1802-L1811 | 10 | `loadMindset(subTab)` → рендер header, делегирование loadJournal / loadMoodLog / loadPrinciples | control | Trivial |
| F2.2 | Рендер и сохранение Journal | L1813-L1851 | 39 | `loadJournal(el)` → invoke get_journal_entry + get_journal_entries → форма (mood, energy, stress, gratitude, wins, struggles, reflection) + список → Save → invoke save_journal_entry | render + interaction | Medium |
| F2.3 | Рендер и логирование Mood | L1853-L1882 | 30 | `loadMoodLog(el)` → invoke get_mood_history → эмодзи-кнопки + история → клик → invoke log_mood → перезагрузка | render + interaction | Low |
| F2.4 | Рендер и управление Principles | L1884-L1907 | 24 | `loadPrinciples(el)` → invoke get_principles → список с toggle + delete → "+ Add" (prompt) → invoke create_principle | render + interaction | Low |

## Data Flow

```
[loadMindset(subTab)]
      │
      ├── 'Journal' → loadJournal(pc)
      ├── 'Mood' → loadMoodLog(pc)
      ├── 'Principles' → loadPrinciples(pc)
      └── default → loadJournal(pc)

[loadJournal]
      │
      ▼
  invoke('get_journal_entry', date=null) ──→ [Backend: B12]
  invoke('get_journal_entries', days=7)  ──→ [Backend: B12]
      │
      ▼
  Форма Today: mood, energy, stress, gratitude, wins, struggles, reflection
  + Recent Entries список
      │
      └── Save click → invoke('save_journal_entry') → loadJournal()

[loadMoodLog]
      │
      ▼
  invoke('get_mood_history', days=14)
      │
      ▼
  5 emoji-кнопок (1-5) + optional note + история
      │
      └── Emoji click → invoke('log_mood') → loadMoodLog()

[loadPrinciples]
      │
      ▼
  invoke('get_principles')
      │
      ▼
  Список: checkbox (active) + title + category + delete
      ├── Delete → invoke('delete_principle') → loadPrinciples()
      └── Add → prompt() → invoke('create_principle') → loadPrinciples()
```

## Проблемные зоны

Нет критичных проблем. Каждая подвкладка чётко изолирована.
