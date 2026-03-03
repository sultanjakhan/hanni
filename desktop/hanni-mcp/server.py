#!/usr/bin/env python3
"""
Hanni MCP Server — full CRUD access to Hanni's SQLite database.

Tools:
  READ:
    - get_facts: Read memory facts (optionally filtered)
    - get_conversations: List recent conversations
    - get_conversation: Read a specific conversation's messages
    - get_proactive_history: Recent proactive messages
    - get_settings: App settings
    - run_sql: Run a read-only SQL query
    - get_events: Get calendar events by date range
    - get_transactions: Get financial transactions
    - search_notes: Search notes/tasks

  WRITE — Media:
    - add_media_item: Add anime/manga/movie/series/game/book/music
    - update_media_item: Update media item fields
    - delete_media_item: Delete media item

  WRITE — Calendar:
    - create_event: Create calendar event
    - update_event: Update event fields
    - delete_event: Delete event

  WRITE — Notes & Tasks:
    - create_note: Create a note or task
    - update_note: Update note/task fields
    - complete_task: Mark a task as done

  WRITE — Finance:
    - add_transaction: Add income/expense transaction

  WRITE — Memory:
    - remember: Store a fact (category/key/value)
    - forget: Delete a fact by category+key

  WRITE — Health:
    - log_health: Log health data (sleep, water, steps, weight)
    - log_mood: Log mood (1-5) with optional note

  WRITE — General:
    - run_sql_write: INSERT/UPDATE/DELETE with table whitelist
"""
import json, os, sys, sqlite3, time
from datetime import datetime

DB_PATH = os.path.expanduser("~/Library/Application Support/Hanni/hanni.db")

WRITE_ALLOWED_TABLES = {
    "media_items", "events", "notes", "tasks", "transactions",
    "facts", "health_log", "mood_log", "habits", "habit_checks",
    "tab_goals", "projects", "contacts", "reminders",
    "hobbies", "hobby_entries", "journal_entries",
}


def now_iso():
    return datetime.now().strftime("%Y-%m-%dT%H:%M:%S")


def today_iso():
    return datetime.now().strftime("%Y-%m-%d")


def now_time():
    return datetime.now().strftime("%H:%M")


def get_db():
    conn = sqlite3.connect(DB_PATH, timeout=5)
    conn.row_factory = sqlite3.Row
    return conn


# ─── READ tools ──────────────────────────────────────────────────────────────

