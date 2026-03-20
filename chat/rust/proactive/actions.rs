// proactive/actions.rs — LLM call and prompt building for proactive messages
use crate::types::*;
use super::triggers::truncate_utf8;
use chrono::Timelike;

const PROACTIVE_PROMPT_HEADER: &str = r#"Ты — Ханни, тёплый AI-компаньон. Пиши как друг, который рядом.

Задача: написать ОДНО короткое сообщение (1-2 предложения). По-русски, на "ты".

ДЕЙСТВИЯ (опционально):
Если нужно СДЕЛАТЬ что-то (напомнить, создать задачу, уведомить) — добавь блок после текста:
```action
{"action":"set_reminder","title":"текст","remind_at":"YYYY-MM-DDTHH:MM:SS"}
```
Разрешённые действия: set_reminder, create_task, send_notification, log_health, create_event.
НЕ используй действия без причины. Текст ОБЯЗАТЕЛЕН, действие — дополнение.

Выбери ОДИН стиль:
"#;

const PROACTIVE_PROMPT_FOOTER: &str = r#"
ПРИОРИТЕТЫ:
1. Есть триггер (событие/дистракция/просроченная задача/цель) → пиши про него
2. Есть свежий разговор → продолжи тему с новой стороны
3. Утро (8-10) → дайджест: план дня, задачи, цели
4. Вечер (21-23) → рефлексия: что получилось, что нет
5. Есть данные о здоровье/привычках → мягкое напоминание
6. Иначе → любопытство, забота, юмор (без привязки к приложению)

СТРОГИЕ ЗАПРЕТЫ:
- ЗАПРЕЩЕНО упоминать чай, кофе, перекус, еду, напитки — ВООБЩЕ, НИКОГДА, если контекст НЕ про еду
- ЗАПРЕЩЕНО писать "ты уже X часов/минут в [приложение]" — НЕ комментируй экранное время кроме триггера дистракции
- ЗАПРЕЩЕНО выдумывать факты. Если чего-то нет в [Память] — НЕ приписывай. Нет книги — не упоминай книгу. Нет проекта — не упоминай проект
- ЗАПРЕЩЕНО повторять темы/формулировки из [Уже сказано]. Если вчера было "Доброе утро! Нет событий" — сегодня НЕЛЬЗЯ так же
- ЗАПРЕЩЕНО использовать одинаковые шаблоны: "отличный день для...", "если что-то появится — дай знать", "желаю продуктивного дня"
- ЗАПРЕЩЕНО писать бессмыслицу, метафоры про чай, абсурдные сравнения
- Если нечего сказать — ответь [SKIP]. Лучше [SKIP] чем повтор или банальщина
- При дистракции (YouTube/Reddit) — ОДНО сообщение, потом [SKIP] пока пользователь не переключится

СТИЛЬ:
- МАКСИМУМ 1-2 коротких предложения. Не 3, не 4 — ровно 1-2
- Каждое сообщение должно быть УНИКАЛЬНЫМ по формулировке
- Привязывай к конкретным данным из контекста (цель, задача, событие, музыка)
- Утром — конкретный план (не "отличный день для..."). Вечером — конкретный вопрос. Днём — минимум
- Нет данных для сообщения → [SKIP]

ПРИМЕРЫ:
✅ Контекст: Музыка: Radiohead — Creep
[style:observation] Creep от Radiohead — настроение такое?

❌ Плохо: "Может чайку под Radiohead?" (чай!), "Ты уже 3 часа слушаешь" (время!)

✅ Контекст: Утро, цели: "Найти работу 0/10", "Пить воду 0/8"
[style:digest] Утро! Цель "Найти работу" — 0 из 10 откликов. Сегодня хотя бы один?

❌ Плохо: "Доброе утро! Нет событий — отличный день для отдыха!" (шаблон, пустота)

✅ Контекст: Триггер: просроченная задача "Купить молоко" (вчера)
[style:accountability] "Купить молоко" висит со вчера — ещё актуально?

✅ Контекст: Вечер, настроение не записано, 2 задачи завершены
[style:journal] Два дела сделаны сегодня. Как настроение?

✅ Контекст: Событие через 15 мин: Созвон
[style:calendar] Созвон через 15 минут!
```action
{"action":"send_notification","title":"Ханни","body":"Созвон через 15 минут!"}
```

