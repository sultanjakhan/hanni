# Changelog v0.18.0 — Response Quality Improvements

> 5 улучшений качества ответов. Приоритет: open-source, минимум кастомного кода.

## Изменённые файлы

| Файл | +/- строк | Модули |
|------|-----------|--------|
| desktop/src-tauri/src/lib.rs | +439 / -20 | core, memory, chat, ml_devops |
| desktop/voice_server.py | +50 | voice |
| desktop/src/main.js | +42 / -4 | chat (UI), ml_devops (UI) |
| desktop/finetune.py | +95 (новый) | ml_devops |

## A. System Prompt Refinement

| Элемент | Файл | Строки | Документация |
|---------|------|--------|-------------|
| SYSTEM_PROMPT — RESPONSE QUALITY rules | lib.rs | :18 | [B1-system-prompt/v0.18.0-changes.md](core/functions/B1-system-prompt/v0.18.0-changes.md) |
| SYSTEM_PROMPT — EXAMPLES OF GOOD RESPONSES | lib.rs | :18 | ↑ |
| SYSTEM_PROMPT_LITE — +3 правила | lib.rs | :33 | ↑ |
| is_complex_query() | lib.rs | :74 | ↑ |
| Complex hint в chat_inner | lib.rs | :3695 | ↑ |

## B. FlashRank Reranking

| Элемент | Файл | Строки | Документация |
|---------|------|--------|-------------|
| gather_memory_candidates() | lib.rs | :1508 | [B6-reranking/reranking.md](memory/functions/B6-reranking/reranking.md) |
| rerank_facts() | lib.rs | :1590 | ↑ |
| chat_inner refactoring (gather→rerank→fallback) | lib.rs | :3718 | ↑ |
| POST /rerank endpoint | voice_server.py | :490 | [B5-rerank-endpoint/rerank-endpoint.md](voice/functions/B5-rerank-endpoint/rerank-endpoint.md) |
| ensure_reranker() | voice_server.py | :558 | ↑ |

## C. Conversation Insights

| Элемент | Файл | Строки | Документация |
|---------|------|--------|-------------|
| conversation_insights table | lib.rs | :1269 (init_db) | [B4-fact-extraction/v0.18.0-insights.md](chat/functions/B4-fact-extraction/v0.18.0-insights.md) |
| Extraction prompt — insights section | lib.rs | :4695 | ↑ |
| ExtractedInsight struct | lib.rs | :4796 | ↑ |
| INSERT insights | lib.rs | :5040 | ↑ |
| Inject [Recent decisions & open questions] | lib.rs | :3815 | ↑ |

## D. Self-Critique (Self-Refine)

| Элемент | Файл | Строки | Документация |
|---------|------|--------|-------------|
| quality_check_response() | lib.rs | :3998 | [B5-self-critique/self-critique.md](chat/functions/B5-self-critique/self-critique.md) |
| Self-refine в chat() command | lib.rs | :3579 | ↑ |
| Toggle "Самопроверка" (HTML) | main.js | :902 | ↑ |
| Toggle handler (JS) | main.js | :1035 | ↑ |
| Settings load (enable_self_refine) | main.js | :829 | ↑ |

## E. Fine-tuning Infrastructure

| Элемент | Файл | Строки | Документация |
|---------|------|--------|-------------|
| finetune.py (новый) | desktop/finetune.py | полный | [B2-finetune/finetune.md](ml_devops/functions/B2-finetune/finetune.md) |
| get_adapter_status() | lib.rs | :3026 | ↑ |
| run_finetune() | lib.rs | :3043 | ↑ |
| About UI — Fine-tuning секция | main.js | :3298 | ↑ |
| About UI — кнопка "Запустить fine-tuning" | main.js | :3320 | ↑ |
| invoke_handler registration | lib.rs | :8942 | ↑ |

## Обновлённые индексные файлы

- [memory/functions.md](memory/functions.md) — +B6 (gather_candidates), +B7 (rerank_facts)
- [chat/functions.md](chat/functions.md) — B4 обновлён (insights), +B5 (self-critique)
- [voice/functions.md](voice/functions.md) — +I2 (rerank endpoint)
- [ml_devops/functions.md](ml_devops/functions.md) — +B6 (finetune.py), +B7 (adapter_status), +B8 (run_finetune)

## Зависимости

| Пакет | Команда | Обязательный | Размер |
|-------|---------|-------------|--------|
| flashrank | `pip install flashrank` | Нет (graceful degradation) | ~2MB + ~150MB модель |

## Тестирование

| # | Что | Как |
|---|-----|-----|
| A | System prompt | `cargo check` → OK. Сообщение "устал" → эмпатичный ответ |
| B | Reranking | `curl -X POST http://127.0.0.1:8237/rerank -d '...'` → sorted by score |
| C | Insights | Разговор → завершить → `SELECT * FROM conversation_insights` |
| D | Self-refine | Включить toggle → сложный вопрос → "Уточнение:" если ошибка |
| E | Fine-tuning | Оценить ответы → Экспорт → Запустить → check lora-adapter/ |
| All | Graceful degradation | Остановить voice_server → чат работает через fallback |
