package com.sultanjakhan.hanni

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
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
            return@withContext Result.success()
        }
        try {
            val db = SQLiteDatabase.openDatabase(dbFile.absolutePath, null, SQLiteDatabase.OPEN_READWRITE)
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
            Result.success()
        } catch (e: Exception) {
            // The peer being asleep is expected. Keep the fixed 15-minute
            // cadence instead of entering WorkManager's exponential backoff.
            Log.w(TAG, "peer unavailable: ${e.message}")
            Result.success()
        }
    }

    companion object {
        const val TAG = "HanniLanSyncWorker"
        const val UNIQUE_NAME = "hanni-lan-sync"

        fun ensureScheduled(context: Context, intervalMinutes: Long = 15L) {
            val request = PeriodicWorkRequestBuilder<HanniLanSyncWorker>(
                intervalMinutes.coerceAtLeast(15L), TimeUnit.MINUTES,
            ).build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                UNIQUE_NAME, ExistingPeriodicWorkPolicy.KEEP, request,
            )
        }

        fun enqueueCatchup(context: Context) {
            WorkManager.getInstance(context).enqueueUniqueWork(
                "$UNIQUE_NAME-catchup",
                ExistingWorkPolicy.REPLACE,
                OneTimeWorkRequestBuilder<HanniLanSyncWorker>().build(),
            )
        }
    }
}