def handle_get_facts(args):
    conn = get_db()
    search = args.get("search", "")
    if search:
        like = f"%{search}%"
        rows = conn.execute(
            "SELECT id, category, key, value, source, updated_at FROM facts "
            "WHERE key LIKE ? OR value LIKE ? OR category LIKE ? "
            "ORDER BY updated_at DESC LIMIT 100",
            (like, like, like)
        ).fetchall()
    else:
        rows = conn.execute(
            "SELECT id, category, key, value, source, updated_at FROM facts "
            "ORDER BY category, updated_at DESC LIMIT 200"
        ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_get_conversations(args):
    conn = get_db()
    limit = args.get("limit", 20)
    rows = conn.execute(
        "SELECT id, summary, message_count, started_at, ended_at FROM conversations "
        "ORDER BY id DESC LIMIT ?", (limit,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_get_conversation(args):
    conn = get_db()
    cid = args.get("id")
    if not cid:
        return {"error": "id required"}
    row = conn.execute(
        "SELECT id, summary, messages, message_count, started_at FROM conversations WHERE id=?",
        (cid,)
    ).fetchone()
    conn.close()
    if not row:
        return {"error": "not found"}
    result = dict(row)
    try:
        result["messages"] = json.loads(result["messages"])
    except Exception:
        pass
    return result


def handle_get_proactive_history(args):
    conn = get_db()
    limit = args.get("limit", 30)
    rows = conn.execute(
        "SELECT id, message, sent_at, user_replied, reply_delay_secs FROM proactive_history "
        "ORDER BY id DESC LIMIT ?", (limit,)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_get_settings(args):
    conn = get_db()
    rows = conn.execute("SELECT key, value FROM app_settings").fetchall()
    conn.close()
    return {r["key"]: r["value"] for r in rows}


def handle_run_sql(args):
    query = args.get("query", "")
    if not query:
        return {"error": "query required"}
    if not query.strip().upper().startswith("SELECT"):
        return {"error": "only SELECT queries allowed — use run_sql_write for mutations"}
    conn = get_db()
    try:
        rows = conn.execute(query).fetchall()
        return [dict(r) for r in rows]
    except Exception as e:
        return {"error": str(e)}
    finally:
        conn.close()


def handle_get_events(args):
    conn = get_db()
    date_from = args.get("date_from", today_iso())
    date_to = args.get("date_to", date_from)
    rows = conn.execute(
        "SELECT id, title, description, date, time, duration_minutes, category, color, completed "
        "FROM events WHERE date >= ? AND date <= ? ORDER BY date, time",
        (date_from, date_to)
    ).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_get_transactions(args):
    conn = get_db()
    date_from = args.get("date_from")
    date_to = args.get("date_to")
    category = args.get("category")
    limit = args.get("limit", 50)

    query = "SELECT id, date, type, amount, currency, category, description FROM transactions WHERE 1=1"
    params = []
    if date_from:
        query += " AND date >= ?"
        params.append(date_from)
    if date_to:
        query += " AND date <= ?"
        params.append(date_to)
    if category:
        query += " AND category = ?"
        params.append(category)
    query += " ORDER BY date DESC, id DESC LIMIT ?"
    params.append(limit)

    rows = conn.execute(query, params).fetchall()
    conn.close()
    return [dict(r) for r in rows]


def handle_search_notes(args):
    conn = get_db()
    query = args.get("query", "")
    tab = args.get("tab")
    status = args.get("status")
    limit = args.get("limit", 30)

    if query:
        sql = (
            "SELECT n.id, n.title, n.content, n.tags, n.tab_name, n.status, n.pinned, n.due_date, n.updated_at "
            "FROM notes n JOIN notes_fts f ON n.id = f.rowid "
            "WHERE notes_fts MATCH ? "
        )
        params = [query]
    else:
        sql = (
            "SELECT id, title, content, tags, tab_name, status, pinned, due_date, updated_at "
            "FROM notes WHERE 1=1 "
        )
        params = []

    if tab:
        sql += " AND tab_name = ?"
        params.append(tab)
    if status:
        sql += " AND status = ?"
        params.append(status)
    sql += " ORDER BY pinned DESC, updated_at DESC LIMIT ?"
    params.append(limit)

    rows = conn.execute(sql, params).fetchall()
    conn.close()
    return [dict(r) for r in rows]


# ─── WRITE — Media ──────────────────────────────────────────────────────────

def handle_add_media_item(args):
    conn = get_db()
    now = now_iso()
    cur = conn.execute(
        "INSERT INTO media_items (media_type, title, original_title, year, description, cover_url, "
        "status, rating, progress, total_episodes, notes, created_at, updated_at) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            args.get("media_type", "anime"),
            args.get("title", ""),
            args.get("original_title", ""),
            args.get("year"),
            args.get("description", ""),
            args.get("cover_url", ""),
            args.get("status", "planned"),
            args.get("rating", 0),
            args.get("progress", 0),
            args.get("total_episodes"),
            args.get("notes", ""),
            now, now,
        )
    )
    item_id = cur.lastrowid
    conn.commit()
    conn.close()
    return {"id": item_id, "status": "created"}


def handle_update_media_item(args):
    item_id = args.get("id")
    if not item_id:
        return {"error": "id required"}

    fields = {}
    for key in ("title", "original_title", "year", "description", "cover_url",
                "status", "rating", "progress", "total_episodes", "notes", "media_type"):
        if key in args:
            fields[key] = args[key]

    if not fields:
        return {"error": "no fields to update"}

    fields["updated_at"] = now_iso()
    set_clause = ", ".join(f"{k} = ?" for k in fields)
    values = list(fields.values()) + [item_id]

    conn = get_db()
    conn.execute(f"UPDATE media_items SET {set_clause} WHERE id = ?", values)
    conn.commit()
    conn.close()
    return {"id": item_id, "status": "updated", "fields": list(fields.keys())}


def handle_delete_media_item(args):
    item_id = args.get("id")
    if not item_id:
        return {"error": "id required"}
    conn = get_db()
    conn.execute("DELETE FROM media_items WHERE id = ?", (item_id,))
    conn.commit()
    conn.close()
    return {"id": item_id, "status": "deleted"}


# ─── WRITE — Calendar ───────────────────────────────────────────────────────

def handle_create_event(args):
    conn = get_db()
    cur = conn.execute(
        "INSERT INTO events (title, description, date, time, duration_minutes, category, color, created_at) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            args.get("title", ""),
            args.get("description", ""),
            args.get("date", today_iso()),
            args.get("time", ""),
            args.get("duration_minutes", 60),
            args.get("category", "general"),
            args.get("color", "#818cf8"),
            now_iso(),
        )
    )
    event_id = cur.lastrowid
    conn.commit()
    conn.close()
    return {"id": event_id, "status": "created"}


def handle_update_event(args):
    event_id = args.get("id")
    if not event_id:
        return {"error": "id required"}

    fields = {}
    for key in ("title", "description", "date", "time", "duration_minutes",
                "category", "color", "completed"):
        if key in args:
            fields[key] = args[key]

    if not fields:
        return {"error": "no fields to update"}

    set_clause = ", ".join(f"{k} = ?" for k in fields)
    values = list(fields.values()) + [event_id]

    conn = get_db()
    conn.execute(f"UPDATE events SET {set_clause} WHERE id = ?", values)
    conn.commit()
    conn.close()
    return {"id": event_id, "status": "updated", "fields": list(fields.keys())}


def handle_delete_event(args):
    event_id = args.get("id")
    if not event_id:
        return {"error": "id required"}
    conn = get_db()
    conn.execute("DELETE FROM events WHERE id = ?", (event_id,))
    conn.commit()
    conn.close()
    return {"id": event_id, "status": "deleted"}


# ─── WRITE — Notes & Tasks ──────────────────────────────────────────────────

def handle_create_note(args):
    conn = get_db()
    now = now_iso()
    cur = conn.execute(
        "INSERT INTO notes (title, content, tags, tab_name, status, pinned, due_date, color, created_at, updated_at) "
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            args.get("title", ""),
            args.get("content", ""),
            args.get("tags", ""),
            args.get("tab", None),
            args.get("status", "note"),
            args.get("pinned", 0),
            args.get("due_date"),
            args.get("color"),
            now, now,
        )
    )
    note_id = cur.lastrowid
    conn.commit()
    conn.close()
    return {"id": note_id, "status": "created"}


def handle_update_note(args):
    note_id = args.get("id")
    if not note_id:
        return {"error": "id required"}

    fields = {}
    for key in ("title", "content", "tags", "tab_name", "status", "pinned",
                "due_date", "color", "archived"):
        if key in args:
            fields[key] = args[key]
    # Allow "tab" as alias for "tab_name"
    if "tab" in args and "tab_name" not in fields:
        fields["tab_name"] = args["tab"]

    if not fields:
        return {"error": "no fields to update"}

    fields["updated_at"] = now_iso()
    set_clause = ", ".join(f"{k} = ?" for k in fields)
    values = list(fields.values()) + [note_id]

    conn = get_db()
    conn.execute(f"UPDATE notes SET {set_clause} WHERE id = ?", values)
    conn.commit()
    conn.close()
    return {"id": note_id, "status": "updated", "fields": list(fields.keys())}


def handle_complete_task(args):
    task_id = args.get("id")
    if not task_id:
        return {"error": "id required"}
    conn = get_db()
    now = now_iso()
    # Try notes table first (status-based tasks)
    cur = conn.execute("UPDATE notes SET status = 'done', updated_at = ? WHERE id = ? AND status = 'task'", (now, task_id))
    if cur.rowcount > 0:
        conn.commit()
        conn.close()
        return {"id": task_id, "table": "notes", "status": "completed"}
    # Try tasks table (project tasks)
    cur = conn.execute("UPDATE tasks SET status = 'done', completed_at = ? WHERE id = ?", (now, task_id))
    if cur.rowcount > 0:
        conn.commit()
        conn.close()
        return {"id": task_id, "table": "tasks", "status": "completed"}
    conn.close()
    return {"error": f"task {task_id} not found"}


# ─── WRITE — Finance ────────────────────────────────────────────────────────

def handle_add_transaction(args):
    conn = get_db()
    cur = conn.execute(
        "INSERT INTO transactions (date, type, amount, currency, category, description, created_at) "
        "VALUES (?, ?, ?, ?, ?, ?, ?)",
        (
            args.get("date", today_iso()),
            args.get("type", "expense"),
            args.get("amount", 0),
            args.get("currency", "KZT"),
            args.get("category", "other"),
            args.get("description", ""),
            now_iso(),
        )
    )
    tx_id = cur.lastrowid
    conn.commit()
    conn.close()
    return {"id": tx_id, "status": "created"}


# ─── WRITE — Memory ─────────────────────────────────────────────────────────

def handle_remember(args):
    category = args.get("category", "general")
    key = args.get("key", "")
    value = args.get("value", "")
    if not key or not value:
        return {"error": "key and value required"}
    conn = get_db()
    now = now_iso()
    conn.execute(
        "INSERT INTO facts (category, key, value, source, created_at, updated_at) "
        "VALUES (?, ?, ?, 'ai', ?, ?) "
        "ON CONFLICT(category, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        (category, key, value, now, now)
    )
    conn.commit()
    conn.close()
    return {"category": category, "key": key, "status": "remembered"}


def handle_forget(args):
    category = args.get("category", "general")
    key = args.get("key", "")
    if not key:
        return {"error": "key required"}
    conn = get_db()
    cur = conn.execute("DELETE FROM facts WHERE category = ? AND key = ?", (category, key))
    conn.commit()
    conn.close()
    if cur.rowcount > 0:
        return {"category": category, "key": key, "status": "forgotten"}
    return {"error": f"fact '{category}/{key}' not found"}


# ─── WRITE — Health ──────────────────────────────────────────────────────────

def handle_log_health(args):
    conn = get_db()
    entries = []
    now = now_iso()
    date = args.get("date", today_iso())

    for metric in ("sleep", "water", "steps", "weight"):
        if metric in args:
            units = {"sleep": "hours", "water": "ml", "steps": "steps", "weight": "kg"}
            conn.execute(
                "INSERT INTO health_log (date, type, value, unit, notes, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                (date, metric, args[metric], units[metric], args.get("notes", ""), now)
            )
            entries.append(metric)

    conn.commit()
    conn.close()
    if not entries:
        return {"error": "provide at least one metric: sleep, water, steps, weight"}
    return {"date": date, "logged": entries, "status": "created"}


def handle_log_mood(args):
    mood = args.get("mood")
    if mood is None:
        return {"error": "mood (1-5) required"}
    conn = get_db()
    now = now_iso()
    conn.execute(
        "INSERT INTO mood_log (date, time, mood, note, trigger_text, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        (
            args.get("date", today_iso()),
            args.get("time", now_time()),
            mood,
            args.get("note", ""),
            args.get("trigger", ""),
            now,
        )
    )
    conn.commit()
    conn.close()
    return {"mood": mood, "status": "logged"}


