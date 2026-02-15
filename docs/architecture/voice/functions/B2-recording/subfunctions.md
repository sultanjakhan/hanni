# B2 — Запись и транскрипция: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Запись и транскрипция |
| Файл(ы) | lib.rs:L1154-1233 |
| LOC | 80 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B2.1 | Начало записи (start_recording) | L1154-1171 | 18 | State<AudioRecording> → Result<String> | Мутация state (recording=true, clear buffer), запуск capture thread | control | Low |
| B2.2 | Остановка записи и извлечение сэмплов | L1173-1191 | 19 | State<AudioRecording> → samples Vec<f32> | Мутация state (recording=false, clear buffer) | control | Low |
| B2.3 | Инициализация Whisper-контекста | L1198-1208 | 11 | model_path → WhisperContext + State | Загрузка модели в память | external | Low |
| B2.4 | Настройка параметров транскрипции | L1210-1218 | 9 | — → FullParams | — | transform | Trivial |
| B2.5 | Выполнение транскрипции и сборка текста | L1220-1230 | 11 | (State, params, samples) → String | Вычисление (CPU-intensive) | process | Medium |

## Data Flow

```
[Tauri invoke: start_recording]
    │
    ▼
┌─ B2.1 start_recording ────────────────┐
│  Lock state → check !recording         │
│  Set recording=true, clear buffer      │
│  If !capture_running → start_audio_    │ ──→ [B3: start_audio_capture]
│    capture()                            │
└────────────────────────────────────────┘

[Tauri invoke: stop_recording]
    │
    ▼
┌─ B2.2 stop_recording ─────────────────┐
│  Lock state → recording=false          │
│  Check buffer not empty                │
│  Clone samples, clear buffer           │
└──────┬─────────────────────────────────┘
       │ samples: Vec<f32>
       ▼
┌─ B2.3 Инициализация Whisper ───────────┐
│  WhisperContext::new_with_params(path)  │ ──→ [FS: ggml model file]
│  ctx.create_state()                     │
└──────┬─────────────────────────────────┘
       │ (state, ctx)
       ▼
┌─ B2.4 Настройка параметров ────────────┐
│  FullParams::new(Greedy)                │
│  set_language("ru"), set_initial_prompt │
│  set_no_speech_thold(0.6)              │
└──────┬─────────────────────────────────┘
       │ params
       ▼
┌─ B2.5 Транскрипция ───────────────────┐
│  state.full(params, samples)           │
│  for i in 0..segments → collect text   │
└──────┬─────────────────────────────────┘
       │ String (транскрипт)
       ▼
[Return → Frontend]
```

## Проблемные зоны

Нет критичных проблем. Код компактный и линейный.
