-- Hanni Sync D1 Schema — stores cr-sqlite changesets
CREATE TABLE IF NOT EXISTS sync_changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tbl TEXT NOT NULL,
    pk TEXT NOT NULL,
    cid TEXT NOT NULL,
    val TEXT,
    col_version INTEGER NOT NULL,
    db_version INTEGER NOT NULL,
    site_id TEXT NOT NULL,
    cl INTEGER NOT NULL,
    seq INTEGER NOT NULL,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sync_changes_id ON sync_changes(id);
CREATE INDEX IF NOT EXISTS idx_sync_changes_site ON sync_changes(site_id);
