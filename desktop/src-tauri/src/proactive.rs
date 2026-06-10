// proactive.rs — Proactive messaging logic
use crate::types::*;
use crate::memory_embed::{build_memory_context_from_db, save_proactive_settings};
use crate::macos::{get_macos_idle_seconds, is_screen_locked};
use crate::voice::speak_tts;
use tauri::{AppHandle, Emitter, Manager};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use chrono::Timelike;
use crate::proactive_context::*;

// ── Proactive messaging logic ──

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

pub async fn gather_context() -> String {
    // All context functions are internally blocking (run_osascript, rusqlite).
    // Run in spawn_blocking to avoid starving tokio worker threads.
    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::task::spawn_blocking(gather_context_blocking),
    ).await {
        Ok(Ok(ctx)) => ctx,
        _ => format!("Current time: {}\n", chrono::Local::now().format("%H:%M %A, %d %B %Y")),
    }
}

// ── Proactive messaging commands ──
// ── Proactive messaging commands ──

#[tauri::command]
pub async fn get_proactive_settings(state: tauri::State<'_, Arc<Mutex<ProactiveState>>>) -> Result<ProactiveSettings, String> {
    let state = state.lock().await;
    Ok(state.settings.clone())
}

#[tauri::command]
pub async fn set_proactive_settings(
    settings: ProactiveSettings,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    save_proactive_settings(&settings)?;
    let mut state = state.lock().await;
    state.settings = settings;
    Ok(())
}

#[tauri::command]
pub async fn set_user_typing(
    typing: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.user_is_typing = typing;
    Ok(())
}

#[tauri::command]
pub async fn set_recording_state(
    recording: bool,
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    state.lock().await.is_recording = recording;
    Ok(())
}

#[tauri::command]
pub async fn report_proactive_engagement(
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let mut pstate = state.lock().await;
    // Mark the last proactive message as replied
    if let Some(pid) = pstate.last_proactive_id {
        {
                let conn = db.conn();
            let delay = pstate.last_message_time
                .map(|t| (chrono::Local::now() - t).num_seconds())
                .unwrap_or(0);
            let _ = conn.execute(
                "UPDATE proactive_history SET user_replied = 1, reply_delay_secs = ?1 WHERE id = ?2",
                rusqlite::params![delay, pid],
            );
        }
    }
    // Recompute engagement rate: rolling avg of last 20 proactive messages
    {
                let conn = db.conn();
        let rate: f64 = conn.query_row(
            "SELECT COALESCE(AVG(CAST(user_replied AS REAL)), 0.5) FROM (SELECT user_replied FROM proactive_history ORDER BY id DESC LIMIT 20)",
            [], |row| row.get(0),
        ).unwrap_or(0.5);
        pstate.engagement_rate = rate;
    }
    Ok(())
}

#[tauri::command]
pub async fn report_user_chat_activity(
    state: tauri::State<'_, Arc<Mutex<ProactiveState>>>,
) -> Result<(), String> {
    let mut pstate = state.lock().await;
    pstate.last_user_chat_time = Some(chrono::Local::now());
    Ok(())
}

#[tauri::command]
pub fn rate_proactive(db: tauri::State<'_, HanniDb>, proactive_id: i64, rating: i64) -> Result<(), String> {
    db.conn().execute(
        "UPDATE proactive_history SET rating = ?1 WHERE id = ?2",
        rusqlite::params![rating, proactive_id],
    ).map_err(|e| format!("DB: {}", e))?;
    Ok(())
}

// ── Proactive background loop ──

