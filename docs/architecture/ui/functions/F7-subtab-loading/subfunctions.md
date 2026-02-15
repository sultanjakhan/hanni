# F7 — Sub-tab content loading: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F7: Sub-tab content loading |
| Файл(ы) | main.js:L644-700 |
| LOC | 57 |
| Подфункций | 4 |
| Сложность (max) | Low |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F7.1 | Диспетчер контента по tabId | L644-666 | 23 | loadSubTabContent(tabId, subTab) → switch 15 case: chat, dashboard, calendar, focus, notes, work, development, home, hobbies, sports, health, mindset, food, money, people, settings | control | Low |
| F7.2 | Tab add dropdown | L668-687 | 20 | click #tab-add → показать dropdown с незанятыми табами → click item → openTab(id) | interaction | Low |
| F7.3 | Глобальное закрытие dropdown | L689-691 | 3 | click на document → скрыть #tab-dropdown | interaction | Trivial |
| F7.4 | Клавиатурные шорткаты | L693-700 | 8 | Cmd+W → closeTab; Cmd+T → open tab dropdown; Cmd+1..9 → switchTab | interaction | Trivial |

## Data Flow

```
[Input: loadSubTabContent(tabId, subTab)]
    │
    ▼
┌─ F7.1 Диспетчер ────────────────┐
│  switch(tabId):                    │
│    'chat' → subTab='Настройки'?    │
│      showChatSettingsMode() +      │
│      loadChatSettings()            │
│    : hideChatSettingsMode() +      │
│      loadConversationsList()       │
│    'dashboard' → loadDashboard()   │
│    'calendar'  → loadCalendar(sub) │
│    'focus'     → loadFocus()       │
│    'notes'     → loadNotes(sub)    │
│    'hobbies'   → loadHobbies(sub)  │
│    'settings'  → loadSettings(sub) │
│    ... (15 cases total)            │
└──────────────────────────────────────┘

[Input: click #tab-add]
    │
    ▼
┌─ F7.2 Tab dropdown ─────────────┐
│  filter TAB_REGISTRY — openTabs    │
│  → dropdown items → click →        │
│    openTab(id)                     │
└──────────────────────────────────────┘

[Input: keydown Cmd+key]
    │
    ▼
┌─ F7.4 Keyboard shortcuts ───────┐
│  Cmd+W → closeTab(activeTab)       │
│  Cmd+T → tab-add click             │
│  Cmd+1..9 → switchTab(openTabs[n]) │
└──────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F7.1 | Диспетчер | 15 case в switch — растёт с каждым новым табом | Использовать lookup-таблицу { tabId: loaderFn } вместо switch | Low |
| F7.2 | Dropdown | Позиционирование через getBoundingClientRect — может выходить за экран | Добавить проверку viewport boundaries | Low |
