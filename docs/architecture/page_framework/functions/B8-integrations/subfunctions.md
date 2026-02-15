# B8 — Integrations info: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B8: Integrations info |
| Файл(ы) | lib.rs:L6151-6345 |
| LOC | 195 |
| Подфункций | 8 |
| Сложность (max) | Medium |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B8.1 | Определение структур IntegrationItem, IntegrationsInfo | L6151-6166 | 16 | — → типы данных | — | data | Trivial |
| B8.2 | Сбор Access-интеграций | L6169-6193 | 25 | — → Vec<IntegrationItem> (Life Tracker, File System, Shell) | FS read: проверка существования tracker_path | query | Low |
| B8.3 | Сбор Tracking-данных | L6195-6229 | 35 | tracker_exists → Vec<IntegrationItem> (Расходы, Время, Цели, Заметки) | FS read: load_tracker_data() | query | Low |
| B8.4 | Загрузка конфигурации блокировщика | L6231-6259 | 29 | — → (apps: Vec<String>, sites: Vec<String>) | FS read: ~/hanni/blocker_config.json | parse | Low |
| B8.5 | Проверка активности блокировщика | L6261-6264 | 4 | — → bool | FS read: /etc/hosts | query | Trivial |
| B8.6 | Формирование списков заблокированных | L6266-6281 | 16 | apps, sites, blocker_active → blocked_apps, blocked_sites (Vec<IntegrationItem>) | — | transform | Trivial |
| B8.7 | Формирование macOS-интеграций + ответ | L6283-6315 | 33 | access, tracking, blocked_*, blocker_active → IntegrationsInfo | — | transform | Low |
| B8.8 | Проверка статуса MLX сервера | L6317-6345 | 29 | — → ModelInfo (model_name, server_url, server_online) | HTTP GET: 127.0.0.1:8234/v1/models | external | Low |

## Data Flow

```
[Input: (нет аргументов)]
    │
    ▼
┌─ B8.2 Access ────────────────┐
│  data_file_path().exists()     │ ←── [FS: life-tracker-data.json]
│  → 3 IntegrationItem           │
└──────┬────────────────────────┘
       │ access
       ▼
┌─ B8.3 Tracking ──────────────┐
│  load_tracker_data()           │ ←── [FS: life-tracker-data.json]
│  → purchases/time/goals/notes  │
│    counts → IntegrationItem[]  │
└──────┬────────────────────────┘
       │ tracking
       ▼
┌─ B8.4 Blocker config ────────┐
│  read ~/hanni/blocker_config   │ ←── [FS: blocker_config.json]
│  parse JSON → apps[], sites[]  │
│  fallback: default lists       │
└──────┬────────────────────────┘
       │ (apps, sites)
       ▼
┌─ B8.5 Blocker active? ───────┐
│  read /etc/hosts               │ ←── [FS: /etc/hosts]
│  contains "HANNI FOCUS BLOCKER"│
└──────┬────────────────────────┘
       │ blocker_active: bool
       ▼
┌─ B8.6 Blocked lists ─────────┐
│  map apps → IntegrationItem[]  │
│  filter www. sites             │
│  map sites → IntegrationItem[] │
└──────┬────────────────────────┘
       │ blocked_apps, blocked_sites
       ▼
┌─ B8.7 macOS + Ответ ─────────┐
│  hardcoded: Screen Time,       │
│    Календарь, Музыка, Браузер  │
│  → Ok(IntegrationsInfo)        │
└───────────────────────────────┘

[Отдельная команда]
    │
    ▼
┌─ B8.8 get_model_info ────────┐
│  HTTP GET /v1/models           │ ──→ [External: MLX Server]
│  timeout 2s                    │
│  → ModelInfo { name, url,      │
│       online: bool }           │
└───────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B8.4 | Загрузка blocker_config | Дублирование дефолтных списков (3 ветки: parse ok, parse fail, file missing) | Вынести defaults в const, единый fallback | Low |
| B8.3 | Tracking-данные | load_tracker_data() может быть медленным на больших файлах | Кешировать или читать только метаданные | Low |
