package com.sultanjakhan.hanni

import android.content.Context
import androidx.concurrent.futures.CallbackToFutureAdapter
import androidx.work.Configuration
import androidx.work.ListenableWorker
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.WorkerFactory
import androidx.work.WorkerParameters
import androidx.work.testing.SynchronousExecutor
import androidx.work.testing.WorkManagerTestInitHelper
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.SQLiteMode
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit

/** Real WorkManager dependencies/timers; native projection itself is tested in Rust. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@SQLiteMode(SQLiteMode.Mode.NATIVE)
class HanniSleepProjectionSchedulingTest {
    private lateinit var context: Context
    private lateinit var manager: WorkManager
    class GateWorker(context: Context, params: WorkerParameters) : ListenableWorker(context, params) {
        override fun startWork(): ListenableFuture<Result> = CallbackToFutureAdapter.getFuture { completer ->
            gates[id] = completer
            "synthetic-projection-gate"
        }
    }
    @Before fun setup() {
        gates.clear()
        context=RuntimeEnvironment.getApplication()
        val factory=object: WorkerFactory() {
            override fun createWorker(appContext: Context, workerClassName: String, workerParameters: WorkerParameters): ListenableWorker? =
                if (workerClassName == HanniSleepProjectionWorker::class.java.name) GateWorker(appContext,workerParameters) else null
        }
        WorkManagerTestInitHelper.initializeTestWorkManager(context,Configuration.Builder()
            .setExecutor(SynchronousExecutor()).setTaskExecutor(SynchronousExecutor())
            .setWorkerFactory(factory).setMinimumLoggingLevel(android.util.Log.ERROR).build())
        manager=WorkManager.getInstance(context)
    }
    @After fun cleanup() {
        manager.cancelAllWork().result.get(5,TimeUnit.SECONDS)
        WorkManagerTestInitHelper.closeWorkDatabase()
        gates.clear()
    }
    private fun chain(suffix: String): List<WorkInfo> = manager.getWorkInfosForUniqueWork("${HanniSleepProjectionWorker.UNIQUE_NAME}-$suffix").get(5,TimeUnit.SECONDS)
    private fun result(pending: Boolean,retry: Boolean,delay: Long=30) = JSONObject()
        .put("more_pending",pending).put("retry_needed",retry).put("retry_after_seconds",delay)

    @Test fun failedRowCooldownIsCoalescedAndDoesNotBlockHealthyCatchup() = runBlocking {
        repeat(20) { HanniSleepProjectionWorker.scheduleContinuation(context,result(false,true)) }
        val retry=chain("retry").single()
        assertEquals(WorkInfo.State.ENQUEUED,retry.state)
        assertTrue(gates.isEmpty())
        assertTrue(chain("catchup").isEmpty())
        HanniSleepProjectionWorker.scheduleContinuation(context,result(true,true))
        val healthy=chain("catchup").single()
        assertEquals(WorkInfo.State.RUNNING,healthy.state)
        assertTrue(gates.containsKey(healthy.id))
        assertEquals(retry.id,chain("retry").single().id)
        assertEquals(WorkInfo.State.ENQUEUED,chain("retry").single().state)
        WorkManagerTestInitHelper.getTestDriver(context)!!.setInitialDelayMet(retry.id)
        assertEquals(WorkInfo.State.RUNNING,chain("retry").single().state)
        assertTrue(gates.containsKey(retry.id))
    }

    @Test fun readyStatusDoesNotScheduleAndZeroDelayStillWaitsBeforeRetry() = runBlocking {
        HanniSleepProjectionWorker.scheduleContinuation(context,result(false,false))
        assertTrue(chain("retry").isEmpty())
        assertTrue(chain("catchup").isEmpty())
        HanniSleepProjectionWorker.scheduleContinuation(context,result(false,true,0))
        assertEquals(WorkInfo.State.ENQUEUED,chain("retry").single().state)
        assertTrue(gates.isEmpty())
        HanniSleepProjectionWorker.cancel(context)
        assertEquals(WorkInfo.State.CANCELLED,chain("retry").single().state)
    }
    companion object {
        private val gates=ConcurrentHashMap<UUID,CallbackToFutureAdapter.Completer<ListenableWorker.Result>>()
    }
}
