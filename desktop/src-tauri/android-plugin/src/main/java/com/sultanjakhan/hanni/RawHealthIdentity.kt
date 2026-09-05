package com.sultanjakhan.hanni

import android.content.Context
import android.util.AtomicFile
import java.io.File
import java.nio.CharBuffer
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import java.util.UUID

internal class RawHealthImportException(val code: String) : IllegalStateException(code)

/** Local HC datastore identity. It is deliberately outside Android backup and app_settings sync. */
internal object RawHealthIdentity {
    @Synchronized fun loadOrCreate(context: Context): String {
        val file = AtomicFile(File(context.noBackupFilesDir, "hc-source-store-v1"))
        try {
            if (file.baseFile.exists() || File(file.baseFile.path + ".bak").exists()) {
                val bytes = file.openRead().use { input ->
                    val data = ByteArray(65)
                    var count = 0
                    while (count < data.size) {
                        val size = input.read(data, count, data.size - count)
                        if (size == -1) break
                        if (size == 0) continue
                        count += size
                    }
                    require(count == 36)
                    data.copyOf(count)
                }
                val id = String(bytes, Charsets.US_ASCII)
                require(UUID.fromString(id).toString() == id)
                return id
            }
            val id = UUID.randomUUID().toString()
            var output: java.io.FileOutputStream? = null
            try {
                output = file.startWrite()
                output.write(id.toByteArray(Charsets.US_ASCII))
                file.finishWrite(output)
                output = null
            } finally {
                if (output != null) file.failWrite(output)
            }
            return id
        } catch (_: Exception) {
            // Never replace an unreadable identity: doing so would fork the source history.
            throw RawHealthImportException("hc_source_identity_unavailable")
        }
    }

    fun utf8(value: String): ByteArray = try {
        val encoded = Charsets.UTF_8.newEncoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .encode(CharBuffer.wrap(value))
        ByteArray(encoded.remaining()).also { encoded.get(it) }
    } catch (_: Exception) {
        throw RawHealthImportException("hc_invalid_text")
    }

    fun globalId(store: String, type: String, hcId: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        for (part in listOf(store, type, hcId)) {
            val bytes = utf8(part)
            digest.update(bytes.size.toString().toByteArray(Charsets.US_ASCII))
            digest.update(':'.code.toByte())
            digest.update(bytes)
        }
        return hex(digest.digest())
    }

    fun sha256(value: String): String = hex(MessageDigest.getInstance("SHA-256").digest(utf8(value)))

    private fun hex(bytes: ByteArray): String {
        val alphabet = "0123456789abcdef"
        return buildString(bytes.size * 2) {
            for (byte in bytes) {
                val value = byte.toInt() and 255
                append(alphabet[value ushr 4]); append(alphabet[value and 15])
            }
        }
    }
}
