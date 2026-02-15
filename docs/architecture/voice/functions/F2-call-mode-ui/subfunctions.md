# F2 — UI режима звонка: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F2: UI режима звонка |
| Файл(ы) | main.js:L5059-5376 |
| LOC | 318 |
| Подфункций | 10 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.1 | Инициализация состояния и DOM-ссылок | L5059-5073 | 15 | module load → переменные + PHASE_LABELS | render | Trivial |
| F2.2 | toggleCallMode — переключатель | L5075-5081 | 7 | click callBtn → startCallMode / endCallMode | control | Trivial |
| F2.3 | startCallMode — проверка Whisper + старт | L5083-5138 | 56 | toggle → check model → download if needed → invoke start_call_mode → UI setup | interaction | Medium |
| F2.4 | endCallMode — завершение звонка | L5140-5155 | 16 | toggle/escape → UI reset → invoke stop_call_mode → re-enable input | interaction | Low |
| F2.5 | Слушатель фазовых изменений | L5157-5163 | 7 | event call-phase-changed → update overlay data-phase + label | stream | Trivial |
| F2.6 | Обработка транскрипта: UI + LLM agentic loop | L5165-5224 | 60 | event call-transcript → show user bubble → streamChat → parseActions → execute | process | High |
| F2.7 | Отображение ответа в overlay | L5226-5241 | 16 | lastReply → strip actions/think → create bot bubble in overlay | render | Trivial |
| F2.8 | Сохранение разговора (async) | L5243-5256 | 14 | after LLM loop → save/update conversation + process_conversation_end | persist | Low |
| F2.9 | speakAndListen — streaming TTS с barge-in | L5274-5348 | 75 | lastReply → set speaking phase → split sentences → TTS per sentence → poll barge-in → resume listening | external | High |
| F2.10 | Горячие клавиши и event bindings | L5350-5376 | 27 | click/global-toggle-call/Escape → toggleCallMode/endCallMode | interaction | Low |

## Data Flow

```
[Click: callBtn / Cmd+Shift+H / Escape]
    │
    ▼
┌─ F2.2 toggleCallMode ─────────────────┐
│  if active → endCallMode()             │
│  else → startCallMode()                │
└──────┬─────────────────────────────────┘
       │
       ├─ start ──┐
       │           ▼
       │  ┌─ F2.3 startCallMode ──────────────────────┐
       │  │  Check whisper model (invoke)               │ ──→ [B1.6]
       │  │  If missing → confirm → download            │ ──→ [B1: download]
       │  │  callModeActive = true                      │
       │  │  Show overlay, set phase=listening           │ ──→ [DOM: overlay]
       │  │  autoSaveConversation(), reset history       │ ──→ [DB: conversations]
       │  │  Disable normal input                        │ ──→ [DOM: input/send/record]
       │  │  invoke('start_call_mode')                   │ ──→ [B4.1]
       │  └────────────────────────────────────────────┘
       │
       └─ stop ───┐
                   ▼
          ┌─ F2.4 endCallMode ────────────────────────┐
          │  callModeActive = false                     │
          │  Hide overlay, remove .active               │ ──→ [DOM]
          │  Re-enable input/send/record                │ ──→ [DOM]
          │  invoke('stop_call_mode')                   │ ──→ [B4.2]
          └────────────────────────────────────────────┘

[Event: call-phase-changed]
    │
    ▼
┌─ F2.5 Фазовый слушатель ─────────────┐
│  overlay.data-phase = phase            │ ──→ [DOM: CSS animation]
│  callPhaseText = PHASE_LABELS[phase]   │
└────────────────────────────────────────┘

[Event: call-transcript]
    │
    ▼
┌─ F2.6 Обработка транскрипта ─────────────────────┐
│  Show user bubble in overlay                       │ ──→ [DOM: call-transcript-area]
│  addMsg('user', text) + history.push               │ ──→ [DOM: chat, State: history]
│  Set phase=processing                              │
│  Agentic loop (max 5 iterations):                  │
│    streamChat() → fullReply                        │ ──→ [Backend: LLM streaming]
│    parseAndExecuteActions()                         │ ──→ [Backend: action execution]
│    Show action results                             │ ──→ [DOM: action-result divs]
│  lastReply = final response                        │
└──────┬────────────────────────────────────────────┘
       │ lastReply
       ▼
┌─ F2.7 Отображение ответа ────────────────────────┐
│  Strip ```action + <think> blocks                  │
│  Create bot bubble in overlay                      │ ──→ [DOM: call-transcript-bot]
└──────┬────────────────────────────────────────────┘
       │
       ▼
┌─ F2.8 Сохранение разговора ──────────────────────┐
│  save/update conversation                          │ ──→ [DB: conversations]
│  process_conversation_end (if history>=4)           │ ──→ [Backend: memory extraction]
│  loadConversationsList()                            │ ──→ [DOM: sidebar]
└──────┬────────────────────────────────────────────┘
       │
       ▼
┌─ F2.9 speakAndListen ────────────────────────────┐
│  Set phase=speaking (UI + invoke set_speaking)     │ ──→ [B4.4, DOM]
│  Get voice from proactive settings                 │ ──→ [Backend: get_proactive_settings]
│  Strip action/think blocks for TTS                 │
│  splitIntoSentences(ttsText)                       │
│  Start barge-in polling (150ms interval):           │
│    invoke('call_mode_check_bargein')               │ ──→ [B4.5]
│    if barge → stop_speaking + resume_listening     │ ──→ [B8.2, B4.3]
│  For each sentence:                                │
│    invoke('speak_sentence_blocking', {sentence})   │ ──→ [B7.8]
│    Check barge-in after each sentence              │
│  clearInterval(bargeInterval)                       │
│  if !bargedIn → resume_listening                   │ ──→ [B4.3]
└──────────────────────────────────────────────────┘

[F2.10: Global shortcut + Escape + button click listeners]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F2.6 | Agentic loop | 60 LOC, дублирует логику send() из chat module | Вынести общий agentic loop в shared function | High |
| F2.9 | speakAndListen | 75 LOC, сложная логика barge-in polling + sequential TTS | Разбить на отдельные fn: startBargeInPolling(), speakSentences() | Medium |
| F2.3 | startCallMode | 56 LOC, дублирует Whisper check из F1.2 | Вынести ensureWhisperModel() shared helper | Low |
