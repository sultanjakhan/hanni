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
fn fixture(conn: &Connection) {
    conn.execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
        INSERT INTO app_settings VALUES('device_id','writer-local');
        CREATE TABLE health_log(id TEXT PRIMARY KEY,date TEXT,type TEXT,value REAL,unit TEXT,notes TEXT,start_time TEXT,updated_at TEXT NOT NULL);
        CREATE TABLE sleep_sessions(id TEXT PRIMARY KEY,date TEXT,start_time TEXT,end_time TEXT,source TEXT,updated_at TEXT NOT NULL,
            UNIQUE(date,start_time,source));
        CREATE TABLE sleep_stages(id TEXT PRIMARY KEY,session_id TEXT REFERENCES sleep_sessions(id) ON DELETE CASCADE,
            start_time TEXT,end_time TEXT,stage TEXT,updated_at TEXT NOT NULL,UNIQUE(session_id,start_time,end_time,stage));
        CREATE TABLE heart_rate_samples(id TEXT PRIMARY KEY,date TEXT,time TEXT,bpm INTEGER,source TEXT DEFAULT 'health_connect',updated_at TEXT NOT NULL);").unwrap();
    crate::db::migrate_sync_meta(conn).unwrap();
}
fn connection() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    fixture(&conn);
    conn
}

#[test]
fn raw_sleep_projection_uses_real_migrations_without_echo_or_legacy_deletion() {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
    let mut conn = Connection::open_in_memory().unwrap();
    crate::db::init_db(&conn).unwrap();
    crate::db::migrate_events_source(&conn);
    crate::db::migrate_timeline(&conn);
    crate::db::migrate_timeline_today(&conn);
    crate::db::migrate_sleep(&conn);
    crate::db::migrate_health_log_start_time(&conn);
    crate::db::migrate_sleep_to_uuid_pk(&conn);
    crate::db::migrate_health_to_uuid_pk(&conn);
    crate::db::migrate_sync_meta(&conn).unwrap();
    let mut cfg = config("device-a");
    cfg.sleep_source_store_id = Some("c9dd6d90-c9f7-4b1d-9d9c-6f7e7b127e00".into());
    initialize(&mut conn, &cfg).unwrap();
    conn.execute("INSERT INTO sleep_sessions(id,date,start_time,end_time,duration_minutes,source) VALUES('legacy','2026-09-01','01:00','08:00',420,'health_connect')", []).unwrap();
    conn.execute("DELETE FROM cloud_relay_dirty", []).unwrap();
    let raw_id = "a".repeat(64);
    let body = json!({"v":1,"sdk":"androidx.health.connect:connect-client:1.1.0","record_type":"SleepSessionRecord","record":{
        "metadata":{"id":"synthetic-sleep"},"startTime":{"seconds":"1788289200","nanos":0},
        "endTime":{"seconds":"1788314400","nanos":0},"startZoneOffset":18000,"endZoneOffset":18000,"notes":null,"stages":[]}}).to_string();
    conn.execute("INSERT INTO health_records(id,source_store_id,record_type,hc_record_id,source_revision,metadata_modified_at,time_start_utc,time_end_utc,payload_version,payload_json,payload_sha256,is_deleted,observed_at,updated_at) VALUES(?1,?2,'SleepSessionRecord','synthetic-sleep',1,'2026-09-03T00:00:00Z','2026-09-01T19:00:00Z','2026-09-02T02:00:00Z',1,?3,?4,0,'2026-09-03T00:00:00Z','2026-09-03T00:00:00Z')",params![raw_id,cfg.sleep_source_store_id,body,hash(body.as_bytes())]).unwrap();
    let result = project_local(&mut conn, &cfg).unwrap();
    assert_eq!(result.records, 1);
    assert_eq!(result.status, "projected");
    assert!(crate::health_raw_sleep_projection::ensure_user_editable(&conn,"sleep_sessions",&format!("raw-sleep:{raw_id}")).is_err());
    assert!(crate::health_raw_sleep_projection::ensure_user_editable(&conn,"sleep_sessions","legacy").is_ok());
    assert_eq!(database_status(&conn).unwrap()["projection"]["projection_revision"],"1");
    assert_eq!(scalar(&conn,"SELECT COUNT(*) FROM sleep_sessions").unwrap(),2);
    assert_eq!(scalar(&conn,"SELECT COUNT(*) FROM events WHERE source GLOB 'auto_health_raw:*'").unwrap(),1);
    assert_eq!(scalar(&conn,"SELECT COUNT(*) FROM timeline_blocks WHERE source GLOB 'auto_health_raw:*'").unwrap(),1);
    assert_eq!(scalar(&conn,"SELECT COUNT(*) FROM cloud_relay_dirty WHERE table_name!='health_records'").unwrap(),0);
    let tombs_before=scalar(&conn,"SELECT COUNT(*) FROM sync_tombstones").unwrap();
    conn.execute("UPDATE health_records SET source_revision=2,is_deleted=1,deletion_basis='changes_token' WHERE id=?1",[&raw_id]).unwrap();
    assert_eq!(project_local(&mut conn, &cfg).unwrap().records,1);
    assert_eq!(scalar(&conn,"SELECT COUNT(*) FROM sleep_sessions").unwrap(),1);
    assert_eq!(scalar(&conn,"SELECT COUNT(*) FROM sleep_sessions WHERE id='legacy'").unwrap(),1);
    assert_eq!(scalar(&conn,"SELECT COUNT(*) FROM sync_tombstones").unwrap(),tombs_before);
    assert_eq!(scalar(&conn,"SELECT remote_apply FROM sync_apply_context").unwrap(),0);
    assert_eq!(scalar(&conn,"SELECT applying FROM cloud_relay_control").unwrap(),0);
}
fn row(id: &str, value: f64) -> Row {
    Row {
        t: "health_log".into(),
        f: json!({"id":id,"date":"2026-09-05","type":"test",
        "value":value,"updated_at":"2026-09-05T09:00:00Z","_updated_at":"2026-09-05T09:00:00Z",
        "_device_id":"writer-peer"})
        .as_object()
        .unwrap()
        .clone(),
    }
}
fn payload(rows: Vec<Row>) -> Payload {
    Payload {
        v: 1,
        kind: "changes".into(),
        applied_seq: 0,
        rows,
        tombs: vec![],
        fragment: None,
    }
}
fn stored(cfg: &RelayConfig, seq: i64, payload: &Payload) -> StoredBatch {
    stored_at(cfg, seq, seq, payload)
}
fn stored_at(cfg: &RelayConfig, seq: i64, client_seq: i64, payload: &Payload) -> StoredBatch {
    let batch = encrypt(cfg, payload, client_seq).unwrap();
    StoredBatch {
        client_seq: batch.client_seq,
        seq,
        sender_device_id: cfg.device_id.clone(),
        batch_id: batch.batch_id,
        envelope_sha256: envelope_hash(&batch.envelope).unwrap(),
        envelope: batch.envelope,
    }
}
fn page(batch: StoredBatch) -> Page {
    Page {
        next_cursor: batch.seq,
        latest_seq: batch.seq,
        has_more: false,
        batches: vec![batch],
    }
}

