"""
HomeFit Integration for Clawd Agent
- Read exercises database
- Track workouts
- Check progress and streaks
- Sync with Notion (optional)
"""
import json
from pathlib import Path
from typing import List, Dict, Optional
from datetime import datetime, timedelta

# HomeFit project path
HOMEFIT_PATH = Path.home() / "Documents" / "homefit-pro-2"
DATA_PATH = HOMEFIT_PATH / "data"
AGENT_DATA_PATH = Path(__file__).parent / "homefit_data"
AGENT_DATA_PATH.mkdir(exist_ok=True)

# User data file (synced with app's localStorage)
USER_DATA_FILE = AGENT_DATA_PATH / "user_data.json"


def _load_exercises() -> List[Dict]:
    """Load exercises from HomeFit data"""
    exercises_file = DATA_PATH / "exercises.ts"
    if not exercises_file.exists():
        return []

    content = exercises_file.read_text()
    # Simple extraction - find exercise objects
    exercises = []
    import re

    # Extract exercise blocks
    pattern = r"\{\s*id:\s*'([^']+)'.*?name:\s*'([^']+)'.*?muscles:\s*\[([^\]]+)\].*?difficulty:\s*(\d+)"
    matches = re.findall(pattern, content, re.DOTALL)

    for match in matches:
        exercises.append({
            "id": match[0],
            "name": match[1],
            "muscles": [m.strip().strip("'") for m in match[2].split(",")],
            "difficulty": int(match[3])
        })

    return exercises


def _load_user_data() -> Dict:
    """Load user fitness data"""
    if USER_DATA_FILE.exists():
        return json.loads(USER_DATA_FILE.read_text())
    return {
        "completedWorkouts": [],
        "currentStreak": 0,
        "longestStreak": 0,
        "exerciseStats": {},
        "lastWorkoutDate": None
    }


def _save_user_data(data: Dict):
    """Save user fitness data"""
    USER_DATA_FILE.write_text(json.dumps(data, ensure_ascii=False, indent=2))


# === Tools ===

def get_exercises(muscle: str = None, difficulty: int = None, limit: int = 10) -> str:
    """
    Get exercises from HomeFit database

    Args:
        muscle: Filter by muscle group (e.g., 'chest', 'back', 'legs')
        difficulty: Filter by difficulty (1-5)
        limit: Max results to return
    """
    exercises = _load_exercises()

    if muscle:
        muscle = muscle.lower()
        exercises = [e for e in exercises if any(muscle in m.lower() for m in e["muscles"])]

    if difficulty:
        exercises = [e for e in exercises if e["difficulty"] == difficulty]

    exercises = exercises[:limit]

    if not exercises:
        return f"No exercises found" + (f" for muscle '{muscle}'" if muscle else "")

    result = []
    for e in exercises:
        result.append(f"• {e['name']} (difficulty: {e['difficulty']}, muscles: {', '.join(e['muscles'])})")

    return f"Found {len(exercises)} exercises:\n" + "\n".join(result)


def get_workout_stats() -> str:
    """Get user's workout statistics"""
    data = _load_user_data()

    stats = []
    stats.append(f"Current streak: {data.get('currentStreak', 0)} days")
    stats.append(f"Longest streak: {data.get('longestStreak', 0)} days")
    stats.append(f"Total workouts: {len(data.get('completedWorkouts', []))}")

    last = data.get('lastWorkoutDate')
    if last:
        stats.append(f"Last workout: {last}")

    # Top exercises
    ex_stats = data.get('exerciseStats', {})
    if ex_stats:
        top = sorted(ex_stats.items(), key=lambda x: x[1].get('count', 0), reverse=True)[:5]
        if top:
            stats.append("\nTop exercises:")
            for ex_id, ex_data in top:
                stats.append(f"  • {ex_id}: {ex_data.get('count', 0)} times")

    return "\n".join(stats)


def log_workout(exercises: List[str], duration_minutes: int = 30, notes: str = None) -> str:
    """
    Log a completed workout

    Args:
        exercises: List of exercise IDs or names
        duration_minutes: Workout duration in minutes
        notes: Optional notes about the workout
    """
    data = _load_user_data()
    today = datetime.now().strftime("%Y-%m-%d")

    # Create workout entry
    workout = {
        "date": today,
        "exercises": exercises,
        "duration": duration_minutes,
        "notes": notes,
        "timestamp": datetime.now().isoformat()
    }

    # Add to completed workouts
    if "completedWorkouts" not in data:
        data["completedWorkouts"] = []
    data["completedWorkouts"].append(workout)

    # Update streak
    last_date = data.get("lastWorkoutDate")
    if last_date:
        last = datetime.strptime(last_date, "%Y-%m-%d")
        today_dt = datetime.strptime(today, "%Y-%m-%d")
        diff = (today_dt - last).days

        if diff == 1:
            data["currentStreak"] = data.get("currentStreak", 0) + 1
        elif diff > 1:
            data["currentStreak"] = 1
        # diff == 0 means same day, don't change streak
    else:
        data["currentStreak"] = 1

    data["lastWorkoutDate"] = today

    # Update longest streak
    if data["currentStreak"] > data.get("longestStreak", 0):
        data["longestStreak"] = data["currentStreak"]

    # Update exercise stats
    if "exerciseStats" not in data:
        data["exerciseStats"] = {}
    for ex in exercises:
        if ex not in data["exerciseStats"]:
            data["exerciseStats"][ex] = {"count": 0, "lastDone": None}
        data["exerciseStats"][ex]["count"] += 1
        data["exerciseStats"][ex]["lastDone"] = today

    _save_user_data(data)

    return f"Workout logged! {len(exercises)} exercises, {duration_minutes} min.\nCurrent streak: {data['currentStreak']} days 🔥"


