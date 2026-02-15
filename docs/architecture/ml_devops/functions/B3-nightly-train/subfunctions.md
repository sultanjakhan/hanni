# B3 — Nightly LoRA fine-tuning: Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B3: Nightly LoRA fine-tuning |
| Файл(ы) | scripts/nightly_train.sh:L1-217 |
| LOC | 217 |
| Подфункций | 9 |
| Сложность (max) | High |

## Подфункции

### Backend (Script)

| # | Подфункция | Строки | LOC | Input -> Output | Side effects | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-------------|-----|-----------|
| B3.1 | Конфигурация и проверка feedback | L1-32 | 32 | -- -> FEEDBACK_COUNT, paths | DB: SELECT COUNT message_feedback | validation | Low |
| B3.2 | Экспорт training data из БД (Python inline) | L34-118 | 85 | DB conversations + feedback -> JSONL | DB: SELECT conversations, message_feedback; File: write train/valid/test.jsonl | query | High |
| B3.3 | Проверка экспорта | L120-124 | 5 | exit code -> continue/abort | -- | validation | Trivial |
| B3.4 | Опциональная Claude distillation | L126-134 | 9 | -- -> augmented training data | Shell: claude_distill.sh | external | Trivial |
| B3.5 | Остановка MLX сервера | L136-142 | 7 | -- -> () | Shell: launchctl bootout, pkill mlx_lm | external | Trivial |
| B3.6 | Бэкап предыдущего адаптера | L144-149 | 6 | adapter_dir -> adapter_backup | File: cp -r adapter dir | persist | Trivial |
| B3.7 | Запуск LoRA обучения | L151-183 | 33 | model + data + config -> adapter | Process: python3 -m mlx_lm lora; File: write adapter | external | High |
| B3.8 | Пометка feedback как exported + обновление plist | L185-203 | 19 | -- -> () | DB: UPDATE message_feedback; File: PlistBuddy edit plist | persist | Low |
| B3.9 | Перезапуск MLX сервера и проверка | L205-217 | 13 | -- -> () | Shell: launchctl bootstrap; Network: curl health check | external | Low |

## Data Flow

```
[Entry: nightly_train.sh (LaunchAgent, 3 AM)]
    │
    ▼
┌─ B3.1 Check Feedback ──────────────────────┐
│  sqlite3 "SELECT COUNT FROM message_feedback│ ←── [DB: hanni.db]
│  WHERE exported = 0"                        │
│  < MIN_FEEDBACK(10)? → exit 0               │
└──────┬─────────────────────────────────────┘
       │ enough feedback
       ▼
┌─ B3.2 Export Data (Python) ──────────────────────────┐
│  Load feedback: conversation_id -> {idx: rating}      │ ←── [DB: message_feedback]
│  Load conversations (message_count >= 4)              │ ←── [DB: conversations]
│  Read system_prompt.txt                               │ ←── [File]
│  Filter: skip negatively rated assistant messages     │
│  Separate: rated (has positive) vs unrated            │
│  Split 80/10/10                                       │
│  Write train.jsonl, valid.jsonl, test.jsonl           │ ──→ [File: training/]
└──────┬───────────────────────────────────────────────┘
       │
       ▼
┌─ B3.4 Claude Distillation (optional) ─┐
│  if claude CLI available:              │
│    ./claude_distill.sh --count 10      │ ──→ [External: Claude]
│  else: skip                            │
└──────┬────────────────────────────────┘
       │
       ▼
┌─ B3.5 Stop MLX ──────────────────────┐
│  launchctl bootout com.hanni.mlx      │ ──→ [System: LaunchAgent]
│  pkill -f "mlx_lm server"            │
│  sleep 3                              │
└──────┬───────────────────────────────┘
       │
       ▼
┌─ B3.6 Backup Adapter ─────────────────┐
│  cp -r lora-adapter/ lora-adapter-backup│ ──→ [File]
└──────┬────────────────────────────────┘
       │
       ▼
┌─ B3.7 LoRA Training ──────────────────────────────────┐
│  python3 -m mlx_lm lora \                             │
│    --train --model Qwen3-32B-4bit \                    │
│    --data TRAINING_DIR \                               │
│    --adapter-path ADAPTER_DIR \                        │
│    --mask-prompt --batch-size 1 --iters 50 \           │
│    --learning-rate 2e-5 --grad-checkpoint --test       │ ──→ [File: adapter]
│                                                        │
│  if FAILED:                                            │
│    restore backup adapter or remove                    │
└──────┬────────────────────────────────────────────────┘
       │
       ▼
┌─ B3.8 Post-training ──────────────────────────────────┐
│  if SUCCESS:                                           │
│    sqlite3 "UPDATE message_feedback SET exported=1"    │ ──→ [DB]
│                                                        │
│  if adapter exists && plist lacks --adapter-path:       │
│    PlistBuddy add --adapter-path to plist              │ ──→ [File: plist]
└──────┬────────────────────────────────────────────────┘
       │
       ▼
┌─ B3.9 Restart MLX ───────────────────────────┐
│  launchctl bootstrap com.hanni.mlx-server     │ ──→ [System: LaunchAgent]
│  sleep 5                                      │
│  curl http://127.0.0.1:8234/v1/models         │ ──→ [Network: health check]
│  log success/warning                          │
└───────────────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| B3.2 | Экспорт данных | 85 LOC Python в bash heredoc — тяжело отлаживать и тестировать | Вынести в отдельный Python скрипт | Medium |
| B3.7 | LoRA training | Фиксированные гиперпараметры (50 iters, lr 2e-5) — нет адаптации к размеру данных | Масштабировать iters от количества примеров | Low |
| B3.5+B3.9 | Stop/Start MLX | Downtime ~10+ минут — пользователь не может использовать Hanni | Добавить уведомление пользователю о тренировке | Medium |
| B3.8 | Обновление plist | PlistBuddy вызовы хрупкие — ошибки подавляются `|| true` | Валидировать plist после изменения | Low |
