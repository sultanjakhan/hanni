package com.sultanjakhan.hanni

import androidx.health.connect.client.permission.HealthPermission
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.SQLiteMode
import java.io.File
import java.time.Instant

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@SQLiteMode(SQLiteMode.Mode.NATIVE)
class RawHealthImporterTest {
    private lateinit var db: PlatformTestHealthDatabase
    private lateinit var store: RawHealthRecordStore
    private var backingFile: File? = null
    private val now = RawHealthTestSupport.now
    private val steps = RawHealthRecordCodec.descriptors.single { it.name == "StepsRecord" }
    private val sleep = RawHealthRecordCodec.descriptors.single { it.name == "SleepSessionRecord" }
    @Before fun open() {
        db = PlatformTestHealthDatabase.create(null); RawHealthTestSupport.initialize(db)
        store = RawHealthRecordStore(db, RawHealthTestSupport.storeId)
    }
    @After fun close() { db.close(); backingFile?.let { PlatformTestHealthDatabase.deleteDatabase(it) } }
    private class Source : RawHealthSource {
        val permissions = mutableSetOf<String>()
        val calls = mutableListOf<String>()
        var page: (RawHealthRecordCodec.Descriptor, Instant, String?) -> RawHealthPage = { _, _, _ -> RawHealthPage(emptyList(), "") }
        var delta: (String) -> RawHealthChangesPage = { RawHealthChangesPage(emptyList(), "head", false, false) }
        override suspend fun grantedPermissions() = permissions.toSet()
        override fun featureAvailable(feature: Int) = true
        override suspend fun newChangesToken(type: RawHealthRecordCodec.Descriptor): String {
            calls.add("token:${type.name}"); return "reserved"
        }
        override suspend fun readPage(type: RawHealthRecordCodec.Descriptor, from: Instant, token: String?, pageSize: Int): RawHealthPage {
            calls.add("page:${type.name}:${token ?: "first"}"); return page(type, from, token)
        }
        override suspend fun changes(type: RawHealthRecordCodec.Descriptor, token: String): RawHealthChangesPage {
            calls.add("changes:${type.name}:$token"); return delta(token)
        }
    }
    private fun engine(source: Source, types: List<RawHealthRecordCodec.Descriptor> = listOf(steps)) =
        RawHealthImporter(store, source, types, { now }, { 0L })
    private fun count() = RawHealthTestSupport.scalar(db, "SELECT COUNT(*) FROM health_records").toInt()

    @Test fun reservesTokenBeforeSnapshotAndResumesPagesAfterDatabaseReopen() = runBlocking {
        db.close()
        val file = File.createTempFile("raw-hc-importer-", ".db")
        backingFile = file
        db = PlatformTestHealthDatabase.openOrCreateDatabase(file, null)
        RawHealthTestSupport.initialize(db)
        store = RawHealthRecordStore(db, RawHealthTestSupport.storeId)
        val source = Source().apply {
            permissions.add(steps.readPermission)
            page = { _, _, token -> if (token == null) RawHealthPage(listOf(RawHealthTestSupport.observation("one")), "second")
                else RawHealthPage(listOf(RawHealthTestSupport.observation("two")), "") }
            delta = { RawHealthChangesPage(listOf(RawHealthChange.Delete("one"),
                RawHealthChange.Upsert(RawHealthTestSupport.observation("race"))), "after-race", false, false) }
        }
        assertTrue(engine(source).runOnce(false).morePending)
        assertEquals(listOf("token:StepsRecord", "page:StepsRecord:first"), source.calls)
        assertEquals("second", store.checkpoint(steps.name).pageToken)
        db.close()
        db = PlatformTestHealthDatabase.openDatabase(file.absolutePath, null, PlatformTestHealthDatabase.OPEN_READWRITE)
        store = RawHealthRecordStore(db, RawHealthTestSupport.storeId)
        assertTrue(engine(source).runOnce(false).morePending)
        assertEquals("replay", store.checkpoint(steps.name).phase)
        assertFalse(engine(source).runOnce(false).morePending)
        assertEquals("after-race", store.checkpoint(steps.name).changesToken)
        assertEquals(3, count())
        assertEquals("1", RawHealthTestSupport.scalar(db, "SELECT is_deleted FROM health_records WHERE hc_record_id='one'"))
        assertEquals(1, source.calls.count { it.startsWith("token:") })
    }

    @Test fun expiredTokenRescansWithoutInferringDeletionAndKeepsGapVisible() = runBlocking {
        val initial = store.checkpoint(steps.name)
        store.commit(initial, initial.copy(phase = "changes", changesToken = "expired", needsCatchup = true),
            listOf(RawHealthChange.Upsert(RawHealthTestSupport.observation("old"))), now)
        val source = Source().apply { permissions.add(steps.readPermission)
            delta = { if (it == "expired") RawHealthChangesPage(emptyList(), "", false, true)
                else RawHealthChangesPage(emptyList(), "head", false, false) }
        }
        engine(source).runOnce(false)
        assertEquals("idle", store.checkpoint(steps.name).phase)
        assertTrue(store.checkpoint(steps.name).deletionGap)
        engine(source).runOnce(false); engine(source).runOnce(false)
        assertEquals("caught_up_with_deletion_gap", store.checkpoint(steps.name).status)
        assertEquals("0", RawHealthTestSupport.scalar(db, "SELECT is_deleted FROM health_records"))
        assertEquals(1, count())
    }

