@file:OptIn(androidx.health.connect.client.feature.ExperimentalMindfulnessSessionApi::class)

package com.sultanjakhan.hanni

import androidx.health.connect.client.HealthConnectFeatures
import androidx.health.connect.client.records.*
import androidx.health.connect.client.records.metadata.*
import androidx.health.connect.client.units.*
import org.json.JSONArray
import org.json.JSONObject
import java.time.Duration
import java.time.Instant
import kotlin.reflect.KClass

/**
 * Pinned HC 1.1.0 public-value archive encoder. No HC, DB, network or permission calls.
 * Generated explicit adapters: no runtime reflection and no Record.toString().
 * JSON contains private health data: callers must never log it.
 * This is an archive format, not a Health Connect write-back API.
 */
internal object RawHealthRecordCodec {
    const val VERSION = 1
    const val SDK = "androidx.health.connect:connect-client:1.1.0"

    data class Descriptor(
        val name: String,
        val recordClass: KClass<out Record>,
        val readPermission: String,
        val requiredFeature: Int?,
    )

    val descriptors: List<Descriptor> = listOf(
        Descriptor("ActiveCaloriesBurnedRecord", ActiveCaloriesBurnedRecord::class, "android.permission.health.READ_ACTIVE_CALORIES_BURNED", null),
        Descriptor("BasalBodyTemperatureRecord", BasalBodyTemperatureRecord::class, "android.permission.health.READ_BASAL_BODY_TEMPERATURE", null),
        Descriptor("BasalMetabolicRateRecord", BasalMetabolicRateRecord::class, "android.permission.health.READ_BASAL_METABOLIC_RATE", null),
        Descriptor("BloodGlucoseRecord", BloodGlucoseRecord::class, "android.permission.health.READ_BLOOD_GLUCOSE", null),
        Descriptor("BloodPressureRecord", BloodPressureRecord::class, "android.permission.health.READ_BLOOD_PRESSURE", null),
        Descriptor("BodyFatRecord", BodyFatRecord::class, "android.permission.health.READ_BODY_FAT", null),
        Descriptor("BodyTemperatureRecord", BodyTemperatureRecord::class, "android.permission.health.READ_BODY_TEMPERATURE", null),
        Descriptor("BodyWaterMassRecord", BodyWaterMassRecord::class, "android.permission.health.READ_BODY_WATER_MASS", null),
        Descriptor("BoneMassRecord", BoneMassRecord::class, "android.permission.health.READ_BONE_MASS", null),
        Descriptor("CervicalMucusRecord", CervicalMucusRecord::class, "android.permission.health.READ_CERVICAL_MUCUS", null),
        Descriptor("CyclingPedalingCadenceRecord", CyclingPedalingCadenceRecord::class, "android.permission.health.READ_EXERCISE", null),
        Descriptor("DistanceRecord", DistanceRecord::class, "android.permission.health.READ_DISTANCE", null),
        Descriptor("ElevationGainedRecord", ElevationGainedRecord::class, "android.permission.health.READ_ELEVATION_GAINED", null),
        Descriptor("ExerciseSessionRecord", ExerciseSessionRecord::class, "android.permission.health.READ_EXERCISE", null),
        Descriptor("FloorsClimbedRecord", FloorsClimbedRecord::class, "android.permission.health.READ_FLOORS_CLIMBED", null),
        Descriptor("HeartRateRecord", HeartRateRecord::class, "android.permission.health.READ_HEART_RATE", null),
        Descriptor("HeartRateVariabilityRmssdRecord", HeartRateVariabilityRmssdRecord::class, "android.permission.health.READ_HEART_RATE_VARIABILITY", null),
        Descriptor("HeightRecord", HeightRecord::class, "android.permission.health.READ_HEIGHT", null),
        Descriptor("HydrationRecord", HydrationRecord::class, "android.permission.health.READ_HYDRATION", null),
        Descriptor("IntermenstrualBleedingRecord", IntermenstrualBleedingRecord::class, "android.permission.health.READ_INTERMENSTRUAL_BLEEDING", null),
        Descriptor("LeanBodyMassRecord", LeanBodyMassRecord::class, "android.permission.health.READ_LEAN_BODY_MASS", null),
        Descriptor("MenstruationFlowRecord", MenstruationFlowRecord::class, "android.permission.health.READ_MENSTRUATION", null),
        Descriptor("MenstruationPeriodRecord", MenstruationPeriodRecord::class, "android.permission.health.READ_MENSTRUATION", null),
        Descriptor("MindfulnessSessionRecord", MindfulnessSessionRecord::class, "android.permission.health.READ_MINDFULNESS", HealthConnectFeatures.FEATURE_MINDFULNESS_SESSION),
        Descriptor("NutritionRecord", NutritionRecord::class, "android.permission.health.READ_NUTRITION", null),
        Descriptor("OvulationTestRecord", OvulationTestRecord::class, "android.permission.health.READ_OVULATION_TEST", null),
        Descriptor("OxygenSaturationRecord", OxygenSaturationRecord::class, "android.permission.health.READ_OXYGEN_SATURATION", null),
        Descriptor("PlannedExerciseSessionRecord", PlannedExerciseSessionRecord::class, "android.permission.health.READ_PLANNED_EXERCISE", HealthConnectFeatures.FEATURE_PLANNED_EXERCISE),
        Descriptor("PowerRecord", PowerRecord::class, "android.permission.health.READ_POWER", null),
        Descriptor("RespiratoryRateRecord", RespiratoryRateRecord::class, "android.permission.health.READ_RESPIRATORY_RATE", null),
        Descriptor("RestingHeartRateRecord", RestingHeartRateRecord::class, "android.permission.health.READ_RESTING_HEART_RATE", null),
        Descriptor("SexualActivityRecord", SexualActivityRecord::class, "android.permission.health.READ_SEXUAL_ACTIVITY", null),
        Descriptor("SkinTemperatureRecord", SkinTemperatureRecord::class, "android.permission.health.READ_SKIN_TEMPERATURE", HealthConnectFeatures.FEATURE_SKIN_TEMPERATURE),
        Descriptor("SleepSessionRecord", SleepSessionRecord::class, "android.permission.health.READ_SLEEP", null),
        Descriptor("SpeedRecord", SpeedRecord::class, "android.permission.health.READ_SPEED", null),
        Descriptor("StepsCadenceRecord", StepsCadenceRecord::class, "android.permission.health.READ_STEPS", null),
        Descriptor("StepsRecord", StepsRecord::class, "android.permission.health.READ_STEPS", null),
        Descriptor("TotalCaloriesBurnedRecord", TotalCaloriesBurnedRecord::class, "android.permission.health.READ_TOTAL_CALORIES_BURNED", null),
        Descriptor("Vo2MaxRecord", Vo2MaxRecord::class, "android.permission.health.READ_VO2_MAX", null),
        Descriptor("WeightRecord", WeightRecord::class, "android.permission.health.READ_WEIGHT", null),
        Descriptor("WheelchairPushesRecord", WheelchairPushesRecord::class, "android.permission.health.READ_WHEELCHAIR_PUSHES", null),
    )

