package com.sultanjakhan.hanni

import android.database.Cursor
import android.database.sqlite.SQLiteDatabase
import org.json.JSONArray
import org.json.JSONObject

/** SQLite codec for Android's background LAN client. Keep TABLES in db.rs order. */
internal object LanSyncDatabase {
    private const val EPOCH = "1970-01-01T00:00:00"
    private const val LIMIT = 500
    private const val CURSOR_PREFIX = "background_lan_cursor_"

    private val TABLES = listOf(
        "facts", "conversations", "activities", "notes", "events", "projects", "tasks",
        "learning_items", "hobbies", "hobby_entries", "workouts", "exercises", "health_log",
        "habits", "habit_checks", "media_items", "user_lists", "list_items", "food_log",
        "recipes", "products", "transactions", "budgets", "savings_goals", "subscriptions",
        "debts", "blocklist", "tab_goals", "home_items", "contacts", "contact_blocks",
        "property_definitions", "property_values", "view_configs", "activity_snapshots",
        "proactive_history", "message_feedback", "conversation_insights", "reminders",
        "flywheel_cycles", "schedules", "schedule_completions", "dan_koe_entries",
        "proactive_messages", "project_records", "body_records", "job_sources", "job_roles",
        "job_vacancies", "job_search_log", "dashboard_widgets", "timeline_activity_types",
        "timeline_blocks", "timeline_goals", "sleep_sessions", "sleep_stages",
        "heart_rate_samples", "event_categories", "routine_chains", "routine_nodes",
        "routine_edges", "routine_runs", "routine_node_status",
    )
    private val allowed = TABLES.toHashSet()

    data class Outbound(
        val cursors: JSONObject,
        val tombCursor: String,
        val rows: JSONArray,
        val tombs: JSONArray,
        val rowMax: MutableMap<String, String>,
        var tombMax: String,
    )

    fun gather(db: SQLiteDatabase): Outbound {
        val cursors = JSONObject()
        val rows = JSONArray()
        val rowMax = linkedMapOf<String, String>()
        for (table in TABLES) {
            // Bootstrap from the foreground Rust cursor so installing this
            // worker does not replay the device's entire already-synced DB.
            // Old Health-only worker cursors can be even newer for HC tables.
            val since = maxOf(
                readSetting(db, "$CURSOR_PREFIX$table") ?: EPOCH,
                readSetting(db, "lan_cursor_$table") ?: EPOCH,
                readSetting(db, "health_worker_push_cursor_$table") ?: EPOCH,
            )
            cursors.put(table, since)
            runCatching {
                db.rawQuery(
                    "SELECT * FROM $table WHERE updated_at>? ORDER BY updated_at,CAST(id AS TEXT) LIMIT $LIMIT",
                    arrayOf(since),
                ).use { cursor ->
                    while (cursor.moveToNext()) {
                        val fields = cursor.toJson()
                        val updated = fields.optString("updated_at")
                        fields.put("_updated_at", updated)
                        rows.put(JSONObject().put("t", table).put("f", fields))
                        if (updated > (rowMax[table] ?: "")) rowMax[table] = updated
                    }
                }
            }
        }
        val tombCursor = maxOf(
            readSetting(db, "${CURSOR_PREFIX}tombstones") ?: EPOCH,
            readSetting(db, "lan_cursor_tombstones") ?: EPOCH,
        )
        val tombs = JSONArray()
        var tombMax = ""
        db.rawQuery(
            "SELECT table_name,row_id,deleted_at FROM sync_tombstones " +
                "WHERE deleted_at>? ORDER BY deleted_at LIMIT $LIMIT",
            arrayOf(tombCursor),
        ).use { cursor ->
            while (cursor.moveToNext()) {
                val table = cursor.getString(0)
                if (table !in allowed) continue
                val deleted = cursor.getString(2)
                tombs.put(JSONObject().put("tt", table).put("id", cursor.getString(1)).put("deleted_at", deleted))
                if (deleted > tombMax) tombMax = deleted
            }
        }
        return Outbound(cursors, tombCursor, rows, tombs, rowMax, tombMax)
    }

    fun applyResponse(db: SQLiteDatabase, outbound: Outbound, response: JSONObject): Int {
        val rows = response.optJSONArray("rows") ?: JSONArray()
        val tombs = response.optJSONArray("tombs") ?: JSONArray()
        val receivedMax = linkedMapOf<String, String>()
        var receivedTombMax = ""
        var applied = 0
        db.beginTransaction()
        try {
            for (i in 0 until rows.length()) {
                val item = rows.optJSONObject(i) ?: continue
                val table = item.optString("t")
                val fields = item.optJSONObject("f") ?: continue
                if (table !in allowed) continue
                val updated = fields.optString("_updated_at", fields.optString("updated_at"))
                if (updated > (receivedMax[table] ?: "")) receivedMax[table] = updated
                if (upsertRow(db, table, fields, updated)) applied++
            }
            for (i in 0 until tombs.length()) {
                val tomb = tombs.optJSONObject(i) ?: continue
                val table = tomb.optString("tt")
                val id = tomb.opt("id") ?: continue
                val deleted = tomb.optString("deleted_at")
                if (table !in allowed || deleted.isBlank()) continue
                if (deleted > receivedTombMax) receivedTombMax = deleted
                val local = queryString(db, "SELECT updated_at FROM $table WHERE id=?", arrayOf(sqlValue(id)))
                if (local != null && local > deleted) continue
                if (db.delete(table, "id=?", arrayOf(id.toString())) > 0) applied++
                db.execSQL(
                    "INSERT OR REPLACE INTO sync_tombstones(table_name,row_id,deleted_at) VALUES(?,?,?)",
                    arrayOf(table, id.toString(), deleted),
                )
            }
            advanceCursors(db, outbound, receivedMax, maxOf(outbound.tombMax, receivedTombMax))
            db.setTransactionSuccessful()
        } finally {
            db.endTransaction()
        }
        return applied
    }

