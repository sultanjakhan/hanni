package com.sultanjakhan.hanni

import android.database.Cursor
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneOffset
import java.time.format.DateTimeFormatterBuilder
import java.util.Locale

/** SQLite codec for Android's background LAN client. Keep TABLES in db.rs order. */
internal object LanSyncDatabase {
    private const val EPOCH = "1970-01-01T00:00:00"
    private const val LIMIT = 500
    private const val PUSH_PREFIX = "lan_push_cursor_"
    private const val PULL_PREFIX = "lan_pull_cursor_"
    private val canonicalTime = DateTimeFormatterBuilder().appendInstant(9).toFormatter(Locale.ROOT)

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

    data class CursorSnapshot(
        val push: Map<String, String>,
        val pull: Map<String, String>,
        val pushTomb: String,
        val pullTomb: String,
    )

    data class Outbound(
        // Wire cursors describe received records, never sent records.
        val cursors: JSONObject,
        val tombCursor: String,
        val rows: JSONArray,
        val tombs: JSONArray,
        val rowMax: MutableMap<String, String>,
        var tombMax: String,
        val snapshot: CursorSnapshot,
        val peer: String,
    )

    private class SyncFailure(message: String) : IllegalStateException(message)

    // Do not expose SQLite errors or record values through worker logs.
    private fun <T> databaseTransaction(db: HealthDatabase, action: () -> T): T {
        try {
            check(!db.inTransaction())
            db.beginTransaction()
            try {
                val result = action()
                db.setTransactionSuccessful()
                return result
            } finally {
                db.endTransaction()
            }
        } catch (error: Exception) {
            if (error is SyncFailure) throw error
            throw SyncFailure("LAN database batch failed; progress not acknowledged")
        }
    }

    fun gather(db: HealthDatabase, expectedPeer: String? = null): Outbound = databaseTransaction(db) {
        val peer = readSetting(db, "lan_sync_peer").orEmpty()
        if (expectedPeer != null && peer != expectedPeer) {
            throw SyncFailure("LAN configuration changed; retry")
        }
        val snapshot = readSnapshot(db)
        val rows = JSONArray()
        val rowMax = linkedMapOf<String, String>()
        for (table in TABLES) {
            val since = snapshot.push.getValue(table)
            val projectionFilter = projectionRowFilter(db, table)
            // LIMIT is soft: finish the cutoff timestamp group. An oversized
            // HTTP batch must fail without acknowledging cursor progress.
            db.rawQuery(
                "WITH page AS (SELECT updated_at FROM $table WHERE updated_at>? AND ($projectionFilter) " +
                    "ORDER BY updated_at LIMIT $LIMIT) " +
                    "SELECT * FROM $table WHERE updated_at>? AND ($projectionFilter) " +
                    "AND updated_at<=(SELECT MAX(updated_at) FROM page) " +
                    "ORDER BY updated_at,CAST(id AS TEXT)",
                arrayOf(since, since),
            ).use { cursor ->
                while (cursor.moveToNext()) {
                    val fields = cursor.toJson()
                    val updated = fields.opt("updated_at") as? String
                        ?: throw SyncFailure("Invalid LAN local row timestamp")
                    parseTime(updated)
                    fields.put("_updated_at", updated)
                    rows.put(JSONObject().put("t", table).put("f", fields))
                    rowMax[table] = maxOf(rowMax[table].orEmpty(), updated)
                }
            }
        }
        val tombs = JSONArray()
        var tombMax = ""
        val projectionFilter = projectionTombFilter(db)
        db.rawQuery(
            "WITH page AS (SELECT deleted_at FROM sync_tombstones WHERE deleted_at>? AND ($projectionFilter) " +
                "ORDER BY deleted_at LIMIT $LIMIT) " +
                "SELECT table_name,row_id,deleted_at FROM sync_tombstones WHERE deleted_at>? AND ($projectionFilter) " +
                "AND deleted_at<=(SELECT MAX(deleted_at) FROM page) " +
                "ORDER BY deleted_at,table_name,CAST(row_id AS TEXT)",
            arrayOf(snapshot.pushTomb, snapshot.pushTomb),
        ).use { cursor ->
            while (cursor.moveToNext()) {
                val table = cursor.getString(0)
                require(table in allowed)
                val deleted = cursor.getString(2)
                parseTime(deleted)
                tombs.put(JSONObject().put("tt", table).put("id", cursor.getString(1))
                    .put("deleted_at", deleted))
                tombMax = maxOf(tombMax, deleted)
            }
        }
        Outbound(JSONObject(snapshot.pull), snapshot.pullTomb, rows, tombs,
            rowMax, tombMax, snapshot, peer)
    }

