# Hanni AI Core — План улучшений v0.18

**Дата:** 2026-02-23
**Scope:** M01-M06 + M15 (AI Core модули)
**Статус:** Wave 3 Complete (ALL DONE)

---

## M01: Core (5 функций)

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| C1 | Адаптивный system prompt (имя из facts, русские заголовки) | S | ✅ |
| C2 | Prompt versioning + A/B тестирование | M | ⬜ |
| C3 | DB миграции как отдельная система | M | ⬜ |
| C4 | Health monitor (MLX, voice_server, DB integrity, stats) | S | ✅ |
| C5 | Adaptive model params (temperature по типу запроса) | S | ✅ |

## M02: Chat (11 функций)

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| CH1 | Markdown + code highlighting (marked.js + highlight.js) | S | ✅ |
| CH2 | Regenerate response | S | ✅ |
| CH3 | Copy message button | S | ✅ |
| CH4 | Edit user message + resend | M | ✅ |
| CH5 | Адаптивная история (лимит по токенам, не по сообщениям) | S | ⬜ |
| CH6 | Implicit feedback loop (retry → training data) | M | ⬜ |
| CH7 | Streaming action indicator ("Выполняю: web_search...") | S | ✅ |
| CH8 | Auto-categorize conversations (8 категорий) | S | ✅ |
| CH9 | Smart context injection (по всей истории, не только последнее сообщение) | M | ✅ |

## M03: Memory (10 функций)

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| ME1 | Memory decay (TTL / exponential decay) | M | ✅ |
| ME2 | Уведомление о запомненных фактах | S | ✅ |
| ME3 | Confidence scoring в UI | S | ⬜ |
| ME4 | Bulk operations ("забудь всё о X") | S | ⬜ |
| ME5 | Adaptive extraction (обучение на удалениях) | M | ⬜ |
| ME6 | Cross-conversation linking | M | ⬜ |
| ME7 | Proactive memory conflict detection | S | ✅ |
| ME8 | Auto-synthesis user profile при изменении фактов | S | ✅ |

## M04: Voice (12 функций)

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| V1 | Multi-language STT (auto-detect / настройки) | S | ✅ |
| V2 | Wake word (openWakeWord / Porcupine) | L | ✅ |
| V3 | Sentence-level streaming TTS | M | ✅ |
| V4 | Voice activity indicator (waveform) | S | ⬜ |
| V5 | Adaptive speech rate | S | ⬜ |
| V6 | TTS completion event (убрать setTimeout хак) | M | ✅ |
| V7 | Noise cancellation (rnnoise) | M | ⬜ |
| V8 | Voice cloning (Qwen3-TTS) | L | ✅ |

## M05: Proactive (10 функций)

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| P1 | Proactive actions (```action blocks in proactive) | M | ✅ |
| P2 | Event-driven triggers (WiFi, батарея, встреча) | M | ⬜ |
| P3 | Engagement-adaptive frequency | S | ⬜ |
| P4 | Proactive cancellation при user chat | M | ✅ |
| P5 | Context caching (30с) | S | ⬜ |
| P6 | Smart timing (паттерны активности) | M | ⬜ |
| P7 | Morning briefing v2 (погода, валюты, напоминания) | M | ⬜ |

## M06: System (8 функций)

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| S1 | App control (open/close/switch) | S | ✅ |
| S2 | Music control (play/pause/next) | S | ✅ |
| S3 | Timers & reminders (scheduler + push) | M | ✅ |
| S4 | Better web search (full page fetch) | M | ⬜ |
| S5 | Dangerous action confirmation (UI modal) | M | ✅ |
| S6 | Brightness / Night Shift / DND control | S | ⬜ |
| S7 | Screenshot + analysis | M | ⬜ |

## M15: ML/DevOps — Обучение и адаптивность (8 функций)

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| ML1 | Continuous feedback → training dataset | S | ✅ |
| ML2 | Auto-evaluation pipeline (50 test questions) | M | ⬜ |
| ML3 | Prompt A/B testing | M | ⬜ |
| ML4 | Adaptive tool selection (track correct/incorrect) | M | ⬜ |
| ML5 | Conversation quality scoring | S | ⬜ |
| ML6 | Fine-tune scheduler UI (кнопка + прогресс) | M | ✅ |
| ML7 | Data flywheel (замкнутый цикл обучения) | L | ✅ |
| ML8 | Style adaptation (краткость ответов по стилю пользователя) | S | ✅ |

---

## Порядок реализации

### Wave 1 — Quick Wins (S items, максимальный ROI) ✅
CH1, CH2, CH3, S1, S2, V1, ME2, C5, ML1, ML8

### Wave 2 — Medium (M items, высокий impact) ✅
CH4, CH9, ME1, V3, V6, P1, P4, S3, S5, ML6

### Wave 3 — Large (L items, трансформирующие) ✅
V2, V8, ML7

---

## Changelog

- 2026-02-23: Создан план, Sprint 0 завершён (UTF-8 fix, AppleScript sanitization, unwrap fix, 16 SQL indexes)
- 2026-02-23: Wave 1 завершён — 10 items (CH1-3, S1-2, V1, ME2, C5, ML1, ML8)
- 2026-02-23: Wave 2 завершён — 10 items (CH4, CH9, ME1, V3, V6, P1, P4, S3, S5, ML6)
- 2026-02-24: Wave 3 завершён — 3 items (V2: Wake word VAD+Whisper, V8: Voice clone Qwen3-TTS, ML7: Data flywheel)
