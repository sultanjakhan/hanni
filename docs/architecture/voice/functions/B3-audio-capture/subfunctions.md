# B3 — Захват аудио (CPAL): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B3: Захват аудио (CPAL) |
| Файл(ы) | lib.rs:L1234-1371 |
| LOC | 138 |
| Подфункций | 6 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.1 | Получение input-устройства | L1234-1250 | 17 | Arc<AudioRecording> → Device | Мутация state при ошибке (capture_running=false) | capture | Low |
| B3.2 | Выбор конфигурации (16kHz или fallback) | L1252-1292 | 41 | Device → (StreamConfig, resample_ratio) | Тестовый build_input_stream, мутация state при ошибке | validation | Medium |
| B3.3 | Построение input-стрима с callback | L1294-1330 | 37 | (Device, config, state_clone) → BuildStreamResult | Audio callback: downmix + resample → state.audio_buffer | capture | Medium |
| B3.4 | Запуск стрима (play) | L1332-1342 | 11 | Stream → playing stream | stream.play(), мутация state при ошибке | control | Trivial |
| B3.5 | Polling-луп (ожидание окончания записи) | L1343-1351 | 9 | running stream → — | sleep(100ms), чтение state.recording | control | Trivial |
| B3.6 | Завершение и очистка | L1352-1367 | 16 | — → — | Мутация state (capture_running=false), drop stream при ошибке | control | Trivial |

## Data Flow

```
[start_audio_capture(Arc<AudioRecording>)]
    │
    ▼ (новый thread)
┌─ B3.1 Получение устройства ────────────┐
│  cpal::default_host()                   │
│  host.default_input_device()            │ ──→ [Hardware: микрофон]
│  Ошибка → state.capture_running=false   │
└──────┬─────────────────────────────────┘
       │ Device
       ▼
┌─ B3.2 Выбор конфигурации ─────────────┐
│  Попытка 16kHz/1ch (Whisper native)    │
│  Fallback: device.default_input_config │
│  Вычисление resample_ratio             │
│  Ошибка → state.capture_running=false  │
└──────┬─────────────────────────────────┘
       │ (config, ratio)
       ▼
┌─ B3.3 Построение input-стрима ─────────┐
│  device.build_input_stream(config,      │
│    callback: |data| {                   │
│      if recording:                      │
│        if 1ch + 16kHz → direct copy     │
│        else → downmix + resample        │
│        → state.audio_buffer.extend()    │ ──→ [State: audio_buffer]
│    })                                   │
└──────┬─────────────────────────────────┘
       │ Stream
       ▼
┌─ B3.4 Запуск стрима ──────────────────┐
│  stream.play()                         │
│  Ошибка → state reset, return          │
└──────┬─────────────────────────────────┘
       │ playing
       ▼
┌─ B3.5 Polling-луп ────────────────────┐
│  loop {                                │
│    sleep(100ms)                        │
│    if !state.recording → break         │
│  }                                     │
└──────┬─────────────────────────────────┘
       │ recording=false
       ▼
┌─ B3.6 Завершение ─────────────────────┐
│  state.capture_running = false          │
│  (stream dropped автоматически)         │
└────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B3.2 | Выбор конфигурации | 41 LOC — вложенные match с дублированием логики тестового стрима | Вынести логику выбора конфигурации в отдельную fn | Low |
| B3.3 | Построение стрима | Callback дублирует downmix+resample логику из B5 (call audio loop) | Вынести resample в shared helper | Medium |