#[test]
fn authenticated_envelope_matches_wire_and_rejects_tamper() {
    let cfg = config("device-a");
    let mut packet = stored(&cfg, 1, &payload(vec![row("r1", 42.0)]));
    let canonical = serde_json::to_string(&packet.envelope).unwrap();
    assert!(canonical.starts_with("{\"v\":1,\"alg\":\"XChaCha20-Poly1305\",\"key_id\":"));
    assert_eq!(decrypt(&cfg, &packet).unwrap().rows[0].f["value"], 42.0);
    packet.sender_device_id = "different-sender".into();
    assert!(decrypt(&cfg, &packet).is_err());
    packet.sender_device_id = cfg.device_id.clone();
    let mut damaged = B64.decode(&packet.envelope.ciphertext).unwrap();
    damaged[0] ^= 1;
    packet.envelope.ciphertext = B64.encode(damaged);
    packet.envelope_sha256 = envelope_hash(&packet.envelope).unwrap();
    assert!(decrypt(&cfg, &packet).is_err());
}

#[test]
fn config_rejects_unsafe_origins_and_server_incompatible_ids() {
    let cfg = config("device-a");
    for endpoint in [
        "http://outside.test",
        "https://user:pass@relay.test",
        "https://relay.test/path",
        "https://relay.test?token=x",
    ] {
        let mut invalid = cfg.clone();
        invalid.endpoint = endpoint.into();
        assert!(RelayConfig::parse(&serde_json::to_string(&invalid).unwrap()).is_err());
    }
    let mut invalid = cfg;
    invalid.device_id = "a".repeat(65);
    assert!(RelayConfig::parse(&serde_json::to_string(&invalid).unwrap()).is_err());
}

