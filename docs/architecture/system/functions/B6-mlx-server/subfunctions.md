# B6 — Управление MLX сервером: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B6: Управление MLX сервером |
| Файл(ы) | lib.rs:L2525-2605 |
| LOC | 81 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B6.1 | Поиск python3 (find_python) | L2525-2538 | 14 | -- -> Option<String> | File: check existence | validation | Trivial |
| B6.2 | Проверка запущен ли сервер | L2540-2557 | 18 | -- -> bool | Network: GET :8234/v1/models | external | Low |
| B6.3 | Проверка LoRA адаптера | L2559-2566 | 8 | -- -> bool | File: check adapters.safetensors | validation | Trivial |
| B6.4 | Формирование аргументов запуска | L2568-2576 | 9 | model, adapter -> Vec<&str> | -- | transform | Trivial |
| B6.5 | Запуск процесса MLX | L2578-2592 | 15 | python, args -> Child | Process: spawn; File: create log | external | Low |
| B6.6 | Обработка результата запуска | L2589-2599 | 11 | Result<Child> -> Option<Child> | Stderr: log | error | Trivial |

## Data Flow

```
[Entry: start_mlx_server()]
    │
    ▼
┌─ B6.1 find_python() ────────┐
│  /opt/homebrew/bin/python3    │ ←── [File: check exists]
│  /usr/local/bin/python3       │
│  /usr/bin/python3             │
└──────┬───────────────────────┘
       │ python: String
       ▼
┌─ B6.2 Server Running? ──┐
│  GET :8234/v1/models      │ ──→ [Network: MLX]
│  timeout: 2s              │
│  running? -> return None  │
└──────┬───────────────────┘
       │ not running
       ▼
┌─ B6.3 Check Adapter ──────────────────────┐
│  hanni_data_dir()/lora-adapter/            │ ←── [File]
│  adapters.safetensors exists?              │
└──────┬────────────────────────────────────┘
       │ has_adapter: bool
       ▼
┌─ B6.4 Build Args ──────────────────────────┐
│  ["-m", "mlx_lm", "server",                │
│   "--model", MODEL, "--port", "8234"]       │
│  + optional: "--adapter-path", adapter_dir  │
└──────┬─────────────────────────────────────┘
       │ args
       ▼
┌─ B6.5-B6.6 Spawn Process ──┐
│  Command::new(python)        │ ──→ [Process: mlx_lm server]
│  stderr -> mlx_server.log    │ ──→ [File: log]
│  Ok(child) -> Some(child)    │
│  Err(e) -> None              │
└──────┬──────────────────────┘
       │
       ▼
[Output: Option<Child>]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B6.2 | Проверка запуска | Использует blocking HTTP клиент в sync контексте | Приемлемо для startup, но можно сделать async | Low |
| B6.1 | find_python | Хардкод путей — может не найти python из pyenv/conda | Добавить `which python3` как fallback | Low |
