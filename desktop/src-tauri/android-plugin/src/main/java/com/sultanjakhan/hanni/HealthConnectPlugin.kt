// HealthConnectPlugin.kt — Tauri plugin for Samsung Health / Health Connect sleep import
// Will be integrated after `tauri android init` generates the Android project
package com.sultanjakhan.hanni

import android.app.Activity
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.SleepSessionRecord
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@TauriPlugin
class HealthConnectPlugin(private val activity: Activity) : Plugin(activity) {

    private var healthClient: HealthConnectClient? = null

    override fun load(webView: WebView) {
        val status = HealthConnectClient.getSdkStatus(activity)
        if (status == HealthConnectClient.SDK_AVAILABLE) {
            healthClient = HealthConnectClient.getOrCreate(activity)
        }
    }

    @Command
    fun readSleep(invoke: Invoke) {
        val client = healthClient
        if (client == null) {
            invoke.reject("Health Connect not available")
            return
        }

        CoroutineScope(Dispatchers.IO).launch {
            try {
                // Check permissions
                val perms = setOf(
                    HealthPermission.getReadPermission(SleepSessionRecord::class)
                )
                val granted = client.permissionController.getGrantedPermissions()
                if (!granted.containsAll(perms)) {
                    invoke.reject("Sleep read permission not granted")
                    return@launch
                }

                // Read last 30 days of sleep
                val end = Instant.now()
                val start = end.minusSeconds(30L * 24 * 3600)
                val response = client.readRecords(
                    ReadRecordsRequest(
                        SleepSessionRecord::class,
                        timeRangeFilter = TimeRangeFilter.between(start, end)
                    )
                )

                val sessions = JSONArray()
                val dateFmt = DateTimeFormatter.ISO_LOCAL_DATE
                val timeFmt = DateTimeFormatter.ISO_INSTANT

                for (record in response.records) {
                    val session = JSONObject()
                    val localDate = record.startTime.atZone(ZoneId.systemDefault()).toLocalDate()
                    session.put("date", localDate.format(dateFmt))
                    session.put("start_time", timeFmt.format(record.startTime))
                    session.put("end_time", timeFmt.format(record.endTime))
                    val durMin = java.time.Duration.between(record.startTime, record.endTime).toMinutes()
                    session.put("duration_minutes", durMin)
                    session.put("source", "health_connect")

                    val stages = JSONArray()
                    for (stage in record.stages) {
                        val stObj = JSONObject()
                        stObj.put("start_time", timeFmt.format(stage.startTime))
                        stObj.put("end_time", timeFmt.format(stage.endTime))
                        stObj.put("stage", stageToString(stage.stage))
                        stages.put(stObj)
                    }
                    session.put("stages", stages)
                    sessions.put(session)
                }

                val ret = JSObject()
                ret.put("sessions", sessions)
                invoke.resolve(ret)
            } catch (e: Exception) {
                invoke.reject("Failed to read sleep: ${e.message}")
            }
        }
    }

    private fun stageToString(stage: Int): String = when (stage) {
        SleepSessionRecord.STAGE_TYPE_AWAKE -> "awake"
        SleepSessionRecord.STAGE_TYPE_LIGHT -> "light"
        SleepSessionRecord.STAGE_TYPE_DEEP -> "deep"
        SleepSessionRecord.STAGE_TYPE_REM -> "rem"
        SleepSessionRecord.STAGE_TYPE_SLEEPING -> "sleeping"
        SleepSessionRecord.STAGE_TYPE_OUT_OF_BED -> "out_of_bed"
        SleepSessionRecord.STAGE_TYPE_AWAKE_IN_BED -> "awake"
        else -> "unknown"
    }
}
