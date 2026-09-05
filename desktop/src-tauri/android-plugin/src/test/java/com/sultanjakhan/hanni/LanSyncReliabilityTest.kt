package com.sultanjakhan.hanni

import java.io.File
import java.net.InetAddress
import java.net.ServerSocket
import java.time.Instant
import java.util.concurrent.atomic.AtomicReference
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.SQLiteMode

/** Executes canonical Kotlin against Robolectric's native SQLite using synthetic rows only. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@SQLiteMode(SQLiteMode.Mode.NATIVE)
class LanSyncReliabilityTest {
    @get:Rule val temporaryFolder = TemporaryFolder()
    private lateinit var db: PlatformTestHealthDatabase
    private lateinit var databaseFile: File
    private val stamp = "2026-01-01T00:00:00Z"

    @Before fun setUp() {
        databaseFile = temporaryFolder.newFile("synthetic.db")
        db = PlatformTestHealthDatabase.openOrCreateDatabase(databaseFile, null)
        db.execSQL("CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
        db.execSQL("CREATE TABLE sync_tombstones(table_name TEXT NOT NULL,row_id TEXT NOT NULL,deleted_at TEXT NOT NULL,PRIMARY KEY(table_name,row_id))")
        db.execSQL("CREATE TABLE notes(id TEXT PRIMARY KEY,title TEXT NOT NULL,updated_at TEXT NOT NULL)")
        db.execSQL("CREATE TABLE sync_hlc_state(singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1),last_millis INTEGER NOT NULL)")
        db.execSQL("INSERT INTO sync_hlc_state(singleton,last_millis) VALUES(1,0)")
        db.execSQL("CREATE TABLE sync_apply_context(singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1),remote_apply INTEGER NOT NULL CHECK(remote_apply IN (0,1)),stamp_depth INTEGER NOT NULL CHECK(stamp_depth>=0))")
        db.execSQL("INSERT INTO sync_apply_context(singleton,remote_apply,stamp_depth) VALUES(1,0,0)")
        val tablesField = LanSyncDatabase::class.java.getDeclaredField("TABLES").apply { isAccessible = true }
        @Suppress("UNCHECKED_CAST")
        val tables = tablesField.get(null) as List<String>
        for (table in tables.filter { it != "notes" }) {
            if (table == "event_categories") db.execSQL("CREATE TABLE event_categories(id INTEGER PRIMARY KEY,name TEXT UNIQUE NOT NULL,updated_at TEXT NOT NULL)")
            else db.execSQL("CREATE TABLE $table(id TEXT PRIMARY KEY,updated_at TEXT NOT NULL)")
        }
        setting("lan_sync_peer", "synthetic-peer")
    }

    @After fun tearDown() { db.close() }

    private fun insert(id: String, time: String = stamp) {
        db.execSQL("INSERT INTO notes(id,title,updated_at) VALUES(?,?,?)", arrayOf(id, "synthetic", time))
    }

    private fun setting(key: String, value: String) {
        db.execSQL("INSERT OR REPLACE INTO app_settings(key,value) VALUES(?,?)", arrayOf(key, value))
    }

    private fun settings(): Map<String, String> = linkedMapOf<String, String>().also { out ->
        db.rawQuery("SELECT key,value FROM app_settings ORDER BY key", null).use { cursor ->
            while (cursor.moveToNext()) out[cursor.getString(0)] = cursor.getString(1)
        }
    }

    private fun count(id: String): Int = db.rawQuery("SELECT count(*) FROM notes WHERE id=?", arrayOf(id)).use {
        it.moveToFirst(); it.getInt(0)
    }

    private fun row(id: String, time: String, valid: Boolean = true): JSONObject {
        val fields = JSONObject().put("id", id).put("updated_at", time).put("_updated_at", time)
        if (valid) fields.put("title", "synthetic")
        return JSONObject().put("t", "notes").put("f", fields)
    }

    private fun response(vararg rows: JSONObject) = JSONObject()
        .put("rows", JSONArray(rows.toList())).put("tombs", JSONArray())

    private fun outboundIds(outbound: LanSyncDatabase.Outbound): Set<String> = (0 until outbound.rows.length())
        .map { outbound.rows.getJSONObject(it) }.filter { it.getString("t") == "notes" }
        .map { it.getJSONObject("f").getString("id") }.toSet()

    @Test fun all501RowsWithIdenticalTimestampEventuallyLeaveTheQueue() {
        repeat(501) { insert("local-${it.toString().padStart(4, '0')}") }
        val sent = mutableSetOf<String>()
        repeat(8) {
            val outbound = LanSyncDatabase.gather(db)
            sent.addAll(outboundIds(outbound))
            LanSyncDatabase.applyResponse(db, outbound, response())
        }
        assertEquals("No row may disappear behind a timestamp-only page boundary", 501, sent.size)
    }

    @Test fun all501TombstonesWithIdenticalTimestampEventuallyLeaveTheQueue() {
        repeat(501) { db.execSQL("INSERT INTO sync_tombstones(table_name,row_id,deleted_at) VALUES('notes',?,?)", arrayOf("deleted-$it", stamp)) }
        val sent = mutableSetOf<String>()
        repeat(8) {
            val outbound = LanSyncDatabase.gather(db)
            repeat(outbound.tombs.length()) { index -> sent.add(outbound.tombs.getJSONObject(index).getString("id")) }
            LanSyncDatabase.applyResponse(db, outbound, response())
        }
        assertEquals(501, sent.size)
    }

    @Test fun rawOwnedSleepRowsAndTombsAreFilteredBeforePageCutoffWithoutHidingLegacy() {
        db.execSQL("CREATE TABLE hc_sleep_projection_owned(table_name TEXT NOT NULL,row_id TEXT NOT NULL,raw_id TEXT NOT NULL,PRIMARY KEY(table_name,row_id))")
        db.execSQL("ALTER TABLE events ADD COLUMN source TEXT")
        repeat(501) { i ->
            val id = "raw-sleep:${i.toString().padStart(4, '0')}"
            db.execSQL("INSERT INTO sleep_sessions(id,updated_at) VALUES(?,?)", arrayOf(id,stamp))
            db.execSQL("INSERT INTO sync_tombstones VALUES('sleep_sessions',?,?)", arrayOf(id,stamp))
            val event = "projected-event-$i"
            db.execSQL("INSERT INTO events(id,source,updated_at) VALUES(?,'auto_health_raw:synthetic',?)", arrayOf(event,stamp))
            db.execSQL("INSERT INTO hc_sleep_projection_owned VALUES('events',?,'synthetic-raw')", arrayOf(event))
            db.execSQL("INSERT INTO sync_tombstones VALUES('events',?,?)", arrayOf(event,stamp))
        }
        val newer = "2026-01-02T00:00:00Z"
        db.execSQL("INSERT INTO sleep_sessions(id,updated_at) VALUES('legacy-sleep',?)", arrayOf(newer))
        db.execSQL("INSERT INTO sync_tombstones VALUES('sleep_sessions','legacy-tomb',?)", arrayOf(newer))
        val outbound = LanSyncDatabase.gather(db)
        assertEquals(1,outbound.rows.length())
        assertEquals("legacy-sleep",outbound.rows.getJSONObject(0).getJSONObject("f").getString("id"))
        assertEquals(1,outbound.tombs.length())
        assertEquals("legacy-tomb",outbound.tombs.getJSONObject(0).getString("id"))
        LanSyncDatabase.applyResponse(db,outbound,response())
        val again = LanSyncDatabase.gather(db)
        assertEquals(0,again.rows.length())
        assertEquals(0,again.tombs.length())
    }

    @Test fun staleResponseCannotOverwriteNewerAcknowledgement() {
        insert("local")
        val first = LanSyncDatabase.gather(db)
        val stale = LanSyncDatabase.gather(db)
        LanSyncDatabase.applyResponse(db, first, response())
        val acknowledged = settings()
        assertTrue(runCatching { LanSyncDatabase.applyResponse(db, stale, response(row("remote", "2030-01-01T00:00:00Z"))) }.isFailure)
        assertEquals(acknowledged, settings())
        assertEquals(0, count("remote"))
        assertEquals(0L, scalar("SELECT remote_apply+stamp_depth FROM sync_apply_context WHERE singleton=1"))
    }

    @Test fun remoteApplyPreservesTimestampAndAdvancesLocalClockBeyondSubmillisecond() {
        val outbound = LanSyncDatabase.gather(db)
        // Same insert-stamping trigger logic as db.rs, installed only for this
        // test so local fixture writes in pagination tests retain chosen times.
        val wallMillis = "(CAST(strftime('%s','now') AS INTEGER)*1000 + CAST(substr(strftime('%f','now'),4,3) AS INTEGER))"
        val renderedHlc = "strftime('%Y-%m-%dT%H:%M:%fZ',(SELECT last_millis FROM sync_hlc_state WHERE singleton=1)/1000.0,'unixepoch')"
        db.execSQL("CREATE TRIGGER notes_set_updated_at_on_insert AFTER INSERT ON notes FOR EACH ROW WHEN (SELECT remote_apply=0 AND stamp_depth=0 FROM sync_apply_context WHERE singleton=1) BEGIN UPDATE sync_hlc_state SET last_millis=MAX(last_millis+1,$wallMillis) WHERE singleton=1; UPDATE sync_apply_context SET stamp_depth=stamp_depth+1 WHERE singleton=1; UPDATE notes SET updated_at=$renderedHlc WHERE rowid=NEW.rowid; UPDATE sync_apply_context SET stamp_depth=stamp_depth-1 WHERE singleton=1; END")
        val remote = "2030-01-01T00:00:00.000000001Z"
        LanSyncDatabase.applyResponse(db, outbound, response(row("remote", remote)))
        val stored = db.rawQuery("SELECT updated_at FROM notes WHERE id='remote'", null).use { it.moveToFirst(); it.getString(0) }
        assertEquals(Instant.parse(remote), Instant.parse(stored))
        assertEquals(Instant.parse("2030-01-01T00:00:00Z").toEpochMilli() + 1, scalar("SELECT last_millis FROM sync_hlc_state WHERE singleton=1"))
        assertEquals(0L, scalar("SELECT remote_apply+stamp_depth FROM sync_apply_context WHERE singleton=1"))
    }

    @Test fun oldInboundTombstoneCannotWeakenANewerKnownDeletion() {
        db.execSQL("INSERT INTO sync_tombstones(table_name,row_id,deleted_at) VALUES('notes','deleted',?)", arrayOf("2030-01-01T00:00:00Z"))
        val outbound = LanSyncDatabase.gather(db)
        val reply = response().put("tombs", JSONArray().put(JSONObject().put("tt", "notes").put("id", "deleted").put("deleted_at", stamp)))
        LanSyncDatabase.applyResponse(db, outbound, reply)
        val stored = db.rawQuery("SELECT deleted_at FROM sync_tombstones WHERE table_name='notes' AND row_id='deleted'", null).use { it.moveToFirst(); it.getString(0) }
        assertEquals(Instant.parse("2030-01-01T00:00:00Z"), Instant.parse(stored))
    }

    @Test fun newerInboundTimestampDoesNotSkipUnsentOlderPages() {
        repeat(601) { insert("local-${it.toString().padStart(4, '0')}", Instant.parse(stamp).plusSeconds(it.toLong()).toString()) }
        val sent = mutableSetOf<String>()
        val first = LanSyncDatabase.gather(db)
        sent.addAll(outboundIds(first))
        LanSyncDatabase.applyResponse(db, first, response(row("remote", "2026-12-31T00:00:00Z")))
        repeat(8) {
            val outbound = LanSyncDatabase.gather(db)
            sent.addAll(outboundIds(outbound).filter { it.startsWith("local-") })
            LanSyncDatabase.applyResponse(db, outbound, response())
        }
        assertEquals("Receiving a newer row cannot acknowledge unsent local rows", 601, sent.size)
        assertEquals(1, count("remote"))
    }

    @Test fun malformedInboundRowRollsBackEarlierRowsAndAcknowledgement() {
        insert("local")
        val outbound = LanSyncDatabase.gather(db)
        val before = settings()
        val result = runCatching {
            LanSyncDatabase.applyResponse(db, outbound, response(
                row("remote-valid", "2026-02-01T00:00:00Z"),
                row("remote-missing-required-title", "2026-02-02T00:00:00Z", valid = false),
            ))
        }
        assertTrue("Malformed data must fail instead of silently acknowledging a partial page", result.isFailure)
        assertEquals("Earlier rows in the failed response must roll back", 0, count("remote-valid"))
        assertEquals(before, settings())
        assertEquals(0L, scalar("SELECT last_millis FROM sync_hlc_state WHERE singleton=1"))
        assertEquals(0L, scalar("SELECT remote_apply+stamp_depth FROM sync_apply_context WHERE singleton=1"))
        assertEquals(setOf("local"), outboundIds(LanSyncDatabase.gather(db)))
    }

    @Test fun malformed200ResponseCannotAcknowledgeOutboundRecords() {
        insert("local")
        withServer("{}") { address, _ ->
            configure(address)
            val before = settings()
            assertTrue(runCatching { BackgroundLanSync.runConfigured(db) }.isFailure)
            assertEquals(before, settings())
            assertEquals(setOf("local"), outboundIds(LanSyncDatabase.gather(db)))
        }
    }

    @Test fun unavailablePeerLeavesDurableQueueForSuccessfulRetry() {
        insert("local")
        val unavailablePort = ServerSocket(0).use { it.localPort }
        configure("127.0.0.1:$unavailablePort")
        val before = settings()
        assertTrue(runCatching { BackgroundLanSync.runConfigured(db) }.isFailure)
        assertEquals(before, settings())
        db.close()
        db = PlatformTestHealthDatabase.openOrCreateDatabase(databaseFile, null)
        assertEquals(setOf("local"), outboundIds(LanSyncDatabase.gather(db)))
        withServer(response().toString()) { address, request ->
            configure(address)
            val result = BackgroundLanSync.runConfigured(db)
            assertNotNull(result)
            assertEquals(1, result!!.sent)
            assertEquals("local", request.get()!!.getJSONArray("rows").getJSONObject(0).getJSONObject("f").getString("id"))
            assertTrue("Only a successful response may clear the pending batch", outboundIds(LanSyncDatabase.gather(db)).isEmpty())
        }
    }

    private fun configure(peer: String) {
        setting("lan_sync_enabled", "true")
        setting("lan_sync_peer", peer)
        setting("lan_sync_key", "synthetic-test-key")
    }

    private fun scalar(sql: String): Long = db.rawQuery(sql, null).use { it.moveToFirst(); it.getLong(0) }

    private fun withServer(body: String, block: (String, AtomicReference<JSONObject?>) -> Unit) {
        val captured = AtomicReference<JSONObject?>(null)
        val server = ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))
        val serverFailure = AtomicReference<Throwable?>(null)
        val serverThread = Thread {
            try {
                server.accept().use { socket ->
                    socket.soTimeout = 5000
                    val input = socket.getInputStream().buffered()
                    fun line(): String {
                        val text = StringBuilder()
                        while (true) {
                            val byte = input.read()
                            if (byte == -1 || byte == 10) return text.toString().trimEnd('\r')
                            text.append(byte.toChar())
                        }
                    }
                    line()
                    var contentLength = 0
                    while (true) {
                        val header = line()
                        if (header.isEmpty()) break
                        if (header.startsWith("Content-Length:", ignoreCase = true)) {
                            contentLength = header.substringAfter(':').trim().toInt()
                        }
                    }
                    val requestBytes = ByteArray(contentLength)
                    var offset = 0
                    while (offset < contentLength) {
                        val read = input.read(requestBytes, offset, contentLength - offset)
                        check(read > 0) { "Truncated synthetic HTTP request" }
                        offset += read
                    }
                    captured.set(JSONObject(String(requestBytes, Charsets.UTF_8)))
                    val bytes = body.toByteArray(Charsets.UTF_8)
                    socket.getOutputStream().use { output ->
                        output.write(("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ${bytes.size}\r\nConnection: close\r\n\r\n").toByteArray(Charsets.US_ASCII))
                        output.write(bytes)
                        output.flush()
                    }
                }
            } catch (error: Throwable) { serverFailure.set(error) }
        }
        serverThread.isDaemon = true
        serverThread.start()
        try { block("127.0.0.1:${server.localPort}", captured) } finally {
            server.close()
            serverThread.join(5000)
        }
        serverFailure.get()?.let { throw AssertionError("Synthetic HTTP server failed", it) }
    }
}
