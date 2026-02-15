# B7 — View configurations: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B7: View configurations |
| Файл(ы) | lib.rs:L6106-6384 |
| LOC | 279 |
| Подфункций | 5 |
| Сложность (max) | Low |

Примечание: диапазон L6106-6384 включает три Tauri-команды для view configs (L6106-6148), а также два блока констант для proactive messaging (L6347-6384). Структуры IntegrationItem/IntegrationsInfo и команды get_integrations/get_model_info формально попадают в диапазон, но документированы отдельно в B8.

## Подфункции

### Backend

| # | Подфункция | Строки | LOC | Input → Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|---------------|-------------|-----|-----------|
| B7.1 | Получение конфигураций вью | L6106-6126 | 21 | tab_id → Vec<JSON> (id, name, view_type, filter_json, sort_json, visible_columns, is_default, position) | DB read: view_configs | query | Low |
| B7.2 | Создание конфигурации вью | L6128-6138 | 11 | tab_id, name, view_type? → i64 (new id) | DB write: view_configs INSERT | persist | Trivial |
| B7.3 | Обновление конфигурации вью | L6140-6147 | 8 | id, filter_json?, sort_json?, visible_columns? → () | DB write: view_configs UPDATE (до 3 отдельных) | persist | Trivial |
| B7.4 | Константа PROACTIVE_PROMPT_HEADER | L6349-6360 | 12 | — → &str | — | data | Trivial |
| B7.5 | Константа PROACTIVE_PROMPT_FOOTER | L6362-6384 | 23 | — → &str | — | data | Trivial |

## Data Flow

```
[Frontend] ──invoke──→ B7.1 get_view_configs(tab_id)         ──SELECT──→ [DB: view_configs]
[Frontend] ──invoke──→ B7.2 create_view_config(tab_id, name) ──INSERT──→ [DB: view_configs]
[Frontend] ──invoke──→ B7.3 update_view_config(id, ...)      ──UPDATE──→ [DB: view_configs]

B7.4 PROACTIVE_PROMPT_HEADER ──используется──→ proactive messaging loop
B7.5 PROACTIVE_PROMPT_FOOTER ──используется──→ proactive messaging loop
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B7.3 | Обновление конфигурации | До 3 отдельных UPDATE-запросов вместо одного | Собрать SET-клаузы динамически | Low |
| — | Весь блок L6106-6384 | Диапазон смешивает view configs, integrations (B8) и proactive prompt constants | Перенести константы в отдельный блок, уточнить границы B7/B8 | Low |
