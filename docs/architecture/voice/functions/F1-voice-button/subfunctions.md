# F1 — Кнопка записи голоса: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F1: Кнопка записи голоса |
| Файл(ы) | main.js:L204-257 |
| LOC | 54 |
| Подфункций | 4 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F1.1 | Остановка записи и вставка текста | L204-218 | 15 | click (isRecording=true) → invoke stop_recording → input.value += text | interaction | Low |
| F1.2 | Проверка наличия модели Whisper | L220-245 | 26 | click (isRecording=false) → invoke check_whisper_model → confirm dialog → download | interaction | Medium |
| F1.3 | Загрузка модели с прогрессом | L226-238 | 13 | confirm=true → listen whisper-download-progress → update DOM | stream | Low |
| F1.4 | Запуск записи | L247-256 | 10 | check passed → invoke start_recording → toggle UI state | interaction | Trivial |

## Data Flow

```
[Click: recordBtn]
    │
    ├─ isRecording=true ──┐
    │                      ▼
    │          ┌─ F1.1 Остановка ──────────────┐
    │          │  invoke('stop_recording')       │ ──→ [B2.2: stop_recording]
    │          │  input.value += transcript      │ ──→ [DOM: input field]
    │          │  toggle UI (remove .recording)  │
    │          └────────────────────────────────┘
    │
    └─ isRecording=false ─┐
                           ▼
               ┌─ F1.2 Проверка модели ──────────┐
               │  invoke('check_whisper_model')    │ ──→ [B1.6: check]
               │  if !hasModel → confirm()         │ ──→ [Browser: dialog]
               └──────┬───────────────────────────┘
                      │ (модель нужна)
                      ▼
               ┌─ F1.3 Загрузка модели ──────────┐
               │  invoke('download_whisper_model') │ ──→ [B1: download]
               │  listen('whisper-download-        │
               │    progress') → update DOM        │ ──→ [DOM: progress %]
               └──────┬───────────────────────────┘
                      │ (модель есть)
                      ▼
               ┌─ F1.4 Запуск записи ───────────┐
               │  invoke('start_recording')       │ ──→ [B2.1: start_recording]
               │  isRecording = true              │
               │  add .recording class            │ ──→ [DOM: button style]
               └──────────────────────────────────┘
```

## Проблемные зоны

Нет критичных проблем. Код линейный, обработка ошибок через addMsg.
