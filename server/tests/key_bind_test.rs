//! Integration tests for idempotent key binding (live Postgres; skips without a DB).
//!
//! Covers Task 3.1: re-binding the SAME public_key to the SAME account must be
//! idempotent (detected as already-bound, not a conflict), while the same key
//! bound to a DIFFERENT account is still a cross-account conflict.

use ainm_server::accounts;

async fn pool_or_skip() -> Option<sqlx::PgPool> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_dir).await.ok()?.run(&pool).await.ok()?;
    Some(pool)
}

async fn make_account(pool: &sqlx::PgPool, tag: &str) -> uuid::Uuid {
    let suffix = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let username = format!("kb-{tag}-{suffix}");
    let address = format!("{username}@ainm.local");
    sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, 'ainm.local', $2, $3, true, 'active') RETURNING id",
    )
    .bind(&username).bind(&address).bind(format!("hash-{address}"))
    .fetch_one(pool).await.unwrap()
}

fn fresh_pubkey() -> String {
    // Any unique string is fine for the public_key column (TEXT).
    format!("pk-{:x}", uuid::Uuid::new_v4().as_u128())
}

/// Same key re-bound to the SAME account: the repo helper reports it as already
/// bound to this account, so the handler can return 204 instead of 409.
#[tokio::test]
async fn same_key_same_account_is_already_bound() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account = make_account(&pool, "same").await;
    let pk = fresh_pubkey();

    accounts::insert_account_key(&pool, account, &pk, Some("iPhone")).await.unwrap();

    // The duplicate insert still errors with Duplicate...
    let dup = accounts::insert_account_key(&pool, account, &pk, Some("iPhone")).await;
    assert!(matches!(dup, Err(ainm_server::error::StorageError::Duplicate(_))));

    // ...but the helper distinguishes "already bound to THIS account" → true.
    assert!(accounts::key_bound_to_account(&pool, account, &pk).await.unwrap());
}

/// Same key against a DIFFERENT account: the helper reports false, so the handler
/// keeps returning 409 (cross-account conflict; the global unique index holds).
#[tokio::test]
async fn same_key_different_account_is_conflict() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account_a = make_account(&pool, "a").await;
    let account_b = make_account(&pool, "b").await;
    let pk = fresh_pubkey();

    accounts::insert_account_key(&pool, account_a, &pk, None).await.unwrap();

    // From account B's perspective the key is NOT bound to it...
    assert!(!accounts::key_bound_to_account(&pool, account_b, &pk).await.unwrap());

    // ...and trying to bind it to B is a genuine duplicate (cross-account).
    let dup = accounts::insert_account_key(&pool, account_b, &pk, None).await;
    assert!(matches!(dup, Err(ainm_server::error::StorageError::Duplicate(_))));
}

/// A key never bound to the account → helper returns false.
#[tokio::test]
async fn unknown_key_is_not_bound() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account = make_account(&pool, "unknown").await;
    assert!(!accounts::key_bound_to_account(&pool, account, &fresh_pubkey()).await.unwrap());
}

/// A revoked key for the account → helper returns false (only active bindings count).
#[tokio::test]
async fn revoked_key_is_not_bound() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account = make_account(&pool, "revoked").await;
    let pk = fresh_pubkey();
    sqlx::query("INSERT INTO account_keys (account_id, public_key, revoked_at) VALUES ($1, $2, NOW())")
        .bind(account).bind(&pk).execute(&pool).await.unwrap();
    assert!(!accounts::key_bound_to_account(&pool, account, &pk).await.unwrap());
}