pub async fn proactive_loop(proactive_handle: AppHandle, proactive_state_ref: Arc<Mutex<ProactiveState>>) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // Initial delay — let the app fully start
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Load recent proactive messages from DB (survives app restart)
    let loaded_msgs: Vec<(String, chrono::DateTime<chrono::Local>)> = {
        let db = proactive_handle.state::<HanniDb>();
        let conn = db.conn();
        let mut result = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT message, sent_at FROM proactive_history ORDER BY id DESC LIMIT 15"
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                let msg: String = row.get(0)?;
                let ts_str: String = row.get(1)?;
                Ok((msg, ts_str))
            }) {
                for row in rows.flatten() {
                    let ts = chrono::DateTime::parse_from_rfc3339(&row.1)
                        .map(|dt| dt.with_timezone(&chrono::Local))
                        .unwrap_or_else(|_| chrono::Local::now());
                    result.push((row.0, ts));
                }
            }
        }
        result.reverse(); // oldest first
        result
    };
    if !loaded_msgs.is_empty() {
        let mut state = proactive_state_ref.lock().await;
        state.recent_messages = loaded_msgs;
    }

    // Compute initial engagement rate from DB history (last 20 messages)
    let initial_engagement = {
        let db = proactive_handle.state::<HanniDb>();
        let conn = db.conn();
        let replied: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (SELECT user_replied FROM proactive_history ORDER BY id DESC LIMIT 20) WHERE user_replied=1",
            [], |row| row.get(0),
        ).unwrap_or(0);
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM (SELECT id FROM proactive_history ORDER BY id DESC LIMIT 20)",
            [], |row| row.get(0),
        ).unwrap_or(0);
        if total > 0 { Some(replied as f64 / total as f64) } else { None }
    }; // conn dropped here before await
    if let Some(eng) = initial_engagement {
        let mut state = proactive_state_ref.lock().await;
        state.engagement_rate = eng;
    }

    let mut last_check = std::time::Instant::now();
    let mut first_run = true;
    let mut prev_auto_quiet = false;
    let mut wake_up_pending = false;

    loop {
        // Poll every 5 seconds so we react quickly to settings changes
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let (enabled, interval, quiet_start_min, quiet_end_min, is_typing, is_recording, skips, voice_enabled, voice_name, recent_msgs, last_ctx, engagement, triggers, last_user_chat, enabled_styles, daily_limit) = {
            let state = proactive_state_ref.lock().await;
            (
                state.settings.enabled,
                state.settings.interval_minutes,
                state.settings.quiet_start_minutes(),
                state.settings.quiet_end_minutes(),
                state.user_is_typing,
                state.is_recording,
                state.consecutive_skips,
                state.settings.voice_enabled,
                state.settings.voice_name.clone(),
                state.recent_messages.clone(),
                state.last_context_snapshot.clone(),
                state.engagement_rate,
                state.pending_triggers.iter()
                    .filter(|(_, created)| created.elapsed().as_secs() < 600)
                    .map(|(t, _)| t.clone())
                    .collect::<Vec<String>>(),
                state.last_user_chat_time,
                state.settings.enabled_styles.clone(),
                state.settings.daily_limit,
            )
        };

        if !enabled {
            first_run = true;
            continue;
        }

        // Skip proactive when OpenClaw is active — it has its own proactive system
        let openclaw_active: bool = {
            let db = proactive_handle.state::<HanniDb>();
            let conn = db.conn();
            conn.query_row(
                "SELECT value FROM app_settings WHERE key='use_openclaw'",
                [], |row| row.get::<_, String>(0),
            ).map(|v| v == "true" || v == "1").unwrap_or(false)
        };
        if openclaw_active {
            continue;
        }

        // ── Smart Adaptive Timing (Step 8) ──
        let base_interval_secs = interval * 60;
        let elapsed = last_check.elapsed().as_secs();
        let time_ratio = elapsed as f64 / base_interval_secs as f64;

        if !first_run {
            // Compute firing score
            let mut score: f64 = 0.0;

            // time_ratio: 0→0, 1→0.3, 2→0.6
            score += (time_ratio * 0.3).min(0.6);

            // upcoming event trigger
            if triggers.iter().any(|t| t.contains("событие")) {
                score += 0.3;
            }

            // distraction trigger
            if triggers.iter().any(|t| t.contains("Дистракция")) {
                score += 0.25;
            }

            // pending trigger (generic — includes smart triggers from DB)
            if !triggers.is_empty() {
                score += 0.4_f64.min(score + 0.4) - score; // ensure at least +0.15
                score += 0.15;
            }

            // Smart triggers from DB (overdue tasks, goals) — checked before LLM call
            // These are lower priority than real-time triggers but still boost score
            // We check DB triggers at LLM call time, but hint the score here based on time
            let hour = chrono::Local::now().hour();
            // Morning & evening are prime times for smart triggers
            if (8..=10).contains(&hour) || (20..=22).contains(&hour) {
                // Will be populated with smart_triggers later; give a small bonus
                // to increase chance of firing during ritual times
            }

            // Block proactive while user is actively chatting
            if let Some(last_chat) = last_user_chat {
                let chat_idle_min = (chrono::Local::now() - last_chat).num_minutes();
                if chat_idle_min < 5 {
                    // User chatted in last 5 min — suppress proactive completely
                    continue;
                }
                if chat_idle_min > 30 { score += 0.1; }
            } else {
                score += 0.1; // no chat at all — consider idle
            }

            // Engagement-adaptive: high engagement → slightly more proactive
            if engagement > 0.6 { score += 0.1; }
            if engagement > 0.8 { score += 0.05; }
            // Low engagement → harder to fire (but only with enough history)
            let total_msgs = {
                let db = proactive_handle.state::<HanniDb>();
                let conn = db.conn();
                conn.query_row("SELECT COUNT(*) FROM proactive_history", [], |r| r.get::<_, i64>(0)).unwrap_or(0)
            };
            if total_msgs > 10 {
                if engagement < 0.2 { score -= 0.2; }
                else if engagement < 0.4 { score -= 0.1; }
            }

            // deep work hours penalty (10-12, 14-17)
            let hour = chrono::Local::now().hour();
            if (10..=12).contains(&hour) || (14..=17).contains(&hour) {
                score -= 0.1;
            }

            // Morning/evening bonus (ritual times)
            if (8..=10).contains(&hour) || (21..=23).contains(&hour) {
                score += 0.1;
            }

            // many skips penalty
            if skips > 3 { score -= 0.15; }
            if skips > 6 { score -= 0.15; } // progressive backoff

            // Minimum floor: 3 minutes
            if elapsed < 180 {
                score = 0.0;
            }

            if score < 0.5 {
                continue;
            }
        }

        let now_t = chrono::Local::now();
        let now_min = now_t.hour() * 60 + now_t.minute();
        let in_quiet = if quiet_start_min > quiet_end_min {
            // Wraps midnight: e.g. 23:30 → 08:00
            now_min >= quiet_start_min || now_min < quiet_end_min
        } else {
            now_min >= quiet_start_min && now_min < quiet_end_min
        };

        let llm_busy = proactive_handle.state::<LlmBusy>().0.available_permits() == 0;
        let call_active = {
            let cm = proactive_handle.state::<Arc<CallMode>>();
            let guard = cm.0.lock().unwrap_or_else(|e| e.into_inner());
            guard.active
        };
        let is_speaking_now = IS_SPEAKING.load(Ordering::Relaxed);

        // Auto-quiet: detect idle/sleep without fixed quiet hours
        let (idle_secs, screen_locked) = tokio::task::spawn_blocking(|| {
            (get_macos_idle_seconds(), is_screen_locked())
        }).await.unwrap_or((0.0, false));
        let idle_min = idle_secs / 60.0;
        let hour = now_t.hour();
        let is_night = hour >= 22 || hour < 8;
        let auto_quiet = (screen_locked && is_night)  // locked at night = sleeping
            || (idle_min > 15.0 && is_night)          // 15 min idle at night
            || idle_min > 30.0;                        // 30 min idle anytime = away

        // Emit event on state change
        if auto_quiet != prev_auto_quiet {
            let _ = proactive_handle.emit("proactive-auto-quiet", auto_quiet);
            // Update ProactiveState for external queries
            proactive_state_ref.lock().await.auto_quiet = auto_quiet;
            prev_auto_quiet = auto_quiet;
        }

        eprintln!("[proactive] gate: quiet={} auto={} typing={} rec={} llm={} call={} speak={} wake={} idle={:.0}s",
            in_quiet, auto_quiet, is_typing, is_recording, llm_busy, call_active, is_speaking_now, wake_up_pending, idle_secs);

        if in_quiet || auto_quiet {
            wake_up_pending = true;
            continue;
        }

        // Smooth wake-up: wait for user activity after quiet period
        if wake_up_pending {
            if idle_min > 5.0 {
                continue; // user not active yet
            }
            wake_up_pending = false;
        }

        if is_typing || llm_busy || call_active || is_speaking_now || is_recording {
            continue;
        }

        last_check = std::time::Instant::now();
        first_run = false;

        let context = gather_context().await;

        // Compute delta from last context
        let delta = if !last_ctx.is_empty() {
            compute_activity_delta(&last_ctx, &context)
        } else {
            String::new()
        };

        // Gather smart triggers from DB (overdue tasks, goals, health)
        let smart_triggers: Vec<String> = tokio::task::spawn_blocking(gather_smart_triggers)
            .await.unwrap_or_default();
        // Merge with pending triggers (event/distraction triggers from snapshot loop)
        let mut all_triggers = triggers.clone();
        for st in &smart_triggers {
            if !all_triggers.contains(st) {
                all_triggers.push(st.clone());
            }
        }

        // Build memory context (8 core facts — better personalization)
        let (mem_ctx, chat_snippet, user_name, todays_msgs, recent_styles) = {
            let db = proactive_handle.state::<HanniDb>();
            let conn = db.conn();
            // Pass current app as context hint for memory search
            let ctx_hint = context.lines()
                .find(|l| l.contains("Frontmost:"))
                .unwrap_or("")
                .to_string();
            (
                build_memory_context_from_db(&conn, &ctx_hint, 8, None),
                get_recent_chat_snippet(&conn, 4),
                get_user_name_from_memory(&conn),
                get_todays_proactive_messages(&conn),
                get_recent_proactive_styles(&conn, 3),
            )
        };

        // Daily limit check: skip if limit reached and no triggers
        if daily_limit > 0 && todays_msgs.len() as u32 >= daily_limit && triggers.is_empty() {
            eprintln!("[proactive] daily_count={} limit={} triggers=0 — skipping (daily limit reached)", todays_msgs.len(), daily_limit);
            continue;
        }
        eprintln!("[proactive] daily_count={} limit={} triggers={}", todays_msgs.len(), daily_limit, triggers.len());

        // Acquire LLM semaphore during proactive call to prevent concurrent MLX requests
        let proactive_sem = proactive_handle.state::<LlmBusy>();
        let _proactive_permit = match proactive_sem.0.try_acquire() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let proactive_result = proactive_llm_call(&client, &context, &recent_msgs, skips, &mem_ctx, &delta, &all_triggers, &chat_snippet, engagement, &user_name, &todays_msgs, &enabled_styles, &recent_styles).await;
        drop(_proactive_permit);

        // P4: Re-check typing after LLM call — discard proactive if user started chatting
        let typing_during_call = proactive_state_ref.lock().await.user_is_typing;

        match proactive_result {
            Ok(Some(raw_message)) if !typing_during_call => {
                // Parse [style:X] prefix if present
                let (style, message) = if raw_message.starts_with("[style:") {
                    if let Some(end) = raw_message.find(']') {
                        let s = raw_message[7..end].to_string();
                        let m = raw_message[end+1..].trim().to_string();
                        (s, if m.is_empty() { raw_message.clone() } else { m })
                    } else { (String::new(), raw_message) }
                } else { (String::new(), raw_message) };

                // Record in proactive_history (with style)
                let proactive_id = {
                    let db = proactive_handle.state::<HanniDb>();
                    let conn = db.conn();
                    let _ = conn.execute(
                        "INSERT INTO proactive_history (sent_at, message, style) VALUES (?1, ?2, ?3)",
                        rusqlite::params![chrono::Local::now().to_rfc3339(), &message, &style],
                    );
                    conn.last_insert_rowid()
                };

                // Emit as JSON {text, id} for frontend feedback buttons
                let _ = proactive_handle.emit("proactive-message", serde_json::json!({
                    "text": &message,
                    "id": proactive_id,
                }));
                // Voice: once per period (morning 8-11, day 12-19, evening 20-23)
                if voice_enabled {
                    let vh = chrono::Local::now().hour();
                    let period = match vh {
                        8..=11 => "morning",
                        12..=19 => "day",
                        20..=23 => "evening",
                        _ => "",
                    };
                    let mut state = proactive_state_ref.lock().await;
                    // Reset periods on new day
                    let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();
                    if state.voiced_periods_date != today_str {
                        state.voiced_periods_today.clear();
                        state.voiced_periods_date = today_str;
                    }
                    if !period.is_empty() && !state.voiced_periods_today.contains(&period.to_string()) {
                        state.voiced_periods_today.push(period.to_string());
                        drop(state);
                        speak_tts(&message, &voice_name);
                    } else {
                        drop(state);
                    }
                }

                let mut state = proactive_state_ref.lock().await;
                state.last_message_time = Some(chrono::Local::now());
                state.last_message_text = message.clone();
                state.consecutive_skips = 0;
                state.last_context_snapshot = context;
                state.last_proactive_id = Some(proactive_id);
                // Update recent_messages (keep last 15)
                state.recent_messages.push((message, chrono::Local::now()));
                if state.recent_messages.len() > 15 {
                    state.recent_messages.remove(0);
                }
                state.pending_triggers.clear();
            }
            // P4: User started typing during LLM call — discard message
            Ok(Some(_)) => {
                let mut state = proactive_state_ref.lock().await;
                state.consecutive_skips += 1;
                state.last_context_snapshot = context;
            }
            Ok(None) => {
                let mut state = proactive_state_ref.lock().await;
                state.consecutive_skips += 1;
                state.last_context_snapshot = context;
            }
            Err(e) => {
                // LLM error — retry once after 30s by setting short backoff
                eprintln!("[proactive] LLM error: {} — will retry sooner", e);
                let base_interval_secs = interval * 60;
                last_check = std::time::Instant::now() - std::time::Duration::from_secs(base_interval_secs.saturating_sub(30));
            }
        }
    }
}
