# Stable server v2 wire (isolated proposal; not deployed)

All endpoints require Authorization: Bearer <per-device token>, including native
WebSocket. No tokens in URLs. Exact JSON keys; no unexpected fields.

## Ordinary append / pull

POST /v1/batches: `{client_seq,batch_id,envelope}`. client_seq starts at 1 per device,
is persisted before encryption, and advances only with durable ordered outbox.
Envelope remains `{v:1,alg:"XChaCha20-Poly1305",key_id,nonce,ciphertext}` (nonce24,
ct+tag<=65536 bytes, canonical base64url). Root's new AAD:
`["hanni-relay-v2",sender_device_id,batch_id,key_id,client_seq]`.

ACK: `{seq,duplicate,client_seq,sender_device_id,batch_id,envelope_sha256}`.
Only n==accepted+1 appends; n==accepted verifies exact UUID+digest and returns cached
ACK even after payload GC. n<accepted returns409 device_state_stale; n>accepted+1
returns409 client_sequence_gap. Both include accepted_client_seq. Never silently
clear/re-encrypt/re-number an old outbox on those errors.

GET /v1/device-state: `{accepted_client_seq,last_ack,checkpoint,latest_seq}` for the
authenticated device only. last_ack has the five matching ACK fields above except
duplicate. Initial accepted=0,last_ack=null. This does not acknowledge a local item.

GET /v1/batches?after=N&limit=16 retains old page fields; StoredBatch adds client_seq.
If after<compacted_through:409 `{error:"checkpoint_required",checkpoint:{checkpoint_id,
base_seq,generation}}`. Cursor greater than latest still409 cursor_ahead. seq never
resets. A client must atomically apply a verified checkpoint before setting receive
cursor to base_seq; preserve local dirty/outbox, logical tombs and applied floors.

## Checkpoint upload

POST /v1/checkpoints/lease exact body:
`{checkpoint_id,expected_generation,base_seq,chunk_count,total_bytes}`.
checkpoint_id is UUID, expected_generation starts0, base_seq>0 and <=latest_seq,
chunk_count1..4096. total_bytes=sum UTF8 byte lengths of the CANONICAL envelope JSON
for all data parts, not plaintext/ciphertext lengths, and excludes final manifest.
Total<=128MiB; each part uses the same <=64KiB ciphertext envelope cap.

Response201 `{checkpoint_id,lease_epoch,expires_at}`; expires_at is Unix milliseconds.
Lease lasts15min. Repeating the same lease body renews it and returns a NEW fencing
epoch; subsequent writes must use that epoch. A live competing lease returns409
checkpoint_lease_busy (Retry-After30). Expired uploader does not block log append.
Acquiring another lease abandons the previous expired staging snapshot. Inactive
staging expires after24h. No account-level or paid resources are touched.

PUT /v1/checkpoints/{id}/chunks/{zero_based_index}:
`{lease_epoch,envelope}`. ACK201/200 `{duplicate,checkpoint_id,index,envelope_sha256}`.
Same envelope repeats safely; same index with different bytes returns409. Save
encrypted parts locally and retry them unchanged. All parts have the same key_id.

Part AAD (client responsibility):
`["hanni-checkpoint-v1",uploader_device_id,checkpoint_id,key_id,base_seq,index]`.
Manifest AAD:
`["hanni-checkpoint-manifest-v1",uploader_device_id,checkpoint_id,key_id,base_seq]`.

POST /v1/checkpoints/{id}/finalize:
`{lease_epoch,chunk_root_sha256,envelope}`. envelope is the encrypted final manifest,
using the same key_id; the server does not decrypt it. chunk_root_sha256 is SHA256
of UTF8 JSON.stringify([part0_envelope_sha256,part1_envelope_sha256,...]), no spaces,
lowercase hex strings in exact part order. Each part digest hashes canonical
envelope JSON in field order v,alg,key_id,nonce,ciphertext.

Server requires all declared indices and exact total_bytes; atomically CASes
expected_generation/base/lease, publishes the immutable manifest and advances
compacted_through. latest_seq may have grown beyond base_seq: that tail is retained.
ACK201 `{checkpoint_id,base_seq,generation,envelope_sha256,duplicate:false}` after
storage.sync(); exact finalize repeat returns200 with duplicate:true.

Encrypted manifest must bind schema/profile, base_seq, count, chunk_root, exact
plaintext size/hash, receipts, and actual applied floor. The server can check
ciphertext completeness but cannot prove client snapshot completeness. Snapshot is
one SQLite read transaction and may be a superset of the committed relay prefix:
local dirty/outbox and incomplete fragments do not block creation, provided the
encrypted snapshot includes every partial cloud_relay_fragments entry, including
own-echo accepted prefixes, and preserves the actual applied floor. This permits
recovery when a full relay interrupts a fragmented row. Legacy unresolved tombs
MUST remain with original first_seq. Snapshot creation or bootstrap never clears,
acknowledges, renumbers or re-encrypts the ordinary local outbox. Client merge must
preserve local dirty rows, all soft-deletes and incomplete/unresolved state.

