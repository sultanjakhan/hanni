//! Cooperative process ownership for one existing private Hanni data directory.
//! Acquire after single-instance notification, before any database initialization.
//! No Tauri state/Exit hook owns this handle: it remains locked until OS process exit.
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

const FILE_NAME: &str = ".hanni-process.lock";
const WAIT: Duration = Duration::from_secs(2);
const STEP: Duration = Duration::from_millis(25);
const BUSY: &str = "hanni_process_busy";
const ERROR: &str = "hanni_process_lease_unavailable";

struct Lease {
    directory: PathBuf,
    // Static storage is intentional: no cleanup_before_exit/drop-before-restart.
    // Standard File opens are noninheritable on Windows and CLOEXEC on Unix;
    // never clone/export the handle to a child or explicitly unlock it.
    _file: File,
}
static OWNER: Mutex<Option<Lease>> = Mutex::new(None);

/// The caller creates/secures this absolute directory before invoking this function.
/// This creates only an empty stable lock file; it does not create/open a database.
pub(crate) fn acquire_for_process(data_dir: &Path) -> Result<(), &'static str> {
    acquire_with_timeout(data_dir, WAIT)
}

fn acquire_with_timeout(data_dir: &Path, wait: Duration) -> Result<(), &'static str> {
    if !data_dir.is_absolute() || !data_dir.is_dir() {
        return Err(ERROR);
    }
    let directory = data_dir.canonicalize().map_err(|_| ERROR)?;
    let mut owner = OWNER.lock().map_err(|_| ERROR)?;
    if let Some(owner) = owner.as_ref() {
        return if owner.directory == directory {
            Ok(())
        } else {
            Err(ERROR)
        };
    }
    let path = directory.join(FILE_NAME);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(ERROR)
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(ERROR),
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ | FILE_SHARE_WRITE. Do not allow replacing/unlinking
        // this lock file while any process waits on or holds its handle.
        options.share_mode(0x0000_0001 | 0x0000_0002);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path).map_err(|_| ERROR)?;
    if !file.metadata().map_err(|_| ERROR)?.is_file() {
        return Err(ERROR);
    }
    let deadline = Instant::now().checked_add(wait.min(WAIT)).ok_or(ERROR)?;
    loop {
        match file.try_lock() {
            Ok(()) => {
                *owner = Some(Lease {
                    directory,
                    _file: file,
                });
                return Ok(());
            }
            Err(TryLockError::WouldBlock) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(BUSY);
                }
                std::thread::sleep(remaining.min(STEP));
            }
            Err(TryLockError::Error(_)) => return Err(ERROR),
        }
    }
}

// Never remove FILE_NAME on normal exit, crash, cleanup, or mode handoff. On Unix
// unlink+recreate would let two processes lock different inodes at the same path.
