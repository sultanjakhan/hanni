package com.sultanjakhan.hanni

import android.content.ContentValues
import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.util.Log
import java.io.File
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.ExerciseSessionRecord
import androidx.health.connect.client.records.HeartRateRecord
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.records.StepsRecord
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit

// Periodic background sync: reads sleep/exercise/heart-rate/steps from
// Health Connect, persists into hanni.db so the user sees them next launch,
// and pushes the freshly-inserted rows directly to the configured LAN peer
// (Mac) so Calendar on the laptop stays current even when the phone's
// Tauri/WebView process is not running.
//
// Runs from WorkManager at a 15-min minimum interval — Android won't let us
// go tighter for periodic work. Coupled with the in-app polling on
// foreground / visibilitychange, freshness becomes ~3 min when the app is
// open and ~15 min worst case when it isn't.
class HanniHealthWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        Log.i(TAG, "doWork: enter")
        try {
            val ctx = applicationContext
            val sdk = HealthConnectClient.getSdkStatus(ctx)
            if (sdk != HealthConnectClient.SDK_AVAILABLE) {
                Log.w(TAG, "HC SDK not available: $sdk")
                return@withContext Result.success()
            }
            val client = HealthConnectClient.getOrCreate(ctx)
            val granted = client.permissionController.getGrantedPermissions()
            Log.i(TAG, "doWork: granted=${granted.size} HC permissions")
            val needed = setOf(
                HealthPermission.getReadPermission(SleepSessionRecord::class),
                HealthPermission.getReadPermission(ExerciseSessionRecord::class),
                HealthPermission.getReadPermission(StepsRecord::class),
                HealthPermission.getReadPermission(HeartRateRecord::class),
            )
            if (!granted.containsAll(needed)) {
                Log.w(TAG, "HC permissions missing; nothing to do until user grants")
                return@withContext Result.success()
            }

            val end = Instant.now()
            val start = end.minus(30, ChronoUnit.DAYS)

            val sleep = readSleepSessions(client, start, end)
            val exercise = readExerciseSessions(client, start, end)
            val steps = readDailySteps(client, start, end)

            // Hanni keeps the DB in app_data_dir (Tauri's path resolver), not
            // the standard `databases/` sub-dir, so getDatabasePath() misses it.
            // app_data_dir on Android = filesDir's parent = /data/user/0/<pkg>/.
            val dbFile = File(ctx.filesDir.parentFile, "hanni.db")
            // Race-guard: Worker may fire before Rust ever created the DB (fresh
            // install, or after corruption-recovery the file briefly disappears).
            // Returning success() — not retry() — avoids WorkManager backoff storm
            // while Hanni isn't even running; next 15-min tick will see the file.
            if (!dbFile.exists() || dbFile.length() == 0L) {
                Log.i(TAG, "doWork skip: DB not ready yet (exists=${dbFile.exists()} size=${dbFile.length()})")
                return@withContext Result.success()
            }
            val db = SQLiteDatabase.openDatabase(
                dbFile.absolutePath, null, SQLiteDatabase.OPEN_READWRITE
            )
            try {
                insertSleep(db, sleep)
                insertExercise(db, exercise)
                insertSteps(db, steps)

                // Best-effort push to the configured LAN peer. We only push
                // (no apply of the response) — Hanni Rust will fully sync
                // next time it boots and runs lan_sync_now.
                val (peer, key) = readLanConfig(db) ?: Pair(null, null)
                if (peer != null && key != null && peer.isNotBlank() && key.isNotBlank()) {
                    pushLan(db, peer, key)
                }
            } finally {
                db.close()
            }
            Log.i(TAG, "doWork ok: sleep=${sleep.length()} exercise=${exercise.length()} steps=${steps.length()}")
            Result.success()
        } catch (e: Exception) {
            Log.e(TAG, "doWork failed", e)
            // retry with WorkManager's default backoff
            Result.retry()
        }
    }

    private fun insertSleep(db: SQLiteDatabase, arr: JSONArray) {
        val now = isoNow()
        for (i in 0 until arr.length()) {
            val s = arr.optJSONObject(i) ?: continue
            val date = s.optString("date")
            val startTime = s.optString("start_time")
            val endTime = s.optString("end_time")
            val dur = s.optLong("duration_minutes", 0L)
            if (date.isEmpty() || startTime.isEmpty() || endTime.isEmpty() || dur <= 0) continue
            // Phase 1 UUID PK: generate a v4 UUID (Java stdlib) and INSERT
            // with CONFLICT_IGNORE on UNIQUE(date, start_time, source). If a
            // session already exists (either ours from a prior tick or one
            // pulled from the Mac via LAN sync), the insert is a no-op and
            // we just refresh end_time/duration so the night doesn't get
            // pinned to the first segment we saw. We never DELETE the
            // existing row — that would replace a Mac-side UUID and create
            // a duplicate after the next sync.
            val cv = ContentValues().apply {
                put("id", java.util.UUID.randomUUID().toString())
                put("date", date)
                put("start_time", startTime)
                put("end_time", endTime)
                put("duration_minutes", dur)
                put("source", "health_connect")
                put("created_at", now)
            }
            val rowId = db.insertWithOnConflict(
                "sleep_sessions", null, cv, SQLiteDatabase.CONFLICT_IGNORE
            )
            if (rowId == -1L) {
                // Conflict — session already existed. Refresh end_time + duration
                // (last write wins, HC may extend a session as the night progresses).
                val patch = ContentValues().apply {
                    put("end_time", endTime)
                    put("duration_minutes", dur)
                }
                db.update(
                    "sleep_sessions", patch,
                    "date=? AND start_time=? AND source='health_connect'",
                    arrayOf(date, startTime)
                )
            }
        }
    }

    private fun insertExercise(db: SQLiteDatabase, arr: JSONArray) {
        val now = isoNow()
        // Upsert per session by (date, start_time, notes). Old code did
        // delete-by-date which clobbered Mac-synced rows on every poll —
        // post-Phase 2 ids are UUIDs, deleting them creates orphans.
        for (i in 0 until arr.length()) {
            val s = arr.optJSONObject(i) ?: continue
            val date = s.optString("date")
            val dur = s.optDouble("duration_minutes", 0.0)
            if (date.isEmpty() || dur < 1.0) continue
            val etype = s.optString("type", "other")
            val title = s.optString("title", "")
            val startTime = s.optString("start_time", "")
            val notes = "$etype: $title"
            val cur = db.rawQuery(
                "SELECT id FROM health_log " +
                "WHERE type='exercise' AND date=? AND COALESCE(start_time,'')=? AND notes=? LIMIT 1",
                arrayOf(date, startTime, notes)
            )
            val existingId: String? = if (cur.moveToFirst()) cur.getString(0) else null
            cur.close()
            if (existingId != null) {
                val patch = ContentValues().apply { put("value", dur) }
                db.update("health_log", patch, "id=?", arrayOf(existingId))
            } else {
                val cv = ContentValues().apply {
                    put("id", java.util.UUID.randomUUID().toString())
                    put("date", date)
                    put("type", "exercise")
                    put("value", dur)
                    put("unit", "minutes")
                    put("notes", notes)
                    put("start_time", startTime)
                    put("created_at", now)
                }
                db.insert("health_log", null, cv)
            }
        }
    }

    private fun insertSteps(db: SQLiteDatabase, arr: JSONArray) {
        val now = isoNow()
        for (i in 0 until arr.length()) {
            val d = arr.optJSONObject(i) ?: continue
            val date = d.optString("date")
            if (date.isEmpty()) continue
            val steps = d.optDouble("steps", 0.0)
            // Upsert by (date, type='steps'). Post-Phase 2 id is TEXT UUIDv4
            // — never DELETE first, that would orphan stages/timeline blocks
            // and replace a Mac-synced UUID on every poll.
            val cur = db.rawQuery(
                "SELECT id FROM health_log WHERE date=? AND type='steps' LIMIT 1",
                arrayOf(date)
            )
            val existingId: String? = if (cur.moveToFirst()) cur.getString(0) else null
            cur.close()
            if (existingId != null) {
                val patch = ContentValues().apply { put("value", steps) }
                db.update("health_log", patch, "id=?", arrayOf(existingId))
            } else {
                val cv = ContentValues().apply {
                    put("id", java.util.UUID.randomUUID().toString())
                    put("date", date)
                    put("type", "steps")
                    put("value", steps)
                    put("unit", "count")
                    put("notes", "")
                    put("start_time", "")
                    put("created_at", now)
                }
                db.insert("health_log", null, cv)
            }
        }
    }

    // Read lan_sync_peer + lan_sync_key from app_settings. Returns null
    // if the user hasn't configured LAN sync yet (worker just writes
    // locally then; Hanni will push next launch).
    private fun readLanConfig(db: SQLiteDatabase): Pair<String, String>? {
        var peer: String? = null
        var key: String? = null
        db.rawQuery(
            "SELECT key, value FROM app_settings WHERE key IN ('lan_sync_peer','lan_sync_key','lan_sync_enabled')",
            null
        ).use { c ->
            while (c.moveToNext()) {
                when (c.getString(0)) {
                    "lan_sync_peer" -> peer = c.getString(1)
                    "lan_sync_key" -> key = c.getString(1)
                    "lan_sync_enabled" -> if (c.getString(1) != "true") return null
                }
            }
        }
        return if (peer != null && key != null) Pair(peer!!, key!!) else null
    }

    // Gather rows newer than each table's lan_cursor_* setting and POST them
    // to the configured peer in the same shape Rust's lan_sync_now does, so
    // Mac's lan_sync server can upsert them via LWW. We don't apply the
    // response — Hanni Rust pulls Mac's rows on next launch.
    private fun pushLan(db: SQLiteDatabase, peer: String, key: String) {
        val tables = listOf(
            "events", "sleep_sessions", "health_log", "notes", "facts",
            "recipes", "transactions", "body_records", "conversations",
        )
        val cursors = JSONObject()
        val rows = JSONArray()
        for (t in tables) {
            val cursor = readSetting(db, "lan_cursor_$t") ?: "1970-01-01T00:00:00"
            cursors.put(t, cursor)
            // SELECT * for rows with updated_at > cursor. SQLite WHERE
            // applied per-row.
            val cur = runCatching {
                db.rawQuery("SELECT * FROM $t WHERE updated_at > ? LIMIT 500", arrayOf(cursor))
            }.getOrNull()
            if (cur == null) continue
            cur.use { c ->
                val cols = c.columnNames
                while (c.moveToNext()) {
                    val f = JSONObject()
                    for (i in cols.indices) {
                        when (c.getType(i)) {
                            android.database.Cursor.FIELD_TYPE_INTEGER -> f.put(cols[i], c.getLong(i))
                            android.database.Cursor.FIELD_TYPE_FLOAT -> f.put(cols[i], c.getDouble(i))
                            android.database.Cursor.FIELD_TYPE_STRING -> f.put(cols[i], c.getString(i))
                            android.database.Cursor.FIELD_TYPE_NULL -> f.put(cols[i], JSONObject.NULL)
                            else -> f.put(cols[i], c.getString(i))
                        }
                    }
                    val item = JSONObject().apply { put("t", t); put("f", f) }
                    rows.put(item)
                }
            }
        }
        if (rows.length() == 0) {
            Log.i(TAG, "pushLan: nothing new")
            return
        }
        val body = JSONObject().apply {
            put("key", key)
            put("cursors", cursors)
            put("tomb_cursor", readSetting(db, "lan_cursor_tombstones") ?: "1970-01-01T00:00:00")
            put("rows", rows)
            put("tombs", JSONArray())
        }
        val url = URL("http://$peer/lan/sync")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "POST"
        conn.setRequestProperty("Content-Type", "application/json")
        conn.connectTimeout = 6000
        conn.readTimeout = 8000
        conn.doOutput = true
        conn.outputStream.use { it.write(body.toString().toByteArray()) }
        val code = conn.responseCode
        Log.i(TAG, "pushLan rows=${rows.length()} http=$code")
        conn.disconnect()

        // Advance cursors to the max updated_at we just sent so we don't
        // re-push on the next tick.
        var maxTs = "1970-01-01T00:00:00"
        for (i in 0 until rows.length()) {
            val f = rows.optJSONObject(i)?.optJSONObject("f")
            if (f != null) {
                val ts = f.optString("updated_at", "")
                if (ts > maxTs) maxTs = ts
            }
        }
        for (t in tables) writeSetting(db, "lan_cursor_$t", maxTs)
    }

    private fun readSetting(db: SQLiteDatabase, key: String): String? {
        db.rawQuery("SELECT value FROM app_settings WHERE key=?", arrayOf(key)).use {
            return if (it.moveToFirst()) it.getString(0) else null
        }
    }

    private fun writeSetting(db: SQLiteDatabase, key: String, value: String) {
        db.execSQL(
            "INSERT INTO app_settings(key, value) VALUES(?, ?) " +
                "ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            arrayOf(key, value)
        )
    }

    private fun isoNow(): String =
        DateTimeFormatter.ISO_OFFSET_DATE_TIME.format(
            Instant.now().atZone(ZoneId.systemDefault())
        )

    companion object {
        const val TAG = "HanniHealthWorker"
        const val UNIQUE_NAME = "hanni-health-sync"
    }
}
