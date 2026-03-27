// calendar.rs — Calendar events, sync, ICS parsing
use crate::types::*;
use crate::macos::{check_calendar_access, run_osascript};
use chrono::Timelike;

// ── v0.7.0: Events (Calendar) commands ──

#[tauri::command]
pub fn create_event(title: String, description: String, date: String, time: String, duration_minutes: i64, category: String, color: String, db: tauri::State<'_, HanniDb>) -> Result<i64, String> {
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO events (title, description, date, time, duration_minutes, category, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![title, description, date, time, duration_minutes, category, color, now],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn get_events(month: u32, year: i32, db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let prefix = format!("{}-{:02}", year, month);
    let mut stmt = conn.prepare(
        "SELECT id, title, description, date, time, duration_minutes, category, color, completed, COALESCE(source,'manual') FROM events WHERE date LIKE ?1 ORDER BY date, time"
    ).map_err(|e| format!("DB error: {}", e))?;
    let pattern = format!("{}%", prefix);
    let rows = stmt.query_map(rusqlite::params![pattern], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "time": row.get::<_, String>(4)?,
            "duration_minutes": row.get::<_, i64>(5)?,
            "category": row.get::<_, String>(6)?,
            "color": row.get::<_, String>(7)?,
            "completed": row.get::<_, i32>(8)? != 0,
            "source": row.get::<_, String>(9)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub fn delete_event(id: i64, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM events WHERE id=?1", rusqlite::params![id]).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_event(id: i64, title: Option<String>, description: Option<String>, date: Option<String>, time: Option<String>, duration_minutes: Option<i64>, category: Option<String>, color: Option<String>, completed: Option<bool>, db: tauri::State<'_, HanniDb>) -> Result<(), String> {
    let conn = db.conn();
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    if let Some(v) = title { updates.push(format!("title=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = description { updates.push(format!("description=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = date { updates.push(format!("date=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = time { updates.push(format!("time=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = duration_minutes { updates.push(format!("duration_minutes=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = category { updates.push(format!("category=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = color { updates.push(format!("color=?{}", idx)); params.push(Box::new(v)); idx += 1; }
    if let Some(v) = completed { updates.push(format!("completed=?{}", idx)); params.push(Box::new(v as i64)); idx += 1; }
    if updates.is_empty() { return Ok(()); }
    params.push(Box::new(id));
    let sql = format!("UPDATE events SET {} WHERE id=?{}", updates.join(","), idx);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice()).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_all_events(db: tauri::State<'_, HanniDb>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, title, description, date, time, duration_minutes, category, color, completed, source
         FROM events ORDER BY date DESC, time DESC"
    ).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?,
            "title": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "date": row.get::<_, String>(3)?,
            "time": row.get::<_, String>(4)?,
            "duration_minutes": row.get::<_, i64>(5)?,
            "category": row.get::<_, String>(6)?,
            "color": row.get::<_, String>(7)?,
            "completed": row.get::<_, i64>(8)?,
            "source": row.get::<_, Option<String>>(9)?,
        }))
    }).map_err(|e| format!("Query error: {}", e))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ── v0.8.3: Calendar Sync ──

#[tauri::command]
pub async fn sync_apple_calendar(month: u32, year: i32, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    // AppleScript to get events from Calendar.app for the given month
    let prefix = format!("{}-{:02}", year, month);
    let last_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 },
        _ => 31,
    };

    let script = format!(
        r#"
        if application "Calendar" is not running then
            return "NOT_RUNNING"
        end if
        set output to ""
        set startD to current date
        set year of startD to {year}
        set month of startD to {month}
        set day of startD to 1
        set time of startD to 0
        set endD to current date
        set year of endD to {year}
        set month of endD to {month}
        set day of endD to {last_day}
        set time of endD to 86399
        tell application "Calendar"
            repeat with cal in calendars
                set calName to name of cal
                set evts to (every event of cal whose start date >= startD and start date <= endD)
                repeat with evt in evts
                    set evtStart to start date of evt
                    set evtName to summary of evt
                    set evtDur to 60
                    try
                        set evtEnd to end date of evt
                        set evtDur to ((evtEnd - evtStart) / 60) as integer
                    end try
                    set evtDesc to ""
                    try
                        set evtDesc to description of evt
                    end try
                    set evtUID to uid of evt
                    set m to (month of evtStart as integer)
                    set d to day of evtStart
                    set h to hours of evtStart
                    set mn to minutes of evtStart
                    set dateStr to "{year}-" & text -2 thru -1 of ("0" & m) & "-" & text -2 thru -1 of ("0" & d)
                    set timeStr to text -2 thru -1 of ("0" & h) & ":" & text -2 thru -1 of ("0" & mn)
                    set output to output & evtUID & "||" & evtName & "||" & dateStr & "||" & timeStr & "||" & evtDur & "||" & calName & "||" & evtDesc & linefeed
                end repeat
            end repeat
        end tell
        return output
        "#,
        year = year, month = month, last_day = last_day
    );

    // Pre-check: verify Calendar.app access permission (cached — won't re-prompt after denial)
    if !check_calendar_access() {
        return Ok(serde_json::json!({
            "synced": 0,
            "source": "apple",
            "error": "Нет доступа к Calendar.app. Включите в Системные настройки → Конфиденциальность → Автоматизация"
        }));
    }

    let output = match run_osascript(&script) {
        Ok(s) => s,
        Err(e) => {
            return Ok(serde_json::json!({
                "synced": 0,
                "source": "apple",
                "error": format!("Ошибка синхронизации: {}", e)
            }));
        }
    };

    // Calendar.app not running — return zero events without clearing DB
    if output.trim() == "NOT_RUNNING" {
        return Ok(serde_json::json!({ "synced": 0, "source": "apple", "skipped": true, "error": null }));
    }

    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();

    // Clear old apple events for this month
    conn.execute(
        "DELETE FROM events WHERE source='apple' AND date LIKE ?1",
        rusqlite::params![format!("{}%", prefix)],
    ).map_err(|e| format!("DB error: {}", e))?;

    let mut count = 0i32;
    for line in output.lines() {
        let parts: Vec<&str> = line.split("||").collect();
        if parts.len() < 6 { continue; }
        let uid = parts[0].trim();
        let title = parts[1].trim();
        let date = parts[2].trim();
        let time = parts[3].trim();
        let dur: i64 = parts[4].trim().parse().unwrap_or(60);
        let cal_name = parts[5].trim();
        let desc = parts.get(6).unwrap_or(&"").trim();

        conn.execute(
            "INSERT INTO events (title, description, date, time, duration_minutes, category, color, source, external_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'apple', ?8, ?9)",
            rusqlite::params![title, desc, date, time, dur, cal_name, "#a1a1a6", uid, now],
        ).map_err(|e| format!("Insert error: {}", e))?;
        count += 1;
    }

    Ok(serde_json::json!({ "synced": count, "source": "apple", "error": null }))
}

/// Parse ICS datetime line, handling TZID and UTC 'Z' suffix. Returns (NaiveDate, Option<NaiveTime>, is_allday).
pub fn parse_ics_datetime(line: &str) -> Option<(chrono::NaiveDate, Option<chrono::NaiveTime>, bool)> {
    use chrono::{NaiveDate, NaiveTime, NaiveDateTime, TimeZone};

    // All-day: DTSTART;VALUE=DATE:20250215
    if line.contains("VALUE=DATE") {
        let re_d = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})").unwrap();
        let caps = re_d.captures(line)?;
        let d = NaiveDate::from_ymd_opt(caps[1].parse().ok()?, caps[2].parse().ok()?, caps[3].parse().ok()?)?;
        return Some((d, None, true));
    }

    let re_dt = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})?").unwrap();
    let caps = re_dt.captures(line)?;
    let y: i32 = caps[1].parse().ok()?;
    let mo: u32 = caps[2].parse().ok()?;
    let da: u32 = caps[3].parse().ok()?;
    let h: u32 = caps[4].parse().ok()?;
    let mi: u32 = caps[5].parse().ok()?;
    let s: u32 = caps.get(6).and_then(|c| c.as_str().parse().ok()).unwrap_or(0);

    let naive_dt = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(y, mo, da)?,
        NaiveTime::from_hms_opt(h, mi, s)?,
    );

    // Extract TZID if present: DTSTART;TZID=America/New_York:20250215T093000
    let re_tzid = regex::Regex::new(r"TZID=([^:;]+)").unwrap();
    let local_dt = if let Some(tz_caps) = re_tzid.captures(line) {
        let tz_str = tz_caps[1].trim();
        if let Ok(tz) = tz_str.parse::<chrono_tz::Tz>() {
            // Parse in source timezone, convert to local
            match tz.from_local_datetime(&naive_dt).earliest() {
                Some(zoned) => zoned.with_timezone(&chrono::Local).naive_local(),
                None => naive_dt,
            }
        } else {
            naive_dt // Unknown timezone, use as-is
        }
    } else if line.contains('Z') {
        // UTC: convert to local
        match chrono::Utc.from_utc_datetime(&naive_dt).with_timezone(&chrono::Local).naive_local() {
            dt => dt,
        }
    } else {
        // No timezone info — treat as local (floating time)
        naive_dt
    };

    Some((local_dt.date(), Some(local_dt.time()), false))
}

/// Parse RRULE line into components
pub struct RRule {
    freq: String,
    interval: u32,
    count: Option<u32>,
    until: Option<chrono::NaiveDate>,
    byday: Vec<String>,
}

pub fn parse_rrule(block: &str) -> Option<RRule> {
    let rrule_line = block.lines().find(|l| l.starts_with("RRULE:"))?;
    let params = &rrule_line["RRULE:".len()..];

    let mut freq = String::new();
    let mut interval = 1u32;
    let mut count = None;
    let mut until = None;
    let mut byday = Vec::new();

    for part in params.split(';') {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let val = kv.next().unwrap_or("").trim();
        match key {
            "FREQ" => freq = val.to_string(),
            "INTERVAL" => interval = val.parse().unwrap_or(1),
            "COUNT" => count = val.parse().ok(),
            "UNTIL" => {
                let re_d = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})").unwrap();
                if let Some(c) = re_d.captures(val) {
                    until = chrono::NaiveDate::from_ymd_opt(
                        c[1].parse().unwrap_or(2099), c[2].parse().unwrap_or(1), c[3].parse().unwrap_or(1)
                    );
                }
            }
            "BYDAY" => byday = val.split(',').map(|s| s.trim().to_string()).collect(),
            _ => {}
        }
    }

    if freq.is_empty() { return None; }
    Some(RRule { freq, interval, count, until, byday })
}

