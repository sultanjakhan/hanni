package com.sultanjakhan.hanni

import android.app.Activity
import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

@InvokeArg
class ScheduleArgs {
  var intervalMinutes: Long = 15
}

@InvokeArg
class RelayConfigArgs {
  var config: String = ""
}

// Schedules HanniHealthWorker as periodic background work. Android caps
// periodic work at a 15-minute minimum interval.
@TauriPlugin
class BackgroundSyncPlugin(private val activity: Activity) : Plugin(activity) {

  @Command
  fun scheduleBackgroundSync(invoke: Invoke) {
    val args = try { invoke.parseArgs(ScheduleArgs::class.java) } catch (_: Throwable) { ScheduleArgs() }
    val minutes = args.intervalMinutes.coerceAtLeast(15L)
    val req = PeriodicWorkRequestBuilder<HanniHealthWorker>(
      minutes, TimeUnit.MINUTES
    ).build()
    val workManager = WorkManager.getInstance(activity.applicationContext)
    workManager.enqueueUniquePeriodicWork(
        HanniHealthWorker.UNIQUE_NAME,
        // KEEP: don't restart the running worker if already scheduled.
        ExistingPeriodicWorkPolicy.KEEP,
        req
      )
    requestSleepProjection(activity.applicationContext)
    HanniLanSyncWorker.ensureScheduled(activity.applicationContext, minutes)
    HanniRawHealthSyncWorker.ensureScheduled(activity.applicationContext)
    HanniRelaySyncWorker.ensureScheduled(activity.applicationContext)
    // A failed periodic worker can retain Android's multi-hour backoff after
    // an app update fixes the cause. One revisioned, idempotent catch-up makes
    // the new Health Connect reader and LAN transport take effect immediately.
    workManager.enqueueUniqueWork(
      "${HanniHealthWorker.UNIQUE_NAME}-catchup-v10",
      ExistingWorkPolicy.KEEP,
      OneTimeWorkRequestBuilder<HanniHealthWorker>().build()
    )
    HanniRawHealthSyncWorker.enqueueCatchup(activity.applicationContext)
    HanniLanSyncWorker.enqueueCatchup(activity.applicationContext)
    HanniRelaySyncWorker.enqueueCatchup(activity.applicationContext)
    invoke.resolve(JSObject().put("scheduled", true).put("intervalMinutes", minutes))
  }


  private fun requestSleepProjection(context: Context) {
    HanniSleepProjectionWorker.ensureScheduled(context)
    HanniCatchupQueue.request(context, "${HanniSleepProjectionWorker.UNIQUE_NAME}-catchup",
      OneTimeWorkRequestBuilder<HanniSleepProjectionWorker>()
        .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build())
  }

  // Explicit pairing metadata only. Creating this local UUID does not open
  // Health Connect, request permissions or read any health records.
  @Command
  fun getRelayPairingSource(invoke: Invoke) {
    val context = activity.applicationContext
    CoroutineScope(Dispatchers.IO).launch {
      try {
        val source = RawHealthIdentity.loadOrCreate(context)
        invoke.resolve(JSObject().put("supported", true).put("source_store_id", source))
      } catch (error: kotlinx.coroutines.CancellationException) { throw error }
      catch (_: Exception) { invoke.reject("relay_pairing_source_unavailable") }
    }
  }

  // These are local Tauri plugin calls, never exported Intent/HTTP endpoints.
  // Only status crosses back to the UI; there is no read-credentials command.
  @Command
  fun setRelayConfig(invoke: Invoke) {
    val args = try { invoke.parseArgs(RelayConfigArgs::class.java) }
      catch (_: Exception) { invoke.reject("Relay configuration not accepted"); return }
    val context = activity.applicationContext
    CoroutineScope(Dispatchers.IO).launch {
      try {
        val config = RelayConfigStore.save(context, args.config)
        // Projection uses committed local rows even while cloud transport is disabled.
        if (config.hasSleepAuthority) requestSleepProjection(context)
        if (config.enabled) {
          HanniRelaySyncWorker.ensureScheduled(context)
          HanniRelaySyncWorker.enqueueCatchup(context)
        } else HanniRelaySyncWorker.cancel(context)
        invoke.resolve(JSObject().put("configured", true).put("enabled", config.enabled))
      } catch (_: Exception) { invoke.reject("Relay configuration not accepted") }
    }
  }

  @Command
  fun relaySyncStatus(invoke: Invoke) {
    val context = activity.applicationContext
    CoroutineScope(Dispatchers.IO).launch {
      val status = RelayConfigStore.status(context)
      invoke.resolve(JSObject().put("configured", status.getBoolean("configured"))
        .put("enabled", status.getBoolean("enabled")).put("status", status.getString("status")))
    }
  }

  @Command
  fun runRelaySyncOnce(invoke: Invoke) {
    requestSleepProjection(activity.applicationContext)
    HanniRawHealthSyncWorker.ensureScheduled(activity.applicationContext)
    HanniRelaySyncWorker.ensureScheduled(activity.applicationContext)
    HanniRelaySyncWorker.enqueueCatchup(activity.applicationContext)
    invoke.resolve(JSObject().put("enqueued", true))
  }

  @Command
  fun cancelBackgroundSync(invoke: Invoke) {
    val workManager = WorkManager.getInstance(activity.applicationContext)
    HanniSleepProjectionWorker.cancel(activity.applicationContext)
    HanniRawHealthSyncWorker.cancel(activity.applicationContext)
    HanniRelaySyncWorker.cancel(activity.applicationContext)
    workManager.cancelUniqueWork(HanniHealthWorker.UNIQUE_NAME)
    workManager.cancelUniqueWork(HanniLanSyncWorker.UNIQUE_NAME)
    workManager.cancelUniqueWork("${HanniLanSyncWorker.UNIQUE_NAME}-catchup")
    for (revision in 8..10) {
      workManager.cancelUniqueWork("${HanniHealthWorker.UNIQUE_NAME}-catchup-v$revision")
    }
    invoke.resolve(JSObject().put("cancelled", true))
  }

  // Diagnostic: run the worker once immediately, bypassing the 15-min
  // periodic schedule. Lets us validate the worker without waiting.
  @Command
  fun runBackgroundSyncOnce(invoke: Invoke) {
    HanniRawHealthSyncWorker.ensureScheduled(activity.applicationContext)
    HanniRawHealthSyncWorker.enqueueCatchup(activity.applicationContext)
    val req = OneTimeWorkRequestBuilder<HanniHealthWorker>().build()
    WorkManager.getInstance(activity.applicationContext).enqueue(req)
    invoke.resolve(JSObject().put("enqueued", true).put("id", req.id.toString()))
  }

  @Command
  fun backgroundSyncStatus(invoke: Invoke) {
    // Avoid blocking on the ListenableFuture (needs guava in the classpath
    // and doesn't fit a Tauri sync command anyway). Caller can rely on the
    // schedule result alone; this command just confirms the plugin is wired.
    invoke.resolve(JSObject().put("plugin", "ok"))
  }
}
