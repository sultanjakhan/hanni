-- Shared by the Android importer and the Rust relay. Additive only.
CREATE TABLE IF NOT EXISTS health_records(
    id TEXT PRIMARY KEY NOT NULL,
    source_store_id TEXT NOT NULL,
    record_type TEXT NOT NULL,
    hc_record_id TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK(source_revision > 0),
    metadata_modified_at TEXT NOT NULL,
    time_start_utc TEXT,
    time_end_utc TEXT,
    payload_version INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    payload_sha256 TEXT NOT NULL,
    is_deleted INTEGER NOT NULL CHECK(is_deleted IN (0,1)),
    deletion_basis TEXT,
    observed_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(source_store_id,record_type,hc_record_id)
);
-- hanni-statement
CREATE INDEX IF NOT EXISTS health_records_by_type_time
    ON health_records(record_type,is_deleted,time_start_utc);
-- hanni-statement
CREATE TABLE IF NOT EXISTS cloud_relay_control(
    id INTEGER PRIMARY KEY CHECK(id=1),applying INTEGER NOT NULL
);
-- hanni-statement
INSERT OR IGNORE INTO cloud_relay_control VALUES(1,0);
-- hanni-statement
CREATE TABLE IF NOT EXISTS cloud_relay_dirty(
    seq INTEGER PRIMARY KEY AUTOINCREMENT,table_name TEXT NOT NULL,row_id TEXT NOT NULL,
    UNIQUE(table_name,row_id)
);
-- hanni-statement
CREATE TRIGGER IF NOT EXISTS relay_health_records_INSERT AFTER INSERT ON health_records
    WHEN (SELECT applying FROM cloud_relay_control WHERE id=1)=0
    BEGIN INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id) VALUES('health_records',NEW.id); END;
-- hanni-statement
CREATE TRIGGER IF NOT EXISTS relay_health_records_UPDATE AFTER UPDATE ON health_records
    WHEN (SELECT applying FROM cloud_relay_control WHERE id=1)=0
    BEGIN INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id) VALUES('health_records',NEW.id); END;
-- hanni-statement
CREATE TRIGGER IF NOT EXISTS health_records_require_source_delete BEFORE DELETE ON health_records
    BEGIN SELECT RAISE(ABORT,'hc_source_deletion_required'); END;
