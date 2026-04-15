// lib.rs — Modular orchestrator: declares modules, wires state, registers commands, runs setup.

mod types;
mod prompts;
mod db;
mod chat;
mod memory;
#[cfg(not(target_os = "android"))]
mod voice;
#[cfg(target_os = "android")]
#[path = "voice_stubs.rs"]
mod voice;
mod proactive;
mod macos;
mod calendar;
mod notes;
mod commands_data;
mod commands_meta;
mod mcp;
mod agent;
mod vacancy;
mod dashboard;
mod commands_timeline;
mod timeline_stats;
mod timeline_afk;
mod mlx_manager;
mod sync;
mod sync_commands;
mod health_connect;
mod health_connect_plugin;
mod health_import;
mod sleep_analysis;
mod timeline_health;

// Re-export types used by run() for state setup
use types::*;
use prompts::SYSTEM_PROMPT;
use db::*;
use memory::load_proactive_settings;
#[cfg(not(target_os = "android"))]
use memory::{embed_texts, store_fact_embedding};
#[cfg(not(target_os = "android"))]
use voice::speak_silero_core;
#[cfg(not(target_os = "android"))]
use proactive::{
    get_frontmost_app, get_browser_url, get_now_playing_sync,
    get_upcoming_events_soon, proactive_loop,
};
#[cfg(not(target_os = "android"))]
use macos::run_osascript;
use commands_meta::spawn_api_server;
#[cfg(not(target_os = "android"))]
use commands_meta::{
    updater_with_headers,
    ensure_voice_server_launchagent, ensure_openclaw_gateway,
};

// Imports needed by run()
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tauri::{Emitter, Manager};
#[cfg(not(target_os = "android"))]
use chrono::{Timelike, Datelike};

/// Load cr-sqlite extension for CRDT-based sync
fn load_crsqlite(conn: &rusqlite::Connection) {
    unsafe { conn.load_extension_enable().ok(); }
    let lib_path = crsqlite_lib_path();
    match unsafe { conn.load_extension(&lib_path, Some("sqlite3_crsqlite_init")) } {
        Ok(_) => eprintln!("cr-sqlite loaded from {:?}", lib_path),
        Err(e) => eprintln!("Warning: cr-sqlite not loaded: {e}. Sync disabled."),
    }
    unsafe { conn.load_extension_disable().ok(); }
}

fn crsqlite_lib_path() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    let name = "crsqlite.dylib";
    #[cfg(target_os = "android")]
    let name = "crsqlite.so";
    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    let name = "crsqlite.so";

    // In dev mode, load from libs/ relative to src-tauri
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("libs")
        .join(if cfg!(target_os = "macos") { "darwin-aarch64" } else { "android-aarch64" })
        .join(name);
    if dev_path.exists() { return dev_path; }

    // In production, load from Tauri resource dir
    let data_dir = types::hanni_data_dir();
    data_dir.parent().unwrap_or(&data_dir).join(name)
}

