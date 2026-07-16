// One-shot repair for GitHub cursors created before TEXT primary keys were
// supported. Only outbound cursors for TEXT-id tables are cleared; pull/LAN
// state and INTEGER-id tables remain untouched.

use rusqlite::{Connection, OptionalExtension};

use crate::sync_owner::get_setting;

const REPLAY_MARKER: &str = "cloud_owner_gh_text_id_replay_v1";

pub(crate) fn prepare_text_id_replay(
    conn: &Connection,
    tables: &[&str],
) -> Result<Vec<String>, String> {
    if get_setting(conn, REPLAY_MARKER).is_some() {
        return Ok(Vec::new());
    }

    let mut reset = Vec::new();
    for table in tables {
        let id_type: Option<String> = conn
            .query_row(
                "SELECT type FROM pragma_table_info(?1) WHERE name='id'",
                rusqlite::params![table],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("id type {}: {}", table, e))?;
        if !id_type
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case("TEXT"))
        {
            continue;
        }

        let key = format!("cloud_owner_gh_push_{}", table);
        let changed = conn
            .execute(
                "DELETE FROM app_settings WHERE key=?1",
                rusqlite::params![key],
            )
            .map_err(|e| format!("reset cursor {}: {}", table, e))?;
        if changed > 0 {
            reset.push((*table).to_string());
        }
    }

    conn.execute(
        "INSERT INTO app_settings (key,value) VALUES (?1,?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![REPLAY_MARKER, chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|e| format!("save replay marker: {}", e))?;
    Ok(reset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(conn: &Connection, key: &str) -> Option<String> {
        get_setting(conn, key)
    }

    #[test]
    fn resets_only_text_push_cursors() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE events (id INTEGER PRIMARY KEY);
             CREATE TABLE sleep_sessions (id TEXT PRIMARY KEY);
             CREATE TABLE schedules (id TEXT PRIMARY KEY);
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_events','event-cursor');
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','sleep-cursor');
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_schedules','schedule-cursor');
             INSERT INTO app_settings VALUES ('cloud_owner_gh_pull_sha','pull-sha');
             INSERT INTO app_settings VALUES ('lan_cursor_sleep_sessions','lan-cursor');",
        )
        .unwrap();

        let reset = prepare_text_id_replay(
            &conn,
            &["events", "sleep_sessions", "schedules", "missing_table"],
        )
        .unwrap();

        assert_eq!(reset, vec!["sleep_sessions", "schedules"]);
        assert_eq!(
            setting(&conn, "cloud_owner_gh_push_events").as_deref(),
            Some("event-cursor")
        );
        assert_eq!(setting(&conn, "cloud_owner_gh_push_sleep_sessions"), None);
        assert_eq!(setting(&conn, "cloud_owner_gh_push_schedules"), None);
        assert_eq!(
            setting(&conn, "cloud_owner_gh_pull_sha").as_deref(),
            Some("pull-sha")
        );
        assert_eq!(
            setting(&conn, "lan_cursor_sleep_sessions").as_deref(),
            Some("lan-cursor")
        );
        assert!(setting(&conn, REPLAY_MARKER).is_some());
    }

    #[test]
    fn replay_preparation_runs_only_once() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE sleep_sessions (id TEXT PRIMARY KEY);
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','old');",
        )
        .unwrap();
        prepare_text_id_replay(&conn, &["sleep_sessions"]).unwrap();
        conn.execute(
            "INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','new')",
            [],
        )
        .unwrap();

        let reset = prepare_text_id_replay(&conn, &["sleep_sessions"]).unwrap();

        assert!(reset.is_empty());
        assert_eq!(
            setting(&conn, "cloud_owner_gh_push_sleep_sessions").as_deref(),
            Some("new")
        );
    }
}