## Immutable download and cleanup

GET /v1/checkpoints/latest: `{checkpoint_id,base_seq,generation}` or404.
POST /v1/checkpoints/{id}/read-lease body `{}`:
201 `{checkpoint_id,read_lease_id,expires_at}` (10min); <=2 active leases per device.
Existing valid lease for the same checkpoint is reused. expires_at is milliseconds.
Use X-Hanni-Read-Lease: <read_lease_id> alongside Authorization for both GETs:

GET /v1/checkpoints/{id}: `{checkpoint_id,base_seq,generation,uploader_device_id,
chunk_count,total_bytes,chunk_root_sha256,envelope_sha256,envelope}`.
GET /v1/checkpoints/{id}/chunks/{index}: `{checkpoint_id,index,envelope_sha256,envelope}`.

Old finalized snapshots retain30min grace after replacement; valid read leases pin
their data. A newly acquired retired lease cannot extend beyond grace; an existing
valid lease keeps its original deadline. A retired snapshot past grace returns410
checkpoint_expired with the latest summary when acquiring a lease. An unknown or
already collected snapshot returns404 checkpoint_missing. Refresh GET latest and
restart/reuse verified download parts for that immutable ID; leave the working DB
unchanged. Missing/invalid lease returns409 read_lease_required; an expired or
wrong-device lease returns409 read_lease_expired.

## Client recovery decisions

| Operation/error | Safe action |
| --- | --- |
| Lost finalize response | Retry the identical finalize first. A published checkpoint returns200 duplicate with the same generation and digest, even with its old epoch. |
| PUT/finalize409 checkpoint_lease_expired | Renew by repeating the exact acquire body, retaining all immutable ciphertext. The error also covers a checkpoint abandoned after another uploader won. |
| Acquire409 checkpoint_lease_busy | Wait Retry-After30; ordinary append/pull continue. |
| Acquire/PUT/finalize409 checkpoint_generation_changed | Read latest, authenticate and merge that snapshot as necessary; discard only superseded local staging, never dirty/outbox. Start a new snapshot with a new ID and current generation. |
| Acquire409 checkpoint_not_staging | First retry a previously attempted finalize unchanged. Otherwise read/verify latest before discarding obsolete staging. |
| Upload404 checkpoint_missing | The stage may have expired and been collected. Read/verify latest; if still needed create a new stage/ID from a fresh consistent snapshot. |
| Finalize409 checkpoint_incomplete | Repeat immutable part PUTs; exact duplicates are accepted. No compacted floor changed. |
| 409 checkpoint_payload_mismatch / chunk_payload_mismatch;400 digest/key/size mismatch | Integrity/protocol failure; stop automatic publication and preserve local data. Do not overwrite, renumber or re-encrypt an existing stage. |
| 429 with Retry-After / 507 capacity | Keep ciphertext and working data. Retry with bounded backoff; capacity may need a completed checkpoint plus bounded GC. |

GET latest/manifest authenticity and encrypted content validation are separate: a
server response alone never proves that a snapshot safely represents the local DB.
None of these server errors authorizes clearing an ordinary pending outbox.

POST /v1/maintenance body `{}` performs one bounded GC step, returning
`{removed_rows,more_pending}`. Durable alarms continue cleanup. Max100 payload rows
per transaction,10000 GC rows/day,80000 conservative write units/day. Prior payload
is removed only after finalize. Retained counters shrink; latest_seq and device
ACK watermarks remain monotonic. Manifest/chunks for active or pinned checkpoints
are never removed. Data budget: log128MiB plus separately reserved staging and
snapshot space; conservative combined cap768MiB, below1GB Free/DO. The limit may
only be lowered for tests. Checkpoint upload has its own128MiB/day byte budget;
normal append keeps16MiB/day. Combined authenticated request cap15000/day.

Daily quota errors429 carry Retry-After. Capacity507 preserves all committed data
and outboxes. This removes obsolete versions, not unbounded unique health history.

## Local seam

src/worker.mjs exports Relay and a Worker router. Miniflare: modules:true,
durableObjects:{RELAY:{className:"Relay",useSQLite:true}}, compatibilityDate2026-09-01,
HANNI_DEVICE_TOKEN_HASHES JSON of opaque device=>SHA256(base64url token string).
Tests run with installed ../cloudflare-tooling Miniflare through
convertV4MiniflareOptions; resourcePersistencePath enables restart persistence.
No actual deploy/auth/provisioning occurs. This is a fresh v2 namespace; existing
v1 databases fail closed with protocol_migration_required, never auto-reset.
