// HealthConnectReaders.kt — Suspend functions to read data from Health Connect
package com.sultanjakhan.hanni

import android.util.Log
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.records.*
import androidx.health.connect.client.request.AggregateGroupByPeriodRequest
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import org.json.JSONArray
import org.json.JSONObject
import java.time.Duration
import java.time.Instant
import java.time.Period
import java.time.ZoneId
import java.time.format.DateTimeFormatter

private val dateFmt = DateTimeFormatter.ISO_LOCAL_DATE
private val timeFmt = DateTimeFormatter.ofPattern("HH:mm")
private const val SLEEP_MERGE_GAP_MINUTES = 180L
private const val WALK_MERGE_GAP_MINUTES = 3L
private const val WALK_MIN_MINUTES = 8L
private const val WALK_MIN_STEPS = 500L

private data class SleepGroup(
    val start: Instant,
    var end: Instant,
    val stages: MutableList<JSONObject>,
    val sourcePackages: MutableSet<String>,
)

private data class StepWalk(
    val start: Instant,
    var end: Instant,
    var steps: Long,
    val sourcePackage: String,
)

internal suspend fun <T> collectHealthPages(
    readPage: suspend (String?) -> Pair<List<T>, String?>,
): List<T> {
    val records = mutableListOf<T>()
    var pageToken: String? = null
    do {
        val (page, nextToken) = readPage(pageToken)
        records.addAll(page)
        pageToken = nextToken
        // Some Android 12/13 providers return an empty terminal token.
    } while (!pageToken.isNullOrEmpty())
    return records
}

private suspend fun <T : Record> readAllRecords(
    client: HealthConnectClient,
    recordType: kotlin.reflect.KClass<T>,
    start: Instant,
    end: Instant,
): List<T> = collectHealthPages { pageToken ->
    val response = client.readRecords(
        ReadRecordsRequest(
            recordType = recordType,
            timeRangeFilter = TimeRangeFilter.between(start, end),
            pageSize = 1000,
            pageToken = pageToken,
        )
    )
    Pair(response.records, response.pageToken)
}

private suspend fun readStepRecordsByDay(
    client: HealthConnectClient,
    start: Instant,
    end: Instant,
): List<StepsRecord> {
    val zone = ZoneId.systemDefault()
    val records = mutableListOf<StepsRecord>()
    var failedDays = 0
    var successfulDays = 0
    var day = start.atZone(zone).toLocalDate()
    val lastDay = end.atZone(zone).toLocalDate()
    while (!day.isAfter(lastDay)) {
        val dayStart = day.atStartOfDay(zone).toInstant()
        val nextDay = day.plusDays(1).atStartOfDay(zone).toInstant()
        val rangeStart = if (dayStart > start) dayStart else start
        val rangeEnd = if (nextDay < end) nextDay else end
        if (rangeStart < rangeEnd) {
            try {
                records.addAll(readAllRecords(client, StepsRecord::class, rangeStart, rangeEnd))
                successfulDays += 1
            } catch (_: IllegalArgumentException) {
                failedDays += 1
                // A malformed vendor row (startTime >= endTime) must not make
                // every other day's walk inference fail. Daily HC aggregation
                // still supplies the canonical step total for this date.
            }
        }
        day = day.plusDays(1)
    }
    Log.i("HanniHealthReader", "rawSteps=${records.size} successfulDays=$successfulDays failedDays=$failedDays")
    return records
}

