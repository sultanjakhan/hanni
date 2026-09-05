package com.sultanjakhan.hanni

import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.Base64
import javax.crypto.KeyGenerator

/** Synthetic keys only. This does not emulate or prove Android Keystore. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class RelayConfigTest {
    private fun config() = JSONObject().put("v", 1).put("enabled", true)
        .put("endpoint", "https://relay.example/").put("device_id", "synthetic-phone")
        .put("key_id", "synthetic-v1")
        .put("token", Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(32) { 1 }))
        .put("key", Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(32) { 2 }))

    private fun rejected(value: JSONObject) {
        try { RelayConfig.parse(value.toString()); fail("Invalid configuration accepted") }
        catch (_: IllegalArgumentException) { }
    }

    @Test fun strictConfigurationAndSafeIdentityChanges() {
        val initial = RelayConfig.parse(config().toString())
        assertTrue(initial.enabled)
        assertEquals("https://relay.example", JSONObject(initial.nativeJson()).getString("endpoint"))
        assertTrue(initial.sameIdentity(RelayConfig.parse(config().put("enabled", false)
            .put("token", Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(32) { 3 })).toString())))
        assertFalse(initial.sameIdentity(RelayConfig.parse(config().put("key_id", "different").toString())))
        for (url in listOf("http://relay.example", "https://user@relay.example", "https://relay.example/path",
            "https://relay.example/?token=forbidden", "https://relay.example/#fragment", "https://relay.example:8443")) {
            rejected(config().put("endpoint", url))
        }
        rejected(config().put("enabled", "true"))
        rejected(config().put("device_id", "../private"))
        rejected(config().put("token", "x".repeat(43))) // Non-canonical unused base64 bits.
        rejected(config().put("extra", "forbidden"))
    }

    @Test fun ciphertextRoundTripAndTampering() {
        val key = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
        val initial = RelayConfig.parse(config().toString())
        val first = RelayConfigCipher.encrypt(initial, key)
        val second = RelayConfigCipher.encrypt(initial, key)
        assertFalse(first.contentEquals(second))
        assertTrue(initial.sameIdentity(RelayConfigCipher.decrypt(first, key)))
        assertFalse(first.toString(Charsets.UTF_8).contains("relay.example"))
        for (position in listOf(0, 1, first.lastIndex)) {
            val changed = first.copyOf().also { it[position] = (it[position].toInt() xor 1).toByte() }
            try { RelayConfigCipher.decrypt(changed, key); fail("Tampered configuration accepted") }
            catch (_: Exception) { }
        }
        val other = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
        try { RelayConfigCipher.decrypt(first, other); fail("Wrong key accepted") }
        catch (_: Exception) { }
    }
}
