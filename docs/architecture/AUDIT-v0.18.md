# Hanni Core — Полный аудит (v0.18, 5 перспектив)

**Дата:** 2026-02-23
**Scope:** M01-M06 + M24 (AI Core)
**Агенты:** Security, Architecture, Performance, QA, Product

## Масштаб

| Перспектива | Находок | Критичных |
|-------------|---------|-----------|
| Security | 14 | 4 HIGH |
| Architecture | 16 | 6 HIGH |
| Performance | 16 | 6 HIGH |
| QA (Bugs) | 20 | 4 CRASH/DATA_LOSS |
| Product (Gaps) | 29 | 4 критичных |
| **Итого** | **~95** | **~24** |

---

## Sprint 0 — Quick Fixes (1 день)

### 1. UTF-8 panic в TTS (CRASH)
- **Файл:** lib.rs:8607, 8642
- **Баг:** `&text[..MAX_TTS_TEXT_LEN]` крашит на русском (mid-char boundary)
- **Фикс:** `&text[..text.floor_char_boundary(MAX_TTS_TEXT_LEN)]`

### 2. AppleScript injection (3 места)
- **send_notification** lib.rs:2776 — экранировать `\` перед `"`
- **start_focus apps** lib.rs:2624 — фильтровать `[^a-zA-Z0-9 .]` из имен приложений
- **start_focus sites** lib.rs:2606 — фильтровать `[^a-zA-Z0-9.-]` из имен сайтов

### 3. response.unwrap() (panic)
- **Файл:** lib.rs:3898
- **Фикс:** `.ok_or("MLX: all retries exhausted")?`

### 4. SQL индексы (0 индексов на 43 таблицы!)
- events(date), transactions(date,category), food_log(date), health_log(date)
- media_items(media_type,status), tasks(project_id,status)
- proactive_history(sent_at), facts(category), conversations(started_at DESC)

---

## Sprint 1 — Chat UX (2-3 дня)

### G01: Markdown + code highlighting
- Подключить marked.js + highlight.js
- Заменить textContent на rendered markdown в addMsg/streamChat
- ~100 строк

### G02: Regenerate response
- Кнопка на каждом ответе бота
- Удалить последний ответ из history, вызвать send() заново

### G03: Copy message button
- Кнопка copy рядом с TTS и feedback

### G04: Edit user messages
- Click-to-edit, re-send с обрезанной историей

---

## Sprint 2 — Stability (2-3 дня)

### process_conversation_end без LLM семафора
- lib.rs:4676 — два LLM-запроса одновременно на MLX
- Фикс: обернуть в LlmBusy semaphore

### Race: proactive + chat
- main.js:198 — proactive msg мутит history во время streaming
- Фикс: `if (busy) return;` в proactive handler

### Skip embed/rerank в CHAT_CALL
- lib.rs:3714 — 150-500ms экономии на каждый голосовой запрос
- Фикс: в call_mode использовать только FTS5

### Извлечь agenticLoop()
- main.js: send() и handleCallTranscript() дублируют 170 строк
- Фикс: `async function agenticLoop({ callMode, onToolResult })`

### scrollDown() дебаунс
- main.js:1732 — 20-40 reflow/sec
- Фикс: requestAnimationFrame throttle

### speak_text fire-and-forget
- lib.rs:8640 — spawn_blocking без await = параллельные аудио
- Фикс: добавить .await или глобальный guard

---

## Sprint 3 — Security (1-2 дня)

### FTS5 MATCH injection
- lib.rs:4497 — спецсимволы ломают поиск
- Фикс: оборачивать слова в двойные кавычки

### Path traversal в read_file/list_dir
- lib.rs:4056 — LLM может прочитать любой файл
- Фикс: ограничить $HOME

### Prompt injection mitigation
- Помечать внешние данные как [EXTERNAL DATA]
- Подтверждение для open_url, set_clipboard

---

## Sprint 4 — Voice (3-5 дней)

- Multi-language STT (убрать хардкод language="ru")
- Wake word research (openWakeWord)
- Proactive cancellation при user chat
- TTS duration sync (вместо wordCount * 300)

---

## Product Gaps (приоритизированные)

### Tier 1 — Критично
1. Markdown + code highlighting (S)
2. Regenerate response (S)
3. Copy message (S)
4. Edit messages (M)

### Tier 2 — Высокий приоритет
5. Wake word (L)
6. App control: open/close/switch (S)
7. Music control: play/pause/next (S)
8. Timers & reminders (M)
9. Memory extraction notification (S)
10. Multi-language STT (S)

### Tier 3 — Средний
11. In-chat search (S)
12. Vision / image analysis (L)
13. Dangerous action confirmation (M)
14. Better web search (M)
15. Memory decay / TTL (M)

### Уникальные преимущества Hanni (сохранять!)
- 100% локальный (никаких данных в облако)
- Persistent semantic memory (embed + FTS5 + Mem0 dedup)
- 30+ action types с agent loop до 5 шагов
- Context-aware proactive messaging (14 стилей)
- Bidirectional voice с VAD + barge-in
- Full training data export для fine-tuning
