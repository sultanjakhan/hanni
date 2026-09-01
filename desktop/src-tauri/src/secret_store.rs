//! Windows user-bound storage for local Hanni secrets.
//!
//! Sensitive `app_settings` values and local API token files are protected
//! with DPAPI on Windows. Existing plaintext values are migrated only after a
//! protected replacement has been decrypted and compared with the source.

#[cfg(windows)]
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rusqlite::OptionalExtension;
use std::io::{Read, Write};
use std::path::Path;

const PREFIX: &str = "hanni-dpapi:v1:";
#[cfg(windows)]
const ENTROPY: &[u8] = b"Hanni local secret store v1";
#[cfg(windows)]
const SCRUB_MARKER: &str = "secret_store_dpapi_v1_scrubbed";

const SENSITIVE_SETTING_KEYS: &[&str] = &[
    "cloud_owner_gh_key",
    "cloud_owner_gh_pat",
    "cloud_share_config",
    "google_auth_config",
    "google_auth_pending_state",
    "google_auth_session",
    "lan_sync_key",
    "openclaw_token",
    "share_gist_token",
    "sync_device_token",
];

pub fn is_sensitive_setting(key: &str) -> bool {
    SENSITIVE_SETTING_KEYS.contains(&key)
}

pub fn get_setting(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("read setting {key}: {error}"))?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    if !is_sensitive_setting(key) {
        return Ok(Some(raw));
    }
    decode_or_migrate_setting(conn, key, &raw).map(Some)
}

pub fn set_setting(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    let stored = if is_sensitive_setting(key) {
        let protected = encode_for_storage(key, value)?;
        verify_roundtrip(key, &protected, value)?;
        protected
    } else {
        value.to_owned()
    };
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, stored],
    )
    .map_err(|error| format!("write setting {key}: {error}"))?;
    Ok(())
}

/// Migrate every known sensitive setting independently. A failed row remains
/// plaintext because the database is updated only after protect + unprotect
/// succeeds for that exact value.
pub fn migrate_sensitive_settings(conn: &rusqlite::Connection) -> Result<(), String> {
    #[cfg(windows)]
    conn.pragma_update(None, "secure_delete", "ON")
        .map_err(|error| format!("enable secure secret cleanup: {error}"))?;

    #[cfg(windows)]
    let scrubbed = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [SCRUB_MARKER],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("read secret scrub marker: {error}"))?
        .as_deref()
        == Some("1");
    #[cfg(windows)]
    let mut found_plaintext = false;

    for key in SENSITIVE_SETTING_KEYS {
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                rusqlite::params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("read setting {key} for migration: {error}"))?;
        if let Some(raw) = raw {
            #[cfg(windows)]
            if !raw.starts_with(PREFIX) {
                found_plaintext = true;
                // Clear the completion marker before rewriting a plaintext
                // row. If the process stops before VACUUM, the next startup
                // must still scrub the historical SQLite pages.
                conn.execute("DELETE FROM app_settings WHERE key=?1", [SCRUB_MARKER])
                    .map_err(|error| format!("clear secret scrub marker: {error}"))?;
            }
            decode_or_migrate_setting(conn, key, &raw)?;
        }
    }

    #[cfg(windows)]
    {
        if !scrubbed || found_plaintext {
            checkpoint_truncate(conn, "before secret database scrub")?;
            conn.execute_batch("VACUUM;")
                .map_err(|error| format!("scrub historical secret pages: {error}"))?;
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, '1') \
                 ON CONFLICT(key) DO UPDATE SET value='1'",
                [SCRUB_MARKER],
            )
            .map_err(|error| format!("record completed secret database scrub: {error}"))?;
        }
        checkpoint_truncate(conn, "after protected secret migration")?;
    }
    Ok(())
}

pub(crate) fn checkpoint_truncate(
    conn: &rusqlite::Connection,
    context: &str,
) -> Result<(), String> {
    let (busy, _log_frames, _checkpointed_frames): (i64, i64, i64) = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|error| format!("checkpoint {context}: {error}"))?;
    if busy != 0 {
        return Err(format!("checkpoint {context}: database is busy"));
    }
    Ok(())
}

