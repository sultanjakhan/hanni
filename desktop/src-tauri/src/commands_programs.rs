// commands_programs.rs — Multi-day workout programs (monthly / split / muscle-focus / warmup).
// A program references existing workout_templates per cycle day; a run tracks progress.
// Starting a program-day reuses make_workout_from_template, so it logs like a manual start.
use crate::types::HanniDb;

fn insert_days(conn: &rusqlite::Connection, program_id: i64, days: &[serde_json::Value]) {
    for (i, d) in days.iter().enumerate() {
        let label = d.get("label").and_then(|v| v.as_str()).unwrap_or("");
        let tid = d.get("template_id").and_then(|v| v.as_i64());
        let is_rest = d.get("is_rest").and_then(|v| v.as_bool()).unwrap_or(false) as i64;
        let notes = d.get("notes").and_then(|v| v.as_str()).unwrap_or("");
        let day_index = d.get("day_index").and_then(|v| v.as_i64()).unwrap_or(i as i64);
        let order_index = d.get("order_index").and_then(|v| v.as_i64()).unwrap_or(i as i64);
        conn.execute(
            "INSERT INTO program_days (program_id, day_index, label, template_id, is_rest, notes, order_index) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![program_id, day_index, label, tid, is_rest, notes, order_index],
        ).ok();
    }
}

fn program_list_row(r: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": r.get::<_,i64>(0)?, "name": r.get::<_,String>(1)?, "kind": r.get::<_,String>(2)?,
        "cycle_length_days": r.get::<_,i64>(3)?, "duration_weeks": r.get::<_,i64>(4)?,
        "target_muscle_groups": r.get::<_,String>(5)?, "favorite": r.get::<_,i64>(6)?,
        "active": r.get::<_,i64>(7)?, "notes": r.get::<_,String>(8)?, "day_count": r.get::<_,i64>(9)?,
    }))
}