#[test]
fn durable_outbox_survives_restart_and_does_not_reencrypt_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("relay.db");
    let cfg = config("device-a");
    let body = {
        let mut conn = Connection::open(&path).unwrap();
        fixture(&conn);
        conn.execute("INSERT INTO health_log(id,value,updated_at) VALUES('old-gap',1,'2020-01-01T00:00:00Z')",[]).unwrap();
        initialize(&mut conn, &cfg).unwrap();
        assert!(enqueue(&mut conn, &cfg).unwrap());
        assert_eq!(
            scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_dirty").unwrap(),
            0
        );
        conn.query_row("SELECT body FROM cloud_relay_outbox", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap()
    };
    let mut restarted = Connection::open(path).unwrap();
    initialize(&mut restarted, &cfg).unwrap();
    assert!(enqueue(&mut restarted, &cfg).unwrap());
    assert_eq!(
        body,
        restarted
            .query_row("SELECT body FROM cloud_relay_outbox", [], |r| r
                .get::<_, String>(0))
            .unwrap()
    );
    restarted
        .execute("UPDATE health_log SET value=2 WHERE id='old-gap'", [])
        .unwrap();
    assert_eq!(
        scalar(&restarted, "SELECT COUNT(*) FROM cloud_relay_dirty").unwrap(),
        1
    );
    assert_eq!(
        body,
        restarted
            .query_row("SELECT body FROM cloud_relay_outbox", [], |r| r
                .get::<_, String>(0))
            .unwrap()
    );
}

#[test]
fn malformed_batch_rolls_back_rows_cursor_and_receipt() {
    let mut conn = connection();
    let cfg = config("receiver");
    initialize(&mut conn, &cfg).unwrap();
    let mut bad = row("bad", 2.0);
    bad.t = "app_settings".into();
    let packet = stored(&config("sender"), 1, &payload(vec![row("good", 1.0), bad]));
    assert!(apply_page(&mut conn, &cfg, 0, page(packet)).is_err());
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM health_log").unwrap(), 0);
    assert_eq!(
        scalar(&conn, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT applying FROM cloud_relay_control").unwrap(),
        0
    );
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_receipts").unwrap(),
        0
    );
}

#[test]
fn committed_delivery_is_idempotent_and_receipts_do_not_loop() {
    let mut conn = connection();
    let cfg = config("receiver");
    initialize(&mut conn, &cfg).unwrap();
    let sender = config("sender");
    assert_eq!(
        apply_page(
            &mut conn,
            &cfg,
            0,
            page(stored(&sender, 1, &payload(vec![row("r", 1.0)])))
        )
        .unwrap(),
        1
    );
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_dirty").unwrap(),
        0
    );
    assert!(enqueue(&mut conn, &cfg).unwrap());
    let body: String = conn
        .query_row("SELECT body FROM cloud_relay_outbox", [], |r| r.get(0))
        .unwrap();
    let b: Batch = serde_json::from_str(&body).unwrap();
    let digest = envelope_hash(&b.envelope).unwrap();
    let acknowledgement = StoredBatch {
        client_seq: b.client_seq,
        seq: 2,
        sender_device_id: cfg.device_id.clone(),
        batch_id: b.batch_id,
        envelope: b.envelope,
        envelope_sha256: digest,
    };
    assert_eq!(decrypt(&cfg, &acknowledgement).unwrap().kind, "receipt");
    assert_eq!(
        apply_page(&mut conn, &cfg, 1, page(acknowledgement)).unwrap(),
        0
    );
    let mut receipt = payload(vec![]);
    receipt.kind = "receipt".into();
    receipt.applied_seq = 2;
    apply_page(&mut conn, &cfg, 2, page(stored_at(&sender, 3, 2, &receipt))).unwrap();
    assert_eq!(
        scalar(&conn, "SELECT receipt_needed FROM cloud_relay_state").unwrap(),
        0
    );
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM health_log").unwrap(), 1);
}

#[test]
fn sequence_gap_and_cursor_ahead_are_not_acknowledged() {
    let mut conn = connection();
    let cfg = config("receiver");
    initialize(&mut conn, &cfg).unwrap();
    assert!(apply_page(
        &mut conn,
        &cfg,
        0,
        page(stored(&config("sender"), 2, &payload(vec![])))
    )
    .is_err());
    assert_eq!(
        scalar(&conn, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        0
    );
    let empty = Page {
        batches: vec![],
        next_cursor: 100,
        latest_seq: 100,
        has_more: false,
    };
    assert!(apply_page(&mut conn, &cfg, 0, empty).is_err());
}

#[test]
fn sender_sequence_is_authenticated_and_replay_does_not_advance_cursor() {
    let cfg = config("receiver");
    let sender = config("sender");
    let mut conn = connection();
    initialize(&mut conn, &cfg).unwrap();
    apply_page(
        &mut conn,
        &cfg,
        0,
        page(stored(&sender, 1, &payload(vec![row("a", 1.0)]))),
    )
    .unwrap();
    let replay = stored_at(&sender, 2, 1, &payload(vec![row("b", 2.0)]));
    assert_eq!(
        apply_page(&mut conn, &cfg, 1, page(replay)).unwrap_err(),
        "relay_sender_sequence_gap"
    );
    assert_eq!(
        scalar(&conn, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT client_seq FROM cloud_relay_sender_watermarks"
        )
        .unwrap(),
        1
    );
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM health_log").unwrap(), 1);
    let mut tampered = stored_at(&sender, 2, 1, &payload(vec![]));
    tampered.client_seq = 2;
    assert!(decrypt(&cfg, &tampered).is_err());
    apply_page(
        &mut conn,
        &cfg,
        1,
        page(stored_at(&sender, 2, 2, &payload(vec![row("b", 2.0)]))),
    )
    .unwrap();
    assert_eq!(
        scalar(
            &conn,
            "SELECT client_seq FROM cloud_relay_sender_watermarks"
        )
        .unwrap(),
        2
    );
}

#[test]
fn changed_pairing_preserves_existing_queue() {
    let mut conn = connection();
    let cfg = config("one");
    initialize(&mut conn, &cfg).unwrap();
    conn.execute(
        "INSERT INTO health_log(id,value,updated_at) VALUES('r',1,'2026-09-05T00:00:00Z')",
        [],
    )
    .unwrap();
    enqueue(&mut conn, &cfg).unwrap();
    assert!(initialize(&mut conn, &config("other")).is_err());
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_outbox").unwrap(),
        1
    );
}