/// Protect the two local HTTP bearer-token files before either server starts.
/// Invalid legacy contents are left untouched and abort startup.
pub fn migrate_token_files(data_dir: &Path) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = data_dir;
        return Ok(());
    }

    #[cfg(windows)]
    for name in ["api_token.txt", "jobs_api_token.txt"] {
        let path = data_dir.join(name);
        if !path.exists() {
            continue;
        }
        crate::secure_fs::restrict_file_if_present(&path)
            .map_err(|error| format!("secure {name}: {error}"))?;
        let raw =
            std::fs::read_to_string(&path).map_err(|error| format!("read {name}: {error}"))?;
        let scope = file_scope(&path)?;
        let plaintext = if raw.starts_with(PREFIX) {
            decode_from_storage(&scope, &raw)?
        } else {
            raw
        };
        let trimmed = plaintext.trim();
        let parsed = uuid::Uuid::parse_str(trimmed)
            .map_err(|_| format!("{name} is not a canonical UUID"))?;
        let canonical = parsed.hyphenated().to_string();
        if trimmed != canonical {
            return Err(format!("{name} is not a canonical UUID"));
        }
        if !std::fs::read_to_string(&path)
            .map_err(|error| format!("re-read {name}: {error}"))?
            .starts_with(PREFIX)
        {
            write_file(&path, &canonical)?;
        }
        if read_file(&path)? != canonical {
            return Err(format!("{name} protected readback mismatch"));
        }
    }
    Ok(())
}

/// Sanitize only Hanni's bounded rolling SQLite backups. Automation log
/// previews are removed on every platform; Windows additionally migrates
/// protected credential rows. Managed WAL/SHM sidecars without their database
/// fail closed because they may still contain sensitive historical pages.
pub fn migrate_backup_databases(data_dir: &Path) -> Result<(), String> {
    let backup_dir = data_dir.join("backups");
    let entries = match std::fs::read_dir(&backup_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("scan Hanni backups: {error}")),
    };
    let mut databases = std::collections::BTreeSet::new();
    let mut sidecar_bases = std::collections::BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read Hanni backup entry: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if is_managed_backup_database_name(name) {
            databases.insert(name.to_string());
        } else if let Some(base) = managed_backup_sidecar_base(name) {
            sidecar_bases.insert(base);
        }
    }

    for base in &sidecar_bases {
        if !databases.contains(base) {
            return Err(format!("orphan managed backup sidecar for {base}"));
        }
    }

    for name in databases {
        let path = backup_dir.join(&name);
        let wal_path = path.with_extension("db-wal");
        let shm_path = path.with_extension("db-shm");
        for candidate in [&path, &wal_path, &shm_path] {
            crate::secure_fs::restrict_file_if_present(candidate)
                .map_err(|error| format!("secure backup {}: {error}", candidate.display()))?;
        }

        let conn = rusqlite::Connection::open(&path)
            .map_err(|error| format!("open backup {name}: {error}"))?;
        crate::db::migrate_automation_log(&conn)
            .map_err(|error| format!("sanitize backup {name}: {error}"))?;

        #[cfg(windows)]
        {
            let has_settings: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='app_settings')",
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| format!("inspect backup {name}: {error}"))?;
            if has_settings {
                migrate_sensitive_settings(&conn)
                    .map_err(|error| format!("migrate backup {name}: {error}"))?;
            }
        }
        drop(conn);

        for candidate in [&path, &wal_path, &shm_path] {
            crate::secure_fs::restrict_file_if_present(candidate)
                .map_err(|error| format!("re-secure backup {}: {error}", candidate.display()))?;
        }
    }
    Ok(())
}

fn is_managed_backup_database_name(name: &str) -> bool {
    let Some(stem) = name
        .strip_prefix("hanni_")
        .and_then(|value| value.strip_suffix(".db"))
    else {
        return false;
    };
    !stem.is_empty()
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'_')
}

