# Proactive Module

Проактивные сообщения, сбор контекста ОС, стили, engagement tracking.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~690 |
| Backend functions | 8 |
| Frontend functions | 2 |
| Complexity | Simple: 4, Medium: 3, Complex: 2 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L630-721 | Типы ProactiveSettings, ProactiveState |
| Backend | `lib.rs` | L8818-8901 | Промпт, стили, context gathering |
| Backend | `lib.rs` | L9274-9394 | proactive_llm_call + validation |
| Backend | `lib.rs` | L10500-10725 | Main polling loop, firing score, engagement |
| Frontend | `main.js` | L102-108 | PROACTIVE_STYLE_DEFINITIONS |
| Frontend | `main.js` | L225-265 | Proactive message handler |
| Frontend | `main.js` | L278-285 | Typing detection |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB), memory (user name, context), system (osascript) |
| **Used by** | chat (proactive triggers) |

## Architecture

### Firing Score (adaptive timing)
- Time ratio: +0.0 to +0.6 (interval elapsed)
- Upcoming event: +0.3
- Distraction (YouTube/Reddit/TikTok >30min): +0.25
- High engagement (>0.6): +0.1
- Deep work penalty (10-12, 14-17): -0.1
- Skip penalty (>3 consecutive): -0.15
- Minimum threshold: 0.25

### Anti-Hallucination (v0.18.6)
Generalized pattern matching: rejects food/drink/cooking suggestions not grounded in context.
Checks tea, coffee, recipe triggers against context markers.

### Engagement Tracking
- Rolling average of last 20 messages (replied/total)
- Computed at app startup from DB + updated on each reply
- Reply window: 10 minutes in frontend

## Key Improvements (v0.18.6)

### Stability Fixes
- **Generalized anti-hallucination** — tea-only check replaced with pattern table (tea/coffee/cooking)
- **Typing timeout extended** — 5s → 10s, prevents proactive firing mid-composition
- **Memory context increased** — 5 → 8 facts for better personalization
- **Engagement rate on startup** — computed from DB history at proactive loop start (was only on reply)

## Improvements Status

| # | Улучшение | Effort | Статус |
|---|-----------|--------|--------|
| P1 | Proactive actions (```action blocks in proactive) | M | ✅ |
| P2 | Event-driven triggers (WiFi, батарея, встреча) | M | ⬜ |
| P3 | Engagement-adaptive frequency | S | ⬜ |
| P4 | Proactive cancellation при user chat | M | ✅ |
| P5 | Context caching (30с) | S | ⬜ |
| P6 | Smart timing (паттерны активности) | M | ⬜ |
| P7 | Morning briefing v2 (погода, валюты, напоминания) | M | ⬜ |