fn large_packets(sender: &RelayConfig) -> Vec<StoredBatch> {
    let mut conn = connection();
    initialize(&mut conn, sender).unwrap();
    conn.execute("INSERT INTO health_log(id,type,value,notes,updated_at) VALUES('large','test',1,?1,'2026-09-05T00:00:00Z')", ["x".repeat(130_000)]).unwrap();
    enqueue(&mut conn, sender).unwrap();
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_dirty").unwrap(),
        0
    );
    let mut stmt = conn
        .prepare("SELECT body FROM cloud_relay_outbox ORDER BY local_seq")
        .unwrap();
    let bodies = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(bodies.len(), 4);
    bodies
        .into_iter()
        .enumerate()
        .map(|(i, body)| {
            let batch: Batch = serde_json::from_str(&body).unwrap();
            StoredBatch {
                client_seq: batch.client_seq,
                seq: i as i64 + 1,
                sender_device_id: sender.device_id.clone(),
                batch_id: batch.batch_id,
                envelope_sha256: envelope_hash(&batch.envelope).unwrap(),
                envelope: batch.envelope,
            }
        })
        .collect()
}

#[test]
fn fragmented_record_survives_restart_and_is_not_acknowledged_until_complete() {
    let sender = config("sender");
    let cfg = config("receiver");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fragments.db");
    let mut conn = Connection::open(&path).unwrap();
    fixture(&conn);
    initialize(&mut conn, &cfg).unwrap();
    let mut packets = large_packets(&sender).into_iter();
    for before in 0..2 {
        assert_eq!(
            apply_page(&mut conn, &cfg, before, page(packets.next().unwrap())).unwrap(),
            0
        );
    }
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM health_log").unwrap(), 0);
    assert_eq!(fragments::applied_cursor(&conn).unwrap(), 0);
    drop(conn);
    let mut conn = Connection::open(&path).unwrap();
    initialize(&mut conn, &cfg).unwrap();
    assert_eq!(
        scalar(&conn, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        2
    );
    assert_eq!(
        apply_page(&mut conn, &cfg, 2, page(packets.next().unwrap())).unwrap(),
        0
    );
    assert_eq!(
        apply_page(&mut conn, &cfg, 3, page(packets.next().unwrap())).unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT LENGTH(notes) FROM health_log WHERE id='large'"
        )
        .unwrap(),
        130_000
    );
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        0
    );
    assert_eq!(fragments::applied_cursor(&conn).unwrap(), 4);
    enqueue(&mut conn, &cfg).unwrap();
    let body: String = conn
        .query_row("SELECT body FROM cloud_relay_outbox", [], |r| r.get(0))
        .unwrap();
    let batch: Batch = serde_json::from_str(&body).unwrap();
    let receipt = decrypt(
        &cfg,
        &StoredBatch {
            client_seq: batch.client_seq,
            seq: 5,
            sender_device_id: cfg.device_id.clone(),
            batch_id: batch.batch_id,
            envelope_sha256: envelope_hash(&batch.envelope).unwrap(),
            envelope: batch.envelope,
        },
    )
    .unwrap();
    assert_eq!(receipt.kind, "receipt");
    assert_eq!(receipt.applied_seq, 4);
}

