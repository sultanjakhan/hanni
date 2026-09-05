//! Read-only Android startup evidence check. Never read identities or decrypt configuration.
use rusqlite::OpenFlags;
use std::{fs, io::ErrorKind, path::Path};

const CHECK_FAILED: &str = "database_startup_check_failed";
const RECOVERY_REQUIRED: &str = "database_recovery_required";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenPolicy { Existing, Fresh }

fn metadata(path: &Path) -> Result<Option<fs::Metadata>, &'static str> {
    match fs::symlink_metadata(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(CHECK_FAILED),
    }
}

fn directory_if_present(path: &Path) -> Result<bool, &'static str> {
    match metadata(path)? {
        None => Ok(false),
        Some(value) if value.is_dir() && !value.file_type().is_symlink() => Ok(true),
        Some(_) => Err(CHECK_FAILED),
    }
}

fn has_managed_backup(data_dir: &Path) -> Result<bool, &'static str> {
    let backups = data_dir.join("backups");
    if directory_if_present(&backups)? {
        for (count, entry) in fs::read_dir(backups).map_err(|_| CHECK_FAILED)?.enumerate() {
            if count >= 1024 { return Err(CHECK_FAILED); }
            let entry = entry.map_err(|_| CHECK_FAILED)?;
            let name = entry.file_name();
            if let Some(name) = name.to_str() {
                if let Some(stem) = name.strip_prefix("hanni_") {
                    if [".db", ".db-wal", ".db-shm"].iter().any(|suffix|
                        stem.strip_suffix(suffix).is_some_and(|body| !body.is_empty())) {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub(crate) fn inspect_android(data_dir: &Path) -> Result<OpenPolicy, &'static str> {
    directory_if_present(data_dir)?;
    if let Some(value) = metadata(&data_dir.join("hanni.db"))? {
        return if value.is_file() && !value.file_type().is_symlink() {
            Ok(OpenPolicy::Existing)
        } else { Err(CHECK_FAILED) };
    }
    for name in ["hanni.db-wal", "hanni.db-shm", "hanni.db-journal",
        "hanni.db.corrupt", "hanni.db.corrupt-wal", "hanni.db.corrupt-shm", "hanni.db.corrupt-journal"] {
        if metadata(&data_dir.join(name))?.is_some() { return Err(RECOVERY_REQUIRED); }
    }
    // Tauri uses the application root; lib.rs has an explicit private-files fallback.
    // Alternate paths are evidence only: never open, select or migrate their databases.
    let app_root = if data_dir.file_name().is_some_and(|name| name == "files") {
        data_dir.parent().ok_or(CHECK_FAILED)?
    } else { data_dir };
    directory_if_present(app_root)?;
    let alternate = if data_dir == app_root { app_root.join("files") } else { app_root.to_path_buf() };
    if directory_if_present(&alternate)? {
        for name in ["hanni.db", "hanni.db-wal", "hanni.db-shm", "hanni.db-journal",
            "hanni.db.corrupt", "hanni.db.corrupt-wal", "hanni.db.corrupt-shm", "hanni.db.corrupt-journal"] {
            if metadata(&alternate.join(name))?.is_some() { return Err(RECOVERY_REQUIRED); }
        }
        if has_managed_backup(&alternate)? { return Err(RECOVERY_REQUIRED); }
    }
    let identities = app_root.join("no_backup");
    if directory_if_present(&identities)? {
        for name in ["hc-source-store-v1", "hc-source-store-v1.bak", "hc-source-store-v1.new",
            "relay-config-v1.enc", "relay-config-v1.enc.bak", "relay-config-v1.enc.new"] {
            if metadata(&identities.join(name))?.is_some() { return Err(RECOVERY_REQUIRED); }
        }
    }
    if has_managed_backup(data_dir)? { return Err(RECOVERY_REQUIRED); }
    Ok(OpenPolicy::Fresh)
}

pub(crate) fn existing_flags() -> OpenFlags {
    OpenFlags::default() & !OpenFlags::SQLITE_OPEN_CREATE
}

pub(crate) fn checked_open_flags(data_dir: &Path, policy: OpenPolicy) -> Result<OpenFlags, &'static str> {
    // An observed existing database must never regain CREATE if it disappears later.
    if policy == OpenPolicy::Existing { return Ok(existing_flags()); }
    // Recheck fresh-install evidence immediately before the SQL open.
    match inspect_android(data_dir)? {
        OpenPolicy::Existing => Ok(existing_flags()),
        OpenPolicy::Fresh => Ok(OpenFlags::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn fresh_directory_check_does_not_create_files_or_directories() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("app");
        assert_eq!(inspect_android(&missing), Ok(OpenPolicy::Fresh));
        assert!(!missing.exists());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn fresh_install_can_create_and_reopen_real_sqlite_database() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let policy = inspect_android(root).unwrap();
        let conn = Connection::open_with_flags(root.join("hanni.db"), checked_open_flags(root, policy).unwrap()).unwrap();
        conn.execute_batch("CREATE TABLE preserved(value INTEGER); INSERT INTO preserved VALUES(7);").unwrap();
        drop(conn);
        assert_eq!(inspect_android(root), Ok(OpenPolicy::Existing));
        let conn = Connection::open_with_flags(root.join("hanni.db"), existing_flags()).unwrap();
        assert_eq!(conn.query_row("SELECT value FROM preserved", [], |row| row.get::<_, i64>(0)).unwrap(), 7);
    }

    #[test]
    fn existing_database_removed_after_inspection_is_not_recreated() {
        let temp = tempfile::tempdir().unwrap(); let path = temp.path().join("hanni.db");
        fs::write(&path, b"synthetic bytes").unwrap();
        let policy = inspect_android(temp.path()).unwrap();
        fs::remove_file(&path).unwrap();
        let flags = checked_open_flags(temp.path(), policy).unwrap();
        assert!(!flags.contains(OpenFlags::SQLITE_OPEN_CREATE));
        assert!(Connection::open_with_flags(&path, flags).is_err()); assert!(!path.exists());
    }

    #[test]
    fn every_identity_envelope_and_atomic_sidecar_blocks_without_reading_contents() {
        for name in ["hc-source-store-v1", "hc-source-store-v1.bak", "hc-source-store-v1.new",
            "relay-config-v1.enc", "relay-config-v1.enc.bak", "relay-config-v1.enc.new"] {
            let temp = tempfile::tempdir().unwrap(); let root = temp.path();
            fs::create_dir(root.join("no_backup")).unwrap();
            let marker = root.join("no_backup").join(name);
            fs::write(&marker, b"deliberately invalid public bytes").unwrap();
            let before = fs::read(&marker).unwrap();
            assert_eq!(inspect_android(root), Err(RECOVERY_REQUIRED));
            assert_eq!(fs::read(&marker).unwrap(), before); assert!(!root.join("hanni.db").exists());
        }
    }

    #[test]
    fn files_fallback_uses_parent_markers_but_never_opens_parent_database() {
        let temp = tempfile::tempdir().unwrap(); let root = temp.path();
        fs::create_dir(root.join("files")).unwrap(); fs::create_dir(root.join("no_backup")).unwrap();
        fs::write(root.join("hanni.db"), b"other database is not selected").unwrap();
        fs::write(root.join("no_backup/hc-source-store-v1"), b"public marker").unwrap();
        assert_eq!(inspect_android(&root.join("files")), Err(RECOVERY_REQUIRED));
        assert!(!root.join("files/hanni.db").exists());
    }

    #[test]
    fn selected_database_sidecars_and_managed_backups_block_recreation() {
        for name in ["hanni.db-wal", "hanni.db-shm", "hanni.db-journal", "backups/hanni_20260906_000000.db", "backups/hanni_20260906_000000.db-wal"] {
            let temp = tempfile::tempdir().unwrap(); let root = temp.path();
            fs::create_dir(root.join("backups")).unwrap(); fs::write(root.join(name), b"public evidence").unwrap();
            assert_eq!(inspect_android(root), Err(RECOVERY_REQUIRED)); assert!(!root.join("hanni.db").exists());
        }
    }

    #[test]
    fn ordinary_android_bootstrap_files_and_empty_backup_directory_allow_fresh_install() {
        let temp = tempfile::tempdir().unwrap(); let root = temp.path();
        for name in ["databases", "shared_prefs", "no_backup", "backups", "cache"] { fs::create_dir(root.join(name)).unwrap(); }
        fs::write(root.join("databases/androidx.work.workdb"), b"public work manager fixture").unwrap();
        fs::write(root.join("backups/readme.txt"), b"public unrelated file").unwrap();
        assert_eq!(inspect_android(root), Ok(OpenPolicy::Fresh));
    }

    #[test]
    fn nonregular_main_or_marker_directory_fails_closed() {
        let temp = tempfile::tempdir().unwrap(); fs::create_dir(temp.path().join("hanni.db")).unwrap();
        assert_eq!(inspect_android(temp.path()), Err(CHECK_FAILED));
        let temp = tempfile::tempdir().unwrap(); fs::write(temp.path().join("no_backup"), b"public fixture").unwrap();
        assert_eq!(inspect_android(temp.path()), Err(CHECK_FAILED));
    }

    #[test]
    fn evidence_arriving_after_fresh_inspection_still_blocks_create() {
        let temp = tempfile::tempdir().unwrap(); let root = temp.path();
        let policy = inspect_android(root).unwrap(); fs::write(root.join("hanni.db-wal"), b"public evidence").unwrap();
        assert_eq!(checked_open_flags(root, policy), Err(RECOVERY_REQUIRED)); assert!(!root.join("hanni.db").exists());
    }

    fn check_alternate_family(selected_files: bool) {
        for name in ["hanni.db", "hanni.db-wal", "hanni.db-shm", "hanni.db-journal",
            "backups/hanni_20260906_000000.db", "backups/hanni_20260906_000000.db-wal", "backups/hanni_20260906_000000.db-shm"] {
            let temp = tempfile::tempdir().unwrap(); let root = temp.path();
            fs::create_dir(root.join("files")).unwrap();
            let selected = if selected_files { root.join("files") } else { root.to_path_buf() };
            let alternate = if selected_files { root.to_path_buf() } else { root.join("files") };
            fs::create_dir(alternate.join("backups")).unwrap();
            let evidence = alternate.join(name); fs::write(&evidence, b"public alternate evidence").unwrap();
            assert_eq!(inspect_android(&selected), Err(RECOVERY_REQUIRED));
            assert_eq!(fs::read(&evidence).unwrap(), b"public alternate evidence");
            assert!(!selected.join("hanni.db").exists());
        }
    }

    #[test]
    fn missing_root_main_refuses_files_alternate_without_identity_markers() { check_alternate_family(false); }

    #[test]
    fn missing_files_main_refuses_root_alternate_without_identity_markers() { check_alternate_family(true); }

    fn check_preserved_corrupt_family(selected_files: bool, evidence_files: bool) {
        for name in ["hanni.db.corrupt", "hanni.db.corrupt-wal", "hanni.db.corrupt-shm", "hanni.db.corrupt-journal"] {
            let temp = tempfile::tempdir().unwrap(); let root = temp.path();
            fs::create_dir(root.join("files")).unwrap();
            let selected = if selected_files { root.join("files") } else { root.to_path_buf() };
            let evidence_dir = if evidence_files { root.join("files") } else { root.to_path_buf() };
            let evidence = evidence_dir.join(name);
            let bytes: &[u8] = if name == "hanni.db.corrupt" { b"public preserved corrupt fixture" } else { b"" };
            fs::write(&evidence, bytes).unwrap();
            assert_eq!(inspect_android(&selected), Err(RECOVERY_REQUIRED));
            assert!(!selected.join("hanni.db").exists());
            assert_eq!(fs::read(&evidence).unwrap(), bytes);
        }
    }

    #[test]
    fn root_selection_refuses_preserved_root_corrupt_family() { check_preserved_corrupt_family(false, false); }

    #[test]
    fn root_selection_refuses_preserved_files_corrupt_family() { check_preserved_corrupt_family(false, true); }

    #[test]
    fn files_selection_refuses_preserved_root_corrupt_family() { check_preserved_corrupt_family(true, false); }

    #[test]
    fn files_selection_refuses_preserved_files_corrupt_family() { check_preserved_corrupt_family(true, true); }

    #[test]
    #[cfg(windows)]
    fn denied_backup_directory_is_not_treated_as_fresh_install() {
        use std::process::Command;
        let temp = tempfile::tempdir().unwrap(); let root = temp.path();
        let backups = root.join("backups"); fs::create_dir(&backups).unwrap();
        let denied = Command::new("icacls.exe").arg(&backups).args(["/deny", "*S-1-1-0:(RD)"]).output().unwrap();
        assert!(denied.status.success(), "synthetic ACL setup failed");
        // Capture results before restoring the fixture ACL; assertions occur after cleanup.
        let read_error = fs::read_dir(&backups).err().map(|error| error.kind());
        let decision = inspect_android(root);
        let restored = Command::new("icacls.exe").arg(&backups).args(["/remove:d", "*S-1-1-0"]).output().unwrap();
        assert!(restored.status.success(), "synthetic ACL restore failed");
        assert_eq!(read_error, Some(ErrorKind::PermissionDenied));
        assert_eq!(decision, Err(CHECK_FAILED)); assert!(!root.join("hanni.db").exists());
    }
}
