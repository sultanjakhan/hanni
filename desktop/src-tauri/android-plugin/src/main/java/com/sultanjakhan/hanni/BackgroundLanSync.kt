package com.sultanjakhan.hanni

import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

/** Runs the same bidirectional LAN exchange as Rust while Tauri is stopped. */
internal object BackgroundLanSync {
    data class Stats(val sent: Int, val received: Int, val deletes: Int)

    fun runConfigured(db: HealthDatabase): Stats? {
        val config = readConfig(db) ?: return null
        val outbound = LanSyncDatabase.gather(db, config.second)
        val request = JSONObject().apply {
            put("key", config.first)
            put("cursors", outbound.cursors)
            put("tomb_cursor", outbound.tombCursor)
            put("rows", outbound.rows)
            put("tombs", outbound.tombs)
            put("push_only", false)
        }
        val conn = URL("http://${config.second}/lan/sync").openConnection() as HttpURLConnection
        try {
            conn.requestMethod = "POST"
            conn.setRequestProperty("Content-Type", "application/json")
            conn.connectTimeout = 8_000
            conn.readTimeout = 30_000
            conn.doOutput = true
            conn.outputStream.use { it.write(request.toString().toByteArray()) }
            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val responseText = stream?.bufferedReader()?.use { it.readText() }.orEmpty()
            if (code !in 200..299) {
                throw IllegalStateException("LAN sync HTTP $code")
            }
            val response = JSONObject(responseText)
            require(response.optJSONArray("rows") != null && response.optJSONArray("tombs") != null) {
                "Invalid LAN sync response"
            }
            val received = LanSyncDatabase.applyResponse(db, outbound, response)
            return Stats(outbound.rows.length(), received, response.optJSONArray("tombs")?.length() ?: 0)
        } finally {
            conn.disconnect()
        }
    }

    private fun readConfig(db: HealthDatabase): Pair<String, String>? {
        var peer = ""
        var key = ""
        var enabled = false
        db.rawQuery(
            "SELECT key,value FROM app_settings " +
                "WHERE key IN ('lan_sync_peer','lan_sync_key','lan_sync_enabled')",
            null,
        ).use { cursor ->
            while (cursor.moveToNext()) {
                when (cursor.getString(0)) {
                    "lan_sync_peer" -> peer = cursor.getString(1)
                    "lan_sync_key" -> key = cursor.getString(1)
                    "lan_sync_enabled" -> enabled = cursor.getString(1) == "true"
                }
            }
        }
        return if (enabled && peer.isNotBlank() && key.isNotBlank()) Pair(key, peer) else null
    }
}
