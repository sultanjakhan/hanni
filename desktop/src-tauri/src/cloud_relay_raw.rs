//! Source-preserving Health Connect archive. This table is intentionally not
//! part of legacy LAN/Firestore sync: only the encrypted relay transports it.
use super::*;

pub(super) const TABLE: &str = "health_records";
const SCHEMA: &str = include_str!("../android-plugin/src/main/assets/health-records-schema.sql");
const COLUMNS: &[&str] = &[
    "id",
    "source_store_id",
    "record_type",
    "hc_record_id",
    "source_revision",
    "metadata_modified_at",
    "time_start_utc",
    "time_end_utc",
    "payload_version",
    "payload_json",
    "payload_sha256",
    "is_deleted",
    "deletion_basis",
    "observed_at",
    "updated_at",
];
const TYPES: &[&str] = &[
    "ActiveCaloriesBurnedRecord",
    "BasalBodyTemperatureRecord",
    "BasalMetabolicRateRecord",
    "BloodGlucoseRecord",
    "BloodPressureRecord",
    "BodyFatRecord",
    "BodyTemperatureRecord",
    "BodyWaterMassRecord",
    "BoneMassRecord",
    "CervicalMucusRecord",
    "CyclingPedalingCadenceRecord",
    "DistanceRecord",
    "ElevationGainedRecord",
    "ExerciseSessionRecord",
    "FloorsClimbedRecord",
    "HeartRateRecord",
    "HeartRateVariabilityRmssdRecord",
    "HeightRecord",
    "HydrationRecord",
    "IntermenstrualBleedingRecord",
    "LeanBodyMassRecord",
    "MenstruationFlowRecord",
    "MenstruationPeriodRecord",
    "MindfulnessSessionRecord",
    "NutritionRecord",
    "OvulationTestRecord",
    "OxygenSaturationRecord",
    "PlannedExerciseSessionRecord",
    "PowerRecord",
    "RespiratoryRateRecord",
    "RestingHeartRateRecord",
    "SexualActivityRecord",
    "SkinTemperatureRecord",
    "SleepSessionRecord",
    "SpeedRecord",
    "StepsCadenceRecord",
    "StepsRecord",
    "TotalCaloriesBurnedRecord",
    "Vo2MaxRecord",
    "WeightRecord",
    "WheelchairPushesRecord",
];

pub(super) fn initialize(conn: &Connection) -> Result<(), String> {
    sql(conn.execute_batch(SCHEMA))
}

fn text<'a>(fields: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    fields
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| "relay_archive_invalid_field".into())
}
fn identity(store: &str, kind: &str, record: &str) -> String {
    let mut bytes = Vec::new();
    for value in [store, kind, record] {
        bytes.extend_from_slice(value.len().to_string().as_bytes());
        bytes.push(b':');
        bytes.extend_from_slice(value.as_bytes());
    }
    hash(&bytes)
}
fn canonical(value: &str) -> Result<String, String> {
    crate::sync_owner::canonical_sync_timestamp(value, "archive")
        .map_err(|_| "relay_archive_invalid_time".into())
}
fn payload_time(value: &Value) -> Result<String, String> {
    let seconds = value
        .get("seconds")
        .and_then(Value::as_str)
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or("relay_archive_invalid_time")?;
    let nanos = value
        .get("nanos")
        .and_then(Value::as_u64)
        .filter(|n| *n < 1_000_000_000)
        .ok_or("relay_archive_invalid_time")?;
    let time = chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, nanos as u32)
        .ok_or("relay_archive_invalid_time")?;
    canonical(&time.to_rfc3339())
}

