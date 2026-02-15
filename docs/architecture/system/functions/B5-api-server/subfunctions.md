# B5 — REST API сервер: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B5: REST API сервер (port 8235) |
| Файл(ы) | lib.rs:L2344-2523 |
| LOC | 180 |
| Подфункций | 9 |
| Сложность (max) | High |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B5.1 | Генерация API токена и создание состояния | L2344-2358 | 15 | AppHandle -> ApiState | File: read/create token | control | Trivial |
| B5.2 | Функция проверки авторизации check_auth() | L2360-2371 | 12 | headers, token -> Result | -- | validation | Trivial |
| B5.3 | Определение структур запросов (ChatReq, SearchQuery, RememberReq) | L2373-2390 | 18 | -- -> types | -- | transform | Trivial |
| B5.4 | Эндпоинт GET /api/status | L2392-2416 | 25 | ApiState -> Json | Network: check MLX health; State: read LlmBusy, FocusManager | query | Low |
| B5.5 | Эндпоинт POST /api/chat | L2418-2432 | 15 | ChatReq, auth -> Json | Network: LLM call (через chat_inner) | external | Low |
| B5.6 | Эндпоинт GET /api/memory/search | L2434-2485 | 52 | SearchQuery, auth -> Json | DB: SELECT facts_fts + fallback LIKE | query | Medium |
| B5.7 | Эндпоинт POST /api/memory | L2487-2505 | 19 | RememberReq, auth -> Json | DB: INSERT/UPDATE facts | persist | Low |
| B5.8 | Сборка роутера (Router) | L2507-2512 | 6 | endpoints, state -> Router | -- | control | Trivial |
| B5.9 | Привязка к порту и запуск сервера | L2514-2523 | 10 | Router -> () | Network: bind 127.0.0.1:8235, serve | external | Low |

## Data Flow

```
[Input: AppHandle]
    │
    ▼
┌─ B5.1 Init State ──────┐
│  get_or_create_api_token │ ←── [File: token]
│  ApiState { app, token } │
└──────┬──────────────────┘
       │
       ▼
┌─ B5.2-B5.3 Auth + Types ─┐
│  check_auth(headers)       │
│  ChatReq, SearchQuery,     │
│  RememberReq               │
└──────┬────────────────────┘
       │
       ▼
┌─ B5.4-B5.7 Endpoints ────────────────────────────────┐
│                                                        │
│  /api/status (GET, no auth)                            │
│    ├── LlmBusy.load()                                  │
│    ├── FocusManager.lock()                              │
│    └── GET http://127.0.0.1:8234/v1/models ──→ [MLX]   │
│                                                        │
│  /api/chat (POST, auth)                                │
│    └── chat_inner() ──→ [MLX Server]                   │
│                                                        │
│  /api/memory/search (GET, auth)                        │
│    ├── FTS5 MATCH ←── [DB: facts_fts]                  │
│    └── fallback LIKE ←── [DB: facts]                   │
│                                                        │
│  /api/memory (POST, auth)                              │
│    └── INSERT/UPDATE ──→ [DB: facts]                   │
└──────┬────────────────────────────────────────────────┘
       │
       ▼
┌─ B5.8-B5.9 Router + Serve ─┐
│  Router::new()               │
│    .route("/api/status")     │
│    .route("/api/chat")       │
│    .route("/api/memory/...")  │
│  TcpListener::bind(:8235)    │ ──→ [Network: localhost:8235]
│  axum::serve()               │
└─────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B5.6 | Memory search | 52 LOC с двойной стратегией поиска (FTS + LIKE fallback) в одном обработчике | Вынести поиск в отдельную функцию search_facts() | Low |
| B5.9 | Привязка к порту | Нет retry — если порт занят, сервер не стартует | Добавить retry или выбор альтернативного порта | Low |
| B5.* | Все эндпоинты | API не версионирован (/api/... вместо /api/v1/...) | Добавить версию API для обратной совместимости | Low |