✅ Контекст: Ничего интересного, нет триггеров
[SKIP]

❌ Плохо: "Как дела?", "Желаю продуктивного дня!", "Если что — дай знать" (пустота → [SKIP])

Формат: [style:ID] текст (+ опциональный ```action блок), или [SKIP]."#;

pub fn build_proactive_system_prompt(enabled_styles: &[String], recent_styles: &[String]) -> String {
    let hour = chrono::Local::now().hour();
    let mut prompt = PROACTIVE_PROMPT_HEADER.to_string();
    let styles: Vec<&ProactiveStyleDef> = if enabled_styles.is_empty() {
        ALL_PROACTIVE_STYLES.iter().collect()
    } else {
        ALL_PROACTIVE_STYLES.iter()
            .filter(|s| enabled_styles.iter().any(|e| e == s.id))
            .collect()
    };
    // Time-gate: digest only 8-10, journal only 19-23
    let filtered: Vec<&&ProactiveStyleDef> = styles.iter()
        .filter(|s| {
            if s.id == "digest" && !(8..=10).contains(&hour) { return false; }
            if s.id == "journal" && !(19..=23).contains(&hour) { return false; }
            true
        })
        .collect();
    for style in &filtered {
        prompt.push_str(&format!("- {}\n", style.description));
    }
    // Per-style cooldown hint
    if !recent_styles.is_empty() {
        prompt.push_str(&format!("\nНе используй эти стили (были недавно): {}\n", recent_styles.join(", ")));
    }
    prompt.push_str(PROACTIVE_PROMPT_FOOTER);
    prompt
}

pub async fn proactive_llm_call(
    client: &reqwest::Client,
    context: &str,
    _recent_messages: &[(String, chrono::DateTime<chrono::Local>)],
    _consecutive_skips: u32,
    memory_context: &str,
    delta: &str,
    triggers: &[String],
    chat_snippet: &str,
    engagement_rate: f64,
    user_name: &str,
    todays_messages: &[(String, String)],
    enabled_styles: &[String],
    recent_styles: &[String],
) -> Result<Option<String>, String> {
    // Build dynamic system prompt from enabled styles (with time-gating & cooldown)
    let mut sys_prompt = build_proactive_system_prompt(enabled_styles, recent_styles);
    if !user_name.is_empty() {
        sys_prompt = format!(
            "Пользователя зовут {}. Обращайся к нему по имени, на \"ты\".\n\n{}",
            user_name, sys_prompt
        );
    }

    let mut user_content = String::new();

    // Active triggers FIRST (highest priority)
    if !triggers.is_empty() {
        user_content.push_str(&format!("[Триггеры]\n{}\n\n", triggers.join("\n")));
    }

    // Current context (activity, music, browser)
    user_content.push_str(&format!("{}\n", context));

    // Activity delta (what changed)
    if !delta.is_empty() {
        user_content.push_str(&format!("\n[Изменения]\n{}\n", delta));
    }

    // Recent chat (for continuity, last 4 messages)
    if !chat_snippet.is_empty() {
        user_content.push_str(&format!("\n[Последний разговор]\n{}\n", chat_snippet));
    }

    // Memory (only 5 most relevant facts — less noise)
    if !memory_context.is_empty() {
        user_content.push_str(&format!("\n[Память]\n{}\n", memory_context));
    }

    // Anti-repetition: last 5 topics with timestamps
    if !todays_messages.is_empty() {
        let last_n: Vec<_> = todays_messages.iter().rev().take(5).collect();
        user_content.push_str("\n[Уже сказано сегодня]\n");
        for (msg, sent_at) in last_n.iter().rev() {
            let short = truncate_utf8(msg, 60);
            // Extract HH:MM from RFC3339 timestamp
            let hm = if sent_at.len() >= 16 { &sent_at[11..16] } else { "" };
            user_content.push_str(&format!("- {} \"{}\"\n", hm, short));
        }
    }

    // Engagement-adaptive guidance (skip penalty for new users with <5 messages)
    let has_enough_history = todays_messages.len() >= 3;
    if has_enough_history && engagement_rate < 0.2 {
        user_content.push_str("\nВовлечённость очень низкая — только [SKIP] или критичный триггер.\n");
    } else if has_enough_history && engagement_rate < 0.4 {
        user_content.push_str("\nВовлечённость низкая — пиши только если есть триггер или полезное действие.\n");
    }

    // Time-of-day tone hint
    let hour = chrono::Local::now().hour();
    if hour >= 8 && hour <= 10 {
        user_content.push_str("\n[Тон: бодрый, конкретный — утренний план]\n");
    } else if hour >= 21 && hour <= 23 {
        user_content.push_str("\n[Тон: тёплый, рефлексивный — вечерний чекин]\n");
    } else if hour >= 12 && hour <= 14 {
        user_content.push_str("\n[Тон: ненавязчивый, лёгкий]\n");
    }

    let request = ChatRequest {
        model: MODEL.into(),
        messages: vec![
            ChatMessage::text("system", &sys_prompt),
            ChatMessage::text("user", &user_content),
        ],
        max_tokens: 350,
        stream: false,
        temperature: 0.6,
        repetition_penalty: None,
        chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
        tools: None,
    };

    let response = client
        .post(MLX_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("LLM error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("MLX proactive error {}: {}", status, &body[..body.len().min(200)]));
    }

    let parsed: NonStreamResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let raw = parsed
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    // Strip <think>...</think> tags
    let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
    let text = re.replace_all(&raw, "").trim().to_string();

    if text.contains("[SKIP]") || text.is_empty() {
        return Ok(None);
    }

    // Validate output: reject gibberish (too short, no Cyrillic, or single-word answers)
    let word_count = text.split_whitespace().count();
    let has_cyrillic = text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    if word_count < 3 || !has_cyrillic {
        return Ok(None);
    }

    // Reject if model hallucinates food/drink/topics not in context
    let lower = text.to_lowercase();
    let ctx_lower = context.to_lowercase();
    let mem_lower = memory_context.to_lowercase();

    // Hard-ban food/drink words — always reject unless context is about food
    let food_words = ["чай", "чаёк", "чайку", "чайн", "кофе", "кофейку", "кофеёк", "латте",
        "капучино", "перекус", "перекуси", "покушать", "поешь", "перерыв на обед",
        "чайная пауза", "чайную паузу", "кола", "напиток"];
    let ctx_is_food = ["чай", "кофе", "еда", "блюд", "рецепт", "готов", "food", "кухн", "обед", "ужин", "завтрак"]
        .iter().any(|m| ctx_lower.contains(m));
    if !ctx_is_food && food_words.iter().any(|w| lower.contains(w)) {
        return Ok(None);
    }

    // Reject empty template phrases
    let empty_templates = [
        "отличный день для",
        "если что-то появится",
        "если что-то нужно",
        "просто дай знать",
        "желаю продуктивного",
        "желаю приятного",
        "хорошего дня",
        "наслаждайся",
        "начать день с",
    ];
    if empty_templates.iter().any(|t| lower.contains(t)) {
        return Ok(None);
    }

    // Reject if model mentions things not in memory or context (common hallucinations)
    let hallucination_words = ["книга", "книгу", "книге", "книги",
        "castlewebtoon", "тяпляп", "афкш"];
    for hw in &hallucination_words {
        if lower.contains(hw) && !ctx_lower.contains(hw) && !mem_lower.contains(hw) {
            return Ok(None);
        }
    }

    // Reject messages that are too long (model rambling)
    if text.split_whitespace().count() > 40 {
        return Ok(None);
    }

    // Reject near-duplicate of recent messages (simple word overlap check)
    for (prev_msg, _) in todays_messages.iter().rev().take(10) {
        let prev_lower = prev_msg.to_lowercase();
        let prev_words: std::collections::HashSet<&str> = prev_lower.split_whitespace().collect();
        let new_words: std::collections::HashSet<&str> = lower.split_whitespace().collect();
        if prev_words.len() >= 3 && new_words.len() >= 3 {
            let overlap = prev_words.intersection(&new_words).count();
            let max_len = prev_words.len().max(new_words.len());
            if max_len > 0 && (overlap as f64 / max_len as f64) > 0.7 {
                return Ok(None); // >70% word overlap = too similar
            }
        }
    }

    Ok(Some(text))
}
