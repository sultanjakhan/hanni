// macos.rs — macOS actions, OS context helpers, reminders, web search
use crate::proactive::truncate_utf8;
use crate::types::*;
use std::io::Write;
use std::sync::atomic::Ordering;
use tauri::Manager;

// ── Idle / screen lock detection (for auto-quiet) ──

/// Returns seconds since last user input (mouse/keyboard) via IOKit HIDIdleTime
pub fn get_macos_idle_seconds() -> f64 {
    let output = match std::process::Command::new("ioreg")
        .args(["-c", "IOHIDSystem", "-d", "4"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return 0.0,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    // Parse HIDIdleTime = <nanoseconds>
    for line in text.lines() {
        if line.contains("HIDIdleTime") && !line.contains("HIDIdleTimeDelta") {
            if let Some(pos) = line.rfind('=') {
                let val = line[pos + 1..].trim().trim_matches('"');
                if let Ok(ns) = val.parse::<u64>() {
                    return ns as f64 / 1_000_000_000.0;
                }
            }
        }
    }
    0.0
}

/// Checks if screen is locked via CGSession dictionary
pub fn is_screen_locked() -> bool {
    // Method 1: Check CGSessionScreenIsLocked via ioreg
    if let Ok(output) = std::process::Command::new("ioreg")
        .args(["-n", "Root", "-d", "1"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("CGSSessionScreenIsLocked") {
            return true;
        }
    }
    // Method 2: Check if loginwindow is active (works when GUI available)
    match run_osascript(
        r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
    ) {
        Ok(s) => s.trim() == "loginwindow",
        Err(_) => {
            // osascript fails when screen is locked — treat as locked
            true
        }
    }
}

pub fn persist_calendar_result(key: &str, value: &str) {
    if let Ok(conn) = rusqlite::Connection::open(hanni_db_path()) {
        let _ = conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2",
            rusqlite::params![key, value],
        );
    }
}

pub fn check_calendar_access() -> bool {
    if APPLE_CALENDAR_DISABLED.load(Ordering::Relaxed) {
        return false;
    }
    if CALENDAR_ACCESS_DENIED.load(Ordering::Relaxed) {
        return false;
    }
    // Don't launch Calendar.app — only proceed if it's already running
    let running =
        run_osascript(r#"if application "Calendar" is running then return "YES" else return "NO""#);
    if running.as_deref() != Ok("YES") {
        return false;
    }
    if CALENDAR_ACCESS_CHECKED.load(Ordering::Relaxed) {
        return true;
    }
    let result = run_osascript(r#"tell application "Calendar" to count of calendars"#);
    match result {
        Ok(_) => {
            CALENDAR_ACCESS_CHECKED.store(true, Ordering::Relaxed);
            persist_calendar_result("calendar_access_ok", "true");
            true
        }
        Err(_) => {
            CALENDAR_ACCESS_DENIED.store(true, Ordering::Relaxed);
            persist_calendar_result("calendar_access_denied", "true");
            false
        }
    }
}

/// Escape a string for safe interpolation into AppleScript double-quoted strings.
/// Handles backslashes, double quotes, and newlines (which can break out of strings).
pub fn osa_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', " ")
}

pub fn run_osascript(script: &str) -> Result<String, String> {
    let mut child = std::process::Command::new("osascript")
        .args(["-e", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("osascript error: {}", e))?;

    // 10-second timeout — prevents hanging on permission dialogs
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_string(&mut stderr);
                }
                return if status.success() {
                    Ok(stdout.trim().to_string())
                } else {
                    Err(stderr.trim().to_string())
                };
            }
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("osascript timeout (10s)".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(format!("osascript error: {}", e)),
        }
    }
}

pub fn classify_app(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    let productive = [
        "code",
        "cursor",
        "terminal",
        "iterm",
        "xcode",
        "intellij",
        "webstorm",
        "sublime",
        "vim",
        "neovim",
        "warp",
        "alacritty",
        "kitty",
        "notion",
        "obsidian",
        "figma",
        "linear",
        "github",
        "postman",
    ];
    let distraction = [
        "telegram",
        "discord",
        "slack",
        "whatsapp",
        "instagram",
        "twitter",
        "tiktok",
        "youtube",
        "reddit",
        "netflix",
        "twitch",
        "facebook",
    ];
    if productive.iter().any(|p| lower.contains(p)) {
        "productive"
    } else if distraction.iter().any(|d| lower.contains(d)) {
        "distraction"
    } else {
        "neutral"
    }
}

// ── macOS Actions ──
// ── Phase 5: macOS Actions ──

#[derive(Debug, PartialEq)]
struct SafeCommand {
    program: &'static str,
    args: Vec<String>,
}

/// Parse the small read-only command language exposed to the model.
///
/// This deliberately does not invoke a shell. Prefix checks are not a
/// security boundary when `sh -c` can interpret newlines, redirects or other
/// syntax after the accepted prefix.
fn parse_safe_command(command: &str) -> Result<SafeCommand, String> {
    if command.len() > 500 {
        return Err("Command too long (max 500 chars)".into());
    }
    if command
        .chars()
        .any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}')
    {
        return Err("Control characters are not allowed".into());
    }
    const SHELL_META: [char; 11] = [';', '|', '&', '`', '$', '(', ')', '{', '}', '<', '>'];
    if command.chars().any(|c| SHELL_META.contains(&c)) {
        return Err("Shell syntax is not allowed".into());
    }

    let words = shlex::split(command).ok_or_else(|| "Invalid or unmatched quoting".to_string())?;
    let (name, args) = words
        .split_first()
        .ok_or_else(|| "Command is empty".to_string())?;
    let fixed = |program: &'static str, expected: &[&str]| {
        if args.iter().map(String::as_str).eq(expected.iter().copied()) {
            Ok(SafeCommand {
                program,
                args: args.to_vec(),
            })
        } else {
            Err("Command arguments are not allowed".into())
        }
    };
    let direct = |program: &'static str| SafeCommand {
        program,
        args: args.to_vec(),
    };

    match name.as_str() {
        "date" if args.is_empty() => fixed("/bin/date", &[]),
        "whoami" if args.is_empty() => fixed("/usr/bin/whoami", &[]),
        "pwd" if args.is_empty() => fixed("/bin/pwd", &[]),
        "uname"
            if args.is_empty()
                || args.iter().all(|a| {
                    a.starts_with('-')
                        && a.len() > 1
                        && a[1..].chars().all(|c| "amnrspv".contains(c))
                }) =>
        {
            Ok(direct("/usr/bin/uname"))
        }
        "sw_vers" if args.is_empty() => fixed("/usr/bin/sw_vers", &[]),
        "uptime" if args.is_empty() => fixed("/usr/bin/uptime", &[]),
        "df" => fixed("/bin/df", &["-h"]),
        "python3" => fixed("/usr/bin/python3", &["--version"]),
        "pip" if args.iter().map(String::as_str).eq(["list"]) => Ok(SafeCommand {
            program: "/usr/bin/python3",
            args: vec!["-I".into(), "-m".into(), "pip".into(), "list".into()],
        }),
        "diskutil" => fixed("/usr/sbin/diskutil", &["list"]),
        "pmset" => fixed("/usr/bin/pmset", &["-g"]),
        "ls" if args.is_empty()
            || (args.len() == 1
                && ["-l", "-a", "-la", "-al", "-h", "-lh", "-hl"].contains(&args[0].as_str())) =>
        {
            Ok(direct("/bin/ls"))
        }
        "cat"
            if args.len() == 1
                && [
                    "/etc/hosts",
                    "/etc/paths",
                    "/etc/shells",
                    "/etc/resolv.conf",
                ]
                .contains(&args[0].as_str()) =>
        {
            Ok(direct("/bin/cat"))
        }
        "which"
            if args.len() == 1
                && !args[0].starts_with('-')
                && args[0]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c)) =>
        {
            Ok(direct("/usr/bin/which"))
        }
        "echo" => Ok(direct("/bin/echo")),
        "defaults" => fixed(
            "/usr/bin/defaults",
            &["read", "NSGlobalDomain", "AppleInterfaceStyle"],
        ),
        "system_profiler" => fixed(
            "/usr/sbin/system_profiler",
            &["SPSoftwareDataType", "-detailLevel", "mini"],
        ),
        "sysctl"
            if args.len() == 1
                && args[0]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._".contains(c)) =>
        {
            Ok(direct("/usr/sbin/sysctl"))
        }
        "sysctl"
            if args.len() == 2
                && args[0] == "-n"
                && args[1]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._".contains(c)) =>
        {
            Ok(direct("/usr/sbin/sysctl"))
        }
        "brew" if args.len() == 1 && args[0] == "list" => Ok(direct("/opt/homebrew/bin/brew")),
        "brew"
            if args.len() == 2
                && args[0] == "info"
                && !args[1].starts_with('-')
                && args[1]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "@+._-".contains(c)) =>
        {
            Ok(direct("/opt/homebrew/bin/brew"))
        }
        _ => Err("Command not allowed. Only safe read-only commands are permitted.".into()),
    }
}

