//! One-time seed of the exercise catalog from a bundled public-domain dataset
//! (free-exercise-db, Unlicense). Field + taxonomy mapping happens here at import
//! time, so the resulting categories live in the DB rather than as runtime
//! constants. Re-running is a no-op (gated by a `_migrations` flag + `INSERT OR
//! IGNORE` on the UNIQUE name).

use serde::Deserialize;

#[derive(Deserialize)]
struct SeedExercise {
    name: String,
    #[serde(default)]
    force: String,
    #[serde(default)]
    level: String,
    #[serde(default)]
    equipment: String,
    #[serde(default)]
    primary_muscles: Vec<String>,
    #[serde(default)]
    secondary_muscles: Vec<String>,
    #[serde(default)]
    category: String,
    #[serde(default)]
    instructions: Vec<String>,
}

/// Dataset `level` → the templates' easy/medium/hard vocabulary.
fn map_difficulty(level: &str) -> &'static str {
    match level {
        "beginner" => "easy",
        "intermediate" => "medium",
        "expert" => "hard",
        _ => "medium",
    }
}

/// The dataset's 17 primary muscles → the app's 8 muscle groups.
fn map_muscle_group(primary: &str) -> &'static str {
    match primary {
        "chest" => "chest",
        "shoulders" => "shoulders",
        "biceps" | "forearms" => "biceps",
        "triceps" => "triceps",
        "abdominals" => "core",
        "lats" | "middle back" | "lower back" | "traps" | "neck" => "back",
        "quadriceps" | "hamstrings" | "glutes" | "calves" | "adductors" | "abductors" => "legs",
        _ => "full_body",
    }
}

/// Dataset `category` → the app's 4 exercise types. Body-weight strength work
/// (no/own-body equipment) is surfaced as the dedicated `bodyweight` type so
/// that chip isn't empty (the dataset has no bodyweight category of its own).
fn map_type(category: &str, equipment: &str) -> &'static str {
    match category {
        "stretching" => "stretch",
        "plyometrics" | "cardio" => "cardio",
        "strength" | "strongman" | "powerlifting" | "olympic weightlifting" => {
            if equipment.is_empty() || equipment == "body only" {
                "bodyweight"
            } else {
                "strength"
            }
        }
        _ => "strength",
    }
}

pub fn seed_exercise_catalog(conn: &rusqlite::Connection) {
    let done = conn
        .prepare("SELECT 1 FROM _migrations WHERE name='seed_exercise_catalog_v1'")
        .ok()
        .and_then(|mut s| s.query_row([], |_| Ok(())).ok())
        .is_some();
    if done {
        return;
    }
    let _ = conn.execute("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY)", []);

    let raw = include_str!("sports_seed/exercises.json");
    let items: Vec<SeedExercise> = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return, // malformed bundle → skip seeding, never block startup
    };

    let _ = conn.execute_batch("BEGIN");
    if let Ok(mut stmt) = conn.prepare(
        "INSERT OR IGNORE INTO exercise_catalog \
         (name, muscle_group, equipment, type, difficulty, \
          primary_muscles, secondary_muscles, category, force, description) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
    ) {
        for ex in &items {
            let equip = ex.equipment.trim().to_lowercase();
            let mg = map_muscle_group(ex.primary_muscles.first().map(|s| s.as_str()).unwrap_or(""));
            let typ = map_type(&ex.category, &equip);
            let diff = map_difficulty(&ex.level);
            let _ = stmt.execute(rusqlite::params![
                ex.name.trim(),
                mg,
                equip,
                typ,
                diff,
                ex.primary_muscles.join(","),
                ex.secondary_muscles.join(","),
                ex.category,
                ex.force,
                ex.instructions.join("\n"),
            ]);
        }
    }
    let _ = conn.execute_batch("COMMIT");
    let _ = conn.execute(
        "INSERT OR IGNORE INTO _migrations (name) VALUES ('seed_exercise_catalog_v1')",
        [],
    );
}
