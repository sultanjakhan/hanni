// commands_sports.rs — Workouts, exercise catalog, workout templates
use crate::types::*;

// ── v0.7.0: Workouts (Sports) commands ──

#[tauri::command]
pub fn create_workout(workout_type: String, title: String, duration_minutes: i64, calories: Option<i64>, notes: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO workouts (type, title, date, duration_minutes, calories, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![workout_type, title, date, duration_minutes, calories, notes, now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_workouts(_date_range: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, type, title, date, duration_minutes, calories, notes FROM workouts ORDER BY date DESC, created_at DESC LIMIT 50"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "type": row.get::<_, String>(1)?,
            "title": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "duration_minutes": row.get::<_, i64>(4)?,
            "calories": row.get::<_, Option<i64>>(5)?,
            "notes": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn get_workout_stats(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let week_ago = (chrono::Local::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let (count, total_min, total_cal): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(duration_minutes), 0), COALESCE(SUM(calories), 0) FROM workouts WHERE date >= ?1",
        rusqlite::params![week_ago],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap_or((0, 0, 0));
    Ok(serde_json::json!({ "count": count, "total_minutes": total_min, "total_calories": total_cal }))
}

#[tauri::command]
pub fn delete_workout(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM workouts WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_workout(id: i64, title: Option<String>, workout_type: Option<String>, duration_minutes: Option<i64>, calories: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = title { updates.push(format!("title=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = workout_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = duration_minutes { updates.push(format!("duration_minutes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = calories { updates.push(format!("calories=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE workouts SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Exercise Catalog commands ──

#[tauri::command]
pub fn get_exercise_catalog(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(q) = search {
        let like = format!("%{}%", q);
        let mut stmt = conn.prepare(
            "SELECT id, name, muscle_group, equipment, type, description, difficulty, primary_muscles, secondary_muscles, category, force FROM exercise_catalog WHERE name LIKE ?1 ORDER BY name"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows = stmt.query_map(rusqlite::params![like], |row| exercise_catalog_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, name, muscle_group, equipment, type, description, difficulty, primary_muscles, secondary_muscles, category, force FROM exercise_catalog ORDER BY name"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows = stmt.query_map([], |row| exercise_catalog_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

fn exercise_catalog_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "muscle_group": row.get::<_, String>(2)?, "equipment": row.get::<_, String>(3)?,
        "type": row.get::<_, String>(4)?, "description": row.get::<_, String>(5)?,
        "difficulty": row.get::<_, String>(6).unwrap_or_default(),
        "primary_muscles": row.get::<_, String>(7).unwrap_or_default(),
        "secondary_muscles": row.get::<_, String>(8).unwrap_or_default(),
        "category": row.get::<_, String>(9).unwrap_or_default(),
        "force": row.get::<_, String>(10).unwrap_or_default(),
    }))
}

#[tauri::command]
pub fn add_exercise_to_catalog(
    name: String, muscle_group: Option<String>, equipment: Option<String>,
    exercise_type: Option<String>, description: Option<String>, difficulty: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT OR IGNORE INTO exercise_catalog (name, muscle_group, equipment, type, description, difficulty) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![name, muscle_group.unwrap_or_else(|| "full_body".into()),
            equipment.unwrap_or_default(), exercise_type.unwrap_or_else(|| "strength".into()),
            description.unwrap_or_default(), difficulty.unwrap_or_else(|| "medium".into())],
    ).map_err(|e| format!("DB error: {}", e))?;
    let id: i64 = conn.query_row("SELECT id FROM exercise_catalog WHERE name=?1 COLLATE NOCASE",
        rusqlite::params![name], |r| r.get(0)).map_err(|e| format!("DB error: {}", e))?;
    Ok(id)
}

#[tauri::command]
pub fn update_exercise_catalog(
    id: i64, name: Option<String>, muscle_group: Option<String>,
    equipment: Option<String>, exercise_type: Option<String>, description: Option<String>,
    difficulty: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = muscle_group { updates.push(format!("muscle_group=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = equipment { updates.push(format!("equipment=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = exercise_type { updates.push(format!("type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = description { updates.push(format!("description=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = difficulty { updates.push(format!("difficulty=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE exercise_catalog SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_exercise_catalog(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    db.conn().execute("DELETE FROM exercise_catalog WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// Distinct equipment + category values present in the catalog, so the UI builds
// its filter chips from the DB instead of a hardcoded list.
#[tauri::command]
pub fn get_exercise_facets(db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut eq_stmt = conn.prepare(
        "SELECT DISTINCT equipment FROM exercise_catalog WHERE equipment<>'' ORDER BY equipment"
    ).map_err(|e| format!("DB error: {}", e))?;
    let equipment: Vec<String> = eq_stmt.query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    let mut cat_stmt = conn.prepare(
        "SELECT DISTINCT category FROM exercise_catalog WHERE category<>'' ORDER BY category"
    ).map_err(|e| format!("DB error: {}", e))?;
    let categories: Vec<String> = cat_stmt.query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!({ "equipment": equipment, "categories": categories }))
}

// Map a clicked 3D body zone (GLB mesh name) to one of the catalog's 8 muscle
// groups. Substring match on the lowercased name; leg-specific names are tested
// before the bare "biceps" fallback so "Biceps Femoris" resolves to legs.
fn body_zone_to_muscle_group(zone: &str) -> Option<&'static str> {
    let z = zone.to_lowercase();
    if z.contains("biceps femoris") || z.contains("semitendinosus") || z.contains("semimembranosus")
        || z.contains("quadriceps") || z.contains("femoris") || z.contains("vastus")
        || z.contains("gluteus") || z.contains("gastrocnemius") || z.contains("soleus")
        || z.contains("tibialis") || z.contains("peroneus") || z.contains("fibularis")
        || z.contains("adductor") || z.contains("gracilis") || z.contains("pectineus")
        || z.contains("sartorius") || z.contains("iliacus") || z.contains("psoas")
        || z.contains("popliteus") || z.contains("tensor fasc") {
        return Some("legs");
    }
    if z.contains("pectoralis") { return Some("chest"); }
    if z.contains("deltoid") { return Some("shoulders"); }
    if z.contains("triceps") || z.contains("anconeus") { return Some("triceps"); }
    if z.contains("biceps") || z.contains("brachialis") || z.contains("brachioradialis") || z.contains("coracobrachialis") {
        return Some("biceps");
    }
    if z.contains("rectus abdominis") || z.contains("oblique") || z.contains("transvers") || z.contains("quadratus lumborum") {
        return Some("core");
    }
    if z.contains("latissimus") || z.contains("trapezius") || z.contains("rhomboid") || z.contains("teres")
        || z.contains("infraspinatus") || z.contains("supraspinatus") || z.contains("erector")
        || z.contains("serratus") || z.contains("subscapularis") || z.contains("levator scap") {
        return Some("back");
    }
    None
}

// Exercises for a clicked muscle zone. Pain recorded on the zone biases the list
// toward stretch/mobility; otherwise strength-first. Empty when the zone has no
// training mapping (face/hand/foot detail) — the UI shows a hint.
#[tauri::command]
pub fn get_exercises_by_body_zone(zone: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let group = match body_zone_to_muscle_group(&zone) { Some(g) => g, None => return Ok(vec![]) };
    let has_pain: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM body_records WHERE zone=?1 AND record_type='pain')",
        rusqlite::params![zone], |r| r.get(0),
    ).unwrap_or(false);
    let order = if has_pain {
        "CASE type WHEN 'stretch' THEN 0 WHEN 'cardio' THEN 1 WHEN 'bodyweight' THEN 2 ELSE 3 END, name"
    } else {
        "CASE type WHEN 'strength' THEN 0 WHEN 'bodyweight' THEN 1 ELSE 2 END, name"
    };
    let sql = format!(
        "SELECT id, name, muscle_group, equipment, type, description, difficulty, primary_muscles, secondary_muscles, category, force \
         FROM exercise_catalog WHERE muscle_group=?1 ORDER BY {} LIMIT 50", order);
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![group], |row| exercise_catalog_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── Workout Templates commands ──

#[tauri::command]
pub fn create_workout_template(
    name: String, template_type: Option<String>, difficulty: Option<String>,
    target_muscle_groups: Option<String>, notes: Option<String>,
    exercise_items: Option<Vec<serde_json::Value>>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO workout_templates (name, type, difficulty, target_muscle_groups, notes, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?6)",
        rusqlite::params![name, template_type.unwrap_or_else(|| "gym".into()),
            difficulty.unwrap_or_else(|| "easy".into()),
            target_muscle_groups.unwrap_or_default(), notes.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    let tmpl_id = conn.last_insert_rowid();
    if let Some(items) = exercise_items {
        for (i, item) in items.iter().enumerate() {
            let n = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if n.is_empty() { continue; }
            let cat_id = item.get("exercise_catalog_id").and_then(|v| v.as_i64());
            let sets = item.get("sets").and_then(|v| v.as_i64()).unwrap_or(3);
            let reps = item.get("reps").and_then(|v| v.as_i64()).unwrap_or(10);
            let weight = item.get("weight_kg").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let dur = item.get("duration_seconds").and_then(|v| v.as_i64()).unwrap_or(0);
            let rest = item.get("rest_seconds").and_then(|v| v.as_i64()).unwrap_or(60);
            conn.execute(
                "INSERT INTO template_exercises (template_id, exercise_catalog_id, name, sets, reps, weight_kg, duration_seconds, rest_seconds, order_index) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                rusqlite::params![tmpl_id, cat_id, n, sets, reps, weight, dur, rest, i as i64],
            ).map_err(|e| format!("DB error: {}", e))?;
        }
    }
    Ok(tmpl_id)
}

#[tauri::command]
pub fn get_workout_templates(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let base = "SELECT t.id, t.name, t.type, t.difficulty, t.target_muscle_groups, t.favorite, t.notes, t.updated_at, COUNT(e.id) as exercise_count FROM workout_templates t LEFT JOIN template_exercises e ON e.template_id=t.id";
    let sql = if search.is_some() {
        format!("{} WHERE t.name LIKE ?1 GROUP BY t.id ORDER BY t.updated_at DESC LIMIT 50", base)
    } else {
        format!("{} GROUP BY t.id ORDER BY t.updated_at DESC LIMIT 50", base)
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = if let Some(q) = search {
        let like = format!("%{}%", q);
        stmt.query_map(rusqlite::params![like], |row| template_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect()
    } else {
        stmt.query_map([], |row| template_from_row(row))
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect()
    };
    Ok(rows)
}

fn template_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "type": row.get::<_, String>(2)?, "difficulty": row.get::<_, String>(3)?,
        "target_muscle_groups": row.get::<_, String>(4)?,
        "favorite": row.get::<_, i64>(5)?, "notes": row.get::<_, String>(6)?,
        "updated_at": row.get::<_, String>(7)?,
        "exercise_count": row.get::<_, i64>(8)?,
    }))
}

#[tauri::command]
pub fn get_workout_template(id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut tmpl = conn.query_row(
        "SELECT id, name, type, difficulty, target_muscle_groups, favorite, notes, created_at, updated_at FROM workout_templates WHERE id=?1",
        rusqlite::params![id],
        |row| Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2)?, "difficulty": row.get::<_, String>(3)?,
            "target_muscle_groups": row.get::<_, String>(4)?,
            "favorite": row.get::<_, i64>(5)?, "notes": row.get::<_, String>(6)?,
            "created_at": row.get::<_, String>(7)?, "updated_at": row.get::<_, String>(8)?,
        })),
    ).map_err(|e| format!("Template not found: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, exercise_catalog_id, name, sets, reps, weight_kg, duration_seconds, rest_seconds, order_index FROM template_exercises WHERE template_id=?1 ORDER BY order_index"
    ).map_err(|e| format!("DB error: {}", e))?;
    let items: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "exercise_catalog_id": row.get::<_, Option<i64>>(1)?,
            "name": row.get::<_, String>(2)?, "sets": row.get::<_, i64>(3)?,
            "reps": row.get::<_, i64>(4)?, "weight_kg": row.get::<_, f64>(5)?,
            "duration_seconds": row.get::<_, i64>(6)?, "rest_seconds": row.get::<_, i64>(7)?,
            "order_index": row.get::<_, i64>(8)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    tmpl.as_object_mut().unwrap().insert("exercise_items".into(), serde_json::json!(items));
    Ok(tmpl)
}

#[tauri::command]
pub fn delete_workout_template(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM template_exercises WHERE template_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM workout_templates WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite_template(id: i64, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let cur: i64 = conn.query_row("SELECT favorite FROM workout_templates WHERE id=?1",
        rusqlite::params![id], |r| r.get(0)).unwrap_or(0);
    let next = if cur == 0 { 1 } else { 0 };
    conn.execute("UPDATE workout_templates SET favorite=?1 WHERE id=?2",
        rusqlite::params![next, id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(next)
}

// Materialize a logged workout (+ its exercises) from a template. Shared by the
// manual "start template" command and program-day completion.
pub(crate) fn make_workout_from_template(conn: &rusqlite::Connection, template_id: i64) -> Result<i64, String> {
    let (tmpl_type, tmpl_name): (String, String) = conn.query_row(
        "SELECT type, name FROM workout_templates WHERE id=?1",
        rusqlite::params![template_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| format!("Template not found: {}", e))?;
    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO workouts (type, title, date, duration_minutes, calories, notes, created_at, template_id) VALUES (?1,?2,?3,0,NULL,'',?4,?5)",
        rusqlite::params![tmpl_type, tmpl_name, date, now.to_rfc3339(), template_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    let workout_id = conn.last_insert_rowid();
    let mut stmt = conn.prepare(
        "SELECT name, sets, reps, weight_kg, duration_seconds FROM template_exercises WHERE template_id=?1 ORDER BY order_index"
    ).map_err(|e| format!("DB error: {}", e))?;
    let exercises: Vec<(String, i64, i64, f64, i64)> = stmt.query_map(rusqlite::params![template_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    for (name, sets, reps, weight, dur) in &exercises {
        conn.execute(
            "INSERT INTO exercises (workout_id, name, sets, reps, weight_kg, duration_seconds, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![workout_id, name, sets, reps, weight, dur, now.to_rfc3339()],
        ).map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(workout_id)
}

#[tauri::command]
pub fn create_workout_from_template(template_id: i64, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    make_workout_from_template(&db.conn(), template_id)
}