def get_workout_plan(day: str = None) -> str:
    """
    Get workout plan for a day

    Args:
        day: Day of week (monday, tuesday, etc.) or None for today
    """
    if day is None:
        day = datetime.now().strftime("%A").lower()

    # Default schedule
    schedule = {
        "monday": "workout",
        "tuesday": "recovery",
        "wednesday": "workout",
        "thursday": "recovery",
        "friday": "workout",
        "saturday": "recovery",
        "sunday": "rest"
    }

    day = day.lower()
    if day not in schedule:
        return f"Unknown day: {day}"

    workout_type = schedule[day]

    if workout_type == "rest":
        return f"📅 {day.capitalize()}: Rest day! Take it easy."
    elif workout_type == "recovery":
        return f"📅 {day.capitalize()}: Recovery day (15 min stretching/light cardio)"
    else:
        # Get some exercise suggestions
        exercises = _load_exercises()[:5]
        ex_list = "\n".join([f"  • {e['name']}" for e in exercises])
        return f"📅 {day.capitalize()}: Workout day (30 min)\n\nSuggested exercises:\n{ex_list}"


def search_exercises(query: str) -> str:
    """
    Search exercises by name or muscle

    Args:
        query: Search query (exercise name or muscle group)
    """
    exercises = _load_exercises()
    query = query.lower()

    matches = []
    for e in exercises:
        if query in e["name"].lower() or any(query in m.lower() for m in e["muscles"]):
            matches.append(e)

    if not matches:
        return f"No exercises found for '{query}'"

    matches = matches[:10]
    result = []
    for e in matches:
        result.append(f"• {e['name']} - {', '.join(e['muscles'])} (lvl {e['difficulty']})")

    return f"Found {len(matches)} exercises:\n" + "\n".join(result)


# === Tool Registry ===

HOMEFIT_TOOLS = {
    "get_exercises": {
        "func": get_exercises,
        "description": "Get exercises from HomeFit database, optionally filtered by muscle or difficulty",
        "parameters": {
            "type": "object",
            "properties": {
                "muscle": {"type": "string", "description": "Muscle group to filter (chest, back, legs, etc.)"},
                "difficulty": {"type": "integer", "description": "Difficulty level 1-5"},
                "limit": {"type": "integer", "description": "Max results (default 10)"}
            }
        }
    },
    "search_exercises": {
        "func": search_exercises,
        "description": "Search HomeFit exercises by name or muscle group",
        "parameters": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search query"}
            },
            "required": ["query"]
        }
    },
    "get_workout_stats": {
        "func": get_workout_stats,
        "description": "Get user's workout statistics (streaks, total workouts, etc.)",
        "parameters": {}
    },
    "log_workout": {
        "func": log_workout,
        "description": "Log a completed workout",
        "parameters": {
            "type": "object",
            "properties": {
                "exercises": {"type": "array", "items": {"type": "string"}, "description": "List of exercises done"},
                "duration_minutes": {"type": "integer", "description": "Workout duration in minutes"},
                "notes": {"type": "string", "description": "Optional notes"}
            },
            "required": ["exercises"]
        }
    },
    "get_workout_plan": {
        "func": get_workout_plan,
        "description": "Get workout plan for a specific day",
        "parameters": {
            "type": "object",
            "properties": {
                "day": {"type": "string", "description": "Day of week (monday, tuesday, etc.)"}
            }
        }
    }
}


def register_homefit_tools(agent):
    """Register HomeFit tools with an agent"""
    for name, tool in HOMEFIT_TOOLS.items():
        agent.register_tool(
            name=name,
            description=tool["description"],
            func=tool["func"],
            parameters=tool.get("parameters")
        )


# Quick test
if __name__ == "__main__":
    print("=== HomeFit Tools Test ===\n")

    print("Exercises (chest):")
    print(get_exercises(muscle="chest", limit=3))
    print()

    print("Search 'neck':")
    print(search_exercises("neck"))
    print()

    print("Workout plan for today:")
    print(get_workout_plan())
    print()

    print("Stats:")
    print(get_workout_stats())
