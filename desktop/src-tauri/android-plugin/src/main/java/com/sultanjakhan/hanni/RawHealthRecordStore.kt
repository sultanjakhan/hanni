package com.sultanjakhan.hanni

import android.content.ContentValues
import android.content.Context
import org.json.JSONObject
import java.time.Instant
import java.time.format.DateTimeFormatterBuilder
import java.util.Locale

internal class RawHealthObservation(
    val hcId: String, val payloadJson: String, val modifiedAt: Instant,
    val startTime: Instant?, val endTime: Instant?,
)

internal sealed class RawHealthChange {
    class Upsert(val observation: RawHealthObservation) : RawHealthChange()
    class Delete(val hcId: String) : RawHealthChange()
}

internal data class RawHealthCheckpoint(
    val type: String,
    val version: Long = 0,
    val phase: String = "idle",
    val bootstrapToken: String? = null,
    val changesToken: String? = null,
    val pageToken: String? = null,
    val scanFrom: Instant? = null,
    val firstGrantObserved: Instant? = null,
    val historyGranted: Boolean = false,
    val historyCoverage: String = "not_started",
    val deletionGap: Boolean = false,
    val needsCatchup: Boolean = true,
    val status: String = "not_started",
    val lastAttempt: Instant? = null,
    val lastSuccess: Instant? = null,
    val notBefore: Instant? = null,
) {
    override fun toString(): String = "RawHealthCheckpoint(redacted)"
}

/** Source-only persistence. The common SQL asset supplies the same relay dirty triggers as Rust. */
internal class RawHealthRecordStore(private val db: HealthDatabase, val sourceStoreId: String) {
    companion object {
        private val timestamp = DateTimeFormatterBuilder().appendInstant(9).toFormatter(Locale.ROOT)
        fun iso(value: Instant): String = timestamp.format(value)
        fun initialize(db: HealthDatabase, context: Context) = initialize(
            db, context.assets.open("health-records-schema.sql").bufferedReader(Charsets.UTF_8).use { it.readText() },
        )

        fun initialize(db: HealthDatabase, sharedSql: String) {
            transaction(db) {
                require(sharedSql.contains("-- hanni-statement"))
                for (statement in sharedSql.split("-- hanni-statement")) {
                    if (statement.lineSequence().any { it.isNotBlank() && !it.trimStart().startsWith("--") }) {
                        db.execSQL(statement)
                    }
                }
                db.execSQL("""CREATE TABLE IF NOT EXISTS hc_raw_import_state(
                    source_store_id TEXT NOT NULL, record_type TEXT NOT NULL,
                    state_version INTEGER NOT NULL DEFAULT 0, phase TEXT NOT NULL DEFAULT 'idle',
                    bootstrap_token TEXT, changes_token TEXT, page_token TEXT, scan_from_utc TEXT,
                    first_grant_observed_at TEXT, history_granted INTEGER NOT NULL DEFAULT 0,
                    history_coverage TEXT NOT NULL DEFAULT 'not_started', deletion_gap INTEGER NOT NULL DEFAULT 0,
                    needs_catchup INTEGER NOT NULL DEFAULT 1, status TEXT NOT NULL DEFAULT 'not_started',
                    last_attempt_at TEXT, last_success_at TEXT, not_before_utc TEXT,
                    PRIMARY KEY(source_store_id,record_type))""")
            }
        }

        private fun <T> transaction(db: HealthDatabase, action: () -> T): T {
            try {
                check(!db.inTransaction())
                db.beginTransaction()
                try {
                    val result = action()
                    db.setTransactionSuccessful()
                    return result
                } finally { db.endTransaction() }
            } catch (error: RawHealthImportException) { throw error }
            catch (_: Exception) { throw RawHealthImportException("hc_database_page_failed") }
        }
    }

    fun checkpoint(type: String): RawHealthCheckpoint {
        requireType(type)
        try {
            db.rawQuery("SELECT * FROM hc_raw_import_state WHERE source_store_id=? AND record_type=?",
                arrayOf(sourceStoreId, type)).use { cursor ->
                if (!cursor.moveToFirst()) return RawHealthCheckpoint(type)
                fun text(name: String): String? = cursor.getColumnIndexOrThrow(name).let { if (cursor.isNull(it)) null else cursor.getString(it) }
                fun instant(name: String) = text(name)?.let(Instant::parse)
                fun number(name: String) = cursor.getLong(cursor.getColumnIndexOrThrow(name))
                return RawHealthCheckpoint(type, number("state_version"), text("phase")!!,
                    text("bootstrap_token"), text("changes_token"), text("page_token"), instant("scan_from_utc"),
                    instant("first_grant_observed_at"), number("history_granted") == 1L, text("history_coverage")!!,
                    number("deletion_gap") == 1L, number("needs_catchup") == 1L, text("status")!!,
                    instant("last_attempt_at"), instant("last_success_at"), instant("not_before_utc"))
            }
        } catch (_: Exception) { throw RawHealthImportException("hc_checkpoint_unavailable") }
    }

