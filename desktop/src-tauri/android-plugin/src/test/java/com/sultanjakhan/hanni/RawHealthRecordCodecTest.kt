@file:OptIn(androidx.health.connect.client.feature.ExperimentalMindfulnessSessionApi::class)

package com.sultanjakhan.hanni

import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.*
import androidx.health.connect.client.records.metadata.Metadata
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.lang.reflect.Modifier
import java.time.Duration
import java.time.Instant
import java.time.ZoneOffset

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class RawHealthRecordCodecTest {
    @Test fun all41SdkTypesAndPermissionsMatchTheRegistry() {
        val records = RawHealthRecordFixtures.all()
        val descriptors = RawHealthRecordCodec.descriptors
        assertEquals(41, records.size)
        assertEquals(41, descriptors.size)
        assertEquals(41, descriptors.map { it.name }.toSet().size)
        assertEquals(records.map { it::class }.toSet(), descriptors.map { it.recordClass }.toSet())
        for (descriptor in descriptors) {
            assertEquals(HealthPermission.getReadPermission(descriptor.recordClass), descriptor.readPermission)
        }
        assertEquals(setOf("MindfulnessSessionRecord", "PlannedExerciseSessionRecord", "SkinTemperatureRecord"),
            descriptors.filter { it.requiredFeature != null }.map { it.name }.toSet())
    }

    @Test fun actualSdkPublicValuesSurviveJsonSerializationForEveryRecord() {
        for (record in RawHealthRecordFixtures.all()) {
            // JSON string parse exercises escaping and transport serialization, not only in-memory objects.
            val payload = JSONObject(RawHealthRecordCodec.encode(record).toString())
            assertEquals(1, payload.getInt("v"))
            assertEquals("androidx.health.connect:connect-client:1.1.0", payload.getString("sdk"))
            assertEquals(record.javaClass.simpleName, payload.getString("record_type"))
            assertPublicValues(record, payload.getJSONObject("record"))
        }
    }

    @Test fun floatingPointEncodingKeepsAllBitsIncludingSignedZeroAndNanPayload() {
        val values = listOf(0L, Long.MIN_VALUE, 1L, 0x0010000000000000L,
            0x3ff0000000000001L, 0x7fefffffffffffffL, 0x7ff0000000000000L,
            -0x0010000000000000L, 0x7ff8000000001234L)
        for (bits in values) {
            val encoded = JSONObject(RawHealthRecordCodec.f64(java.lang.Double.longBitsToDouble(bits)).toString())
            assertEquals(bits, java.lang.Double.doubleToRawLongBits(RawHealthRecordCodec.readF64(encoded)))
        }
    }

    @Test fun malformedFloatingPointTagsFailWithoutCoercion() {
        for (value in listOf(JSONObject("{}"), JSONObject("{\"f64\":0}"),
            JSONObject("{\"f64\":\"000000000000000G\"}"), JSONObject("{\"f64\":\"0\"}"),
            JSONObject("{\"f64\":\"0000000000000000\",\"extra\":1}"))) {
            try {
                RawHealthRecordCodec.readF64(value)
                fail("Malformed floating value accepted")
            } catch (expected: IllegalArgumentException) {
                assertEquals("hc_f64_invalid", expected.message)
            }
        }
    }

    @Test fun nullableNutritionIsDistinctFromZeroAndKeepsEveryField() {
        val record = NutritionRecord(startTime = RawHealthRecordFixtures.start, startZoneOffset = null,
            endTime = RawHealthRecordFixtures.end, endZoneOffset = null, metadata = Metadata.manualEntry())
        val encoded = RawHealthRecordCodec.encode(record).getJSONObject("record")
        assertPublicValues(record, encoded)
        for (field in listOf("energy", "energyFromFat", "protein", "vitaminK", "name", "startZoneOffset")) {
            assertTrue(encoded.has(field))
            assertTrue(encoded.isNull(field))
        }
    }

    @Test fun largeSeriesKeepsEverySampleWithoutTruncation() {
        val start = RawHealthRecordFixtures.start
        val samples = (0 until 4_000).map { HeartRateRecord.Sample(start.plusNanos(it.toLong()), 63L + (it % 2)) }
        val record = HeartRateRecord(startTime = start, startZoneOffset = null,
            endTime = start.plusSeconds(1), endZoneOffset = null, metadata = Metadata.manualEntry(), samples = samples)
        val serialized = RawHealthRecordCodec.encode(record).toString()
        assertTrue(serialized.toByteArray(Charsets.UTF_8).size > 60_000)
        val restored = JSONObject(serialized).getJSONObject("record").getJSONArray("samples")
        assertEquals(samples.size, restored.length())
        assertPublicValues(samples, restored)
    }

    @Test fun unsupportedRecordFailsWithSafeError() {
        val record = object : Record { override val metadata: Metadata = Metadata.manualEntry() }
        try {
            RawHealthRecordCodec.encode(record)
            fail("Unknown record accepted")
        } catch (expected: IllegalArgumentException) {
            assertEquals("hc_record_type_unsupported", expected.message)
        }
    }

    // Reflection is confined to synthetic tests. The production codec uses explicit typed adapters.
    // This independently enumerates the compiled SDK getters, detecting fields accidentally omitted
    // by a hand-written adapter or by the development generator's getter inventory.
    private fun assertPublicValues(original: Any?, encoded: Any?) {
        when (original) {
            null -> assertEquals(JSONObject.NULL, encoded)
            is Double -> assertEquals(java.lang.Double.doubleToRawLongBits(original),
                java.lang.Double.doubleToRawLongBits(RawHealthRecordCodec.readF64(encoded as JSONObject)))
            is Long -> assertEquals(java.lang.Long.toString(original), encoded)
            is String, is Int, is Boolean -> assertEquals(original, encoded)
            is Instant -> {
                encoded as JSONObject
                assertEquals(java.lang.Long.toString(original.epochSecond), encoded.getString("seconds"))
                assertEquals(original.nano, encoded.getInt("nanos"))
                assertEquals(setOf("seconds", "nanos"), encoded.keys().asSequence().toSet())
            }
            is Duration -> {
                encoded as JSONObject
                assertEquals(java.lang.Long.toString(original.seconds), encoded.getString("seconds"))
                assertEquals(original.nano, encoded.getInt("nanos"))
            }
            is ZoneOffset -> assertEquals(original.totalSeconds, encoded)
            is List<*> -> {
                encoded as JSONArray
                assertEquals(original.size, encoded.length())
                original.forEachIndexed { index, value -> assertPublicValues(value, encoded.get(index)) }
            }
            else -> {
                val clazz = original.javaClass
                assertTrue(clazz.name.startsWith("androidx.health.connect.client."))
                encoded as JSONObject
                val getters = clazz.methods.filter { method ->
                    Modifier.isPublic(method.modifiers) && !Modifier.isStatic(method.modifiers) &&
                        method.parameterCount == 0 && !method.name.contains('$') && method.name != "getClass" && method.name != "hashCode" &&
                        (method.name.startsWith("get") || method.name.startsWith("is") || method.name.startsWith("has"))
                }
                val names = getters.map { method ->
                    if (method.name.startsWith("get")) method.name.substring(3).replaceFirstChar { it.lowercase() }
                    else method.name
                }.toSet()
                val hasType = clazz.name.contains(".units.") ||
                    clazz.name.contains(".ExerciseCompletionGoal$") ||
                    clazz.name.contains(".ExercisePerformanceTarget$") || clazz.name.contains(".ExerciseRouteResult$")
                val unit = clazz.name.contains(".units.")
                val tags = (if (hasType) setOf("type") else emptySet()) + (if (unit) setOf("primary_unit") else emptySet())
                assertEquals(names + tags, encoded.keys().asSequence().toSet())
                if (hasType) assertEquals(clazz.name.substringAfterLast('.').replace('$', '.'), encoded.getString("type"))
                if (unit) assertTrue(names.contains(encoded.getString("primary_unit")))
                for (method in getters) {
                    val field = if (method.name.startsWith("get")) method.name.substring(3).replaceFirstChar { it.lowercase() } else method.name
                    assertPublicValues(method.invoke(original), encoded.get(field))
                }
            }
        }
    }
}