suspend fun readSleepSessions(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val zone = ZoneId.systemDefault()
    val groups = mutableListOf<SleepGroup>()
    val records = readAllRecords(client, SleepSessionRecord::class, start, end)
        .sortedBy { it.startTime }
    for (record in records) {
        val stageRows = record.stages.map { stage ->
            JSONObject().apply {
                put("start_time", stage.startTime.atZone(zone).format(timeFmt))
                put("end_time", stage.endTime.atZone(zone).format(timeFmt))
                put("stage", stageToString(stage.stage))
            }
        }
        val previous = groups.lastOrNull()
        val gap = previous?.let { Duration.between(it.end, record.startTime).toMinutes() }
        if (previous != null && gap != null && gap < SLEEP_MERGE_GAP_MINUTES) {
            if (record.endTime > previous.end) previous.end = record.endTime
            previous.stages.addAll(stageRows)
            previous.sourcePackages.add(record.metadata.dataOrigin.packageName)
        } else {
            groups.add(SleepGroup(
                start = record.startTime,
                end = record.endTime,
                stages = stageRows.toMutableList(),
                sourcePackages = mutableSetOf(record.metadata.dataOrigin.packageName),
            ))
        }
    }

    val sessions = JSONArray()
    for (group in groups) {
        val session = JSONObject()
        val localDate = group.start.atZone(zone).toLocalDate()
        val startTime = group.start.atZone(zone).format(timeFmt)
        session.put("date", localDate.format(dateFmt))
        session.put("start_time", startTime)
        session.put("end_time", group.end.atZone(zone).format(timeFmt))
        session.put("duration_minutes", Duration.between(group.start, group.end).toMinutes())
        // Full instants — the Rust importer needs these to merge segments of a
        // night that Samsung Health writes as separate SleepSessionRecords.
        session.put("start_iso", group.start.toString())
        session.put("end_iso", group.end.toString())
        session.put("source", "health_connect")
        session.put("record_id", "sleep:${localDate.format(dateFmt)}:$startTime")
        session.put("source_package", group.sourcePackages.sorted().joinToString(","))
        val stages = JSONArray()
        group.stages.forEach { stages.put(it) }
        session.put("stages", stages)
        sessions.put(session)
    }
    return sessions
}

suspend fun readDailySteps(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val zone = ZoneId.systemDefault()
    val startLocal = start.atZone(zone).toLocalDate().atStartOfDay()
    val endLocal = end.atZone(zone).toLocalDate().plusDays(1).atStartOfDay()
    val response = client.aggregateGroupByPeriod(
        AggregateGroupByPeriodRequest(
            metrics = setOf(StepsRecord.COUNT_TOTAL),
            timeRangeFilter = TimeRangeFilter.between(startLocal, endLocal),
            timeRangeSlicer = Period.ofDays(1)
        )
    )
    val result = JSONArray()
    for (bucket in response) {
        val obj = JSONObject()
        val date = bucket.startTime.toLocalDate()
        obj.put("date", date.format(dateFmt))
        obj.put("steps", bucket.result[StepsRecord.COUNT_TOTAL] ?: 0L)
        result.put(obj)
    }
    return result
}

suspend fun readHeartRateSamples(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val result = JSONArray()
    val zone = ZoneId.systemDefault()
    for (record in readAllRecords(client, HeartRateRecord::class, start, end)) {
        for ((sampleIndex, sample) in record.samples.withIndex()) {
            val obj = JSONObject()
            val zdt = sample.time.atZone(zone)
            obj.put("date", zdt.toLocalDate().format(dateFmt))
            obj.put("time", zdt.format(timeFmt))
            obj.put("bpm", sample.beatsPerMinute)
            obj.put("record_id", record.metadata.id)
            obj.put("sample_index", sampleIndex)
            obj.put("source_package", record.metadata.dataOrigin.packageName)
            result.put(obj)
        }
    }
    return result
}