    fun encode(record: Record): JSONObject {
        val (name, body) = when (record) {
            is ActiveCaloriesBurnedRecord -> "ActiveCaloriesBurnedRecord" to encodeActiveCaloriesBurnedRecord(record)
            is BasalBodyTemperatureRecord -> "BasalBodyTemperatureRecord" to encodeBasalBodyTemperatureRecord(record)
            is BasalMetabolicRateRecord -> "BasalMetabolicRateRecord" to encodeBasalMetabolicRateRecord(record)
            is BloodGlucoseRecord -> "BloodGlucoseRecord" to encodeBloodGlucoseRecord(record)
            is BloodPressureRecord -> "BloodPressureRecord" to encodeBloodPressureRecord(record)
            is BodyFatRecord -> "BodyFatRecord" to encodeBodyFatRecord(record)
            is BodyTemperatureRecord -> "BodyTemperatureRecord" to encodeBodyTemperatureRecord(record)
            is BodyWaterMassRecord -> "BodyWaterMassRecord" to encodeBodyWaterMassRecord(record)
            is BoneMassRecord -> "BoneMassRecord" to encodeBoneMassRecord(record)
            is CervicalMucusRecord -> "CervicalMucusRecord" to encodeCervicalMucusRecord(record)
            is CyclingPedalingCadenceRecord -> "CyclingPedalingCadenceRecord" to encodeCyclingPedalingCadenceRecord(record)
            is DistanceRecord -> "DistanceRecord" to encodeDistanceRecord(record)
            is ElevationGainedRecord -> "ElevationGainedRecord" to encodeElevationGainedRecord(record)
            is ExerciseSessionRecord -> "ExerciseSessionRecord" to encodeExerciseSessionRecord(record)
            is FloorsClimbedRecord -> "FloorsClimbedRecord" to encodeFloorsClimbedRecord(record)
            is HeartRateRecord -> "HeartRateRecord" to encodeHeartRateRecord(record)
            is HeartRateVariabilityRmssdRecord -> "HeartRateVariabilityRmssdRecord" to encodeHeartRateVariabilityRmssdRecord(record)
            is HeightRecord -> "HeightRecord" to encodeHeightRecord(record)
            is HydrationRecord -> "HydrationRecord" to encodeHydrationRecord(record)
            is IntermenstrualBleedingRecord -> "IntermenstrualBleedingRecord" to encodeIntermenstrualBleedingRecord(record)
            is LeanBodyMassRecord -> "LeanBodyMassRecord" to encodeLeanBodyMassRecord(record)
            is MenstruationFlowRecord -> "MenstruationFlowRecord" to encodeMenstruationFlowRecord(record)
            is MenstruationPeriodRecord -> "MenstruationPeriodRecord" to encodeMenstruationPeriodRecord(record)
            is MindfulnessSessionRecord -> "MindfulnessSessionRecord" to encodeMindfulnessSessionRecord(record)
            is NutritionRecord -> "NutritionRecord" to encodeNutritionRecord(record)
            is OvulationTestRecord -> "OvulationTestRecord" to encodeOvulationTestRecord(record)
            is OxygenSaturationRecord -> "OxygenSaturationRecord" to encodeOxygenSaturationRecord(record)
            is PlannedExerciseSessionRecord -> "PlannedExerciseSessionRecord" to encodePlannedExerciseSessionRecord(record)
            is PowerRecord -> "PowerRecord" to encodePowerRecord(record)
            is RespiratoryRateRecord -> "RespiratoryRateRecord" to encodeRespiratoryRateRecord(record)
            is RestingHeartRateRecord -> "RestingHeartRateRecord" to encodeRestingHeartRateRecord(record)
            is SexualActivityRecord -> "SexualActivityRecord" to encodeSexualActivityRecord(record)
            is SkinTemperatureRecord -> "SkinTemperatureRecord" to encodeSkinTemperatureRecord(record)
            is SleepSessionRecord -> "SleepSessionRecord" to encodeSleepSessionRecord(record)
            is SpeedRecord -> "SpeedRecord" to encodeSpeedRecord(record)
            is StepsCadenceRecord -> "StepsCadenceRecord" to encodeStepsCadenceRecord(record)
            is StepsRecord -> "StepsRecord" to encodeStepsRecord(record)
            is TotalCaloriesBurnedRecord -> "TotalCaloriesBurnedRecord" to encodeTotalCaloriesBurnedRecord(record)
            is Vo2MaxRecord -> "Vo2MaxRecord" to encodeVo2MaxRecord(record)
            is WeightRecord -> "WeightRecord" to encodeWeightRecord(record)
            is WheelchairPushesRecord -> "WheelchairPushesRecord" to encodeWheelchairPushesRecord(record)
            else -> throw IllegalArgumentException("hc_record_type_unsupported")
        }
        return obj("v" to VERSION, "sdk" to SDK, "record_type" to name, "record" to body)
    }

