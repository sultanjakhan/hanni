package com.sultanjakhan.hanni

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.util.Log
import androidx.work.BackoffPolicy
import androidx.work.CoroutineWorker
import androidx.work.workDataOf
import kotlinx.coroutines.CancellationException
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.TimeUnit

/** Full LAN sync independent of Health Connect and of the Tauri UI process. */
class HanniLanSyncWorker(context: Context, params: WorkerParameters) :
    CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val dbFile = File(applicationContext.filesDir.parentFile, "hanni.db")
        if (!dbFile.exists() || dbFile.length() == 0L) {
            Log.i(TAG, "skip: DB not ready")
            return@withContext Result.retry()
        }
        try {
            val db = SQLiteDatabase.openDatabase(dbFile.absolutePath, null,
                SQLiteDatabase.OPEN_READWRITE or SQLiteDatabase.ENABLE_WRITE_AHEAD_LOGGING)
            val stats = try {
                BackgroundLanSync.runConfigured(db)
            } finally {
                db.close()
            }
            if (stats == null) {
                Log.i(TAG, "skip: LAN sync disabled or not configured")
            } else {
                Log.i(TAG, "ok: sent=${stats.sent} received=${stats.received} deletes=${stats.deletes}")
            }
            Result.success(workDataOf("status" to if (stats == null) "not_configured" else "exchanged"))
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "exchange deferred: ${e.javaClass.simpleName}")
            Result.retry()
        }
    }

    companion object {
        const val TAG = "HanniLanSyncWorker"
        const val UNIQUE_NAME = "hanni-lan-sync"

        fun ensureScheduled(context: Context, intervalMinutes: Long = 15L) {
            val request = PeriodicWorkRequestBuilder<HanniLanSyncWorker>(
                intervalMinutes.coerceAtLeast(15L), TimeUnit.MINUTES,
            ).setBackoffCriteria(BackoffPolicy.LINEAR, 30, TimeUnit.SECONDS).build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                UNIQUE_NAME, ExistingPeriodicWorkPolicy.UPDATE, request,
            )
        }

        fun enqueueCatchup(context: Context) {
            HanniCatchupQueue.request(context,
                "$UNIQUE_NAME-catchup",
                OneTimeWorkRequestBuilder<HanniLanSyncWorker>()
                    .setBackoffCriteria(BackoffPolicy.LINEAR, 30, TimeUnit.SECONDS).build(),
            )
        }
    }
}
