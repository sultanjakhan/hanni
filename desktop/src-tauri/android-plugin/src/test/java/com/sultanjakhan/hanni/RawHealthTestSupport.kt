package com.sultanjakhan.hanni

import androidx.health.connect.client.records.StepsRecord
import androidx.health.connect.client.records.metadata.Metadata
import org.json.JSONObject
import java.time.Instant

internal object RawHealthTestSupport {
    val now: Instant = Instant.ofEpochSecond(1_700_000_000, 123_456_789)
    const val type = "StepsRecord"
    const val storeId = "00000000-0000-4000-8000-000000000001"
    fun initialize(db: HealthDatabase) {
        val sql = checkNotNull(javaClass.getResourceAsStream("/health-records-schema.sql")) {
            "The actual shared SQL asset must be included as a test resource"
        }.bufferedReader(Charsets.UTF_8).use { it.readText() }
        RawHealthRecordStore.initialize(db, sql)
    }
    fun observation(id: String, count: Long = 123, modified: Instant = now): RawHealthObservation {
        val source = StepsRecord(now, null, now.plusSeconds(5), null, count, Metadata.manualEntry())
        val payload = RawHealthRecordCodec.encode(source)
        payload.getJSONObject("record").getJSONObject("metadata").apply {
            put("id", id)
            put("lastModifiedTime", JSONObject().put("seconds", modified.epochSecond.toString()).put("nanos", modified.nano))
        }
        return RawHealthObservation(id, payload.toString(), modified, now, now.plusSeconds(5))
    }
    fun scalar(db: HealthDatabase, sql: String): String = db.rawQuery(sql, emptyArray()).use {
        check(it.moveToFirst()); it.getString(0)
    }
}