async fn read_capped<R>(mut reader: R, cap: usize) -> std::io::Result<(Vec<u8>, u64)>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut kept = Vec::with_capacity(cap);
    let mut total = 0u64;
    let mut chunk = [0u8; 1024];
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        let room = cap.saturating_sub(kept.len());
        kept.extend_from_slice(&chunk[..count.min(room)]);
    }
    Ok((kept, total))
}

#[tauri::command]
pub async fn run_shell(command: String) -> Result<String, String> {
    let safe = parse_safe_command(&command)?;
    let program = if safe.program == "/opt/homebrew/bin/brew"
        && !std::path::Path::new(safe.program).exists()
    {
        "/usr/local/bin/brew"
    } else {
        safe.program
    };
    let mut process = tokio::process::Command::new(program);
    process
        .args(&safe.args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // Isolate descendants too: package managers may spawn helpers that inherit
    // stdout/stderr, so killing only the direct child does not enforce a bound.
    #[cfg(unix)]
    process.process_group(0);
    let mut child = process.spawn().map_err(|e| format!("Command error: {e}"))?;
    let _process_group = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Command stdout unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Command stderr unavailable".to_string())?;
    let mut stdout_task = tokio::spawn(read_capped(stdout, 5000));
    let mut stderr_task = tokio::spawn(read_capped(stderr, 5000));
    let status = match tokio::time::timeout(std::time::Duration::from_secs(10), child.wait()).await
    {
        Ok(result) => result.map_err(|e| format!("Command error: {e}"))?,
        Err(_) => {
            #[cfg(unix)]
            if let Some(pid) = _process_group {
                // SAFETY: the child was placed in a new process group whose id
                // equals its pid. A negative id targets only that group.
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            let _ = child.kill().await;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(1), child.wait()).await;
            stdout_task.abort();
            stderr_task.abort();
            return Err("Command timed out (10s)".into());
        }
    };
    let readers = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        let stdout = (&mut stdout_task)
            .await
            .map_err(|e| format!("stdout task: {e}"))?
            .map_err(|e| format!("stdout read: {e}"))?;
        let stderr = (&mut stderr_task)
            .await
            .map_err(|e| format!("stderr task: {e}"))?
            .map_err(|e| format!("stderr read: {e}"))?;
        Ok::<_, String>((stdout, stderr))
    })
    .await;
    let ((stdout, stdout_total), (stderr, stderr_total)) = match readers {
        Ok(result) => result?,
        Err(_) => {
            #[cfg(unix)]
            if let Some(pid) = _process_group {
                // SAFETY: see the timeout branch above.
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            stdout_task.abort();
            stderr_task.abort();
            return Err("Command output pipes did not close after exit".into());
        }
    };
    let stdout = String::from_utf8_lossy(&stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&stderr).trim().to_string();

    if status.success() {
        if stdout_total > 5000 {
            Ok(format!(
                "{}...\n[truncated, {stdout_total} bytes total]",
                truncate_utf8(&stdout, 5000)
            ))
        } else {
            Ok(stdout)
        }
    } else {
        let suffix = if stderr_total > 5000 {
            format!("... [truncated, {stderr_total} bytes total]")
        } else {
            String::new()
        };
        Err(format!("Command failed: {stderr}{suffix}"))
    }
}

