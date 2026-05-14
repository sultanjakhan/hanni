// DPI-correct window position/size persistence.
//
// Replaces tauri-plugin-window-state for POSITION/SIZE: that plugin stores
// PhysicalPosition and on macOS multi-monitor tao halves it on restore (uses
// primary-monitor scale to convert). We store and apply Logical* which is
// what NSWindow uses natively on macOS.
//
// Save is synchronous (std::fs::write + atomic rename) and triggered from the
// Rust run-event loop (ExitRequested / CloseRequested / Focused-out), so it
// survives cmd+Q, the close button, reboot, and auto-update — unlike the
// previous localStorage approach where WKWebView did not flush in time.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, WebviewWindow};

use crate::types::hanni_data_dir;

// Debug-only logger. Compiles to a no-op in release builds; in dev (cargo
// tauri dev) it writes to stderr. Used to diagnose multi-monitor edge cases
// without leaking noise into prod logs.
macro_rules! dlog {
    ($($arg:tt)*) => { if cfg!(debug_assertions) { eprintln!($($arg)*); } };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedWin {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

fn state_path() -> PathBuf {
    let suffix = if cfg!(debug_assertions) { "dev" } else { "prod" };
    hanni_data_dir().join(format!(".hanni-window-{}.json", suffix))
}

fn valid(s: &SavedWin) -> bool {
    s.x.is_finite() && s.y.is_finite()
        && s.w.is_finite() && s.h.is_finite()
        && s.w >= 1.0 && s.h >= 1.0
}

pub fn load() -> Option<SavedWin> {
    let raw = std::fs::read_to_string(state_path()).ok()?;
    let s: SavedWin = serde_json::from_str(&raw).ok()?;
    if !valid(&s) { return None; }
    Some(s)
}

pub fn save<R: Runtime>(window: &WebviewWindow<R>) {
    let sf = match window.scale_factor() { Ok(v) => v, Err(e) => {
        dlog!("[window_state] save: scale_factor err: {e:?}"); return;
    }};
    let pos = match window.outer_position() { Ok(v) => v, Err(e) => {
        dlog!("[window_state] save: outer_position err: {e:?}"); return;
    }};
    let sz = match window.outer_size() { Ok(v) => v, Err(e) => {
        dlog!("[window_state] save: outer_size err: {e:?}"); return;
    }};
    let lp = pos.to_logical::<f64>(sf);
    let ls = sz.to_logical::<f64>(sf);
    let s = SavedWin { x: lp.x, y: lp.y, w: ls.width, h: ls.height };
    dlog!(
        "[window_state] save: physical=({},{}) {}x{} sf={} → logical=({:.1},{:.1}) {:.1}x{:.1}",
        pos.x, pos.y, sz.width, sz.height, sf, s.x, s.y, s.w, s.h
    );
    if !valid(&s) {
        dlog!("[window_state] save: invalid SavedWin, skipping");
        return;
    }

    let path = state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string(&s) else { return };
    // Atomic write: write to .tmp, then rename. Prevents a half-written file
    // if the process is killed mid-write (cmd+Q, reboot).
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp, json) {
        dlog!("[window_state] save: tmp write err: {e:?}");
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        dlog!("[window_state] save: rename err: {e:?}");
    } else {
        dlog!("[window_state] save: wrote {}", path.display());
    }
}

pub fn apply<R: Runtime>(window: &WebviewWindow<R>, s: &SavedWin) {
    // Probe whether the saved top-left intersects any currently-attached
    // monitor; warn if not (e.g. external display unplugged) but still apply,
    // because macOS NSWindow auto-clamps off-screen frames to the nearest
    // visible screen — a much saner fallback than skipping restore entirely.
    let monitors = match window.available_monitors() {
        Ok(v) => v,
        Err(e) => {
            dlog!("[window_state] apply: available_monitors err: {e:?}, applying anyway");
            Vec::new()
        }
    };
    dlog!("[window_state] apply: saved=({:.1},{:.1}) {:.1}x{:.1}, {} monitors", s.x, s.y, s.w, s.h, monitors.len());
    let mut visible = false;
    for (i, m) in monitors.iter().enumerate() {
        let ms = m.scale_factor();
        let mp = m.position().to_logical::<f64>(ms);
        let msz = m.size().to_logical::<f64>(ms);
        let probe_x = s.x + 50.0;
        let probe_y = s.y + 50.0;
        let hit = probe_x >= mp.x && probe_x < mp.x + msz.width
            && probe_y >= mp.y && probe_y < mp.y + msz.height;
        dlog!(
            "[window_state] apply: monitor[{}] origin=({:.1},{:.1}) size={:.1}x{:.1} sf={} hit={}",
            i, mp.x, mp.y, msz.width, msz.height, ms, hit
        );
        if hit { visible = true; }
    }
    if !visible && !monitors.is_empty() {
        dlog!("[window_state] apply: saved position is off all monitors — applying anyway, OS will clamp");
    }

    // Size first, then position: a resize alone may shift the window slightly
    // on macOS, so applying position last pins it to the saved spot.
    if let Err(e) = window.set_size(LogicalSize::new(s.w, s.h)) {
        dlog!("[window_state] apply: set_size err: {e:?}");
    }
    if let Err(e) = window.set_position(LogicalPosition::new(s.x, s.y)) {
        dlog!("[window_state] apply: set_position err: {e:?}");
    } else {
        dlog!("[window_state] apply: set_position done");
    }
}

pub fn restore_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(saved) = load() else { return };
    let Some(w) = app.get_webview_window("main") else { return };
    apply(&w, &saved);
}

pub fn save_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(w) = app.get_webview_window("main") else { return };
    save(&w);
}