    private fun upsertRow(db: SQLiteDatabase, table: String, fields: JSONObject, remoteTs: String): Boolean {
        val id = fields.opt("id") ?: return false
        if (id == JSONObject.NULL) return false
        if (table == "event_categories") return upsertCategory(db, fields, remoteTs)
        val idText = id.toString()
        val tomb = queryString(
            db, "SELECT deleted_at FROM sync_tombstones WHERE table_name=? AND row_id=?",
            arrayOf(table, idText),
        )
        if (tomb != null && tomb >= remoteTs) return false
        val local = queryString(db, "SELECT updated_at FROM $table WHERE id=?", arrayOf(sqlValue(id)))
        if (local != null && local >= remoteTs) return false
        return executeUpsert(db, table, "id", fields, includeId = true)
    }

    private fun upsertCategory(db: SQLiteDatabase, fields: JSONObject, remoteTs: String): Boolean {
        val name = fields.optString("name")
        if (name.isBlank()) return false
        val local = queryString(db, "SELECT updated_at FROM event_categories WHERE name=?", arrayOf(name))
        if (local != null && local >= remoteTs) return false
        return executeUpsert(db, "event_categories", "name", fields, includeId = false)
    }

    private fun executeUpsert(
        db: SQLiteDatabase,
        table: String,
        conflict: String,
        fields: JSONObject,
        includeId: Boolean,
    ): Boolean {
        val schema = tableColumns(db, table)
        val cols = schema.filter { fields.has(it) && (includeId || it != "id") }
        if (cols.isEmpty() || conflict !in cols) return false
        val updates = cols.filter { it != conflict }.joinToString(",") { "$it=excluded.$it" }
        val action = if (updates.isEmpty()) "DO NOTHING" else "DO UPDATE SET $updates"
        val sql = "INSERT INTO $table(${cols.joinToString(",")}) VALUES(" +
            cols.joinToString(",") { "?" } + ") ON CONFLICT($conflict) $action"
        val args = cols.map { sqlValue(fields.opt(it)) }.toTypedArray()
        return runCatching { db.execSQL(sql, args); true }.getOrDefault(false)
    }

    private fun advanceCursors(
        db: SQLiteDatabase,
        outbound: Outbound,
        received: Map<String, String>,
        tombMax: String,
    ) {
        for (table in TABLES) {
            val max = maxOf(outbound.rowMax[table] ?: "", received[table] ?: "")
            if (max.isNotBlank()) writeSetting(db, "$CURSOR_PREFIX$table", max)
        }
        if (tombMax.isNotBlank()) writeSetting(db, "${CURSOR_PREFIX}tombstones", tombMax)
    }

    private fun tableColumns(db: SQLiteDatabase, table: String): List<String> {
        val out = mutableListOf<String>()
        db.rawQuery("PRAGMA table_info($table)", null).use { while (it.moveToNext()) out.add(it.getString(1)) }
        return out
    }

    private fun queryString(db: SQLiteDatabase, sql: String, args: Array<Any?>): String? =
        db.rawQuery(sql, args.map { it?.toString() ?: "" }.toTypedArray()).use {
            if (it.moveToFirst()) it.getString(0) else null
        }

    private fun sqlValue(value: Any?): Any? = when (value) {
        null, JSONObject.NULL -> null
        is Boolean -> if (value) 1 else 0
        is JSONObject, is JSONArray -> value.toString()
        else -> value
    }

    private fun Cursor.toJson(): JSONObject = JSONObject().also { out ->
        for (i in columnNames.indices) out.put(columnNames[i], when (getType(i)) {
            Cursor.FIELD_TYPE_NULL -> JSONObject.NULL
            Cursor.FIELD_TYPE_INTEGER -> getLong(i)
            Cursor.FIELD_TYPE_FLOAT -> getDouble(i)
            Cursor.FIELD_TYPE_STRING -> getString(i)
            else -> getBlob(i)?.let { "blob:${it.size}" } ?: JSONObject.NULL
        })
    }

    private fun readSetting(db: SQLiteDatabase, key: String): String? =
        db.rawQuery("SELECT value FROM app_settings WHERE key=?", arrayOf(key)).use {
            if (it.moveToFirst()) it.getString(0) else null
        }

    private fun writeSetting(db: SQLiteDatabase, key: String, value: String) = db.execSQL(
        "INSERT INTO app_settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        arrayOf(key, value),
    )
}