#[cfg(test)]
mod safe_command_tests {
    use super::{parse_safe_command, SafeCommand};

    #[test]
    fn accepts_exact_read_only_commands_without_a_shell() {
        assert_eq!(
            parse_safe_command("df -h").unwrap(),
            SafeCommand {
                program: "/bin/df",
                args: vec!["-h".into()]
            }
        );
        assert_eq!(
            parse_safe_command("echo hello world").unwrap().program,
            "/bin/echo"
        );
        assert_eq!(
            parse_safe_command("echo \"hello world\"").unwrap().args,
            vec!["hello world"]
        );
        assert_eq!(parse_safe_command("ls -la").unwrap().args, vec!["-la"]);
    }

    #[test]
    fn rejects_prefix_and_shell_injection() {
        for input in [
            "echo ok\nid",
            "echo ok; id",
            "echo $(id)",
            "date --help",
            "cat /tmp/secret",
            "cat /etc/../Users/alice/.ssh/id_ed25519",
            "cat /etc/sudoers",
            "sysctl -w kern.hostname=owned",
            "defaults write com.apple.dock orientation left",
            "diskutil eraseDisk APFS Hanni disk0",
            "echo \"unterminated",
            "brew info good;id",
            "ls /Users/alice/.ssh",
            "sysctl -a",
            "system_profiler SPHardwareDataType -detailLevel full",
            "ioreg -l",
            "ioreg -c IOHIDSystem -d 4",
            "networksetup -listallhardwareports",
            "defaults read com.example.secret token",
            "system_profiler SPHardwareDataType -detailLevel mini",
            "which --help",
            "brew info --github",
        ] {
            assert!(parse_safe_command(input).is_err(), "accepted {input:?}");
        }
    }
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<String, String> {
    // Only allow http:// and https:// to prevent file://, javascript:, etc.
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only http:// and https:// URLs allowed".into());
    }
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Open error: {}", e))?;
    Ok(format!("Opened {}", url))
}

