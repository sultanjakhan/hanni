//! Native macOS menu-bar timer for the currently active timeline task.

use crate::types::HanniDb;
use chrono::{Local, NaiveDateTime};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, Wry,
};

const TRAY_ID: &str = "active-task-timer";
const STATUS_ID: &str = "active-task.status";
const PAUSE_ID: &str = "active-task.pause";
const EXTEND_ID: &str = "active-task.extend";
const FINISH_ID: &str = "active-task.finish";
const CANCEL_ID: &str = "active-task.cancel";
const OPEN_ID: &str = "active-task.open";

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveTask {
    id: i64,
    date: String,
    start_time: String,
    title: String,
    source_type: Option<String>,
    source_id: Option<String>,
    planned_minutes: Option<i64>,
}

struct MenuBarTimerState {
    tray: TrayIcon<Wry>,
    status_item: MenuItem<Wry>,
    extend_item: MenuItem<Wry>,
    active: Mutex<Option<ActiveTask>>,
}

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    let status_item =
        MenuItem::with_id(app, STATUS_ID, "Нет активной задачи", false, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, PAUSE_ID, "Пауза", true, None::<&str>)?;
    let extend_item = MenuItem::with_id(app, EXTEND_ID, "Продлить на 15 мин", false, None::<&str>)?;
    let finish_item = MenuItem::with_id(app, FINISH_ID, "Завершить", true, None::<&str>)?;
    let cancel_item = MenuItem::with_id(app, CANCEL_ID, "Отменить таймер", true, None::<&str>)?;
    let open_item = MenuItem::with_id(app, OPEN_ID, "Открыть Hanni", true, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &separator_one,
            &pause_item,
            &extend_item,
            &finish_item,
            &cancel_item,
            &separator_two,
            &open_item,
        ],
    )?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .title("Hanni")
        .tooltip("Активный таймер Hanni")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(handle_menu_event)
        .build(app)?;
    tray.set_visible(false)?;

    app.manage(MenuBarTimerState {
        tray,
        status_item,
        extend_item,
        active: Mutex::new(None),
    });

    refresh(app.handle());
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut ticks = 0u8;
        loop {
            interval.tick().await;
            if ticks == 0 {
                refresh(&app_handle);
            } else {
                refresh_display(&app_handle);
            }
            ticks = (ticks + 1) % 5;
        }
    });

    Ok(())
}

fn handle_menu_event(app: &AppHandle<Wry>, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    if !id.starts_with("active-task.") {
        return;
    }

    if id == OPEN_ID {
        show_main_window(app);
        return;
    }

    let Some(task) = current_task(app) else {
        refresh(app);
        return;
    };

    let db = app.state::<HanniDb>();
    let result = match id {
        PAUSE_ID => crate::commands_timeline_today::pause_task_block(task.id, db),
        FINISH_ID => crate::commands_timeline_today::complete_task_block(task.id, db),
        CANCEL_ID => crate::commands_timeline::delete_timeline_block(task.id, db),
        EXTEND_ID => extend_event(&task, &db),
        _ => return,
    };

    if let Err(error) = result {
        eprintln!("[menu-bar-timer] {id}: {error}");
    }
    refresh(app);
    notify_webview(app);
}

fn extend_event(task: &ActiveTask, db: &HanniDb) -> Result<(), String> {
    if task.source_type.as_deref() != Some("event") {
        return Ok(());
    }
    let event_id = task
        .source_id
        .as_deref()
        .ok_or_else(|| "У активной задачи нет event id".to_string())?;
    db.conn()
        .execute(
            "UPDATE events SET duration_minutes=COALESCE(duration_minutes, 0)+15 WHERE id=?1",
            rusqlite::params![event_id],
        )
        .map_err(|error| format!("DB error: {error}"))?;
    Ok(())
}

fn refresh(app: &AppHandle<Wry>) {
    let active = query_active_task(&app.state::<HanniDb>());
    let state = app.state::<MenuBarTimerState>();

    render_timer(&state, active.as_ref(), Local::now().naive_local());
    *state
        .active
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = active;
}

fn refresh_display(app: &AppHandle<Wry>) {
    let state = app.state::<MenuBarTimerState>();
    let active = state
        .active
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    render_timer(&state, active.as_ref(), Local::now().naive_local());
}

