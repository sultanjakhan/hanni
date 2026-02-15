# Phase 3: Function → Subfunctions — Deep Decomposition (per function)

> **Модуль:** [MODULE_NAME]
> **Функция:** [FUNCTION_ID] — [FUNCTION_NAME]
> **Входной документ:** `docs/architecture/[module]/functions.md`
> **Трекер:** `docs/architecture/COVERAGE.md`
> **Цель:** Разобрать каждую бизнес-функцию на атомарные подфункции (шаги/блоки логики).

---

## Контекст

Phase 3 — третий шаг иерархии **Проект → Модули → Функции → Подфункции**.
Phase 2 определил бизнес-функции модуля (B1, F1, I1...).
Этот промпт **декомпозирует одну бизнес-функцию** на атомарные подфункции.

**Запускается для каждой функции, у которой Complexity = Medium или Complex.**
Simple функции (<50 LOC) обычно не требуют декомпозиции — пропускай их.

### Что такое «подфункция»

Подфункция — это **один логический шаг** внутри бизнес-функции:
- Принимает определённый input
- Выполняет одно действие
- Передаёт результат следующему шагу

Примеры:
| Бизнес-функция (Phase 2) | Подфункции (Phase 3) |
|--------------------------|---------------------|
| B1: Chat с AI | → валидация ввода → построение промпта → вызов LLM → стриминг ответа → парсинг action-блоков → выполнение действий → сохранение в историю |
| F1: Chat UI | → рендер списка сообщений → поле ввода → отображение стриминга → рендер action-результатов → скролл к новому сообщению |
| I1: Голосовой ввод | → захват аудио (cpal) → VAD (определение речи) → транскрипция (Whisper) → отправка текста в чат |

### Granularity

- **Слишком мелко:** "присвоить переменную", "вызвать функцию" — это НЕ подфункция
- **Правильно:** "валидация входных данных", "запрос к БД", "форматирование ответа"
- **Слишком крупно:** "обработка запроса" — это вся бизнес-функция, не подфункция

Ориентир: подфункция = **5-50 строк кода**, выполняющих один логический шаг.

---

## ШАГ 0: Загрузка контекста

### 0.1 — Прочитай functions.md

Открой `docs/architecture/[module]/functions.md`. Найди строку функции `[FUNCTION_ID]`:
- Файл и строки
- LOC и Complexity
- Тип (endpoint / internal / frontend / integration)

### 0.2 — Прочитай код функции

Прочитай код **полностью**. Если функция >300 строк — частями, но все части.

### 0.3 — Определи зависимости

```bash
# Что вызывает эта функция?
grep -n "[function_name]\|[related_calls]" [source_files]

# Кто вызывает эту функцию?
grep -rn "[function_name]" [source_dirs] --include="*.rs" --include="*.js"
```

Составь граф:
```
[Caller 1] → [FUNCTION] → [Dependency A]
[Caller 2] ↗              → [Dependency B]
                           → [DB table X]
                           → [External API Y]
```

---

## ШАГ 1: Идентификация подфункций

### 1.1 — Прочитай код и выдели логические блоки

Для каждого блока определи:
- **Что делает** (1 предложение)
- **Строки** (от-до)
- **Input** (что получает)
- **Output** (что возвращает/передаёт)
- **Side effects** (БД, файлы, внешние вызовы, state)

### 1.2 — Маркеры границ подфункций

Границу подфункции определяй по:

| Маркер | Пример |
|--------|--------|
| Смена типа операции | Валидация → Запрос к БД → Форматирование ответа |
| Блок try/catch/match | Каждый catch-блок может быть отдельным |
| Await / async вызов | Каждый значимый await = потенциальная граница |
| Условный блок (if/match) | Ветвление логики = отдельные подфункции |
| Цикл с логикой | Обработка коллекции = подфункция |
| Вызов внешней зависимости | DB query, API call, file I/O |
| Комментарий-разделитель | `// Step 1: ...`, `// --- Parse actions ---` |
| Пустая строка между блоками | Визуальное разделение автором |

### 1.3 — Составь список подфункций

Для backend функции (B):

| # | Подфункция | Строки | LOC | Input | Output | Side effects | Тип |
|---|-----------|--------|-----|-------|--------|-------------|-----|
| B1.1 | Валидация ввода | L100-L115 | 16 | raw request | validated data | — | validation |
| B1.2 | Построение промпта | L116-L150 | 35 | user message + context | prompt string | read memory DB | transform |
| B1.3 | Вызов LLM API | L151-L200 | 50 | prompt | response stream | HTTP call | external |
| B1.4 | Парсинг action-блоков | L201-L260 | 60 | response text | Action[] | — | parse |
| B1.5 | Выполнение действий | L261-L340 | 80 | Action[] | results[] | DB writes, shell, etc. | execute |
| B1.6 | Сохранение в историю | L341-L370 | 30 | message + response | — | DB insert | persist |