#[tauri::command]
pub async fn send_notification(title: String, body: String) -> Result<String, String> {
    let script = format!(
        "display notification \"{}\" with title \"{}\" sound name \"Ping\"",
        osa_escape(&body),
        osa_escape(&title)
    );
    run_osascript(&script)?;
    Ok("Notification sent".into())
}

#[tauri::command]
pub async fn set_volume(level: u32) -> Result<String, String> {
    let clamped = level.min(100);
    run_osascript(&format!("set volume output volume {}", clamped))?;
    Ok(format!("Volume set to {}%", clamped))
}

#[tauri::command]
pub async fn open_app(name: String) -> Result<String, String> {
    let safe = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '.')
        .collect::<String>();
    if safe.is_empty() {
        return Err("Invalid app name".into());
    }
    run_osascript(&format!("tell application \"{}\" to activate", safe))?;
    Ok(format!("Opened {}", safe))
}

#[tauri::command]
pub async fn close_app(name: String) -> Result<String, String> {
    let safe = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '.')
        .collect::<String>();
    if safe.is_empty() {
        return Err("Invalid app name".into());
    }
    run_osascript(&format!("tell application \"{}\" to quit", safe))?;
    Ok(format!("Closed {}", safe))
}

#[tauri::command]
pub async fn music_control(action: String) -> Result<String, String> {
    let script = match action.as_str() {
        "play" | "resume" => "tell application \"Music\" to play",
        "pause" | "stop" => "tell application \"Music\" to pause",
        "next" | "skip" => "tell application \"Music\" to next track",
        "previous" | "prev" | "back" => "tell application \"Music\" to previous track",
        "toggle" => "tell application \"Music\" to playpause",
        _ => return Err(format!("Unknown music action: {}", action)),
    };
    run_osascript(script)?;
    Ok(format!("Music: {}", action))
}

