# B1 — Training data generation: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: Training data generation (77+ examples) |
| Файл(ы) | scripts/generate_training_data.py:L1-497 |
| LOC | 497 |
| Подфункций | 7 |
| Сложность (max) | High |

## Подфункции

### Backend (Script)

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B1.1 | Конфигурация путей и чтение system prompt | L1-13 | 13 | -- -> SYSTEM_PROMPT, paths | File: read system_prompt.txt | control | Trivial |
| B1.2 | Шаблон контекста и функция make_example() | L15-34 | 20 | messages, context params -> training example | -- | transform | Low |
| B1.3 | Примеры Memory (remember, recall, search, forget) | L42-79 | 38 | -- -> examples[] | -- | transform | Low |
| B1.4 | Примеры Calendar, Money, Food, Health, Mood, Journal, Media | L81-197 | 117 | -- -> examples[] | -- | transform | Medium |
| B1.5 | Примеры Focus, Activity, Fitness, Notes, Home, Goals, System, Tasks, Proactive, Multi-step, Ambiguous, Casual, Notification, Products, Supplies, Calendar Sync, Memory Context, Shell | L199-414 | 216 | -- -> examples[] | -- | transform | High |
| B1.6 | Примеры Web Search (8 примеров) | L421-476 | 56 | -- -> examples[] | -- | transform | Medium |
| B1.7 | Разбиение на train/valid/test и запись файлов | L478-497 | 20 | examples[] -> JSONL files | File: write train.jsonl, valid.jsonl, test.jsonl | persist | Low |

## Data Flow

```
[Entry: python3 generate_training_data.py]
    │
    ▼
┌─ B1.1 Load Config ─────────────────────┐
│  HANNI_DIR = ~/Library/.../Hanni        │
│  read system_prompt.txt                  │ ←── [File]
│  OUTPUT_DIR = HANNI_DIR/training         │
└──────┬──────────────────────────────────┘
       │ SYSTEM_PROMPT
       ▼
┌─ B1.2 make_example() ──────────────────┐
│  Обёртка: system_prompt + context +     │
│  conversation messages -> {messages:[]} │
└──────┬─────────────────────────────────┘
       │
       ▼
┌─ B1.3-B1.6 Generate Examples ──────────────────────┐
│  77+ примеров по 30 категориям:                     │
│                                                     │
│  B1.3: Memory (7 примеров)                          │
│    remember, recall, search_memory, forget           │
│                                                     │
│  B1.4: Calendar/Money/Food/Health/Mood/Journal/Media │
│    (25 примеров)                                     │
│    create_event, add_transaction, log_food,           │
│    log_health, log_mood, save_journal, add_media      │
│                                                     │
│  B1.5: Focus/Activity/Fitness/Notes/Home/Goals/      │
│    System/Tasks/Proactive/Multi-step/Ambiguous/       │
│    Casual/Notification/Products (32 примера)          │
│                                                     │
│  B1.6: Web Search (8 примеров)                       │
│    web_search + результат + пересказ                  │
└──────┬─────────────────────────────────────────────┘
       │ examples: list[dict]
       ▼
┌─ B1.7 Split & Write ───────────────────┐
│  80% train / 10% valid / 10% test       │
│  train.jsonl                             │ ──→ [File: HANNI_DIR/training/]
│  valid.jsonl                             │
│  test.jsonl                              │
└─────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B1.5 | Основные примеры | 216 LOC — слишком много примеров в одном блоке | Разбить по категориям в отдельные функции или файлы | Low |
| B1.3-B1.6 | Все примеры | Хардкод примеров — нет валидации JSON в action-блоках | Добавить валидацию action JSON при генерации | Medium |
| B1.7 | Split | Фиксированный split 80/10/10 — при малом количестве примеров valid/test могут быть пустыми | Добавить min(1) для каждого split | Low |