suspend fun readExerciseSessions(client: HealthConnectClient, start: Instant, end: Instant): JSONArray {
    val result = JSONArray()
    val zone = ZoneId.systemDefault()
    val exerciseRecords = readAllRecords(client, ExerciseSessionRecord::class, start, end)
        .sortedBy { it.startTime }
    for (record in exerciseRecords) {
        val obj = JSONObject()
        val zdt = record.startTime.atZone(zone)
        obj.put("date", zdt.toLocalDate().format(dateFmt))
        obj.put("start_time", zdt.format(timeFmt))
        obj.put("end_time", record.endTime.atZone(zone).format(timeFmt))
        obj.put("duration_minutes", Duration.between(record.startTime, record.endTime).toMinutes())
        obj.put("type", exerciseTypeToString(record.exerciseType))
        obj.put("title", record.title ?: "")
        obj.put("record_id", record.metadata.id)
        obj.put("source_package", record.metadata.dataOrigin.packageName)
        result.put(obj)
    }
    // Samsung sometimes continues exporting step intervals while no longer
    // creating ExerciseSessionRecord rows for its auto-detected walks. Recover
    // only sustained, walking-cadence intervals and never overlap a real
    // exercise session. Daily step totals remain sourced from HC aggregation;
    // these synthetic rows are used only to restore the missing walk timeline.
    val granted = client.permissionController.getGrantedPermissions()
    if (androidx.health.connect.client.permission.HealthPermission.getReadPermission(StepsRecord::class) !in granted) {
        return result
    }
    val stepRecords = readStepRecordsByDay(client, start, end)
    stepRecords.groupBy { it.metadata.dataOrigin.packageName }.forEach { (source, records) ->
        val short = records.count { Duration.between(it.startTime, it.endTime).toMinutes() <= 30L }
        val medium = records.count { Duration.between(it.startTime, it.endTime).toMinutes() in 31L..120L }
        Log.i("HanniHealthReader", "stepSource=$source records=${records.size} short=$short medium=$medium")
    }
    val recordsByDay = stepRecords.groupBy { it.startTime.atZone(zone).toLocalDate() }
    var walkGroups = 0
    var inferredWalks = 0
    val selectedSources = mutableMapOf<String, Int>()
    for ((date, dayRecords) in recordsByDay.toSortedMap()) {
        // Multiple apps may write the same steps. Pick the day's most complete
        // source instead of generating duplicate walks from every origin.
        val bySource = dayRecords.groupBy { it.metadata.dataOrigin.packageName }
        val chosen = (bySource.entries.firstOrNull { it.key.contains("shealth", ignoreCase = true) }
            ?: bySource.maxByOrNull { (_, records) -> records.size })
            ?.value
            .orEmpty()
            .sortedBy { it.startTime }
        chosen.firstOrNull()?.metadata?.dataOrigin?.packageName?.let {
            selectedSources[it] = (selectedSources[it] ?: 0) + 1
        }
        val walks = mutableListOf<StepWalk>()
        for (record in chosen) {
            val minutes = Duration.between(record.startTime, record.endTime).toMillis() / 60_000.0
            if (minutes <= 0.0 || minutes > 30.0 || record.count < 1L) continue
            val cadence = record.count / minutes
            if (cadence > 240.0) continue
            val previous = walks.lastOrNull()
            val gap = previous?.let { Duration.between(it.end, record.startTime).toMinutes() }
            if (previous != null && gap != null && gap in -2L..WALK_MERGE_GAP_MINUTES) {
                if (record.endTime > previous.end) previous.end = record.endTime
                previous.steps += record.count
            } else {
                walks.add(StepWalk(
                    start = record.startTime,
                    end = record.endTime,
                    steps = record.count,
                    sourcePackage = record.metadata.dataOrigin.packageName,
                ))
            }
        }
        for (walk in walks) {
            walkGroups += 1
            val minutes = Duration.between(walk.start, walk.end).toMinutes()
            if (minutes < WALK_MIN_MINUTES || minutes > 180L || walk.steps < WALK_MIN_STEPS) continue
            if (walk.steps.toDouble() / minutes.coerceAtLeast(1L) < 45.0) continue
            if (exerciseRecords.any { walk.start < it.endTime && walk.end > it.startTime }) continue
            val startTime = walk.start.atZone(zone).format(timeFmt)
            result.put(JSONObject().apply {
                put("date", date.format(dateFmt))
                put("start_time", startTime)
                put("end_time", walk.end.atZone(zone).format(timeFmt))
                put("duration_minutes", minutes)
                put("type", "walking")
                put("title", "Авто-прогулка по шагам")
                put("record_id", "steps-walk:${date.format(dateFmt)}:$startTime:${walk.sourcePackage}")
                put("source_package", walk.sourcePackage)
                put("steps", walk.steps)
                put("inferred_from_steps", true)
            })
            inferredWalks += 1
        }
    }
    Log.i("HanniHealthReader", "selectedSources=$selectedSources walkGroups=$walkGroups inferredWalks=$inferredWalks explicit=${exerciseRecords.size}")
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
