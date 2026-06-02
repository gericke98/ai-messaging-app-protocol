//! Hosted first-party agent: server-held identity, config, and startup provisioning.
//!
//! The agent's Ed25519 private key (a PKCS#8 document) is stored server-side. This is
//! a legitimately server-held key for OUR first-party agent — distinct from the
//! zero-knowledge human key escrow. Never log the private key bytes.

use crate::error::StorageError;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HostedAgentRow {
    pub id: uuid::Uuid,
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
    .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

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
