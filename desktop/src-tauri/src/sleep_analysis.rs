// sleep_analysis.rs — "Why am I sleepy?" analytics based on sleep data
use crate::types::HanniDb;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct SleepAnalysis {
    pub optimal_bedtime: String,
    pub current_avg_bedtime: String,
    pub bedtime_deviation_minutes: i64,
    pub avg_sleep_quality: f64,
    pub sleep_quality_trend: String,
    pub sleep_debt_hours: f64,
    pub consistency_minutes: i64,
    pub recommendations: Vec<String>,
}

#[tauri::command]
pub fn get_sleep_analysis(db: State<'_, HanniDb>, days: i64) -> SleepAnalysis {
    let conn = db.conn();
    let since = (chrono::Utc::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();

    // Fetch all sessions with durations and bedtimes
    let mut stmt = conn.prepare(
        "SELECT date, start_time, duration_minutes FROM sleep_sessions WHERE date >= ?1 ORDER BY date"
    ).unwrap();
    let rows: Vec<(String, String, i64)> = stmt.query_map([&since], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    }).unwrap().filter_map(|r| r.ok()).collect();

    if rows.is_empty() {
        return SleepAnalysis {
            optimal_bedtime: "—".into(), current_avg_bedtime: "—".into(),
            bedtime_deviation_minutes: 0, avg_sleep_quality: 0.0,
            sleep_quality_trend: "unknown".into(), sleep_debt_hours: 0.0,
            consistency_minutes: 0, recommendations: vec!["Нет данных о сне. Синхронизируйте Samsung Health.".into()],
        };
    }

    // Parse bedtimes as minutes-from-midnight (negative = before midnight)
    let bedtimes: Vec<i64> = rows.iter().map(|(_, st, _)| parse_bedtime_min(st)).collect();
    let durations: Vec<i64> = rows.iter().map(|(_, _, d)| *d).collect();

    // Average bedtime
    let avg_bed = bedtimes.iter().sum::<i64>() as f64 / bedtimes.len() as f64;
    let current_avg_bedtime = min_to_time(avg_bed as i64);

    // Consistency: stddev of bedtimes
    let variance = bedtimes.iter().map(|b| (*b as f64 - avg_bed).powi(2)).sum::<f64>() / bedtimes.len() as f64;
    let consistency = variance.sqrt() as i64;

    // Find optimal bedtime: bucket by 30-min windows, find the one with highest avg duration
    let mut buckets: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for (bed, dur) in bedtimes.iter().zip(durations.iter()) {
        let bucket = (*bed / 30) * 30; // round to 30-min
        buckets.entry(bucket).or_default().push(*dur);
    }
    let (best_bucket, _) = buckets.iter()
        .map(|(b, durs)| (*b, durs.iter().sum::<i64>() as f64 / durs.len() as f64))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((avg_bed as i64, 0.0));
    let optimal_bedtime = format!("{} — {}", min_to_time(best_bucket), min_to_time(best_bucket + 30));

    // Deviation from optimal
    let deviation = avg_bed as i64 - best_bucket;

    // Sleep quality: avg duration score (target 480 min = 8h)
    let avg_dur = durations.iter().sum::<i64>() as f64 / durations.len() as f64;
    let quality = (avg_dur / 480.0 * 100.0).min(100.0);

    // Trend: compare first half vs second half
    let mid = durations.len() / 2;
    let first_avg = if mid > 0 { durations[..mid].iter().sum::<i64>() as f64 / mid as f64 } else { 0.0 };
    let second_avg = if mid > 0 { durations[mid..].iter().sum::<i64>() as f64 / (durations.len() - mid) as f64 } else { 0.0 };
    let trend = if second_avg > first_avg + 10.0 { "improving" } else if second_avg < first_avg - 10.0 { "declining" } else { "stable" };

    // Sleep debt: cumulative deficit from 8h target
    let total_debt: f64 = durations.iter().map(|d| (480.0 - *d as f64).max(0.0)).sum::<f64>() / 60.0;

    // Recommendations
    let mut recs = Vec::new();
    if deviation.abs() > 60 {
        recs.push(format!("Ложитесь ближе к {}, сейчас вы ложитесь на {}мин позже оптимального", min_to_time(best_bucket), deviation.abs()));
    }
    if consistency > 60 {
        recs.push("Ваш режим нестабилен. Старайтесь ложиться в одно время (±30 мин)".into());
    }
    if avg_dur < 420.0 {
        recs.push(format!("Средний сон {:.0} мин — менее 7 часов. Добавьте хотя бы 30 мин", avg_dur));
    }
    if total_debt > 5.0 {
        recs.push(format!("Сонный долг {:.0}ч за {} дней. Нужно компенсировать", total_debt, days));
    }
    if recs.is_empty() {
        recs.push("Ваш сон в норме! Продолжайте в том же режиме.".into());
    }

    SleepAnalysis {
        optimal_bedtime, current_avg_bedtime, bedtime_deviation_minutes: deviation,
        avg_sleep_quality: quality, sleep_quality_trend: trend.into(),
        sleep_debt_hours: total_debt, consistency_minutes: consistency, recommendations: recs,
    }
}

fn parse_bedtime_min(time_str: &str) -> i64 {
    // Parse HH:MM or ISO datetime, return minutes from midnight
    // Before midnight (20:00-23:59) → negative (-240 to -1)
    // After midnight (00:00-06:00) → positive (0 to 360)
    let hm = if time_str.len() >= 16 { &time_str[11..16] } else { time_str };
    let parts: Vec<&str> = hm.split(':').collect();
    let h: i64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: i64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let total = h * 60 + m;
    if total >= 720 { total - 1440 } else { total } // 12:00+ = before midnight
}

fn min_to_time(min: i64) -> String {
    let m = if min < 0 { min + 1440 } else if min >= 1440 { min - 1440 } else { min };
    format!("{:02}:{:02}", m / 60, m % 60)
}
