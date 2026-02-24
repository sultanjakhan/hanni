# B2 — Fine-tuning Infrastructure

> E. Fine-tuning — QLoRA обучение через mlx_lm

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B2: Дообучение модели на данных пользователя |
| Файлы | desktop/finetune.py (новый), lib.rs (commands), main.js (UI) |
| Библиотека | mlx_lm.lora (QLoRA) |
| Модель | mlx-community/Qwen3-32B-4bit |
| Данные | ~/Library/Application Support/Hanni/training/{train,valid,test}.jsonl |
| Адаптер | ~/Library/Application Support/Hanni/lora-adapter/ |

## finetune.py — desktop/finetune.py

Самостоятельный скрипт-обёртка над `mlx_lm.lora`.

### Использование

```bash
python3 finetune.py [--epochs 1] [--lr 1e-5] [--rank 8] [--num-layers 8] [--batch-size 1]
```

### Параметры по умолчанию

| Параметр | Значение | Описание |
|----------|---------|---------|
| --epochs | 1 | Число эпох |
| --lr | 1e-5 | Learning rate |
| --rank | 8 | LoRA rank |
| --num-layers | 8 | Кол-во LoRA-слоёв |
| --batch-size | 1 | Размер батча (для 36GB RAM) |
| --val-batches | 5 | Батчей валидации |
| --save-every | 50 | Сохранение каждые N итераций |

### Вызываемая команда

```bash
python3 -m mlx_lm.lora \
  --model mlx-community/Qwen3-32B-4bit \
  --data ~/Library/Application Support/Hanni/training/ \
  --adapter-path ~/Library/Application Support/Hanni/lora-adapter/ \
  --train \
  --iters {epochs * 100} \
  --learning-rate 1e-5 \
  --lora-rank 8 \
  --lora-layers 8 \
  --batch-size 1 \
  --val-batches 5 \
  --save-every 50
```

### Метаданные

После успешного обучения пишет `hanni_meta.json`:
```json
{
  "model": "mlx-community/Qwen3-32B-4bit",
  "epochs": 1,
  "lr": 1e-5,
  "rank": 8,
  "num_layers": 8,
  "trained_at": "2026-02-23T15:30:00.000000"
}
```

### Проверки

- `check_data()`: проверяет наличие train.jsonl и valid.jsonl
- WARNING если < 5 примеров
- Завершает с кодом ошибки subprocess при провале

## Rust Commands — lib.rs

### get_adapter_status — lib.rs:3026

```rust
#[tauri::command]
fn get_adapter_status() -> Result<serde_json::Value, String>
```

Проверяет наличие адаптера:
- `adapters.safetensors` ИЛИ `adapter_config.json` в `lora-adapter/`
- Читает `hanni_meta.json` если существует

Возвращает:
```json
{
  "exists": true,
  "meta": { "model": "...", "trained_at": "...", ... }
}
```

### run_finetune — lib.rs:3043

```rust
#[tauri::command]
async fn run_finetune() -> Result<String, String>
```

- Ищет `finetune.py` в CWD → fallback в `CARGO_MANIFEST_DIR/../../`
- Запускает `python3 finetune.py` через `Command::new`
- Возвращает stdout+stderr при успехе
- Возвращает Err с stdout+stderr при ошибке
- **Блокирующий**: UI кнопка ждёт завершения

### Регистрация — lib.rs:8942

```rust
get_adapter_status,
run_finetune,
```

Добавлены в `tauri::generate_handler![]`.

## Frontend — main.js (About section)

### Загрузка (main.js:3273)

```javascript
const [info, trainingStats, adapterStatus] = await Promise.all([
  invoke('get_model_info'),
  invoke('get_training_stats'),
  invoke('get_adapter_status'),
]);
```

### UI — секция "Fine-tuning (LoRA)"

```html
<div class="settings-section">
  <div class="settings-section-title">Fine-tuning (LoRA)</div>
  <div class="settings-row">
    <span class="settings-label">Адаптер</span>
    <span class="settings-value">${adapterInfo}</span>  <!-- "Есть (2026-02-23)" или "Нет" -->
  </div>
  <div class="settings-row">
    <span class="settings-label">Обучение</span>
    <button class="settings-btn" id="about-finetune-btn">Запустить fine-tuning</button>
  </div>
</div>
```

### Кнопка "Запустить fine-tuning" (main.js:3320)

Поток:
1. `btn.textContent = 'Запуск...'`
2. `await invoke('export_training_data')` — свежий экспорт
3. `btn.textContent = 'Обучение...'`
4. `await invoke('run_finetune')` — запуск finetune.py
5. `btn.textContent = 'Готово!'` / `btn.textContent = error`
6. Через 5с возврат к "Запустить fine-tuning"

## Существующая инфраструктура

### export_training_data — lib.rs:2888 (без изменений)

Уже делает:
- 80/10/10 split на train/valid/test.jsonl
- Фильтрация по message_feedback (positives first)
- Формат: `{"messages": [{"role":"system","content":"..."},{"role":"user",...},{"role":"assistant",...}]}`

### start_mlx_server — lib.rs:3109 (без изменений)

Уже грузит LoRA адаптер если существует:
```
--adapter-path ~/Library/Application Support/Hanni/lora-adapter/
```