/// Collect EXDATE dates from a VEVENT block
pub fn parse_exdates(block: &str) -> std::collections::HashSet<chrono::NaiveDate> {
    let mut set = std::collections::HashSet::new();
    let re_d = regex::Regex::new(r"(\d{4})(\d{2})(\d{2})").unwrap();
    for line in block.lines() {
        if line.starts_with("EXDATE") {
            for caps in re_d.captures_iter(line) {
                if let Some(d) = chrono::NaiveDate::from_ymd_opt(
                    caps[1].parse().unwrap_or(0), caps[2].parse().unwrap_or(0), caps[3].parse().unwrap_or(0)
                ) {
                    set.insert(d);
                }
            }
        }
    }
    set
}

/// Map BYDAY codes to chrono::Weekday
pub fn byday_to_weekday(code: &str) -> Option<chrono::Weekday> {
    // Strip numeric prefix (e.g. "2MO" → "MO")
    let code = code.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '+');
    match code {
        "MO" => Some(chrono::Weekday::Mon),
        "TU" => Some(chrono::Weekday::Tue),
        "WE" => Some(chrono::Weekday::Wed),
        "TH" => Some(chrono::Weekday::Thu),
        "FR" => Some(chrono::Weekday::Fri),
        "SA" => Some(chrono::Weekday::Sat),
        "SU" => Some(chrono::Weekday::Sun),
        _ => None,
    }
}

