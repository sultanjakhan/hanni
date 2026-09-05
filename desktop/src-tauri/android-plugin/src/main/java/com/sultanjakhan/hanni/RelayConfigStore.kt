package com.sultanjakhan.hanni

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import org.json.JSONObject
import java.io.File
import java.net.URI
import java.security.KeyStore
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Never put this object, its JSON, or credentials into logs or sync tables. */
internal class RelayConfig private constructor(private val json: String, val enabled: Boolean) {
    fun nativeJson(): String = json
    val hasSleepAuthority: Boolean get() = JSONObject(json).has("sleep_source_store_id")

    fun sameIdentity(other: RelayConfig): Boolean {
        val left = JSONObject(json)
        val right = JSONObject(other.json)
        return listOf("endpoint", "device_id", "key_id", "key").all {
            left.getString(it) == right.getString(it)
        } && left.optString("sleep_source_store_id") == right.optString("sleep_source_store_id")
    }

    companion object {
        const val MAX_BYTES = 4096
        private val fields = setOf("v", "endpoint", "device_id", "key_id", "token", "key", "enabled")
        private val id = Regex("[A-Za-z0-9_-]{1,64}")
        private val secret = Regex("[A-Za-z0-9_-]{43}")

        fun parse(raw: String): RelayConfig {
            require(raw.toByteArray(Charsets.UTF_8).size <= MAX_BYTES)
            val value = JSONObject(raw)
            val supplied = value.keys().asSequence().toSet()
            require(supplied.containsAll(fields) && (supplied - fields - "sleep_source_store_id").isEmpty())
            if (value.has("sleep_source_store_id")) {
                require(value.opt("sleep_source_store_id") is String)
                val source = value.getString("sleep_source_store_id")
                require(java.util.UUID.fromString(source).toString() == source)
            }
            require(value.opt("v") == 1 && value.opt("enabled") is Boolean)
            for (field in fields - setOf("v", "enabled")) require(value.opt(field) is String)
            for (field in listOf("device_id", "key_id")) require(id.matches(value.getString(field)))
            for (field in listOf("token", "key")) {
                val encoded = value.getString(field)
                require(secret.matches(encoded))
                val decoded = Base64.getUrlDecoder().decode(encoded)
                try {
                    require(decoded.size == 32)
                    require(Base64.getUrlEncoder().withoutPadding().encodeToString(decoded) == encoded)
                } finally { decoded.fill(0) }
            }
            val endpoint = URI(value.getString("endpoint"))
            require(endpoint.scheme == "https" && !endpoint.host.isNullOrEmpty())
            require(endpoint.rawUserInfo == null && endpoint.rawQuery == null && endpoint.rawFragment == null)
            require(endpoint.rawPath.isNullOrEmpty() || endpoint.rawPath == "/")
            require(endpoint.port == -1 || endpoint.port == 443)
            value.put("endpoint", "https://${endpoint.host.lowercase()}")
            return RelayConfig(value.toString(), value.getBoolean("enabled"))
        }
    }
}

/** Small, independently testable local envelope; unrelated to the relay wire AEAD. */
internal object RelayConfigCipher {
    private val aad = "com.sultanjakhan.hanni:relay-config:v1".toByteArray(Charsets.UTF_8)

    fun encrypt(config: RelayConfig, key: SecretKey): ByteArray {
        val plaintext = config.nativeJson().toByteArray(Charsets.UTF_8)
        try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key)
            cipher.updateAAD(aad)
            check(cipher.iv.size == 12)
            return byteArrayOf(1) + cipher.iv + cipher.doFinal(plaintext)
        } finally { plaintext.fill(0) }
    }

    fun decrypt(bytes: ByteArray, key: SecretKey): RelayConfig {
        require(bytes.size in 29..(RelayConfig.MAX_BYTES + 29) && bytes[0] == 1.toByte())
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, bytes.copyOfRange(1, 13)))
        cipher.updateAAD(aad)
        val plaintext = cipher.doFinal(bytes, 13, bytes.size - 13)
        try {
            return RelayConfig.parse(plaintext.toString(Charsets.UTF_8))
        } finally { plaintext.fill(0) }
    }
}

internal object RelayConfigStore {
    private const val KEY_ALIAS = "hanni-relay-config-v1"
    private fun file(context: Context) = AtomicFile(File(context.noBackupFilesDir, "relay-config-v1.enc"))

    private fun key(create: Boolean): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        // Never replace a missing key while an encrypted configuration exists.
        check(create) { "Relay configuration unavailable" }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(KeyGenParameterSpec.Builder(KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setKeySize(256)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setRandomizedEncryptionRequired(true)
            .setUserAuthenticationRequired(false)
            .build())
        return generator.generateKey()
    }

    @Synchronized
    fun load(context: Context): RelayConfig? {
        val saved = file(context)
        if (!saved.baseFile.exists() && !File(saved.baseFile.path + ".bak").exists()) return null
        val bytes = saved.openRead().use { input ->
            val bounded = ByteArray(RelayConfig.MAX_BYTES + 30)
            var count = 0
            while (count < bounded.size) {
                val n = input.read(bounded, count, bounded.size - count)
                if (n < 0) break
                count += n
            }
            require(count <= RelayConfig.MAX_BYTES + 29)
            bounded.copyOf(count)
        }
        return RelayConfigCipher.decrypt(bytes, key(create = false))
    }

    @Synchronized
    fun save(context: Context, raw: String): RelayConfig {
        val config = RelayConfig.parse(raw)
        val previous = load(context)
        // A relay/key/device change needs an explicit reconciliation protocol.
        // Enabling/disabling and token rotation are safe without deleting history.
        require(previous == null || previous.sameIdentity(config)) { "Relay pairing change requires reconciliation" }
        val encrypted = RelayConfigCipher.encrypt(config, key(create = previous == null))
        val saved = file(context)
        val output = saved.startWrite()
        try {
            output.write(encrypted)
            saved.finishWrite(output)
        } catch (error: Exception) {
            saved.failWrite(output)
            throw error
        }
        return config
    }

    fun status(context: Context): JSONObject = try {
        val config = load(context)
        JSONObject().put("configured", config != null).put("enabled", config?.enabled ?: false)
            .put("status", if (config == null) "not_configured" else if (config.enabled) "configured" else "disabled")
    } catch (_: Exception) {
        JSONObject().put("configured", true).put("enabled", false).put("status", "configuration_unavailable")
    }
}
