// HealthConnectReaders.kt — Suspend functions to read data from Health Connect
package com.sultanjakhan.hanni

import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.records.*
import androidx.health.connect.client.request.AggregateGroupByDurationRequest
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import org.json.JSONArray
import org.json.JSONObject
import java.time.Duration
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

private val dateFmt = DateTimeFormatter.ISO_LOCAL_DATE
private val timeFmt = DateTimeFormatter.ofPattern("HH:mm")

suspend fun readSleepSessions(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val response = client.readRecords(
        ReadRecordsRequest(SleepSessionRecord::class, timeRangeFilter = TimeRangeFilter.between(start, end))
    )
    val sessions = JSONArray()
    for (record in response.records) {
        val session = JSONObject()
        val zone = ZoneId.systemDefault()
        val localDate = record.startTime.atZone(zone).toLocalDate()
        session.put("date", localDate.format(dateFmt))
        session.put("start_time", record.startTime.atZone(zone).format(timeFmt))
        session.put("end_time", record.endTime.atZone(zone).format(timeFmt))
        session.put("duration_minutes", Duration.between(record.startTime, record.endTime).toMinutes())
        session.put("source", "health_connect")
        val stages = JSONArray()
        for (stage in record.stages) {
            val st = JSONObject()
            st.put("start_time", stage.startTime.atZone(zone).format(timeFmt))
            st.put("end_time", stage.endTime.atZone(zone).format(timeFmt))
            st.put("stage", stageToString(stage.stage))
            stages.put(st)
        }
        session.put("stages", stages)
        sessions.put(session)
    }
    return sessions
}

suspend fun readDailySteps(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val response = client.aggregateGroupByDuration(
        AggregateGroupByDurationRequest(
            metrics = setOf(StepsRecord.COUNT_TOTAL),
            timeRangeFilter = TimeRangeFilter.between(start, end),
            timeRangeSlicer = Duration.ofDays(1)
        )
    )
    val result = JSONArray()
    for (bucket in response) {
        val obj = JSONObject()
        val date = bucket.startTime.atZone(ZoneId.systemDefault()).toLocalDate()
        obj.put("date", date.format(dateFmt))
        obj.put("steps", bucket.result[StepsRecord.COUNT_TOTAL] ?: 0L)
        result.put(obj)
    }
    return result
}

suspend fun readHeartRateSamples(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val response = client.readRecords(
        ReadRecordsRequest(HeartRateRecord::class, timeRangeFilter = TimeRangeFilter.between(start, end))
    )
    val result = JSONArray()
    val zone = ZoneId.systemDefault()
    for (record in response.records) {
        for (sample in record.samples) {
            val obj = JSONObject()
            val zdt = sample.time.atZone(zone)
            obj.put("date", zdt.toLocalDate().format(dateFmt))
            obj.put("time", zdt.format(timeFmt))
            obj.put("bpm", sample.beatsPerMinute)
            result.put(obj)
        }
    }
    return result
}

suspend fun readExerciseSessions(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val response = client.readRecords(
        ReadRecordsRequest(ExerciseSessionRecord::class, timeRangeFilter = TimeRangeFilter.between(start, end))
    )
    val result = JSONArray()
    val zone = ZoneId.systemDefault()
    for (record in response.records) {
        val obj = JSONObject()
        val zdt = record.startTime.atZone(zone)
        obj.put("date", zdt.toLocalDate().format(dateFmt))
        obj.put("start_time", zdt.format(timeFmt))
        obj.put("end_time", record.endTime.atZone(zone).format(timeFmt))
        obj.put("duration_minutes", Duration.between(record.startTime, record.endTime).toMinutes())
        obj.put("type", exerciseTypeToString(record.exerciseType))
        obj.put("title", record.title ?: "")
        result.put(obj)
    }
    return result
}

fun stageToString(stage: Int): String = when (stage) {
    SleepSessionRecord.STAGE_TYPE_AWAKE -> "awake"
    SleepSessionRecord.STAGE_TYPE_LIGHT -> "light"
    SleepSessionRecord.STAGE_TYPE_DEEP -> "deep"
    SleepSessionRecord.STAGE_TYPE_REM -> "rem"
    SleepSessionRecord.STAGE_TYPE_SLEEPING -> "sleeping"
    SleepSessionRecord.STAGE_TYPE_OUT_OF_BED -> "out_of_bed"
    SleepSessionRecord.STAGE_TYPE_AWAKE_IN_BED -> "awake"
    else -> "unknown"
}

fun exerciseTypeToString(exerciseType: Int): String = when (exerciseType) {
    ExerciseSessionRecord.EXERCISE_TYPE_RUNNING -> "running"
    ExerciseSessionRecord.EXERCISE_TYPE_WALKING -> "walking"
    ExerciseSessionRecord.EXERCISE_TYPE_SWIMMING_POOL -> "swimming"
    ExerciseSessionRecord.EXERCISE_TYPE_BIKING -> "cycling"
    ExerciseSessionRecord.EXERCISE_TYPE_YOGA -> "yoga"
    ExerciseSessionRecord.EXERCISE_TYPE_STRENGTH_TRAINING -> "strength"
    ExerciseSessionRecord.EXERCISE_TYPE_HIKING -> "hiking"
    else -> "other"
}
