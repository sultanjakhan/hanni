// routine.rs — Next-action engine, graph CRUD.
// A chain is a canvas; a node is a task (referencing a schedule/note/event,
// or a start trigger); an edge is an arrow with a transition trigger.
// Runtime logic (runs, "what now") lives in routine_engine.rs.
use crate::types::HanniDb;

fn node_json(r: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": r.get::<_, i64>(0)?,
        "source_type": r.get::<_, String>(1)?,
        "source_id": r.get::<_, Option<i64>>(2)?,
        "title": r.get::<_, String>(3)?,
        "category": r.get::<_, String>(4)?,
        "icon": r.get::<_, Option<String>>(5)?,
        "pos_x": r.get::<_, i64>(6)?,
        "pos_y": r.get::<_, i64>(7)?,
        "priority": r.get::<_, i64>(8)?,
        "requirement": r.get::<_, String>(9)?,
        "is_start": r.get::<_, i64>(10)? == 1,
    }))
}

const NODE_COLS: &str =
    "id, source_type, source_id, title, category, icon, pos_x, pos_y, priority, requirement, is_start";

/// All chains, each with its nodes and edges — feeds the graph constructor.
#[tauri::command]
pub fn get_routine_chains(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut cstmt = conn.prepare(
        "SELECT id, title, trigger_type, is_active FROM routine_chains ORDER BY sort_order, id"
    ).map_err(|e| format!("DB error: {}", e))?;
    let chains: Vec<(i64, String, String, bool)> = cstmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? == 1))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    let mut out = Vec::new();
    for (id, title, trigger_type, is_active) in chains {
        let mut nstmt = conn.prepare(&format!(
            "SELECT {} FROM routine_nodes WHERE chain_id=?1 ORDER BY id", NODE_COLS
        )).map_err(|e| format!("DB error: {}", e))?;
        let nodes: Vec<serde_json::Value> = nstmt.query_map(rusqlite::params![id], node_json)
            .map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

        let mut estmt = conn.prepare(
            "SELECT id, from_node_id, to_node_id, trigger_type, trigger_value
             FROM routine_edges WHERE chain_id=?1 ORDER BY id"
        ).map_err(|e| format!("DB error: {}", e))?;
        let edges: Vec<serde_json::Value> = estmt.query_map(rusqlite::params![id], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, i64>(0)?,
                "from_node_id": r.get::<_, i64>(1)?,
                "to_node_id": r.get::<_, i64>(2)?,
                "trigger_type": r.get::<_, String>(3)?,
                "trigger_value": r.get::<_, Option<i64>>(4)?,
            }))
        }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

        out.push(serde_json::json!({
            "id": id, "title": title, "trigger_type": trigger_type,
            "is_active": is_active, "nodes": nodes, "edges": edges,
        }));
    }
    Ok(out)
}

/// Add a task node to a chain. source_id NULL = autonomous node (not linked yet).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn create_routine_node(
    chain_id: i64, source_type: String, source_id: Option<i64>, title: String,
    category: Option<String>, pos_x: i64, pos_y: i64, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO routine_nodes (chain_id, source_type, source_id, title, category, pos_x, pos_y)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![chain_id, source_type, source_id, title,
                          category.unwrap_or_else(|| "other".into()), pos_x, pos_y],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// Patch any subset of a node's editable fields.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_routine_node(
    id: i64, title: Option<String>, category: Option<String>, pos_x: Option<i64>,
    pos_y: Option<i64>, priority: Option<i64>, requirement: Option<String>,
    source_id: Option<i64>, db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    if let Some(v) = title { conn.execute("UPDATE routine_nodes SET title=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = category { conn.execute("UPDATE routine_nodes SET category=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = pos_x { conn.execute("UPDATE routine_nodes SET pos_x=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = pos_y { conn.execute("UPDATE routine_nodes SET pos_y=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = priority { conn.execute("UPDATE routine_nodes SET priority=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    if let Some(v) = requirement {
        let val = if v == "optional" { "optional" } else { "required" };
        conn.execute("UPDATE routine_nodes SET requirement=?1 WHERE id=?2", rusqlite::params![val, id]).ok();
    }
    if let Some(v) = source_id { conn.execute("UPDATE routine_nodes SET source_id=?1 WHERE id=?2", rusqlite::params![v, id]).ok(); }
    Ok(())
}

/// Delete a node together with its edges and run statuses.
#[tauri::command]
pub fn delete_routine_node(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM routine_edges WHERE from_node_id=?1 OR to_node_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM routine_node_status WHERE node_id=?1", rusqlite::params![id]).ok();
    conn.execute("DELETE FROM routine_nodes WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

/// Create an arrow between two nodes. Trigger defaults to 'after_completion'.
#[tauri::command]
pub fn create_routine_edge(
    chain_id: i64, from_node_id: i64, to_node_id: i64, trigger_type: Option<String>,
    trigger_value: Option<i64>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    let trg = trigger_type.unwrap_or_else(|| "after_completion".into());
    conn.execute(
        "INSERT INTO routine_edges (chain_id, from_node_id, to_node_id, trigger_type, trigger_value)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![chain_id, from_node_id, to_node_id, trg, trigger_value],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// Change an edge's trigger type / value.
#[tauri::command]
pub fn update_routine_edge(
    id: i64, trigger_type: String, trigger_value: Option<i64>, db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "UPDATE routine_edges SET trigger_type=?1, trigger_value=?2 WHERE id=?3",
        rusqlite::params![trigger_type, trigger_value, id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

/// Delete an arrow.
#[tauri::command]
pub fn delete_routine_edge(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM routine_edges WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

/// Create a new chain seeded with a single start node, ready for the constructor.
/// Defaults to a 'manual' trigger (started by hand from the task widget).
#[tauri::command]
pub fn create_routine_chain(title: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let sort: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM routine_chains", [], |r| r.get(0),
    ).unwrap_or(0);
    conn.execute(
        "INSERT INTO routine_chains (title, trigger_type, sort_order) VALUES (?1, 'manual', ?2)",
        rusqlite::params![title, sort],
    ).map_err(|e| format!("DB error: {}", e))?;
    let chain_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO routine_nodes (chain_id, source_type, title, category, pos_x, pos_y, is_start)
         VALUES (?1, 'start', 'Старт', 'other', 30, 200, 1)",
        rusqlite::params![chain_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(chain_id)
}