    fun applyResponse(db: HealthDatabase, outbound: Outbound, response: JSONObject): Int =
        databaseTransaction(db) {
            // A malformed HTTP 200 is not an empty successful pull.
            val rows = response.getJSONArray("rows")
            val tombs = response.getJSONArray("tombs")
            if (readSetting(db, "lan_sync_peer").orEmpty() != outbound.peer ||
                readSnapshot(db) != outbound.snapshot) {
                throw SyncFailure("LAN configuration or progress changed; retry")
            }
            setRemoteApply(db, true)
            var applied = 0
            val receivedMax = linkedMapOf<String, String>()
            var receivedTombMax = ""
            for (i in 0 until rows.length()) {
                val item = rows.getJSONObject(i)
                val table = item.opt("t") as? String ?: throw SyncFailure("Invalid LAN row")
                require(table in allowed)
                val fields = item.getJSONObject("f")
                val updated = fields.opt("updated_at") as? String
                    ?: throw SyncFailure("Invalid LAN row timestamp")
                val remote = fields.opt("_updated_at") as? String
                    ?: throw SyncFailure("Invalid LAN row timestamp")
                val timestamp = parseTime(remote)
                require(parseTime(updated) == timestamp)
                val canonicalFields = JSONObject(fields.toString())
                    .put("updated_at", canonicalTime.format(timestamp))
                if (upsertRow(db, table, canonicalFields, timestamp)) applied++
                receivedMax[table] = maxOf(receivedMax[table].orEmpty(), updated)
            }
            for (i in 0 until tombs.length()) {
                val tomb = tombs.getJSONObject(i)
                val table = tomb.opt("tt") as? String ?: throw SyncFailure("Invalid LAN tombstone")
                require(table in allowed)
                val id = tomb.opt("id") ?: throw SyncFailure("Invalid LAN tombstone")
                require(id != JSONObject.NULL)
                val deleted = tomb.opt("deleted_at") as? String
                    ?: throw SyncFailure("Invalid LAN tombstone timestamp")
                if (applyTombstone(db, table, id, parseTime(deleted))) applied++
                receivedTombMax = maxOf(receivedTombMax, deleted)
            }
            setRemoteApply(db, false)
            // Incoming high timestamps cannot skip unsent local backlog.
            advanceCursors(db, PUSH_PREFIX, outbound.rowMax, outbound.tombMax)
            advanceCursors(db, PULL_PREFIX, receivedMax, receivedTombMax)
            applied
        }

    private fun setRemoteApply(db: HealthDatabase, active: Boolean) {
        val previous = if (active) 0 else 1
        val desired = if (active) 1 else 0
        val changed = db.compileStatement(
            "UPDATE sync_apply_context SET remote_apply=$desired " +
                "WHERE singleton=1 AND remote_apply=$previous AND stamp_depth=0",
        ).use { it.executeUpdateDelete() }
        check(changed == 1)
    }

    private fun observeTimestamp(db: HealthDatabase, time: Instant) {
        // Match Rust: ceil sub-millisecond remote timestamps before local HLC.
        require(time.epochSecond >= 0)
        val millis = Math.addExact(Math.multiplyExact(time.epochSecond, 1000L),
            (time.nano.toLong() + 999_999L) / 1_000_000L)
        val changed = db.compileStatement(
            "UPDATE sync_hlc_state SET last_millis=MAX(last_millis,?) WHERE singleton=1",
        ).use { it.bindLong(1, millis); it.executeUpdateDelete() }
        check(changed == 1)
    }

    private fun parseTime(raw: String): Instant {
        val text = raw.trim()
        if (text.isEmpty()) throw SyncFailure("Invalid LAN timestamp")
        val instant = try {
            Instant.parse(text)
        } catch (_: Exception) {
            try {
                LocalDateTime.parse(text.replace(' ', 'T')).toInstant(ZoneOffset.UTC)
            } catch (_: Exception) {
                try {
                    LocalDate.parse(text).atStartOfDay().toInstant(ZoneOffset.UTC)
                } catch (_: Exception) {
                    throw SyncFailure("Invalid LAN timestamp")
                }
            }
        }
        if (instant.epochSecond < 0) throw SyncFailure("Invalid LAN timestamp")
        return instant
    }

