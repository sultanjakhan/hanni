// HealthConnectPlugin.kt — Tauri plugin: sleep, steps, heart rate, exercise from Health Connect
package com.sultanjakhan.hanni

import android.app.Activity
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.PermissionController
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.*
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.time.Instant

@TauriPlugin
class HealthConnectPlugin(private val activity: Activity) : Plugin(activity) {

    private var healthClient: HealthConnectClient? = null
    private var permLauncher: ActivityResultLauncher<Set<String>>? = null
    @Volatile private var pendingPermInvoke: Invoke? = null

    private val requiredPermissions = setOf(
        HealthPermission.getReadPermission(SleepSessionRecord::class),
        HealthPermission.getReadPermission(StepsRecord::class),
        HealthPermission.getReadPermission(HeartRateRecord::class),
        HealthPermission.getReadPermission(ExerciseSessionRecord::class),
    )

    override fun load(webView: WebView) {
        try {
            val status = HealthConnectClient.getSdkStatus(activity)
            if (status == HealthConnectClient.SDK_AVAILABLE) {
                healthClient = HealthConnectClient.getOrCreate(activity)
            }
        } catch (_: Throwable) {
            // SDK probe failed (provider missing, old Android, etc.) — leave
            // healthClient null; commands will reject with "not available".
        }
        // Register permission launcher so requestPermissions() can drive the
        // Health Connect system UI on demand. Wrapped in try/catch because
        // registerForActivityResult on a non-ComponentActivity or after the
        // host's onCreate has fully resolved can throw.
        try {
            (activity as? ComponentActivity)?.let { ca ->
                val contract = PermissionController.createRequestPermissionResultContract()
                permLauncher = ca.activityResultRegistry.register("hc_perm", contract) { granted ->
                    val invoke = pendingPermInvoke
                    pendingPermInvoke = null
                    val ret = JSObject()
                    ret.put("granted", granted.containsAll(requiredPermissions))
                    invoke?.resolve(ret)
                }
            }
        } catch (_: Throwable) {
            permLauncher = null
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
    fun hasPermissions(invoke: Invoke) {
        val client = healthClient
        if (client == null) {
            val ret = JSObject(); ret.put("granted", false); ret.put("available", false)
            invoke.resolve(ret); return
        }
        CoroutineScope(Dispatchers.IO).launch {
            try {
                val granted = client.permissionController.getGrantedPermissions()
                val ret = JSObject()
                ret.put("granted", granted.containsAll(requiredPermissions))
                ret.put("available", true)
                invoke.resolve(ret)
            } catch (e: Exception) {
                invoke.reject("Health Connect error: ${e.message}")
            }
        }
    }

    @Command
    fun requestHealthPermissions(invoke: Invoke) {
        val launcher = permLauncher
        if (launcher == null) { invoke.reject("Permission launcher not available"); return }
        if (pendingPermInvoke != null) { invoke.reject("Permission request already in progress"); return }
        pendingPermInvoke = invoke
        activity.runOnUiThread { launcher.launch(requiredPermissions) }
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