/// A source store owns its monotonically increasing revision. Wall clocks on
/// receiving devices never override a newer source deletion/correction.
pub(super) fn apply(conn: &Connection, fields: &Map<String, Value>) -> Result<bool, String> {
    if fields.len() != COLUMNS.len() + 2
        || COLUMNS.iter().any(|name| !fields.contains_key(*name))
        || !fields.contains_key("_updated_at")
        || !fields.contains_key("_device_id")
    {
        return Err("relay_archive_invalid_fields".into());
    }
    let id = text(fields, "id")?;
    let store = text(fields, "source_store_id")?;
    let kind = text(fields, "record_type")?;
    let hc_id = text(fields, "hc_record_id")?;
    if uuid::Uuid::parse_str(store)
        .map(|v| v.to_string())
        .ok()
        .as_deref()
        != Some(store)
        || !TYPES.contains(&kind)
        || hc_id.is_empty()
        || hc_id.len() > 4096
        || id != identity(store, kind, hc_id)
    {
        return Err("relay_archive_invalid_identity".into());
    }
    let revision = fields["source_revision"]
        .as_i64()
        .filter(|v| *v > 0)
        .ok_or("relay_archive_invalid_revision")?;
    let deleted = fields["is_deleted"]
        .as_i64()
        .filter(|v| *v == 0 || *v == 1)
        .ok_or("relay_archive_invalid_deletion")?;
    if (deleted == 1 && fields["deletion_basis"] != "getChanges")
        || (deleted == 0 && !fields["deletion_basis"].is_null())
    {
        return Err("relay_archive_invalid_deletion".into());
    }
    if fields["payload_version"] != 1 {
        return Err("relay_archive_unknown_version".into());
    }
    let payload = text(fields, "payload_json")?;
    let digest = text(fields, "payload_sha256")?;
    if payload.len() > 8 * 1024 * 1024 || hash(payload.as_bytes()) != digest {
        return Err("relay_archive_digest_mismatch".into());
    }
    let value: Value = serde_json::from_str(payload).map_err(|_| "relay_archive_invalid_json")?;
    if value["v"] != 1 || value["record_type"] != kind {
        return Err("relay_archive_invalid_payload".into());
    }
    for name in [
        "metadata_modified_at",
        "observed_at",
        "updated_at",
        "_updated_at",
    ] {
        canonical(text(fields, name)?)?;
    }
    for name in ["time_start_utc", "time_end_utc"] {
        if !fields[name].is_null() {
            canonical(text(fields, name)?)?;
        }
    }
    if deleted == 1 && value["deleted"] == true {
        if value["hc_record_id"] != hc_id
            || value.as_object().is_none_or(|v| v.len() != 4)
            || !fields["time_start_utc"].is_null()
            || !fields["time_end_utc"].is_null()
        {
            return Err("relay_archive_invalid_deletion".into());
        }
    } else {
        if value["sdk"] != "androidx.health.connect:connect-client:1.1.0"
            || value["record"]["metadata"]["id"] != hc_id
            || payload_time(&value["record"]["metadata"]["lastModifiedTime"])?
                != canonical(text(fields, "metadata_modified_at")?)?
        {
            return Err("relay_archive_invalid_payload".into());
        }
    }
    let current: Option<(i64, String, i64)> = sql(conn
        .query_row(
            "SELECT source_revision,payload_sha256,is_deleted FROM health_records WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional())?;
    let exists = current.is_some();
    if let Some((old_rev, old_hash, old_deleted)) = current {
        if old_rev > revision {
            return Ok(false);
        }
        if old_rev == revision {
            if old_hash != digest || old_deleted != deleted {
                return Err("relay_archive_revision_conflict".into());
            }
            return Ok(false);
        }
    }
    let values = COLUMNS
        .iter()
        .map(|name| match &fields[*name] {
            Value::Null => Ok(rusqlite::types::Value::Null),
            Value::String(v) => Ok(rusqlite::types::Value::Text(v.clone())),
            Value::Number(v) => v
                .as_i64()
                .map(rusqlite::types::Value::Integer)
                .ok_or("relay_archive_invalid_field"),
            _ => Err("relay_archive_invalid_field"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let names = COLUMNS.join(",");
    let placeholders = vec!["?"; COLUMNS.len()].join(",");
    let assignments = COLUMNS
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, name)| format!("{name}=?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");
    // An UPSERT's conflict policy can override OR REPLACE inside our dirty
    // journal trigger. Separate INSERT/UPDATE keeps that journal idempotent.
    // The caller holds an IMMEDIATE transaction across the version comparison.
    let statement = if exists {
        format!("UPDATE health_records SET {assignments} WHERE id=?1")
    } else {
        format!("INSERT INTO health_records({names}) VALUES({placeholders})")
    };
    sql(conn.execute(&statement, rusqlite::params_from_iter(values)))?;
    Ok(true)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    pub(crate) fn record(revision: i64, deleted: bool) -> Map<String, Value> {
        let store = "c9dd6d90-c9f7-4b1d-9d9c-6f7e7b127e00";
        let hc_id = "source-идентификатор";
        let time = "2026-09-05T00:00:00Z";
        let payload=json!({"v":1,"sdk":"androidx.health.connect:connect-client:1.1.0","record_type":"StepsRecord",
            "record":{"metadata":{"id":hc_id,"lastModifiedTime":{"seconds":"1788566400","nanos":0}},
                "count":"9007199254740993"}}).to_string();
        json!({"id":identity(store,"StepsRecord",hc_id),"source_store_id":store,"record_type":"StepsRecord",
            "hc_record_id":hc_id,"source_revision":revision,"metadata_modified_at":time,
            "time_start_utc":time,"time_end_utc":time,"payload_version":1,"payload_sha256":hash(payload.as_bytes()),
            "payload_json":payload,"is_deleted":if deleted {1}else{0},"deletion_basis":if deleted {json!("getChanges")}else{Value::Null},
            "observed_at":time,"updated_at":time,"_updated_at":time,"_device_id":"sender"}).as_object().unwrap().clone()
    }
    #[test]
    fn source_revision_preserves_corrections_and_deletions_across_stale_replay() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let initial = record(1, false);
        assert!(apply(&conn, &initial).unwrap());
        assert!(apply(&conn, &record(2, true)).unwrap());
        assert!(!apply(&conn, &initial).unwrap());
        assert_eq!(
            scalar(&conn, "SELECT is_deleted FROM health_records").unwrap(),
            1
        );
        assert_eq!(
            scalar(&conn, "SELECT COUNT(*) FROM health_records").unwrap(),
            1
        );
        assert!(conn.execute("DELETE FROM health_records", []).is_err());
        assert!(!apply(&conn, &record(2, true)).unwrap());
    }
    #[test]
    fn conflicting_revision_and_damaged_archive_are_not_acknowledged() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        assert!(apply(&conn, &record(1, false)).unwrap());
        assert_eq!(
            apply(&conn, &record(1, true)).unwrap_err(),
            "relay_archive_revision_conflict"
        );
        let mut damaged = record(2, false);
        damaged.insert("payload_json".into(), json!("{}"));
        assert_eq!(
            apply(&conn, &damaged).unwrap_err(),
            "relay_archive_digest_mismatch"
        );
        assert_eq!(
            scalar(&conn, "SELECT source_revision FROM health_records").unwrap(),
            1
        );
    }
    #[test]
    fn archive_identity_is_length_delimited_and_preserves_unicode_bytes() {
        assert_ne!(identity("ab", "c", "d"), identity("a", "bc", "d"));
        assert_ne!(identity("a", "b", "é"), identity("a", "b", "e\u{301}"));
        assert_eq!(identity("a", "b", "c"), hash(b"1:a1:b1:c"));
    }
}
