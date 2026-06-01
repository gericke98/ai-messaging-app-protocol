//! Zero-knowledge key escrow: RFC 9497 OPRF(ristretto255, SHA-512) + Argon2id + HKDF + XChaCha20Poly1305.
//!
//! The server never learns the PIN or the wrapping key — only a serialized OPRF server state
//! (the per-account secret), the ciphertext, and a guess counter. See spec §6.

use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use voprf::{BlindedElement, EvaluationElement, OprfClient, OprfServer, Ristretto255};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub salt: String, // base64url
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    fn fresh() -> Self {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        Self {
            salt: URL_SAFE_NO_PAD.encode(salt),
            m_cost: 19_456,
            t_cost: 2,
            p_cost: 1,
        }
    }
}

/// Argon2id(PIN, salt) -> 32-byte OPRF input.
fn pin_to_input(pin: &str, kdf: &KdfParams) -> Result<[u8; 32], String> {
    let salt = URL_SAFE_NO_PAD.decode(&kdf.salt).map_err(|e| e.to_string())?;
    let params = argon2::Params::new(kdf.m_cost, kdf.t_cost, kdf.p_cost, Some(32))
        .map_err(|e| e.to_string())?;
    let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut out = [0u8; 32];
    argon
        .hash_password_into(pin.as_bytes(), &salt, &mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// HKDF-SHA256(oprf_output) -> 32-byte wrapping key.
fn wrapping_key(oprf_output: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"ainm-escrow-v1"), oprf_output);
    let mut key = [0u8; 32];
    hk.expand(b"wrapping-key", &mut key).expect("hkdf expand");
    key
}

pub struct Enrolled {
    pub oprf_key: Vec<u8>,
    pub ciphertext: Vec<u8>, // nonce(24) || aead ciphertext
    pub kdf_params: KdfParams,
}

/// Client-side enrollment: derive the wrapping key via a local OPRF round, seal the secret.
pub fn enroll(secret: &[u8; 32], pin: &str) -> Result<Enrolled, String> {
    let kdf = KdfParams::fresh();
    let input = pin_to_input(pin, &kdf)?;

    let server =
        OprfServer::<Ristretto255>::new(&mut rand::thread_rng()).map_err(|e| e.to_string())?;
    let blind = OprfClient::<Ristretto255>::blind(&input, &mut rand::thread_rng())
        .map_err(|e| e.to_string())?;
    let eval = server.blind_evaluate(&blind.message);
    let output = blind.state.finalize(&input, &eval).map_err(|e| e.to_string())?;

    let wk = wrapping_key(&output);
    let cipher = XChaCha20Poly1305::new((&wk).into());
    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, secret.as_ref())
        .map_err(|e| e.to_string())?;

    let mut ciphertext = nonce_bytes.to_vec();
    ciphertext.extend_from_slice(&ct);

    Ok(Enrolled {
        oprf_key: server.serialize().to_vec(),
        ciphertext,
        kdf_params: kdf,
    })
}

pub struct BlindState {
    pub blinded_b64: String,
    client: OprfClient<Ristretto255>,
}

/// Client step 1 of recovery: blind the PIN-derived input.
pub fn client_blind(pin: &str, kdf: &KdfParams) -> Result<BlindState, String> {
    let input = pin_to_input(pin, kdf)?;
    let blind = OprfClient::<Ristretto255>::blind(&input, &mut rand::thread_rng())
        .map_err(|e| e.to_string())?;
    Ok(BlindState {
        blinded_b64: URL_SAFE_NO_PAD.encode(blind.message.serialize()),
        client: blind.state,
    })
}

/// Server step: evaluate a blinded element with the stored OPRF key. Returns base64url evaluated point.
pub fn server_evaluate(oprf_key: &[u8], blinded_b64: &str) -> Result<String, String> {
    let server = OprfServer::<Ristretto255>::deserialize(oprf_key).map_err(|e| e.to_string())?;
    let blinded_bytes = URL_SAFE_NO_PAD.decode(blinded_b64).map_err(|e| e.to_string())?;
    let blinded =
        BlindedElement::<Ristretto255>::deserialize(&blinded_bytes).map_err(|e| e.to_string())?;
    let eval = server.blind_evaluate(&blinded);
    Ok(URL_SAFE_NO_PAD.encode(eval.serialize()))
}

/// Client step 2 of recovery: finalize the OPRF, derive the wrapping key, open the ciphertext.
pub fn client_finalize_and_open(
    pin: &str,
    kdf: &KdfParams,
    blind: &BlindState,
    evaluated_b64: &str,
    ciphertext: &[u8],
) -> Result<[u8; 32], String> {
    let input = pin_to_input(pin, kdf)?;
    let eval_bytes = URL_SAFE_NO_PAD
        .decode(evaluated_b64)
        .map_err(|e| e.to_string())?;
    let eval =
        EvaluationElement::<Ristretto255>::deserialize(&eval_bytes).map_err(|e| e.to_string())?;
    let output = blind
        .client
        .clone()
        .finalize(&input, &eval)
        .map_err(|e| e.to_string())?;
    let wk = wrapping_key(&output);

    if ciphertext.len() < 24 {
        return Err("ciphertext too short".into());
    }
    let (nonce_bytes, ct) = ciphertext.split_at(24);
    let cipher = XChaCha20Poly1305::new((&wk).into());
    let plain = cipher
        .decrypt(XNonce::from_slice(nonce_bytes), ct)
        .map_err(|_| "decryption failed (wrong PIN)".to_string())?;
    if plain.len() != 32 {
        return Err("unexpected plaintext length".into());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&plain);
    Ok(out)
}

