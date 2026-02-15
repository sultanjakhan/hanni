# B3 — Парсинг RRULE + расширение: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B3: Парсинг RRULE + расширение |
| Файл(ы) | lib.rs:L4034-4219 |
| LOC | 186 |
| Подфункций | 10 |
| Сложность (max) | High |

## Подфункции

### Структура RRule (L4034-4040)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.1 | Определение структуры RRule | L4034-4040 | 7 | — → `struct RRule` | — | transform | Trivial |

### Функция parse_rrule (L4042-4075)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.2 | Извлечение строки RRULE | L4043-4044 | 2 | `block: &str` → `rrule_line: &str` | — | parse | Trivial |
| B3.3 | Парсинг параметров RRULE | L4046-4074 | 29 | `params: &str` → `RRule { freq, interval, count, until, byday }` | — | parse | Low |

### Функция parse_exdates (L4078-4093)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.4 | Сбор исключённых дат EXDATE | L4078-4093 | 16 | `block: &str` → `HashSet<NaiveDate>` | — | parse | Low |

### Функция byday_to_weekday (L4096-4109)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.5 | Маппинг BYDAY кодов в Weekday | L4096-4109 | 14 | `code: &str` → `Option<Weekday>` | — | transform | Trivial |

### Функция expand_rrule (L4112-4219)

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B3.6 | Вычисление границ месяца и лимитов | L4118-4135 | 18 | `target_year, target_month, start_date, rrule` → `month_start, month_end, effective_end, max_count, until` | — | transform | Low |
| B3.7 | Расширение DAILY правила | L4141-4151 | 11 | `start_date, step, until, exdates` → `Vec<NaiveDate>` | — | process | Trivial |
| B3.8 | Расширение WEEKLY правила | L4152-4173 | 22 | `start_date, weekdays, step, until, exdates` → `Vec<NaiveDate>` | — | process | Low |
| B3.9 | Расширение MONTHLY правила | L4175-4197 | 23 | `start_date, interval, until, exdates` → `Vec<NaiveDate>` | — | process | Low |
| B3.10 | Расширение YEARLY правила | L4198-4216 | 19 | `start_date, interval, until, exdates` → `Vec<NaiveDate>` | — | process | Low |

## Data Flow

```
[Input: VEVENT block (ICS text)]
    │
    ├──────────────────────┐
    ▼                      ▼
┌─ B3.2 Извлечение ─┐  ┌─ B3.4 EXDATE ─┐
│  find RRULE: line   │  │  find EXDATE   │
└──────┬─────────────┘  │  parse dates   │
       │                 └──────┬─────────┘
       ▼                        │
┌─ B3.3 Парсинг ─────┐         │
│  split(';')          │        │
│  match key → val     │        │
│  FREQ, INTERVAL,     │        │
│  COUNT, UNTIL, BYDAY │        │
└──────┬──────────────┘         │
       │ RRule                  │ HashSet<NaiveDate>
       ▼                        │
┌─ B3.6 Границы + лимиты ─┐   │
│  month_start/end          │   │
│  effective_end, max_count │   │
└──────┬───────────────────┘   │
       │                        │
       ▼                        ▼
┌─ B3.7-B3.10 Расширение ──────────────┐
│  match freq:                          │
│  DAILY  → B3.7: step по дням         │
│  WEEKLY → B3.8: step + weekdays      │ ←── B3.5 byday_to_weekday()
│  MONTHLY→ B3.9: advance by months    │
│  YEARLY → B3.10: advance by years    │
│  filter: in month? not exdate?       │
└──────┬───────────────────────────────┘
       │ Vec<NaiveDate>
       ▼
[Output: даты вхождений в целевом месяце]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B3.7-B3.10 | Расширение по типам | Дублирование логики фильтрации (month_start/month_end/exdates проверка в каждой ветке) | Вынести общую фильтрацию в отдельную функцию-обёртку | Low |
| B3.9 | MONTHLY расширение | `.min(28)` для дня — теряется точность для 29-31 числа | Использовать корректный last-day-of-month fallback | Low |