    @Test fun onePermissionRaceDoesNotDiscardAnotherTypesPage() = runBlocking {
        val source = Source().apply {
            permissions.addAll(listOf(steps.readPermission, sleep.readPermission))
            page = { type, _, _ -> if (type == steps) throw SecurityException("synthetic") else RawHealthPage(emptyList(), null) }
        }
        val result = engine(source, listOf(steps, sleep)).runOnce(false)
        assertEquals(1, result.pages)
        assertFalse(result.types.single { it.type == steps.name }.granted)
        assertEquals("replay", store.checkpoint(sleep.name).phase)
        assertEquals("snapshot", store.checkpoint(steps.name).phase)
    }

    @Test fun historyExpansionReusesPriorCursorSoInterveningDeletesAreNotLost() = runBlocking {
        val initial = store.checkpoint(steps.name)
        store.commit(initial, initial.copy(phase = "changes", changesToken = "continuous", needsCatchup = false,
            firstGrantObserved = now, lastSuccess = now, historyGranted = false),
            listOf(RawHealthChange.Upsert(RawHealthTestSupport.observation("deleted-during-upgrade"))), now)
        val source = Source().apply {
            permissions.addAll(listOf(steps.readPermission, HealthPermission.PERMISSION_READ_HEALTH_DATA_HISTORY))
            page = { _, from, _ -> assertEquals(Instant.EPOCH, from); RawHealthPage(emptyList(), null) }
            delta = { token -> assertEquals("continuous", token)
                RawHealthChangesPage(listOf(RawHealthChange.Delete("deleted-during-upgrade")), "head", false, false) }
        }
        engine(source).runOnce(false)
        assertEquals("continuous", store.checkpoint(steps.name).bootstrapToken)
        engine(source).runOnce(false)
        assertEquals(0, source.calls.count { it.startsWith("token:") })
        assertEquals("1", RawHealthTestSupport.scalar(db, "SELECT is_deleted FROM health_records"))
    }

    @Test fun budgetsRotateTypesAndContinuationStopsAfterAllHeadsAreReached() = runBlocking {
        val source = Source().apply { permissions.addAll(listOf(steps.readPermission, sleep.readPermission)) }
        val small = RawHealthImporter.Budget(maxCalls = 2, maxPages = 1)
        assertTrue(engine(source, listOf(steps, sleep)).runOnce(false, small).morePending)
        assertTrue(engine(source, listOf(steps, sleep)).runOnce(false, small).morePending)
        assertEquals(2, source.calls.count { it.startsWith("token:") })
        val bigger = RawHealthImporter.Budget(maxCalls = 2, maxPages = 2)
        assertFalse(engine(source, listOf(steps, sleep)).runOnce(false, bigger).morePending)
        val calls = source.calls.size
        assertFalse(engine(source, listOf(steps, sleep)).runOnce(false, bigger).morePending)
        assertEquals(calls, source.calls.size)
    }

    @Test fun missingBackgroundPermissionLeavesTokensAndRowsUntouched() = runBlocking {
        val source = Source().apply { permissions.add(steps.readPermission) }
        val result = engine(source).runOnce(true)
        assertEquals(0, count())
        assertEquals(0, source.calls.size)
        assertEquals("background_permission_required", result.types.single().status)
        assertFalse(result.morePending)
    }

    @Test fun actualRawReadRequestStartsNewestAndDisablesProviderDeduplication() {
        val request = rawHealthReadRequest(steps, Instant.EPOCH, "next-page", 100)
        assertFalse(request.ascendingOrder)
        assertEquals(Instant.EPOCH, request.timeRangeFilter.startTime)
        assertEquals("next-page", request.pageToken)
        assertEquals(100, request.pageSize)
        assertEquals(steps.recordClass, request.recordType)
        assertTrue(request.dataOriginFilter.isEmpty())
        // Test-only reflection reads the SDK's internal strategy without making the runtime
        // importer depend on the experimental API or inaccessible Companion constants.
        assertEquals(0, request.javaClass.getMethod("getDeduplicateStrategy").invoke(request))
    }

    @Test fun newestFirstPlannedReadKeepsFutureRecordsInItsUnboundedUpperRange() {
        val planned = RawHealthRecordCodec.descriptors.single { it.name == "PlannedExerciseSessionRecord" }
        val request = rawHealthReadRequest(planned, now.minusSeconds(30L * 86400), null, 100)
        assertFalse(request.ascendingOrder)
        assertEquals(planned.recordClass, request.recordType)
        assertEquals(now.minusSeconds(30L * 86400), request.timeRangeFilter.startTime)
        assertNull(request.timeRangeFilter.endTime)
        assertNull(request.timeRangeFilter.localStartTime)
        assertNull(request.timeRangeFilter.localEndTime)
        assertNull(request.pageToken)
        assertTrue(request.dataOriginFilter.isEmpty())
    }
}
