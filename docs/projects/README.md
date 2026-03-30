# Hanni Projects Overview

**[overview.md](overview.md)** — зачем Hanni, как табы связаны, где какие данные живут (пользовательская перспектива)

All tabs/projects in Hanni, their purpose, database tables, and views.

## Summary Table

| Tab | Icon | Purpose | DB Tables | Views |
|-----|------|---------|-----------|-------|
| [Chat](chat.md) | `💬` | AI assistant, memory, training | conversations, facts, vec_facts, memory_decay, flywheel_cycles | Single chat view |
| [Calendar](calendar.md) | `📅` | Events, recurring schedules | events, schedules, schedule_completions | Month, Week, Day, List, Table |
| [Focus](focus.md) | `🎯` | Deep work timer, activity tracking | activities | Current, History |
| [Notes](notes.md) | `📝` | Quick notes, tasks, tags | notes, note_tags | List, Kanban, Timeline, Table, Gallery |
| [Work](work.md) | `💼` | Task management by project | projects, tasks | Unified task list |
| [Projects](projects.md) | `📁` | Custom user-created pages | custom_pages, project_records, property_definitions, property_values | Dynamic (user-defined) |
| [Development](development.md) | `📖` | Learning resources tracker | learning_items | All, Courses, Books, Skills, Articles |
| [Home](home.md) | `🏠` | Household inventory | home_items | Dashboard + Table |
| [Hobbies](hobbies.md) | `🎮` | Media collections | media_items, user_lists, list_items | 10 media types, Gallery/Kanban/Table |
| [Sports](sports.md) | `⚔️` | Workouts, body tracking | workouts, exercises, body_records | Workouts, Martial Arts, Stats |
| [Health](health.md) | `❤️` | Habits, health metrics | habits, habit_checks, health_log | Habits, Body |
| [Mindset](mindset.md) | `🧠` | Journal, mood, principles | journal_entries, mood_log, principles | Journal, Mood, Principles |
| [Food](food.md) | `🍴` | Nutrition, recipes, pantry | food_log, recipes, products | Food Log, Recipes, Products |
| [Money](money.md) | `💳` | Finances, budgets, debts | transactions, budgets, savings_goals, subscriptions, debts | 5 sub-tabs |
| [People](people.md) | `👥` | Contacts, relationships | contacts, contact_blocks | All, Favorites, Blocked |
| [Schedule](schedule.md) | `📅` | Recurring routines, streaks | schedules, schedule_completions | Table, Tracking |
| [DanKoe](dankoe.md) | `🧠` | Personal development protocol | dan_koe_entries | Dashboard + 4 practices |
| [Screen Time](screen-time.md) | `🖥️` | Auto screen time & productivity | activity_snapshots | **Planned** — Dashboard, Apps, Sites, Heatmap |

## Architecture Notes

- **Central hub**: Schedule is the central tracking hub; other projects link to it via relations
- **State management**: All persistent data in `S` object (state.js), DB via Tauri `invoke()`
- **Custom pages**: Notion-style dynamic properties via `property_definitions` + `property_values`
- **Unified layout**: Most tabs use `renderUnifiedLayout()` with Dashboard + Table pattern
- **Block editor**: Each tab can have block-based notes via `tab_page_blocks` table