#[test]
fn corrupted_final_fragment_rolls_back_cursor_and_keeps_prior_parts() {
    let sender = config("sender");
    let cfg = config("receiver");
    let mut conn = connection();
    initialize(&mut conn, &cfg).unwrap();
    let mut packets = large_packets(&sender).into_iter();
    for before in 0..3 {
        apply_page(&mut conn, &cfg, before, page(packets.next().unwrap())).unwrap();
    }
    let mut bad = decrypt(&sender, &packets.next().unwrap()).unwrap();
    let serialized = serde_json::to_value(&bad).unwrap();
    let mut damaged = serialized;
    let data = damaged["fragment"]["data"].as_str().unwrap();
    let mut bytes = B64.decode(data).unwrap();
    bytes[0] ^= 1;
    damaged["fragment"]["data"] = json!(B64.encode(bytes));
    bad = serde_json::from_value(damaged).unwrap();
    assert_eq!(
        apply_page(&mut conn, &cfg, 3, page(stored(&sender, 4, &bad))).unwrap_err(),
        "relay_fragment_digest_mismatch"
    );
    assert_eq!(
        scalar(&conn, "SELECT receive_seq FROM cloud_relay_state").unwrap(),
        3
    );
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        3
    );
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM health_log").unwrap(), 0);
    assert_eq!(fragments::applied_cursor(&conn).unwrap(), 0);
}

