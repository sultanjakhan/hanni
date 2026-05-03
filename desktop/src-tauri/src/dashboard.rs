// dashboard.rs — Configurable dashboard widget CRUD + default seeds
use crate::types::*;

#[tauri::command]
pub fn get_dashboard_widgets(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, widget_type, position, config FROM dashboard_widgets WHERE tab_id = ?1 ORDER BY position"
    ).map_err(|e| format!("DB error: {e}"))?;
    let rows = stmt.query_map([&tab_id], |row| {
        let config_str: String = row.get(3)?;
        let config: serde_json::Value = serde_json::from_str(&config_str).unwrap_or(serde_json::json!({}));
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "widget_type": row.get::<_, String>(1)?,
            "position": row.get::<_, i64>(2)?,
            "config": config,
        }))
    }).map_err(|e| format!("Query error: {e}"))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn save_dashboard_widgets(tab_id: String, widgets: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let items: Vec<serde_json::Value> = serde_json::from_str(&widgets).map_err(|e| format!("JSON error: {e}"))?;
    let conn = db.conn();
    conn.execute("DELETE FROM dashboard_widgets WHERE tab_id = ?1", [&tab_id])
        .map_err(|e| format!("DB error: {e}"))?;
    for (i, item) in items.iter().enumerate() {
        let wtype = item["widget_type"].as_str().unwrap_or("stat");
        let config = serde_json::to_string(&item["config"]).unwrap_or_default();
        conn.execute(
            "INSERT INTO dashboard_widgets (tab_id, widget_type, position, config) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![tab_id, wtype, i as i64, config],
        ).map_err(|e| format!("Insert error: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn seed_dashboard_defaults(tab_id: String, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM dashboard_widgets WHERE tab_id = ?1", [&tab_id], |r| r.get(0)
    ).unwrap_or(0);
    if count > 0 { return Ok(()); }

    let defaults = get_default_widgets(&tab_id);
    for (i, item) in defaults.iter().enumerate() {
        let wtype = item["widget_type"].as_str().unwrap_or("stat");
        let config = serde_json::to_string(&item["config"]).unwrap_or_default();
        conn.execute(
            "INSERT INTO dashboard_widgets (tab_id, widget_type, position, config) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![tab_id, wtype, i as i64, config],
        ).map_err(|e| format!("Seed error: {e}"))?;
    }
    Ok(())
}

fn stat(label: &str, color: &str, cmd: &str, path: &str) -> serde_json::Value {
    serde_json::json!({"widget_type": "stat", "config": {"label": label, "color": color, "command": cmd, "commandArgs": {}, "valuePath": path, "emptyValue": "0"}})
}

fn stat_args(label: &str, color: &str, cmd: &str, args: serde_json::Value, path: &str) -> serde_json::Value {
    serde_json::json!({"widget_type": "stat", "config": {"label": label, "color": color, "command": cmd, "commandArgs": args, "valuePath": path, "emptyValue": "0"}})
}

fn interactive(label: &str, color: &str, suffix: &str, cmd: &str, path: &str, prompt: &str, act_cmd: &str, htype: &str) -> serde_json::Value {
    serde_json::json!({"widget_type": "interactive", "config": {
        "label": label, "color": color, "suffix": suffix, "command": cmd, "commandArgs": {},
        "valuePath": path, "emptyValue": "—",
        "action": {"prompt": prompt, "command": act_cmd, "commandArgs": {"healthType": htype, "notes": null}, "valueParam": "value", "valueType": "float"}
    }})
}

fn get_default_widgets(tab_id: &str) -> Vec<serde_json::Value> {
    match tab_id {
        "jobs" => vec![
            stat("Вакансий", "blue", "get_job_stats", "total"),
            stat("Откликов", "green", "get_job_stats", "by_stage.applied"),
            stat("Интервью", "yellow", "get_job_stats", "by_stage.interview"),
            stat("За неделю", "purple", "get_job_stats", "applied_this_week"),
        ],
        "health" => vec![
            interactive("Сон", "blue", "ч", "get_health_today", "sleep", "Сон (часы):", "log_health", "sleep"),
            interactive("Вода", "green", "", "get_health_today", "water", "Вода (стаканы):", "log_health", "water"),
            interactive("Настроение", "yellow", "/5", "get_health_today", "mood", "Настроение (1-5):", "log_health", "mood"),
            interactive("Вес", "purple", "кг", "get_health_today", "weight", "Вес (кг):", "log_health", "weight"),
        ],
        "food" => vec![
            stat_args("Калории", "blue", "get_food_stats", serde_json::json!({"days": 1}), "avg_calories"),
            stat_args("Белок", "green", "get_food_stats", serde_json::json!({"days": 1}), "avg_protein"),
            stat_args("Углеводы", "yellow", "get_food_stats", serde_json::json!({"days": 1}), "avg_carbs"),
            stat_args("Жиры", "purple", "get_food_stats", serde_json::json!({"days": 1}), "avg_fat"),
        ],
        "money" => vec![
            stat_args("Баланс", "blue", "get_transaction_stats", serde_json::json!({"month": null}), "balance"),
            stat_args("Расходы", "red", "get_transaction_stats", serde_json::json!({"month": null}), "total_expense"),
            stat_args("Доходы", "green", "get_transaction_stats", serde_json::json!({"month": null}), "total_income"),
            stat("Подписки", "purple", "get_subscriptions", "_count"),
        ],
        "sports" => vec![
            stat("Тренировок", "blue", "get_workout_stats", "count"),
            stat("Минут", "green", "get_workout_stats", "total_minutes"),
            stat("Калорий", "yellow", "get_workout_stats", "total_calories"),
        ],
        "home" => vec![
            stat_args("Всего вещей", "blue", "get_home_items", serde_json::json!({"category": null, "neededOnly": false}), "_count"),
            stat_args("Нужно купить", "yellow", "get_home_items", serde_json::json!({"category": null, "neededOnly": true}), "_count"),
        ],
        "development" => vec![
            stat_args("Всего", "blue", "get_learning_items", serde_json::json!({"typeFilter": null}), "_count"),
            stat_args("В процессе", "green", "get_learning_items", serde_json::json!({"typeFilter": null}), "_filter:status=in_progress"),
            stat_args("Завершено", "yellow", "get_learning_items", serde_json::json!({"typeFilter": null}), "_filter:status=completed"),
        ],
        "people" => vec![
            stat_args("Контактов", "blue", "get_contacts", serde_json::json!({"category": null, "blocked": null}), "_count"),
            stat_args("Избранных", "green", "get_contacts", serde_json::json!({"category": null, "blocked": null}), "_filter:favorite=true"),
        ],
        "schedule" => vec![
            stat("Активных", "blue", "get_schedule_stats", "total_active"),
            stat("Сегодня", "green", "get_schedule_stats", "completed_today"),
            stat("За неделю", "yellow", "get_schedule_stats", "completed_week"),
        ],
        "dankoe" => vec![
            stat("Серия", "blue", "get_dan_koe_stats", "streak"),
            stat("Полных дней", "green", "get_dan_koe_stats", "week_complete"),
            stat("За неделю", "yellow", "get_dan_koe_stats", "week_entries"),
        ],
        _ => vec![],
    }
}
