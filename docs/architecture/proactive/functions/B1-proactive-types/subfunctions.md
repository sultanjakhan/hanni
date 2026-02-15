# B1 — Типы и настройки проактивности: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: Типы и настройки проактивности |
| Файл(ы) | lib.rs:L181-271 |
| LOC | 91 |
| Подфункций | 5 |
| Сложность (max) | Low |

## Подфункции

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B1.1 | Определение структуры ProactiveSettings | L181-197 | 17 | — → struct (serde Serialize/Deserialize) | — | transform | Low |
| B1.2 | Методы quiet hours (quiet_start_minutes, quiet_end_minutes) | L199-212 | 14 | self → u32 (минуты от полуночи) | — | transform | Trivial |
| B1.3 | Парсер времени "HH:MM" → минуты | L214-223 | 10 | s: &str → Option<u32> | — | parse | Trivial |
| B1.4 | Default для ProactiveSettings | L225-239 | 15 | — → ProactiveSettings с дефолтами | — | transform | Trivial |
| B1.5 | Определение структуры ProactiveState + конструктор | L241-271 | 31 | settings → ProactiveState | — | transform | Low |

## Проблемные зоны

Нет критичных проблем.
