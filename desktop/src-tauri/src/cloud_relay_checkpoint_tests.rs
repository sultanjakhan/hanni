use super::*;

fn config(device: &str) -> RelayConfig {
    RelayConfig {
        v: 1,
        endpoint: "https://relay.example.workers.dev".into(),
        device_id: device.into(),
        key_id: "test-key".into(),
        token: B64.encode([3u8; 32]),
        key: B64.encode([4u8; 32]),
        enabled: true,
        sleep_source_store_id: None,
    }
}
fn fixture(conn: &mut Connection, cfg: &RelayConfig) {
    conn.execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
        INSERT INTO app_settings VALUES('device_id','writer-local');
        CREATE TABLE health_log(id TEXT PRIMARY KEY,date TEXT,type TEXT,value REAL,unit TEXT,notes TEXT,start_time TEXT,updated_at TEXT NOT NULL);
        CREATE TABLE sleep_sessions(id TEXT PRIMARY KEY,date TEXT,start_time TEXT,end_time TEXT,source TEXT,updated_at TEXT NOT NULL,
            UNIQUE(date,start_time,source));
        CREATE TABLE sleep_stages(id TEXT PRIMARY KEY,session_id TEXT REFERENCES sleep_sessions(id) ON DELETE CASCADE,
            start_time TEXT,end_time TEXT,stage TEXT,updated_at TEXT NOT NULL,UNIQUE(session_id,start_time,end_time,stage));
        CREATE TABLE heart_rate_samples(id TEXT PRIMARY KEY,date TEXT,time TEXT,bpm INTEGER,source TEXT DEFAULT 'health_connect',updated_at TEXT NOT NULL);").unwrap();
    crate::db::migrate_sync_meta(conn).unwrap();
    super::super::initialize(conn, cfg).unwrap();
    initialize(conn).unwrap();
}
fn connection(cfg: &RelayConfig) -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    fixture(&mut conn, cfg);
    conn
}
fn row(id: &str, value: f64) -> Row {
    Row {t:"health_log".into(),f:json!({"id":id,"date":"2026-09-05","type":"test","value":value,
        "updated_at":"2026-09-05T09:00:00Z","_updated_at":"2026-09-05T09:00:00Z","_device_id":"writer-peer"})
        .as_object().unwrap().clone()}
}
fn page(
    cfg: &RelayConfig,
    seq: i64,
    client_seq: i64,
    rows: Vec<Row>,
    fragment: Option<fragments::Fragment>,
) -> Page {
    let payload = Payload {
        v: 1,
        kind: if fragment.is_some() {
            "fragment"
        } else {
            "changes"
        }
        .into(),
        applied_seq: 0,
        rows,
        tombs: vec![],
        fragment,
    };
    let batch = encrypt(cfg, &payload, client_seq).unwrap();
    Page {
        next_cursor: seq,
        latest_seq: seq,
        has_more: false,
        batches: vec![StoredBatch {
            seq,
            client_seq,
            sender_device_id: cfg.device_id.clone(),
            batch_id: batch.batch_id,
            envelope_sha256: envelope_hash(&batch.envelope).unwrap(),
            envelope: batch.envelope,
        }],
    }
}
fn source() -> (Connection, RelayConfig) {
    let cfg = config("snapshot-source");
    let mut conn = connection(&cfg);
    apply_page(
        &mut conn,
        &cfg,
        0,
        page(
            &config("original-peer"),
            1,
            1,
            vec![row("remote-row", 12.0)],
            None,
        ),
    )
    .unwrap();
    (conn, cfg)
}
fn descriptor(source: &Connection, sender: &RelayConfig, receiver: &Connection) -> Descriptor {
    let job = load::<Upload>(source, "upload").unwrap().unwrap();
    for i in 0..job.chunk_count {
        let (env, digest) = load_part(source, "upload", i).unwrap();
        stage_part(receiver, "download", i, &env, &digest).unwrap();
    }
    Descriptor {
        checkpoint_id: job.checkpoint_id,
        base_seq: job.base_seq,
        generation: job.expected_generation + 1,
        uploader_device_id: sender.device_id.clone(),
        chunk_count: job.chunk_count,
        total_bytes: job.total_bytes,
        chunk_root_sha256: job.chunk_root_sha256,
        envelope_sha256: envelope_hash(&job.envelope).unwrap(),
        envelope: job.envelope,
    }
}
fn queues(conn: &Connection) -> (Vec<String>, Vec<String>) {
    let mut s = conn
        .prepare("SELECT body FROM cloud_relay_outbox ORDER BY local_seq")
        .unwrap();
    let out = s
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<String>>>()
        .unwrap();
    let mut s = conn
        .prepare("SELECT seq||':'||table_name||':'||row_id FROM cloud_relay_dirty ORDER BY seq")
        .unwrap();
    let dirty = s
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<String>>>()
        .unwrap();
    (out, dirty)
}
fn stage_plain(
    conn: &Connection,
    cfg: &RelayConfig,
    plain: &[u8],
    base: i64,
    applied: i64,
) -> Descriptor {
    let id = uuid::Uuid::new_v4().to_string();
    let mut digests = vec![];
    let mut total = 0;
    clear(conn, "download").unwrap();
    for (i, bytes) in plain.chunks(PART).enumerate() {
        let env = seal(cfg, &id, base, Some(i), bytes).unwrap();
        let digest = envelope_hash(&env).unwrap();
        total += encode(&env).unwrap().len();
        stage_part(conn, "download", i, &env, &digest).unwrap();
        digests.push(digest);
    }
    let root = hash(encode(&digests).unwrap().as_bytes());
    let Line::Header(h) = parse::<Line>(plain.split(|b| *b == b'\n').next().unwrap()).unwrap()
    else {
        panic!()
    };
    let m = Manifest {
        v: 1,
        schema: SCHEMA.into(),
        tables: tables(),
        base_seq: base,
        applied_seq: applied,
        chunk_count: digests.len(),
        chunk_root_sha256: root.clone(),
        plain_bytes: plain.len(),
        plain_sha256: hash(plain),
        receipts: h.receipts,
    };
    let env = seal(cfg, &id, base, None, encode(&m).unwrap().as_bytes()).unwrap();
    Descriptor {
        checkpoint_id: id,
        base_seq: base,
        generation: 1,
        uploader_device_id: cfg.device_id.clone(),
        chunk_count: digests.len(),
        total_bytes: total,
        chunk_root_sha256: root,
        envelope_sha256: envelope_hash(&env).unwrap(),
        envelope: env,
    }
}

