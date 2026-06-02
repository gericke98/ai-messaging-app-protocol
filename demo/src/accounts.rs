//! Account lifecycle: username validation (pure) + account/key repository queries (DB).

use crate::error::StorageError;
use sqlx::PgPool;
use sqlx::FromRow;

/// Reserved usernames that may never be registered.
const RESERVED: &[&str] = &[
    "admin", "administrator", "system", "compiler", "support",
    "root", "ainm", "server", "help", "abuse", "security", "postmaster",
];

#[derive(Debug, PartialEq, Eq)]
pub enum UsernameError {
    TooShort,
    TooLong,
    InvalidChars,
    Reserved,
}

/// Validate a username against the v1 rules: `[a-z0-9_]`, 3–30 chars, not reserved.
/// Input is lowercased before checking (usernames are case-insensitive).
pub fn validate_username(raw: &str) -> Result<String, UsernameError> {
    let name = raw.trim().to_ascii_lowercase();
    if name.len() < 3 { return Err(UsernameError::TooShort); }
    if name.len() > 30 { return Err(UsernameError::TooLong); }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(UsernameError::InvalidChars);
    }
    if RESERVED.contains(&name.as_str()) { return Err(UsernameError::Reserved); }
    Ok(name)
}

#[derive(Debug, Clone, FromRow)]
pub struct AccountRow {
    pub id: uuid::Uuid,
    // Hydrated from the DB row; read by integration tests and the iOS client, not the server binary.
    #[allow(dead_code)]
    pub username: String,
    #[allow(dead_code)]
    pub domain: String,
    pub address: String,
    #[allow(dead_code)]
    pub email_hash: String,
    #[allow(dead_code)]
    pub email_verified: bool,
    pub status: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct AccountKeyRow {
    // Hydrated from the DB row; read by integration tests and the iOS client, not the server binary.
    #[allow(dead_code)]
    pub id: uuid::Uuid,
    #[allow(dead_code)]
    pub account_id: uuid::Uuid,
    pub public_key: String,
    #[allow(dead_code)]
    pub device_label: Option<String>,
}

/// Create a `pending` account. Returns `Duplicate` if username/address/email already exist.
pub async fn create_pending_account(
    pool: &PgPool,
    username: &str,
    domain: &str,
    email_hash: &str,
) -> Result<AccountRow, StorageError> {
    let address = format!("{username}@{domain}");
    let row = sqlx::query_as::<_, AccountRow>(
        r#"
        INSERT INTO accounts (username, domain, address, email_hash, status)
        VALUES ($1, $2, $3, $4, 'pending')
        RETURNING id, username, domain, address, email_hash, email_verified, status
        "#,
    )
    .bind(username)
    .bind(domain)
    .bind(&address)
    .bind(email_hash)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            StorageError::Duplicate(format!("account {address}"))
        }
        _ => StorageError::Query(e),
    })?;
    Ok(row)
}

pub async fn find_account_by_address(
    pool: &PgPool,
    address: &str,
) -> Result<Option<AccountRow>, StorageError> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, username, domain, address, email_hash, email_verified, status \
         FROM accounts WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_account_by_email_hash(
    pool: &PgPool,
    email_hash: &str,
) -> Result<Option<AccountRow>, StorageError> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, username, domain, address, email_hash, email_verified, status \
         FROM accounts WHERE email_hash = $1",
    )
    .bind(email_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn set_email_verified(pool: &PgPool, account_id: uuid::Uuid) -> Result<(), StorageError> {
    sqlx::query("UPDATE accounts SET email_verified = true, updated_at = NOW() WHERE id = $1")
        .bind(account_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_status(
    pool: &PgPool,
    account_id: uuid::Uuid,
    status: &str,
) -> Result<(), StorageError> {
    sqlx::query("UPDATE accounts SET status = $2, updated_at = NOW() WHERE id = $1")
        .bind(account_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

/// Bind a key to an account (v1: one active key). `Duplicate` if the pubkey is already bound
/// or the account already has an active key.
pub async fn insert_account_key(
    pool: &PgPool,
    account_id: uuid::Uuid,
    public_key: &str,
    device_label: Option<&str>,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO account_keys (account_id, public_key, device_label) VALUES ($1, $2, $3)",
    )
    .bind(account_id)
    .bind(public_key)
    .bind(device_label)
    .execute(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            StorageError::Duplicate(format!("key for account {account_id}"))
        }
        _ => StorageError::Query(e),
    })?;
    Ok(())
}

/// The active (non-revoked) key for an account, if any.
pub async fn find_active_key(
    pool: &PgPool,
    account_id: uuid::Uuid,
) -> Result<Option<AccountKeyRow>, StorageError> {
    let row = sqlx::query_as::<_, AccountKeyRow>(
        "SELECT id, account_id, public_key, device_label \
         FROM account_keys WHERE account_id = $1 AND revoked_at IS NULL",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_and_normalizes_case() {
        assert_eq!(validate_username("Santi").unwrap(), "santi");
        assert_eq!(validate_username("  user_42 ").unwrap(), "user_42");
    }

    #[test]
    fn rejects_length_and_charset() {
        assert_eq!(validate_username("ab"), Err(UsernameError::TooShort));
        assert_eq!(validate_username(&"x".repeat(31)), Err(UsernameError::TooLong));
        assert_eq!(validate_username("bad name"), Err(UsernameError::InvalidChars));
        assert_eq!(validate_username("bad-name"), Err(UsernameError::InvalidChars));
    }

    #[test]
    fn rejects_reserved() {
        assert_eq!(validate_username("admin"), Err(UsernameError::Reserved));
        assert_eq!(validate_username("Compiler"), Err(UsernameError::Reserved));
    }
}
