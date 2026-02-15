# B1 — Training stats and export: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: Training stats and export |
| Файл(ы) | lib.rs:L2133-2293 |
| LOC | 161 |
| Подфункций | 9 |
| Сложность (max) | Medium |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.1 | Получение статистики диалогов | L2133-2161 | 29 | db State → JSON (conversations, total_messages, earliest, latest) | DB read: conversations | query | Low |
| B1.2 | Загрузка карты рейтингов | L2163-2179 | 17 | db State → HashMap<conv_id, HashMap<msg_idx, rating>> | DB read: message_feedback | query | Low |
| B1.3 | Выборка диалогов для экспорта | L2181-2190 | 10 | db State → Vec<(id, messages_json, summary)> | DB read: conversations | query | Trivial |
| B1.4 | Итерация и фильтрация сообщений | L2192-2215 | 24 | rows + feedback_map → цикл: парсинг JSON, подсчёт real_msgs | — | transform | Low |
| B1.5 | Формирование training-примера | L2217-2245 | 29 | messages + ratings → JSON example (с system prompt, без негативных) | — | transform | Low |
| B1.6 | Сортировка: rated first | L2248-2253 | 6 | rated_examples + unrated_examples → training_examples[] | — | transform | Trivial |
| B1.7 | Разбиение 80/10/10 | L2256-2262 | 7 | training_examples → (train, valid, test) слайсы | — | transform | Trivial |
| B1.8 | Запись JSONL файлов | L2264-2278 | 15 | (train, valid, test) слайсы → 3 файла .jsonl | File write: training/*.jsonl | persist | Low |
| B1.9 | Обновление feedback + ответ | L2280-2293 | 14 | пути файлов + счётчики → JSON ответ | DB write: message_feedback.exported | persist | Trivial |

## Data Flow

```
[Input: db State]
    │
    ▼
┌─ B1.1 get_training_stats ─┐
│  3 COUNT/SUM/MIN/MAX запроса │
│  → json!(conversations,       │
│    total_messages, earliest,  │
│    latest)                    │
└───────────────────────────────┘
        (отдельная команда)

[Input: db State]
    │
    ▼
┌─ B1.2 Загрузка рейтингов ──┐
│  SELECT * FROM message_feedback│ ←── [DB: message_feedback]
│  → HashMap<conv_id,            │
│       HashMap<msg_idx, rating>>│
└──────┬────────────────────────┘
       │ feedback_map
       ▼
┌─ B1.3 Выборка диалогов ────┐
│  SELECT id, messages, summary  │ ←── [DB: conversations]
│  WHERE message_count >= 4      │
└──────┬────────────────────────┘
       │ rows[]
       ▼
┌─ B1.4 Фильтрация ──────────┐
│  for (conv_id, json, _) in    │
│    rows:                       │
│  parse JSON → filter real_msgs │
│  skip if < 2 real messages     │
└──────┬────────────────────────┘
       │ messages per conv
       ▼
┌─ B1.5 Формирование примера ─┐
│  system prompt + messages      │
│  skip assistant с rating=-1    │
│  clean /no_think suffix        │
│  → rated / unrated bucket      │
└──────┬────────────────────────┘
       │ rated_examples, unrated_examples
       ▼
┌─ B1.6 Сортировка ──────────┐
│  rated first, then unrated     │
└──────┬────────────────────────┘
       │ training_examples[]
       ▼
┌─ B1.7 Разбиение 80/10/10 ──┐
│  train = 80%, valid = 10%,     │
│  test = 10%                    │
└──────┬────────────────────────┘
       │ (train, valid, test)
       ▼
┌─ B1.8 Запись файлов ────────┐
│  create_dir_all(training/)     │ ──→ [FS: training/]
│  write train.jsonl, valid.jsonl│ ──→ [FS: *.jsonl]
│  write test.jsonl              │
└──────┬────────────────────────┘
       │
       ▼
┌─ B1.9 Финализация ─────────┐
│  UPDATE message_feedback       │ ──→ [DB: message_feedback]
│    SET exported=1              │
│  → json!(paths, counts)       │
└───────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B1.4+B1.5 | Фильтрация + формирование | Один цикл for содержит и фильтрацию, и трансформацию (~50 LOC суммарно) | Вынести в отдельные функции: filter_messages(), build_training_example() | Low |
| B1.2 | Загрузка рейтингов | filter_map(ok) — молча игнорирует ошибки строк | Логировать ошибки или использовать collect::<Result<>> | Low |
