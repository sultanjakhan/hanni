# Hanni — План разделения на 2 приложения

## Решение (v0.18+)

Разделить монолит на 2 приложения:

### 1. Hanni (AI Core) — уникальное, развиваем сами
| Модуль | Ответственность | Rust cmds | Сложность |
|--------|-----------------|-----------|-----------|
| M01: Chat Core | Сообщения, стриминг, история | 8 | High |
| M02: Voice & Call | Запись, STT, call mode, barge-in | 10 | High |
| M03: TTS | Синтез речи, голоса | 5 | Medium |
| M04: Memory | Факты, семантический поиск, экстракция | 7 | High |
| M05: AI Actions | Парсинг и исполнение action-блоков | 7 | High |
| M06: LLM & Proactive | MLX, system prompt, проактивность | 7 | High |
| M24: System | Настройки, версия, обновления | 8 | Medium |
| **Итого** | | **~52** | |

### 2. Life Tracker (Notion-like) — заменить open-source
| Модуль | Ответственность | Rust cmds | DB tables |
|--------|-----------------|-----------|-----------|
| M07: Focus & Blocklist | Таймер, блокировка | 8 | 1 |
| M08: Media Collections | Музыка, аниме, фильмы... | 12 | 3 |
| M09: Food | Питание, рецепты, продукты | 12 | 3 |
| M10: Money | Финансы, бюджеты, подписки | 19 | 5 |
| M11: Mindset | Журнал, настроение, принципы | 10 | 3 |
| M12: Sports & Health | Тренировки, привычки | 8 | 5 |
| M13: People | Контакты | 10 | 2 |
| M14: Calendar | События, синхронизация | 6 | 1 |
| M15: Notes | Заметки | 5 | 1 |
| M16: Work | Проекты, задачи | 5 | 2 |
| M17: Development | Обучение | 2 | 1 |
| M18: Home | Инвентарь, покупки | 5 | 1 |
| M19: DB Views | Notion-like свойства, фильтры | 12 | 4 |
| M20: UI Shell | Табы, навигация | 0 | 0 |
| M21: Dashboard | Обзорная панель | 2 | 1 |
| M22: Activity Tracking | Трекинг времени | 10 | 1 |
| M23: Finetune | Фидбек, training data | 6 | 1 |
| M25: Goals | Цели | 4 | 1 |
| **Итого** | | **~95** | **38** |

## Стратегия миграции Life Tracker

**Кандидаты на замену:**
- **AppFlowy** — Rust + Flutter, open-source Notion, локальный
- **NocoDB / Baserow / Teable** — open-source Airtable с REST API

**Целевая архитектура:**
```
┌─────────────┐     API/IPC     ┌──────────────┐
│   Hanni     │ ◄─────────────► │  AppFlowy    │
│  (AI Core)  │                 │ (Life Data)  │
│  Chat       │  "добавь трату  │  Money DB    │
│  Voice      │   500р кафе"    │  Food DB     │
│  Memory     │ ──────────────► │  Calendar    │
│  Actions    │                 │  Notes       │
│  LLM        │  ◄── данные ──  │  Media       │
└─────────────┘                 └──────────────┘
```

**Порядок:**
1. Сначала улучшаем Hanni Core (M01-M06)
2. Параллельно исследуем AppFlowy API
3. Постепенно мигрируем модули Life Tracker
4. Life Tracker код в Hanni становится тонким клиентом

## Текущий приоритет: Hanni Core
