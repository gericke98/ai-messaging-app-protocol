//! Per-account LLM credentials: encryption-at-rest for BYO API keys + the settings repo.
//!
//! BYO keys are encrypted with XChaCha20-Poly1305; the AEAD key is derived once via
//! HKDF-SHA256 from a stable server secret (AINM_LLM_ENCRYPTION_KEY). Never log plaintext keys.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

/// Encrypts/decrypts BYO API keys. Built once at startup from the server master secret.
pub struct CredCipher {
    cipher: XChaCha20Poly1305,
}

impl CredCipher {
    /// Derive the AEAD key from an arbitrary-length master secret via HKDF-SHA256.
    pub fn new(master_secret: &[u8]) -> Self {
        let hk = Hkdf::<Sha256>::new(Some(b"ainm-llm-cred-v1"), master_secret);
        let mut key = [0u8; 32];
        hk.expand(b"cred-key", &mut key).expect("hkdf expand");
        Self { cipher: XChaCha20Poly1305::new((&key).into()) }
    }

    /// Encrypt plaintext → (ciphertext, 24-byte nonce). Fresh random nonce per call.
    pub fn encrypt(&self, plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
        let mut nonce = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ct = self
            .cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|e| format!("encrypt failed: {e}"))?;
        Ok((ct, nonce.to_vec()))
    }

    /// Decrypt (ciphertext, nonce) → plaintext. Errs on wrong key / tampering.
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<String, String> {
        if nonce.len() != 24 {
            return Err("bad nonce length".into());
        }
        let pt = self
            .cipher
            .decrypt(XNonce::from_slice(nonce), ciphertext)
            .map_err(|_| "decrypt failed".to_string())?;
        String::from_utf8(pt).map_err(|_| "decrypted bytes not utf8".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cipher_roundtrips() {
        let c = CredCipher::new(b"a-32-byte-or-any-length-master-key");
        let (ct, nonce) = c.encrypt("sk-secret-api-key").unwrap();
        assert_ne!(ct, b"sk-secret-api-key"); // ciphertext is not the plaintext
        let pt = c.decrypt(&ct, &nonce).unwrap();
        assert_eq!(pt, "sk-secret-api-key");
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let c1 = CredCipher::new(b"master-key-one");
        let c2 = CredCipher::new(b"master-key-two");
        let (ct, nonce) = c1.encrypt("sk-secret").unwrap();
        assert!(c2.decrypt(&ct, &nonce).is_err());
    }

    #[test]
    fn each_encrypt_uses_fresh_nonce() {
        let c = CredCipher::new(b"k");
        let (_, n1) = c.encrypt("x").unwrap();
        let (_, n2) = c.encrypt("x").unwrap();
        assert_ne!(n1, n2);
    }
}
