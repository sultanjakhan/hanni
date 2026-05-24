package com.sultanjakhan.hanni

import android.app.Activity
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

@InvokeArg
class ScheduleArgs {
  var intervalMinutes: Long = 15
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
    WorkManager.getInstance(activity.applicationContext)
      .enqueueUniquePeriodicWork(
        HanniHealthWorker.UNIQUE_NAME,
        // KEEP: don't restart the running worker if already scheduled.
        ExistingPeriodicWorkPolicy.KEEP,
        req
      )
    invoke.resolve(JSObject().put("scheduled", true).put("intervalMinutes", minutes))
  }

  @Command
  fun cancelBackgroundSync(invoke: Invoke) {
    WorkManager.getInstance(activity.applicationContext)
      .cancelUniqueWork(HanniHealthWorker.UNIQUE_NAME)
    invoke.resolve(JSObject().put("cancelled", true))
  }

  // Diagnostic: run the worker once immediately, bypassing the 15-min
  // periodic schedule. Lets us validate the worker without waiting.
  @Command
  fun runBackgroundSyncOnce(invoke: Invoke) {
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
