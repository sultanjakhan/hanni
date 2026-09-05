package com.sultanjakhan.hanni

import android.database.sqlite.SQLiteDatabase
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.SQLiteMode
import org.robolectric.shadows.ShadowLog
import java.time.Instant

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@SQLiteMode(SQLiteMode.Mode.NATIVE)
class RawHealthDiagnosticsTest {
    private val steps = RawHealthRecordCodec.descriptors.single { it.name == "StepsRecord" }
    @Before fun clearLogs() { ShadowLog.clear() }
    private fun messages() = ShadowLog.getLogsForTag("HanniWorkerDiag").map {
        assertNull(it.throwable)
        it.msg
    }

    @Test fun unknownCodeAndPhaseCannotLeakAndThrowableIsNeverFormatted() {
        recordRawHealthFailure(steps, "synthetic-private-phase", RawHealthFailureStage.ENCODE,
            RawHealthImportException("synthetic-private-code"))
        val error = object : IllegalStateException("synthetic-private-message") {
            override val message: String get() = throw AssertionError("message was accessed")
            override fun toString(): String = throw AssertionError("Throwable was formatted")
        }
        try {
            observeRawHealthFailure(steps, "snapshot", RawHealthFailureStage.READ) { throw error }
            fail("expected original exception")
        } catch (caught: IllegalStateException) { assertSame(error, caught) }
        assertEquals(listOf(
            "hc_page_failure type=StepsRecord phase=unknown stage=ENCODE code=hc_source_read_failed category=import hc_error=0",
            "hc_page_failure type=StepsRecord phase=snapshot stage=READ code=hc_source_read_failed category=state hc_error=0"
        ), messages())
    }

    @Test fun unregisteredDescriptorIsSuppressedAndCancellationIsUnchanged() {
        recordRawHealthFailure(steps.copy(name = "synthetic-private-type"), "snapshot",
            RawHealthFailureStage.READ, IllegalArgumentException("synthetic-private-token"))
        val error = CancellationException("synthetic-private-cancellation")
        try {
            observeRawHealthFailure(steps, "snapshot", RawHealthFailureStage.READ) { throw error }
            fail("expected original cancellation")
        } catch (caught: CancellationException) { assertSame(error, caught) }
        assertTrue(messages().isEmpty())
    }

    @Test fun permissionFailureRetainsItsClassAndUsesOnlyFixedMetadata() {
        val error = SecurityException("synthetic-private-record")
        try {
            observeRawHealthFailure(steps, "snapshot", RawHealthFailureStage.READ) { throw error }
            fail("expected original security exception")
        } catch (caught: SecurityException) { assertSame(error, caught) }
        assertEquals(listOf("hc_page_failure type=StepsRecord phase=snapshot stage=READ code=permission_required category=security hc_error=0"), messages())
    }

    private fun source(page: () -> RawHealthPage) = object : RawHealthSource {
        override suspend fun grantedPermissions() = setOf(steps.readPermission)
        override fun featureAvailable(feature: Int) = true
        override suspend fun newChangesToken(type: RawHealthRecordCodec.Descriptor) = "synthetic-private-bootstrap"
        override suspend fun readPage(type: RawHealthRecordCodec.Descriptor, from: Instant, token: String?, pageSize: Int) = page()
        override suspend fun changes(type: RawHealthRecordCodec.Descriptor, token: String) = RawHealthChangesPage(emptyList(), "unused", false, false)
    }

    @Test fun failedPageCommitReportsCommitWithoutAdvancingCheckpointOrWritingRows() = runBlocking {
        SQLiteDatabase.create(null).use { db ->
            RawHealthTestSupport.initialize(db)
            db.execSQL("CREATE TRIGGER synthetic_failure BEFORE INSERT ON health_records BEGIN SELECT RAISE(ABORT,'synthetic-private-value'); END")
            val store = RawHealthRecordStore(db, RawHealthTestSupport.storeId)
            val engine = RawHealthImporter(store, source { RawHealthPage(listOf(RawHealthTestSupport.observation("synthetic-private-id")), "synthetic-private-next") },
                listOf(steps), { RawHealthTestSupport.now }, { 0L })
            val result = engine.runOnce(false)
            val checkpoint = store.checkpoint(steps.name)
            assertTrue(result.retryNeeded)
            assertEquals("snapshot", checkpoint.phase)
            assertNull(checkpoint.pageToken)
            assertEquals("hc_database_page_failed", checkpoint.status)
            assertEquals(RawHealthTestSupport.now.plusSeconds(30), checkpoint.notBefore)
            assertEquals("0", RawHealthTestSupport.scalar(db, "SELECT COUNT(*) FROM health_records"))
            assertEquals(listOf("hc_page_failure type=StepsRecord phase=snapshot stage=COMMIT code=hc_database_page_failed category=import hc_error=0"), messages())
        }
    }

    @Test fun repeatedPageTokenReportsCursorAndPreservesExistingRetryState() = runBlocking {
        SQLiteDatabase.create(null).use { db ->
            RawHealthTestSupport.initialize(db)
            val store = RawHealthRecordStore(db, RawHealthTestSupport.storeId)
            val initial = store.checkpoint(steps.name)
            store.commit(initial, initial.copy(phase = "snapshot", bootstrapToken = "synthetic-private-bootstrap",
                pageToken = "synthetic-private-page", scanFrom = Instant.EPOCH), observedAt = RawHealthTestSupport.now)
            val engine = RawHealthImporter(store, source { RawHealthPage(emptyList(), "synthetic-private-page") },
                listOf(steps), { RawHealthTestSupport.now }, { 0L })
            val result = engine.runOnce(false)
            assertTrue(result.retryNeeded)
            val checkpoint = store.checkpoint(steps.name)
            assertEquals("synthetic-private-page", checkpoint.pageToken)
            assertEquals("snapshot", checkpoint.phase)
            assertEquals("hc_page_token_repeated", checkpoint.status)
            assertEquals(RawHealthTestSupport.now.plusSeconds(30), checkpoint.notBefore)
            assertEquals(listOf("hc_page_failure type=StepsRecord phase=snapshot stage=CURSOR code=hc_page_token_repeated category=import hc_error=0"), messages())
        }
    }
}
