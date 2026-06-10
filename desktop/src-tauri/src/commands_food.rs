// commands_food.rs — Food, recipes, ingredients, blacklist, cuisines, products, meal-plan
use crate::types::*;

// ── v0.8.0: Food commands ──

#[tauri::command]
pub fn log_food(
    date: Option<String>, meal_type: String, name: String,
    calories: Option<i64>, protein: Option<f64>, carbs: Option<f64>, fat: Option<f64>,
    notes: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now();
    let d = date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    conn.execute(
        "INSERT INTO food_log (date, meal_type, name, calories, protein, carbs, fat, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![d, meal_type, name, calories.unwrap_or(0), protein.unwrap_or(0.0),
            carbs.unwrap_or(0.0), fat.unwrap_or(0.0), notes.unwrap_or_default(), now.to_rfc3339()],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_food_log(date: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut stmt = conn.prepare(
        "SELECT id, meal_type, name, calories, protein, carbs, fat, notes FROM food_log WHERE date=?1 ORDER BY created_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![d], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "meal_type": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?, "calories": row.get::<_, i64>(3)?,
            "protein": row.get::<_, f64>(4)?, "carbs": row.get::<_, f64>(5)?,
            "fat": row.get::<_, f64>(6)?, "notes": row.get::<_, String>(7)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_food_entry(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM food_log WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_food_entry(id: i64, name: Option<String>, meal_type: Option<String>, calories: Option<i64>, protein: Option<f64>, carbs: Option<f64>, fat: Option<f64>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = meal_type { updates.push(format!("meal_type=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = calories { updates.push(format!("calories=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = protein { updates.push(format!("protein=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = carbs { updates.push(format!("carbs=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = fat { updates.push(format!("fat=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE food_log SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_food_stats(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let d = days.unwrap_or(7);
    let since = (chrono::Local::now() - chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let (total_cal, avg_cal, total_protein): (i64, f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(calories),0), COALESCE(AVG(daily_cal),0), COALESCE(SUM(protein),0)
         FROM (SELECT date, SUM(calories) as daily_cal, SUM(protein) as protein FROM food_log WHERE date>=?1 GROUP BY date)",
        rusqlite::params![since], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap_or((0, 0.0, 0.0));
    Ok(serde_json::json!({ "total_calories": total_cal, "avg_daily_calories": format!("{:.0}", avg_cal), "total_protein": format!("{:.1}", total_protein), "days": d }))
}

#[tauri::command]
pub fn create_recipe(
    name: String, description: Option<String>, ingredients: String, instructions: String,
    prep_time: Option<i64>, cook_time: Option<i64>, servings: Option<i64>,
    calories: Option<i64>, tags: Option<String>, difficulty: Option<String>,
    cuisine: Option<String>, health_score: Option<i64>, price_score: Option<i64>,
    protein: Option<i64>, fat: Option<i64>, carbs: Option<i64>,
    image: Option<String>, taste_rating: Option<i64>, cook_note: Option<String>,
    ingredient_items: Option<Vec<serde_json::Value>>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO recipes (name, description, ingredients, instructions, prep_time, cook_time, servings, calories, tags, difficulty, cuisine, health_score, price_score, protein, fat, carbs, image, taste_rating, cook_note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?20)",
        rusqlite::params![name, description.unwrap_or_default(), ingredients, instructions,
            prep_time.unwrap_or(0), cook_time.unwrap_or(0), servings.unwrap_or(1),
            calories.unwrap_or(0), tags.unwrap_or_default(),
            difficulty.unwrap_or_else(|| "easy".into()),
            cuisine.unwrap_or_else(|| "kz".into()),
            health_score.unwrap_or(5), price_score.unwrap_or(5),
            protein.unwrap_or(0), fat.unwrap_or(0), carbs.unwrap_or(0),
            image.unwrap_or_default(), taste_rating.unwrap_or(0), cook_note.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    let recipe_id = conn.last_insert_rowid();
    if let Some(items) = ingredient_items {
        for item in &items {
            let n = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let a = item.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let u = item.get("unit").and_then(|v| v.as_str()).unwrap_or("г");
            if n.is_empty() { continue; }
            let alts = item.get("alternatives").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let cat_id: Option<i64> = item.get("catalog_id").and_then(|v| v.as_i64())
                .or_else(|| crate::db::resolve_catalog_id_by_name(&conn, n));
            let _ = conn.execute(
                "INSERT INTO recipe_ingredients (recipe_id, name, amount, unit, catalog_id, alternatives) VALUES (?1,?2,?3,?4,?5,?6)",
                rusqlite::params![recipe_id, n, a, u, cat_id, alts],
            );
        }
        crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    }
    crate::sync_share::mark_dirty(&conn, "recipes");
    Ok(recipe_id)
}

#[tauri::command]
pub fn get_recipes(search: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    if let Some(q) = search {
        let like = format!("%{}%", q);
        let mut stmt = conn.prepare(
            "SELECT id, name, description, prep_time, cook_time, servings, calories, tags, ingredients, difficulty, cuisine, health_score, price_score, protein, fat, carbs, favorite, last_cooked, (SELECT COALESCE(AVG(taste_rating),0) FROM cooking_log WHERE recipe_id=recipes.id AND taste_rating>0) AS avg_rating, (SELECT COUNT(*) FROM cooking_log WHERE recipe_id=recipes.id) AS cook_count, image FROM recipes WHERE name LIKE ?1 OR tags LIKE ?1 ORDER BY updated_at DESC LIMIT 50"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![like], |row| recipe_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, name, description, prep_time, cook_time, servings, calories, tags, ingredients, difficulty, cuisine, health_score, price_score, protein, fat, carbs, favorite, last_cooked, (SELECT COALESCE(AVG(taste_rating),0) FROM cooking_log WHERE recipe_id=recipes.id AND taste_rating>0) AS avg_rating, (SELECT COUNT(*) FROM cooking_log WHERE recipe_id=recipes.id) AS cook_count, image FROM recipes ORDER BY updated_at DESC LIMIT 50"
        ).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| recipe_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn recipe_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "description": row.get::<_, String>(2)?, "prep_time": row.get::<_, i64>(3)?,
        "cook_time": row.get::<_, i64>(4)?, "servings": row.get::<_, i64>(5)?,
        "calories": row.get::<_, i64>(6)?, "tags": row.get::<_, String>(7)?,
        "ingredients": row.get::<_, String>(8)?,
        "difficulty": row.get::<_, String>(9).unwrap_or_else(|_| "easy".into()),
        "cuisine": row.get::<_, String>(10).unwrap_or_else(|_| "kz".into()),
        "health_score": row.get::<_, i64>(11).unwrap_or(5),
        "price_score": row.get::<_, i64>(12).unwrap_or(5),
        "protein": row.get::<_, i64>(13).unwrap_or(0),
        "fat": row.get::<_, i64>(14).unwrap_or(0),
        "carbs": row.get::<_, i64>(15).unwrap_or(0),
        "favorite": row.get::<_, i64>(16).unwrap_or(0),
        "last_cooked": row.get::<_, Option<String>>(17).unwrap_or(None),
        "avg_rating": row.get::<_, f64>(18).unwrap_or(0.0),
        "cook_count": row.get::<_, i64>(19).unwrap_or(0),
        "image": row.get::<_, String>(20).unwrap_or_default(),
    }))
}

#[tauri::command]
pub fn get_recipe(id: i64, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut recipe = conn.query_row(
        "SELECT id, name, description, ingredients, instructions, prep_time, cook_time, servings, calories, tags, difficulty, cuisine, health_score, price_score, protein, fat, carbs, favorite, last_cooked, image, taste_rating, cook_note FROM recipes WHERE id=?1",
        rusqlite::params![id],
        |row| Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?, "ingredients": row.get::<_, String>(3)?,
            "instructions": row.get::<_, String>(4)?, "prep_time": row.get::<_, i64>(5)?,
            "cook_time": row.get::<_, i64>(6)?, "servings": row.get::<_, i64>(7)?,
            "calories": row.get::<_, i64>(8)?, "tags": row.get::<_, String>(9)?,
            "difficulty": row.get::<_, String>(10).unwrap_or_else(|_| "easy".into()),
            "cuisine": row.get::<_, String>(11).unwrap_or_else(|_| "kz".into()),
            "health_score": row.get::<_, i64>(12).unwrap_or(5),
            "price_score": row.get::<_, i64>(13).unwrap_or(5),
            "protein": row.get::<_, i64>(14).unwrap_or(0),
            "fat": row.get::<_, i64>(15).unwrap_or(0),
            "carbs": row.get::<_, i64>(16).unwrap_or(0),
            "favorite": row.get::<_, i64>(17).unwrap_or(0),
            "last_cooked": row.get::<_, Option<String>>(18).unwrap_or(None),
            "image": row.get::<_, String>(19).unwrap_or_default(),
            "taste_rating": row.get::<_, i64>(20).unwrap_or(0),
            "cook_note": row.get::<_, String>(21).unwrap_or_default(),
        })),
    ).map_err(|e| format!("Recipe not found: {}", e))?;
    // Attach structured ingredients
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, unit, catalog_id, alternatives FROM recipe_ingredients WHERE recipe_id=?1 ORDER BY id"
    ).map_err(|e| format!("DB error: {}", e))?;
    let items: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "amount": row.get::<_, f64>(2)?, "unit": row.get::<_, String>(3)?,
            "catalog_id": row.get::<_, Option<i64>>(4).unwrap_or(None),
            "alternatives": row.get::<_, String>(5).unwrap_or_default(),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    recipe.as_object_mut().unwrap().insert("ingredient_items".into(), serde_json::json!(items));
    Ok(recipe)
}

#[tauri::command]
pub fn update_recipe(
    id: i64, name: Option<String>, description: Option<String>, ingredients: Option<String>,
    instructions: Option<String>, prep_time: Option<i64>, cook_time: Option<i64>,
    servings: Option<i64>, calories: Option<i64>, tags: Option<String>, difficulty: Option<String>,
    cuisine: Option<String>, health_score: Option<i64>, price_score: Option<i64>,
    protein: Option<i64>, fat: Option<i64>, carbs: Option<i64>,
    image: Option<String>, taste_rating: Option<i64>, cook_note: Option<String>,
    ingredient_items: Option<Vec<serde_json::Value>>, db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    macro_rules! set { ($col:literal, $v:expr) => {
        if let Some(v) = $v { updates.push(format!("{}=?{}", $col, idx)); params.push(Box::new(v)); idx += 1; }
    }; }
    set!("name", name); set!("description", description); set!("ingredients", ingredients);
    set!("instructions", instructions); set!("prep_time", prep_time); set!("cook_time", cook_time);
    set!("servings", servings); set!("calories", calories); set!("tags", tags);
    set!("difficulty", difficulty); set!("cuisine", cuisine); set!("health_score", health_score);
    set!("price_score", price_score); set!("protein", protein); set!("fat", fat); set!("carbs", carbs);
    set!("image", image); set!("taste_rating", taste_rating); set!("cook_note", cook_note);
    if !updates.is_empty() {
        params.push(Box::new(id));
        let sql = format!("UPDATE recipes SET {} WHERE id=?{}", updates.join(","), idx);
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    }
    // Replace ingredient rows when provided.
    if let Some(items) = ingredient_items {
        conn.execute("DELETE FROM recipe_ingredients WHERE recipe_id=?1", rusqlite::params![id]).ok();
        for item in &items {
            let n = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if n.is_empty() { continue; }
            let a = item.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let u = item.get("unit").and_then(|v| v.as_str()).unwrap_or("г");
            let alts = item.get("alternatives").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let cat_id: Option<i64> = item.get("catalog_id").and_then(|v| v.as_i64())
                .or_else(|| crate::db::resolve_catalog_id_by_name(&conn, n));
            let _ = conn.execute(
                "INSERT INTO recipe_ingredients (recipe_id, name, amount, unit, catalog_id, alternatives) VALUES (?1,?2,?3,?4,?5,?6)",
                rusqlite::params![id, n, a, u, cat_id, alts]);
        }
        crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    }
    crate::sync_share::mark_dirty(&conn, "recipes");
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite_recipe(id: i64, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let cur: i64 = conn.query_row("SELECT favorite FROM recipes WHERE id=?1", rusqlite::params![id], |r| r.get(0)).unwrap_or(0);
    let next = if cur == 0 { 1 } else { 0 };
    conn.execute("UPDATE recipes SET favorite=?1 WHERE id=?2", rusqlite::params![next, id]).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "recipes");
    Ok(next)
}

#[tauri::command]
pub fn mark_recipe_cooked(id: i64, db: tauri::State<'_, HanniDb>) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
    conn.execute("UPDATE recipes SET last_cooked=?1 WHERE id=?2", rusqlite::params![now, id]).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "recipes");
    Ok(now)
}

// Log one cooking of a recipe (immutable history entry) + bump recipes.last_cooked.
#[tauri::command]
pub fn log_cooking(
    recipe_id: i64, date: String, taste_rating: Option<i64>, cook_note: Option<String>,
    event_id: Option<i64>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO cooking_log (recipe_id, date, taste_rating, cook_note, event_id, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![recipe_id, date, taste_rating.unwrap_or(0), cook_note.unwrap_or_default(), event_id, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    let id = conn.last_insert_rowid();
    // last_cooked = latest date in the log for this recipe.
    conn.execute(
        "UPDATE recipes SET last_cooked=(SELECT MAX(date) FROM cooking_log WHERE recipe_id=?1) WHERE id=?1",
        rusqlite::params![recipe_id],
    ).ok();
    crate::sync_share::mark_dirty(&conn, "cooking_log");
    crate::sync_share::mark_dirty(&conn, "recipes");
    // Auto-complete any schedule linked to cooking (auto_source='cooking').
    crate::calendar_health::auto_complete_from_cooking(&conn, &date, &now);
    Ok(id)
}

#[tauri::command]
pub fn get_cooking_log(recipe_id: i64, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, date, taste_rating, cook_note, event_id FROM cooking_log WHERE recipe_id=?1 ORDER BY date DESC, id DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![recipe_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "date": row.get::<_, String>(1)?,
            "taste_rating": row.get::<_, i64>(2)?,
            "cook_note": row.get::<_, String>(3)?,
            "event_id": row.get::<_, Option<i64>>(4).unwrap_or(None),
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_recipe(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM recipes WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "recipes");
    crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    Ok(())
}

#[tauri::command]
pub fn duplicate_recipe(id: i64, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO recipes (name, description, ingredients, instructions, prep_time, cook_time, servings, calories, tags, created_at, updated_at, difficulty, cuisine, health_score, price_score, protein, fat, carbs, favorite, last_cooked) \
         SELECT name || ' (копия)', description, ingredients, instructions, prep_time, cook_time, servings, calories, tags, ?1, ?1, difficulty, cuisine, health_score, price_score, protein, fat, carbs, 0, NULL \
         FROM recipes WHERE id=?2",
        rusqlite::params![now, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    let new_id = conn.last_insert_rowid();
    if new_id == 0 { return Err("Recipe not found".into()); }
    conn.execute(
        "INSERT INTO recipe_ingredients (recipe_id, name, amount, unit, catalog_id) \
         SELECT ?1, name, amount, unit, catalog_id FROM recipe_ingredients WHERE recipe_id=?2",
        rusqlite::params![new_id, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "recipes");
    crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    Ok(new_id)
}

#[tauri::command]
pub fn get_ingredient_catalog(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.category, c.tags, COALESCE(c.subgroup,''), \
                c.parent_id, COALESCE(p.name,'') \
         FROM ingredient_catalog c \
         LEFT JOIN ingredient_catalog p ON p.id = c.parent_id \
         ORDER BY c.name"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
            "category": row.get::<_, String>(2)?, "tags": row.get::<_, String>(3)?,
            "subgroup": row.get::<_, String>(4)?,
            "parent_id": row.get::<_, Option<i64>>(5)?,
            "parent_name": row.get::<_, String>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_ingredient_to_catalog(
    name: String, category: Option<String>, tags: Option<String>,
    subgroup: Option<String>, parent_id: Option<i64>, parent_name: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let cat = category.unwrap_or_else(|| "other".into());
    let t = tags.unwrap_or_default();
    let sg = subgroup.filter(|s| !s.is_empty());
    let pid: Option<i64> = match parent_id {
        Some(id) => Some(id),
        None => parent_name.and_then(|pn| {
            let trimmed = pn.trim().to_string();
            if trimmed.is_empty() { None } else {
                conn.query_row(
                    "SELECT id FROM ingredient_catalog WHERE name=?1 COLLATE NOCASE",
                    rusqlite::params![trimmed], |r| r.get::<_, i64>(0),
                ).ok()
            }
        }),
    };
    conn.execute(
        "INSERT OR IGNORE INTO ingredient_catalog (name, category, tags, subgroup, parent_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, cat, t, sg, pid],
    ).map_err(|e| format!("DB error: {}", e))?;
    let id: i64 = conn.query_row("SELECT id FROM ingredient_catalog WHERE name=?1 COLLATE NOCASE",
        rusqlite::params![name], |r| r.get(0)).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "ingredient_catalog");
    Ok(id)
}

#[tauri::command]
pub fn update_ingredient_in_catalog(
    id: i64, name: Option<String>, category: Option<String>, tags: Option<String>,
    subgroup: Option<String>, parent_id: Option<i64>, clear_parent: Option<bool>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    if let Some(ref new_name) = name {
        let old_name: String = conn.query_row("SELECT name FROM ingredient_catalog WHERE id=?1",
            rusqlite::params![id], |r| r.get(0)).map_err(|e| format!("DB error: {}", e))?;
        conn.execute("UPDATE ingredient_catalog SET name=?1 WHERE id=?2",
            rusqlite::params![new_name, id]).map_err(|e| format!("DB error: {}", e))?;
        // Cascade by catalog_id first (Stage 2 soft-link), then Unicode-aware fallback for legacy rows.
        let lower_new = new_name.trim().to_lowercase();
        let _ = conn.execute("UPDATE products SET name=?1 WHERE catalog_id=?2",
            rusqlite::params![new_name, id]);
        let _ = conn.execute("UPDATE recipe_ingredients SET name=?1 WHERE catalog_id=?2",
            rusqlite::params![new_name, id]);
        let _ = conn.execute("UPDATE food_blacklist SET value=?1 WHERE catalog_id=?2 AND type='product'",
            rusqlite::params![lower_new, id]);
        // SQLite COLLATE NOCASE doesn't fold Cyrillic — normalize_name() does.
        crate::db::rename_legacy_by_name(&conn, "products", "name", &old_name, new_name, "");
        crate::db::rename_legacy_by_name(&conn, "recipe_ingredients", "name", &old_name, new_name, "");
        crate::db::rename_legacy_by_name(&conn, "food_blacklist", "value", &old_name, &lower_new, "type='product'");
    }
    if let Some(ref cat) = category {
        conn.execute("UPDATE ingredient_catalog SET category=?1 WHERE id=?2",
            rusqlite::params![cat, id]).map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(ref t) = tags {
        conn.execute("UPDATE ingredient_catalog SET tags=?1 WHERE id=?2",
            rusqlite::params![t, id]).map_err(|e| format!("DB error: {}", e))?;
    }
    if let Some(ref sg) = subgroup {
        let val: Option<&str> = if sg.is_empty() { None } else { Some(sg.as_str()) };
        conn.execute("UPDATE ingredient_catalog SET subgroup=?1 WHERE id=?2",
            rusqlite::params![val, id]).map_err(|e| format!("DB error: {}", e))?;
    }
    if clear_parent.unwrap_or(false) {
        conn.execute("UPDATE ingredient_catalog SET parent_id=NULL WHERE id=?1",
            rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    } else if let Some(pid) = parent_id {
        if pid == id { return Err("Cannot set self as parent".into()); }
        // Stage 1: hierarchy is strictly 2 levels — parent must be top-level.
        let parent_of_parent: Option<i64> = conn.query_row(
            "SELECT parent_id FROM ingredient_catalog WHERE id=?1",
            rusqlite::params![pid], |r| r.get(0),
        ).unwrap_or(None);
        if parent_of_parent.is_some() {
            return Err("Parent must be top-level (no grandparents)".into());
        }
        conn.execute("UPDATE ingredient_catalog SET parent_id=?1 WHERE id=?2",
            rusqlite::params![pid, id]).map_err(|e| format!("DB error: {}", e))?;
    }
    crate::sync_share::mark_dirty(&conn, "ingredient_catalog");
    crate::sync_share::mark_dirty(&conn, "products");
    crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    crate::sync_share::mark_dirty(&conn, "food_blacklist");
    Ok(())
}

#[tauri::command]
pub fn list_catalog_subgroups(category: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT COALESCE(subgroup,'') AS sg, COUNT(*) AS cnt FROM ingredient_catalog WHERE category=?1 GROUP BY sg ORDER BY sg"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![category], |row| {
        Ok(serde_json::json!({ "name": row.get::<_, String>(0)?, "count": row.get::<_, i64>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_ingredient_from_catalog(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    // Detach children first (we don't rely on PRAGMA foreign_keys being on).
    let _ = conn.execute("UPDATE ingredient_catalog SET parent_id=NULL WHERE parent_id=?1",
        rusqlite::params![id]);
    conn.execute("DELETE FROM ingredient_catalog WHERE id=?1",
        rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "ingredient_catalog");
    Ok(())
}

#[tauri::command]
pub fn check_ingredient_usage(ingredient_name: String, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT r.name FROM recipes r JOIN recipe_ingredients ri ON ri.recipe_id = r.id WHERE ri.name = ?1 COLLATE NOCASE"
    ).map_err(|e| format!("DB error: {}", e))?;
    let names: Vec<String> = stmt.query_map(rusqlite::params![ingredient_name], |r| r.get(0))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(serde_json::json!({ "count": names.len(), "recipe_names": names }))
}

// ── Food blacklist ──────────────────────────────────────────────────
fn blacklist_keywords(conn: &rusqlite::Connection, entry_type: &str, value: &str) -> Vec<String> {
    let v = value.trim().to_lowercase();
    match entry_type {
        "tag" => {
            let mut kws = vec![v.clone()];
            if let Ok(mut stmt) = conn.prepare(
                "SELECT name FROM ingredient_catalog WHERE (',' || tags || ',') LIKE ?1"
            ) {
                let pat = format!("%,{},%", v);
                let names: Vec<String> = stmt.query_map(rusqlite::params![pat], |r| r.get(0))
                    .map(|m| m.filter_map(|x| x.ok()).collect()).unwrap_or_default();
                kws.extend(names);
            }
            kws
        }
        "category" => {
            let mut kws = vec![];
            if let Ok(mut stmt) = conn.prepare("SELECT name FROM ingredient_catalog WHERE category = ?1") {
                let names: Vec<String> = stmt.query_map(rusqlite::params![v], |r| r.get(0))
                    .map(|m| m.filter_map(|x| x.ok()).collect()).unwrap_or_default();
                kws.extend(names);
            }
            kws
        }
        _ => vec![v],
    }
}

fn recipes_matching_keywords(conn: &rusqlite::Connection, kws: &[String]) -> Vec<(i64, String)> {
    if kws.is_empty() { return vec![]; }
    let mut ids: std::collections::BTreeMap<i64, String> = std::collections::BTreeMap::new();
    for kw in kws {
        let pat = format!("%{}%", kw);
        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT r.id, r.name FROM recipes r \
             LEFT JOIN recipe_ingredients ri ON ri.recipe_id = r.id \
             WHERE LOWER(r.name) LIKE ?1 OR LOWER(IFNULL(r.ingredients,'')) LIKE ?1 \
                OR LOWER(IFNULL(ri.name,'')) LIKE ?1"
        ) {
            let rows: Vec<(i64, String)> = stmt.query_map(rusqlite::params![pat], |r|
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map(|m| m.filter_map(|x| x.ok()).collect()).unwrap_or_default();
            for (id, name) in rows { ids.insert(id, name); }
        }
    }
    ids.into_iter().collect()
}

#[tauri::command]
pub fn list_food_blacklist(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT b.id, b.type, b.value, b.catalog_id, COALESCE(c.name,''), b.level \
         FROM food_blacklist b \
         LEFT JOIN ingredient_catalog c ON c.id = b.catalog_id \
         ORDER BY b.type, b.value"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "type": row.get::<_, String>(1)?,
            "value": row.get::<_, String>(2)?,
            "catalog_id": row.get::<_, Option<i64>>(3).unwrap_or(None),
            "catalog_name": row.get::<_, String>(4).unwrap_or_default(),
            "level": row.get::<_, String>(5)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_food_blacklist(
    entry_type: String, value: String,
    level: Option<String>,
    catalog_id: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    if !["tag","product","category","keyword","recipe"].contains(&entry_type.as_str()) {
        return Err("invalid type".into());
    }
    let lvl = level.unwrap_or_else(|| "hard".into());
    if !["hard","soft","love"].contains(&lvl.as_str()) {
        return Err("invalid level".into());
    }
    let conn = db.conn();
    let v = value.trim().to_lowercase();
    if v.is_empty() { return Err("empty value".into()); }
    // Auto-resolve catalog_id for type=product when not provided.
    let cat_id: Option<i64> = match catalog_id {
        Some(id) => Some(id),
        None if entry_type == "product" => crate::db::resolve_catalog_id_by_name(&conn, &v),
        None => None,
    };
    // Upsert: re-adding an existing entry switches its level (hard ↔ soft).
    conn.execute(
        "INSERT INTO food_blacklist (type, value, level, catalog_id) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(type, value) DO UPDATE SET level=excluded.level",
        rusqlite::params![entry_type, v, lvl, cat_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    let id: i64 = conn.query_row("SELECT id FROM food_blacklist WHERE type=?1 AND value=?2",
        rusqlite::params![entry_type, v], |r| r.get(0)).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "food_blacklist");
    Ok(id)
}

#[tauri::command]
pub fn remove_food_blacklist(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM food_blacklist WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "food_blacklist");
    Ok(())
}

#[tauri::command]
pub fn find_recipes_matching_blacklist(entry_type: String, value: String, db: tauri::State<'_, HanniDb>)
    -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let kws = blacklist_keywords(&conn, &entry_type, &value);
    let hits = recipes_matching_keywords(&conn, &kws);
    Ok(hits.into_iter().map(|(id, name)| serde_json::json!({ "id": id, "name": name })).collect())
}

#[tauri::command]
pub fn delete_recipes_matching_blacklist(entry_type: String, value: String, db: tauri::State<'_, HanniDb>)
    -> Result<usize, String> {
    let conn = db.conn();
    let kws = blacklist_keywords(&conn, &entry_type, &value);
    let hits = recipes_matching_keywords(&conn, &kws);
    let count = hits.len();
    for (id, _) in hits {
        let _ = conn.execute("DELETE FROM recipes WHERE id=?1", rusqlite::params![id]);
    }
    if count > 0 {
        crate::sync_share::mark_dirty(&conn, "recipes");
        crate::sync_share::mark_dirty(&conn, "recipe_ingredients");
    }
    Ok(count)
}

#[tauri::command]
pub fn get_cuisines(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, code, name, emoji, is_default FROM custom_cuisines ORDER BY is_default DESC, name")
        .map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(serde_json::json!({ "id": row.get::<_, i64>(0)?, "code": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?, "emoji": row.get::<_, String>(3)?,
            "is_default": row.get::<_, i64>(4)? }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_cuisine(code: String, name: String, emoji: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let em = emoji.unwrap_or_else(|| "🌍".into());
    conn.execute("INSERT INTO custom_cuisines (code, name, emoji, is_default) VALUES (?1, ?2, ?3, 0)",
        rusqlite::params![code, name, em]).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn add_product(
    name: String, category: Option<String>, quantity: Option<f64>, unit: Option<String>,
    expiry_date: Option<String>, location: Option<String>, notes: Option<String>,
    catalog_id: Option<i64>,
    db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    // Strict auto-link by name when caller didn't provide catalog_id explicitly.
    let resolved_cat_id: Option<i64> = match catalog_id {
        Some(id) => Some(id),
        None => crate::db::resolve_catalog_id_by_name(&conn, &name),
    };
    // If linked, inherit category from catalog so it stays canonical.
    let final_category: String = match resolved_cat_id {
        Some(id) => conn.query_row(
            "SELECT category FROM ingredient_catalog WHERE id=?1",
            rusqlite::params![id], |r| r.get::<_, String>(0),
        ).unwrap_or_else(|_| category.clone().unwrap_or_else(|| "other".into())),
        None => category.unwrap_or_else(|| "other".into()),
    };
    conn.execute(
        "INSERT INTO products (name, category, quantity, unit, expiry_date, location, notes, purchased_at, created_at, catalog_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
        rusqlite::params![name, final_category, quantity.unwrap_or(1.0),
            unit.unwrap_or_else(|| "шт".into()), expiry_date,
            location.unwrap_or_else(|| "fridge".into()), notes.unwrap_or_default(), now,
            resolved_cat_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    let new_id = conn.last_insert_rowid();
    crate::sync_share::mark_dirty(&conn, "products");
    Ok(new_id)
}

#[tauri::command]
pub fn get_products(location: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let select = "SELECT p.id, p.name, p.category, p.quantity, p.unit, p.expiry_date, p.location, p.notes, \
                         p.catalog_id, COALESCE(c.name,'') \
                  FROM products p LEFT JOIN ingredient_catalog c ON c.id = p.catalog_id";
    if let Some(loc) = location {
        let sql = format!("{} WHERE p.location=?1 ORDER BY p.expiry_date NULLS LAST", select);
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map(rusqlite::params![loc], |row| product_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    } else {
        let sql = format!("{} ORDER BY p.expiry_date NULLS LAST", select);
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("DB error: {}", e))?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |row| product_from_row(row)).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }
}

pub fn product_from_row(row: &rusqlite::Row) -> Result<serde_json::Value, rusqlite::Error> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?,
        "category": row.get::<_, String>(2)?, "quantity": row.get::<_, f64>(3)?,
        "unit": row.get::<_, String>(4)?, "expiry_date": row.get::<_, Option<String>>(5)?,
        "location": row.get::<_, String>(6)?, "notes": row.get::<_, String>(7)?,
        "catalog_id": row.get::<_, Option<i64>>(8).unwrap_or(None),
        "catalog_name": row.get::<_, String>(9).unwrap_or_default(),
    }))
}

#[tauri::command]
pub fn update_product(
    id: i64, name: Option<String>, quantity: Option<f64>, expiry_date: Option<String>,
    location: Option<String>, notes: Option<String>,
    catalog_id: Option<i64>, clear_catalog: Option<bool>,
    db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    let name_changed = name.is_some();
    if let Some(ref v) = name { updates.push(format!("name=?{}", idx)); params.push(Box::new(v.clone())); idx += 1; }
    if let Some(v) = quantity { updates.push(format!("quantity=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = expiry_date { updates.push(format!("expiry_date=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = location { updates.push(format!("location=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = notes { updates.push(format!("notes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if clear_catalog.unwrap_or(false) {
        updates.push(format!("catalog_id=NULL"));
    } else if let Some(cid) = catalog_id {
        updates.push(format!("catalog_id=?{}", idx)); params.push(Box::new(cid)); idx += 1;
    } else if name_changed {
        // Auto-resolve by new name if caller didn't set explicitly.
        if let Some(ref nm) = name {
            let auto: Option<i64> = crate::db::resolve_catalog_id_by_name(&conn, nm);
            updates.push(format!("catalog_id=?{}", idx));
            params.push(Box::new(auto));
            idx += 1;
        }
    }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE products SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "products");
    Ok(())
}

#[tauri::command]
pub fn delete_product(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM products WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "products");
    Ok(())
}

#[tauri::command]
pub fn get_expiring_products(days: Option<i64>, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let d = days.unwrap_or(3);
    let deadline = (chrono::Local::now() + chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, name, category, quantity, unit, expiry_date, location, notes FROM products
         WHERE expiry_date IS NOT NULL AND expiry_date <= ?1 ORDER BY expiry_date"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![deadline], |row| product_from_row(row))
        .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── Meal Plan commands ──

#[tauri::command]
pub fn plan_meal(date: String, meal_type: String, recipe_id: i64, notes: Option<String>, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO meal_plan (date, meal_type, recipe_id, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![date, meal_type, recipe_id, notes.unwrap_or_default(), now],
    ).map_err(|e| format!("DB error: {}", e))?;
    let new_id = conn.last_insert_rowid();
    crate::sync_share::mark_dirty(&conn, "meal_plan");
    Ok(new_id)
}

#[tauri::command]
pub fn get_meal_plan(date: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT mp.id, mp.date, mp.meal_type, mp.recipe_id, mp.notes, r.name, r.calories
         FROM meal_plan mp JOIN recipes r ON mp.recipe_id = r.id WHERE mp.date = ?1 ORDER BY CASE mp.meal_type WHEN 'breakfast' THEN 1 WHEN 'lunch' THEN 2 WHEN 'dinner' THEN 3 ELSE 4 END"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![date], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "date": row.get::<_, String>(1)?,
            "meal_type": row.get::<_, String>(2)?, "recipe_id": row.get::<_, i64>(3)?,
            "notes": row.get::<_, String>(4)?, "recipe_name": row.get::<_, String>(5)?,
            "calories": row.get::<_, i64>(6)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_meal_plan(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM meal_plan WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    crate::sync_share::mark_dirty(&conn, "meal_plan");
    Ok(())
}
