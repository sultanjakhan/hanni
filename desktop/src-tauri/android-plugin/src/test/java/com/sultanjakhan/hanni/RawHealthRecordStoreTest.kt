package com.sultanjakhan.hanni

import org.json.JSONObject
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.SQLiteMode

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@SQLiteMode(SQLiteMode.Mode.NATIVE)
class RawHealthRecordStoreTest {
    private lateinit var db: PlatformTestHealthDatabase
    private lateinit var store: RawHealthRecordStore
    private val now = RawHealthTestSupport.now
    private val type = RawHealthTestSupport.type
    @Before fun open() {
        db = PlatformTestHealthDatabase.create(null)
        RawHealthTestSupport.initialize(db)
        store = RawHealthRecordStore(db, RawHealthTestSupport.storeId)
    }
    @After fun close() { db.close() }
    private fun write(vararg changes: RawHealthChange): Int {
        val state = store.checkpoint(type)
        return store.commit(state, state.copy(changesToken = "token-${state.version}"), changes.toList(), now)
    }
    private fun value(column: String) = RawHealthTestSupport.scalar(db, "SELECT $column FROM health_records")

    @Test fun identicalReplayAdvancesOnlyCheckpointNotRecordRevisionOrRelayDirty() {
        val observation = RawHealthTestSupport.observation("one")
        assertEquals(1, write(RawHealthChange.Upsert(observation)))
        val seq = RawHealthTestSupport.scalar(db, "SELECT seq FROM cloud_relay_dirty")
        val updated = value("updated_at")
        assertEquals(0, write(RawHealthChange.Upsert(observation)))
        assertEquals("1", value("source_revision"))
        assertEquals(updated, value("updated_at"))
        assertEquals(seq, RawHealthTestSupport.scalar(db, "SELECT seq FROM cloud_relay_dirty"))
        assertEquals(2, store.checkpoint(type).version)
    }

    @Test fun correctionAndSoftDeleteRetainPayloadAndIncrementExactlyOnce() {
        write(RawHealthChange.Upsert(RawHealthTestSupport.observation("one")))
        val corrected = RawHealthTestSupport.observation("one", 456, now.plusNanos(1))
        assertEquals(1, write(RawHealthChange.Upsert(corrected)))
        assertEquals("2", value("source_revision"))
        assertEquals(RawHealthIdentity.sha256(corrected.payloadJson), value("payload_sha256"))
        assertEquals(1, write(RawHealthChange.Delete("one")))
        assertEquals("3", value("source_revision"))
        assertEquals("1", value("is_deleted"))
        assertEquals(corrected.payloadJson, value("payload_json"))
        assertEquals("getChanges", value("deletion_basis"))
        val seq = RawHealthTestSupport.scalar(db, "SELECT seq FROM cloud_relay_dirty")
        assertEquals(0, write(RawHealthChange.Delete("one")))
        assertEquals("3", value("source_revision"))
        assertEquals(seq, RawHealthTestSupport.scalar(db, "SELECT seq FROM cloud_relay_dirty"))
    }

    @Test fun unknownDeleteCreatesDeterministicAuditablePlaceholder() {
        write(RawHealthChange.Delete("unknown"))
        val payload = JSONObject(value("payload_json"))
        assertEquals(setOf("v", "record_type", "hc_record_id", "deleted"), payload.keys().asSequence().toSet())
        assertEquals("unknown", payload.getString("hc_record_id"))
        assertTrue(payload.getBoolean("deleted"))
        assertEquals(RawHealthIdentity.globalId(RawHealthTestSupport.storeId, type, "unknown"), value("id"))
        assertEquals(RawHealthIdentity.sha256(value("payload_json")), value("payload_sha256"))
        assertEquals("1", value("source_revision"))
    }

    @Test fun causalCorrectionStillAppliesAfterSourceWallClockRollsBack() {
        write(RawHealthChange.Upsert(RawHealthTestSupport.observation("one", 123, now)))
        val corrected = RawHealthTestSupport.observation("one", 456, now.minusSeconds(3600))
        assertEquals(1, write(RawHealthChange.Upsert(corrected)))
        assertEquals("2", value("source_revision"))
        assertEquals(corrected.payloadJson, value("payload_json"))
        assertEquals(RawHealthRecordStore.iso(corrected.modifiedAt), value("metadata_modified_at"))
    }

    @Test fun malformedSecondRowRollsBackFirstRowDirtyJournalAndCheckpoint() {
        val valid = RawHealthTestSupport.observation("first")
        val malformed = RawHealthObservation("wrong-id", valid.payloadJson, valid.modifiedAt, valid.startTime, valid.endTime)
        try {
            write(RawHealthChange.Upsert(valid), RawHealthChange.Upsert(malformed))
            fail("Malformed batch accepted")
        } catch (expected: RawHealthImportException) { assertEquals("hc_database_page_failed", expected.code) }
        assertEquals("0", RawHealthTestSupport.scalar(db, "SELECT COUNT(*) FROM health_records"))
        assertEquals("0", RawHealthTestSupport.scalar(db, "SELECT COUNT(*) FROM cloud_relay_dirty"))
        assertEquals(0, store.checkpoint(type).version)
    }

    @Test fun stalePageCannotWriteDataOrAcknowledgeItsToken() {
        val stale = store.checkpoint(type)
        write(RawHealthChange.Upsert(RawHealthTestSupport.observation("winner")))
        try {
            store.commit(stale, stale.copy(changesToken = "stale-token"),
                listOf(RawHealthChange.Upsert(RawHealthTestSupport.observation("loser"))), now)
            fail("Stale batch accepted")
        } catch (expected: RawHealthImportException) { assertEquals("hc_checkpoint_stale", expected.code) }
        assertEquals("1", RawHealthTestSupport.scalar(db, "SELECT COUNT(*) FROM health_records"))
        assertEquals("winner", value("hc_record_id"))
        assertEquals("token-0", store.checkpoint(type).changesToken)
    }

    @Test fun sharedSchemaRejectsPhysicalDeleteAndAllowsUpdateWhileAlreadyDirty() {
        write(RawHealthChange.Upsert(RawHealthTestSupport.observation("one")))
        write(RawHealthChange.Upsert(RawHealthTestSupport.observation("one", 234, now.plusSeconds(1))))
        assertEquals("2", value("source_revision"))
        try { db.delete("health_records", null, null); fail("Physical delete accepted") }
        catch (_: android.database.sqlite.SQLiteException) { }
        assertEquals("1", RawHealthTestSupport.scalar(db, "SELECT COUNT(*) FROM health_records"))
    }

    @Test fun identityUsesUnambiguousLengthPrefixedStrictUtf8() {
        assertEquals("297c1090f8935d536641ea0ffad3c742eafc89769d0f90dd4e34a9cdaf83bf27", RawHealthIdentity.globalId("store", "StepsRecord", "record-1"))
        assertEquals("cafcaf7f96f77f9f51ec114c5da8c0f8f977e027b7eee677e4f53c5d4c82bb99", RawHealthIdentity.globalId("s", "Type", "A\u03bb\uD83D\uDE00\n\u0000/<"))
        assertNotEquals(RawHealthIdentity.globalId("a", "bc", "d"), RawHealthIdentity.globalId("ab", "c", "d"))
        try { RawHealthIdentity.globalId("s", "t", "\uD800"); fail("Malformed Unicode accepted") }
        catch (expected: RawHealthImportException) { assertEquals("hc_invalid_text", expected.code) }
    }
}
