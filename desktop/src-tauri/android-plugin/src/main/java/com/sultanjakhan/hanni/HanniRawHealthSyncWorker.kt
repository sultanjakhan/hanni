package com.sultanjakhan.hanni

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import kotlinx.coroutines.CancellationException
import java.util.concurrent.TimeUnit

/** Reads every granted HC type independently of the four Calendar projections. */
class HanniRawHealthSyncWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result = try {
        val result = RawHealthSync.runOnce(applicationContext, background = true)
        if (result.getBoolean("more_pending")) {
            // Append before completing: KEEP would discard this continuation
            // while this unique worker is RUNNING. Healthy backlog is not an error.
            HanniCatchupQueue.enqueue(applicationContext, "$UNIQUE_NAME-catchup", catchupRequest())
        }
        if (result.getBoolean("retry_needed") && !result.getBoolean("more_pending")) Result.retry()
        else Result.success(workDataOf("status" to "checked", "pages" to result.optInt("pages")))
    } catch (error: CancellationException) {
        throw error
    } catch (error: Exception) {
        val code = when ((error as? RawHealthImportException)?.code) {
            "hc_database_not_ready" -> "hc_database_not_ready"
            "hc_database_page_failed" -> "hc_database_page_failed"
            "hc_permissions_timeout" -> "hc_permissions_timeout"
            "hc_permissions_unavailable" -> "hc_permissions_unavailable"
            "hc_checkpoint_unavailable" -> "hc_checkpoint_unavailable"
            else -> "hc_unclassified_retry"
        }
        android.util.Log.w("HanniWorkerDiag", code)
        Result.retry()
    }

    companion object {
        const val UNIQUE_NAME = "hanni-raw-health-sync-v1"

        fun ensureScheduled(context: Context) {
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(UNIQUE_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                PeriodicWorkRequestBuilder<HanniRawHealthSyncWorker>(15, TimeUnit.MINUTES)
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build())
        }
        private fun catchupRequest() = OneTimeWorkRequestBuilder<HanniRawHealthSyncWorker>()
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build()
        fun enqueueCatchup(context: Context) = HanniCatchupQueue.request(context, "$UNIQUE_NAME-catchup", catchupRequest())
        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(UNIQUE_NAME)
            WorkManager.getInstance(context).cancelUniqueWork("$UNIQUE_NAME-catchup")
        }
    }
}