#[test]
fn checkpoint_partial_own_fragment_prefix_survives_compaction_and_finishes_from_tail() {
    let sender = config("sender");
    let receiver = config("receiver");
    let mut source = connection();
    initialize(&mut source, &sender).unwrap();
    let mut packets = large_packets(&sender).into_iter();
    for before in 0..2 {
        apply_page(&mut source, &sender, before, page(packets.next().unwrap())).unwrap();
    }
    assert_eq!(fragments::applied_cursor(&source).unwrap(), 0);
    let mut snapshot = Vec::new();
    fragments::checkpoint_export(&source, &mut |value| {
        snapshot.push(value);
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshot.len(), 2);
    let mut restored = connection();
    initialize(&mut restored, &receiver).unwrap();
    {
        let tx = restored
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        for entry in snapshot {
            fragments::checkpoint_import_entry(&tx, entry, 2).unwrap();
        }
        tx.execute("UPDATE cloud_relay_state SET receive_seq=2", [])
            .unwrap();
        tx.execute(
            "INSERT INTO cloud_relay_sender_watermarks VALUES('sender',2,2)",
            [],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(fragments::applied_cursor(&restored).unwrap(), 0);
    for before in 2..4 {
        apply_page(
            &mut restored,
            &receiver,
            before,
            page(packets.next().unwrap()),
        )
        .unwrap();
    }
    assert_eq!(fragments::applied_cursor(&restored).unwrap(), 4);
    assert_eq!(
        scalar(
            &restored,
            "SELECT LENGTH(notes) FROM health_log WHERE id='large'"
        )
        .unwrap(),
        130_000
    );
    assert_eq!(
        scalar(&restored, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        0
    );
}

#[test]
fn unresolved_old_deletion_does_not_block_fetch_but_limits_delivery_claim() {
    let mut conn = connection();
    let cfg = config("receiver");
    initialize(&mut conn, &cfg).unwrap();
    let mut deleted = payload(vec![]);
    deleted.tombs.push(Tomb {
        tt: "sleep_sessions".into(),
        id: json!("old-unknown"),
        deleted_at: "2026-09-05T10:00:00Z".into(),
        identity: None,
    });
    apply_page(
        &mut conn,
        &cfg,
        0,
        page(stored(&config("sender"), 1, &deleted)),
    )
    .unwrap();
    apply_page(
        &mut conn,
        &cfg,
        1,
        page(stored(
            &config("sender"),
            2,
            &payload(vec![row("new", 1.0)]),
        )),
    )
    .unwrap();
    let status = database_status(&conn).unwrap();
    assert_eq!(status["received_seq"], 2);
    assert_eq!(status["applied_seq"], 0);
    assert_eq!(status["unresolved_deletions"], 1);
    assert_eq!(scalar(&conn, "SELECT COUNT(*) FROM health_log").unwrap(), 1);
}

// Synthetic loopback HTTP server: responses are bounded, all keys are fixed
// test values, and no actual user data or Cloudflare credentials are involved.
fn http_server(responses: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let mut requests = vec![];
        for (status, body) in responses {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => panic!("test HTTP request did not arrive"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut data = vec![];
            let mut buf = [0; 4096];
            loop {
                let n = socket.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(index) = data.windows(4).position(|v| v == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&data[..index]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if data.len() >= index + 4 + length {
                        break;
                    }
                }
            }
            requests.push(
                String::from_utf8(data)
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_owned(),
            );
            write!(socket,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nRetry-After: 3600\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        requests
    });
    (url, thread)
}

#[test]
fn full_upload_store_does_not_block_pull_or_hide_capacity_error() {
    let remote = stored(&config("peer"), 1, &payload(vec![row("remote", 9.0)]));
    let populated=json!({"batches":[{"seq":remote.seq,"client_seq":remote.client_seq,"sender_device_id":remote.sender_device_id,
        "batch_id":remote.batch_id,"envelope":remote.envelope,"envelope_sha256":remote.envelope_sha256}],
        "next_cursor":1,"latest_seq":1,"has_more":false}).to_string();
    let empty = json!({"batches":[],"next_cursor":1,"latest_seq":1,"has_more":false}).to_string();
    let (url, server) = http_server(vec![(507, "{}".into()), (200, populated), (200, empty)]);
    let mut cfg = config("local");
    cfg.endpoint = url;
    let mut conn = connection();
    conn.execute(
        "INSERT INTO health_log(id,value,updated_at) VALUES('local',1,'2026-09-05T00:00:00Z')",
        [],
    )
    .unwrap();
    initialize(&mut conn, &cfg).unwrap();
    conn.execute(
        "UPDATE cloud_relay_checkpoint_state SET checked_at=?1",
        [chrono::Utc::now().timestamp()],
    )
    .unwrap();
    let first = sync_once(&mut conn, &cfg).unwrap();
    assert_eq!(first["applied_rows"], 1);
    assert_eq!(first["error_code"], "relay_http_507");
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_outbox").unwrap(),
        1
    );
    let next = sync_once(&mut conn, &cfg).unwrap();
    assert_eq!(next["error_code"], "relay_http_507");
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("POST "));
    assert!(requests[1].starts_with("GET /v1/batches?after=0&limit=16 "));
    assert!(requests[2].starts_with("GET /v1/batches?after=1&limit=16 "));
}

#[test]
#[ignore = "Run via sync-relay/test/native-client.mjs with isolated workerd"]
fn real_workerd_client_roundtrip() {
    let endpoint = std::env::var("HANNI_RELAY_TEST_URL").expect("isolated workerd URL required");
    assert!(endpoint.starts_with("http://127.0.0.1:"));
    let dir = tempfile::tempdir().unwrap();
    let a_path = dir.path().join("a.db");
    let b_path = dir.path().join("b.db");
    let mut a = Connection::open(&a_path).unwrap();
    fixture(&a);
    a.pragma_update(None, "journal_mode", "WAL").unwrap();
    let b = Connection::open(&b_path).unwrap();
    fixture(&b);
    b.pragma_update(None, "journal_mode", "WAL").unwrap();
    drop(b);
    let mut a_cfg = config("device-a");
    a_cfg.endpoint = endpoint.clone();
    let mut b_cfg = config("device-b");
    b_cfg.endpoint = endpoint;
    b_cfg.token = B64.encode([5u8; 32]);
    a.execute("INSERT INTO health_log(id,date,type,value,updated_at) VALUES('record','2026-09-05','test',1,'2026-09-05T00:00:00Z')",[]).unwrap();
    a.execute("INSERT INTO health_log(id,date,type,value,updated_at) VALUES('deleted','2026-09-05','test',2,'2026-09-05T00:00:00Z')",[]).unwrap();
    let sent = sync_once(&mut a, &a_cfg).unwrap();
    assert!(sent["error_code"].is_null(), "{}", sent["error_code"]);
    let mut b = open_existing(b_path.to_str().unwrap()).unwrap();
    let received = sync_once(&mut b, &b_cfg).unwrap();
    assert!(
        received["error_code"].is_null(),
        "{}",
        received["error_code"]
    );
    assert_eq!(scalar(&b, "SELECT COUNT(*) FROM health_log").unwrap(), 2);
    drop(b); // Receiver stops; its durable cursor remains in its database.
    a.execute("UPDATE health_log SET value=3 WHERE id='record'", [])
        .unwrap();
    a.execute("DELETE FROM health_log WHERE id='deleted'", [])
        .unwrap();
    assert!(sync_once(&mut a, &a_cfg).unwrap()["error_code"].is_null());
    let mut b = open_existing(b_path.to_str().unwrap()).unwrap();
    let resumed = sync_once(&mut b, &b_cfg).unwrap();
    assert!(resumed["error_code"].is_null(), "{}", resumed["error_code"]);
    assert_eq!(scalar(&b, "SELECT COUNT(*) FROM health_log").unwrap(), 1);
    assert_eq!(
        b.query_row("SELECT value FROM health_log WHERE id='record'", [], |r| {
            r.get::<_, f64>(0)
        })
        .unwrap(),
        3.0
    );
    // Repeated catch-up is idempotent and automatic apply receipts reach sender.
    assert!(sync_once(&mut b, &b_cfg).unwrap()["error_code"].is_null());
    assert!(sync_once(&mut a, &a_cfg).unwrap()["error_code"].is_null());
    assert_eq!(scalar(&b, "SELECT COUNT(*) FROM health_log").unwrap(), 1);
    assert!(
        scalar(
            &a,
            "SELECT applied_seq FROM cloud_relay_receipts WHERE device_id='device-b'"
        )
        .unwrap()
            >= 3
    );
    assert_eq!(
        scalar(&a, "SELECT COUNT(*) FROM cloud_relay_outbox").unwrap(),
        0
    );
    // Exercise the actual encrypted fragmented wire contract through workerd.
    a.execute(
        "UPDATE health_log SET notes=?1 WHERE id='record'",
        ["z".repeat(130_000)],
    )
    .unwrap();
    for _ in 0..3 {
        assert!(sync_once(&mut a, &a_cfg).unwrap()["error_code"].is_null());
    }
    for _ in 0..3 {
        assert!(sync_once(&mut b, &b_cfg).unwrap()["error_code"].is_null());
    }
    assert_eq!(
        scalar(&b, "SELECT LENGTH(notes) FROM health_log WHERE id='record'").unwrap(),
        130_000
    );
    assert_eq!(
        scalar(&b, "SELECT COUNT(*) FROM cloud_relay_fragments").unwrap(),
        0
    );
    // The raw archive retains exact payload bytes and source deletion revision.
    raw::apply(&a, &raw::tests::record(1, false)).unwrap();
    assert!(sync_once(&mut a, &a_cfg).unwrap()["error_code"].is_null());
    assert!(sync_once(&mut b, &b_cfg).unwrap()["error_code"].is_null());
    assert_eq!(
        scalar(&b, "SELECT COUNT(*) FROM health_records").unwrap(),
        1
    );
    let archive: String = b
        .query_row("SELECT payload_json FROM health_records", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&archive).unwrap()["record"]["count"],
        "9007199254740993"
    );
    raw::apply(&a, &raw::tests::record(2, true)).unwrap();
    assert!(sync_once(&mut a, &a_cfg).unwrap()["error_code"].is_null());
    assert!(sync_once(&mut b, &b_cfg).unwrap()["error_code"].is_null());
    assert_eq!(
        scalar(&b, "SELECT is_deleted FROM health_records").unwrap(),
        1
    );
    assert_eq!(
        scalar(&b, "SELECT source_revision FROM health_records").unwrap(),
        2
    );
    // The public endpoint rejects an unprovisioned bearer.
    let denied = reqwest::blocking::Client::new()
        .get(a_cfg.url("/v1/batches"))
        .bearer_auth(B64.encode([9u8; 32]))
        .send()
        .unwrap();
    assert_eq!(denied.status().as_u16(), 401);
}

// These regressions inspect actual immutable encrypted outbox payloads. Synthetic
// ACK deletion below is explicit; production upload remains the only ACK path.
fn selection_payload(conn: &Connection, cfg: &RelayConfig) -> Payload {
    let text: String = conn
        .query_row(
            "SELECT body FROM cloud_relay_outbox ORDER BY local_seq LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let batch: Batch = serde_json::from_str(&text).unwrap();
    decrypt(
        cfg,
        &StoredBatch {
            seq: 1,
            client_seq: batch.client_seq,
            sender_device_id: cfg.device_id.clone(),
            batch_id: batch.batch_id,
            envelope_sha256: envelope_hash(&batch.envelope).unwrap(),
            envelope: batch.envelope,
        },
    )
    .unwrap()
}
fn selection_outbox(conn: &Connection) -> Vec<(i64, String, String, String)> {
    let mut s=conn.prepare("SELECT local_seq,batch_id,body,envelope_hash FROM cloud_relay_outbox ORDER BY local_seq").unwrap();
    s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}
fn selection_big_raw(revision: i64) -> Map<String, Value> {
    let mut fields = raw::tests::record(revision, false);
    let mut payload: Value =
        serde_json::from_str(fields["payload_json"].as_str().unwrap()).unwrap();
    payload["record"]["synthetic_padding"] = json!("x".repeat(145_000));
    let body = serde_json::to_string(&payload).unwrap();
    fields.insert("payload_sha256".into(), json!(hash(body.as_bytes())));
    fields.insert("payload_json".into(), json!(body));
    fields
}

#[test]
fn fresh_raw_slot_skips_archive_but_oldest_turn_advances_without_rewriting_outbox() {
    let cfg = config("device-a");
    let mut conn = connection();
    initialize(&mut conn, &cfg).unwrap();
    for i in 0..400 {
        conn.execute("INSERT INTO health_log(id,date,type,value,notes,updated_at) VALUES(?1,'2026-09-01','test',1,?2,'2026-09-01T09:00:00Z')",
            params![format!("archive-{i:04}"),"x".repeat(500)]).unwrap();
    }
    raw::apply(&conn, &raw::tests::record(1, false)).unwrap();
    let oldest_before = scalar(
        &conn,
        "SELECT COUNT(*) FROM cloud_relay_dirty WHERE table_name='health_log'",
    )
    .unwrap();
    assert!(enqueue(&mut conn, &cfg).unwrap());
    let first = selection_payload(&conn, &cfg);
    assert_eq!(first.rows.first().unwrap().t, "health_records");
    assert!(first.rows.iter().any(|r| r.t == "health_log"));
    let after_first = scalar(
        &conn,
        "SELECT COUNT(*) FROM cloud_relay_dirty WHERE table_name='health_log'",
    )
    .unwrap();
    assert!(after_first < oldest_before && after_first > 256);
    assert_eq!(
        scalar(&conn, "SELECT urgent_next FROM cloud_relay_selection").unwrap(),
        0
    );
    let immutable = selection_outbox(&conn);
    // A correction to an old event is urgent by its newly journalled revision,
    // independent of event-time sorting. It cannot preempt ciphertext in flight.
    raw::apply(&conn, &raw::tests::record(2, false)).unwrap();
    for _ in 0..3 {
        assert!(enqueue(&mut conn, &cfg).unwrap());
    }
    assert_eq!(selection_outbox(&conn), immutable);
    assert_eq!(
        scalar(&conn, "SELECT urgent_next FROM cloud_relay_selection").unwrap(),
        0
    );
    conn.execute("DELETE FROM cloud_relay_outbox", []).unwrap(); // synthetic ACK
    assert!(enqueue(&mut conn, &cfg).unwrap());
    let second = selection_payload(&conn, &cfg);
    assert!(second.rows.iter().all(|r| r.t == "health_log"));
    assert!(
        scalar(
            &conn,
            "SELECT COUNT(*) FROM cloud_relay_dirty WHERE table_name='health_log'"
        )
        .unwrap()
            < after_first
    );
    assert_eq!(
        scalar(&conn, "SELECT urgent_next FROM cloud_relay_selection").unwrap(),
        1
    );
    conn.execute("DELETE FROM cloud_relay_outbox", []).unwrap(); // synthetic ACK
    enqueue(&mut conn, &cfg).unwrap();
    let third = selection_payload(&conn, &cfg);
    assert_eq!(third.rows.first().unwrap().t, "health_records");
    assert_eq!(third.rows.first().unwrap().f["source_revision"], 2);
}

#[test]
fn fragmented_urgent_row_flips_once_then_oldest_history_gets_its_turn() {
    let cfg = config("device-a");
    let mut conn = connection();
    initialize(&mut conn, &cfg).unwrap();
    conn.execute("INSERT INTO health_log(id,type,value,updated_at) VALUES('oldest','test',1,'2026-09-01T09:00:00Z')",[]).unwrap();
    raw::apply(&conn, &selection_big_raw(1)).unwrap();
    enqueue(&mut conn, &cfg).unwrap();
    let immutable = selection_outbox(&conn);
    assert!(immutable.len() > 2);
    assert_eq!(
        scalar(&conn, "SELECT urgent_next FROM cloud_relay_selection").unwrap(),
        0
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT COUNT(*) FROM cloud_relay_dirty WHERE table_name='health_log'"
        )
        .unwrap(),
        1
    );
    raw::apply(&conn, &selection_big_raw(2)).unwrap();
    enqueue(&mut conn, &cfg).unwrap();
    assert_eq!(selection_outbox(&conn), immutable);
    conn.execute("DELETE FROM cloud_relay_outbox", []).unwrap(); // synthetic ACK of all parts
    enqueue(&mut conn, &cfg).unwrap();
    let next = selection_payload(&conn, &cfg);
    assert!(next.fragment.is_none());
    assert_eq!(next.rows.len(), 1);
    assert_eq!(next.rows[0].f["id"], "oldest");
    assert_eq!(
        scalar(&conn, "SELECT urgent_next FROM cloud_relay_selection").unwrap(),
        1
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT COUNT(*) FROM cloud_relay_dirty WHERE table_name='health_records'"
        )
        .unwrap(),
        1
    );
}

#[test]
fn failed_fragment_materialization_rolls_back_queue_journal_and_selection_turn() {
    let cfg = config("device-a");
    let mut conn = connection();
    initialize(&mut conn, &cfg).unwrap();
    raw::apply(&conn, &selection_big_raw(1)).unwrap();
    let seq = scalar(
        &conn,
        "SELECT seq FROM cloud_relay_dirty WHERE table_name='health_records'",
    )
    .unwrap();
    conn.execute_batch(
        "CREATE TRIGGER synthetic_outbox_failure BEFORE INSERT ON cloud_relay_outbox
        WHEN NEW.local_seq=2 BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;",
    )
    .unwrap();
    assert!(enqueue(&mut conn, &cfg).is_err());
    assert_eq!(
        scalar(&conn, "SELECT COUNT(*) FROM cloud_relay_outbox").unwrap(),
        0
    );
    assert_eq!(
        scalar(
            &conn,
            "SELECT seq FROM cloud_relay_dirty WHERE table_name='health_records'"
        )
        .unwrap(),
        seq
    );
    assert_eq!(
        scalar(&conn, "SELECT urgent_next FROM cloud_relay_selection").unwrap(),
        1
    );
}
