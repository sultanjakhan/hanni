# B5 — Call Mode аудио-луп (VAD): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B5: Call Mode аудио-луп (VAD) |
| Файл(ы) | lib.rs:L1488-1803 |
| LOC | 316 |
| Подфункций | 10 |
| Сложность (max) | High |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B5.1 | Инициализация устройства и конфигурации | L1488-1537 | 50 | (Arc<CallMode>, AppHandle) → (Device, config, ratio, channels) | emit("idle") при ошибке | capture | Medium |
| B5.2 | Создание ring buffer и input-стрима | L1539-1572 | 34 | (Device, config, ratio, channels) → Stream | Audio callback: downmix + resample → chunk_buf | capture | Medium |
| B5.3 | Запуск стрима и валидация | L1574-1586 | 13 | Stream → playing stream | stream.play(), emit("idle") при ошибке | control | Trivial |
| B5.4 | Инициализация VAD (Silero / fallback energy) | L1588-1600 | 13 | — → Option<VoiceActivityDetector> | Загрузка Silero модели | external | Low |
| B5.5 | Основной цикл: проверка active + drain buffer | L1602-1626 | 25 | chunk_buf → process_buf | Чтение state.active, drain ring buffer | control | Low |
| B5.6 | Вычисление RMS + адаптивный noise floor | L1628-1642 | 15 | chunk 512 samples → (rms, noise_gate) | Обновление noise_floor | process | Low |
| B5.7 | Обработка тишины (ниже noise gate) | L1644-1696 | 53 | (rms < noise_gate, phase) → state transition | Мутация state, emit events, spawn transcribe thread | control | High |
| B5.8 | VAD предсказание (Silero / energy fallback) | L1698-1702 | 5 | chunk → prob (f32) | Вызов VAD модели | process | Trivial |
| B5.9 | State machine: listening + recording фазы | L1704-1775 | 72 | (prob, phase, chunk) → state transition | Мутация state, emit events, spawn transcribe thread | control | High |
| B5.10 | Barge-in детекция (speaking фаза) | L1776-1797 | 22 | (prob, rms, noise_floor) → barge_in flag | Мутация state.barge_in | process | Low |

## Data Flow

```
[start_call_audio_loop(Arc<CallMode>, AppHandle)]
    │
    ▼ (новый thread)
┌─ B5.1 Инициализация устройства ────────────────┐
│  cpal::default_host() → default_input_device()  │ ──→ [Hardware: микрофон]
│  Try 16kHz → fallback device default             │
│  Вычисление resample_ratio, num_channels         │
└──────┬──────────────────────────────────────────┘
       │ (device, config, ratio, channels)
       ▼
┌─ B5.2 Ring buffer + input stream ──────────────┐
│  Arc<Mutex<Vec<f32>>> chunk_buf                 │
│  device.build_input_stream(callback:            │
│    downmix → resample → chunk_buf.extend())     │ ──→ [Shared: chunk_buf]
└──────┬──────────────────────────────────────────┘
       │ Stream
       ▼
┌─ B5.3 Запуск стрима ──────────────────────────┐
│  stream.play()                                  │
└──────┬──────────────────────────────────────────┘
       │
       ▼
┌─ B5.4 Инициализация VAD ─────────────────────┐
│  VoiceActivityDetector::builder()              │
│    .sample_rate(16000).chunk_size(512).build() │
│  Fallback: energy-based (rms * 50)             │
└──────┬──────────────────────────────────────────┘
       │ vad_opt
       ▼
┌═══════════════════════════════════════════════════════════┐
║  MAIN LOOP (16ms tick)                                    ║
║                                                           ║
║  ┌─ B5.5 Check active + drain ───────────────────┐       ║
║  │  if !state.active → break                      │       ║
║  │  chunk_buf.drain(..) → process_buf             │       ║
║  └──────┬─────────────────────────────────────────┘       ║
║         │                                                  ║
║         ▼ (per 512-sample chunk)                           ║
║  ┌─ B5.6 RMS + noise floor ──────────────────────┐       ║
║  │  energy = sum(s*s)/len → rms = sqrt(energy)    │       ║
║  │  if listening && rms < floor*3:                 │       ║
║  │    noise_floor = EMA(noise_floor, rms)          │       ║
║  │  noise_gate = max(floor*2, 0.003)               │       ║
║  └──────┬─────────────────────────────────────────┘       ║
║         │ (rms, noise_gate)                                ║
║         ▼                                                  ║
║  ┌─ B5.7 Тишина (rms < gate) ───────────────────┐       ║
║  │  listening: reset speech_frames, clear buffer  │       ║
║  │  recording + silence>=15:                      │       ║
║  │    → phase="processing"                        │       ║
║  │    → spawn transcribe thread ─────────────────│──→ [B2: transcribe_samples]
║  │    → emit("processing") / emit("transcript")  │──→ [Event → Frontend]
║  │  continue (skip VAD predict)                   │       ║
║  └──────┬─────────────────────────────────────────┘       ║
║         │ (rms >= gate)                                    ║
║         ▼                                                  ║
║  ┌─ B5.8 VAD predict ───────────────────────────┐       ║
║  │  Silero: vad.predict(chunk) → prob             │       ║
║  │  Fallback: (rms * 50).min(1.0) → prob          │       ║
║  └──────┬─────────────────────────────────────────┘       ║
║         │ prob                                             ║
║         ▼                                                  ║
║  ┌─ B5.9 State machine (listening/recording) ────┐       ║
║  │  listening + prob>0.5:                         │       ║
║  │    speech_frames++ → if >=5: phase="recording" │──→ [Event: recording]
║  │  listening + prob<=0.5:                        │       ║
║  │    reset frames, clear buffer                  │       ║
║  │  recording + prob<0.5:                         │       ║
║  │    silence_frames++ → if >=15: transcribe      │──→ [B2: transcribe_samples]
║  │  recording + prob>=0.5:                        │       ║
║  │    silence_frames=0, buffer audio              │       ║
║  └──────┬─────────────────────────────────────────┘       ║
║         │                                                  ║
║         ▼                                                  ║
║  ┌─ B5.10 Barge-in (speaking) ──────────────────┐       ║
║  │  thresh = max(noise_floor*8, 0.04)             │       ║
║  │  prob>0.8 && rms>thresh → speech_frames++      │       ║
║  │  if >=6 → barge_in = true                      │──→ [State: barge_in]
║  │  prob<0.4 → reset frames                       │       ║
║  └────────────────────────────────────────────────┘       ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝
       │ (state.active = false)
       ▼
[drop(stream) — thread exit]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B5.7 | Обработка тишины | 53 LOC, дублирует логику транскрипции из B5.9 (recording+silence>=15) | Вынести общий transition_to_processing() helper | Medium |
| B5.9 | State machine | 72 LOC, содержит spawn transcribe thread — дублирует B5.7 | Объединить transition логику в единую fn | Medium |
| B5.1+B5.2 | Инициализация устройства и стрима | Дублирование кода из B3 (audio capture) — тот же downmix + resample | Вынести в shared helper (build_input_device, resample_callback) | High |
| — | Весь цикл | 316 LOC в одной fn, сложная state machine | Декомпозировать на отдельные fn per-phase | Low |