# ─── WRITE — General SQL ────────────────────────────────────────────────────

def handle_run_sql_write(args):
    query = args.get("query", "").strip()
    if not query:
        return {"error": "query required"}

    prefix = query.upper().split()[0] if query.split() else ""
    if prefix not in ("INSERT", "UPDATE", "DELETE"):
        return {"error": "only INSERT/UPDATE/DELETE allowed — use run_sql for SELECT"}

    # Check table whitelist
    query_upper = query.upper()
    table_found = False
    for table in WRITE_ALLOWED_TABLES:
        if table.upper() in query_upper:
            table_found = True
            break
    if not table_found:
        return {"error": f"table not in whitelist. Allowed: {', '.join(sorted(WRITE_ALLOWED_TABLES))}"}

    conn = get_db()
    try:
        cur = conn.execute(query)
        conn.commit()
        return {"rows_affected": cur.rowcount, "lastrowid": cur.lastrowid}
    except Exception as e:
        return {"error": str(e)}
    finally:
        conn.close()


# ─── Tool registry ──────────────────────────────────────────────────────────

TOOLS = {
    # READ
    "get_facts": {
        "description": "Get memory facts from Hanni's database. Optionally filter by search term.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "search": {"type": "string", "description": "Optional search filter"}
            }
        },
        "handler": handle_get_facts,
    },
    "get_conversations": {
        "description": "List recent chat conversations with summaries.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "limit": {"type": "number", "description": "Max results (default 20)"}
            }
        },
        "handler": handle_get_conversations,
    },
    "get_conversation": {
        "description": "Get full conversation messages by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Conversation ID"}
            },
            "required": ["id"]
        },
        "handler": handle_get_conversation,
    },
    "get_proactive_history": {
        "description": "Get recent proactive messages sent by Hanni.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "limit": {"type": "number", "description": "Max results (default 30)"}
            }
        },
        "handler": handle_get_proactive_history,
    },
    "get_settings": {
        "description": "Get all Hanni app settings.",
        "inputSchema": {"type": "object", "properties": {}},
        "handler": handle_get_settings,
    },
    "run_sql": {
        "description": "Run a read-only SELECT query on hanni.db. Supports all tables.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "SQL SELECT query"}
            },
            "required": ["query"]
        },
        "handler": handle_run_sql,
    },
    "get_events": {
        "description": "Get calendar events by date range. Defaults to today.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "date_from": {"type": "string", "description": "Start date (YYYY-MM-DD), default today"},
                "date_to": {"type": "string", "description": "End date (YYYY-MM-DD), default same as date_from"}
            }
        },
        "handler": handle_get_events,
    },
    "get_transactions": {
        "description": "Get financial transactions. Filter by dates and category.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "date_from": {"type": "string", "description": "Start date (YYYY-MM-DD)"},
                "date_to": {"type": "string", "description": "End date (YYYY-MM-DD)"},
                "category": {"type": "string", "description": "Filter by category"},
                "limit": {"type": "number", "description": "Max results (default 50)"}
            }
        },
        "handler": handle_get_transactions,
    },
    "search_notes": {
        "description": "Search notes and tasks by full-text query. Filter by tab and status.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Full-text search query"},
                "tab": {"type": "string", "description": "Filter by tab name"},
                "status": {"type": "string", "description": "Filter by status (note/task/done)"},
                "limit": {"type": "number", "description": "Max results (default 30)"}
            }
        },
        "handler": handle_search_notes,
    },

    # WRITE — Media
    "add_media_item": {
        "description": "Add a media item (anime, manga, movie, series, game, book, music, cartoon, podcast).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "media_type": {"type": "string", "description": "Type: anime/manga/movie/series/game/book/music/cartoon/podcast"},
                "title": {"type": "string", "description": "Title in Russian or original"},
                "original_title": {"type": "string", "description": "Original title (if different)"},
                "year": {"type": "number", "description": "Release year"},
                "description": {"type": "string", "description": "Short description"},
                "cover_url": {"type": "string", "description": "Cover image URL"},
                "status": {"type": "string", "description": "Status: planned/watching/completed/dropped (default: planned)"},
                "rating": {"type": "number", "description": "Rating 0-10"},
                "total_episodes": {"type": "number", "description": "Total episodes/chapters"},
                "notes": {"type": "string", "description": "Personal notes"}
            },
            "required": ["media_type", "title"]
        },
        "handler": handle_add_media_item,
    },
    "update_media_item": {
        "description": "Update a media item by ID. Pass only fields to change.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Media item ID"},
                "title": {"type": "string"},
                "status": {"type": "string"},
                "rating": {"type": "number"},
                "progress": {"type": "number"},
                "notes": {"type": "string"},
                "cover_url": {"type": "string"}
            },
            "required": ["id"]
        },
        "handler": handle_update_media_item,
    },
    "delete_media_item": {
        "description": "Delete a media item by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Media item ID"}
            },
            "required": ["id"]
        },
        "handler": handle_delete_media_item,
    },

    # WRITE — Calendar
    "create_event": {
        "description": "Create a calendar event.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Event title"},
                "date": {"type": "string", "description": "Date YYYY-MM-DD (default today)"},
                "time": {"type": "string", "description": "Time HH:MM"},
                "duration_minutes": {"type": "number", "description": "Duration in minutes (default 60)"},
                "category": {"type": "string", "description": "Category (default: general)"},
                "description": {"type": "string", "description": "Description"},
                "color": {"type": "string", "description": "Color hex (default: #818cf8)"}
            },
            "required": ["title"]
        },
        "handler": handle_create_event,
    },
    "update_event": {
        "description": "Update a calendar event by ID. Pass only fields to change.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Event ID"},
                "title": {"type": "string"},
                "date": {"type": "string"},
                "time": {"type": "string"},
                "duration_minutes": {"type": "number"},
                "category": {"type": "string"},
                "description": {"type": "string"},
                "completed": {"type": "number", "description": "1 = completed, 0 = not completed"}
            },
            "required": ["id"]
        },
        "handler": handle_update_event,
    },
    "delete_event": {
        "description": "Delete a calendar event by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Event ID"}
            },
            "required": ["id"]
        },
        "handler": handle_delete_event,
    },

    # WRITE — Notes & Tasks
    "create_note": {
        "description": "Create a note or task. Set status='task' for tasks.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Note/task title"},
                "content": {"type": "string", "description": "Content body"},
                "tags": {"type": "string", "description": "Comma-separated tags"},
                "tab": {"type": "string", "description": "Tab name to associate with"},
                "status": {"type": "string", "description": "Status: note/task/done (default: note)"},
                "pinned": {"type": "number", "description": "1 = pinned"},
                "due_date": {"type": "string", "description": "Due date YYYY-MM-DD (for tasks)"},
                "color": {"type": "string", "description": "Color hex"}
            },
            "required": ["title"]
        },
        "handler": handle_create_note,
    },
    "update_note": {
        "description": "Update a note or task by ID. Pass only fields to change.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Note/task ID"},
                "title": {"type": "string"},
                "content": {"type": "string"},
                "tags": {"type": "string"},
                "tab": {"type": "string"},
                "status": {"type": "string"},
                "pinned": {"type": "number"},
                "due_date": {"type": "string"},
                "color": {"type": "string"},
                "archived": {"type": "number"}
            },
            "required": ["id"]
        },
        "handler": handle_update_note,
    },
    "complete_task": {
        "description": "Mark a task as completed (works for both notes-tasks and project-tasks).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "number", "description": "Task ID"}
            },
            "required": ["id"]
        },
        "handler": handle_complete_task,
    },

    # WRITE — Finance
    "add_transaction": {
        "description": "Add a financial transaction (expense or income).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "amount": {"type": "number", "description": "Amount (positive number)"},
                "type": {"type": "string", "description": "Type: expense/income (default: expense)"},
                "category": {"type": "string", "description": "Category (food, transport, entertainment, salary, etc.)"},
                "description": {"type": "string", "description": "Description"},
                "currency": {"type": "string", "description": "Currency code (default: KZT)"},
                "date": {"type": "string", "description": "Date YYYY-MM-DD (default: today)"}
            },
            "required": ["amount"]
        },
        "handler": handle_add_transaction,
    },

    # WRITE — Memory
    "remember": {
        "description": "Store a fact in long-term memory. Upserts: overwrites existing fact with same category+key.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "category": {"type": "string", "description": "Fact category (preferences, personal, work, etc.)"},
                "key": {"type": "string", "description": "Fact key/name"},
                "value": {"type": "string", "description": "Fact value"}
            },
            "required": ["key", "value"]
        },
        "handler": handle_remember,
    },
    "forget": {
        "description": "Delete a fact from long-term memory by category and key.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "category": {"type": "string", "description": "Fact category"},
                "key": {"type": "string", "description": "Fact key"}
            },
            "required": ["key"]
        },
        "handler": handle_forget,
    },

    # WRITE — Health
    "log_health": {
        "description": "Log health metrics. Provide at least one: sleep (hours), water (ml), steps, weight (kg).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "sleep": {"type": "number", "description": "Hours slept"},
                "water": {"type": "number", "description": "Water in ml"},
                "steps": {"type": "number", "description": "Step count"},
                "weight": {"type": "number", "description": "Weight in kg"},
                "date": {"type": "string", "description": "Date YYYY-MM-DD (default: today)"},
                "notes": {"type": "string", "description": "Additional notes"}
            }
        },
        "handler": handle_log_health,
    },
    "log_mood": {
        "description": "Log current mood (1-5 scale) with optional note and trigger.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "mood": {"type": "number", "description": "Mood level 1-5 (1=terrible, 5=great)"},
                "note": {"type": "string", "description": "Optional note about how you feel"},
                "trigger": {"type": "string", "description": "What caused this mood"},
                "date": {"type": "string", "description": "Date YYYY-MM-DD (default: today)"},
                "time": {"type": "string", "description": "Time HH:MM (default: now)"}
            },
            "required": ["mood"]
        },
        "handler": handle_log_mood,
    },

    # WRITE — General
    "run_sql_write": {
        "description": "Run INSERT/UPDATE/DELETE on whitelisted tables. For advanced mutations not covered by specific tools.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "SQL INSERT/UPDATE/DELETE query"}
            },
            "required": ["query"]
        },
        "handler": handle_run_sql_write,
    },
}


