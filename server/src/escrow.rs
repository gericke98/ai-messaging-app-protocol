//! Server-side escrow persistence. Pure crypto lives in the `ainm-escrow` crate
//! (shared verbatim with the iOS FFI client so recovery is wire-compatible).

// Re-exported verbatim for downstream consumers (integration tests, recovery flows). The
// server binary itself only calls `server_evaluate`; the client-side helpers are part of the
// crate's public surface, so silence the binary-target unused-import lint.
#[allow(unused_imports)]
pub use ainm_escrow::{
    client_blind, client_finalize_and_open, enroll, server_evaluate, BlindState, Enrolled,
    KdfParams,
};

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

/// Read kdf_params without touching eval_attempts (lets the client blind before guessing).
/// Returns (params, destroyed). None if no escrow row.
pub async fn get_kdf_params(
    pool: &PgPool,
    account_id: uuid::Uuid,
) -> Result<Option<(KdfParams, bool)>, StorageError> {
    let row: Option<(serde_json::Value, bool)> =
        sqlx::query_as("SELECT kdf_params, destroyed FROM key_escrow WHERE account_id = $1")
            .bind(account_id)
            .fetch_optional(pool)
            .await?;
    match row {
        Some((json, destroyed)) => {
            let kdf: KdfParams =
                serde_json::from_value(json).map_err(|e| StorageError::Duplicate(e.to_string()))?;
            Ok(Some((kdf, destroyed)))
        }
        None => Ok(None),
    }
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
