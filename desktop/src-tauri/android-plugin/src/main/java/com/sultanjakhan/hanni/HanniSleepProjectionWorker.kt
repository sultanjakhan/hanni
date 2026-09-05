package com.sultanjakhan.hanni

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.util.concurrent.TimeUnit

/** Independent of provider availability and network: materializes already committed raw rows. */
internal object RawSleepProjectionSync {
    suspend fun runOnce(context: Context): JSONObject = withContext(Dispatchers.IO) {
        val config = RelayConfigStore.load(context)
            ?: return@withContext JSONObject().put("status", "authority_not_configured").put("more_pending", false).put("retry_needed", false)
        val file = File(context.filesDir.parentFile, "hanni.db")
        if (!file.isFile || file.length() == 0L) throw IllegalStateException("projection_database_not_ready")
        val reply = RelayNative.nativeProjectOnce(file.absolutePath, config.nativeJson())
        require(reply.toByteArray(Charsets.UTF_8).size <= 4096)
        val value = JSONObject(reply)
        require(value.opt("ok") is Boolean && value.getBoolean("ok"))
        value.getJSONObject("result").also {
            require(it.opt("more_pending") is Boolean && it.opt("status") is String && it.opt("retry_needed") is Boolean)
        }
    }

    suspend fun afterRawCommit(context: Context): JSONObject {
        return try {
            HanniSleepProjectionWorker.ensureScheduled(context)
            runOnce(context).also { HanniSleepProjectionWorker.scheduleContinuation(context, it) }
        } catch (error: CancellationException) { throw error }
        catch (_: Exception) {
            scheduleSafely(context)
            JSONObject().put("status", "projection_deferred").put("more_pending", false).put("retry_needed", true).put("error_code", "projection_deferred")
        } catch (_: LinkageError) {
            scheduleSafely(context)
            JSONObject().put("status", "projection_deferred").put("more_pending", false).put("retry_needed", true).put("error_code", "projection_deferred")
        }
    }

    private suspend fun scheduleSafely(context: Context) {
        try { HanniSleepProjectionWorker.enqueueRetry(context, 30) }
        catch (error: CancellationException) { throw error }
        catch (_: Exception) { /* Durable projection revision mismatch remains pending. */ }
    }
}

class HanniSleepProjectionWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result = try {
        val result = RawSleepProjectionSync.runOnce(applicationContext)
        scheduleContinuation(applicationContext, result)
        Result.success()
    } catch (error: CancellationException) { throw error }
    catch (_: Exception) { Result.retry() }
    catch (_: LinkageError) { Result.retry() }

    companion object {
        const val UNIQUE_NAME = "hanni-sleep-projection-v1"
        fun ensureScheduled(context: Context) {
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(UNIQUE_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                PeriodicWorkRequestBuilder<HanniSleepProjectionWorker>(15, TimeUnit.MINUTES)
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build())
        }
        suspend fun enqueueCatchup(context: Context) {
            HanniCatchupQueue.enqueue(context, "$UNIQUE_NAME-catchup",
                OneTimeWorkRequestBuilder<HanniSleepProjectionWorker>()
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build())
        }
        // A retrying projection must not hold the healthy catchup chain behind
        // WorkManager's backoff. The Rust journal remains the authoritative due time.
        internal suspend fun scheduleContinuation(context: Context, result: JSONObject) {
            if (result.getBoolean("more_pending")) enqueueCatchup(context)
            if (result.getBoolean("retry_needed")) enqueueRetry(context, result.optLong("retry_after_seconds", 30))
        }
        internal suspend fun enqueueRetry(context: Context, seconds: Long) {
            HanniCatchupQueue.enqueue(context, "$UNIQUE_NAME-retry",
                OneTimeWorkRequestBuilder<HanniSleepProjectionWorker>()
                    .setInitialDelay(seconds.coerceIn(30, 3600), TimeUnit.SECONDS)
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build())
        }
        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(UNIQUE_NAME)
            WorkManager.getInstance(context).cancelUniqueWork("$UNIQUE_NAME-catchup")
            WorkManager.getInstance(context).cancelUniqueWork("$UNIQUE_NAME-retry")
        }
    }
}