    // String bits preserve signed zero, all finite values and any non-finite SDK values.
    // Do not coerce these objects through JSON Double/JavaScript Number conversion.
    internal fun f64(value: Double): JSONObject = obj(
        "f64" to java.lang.Long.toHexString(java.lang.Double.doubleToRawLongBits(value)).padStart(16, '0'),
    )

    internal fun readF64(value: JSONObject): Double {
        require(value.length() == 1) { "hc_f64_invalid" }
        val hex = value.opt("f64")
        require(hex is String && hex.length == 16 && hex.all { it in '0'..'9' || it in 'a'..'f' }) {
            "hc_f64_invalid"
        }
        return java.lang.Double.longBitsToDouble(java.lang.Long.parseUnsignedLong(hex, 16))
    }

    private fun instant(value: Instant?): Any = if (value == null) JSONObject.NULL else obj(
        "seconds" to java.lang.Long.toString(value.epochSecond), "nanos" to value.nano,
    )

    private fun duration(value: Duration?): Any = if (value == null) JSONObject.NULL else obj(
        "seconds" to java.lang.Long.toString(value.seconds), "nanos" to value.nano,
    )

    private fun obj(vararg fields: Pair<String, Any?>): JSONObject = JSONObject().apply {
        for ((name, value) in fields) put(name, value ?: JSONObject.NULL)
    }

    private fun <T> array(values: List<T>, encode: (T) -> Any): JSONArray = JSONArray().apply {
        for (value in values) put(encode(value))
    }

    private fun encodeActiveCaloriesBurnedRecord(value: androidx.health.connect.client.records.ActiveCaloriesBurnedRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "energy" to encodeEnergy(value.energy),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeBasalBodyTemperatureRecord(value: androidx.health.connect.client.records.BasalBodyTemperatureRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "temperature" to encodeTemperature(value.temperature),
            "measurementLocation" to value.measurementLocation,
        )
    }

    private fun encodeBasalMetabolicRateRecord(value: androidx.health.connect.client.records.BasalMetabolicRateRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "basalMetabolicRate" to encodePower(value.basalMetabolicRate),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeBloodGlucoseRecord(value: androidx.health.connect.client.records.BloodGlucoseRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "level" to encodeBloodGlucose(value.level),
            "specimenSource" to value.specimenSource,
            "mealType" to value.mealType,
            "relationToMeal" to value.relationToMeal,
        )
    }