fn managed_backup_sidecar_base(name: &str) -> Option<String> {
    ["-wal", "-shm"].iter().find_map(|suffix| {
        let base = name.strip_suffix(suffix)?;
        if is_managed_backup_database_name(base) {
            Some(base.to_string())
        } else {
            None
        }
    })
}

fn decode_or_migrate_setting(
    conn: &rusqlite::Connection,
    key: &str,
    raw: &str,
) -> Result<String, String> {
    if raw.starts_with(PREFIX) {
        return decode_from_storage(key, raw);
    }

    #[cfg(not(windows))]
    {
        return Ok(raw.to_owned());
    }

    #[cfg(windows)]
    {
        let protected = encode_for_storage(key, raw)?;
        verify_roundtrip(key, &protected, raw)?;
        let changed = conn
            .execute(
                "UPDATE app_settings SET value=?1 WHERE key=?2 AND value=?3",
                rusqlite::params![protected, key, raw],
            )
            .map_err(|error| format!("migrate setting {key}: {error}"))?;
        if changed != 1 {
            return Err(format!("migrate setting {key}: value changed concurrently"));
        }
        Ok(raw.to_owned())
    }
}

pub fn read_file(path: &Path) -> Result<String, String> {
    crate::secure_fs::restrict_file_if_present(path)
        .map_err(|error| format!("secure {}: {error}", path.display()))?;
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    let scope = file_scope(path)?;
    if raw.starts_with(PREFIX) {
        return decode_from_storage(&scope, &raw);
    }

    #[cfg(not(windows))]
    {
        return Ok(raw);
    }

    #[cfg(windows)]
    {
        // `write_file` validates the temporary protected file before the
        // atomic replace, so the plaintext path survives every earlier error.
        write_file(path, &raw)?;
        Ok(raw)
    }
}

pub fn write_file(path: &Path, plaintext: &str) -> Result<(), String> {
    let parent = prepare_secret_path(path)?;
    let scope = file_scope(path)?;
    let stored = encode_for_storage(&scope, plaintext)?;
    verify_roundtrip(&scope, &stored, plaintext)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("create protected temp file: {error}"))?;
    temp.write_all(stored.as_bytes())
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|error| format!("write protected temp file: {error}"))?;
    crate::secure_fs::restrict_file(temp.path())
        .map_err(|error| format!("secure protected temp file: {error}"))?;

    let mut check = String::new();
    std::fs::File::open(temp.path())
        .and_then(|mut file| file.read_to_string(&mut check))
        .map_err(|error| format!("verify protected temp file: {error}"))?;
    verify_roundtrip(&scope, &check, plaintext)?;

    crate::secure_fs::restrict_file_if_present(path)
        .map_err(|error| format!("validate existing secret file: {error}"))?;
    temp.persist(path)
        .map_err(|error| format!("replace {}: {}", path.display(), error.error))?;
    crate::secure_fs::restrict_file(path)
        .map_err(|error| format!("secure {}: {error}", path.display()))?;
    Ok(())
}

pub fn create_file(path: &Path, plaintext: &str) -> Result<(), String> {
    use std::fs::OpenOptions;

    prepare_secret_path(path)?;
    let scope = file_scope(path)?;
    let stored = encode_for_storage(&scope, plaintext)?;
    verify_roundtrip(&scope, &stored, plaintext)?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("create {}: {error}", path.display()))?;
    if let Err(error) = file
        .write_all(stored.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("write {}: {error}", path.display()));
    }
    if let Err(error) = crate::secure_fs::restrict_file(path) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("secure {}: {error}", path.display()));
    }
    drop(file);
    let check = std::fs::read_to_string(path)
        .map_err(|error| format!("verify {}: {error}", path.display()))?;
    if let Err(error) = verify_roundtrip(&scope, &check, plaintext) {
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

fn prepare_secret_path(path: &Path) -> Result<&Path, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "secret path has no parent".to_string())?;
    crate::secure_fs::ensure_private_dir(parent)
        .map_err(|error| format!("secure secret directory: {error}"))?;
    crate::secure_fs::restrict_file_if_present(path)
        .map_err(|error| format!("secure existing secret file: {error}"))?;
    Ok(parent)
}

