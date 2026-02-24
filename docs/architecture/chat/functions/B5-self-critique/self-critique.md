# B5 — Self-Critique (Self-Refine)

> D. Self-Critique — автоматическая проверка сложных ответов

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | B5: Самокритика ответов |
| Файлы | lib.rs (quality_check_response, chat), main.js (toggle) |
| По умолчанию | ВЫКЛЮЧЕНО (opt-in через настройки) |
| Настройка | `app_settings.enable_self_refine` = "true"/"false" |
| Латентность | ~1.5-2с (150 токенов, non-streaming, M3 Pro) |

## quality_check_response() — lib.rs:3998

```rust
async fn quality_check_response(
    client: &reqwest::Client,
    user_msg: &str,
    assistant_response: &str,
) -> Result<Option<String>, String>
```

### Промпт

| Роль | Содержание |
|------|-----------|
| system | "Ты — критик ответов. Будь краток. Отвечай на русском." |
| user | "Пользователь спросил: \"{user_msg}\"\n\nТвой ответ: \"{response}\"\n\nПроверь ответ. Если он корректный и полный — напиши только [OK]. Если есть фактическая ошибка или важное упущение — коротко укажи (1-2 предложения)." |

### Параметры LLM

| Параметр | Значение |
|----------|---------|
| max_tokens | 150 |
| temperature | 0.2 |
| stream | false |
| timeout | 30s |
| enable_thinking | false |

### Возвращает

- `Ok(None)` — ответ корректный (содержит `[OK]` или пустой)
- `Ok(Some(correction))` — текст коррекции
- `Err(...)` — ошибка запроса/парсинга

Стрипает `<think>...</think>` из ответа.

## Интеграция в chat() — lib.rs:3579

### Условия срабатывания (ВСЕ должны быть true)

| # | Условие | Зачем |
|---|---------|-------|
| 1 | `!is_call` | Не в голосовом режиме (слишком медленно) |
| 2 | `result.tool_calls.is_empty()` | Не вызываются инструменты (там своя логика) |
| 3 | `result.text.len() > 150` | Короткие ответы проверять бессмысленно |
| 4 | `is_complex_query(last_user_msg)` | Только сложные вопросы |
| 5 | `enable_self_refine == "true"` | Пользователь включил в настройках |

### Поведение

```
chat_inner() → result
     │
     ├─ conditions not met → return result as-is
     │
     └─ conditions met → quality_check_response()
          │
          ├─ Ok(None)     → return result as-is (ответ хорош)
          ├─ Ok(Some(c))  → emit("chat-token", "\n\n_{c}_") + return result
          └─ Err(_)       → ignore error, return result as-is
```

Коррекция доставляется через тот же `chat-token` event — появляется как italics в конце ответа.

## Frontend — main.js:902

### Toggle в Chat Settings > Основные

```html
<div class="settings-row">
  <span class="settings-label">Самопроверка</span>
  <span class="settings-hint">Авто-критика сложных ответов</span>
  <label class="toggle">
    <input type="checkbox" id="chat-self-refine-toggle">
    <span class="toggle-slider"></span>
  </label>
</div>
```

### Загрузка (main.js:829)

```javascript
invoke('get_app_setting', { key: 'enable_self_refine' }).catch(() => null)
```

### Сохранение (main.js:1035)

```javascript
document.getElementById('chat-self-refine-toggle')?.addEventListener('change', (e) => {
  invoke('set_app_setting', { key: 'enable_self_refine', value: e.target.checked ? 'true' : 'false' });
});
```
