package com.sultanjakhan.hanni

import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.HealthConnectFeatures
import androidx.health.connect.client.changes.DeletionChange
import androidx.health.connect.client.changes.UpsertionChange
import androidx.health.connect.client.records.Record
import androidx.health.connect.client.request.ChangesTokenRequest
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import org.json.JSONObject
import java.time.Instant

internal class RawHealthPage(val records: List<RawHealthObservation>, val nextPageToken: String?)
internal class RawHealthChangesPage(val changes: List<RawHealthChange>, val nextChangesToken: String,
    val hasMore: Boolean, val tokenExpired: Boolean)

/** Test seam models SDK pages, not a second on-device implementation. */
internal interface RawHealthSource {
    suspend fun grantedPermissions(): Set<String>
    fun featureAvailable(feature: Int): Boolean
    suspend fun newChangesToken(type: RawHealthRecordCodec.Descriptor): String
    suspend fun readPage(type: RawHealthRecordCodec.Descriptor, from: Instant, token: String?, pageSize: Int): RawHealthPage
    suspend fun changes(type: RawHealthRecordCodec.Descriptor, token: String): RawHealthChangesPage
}

// The actual SDK request is shared with a contract test. Initial archives start
// with their newest available records; future planned sessions remain included.
internal fun rawHealthReadRequest(type: RawHealthRecordCodec.Descriptor, from: Instant, token: String?, pageSize: Int) =
    ReadRecordsRequest(recordType = type.recordClass, timeRangeFilter = TimeRangeFilter.after(from),
        dataOriginFilter = emptySet(), ascendingOrder = false, pageToken = token, pageSize = pageSize)

internal class HealthConnectRawSource(private val client: HealthConnectClient) : RawHealthSource {
    override suspend fun grantedPermissions(): Set<String> = client.permissionController.getGrantedPermissions()
    override fun featureAvailable(feature: Int): Boolean =
        client.features.getFeatureStatus(feature) == HealthConnectFeatures.FEATURE_STATUS_AVAILABLE

    override suspend fun newChangesToken(type: RawHealthRecordCodec.Descriptor): String =
        client.getChangesToken(ChangesTokenRequest(recordTypes = setOf(type.recordClass)))

    override suspend fun readPage(type: RawHealthRecordCodec.Descriptor, from: Instant, token: String?, pageSize: Int): RawHealthPage {
        val response = client.readRecords(rawHealthReadRequest(type, from, token, pageSize))
        // The stable six-argument HC 1.1.0 constructor sets deduplicateStrategy=0.
        // The experimental seven-argument overload defaults to dedupe and is not used here.
        // No end-at-now filter: future PlannedExerciseSession records are part of the archive too.
        return RawHealthPage(response.records.map { observation(type, it) }, response.pageToken)
    }

    override suspend fun changes(type: RawHealthRecordCodec.Descriptor, token: String): RawHealthChangesPage {
        val response = try { client.getChanges(token) }
        catch (_: IllegalArgumentException) { return RawHealthChangesPage(emptyList(), "", false, true) }
        if (response.changesTokenExpired) return RawHealthChangesPage(emptyList(), "", false, true)
        val changes = response.changes.map { change ->
            when (change) {
                is UpsertionChange -> RawHealthChange.Upsert(observation(type, change.record))
                is DeletionChange -> RawHealthChange.Delete(change.recordId)
                else -> throw RawHealthImportException("hc_change_kind_unsupported")
            }
        }
        return RawHealthChangesPage(changes, response.nextChangesToken, response.hasMore, false)
    }

    private fun observation(type: RawHealthRecordCodec.Descriptor, record: Record): RawHealthObservation {
        val payload = RawHealthRecordCodec.encode(record)
        if (payload.getString("record_type") != type.name) throw RawHealthImportException("hc_change_type_mismatch")
        val fields = payload.getJSONObject("record")
        val point = time(fields, "time")
        return RawHealthObservation(record.metadata.id, payload.toString(), record.metadata.lastModifiedTime,
            point ?: time(fields, "startTime"), point ?: time(fields, "endTime"))
    }

    private fun time(fields: JSONObject, key: String): Instant? {
        val value = fields.optJSONObject(key) ?: return null
        return Instant.ofEpochSecond(value.getString("seconds").toLong(), value.getInt("nanos").toLong())
    }
}