fn file_scope(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(|name| format!("file:{name}"))
        .ok_or_else(|| "secret path has no valid UTF-8 file name".to_string())
}

fn verify_roundtrip(scope: &str, stored: &str, expected: &str) -> Result<(), String> {
    let decoded = decode_from_storage(scope, stored)?;
    if decoded.as_bytes() != expected.as_bytes() {
        return Err("protected secret verification mismatch".to_string());
    }
    Ok(())
}

#[cfg(windows)]
fn encode_for_storage(scope: &str, plaintext: &str) -> Result<String, String> {
    let entropy = slot_entropy(scope);
    let encrypted = dpapi_protect(plaintext.as_bytes(), &entropy)?;
    Ok(format!("{PREFIX}{}", BASE64.encode(encrypted)))
}

#[cfg(not(windows))]
fn encode_for_storage(_scope: &str, plaintext: &str) -> Result<String, String> {
    Ok(plaintext.to_owned())
}

fn decode_from_storage(scope: &str, stored: &str) -> Result<String, String> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else {
        #[cfg(not(windows))]
        return Ok(stored.to_owned());
        #[cfg(windows)]
        return Err("secret is not DPAPI-protected".to_string());
    };

    #[cfg(not(windows))]
    {
        let _ = encoded;
        Err("Windows-protected secret is unavailable on this platform".to_string())
    }

    #[cfg(windows)]
    {
        let encrypted = BASE64
            .decode(encoded)
            .map_err(|_| "protected secret encoding is invalid".to_string())?;
        let entropy = slot_entropy(scope);
        let plaintext = dpapi_unprotect(&encrypted, &entropy)?;
        String::from_utf8(plaintext).map_err(|_| "protected secret is not valid UTF-8".to_string())
    }
}

#[cfg(windows)]
fn slot_entropy(scope: &str) -> Vec<u8> {
    let mut entropy = Vec::with_capacity(ENTROPY.len() + 1 + scope.len());
    entropy.extend_from_slice(ENTROPY);
    entropy.push(b':');
    entropy.extend_from_slice(scope.as_bytes());
    entropy
}

#[cfg(windows)]
fn dpapi_protect(plaintext: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input_len =
        u32::try_from(plaintext.len()).map_err(|_| "secret is too large for DPAPI".to_string())?;
    let entropy_len =
        u32::try_from(entropy.len()).map_err(|_| "DPAPI entropy is too large".to_string())?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: plaintext.as_ptr().cast_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_len,
        pbData: entropy.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(|error| format!("DPAPI protect failed: {error}"))?;
    if output.pbData.is_null() {
        return Err("DPAPI protect returned no data".to_string());
    }
    let protected =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    Ok(protected)
}

