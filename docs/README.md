# Hanni docs

Минимально по дизайну.

**Структура проекта живёт в [`../projects.yaml`](../projects.yaml)** — single source of truth
(проект → файлы / таблицы / миграции / memory). Не дублируй её прозой в отдельных MD.

**Навигация по коду** — `serena` (`find_symbol` / `get_symbols_overview`) или Grep по консистентному
неймингу (`<tab>-<component>.js`, `commands_<domain>.rs`), а не устаревшие доки.

**2026-05-31 — удалён слой авто-документации** (159 файлов): `architecture/` module-индексы с
номерами строк, `projects/` per-tab проза, `MODULE_MAP.md`, `cell-types-audit.md`, `prompts/`
генераторы, v0.18-артефакты. Он был трёхмесячной давности и врал (ссылался на удалённые табы,
сдвинутые номера строк). Восстановить любой файл: `git log --diff-filter=D -- docs/` → `git restore`.

## Что осталось
- [`architecture/firebase-off-plan.md`](architecture/firebase-off-plan.md) — активный план (firebase-off).
