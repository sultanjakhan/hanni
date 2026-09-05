package com.sultanjakhan.hanni

import android.content.Context
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequest
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.await
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeout
import java.util.concurrent.ConcurrentHashMap

/** At most one waiting successor per chain; a RUNNING worker cannot consume a new wake. */
internal object HanniCatchupQueue {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val locks = ConcurrentHashMap<String, Mutex>()

    suspend fun enqueue(context: Context, name: String, request: OneTimeWorkRequest) {
        locks.getOrPut(name) { Mutex() }.withLock {
            withTimeout(10_000) {
                val manager = WorkManager.getInstance(context)
                val work = manager.getWorkInfosForUniqueWorkFlow(name).first()
                // If the pending job starts after this query, its source read
                // still follows the producer's already-committed change.
                if (work.any { it.state == WorkInfo.State.ENQUEUED || it.state == WorkInfo.State.BLOCKED }) {
                    return@withTimeout
                }
                manager.enqueueUniqueWork(name, ExistingWorkPolicy.APPEND_OR_REPLACE, request).await()
            }
        }
    }

    fun request(context: Context, name: String, request: OneTimeWorkRequest) {
        val appContext = context.applicationContext
        scope.launch {
            try { enqueue(appContext, name, request) }
            catch (error: CancellationException) { throw error }
            catch (_: Exception) {
                // The durable source journal and periodic worker remain the fallback.
                android.util.Log.w("HanniCatchupQueue", "catchup_schedule_failed")
            }
        }
    }
}
