# F4 — Отправка сообщения: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F4: Отправка сообщения |
| Файл(ы) | main.js:L1522-1673 |
| LOC | 152 |
| Подфункций | 8 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F4.1 | Валидация и инициализация | L1522-1536 | 15 | input text → busy=true, report activity | validation | Low |
| F4.2 | Подготовка сообщения пользователя | L1538-1549 | 12 | text, attachedFile → userContent, history push | transform | Low |
| F4.3 | Agentic loop: стриминг + парсинг actions | L1551-1606 | 56 | history → streamed reply, action results | control | High |
| F4.4 | Добавление TTS-кнопки к финальному ответу | L1608-1621 | 14 | last bot wrapper → DOM TTS button | render | Trivial |
| F4.5 | Отображение тайминга (TTFT, total, tokens) | L1623-1631 | 9 | t0, firstToken, totalTokens → timing div | render | Trivial |
| F4.6 | Инкрементальное сохранение разговора | L1633-1640 | 8 | history, currentConversationId → saved conversation | persist | Low |
| F4.7 | Добавление feedback-кнопок + извлечение фактов | L1641-1667 | 27 | wrappers[], history → feedback buttons, facts extracted | interaction | Medium |
| F4.8 | Сброс UI-состояния | L1670-1673 | 4 | — → busy=false, sendBtn enabled, input focus | control | Trivial |

## Data Flow

```
[Input: user types text + Enter]
    │
    ▼
┌─ F4.1 Validate + Init ─────┐
│  trim, check !empty & !busy  │
│  busy=true, disable send     │
│  report_user_chat_activity   │ ──→ [Tauri: report_user_chat_activity]
│  check proactive engagement  │ ──→ [Tauri: report_proactive_engagement]
└──────┬──────────────────────┘
       │ text
       ▼
┌─ F4.2 Build User Message ──┐
│  append file if attached     │
│  addMsg('user', text)        │ ──→ [DOM: user message]
│  history.push(['user', ...]) │
└──────┬──────────────────────┘
       │ history
       ▼
┌─ F4.3 Agentic Loop ────────┐  ←── MAX_ITERATIONS = 5
│  iteration++                 │
│  create bot wrapper div      │ ──→ [DOM: bot message]
│  streamChat(botDiv, t0)      │ ──→ [Tauri: chat] ──→ [MLX]
│  parseAndExecuteActions()    │
│  if no actions → break       │
│  executeAction() per action  │ ──→ [Tauri: various commands]
│  show action results         │ ──→ [DOM: action-result divs]
│  feed results back to history│
│  loop ───────────────────────│
└──────┬──────────────────────┘
       │ final reply
       ▼
┌─ F4.4 TTS Button ──────────┐
│  add play button to wrapper  │ ──→ [DOM: tts-btn]
└──────┬──────────────────────┘
       │
       ▼
┌─ F4.5 Timing Display ──────┐
│  calculate TTFT, total time  │
│  show tokens + steps count   │ ──→ [DOM: timing div]
└──────┬──────────────────────┘
       │
       ▼
┌─ F4.6 Save Conversation ───┐
│  update or create in DB      │ ──→ [Tauri: save/update_conversation]
└──────┬──────────────────────┘
       │
       ▼
┌─ F4.7 Feedback + Facts ────┐
│  addFeedbackButtons to all   │ ──→ [DOM: feedback buttons]
│  process_conversation_end    │ ──→ [Tauri: process_conversation_end]
│  loadConversationsList()     │ ──→ [DOM: sidebar refresh]
└──────┬──────────────────────┘
       │
       ▼
┌─ F4.8 Reset UI ────────────┐
│  busy=false                  │
│  sendBtn.disabled=false      │
│  input.focus()               │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F4.3 | Agentic Loop | 56 LOC, содержит стриминг + парсинг + исполнение + рекурсию — слишком много ответственности | Вынести тело цикла в отдельную функцию agentStep() | Medium |
| F4.7 | Feedback + Facts | Маппинг wrapper index → history index через двойной перебор — хрупкий алгоритм | Хранить data-атрибут с history index на каждом wrapper | Medium |
| F4.6-F4.7 | Post-chat block | Выполняется в IIFE (() => {})() без await — ошибки молча глотаются | Добавить хотя бы console.error в catch | Low |
