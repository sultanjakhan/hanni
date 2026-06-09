// routine_engine.rs — Next-action engine runtime.
// Tracks routine_runs (daily passes of a chain) and resolves "what to do now"
// by walking the graph: a node is available once its incoming edges are satisfied.
use crate::types::HanniDb;
use std::collections::HashMap;

struct RNode {
    id: i64, is_start: bool, requirement: String,
    title: String, category: String, priority: i64,
    // source_id: UUIDv7 string (post-CRR) or legacy int as text — never i64,
    // or schedule-linked nodes silently drop out of load_nodes (InvalidColumnType).
    source_type: String, source_id: Option<String>,
}
struct REdge { from: i64, to: i64, trigger: String, value: Option<i64> }

/// Start (or restart) an active pass of a chain for the day. If a run already
/// exists — including one completed earlier today — it is reset to 'active' and
/// its step statuses cleared, so re-clicking "Я встал" restarts the routine
/// instead of silently doing nothing.
#[tauri::command]
pub fn start_routine_run(
    chain_id: i64, date: String, slot: Option<String>, db: tauri::State<'_, HanniDb>,
) -> Result<i64, String> {
    let conn = db.conn();
    // slot='' = the usual one-run-per-day chain; non-empty = a meal/launch slot.
    let slot = slot.unwrap_or_default();
    // Deterministic run id (chain_id|date|slot): both devices compute the same id
    // for one daily pass, so a pulled run UPSERTs onto the same row instead of
    // violating UNIQUE(chain_id,date,slot).
    let run_id = crate::types::routine_run_id(chain_id, &date, &slot);
    conn.execute(
        "INSERT INTO routine_runs (id, chain_id, date, slot, state) VALUES (?1, ?2, ?3, ?4, 'active')
         ON CONFLICT(chain_id, date, slot) DO UPDATE SET state='active', completed_at=NULL",
        rusqlite::params![run_id, chain_id, date, slot],
    ).map_err(|e| format!("DB error: {}", e))?;
    conn.execute("DELETE FROM routine_node_status WHERE run_id=?1", rusqlite::params![run_id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(run_id)
}

/// Cancel a run: drop it and its node statuses (the chain returns to "not started").
#[tauri::command]
pub fn delete_routine_run(run_id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM routine_node_status WHERE run_id=?1", rusqlite::params![run_id]).ok();
    conn.execute("DELETE FROM routine_runs WHERE id=?1", rusqlite::params![run_id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

/// Set a node's state inside a run. state 'pending' (or empty) clears the record.
/// If the node is backed by a schedule (source_type='schedule', source_id set),
/// mirror the state into schedule_completions so the same task in Calendar
/// Day-view / Список / sidebar stays in sync with the routine canvas.
#[tauri::command]
pub fn set_routine_node_status(
    run_id: i64, node_id: i64, state: String, db: tauri::State<'_, HanniDb>,
) -> Result<(), String> {
    let conn = db.conn();
    let clearing = state.is_empty() || state == "pending";
    if clearing {
        conn.execute("DELETE FROM routine_node_status WHERE run_id=?1 AND node_id=?2",
            rusqlite::params![run_id, node_id]).map_err(|e| format!("DB error: {}", e))?;
    } else {
        // Deterministic status id (run_id|node_id); updated_at='' so the AFTER
        // INSERT/UPDATE sync triggers stamp the RFC3339-'T' form (a literal
        // datetime('now') would be space-separated and sort below the cursor).
        let nstat_id = crate::types::routine_node_status_id(run_id, node_id);
        conn.execute(
            "INSERT INTO routine_node_status (id, run_id, node_id, state, updated_at)
             VALUES (?1, ?2, ?3, ?4, '')
             ON CONFLICT(run_id, node_id) DO UPDATE SET state=excluded.state",
            rusqlite::params![nstat_id, run_id, node_id, state],
        ).map_err(|e| format!("DB error: {}", e))?;
    }

    // Mirror into schedule_completions when the node wraps a schedule.
    // Two paths to find the schedule id:
    //   1) explicit source_id on the node (set when the node was created
    //      from a schedule);
    //   2) fallback by matching node.title against schedules.title
    //      case-insensitively — covers older nodes built before source_id
    //      was assigned, so the routine click still propagates.
    let node_info: Option<(String, Option<String>, String)> = conn.query_row(
        "SELECT source_type, source_id, title FROM routine_nodes WHERE id=?1",
        rusqlite::params![node_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)),
    ).ok();
    let date: Option<String> = conn.query_row(
        "SELECT date FROM routine_runs WHERE id=?1",
        rusqlite::params![run_id], |r| r.get::<_, String>(0),
    ).ok();
    let resolved_sid: Option<String> = node_info.as_ref().and_then(|(stype, sid, title)| {
        if stype != "schedule" { return None; }
        if let Some(s) = sid { if !s.is_empty() { return Some(s.clone()); } }
        // Title fallback (Rust lowercase — Unicode-aware, unlike SQLite LOWER):
        //   1) exact match on lowercase title
        //   2) substring match — accept only when unique.
        let want = title.to_lowercase();
        let mut stmt = conn.prepare(
            "SELECT id, title FROM schedules WHERE is_active = 1"
        ).ok()?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).ok()?;
        let all: Vec<(String, String)> = rows.filter_map(|r| r.ok())
            .map(|(i, t)| (i, t.to_lowercase())).collect();
        if let Some((id, _)) = all.iter().find(|(_, t)| *t == want) {
            return Some(id.clone());
        }
        let subs: Vec<String> = all.iter()
            .filter(|(_, t)| t.contains(&want) || want.contains(t))
            .map(|(i, _)| i.clone()).collect();
        if subs.len() == 1 { Some(subs.into_iter().next().unwrap()) } else { None }
    });
    if let (Some(sid), Some(d)) = (resolved_sid, date) {
        // Reflections (marks_previous_day) record completion for the PREVIOUS day —
        // same as Список/Месяц/День/picker — so a routine step ✓/✗ lands on yesterday.
        let marks_prev: i64 = conn.query_row(
            "SELECT COALESCE(marks_previous_day, 0) FROM schedules WHERE id=?1",
            rusqlite::params![sid], |r| r.get(0),
        ).unwrap_or(0);
        let d = if marks_prev == 1 {
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map(|nd| (nd - chrono::Duration::days(1)).format("%Y-%m-%d").to_string())
                .unwrap_or(d)
        } else { d };
        {
            if clearing {
                let _ = conn.execute(
                    "UPDATE schedule_completions SET completed=0, completed_at=NULL, status='planned'
                     WHERE schedule_id=?1 AND date=?2",
                    rusqlite::params![sid, d],
                );
            } else if state == "done" {
                let now = chrono::Local::now().to_rfc3339();
                let new_id = crate::types::new_uuid_v7();
                let _ = conn.execute(
                    "INSERT INTO schedule_completions (id, schedule_id, date, completed, completed_at, status)
                     VALUES (?1, ?2, ?3, 1, ?4, 'done')
                     ON CONFLICT(schedule_id, date) DO UPDATE
                       SET completed=1, completed_at=COALESCE(schedule_completions.completed_at, ?4), status='done'",
                    rusqlite::params![new_id, sid, d, now],
                );
            } else if state == "skipped" {
                let new_id = crate::types::new_uuid_v7();
                let _ = conn.execute(
                    "INSERT INTO schedule_completions (id, schedule_id, date, completed, completed_at, status)
                     VALUES (?1, ?2, ?3, 0, NULL, 'skipped')
                     ON CONFLICT(schedule_id, date) DO UPDATE
                       SET completed=0, completed_at=NULL, status='skipped'",
                    rusqlite::params![new_id, sid, d],
                );
            }
        }
    }
    Ok(())
}

/// Reverse of set_routine_node_status's schedule mirror: when a schedule is
/// completed/skipped/cleared elsewhere (Health auto-complete, Список ✓/✗ toggle),
/// reflect that into any active routine run whose node wraps this schedule, so the
/// routine canvas/widget stays in sync. `state` ∈ done | skipped | clear.
pub(crate) fn mirror_schedule_to_routine(
    conn: &rusqlite::Connection, schedule_id: &str, comp_date: &str, state: &str,
) {
    // Reflection schedules (marks_previous_day) record completion for the PREVIOUS
    // day while their routine run lives on the following day — invert to find the run.
    let marks_prev: i64 = conn.query_row(
        "SELECT COALESCE(marks_previous_day, 0) FROM schedules WHERE id=?1",
        rusqlite::params![schedule_id], |r| r.get(0),
    ).unwrap_or(0);
    let run_date = if marks_prev == 1 {
        chrono::NaiveDate::parse_from_str(comp_date, "%Y-%m-%d")
            .map(|nd| (nd + chrono::Duration::days(1)).format("%Y-%m-%d").to_string())
            .unwrap_or_else(|_| comp_date.to_string())
    } else { comp_date.to_string() };

    // One schedule can sit in several chains — mirror into every active run on run_date.
    let pairs: Vec<(i64, i64)> = {
        let mut stmt = match conn.prepare(
            "SELECT rr.id, rn.id FROM routine_runs rr
             JOIN routine_nodes rn ON rn.chain_id = rr.chain_id
             WHERE rr.date=?1 AND rr.state='active'
               AND rn.source_type='schedule' AND rn.source_id=?2"
        ) { Ok(s) => s, Err(_) => return };
        let rows = match stmt.query_map(rusqlite::params![run_date, schedule_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        }) { Ok(r) => r, Err(_) => return };
        rows.filter_map(|r| r.ok()).collect()
    };
    for (run_id, node_id) in pairs {
        if state == "clear" {
            let _ = conn.execute(
                "DELETE FROM routine_node_status WHERE run_id=?1 AND node_id=?2",
                rusqlite::params![run_id, node_id]);
        } else {
            let nstat_id = crate::types::routine_node_status_id(run_id, node_id);
            let _ = conn.execute(
                "INSERT INTO routine_node_status (id, run_id, node_id, state, updated_at)
                 VALUES (?1, ?2, ?3, ?4, '')
                 ON CONFLICT(run_id, node_id) DO UPDATE SET state=excluded.state",
                rusqlite::params![nstat_id, run_id, node_id, state]);
        }
    }
}

/// Chain ids whose run for `date` is completed — the picker hides these so a
/// finished chain doesn't keep showing "Я встал".
#[tauri::command]
pub fn get_completed_routine_chains(date: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT chain_id, slot FROM routine_runs WHERE date=?1 AND state='completed'"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![date], |r| {
        Ok(serde_json::json!({ "chain_id": r.get::<_, i64>(0)?, "slot": r.get::<_, String>(1)? }))
    }).map_err(|e| format!("Query error: {}", e))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Core of the engine: for every active run on `date`, return the available nodes
/// (incoming edges satisfied, not yet closed). A run with all required nodes
/// closed is auto-marked 'completed'.
#[tauri::command]
pub fn get_routine_now(date: String, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut rstmt = conn.prepare(
        "SELECT r.id, r.chain_id, c.title, r.slot FROM routine_runs r
         JOIN routine_chains c ON c.id = r.chain_id
         WHERE r.date=?1 AND r.state='active'"
    ).map_err(|e| format!("DB error: {}", e))?;
    let runs: Vec<(i64, i64, String, String)> = rstmt.query_map(rusqlite::params![date], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();

    let mut out = Vec::new();
    for (run_id, chain_id, chain_title, slot) in runs {
        let nodes = load_nodes(&conn, chain_id)?;
        let edges = load_edges(&conn, chain_id)?;
        let status = load_status(&conn, run_id)?;          // node_id → (state, updated_at)
        let closed = |id: i64| matches!(status.get(&id).map(|s| s.0.as_str()), Some("done") | Some("skipped"));

        let avail: Vec<&RNode> = nodes.values()
            .filter(|n| !n.is_start && !closed(n.id))
            .filter(|n| {
                let inc: Vec<&REdge> = edges.iter().filter(|e| e.to == n.id).collect();
                inc.is_empty() || inc.iter().all(|e| edge_satisfied(e, &nodes, &status, &closed))
            })
            .collect();

        // Nodes gated only by a manual edge whose source is already done: the
        // user can open them by hand. Excludes already-unlocked (those are in avail).
        let unlocked = |id: i64| matches!(status.get(&id).map(|s| s.0.as_str()), Some("unlocked"));
        let locked: Vec<&RNode> = nodes.values()
            .filter(|n| !n.is_start && !closed(n.id) && !unlocked(n.id))
            .filter(|n| {
                let inc: Vec<&REdge> = edges.iter().filter(|e| e.to == n.id).collect();
                let ready_manual = inc.iter().any(|e| e.trigger == "manual"
                    && (nodes.get(&e.from).map(|s| s.is_start).unwrap_or(false) || closed(e.from)));
                let others_ok = inc.iter().filter(|e| e.trigger != "manual")
                    .all(|e| edge_satisfied(e, &nodes, &status, &closed));
                ready_manual && others_ok
            })
            .collect();

        let has_required = nodes.values().any(|n| !n.is_start && n.requirement == "required");
        let all_required_closed = nodes.values()
            .filter(|n| !n.is_start && n.requirement == "required")
            .all(|n| closed(n.id));
        // Done = nothing left to do. Chains WITH required steps finish once those
        // are closed (optional bonus steps may remain). All-optional chains finish
        // only when no step is still available/locked — otherwise the empty
        // "required" set makes all() == true and the run auto-completes the instant
        // it starts (e.g. the all-optional "Спорт" chain).
        let done = if has_required { all_required_closed } else { avail.is_empty() && locked.is_empty() };
        if done && nodes.values().any(|n| !n.is_start) {
            conn.execute("UPDATE routine_runs SET state='completed', completed_at=?1 WHERE id=?2",
                rusqlite::params![chrono::Local::now().to_rfc3339(), run_id]).ok();
        }
        // tracking_mode/marks_previous_day come from the wrapped schedule (if any) so
        // the picker shows ✓/✗ for check/reflection steps but ▶ start for timer steps.
        let to_json = |n: &&RNode| {
            let (tracking_mode, marks_prev): (Option<String>, bool) = if n.source_type == "schedule" {
                n.source_id.as_ref().and_then(|sid| conn.query_row(
                    "SELECT COALESCE(tracking_mode,'track'), COALESCE(marks_previous_day,0) FROM schedules WHERE id=?1",
                    rusqlite::params![sid],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? == 1)),
                ).ok()).map(|(tm, mp)| (Some(tm), mp)).unwrap_or((None, false))
            } else { (None, false) };
            serde_json::json!({
                "id": n.id, "title": n.title, "category": n.category, "priority": n.priority,
                "requirement": n.requirement, "source_type": n.source_type, "source_id": n.source_id,
                "tracking_mode": tracking_mode, "marks_previous_day": marks_prev,
            })
        };
        let tasks: Vec<serde_json::Value> = avail.iter().map(to_json).collect();
        let locked_tasks: Vec<serde_json::Value> = locked.iter().map(to_json).collect();
        out.push(serde_json::json!({
            "run_id": run_id, "chain_id": chain_id, "chain_title": chain_title, "slot": slot,
            "tasks": tasks, "locked": locked_tasks,
        }));
    }
    Ok(out)
}

/// An edge is satisfied when its source node has fired the right trigger.
fn edge_satisfied(
    e: &REdge, nodes: &HashMap<i64, RNode>, status: &HashMap<i64, (String, String)>,
    closed: &impl Fn(i64) -> bool,
) -> bool {
    if nodes.get(&e.from).map(|n| n.is_start).unwrap_or(false) { return true; }
    match e.trigger.as_str() {
        // Opened by hand: the target node carries an 'unlocked' mark.
        "manual" => matches!(status.get(&e.to).map(|s| s.0.as_str()), Some("unlocked")),
        "after_duration" => {
            if status.get(&e.from).map(|s| s.0.as_str()) != Some("done") { return false; }
            let mins = e.value.unwrap_or(0);
            let done_at = status.get(&e.from).map(|s| s.1.as_str()).unwrap_or("");
            // Sync triggers stamp updated_at as 'T'-separated LOCAL time with
            // fractional seconds; legacy rows hold space-separated UTC.
            let elapsed = if let Ok(t) =
                chrono::NaiveDateTime::parse_from_str(done_at, "%Y-%m-%dT%H:%M:%S%.f") {
                chrono::Local::now().naive_local() - t
            } else if let Ok(t) =
                chrono::NaiveDateTime::parse_from_str(done_at, "%Y-%m-%d %H:%M:%S") {
                chrono::Utc::now().naive_utc() - t
            } else {
                return true;                               // unparseable → don't block
            };
            elapsed.num_minutes() >= mins
        }
        _ => closed(e.from),                                // after_completion
    }
}

fn load_nodes(conn: &rusqlite::Connection, chain_id: i64) -> Result<HashMap<i64, RNode>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, is_start, requirement, title, category, priority, source_type, source_id
         FROM routine_nodes WHERE chain_id=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![chain_id], |r| {
        Ok(RNode {
            id: r.get(0)?, is_start: r.get::<_, i64>(1)? == 1, requirement: r.get(2)?,
            title: r.get(3)?, category: r.get(4)?, priority: r.get(5)?,
            source_type: r.get(6)?, source_id: r.get(7)?,
        })
    }).map_err(|e| format!("Query error: {}", e))?;
    Ok(rows.filter_map(|r| r.ok()).map(|n| (n.id, n)).collect())
}

fn load_edges(conn: &rusqlite::Connection, chain_id: i64) -> Result<Vec<REdge>, String> {
    let mut stmt = conn.prepare(
        "SELECT from_node_id, to_node_id, trigger_type, trigger_value
         FROM routine_edges WHERE chain_id=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![chain_id], |r| {
        Ok(REdge { from: r.get(0)?, to: r.get(1)?, trigger: r.get(2)?, value: r.get(3)? })
    }).map_err(|e| format!("Query error: {}", e))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn load_status(conn: &rusqlite::Connection, run_id: i64) -> Result<HashMap<i64, (String, String)>, String> {
    let mut stmt = conn.prepare(
        "SELECT node_id, state, updated_at FROM routine_node_status WHERE run_id=?1"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map(rusqlite::params![run_id], |r| {
        Ok((r.get::<_, i64>(0)?, (r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
    }).map_err(|e| format!("Query error: {}", e))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