    private data class RowKey(val value: Any, val text: String)

    private fun rowKey(db: HealthDatabase, table: String, id: Any?, tombstone: Boolean = false): RowKey {
        require(id != null && id != JSONObject.NULL)
        val type = tableSchema(db, table)["id"] ?: throw SyncFailure("Invalid LAN table schema")
        return if (type.uppercase(Locale.ROOT).contains("TEXT")) {
            val text = when {
                id is String -> id
                tombstone && (id is Int || id is Long) -> id.toString()
                else -> throw SyncFailure("Invalid LAN record identity")
            }
            require(text.isNotEmpty())
            RowKey(text, text)
        } else {
            val number = when (id) {
                is Int -> id.toLong()
                is Long -> id
                is String -> if (tombstone) id.toLongOrNull() else null
                else -> null
            } ?: throw SyncFailure("Invalid LAN record identity")
            RowKey(number, number.toString())
        }
    }

    private fun upsertRow(db: HealthDatabase, table: String, fields: JSONObject, remote: Instant): Boolean {
        val key = rowKey(db, table, fields.opt("id"))
        if (table == "event_categories") return upsertCategory(db, fields, remote)
        val tomb = queryString(db,
            "SELECT deleted_at FROM sync_tombstones WHERE table_name=? AND row_id=?",
            arrayOf(table, key.text))
        if (tomb != null && parseTime(tomb) >= remote) return false
        val local = queryString(db, "SELECT updated_at FROM $table WHERE id=?", arrayOf(key.value))
        // Legacy rows have no writer id; preserve the existing equal-time skip.
        if (local != null && parseTime(local) >= remote) return false
        executeUpsert(db, table, "id", fields, includeId = true)
        observeTimestamp(db, remote)
        return true
    }

    private fun upsertCategory(db: HealthDatabase, fields: JSONObject, remote: Instant): Boolean {
        val name = fields.opt("name") as? String ?: throw SyncFailure("Invalid LAN category")
        require(name.isNotEmpty())
        val tomb = queryString(db,
            "SELECT deleted_at FROM sync_tombstones WHERE table_name='event_categories' AND row_id=?",
            arrayOf("name:$name"))
        if (tomb != null && parseTime(tomb) >= remote) return false
        val local = queryString(db, "SELECT updated_at FROM event_categories WHERE name=?", arrayOf(name))
        if (local != null && parseTime(local) >= remote) return false
        executeUpsert(db, "event_categories", "name", fields, includeId = false)
        observeTimestamp(db, remote)
        return true
    }

    private fun applyTombstone(db: HealthDatabase, table: String, id: Any, remote: Instant): Boolean {
        val key: RowKey
        val column: String
        if (table == "event_categories") {
            // Match Rust: do not apply legacy device-local category ids.
            val text = id as? String ?: return false
            if (!text.startsWith("name:") || text.length == 5) return false
            key = RowKey(text.substring(5), text)
            column = "name"
        } else {
            key = rowKey(db, table, id, tombstone = true)
            column = "id"
        }
        val knownRaw = queryString(db,
            "SELECT deleted_at FROM sync_tombstones WHERE table_name=? AND row_id=?",
            arrayOf(table, key.text))
        val known = knownRaw?.let(::parseTime)
        val effective = if (known != null && known > remote) known else remote
        val local = queryString(db, "SELECT updated_at FROM $table WHERE $column=?", arrayOf(key.value))
        if (local != null && parseTime(local) > effective) return false
        val deleted = db.delete(table, "$column=?", arrayOf(key.value.toString()))
        val timestamp = canonicalTime.format(effective)
        db.execSQL(
            "INSERT INTO sync_tombstones(table_name,row_id,deleted_at) VALUES(?,?,?) " +
                "ON CONFLICT(table_name,row_id) DO UPDATE SET deleted_at=excluded.deleted_at",
            arrayOf(table, key.text, timestamp),
        )
        observeTimestamp(db, effective)
        return deleted > 0 || known != effective
    }

