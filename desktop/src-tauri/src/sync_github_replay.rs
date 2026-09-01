// One-shot repair for GitHub cursors created before canonical timestamp
// ordering and tuple tie-breakers. Outbound cursors and pull HEAD are replayed;
// LAN state remains untouched.

use rusqlite::Connection;

use crate::sync_owner::get_setting_checked;

const REPLAY_MARKER: &str = "cloud_owner_gh_cursor_replay_v2";
const REPLAY_SCOPE: &str = "cloud_owner_gh_cursor_scope_v2";

pub(crate) fn prepare_cursor_v2_replay(
    conn: &Connection,
    tables: &[&str],
    scope_fingerprint: &str,
) -> Result<Vec<String>, String> {
    if get_setting_checked(conn, REPLAY_MARKER)?.is_some()
        && get_setting_checked(conn, REPLAY_SCOPE)?.as_deref() == Some(scope_fingerprint)
    {
        return Ok(Vec::new());
    }

    let transaction = conn
        .unchecked_transaction()
        .map_err(|error| format!("start GitHub cursor replay: {error}"))?;
    if get_setting_checked(&transaction, REPLAY_MARKER)?.is_some()
        && get_setting_checked(&transaction, REPLAY_SCOPE)?.as_deref() == Some(scope_fingerprint) {
        return Ok(Vec::new());
    }

    let mut reset = Vec::new();
    for table in tables {
        let key = format!("cloud_owner_gh_push_{}", table);
        let id_key = format!("{key}_id");
        let changed = transaction
            .execute(
                "DELETE FROM app_settings WHERE key IN (?1,?2)",
                rusqlite::params![key, id_key],
            )
            .map_err(|e| format!("reset cursor {}: {}", table, e))?;
        if changed > 0 {
            reset.push((*table).to_string());
        }
    }
    for key in [
        "cloud_owner_gh_push_tombstones",
        "cloud_owner_gh_push_tombstones_table",
        "cloud_owner_gh_push_tombstones_row_id",
        "cloud_owner_gh_pull_sha",
    ] {
        transaction
            .execute("DELETE FROM app_settings WHERE key=?1", [key])
            .map_err(|error| format!("reset GitHub cursor {key}: {error}"))?;
    }
    transaction
        .execute(
                "INSERT INTO app_settings (key,value) VALUES (?1,?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                rusqlite::params![REPLAY_MARKER, chrono::Utc::now().to_rfc3339()],
            )
            .map_err(|e| format!("save replay marker: {}", e))?;
    transaction
        .execute(
        "INSERT INTO app_settings (key,value) VALUES (?1,?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![REPLAY_SCOPE, scope_fingerprint],
    )
    .map_err(|e| format!("save replay scope: {}", e))?;
    transaction
        .commit()
        .map_err(|error| format!("commit GitHub cursor replay: {error}"))?;
    Ok(reset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(conn: &Connection, key: &str) -> Option<String> {
        get_setting_checked(conn, key).unwrap()
    }

    #[test]
    fn resets_all_push_and_pull_cursors_but_not_lan() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE events (id INTEGER PRIMARY KEY);
             CREATE TABLE sleep_sessions (id TEXT PRIMARY KEY);
             CREATE TABLE schedules (id TEXT PRIMARY KEY);
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_events','event-cursor');
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','sleep-cursor');
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions_id','t:sleep-id');
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_schedules','schedule-cursor');
             INSERT INTO app_settings VALUES ('cloud_owner_gh_pull_sha','pull-sha');
             INSERT INTO app_settings VALUES ('lan_cursor_sleep_sessions','lan-cursor');",
        )
        .unwrap();

        let reset = prepare_cursor_v2_replay(
            &conn,
            &["events", "sleep_sessions", "schedules", "missing_table"],
            "scope-a",
        )
        .unwrap();

        assert_eq!(reset, vec!["events", "sleep_sessions", "schedules"]);
        assert_eq!(
            setting(&conn, "cloud_owner_gh_push_events"), None);
        assert_eq!(setting(&conn, "cloud_owner_gh_push_sleep_sessions"), None);
        assert_eq!(setting(&conn, "cloud_owner_gh_push_sleep_sessions_id"),
            None
        );
        assert_eq!(setting(&conn, "cloud_owner_gh_push_schedules"), None);
        assert_eq!(
            setting(&conn, "cloud_owner_gh_pull_sha"), None);
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
        prepare_cursor_v2_replay(&conn, &["sleep_sessions"], "scope-a").unwrap();
        conn.execute(
            "INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','new')",
            [],
        )
        .unwrap();

        let reset = prepare_cursor_v2_replay(&conn, &["sleep_sessions"], "scope-a").unwrap();

        assert!(reset.is_empty());
        assert_eq!(
            setting(&conn, "cloud_owner_gh_push_sleep_sessions").as_deref(),
            Some("new")
        );
    }

    #[test]
    fn scope_change_replays_cursors_again() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','old');",
        )
        .unwrap();
        prepare_cursor_v2_replay(&conn, &["sleep_sessions"], "scope-a").unwrap();
        conn.execute(
            "INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','new')",
            [],
        )
        .unwrap();

        let reset = prepare_cursor_v2_replay(&conn, &["sleep_sessions"], "scope-b").unwrap();

        assert_eq!(reset, vec!["sleep_sessions"]);
        assert_eq!(setting(&conn, "cloud_owner_gh_push_sleep_sessions"), None);
        assert_eq!(setting(&conn, REPLAY_SCOPE).as_deref(), Some("scope-b"));
    }

    #[test]
    fn replay_scope_write_failure_rolls_back_cursor_deletes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings VALUES ('cloud_owner_gh_push_sleep_sessions','old');
             CREATE TRIGGER reject_replay_scope
             BEFORE INSERT ON app_settings
             WHEN NEW.key = '{REPLAY_SCOPE}'
             BEGIN
                 SELECT RAISE(ABORT, 'synthetic scope failure');
             END;"
        ))
        .unwrap();

        assert!(prepare_cursor_v2_replay(&conn, &["sleep_sessions"], "scope-a").is_err());
        assert_eq!(
            setting(&conn, "cloud_owner_gh_push_sleep_sessions").as_deref(),
            Some("old")
        );
        assert_eq!(setting(&conn, REPLAY_MARKER), None);
    }

    #[test]
    fn replay_marker_read_failure_is_not_suppressed() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(prepare_cursor_v2_replay(&conn, &[], "scope-a").is_err());
    }
}
