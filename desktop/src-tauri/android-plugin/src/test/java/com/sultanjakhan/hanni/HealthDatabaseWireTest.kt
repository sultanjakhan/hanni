package com.sultanjakhan.hanni

import android.database.Cursor
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class HealthDatabaseWireTest {
    @Test fun matrixCursorPreservesSqliteTypesAndEveryIntegerBit() {
        val input = listOf(null, Long.MIN_VALUE, Long.MAX_VALUE, 9_007_199_254_740_993L, -0.125,
            "Aλ😀\n\u0000", byteArrayOf(0, -1, 1, -128))
        val reply = JSONObject().put("columns", JSONArray(input.indices.map { "c$it" }))
            .put("rows", JSONArray().put(JSONArray().apply { input.forEach { put(HealthDatabaseWire.encode(it)) } }))
        HealthDatabaseWire.cursor(reply).use { cursor ->
            assertTrue(cursor.moveToFirst())
            assertTrue(cursor.isNull(0))
            assertEquals(Long.MIN_VALUE, cursor.getLong(1))
            assertEquals(Long.MAX_VALUE, cursor.getLong(2))
            assertEquals(9_007_199_254_740_993L, cursor.getLong(3))
            assertEquals(-0.125, cursor.getDouble(4), 0.0)
            assertEquals(input[5], cursor.getString(5))
            assertArrayEquals(input[6] as ByteArray, cursor.getBlob(6))
            assertEquals(Cursor.FIELD_TYPE_INTEGER, cursor.getType(3))
            assertEquals(Cursor.FIELD_TYPE_FLOAT, cursor.getType(4))
            assertEquals(Cursor.FIELD_TYPE_BLOB, cursor.getType(6))
        }
    }
    @Test fun unsupportedBindingsFailWithoutStringifyingTheirContent() {
        val hostile = object { override fun toString(): String = error("SYNTHETIC_CANARY_DO_NOT_FORMAT") }
        for (value in listOf(hostile, Double.NaN, Double.POSITIVE_INFINITY, "\uD800", "\uDC00")) {
            try { HealthDatabaseWire.encode(value); fail("Unsupported value accepted") }
            catch (error: IllegalStateException) { assertFalse(error.message.orEmpty().contains("CANARY")) }
            catch (error: IllegalArgumentException) { assertFalse(error.message.orEmpty().contains("CANARY")) }
        }
    }
    @Test fun malformedCursorDoesNotCoerceUnknownTypesOrTruncateIntegers() {
        for (cell in listOf(JSONObject("{\"t\":\"i\",\"v\":\"9223372036854775808\"}"),
            JSONObject("{\"t\":\"i\",\"v\":1}"), JSONObject("{\"t\":\"x\",\"v\":\"text\"}"),
            JSONObject("{\"t\":\"n\",\"v\":\"text\"}"))) {
            try { HealthDatabaseWire.decode(cell); fail("Malformed value accepted") } catch (_: Exception) { }
        }
        val mismatch = JSONObject().put("columns", JSONArray().put("a")).put("rows", JSONArray().put(JSONArray()))
        try { HealthDatabaseWire.cursor(mismatch); fail("Mismatched columns accepted") } catch (_: Exception) { }
    }
}