use crate::error::StorageError;
use sqlx::PgPool;

/// Finalize enrollment: persist ciphertext + kdf_params + oprf server state, reset counters.
pub async fn store_escrow(
    pool: &PgPool,
    account_id: uuid::Uuid,
    ciphertext: &[u8],
    kdf_params: &KdfParams,
    oprf_key: &[u8],
) -> Result<(), StorageError> {
    let kdf_json =
        serde_json::to_value(kdf_params).map_err(|e| StorageError::Duplicate(e.to_string()))?;
    sqlx::query(
        "INSERT INTO key_escrow (account_id, ciphertext, kdf_params, oprf_key) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (account_id) DO UPDATE SET \
           ciphertext = $2, kdf_params = $3, oprf_key = $4, eval_attempts = 0, destroyed = false, updated_at = NOW()",
    )
    .bind(account_id)
    .bind(ciphertext)
    .bind(kdf_json)
    .bind(oprf_key)
    .execute(pool)
    .await?;
    Ok(())
}

pub struct EscrowFetch {
    pub evaluated_b64: String,
    pub ciphertext: Vec<u8>,
    pub kdf_params: KdfParams,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FetchError {
    Destroyed,
    NotFound,
    Crypto(String),
    Db(String),
}

/// One recovery guess = one server OPRF evaluation. Increments eval_attempts; on reaching
/// max_attempts, deletes oprf_key + sets destroyed and returns Destroyed (fail closed).
pub async fn fetch_and_evaluate(
    pool: &PgPool,
    account_id: uuid::Uuid,
    blinded_b64: &str,
) -> Result<EscrowFetch, FetchError> {
    // Atomic: increment the attempt counter and, if this guess trips the cap, destroy the
    // OPRF key in the SAME statement (closes the read-then-update TOCTOU window). The row lock
    // taken by UPDATE serializes concurrent guesses, so eval_attempts is re-read per guess.
    // Only rows that are still live (not destroyed, key present) are updated.
    let updated: Option<(Option<Vec<u8>>, Vec<u8>, serde_json::Value)> = sqlx::query_as(
        "UPDATE key_escrow \
         SET eval_attempts = eval_attempts + 1, \
             oprf_key  = CASE WHEN eval_attempts + 1 >= max_attempts THEN NULL ELSE oprf_key END, \
             destroyed = CASE WHEN eval_attempts + 1 >= max_attempts THEN true ELSE destroyed END, \
             updated_at = NOW() \
         WHERE account_id = $1 AND destroyed = false AND oprf_key IS NOT NULL \
         RETURNING oprf_key, ciphertext, kdf_params",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FetchError::Db(e.to_string()))?;

    let Some((oprf_key_opt, ciphertext, kdf_json)) = updated else {
        // No live row was updated: either there is no escrow at all (NotFound) or it was
        // already destroyed/null (Destroyed). Classify with a cheap existence check.
        let exists: Option<(bool,)> =
            sqlx::query_as("SELECT destroyed FROM key_escrow WHERE account_id = $1")
                .bind(account_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| FetchError::Db(e.to_string()))?;
        return match exists {
            Some(_) => Err(FetchError::Destroyed), // row present but not live -> destroyed/null
            None => Err(FetchError::NotFound),
        };
    };

    // If this very request tripped the cap, the CASE just NULLed the key: fail closed, no eval.
    let Some(oprf_key) = oprf_key_opt else {
        return Err(FetchError::Destroyed);
    };

    let evaluated_b64 = server_evaluate(&oprf_key, blinded_b64).map_err(FetchError::Crypto)?;
    let kdf_params: KdfParams =
        serde_json::from_value(kdf_json).map_err(|e| FetchError::Crypto(e.to_string()))?;
    Ok(EscrowFetch {
        evaluated_b64,
        ciphertext,
        kdf_params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escrow_round_trip_correct_pin_recovers_wrong_pin_fails() {
        let secret = [7u8; 32];
        let pin = "246810";

        let enrolled = enroll(&secret, pin).expect("enroll");

        // Recovery with the CORRECT pin:
        let blind = client_blind(pin, &enrolled.kdf_params).expect("blind");
        let evaluated = server_evaluate(&enrolled.oprf_key, &blind.blinded_b64).expect("eval");
        let recovered = client_finalize_and_open(
            pin,
            &enrolled.kdf_params,
            &blind,
            &evaluated,
            &enrolled.ciphertext,
        )
        .expect("recover");
        assert_eq!(recovered, secret, "correct PIN must recover the exact secret");

        // Recovery with a WRONG pin must fail to decrypt:
        let bad = "000000";
        let blind2 = client_blind(bad, &enrolled.kdf_params).expect("blind2");
        let evaluated2 = server_evaluate(&enrolled.oprf_key, &blind2.blinded_b64).expect("eval2");
        let res = client_finalize_and_open(
            bad,
            &enrolled.kdf_params,
            &blind2,
            &evaluated2,
            &enrolled.ciphertext,
        );
        assert!(res.is_err(), "wrong PIN must fail AEAD decryption");
    }
}
