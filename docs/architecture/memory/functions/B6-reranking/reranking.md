# B6 — FlashRank Reranking

> B. FlashRank Reranking — улучшение релевантности контекста памяти

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B6: Reranking фактов памяти |
| Файлы | lib.rs (gather + rerank), voice_server.py (/rerank endpoint) |
| Зависимости | `pip install flashrank` (~2MB код, ~150MB модель) |
| Модель | ms-marco-MultiBERT-L-12 (ONNX, мультиязычный) |
| Fallback | При ошибке → оригинальный порядок candidates → build_memory_context_from_db |

## Новые функции

### gather_memory_candidates() — lib.rs:1508

```rust
fn gather_memory_candidates(
    conn: &rusqlite::Connection,
    user_msg: &str,
    pool_size: usize,
    semantic_hits: Option<&[(i64, f64)]>,
) -> Vec<(i64, String, String, String)>
```

Собирает факты из 4 тиров в единый пул для последующего реранкинга:

| Тир | Источник | Лимит | Приоритет |
|-----|---------|-------|-----------|
| 0 | Семантический (sqlite-vec) | 15 hits | Высший |
| 1 | Core (user/preferences) | 20 | Высокий |
| 2 | FTS5 (текстовый поиск) | pool_size | Средний |
| 3 | Recent (по updated_at) | остаток | Низкий |

- `pool_size` = `memory_limit * 2` (удвоенный лимит — для отбора через rerank)
- Дедупликация по `seen_ids: HashSet<i64>`
- Возвращает `Vec<(fact_id, category, key, value)>`
- Sync, вызывается внутри DB lock

### rerank_facts() — lib.rs:1590

```rust
async fn rerank_facts(
    client: &reqwest::Client,
    query: &str,
    facts: &[(i64, String, String, String)],
    top_k: usize,
) -> Result<Vec<(i64, f64)>, String>
```

- POST на `http://127.0.0.1:8237/rerank`
- Timeout: 10 секунд
- Body: `{"query": "...", "passages": [{"id": N, "text": "[cat] key=val"}], "top_k": N}`
- Response: `{"results": [{"id": N, "text": "...", "score": 0.95}]}`
- Async, вызывается после отпускания DB lock

### /rerank endpoint — voice_server.py:490

```python
POST /rerank
Content-Type: application/json

{
    "query": "кофе",
    "passages": [
        {"id": 1, "text": "[user] coffee=likes espresso"},
        {"id": 2, "text": "[user] age=21"}
    ],
    "top_k": 30
}

→ {"results": [{"id": 1, "text": "...", "score": 0.87}, ...]}
```

### ensure_reranker() — voice_server.py:558

```python
def ensure_reranker() -> Ranker | None
```

- Lazy-load с thread lock (double-checked locking)
- `flashrank.Ranker(model_name="ms-marco-MultiBERT-L-12")`
- При `ImportError` → `logger.warning`, возвращает `None`
- HTTP 501 если None

## Data Flow в chat_inner

```
[User message]
     │
     ▼
┌─ embed_texts() ──────────────┐  async, no DB lock
│  query_embedding (384-dim)    │
└──────┬───────────────────────┘
       │
       ▼
┌─ DB lock ────────────────────┐
│  search_similar_facts()       │  semantic hits
│  gather_memory_candidates()   │  pool_size = limit * 2
│  profile (user_profile)       │
└──────┬───────────────────────┘  DB lock dropped
       │
       ▼
┌─ rerank_facts() ─────────────┐  async HTTP POST
│  POST /rerank                 │  timeout 10s
│  → top_k sorted by score      │
└──────┬───────────────────────┘
       │ Ok → reranked facts
       │ Err → fallback to original order
       ▼
┌─ Build context ──────────────┐
│  [About the user]\n{profile}  │  profile всегда включён
│  [Relevant details]\n{facts}  │  reranked или fallback
└──────────────────────────────┘
```

## Graceful Degradation

| Сценарий | Поведение |
|----------|----------|
| flashrank не установлен | HTTP 501 → fallback на original order candidates |
| voice_server не запущен | Connection error → fallback на original order candidates |
| Timeout (>10s) | Timeout error → fallback на original order candidates |
| Candidates пусты | → fallback на `build_memory_context_from_db()` (sync) |
| embed_texts() упал | semantic_hits = None → gather без семантики, rerank от FTS+recent |
