// commands_money.rs — Transactions, budgets, savings goals, subscriptions, debts
use crate::types::*;

// ── v0.8.0: Money commands ──

#[tauri::command]
pub fn add_transaction(
    date: Option<String>, transaction_type: String, amount: f64, currency: Option<String>,
    category: String, description: Option<String>, recurring: Option<bool>,
    recurring_period: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO transactions (date, type, amount, currency, category, description, recurring, recurring_period, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![d, transaction_type, amount, currency.unwrap_or_else(|| "KZT".into()),
            category, description.unwrap_or_default(), recurring.unwrap_or(false) as i32,
            recurring_period, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_transactions(month: Option<String>, transaction_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let prefix = month.unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    let pattern = format!("{}%", prefix);
    if let Some(t) = transaction_type {
        let mut stmt = conn.prepare(
            "SELECT id, date, type, amount, currency, category, description FROM transactions WHERE date LIKE ?1 AND type=?2 ORDER BY date DESC, created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern, t], |row| tx_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, date, type, amount, currency, category, description FROM transactions WHERE date LIKE ?1 ORDER BY date DESC, created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| tx_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn tx_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
        "type": row.get::<_, String>(2)?, "amount": row.get::<_, f64>(3)?,
        "currency": row.get::<_, String>(4)?, "category": row.get::<_, String>(5)?,
        "description": row.get::<_, String>(6)?,
    }))
}

#[tauri::command]
pub fn update_transaction(id: i64, amount: Option<f64>, category: Option<String>, description: Option<String>, tx_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = amount { updates.push(format!("amount=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = description { updates.push(format!("description=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = tx_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE transactions SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_transaction(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM transactions WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_transaction_stats(month: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let prefix = month.unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    let pattern = format!("{}%", prefix);
    let (total_expense, total_income): (f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN type='expense' THEN amount END), 0),
                COALESCE(SUM(CASE WHEN type='income' THEN amount END), 0)
         FROM transactions WHERE date LIKE ?1",
        rusqlite::params![pattern], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0.0, 0.0));
    // By category
    let mut stmt = conn.prepare(
        "SELECT category, SUM(amount) FROM transactions WHERE date LIKE ?1 AND type='expense' GROUP BY category ORDER BY SUM(amount) DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let by_cat: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({ "category": row.get::<_, String>(0)?, "amount": row.get::<_, f64>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!({ "total_expense": total_expense, "total_income": total_income, "balance": total_income - total_expense, "by_category": by_cat }))
}

#[tauri::command]
pub fn create_budget(category: String, amount: f64, period: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    let p = period.unwrap_or_else(|| "monthly".into());
    conn.execute(
        "INSERT INTO budgets (category, amount, period, created_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(category, period) DO UPDATE SET amount=?2",
        rusqlite::params![category, amount, p, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_budgets(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let month = chrono::Local::now().format("%Y-%m").to_string();
    let pattern = format!("{}%", month);
    let mut stmt = conn.prepare(
        "SELECT b.id, b.category, b.amount, b.period,
                COALESCE((SELECT SUM(amount) FROM transactions WHERE category=b.category AND type='expense' AND date LIKE ?1), 0) as spent
         FROM budgets b ORDER BY b.category"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "category": row.get::<_, String>(1)?,
            "amount": row.get::<_, f64>(2)?, "period": row.get::<_, String>(3)?,
            "spent": row.get::<_, f64>(4)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_budget(id: i64, category: Option<String>, amount: Option<f64>, period: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = amount { updates.push(format!("amount=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = period { updates.push(format!("period=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE budgets SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_budget(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM budgets WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn create_savings_goal(name: String, target_amount: f64, deadline: Option<String>, color: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO savings_goals (name, target_amount, deadline, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, target_amount, deadline, color.unwrap_or_else(|| "#818cf8".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_savings_goals(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, target_amount, current_amount, deadline, color FROM savings_goals ORDER BY created_at DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        let target: f64 = row.get(2)?;
        let current: f64 = row.get(3)?;
        let pct = if target > 0.0 { (current / target * 100.0).min(100.0) } else { 0.0 };
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "target_amount": target, "current_amount": current,
            "deadline": row.get::<_, Option<String>>(4)?, "color": row.get::<_, String>(5)?,
            "percent": format!("{:.0}", pct),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_savings_goal(id: i64, add_amount: Option<f64>, target_amount: Option<f64>, name: Option<String>, deadline: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(add) = add_amount {
        conn.execute("UPDATE savings_goals SET current_amount = current_amount + ?1 WHERE id=?2", rusqlite::params![add, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(target) = target_amount {
        conn.execute("UPDATE savings_goals SET target_amount=?1 WHERE id=?2", rusqlite::params![target, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(v) = name {
        conn.execute("UPDATE savings_goals SET name=?1 WHERE id=?2", rusqlite::params![v, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(v) = deadline {
        conn.execute("UPDATE savings_goals SET deadline=?1 WHERE id=?2", rusqlite::params![v, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_savings_goal(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM savings_goals WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn add_subscription(name: String, amount: f64, currency: Option<String>, period: Option<String>, next_payment: Option<String>, category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO subscriptions (name, amount, currency, period, next_payment, category, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, amount, currency.unwrap_or_else(|| "KZT".into()), period.unwrap_or_else(|| "monthly".into()),
            next_payment, category.unwrap_or_else(|| "other".into()), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_subscriptions(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, currency, period, next_payment, category, active FROM subscriptions ORDER BY active DESC, name"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "amount": row.get::<_, f64>(2)?, "currency": row.get::<_, String>(3)?,
            "period": row.get::<_, String>(4)?, "next_payment": row.get::<_, Option<String>>(5)?,
            "category": row.get::<_, String>(6)?, "active": row.get::<_, i32>(7)? != 0,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_subscription(id: i64, active: Option<bool>, amount: Option<f64>, name: Option<String>, period: Option<String>, category: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = active { updates.push(format!("active=?{}", idx)); params.push(Box::new(v as i32)); idx += 1; }
    if let Some(v) = amount { updates.push(format!("amount=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = period { updates.push(format!("period=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE subscriptions SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_subscription(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM subscriptions WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn add_debt(name: String, debt_type: String, amount: f64, interest_rate: Option<f64>, due_date: Option<String>, description: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO debts (name, type, amount, remaining, interest_rate, due_date, description, created_at) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, debt_type, amount, interest_rate.unwrap_or(0.0), due_date, description.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_debts(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, name, type, amount, remaining, interest_rate, due_date, description FROM debts WHERE remaining > 0 ORDER BY due_date NULLS LAST"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        let amt: f64 = row.get(3)?;
        let rem: f64 = row.get(4)?;
        let pct = if amt > 0.0 { ((amt - rem) / amt * 100.0).min(100.0) } else { 0.0 };
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2)?, "amount": amt, "remaining": rem,
            "interest_rate": row.get::<_, f64>(5)?, "due_date": row.get::<_, Option<String>>(6)?,
            "description": row.get::<_, String>(7)?, "paid_percent": format!("{:.0}", pct),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn update_debt(id: i64, pay_amount: Option<f64>, name: Option<String>, remaining: Option<f64>, debt_type: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    if let Some(pay) = pay_amount {
        conn.execute("UPDATE debts SET remaining = MAX(0, remaining - ?1) WHERE id=?2", rusqlite::params![pay, id])
            .map_err(|e| format!("DB error: {}", e))?;
    }
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = remaining { updates.push(format!("remaining=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = debt_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE debts SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_debt(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM debts WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}