Для frontend функции (F):

| # | Подфункция | Строки | LOC | Trigger | Renders/Effect | Тип |
|---|-----------|--------|-----|---------|---------------|-----|
| F1.1 | Рендер списка сообщений | L50-L120 | 71 | messages[] state | DOM elements | render |
| F1.2 | Поле ввода + отправка | L121-L180 | 60 | user typing + Enter | invoke('chat') | interaction |
| F1.3 | Стриминг ответа | L181-L250 | 70 | stream event | progressive DOM update | stream |
| F1.4 | Рендер action-результатов | L251-L310 | 60 | action results | cards/widgets | render |

Для integration функции (I):

| # | Подфункция | Строки (BE) | Строки (FE) | LOC | Data flow | Тип |
|---|-----------|------------|------------|-----|-----------|-----|
| I1.1 | Захват аудио | — | main.js:L3000-L3050 | 51 | mic → samples | capture |
| I1.2 | VAD | lib.rs:L5000-L5080 | — | 81 | samples → speech segments | process |
| I1.3 | Транскрипция | lib.rs:L5081-L5150 | — | 70 | audio → text | transform |
| I1.4 | Отправка в чат | — | main.js:L3051-L3070 | 20 | text → chat input | bridge |

### 1.4 — Типы подфункций

| Тип | Описание | Пример |
|-----|----------|--------|
| `validation` | Проверка/парсинг входных данных | Zod schema, match guards, type checks |
| `query` | Чтение из БД/кеша | SELECT, prepared statement, cache lookup |
| `persist` | Запись в БД/кеш/файл | INSERT/UPDATE, cache set, file write |
| `transform` | Преобразование данных | Map, format, serialize, build prompt |
| `parse` | Парсинг текста/структуры | Regex, JSON parse, action block extraction |
| `external` | Вызов внешнего сервиса | HTTP API, LLM call, TTS |
| `render` | Отрисовка UI | DOM manipulation, component render |
| `interaction` | Обработка пользовательского ввода | Click handler, keyboard event, form submit |
| `stream` | Потоковая обработка | SSE, WebSocket, async iterator |
| `capture` | Захват данных от устройства | Mic, camera, clipboard |
| `process` | Обработка данных (не I/O) | Вычисление, фильтрация, агрегация |
| `bridge` | Связь между слоями | IPC invoke, event emit, callback |
| `control` | Управление потоком | State machine, routing, dispatch |
| `error` | Обработка ошибок | Error recovery, fallback, retry |

---

## ШАГ 2: Data Flow диаграмма

### 2.1 — Для Medium/Complex функций

Построй текстовую диаграмму потока данных:

```
[Input: user message]
    │
    ▼
┌─ B1.1 Validate ─┐
│  check empty      │
│  trim/sanitize    │
└──────┬───────────┘
       │ validated_msg
       ▼
┌─ B1.2 Build Prompt ─┐
│  load system prompt   │
│  fetch memory context │ ←── [DB: memory]
│  append conversation  │ ←── [DB: conversations]
│  format messages[]    │
└──────┬───────────────┘
       │ messages[]
       ▼
┌─ B1.3 Call LLM ──┐
│  POST /v1/chat     │ ──→ [External: MLX Server]
│  stream=true       │
│  handle timeout    │
└──────┬────────────┘
       │ response_stream
       ▼
┌─ B1.4 Parse Actions ─┐
│  regex: ```action      │
│  extract type + params │
│  validate action type  │
└──────┬───────────────┘
       │ Action[]
       ▼
┌─ B1.5 Execute ───┐
│  match action.type │
│  → remember()      │ ──→ [DB: memory]
│  → create_event()  │ ──→ [DB: events]
│  → run_shell()     │ ──→ [System: shell]
│  → web_search()    │ ──→ [External: DuckDuckGo]
└──────┬────────────┘
       │ results[]
       ▼
