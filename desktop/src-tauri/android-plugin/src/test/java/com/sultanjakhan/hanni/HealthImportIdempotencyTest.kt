package com.sultanjakhan.hanni

import android.database.sqlite.SQLiteDatabase
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.SQLiteMode

/** Execute the real private importer methods against native SQLite and synthetic rows. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@SQLiteMode(SQLiteMode.Mode.NATIVE)
class HealthImportIdempotencyTest {
    private lateinit var db: SQLiteDatabase
    private lateinit var worker: HanniHealthWorker

    @Before fun setup() {
        db = SQLiteDatabase.create(null)
        // Import methods are pure DB operations; bypass only the WorkManager constructor.
        // This fixture does not claim to test scheduling or a Health Connect provider.
        val unsafeClass = Class.forName("sun.misc.Unsafe")
        val unsafe = unsafeClass.getDeclaredField("theUnsafe").apply { isAccessible = true }.get(null)
        worker = unsafeClass.getMethod("allocateInstance", Class::class.java)
            .invoke(unsafe, HanniHealthWorker::class.java) as HanniHealthWorker
        db.execSQL("CREATE TABLE observed_mutations(revision INTEGER PRIMARY KEY AUTOINCREMENT)")
        db.execSQL("CREATE TABLE cloud_relay_control(id INTEGER PRIMARY KEY,applying INTEGER NOT NULL)")
        db.execSQL("INSERT INTO cloud_relay_control VALUES(1,0)")
        db.execSQL("CREATE TABLE cloud_relay_dirty(seq INTEGER PRIMARY KEY AUTOINCREMENT,table_name TEXT NOT NULL,row_id TEXT NOT NULL,UNIQUE(table_name,row_id))")
        db.execSQL("CREATE TABLE sleep_sessions(id TEXT PRIMARY KEY,date TEXT,start_time TEXT,end_time TEXT,duration_minutes INTEGER,source TEXT,created_at TEXT,updated_at TEXT DEFAULT 'initial',UNIQUE(date,start_time,source))")
        db.execSQL("CREATE TABLE sleep_stages(id TEXT PRIMARY KEY,session_id TEXT,start_time TEXT,end_time TEXT,stage TEXT,updated_at TEXT DEFAULT 'initial')")
        db.execSQL("CREATE TABLE health_log(id TEXT PRIMARY KEY,date TEXT,type TEXT,value REAL,unit TEXT,notes TEXT,start_time TEXT,created_at TEXT,updated_at TEXT DEFAULT 'initial')")
        db.execSQL("CREATE TABLE heart_rate_samples(id TEXT PRIMARY KEY,date TEXT,time TEXT,bpm INTEGER,updated_at TEXT DEFAULT 'initial')")
        for ((table, columns) in mapOf(
            "sleep_sessions" to "date,start_time,end_time,duration_minutes,source",
            "sleep_stages" to "session_id,start_time,end_time,stage",
            "health_log" to "date,type,value,unit,notes,start_time",
            "heart_rate_samples" to "date,time,bpm",
        )) {
            // Same dirty-journal trigger SQL used by cloud_relay::initialize.
            for ((action, reference) in mapOf("INSERT" to "NEW", "UPDATE" to "NEW", "DELETE" to "OLD")) {
                db.execSQL("CREATE TRIGGER relay_${table}_$action AFTER $action ON $table " +
                    "WHEN (SELECT applying FROM cloud_relay_control WHERE id=1)=0 " +
                    "BEGIN INSERT OR REPLACE INTO cloud_relay_dirty(table_name,row_id) " +
                    "VALUES('$table',CAST($reference.id AS TEXT)); END")
            }
            for ((suffix, operation) in mapOf("insert" to "INSERT", "update" to "UPDATE OF $columns")) {
                db.execSQL("CREATE TRIGGER observe_${table}_$suffix AFTER $operation ON $table BEGIN " +
                    "INSERT INTO observed_mutations(revision) VALUES(NULL); " +
                    "UPDATE $table SET updated_at='revision:' || last_insert_rowid() WHERE id=NEW.id; END")
            }
        }
    }

    @After fun close() { db.close() }

    private fun records() = listOf(
        JSONObject().put("record_id", "sleep-one").put("date", "2026-01-01").put("start_time", "23:00")
            .put("end_time", "07:00").put("duration_minutes", 480)
            .put("stages", JSONArray().put(JSONObject().put("start_time", "23:00").put("end_time", "07:00").put("stage", "deep"))),
        JSONObject().put("date", "2026-01-01").put("steps", 123.0),
        JSONObject().put("record_id", "exercise-one").put("date", "2026-01-01").put("start_time", "10:00")
            .put("duration_minutes", 15.0).put("type", "walking").put("title", "synthetic"),
        JSONObject().put("record_id", "hr-one").put("sample_index", 0).put("date", "2026-01-01").put("time", "10:01").put("bpm", 70),
    )

    private fun importRecords(records: List<JSONObject>) {
        for ((index, name) in listOf("insertSleep", "insertSteps", "insertExercise", "insertHeartRate").withIndex()) {
            HanniHealthWorker::class.java.getDeclaredMethod(name, SQLiteDatabase::class.java, JSONArray::class.java)
                .apply { isAccessible = true }.invoke(worker, db, JSONArray().put(records[index]))
        }
    }

    private fun scalar(sql: String): String = db.rawQuery(sql, null).use { assertTrue(it.moveToFirst()); it.getString(0) }
    private fun snapshot(): Pair<Int, List<String>> {
        val stamps = mutableListOf<String>()
        for (table in listOf("sleep_sessions", "sleep_stages", "health_log", "heart_rate_samples")) {
            db.rawQuery("SELECT id || ':' || updated_at FROM $table ORDER BY id", null).use {
                while (it.moveToNext()) stamps.add(it.getString(0))
            }
        }
        return scalar("SELECT COUNT(*) FROM observed_mutations").toInt() to stamps
    }

    @Test fun identicalRepeatedWindowDoesNotStampOrQueueRows() {
        val values = records()
        importRecords(values)
        val first = snapshot()
        assertEquals(5, first.second.size)
        db.execSQL("DELETE FROM cloud_relay_dirty")
        importRecords(values)
        importRecords(values)
        assertEquals(first, snapshot())
        assertEquals("0", scalar("SELECT COUNT(*) FROM cloud_relay_dirty"))
    }

    @Test fun correctionsUpdateOnceAndPreserveStableIds() {
        val values = records()
        importRecords(values)
        val first = snapshot()
        values[0].put("end_time", "07:30").put("duration_minutes", 510)
        values[1].put("steps", 456.0)
        values[2].put("date", "2026-01-02").put("start_time", "11:00").put("title", "renamed")
        values[3].put("time", "10:02").put("bpm", 80)
        importRecords(values)
        assertEquals("510", scalar("SELECT duration_minutes FROM sleep_sessions WHERE id='sleep-one'"))
        assertEquals(456.0, scalar("SELECT value FROM health_log WHERE type='steps'").toDouble(), 0.0)
        assertEquals("2026-01-02:11:00:walking: renamed", scalar("SELECT date || ':' || start_time || ':' || notes FROM health_log WHERE id='health:exercise:exercise-one'"))
        assertEquals("80", scalar("SELECT bpm FROM heart_rate_samples WHERE id='health:hr:hr-one:0'"))
        assertEquals("10:02", scalar("SELECT time FROM heart_rate_samples WHERE id='health:hr:hr-one:0'"))
        val corrected = snapshot()
        assertEquals(first.first + 4, corrected.first)
        assertEquals(first.second.size, corrected.second.size)
        db.execSQL("DELETE FROM cloud_relay_dirty")
        importRecords(values)
        assertEquals(corrected, snapshot())
        assertEquals("0", scalar("SELECT COUNT(*) FROM cloud_relay_dirty"))
    }

    @Test fun correctedSleepDateAndStartKeepOriginalSessionAndStageIds() {
        val values = records()
        importRecords(values)
        val before = snapshot()
        values[0].put("date", "2026-01-02").put("start_time", "22:30").put("duration_minutes", 510)
        importRecords(values)
        assertEquals("2026-01-02:22:30:510", scalar("SELECT date || ':' || start_time || ':' || duration_minutes FROM sleep_sessions WHERE id='sleep-one'"))
        assertEquals("1", scalar("SELECT COUNT(*) FROM sleep_sessions"))
        val after = snapshot()
        assertEquals(before.first + 1, after.first)
        assertEquals(before.second.size, after.second.size)
        db.execSQL("DELETE FROM cloud_relay_dirty")
        importRecords(values)
        assertEquals(after, snapshot())
        assertEquals("0", scalar("SELECT COUNT(*) FROM cloud_relay_dirty"))
    }

    @Test fun correctedSleepConflictingWithAnotherSessionDoesNotRewriteEither() {
        val values = records()
        importRecords(values)
        db.execSQL("INSERT INTO sleep_sessions(id,date,start_time,end_time,duration_minutes,source) VALUES('sleep-other','2026-01-02','22:30','07:00',510,'health_connect')")
        val before = snapshot()
        db.execSQL("DELETE FROM cloud_relay_dirty")
        values[0].put("date", "2026-01-02").put("start_time", "22:30").put("duration_minutes", 510)
            .put("stages", JSONArray().put(JSONObject().put("start_time", "22:30").put("end_time", "07:00").put("stage", "light")))
        val failure = runCatching { importRecords(values) }.exceptionOrNull()
        assertTrue(failure is java.lang.reflect.InvocationTargetException)
        assertTrue(failure?.cause is android.database.sqlite.SQLiteConstraintException)
        assertEquals(before, snapshot())
        assertEquals("0", scalar("SELECT COUNT(*) FROM cloud_relay_dirty"))
    }
}
