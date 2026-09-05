package com.sultanjakhan.hanni

import androidx.health.connect.client.HealthConnectFeatures
import androidx.health.connect.client.permission.HealthPermission
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.withTimeout
import java.time.Duration
import java.time.Instant

internal data class RawHealthTypeStatus(val type: String, val granted: Boolean, val featureAvailable: Boolean,
    val backgroundAllowed: Boolean, val historyGranted: Boolean, val historyStatus: String, val historyCoverage: String,
    val phase: String, val status: String, val deletionGap: Boolean)

internal class RawHealthImportResult(val types: List<RawHealthTypeStatus>, val pages: Int,
    val modifiedRecords: Int, val morePending: Boolean, val retryNeeded: Boolean)

/** Bounded, restartable import engine. Call on Dispatchers.IO; it owns no Worker/Activity lifetime. */
internal class RawHealthImporter(
    private val store: RawHealthRecordStore,
    private val source: RawHealthSource,
    private val descriptors: List<RawHealthRecordCodec.Descriptor> = RawHealthRecordCodec.descriptors,
    private val clock: () -> Instant = Instant::now,
    private val elapsedNanos: () -> Long = System::nanoTime,
) {
    data class Budget(val maxCalls: Int = 8, val maxPages: Int = 6, val pageSize: Int = 100,
        val maxDurationMillis: Long = 20_000, val pollInterval: Duration = Duration.ofMinutes(5))

    suspend fun runOnce(background: Boolean, budget: Budget = Budget()): RawHealthImportResult {
        require(budget.maxCalls > 0 && budget.maxPages > 0 && budget.pageSize in 1..1000 && budget.maxDurationMillis > 0)
        val started = elapsedNanos()
        val granted = try { withTimeout(budget.maxDurationMillis) { source.grantedPermissions() } }
        catch (_: TimeoutCancellationException) { throw RawHealthImportException("hc_permissions_timeout") }
        catch (error: CancellationException) { throw error }
        catch (_: Exception) { throw RawHealthImportException("hc_permissions_unavailable") }
        val featureFailures = mutableSetOf<Int>()
        fun probe(feature: Int): Boolean = try { source.featureAvailable(feature) }
        catch (error: CancellationException) { throw error }
        catch (_: Exception) { featureFailures.add(feature); false }
        val backgroundGranted = HealthPermission.PERMISSION_READ_HEALTH_DATA_IN_BACKGROUND in granted &&
            probe(HealthConnectFeatures.FEATURE_READ_HEALTH_DATA_IN_BACKGROUND)
        val backgroundAllowed = !background || backgroundGranted
        val historyGranted = HealthPermission.PERMISSION_READ_HEALTH_DATA_HISTORY in granted &&
            probe(HealthConnectFeatures.FEATURE_READ_HEALTH_DATA_HISTORY)
        val available = descriptors.associate { type -> type.name to
            (type.requiredFeature == null || probe(type.requiredFeature)) }
        val historyStatus = if (HealthConnectFeatures.FEATURE_READ_HEALTH_DATA_HISTORY in featureFailures) "history_feature_probe_failed"
            else if (historyGranted) "history_granted"
            else if (HealthPermission.PERMISSION_READ_HEALTH_DATA_HISTORY !in granted) "history_permission_missing"
            else "history_feature_unavailable"
        val deniedDuringRead = mutableSetOf<String>()
        var calls = 0
        var pages = 0
        var modified = 0
        var retry = featureFailures.isNotEmpty()
        fun remainingMillis(): Long = budget.maxDurationMillis - (elapsedNanos() - started) / 1_000_000
        fun canCall(): Boolean = calls < budget.maxCalls && pages < budget.maxPages && remainingMillis() > 0
        suspend fun <T> request(action: suspend () -> T): T {
            if (!canCall()) throw RawHealthImportException("hc_budget_exhausted")
            calls++
            return withTimeout(remainingMillis().coerceAtLeast(1)) { action() }
        }
        fun due(state: RawHealthCheckpoint, now: Instant): Boolean =
            (state.notBefore == null || now >= state.notBefore) &&
                (state.phase != "changes" || state.needsCatchup || state.lastSuccess == null ||
                    now >= state.lastSuccess.plus(budget.pollInterval) || (historyGranted && !state.historyGranted))

        // Least recently attempted first prevents a large first type or repeated failure starving others.
        val ordered = descriptors.sortedWith(compareBy { store.checkpoint(it.name).lastAttempt ?: Instant.MIN })
        for (type in ordered) {
            var state = store.checkpoint(type.name)
            fun commit(next: RawHealthCheckpoint, changes: List<RawHealthChange> = emptyList(), now: Instant = clock()) {
                modified += store.commit(state, next, changes, now)
                state = next.copy(version = state.version + 1)
            }
            val permitted = type.readPermission in granted
            val supported = available.getValue(type.name)
            val now = clock()
            if (!permitted || !supported || !backgroundAllowed) {
                val status = if (type.requiredFeature in featureFailures) "feature_probe_failed"
                    else if (!supported) "feature_unavailable" else if (!permitted) "permission_required"
                    else if (HealthConnectFeatures.FEATURE_READ_HEALTH_DATA_IN_BACKGROUND in featureFailures) "background_feature_probe_failed"
                    else "background_permission_required"
                val firstGrant = if (permitted) state.firstGrantObserved ?: now else state.firstGrantObserved
                if (state.status != status || firstGrant != state.firstGrantObserved) {
                    try { commit(state.copy(status = status, firstGrantObserved = firstGrant)) }
                    catch (_: RawHealthImportException) { retry = true }
                }
                continue
            }
            if (!due(state, now) || !canCall()) continue
            try {
                commit(state.copy(lastAttempt = now, firstGrantObserved = state.firstGrantObserved ?: now, notBefore = null))
                if (state.phase == "idle" || (historyGranted && !state.historyGranted)) {
                    // Reuse a still-valid prior token when extending history. Dropping it could lose
                    // deletes between the previous cursor and the new snapshot's token acquisition.
                    val token = state.changesToken ?: state.bootstrapToken ?: request { source.newChangesToken(type) }
                    if (token.isEmpty()) throw RawHealthImportException("hc_token_invalid")
                    val from = if (historyGranted) Instant.EPOCH else state.firstGrantObserved!!.minus(Duration.ofDays(30))
                    commit(state.copy(phase = "snapshot", bootstrapToken = token, changesToken = null,
                        pageToken = null, scanFrom = from, historyGranted = historyGranted,
                        historyCoverage = if (historyGranted) "history_permission_scan" else "limited_unknown_grant_start",
                        needsCatchup = true, status = "snapshot_pending"))
                }
                // At most one data page per type per turn; the durable state drives fair continuation.
                if (!canCall()) continue
                when (state.phase) {
                    "snapshot" -> {
                        val response = request { source.readPage(type, state.scanFrom!!, state.pageToken, budget.pageSize) }
                        val next = response.nextPageToken?.takeIf { it.isNotEmpty() }
                        if (next != null && next == state.pageToken) throw RawHealthImportException("hc_page_token_repeated")
                        commit(state.copy(pageToken = next, phase = if (next == null) "replay" else "snapshot",
                            needsCatchup = true, status = if (next == null) "replay_pending" else "snapshot_pending"),
                            response.records.map { RawHealthChange.Upsert(it) })
                        pages++
                    }
                    "replay", "changes" -> {
                        val token = if (state.phase == "replay") state.bootstrapToken!! else state.changesToken!!
                        val response = request { source.changes(type, token) }
                        if (response.tokenExpired) {
                            commit(state.copy(phase = "idle", bootstrapToken = null, changesToken = null,
                                pageToken = null, needsCatchup = true, deletionGap = true, status = "token_expired_rescan_pending"))
                        } else {
                            if (response.nextChangesToken.isEmpty() || (response.hasMore && response.nextChangesToken == token)) {
                                throw RawHealthImportException("hc_changes_token_invalid")
                            }
                            commit(state.copy(phase = "changes", bootstrapToken = null, changesToken = response.nextChangesToken,
                                needsCatchup = response.hasMore, status = if (response.hasMore) "changes_pending" else if (state.deletionGap) "caught_up_with_deletion_gap" else "caught_up",
                                lastSuccess = if (response.hasMore) state.lastSuccess else clock()), response.changes)
                        }
                        pages++
                    }
                    else -> throw RawHealthImportException("hc_checkpoint_phase_invalid")
                }
            } catch (_: TimeoutCancellationException) {
                retry = true
                try { commit(state.copy(status = "source_timeout", notBefore = clock().plusSeconds(30))) }
                catch (_: RawHealthImportException) { }
            } catch (error: CancellationException) { throw error }
            catch (error: Exception) {
                val status = when (error) {
                    is SecurityException -> { deniedDuringRead.add(type.name); "permission_required" }
                    is RawHealthImportException -> error.code
                    else -> "hc_source_read_failed"
                }
                if (status != "hc_budget_exhausted") {
                    retry = true
                    try { commit(state.copy(status = status, notBefore = clock().plusSeconds(30))) }
                    catch (_: RawHealthImportException) { }
                }
            }
        }
        val statuses = descriptors.map { type ->
            val state = store.checkpoint(type.name)
            RawHealthTypeStatus(type.name, type.readPermission in granted && type.name !in deniedDuringRead,
                available.getValue(type.name), backgroundGranted, historyGranted, historyStatus,
                state.historyCoverage, state.phase, state.status, state.deletionGap)
        }
        val pending = descriptors.any { type -> type.readPermission in granted && type.name !in deniedDuringRead &&
            available.getValue(type.name) && backgroundAllowed && due(store.checkpoint(type.name), clock()) }
        return RawHealthImportResult(statuses, pages, modified, pending, retry)
    }
}
