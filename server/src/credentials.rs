//! Per-account LLM credentials: encryption-at-rest for BYO API keys + the settings repo.
//!
//! BYO keys are encrypted with XChaCha20-Poly1305; the AEAD key is derived once via
//! HKDF-SHA256 from a stable server secret (AINM_LLM_ENCRYPTION_KEY). Never log plaintext keys.

use crate::error::StorageError;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

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

/// One row of account_llm_settings (all fields optional).
#[derive(Clone, sqlx::FromRow)]
pub struct AccountLlmSettings {
    pub api_key_ciphertext: Option<Vec<u8>>,
    pub api_key_nonce: Option<Vec<u8>>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub monthly_token_budget: Option<i64>,
}

/// The effective LLM credential for a turn.
pub enum Credential {
    ByoKey { api_key: String, model: Option<String>, base_url: Option<String> },
    ServerDefault,
}

/// Pure: pick the effective credential. BYO if a decryptable key is present, else ServerDefault.
/// A decrypt failure logs a warning and falls back (never panics, never leaks).
pub fn resolve_credential(cipher: Option<&CredCipher>, settings: Option<&AccountLlmSettings>) -> Credential {
    let (Some(cipher), Some(s)) = (cipher, settings) else { return Credential::ServerDefault };
    let (Some(ct), Some(nonce)) = (s.api_key_ciphertext.as_ref(), s.api_key_nonce.as_ref()) else {
        return Credential::ServerDefault;
    };
    match cipher.decrypt(ct, nonce) {
        Ok(api_key) => Credential::ByoKey {
            api_key,
            model: s.model.clone(),
            base_url: s.base_url.clone(),
        },
        Err(_) => {
            tracing::warn!("[cred] stored BYO key failed to decrypt; falling back to server key");
            Credential::ServerDefault
        }
    }
}

/// Fetch an account's settings row, if present.
// consumed in C-8/C-9
pub async fn get_settings(pool: &PgPool, account_id: Uuid) -> Result<Option<AccountLlmSettings>, StorageError> {
    let row = sqlx::query_as::<_, AccountLlmSettings>(
        "SELECT api_key_ciphertext, api_key_nonce, model, base_url, monthly_token_budget \
         FROM account_llm_settings WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Upsert the BYO key (and optional model/base_url), preserving any existing budget override.
// consumed in C-8/C-9
pub async fn upsert_byo_key(
    pool: &PgPool, account_id: Uuid, ciphertext: &[u8], nonce: &[u8],
    model: Option<&str>, base_url: Option<&str>,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO account_llm_settings (account_id, api_key_ciphertext, api_key_nonce, model, base_url) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (account_id) DO UPDATE SET \
            api_key_ciphertext = EXCLUDED.api_key_ciphertext, \
            api_key_nonce = EXCLUDED.api_key_nonce, \
            model = EXCLUDED.model, base_url = EXCLUDED.base_url, updated_at = NOW()",
    )
    .bind(account_id).bind(ciphertext).bind(nonce).bind(model).bind(base_url)
    .execute(pool).await?;
    Ok(())
}

/// Clear the BYO key (NULL the cipher columns), keeping any budget override.
// consumed in C-8/C-9
pub async fn clear_byo_key(pool: &PgPool, account_id: Uuid) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE account_llm_settings \
         SET api_key_ciphertext = NULL, api_key_nonce = NULL, model = NULL, base_url = NULL, updated_at = NOW() \
         WHERE account_id = $1",
    )
    .bind(account_id).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_server_default_when_no_settings() {
        let cred = resolve_credential(None, None);
        assert!(matches!(cred, Credential::ServerDefault));
    }

    #[test]
    fn resolve_returns_byo_when_key_present_and_decrypts() {
        let cipher = CredCipher::new(b"k");
        let (ct, nonce) = cipher.encrypt("sk-user").unwrap();
        let settings = AccountLlmSettings {
            api_key_ciphertext: Some(ct),
            api_key_nonce: Some(nonce),
            model: Some("gpt-4o".into()),
            base_url: None,
            monthly_token_budget: None,
        };
        let cred = resolve_credential(Some(&cipher), Some(&settings));
        match cred {
            Credential::ByoKey { api_key, model, .. } => {
                assert_eq!(api_key, "sk-user");
                assert_eq!(model.as_deref(), Some("gpt-4o"));
            }
            _ => panic!("expected ByoKey"),
        }
    }

    #[test]
    fn resolve_falls_back_when_key_undecryptable() {
        let good = CredCipher::new(b"good");
        let bad = CredCipher::new(b"bad");
        let (ct, nonce) = good.encrypt("sk-user").unwrap();
        let settings = AccountLlmSettings {
            api_key_ciphertext: Some(ct), api_key_nonce: Some(nonce),
            model: None, base_url: None, monthly_token_budget: None,
        };
        // Wrong cipher → cannot decrypt → ServerDefault (does NOT panic / leak).
        assert!(matches!(resolve_credential(Some(&bad), Some(&settings)), Credential::ServerDefault));
    }

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
