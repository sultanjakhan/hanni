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
import androidx.health.connect.client.HealthConnectFeatures
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

    private fun requiredPermissions(client: HealthConnectClient): Set<String> =
        RawHealthRecordCodec.descriptors.filter { descriptor ->
            descriptor.requiredFeature == null || client.features.getFeatureStatus(descriptor.requiredFeature) ==
                HealthConnectFeatures.FEATURE_STATUS_AVAILABLE
        }.map { it.readPermission }.toSet()

    private val backgroundPermission =
        HealthPermission.PERMISSION_READ_HEALTH_DATA_IN_BACKGROUND

    private fun backgroundAvailable(client: HealthConnectClient): Boolean =
        client.features.getFeatureStatus(
            HealthConnectFeatures.FEATURE_READ_HEALTH_DATA_IN_BACKGROUND
        ) == HealthConnectFeatures.FEATURE_STATUS_AVAILABLE

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
                    ret.put("granted", healthClient?.let { granted.containsAll(requiredPermissions(it)) } ?: false)
                    invoke?.resolve(ret)
                }
            }
        } catch (_: Throwable) {
            permLauncher = null
        }
    }

    private fun withClient(
        invoke: Invoke,
        permission: String,
        block: suspend (HealthConnectClient) -> Unit,
    ) {
        val client = healthClient
        if (client == null) { invoke.reject("Health Connect not available"); return }
        CoroutineScope(Dispatchers.IO).launch {
            try {
                val granted = client.permissionController.getGrantedPermissions()
                if (!granted.contains(permission)) {
                    invoke.reject("Health permission not granted: $permission"); return@launch
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
                ret.put("granted", healthClient?.let { granted.containsAll(requiredPermissions(it)) } ?: false)
                ret.put("available", true)
                invoke.resolve(ret)
            } catch (e: Exception) {
                invoke.reject("Health Connect error: ${e.message}")
            }
        }
    }

    @Command
    fun requestHealthPermissions(invoke: Invoke) {
        activity.runOnUiThread {
            val client = healthClient
            val launcher = permLauncher
            if (client == null) {
                invoke.reject("Health Connect not available"); return@runOnUiThread
            }
            if (launcher == null) {
                invoke.reject("Permission launcher not available"); return@runOnUiThread
            }
            if (pendingPermInvoke != null) {
                invoke.reject("Permission request already in progress"); return@runOnUiThread
            }
            try {
                val requested = requiredPermissions(client).toMutableSet()
                if (backgroundAvailable(client)) requested.add(backgroundPermission)
                if (client.features.getFeatureStatus(HealthConnectFeatures.FEATURE_READ_HEALTH_DATA_HISTORY) ==
                    HealthConnectFeatures.FEATURE_STATUS_AVAILABLE) {
                    requested.add(HealthPermission.PERMISSION_READ_HEALTH_DATA_HISTORY)
                }
                pendingPermInvoke = invoke
                launcher.launch(requested)
            } catch (e: Exception) {
                pendingPermInvoke = null
                invoke.reject("Health permission request failed: ${e.javaClass.simpleName}")
            }
        }
    }

    @Command
    fun backgroundStatus(invoke: Invoke) {
        val client = healthClient
        if (client == null) {
            val ret = JSObject(); ret.put("granted", false); ret.put("available", false)
            invoke.resolve(ret); return
        }
        CoroutineScope(Dispatchers.IO).launch {
            try {
                val granted = client.permissionController.getGrantedPermissions()
                val ret = JSObject()
                val available = backgroundAvailable(client)
                ret.put("granted", available && granted.contains(backgroundPermission))
                ret.put("available", available)
                invoke.resolve(ret)
            } catch (e: Exception) {
                invoke.reject("Health Connect error: ${e.message}")
            }
        }
    }

    @Command
    fun importRawRecords(invoke: Invoke) {
        val owner = activity as? androidx.lifecycle.LifecycleOwner
        if (owner == null || !owner.lifecycle.currentState.isAtLeast(androidx.lifecycle.Lifecycle.State.RESUMED)) {
            invoke.reject("hc_foreground_required"); return
        }
        CoroutineScope(Dispatchers.IO).launch {
            try {
                val result = RawHealthSync.runOnce(activity.applicationContext, background = false)
                val response = JSObject()
                for (key in result.keys()) response.put(key, result.get(key))
                invoke.resolve(response)
            } catch (_: Exception) {
                invoke.reject("hc_raw_import_failed")
            }
        }
    }

    @Command
    fun readSleep(invoke: Invoke) {
        // Sleep is captured losslessly by the raw importer, then projected from
        // the selected source. Before pairing, never recreate duplicate legacy
        // sleep rows or request provider access from this retired reader.
        invoke.resolve(JSObject().put("sessions", org.json.JSONArray())
            .put("skipped", "raw_sleep_projection"))
    }

    @Command
    fun readSteps(invoke: Invoke) = withClient(
        invoke, HealthPermission.getReadPermission(StepsRecord::class)
    ) { client ->
        val (start, end) = last30Days()
        val ret = JSObject()
        ret.put("days", readDailySteps(client, start, end))
        invoke.resolve(ret)
    }

    @Command
    fun readHeartRate(invoke: Invoke) = withClient(
        invoke, HealthPermission.getReadPermission(HeartRateRecord::class)
    ) { client ->
        val (start, end) = last30Days()
        val ret = JSObject()
        ret.put("samples", readHeartRateSamples(client, start, end))
        invoke.resolve(ret)
    }

    @Command
    fun readExercise(invoke: Invoke) = withClient(
        invoke, HealthPermission.getReadPermission(ExerciseSessionRecord::class)
    ) { client ->
        val (start, end) = last30Days()
        val ret = JSObject()
        ret.put("sessions", readExerciseSessions(client, start, end))
        invoke.resolve(ret)
    }
}
