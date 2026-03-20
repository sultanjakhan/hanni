// proactive/mod.rs — Re-exports for the proactive module
pub mod triggers;
pub mod actions;
pub mod loop_runner;
pub mod settings;

// Only re-export functions used outside this module (lib.rs, chat.rs, macos.rs)
pub use triggers::{
    get_frontmost_app, get_browser_url, get_window_title, classify_activity,
    get_now_playing_sync, get_upcoming_events_soon, truncate_utf8,
};
pub use loop_runner::proactive_loop;
// Re-export with wildcard so that #[tauri::command] generated items
// (__cmd__*, etc.) are also visible at crate::proactive::*
pub use settings::*;
