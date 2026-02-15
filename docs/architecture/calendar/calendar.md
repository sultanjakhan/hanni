# Calendar Module

События CRUD, синхронизация Apple/Google Calendar, RRULE.

## Overview

| Metric | Value |
|--------|-------|
| Total LOC | ~1030 |
| Backend functions | 4 |
| Frontend functions | 6 |
| Complexity | Simple: 1, Medium: 3, Complex: 6 |

## Files

| Layer | File | Lines | Description |
|-------|------|-------|-------------|
| Backend | `lib.rs` | L3829-4329 | CRUD, Apple sync, RRULE, Google ICS |
| Frontend | `main.js` | L3288-3814 | Месяц/неделя/день, список, интеграции, модалка |

## Dependencies

| Direction | Modules |
|-----------|---------|
| **Uses** | core (DB), system (AppleScript for EventKit) |
| **Used by** | tracker (upcoming events), proactive (calendar context) |