/// Expand RRULE occurrences that fall within the target month
pub fn expand_rrule(
    start_date: chrono::NaiveDate,
    rrule: &RRule,
    exdates: &std::collections::HashSet<chrono::NaiveDate>,
    target_year: i32,
    target_month: u32,
) -> Vec<chrono::NaiveDate> {
    use chrono::{NaiveDate, Datelike, Duration};

    let month_start = match NaiveDate::from_ymd_opt(target_year, target_month, 1) {
        Some(d) => d,
        None => return vec![],
    };
    let month_end = if target_month == 12 {
        NaiveDate::from_ymd_opt(target_year + 1, 1, 1).unwrap_or(month_start)
    } else {
        NaiveDate::from_ymd_opt(target_year, target_month + 1, 1).unwrap_or(month_start)
    };
    // Don't expand too far into the future (max 3 years from start)
    let hard_limit = start_date + Duration::days(365 * 3);
    let effective_end = month_end.min(hard_limit);

    let max_count = rrule.count.unwrap_or(1000) as usize;
    let until = rrule.until.unwrap_or(effective_end);

    let mut results = Vec::new();
    let mut occurrence_count = 0usize;

    match rrule.freq.as_str() {
        "DAILY" => {
            let step = Duration::days(rrule.interval as i64);
            let mut d = start_date;
            while d <= until && d < effective_end && occurrence_count < max_count {
                if d >= month_start && d < month_end && !exdates.contains(&d) {
                    results.push(d);
                }
                occurrence_count += 1;
                d += step;
            }
        }
        "WEEKLY" => {
            let weekdays: Vec<chrono::Weekday> = if rrule.byday.is_empty() {
                vec![start_date.weekday()]
            } else {
                rrule.byday.iter().filter_map(|s| byday_to_weekday(s)).collect()
            };
            let step = Duration::weeks(rrule.interval as i64);
            // Walk week by week from start
            let mut week_start = start_date - Duration::days(start_date.weekday().num_days_from_monday() as i64);
            while week_start <= until && week_start < effective_end + Duration::days(7) && occurrence_count < max_count {
                for wd in &weekdays {
                    let d = week_start + Duration::days(wd.num_days_from_monday() as i64);
                    if d < start_date { continue; }
                    if d > until || d >= effective_end { continue; }
                    if occurrence_count >= max_count { break; }
                    occurrence_count += 1;
                    if d >= month_start && d < month_end && !exdates.contains(&d) {
                        results.push(d);
                    }
                }
                week_start += step;
            }
        }
        "MONTHLY" => {
            let day = start_date.day();
            let mut y = start_date.year();
            let mut m = start_date.month();
            while occurrence_count < max_count {
                if let Some(d) = NaiveDate::from_ymd_opt(y, m, day.min(28)) // safe day
                    .or_else(|| NaiveDate::from_ymd_opt(y, m, 28))
                {
                    if d > until || d >= effective_end { break; }
                    if d >= start_date {
                        occurrence_count += 1;
                        if d >= month_start && d < month_end && !exdates.contains(&d) {
                            results.push(d);
                        }
                    }
                }
                // Advance by interval months
                for _ in 0..rrule.interval {
                    m += 1;
                    if m > 12 { m = 1; y += 1; }
                }
            }
        }
        "YEARLY" => {
            let mut y = start_date.year();
            while occurrence_count < max_count {
                if let Some(d) = NaiveDate::from_ymd_opt(y, start_date.month(), start_date.day().min(28))
                    .or_else(|| NaiveDate::from_ymd_opt(y, start_date.month(), 28))
                {
                    if d > until || d >= effective_end { break; }
                    if d >= start_date {
                        occurrence_count += 1;
                        if d >= month_start && d < month_end && !exdates.contains(&d) {
                            results.push(d);
                        }
                    }
                }
                y += rrule.interval as i32;
            }
        }
        _ => {}
    }

    results
}

