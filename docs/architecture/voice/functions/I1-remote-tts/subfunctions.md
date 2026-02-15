# I1 — Remote TTS сервер (PC): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | I1: Remote TTS сервер (PC) |
| Файл(ы) | pc/tts_server.py:L1-120 |
| LOC | 120 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| I1.1 | Загрузка модели (Qwen3-TTS / pipeline / edge-tts fallback) | L36-57 | 22 | MODEL_NAME env → model object | Загрузка модели в GPU/CPU память | external | Medium |
| I1.2 | Валидация запроса и роутинг | L60-68 | 9 | POST /tts JSON → (text, voice) + routing | — | validation | Trivial |
| I1.3 | Генерация через edge-tts (fallback) | L69-87 | 19 | (text, voice) → audio/mp3 BytesIO | asyncio.run edge_tts.Communicate | external | Low |
| I1.4 | Генерация через модель (Qwen3-TTS / pipeline) | L88-105 | 18 | (text, voice) → audio/wav BytesIO | model.generate_speech / pipeline inference | external | Medium |
| I1.5 | Health endpoint | L108-115 | 8 | GET /health → JSON status | — | query | Trivial |

## Data Flow

```
[Startup]
    │
    ▼
┌─ I1.1 Загрузка модели ────────────────────────┐
│  Try: qwen3_tts.Qwen3TTS.from_pretrained()    │ ──→ [GPU: CUDA / CPU]
│  Fallback: transformers.pipeline("text-to-     │
│    speech")                                     │
│  Fallback: USE_EDGE_FALLBACK = True             │
└──────┬─────────────────────────────────────────┘
       │ model / USE_EDGE_FALLBACK
       ▼

[POST /tts {"text": "...", "voice": "..."}]
    │
    ▼
┌─ I1.2 Валидация ──────────────────────────────┐
│  data = request.json                            │
│  text = data.get("text") → if empty: 400        │
│  voice = data.get("voice", default)             │
└──────┬─────────────────────────────────────────┘
       │ (text, voice)
       ▼
       ├─ USE_EDGE_FALLBACK=True ──┐
       │                            ▼
       │  ┌─ I1.3 edge-tts ───────────────────┐
       │  │  communicate = Communicate(text,    │
       │  │    voice)                            │
       │  │  async stream → BytesIO              │
       │  │  return send_file(audio/mp3)         │ ──→ [HTTP Response: mp3]
       │  └────────────────────────────────────┘
       │
       └─ model loaded ──┐
                          ▼
          ┌─ I1.4 Модельная генерация ───────────┐
          │  if generate_speech:                   │
          │    audio = model.generate_speech(text) │ ──→ [GPU: inference]
          │    sf.write(buf, audio, 24000, WAV)    │
          │  else (pipeline):                      │
          │    result = model(text)                │ ──→ [GPU: inference]
          │    sf.write(buf, audio, rate, WAV)     │
          │  return send_file(audio/wav)           │ ──→ [HTTP Response: wav]
          └────────────────────────────────────────┘

[GET /health]
    │
    ▼
┌─ I1.5 Health ─────────────────────────────────┐
│  status: ok, model name, cuda, gpu name        │ ──→ [HTTP Response: JSON]
└────────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| I1.1 | Загрузка модели | Нет graceful handling при CUDA OOM | Добавить try/except для torch.cuda.OutOfMemoryError | Low |
| I1.3 | edge-tts fallback | asyncio.run() внутри Flask — блокирует event loop | Использовать loop.run_until_complete() или async framework (FastAPI) | Low |
