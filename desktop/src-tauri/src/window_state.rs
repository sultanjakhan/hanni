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
    let sf = match window.scale_factor() { Ok(v) => v, Err(_) => return };
    let pos = match window.outer_position() { Ok(v) => v, Err(_) => return };
    let sz = match window.outer_size() { Ok(v) => v, Err(_) => return };
    let lp = pos.to_logical::<f64>(sf);
    let ls = sz.to_logical::<f64>(sf);
    let s = SavedWin { x: lp.x, y: lp.y, w: ls.width, h: ls.height };
    if !valid(&s) { return; }

    let path = state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string(&s) else { return };
    // Atomic write: write to .tmp, then rename. Prevents a half-written file
    // if the process is killed mid-write (cmd+Q, reboot).
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_err() { return; }
    let _ = std::fs::rename(&tmp, &path);
}

pub fn apply<R: Runtime>(window: &WebviewWindow<R>, s: &SavedWin) {
    // Skip if the saved top-left isn't inside any currently-attached monitor
    // (e.g. external display unplugged). Lets the OS place the window in a
    // sane default rather than off-screen.
    let monitors = match window.available_monitors() { Ok(v) => v, Err(_) => return };
    let visible = monitors.iter().any(|m| {
        let ms = m.scale_factor();
        let mp = m.position().to_logical::<f64>(ms);
        let msz = m.size().to_logical::<f64>(ms);
        let probe_x = s.x + 50.0;
        let probe_y = s.y + 50.0;
        probe_x >= mp.x && probe_x < mp.x + msz.width
            && probe_y >= mp.y && probe_y < mp.y + msz.height
    });
    if !visible { return; }

    // Size first, then position: a resize alone may shift the window slightly
    // on macOS, so applying position last pins it to the saved spot.
    let _ = window.set_size(LogicalSize::new(s.w, s.h));
    let _ = window.set_position(LogicalPosition::new(s.x, s.y));
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