# ─── MCP protocol ───────────────────────────────────────────────────────────

def send_response(id, result):
    response = {"jsonrpc": "2.0", "id": id, "result": result}
    msg = json.dumps(response)
    sys.stdout.write(f"Content-Length: {len(msg.encode())}\r\n\r\n{msg}")
    sys.stdout.flush()


def send_error(id, code, message):
    response = {"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}
    msg = json.dumps(response)
    sys.stdout.write(f"Content-Length: {len(msg.encode())}\r\n\r\n{msg}")
    sys.stdout.flush()


def main():
    """MCP server over stdio using JSON-RPC."""
    buf = ""
    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break

            if line.startswith("Content-Length:"):
                length = int(line.split(":")[1].strip())
                sys.stdin.readline()  # empty line
                body = sys.stdin.read(length)
                request = json.loads(body)
            else:
                buf += line
                try:
                    request = json.loads(buf)
                    buf = ""
                except json.JSONDecodeError:
                    continue

            method = request.get("method", "")
            id = request.get("id")
            params = request.get("params", {})

            if method == "initialize":
                send_response(id, {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "hanni-mcp", "version": "2.0.0"},
                })
            elif method == "notifications/initialized":
                pass
            elif method == "tools/list":
                tools_list = []
                for name, tool in TOOLS.items():
                    tools_list.append({
                        "name": name,
                        "description": tool["description"],
                        "inputSchema": tool["inputSchema"],
                    })
                send_response(id, {"tools": tools_list})
            elif method == "tools/call":
                tool_name = params.get("name", "")
                tool_args = params.get("arguments", {})
                if tool_name in TOOLS:
                    try:
                        result = TOOLS[tool_name]["handler"](tool_args)
                        send_response(id, {
                            "content": [{"type": "text", "text": json.dumps(result, ensure_ascii=False, indent=2)}]
                        })
                    except Exception as e:
                        send_response(id, {
                            "content": [{"type": "text", "text": f"Error: {e}"}],
                            "isError": True,
                        })
                else:
                    send_error(id, -32601, f"Unknown tool: {tool_name}")
            elif method == "ping":
                send_response(id, {})
            else:
                if id is not None:
                    send_error(id, -32601, f"Method not found: {method}")

        except Exception as e:
            sys.stderr.write(f"MCP error: {e}\n")
            sys.stderr.flush()


if __name__ == "__main__":
    main()
