// commands_system.rs — Integrations info, model info, health check
use crate::types::*;
use crate::commands_data::load_tracker_data;
use tauri::{AppHandle, Manager};

// ── Integrations, Model Info, Health Check ──
// ── Integrations info ──

#[tauri::command]
pub async fn get_integrations() -> Result<IntegrationsInfo, String> {
    // ── Access ──
    let tracker_path = data_file_path();
    let tracker_exists = tracker_path.exists();
    let access = vec![
        IntegrationItem {
            name: "Life Tracker".into(),
            status: if tracker_exists { "active" } else { "inactive" }.into(),
            detail: if tracker_exists {
                "~/Library/Application Support/Hanni/life-tracker-data.json".into()
            } else {
                "Файл не найден".into()
            },
        },
        IntegrationItem {
            name: "File System".into(),
            status: "active".into(),
            detail: "$HOME/** — чтение файлов".into(),
        },
        IntegrationItem {
            name: "Shell".into(),
            status: "active".into(),
            detail: "Выполнение команд".into(),
        },
    ];

    // ── Tracking ──
    let tracking = if tracker_exists {
        let data = load_tracker_data().unwrap_or(TrackerData {
            purchases: vec![], time_entries: vec![], goals: vec![], notes: vec![],
            settings: serde_json::Value::Null,
        });
        vec![
            IntegrationItem {
                name: "Расходы".into(),
                status: "active".into(),
                detail: format!("{} записей", data.purchases.len()),
            },
            IntegrationItem {
                name: "Время".into(),
                status: "active".into(),
                detail: format!("{} записей", data.time_entries.len()),
            },
            IntegrationItem {
                name: "Цели".into(),
                status: "active".into(),
                detail: format!("{} целей", data.goals.len()),
            },
            IntegrationItem {
                name: "Заметки".into(),
                status: "active".into(),
                detail: format!("{} заметок", data.notes.len()),
            },
        ]
    } else {
        vec![IntegrationItem {
            name: "Life Tracker".into(),
            status: "inactive".into(),
            detail: "Не подключен".into(),
        }]
    };

    // ── Blocker config ──
    let blocker_config_path = hanni_data_dir().join("blocker_config.json");

    let default_apps = vec!["Telegram", "Discord", "Slack", "Safari"];
    let default_sites = vec![
        "youtube.com", "twitter.com", "x.com", "instagram.com",
        "facebook.com", "tiktok.com", "reddit.com", "vk.com", "netflix.com",
    ];

    let (apps, sites) = if blocker_config_path.exists() {
        let content = std::fs::read_to_string(&blocker_config_path).unwrap_or_default();
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&content) {
            let apps: Vec<String> = cfg["apps"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_apps.iter().map(|s| s.to_string()).collect());
            let sites: Vec<String> = cfg["sites"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| default_sites.iter().map(|s| s.to_string()).collect());
            (apps, sites)
        } else {
            (default_apps.iter().map(|s| s.to_string()).collect(),
             default_sites.iter().map(|s| s.to_string()).collect())
        }
    } else {
        (default_apps.iter().map(|s| s.to_string()).collect(),
         default_sites.iter().map(|s| s.to_string()).collect())
    };

    // Check if blocking is active via /etc/hosts
    let blocker_active = std::fs::read_to_string("/etc/hosts")
        .map(|c| c.contains("# === HANNI FOCUS BLOCKER ==="))
        .unwrap_or(false);

    let blocked_apps = apps.iter().map(|a| IntegrationItem {
        name: a.clone(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: format!("/Applications/{}.app", a),
    }).collect();

    // Deduplicate sites (remove www. variants for display)
    let unique_sites: Vec<&String> = sites.iter()
        .filter(|s| !s.starts_with("www."))
        .collect();

    let blocked_sites = unique_sites.iter().map(|s| IntegrationItem {
        name: s.to_string(),
        status: if blocker_active { "blocked" } else { "inactive" }.into(),
        detail: if blocker_active { "Заблокирован" } else { "Не заблокирован" }.into(),
    }).collect();

    // ── macOS integrations ──
    let macos = vec![
        IntegrationItem {
            name: "Screen Time".into(),
            status: "ready".into(),
            detail: "knowledgeC.db · по запросу".into(),
        },
        IntegrationItem {
            name: "Календарь".into(),
            status: "ready".into(),
            detail: "Calendar.app · по запросу".into(),
        },
        IntegrationItem {
            name: "Музыка".into(),
            status: "ready".into(),
            detail: "Music / Spotify · по запросу".into(),
        },
        IntegrationItem {
            name: "Браузер".into(),
            status: "ready".into(),
            detail: "Safari / Chrome / Arc · по запросу".into(),
        },
    ];

    Ok(IntegrationsInfo {
        access,
        tracking,
        blocked_apps,
        blocked_sites,
        blocker_active,
        macos,
    })
}

// ── Model info ──

#[tauri::command]
pub async fn get_model_info() -> Result<ModelInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    let online = client
        .get(llm_models_url())
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    Ok(ModelInfo {
        model_name: llm_model(),
        server_url: llm_chat_url(),
        server_online: online,
    })
}

// ── Health Check (C4) ──

#[tauri::command]
pub async fn health_check(app: AppHandle) -> Result<HealthStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    // MLX server check
    let mlx_online = client
        .get(llm_models_url())
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    // Voice server check
    let voice_server_online = client
        .get(format!("{}/health", VOICE_SERVER_URL))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    // DB checks
    let (db_ok, db_tables, db_facts, db_conversations, db_size_mb) = {
        let db = app.state::<HanniDb>();
        let conn = db.conn();

        let tables: usize = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [], |row| row.get(0),
        ).unwrap_or(0);

        let facts: usize = conn.query_row(
            "SELECT count(*) FROM facts", [], |row| row.get(0),
        ).unwrap_or(0);

        let convs: usize = conn.query_row(
            "SELECT count(*) FROM conversations", [], |row| row.get(0),
        ).unwrap_or(0);

        // DB file size
        let size: f64 = conn.query_row(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            [], |row| row.get::<_, i64>(0),
        ).map(|bytes| bytes as f64 / 1_048_576.0).unwrap_or(0.0);

        let integrity: String = conn.query_row(
            "PRAGMA integrity_check", [], |row| row.get(0),
        ).unwrap_or_else(|_| "error".into());

        (integrity == "ok", tables, facts, convs, size)
    };

    Ok(HealthStatus {
        mlx_online,
        mlx_model: llm_model(),
        voice_server_online,
        db_ok,
        db_tables,
        db_facts,
        db_conversations,
        db_size_mb,
    })
}