#[tauri::command]
pub fn create_workout_program(
    name: String, kind: Option<String>, cycle_length_days: Option<i64>, duration_weeks: Option<i64>,
    target_muscle_groups: Option<String>, notes: Option<String>, days: Option<Vec<serde_json::Value>>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO workout_programs (name, kind, cycle_length_days, duration_weeks, target_muscle_groups, notes, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
        rusqlite::params![name, kind.unwrap_or_else(|| "custom".into()), cycle_length_days.unwrap_or(7),
            duration_weeks.unwrap_or(0), target_muscle_groups.unwrap_or_default(), notes.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    let pid = conn.last_insert_rowid();
    if let Some(ds) = days { insert_days(&conn, pid, &ds); }
    Ok(pid)
}

#[tauri::command]
pub fn update_workout_program(
    id: i64, name: Option<String>, kind: Option<String>, cycle_length_days: Option<i64>,
    duration_weeks: Option<i64>, target_muscle_groups: Option<String>, notes: Option<String>,
    days: Option<Vec<serde_json::Value>>, db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let mut sets = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { sets.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = kind { sets.push(format!("kind=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = cycle_length_days { sets.push(format!("cycle_length_days=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = duration_weeks { sets.push(format!("duration_weeks=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = target_muscle_groups { sets.push(format!("target_muscle_groups=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = notes { sets.push(format!("notes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    sets.push(format!("updated_at=?{}", idx)); params.push(Box::new(chrono::Local::now().to_rfc3339())); idx += 1;
    params.push(Box::new(id));
    let sql = format!("UPDATE workout_programs SET {} WHERE id=?{}", sets.join(","), idx);
    let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    if let Some(ds) = days {
        conn.execute("DELETE FROM program_days WHERE program_id=?1", rusqlite::params![id]).ok();
        insert_days(&conn, id, &ds);
    }
    Ok(())
}

#[tauri::command]
pub fn get_workout_programs(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let base = "SELECT p.id, p.name, p.kind, p.cycle_length_days, p.duration_weeks, p.target_muscle_groups, p.favorite, p.active, p.notes, COUNT(d.id) AS day_count FROM workout_programs p LEFT JOIN program_days d ON d.program_id=p.id";
    let sql = if search.is_some() {
        format!("{} WHERE p.name LIKE ?1 GROUP BY p.id ORDER BY p.updated_at DESC LIMIT 50", base)
    } else {
        format!("{} GROUP BY p.id ORDER BY p.updated_at DESC LIMIT 50", base)
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = if let Some(q) = search {
        let like = format!("%{}%", q);
        stmt.query_map(rusqlite::params![like], program_list_row).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect()
    } else {
        stmt.query_map([], program_list_row).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect()
    };
    Ok(rows)
}

#[tauri::command]
pub fn get_workout_program(id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut prog = conn.query_row(
        "SELECT id, name, kind, cycle_length_days, duration_weeks, target_muscle_groups, favorite, active, notes, created_at, updated_at FROM workout_programs WHERE id=?1",
        rusqlite::params![id],
        |r| Ok(serde_json::json!({
            "id": r.get::<_,i64>(0)?, "name": r.get::<_,String>(1)?, "kind": r.get::<_,String>(2)?,
            "cycle_length_days": r.get::<_,i64>(3)?, "duration_weeks": r.get::<_,i64>(4)?,
            "target_muscle_groups": r.get::<_,String>(5)?, "favorite": r.get::<_,i64>(6)?,
            "active": r.get::<_,i64>(7)?, "notes": r.get::<_,String>(8)?,
            "created_at": r.get::<_,String>(9)?, "updated_at": r.get::<_,String>(10)?,
        })),
    ).map_err(|e| format!("Program not found: {}", e))?;
    let mut dstmt = conn.prepare(
        "SELECT pd.id, pd.day_index, pd.label, pd.template_id, pd.is_rest, pd.notes, pd.order_index, t.name, t.type FROM program_days pd LEFT JOIN workout_templates t ON t.id=pd.template_id WHERE pd.program_id=?1 ORDER BY pd.day_index, pd.order_index"
    ).map_err(|e| format!("DB error: {}", e))?;
    let days: Vec<serde_json::Value> = dstmt.query_map(rusqlite::params![id], |r| Ok(serde_json::json!({
        "id": r.get::<_,i64>(0)?, "day_index": r.get::<_,i64>(1)?, "label": r.get::<_,String>(2)?,
        "template_id": r.get::<_,Option<i64>>(3)?, "is_rest": r.get::<_,i64>(4)?, "notes": r.get::<_,String>(5)?,
        "order_index": r.get::<_,i64>(6)?, "template_name": r.get::<_,Option<String>>(7)?, "template_type": r.get::<_,Option<String>>(8)?,
    }))).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    let mut vstmt = conn.prepare(
        "SELECT COALESCE(ec.muscle_group,'other') AS mg, COALESCE(SUM(te.sets),0) AS vol FROM program_days pd JOIN template_exercises te ON te.template_id=pd.template_id LEFT JOIN exercise_catalog ec ON ec.id=te.exercise_catalog_id WHERE pd.program_id=?1 AND pd.is_rest=0 GROUP BY mg"
    ).map_err(|e| format!("DB error: {}", e))?;
    let mut vol = serde_json::Map::new();
    let vrows = vstmt.query_map(rusqlite::params![id], |r| Ok((r.get::<_,String>(0)?, r.get::<_,i64>(1)?)))
        .map_err(|e| format!("Query error: {}", e))?;
    for row in vrows.filter_map(|r| r.ok()) { vol.insert(row.0, serde_json::json!(row.1)); }
    let run = conn.query_row(
        "SELECT id, started_at, current_day, status, completed_days FROM program_runs WHERE program_id=?1 AND status='active' ORDER BY id DESC LIMIT 1",
        rusqlite::params![id],
        |r| Ok(serde_json::json!({"id": r.get::<_,i64>(0)?, "started_at": r.get::<_,String>(1)?, "current_day": r.get::<_,i64>(2)?, "status": r.get::<_,String>(3)?, "completed_days": r.get::<_,i64>(4)?})),
    ).ok();
    let o = prog.as_object_mut().unwrap();
    o.insert("days".into(), serde_json::json!(days));
    o.insert("muscle_volume".into(), serde_json::Value::Object(vol));
    o.insert("run".into(), run.unwrap_or(serde_json::Value::Null));
    Ok(prog)
}

#[tauri::command]
pub fn delete_workout_program(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM program_days WHERE program_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM program_runs WHERE program_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM workout_programs WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite_program(id: i64, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let cur: i64 = conn.query_row("SELECT favorite FROM workout_programs WHERE id=?1", rusqlite::params![id], |r| r.get(0)).unwrap_or(0);
    let next = if cur == 0 { 1 } else { 0 };
    conn.execute("UPDATE workout_programs SET favorite=?1 WHERE id=?2", rusqlite::params![next, id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(next)
}

#[tauri::command]
pub fn start_program(program_id: i64, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute("UPDATE program_runs SET status='paused' WHERE status='active'", []).ok();
    conn.execute("UPDATE workout_programs SET active=0 WHERE active=1", []).ok();
    conn.execute("UPDATE workout_programs SET active=1 WHERE id=?1", rusqlite::params![program_id]).ok();
    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO program_runs (program_id, started_at, current_day, status, completed_days) VALUES (?1,?2,0,'active',0)",
        rusqlite::params![program_id, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn stop_program(run_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("UPDATE workout_programs SET active=0 WHERE id=(SELECT program_id FROM program_runs WHERE id=?1)", rusqlite::params![run_id]).ok();
    conn.execute("UPDATE program_runs SET status='paused' WHERE id=?1", rusqlite::params![run_id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn complete_program_day(run_id: i64, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let (program_id, current_day): (i64, i64) = conn.query_row(
        "SELECT program_id, current_day FROM program_runs WHERE id=?1",
        rusqlite::params![run_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| format!("Run not found: {}", e))?;
    let (cycle, dur_weeks): (i64, i64) = conn.query_row(
        "SELECT cycle_length_days, duration_weeks FROM workout_programs WHERE id=?1",
        rusqlite::params![program_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| format!("DB error: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT template_id FROM program_days WHERE program_id=?1 AND day_index=?2 AND is_rest=0 AND template_id IS NOT NULL ORDER BY order_index"
    ).map_err(|e| format!("DB error: {}", e))?;
    let tids: Vec<i64> = stmt.query_map(rusqlite::params![program_id, current_day], |r| r.get::<_,i64>(0))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    let mut logged = 0i64;
    for tid in &tids {
        if crate::commands_data::make_workout_from_template(&conn, *tid).is_ok() { logged += 1; }
    }
    let cycle = if cycle > 0 { cycle } else { 1 };
    let next = (current_day + 1) % cycle;
    conn.execute("UPDATE program_runs SET current_day=?1, completed_days=completed_days+1 WHERE id=?2",
        rusqlite::params![next, run_id]).ok();
    if dur_weeks > 0 {
        conn.execute("UPDATE program_runs SET status='done', finished_at=?1 WHERE id=?2 AND completed_days>=?3",
            rusqlite::params![chrono::Local::now().to_rfc3339(), run_id, dur_weeks * cycle]).ok();
    }
    Ok(logged)
}

#[tauri::command]
pub fn get_today_program_workout(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let run = conn.query_row(
        "SELECT r.id, r.program_id, r.current_day, r.completed_days, p.name, p.kind, p.cycle_length_days, p.duration_weeks FROM program_runs r JOIN workout_programs p ON p.id=r.program_id WHERE r.status='active' ORDER BY r.id DESC LIMIT 1",
        [],
        |r| Ok((r.get::<_,i64>(0)?, r.get::<_,i64>(1)?, r.get::<_,i64>(2)?, r.get::<_,i64>(3)?, r.get::<_,String>(4)?, r.get::<_,String>(5)?, r.get::<_,i64>(6)?, r.get::<_,i64>(7)?)),
    ).ok();
    let (run_id, program_id, current_day, completed, pname, kind, cycle, dur_weeks) = match run {
        Some(t) => t,
        None => return Ok(serde_json::Value::Null),
    };
    let mut stmt = conn.prepare(
        "SELECT pd.id, pd.label, pd.template_id, pd.is_rest, t.name, t.type FROM program_days pd LEFT JOIN workout_templates t ON t.id=pd.template_id WHERE pd.program_id=?1 AND pd.day_index=?2 ORDER BY pd.order_index"
    ).map_err(|e| format!("DB error: {}", e))?;
    let days: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![program_id, current_day], |r| Ok(serde_json::json!({
        "id": r.get::<_,i64>(0)?, "label": r.get::<_,String>(1)?, "template_id": r.get::<_,Option<i64>>(2)?,
        "is_rest": r.get::<_,i64>(3)?, "template_name": r.get::<_,Option<String>>(4)?, "template_type": r.get::<_,Option<String>>(5)?,
    }))).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!({
        "run_id": run_id, "program_id": program_id, "program_name": pname, "kind": kind,
        "current_day": current_day, "completed_days": completed, "cycle_length_days": cycle, "duration_weeks": dur_weeks, "days": days,
    }))
}