    private fun encodeBloodPressureRecord(value: androidx.health.connect.client.records.BloodPressureRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "systolic" to encodePressure(value.systolic),
            "diastolic" to encodePressure(value.diastolic),
            "bodyPosition" to value.bodyPosition,
            "measurementLocation" to value.measurementLocation,
        )
    }

    private fun encodeBodyFatRecord(value: androidx.health.connect.client.records.BodyFatRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "percentage" to encodePercentage(value.percentage),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeBodyTemperatureRecord(value: androidx.health.connect.client.records.BodyTemperatureRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "temperature" to encodeTemperature(value.temperature),
            "measurementLocation" to value.measurementLocation,
        )
    }

    private fun encodeBodyWaterMassRecord(value: androidx.health.connect.client.records.BodyWaterMassRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "mass" to encodeMass(value.mass),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeBoneMassRecord(value: androidx.health.connect.client.records.BoneMassRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "mass" to encodeMass(value.mass),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeCervicalMucusRecord(value: androidx.health.connect.client.records.CervicalMucusRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "appearance" to value.appearance,
            "sensation" to value.sensation,
        )
    }

    private fun encodeCyclingPedalingCadenceRecord(value: androidx.health.connect.client.records.CyclingPedalingCadenceRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "samples" to array(value.samples) { encodeCyclingPedalingCadenceRecordSample(it) },
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeCyclingPedalingCadenceRecordSample(value: androidx.health.connect.client.records.CyclingPedalingCadenceRecord.Sample?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "revolutionsPerMinute" to f64(value.revolutionsPerMinute),
        )
    }

    private fun encodeDistanceRecord(value: androidx.health.connect.client.records.DistanceRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "distance" to encodeLength(value.distance),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeElevationGainedRecord(value: androidx.health.connect.client.records.ElevationGainedRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "elevation" to encodeLength(value.elevation),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeExerciseCompletionGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal?): Any {
        if (value == null) return JSONObject.NULL
        return when (value) {
            is androidx.health.connect.client.records.ExerciseCompletionGoal.ActiveCaloriesBurnedGoal -> encodeExerciseCompletionGoalActiveCaloriesBurnedGoal(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.DistanceAndDurationGoal -> encodeExerciseCompletionGoalDistanceAndDurationGoal(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.DistanceGoal -> encodeExerciseCompletionGoalDistanceGoal(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.DurationGoal -> encodeExerciseCompletionGoalDurationGoal(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.ManualCompletion -> encodeExerciseCompletionGoalManualCompletion(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.RepetitionsGoal -> encodeExerciseCompletionGoalRepetitionsGoal(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.StepsGoal -> encodeExerciseCompletionGoalStepsGoal(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.TotalCaloriesBurnedGoal -> encodeExerciseCompletionGoalTotalCaloriesBurnedGoal(value)
            is androidx.health.connect.client.records.ExerciseCompletionGoal.UnknownGoal -> encodeExerciseCompletionGoalUnknownGoal(value)
            else -> throw IllegalArgumentException("hc_nested_type_unsupported")
        }
    }

    private fun encodeExerciseCompletionGoalActiveCaloriesBurnedGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.ActiveCaloriesBurnedGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.ActiveCaloriesBurnedGoal",
            "activeCalories" to encodeEnergy(value.activeCalories),
        )
    }

    private fun encodeExerciseCompletionGoalDistanceAndDurationGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.DistanceAndDurationGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.DistanceAndDurationGoal",
            "distance" to encodeLength(value.distance),
            "duration" to duration(value.duration),
        )
    }

    private fun encodeExerciseCompletionGoalDistanceGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.DistanceGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.DistanceGoal",
            "distance" to encodeLength(value.distance),
        )
    }

    private fun encodeExerciseCompletionGoalDurationGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.DurationGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.DurationGoal",
            "duration" to duration(value.duration),
        )
    }

    private fun encodeExerciseCompletionGoalManualCompletion(value: androidx.health.connect.client.records.ExerciseCompletionGoal.ManualCompletion?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.ManualCompletion",
        )
    }

    private fun encodeExerciseCompletionGoalRepetitionsGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.RepetitionsGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.RepetitionsGoal",
            "repetitions" to value.repetitions,
        )
    }

    private fun encodeExerciseCompletionGoalStepsGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.StepsGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.StepsGoal",
            "steps" to value.steps,
        )
    }

    private fun encodeExerciseCompletionGoalTotalCaloriesBurnedGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.TotalCaloriesBurnedGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.TotalCaloriesBurnedGoal",
            "totalCalories" to encodeEnergy(value.totalCalories),
        )
    }

    private fun encodeExerciseCompletionGoalUnknownGoal(value: androidx.health.connect.client.records.ExerciseCompletionGoal.UnknownGoal?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseCompletionGoal.UnknownGoal",
        )
    }

    private fun encodeExerciseLap(value: androidx.health.connect.client.records.ExerciseLap?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "endTime" to instant(value.endTime),
            "length" to encodeLength(value.length),
        )
    }

    private fun encodeExercisePerformanceTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget?): Any {
        if (value == null) return JSONObject.NULL
        return when (value) {
            is androidx.health.connect.client.records.ExercisePerformanceTarget.AmrapTarget -> encodeExercisePerformanceTargetAmrapTarget(value)
            is androidx.health.connect.client.records.ExercisePerformanceTarget.CadenceTarget -> encodeExercisePerformanceTargetCadenceTarget(value)
            is androidx.health.connect.client.records.ExercisePerformanceTarget.HeartRateTarget -> encodeExercisePerformanceTargetHeartRateTarget(value)
            is androidx.health.connect.client.records.ExercisePerformanceTarget.PowerTarget -> encodeExercisePerformanceTargetPowerTarget(value)
            is androidx.health.connect.client.records.ExercisePerformanceTarget.RateOfPerceivedExertionTarget -> encodeExercisePerformanceTargetRateOfPerceivedExertionTarget(value)
            is androidx.health.connect.client.records.ExercisePerformanceTarget.SpeedTarget -> encodeExercisePerformanceTargetSpeedTarget(value)
            is androidx.health.connect.client.records.ExercisePerformanceTarget.UnknownTarget -> encodeExercisePerformanceTargetUnknownTarget(value)
            is androidx.health.connect.client.records.ExercisePerformanceTarget.WeightTarget -> encodeExercisePerformanceTargetWeightTarget(value)
            else -> throw IllegalArgumentException("hc_nested_type_unsupported")
        }
    }

    private fun encodeExercisePerformanceTargetAmrapTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.AmrapTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.AmrapTarget",
        )
    }

    private fun encodeExercisePerformanceTargetCadenceTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.CadenceTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.CadenceTarget",
            "minCadence" to f64(value.minCadence),
            "maxCadence" to f64(value.maxCadence),
        )
    }

    private fun encodeExercisePerformanceTargetHeartRateTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.HeartRateTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.HeartRateTarget",
            "minHeartRate" to f64(value.minHeartRate),
            "maxHeartRate" to f64(value.maxHeartRate),
        )
    }

    private fun encodeExercisePerformanceTargetPowerTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.PowerTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.PowerTarget",
            "minPower" to encodePower(value.minPower),
            "maxPower" to encodePower(value.maxPower),
        )
    }

    private fun encodeExercisePerformanceTargetRateOfPerceivedExertionTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.RateOfPerceivedExertionTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.RateOfPerceivedExertionTarget",
            "rpe" to value.rpe,
        )
    }

    private fun encodeExercisePerformanceTargetSpeedTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.SpeedTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.SpeedTarget",
            "minSpeed" to encodeVelocity(value.minSpeed),
            "maxSpeed" to encodeVelocity(value.maxSpeed),
        )
    }

    private fun encodeExercisePerformanceTargetUnknownTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.UnknownTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.UnknownTarget",
        )
    }

    private fun encodeExercisePerformanceTargetWeightTarget(value: androidx.health.connect.client.records.ExercisePerformanceTarget.WeightTarget?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExercisePerformanceTarget.WeightTarget",
            "mass" to encodeMass(value.mass),
        )
    }

    private fun encodeExerciseRoute(value: androidx.health.connect.client.records.ExerciseRoute?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "route" to array(value.route) { encodeExerciseRouteLocation(it) },
        )
    }

    private fun encodeExerciseRouteLocation(value: androidx.health.connect.client.records.ExerciseRoute.Location?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "latitude" to f64(value.latitude),
            "longitude" to f64(value.longitude),
            "horizontalAccuracy" to encodeLength(value.horizontalAccuracy),
            "verticalAccuracy" to encodeLength(value.verticalAccuracy),
            "altitude" to encodeLength(value.altitude),
        )
    }

    private fun encodeExerciseRouteResult(value: androidx.health.connect.client.records.ExerciseRouteResult?): Any {
        if (value == null) return JSONObject.NULL
        return when (value) {
            is androidx.health.connect.client.records.ExerciseRouteResult.ConsentRequired -> encodeExerciseRouteResultConsentRequired(value)
            is androidx.health.connect.client.records.ExerciseRouteResult.Data -> encodeExerciseRouteResultData(value)
            is androidx.health.connect.client.records.ExerciseRouteResult.NoData -> encodeExerciseRouteResultNoData(value)
            else -> throw IllegalArgumentException("hc_nested_type_unsupported")
        }
    }

    private fun encodeExerciseRouteResultConsentRequired(value: androidx.health.connect.client.records.ExerciseRouteResult.ConsentRequired?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseRouteResult.ConsentRequired",
        )
    }

    private fun encodeExerciseRouteResultData(value: androidx.health.connect.client.records.ExerciseRouteResult.Data?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseRouteResult.Data",
            "exerciseRoute" to encodeExerciseRoute(value.exerciseRoute),
        )
    }

    private fun encodeExerciseRouteResultNoData(value: androidx.health.connect.client.records.ExerciseRouteResult.NoData?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "ExerciseRouteResult.NoData",
        )
    }

    private fun encodeExerciseSegment(value: androidx.health.connect.client.records.ExerciseSegment?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "endTime" to instant(value.endTime),
            "segmentType" to value.segmentType,
            "repetitions" to value.repetitions,
        )
    }

    private fun encodeExerciseSessionRecord(value: androidx.health.connect.client.records.ExerciseSessionRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "exerciseType" to value.exerciseType,
            "title" to value.title,
            "notes" to value.notes,
            "segments" to array(value.segments) { encodeExerciseSegment(it) },
            "laps" to array(value.laps) { encodeExerciseLap(it) },
            "exerciseRouteResult" to encodeExerciseRouteResult(value.exerciseRouteResult),
            "plannedExerciseSessionId" to value.plannedExerciseSessionId,
        )
    }

    private fun encodeFloorsClimbedRecord(value: androidx.health.connect.client.records.FloorsClimbedRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "floors" to f64(value.floors),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeHeartRateRecord(value: androidx.health.connect.client.records.HeartRateRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "samples" to array(value.samples) { encodeHeartRateRecordSample(it) },
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeHeartRateRecordSample(value: androidx.health.connect.client.records.HeartRateRecord.Sample?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "beatsPerMinute" to java.lang.Long.toString(value.beatsPerMinute),
        )
    }

    private fun encodeHeartRateVariabilityRmssdRecord(value: androidx.health.connect.client.records.HeartRateVariabilityRmssdRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "heartRateVariabilityMillis" to f64(value.heartRateVariabilityMillis),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeHeightRecord(value: androidx.health.connect.client.records.HeightRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "height" to encodeLength(value.height),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeHydrationRecord(value: androidx.health.connect.client.records.HydrationRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "volume" to encodeVolume(value.volume),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeIntermenstrualBleedingRecord(value: androidx.health.connect.client.records.IntermenstrualBleedingRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeLeanBodyMassRecord(value: androidx.health.connect.client.records.LeanBodyMassRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "mass" to encodeMass(value.mass),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeMenstruationFlowRecord(value: androidx.health.connect.client.records.MenstruationFlowRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "flow" to value.flow,
        )
    }

    private fun encodeMenstruationPeriodRecord(value: androidx.health.connect.client.records.MenstruationPeriodRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeMindfulnessSessionRecord(value: androidx.health.connect.client.records.MindfulnessSessionRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "mindfulnessSessionType" to value.mindfulnessSessionType,
            "title" to value.title,
            "notes" to value.notes,
        )
    }

    private fun encodeNutritionRecord(value: androidx.health.connect.client.records.NutritionRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "biotin" to encodeMass(value.biotin),
            "caffeine" to encodeMass(value.caffeine),
            "calcium" to encodeMass(value.calcium),
            "energy" to encodeEnergy(value.energy),
            "energyFromFat" to encodeEnergy(value.energyFromFat),
            "chloride" to encodeMass(value.chloride),
            "cholesterol" to encodeMass(value.cholesterol),
            "chromium" to encodeMass(value.chromium),
            "copper" to encodeMass(value.copper),
            "dietaryFiber" to encodeMass(value.dietaryFiber),
            "folate" to encodeMass(value.folate),
            "folicAcid" to encodeMass(value.folicAcid),
            "iodine" to encodeMass(value.iodine),
            "iron" to encodeMass(value.iron),
            "magnesium" to encodeMass(value.magnesium),
            "manganese" to encodeMass(value.manganese),
            "molybdenum" to encodeMass(value.molybdenum),
            "monounsaturatedFat" to encodeMass(value.monounsaturatedFat),
            "niacin" to encodeMass(value.niacin),
            "pantothenicAcid" to encodeMass(value.pantothenicAcid),
            "phosphorus" to encodeMass(value.phosphorus),
            "polyunsaturatedFat" to encodeMass(value.polyunsaturatedFat),
            "potassium" to encodeMass(value.potassium),
            "protein" to encodeMass(value.protein),
            "riboflavin" to encodeMass(value.riboflavin),
            "saturatedFat" to encodeMass(value.saturatedFat),
            "selenium" to encodeMass(value.selenium),
            "sodium" to encodeMass(value.sodium),
            "sugar" to encodeMass(value.sugar),
            "thiamin" to encodeMass(value.thiamin),
            "totalCarbohydrate" to encodeMass(value.totalCarbohydrate),
            "totalFat" to encodeMass(value.totalFat),
            "transFat" to encodeMass(value.transFat),
            "unsaturatedFat" to encodeMass(value.unsaturatedFat),
            "vitaminA" to encodeMass(value.vitaminA),
            "vitaminB12" to encodeMass(value.vitaminB12),
            "vitaminB6" to encodeMass(value.vitaminB6),
            "vitaminC" to encodeMass(value.vitaminC),
            "vitaminD" to encodeMass(value.vitaminD),
            "vitaminE" to encodeMass(value.vitaminE),
            "vitaminK" to encodeMass(value.vitaminK),
            "zinc" to encodeMass(value.zinc),
            "name" to value.name,
            "mealType" to value.mealType,
        )
    }

    private fun encodeOvulationTestRecord(value: androidx.health.connect.client.records.OvulationTestRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "result" to value.result,
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeOxygenSaturationRecord(value: androidx.health.connect.client.records.OxygenSaturationRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "percentage" to encodePercentage(value.percentage),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodePlannedExerciseBlock(value: androidx.health.connect.client.records.PlannedExerciseBlock?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "repetitions" to value.repetitions,
            "steps" to array(value.steps) { encodePlannedExerciseStep(it) },
            "description" to value.description,
        )
    }

    private fun encodePlannedExerciseSessionRecord(value: androidx.health.connect.client.records.PlannedExerciseSessionRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "hasExplicitTime" to value.hasExplicitTime,
            "exerciseType" to value.exerciseType,
            "completedExerciseSessionId" to value.completedExerciseSessionId,
            "blocks" to array(value.blocks) { encodePlannedExerciseBlock(it) },
            "title" to value.title,
            "notes" to value.notes,
        )
    }

    private fun encodePlannedExerciseStep(value: androidx.health.connect.client.records.PlannedExerciseStep?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "exerciseType" to value.exerciseType,
            "exercisePhase" to value.exercisePhase,
            "completionGoal" to encodeExerciseCompletionGoal(value.completionGoal),
            "performanceTargets" to array(value.performanceTargets) { encodeExercisePerformanceTarget(it) },
            "description" to value.description,
        )
    }

    private fun encodePowerRecord(value: androidx.health.connect.client.records.PowerRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "samples" to array(value.samples) { encodePowerRecordSample(it) },
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodePowerRecordSample(value: androidx.health.connect.client.records.PowerRecord.Sample?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "power" to encodePower(value.power),
        )
    }

    private fun encodeRespiratoryRateRecord(value: androidx.health.connect.client.records.RespiratoryRateRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "rate" to f64(value.rate),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeRestingHeartRateRecord(value: androidx.health.connect.client.records.RestingHeartRateRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "beatsPerMinute" to java.lang.Long.toString(value.beatsPerMinute),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeSexualActivityRecord(value: androidx.health.connect.client.records.SexualActivityRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "protectionUsed" to value.protectionUsed,
        )
    }

    private fun encodeSkinTemperatureRecord(value: androidx.health.connect.client.records.SkinTemperatureRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "deltas" to array(value.deltas) { encodeSkinTemperatureRecordDelta(it) },
            "baseline" to encodeTemperature(value.baseline),
            "measurementLocation" to value.measurementLocation,
        )
    }

    private fun encodeSkinTemperatureRecordDelta(value: androidx.health.connect.client.records.SkinTemperatureRecord.Delta?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "delta" to encodeTemperatureDelta(value.delta),
        )
    }

    private fun encodeSleepSessionRecord(value: androidx.health.connect.client.records.SleepSessionRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "title" to value.title,
            "notes" to value.notes,
            "stages" to array(value.stages) { encodeSleepSessionRecordStage(it) },
        )
    }

    private fun encodeSleepSessionRecordStage(value: androidx.health.connect.client.records.SleepSessionRecord.Stage?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "endTime" to instant(value.endTime),
            "stage" to value.stage,
        )
    }

    private fun encodeSpeedRecord(value: androidx.health.connect.client.records.SpeedRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "samples" to array(value.samples) { encodeSpeedRecordSample(it) },
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeSpeedRecordSample(value: androidx.health.connect.client.records.SpeedRecord.Sample?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "speed" to encodeVelocity(value.speed),
        )
    }

    private fun encodeStepsCadenceRecord(value: androidx.health.connect.client.records.StepsCadenceRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "samples" to array(value.samples) { encodeStepsCadenceRecordSample(it) },
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeStepsCadenceRecordSample(value: androidx.health.connect.client.records.StepsCadenceRecord.Sample?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "rate" to f64(value.rate),
        )
    }

    private fun encodeStepsRecord(value: androidx.health.connect.client.records.StepsRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "count" to java.lang.Long.toString(value.count),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeTotalCaloriesBurnedRecord(value: androidx.health.connect.client.records.TotalCaloriesBurnedRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "energy" to encodeEnergy(value.energy),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeVo2MaxRecord(value: androidx.health.connect.client.records.Vo2MaxRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "metadata" to encodeMetadata(value.metadata),
            "vo2MillilitersPerMinuteKilogram" to f64(value.vo2MillilitersPerMinuteKilogram),
            "measurementMethod" to value.measurementMethod,
        )
    }

    private fun encodeWeightRecord(value: androidx.health.connect.client.records.WeightRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "time" to instant(value.time),
            "zoneOffset" to value.zoneOffset?.totalSeconds,
            "weight" to encodeMass(value.weight),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeWheelchairPushesRecord(value: androidx.health.connect.client.records.WheelchairPushesRecord?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "startTime" to instant(value.startTime),
            "startZoneOffset" to value.startZoneOffset?.totalSeconds,
            "endTime" to instant(value.endTime),
            "endZoneOffset" to value.endZoneOffset?.totalSeconds,
            "count" to java.lang.Long.toString(value.count),
            "metadata" to encodeMetadata(value.metadata),
        )
    }

    private fun encodeDataOrigin(value: androidx.health.connect.client.records.metadata.DataOrigin?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "packageName" to value.packageName,
        )
    }

    private fun encodeDevice(value: androidx.health.connect.client.records.metadata.Device?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to value.type,
            "manufacturer" to value.manufacturer,
            "model" to value.model,
        )
    }

    private fun encodeMetadata(value: androidx.health.connect.client.records.metadata.Metadata?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "recordingMethod" to value.recordingMethod,
            "id" to value.id,
            "dataOrigin" to encodeDataOrigin(value.dataOrigin),
            "lastModifiedTime" to instant(value.lastModifiedTime),
            "clientRecordId" to value.clientRecordId,
            "clientRecordVersion" to java.lang.Long.toString(value.clientRecordVersion),
            "device" to encodeDevice(value.device),
        )
    }

    private fun encodeBloodGlucose(value: androidx.health.connect.client.units.BloodGlucose?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "BloodGlucose",
            "primary_unit" to "millimolesPerLiter",
            "millimolesPerLiter" to f64(value.inMillimolesPerLiter),
            "milligramsPerDeciliter" to f64(value.inMilligramsPerDeciliter),
        )
    }

    private fun encodeEnergy(value: androidx.health.connect.client.units.Energy?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Energy",
            "primary_unit" to "kilocalories",
            "calories" to f64(value.inCalories),
            "kilocalories" to f64(value.inKilocalories),
            "joules" to f64(value.inJoules),
            "kilojoules" to f64(value.inKilojoules),
        )
    }

    private fun encodeLength(value: androidx.health.connect.client.units.Length?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Length",
            "primary_unit" to "meters",
            "meters" to f64(value.inMeters),
            "kilometers" to f64(value.inKilometers),
            "miles" to f64(value.inMiles),
            "inches" to f64(value.inInches),
            "feet" to f64(value.inFeet),
        )
    }

    private fun encodeMass(value: androidx.health.connect.client.units.Mass?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Mass",
            "primary_unit" to "grams",
            "grams" to f64(value.inGrams),
            "kilograms" to f64(value.inKilograms),
            "milligrams" to f64(value.inMilligrams),
            "micrograms" to f64(value.inMicrograms),
            "ounces" to f64(value.inOunces),
            "pounds" to f64(value.inPounds),
        )
    }

    private fun encodePercentage(value: androidx.health.connect.client.units.Percentage?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Percentage",
            "primary_unit" to "value",
            "value" to f64(value.value),
        )
    }

    private fun encodePower(value: androidx.health.connect.client.units.Power?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Power",
            "primary_unit" to "watts",
            "watts" to f64(value.inWatts),
            "kilocaloriesPerDay" to f64(value.inKilocaloriesPerDay),
        )
    }

    private fun encodePressure(value: androidx.health.connect.client.units.Pressure?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Pressure",
            "primary_unit" to "millimetersOfMercury",
            "millimetersOfMercury" to f64(value.inMillimetersOfMercury),
        )
    }

    private fun encodeTemperature(value: androidx.health.connect.client.units.Temperature?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Temperature",
            "primary_unit" to "celsius",
            "celsius" to f64(value.inCelsius),
            "fahrenheit" to f64(value.inFahrenheit),
        )
    }

    private fun encodeTemperatureDelta(value: androidx.health.connect.client.units.TemperatureDelta?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "TemperatureDelta",
            "primary_unit" to "celsius",
            "celsius" to f64(value.inCelsius),
            "fahrenheit" to f64(value.inFahrenheit),
        )
    }

    private fun encodeVelocity(value: androidx.health.connect.client.units.Velocity?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Velocity",
            "primary_unit" to "metersPerSecond",
            "metersPerSecond" to f64(value.inMetersPerSecond),
            "kilometersPerHour" to f64(value.inKilometersPerHour),
            "milesPerHour" to f64(value.inMilesPerHour),
        )
    }

    private fun encodeVolume(value: androidx.health.connect.client.units.Volume?): Any {
        if (value == null) return JSONObject.NULL
        return obj(
            "type" to "Volume",
            "primary_unit" to "liters",
            "liters" to f64(value.inLiters),
            "milliliters" to f64(value.inMilliliters),
            "fluidOuncesUs" to f64(value.inFluidOuncesUs),
        )
    }

}
