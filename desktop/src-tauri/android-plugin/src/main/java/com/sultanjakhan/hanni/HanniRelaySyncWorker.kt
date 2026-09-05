package com.sultanjakhan.hanni

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.util.concurrent.TimeUnit

/** Shared Rust relay core, with no Activity, WebView or Tauri runtime. */
class HanniRelaySyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val config = RelayConfigStore.load(applicationContext)
            if (config == null || !config.enabled) {
                return@withContext Result.success(workDataOf("status" to "not_configured"))
            }
            val file = File(applicationContext.filesDir.parentFile, "hanni.db")
            if (!file.isFile || file.length() == 0L) return@withContext Result.retry()
            val raw = RelayNative.nativeRunOnce(file.absolutePath, config.nativeJson())
            require(raw.toByteArray(Charsets.UTF_8).size <= 4096)
            val response = JSONObject(raw)
            require(response.opt("ok") is Boolean && response.getBoolean("ok"))
            val stats = response.getJSONObject("result")
            require(stats.opt("more_pending") is Boolean)
            // Core persists not_before_epoch and honors Retry-After across every
            // caller. WorkManager retries cannot bypass the server cooldown.
            if (stats.optBoolean("deferred") || stats.optLong("checkpoint_retry_after") > 0 || !stats.isNull("error_code")) Result.retry()
            else {
                if (stats.getBoolean("more_pending")) {
                    HanniCatchupQueue.enqueue(applicationContext, "$UNIQUE_NAME-catchup", catchupRequest())
                }
                Result.success(workDataOf("status" to "exchanged"))
            }
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            Result.retry()
        } catch (_: LinkageError) {
            // No native library/ABI/symbol yet: retain the outbox for an update.
            Result.retry()
        }
    }

    companion object {
        const val UNIQUE_NAME = "hanni-relay-sync-v1"
        private val network = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()

        fun ensureScheduled(context: Context) {
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                UNIQUE_NAME, ExistingPeriodicWorkPolicy.UPDATE,
                PeriodicWorkRequestBuilder<HanniRelaySyncWorker>(15, TimeUnit.MINUTES)
                    .setConstraints(network)
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build())
        }

        fun enqueueCatchup(context: Context) {
            HanniCatchupQueue.request(context, "$UNIQUE_NAME-catchup", catchupRequest())
        }

        private fun catchupRequest() = OneTimeWorkRequestBuilder<HanniRelaySyncWorker>()
                    .setConstraints(network)
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build()

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(UNIQUE_NAME)
            WorkManager.getInstance(context).cancelUniqueWork("$UNIQUE_NAME-catchup")
        }
    }
}
