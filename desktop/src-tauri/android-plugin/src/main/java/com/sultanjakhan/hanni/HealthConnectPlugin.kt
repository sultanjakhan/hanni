// HealthConnectPlugin.kt — Tauri plugin: sleep, steps, heart rate, exercise from Health Connect
package com.sultanjakhan.hanni

import android.app.Activity
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.*
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.time.Instant

@TauriPlugin
class HealthConnectPlugin(private val activity: Activity) : Plugin(activity) {

    private var healthClient: HealthConnectClient? = null

    private val requiredPermissions = setOf(
        HealthPermission.getReadPermission(SleepSessionRecord::class),
        HealthPermission.getReadPermission(StepsRecord::class),
        HealthPermission.getReadPermission(HeartRateRecord::class),
        HealthPermission.getReadPermission(ExerciseSessionRecord::class),
    )

    override fun load(webView: WebView) {
        val status = HealthConnectClient.getSdkStatus(activity)
        if (status == HealthConnectClient.SDK_AVAILABLE) {
            healthClient = HealthConnectClient.getOrCreate(activity)
        }
    }

    private fun withClient(invoke: Invoke, block: suspend (HealthConnectClient) -> Unit) {
        val client = healthClient
        if (client == null) { invoke.reject("Health Connect not available"); return }
        CoroutineScope(Dispatchers.IO).launch {
            try {
                val granted = client.permissionController.getGrantedPermissions()
                if (!granted.containsAll(requiredPermissions)) {
                    invoke.reject("Health permissions not granted"); return@launch
                }
                block(client)
            } catch (e: Exception) {
                invoke.reject("Health Connect error: ${e.message}")
            }
        }
    }

    private fun last30Days(): Pair<Instant, Instant> {
        val end = Instant.now()
        val start = end.minusSeconds(30L * 24 * 3600)
        return Pair(start, end)
    }

    @Command
    fun readSleep(invoke: Invoke) = withClient(invoke) { client ->
        val (start, end) = last30Days()
        val ret = JSObject()
        ret.put("sessions", readSleepSessions(client, start, end))
        invoke.resolve(ret)
    }

    @Command
    fun readSteps(invoke: Invoke) = withClient(invoke) { client ->
        val (start, end) = last30Days()
        val ret = JSObject()
        ret.put("days", readDailySteps(client, start, end))
        invoke.resolve(ret)
    }

    @Command
    fun readHeartRate(invoke: Invoke) = withClient(invoke) { client ->
        val (start, end) = last30Days()
        val ret = JSObject()
        ret.put("samples", readHeartRateSamples(client, start, end))
        invoke.resolve(ret)
    }

    @Command
    fun readExercise(invoke: Invoke) = withClient(invoke) { client ->
        val (start, end) = last30Days()
        val ret = JSObject()
        ret.put("sessions", readExerciseSessions(client, start, end))
        invoke.resolve(ret)
    }
}
