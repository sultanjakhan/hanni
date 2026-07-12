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
            val end = Instant.now()
            val start = end.minus(30, ChronoUnit.DAYS)
            fun has(record: kotlin.reflect.KClass<out androidx.health.connect.client.records.Record>) =
                granted.contains(HealthPermission.getReadPermission(record))

            // Import every granted type independently. Denying heart rate must
            // not disable sleep, steps and walks.
            val sleep = if (has(SleepSessionRecord::class)) readSleepSessions(client, start, end) else JSONArray()
            val exercise = if (has(ExerciseSessionRecord::class)) readExerciseSessions(client, start, end) else JSONArray()
            val steps = if (has(StepsRecord::class)) readDailySteps(client, start, end) else JSONArray()
            val heartRate = if (has(HeartRateRecord::class)) readHeartRateSamples(client, start, end) else JSONArray()

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
                insertHeartRate(db, heartRate)
            } finally {
                db.close()
            }
            Log.i(TAG, "doWork ok: sleep=${sleep.length()} exercise=${exercise.length()} steps=${steps.length()} hr=${heartRate.length()}")
            Result.success()
        } catch (se: SecurityException) {
            // Background read not permitted — READ_HEALTH_DATA_IN_BACKGROUND not
            // granted (required on Android 14+ for HC reads off the foreground).
            // Don't retry-storm: the app re-requests it on next foreground open;
            // succeed quietly until then so we don't churn WorkManager backoff.
            Log.w(TAG, "background read not permitted yet: ${se.message}")
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
            val recordId = s.optString("record_id", "sleep:$date:$startTime")
            val cur = db.rawQuery(
                "SELECT id FROM sleep_sessions WHERE id=? OR (date=? AND start_time=? AND source='health_connect') " +
                    "ORDER BY (id=?) DESC LIMIT 1",
                arrayOf(recordId, date, startTime, recordId)
            )
            var sessionId: String? = if (cur.moveToFirst()) cur.getString(0) else null
            cur.close()
            if (sessionId == null) {
                val cv = ContentValues().apply {
                    put("id", recordId)
                    put("date", date)
                    put("start_time", startTime)
                    put("end_time", endTime)
                    put("duration_minutes", dur)
                    put("source", "health_connect")
                    put("created_at", now)
                }
                if (db.insertWithOnConflict("sleep_sessions", null, cv, SQLiteDatabase.CONFLICT_IGNORE) != -1L) {
                    sessionId = recordId
                } else {
                    db.rawQuery(
                        "SELECT id FROM sleep_sessions WHERE date=? AND start_time=? AND source='health_connect' LIMIT 1",
                        arrayOf(date, startTime)
                    ).use { if (it.moveToFirst()) sessionId = it.getString(0) }
                }
            }
            sessionId?.let { sid ->
                val patch = ContentValues().apply {
                    put("end_time", endTime)
                    put("duration_minutes", dur)
                }
                db.update("sleep_sessions", patch, "id=?", arrayOf(sid))
                reconcileSleepStages(db, sid, s.optJSONArray("stages") ?: JSONArray())
            }
        }
    }

    private fun reconcileSleepStages(db: SQLiteDatabase, sessionId: String, stages: JSONArray) {
        val desired = linkedSetOf<Triple<String, String, String>>()
        for (i in 0 until stages.length()) {
            val st = stages.optJSONObject(i) ?: continue
            val key = Triple(st.optString("start_time"), st.optString("end_time"), st.optString("stage"))
            if (key.first.isNotEmpty() && key.second.isNotEmpty() && key.third.isNotEmpty()) desired.add(key)
        }
        val kept = mutableSetOf<Triple<String, String, String>>()
        val remove = mutableListOf<String>()
        db.rawQuery(
            "SELECT id,start_time,end_time,stage FROM sleep_stages WHERE session_id=?",
            arrayOf(sessionId)
        ).use { c ->
            while (c.moveToNext()) {
                val key = Triple(c.getString(1), c.getString(2), c.getString(3))
                if (key !in desired || !kept.add(key)) remove.add(c.getString(0))
            }
        }
        remove.forEach { db.delete("sleep_stages", "id=?", arrayOf(it)) }
        for ((start, end, stage) in desired) {
            if (Triple(start, end, stage) in kept) continue
            val cv = ContentValues().apply {
                put("id", "stage:$sessionId:$start:$end:$stage")
                put("session_id", sessionId)
                put("start_time", start)
                put("end_time", end)
                put("stage", stage)
            }
            db.insertWithOnConflict("sleep_stages", null, cv, SQLiteDatabase.CONFLICT_IGNORE)
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
                    val recordId = s.optString("record_id")
                    put("id", if (recordId.isEmpty()) "health:exercise:$date:$startTime:$notes" else "health:exercise:$recordId")
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
                    put("id", "health:steps:$date")
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

    private fun insertHeartRate(db: SQLiteDatabase, arr: JSONArray) {
        for (i in 0 until arr.length()) {
            val s = arr.optJSONObject(i) ?: continue
            val date = s.optString("date")
            val time = s.optString("time")
            val bpm = s.optLong("bpm", 0L)
            if (date.isEmpty() || time.isEmpty() || bpm <= 0) continue
            val recordId = s.optString("record_id")
            val sampleIndex = s.optInt("sample_index", 0)
            val id = if (recordId.isEmpty()) "health:hr:$date:$time" else "health:hr:$recordId:$sampleIndex"
            val cv = ContentValues().apply {
                put("id", id); put("date", date); put("time", time); put("bpm", bpm)
            }
            db.insertWithOnConflict("heart_rate_samples", null, cv, SQLiteDatabase.CONFLICT_IGNORE)
        }
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
