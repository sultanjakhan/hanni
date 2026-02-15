# B1 — Управление моделью Whisper: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: Управление моделью Whisper |
| Файл(ы) | lib.rs:L1104-1197 |
| LOC | 94 |
| Подфункций | 5 |
| Сложность (max) | Medium |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.1 | Определение пути модели (приоритет turbo/medium) | L1104-1116 | 13 | — → PathBuf | Чтение FS (exists check) | transform | Trivial |
| B1.2 | Проверка существования модели | L1118-1123 | 6 | model_path → Result | Чтение FS | validation | Trivial |
| B1.3 | Создание директории и инициализация загрузки | L1124-1138 | 15 | AppHandle → (stream, File) | Создание dir, создание tmp-файла, HTTP запрос | external | Low |
| B1.4 | Потоковая загрузка с прогрессом | L1140-1148 | 9 | (stream, File) → downloaded bytes | Запись в tmp-файл, emit progress event | stream | Medium |
| B1.5 | Финализация (rename tmp → final) | L1150-1152 | 3 | (tmp_path, model_path) → Result<String> | Rename файла | persist | Trivial |

### Tauri-команда check_whisper_model

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.6 | Проверка наличия модели (endpoint) | L1193-1197 | 5 | — → Result<bool> | Чтение FS | validation | Trivial |

## Data Flow

```
[Tauri invoke: download_whisper_model]
    │
    ▼
┌─ B1.1 whisper_model_path / whisper_turbo_path ─┐
│  Проверка turbo → medium → default              │
└──────┬──────────────────────────────────────────┘
       │ PathBuf
       ▼
┌─ B1.2 Проверка существования ─┐
│  model_path.exists()           │
│  Если есть → return Ok         │
└──────┬────────────────────────┘
       │ нет модели
       ▼
┌─ B1.3 Инициализация загрузки ─────────┐
│  create_dir_all(parent)                 │
│  reqwest::Client::new().get(url).send() │ ──→ [External: HuggingFace]
│  File::create(tmp_path)                 │ ──→ [FS: .bin.tmp]
└──────┬─────────────────────────────────┘
       │ (stream, file)
       ▼
┌─ B1.4 Потоковая загрузка ──────┐
│  while chunk = stream.next()    │
│  file.write_all(bytes)          │ ──→ [FS: .bin.tmp]
│  app.emit("whisper-download-    │ ──→ [Event: whisper-download-progress]
│    progress", pct)              │
└──────┬─────────────────────────┘
       │ все байты записаны
       ▼
┌─ B1.5 Финализация ─────────┐
│  fs::rename(tmp → model)    │ ──→ [FS: ggml-large-v3-turbo.bin]
└────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B1.3 | Инициализация загрузки | Нет retry при ошибке сети | Добавить retry с backoff для больших файлов | Low |
| B1.4 | Потоковая загрузка | При обрыве соединения tmp-файл остаётся | Очистка tmp при ошибке | Low |