#[test]
fn snapshot_with_pending_changes_survives_restart_without_acknowledging_outbox() {
    let path = std::env::temp_dir().join(format!("hanni-checkpoint-{}.db", uuid::Uuid::new_v4()));
    let cfg = config("source");
    let mut conn = Connection::open(&path).unwrap();
    fixture(&mut conn, &cfg);
    apply_page(
        &mut conn,
        &cfg,
        0,
        page(&config("peer"), 1, 1, vec![row("remote", 1.0)], None),
    )
    .unwrap();
    conn.execute("INSERT INTO health_log(id,type,value,notes,updated_at) VALUES('pending','test',3,'synthetic-private-value','2026-09-05T10:00:00Z')",[]).unwrap();
    enqueue(&mut conn, &cfg).unwrap();
    conn.execute("UPDATE health_log SET value=4 WHERE id='pending'", [])
        .unwrap();
    let before = queues(&conn);
    assert!(capture(&mut conn, &cfg, 0).unwrap());
    assert_eq!(queues(&conn), before);
    let body: String = conn
        .query_row(
            "SELECT body FROM cloud_relay_checkpoint_jobs WHERE direction='upload'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let (env, digest) = load_part(&conn, "upload", 0).unwrap();
    assert!(!body.contains("synthetic-private-value"));
    assert!(!encode(&env).unwrap().contains("synthetic-private-value"));
    drop(conn);
    let mut conn = Connection::open(&path).unwrap();
    assert!(capture(&mut conn, &cfg, 0).unwrap());
    assert_eq!(queues(&conn), before);
    assert_eq!(load_part(&conn, "upload", 0).unwrap().1, digest);
    assert_eq!(
        conn.query_row(
            "SELECT body FROM cloud_relay_checkpoint_jobs WHERE direction='upload'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        body
    );
    drop(conn);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn snapshot_merge_preserves_local_dirty_outbox_and_legacy_unresolved_floor() {
    let (mut src, sender) = source();
    src.execute("INSERT INTO cloud_relay_unresolved_tombs VALUES('heart_rate_samples','historical-missing','2026-09-05T09:00:00Z',1)",[]).unwrap();
    assert!(capture(&mut src, &sender, 0).unwrap());
    let cfg = config("receiver");
    let mut target = connection(&cfg);
    target.execute("INSERT INTO health_log(id,type,value,updated_at) VALUES('local','test',99,'2026-09-05T10:00:00Z')",[]).unwrap();
    enqueue(&mut target, &cfg).unwrap();
    target
        .execute("UPDATE health_log SET value=101 WHERE id='local'", [])
        .unwrap();
    target.execute("INSERT INTO cloud_relay_unresolved_tombs VALUES('heart_rate_samples','receiver-missing','2026-09-05T09:00:00Z',1)",[]).unwrap();
    let before = queues(&target);
    let d = descriptor(&src, &sender, &target);
    assert_eq!(install(&mut target, &cfg, &d).unwrap(), 1);
    assert_eq!(queues(&target), before);
    assert_eq!(
        target
            .query_row("SELECT value FROM health_log WHERE id='local'", [], |r| r
                .get::<_, f64>(
                0
            ))
            .unwrap(),
        101.0
    );
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM cloud_relay_unresolved_tombs").unwrap(),
        2
    );
    assert_eq!(fragments::applied_cursor(&target).unwrap(), 0);
    assert_eq!(
        scalar(&target, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &target,
            "SELECT client_seq FROM cloud_relay_sender_watermarks WHERE device_id='original-peer'"
        )
        .unwrap(),
        1
    );
}

#[test]
fn corrupted_or_missing_parts_do_not_change_rows_or_cursor() {
    let (mut src, sender) = source();
    src.execute(
        "UPDATE health_log SET notes=?1 WHERE id='remote-row'",
        ["x".repeat(140_000)],
    )
    .unwrap();
    capture(&mut src, &sender, 0).unwrap();
    let cfg = config("receiver");
    let mut target = connection(&cfg);
    let d = descriptor(&src, &sender, &target);
    assert!(d.chunk_count > 2);
    target
        .execute(
            "DELETE FROM cloud_relay_checkpoint_parts WHERE direction='download' AND part=1",
            [],
        )
        .unwrap();
    assert!(install(&mut target, &cfg, &d).is_err());
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM health_log").unwrap(),
        0
    );
    assert_eq!(
        scalar(&target, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        0
    );
    let (env, digest) = load_part(&src, "upload", 1).unwrap();
    stage_part(&target, "download", 1, &env, &digest).unwrap();
    target.execute("UPDATE cloud_relay_checkpoint_parts SET digest=?1 WHERE direction='download' AND part=0",["0".repeat(64)]).unwrap();
    assert!(install(&mut target, &cfg, &d).is_err());
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM health_log").unwrap(),
        0
    );
}

#[test]
fn authenticated_malformed_last_row_rolls_back_entire_snapshot() {
    let (mut src, sender) = source();
    capture(&mut src, &sender, 0).unwrap();
    let cfg = config("receiver");
    let mut target = connection(&cfg);
    let d = descriptor(&src, &sender, &target);
    let (_, mut plain) = verified_plain(&target, &cfg, &d).unwrap();
    let mut bad = row("bad", 2.0);
    bad.f
        .insert("updated_at".into(), json!("invalid-source-time"));
    append(&mut plain, &Line::Row(bad)).unwrap();
    let d = stage_plain(&target, &sender, &plain, 1, 1);
    assert!(install(&mut target, &cfg, &d).is_err());
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM health_log").unwrap(),
        0
    );
    assert_eq!(
        scalar(&target, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        0
    );
    assert_eq!(
        scalar(&target, "SELECT applying FROM cloud_relay_control").unwrap(),
        0
    );
    assert_eq!(
        scalar(
            &target,
            "SELECT COUNT(*) FROM cloud_relay_sender_watermarks"
        )
        .unwrap(),
        0
    );
}

#[test]
fn checkpoint_aad_binds_sender_base_index_and_manifest_namespace() {
    let cfg = config("sender");
    let id = uuid::Uuid::new_v4().to_string();
    let env = seal(&cfg, &id, 2, Some(0), b"hello").unwrap();
    assert_eq!(
        open(&cfg, "sender", &id, 2, Some(0), &env).unwrap(),
        b"hello"
    );
    assert!(open(&cfg, "other", &id, 2, Some(0), &env).is_err());
    assert!(open(&cfg, "sender", &id, 3, Some(0), &env).is_err());
    assert!(open(&cfg, "sender", &id, 2, Some(1), &env).is_err());
    assert!(open(&cfg, "sender", &id, 2, None, &env).is_err());
}

#[test]
fn snapshot_mid_own_fragment_series_restores_prefix_and_accepts_tail() {
    let sender = config("sender");
    let mut src = connection(&sender);
    let mut large = row("large", 7.0);
    large.f.insert("notes".into(), json!("test".repeat(35_000)));
    let bytes = serde_json::to_vec(&large).unwrap();
    let count = bytes.len().div_ceil(40_000);
    let pieces: Vec<fragments::Fragment> = bytes
        .chunks(40_000)
        .enumerate()
        .map(|(i, p)| {
            from_value(json!({
        "sha256":hash(&bytes),"part":i,"parts":count,"bytes":bytes.len(),"data":B64.encode(p)}))
            .unwrap()
        })
        .collect();
    // Publisher already owns the complete local row and receives its own ACKed prefix.
    apply_authenticated_row(&src, large, "2026-09-05T10:00:00Z").unwrap();
    for (i, p) in pieces.iter().take(2).enumerate() {
        apply_page(
            &mut src,
            &sender,
            i as i64,
            page(&sender, i as i64 + 1, i as i64 + 1, vec![], Some(p.clone())),
        )
        .unwrap();
    }
    assert_eq!(fragments::applied_cursor(&src).unwrap(), 0);
    capture(&mut src, &sender, 0).unwrap();
    let cfg = config("receiver");
    let mut target = connection(&cfg);
    let d = descriptor(&src, &sender, &target);
    install(&mut target, &cfg, &d).unwrap();
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        2
    );
    assert_eq!(fragments::applied_cursor(&target).unwrap(), 0);
    for (i, p) in pieces.into_iter().enumerate().skip(2) {
        apply_page(
            &mut target,
            &cfg,
            i as i64,
            page(&sender, i as i64 + 1, i as i64 + 1, vec![], Some(p)),
        )
        .unwrap();
    }
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        0
    );
    assert_eq!(fragments::applied_cursor(&target).unwrap(), count as i64);
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM health_log WHERE id='large'").unwrap(),
        1
    );
}

