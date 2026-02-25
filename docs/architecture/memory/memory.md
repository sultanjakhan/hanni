# Memory Module

Факты памяти, FTS5 поиск, контекст для LLM, Mem0-style dedup, decay, браузер памяти.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~800 |
| Backend functions | 8 |
| Frontend functions | 4 |
| Complexity | Simple: 4, Medium: 3, Complex: 2 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L1384-1452 | Миграции (JSON → SQLite, decay columns, category) |
| Backend | `lib.rs` | L1456-1752 | Semantic: embed, store, search, build_context, gather, rerank |
| Backend | `lib.rs` | L5144-5287 | CRUD фактов (remember, recall, forget, search) |
| Backend | `lib.rs` | L5414-5846 | Extraction + Mem0 dedup pipeline + profile synthesis |
| Backend | `lib.rs` | L7083-7161 | Memory cleanup (duplicates, stale, noise) |
| Frontend | `main.js` | L1424-1546 | Память в настройках чата (CRUD, поиск) |
| Frontend | `main.js` | L3303-3415 | Вкладка памяти (все факты, добавление, редактирование) |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB, types), voice_server (embeddings at :8237, rerank) |
| **Used by** | chat (memory context injection), proactive (user name lookup) |

## Architecture

### 4-Tier Memory Retrieval
1. **Semantic** — vector similarity via sqlite-vec + voice_server /embed
2. **Core facts** — user/preferences, sorted by decay score
3. **FTS5** — keyword matching on user message
4. **Recent** — fallback, ordered by updated_at

### Mem0-Style Dedup Pipeline
1. Extract facts from conversation (LLM, temp=0.3)
2. Embed extracted facts via voice_server
3. Find similar existing facts (distance < 0.35)
4. For facts with similar matches → batch LLM dedup (ADD/UPDATE/NOOP)
5. For facts without matches → direct INSERT with ON CONFLICT

### Decay Formula (v0.18.6)
```sql
(access_count * 0.5) + (days_since_accessed * -0.05)
```
Higher access_count = higher priority. Slow penalty for aging (-0.05/day vs old -0.1).

## Key Improvements (v0.18.6)

### Stability Fixes
- **Extraction LLM timeout** — 60s timeout prevents hanging on slow inference
- **Dedup LLM timeout** — 30s timeout on batch dedup call
- **Profile synthesis timeout** — 45s timeout on spawned synthesis task
- **Memory cleanup less aggressive** — only removes facts with access_count=0 after 90 days (was ≤1 after 60)

### New Features
- **ME1: Memory decay** — decay scoring in context building and recall
- **ME7: Conflict detection** — logs `memory_conflict` insight when extracted fact overwrites existing value
- **ME8: Profile synthesis** — auto-generates user profile after extraction (with timeout)

### Quality Fixes
- **Decay formula rebalanced** — access_count weight 0.3→0.5, aging penalty 0.1→0.05
- **Dedup prompt: Russian** — "For #N" → "Для #N"
- **Frontend validation** — min 2 chars for key and value in add modals

## Improvements Status

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| ME1 | Memory decay (TTL / exponential decay) | M | ✅ |
| ME2 | Уведомление о запомненных фактах | S | ✅ |
| ME3 | Confidence scoring в UI | S | ⬜ |
| ME4 | Bulk operations ("забудь всё о X") | S | ⬜ |
| ME5 | Adaptive extraction (обучение на удалениях) | M | ⬜ |
| ME6 | Cross-conversation linking | M | ⬜ |
| ME7 | Proactive memory conflict detection | S | ✅ |
| ME8 | Auto-synthesis user profile при изменении фактов | S | ✅ |