    private fun executeUpsert(db: HealthDatabase, table: String, conflict: String,
                             fields: JSONObject, includeId: Boolean) {
        val schema = tableSchema(db, table)
        val cols = schema.keys.filter { fields.has(it) && (includeId || it != "id") }
        require(cols.isNotEmpty() && conflict in cols)
        val updates = cols.filter { it != conflict }.joinToString(",") { "$it=excluded.$it" }
        val action = if (updates.isEmpty()) "DO NOTHING" else "DO UPDATE SET $updates"
        val sql = "INSERT INTO $table(${cols.joinToString(",")}) VALUES(" +
            cols.joinToString(",") { "?" } + ") ON CONFLICT($conflict) $action"
        db.execSQL(sql, cols.map { sqlValue(fields.opt(it)) }.toTypedArray())
    }

    private fun advanceCursors(db: HealthDatabase, prefix: String, rows: Map<String, String>, tomb: String) {
        for ((table, maximum) in rows) {
            require(table in allowed)
            if (maximum > readCursor(db, prefix, table)) writeSetting(db, "$prefix$table", maximum)
        }
        if (tomb.isNotEmpty() && tomb > readCursor(db, prefix, "tombstones")) {
            writeSetting(db, "${prefix}tombstones", tomb)
        }
    }

    private fun readSnapshot(db: HealthDatabase): CursorSnapshot = CursorSnapshot(
        TABLES.associateWith { readCursor(db, PUSH_PREFIX, it) },
        TABLES.associateWith { readCursor(db, PULL_PREFIX, it) },
        readCursor(db, PUSH_PREFIX, "tombstones"), readCursor(db, PULL_PREFIX, "tombstones"),
    )

    private fun readCursor(db: HealthDatabase, prefix: String, table: String): String {
        readSetting(db, "$prefix$table")?.let { return it }
        // Preserve old progress. Historical reconciliation is explicit.
        var previous = EPOCH
        for (old in listOf("lan_cursor_", "background_lan_cursor_", "health_worker_push_cursor_")) {
            readSetting(db, "$old$table")?.let { previous = maxOf(previous, it) }
        }
        return previous
    }

    private fun projectionRegistryExists(db: HealthDatabase): Boolean =
        queryString(db, "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='hc_sleep_projection_owned' AND type='table')", emptyArray()) == "1"

    private fun projectionRowFilter(db: HealthDatabase, table: String): String {
        require(table in allowed)
        val parts = mutableListOf("1")
        if (table == "sleep_sessions") parts.add("CAST($table.id AS TEXT) NOT GLOB 'raw-sleep:*'")
        if (table == "sleep_stages") parts.add("CAST($table.id AS TEXT) NOT GLOB 'raw-stage:*'")
        if (table in listOf("events", "timeline_blocks") && "source" in tableSchema(db, table)) {
            parts.add("COALESCE($table.source,'') NOT GLOB 'auto_health_raw:*'")
        }
        if (projectionRegistryExists(db)) parts.add("NOT EXISTS(SELECT 1 FROM hc_sleep_projection_owned o WHERE o.table_name='$table' AND o.row_id=CAST($table.id AS TEXT))")
        return parts.joinToString(" AND ")
    }

    private fun projectionTombFilter(db: HealthDatabase): String {
        val parts = mutableListOf(
            "NOT (sync_tombstones.table_name='sleep_sessions' AND sync_tombstones.row_id GLOB 'raw-sleep:*')",
            "NOT (sync_tombstones.table_name='sleep_stages' AND sync_tombstones.row_id GLOB 'raw-stage:*')")
        if (projectionRegistryExists(db)) parts.add("NOT EXISTS(SELECT 1 FROM hc_sleep_projection_owned o WHERE o.table_name=sync_tombstones.table_name AND o.row_id=CAST(sync_tombstones.row_id AS TEXT))")
        return parts.joinToString(" AND ")
    }

    private fun tableSchema(db: HealthDatabase, table: String): Map<String, String> {
        require(table in allowed)
        val columns = linkedMapOf<String, String>()
        db.rawQuery("PRAGMA table_info($table)", null).use {
            while (it.moveToNext()) columns[it.getString(1)] = it.getString(2)
        }
        check(columns.isNotEmpty())
        return columns
    }

    private fun queryString(db: HealthDatabase, sql: String, args: Array<Any?>): String? =
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

    private fun readSetting(db: HealthDatabase, key: String): String? =
        db.rawQuery("SELECT value FROM app_settings WHERE key=?", arrayOf(key)).use {
            if (it.moveToFirst()) it.getString(0) else null
        }

    private fun writeSetting(db: HealthDatabase, key: String, value: String) = db.execSQL(
        "INSERT INTO app_settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        arrayOf(key, value),
    )
}