#[tauri::command]
pub async fn get_clipboard() -> Result<String, String> {
    let output = std::process::Command::new("pbpaste")
        .output()
        .map_err(|e| format!("Clipboard error: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
pub async fn set_clipboard(text: String) -> Result<String, String> {
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Clipboard error: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
    }
    child.wait().map_err(|e| format!("Wait error: {}", e))?;
    Ok("Copied to clipboard".into())
}

// ── Reminders, web_search, read_url ──
// ── Reminders & Timers ──

#[tauri::command]
pub fn set_reminder(
    title: String,
    remind_at: String,
    repeat: Option<String>,
    db: tauri::State<'_, HanniDb>,
) -> Result<String, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO reminders (title, remind_at, repeat, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![title, remind_at, repeat, now],
    )
    .map_err(|e| format!("DB error: {}", e))?;
    Ok(format!("Reminder set: {} at {}", title, remind_at))
}

#[tauri::command]
pub fn get_reminders(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, remind_at, repeat, fired FROM reminders WHERE fired=0 ORDER BY remind_at"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "remind_at": row.get::<_, String>(2)?,
                "repeat": row.get::<_, Option<String>>(3)?,
                "fired": row.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_reminder(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM reminders WHERE id=?1", rusqlite::params![id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// ── Web Search ──

#[tauri::command]
pub async fn web_search(query: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Client error: {}", e))?;

    // Use DuckDuckGo HTML (no API key needed)
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        query
            .replace(' ', "+")
            .replace('&', "%26")
            .replace('#', "%23")
    );

    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
        )
        .send()
        .await
        .map_err(|e| format!("Search error: {}", e))?;

    let html = response
        .text()
        .await
        .map_err(|e| format!("Read error: {}", e))?;

    // Parse results from DuckDuckGo HTML
    let mut results = Vec::new();
    let re_title = regex::Regex::new(r#"class="result__a"[^>]*>([^<]+)</a>"#).unwrap();
    let re_snippet = regex::Regex::new(r#"class="result__snippet"[^>]*>(.*?)</a>"#).unwrap();
    let re_url = regex::Regex::new(r#"class="result__url"[^>]*>([^<]+)</[^>]+>"#).unwrap();

    let titles: Vec<String> = re_title
        .captures_iter(&html)
        .map(|c| c[1].to_string())
        .collect();
    let snippets: Vec<String> = re_snippet
        .captures_iter(&html)
        .map(|c| {
            // Strip HTML tags from snippet
            let raw = c[1].to_string();
            regex::Regex::new(r"<[^>]+>")
                .unwrap()
                .replace_all(&raw, "")
                .to_string()
        })
        .collect();
    let urls: Vec<String> = re_url
        .captures_iter(&html)
        .map(|c| c[1].trim().to_string())
        .collect();

    for i in 0..titles.len().min(5) {
        let snippet = snippets.get(i).map(|s| s.as_str()).unwrap_or("");
        let url = urls.get(i).map(|s| s.as_str()).unwrap_or("");
        results.push(format!(
            "{}. {} — {}\n   {}",
            i + 1,
            titles[i],
            snippet,
            url
        ));
    }

    if results.is_empty() {
        Ok(format!("No results found for '{}'", query))
    } else {
        Ok(results.join("\n\n"))
    }
}

#[tauri::command]
pub async fn read_url(url: String) -> Result<String, String> {
    // Validate URL scheme
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only http/https URLs are supported".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Client error: {}", e))?;

    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
        )
        .send()
        .await
        .map_err(|e| format!("Fetch error: {}", e))?;

    let html = response
        .text()
        .await
        .map_err(|e| format!("Read error: {}", e))?;

    // Strip script, style, nav, footer, header blocks
    let re_blocks = regex::Regex::new(
        r"(?is)<(script|style|nav|footer|header|noscript|svg|iframe)[^>]*>.*?</\1>",
    )
    .unwrap();
    let text = re_blocks.replace_all(&html, "");

    // Strip all remaining HTML tags
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&text, "");

    // Decode common HTML entities
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Collapse whitespace: multiple spaces/tabs → single space, multiple newlines → double newline
    let re_spaces = regex::Regex::new(r"[^\S\n]+").unwrap();
    let text = re_spaces.replace_all(&text, " ");
    let re_newlines = regex::Regex::new(r"\n{3,}").unwrap();
    let text = re_newlines.replace_all(&text, "\n\n");

    let text = text.trim().to_string();

    // Truncate to ~4000 chars to fit LLM context
    if text.len() > 4000 {
        let truncated = &text[..text[..4000].rfind(' ').unwrap_or(4000)];
        Ok(format!(
            "{}\n\n[...truncated, {} chars total]",
            truncated,
            text.len()
        ))
    } else if text.is_empty() {
        Ok(format!("Could not extract text from {}", url))
    } else {
        Ok(text)
    }
}

