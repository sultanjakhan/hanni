---
name: db-migrate
description: Create a new SQLite migration — table creation, column addition, or schema changes with Rust migration code and JS wrappers.
allowed-tools: Read, Edit, Grep, Glob, Task
argument-hint: <table_or_description>
user-invocable: true
---

# SQLite Migration

Create or modify SQLite tables in Hanni's database.

## Input

- Argument: table name or description of the migration (e.g., `recipes` or `add_priority_to_tasks`)

## Steps

1. **Read current DB patterns** in `lib.rs`:
   - Find the `init_db` or migration function
   - Study existing `CREATE TABLE` statements for naming conventions, types, constraints
   - Note the migration versioning pattern (if any)

2. **Design the schema**:
   - Table name: snake_case, plural (e.g., `recipes`, `workout_sessions`)
   - Always include `id INTEGER PRIMARY KEY AUTOINCREMENT`
   - Always include `created_at TEXT DEFAULT (datetime('now'))` and `updated_at TEXT`
   - Use appropriate SQLite types: TEXT, INTEGER, REAL, BLOB
   - Add proper indexes for frequently queried columns
   - Add foreign keys where appropriate

3. **Add migration in Rust** (`lib.rs`):
   - Add `CREATE TABLE IF NOT EXISTS` in the migration section
   - Add any indexes
   - If altering existing table, use `ALTER TABLE`

4. **Create Rust CRUD commands**:
   - `add_{entity}` — INSERT
   - `get_{entities}` — SELECT all (with optional filters)
   - `get_{entity}` — SELECT by id
   - `update_{entity}` — UPDATE by id
   - `delete_{entity}` — DELETE by id
   - Register all in `invoke_handler`

5. **Create JS wrappers** in `main.js`:
   - Matching async functions for each CRUD command
   - Follow existing invoke patterns

6. **Report** the full schema and created commands

## Rules

- Respond in Russian
- Match existing DB patterns exactly
- Use parameterized queries (?) — NEVER string interpolation for SQL
- Consider FTS5 if full-text search will be needed on the table
- Keep schema simple — don't over-normalize
- SQLite uses `std::sync::Mutex` (not async Mutex) because rusqlite is not Send
