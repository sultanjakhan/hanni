// mlx_manager.rs — On-demand MLX server: start/stop with idle timeout
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use std::process::Child;

const IDLE_TIMEOUT_SECS: i64 = 300; // 5 minutes
const MLX_HEALTH_URL: &str = "http://127.0.0.1:8234/v1/models";

static MLX: std::sync::OnceLock<MlxManager> = std::sync::OnceLock::new();

pub struct MlxManager {
    process: Mutex<Option<Child>>,
    last_request: AtomicI64,
}

// Safety: Child is Send, Mutex provides synchronization
unsafe impl Sync for MlxManager {}

impl MlxManager {
    fn new() -> Self {
        Self { process: Mutex::new(None), last_request: AtomicI64::new(0) }
    }
}

/// Initialize the global MLX manager. Call once at app startup.
pub fn init() {
    MLX.get_or_init(MlxManager::new);
}

/// Ensure MLX is running. Call before any MLX_URL request.
pub fn ensure_mlx() -> bool {
    let mgr = match MLX.get() { Some(m) => m, None => return false };
    let now = chrono::Utc::now().timestamp();
    mgr.last_request.store(now, Ordering::Relaxed);
    if is_healthy() { return true; }
    eprintln!("[mlx-manager] MLX not running, starting...");
    let new_child = crate::commands_meta::start_mlx_server();
    {
        let mut child = mgr.process.lock().unwrap_or_else(|e| e.into_inner());
        *child = new_child;
    }
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if is_healthy() {
            eprintln!("[mlx-manager] MLX server ready");
            return true;
        }
    }
    eprintln!("[mlx-manager] MLX failed to start within 30s");
    false
}

/// Stop MLX server process
pub fn stop() {
    if let Some(mgr) = MLX.get() {
        let mut child = mgr.process.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref mut proc) = *child {
            eprintln!("[mlx-manager] Stopping MLX server");
            let _ = proc.kill();
            let _ = proc.wait();
        }
        *child = None;
    }
}

fn is_healthy() -> bool {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build().ok()
        .and_then(|c| c.get(MLX_HEALTH_URL).send().ok())
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Background watchdog: checks idle every 60s, stops MLX if idle > timeout
pub fn spawn_idle_watchdog() {
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            if let Some(mgr) = MLX.get() {
                let last = mgr.last_request.load(Ordering::Relaxed);
                if last == 0 { continue; }
                let now = chrono::Utc::now().timestamp();
                if now - last > IDLE_TIMEOUT_SECS && is_healthy() {
                    stop();
                    eprintln!("[mlx-manager] MLX stopped after idle timeout");
                }
            }
        }
    });
}
