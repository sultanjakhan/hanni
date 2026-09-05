package com.sultanjakhan.hanni

import android.content.Context
import androidx.concurrent.futures.CallbackToFutureAdapter
import androidx.work.Configuration
import androidx.work.Constraints
import androidx.work.ExistingWorkPolicy
import androidx.work.ListenableWorker
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequest
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.testing.SynchronousExecutor
import androidx.work.testing.WorkManagerTestInitHelper
import com.google.common.util.concurrent.ListenableFuture
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.SQLiteMode

/** Canonical queue plus real WorkManager 2.9.1 DB/dependency transitions. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
@SQLiteMode(SQLiteMode.Mode.NATIVE)
class HanniCatchupQueueTest {
    private lateinit var context: Context
    private lateinit var manager: WorkManager
    private lateinit var name: String

    class GateWorker(context: Context, params: WorkerParameters) : ListenableWorker(context, params) {
        override fun startWork(): ListenableFuture<Result> = CallbackToFutureAdapter.getFuture { completer ->
            observed[id] = committedVersion.get()
            gates[id] = completer
            "synthetic-catchup-gate"
        }
    }

    @Before fun setup() {
        gates.clear()
        observed.clear()
        committedVersion.set(0)
        context = RuntimeEnvironment.getApplication()
        name = "synthetic-catchup-${UUID.randomUUID()}"
        val configuration = Configuration.Builder()
            .setExecutor(SynchronousExecutor())
            .setTaskExecutor(SynchronousExecutor())
            .setMinimumLoggingLevel(android.util.Log.ERROR)
            .build()
        WorkManagerTestInitHelper.initializeTestWorkManager(context, configuration)
        manager = WorkManager.getInstance(context)
    }

    @After fun cleanup() {
        manager.cancelAllWork().result.get(5, TimeUnit.SECONDS)
        WorkManagerTestInitHelper.closeWorkDatabase()
        gates.clear()
        observed.clear()
    }

    private fun work(offline: Boolean = false): OneTimeWorkRequest =
        OneTimeWorkRequestBuilder<GateWorker>().apply {
            if (offline) setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
        }.build()

    private fun state(id: UUID): WorkInfo.State =
        manager.getWorkInfoById(id).get(5, TimeUnit.SECONDS).state

    private fun chain(): List<WorkInfo> =
        manager.getWorkInfosForUniqueWork(name).get(5, TimeUnit.SECONDS)

    private fun complete(id: UUID) {
        assertTrue("The real WorkManager must have started this worker", gates.remove(id)?.set(ListenableWorker.Result.success()) == true)
    }

    @Test fun committedWakeDuringRunningWorkerCreatesExactlyOneSuccessor() = runBlocking {
        committedVersion.set(1)
        val first = work()
        HanniCatchupQueue.enqueue(context, name, first)
        assertEquals(WorkInfo.State.RUNNING, state(first.id))
        assertEquals(1, observed[first.id])

        // The first worker already read its source before this producer commit.
        committedVersion.set(2)
        val successor = work()
        HanniCatchupQueue.enqueue(context, name, successor)
        assertEquals(WorkInfo.State.BLOCKED, state(successor.id))
        assertEquals(2, chain().size)
        assertFalse(observed.containsKey(successor.id))

        complete(first.id)
        assertEquals(WorkInfo.State.SUCCEEDED, state(first.id))
        assertEquals(WorkInfo.State.RUNNING, state(successor.id))
        assertEquals(2, observed[successor.id])
        complete(successor.id)
        assertEquals(WorkInfo.State.SUCCEEDED, state(successor.id))
    }

    @Test fun concurrentOfflineWakesCoalesceToOneDurablePendingRequest() = runBlocking {
        val pending = work(offline = true)
        HanniCatchupQueue.enqueue(context, name, pending)
        assertEquals(WorkInfo.State.ENQUEUED, state(pending.id))
        committedVersion.set(100)
        withTimeout(10_000) {
            coroutineScope {
                repeat(100) { launch(Dispatchers.Default) { HanniCatchupQueue.enqueue(context, name, work(offline = true)) } }
            }
        }
        val queued = chain()
        assertEquals(listOf(pending.id), queued.map { it.id })
        assertEquals(WorkInfo.State.ENQUEUED, queued.single().state)
        assertTrue(observed.isEmpty())

        WorkManagerTestInitHelper.getTestDriver(context)!!.setAllConstraintsMet(pending.id)
        assertEquals(WorkInfo.State.RUNNING, state(pending.id))
        assertEquals(100, observed[pending.id])
        complete(pending.id)
        assertEquals(WorkInfo.State.SUCCEEDED, state(pending.id))
    }

    @Test fun anExistingBlockedSuccessorIsPreservedAcrossManyMoreWakes() = runBlocking {
        val first = work()
        HanniCatchupQueue.enqueue(context, name, first)
        assertEquals(WorkInfo.State.RUNNING, state(first.id))
        val pending = work()
        manager.enqueueUniqueWork(name, ExistingWorkPolicy.APPEND_OR_REPLACE, pending)
            .result.get(5, TimeUnit.SECONDS)
        assertEquals(WorkInfo.State.BLOCKED, state(pending.id))

        committedVersion.set(50)
        withTimeout(10_000) {
            coroutineScope {
                repeat(50) { launch(Dispatchers.Default) { HanniCatchupQueue.enqueue(context, name, work()) } }
            }
        }
        assertEquals(setOf(first.id, pending.id), chain().map { it.id }.toSet())
        assertEquals(WorkInfo.State.BLOCKED, state(pending.id))
        complete(first.id)
        assertEquals(WorkInfo.State.RUNNING, state(pending.id))
        assertEquals(50, observed[pending.id])
        complete(pending.id)
        assertTrue(chain().all { it.state == WorkInfo.State.SUCCEEDED })
    }

    companion object {
        private val gates = ConcurrentHashMap<UUID, CallbackToFutureAdapter.Completer<ListenableWorker.Result>>()
        private val observed = ConcurrentHashMap<UUID, Int>()
        private val committedVersion = AtomicInteger()
    }
}