┌─ B1.6 Save History ─┐
│  INSERT conversation  │ ──→ [DB: conversations]
│  INSERT messages      │ ──→ [DB: messages]
└──────────────────────┘
```

### 2.2 — Для Simple функций

Data flow не требуется. Достаточно однострочного описания:
```
[Input] → validate → query DB → format → [Output]
```

---

## ШАГ 3: Метрики и оценка

### 3.1 — Сводка по подфункциям

| Метрика | Значение |
|---------|----------|
| Всего подфункций | [N] |
| Средний размер (LOC) | [avg] |
| Самая крупная | [ID]: [name] ([LOC] строк) |
| С side effects | [count] из [N] |
| С external calls | [count] из [N] |

### 3.2 — Сложность подфункций

| # | Подфункция | LOC | Side effects | External | Error handling | Сложность |
|---|-----------|-----|-------------|----------|---------------|-----------|
| B1.1 | Validate | 16 | — | — | return error | Trivial |
| B1.3 | Call LLM | 50 | — | HTTP stream | timeout + retry | High |
| B1.5 | Execute | 80 | DB, shell | varies | per-action | High |

**Шкала сложности подфункции:**
- **Trivial** — <15 LOC, нет side effects, нет ветвлений
- **Low** — 15-30 LOC, простые side effects (1 DB query)
- **Medium** — 30-60 LOC, несколько side effects, условная логика
- **High** — >60 LOC или external calls с error handling, state management

### 3.3 — Проблемные зоны

Отметь подфункции, которые требуют внимания:

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B1.5 | Execute | 80 LOC, 10+ action types в одном match | Вынести каждый action type в отдельную fn | Medium |
| B1.3 | Call LLM | Нет retry при timeout | Добавить retry с exponential backoff | Low |

---

## ШАГ 4: Обновление документации

### 4.1 — Создай файл подфункций

Создай `docs/architecture/[module]/functions/[ID]-[name]/subfunctions.md`:

```markdown
# [FUNCTION_ID] — [FUNCTION_NAME]: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | [ID]: [Name] |
| Файл(ы) | [file:lines] |
| LOC | [total] |
| Подфункций | [N] |
| Сложность (max) | [High/Medium/Low] |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| [ID].1 | ... | L-L | ... | ... | ... | ... | ... |

### Frontend (если есть)

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| [ID].1 | ... | L-L | ... | ... | ... | ... |

## Data Flow

[Текстовая диаграмма из Шага 2]

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| ... | ... | ... | ... | ... |
```

### 4.2 — Обнови functions.md

В `docs/architecture/[module]/functions.md` добавь колонку `Subfuncs`:

```markdown
| # | Функция | ... | Cmplx | Subfuncs | Audit |
|---|---------|-----|-------|----------|-------|
| B1 | Chat с AI | ... | Complex | 6 | ⬜ |
| B2 | Память | ... | Medium | 4 | ⬜ |
| B3 | Settings | ... | Simple | — | ⬜ |
```

`—` для Simple функций (Phase 3 не применялся).

### 4.3 — Обнови COVERAGE.md

В счётчик модуля добавь подфункции:

```markdown
| Модуль | Элементов | LOC | Функций | Подфункций | Phase 3 |
|--------|-----------|-----|---------|------------|---------|
| chat | 5 | 2000 | B3+I1+F2=6 | 24 | ✅ done |
```

---

## ПРАВИЛА

1. **Читай перед декомпозицией.** Прочитай весь код функции прежде чем выделять подфункции.
2. **Точные строки.** Каждая подфункция = конкретный диапазон `L100-L150`.
3. **Без перекрытий и пробелов.** Подфункции покрывают ВСЕ строки функции: B1.1 [L100-L115] + B1.2 [L116-L150] + ... = весь диапазон. Ничего не пропущено, ничего не дублируется.
4. **Input → Output для каждой.** Что входит, что выходит. Без этого подфункция не определена.
5. **Side effects явно.** DB read/write, file I/O, network, shell — всё перечислено.
6. **Не дроби тривиальное.** Если функция <50 LOC → пропускай Phase 3 целиком.
7. **Не объединяй разное.** "Валидация + запрос к БД + форматирование" — это 3 подфункции, не одна.
8. **Data flow обязателен** для Complex функций. Для Medium — опционально (по полезности).
9. **Проблемные зоны — честно.** Если подфункция >60 LOC или слишком сложная — отметь.

---

## ВЫХОДНЫЕ ФАЙЛЫ

```
docs/architecture/[module]/
├── functions.md                          — обновлён (колонка Subfuncs)
└── functions/[ID]-[name]/
    └── subfunctions.md                   — подфункции + data flow + проблемы
```

Обновлённые:
```
docs/architecture/COVERAGE.md             — колонка Подфункций
```

---

## ПОСЛЕ ЗАВЕРШЕНИЯ

Покажи:

1. **Список подфункций** (компактно: ID, имя, тип, сложность)
2. **Data Flow** (текстовая диаграмма)
3. **Счётчик:** `[FUNCTION_ID]: [N] подфункций, [LOC] строк, max сложность: [X]`
4. **Проблемные зоны:** сколько штук, самая критичная
5. **Покрытие строк:** `сумма LOC подфункций = LOC функции` ✅/❌

Спроси:
- "Подфункции выделены правильно? Хочешь объединить/разбить?"
- "Переходим к Phase 3 для функции [NEXT] или к аудиту [CURRENT]?"
