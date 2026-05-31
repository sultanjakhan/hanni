// sync_crypto.rs — E2E encryption for owner-sync change-sets (Tier 3).
//
// The free sync store (a private GitHub repo) only ever sees ciphertext.
// Both devices share a 32-byte `device_key` (provisioned over LAN-pairing,
// stored in app_settings). Each change-set blob is sealed with
// XChaCha20-Poly1305: a random 24-byte nonce, and AAD = the blob's repo
// path so a ciphertext can't be silently moved between document slots.
// File names are HMAC-SHA256 of the logical doc key, so table names and row
// ids never appear in plaintext in the store.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

const NONCE_LEN: usize = 24;
type HmacSha256 = Hmac<Sha256>;

/// Seal `plaintext` with the shared device key. `aad` binds the ciphertext to
/// its slot (we pass the repo-relative file path). Output layout:
/// `nonce(24) || ciphertext || tag(16)`.
pub fn seal(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "sync_crypto: bad key length".to_string())?;
    let mut nonce = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plaintext, aad })
        .map_err(|_| "sync_crypto: encrypt failed".to_string())?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Reverse of [`seal`]. `aad` must equal what was used to seal (the file path),
/// or AEAD verification fails. Returns the plaintext bytes.
pub fn open(key: &[u8; 32], aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < NONCE_LEN {
        return Err("sync_crypto: blob too short".to_string());
    }
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "sync_crypto: bad key length".to_string())?;
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
        .map_err(|_| "sync_crypto: decrypt/verify failed".to_string())
}

/// Deterministic, opaque file name for a logical doc key (e.g. "row:recipes_5"
/// or "tomb:recipes_5"). HMAC-SHA256(device_key, label) as hex — hides table
/// names and row ids from the store. Same (key, label) → same name, so a row's
/// blob overwrites in place on update.
pub fn doc_name(key: &[u8; 32], label: &str) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .expect("HMAC accepts a key of any length");
    mac.update(label.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let key = [7u8; 32];
        let aad = b"deviceA/abc.bin";
        let pt = br#"{"id":5,"name":"x","_updated_at":"2026-05-31T00:00:00Z"}"#;
        let blob = seal(&key, aad, pt).unwrap();
        assert_eq!(open(&key, aad, &blob).unwrap(), pt);
    }

    #[test]
    fn wrong_aad_fails() {
        let key = [7u8; 32];
        let blob = seal(&key, b"slotA", b"hi").unwrap();
        assert!(open(&key, b"slotB", &blob).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let blob = seal(&[1u8; 32], b"a", b"hi").unwrap();
        assert!(open(&[2u8; 32], b"a", &blob).is_err());
    }

    #[test]
    fn tamper_fails() {
        let key = [5u8; 32];
        let mut blob = seal(&key, b"x", b"hello").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01; // flip a tag bit
        assert!(open(&key, b"x", &blob).is_err());
    }

    #[test]
    fn nonce_is_random_per_seal() {
        let key = [9u8; 32];
        let a = seal(&key, b"x", b"same").unwrap();
        let b = seal(&key, b"x", b"same").unwrap();
        assert_ne!(a, b); // different nonce → different blob for identical input
    }

    #[test]
    fn doc_name_deterministic_and_opaque() {
        let key = [3u8; 32];
        let n1 = doc_name(&key, "row:recipes_5");
        assert_eq!(n1, doc_name(&key, "row:recipes_5")); // deterministic
        assert_eq!(n1.len(), 64); // sha256 hex
        assert!(!n1.contains("recipes")); // table name hidden
        assert_ne!(n1, doc_name(&key, "row:recipes_6")); // distinct per id
        assert_ne!(n1, doc_name(&[4u8; 32], "row:recipes_5")); // key-scoped
    }
}