// ── macOS Info Commands ──
// ── macOS commands ──

#[tauri::command]
pub async fn get_activity_summary() -> Result<String, String> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/Knowledge/knowledgeC.db");

    if !db_path.exists() {
        return Err("Screen Time data unavailable. Grant Full Disk Access: \
             System Settings → Privacy & Security → Full Disk Access → add Hanni"
            .into());
    }

    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        if e.to_string().contains("unable to open")
            || e.to_string().contains("authorization denied")
        {
            "Screen Time data unavailable. Grant Full Disk Access: \
             System Settings → Privacy & Security → Full Disk Access → add Hanni"
                .to_string()
        } else {
            format!("Cannot open knowledgeC.db: {}", e)
        }
    })?;

    let mut stmt = conn
        .prepare(
            "SELECT
                ZSOURCE.ZNAME as app_name,
                ZSOURCE.ZBUNDLEID as bundle_id,
                ROUND(SUM(CAST((ZOBJECT.ZENDDATE - ZOBJECT.ZSTARTDATE) AS REAL)) / 60, 1) as minutes
            FROM ZOBJECT
            JOIN ZSOURCE ON ZOBJECT.ZSOURCE = ZSOURCE.Z_PK
            WHERE
                DATE(datetime(ZOBJECT.ZSTARTDATE + 978307200, 'unixepoch', 'localtime')) = DATE('now')
                AND ZOBJECT.ZSTREAMNAME = '/app/inFocus'
                AND ZOBJECT.ZENDDATE > ZOBJECT.ZSTARTDATE
            GROUP BY ZSOURCE.ZBUNDLEID
            ORDER BY minutes DESC",
        )
        .map_err(|e| format!("SQL error: {}", e))?;

    struct AppRow {
        app_name: String,
        minutes: f64,
        category: String,
    }

    let rows: Vec<AppRow> = stmt
        .query_map([], |row| {
            let app_name: String = row.get::<_, Option<String>>(0)?.unwrap_or_default();
            let minutes: f64 = row.get(2)?;
            Ok((app_name, minutes))
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .map(|(app_name, minutes)| {
            let category = classify_app(&app_name).to_string();
            AppRow {
                app_name,
                minutes,
                category,
            }
        })
        .collect();

    if rows.is_empty() {
        return Ok("No Screen Time data for today yet.".into());
    }

    let mut productive: f64 = 0.0;
    let mut distraction: f64 = 0.0;
    let mut neutral: f64 = 0.0;

    for r in &rows {
        match r.category.as_str() {
            "productive" => productive += r.minutes,
            "distraction" => distraction += r.minutes,
            _ => neutral += r.minutes,
        }
    }

    let top_apps: Vec<String> = rows
        .iter()
        .take(5)
        .map(|r| format!("  {} — {:.0} min ({})", r.app_name, r.minutes, r.category))
        .collect();

    Ok(format!(
        "Activity today (Screen Time):\n\
         Productive: {:.0} min | Distraction: {:.0} min | Neutral: {:.0} min\n\n\
         Top apps:\n{}",
        productive,
        distraction,
        neutral,
        top_apps.join("\n")
    ))
}

#[tauri::command]
pub async fn get_calendar_events() -> Result<String, String> {
    if !check_calendar_access() {
        return Ok(
            "Calendar access denied. Enable in System Settings → Privacy → Automation".into(),
        );
    }
    let script = r#"
        if application "Calendar" is not running then
            return "Calendar.app not running — skipped"
        end if
        set output to ""
        set today to current date
        set endDate to today + (2 * days)
        tell application "Calendar"
            repeat with cal in calendars
                set evts to (every event of cal whose start date >= today and start date <= endDate)
                repeat with evt in evts
                    set evtStart to start date of evt
                    set evtName to summary of evt
                    set output to output & (evtStart as string) & " | " & evtName & linefeed
                end repeat
            end repeat
        end tell
        if output is "" then
            return "No upcoming events in the next 2 days."
        end if
        return output
    "#;
    run_osascript(script)
}

#[tauri::command]
pub async fn get_now_playing() -> Result<String, String> {
    // Check Music.app
    let music_check = run_osascript(
        "tell application \"System Events\" to (name of processes) contains \"Music\"",
    );
    if let Ok(ref val) = music_check {
        if val == "true" {
            let result = run_osascript(
                "tell application \"Music\" to if player state is playing then \
                 return (name of current track) & \" — \" & (artist of current track) \
                 else return \"Music paused\" end if",
            );
            if let Ok(info) = result {
                return Ok(format!("Apple Music: {}", info));
            }
        }
    }

    // Check Spotify
    let spotify_check = run_osascript(
        "tell application \"System Events\" to (name of processes) contains \"Spotify\"",
    );
    if let Ok(ref val) = spotify_check {
        if val == "true" {
            let result = run_osascript(
                "tell application \"Spotify\" to if player state is playing then \
                 return (name of current track) & \" — \" & (artist of current track) \
                 else return \"Spotify paused\" end if",
            );
            if let Ok(info) = result {
                return Ok(format!("Spotify: {}", info));
            }
        }
    }

    Ok("No music app is currently playing.".into())
}

#[tauri::command]
pub async fn get_browser_tab() -> Result<String, String> {
    let browsers = [
        ("Arc", "tell application \"Arc\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Google Chrome", "tell application \"Google Chrome\" to return URL of active tab of front window & \" | \" & title of active tab of front window"),
        ("Safari", "tell application \"Safari\" to return URL of front document & \" | \" & name of front document"),
    ];

    for (name, script) in &browsers {
        let check = run_osascript(&format!(
            "tell application \"System Events\" to (name of processes) contains \"{}\"",
            name
        ));
        if let Ok(ref val) = check {
            if val == "true" {
                if let Ok(info) = run_osascript(script) {
                    return Ok(format!("{}: {}", name, info));
                }
            }
        }
    }

    Ok("No supported browser is currently open.".into())
}