#[test]
fn snapshot_cannot_erase_authenticated_sender_positions() {
    let (mut src, sender) = source();
    src.execute("UPDATE cloud_relay_state SET receive_seq=2", [])
        .unwrap();
    capture(&mut src, &sender, 0).unwrap();
    let cfg = config("receiver");
    let mut target = connection(&cfg);
    apply_page(
        &mut target,
        &cfg,
        0,
        page(&config("another-peer"), 1, 1, vec![], None),
    )
    .unwrap();
    let d = descriptor(&src, &sender, &target);
    assert_eq!(
        install(&mut target, &cfg, &d).unwrap_err(),
        "relay_checkpoint_watermark_missing"
    );
    assert_eq!(
        scalar(&target, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        1
    );
    assert_eq!(
        scalar(&target, "SELECT COUNT(*) FROM health_log").unwrap(),
        0
    );
}

#[test]
#[ignore = "Run with the isolated workerd native-checkpoint.mjs runner"]
fn real_workerd_checkpoint_roundtrip() {
    let endpoint = std::env::var("HANNI_RELAY_TEST_URL").expect("isolated workerd URL required");
    assert!(endpoint.starts_with("http://127.0.0.1:"));
    let mut a_cfg = config("device-a");
    a_cfg.endpoint = endpoint.clone();
    let mut b_cfg = config("device-b");
    b_cfg.endpoint = endpoint;
    b_cfg.token = B64.encode([5u8; 32]);
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let a_path = dir.path().join("a.db");
    let b_path = dir.path().join("b.db");
    let mut a = Connection::open(&a_path).unwrap();
    fixture(&mut a, &a_cfg);
    let mut b = Connection::open(&b_path).unwrap();
    fixture(&mut b, &b_cfg);
    a.execute("INSERT INTO health_log(id,type,value,notes,updated_at) VALUES('large','test',7,?1,'2026-09-05T09:00:00Z')",
        ["synthetic".repeat(16_000)]).unwrap();
    enqueue(&mut a, &a_cfg).unwrap();
    assert!(scalar(&a, "SELECT COUNT(*) FROM cloud_relay_outbox").unwrap() > 2);
    assert_eq!(upload(&mut a, &a_cfg, &http).unwrap(), 1);
    assert_eq!(upload(&mut a, &a_cfg, &http).unwrap(), 1);
    pull(&mut a, &a_cfg, &http).unwrap();
    assert_eq!(
        scalar(&a, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        2
    );
    assert_eq!(
        scalar(&a, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        2
    );
    let a_pending = queues(&a);
    assert!(capture(&mut a, &a_cfg, 0).unwrap());
    // One bounded step persists upload progress. Both clients really close and
    // reopen their SQLite databases before the remaining HTTP operations.
    upload_step(&mut a, &a_cfg, &http).unwrap();
    drop(a);
    a = Connection::open(&a_path).unwrap();
    for _ in 0..30 {
        if !upload_step(&mut a, &a_cfg, &http).unwrap() {
            break;
        }
    }
    assert!(load::<Upload>(&a, "upload").unwrap().is_none());
    assert_eq!(queues(&a), a_pending);
    let published = latest(&a, &a_cfg, &http).unwrap().unwrap();
    assert_eq!(published.base_seq, 2);
    let compacted = request(
        http.get(b_cfg.url("/v1/batches?after=0&limit=16"))
            .bearer_auth(&b_cfg.token),
    )
    .unwrap();
    assert_eq!(compacted.status, 409);
    assert_eq!(compacted.value["error"], "checkpoint_required");
    let cleanup = request(
        http.post(a_cfg.url("/v1/maintenance"))
            .bearer_auth(&a_cfg.token)
            .json(&json!({})),
    )
    .unwrap();
    assert!((200..300).contains(&cleanup.status));
    b.execute("INSERT INTO health_log(id,type,value,updated_at) VALUES('local','test',9,'2026-09-05T10:00:00Z')",[]).unwrap();
    enqueue(&mut b, &b_cfg).unwrap();
    b.execute("UPDATE health_log SET value=11 WHERE id='local'", [])
        .unwrap();
    let b_pending = queues(&b);
    pull(&mut b, &b_cfg, &http).unwrap();
    drop(b);
    b = Connection::open(&b_path).unwrap();
    for _ in 0..30 {
        if scalar(&b, "SELECT receive_seq FROM cloud_relay_state").unwrap() == 2 {
            break;
        }
        pull(&mut b, &b_cfg, &http).unwrap();
    }
    assert_eq!(
        scalar(&b, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        2
    );
    assert_eq!(queues(&b), b_pending);
    assert_eq!(
        scalar(&b, "SELECT LENGTH(notes) FROM health_log WHERE id='large'").unwrap(),
        144_000
    );
    assert_eq!(
        scalar(&b, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        2
    );
    assert_eq!(fragments::applied_cursor(&b).unwrap(), 0);
    // Finish the same immutable outgoing series after its prefix was compacted.
    while upload(&mut a, &a_cfg, &http).unwrap() > 0 {}
    for _ in 0..5 {
        if !pull(&mut b, &b_cfg, &http).unwrap().1 {
            break;
        }
    }
    assert_eq!(
        scalar(&b, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        0
    );
    assert!(fragments::applied_cursor(&b).unwrap() > 2);
    assert_eq!(
        scalar(&b, "SELECT COUNT(*) FROM health_log WHERE id='large'").unwrap(),
        1
    );
    assert_eq!(queues(&b), b_pending);
}

#[test]
fn receiver_known_identity_can_resolve_the_publishers_unresolved_floor() {
    let (mut src, sender) = source();
    src.execute("INSERT INTO cloud_relay_unresolved_tombs VALUES('heart_rate_samples','known-here','2026-09-05T11:00:00Z',1)",[]).unwrap();
    capture(&mut src, &sender, 0).unwrap();
    for (version, expected_rows) in [("2026-09-05T10:00:00Z", 0), ("2026-09-05T12:00:00Z", 1)] {
        let cfg = config("receiver");
        let mut target = connection(&cfg);
        // The real remote-apply helper preserves this controlled historical
        // version. A direct local INSERT would correctly stamp wall-clock HLC.
        let fields = json!({"id":"known-here","date":"2026-09-05","time":"10:00:00","bpm":75,
            "source":"health_connect","updated_at":version,"_updated_at":version,"_device_id":"known-peer"});
        apply_authenticated_row(
            &target,
            Row {
                t: "heart_rate_samples".into(),
                f: fields.as_object().unwrap().clone(),
            },
            "2026-09-05T12:00:00Z",
        )
        .unwrap();
        let d = descriptor(&src, &sender, &target);
        assert_eq!(manifest(&cfg, &d).unwrap().applied_seq, 0);
        install(&mut target, &cfg, &d).unwrap();
        assert_eq!(
            scalar(&target, "SELECT COUNT(*) FROM heart_rate_samples").unwrap(),
            expected_rows
        );
        assert_eq!(
            scalar(&target, "SELECT COUNT(*) FROM cloud_relay_unresolved_tombs").unwrap(),
            0
        );
        assert_eq!(fragments::applied_cursor(&target).unwrap(), 1);
    }
}
