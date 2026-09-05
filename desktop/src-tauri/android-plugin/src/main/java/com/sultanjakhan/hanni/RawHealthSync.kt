package com.sultanjakhan.hanni

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.health.connect.client.HealthConnectClient
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** One coordinator for foreground and WorkManager; HC cursors stay in SQLite. */
internal object RawHealthSync {
    private val mutex = Mutex()

    suspend fun runOnce(context: Context, background: Boolean): JSONObject = withContext(Dispatchers.IO) {
        mutex.withLock {
            // Previously committed data can be projected even if HC is unavailable/offline.
            val priorProjection = RawSleepProjectionSync.afterRawCommit(context)
            if (HealthConnectClient.getSdkStatus(context) != HealthConnectClient.SDK_AVAILABLE) {
                return@withLock JSONObject().put("available", false).put("more_pending", false)
                    .put("retry_needed", false).put("types", JSONArray()).put("projection", priorProjection)
            }
            val file = File(context.filesDir.parentFile, "hanni.db")
            if (!file.isFile || file.length() == 0L) throw RawHealthImportException("hc_database_not_ready")
            val result = SQLiteDatabase.openDatabase(file.absolutePath, null, SQLiteDatabase.OPEN_READWRITE).use { db ->
                db.rawQuery("PRAGMA journal_mode", null).use { cursor ->
                    if (!cursor.moveToFirst() || cursor.getString(0) != "wal") {
                        throw RawHealthImportException("hc_database_not_ready")
                    }
                }
                db.rawQuery("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_settings'", null).use {
                    if (!it.moveToFirst() || it.getInt(0) != 1) throw RawHealthImportException("hc_database_not_ready")
                }
                RawHealthRecordStore.initialize(db, context)
                val store = RawHealthRecordStore(db, RawHealthIdentity.loadOrCreate(context))
                RawHealthImporter(store, HealthConnectRawSource(HealthConnectClient.getOrCreate(context)))
                    .runOnce(background)
            }
            val projection = RawSleepProjectionSync.afterRawCommit(context).apply {
                // The pre-read pass may consume pending cloud data before HC returns no changes.
                put("records", optLong("records", 0L) + priorProjection.optLong("records", 0L))
            }
            if (result.modifiedRecords > 0) HanniRelaySyncWorker.enqueueCatchup(context)
            JSONObject().put("available", true).put("pages", result.pages).put("projection", projection)
                .put("modified_records", result.modifiedRecords).put("more_pending", result.morePending)
                .put("retry_needed", result.retryNeeded).put("types", JSONArray().apply {
                    for (type in result.types) put(JSONObject().put("type", type.type)
                        .put("granted", type.granted).put("feature_available", type.featureAvailable)
                        .put("background_allowed", type.backgroundAllowed).put("history_granted", type.historyGranted)
                        .put("history_status", type.historyStatus).put("history_coverage", type.historyCoverage)
                        .put("phase", type.phase).put("status", type.status).put("deletion_gap", type.deletionGap))
                })
        }
    }
}