    fun commit(expected: RawHealthCheckpoint, next: RawHealthCheckpoint,
        changes: List<RawHealthChange> = emptyList(), observedAt: Instant): Int = transaction(db) {
        requireType(expected.type)
        require(next.type == expected.type)
        val current = checkpoint(expected.type)
        if (current.version != expected.version) throw RawHealthImportException("hc_checkpoint_stale")
        db.rawQuery("SELECT applying FROM cloud_relay_control WHERE id=1", emptyArray()).use {
            check(it.moveToFirst() && it.getInt(0) == 0)
        }
        var modified = 0
        for (change in changes) {
            modified += when (change) {
                is RawHealthChange.Upsert -> upsert(expected.type, change.observation, observedAt)
                is RawHealthChange.Delete -> delete(expected.type, change.hcId, observedAt)
            }
        }
        val values = ContentValues().apply {
            put("source_store_id", sourceStoreId); put("record_type", next.type)
            put("state_version", increment(expected.version)); put("phase", next.phase)
            put("bootstrap_token", next.bootstrapToken); put("changes_token", next.changesToken)
            put("page_token", next.pageToken); put("scan_from_utc", next.scanFrom?.let(::iso))
            put("first_grant_observed_at", next.firstGrantObserved?.let(::iso))
            put("history_granted", if (next.historyGranted) 1 else 0)
            put("history_coverage", next.historyCoverage); put("deletion_gap", if (next.deletionGap) 1 else 0)
            put("needs_catchup", if (next.needsCatchup) 1 else 0); put("status", next.status)
            put("last_attempt_at", next.lastAttempt?.let(::iso)); put("last_success_at", next.lastSuccess?.let(::iso))
            put("not_before_utc", next.notBefore?.let(::iso))
        }
        db.insertWithOnConflict("hc_raw_import_state", null, values, HealthDatabase.CONFLICT_REPLACE)
            .also { if (it == -1L) throw RawHealthImportException("hc_checkpoint_write_failed") }
        modified
    }

    private class Existing(val revision: Long, val payload: String, val hash: String, val deleted: Boolean,
        val modified: Instant, val updated: Instant)

    private fun existing(id: String): Existing? = db.rawQuery(
        "SELECT source_revision,payload_json,payload_sha256,is_deleted,metadata_modified_at,updated_at FROM health_records WHERE id=?",
        arrayOf(id)).use { cursor ->
        if (!cursor.moveToFirst()) null else Existing(cursor.getLong(0), cursor.getString(1), cursor.getString(2),
            cursor.getInt(3) == 1, Instant.parse(cursor.getString(4)), Instant.parse(cursor.getString(5)))
    }

    private fun upsert(type: String, value: RawHealthObservation, now: Instant): Int {
        require(value.hcId.isNotEmpty())
        val payload = JSONObject(value.payloadJson)
        require(payload.getInt("v") == 1 && payload.getString("record_type") == type)
        require(payload.getString("sdk") == RawHealthRecordCodec.SDK)
        val metadata = payload.getJSONObject("record").getJSONObject("metadata")
        require(metadata.getString("id") == value.hcId)
        val modified = metadata.getJSONObject("lastModifiedTime")
        require(Instant.ofEpochSecond(modified.getString("seconds").toLong(), modified.getInt("nanos").toLong()) == value.modifiedAt)
        val id = RawHealthIdentity.globalId(sourceStoreId, type, value.hcId)
        val hash = RawHealthIdentity.sha256(value.payloadJson)
        val old = existing(id)
        // HC cursor order, not a device wall clock, determines the source revision.
        // A real correction can have an older lastModifiedTime after clock rollback.
        if (old != null && !old.deleted && old.hash == hash && old.payload == value.payloadJson) return 0
        val values = ContentValues().apply {
            put("id", id); put("source_store_id", sourceStoreId); put("record_type", type); put("hc_record_id", value.hcId)
            put("source_revision", increment(old?.revision ?: 0)); put("metadata_modified_at", iso(value.modifiedAt))
            put("time_start_utc", value.startTime?.let(::iso)); put("time_end_utc", value.endTime?.let(::iso))
            put("payload_version", 1); put("payload_json", value.payloadJson); put("payload_sha256", hash)
            put("is_deleted", 0); putNull("deletion_basis"); put("observed_at", iso(now))
            put("updated_at", iso(nextTime(now, old?.updated)))
        }
        if (old == null) db.insertOrThrow("health_records", null, values)
        else check(db.update("health_records", values, "id=?", arrayOf(id)) == 1)
        return 1
    }

    private fun delete(type: String, hcId: String, now: Instant): Int {
        require(hcId.isNotEmpty())
        val id = RawHealthIdentity.globalId(sourceStoreId, type, hcId)
        val old = existing(id)
        if (old?.deleted == true) return 0
        val values = ContentValues().apply {
            put("source_revision", increment(old?.revision ?: 0)); put("is_deleted", 1)
            put("deletion_basis", "getChanges"); put("observed_at", iso(now))
            put("updated_at", iso(nextTime(now, old?.updated)))
        }
        if (old == null) {
            val placeholder = JSONObject().put("v", 1).put("record_type", type).put("hc_record_id", hcId).put("deleted", true).toString()
            values.put("id", id); values.put("source_store_id", sourceStoreId); values.put("record_type", type)
            values.put("hc_record_id", hcId); values.put("metadata_modified_at", iso(now)); values.put("payload_version", 1)
            values.put("payload_json", placeholder); values.put("payload_sha256", RawHealthIdentity.sha256(placeholder))
            db.insertOrThrow("health_records", null, values)
        } else check(db.update("health_records", values, "id=?", arrayOf(id)) == 1)
        return 1
    }

    private fun requireType(type: String) {
        if (RawHealthRecordCodec.descriptors.none { it.name == type }) throw RawHealthImportException("hc_record_type_unsupported")
    }
    private fun increment(value: Long): Long {
        if (value == Long.MAX_VALUE) throw RawHealthImportException("hc_revision_exhausted")
        return value + 1
    }
    private fun nextTime(now: Instant, old: Instant?): Instant = if (old != null && now <= old) old.plusNanos(1) else now
}
