//! Hosted first-party agent: server-held identity, config, and startup provisioning.
//!
//! The agent's Ed25519 private key (a PKCS#8 document) is stored server-side. This is
//! a legitimately server-held key for OUR first-party agent — distinct from the
//! zero-knowledge human key escrow. Never log the private key bytes.

use crate::error::StorageError;
use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone, sqlx::FromRow)]
pub struct HostedAgentRow {
    // Mapped from the `RETURNING`/`SELECT` for row completeness; provisioning consumes
    // the other fields. `address` is the caller's own input so it's not re-read here.
    #[allow(dead_code)]
    pub id: uuid::Uuid,
    #[allow(dead_code)]
    pub address: String,
    pub public_key: String,
    pub private_key: Vec<u8>,
    pub system_prompt: String,
    pub enabled: bool,
}

/// Fetch the hosted agent by address, if it exists.
pub async fn find_by_address(
    pool: &PgPool,
    address: &str,
) -> Result<Option<HostedAgentRow>, StorageError> {
    let row = sqlx::query_as::<_, HostedAgentRow>(
        "SELECT id, address, public_key, private_key, system_prompt, enabled \
         FROM hosted_agents WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Insert a new hosted agent row. Caller ensures idempotency.
pub async fn insert(
    pool: &PgPool,
    address: &str,
    public_key: &str,
    private_key: &[u8],
    system_prompt: &str,
) -> Result<HostedAgentRow, StorageError> {
    let row = sqlx::query_as::<_, HostedAgentRow>(
        "INSERT INTO hosted_agents (address, public_key, private_key, system_prompt) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, address, public_key, private_key, system_prompt, enabled",
    )
    .bind(address)
    .bind(public_key)
    .bind(private_key)
    .bind(system_prompt)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            StorageError::Duplicate(format!("hosted agent {address}"))
        }
        _ => StorageError::Query(e),
    })?;
    Ok(row)
}

/// The live, in-process hosted agent: identity + signer + prompt.
#[derive(Clone)]
pub struct HostedAgent {
    pub address: String,
    pub key_pair: Arc<Ed25519KeyPair>,
    // Read by tests today; retained for callers that need the raw verify key
    // without re-deriving it from `key_pair`.
    #[allow(dead_code)]
    pub public_key_bytes: Vec<u8>,
    pub system_prompt: String,
    pub enabled: bool,
}

const DEFAULT_SYSTEM_PROMPT: &str = "You are the AINM assistant, a helpful first-party agent. \
You may use the available tools to recall what you are permitted to know about the person you are \
talking with, and to take actions on their behalf. Only use information returned by your tools. \
Be concise and useful.";

/// Idempotently provision the hosted agent for `server_domain`.
/// Safe to call on every startup; creates rows only when missing.
pub async fn provision(pool: &PgPool, server_domain: &str) -> Result<HostedAgent, StorageError> {
    let address = format!("assistant@{server_domain}");

    // (1) ensure active, verified account row
    sqlx::query(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ('assistant', $1, $2, $3, true, 'active') \
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(server_domain)
    .bind(&address)
    // email_hash is UNIQUE NOT NULL; derive a stable non-email sentinel for the agent.
    .bind(format!("hosted-agent:{address}"))
    .execute(pool)
    .await?;

    let account = crate::accounts::find_account_by_address(pool, &address)
        .await?
        .ok_or_else(|| StorageError::NotFound {
            entity: "hosted agent account",
            id: address.clone(),
        })?;

    // (3 first, so we can reuse its key for (2)) ensure hosted_agents row
    let row = match find_by_address(pool, &address).await? {
        Some(existing) => existing,
        None => {
            let rng = SystemRandom::new();
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
                .map_err(|e| StorageError::Internal(format!("keygen failed: {e}")))?;
            let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
                .map_err(|e| StorageError::Internal(format!("keypair load failed: {e}")))?;
            let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(kp.public_key().as_ref());
            insert(pool, &address, &pk_b64, pkcs8.as_ref(), DEFAULT_SYSTEM_PROMPT).await?
        }
    };

    // (2) ensure an active account_keys row binding the CURRENT agent public key
    match crate::accounts::find_active_key(pool, account.id).await? {
        Some(existing) if existing.public_key == row.public_key => {
            // Already bound and current — nothing to do.
        }
        Some(_) => {
            // The hosted_agents key and the registered handshake key are out of sync
            // (e.g. the hosted_agents row was manually rotated). v1 does not auto-rotate;
            // surface it loudly so an operator can intervene rather than silently failing auth.
            tracing::warn!(
                "[hosted-agent] account_keys public key for {address} does not match hosted_agents key — \
                 manual key rotation required; the agent may fail the handshake gate"
            );
        }
        None => {
            crate::accounts::insert_account_key(pool, account.id, &row.public_key, Some("hosted-agent"))
                .await?;
        }
    }

    // Build the live signer from stored PKCS#8 bytes.
    let key_pair = Ed25519KeyPair::from_pkcs8(&row.private_key)
        .map_err(|e| StorageError::Internal(format!("stored keypair load failed: {e}")))?;
    let public_key_bytes = key_pair.public_key().as_ref().to_vec();

    tracing::info!("[hosted-agent] provisioned {} (enabled={})", address, row.enabled);

    Ok(HostedAgent {
        address,
        key_pair: Arc::new(key_pair),
        public_key_bytes,
        system_prompt: row.system_prompt,
        enabled: row.enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_roundtrips_through_pkcs8_and_signs() {
        use ring::rand::SystemRandom;
        use ring::signature::{Ed25519KeyPair, KeyPair};

        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pkcs8_bytes = pkcs8.as_ref().to_vec();

        // Reconstruct (as provisioning does on restart from stored bytes).
        let kp = Ed25519KeyPair::from_pkcs8(&pkcs8_bytes).unwrap();
        let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(kp.public_key().as_ref());

        let agent = HostedAgent {
            address: "assistant@ainm.local".to_string(),
            key_pair: std::sync::Arc::new(kp),
            public_key_bytes: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(&pk_b64).unwrap(),
            system_prompt: "be helpful".to_string(),
            enabled: true,
        };

        // Signs an envelope and sets a signature field.
        let mut env = serde_json::json!({ "method": "ainm.message.send", "params": {} });
        let sig = crate::signing::sign_envelope(&agent.key_pair, &mut env).unwrap();
        assert!(!sig.is_empty());
        assert_eq!(env["signature"].as_str().unwrap(), sig);
    }

    #[test]
    fn hosted_agent_row_exposes_fields() {
        let row = HostedAgentRow {
            id: uuid::Uuid::nil(),
            address: "assistant@ainm.local".to_string(),
            public_key: "pk".to_string(),
            private_key: vec![1, 2, 3],
            system_prompt: "be helpful".to_string(),
            enabled: true,
        };
        assert_eq!(row.address, "assistant@ainm.local");
        assert!(row.enabled);
        assert_eq!(row.private_key, vec![1, 2, 3]);
    }
}
