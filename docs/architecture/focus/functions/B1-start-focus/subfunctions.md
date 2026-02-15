# B1 — Запуск режима фокуса: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B1: Запуск режима фокуса |
| Файл(ы) | lib.rs:L1839-1926 |
| LOC | 88 |
| Подфункций | 6 |
| Сложность (max) | Medium |

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B1.1 | Проверка активного режима | L1839-1850 | 12 | FocusManager -> FocusState | State: lock mutex | validation | Trivial |
| B1.2 | Загрузка конфигурации блокировки (apps) | L1852-1876 | 25 | apps: Option, config_path -> block_apps: Vec | File: read blocker_config.json | transform | Low |
| B1.3 | Загрузка конфигурации блокировки (sites) | L1878-1890 | 13 | sites: Option, config_path -> block_sites: Vec | File: read blocker_config.json | transform | Trivial |
| B1.4 | Запись в /etc/hosts через osascript | L1892-1908 | 17 | block_sites -> () | Shell: osascript с admin privileges; File: /etc/hosts; System: flush DNS | external | Medium |
| B1.5 | Завершение заблокированных приложений | L1910-1916 | 7 | block_apps -> () | Shell: osascript quit app | external | Trivial |
| B1.6 | Обновление состояния FocusState | L1918-1926 | 9 | duration, apps, sites -> state | State: update FocusState | persist | Trivial |

## Data Flow

```
[Input: duration_minutes, apps?, sites?, FocusManager]
    │
    ▼
┌─ B1.1 Check Active ──┐
│  state.active == true? │
│  → Err("already active")│
└──────┬───────────────┘
       │ not active
       ▼
┌─ B1.2-B1.3 Load Config ──────────────┐
│  apps provided?                        │
│  ├── Yes → use provided                │
│  └── No → read blocker_config.json     │ ←── [File: ~/hanni/]
│           └── fallback: default list   │
│  (same for sites)                      │
└──────┬────────────────────────────────┘
       │ block_apps, block_sites
       ▼
┌─ B1.4 Write /etc/hosts ─────────────────┐
│  Build hosts_entries:                     │
│    "127.0.0.1 {site}\n127.0.0.1 www.{}"  │
│  Wrap in HANNI FOCUS BLOCKER markers      │
│  osascript "do shell script" with admin   │ ──→ [Shell: sudo /etc/hosts]
│  + dscacheutil -flushcache                │
│  + killall mDNSResponder                  │
└──────┬───────────────────────────────────┘
       │
       ▼
┌─ B1.5 Quit Apps ──────────────┐
│  for app in block_apps:        │
│    osascript "tell app to quit"│ ──→ [Shell: osascript]
└──────┬────────────────────────┘
       │
       ▼
┌─ B1.6 Update State ─────────┐
│  state.active = true          │
│  state.end_time = now + dur   │
│  state.blocked_apps = apps    │
│  state.blocked_sites = sites  │
│  monitor_running = true       │
└──────┬───────────────────────┘
       │
       ▼
[Output: Ok("Focus mode started for N minutes")]
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B1.4 | /etc/hosts | Требует admin привилегий (osascript popup) — плохой UX | Рассмотреть PAC-прокси или DNS-based блокировку | Medium |
| B1.2-B1.3 | Загрузка конфига | Дублирование логики чтения config для apps и sites | Вынести в общую функцию load_blocklist_config() | Low |
