@file:OptIn(androidx.health.connect.client.feature.ExperimentalMindfulnessSessionApi::class)

package com.sultanjakhan.hanni

import androidx.health.connect.client.records.*
import androidx.health.connect.client.records.metadata.*
import androidx.health.connect.client.units.*
import java.time.Duration
import java.time.Instant
import java.time.ZoneOffset

/** Synthetic fixtures only. These values are not user records. */
internal object RawHealthRecordFixtures {
    val start: Instant = Instant.ofEpochSecond(1_700_000_000L, 123_456_789)
    val end: Instant = start.plusSeconds(600).plusNanos(17)
    val offset: ZoneOffset = ZoneOffset.ofHoursMinutesSeconds(5, 30, 17)
    val meta: Metadata = Metadata.manualEntry(
        clientRecordId = "fixture-only", clientRecordVersion = Long.MAX_VALUE,
        device = Device(type = Device.TYPE_PHONE, manufacturer = "synthetic", model = "test"),
    )

    fun all(): List<Record> = listOf(
        ActiveCaloriesBurnedRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, energy = Energy.kilojoules(1.23456789012345), metadata = meta),
        BasalBodyTemperatureRecord(time = start, zoneOffset = null, metadata = meta, temperature = Temperature.fahrenheit(98.125), measurementLocation = 0),
        BasalMetabolicRateRecord(time = start, zoneOffset = null, basalMetabolicRate = Power.kilocaloriesPerDay(1.23456789012345), metadata = meta),
        BloodGlucoseRecord(time = start, zoneOffset = null, metadata = meta, level = BloodGlucose.milligramsPerDeciliter(81.125), specimenSource = 0, mealType = 0, relationToMeal = 0),
        BloodPressureRecord(time = start, zoneOffset = null, metadata = meta, systolic = Pressure.millimetersOfMercury(80.5), diastolic = Pressure.millimetersOfMercury(80.5), bodyPosition = 0, measurementLocation = 0),
        BodyFatRecord(time = start, zoneOffset = null, percentage = Percentage(50.125), metadata = meta),
        BodyTemperatureRecord(time = start, zoneOffset = null, metadata = meta, temperature = Temperature.fahrenheit(98.125), measurementLocation = 0),
        BodyWaterMassRecord(time = start, zoneOffset = null, mass = Mass.milligrams(1.23456789012345), metadata = meta),
        BoneMassRecord(time = start, zoneOffset = null, mass = Mass.milligrams(1.23456789012345), metadata = meta),
        CervicalMucusRecord(time = start, zoneOffset = null, metadata = meta, appearance = 0, sensation = 0),
        CyclingPedalingCadenceRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, samples = listOf(CyclingPedalingCadenceRecord.Sample(time = start, revolutionsPerMinute = 1.23456789012345)), metadata = meta),
        DistanceRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, distance = Length.feet(1.23456789012345), metadata = meta),
        ElevationGainedRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, elevation = Length.feet(1.23456789012345), metadata = meta),
        ExerciseSessionRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, metadata = meta, exerciseType = ExerciseSessionRecord.EXERCISE_TYPE_RUNNING, title = "synthetic \"quoted\" \n Unicode \u03bb", notes = "synthetic \"quoted\" \n Unicode \u03bb", segments = listOf(ExerciseSegment(startTime = start, endTime = end, segmentType = ExerciseSegment.EXERCISE_SEGMENT_TYPE_RUNNING, repetitions = 0)), laps = listOf(ExerciseLap(startTime = start, endTime = end, length = Length.feet(1.23456789012345))), exerciseRoute = ExerciseRoute(listOf(ExerciseRoute.Location(time = start, latitude = 1.23456789012345, longitude = -0.0, horizontalAccuracy = Length.feet(1.23456789012345), verticalAccuracy = Length.feet(1.23456789012345), altitude = Length.feet(1.23456789012345))))),
        FloorsClimbedRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, floors = 1.23456789012345, metadata = meta),
        HeartRateRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, samples = listOf(HeartRateRecord.Sample(time = start, beatsPerMinute = 63L)), metadata = meta),
        HeartRateVariabilityRmssdRecord(time = start, zoneOffset = null, heartRateVariabilityMillis = 1.23456789012345, metadata = meta),
        HeightRecord(time = start, zoneOffset = null, height = Length.feet(1.23456789012345), metadata = meta),
        HydrationRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, volume = Volume.fluidOuncesUs(1.23456789012345), metadata = meta),
        IntermenstrualBleedingRecord(time = start, zoneOffset = null, metadata = meta),
        LeanBodyMassRecord(time = start, zoneOffset = null, mass = Mass.milligrams(1.23456789012345), metadata = meta),
        MenstruationFlowRecord(time = start, zoneOffset = null, metadata = meta, flow = 0),
        MenstruationPeriodRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, metadata = meta),
        MindfulnessSessionRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, metadata = meta, mindfulnessSessionType = 0, title = "synthetic \"quoted\" \n Unicode \u03bb", notes = "synthetic \"quoted\" \n Unicode \u03bb"),
        NutritionRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, metadata = meta, biotin = Mass.milligrams(1.23456789012345), caffeine = Mass.milligrams(1.23456789012345), calcium = Mass.milligrams(1.23456789012345), energy = Energy.kilojoules(1.23456789012345), energyFromFat = Energy.kilojoules(1.23456789012345), chloride = Mass.milligrams(1.23456789012345), cholesterol = Mass.milligrams(1.23456789012345), chromium = Mass.milligrams(1.23456789012345), copper = Mass.milligrams(1.23456789012345), dietaryFiber = Mass.milligrams(1.23456789012345), folate = Mass.milligrams(1.23456789012345), folicAcid = Mass.milligrams(1.23456789012345), iodine = Mass.milligrams(1.23456789012345), iron = Mass.milligrams(1.23456789012345), magnesium = Mass.milligrams(1.23456789012345), manganese = Mass.milligrams(1.23456789012345), molybdenum = Mass.milligrams(1.23456789012345), monounsaturatedFat = Mass.milligrams(1.23456789012345), niacin = Mass.milligrams(1.23456789012345), pantothenicAcid = Mass.milligrams(1.23456789012345), phosphorus = Mass.milligrams(1.23456789012345), polyunsaturatedFat = Mass.milligrams(1.23456789012345), potassium = Mass.milligrams(1.23456789012345), protein = Mass.milligrams(1.23456789012345), riboflavin = Mass.milligrams(1.23456789012345), saturatedFat = Mass.milligrams(1.23456789012345), selenium = Mass.milligrams(1.23456789012345), sodium = Mass.milligrams(1.23456789012345), sugar = Mass.milligrams(1.23456789012345), thiamin = Mass.milligrams(1.23456789012345), totalCarbohydrate = Mass.milligrams(1.23456789012345), totalFat = Mass.milligrams(1.23456789012345), transFat = Mass.milligrams(1.23456789012345), unsaturatedFat = Mass.milligrams(1.23456789012345), vitaminA = Mass.milligrams(1.23456789012345), vitaminB12 = Mass.milligrams(1.23456789012345), vitaminB6 = Mass.milligrams(1.23456789012345), vitaminC = Mass.milligrams(1.23456789012345), vitaminD = Mass.milligrams(1.23456789012345), vitaminE = Mass.milligrams(1.23456789012345), vitaminK = Mass.milligrams(1.23456789012345), zinc = Mass.milligrams(1.23456789012345), name = "synthetic \"quoted\" \n Unicode \u03bb", mealType = 0),
        OvulationTestRecord(time = start, zoneOffset = null, result = 0, metadata = meta),
        OxygenSaturationRecord(time = start, zoneOffset = null, percentage = Percentage(50.125), metadata = meta),
        PlannedExerciseSessionRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, metadata = meta, exerciseType = ExerciseSessionRecord.EXERCISE_TYPE_RUNNING, blocks = listOf(plannedBlock()), title = "synthetic \"quoted\" \n Unicode \u03bb", notes = "synthetic \"quoted\" \n Unicode \u03bb"),
        PowerRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, samples = listOf(PowerRecord.Sample(time = start, power = Power.kilocaloriesPerDay(1.23456789012345))), metadata = meta),
        RespiratoryRateRecord(time = start, zoneOffset = null, rate = 1.23456789012345, metadata = meta),
        RestingHeartRateRecord(time = start, zoneOffset = null, beatsPerMinute = 63L, metadata = meta),
        SexualActivityRecord(time = start, zoneOffset = null, metadata = meta, protectionUsed = 0),
        SkinTemperatureRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, metadata = meta, deltas = listOf(SkinTemperatureRecord.Delta(time = start, delta = TemperatureDelta.fahrenheit(-0.125))), baseline = Temperature.fahrenheit(98.125), measurementLocation = 0),
        SleepSessionRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, metadata = meta, title = "synthetic \"quoted\" \n Unicode \u03bb", notes = "synthetic \"quoted\" \n Unicode \u03bb", stages = listOf(SleepSessionRecord.Stage(startTime = start, endTime = end, stage = SleepSessionRecord.STAGE_TYPE_AWAKE_IN_BED))),
        SpeedRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, samples = listOf(SpeedRecord.Sample(time = start, speed = Velocity.milesPerHour(1.23456789012345))), metadata = meta),
        StepsCadenceRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, samples = listOf(StepsCadenceRecord.Sample(time = start, rate = 1.23456789012345)), metadata = meta),
        StepsRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, count = 123L, metadata = meta),
        TotalCaloriesBurnedRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, energy = Energy.kilojoules(1.23456789012345), metadata = meta),
        Vo2MaxRecord(time = start, zoneOffset = null, metadata = meta, vo2MillilitersPerMinuteKilogram = 1.23456789012345, measurementMethod = 0),
        WeightRecord(time = start, zoneOffset = null, weight = Mass.milligrams(1.23456789012345), metadata = meta),
        WheelchairPushesRecord(startTime = start, startZoneOffset = null, endTime = end, endZoneOffset = offset, count = 123L, metadata = meta),
    )

    private fun plannedBlock(): PlannedExerciseBlock {
        val goals = listOf(
            ExerciseCompletionGoal.ActiveCaloriesBurnedGoal(Energy.calories(1.125)),
            ExerciseCompletionGoal.DistanceAndDurationGoal(Length.miles(1.125), Duration.ofSeconds(13, 37)),
            ExerciseCompletionGoal.DistanceGoal(Length.inches(1.125)),
            ExerciseCompletionGoal.DurationGoal(Duration.ofSeconds(17, 43)),
            ExerciseCompletionGoal.ManualCompletion,
            ExerciseCompletionGoal.RepetitionsGoal(7),
            ExerciseCompletionGoal.StepsGoal(123),
            ExerciseCompletionGoal.TotalCaloriesBurnedGoal(Energy.joules(1.125)),
            ExerciseCompletionGoal.UnknownGoal,
        )
        val targets = listOf(
            ExercisePerformanceTarget.AmrapTarget,
            ExercisePerformanceTarget.CadenceTarget(1.125, 2.125),
            ExercisePerformanceTarget.HeartRateTarget(61.125, 73.125),
            ExercisePerformanceTarget.PowerTarget(Power.watts(1.125), Power.watts(2.125)),
            ExercisePerformanceTarget.RateOfPerceivedExertionTarget(3),
            ExercisePerformanceTarget.SpeedTarget(Velocity.kilometersPerHour(1.125), Velocity.kilometersPerHour(2.125)),
            ExercisePerformanceTarget.UnknownTarget,
            ExercisePerformanceTarget.WeightTarget(Mass.pounds(1.125)),
        )
        return PlannedExerciseBlock(
            repetitions = 3,
            steps = goals.map { goal -> PlannedExerciseStep(
                exerciseType = ExerciseSessionRecord.EXERCISE_TYPE_RUNNING,
                exercisePhase = PlannedExerciseStep.EXERCISE_PHASE_ACTIVE,
                completionGoal = goal, performanceTargets = targets,
                description = "synthetic step",
            ) },
            description = "synthetic block",
        )
    }
}
