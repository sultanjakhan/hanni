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
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.TimeoutCancellationException

internal enum class RawHealthFailureStage { READ, ENCODE, COMMIT, CURSOR }

/** Fixed metadata only. Never format a Throwable, its message, stack, or request/record. */
@PublishedApi
internal fun recordRawHealthFailure(type: RawHealthRecordCodec.Descriptor, phase: String,
    stage: RawHealthFailureStage, error: Exception) {
    try {
        val name = RawHealthRecordCodec.descriptors.firstOrNull { it == type }?.name ?: return
        val safePhase = phase.takeIf { it in setOf("idle", "snapshot", "replay", "changes") } ?: "unknown"
        val codes = setOf("hc_database_page_failed", "hc_checkpoint_unavailable", "hc_checkpoint_stale",
            "hc_checkpoint_write_failed", "hc_record_type_unsupported", "hc_revision_exhausted",
            "hc_source_identity_unavailable", "hc_invalid_text", "hc_change_kind_unsupported",
            "hc_change_type_mismatch", "hc_token_invalid", "hc_page_token_repeated",
            "hc_changes_token_invalid", "hc_checkpoint_phase_invalid", "hc_budget_exhausted")
        val code = (error as? RawHealthImportException)?.code?.takeIf { it in codes }
            ?: if (error is TimeoutCancellationException) "source_timeout"
            else if (error is SecurityException) "permission_required" else "hc_source_read_failed"
        val category = when (error) {
            is TimeoutCancellationException -> "timeout"
            is RawHealthImportException -> "import"
            is SecurityException -> "security"
            is android.database.sqlite.SQLiteException -> "sqlite"
            is android.os.RemoteException -> "remote"
            is java.io.IOException -> "io"
            is IllegalArgumentException -> "argument"
            is IllegalStateException -> "state"
            else -> "other"
        }
        var healthCode = 0
        if (android.os.Build.VERSION.SDK_INT >= 34) {
            // SDK wrappers can retain the platform cause. Bound traversal; no messages.
            var cause: Throwable? = error
            repeat(4) {
                if (cause is android.health.connect.HealthConnectException) {
                    val candidate = (cause as android.health.connect.HealthConnectException).errorCode
                    if (candidate in 1..9) healthCode = candidate // Android SDK 36 fixed error constants.
                }
                cause = cause?.cause
            }
        }
        android.util.Log.w("HanniWorkerDiag", "hc_page_failure type=$name phase=$safePhase stage=${stage.name} code=$code category=$category hc_error=$healthCode")
    } catch (_: Exception) { /* Diagnostics must not change import behavior. */ }
    catch (_: LinkageError) { /* Older Android must retain the original failure. */ }
}

internal inline fun <T> observeRawHealthFailure(type: RawHealthRecordCodec.Descriptor, phase: String,
    stage: RawHealthFailureStage, action: () -> T): T = try { action() }
catch (error: Exception) {
    if (error !is CancellationException || error is TimeoutCancellationException) {
        recordRawHealthFailure(type, phase, stage, error)
    }
    throw error
}

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
        val response = observeRawHealthFailure(type, "snapshot", RawHealthFailureStage.READ) {
            client.readRecords(rawHealthReadRequest(type, from, token, pageSize))
        }
        // The stable six-argument HC 1.1.0 constructor sets deduplicateStrategy=0.
        // The experimental seven-argument overload defaults to dedupe and is not used here.
        // No end-at-now filter: future PlannedExerciseSession records are part of the archive too.
        return observeRawHealthFailure(type, "snapshot", RawHealthFailureStage.ENCODE) {
            RawHealthPage(response.records.map { observation(type, it) }, response.pageToken)
        }
    }

    override suspend fun changes(type: RawHealthRecordCodec.Descriptor, token: String): RawHealthChangesPage {
        val response = observeRawHealthFailure(type, "changes", RawHealthFailureStage.READ) {
            try { client.getChanges(token) }
            catch (_: IllegalArgumentException) { return RawHealthChangesPage(emptyList(), "", false, true) }
        }
        if (response.changesTokenExpired) return RawHealthChangesPage(emptyList(), "", false, true)
        val changes = observeRawHealthFailure(type, "changes", RawHealthFailureStage.ENCODE) {
            response.changes.map { change ->
                when (change) {
                    is UpsertionChange -> RawHealthChange.Upsert(observation(type, change.record))
                    is DeletionChange -> RawHealthChange.Delete(change.recordId)
                    else -> throw RawHealthImportException("hc_change_kind_unsupported")
                }
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
