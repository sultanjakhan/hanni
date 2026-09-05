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
import androidx.health.connect.client.records.StepsRecord
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import kotlinx.coroutines.CancellationException
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
// open. Android can delay background work beyond the requested interval.
class HanniHealthWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val ctx = applicationContext
            if (HealthConnectClient.getSdkStatus(ctx) != HealthConnectClient.SDK_AVAILABLE) {
                return@withContext Result.failure(workDataOf("status" to "health_unavailable"))
            }
            val client = HealthConnectClient.getOrCreate(ctx)
            val granted = client.permissionController.getGrantedPermissions()
            val end = Instant.now()
            val start = end.minus(30, ChronoUnit.DAYS)
            val readings = linkedMapOf<String, JSONArray>()
            var temporaryFailure = false
            var permissionFailure = false
            suspend fun readType(
                name: String,
                record: kotlin.reflect.KClass<out androidx.health.connect.client.records.Record>,
                reader: suspend () -> JSONArray,
            ) {
                if (HealthPermission.getReadPermission(record) !in granted) return
                try {
                    readings[name] = reader()
                } catch (e: CancellationException) {
                    throw e
                } catch (_: SecurityException) {
                    permissionFailure = true
                } catch (e: Exception) {
                    temporaryFailure = true
                    Log.w(TAG, "read failed: type=$name reason=${e.javaClass.simpleName}")
                }
            }
            // The raw importer captures sleep before pairing. Only the selected
            // source projection may materialize it; never add legacy duplicates
            // during the first-launch window before authority is configured.
            readType("exercise", ExerciseSessionRecord::class) { readExerciseSessions(client, start, end) }
            readType("steps", StepsRecord::class) { readDailySteps(client, start, end) }
            readType("heart_rate", HeartRateRecord::class) { readHeartRateSamples(client, start, end) }
            if (readings.isEmpty()) {
                return@withContext if (temporaryFailure) Result.retry()
                else Result.failure(workDataOf("status" to "health_permission_required"))
            }
            val dbFile = File(ctx.filesDir.parentFile, "hanni.db")
            if (!dbFile.exists() || dbFile.length() == 0L) return@withContext Result.retry()
            val db = SQLiteDatabase.openDatabase(dbFile.absolutePath, null,
                SQLiteDatabase.OPEN_READWRITE or SQLiteDatabase.ENABLE_WRITE_AHEAD_LOGGING)
            try {
                db.beginTransaction()
                try {
                    readings["exercise"]?.let { insertExercise(db, it) }
                    readings["steps"]?.let { insertSteps(db, it) }
                    readings["heart_rate"]?.let { insertHeartRate(db, it) }
                    db.setTransactionSuccessful()
                } finally {
                    db.endTransaction()
                }
            } finally {
                db.close()
            }
            // Durable WorkManager delivery is enqueued only after DB commit.
            // Its independent periodic worker also runs when HC cannot be read.
            HanniLanSyncWorker.enqueueCatchup(ctx)
            HanniRelaySyncWorker.enqueueCatchup(ctx)
            Log.i(TAG, "import committed: readTypes=${readings.size} temporaryFailure=$temporaryFailure permissionFailure=$permissionFailure")
            if (temporaryFailure) Result.retry()
            else Result.success(workDataOf(
                "status" to if (permissionFailure) "partial_permission_required" else "imported",
                "read_types" to readings.keys.toTypedArray(),
            ))
        } catch (e: CancellationException) {
            throw e
        } catch (_: SecurityException) {
            Result.failure(workDataOf("status" to "health_permission_required"))
        } catch (e: Exception) {
            Log.w(TAG, "import failed: ${e.javaClass.simpleName}")
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
            requireNotNull(sessionId) { "Sleep record could not be stored" }.let { sid ->
                val patch = ContentValues().apply {
                    put("date", date)
                    put("start_time", startTime)
                    put("end_time", endTime)
                    put("duration_minutes", dur)
                }
                // A repeated HC window must not stamp unchanged rows or enqueue them again.
                db.update("sleep_sessions", patch,
                    "id=? AND (date IS NOT ? OR start_time IS NOT ? OR end_time IS NOT ? OR duration_minutes IS NOT ?)",
                    arrayOf(sid, date, startTime, endTime, dur.toString()))
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
            val recordId = s.optString("record_id")
            val stableId = if (recordId.isEmpty()) "health:exercise:$date:$startTime:$notes" else "health:exercise:$recordId"
            val cur = db.rawQuery(
                "SELECT id FROM health_log " +
                "WHERE id=? OR (type='exercise' AND date=? AND COALESCE(start_time,'')=? AND notes=?) " +
                    "ORDER BY (id=?) DESC LIMIT 1",
                arrayOf(stableId, date, startTime, notes, stableId)
            )
            val existingId: String? = if (cur.moveToFirst()) cur.getString(0) else null
            cur.close()
            if (existingId != null) {
                val patch = ContentValues().apply {
                    put("value", dur); put("date", date); put("start_time", startTime); put("notes", notes)
                }
                db.update("health_log", patch,
                    "id=? AND (value IS NOT ? OR date IS NOT ? OR start_time IS NOT ? OR notes IS NOT ?)",
                    arrayOf(existingId, dur.toString(), date, startTime, notes))
            } else {
                val cv = ContentValues().apply {
                    put("id", stableId)
                    put("date", date)
                    put("type", "exercise")
                    put("value", dur)
                    put("unit", "minutes")
                    put("notes", notes)
                    put("start_time", startTime)
                    put("created_at", now)
                }
                db.insertOrThrow("health_log", null, cv)
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
                db.update("health_log", patch, "id=? AND value IS NOT ?",
                    arrayOf(existingId, steps.toString()))
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
                db.insertOrThrow("health_log", null, cv)
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
            // The same HC sample can be corrected later; preserve its stable ID.
            val patch = ContentValues().apply { put("date", date); put("time", time); put("bpm", bpm) }
            db.update("heart_rate_samples", patch,
                "id=? AND (date IS NOT ? OR time IS NOT ? OR bpm IS NOT ?)",
                arrayOf(id, date, time, bpm.toString()))
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