fn render_timer(state: &MenuBarTimerState, active: Option<&ActiveTask>, now: NaiveDateTime) {
    match &active {
        Some(task) => {
            let elapsed = elapsed_seconds(task, now);
            let (clock, clock_hint) = countdown_display(elapsed, task.planned_minutes);
            let timer_title = format!("{} · {}", compact_title(&task.title, 24), clock);
            let status = match task.planned_minutes {
                Some(planned) => format!("{} — {} · план {} мин", task.title, clock_hint, planned),
                None => format!("{} — с {}", task.title, task.start_time),
            };
            let _ = state.tray.set_title(Some(timer_title));
            let _ = state.status_item.set_text(status);
            let _ = state
                .extend_item
                .set_enabled(task.source_type.as_deref() == Some("event"));
            let _ = state.tray.set_visible(true);
        }
        None => {
            let _ = state.tray.set_visible(false);
        }
    }
}

fn query_active_task(db: &HanniDb) -> Option<ActiveTask> {
    let conn = db.read();
    conn.query_row(
        "SELECT b.id, b.date, b.start_time, b.source_type, CAST(b.source_id AS TEXT),
                COALESCE(
                    CASE b.source_type
                        WHEN 'event' THEN (SELECT e.title FROM events e WHERE CAST(e.id AS TEXT)=CAST(b.source_id AS TEXT))
                        WHEN 'schedule' THEN (SELECT s.title FROM schedules s WHERE CAST(s.id AS TEXT)=CAST(b.source_id AS TEXT))
                        WHEN 'note' THEN (SELECT n.title FROM notes n WHERE CAST(n.id AS TEXT)=CAST(b.source_id AS TEXT))
                    END,
                    NULLIF(b.notes, ''), t.name, 'Таск'
                ),
                CASE b.source_type
                    WHEN 'event' THEN (SELECT e.duration_minutes FROM events e WHERE CAST(e.id AS TEXT)=CAST(b.source_id AS TEXT))
                    WHEN 'schedule' THEN (SELECT s.target_minutes FROM schedules s WHERE CAST(s.id AS TEXT)=CAST(b.source_id AS TEXT))
                END
         FROM timeline_blocks b
         JOIN timeline_activity_types t ON t.id=b.type_id
         WHERE b.is_active=1
         ORDER BY b.id DESC LIMIT 1",
        [],
        |row| {
            Ok(ActiveTask {
                id: row.get(0)?,
                date: row.get(1)?,
                start_time: row.get(2)?,
                source_type: row.get(3)?,
                source_id: row.get(4)?,
                title: row.get(5)?,
                planned_minutes: row.get(6)?,
            })
        },
    )
    .ok()
}

fn current_task(app: &AppHandle<Wry>) -> Option<ActiveTask> {
    app.state::<MenuBarTimerState>()
        .active
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn elapsed_seconds(task: &ActiveTask, now: NaiveDateTime) -> i64 {
    let start = NaiveDateTime::parse_from_str(
        &format!("{} {}", task.date, task.start_time),
        "%Y-%m-%d %H:%M",
    );
    start
        .map(|value| (now - value).num_seconds().max(0))
        .unwrap_or(0)
}

fn countdown_display(elapsed_seconds: i64, planned_minutes: Option<i64>) -> (String, String) {
    if let Some(planned) = planned_minutes.filter(|value| *value > 0) {
        let remaining = planned * 60 - elapsed_seconds;
        if remaining >= 0 {
            let clock = format_clock(remaining);
            return (clock.clone(), format!("осталось {clock}"));
        }
        let clock = format!("+{}", format_clock(-remaining));
        return (clock.clone(), format!("сверх плана {clock}"));
    }
    let clock = format!("↑{}", format_clock(elapsed_seconds));
    (clock.clone(), format!("прошло {clock}"))
}

fn format_clock(total_seconds: i64) -> String {
    let seconds = total_seconds.max(0);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60
    )
}

fn compact_title(title: &str, max_chars: usize) -> String {
    let mut chars = title.chars();
    let compact: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}

fn show_main_window(app: &AppHandle<Wry>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn notify_webview(app: &AppHandle<Wry>) {
    let _ = app.emit("task-state-changed", ());
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval("window.dispatchEvent(new Event('task-state-changed'))");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_timer_clock() {
        assert_eq!(format_clock(0), "00:00:00");
        assert_eq!(format_clock(39), "00:00:39");
        assert_eq!(format_clock(9_274), "02:34:34");
    }

    #[test]
    fn counts_down_and_marks_overtime() {
        assert_eq!(countdown_display(75 * 60, Some(90)).0, "00:15:00");
        assert_eq!(countdown_display(95 * 60, Some(90)).0, "+00:05:00");
        assert_eq!(countdown_display(75 * 60, None).0, "↑01:15:00");
    }

    #[test]
    fn truncates_long_unicode_titles_by_character() {
        assert_eq!(compact_title("Job", 24), "Job");
        assert_eq!(
            compact_title("Очень длинное название задачи", 10),
            "Очень длин…"
        );
    }
}