/// Initialize SQLite database: register extensions, open connection, run migrations.
/// Requires set_data_dir() to have been called on Android.
fn init_database() -> HanniDb {
    // Migrate data from ~/Documents/Hanni/ (macOS only)
    #[cfg(not(target_os = "android"))]
    migrate_old_data_dir();

    // Register sqlite-vec extension BEFORE opening any connection
    unsafe {
        use rusqlite::ffi::sqlite3_auto_extension;
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const ()
        )));
    }

    let db_path = hanni_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    backup_db();
    let conn = rusqlite::Connection::open(&db_path)
        .expect("Cannot open hanni.db");

    load_crsqlite(&conn);

    init_db(&conn).expect("Cannot initialize database");
    seed_ingredient_catalog(&conn);
    seed_default_cuisines(&conn);
    migrate_memory_json(&conn);
    migrate_events_source(&conn);
    migrate_facts_decay(&conn);
    migrate_conversations_category(&conn);
    migrate_proactive_history_v2(&conn);
    migrate_proactive_messages_rating(&conn);
    migrate_recipe_difficulty(&conn);
    migrate_recipe_extra(&conn);
    migrate_recipe_extra2(&conn);
    migrate_clear_seed_recipes(&conn);
    migrate_notes_v2(&conn);
    migrate_content_blocks(&conn);
    migrate_activity_tracking(&conn);
    migrate_schedules(&conn);
    migrate_custom_projects(&conn);
    migrate_body_records(&conn);
    migrate_job_search(&conn);
    migrate_dashboard_widgets(&conn);
    migrate_timeline(&conn);
    db::migrate_sleep(&conn);
    db::enable_crr_tables(&conn);

    // Load calendar toggle from DB into static flag
    if let Ok(val) = conn.query_row(
        "SELECT value FROM app_settings WHERE key='apple_calendar_enabled'",
        [], |row| row.get::<_, String>(0),
    ) {
        APPLE_CALENDAR_DISABLED.store(val == "false", Ordering::Relaxed);
    }
    if let Ok(val) = conn.query_row(
        "SELECT value FROM app_settings WHERE key='calendar_access_ok'",
        [], |row| row.get::<_, String>(0),
    ) {
        if val == "true" { CALENDAR_ACCESS_CHECKED.store(true, Ordering::Relaxed); }
    }
    if let Ok(val) = conn.query_row(
        "SELECT value FROM app_settings WHERE key='calendar_access_denied'",
        [], |row| row.get::<_, String>(0),
    ) {
        if val == "true" { CALENDAR_ACCESS_DENIED.store(true, Ordering::Relaxed); }
    }

    HanniDb(std::sync::Mutex::new(conn))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Desktop: init DB before builder (dirs crate works on macOS)
    #[cfg(not(target_os = "android"))]
    let proactive_settings = load_proactive_settings();
    #[cfg(target_os = "android")]
    let proactive_settings = ProactiveSettings::default();
    let proactive_state = Arc::new(Mutex::new(ProactiveState::new(proactive_settings)));

    #[cfg(not(target_os = "android"))]
    let hanni_db = init_database();

    // Desktop-only: MLX server (on-demand — starts when needed, stops after 5min idle)
    #[cfg(not(target_os = "android"))]
    mlx_manager::init();
    #[cfg(not(target_os = "android"))]
    mlx_manager::spawn_idle_watchdog();

    #[cfg(not(target_os = "android"))]
    let openclaw_child = ensure_openclaw_gateway();
    #[cfg(not(target_os = "android"))]
    let openclaw_process = Arc::new(OpenClawProcess(std::sync::Mutex::new(openclaw_child)));
    #[cfg(not(target_os = "android"))]
    let openclaw_cleanup = openclaw_process.clone();

    #[cfg(not(target_os = "android"))]
    std::thread::spawn(|| {
        ensure_voice_server_launchagent();
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let ok = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build().ok()
                .and_then(|c| c.get(&format!("{}/health", VOICE_SERVER_URL)).send().ok())
                .map(|r| r.status().is_success())
                .unwrap_or(false);
            if ok {
                eprintln!("[voice] Server ready, warming up TTS...");
                let _ = speak_silero_core("тест", "xenia");
                eprintln!("[voice] TTS warmup done");
                break;
            }
        }
    });

    // Audio recording state (capture starts lazily on first recording)
    let audio_state = Arc::new(AudioRecording(std::sync::Mutex::new(WhisperState {
        recording: false,
        audio_buffer: Vec::new(),
        capture_running: false,
    }), std::sync::Condvar::new()));

    // Focus mode state
    let focus_monitor_flag = Arc::new(AtomicBool::new(false));
    let focus_manager = FocusManager(std::sync::Mutex::new(FocusState {
        active: false,
        end_time: None,
        blocked_apps: Vec::new(),
        blocked_sites: Vec::new(),
        monitor_running: focus_monitor_flag.clone(),
    }));

    // Call mode state
    let call_mode = Arc::new(CallMode(std::sync::Mutex::new(CallModeState {
        active: false,
        phase: "idle".into(),
        audio_buffer: Vec::new(),
        speech_frames: 0,
        silence_frames: 0,
        barge_in: false,
        last_recording: Vec::new(),
        transcription_gen: 0,
    })));

    let builder = tauri::Builder::default()
        .manage(HttpClient(reqwest::Client::new()))
        .manage(LlmBusy(tokio::sync::Semaphore::new(1)))
        .manage(proactive_state.clone())
        .manage(audio_state)
        .manage(focus_manager)
        .manage(call_mode)
        .manage(mcp::McpState::empty())
        .manage(commands_meta::AutoEvalCallbacks(std::sync::Mutex::new(std::collections::HashMap::new())))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(health_connect_plugin::init());

    // Desktop: manage DB state on builder (initialized before builder)
    #[cfg(not(target_os = "android"))]
    let builder = builder.manage(hanni_db);

    #[cfg(not(target_os = "android"))]
    let builder = builder.plugin(tauri_plugin_global_shortcut::Builder::new().build());

    builder
        .invoke_handler(tauri::generate_handler![
            // Chat
            chat::chat,
            chat::read_file,
            chat::list_dir,
            // Tracker
            commands_data::tracker_add_purchase,
            commands_data::tracker_add_time,
            commands_data::tracker_add_goal,
            commands_data::tracker_add_note,
            commands_data::tracker_get_stats,
            commands_data::tracker_get_recent,
            // Integrations & Meta
            commands_meta::get_integrations,
            commands_meta::get_model_info,
            commands_meta::health_check,
            // macOS
            macos::get_activity_summary,
            macos::get_calendar_events,
            macos::get_now_playing,
            macos::get_browser_tab,
            commands_meta::get_app_version,
            commands_meta::check_update,
            // Proactive
            proactive::get_proactive_settings,
            proactive::set_proactive_settings,
            proactive::set_user_typing,
            proactive::set_recording_state,
            proactive::report_proactive_engagement,
            proactive::report_user_chat_activity,
            // Memory
            memory::memory_remember,
            memory::memory_recall,
            memory::memory_forget,
            memory::memory_list,
            memory::memory_search,
            memory::save_conversation,
            memory::update_conversation,
            memory::get_conversations,
            memory::get_conversation,
            memory::delete_conversation,
            memory::search_conversations,
            memory::process_conversation_end,
            // TTS
            voice::speak_text,
            voice::stop_speaking,
            voice::get_tts_voices,
            // Voice / Whisper
            voice::download_whisper_model,
            voice::start_recording,
            voice::stop_recording,
            voice::check_whisper_model,
            // Focus
            commands_meta::start_focus,
            commands_meta::stop_focus,
            commands_meta::get_focus_status,
            commands_meta::update_blocklist,
            // Training
            commands_data::get_training_stats,
            commands_data::export_training_data,
            commands_data::get_adapter_status,
            commands_data::run_finetune,
            commands_data::rate_message,
            commands_data::get_message_ratings,
            // Actions
            macos::run_shell,
            macos::open_url,
            macos::send_notification,
            macos::set_volume,
            macos::open_app,
            macos::close_app,
            macos::music_control,
            macos::set_reminder,
            macos::get_reminders,
            macos::delete_reminder,
            macos::get_clipboard,
            macos::set_clipboard,
            macos::web_search,
            macos::read_url,
            // Activities (Focus)
            commands_data::start_activity,
            commands_data::stop_activity,
            commands_data::get_current_activity,
            commands_data::get_activity_log,
            commands_data::get_all_activities,
            commands_data::update_activity,
            commands_data::delete_activity,
            // Notes
            notes::create_note,
            notes::update_note,
            notes::delete_note,
            notes::toggle_note_pin,
            notes::toggle_note_archive,
            notes::get_notes,
            notes::get_note,
            notes::update_note_status,
            notes::reorder_notes,
            notes::get_note_tags,
            notes::set_note_tag_color,
            notes::get_notes_for_tab,
            // Events (Calendar)
            calendar::create_event,
            calendar::get_events,
            calendar::delete_event,
            calendar::update_event,
            calendar::get_all_events,
            // Calendar Sync
            calendar::sync_apple_calendar,
            calendar::sync_google_ics,
            // Job Search CRM (Work)
            commands_data::get_job_sources,
            commands_data::add_job_source,
            commands_data::update_job_source,
            commands_data::delete_job_source,
            commands_data::get_job_roles,
            commands_data::add_job_role,
            commands_data::update_job_role,
            commands_data::delete_job_role,
            commands_data::get_job_vacancies,
            commands_data::add_job_vacancy,
            commands_data::update_job_vacancy,
            commands_data::delete_job_vacancy,
            commands_data::restore_job_vacancy,
            commands_data::get_job_stats,
            commands_data::add_job_search_log,
            commands_data::get_job_search_log,
            // Learning Items (Development)
            commands_data::create_learning_item,
            commands_data::get_learning_items,
            commands_data::update_learning_item_status,
            commands_data::update_learning_item,
            commands_data::delete_learning_item,
            // Dev Projects / Skills / Cases
            commands_data::get_dev_projects,
            commands_data::create_dev_project,
            commands_data::delete_dev_project,
            commands_data::get_dev_skills,
            commands_data::create_dev_skill,
            commands_data::update_dev_skill,
            commands_data::delete_dev_skill,
            commands_data::get_dev_cases,
            commands_data::create_dev_case,
            commands_data::update_dev_case,
            commands_data::delete_dev_case,
            // Hobbies
            commands_data::create_hobby,
            commands_data::get_hobbies,
            commands_data::log_hobby_entry,
            commands_data::get_hobby_entries,
            // Workouts (Sports)
            commands_data::create_workout,
            commands_data::get_workouts,
            commands_data::get_workout_stats,
            commands_data::delete_workout,
            commands_data::update_workout,
            // Schedules
            commands_data::create_schedule,
            commands_data::get_schedules,
            commands_data::update_schedule,
            commands_data::delete_schedule,
            commands_data::toggle_schedule_completion,
            commands_data::get_schedule_completions,
            commands_data::get_schedule_stats,
            // Dan Koe Protocol
            commands_data::get_dan_koe_entry,
            commands_data::save_dan_koe_entry,
            commands_data::get_dan_koe_history,
            commands_data::get_dan_koe_stats,
            // Health & Habits
            commands_data::log_health,
            commands_data::get_health_today,
            commands_data::create_habit,
            commands_data::check_habit,
            commands_data::get_habits_today,
            commands_data::update_habit,
            commands_data::delete_habit,
            // Dashboard
            commands_data::get_dashboard_data,
            commands_data::get_notifications,
            commands_data::save_proactive_message,
            commands_data::get_proactive_messages,
            commands_data::rate_proactive_message,
            commands_data::mark_proactive_read,
            commands_data::archive_old_proactive,
            // Activity tracking
            commands_data::get_activity_timeline,
            commands_data::get_activity_weekly,
            // Memory browser
            commands_data::get_all_memories,
            commands_data::delete_memory,
            commands_data::update_memory,
            commands_data::memory_cleanup,
            // Media Items (Hobbies collections)
            commands_data::add_media_item,
            commands_data::update_media_item,
            commands_data::delete_media_item,
            commands_data::get_media_items,
            commands_data::hide_media_item,
            commands_data::unhide_media_item,
            commands_data::create_user_list,
            commands_data::get_user_lists,
            commands_data::add_to_list,
            commands_data::remove_from_list,
            commands_data::get_list_items,
            commands_data::get_media_stats,
            // Food
            commands_data::log_food,
            commands_data::get_food_log,
            commands_data::delete_food_entry,
            commands_data::update_food_entry,
            commands_data::get_food_stats,
            commands_data::create_recipe,
            commands_data::get_recipes,
            commands_data::update_recipe,
            commands_data::delete_recipe,
            commands_data::get_ingredient_catalog,
            commands_data::add_ingredient_to_catalog,
            commands_data::get_cuisines,
            commands_data::add_cuisine,
            commands_data::toggle_favorite_recipe,
            commands_data::mark_recipe_cooked,
            commands_data::add_product,
            commands_data::get_products,
            commands_data::update_product,
            commands_data::delete_product,
            commands_data::get_expiring_products,
            commands_data::get_recipe,
            commands_data::plan_meal,
            commands_data::get_meal_plan,
            commands_data::delete_meal_plan,
            // Money
            commands_data::add_transaction,
            commands_data::get_transactions,
            commands_data::delete_transaction,
            commands_data::update_transaction,
            commands_data::get_transaction_stats,
            commands_data::create_budget,
            commands_data::get_budgets,
            commands_data::update_budget,
            commands_data::delete_budget,
            commands_data::create_savings_goal,
            commands_data::get_savings_goals,
            commands_data::update_savings_goal,
            commands_data::delete_savings_goal,
            commands_data::add_subscription,
            commands_data::get_subscriptions,
            commands_data::update_subscription,
            commands_data::delete_subscription,
            commands_data::add_debt,
            commands_data::get_debts,
            commands_data::update_debt,
            commands_data::delete_debt,
            // Mindset
            commands_meta::save_journal_entry,
            commands_meta::get_journal_entries,
            commands_meta::get_journal_entry,
            commands_meta::log_mood,
            commands_meta::get_mood_history,
            commands_meta::create_principle,
            commands_meta::get_principles,
            commands_meta::update_principle,
            commands_meta::delete_principle,
            commands_meta::get_mindset_check,
            // Blocklist
            commands_meta::add_to_blocklist,
            commands_meta::remove_from_blocklist,
            commands_meta::get_blocklist,
            commands_meta::toggle_blocklist_item,
            // Goals & Settings
            commands_meta::create_goal,
            commands_meta::get_goals,
            commands_meta::update_goal,
            commands_meta::delete_goal,
            commands_meta::set_app_setting,
            commands_meta::get_app_setting,
            // Home Items
            commands_meta::add_home_item,
            commands_meta::get_home_items,
            commands_meta::update_home_item,
            commands_meta::delete_home_item,
            commands_meta::toggle_home_item_needed,
            // People / Contacts
            commands_meta::add_contact,
            commands_meta::get_contacts,
            commands_meta::update_contact,
            commands_meta::delete_contact,
            commands_meta::toggle_contact_blocked,
            commands_meta::toggle_contact_favorite,
            // Contact blocks
            commands_meta::add_contact_block,
            commands_meta::get_contact_blocks,
            commands_meta::delete_contact_block,
            commands_meta::toggle_contact_block_active,
            // Page Meta & Custom Properties
            commands_meta::get_page_meta,
            commands_meta::update_page_meta,
            commands_meta::get_property_definitions,
            commands_meta::create_property_definition,
            commands_meta::update_property_definition,
            commands_meta::delete_property_definition,
            commands_meta::get_property_values,
            commands_meta::set_property_value,
            commands_meta::delete_property_value,
            commands_meta::get_view_configs,
            commands_meta::create_view_config,
            commands_meta::update_view_config,
            commands_meta::get_ui_state,
            commands_meta::set_ui_state,
            // Call Mode
            voice::start_call_mode,
            voice::stop_call_mode,
            voice::call_mode_resume_listening,
            voice::call_mode_set_speaking,
            voice::call_mode_check_bargein,
            voice::speak_text_blocking,
            voice::speak_sentence_blocking,
            voice::save_voice_note,
            // Wake Word
            voice::start_wakeword,
            voice::stop_wakeword,
            // Voice Cloning
            voice::save_voice_sample,
            voice::record_voice_sample,
            voice::list_voice_samples,
            voice::delete_voice_sample,
            voice::speak_clone_blocking,
            // Data Flywheel
            commands_data::get_flywheel_status,
            commands_data::get_flywheel_history,
            commands_data::run_flywheel_cycle,
            proactive::rate_proactive,
            // Custom Pages
            notes::create_custom_page,
            notes::get_custom_pages,
            notes::get_custom_page,
            notes::update_custom_page,
            notes::delete_custom_page,
            // Project Records
            notes::get_project_records,
            notes::create_project_record,
            notes::update_project_record,
            notes::delete_project_record,
            // Focus Overlay
            notes::toggle_focus_overlay,
            // Tab Page Blocks
            commands_data::get_tab_blocks,
            commands_data::save_tab_blocks,
            // MCP
            mcp::mcp_call_tool,
            mcp::mcp_list_tools,
            // Vacancy
            vacancy::vacancy_search_now,
            vacancy::vacancy_search_source,
            // Automation API
            commands_meta::auto_eval_callback,
            // Body Records (3D Body Tab)
            commands_data::create_body_record,
            commands_data::get_body_records,
            commands_data::delete_body_record,
            commands_data::get_body_zones_summary,
            // Dashboard Widgets
            dashboard::get_dashboard_widgets,
            dashboard::save_dashboard_widgets,
            dashboard::seed_dashboard_defaults,
            // Timeline
            commands_timeline::create_activity_type,
            commands_timeline::get_activity_types,
            commands_timeline::update_activity_type,
            commands_timeline::delete_activity_type,
            commands_timeline::create_timeline_block,
            commands_timeline::get_timeline_blocks,
            commands_timeline::update_timeline_block,
            commands_timeline::delete_timeline_block,
            timeline_stats::create_timeline_goal,
            timeline_stats::get_timeline_goals,
            timeline_stats::update_timeline_goal,
            timeline_stats::delete_timeline_goal,
            timeline_stats::get_timeline_day_stats,
            timeline_stats::get_timeline_range_stats,
            timeline_afk::sync_afk_blocks,
            timeline_afk::sync_timeline_auto,
            timeline_health::sync_health_to_timeline,
            // Sync
            sync_commands::sync_now,
            sync_commands::get_sync_status,
            sync_commands::set_sync_config,
            // Health Connect / Sleep
            health_connect::get_sleep_sessions,
            health_connect::add_sleep_session,
            health_connect::get_sleep_stats,
            health_connect::import_health_connect_sleep,
            // Health import & analytics
            health_import::import_health_connect_all,
            health_import::get_heart_rate_samples,
            health_import::get_health_summary,
            sleep_analysis::get_sleep_analysis,
        ])
        .setup(move |app| {
            // Android: resolve data dir from Tauri, then init DB
            #[cfg(target_os = "android")]
            {
                let data_dir = app.path().app_data_dir()
                    .expect("Cannot resolve app_data_dir on Android");
                types::set_data_dir(data_dir);
                let hanni_db = init_database();
                app.manage(hanni_db);
            }

            // Auto-updater (desktop only)
            #[cfg(not(target_os = "android"))]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let updater = match updater_with_headers(&handle) {
                        Ok(u) => u,
                        Err(_) => return,
                    };
                    match updater.check().await {
                        Ok(Some(update)) => {
                            let version = update.version.clone();
                            let _ = handle.emit("update-available", &version);
                            if let Ok(()) = update.download_and_install(|_, _| {}, || {}).await {
                                handle.restart();
                            }
                        }
                        _ => {}
                    }
                });
            }

            // Save system prompt for nightly training script
            let prompt_path = hanni_data_dir().join("system_prompt.txt");
            let _ = std::fs::write(&prompt_path, SYSTEM_PROMPT);

            // HTTP API server (Phase 4)
            let api_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                spawn_api_server(api_handle).await;
            });

            // MCP client manager — connect to configured MCP servers in background
            let mcp_arc = app.state::<mcp::McpState>().0.clone();
            tauri::async_runtime::spawn(async move {
                let config_path = hanni_data_dir().join("mcp.json");
                eprintln!("[mcp] Loading config from: {:?}", config_path);
                let mgr = mcp::McpManager::from_config(
                    config_path.to_str().unwrap_or("")
                ).await;
                let tool_count = mgr.tools_as_openai().len();
                *mcp_arc.lock().await = mgr;
                eprintln!("[mcp] Initialization complete — {} tools loaded", tool_count);
            });

            // Backfill: embed existing facts (desktop only — needs voice server)
            #[cfg(not(target_os = "android"))]
            {
                let backfill_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    let client = &backfill_handle.state::<HttpClient>().0;
                    let health = client.get(&format!("{}/health", VOICE_SERVER_URL))
                        .timeout(std::time::Duration::from_secs(3))
                        .send().await;
                    if health.is_err() {
                        eprintln!("[backfill] Voice server not available, skipping");
                        return;
                    }
                    let facts: Vec<(i64, String)> = {
                        let db = backfill_handle.state::<HanniDb>();
                        let conn = db.conn();
                        let mut result = Vec::new();
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT f.id, '[' || f.category || '] ' || f.key || ': ' || f.value
                             FROM facts f
                             WHERE f.id NOT IN (SELECT fact_id FROM vec_facts)
                             ORDER BY f.id"
                        ) {
                            if let Ok(rows) = stmt.query_map([], |row| {
                                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                            }) {
                                for row in rows.flatten() { result.push(row); }
                            }
                        }
                        result
                    };
                    if facts.is_empty() { return; }
                    eprintln!("[backfill] Embedding {} facts...", facts.len());
                    for chunk in facts.chunks(32) {
                        let texts: Vec<String> = chunk.iter().map(|(_, t)| t.clone()).collect();
                        match embed_texts(client, &texts).await {
                            Ok(embeddings) => {
                                let db = backfill_handle.state::<HanniDb>();
                                let conn = db.conn();
                                for (i, (fact_id, _)) in chunk.iter().enumerate() {
                                    if let Some(emb) = embeddings.get(i) {
                                        store_fact_embedding(&conn, *fact_id, emb);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[backfill] Embed failed: {}, retry on next startup", e);
                                return;
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    eprintln!("[backfill] Embedding backfill complete");
                });
            }

            // Global shortcut: Cmd+Shift+H to toggle Call Mode (desktop only)
            #[cfg(not(target_os = "android"))]
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let shortcut_handle = app.handle().clone();
                let _ = app.global_shortcut().on_shortcut("CommandOrControl+Shift+H", move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let _ = shortcut_handle.emit("global-toggle-call", ());
                    }
                });
            }

            // Startup cleanup: remove stale focus blocker entries from /etc/hosts
            // (in case app crashed or was force-killed during focus mode)
            #[cfg(not(target_os = "android"))]
            if std::fs::read_to_string("/etc/hosts")
                .map(|c| c.contains("# === HANNI FOCUS BLOCKER ==="))
                .unwrap_or(false)
            {
                let _ = run_osascript("do shell script \"sed -i '' '/# === HANNI FOCUS BLOCKER ===/,/# === END HANNI FOCUS BLOCKER ===/d' /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges");
            }

            // Focus mode monitor loop (desktop only — uses osascript)
            #[cfg(not(target_os = "android"))]
            {
            let focus_handle = app.handle().clone();
            let focus_flag = focus_monitor_flag.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if !focus_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    let focus = focus_handle.state::<FocusManager>();
                    let (active, end_time, apps) = {
                        let state = focus.0.lock().unwrap_or_else(|e| e.into_inner());
                        (state.active, state.end_time, state.blocked_apps.clone())
                    };

                    if !active {
                        continue;
                    }

                    // Check if focus timer expired
                    if let Some(end) = end_time {
                        if chrono::Local::now() >= end {
                            // Auto-stop focus mode
                            let script = "do shell script \"sed -i '' '/# === HANNI FOCUS BLOCKER ===/,/# === END HANNI FOCUS BLOCKER ===/d' /etc/hosts && dscacheutil -flushcache && killall -HUP mDNSResponder\" with administrator privileges";
                            let _ = run_osascript(script);
                            {
                                let mut state = focus.0.lock().unwrap_or_else(|e| e.into_inner());
                                state.active = false;
                                state.end_time = None;
                                state.blocked_apps.clear();
                                state.blocked_sites.clear();
                                state.monitor_running.store(false, Ordering::Relaxed);
                            }
                            let _ = focus_handle.emit("focus-ended", ());
                            continue;
                        }
                    }

                    // Kill blocked apps if they relaunch
                    for app_name in &apps {
                        let safe = app_name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '.').collect::<String>();
                        if safe.is_empty() { continue; }
                        let _ = run_osascript(&format!(
                            "tell application \"System Events\"\nif (name of processes) contains \"{}\" then\ntell application \"{}\" to quit\nend if\nend tell",
                            safe, safe
                        ));
                    }
                }
            });

            // Activity snapshot collector — OS data every 30 sec
            let snapshot_handle = app.handle().clone();
            let snapshot_proactive_ref = proactive_state.clone();
            tauri::async_runtime::spawn(async move {
                // Initial delay
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                let mut trigger_counter: u32 = 0; // check triggers every 20th iteration (10 min)
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                    // Collect OS data in blocking thread
                    let (app_name, browser, music, window_title, idle_secs, screen_locked) = tokio::task::spawn_blocking(|| {
                        (
                            get_frontmost_app(),
                            get_browser_url(),
                            get_now_playing_sync(),
                            proactive::get_window_title(),
                            macos::get_macos_idle_seconds(),
                            macos::is_screen_locked(),
                        )
                    }).await.unwrap_or_default();

                    // Skip recording if idle > 30 min (long AFK, saves DB space)
                    if idle_secs > 1800.0 {
                        trigger_counter += 1;
                        continue;
                    }

                    let now = chrono::Local::now();
                    let hour = now.hour() as i64;
                    let weekday = now.weekday().num_days_from_monday() as i64;

                    // AFK detection: screen locked OR idle >= 2 min
                    let is_afk = screen_locked || idle_secs >= 120.0;

                    // Classify activity category (only meaningful when not AFK)
                    let category = if is_afk {
                        "afk"
                    } else {
                        proactive::classify_activity(&app_name, &browser, &window_title)
                    };

                    // Compute productive vs distraction (0.5 min per 30-sec snapshot)
                    // AFK snapshots get 0/0 — they don't count as productive or distraction
                    let (prod_min, dist_min) = if is_afk {
                        (0.0_f64, 0.0_f64)
                    } else {
                        match category {
                            "coding" | "writing" | "learning" => (0.5_f64, 0.0_f64),
                            "social" | "media" => (0.0_f64, 0.5_f64),
                            _ => (0.25_f64, 0.0_f64),
                        }
                    };

                    // Write to DB
                    let db = snapshot_handle.state::<HanniDb>();
                    {
                        let conn = db.conn();
                        let _ = conn.execute(
                            "INSERT INTO activity_snapshots (captured_at, hour, weekday, frontmost_app, browser_url, music_playing, productive_min, distraction_min, idle_secs, window_title, category, screen_locked) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                            rusqlite::params![
                                now.to_rfc3339(),
                                hour,
                                weekday,
                                &app_name,
                                &browser,
                                &music,
                                prod_min,
                                dist_min,
                                idle_secs,
                                &window_title,
                                category,
                                screen_locked as i32,
                            ],
                        );

                        // Auto-cleanup: remove snapshots older than 30 days (run once per hour)
                        if trigger_counter % 120 == 0 {
                            let _ = conn.execute(
                                "DELETE FROM activity_snapshots WHERE captured_at < datetime('now', '-30 days')",
                                [],
                            );
                        }
                    }

                    trigger_counter += 1;

                    // Check triggers every ~10 min (20 iterations × 30 sec)
                    if trigger_counter % 20 == 0 {
                        let mut triggers: Vec<String> = Vec::new();

                        // Trigger: distraction >30 min
                        {
                            let db = snapshot_handle.state::<HanniDb>();
                            let conn = db.conn();
                            let dist_total: f64 = conn.query_row(
                                "SELECT COALESCE(SUM(distraction_min), 0) FROM activity_snapshots WHERE captured_at > datetime('now', '-30 minutes')",
                                [], |row| row.get(0),
                            ).unwrap_or(0.0);
                            if dist_total >= 30.0 {
                                triggers.push(format!("Дистракция: пользователь отвлекается уже {:.0} мин", dist_total));
                            }
                        }

                        // Trigger: upcoming event within 15 min
                        if let Ok(upcoming) = get_upcoming_events_soon() {
                            if !upcoming.is_empty() {
                                triggers.push(format!("Скоро событие: {}", upcoming.lines().next().unwrap_or("")));
                            }
                        }

                        if !triggers.is_empty() {
                            let mut state = snapshot_proactive_ref.lock().await;
                            let now_inst = std::time::Instant::now();
                            state.pending_triggers.retain(|(_, created)| created.elapsed().as_secs() < 600);
                            for t in triggers {
                                state.pending_triggers.push((t, now_inst));
                            }
                        }
                    }

                    // Update OpenClaw HEARTBEAT.md every ~10 min (not every 30 sec)
                    if trigger_counter % 20 != 0 { continue; }
                    let heartbeat_path = dirs::home_dir()
                        .map(|h| h.join("clawd/HEARTBEAT.md"));
                    if let Some(path) = heartbeat_path {
                        let idle_secs = macos::get_macos_idle_seconds();
                        let mut lines = vec![
                            "# Текущий контекст".to_string(),
                            format!("Время: {}", now.format("%H:%M %d.%m.%Y")),
                        ];
                        if idle_secs > 300.0 {
                            lines.push(format!("Статус: неактивен ({:.0} мин)", idle_secs / 60.0));
                        } else {
                            lines.push("Статус: активен".to_string());
                        }
                        if !app_name.is_empty() && app_name != "Hanni" {
                            lines.push(format!("Приложение: {}", app_name));
                        }
                        if !browser.is_empty() {
                            lines.push(format!("Браузер: {}", browser));
                        }
                        if !music.is_empty() {
                            lines.push(format!("Музыка: {}", music));
                        }
                        lines.push(String::new());
                        lines.push("# Задачи".to_string());
                        lines.push("Нет активных задач. Если нечего делать — HEARTBEAT_OK.".to_string());
                        let _ = std::fs::write(&path, lines.join("\n"));
                    }
                }
            });

            // Background learning loop — analyze activity patterns every 30 min
            let learning_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                // Initial delay — let DB populate with some snapshots first
                tokio::time::sleep(std::time::Duration::from_secs(1800)).await;

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1800)).await; // 30 min

                    // Acquire LLM semaphore — skip if busy, hold permit through LLM call
                    let llm_sem = learning_handle.state::<LlmBusy>();
                    let _llm_permit = match llm_sem.0.try_acquire() {
                        Ok(p) => p,
                        Err(_) => continue,
                    };

                    // Read last 18 snapshots (3 hours) and existing observation facts
                    let (snapshots_text, existing_obs) = {
                        let db = learning_handle.state::<HanniDb>();
                        let conn = db.conn();
                        let mut snap_lines = Vec::new();
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT captured_at, frontmost_app, browser_url, music_playing, productive_min, distraction_min FROM activity_snapshots ORDER BY id DESC LIMIT 18"
                        ) {
                            if let Ok(rows) = stmt.query_map([], |row| {
                                Ok(format!(
                                    "{}: app={}, browser={}, music={}, prod={:.0}min, dist={:.0}min",
                                    row.get::<_, String>(0).unwrap_or_default(),
                                    row.get::<_, String>(1).unwrap_or_default(),
                                    row.get::<_, String>(2).unwrap_or_default(),
                                    row.get::<_, String>(3).unwrap_or_default(),
                                    row.get::<_, f64>(4).unwrap_or(0.0),
                                    row.get::<_, f64>(5).unwrap_or(0.0),
                                ))
                            }) {
                                for row in rows.flatten() { snap_lines.push(row); }
                            }
                        }
                        let mut obs = Vec::new();
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT value FROM facts WHERE source = 'observation' ORDER BY updated_at DESC LIMIT 20"
                        ) {
                            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                                for row in rows.flatten() { obs.push(row); }
                            }
                        }
                        (snap_lines.join("\n"), obs.join("\n"))
                    };

                    if snapshots_text.is_empty() { continue; }

                    // Ensure MLX is running before calling it
                    tokio::task::spawn_blocking(|| crate::mlx_manager::ensure_mlx()).await.ok();

                    let prompt = format!(
                        "Проанализируй активность пользователя за последние 3 часа и найди паттерны.\n\n\
                        Снимки активности (от новых к старым):\n{}\n\n\
                        Уже известные наблюдения (не дублируй):\n{}\n\n\
                        Найди: рутины, привычки, продуктивность, предпочтения, аномалии.\n\
                        Формат: одно наблюдение на строку, начиная с '- '. Только новые, не дублируй известные.\n\
                        Если ничего нового — ответь [NONE].\n/no_think",
                        snapshots_text, if existing_obs.is_empty() { "нет" } else { &existing_obs }
                    );

                    let request = ChatRequest {
                        model: MODEL.into(),
                        messages: vec![
                            ChatMessage::text("system", "Ты — аналитик поведения. Твоя задача — находить паттерны в активности пользователя. Будь краток и конкретен. Отвечай на русском."),
                            ChatMessage::text("user", &prompt),
                        ],
                        max_tokens: 400,
                        stream: false,
                        temperature: 0.3,
                        repetition_penalty: None,
                        chat_template_kwargs: ChatTemplateKwargs { enable_thinking: false },
                        tools: None,
                    };

                    let resp = client.post(MLX_URL).json(&request).send().await;
                    if let Ok(resp) = resp {
                        if !resp.status().is_success() { continue; }
                        if let Ok(parsed) = resp.json::<NonStreamResponse>().await {
                            let raw = parsed.choices.first().map(|c| c.message.content.clone()).unwrap_or_default();
                            // Strip think tags
                            let re = regex::Regex::new(r"(?s)<think>.*?</think>").unwrap();
                            let text = re.replace_all(&raw, "").trim().to_string();

                            if !text.contains("[NONE]") && !text.is_empty() {
                                // Parse lines starting with '- '
                                let observations: Vec<&str> = text.lines()
                                    .filter(|l| l.trim().starts_with("- "))
                                    .map(|l| l.trim().trim_start_matches("- "))
                                    .collect();

                                if !observations.is_empty() {
                                    let db = learning_handle.state::<HanniDb>();
                                    let conn = db.conn();
                                    let now = chrono::Local::now().to_rfc3339();
                                    for obs in observations.iter().take(3) {
                                        let key = format!("obs_{}", &now[..16]);
                                        let _ = conn.execute(
                                            "INSERT INTO facts (category, key, value, source, created_at, updated_at) VALUES ('observation', ?1, ?2, 'observation', ?3, ?3)",
                                            rusqlite::params![key, obs, now],
                                        );
                                    }
                                    // Keep max 10 observations — delete oldest if over limit
                                    let _ = conn.execute(
                                        "DELETE FROM facts WHERE category='observation' AND id NOT IN (SELECT id FROM facts WHERE category='observation' ORDER BY updated_at DESC LIMIT 10)",
                                        [],
                                    );
                                    eprintln!("[learning] saved {} observations", observations.len().min(3));
                                }
                            }
                        }
                    }
                }
            });

            // S3: Reminder check loop (every 30s)
            let reminder_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    let now = chrono::Local::now().to_rfc3339();
                    let due: Vec<(i64, String)> = {
                        let db = reminder_handle.state::<HanniDb>();
                        let conn = db.conn();
                        let mut stmt = match conn.prepare(
                            "SELECT id, title FROM reminders WHERE fired=0 AND remind_at <= ?1"
                        ) { Ok(s) => s, Err(_) => continue };
                        let rows: Vec<(i64, String)> = stmt.query_map(rusqlite::params![now], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                        }).ok().into_iter().flatten().filter_map(|r| r.ok()).collect();
                        // Mark as fired
                        for (id, _) in &rows {
                            let _ = conn.execute("UPDATE reminders SET fired=1 WHERE id=?1", rusqlite::params![id]);
                        }
                        rows
                    };
                    for (_, title) in due {
                        let _ = reminder_handle.emit("reminder-fired", &title);
                        let _ = run_osascript(&format!(
                            "display notification \"{}\" with title \"Напоминание\"",
                            macos::osa_escape(&title)
                        ));
                    }
                    // Check note reminders
                    let note_due: Vec<(i64, String)> = {
                        let db = reminder_handle.state::<HanniDb>();
                        let conn = db.conn();
                        let mut stmt = match conn.prepare(
                            "SELECT id, title FROM notes WHERE reminder_at IS NOT NULL AND reminder_at <= ?1"
                        ) { Ok(s) => s, Err(_) => continue };
                        let rows: Vec<(i64, String)> = stmt.query_map(rusqlite::params![now], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                        }).ok().into_iter().flatten().filter_map(|r| r.ok()).collect();
                        for (id, _) in &rows {
                            let _ = conn.execute("UPDATE notes SET reminder_at=NULL WHERE id=?1", rusqlite::params![id]);
                        }
                        rows
                    };
                    for (id, title) in note_due {
                        let payload = serde_json::json!({"id": id, "title": title});
                        let _ = reminder_handle.emit("note-reminder-fired", &payload);
                        let _ = run_osascript(&format!(
                            "display notification \"{}\" with title \"Заметка\"",
                            macos::osa_escape(&title)
                        ));
                    }
                }
            });

            // Proactive messaging background loop
            // OpenClaw cron → Hanni chat bridge: poll openclaw_proactive table
            let openclaw_poll_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait for app startup
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                // Ensure table exists
                {
                    let db = openclaw_poll_handle.state::<HanniDb>();
                    let conn = db.conn();
                    let _ = conn.execute_batch(
                        "CREATE TABLE IF NOT EXISTS openclaw_proactive (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            message TEXT NOT NULL,
                            style TEXT DEFAULT 'observation',
                            created_at TEXT NOT NULL,
                            delivered INTEGER DEFAULT 0
                        )"
                    );
                }
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    let pending: Vec<(i64, String, String)> = {
                        let db = openclaw_poll_handle.state::<HanniDb>();
                        let conn = db.conn();
                        let mut stmt = match conn.prepare(
                            "SELECT id, message, style FROM openclaw_proactive WHERE delivered = 0 ORDER BY id LIMIT 5"
                        ) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        stmt.query_map([], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                        }).ok().map(|rows| rows.flatten().collect()).unwrap_or_default()
                    };
                    for (id, message, style) in &pending {
                        // Mark delivered
                        {
                            let db = openclaw_poll_handle.state::<HanniDb>();
                            let conn = db.conn();
                            let _ = conn.execute("UPDATE openclaw_proactive SET delivered = 1 WHERE id = ?", [id]);
                            // Also save to proactive_history for continuity
                            let _ = conn.execute(
                                "INSERT INTO proactive_history (sent_at, message, style) VALUES (?1, ?2, ?3)",
                                rusqlite::params![chrono::Local::now().to_rfc3339(), message, style],
                            );
                        }
                        // Emit to frontend — same event as native proactive
                        let _ = openclaw_poll_handle.emit("proactive-message", serde_json::json!({
                            "text": message,
                            "id": id,
                        }));
                    }
                }
            });

            let proactive_handle = app.handle().clone();
            let proactive_state_ref = proactive_state.clone();
            tauri::async_runtime::spawn(async move {
                proactive_loop(proactive_handle, proactive_state_ref).await;
            });

            // Vacancy search background loop
            let vacancy_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                vacancy::vacancy_search_loop(vacancy_handle).await;
            });
            } // end #[cfg(not(target_os = "android"))] block

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Hanni")
        .run(move |_app, event| {
            #[cfg(not(target_os = "android"))]
            if let tauri::RunEvent::Exit = event {
                // Kill MLX server process on app exit
                mlx_manager::stop();
                // Kill OpenClaw gateway if we started it as subprocess
                {
                    let mut child = openclaw_cleanup.0.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(ref mut proc) = *child {
                        let _ = proc.kill();
                    }
                }
            }
        });
}