#[tauri::command]
pub async fn sync_google_ics(url: String, month: u32, year: i32, db: tauri::State<'_, HanniDb>) -> Result<serde_json::Value, String> {
    if url.is_empty() { return Err("No ICS URL provided".into()); }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let body = client.get(&url).send().await
        .map_err(|e| format!("Fetch error: {}", e))?
        .text().await
        .map_err(|e| format!("Read error: {}", e))?;

    let prefix = format!("{}-{:02}", year, month);
    let conn = db.conn();
    let now = chrono::Local::now().to_rfc3339();

    // Clear old google events for this month
    conn.execute(
        "DELETE FROM events WHERE source='google' AND date LIKE ?1",
        rusqlite::params![format!("{}%", prefix)],
    ).map_err(|e| format!("DB error: {}", e))?;

    let mut count = 0i32;

    for block in body.split("BEGIN:VEVENT") {
        if !block.contains("END:VEVENT") { continue; }
        let get_field = |field: &str| -> String {
            block.lines()
                .find(|l| l.starts_with(field))
                .map(|l| l[field.len()..].trim().to_string())
                .unwrap_or_default()
        };

        let summary = get_field("SUMMARY:");
        if summary.is_empty() { continue; }

        let dtstart_line = block.lines()
            .find(|l| l.starts_with("DTSTART"))
            .unwrap_or("");
        let dtend_line = block.lines()
            .find(|l| l.starts_with("DTEND"))
            .unwrap_or("");
        let uid = get_field("UID:");
        let desc = get_field("DESCRIPTION:").replace("\\n", "\n").replace("\\,", ",");

        // Parse start datetime with timezone handling
        let (start_date, start_time, is_allday) = match parse_ics_datetime(dtstart_line) {
            Some(v) => v,
            None => continue,
        };
        let time_str = start_time.map(|t| t.format("%H:%M").to_string()).unwrap_or_default();

        // Calculate duration
        let dur: i64 = if is_allday {
            0
        } else if let Some((end_date, end_time, _)) = parse_ics_datetime(dtend_line) {
            if let (Some(st), Some(et)) = (start_time, end_time) {
                let start_mins = st.hour() as i64 * 60 + st.minute() as i64;
                let end_mins = et.hour() as i64 * 60 + et.minute() as i64;
                let day_diff = (end_date - start_date).num_days() * 24 * 60;
                (end_mins - start_mins + day_diff).max(1)
            } else { 60 }
        } else { 60 };

        // Collect dates to insert: original + RRULE expansions
        let mut dates_to_insert: Vec<chrono::NaiveDate> = Vec::new();

        // Check if original date falls in target month
        let date_str = start_date.format("%Y-%m").to_string();
        if date_str == prefix {
            dates_to_insert.push(start_date);
        }

        // RRULE expansion
        if let Some(rrule) = parse_rrule(block) {
            let exdates = parse_exdates(block);
            let mut expanded = expand_rrule(start_date, &rrule, &exdates, year, month as u32);
            // Remove the original date if already added (avoid duplicates)
            expanded.retain(|d| *d != start_date || !dates_to_insert.contains(d));
            dates_to_insert.extend(expanded);
        }

        // Deduplicate
        dates_to_insert.sort();
        dates_to_insert.dedup();

        // Insert each occurrence
        for occ_date in &dates_to_insert {
            let occ_date_str = occ_date.format("%Y-%m-%d").to_string();
            let ext_id = if *occ_date == start_date {
                uid.clone()
            } else {
                format!("{}_{}", uid, occ_date_str)
            };

            conn.execute(
                "INSERT INTO events (title, description, date, time, duration_minutes, category, color, source, external_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'google', ?6, 'google', ?7, ?8)",
                rusqlite::params![summary, desc, occ_date_str, time_str, dur, "#a1a1a6", ext_id, now],
            ).map_err(|e| format!("Insert error: {}", e))?;
            count += 1;
        }
    }

    Ok(serde_json::json!({ "synced": count, "source": "google" }))
}