#[cfg(windows)]
fn dpapi_unprotect(protected: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input_len = u32::try_from(protected.len())
        .map_err(|_| "protected secret is too large for DPAPI".to_string())?;
    let entropy_len =
        u32::try_from(entropy.len()).map_err(|_| "DPAPI entropy is too large".to_string())?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: protected.as_ptr().cast_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_len,
        pbData: entropy.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            Some(&entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(|error| format!("DPAPI unprotect failed: {error}"))?;
    if output.pbData.is_null() {
        return Err("DPAPI unprotect returned no data".to_string());
    }
    let plaintext =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("open test DB");
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .expect("create settings");
        conn
    }

    fn file_contains(path: &Path, needle: &[u8]) -> bool {
        std::fs::read(path)
            .map(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
            .unwrap_or(false)
    }

    fn assert_sqlite_artifacts_do_not_contain(path: &Path, needle: &[u8]) {
        for candidate in [
            path.to_path_buf(),
            path.with_extension("db-wal"),
            path.with_extension("db-shm"),
        ] {
            if !candidate.exists() {
                continue;
            }
            assert!(
                !file_contains(&candidate, needle),
                "sensitive preview remained in {}",
                candidate.display()
            );
        }
    }

    fn create_legacy_automation_log(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "PRAGMA user_version=10;
             CREATE TABLE automation_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                script_hash TEXT NOT NULL,
                script_preview TEXT NOT NULL DEFAULT '',
                success INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_automation_log_ts ON automation_log(ts);",
        )
        .expect("create legacy automation log");
    }

    #[test]
    fn sensitive_key_allowlist_is_exact() {
        assert!(is_sensitive_setting("cloud_owner_gh_pat"));
        assert!(is_sensitive_setting("cloud_share_config"));
        assert!(is_sensitive_setting("google_auth_config"));
        assert!(is_sensitive_setting("google_auth_pending_state"));
        assert!(is_sensitive_setting("google_auth_session"));
        assert!(is_sensitive_setting("lan_sync_key"));
        assert!(is_sensitive_setting("openclaw_token"));
        assert!(is_sensitive_setting("share_gist_token"));
        assert!(is_sensitive_setting("sync_device_token"));
        assert!(!is_sensitive_setting("cloud_owner_gh_pull_sha"));
        assert!(!is_sensitive_setting("lan_sync_enabled"));
    }

    #[test]
    fn ordinary_setting_roundtrips_without_rewrite() {
        let conn = test_db();
        set_setting(&conn, "calendar_autosync", "true").expect("set setting");
        assert_eq!(
            get_setting(&conn, "calendar_autosync").expect("get setting"),
            Some("true".to_string())
        );
        let raw: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='calendar_autosync'",
                [],
                |row| row.get(0),
            )
            .expect("read raw setting");
        assert_eq!(raw, "true");
    }

    #[test]
    fn backup_database_allowlist_is_bounded() {
        assert!(is_managed_backup_database_name("hanni_20260901_010203.db"));
        assert_eq!(
            managed_backup_sidecar_base("hanni_20260901_010203.db-wal").as_deref(),
            Some("hanni_20260901_010203.db")
        );
        assert_eq!(
            managed_backup_sidecar_base("hanni_20260901_010203.db-shm").as_deref(),
            Some("hanni_20260901_010203.db")
        );
        for name in [
            "hanni_.db",
            "hanni_secret.db",
            "hanni_20260901.db-wal",
            "other_20260901.db",
            "hanni_20260901.db.exe",
        ] {
            assert!(!is_managed_backup_database_name(name), "{name}");
        }
        assert!(managed_backup_sidecar_base("hanni_secret.db-wal").is_none());
        assert!(managed_backup_sidecar_base("other_20260901.db-shm").is_none());
    }

    #[test]
    fn managed_backup_legacy_wal_is_scrubbed_and_metadata_survives() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_dir = temp.path().join("backups");
        std::fs::create_dir(&backup_dir).expect("create backup dir");
        let path = backup_dir.join("hanni_20260901_010203.db");
        let canary = b"hanni-backup-wal-preview-canary-5579";

        let fixture = rusqlite::Connection::open(&path).expect("open backup fixture");
        fixture
            .pragma_update(None, "journal_mode", "WAL")
            .expect("enable backup WAL");
        fixture
            .pragma_update(None, "secure_delete", "OFF")
            .expect("disable legacy secure delete");
        create_legacy_automation_log(&fixture);
        fixture
            .execute(
                "INSERT INTO automation_log(ts, script_hash, script_preview, success, duration_ms)
                 VALUES (456, 'backup-action-hash', ?1, 1, 29)",
                [String::from_utf8_lossy(canary).as_ref()],
            )
            .expect("seed backup preview");
        assert!(
            file_contains(&path.with_extension("db-wal"), canary),
            "fixture must place backup preview specifically in WAL"
        );

        migrate_backup_databases(temp.path()).expect("sanitize managed backup");
        assert_sqlite_artifacts_do_not_contain(&path, canary);
        drop(fixture);

        let conn = rusqlite::Connection::open(&path).expect("reopen sanitized backup");
        let row: (i64, String, i64, i64) = conn
            .query_row(
                "SELECT ts, script_hash, success, duration_ms FROM automation_log",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read preserved backup metadata");
        assert_eq!(row, (456, "backup-action-hash".into(), 1, 29));
        let marker: String = conn
            .query_row(
                "SELECT state FROM _hanni_security_migrations
                 WHERE name='automation_log_metadata_v1'",
                [],
                |row| row.get(0),
            )
            .expect("read backup scrub marker");
        assert_eq!(marker, "complete");
    }

    #[test]
    fn orphan_managed_backup_sidecar_fails_closed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_dir = temp.path().join("backups");
        std::fs::create_dir(&backup_dir).expect("create backup dir");
        let sidecar = backup_dir.join("hanni_20260901_010203.db-wal");
        let canary = b"orphan-managed-wal-canary";
        std::fs::write(&sidecar, canary).expect("seed orphan WAL");

        let error = migrate_backup_databases(temp.path())
            .expect_err("orphan managed sidecar must block startup");
        assert!(error.contains("orphan managed backup sidecar"));
        assert_eq!(std::fs::read(&sidecar).expect("read preserved sidecar"), canary);
    }

    #[test]
    fn unknown_backup_schema_fails_without_mutation_and_can_retry_after_repair() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_dir = temp.path().join("backups");
        std::fs::create_dir(&backup_dir).expect("create backup dir");
        let path = backup_dir.join("hanni_20260901_010203.db");
        {
            let conn = rusqlite::Connection::open(&path).expect("open backup");
            conn.execute_batch(
                "CREATE TABLE automation_log (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    ts INTEGER NOT NULL,
                    script_hash BLOB NOT NULL,
                    success INTEGER NOT NULL,
                    duration_ms INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .expect("create unknown backup schema");
        }

        let error = migrate_backup_databases(temp.path())
            .expect_err("unknown backup schema must fail closed");
        assert!(error.contains("unexpected automation_log schema"));
        {
            let conn = rusqlite::Connection::open(&path).expect("reopen failed backup");
            let declared_type: String = conn
                .query_row(
                    "SELECT type FROM pragma_table_xinfo('automation_log')
                     WHERE name='script_hash'",
                    [],
                    |row| row.get(0),
                )
                .expect("read unchanged backup schema");
            assert_eq!(declared_type, "BLOB");
            let has_complete_marker: bool = conn
                .query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM sqlite_master
                       WHERE type='table' AND name='_hanni_security_migrations'
                     )",
                    [],
                    |row| row.get(0),
                )
                .expect("inspect failed marker transaction");
            assert!(!has_complete_marker);
            conn.execute_batch("DROP TABLE automation_log;")
                .expect("repair unknown backup schema");
        }

        migrate_backup_databases(temp.path()).expect("retry repaired backup");
        let conn = rusqlite::Connection::open(&path).expect("reopen repaired backup");
        let marker: String = conn
            .query_row(
                "SELECT state FROM _hanni_security_migrations
                 WHERE name='automation_log_metadata_v1'",
                [],
                |row| row.get(0),
            )
            .expect("read completed retry marker");
        assert_eq!(marker, "complete");
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_setting_roundtrip_hides_plaintext() {
        let conn = test_db();
        let canary = "hanni-canary-secret-value";
        set_setting(&conn, "cloud_owner_gh_pat", canary).expect("set protected setting");
        let raw: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='cloud_owner_gh_pat'",
                [],
                |row| row.get(0),
            )
            .expect("read raw setting");
        assert!(raw.starts_with(PREFIX));
        assert!(!raw.contains(canary));
        assert_eq!(
            get_setting(&conn, "cloud_owner_gh_pat").expect("decrypt setting"),
            Some(canary.to_string())
        );
    }

    #[cfg(windows)]
    #[test]
    fn every_sensitive_setting_roundtrips_without_plaintext() {
        let conn = test_db();
        for (index, key) in SENSITIVE_SETTING_KEYS.iter().enumerate() {
            let canary = format!("hanni-secret-{index}-{key}");
            set_setting(&conn, key, &canary).expect("protect setting");
            let raw: String = conn
                .query_row(
                    "SELECT value FROM app_settings WHERE key=?1",
                    [*key],
                    |row| row.get(0),
                )
                .expect("read protected setting");
            assert!(raw.starts_with(PREFIX), "{key}");
            assert!(!raw.contains(&canary), "{key}");
            assert_eq!(
                get_setting(&conn, key).expect("decrypt setting"),
                Some(canary),
                "{key}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_ciphertext_is_bound_to_its_setting_slot() {
        let protected = encode_for_storage("cloud_owner_gh_pat", "canary").expect("protect canary");
        assert!(decode_from_storage("lan_sync_key", &protected).is_err());
        assert_eq!(
            decode_from_storage("cloud_owner_gh_pat", &protected).expect("correct slot"),
            "canary"
        );
    }

    #[cfg(windows)]
    #[test]
    fn failed_setting_migration_preserves_plaintext() {
        let conn = test_db();
        let canary = "migration-canary-secret";
        conn.execute(
            "INSERT INTO app_settings(key,value) VALUES('cloud_owner_gh_pat',?1)",
            [canary],
        )
        .expect("seed plaintext");
        conn.execute_batch(
            "CREATE TRIGGER block_secret_migration BEFORE UPDATE ON app_settings
             WHEN OLD.key='cloud_owner_gh_pat'
             BEGIN SELECT RAISE(ABORT, 'synthetic migration failure'); END;",
        )
        .expect("create failure trigger");

        let error = migrate_sensitive_settings(&conn).expect_err("migration must fail");
        assert!(!error.contains(canary));
        let raw: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='cloud_owner_gh_pat'",
                [],
                |row| row.get(0),
            )
            .expect("read preserved plaintext");
        assert_eq!(raw, canary);
    }

    #[cfg(windows)]
    #[test]
    fn interrupted_setting_migration_resumes_without_replacing_ciphertext() {
        let conn = test_db();
        set_setting(&conn, "cloud_owner_gh_pat", "already-protected").expect("seed protected row");
        let before: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='cloud_owner_gh_pat'",
                [],
                |row| row.get(0),
            )
            .expect("read protected row");
        conn.execute(
            "INSERT INTO app_settings(key,value) VALUES('lan_sync_key','legacy-lan-key')",
            [],
        )
        .expect("seed legacy row");

        migrate_sensitive_settings(&conn).expect("resume migration");
        let after: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='cloud_owner_gh_pat'",
                [],
                |row| row.get(0),
            )
            .expect("reread protected row");
        assert_eq!(after, before);
        assert_eq!(
            get_setting(&conn, "lan_sync_key").expect("decrypt migrated row"),
            Some("legacy-lan-key".into())
        );
    }

    #[cfg(windows)]
    #[test]
    fn migration_scrubs_plaintext_from_database_and_wal() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("migration.db");
        let canary = "hanni-physical-cleanup-canary-4491";
        {
            let conn = rusqlite::Connection::open(&path).expect("open DB");
            conn.pragma_update(None, "journal_mode", "WAL")
                .expect("enable WAL");
            conn.execute_batch(
                "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .expect("create settings");
            conn.execute(
                "INSERT INTO app_settings(key,value) VALUES('cloud_owner_gh_pat',?1)",
                [canary],
            )
            .expect("seed canary");
            migrate_sensitive_settings(&conn).expect("migrate settings");
        }
        for candidate in [
            path.clone(),
            path.with_extension("db-wal"),
            path.with_extension("db-shm"),
        ] {
            if candidate.exists() {
                let bytes = std::fs::read(&candidate).expect("read SQLite artifact");
                assert!(
                    !bytes
                        .windows(canary.len())
                        .any(|window| window == canary.as_bytes()),
                    "plaintext remained in {}",
                    candidate.display()
                );
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn first_migration_scrubs_historical_plaintext_from_freelist() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("historical.db");
        let historical_prefix = "hanni-historical-secret-canary-7734";
        let historical = format!("{historical_prefix}{}", "x".repeat(32_768));
        {
            let conn = rusqlite::Connection::open(&path).expect("open DB");
            conn.pragma_update(None, "secure_delete", "OFF")
                .expect("disable legacy cleanup");
            conn.execute_batch(
                "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .expect("create settings");
            conn.execute(
                "INSERT INTO app_settings(key,value) VALUES('cloud_owner_gh_pat',?1)",
                [&historical],
            )
            .expect("seed historical secret");
            conn.execute(
                "UPDATE app_settings SET value='current-plaintext-secret' \
                 WHERE key='cloud_owner_gh_pat'",
                [],
            )
            .expect("replace historical secret");
        }

        let legacy_bytes = std::fs::read(&path).expect("read legacy database");
        assert!(
            legacy_bytes
                .windows(historical_prefix.len())
                .any(|window| window == historical_prefix.as_bytes()),
            "fixture did not retain the historical canary"
        );

        {
            let conn = rusqlite::Connection::open(&path).expect("reopen DB");
            migrate_sensitive_settings(&conn).expect("migrate and scrub settings");
            let marker: String = conn
                .query_row(
                    "SELECT value FROM app_settings WHERE key=?1",
                    [SCRUB_MARKER],
                    |row| row.get(0),
                )
                .expect("read scrub marker");
            assert_eq!(marker, "1");
        }

        let scrubbed_bytes = std::fs::read(&path).expect("read scrubbed database");
        for canary in [historical_prefix, "current-plaintext-secret"] {
            assert!(
                !scrubbed_bytes
                    .windows(canary.len())
                    .any(|window| window == canary.as_bytes()),
                "plaintext remained after completed database scrub"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn plaintext_file_migrates_after_verified_roundtrip() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("api_token.txt");
        let canary = "11111111-1111-4111-8111-111111111111";
        std::fs::write(&path, canary).expect("seed plaintext file");

        assert_eq!(read_file(&path).expect("read and migrate"), canary);
        let stored = std::fs::read_to_string(&path).expect("read protected file");
        assert!(stored.starts_with(PREFIX));
        assert!(!stored.contains(canary));
        assert_eq!(read_file(&path).expect("read protected file"), canary);
    }

    #[cfg(windows)]
    #[test]
    fn startup_migrates_both_token_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let api = temp.path().join("api_token.txt");
        let jobs = temp.path().join("jobs_api_token.txt");
        std::fs::write(&api, "11111111-1111-4111-8111-111111111111").expect("seed API token");
        std::fs::write(&jobs, "22222222-2222-4222-8222-222222222222").expect("seed jobs token");

        migrate_token_files(temp.path()).expect("migrate token files");
        for path in [&api, &jobs] {
            let stored = std::fs::read_to_string(path).expect("read protected token");
            assert!(stored.starts_with(PREFIX));
            assert!(!stored.contains("11111111"));
            assert!(!stored.contains("22222222"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn startup_migrates_managed_backup_credentials() {
        let temp = tempfile::tempdir().expect("temp dir");
        let backup_dir = temp.path().join("backups");
        std::fs::create_dir(&backup_dir).expect("create backup dir");
        let path = backup_dir.join("hanni_20260901_010203.db");
        {
            let conn = rusqlite::Connection::open(&path).expect("open backup");
            conn.execute_batch(
                "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO app_settings VALUES ('lan_sync_key','backup-canary');",
            )
            .expect("seed backup");
        }

        migrate_backup_databases(temp.path()).expect("migrate backups");
        let conn = rusqlite::Connection::open(&path).expect("reopen backup");
        let raw: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='lan_sync_key'",
                [],
                |row| row.get(0),
            )
            .expect("read protected backup row");
        assert!(raw.starts_with(PREFIX));
        assert!(!raw.contains("backup-canary"));
        assert_eq!(
            get_setting(&conn, "lan_sync_key").expect("decrypt backup row"),
            Some("backup-canary".into())
        );
    }
}
